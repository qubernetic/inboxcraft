// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Text in a script the app's fonts do not hold never stops the window (#290).
//!
//! egui draws a character no installed font holds as a box (the primary font's `.notdef`), unless the context's
//! missing-glyph policy says to panic. The app's context (eframe's) keeps egui's default, the box; `egui_kittest`'s
//! harness sets the panic by default, so a test that lays out such text must say `allow_missing_glyphs()` to see what
//! the app sees. The sweep below lays one string of each script and awkward class that mail carries, and a 10 000
//! character run of each, out in every font style of the theme at 100 % and 130 %, alone and through the whole window:
//! no panic, a finite size and place for every text, every clip rect finite and inside the window, every one-line fit
//! of an untracked style inside its room. The window is driven twice: in a context made as eframe makes the app's, and
//! in the test harness with the app's policy.
//!
//! A lone surrogate cannot occur: a Rust `str` is valid UTF-8, so no string the window draws can hold one.

use egui::{Color32, Rect, vec2};
use egui_kittest::Harness;
use inboxcraft_mime::{Mailbox as Sender, MessageBuilder};
use inboxcraft_model::{AccountId, Address, EmailAddress, Envelope, Folder, FolderId, FolderListing, FolderPath, Message, MessageId, Summary, Uid};
use inboxcraft_store::{Batch, Body, Change, Storage as _};
use inboxcraft_time::{FakeClock, Timestamp};

use crate::InboxApp;
use crate::mailbox::{Act, GroupKind, Mailbox, Place};
use crate::sample;
use crate::theme::{Fonts, TextScale, install_fonts};

/// 2026-11-05T09:00:00Z.
const NOW: i64 = 1_793_869_200;

/// How long the long run of each sample is, in characters.
const LONG: usize = 10_000;

/// One string per script mail really carries and per awkward class of character: `(what, text)`.
const SWEEP: [(&str, &str); 15] = [
    ("latin", "Allotment rota for November"),
    ("hebrew", "שלום עולם, מה שלומך?"),
    ("arabic", "مرحبا بالعالم، كيف حالك؟"),
    ("devanagari", "नमस्ते दुनिया, क्षत्रिय"),
    ("thai", "สวัสดีชาวโลก ภาษาไทย"),
    ("cjk", "你好世界 日本語のテキスト"),
    ("hangul", "안녕하세요 세계"),
    ("emoji", "👨\u{200D}👩\u{200D}👧\u{200D}👦 🏳\u{FE0F}\u{200D}🌈 ❤\u{FE0F} ✌🏽 #\u{FE0F}\u{20E3} a\u{FE0E}"),
    ("combining", "e\u{0301}\u{0302}\u{0303}\u{0304}\u{0305}\u{0306}\u{0307}\u{0308}\u{0309}\u{030A}\u{0323}\u{0324}"),
    ("private use", "\u{E000}\u{E8FF}\u{F8FF}\u{F0000}\u{10FFFD}"),
    ("unassigned", "\u{0378}\u{0530}\u{2FE0}\u{50000}\u{E0080}"),
    ("non-characters", "\u{FDD0}\u{FDEF}\u{FFFE}\u{FFFF}\u{1FFFF}\u{10FFFF}"),
    ("bidi controls", "\u{202E}gnp.exe\u{202C} \u{2067}abc\u{2069} \u{200F}\u{061C}\u{200E}\u{2066}x"),
    ("controls", "a\u{0}b\rc\nd\te\u{1B}[31m\u{7F}\u{85}"),
    ("mixed", "Re: שלום مرحبا नमस्ते 你好 안녕 👍🏽 é\u{0301}\u{E000}\u{FFFF}"),
];

/// `text` repeated to [`LONG`] characters.
fn long(text: &str) -> String {
    text.chars().cycle().take(LONG).collect()
}

/// Every string of the sweep: each sample and its long run.
fn strings() -> Vec<(String, String)> {
    SWEEP.iter().flat_map(|(what, text)| [((*what).to_owned(), (*text).to_owned()), (format!("{what}, long"), long(text))]).collect()
}

/// A context as the app's is: egui's default missing-glyph policy (the box) and the theme's fonts, past the frame that
/// installs them.
fn app_ctx() -> egui::Context {
    let ctx = egui::Context::default();
    install_fonts(&ctx);
    ctx.run_ui(egui::RawInput::default(), |_| {}).textures_delta.clear();
    ctx
}

fn finite(rect: Rect) -> bool {
    rect.min.x.is_finite() && rect.min.y.is_finite() && rect.max.x.is_finite() && rect.max.y.is_finite()
}

/// Every text shape of `output` (through nested shapes).
fn texts(output: &egui::FullOutput) -> Vec<&egui::epaint::TextShape> {
    fn walk<'a>(shape: &'a egui::Shape, found: &mut Vec<&'a egui::epaint::TextShape>) {
        match shape {
            egui::Shape::Text(text) => found.push(text),
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, found)),
            _ => {}
        }
    }
    let mut found = Vec::new();
    output.shapes.iter().for_each(|clipped| walk(&clipped.shape, &mut found));
    found
}

/// What the last frame of `output` painted, checked: every clip rect finite and, where it meets the window, inside
/// `screen`; every text's place and size finite. `what` names the step in a failure.
fn painted_inside(output: &egui::FullOutput, screen: Rect, what: &str) {
    let window = screen.expand(1.0);
    for clipped in &output.shapes {
        assert!(finite(clipped.clip_rect), "{what}: a clip rect is not finite: {:?}", clipped.clip_rect);
        let clip = clipped.clip_rect.intersect(window);
        assert!(clip.is_negative() || window.contains_rect(clip), "{what}: {clip:?}");
    }
    for text in texts(output) {
        assert!(text.pos.x.is_finite() && text.pos.y.is_finite(), "{what}: {:?}", text.pos);
        assert!(finite(text.galley.rect), "{what}: {:?} of {:?}", text.galley.rect, text.galley.job.text.chars().take(40).collect::<String>());
    }
}

#[test]
fn the_app_draws_a_box_where_the_test_harness_would_panic() {
    // The app: egui's default policy draws the primary font's `.notdef`, a visible box, in sans and mono.
    let ctx = app_ctx();
    let fonts = Fonts::of(TextScale::default());
    for font in [fonts.body, fonts.mono] {
        assert!(!ctx.fonts_mut(|f| f.has_glyph(&font.id(), '日')), "a font for Chinese came in: the sweep's premise moved");
        let mut galley = None;
        ctx.run_ui(egui::RawInput::default(), |ui| galley = Some(crate::component::galley(ui, "a日b", font, Color32::BLACK))).textures_delta.clear();
        let galley = galley.unwrap();
        let glyphs: Vec<_> = galley.rows.iter().flat_map(|row| row.glyphs.iter()).collect();
        assert_eq!(glyphs.iter().map(|glyph| glyph.chr).collect::<String>(), "a日b", "{font:?}: nothing is dropped");
        assert!(glyphs.iter().all(|glyph| !glyph.uv_rect.is_nothing() && glyph.advance_width > 0.0), "{font:?}: the box shows");
    }
    // The test harness: egui_kittest's default policy panics on the same text.
    let harness = std::panic::catch_unwind(|| {
        let mut harness = Harness::builder().with_context_setup(install_fonts).build_ui(|ui| {
            ui.label("日本語");
        });
        harness.run();
    });
    let message = harness.err().and_then(|payload| payload.downcast_ref::<String>().cloned()).unwrap_or_default();
    assert!(message.starts_with("No glyph for"), "{message:?}");
    // With the app's policy, it draws.
    let mut harness = Harness::builder().with_context_setup(install_fonts).allow_missing_glyphs().build_ui(|ui| {
        ui.label("日本語");
    });
    harness.run();
}

#[test]
fn every_script_lays_out_and_paints_in_every_font_style_at_100_and_130() {
    let ctx = app_ctx();
    let strings = strings();
    let screen = Rect::from_min_size(egui::Pos2::ZERO, vec2(800.0, 600.0));
    let room = 240.0;
    let mut over = Vec::new();
    for percent in [100, 130] {
        let scale = TextScale::from_percent(percent).unwrap();
        for font in Fonts::of(scale).all() {
            for (what, text) in &strings {
                let what = format!("{what} in {font:?} at {percent} %");
                let input = egui::RawInput { screen_rect: Some(screen), ..egui::RawInput::default() };
                let mut output = ctx.run_ui(input, |ui| {
                    let whole = crate::component::galley(ui, &crate::component::plain(text), font, Color32::BLACK);
                    assert!(whole.size().x.is_finite() && whole.size().y.is_finite(), "{what}: {:?}", whole.size());
                    let fitted = crate::component::fitted(ui, text, font, Color32::BLACK, room);
                    assert!(fitted.size().x.is_finite(), "{what}: {:?}", fitted.size());
                    // A tracked style (the label) is measured by its letters' advances without the tracking, so its
                    // cut can pass the room by the tracking of the letters kept, in any script, Latin too: a fault of
                    // the cut, not of the scripts, left for its own issue.
                    if font.tracking == 0.0 && fitted.size().x > room + 0.5 {
                        over.push(format!("{what}: {}", fitted.size().x));
                    }
                    let wrapped = crate::component::wrapped(ui, &crate::component::plain(text), font, Color32::BLACK, room, 3, egui::Align::Min);
                    assert!(wrapped.size().x <= room + 0.5 && wrapped.rows.len() <= 3, "{what}: {:?}", wrapped.size());
                    let clip = Rect::from_min_size(egui::pos2(20.0, 20.0), vec2(room, 200.0));
                    let painter = ui.painter().with_clip_rect(clip);
                    painter.galley(clip.min, whole, Color32::BLACK);
                    painter.galley(clip.min, fitted, Color32::BLACK);
                    painter.galley(clip.min, wrapped, Color32::BLACK);
                });
                painted_inside(&output, screen, &what);
                output.textures_delta.clear();
            }
        }
    }
    assert!(over.is_empty(), "{}", over.join("\n"));
}

/// The sample mailbox with a message per sweep string in the first inbox, from a sender whose name is the string, its
/// subject the long run and its body the long run in many lines; and a folder per sweep string, named by it, the last
/// holding its message instead of the inbox. Gives the store and the subjects.
fn hostile_store() -> (inboxcraft_store::Memory, Vec<String>) {
    let now = Timestamp::from_unix(NOW, 0).unwrap();
    let mut store = sample::mailbox(now).unwrap();
    let mut batch = Batch::new();
    let mut subjects = Vec::new();
    for (index, (what, text)) in SWEEP.iter().enumerate() {
        let id = 9_000 + u64::try_from(index).unwrap();
        let name: String = format!("{text} {what}").chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
        // A folder path refuses control characters; the rest of the string is the name.
        let path = FolderPath::new(&name, Some('/')).unwrap();
        batch.push(Change::PutFolder(Folder::new(FolderId::new(id), AccountId::new(1), FolderListing::new(path, Vec::new(), true))));
        let subject = format!("{what}: {}", long(text));
        let address = format!("sweep{index}@example.org");
        let line: String = long(text).chars().take(50).collect();
        let body: String = (0..200).map(|_| format!("{line}\n")).collect();
        let raw = MessageBuilder::new()
            .from(Sender::named(text, &address))
            .to(Sender::new("mira@example.org"))
            .subject(&subject)
            .text(&body)
            .build()
            .unwrap();
        let from = Address::new(Some(*text), EmailAddress::new(&address).unwrap());
        let received = Timestamp::from_unix(NOW - 60 * i64::try_from(index + 1).unwrap(), 0);
        let envelope = Envelope::new(&subject).with_from(vec![from]).with_date(received);
        let summary = Summary::new(Uid::new(u32::try_from(id).unwrap()).unwrap(), envelope).with_received(received).with_preview(Some(&body));
        let home = if index + 1 == SWEEP.len() { id } else { 1 };
        batch.push(Change::PutMessage(Message::new(MessageId::new(id), AccountId::new(1), FolderId::new(home), summary)));
        batch.push(Change::PutBody(MessageId::new(id), Body::new(raw, &body).unwrap()));
        subjects.push(subject);
    }
    store.apply(&batch).unwrap();
    (store, subjects)
}

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(NOW, 0).unwrap()))
}

/// One step of the window sweep, done to the app before its frames.
#[derive(Debug, Clone, Copy)]
enum Step {
    /// The text scale at this percentage, and the unified inbox.
    Inbox(u16),
    /// The sweep folder `id`.
    Folder(u64),
    /// Message `id` in the reader.
    Read(u64),
    /// A toast that names the sender of `id`.
    Toast(u64),
    /// A reply to `id` in the composer, filled in by the engine through `Session::run`.
    Reply(u64),
    /// The composer closed, its draft kept.
    Close,
}

/// The steps of the window sweep: at 100 % and 130 %, the inbox, then for each sweep message its reader, a toast and
/// a reply (the last one's folder first).
fn steps() -> Vec<Step> {
    let mut steps = Vec::new();
    for percent in [100, 130] {
        steps.push(Step::Inbox(percent));
        for index in 0..SWEEP.len() {
            let id = 9_000 + u64::try_from(index).unwrap();
            if index + 1 == SWEEP.len() {
                steps.push(Step::Folder(id));
            }
            steps.extend([Step::Read(id), Step::Toast(id), Step::Reply(id), Step::Close]);
        }
    }
    steps
}

/// Does `step` to `app`, through the same calls the window's buttons make.
fn apply(app: &mut InboxApp, step: Step) {
    match step {
        Step::Inbox(percent) => {
            app.set_text_scale(TextScale::from_percent(percent).unwrap());
            app.mailbox_mut().open(Place::View(inboxcraft_model::Role::Inbox));
        }
        Step::Folder(id) => app.mailbox_mut().open(Place::Folder(id)),
        Step::Read(id) => app.mailbox_mut().select(Some(id)),
        Step::Toast(id) => app.mailbox_mut().act(id, Act::Group(Some(GroupKind::Newsletters))),
        Step::Reply(id) => assert!(app.answer(id, crate::compose::Answer::Reply), "{step:?}: {:?}", app.mailbox().error()),
        Step::Close => app.close_composer(),
    }
}

/// What `step` left in `app`, checked.
fn landed(app: &InboxApp, step: Step) {
    match step {
        Step::Inbox(_) => {
            let listed = app.mailbox().messages().iter().filter(|row| row.id >= 9_000).count();
            assert_eq!(listed + 1, SWEEP.len(), "the sweep's rows are listed");
        }
        Step::Folder(id) => assert!(app.mailbox().messages().iter().any(|row| row.id == id), "{step:?}"),
        Step::Read(id) => assert_eq!(app.reader().open_id(), Some(id), "{step:?}"),
        Step::Toast(_) => assert!(app.mailbox().undoable().is_some(), "{step:?}: {:?}", app.mailbox().error()),
        Step::Reply(_) => assert!(app.composer().is_some(), "{step:?}"),
        Step::Close => assert!(app.composer().is_none(), "{step:?}: {:?}", app.mailbox().error()),
    }
}

/// The window over the sample mailbox and the sweep's mail.
fn hostile_app() -> InboxApp {
    let (store, _) = hostile_store();
    InboxApp::at_mail(Mailbox::new(sample::session_over(store, clock()).unwrap(), clock()))
}

const WINDOW: egui::Vec2 = vec2(1300.0, 820.0);

#[test]
fn the_window_as_the_app_runs_it_shows_every_script_and_keeps_the_box_policy() {
    // The context as eframe makes it, natively and on the web: `egui::Context::default()`, no policy set.
    let ctx = egui::Context::default();
    assert_eq!(ctx.missing_glyph_policy(), egui::MissingGlyphPolicy::Tofu, "eframe's context draws a box");
    let screen = Rect::from_min_size(egui::Pos2::ZERO, WINDOW);
    let mut app = hostile_app();
    let mut time = 0.0;
    let mut frames = |app: &mut InboxApp, what: &str| -> String {
        let mut drawn = String::new();
        for _ in 0..4 {
            time += 1.0 / 60.0;
            let input = egui::RawInput { screen_rect: Some(screen), time: Some(time), ..egui::RawInput::default() };
            let mut output = ctx.run_ui(input, |ui| app.ui(ui));
            painted_inside(&output, screen, what);
            drawn = texts(&output).iter().map(|text| text.galley.job.text.as_str()).collect();
            output.textures_delta.clear();
        }
        // Nothing the window does turns the policy to a panic.
        assert_eq!(ctx.missing_glyph_policy(), egui::MissingGlyphPolicy::Tofu, "{what}");
        drawn
    };
    frames(&mut app, "the first frames");
    assert!(ctx.fonts(|fonts| fonts.definitions().font_data.contains_key("IBMPlexSans-Regular")), "the app's own fonts are in");
    for step in steps() {
        apply(&mut app, step);
        let drawn = frames(&mut app, &format!("{step:?}"));
        landed(&app, step);
        if let Step::Read(id) = step {
            // The reader painted the sweep's own characters, not only the sample's.
            let (what, text) = SWEEP[usize::try_from(id - 9_000).unwrap()];
            let sign = text.chars().find(|c| !c.is_control() && !c.is_whitespace() && !('\u{202A}'..='\u{2069}').contains(c)).unwrap();
            assert!(drawn.contains(sign), "{what}: {sign:?} is painted");
        }
    }
}

#[test]
fn the_whole_window_in_the_test_harness_with_the_app_s_policy_shows_every_script() {
    let screen = Rect::from_min_size(egui::Pos2::ZERO, WINDOW);
    // `allow_missing_glyphs` is the app's policy: a box for a character no font holds (see the first test).
    let mut harness = Harness::builder()
        .with_size(WINDOW)
        .with_max_steps(12)
        .allow_missing_glyphs()
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), hostile_app());
    harness.run();
    painted_inside(harness.output(), screen, "the inbox");
    for step in steps() {
        apply(harness.state_mut(), step);
        harness.run();
        painted_inside(harness.output(), screen, &format!("{step:?}"));
        landed(harness.state(), step);
    }
}
