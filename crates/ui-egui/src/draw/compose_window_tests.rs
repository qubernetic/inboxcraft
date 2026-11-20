// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The composer in a window of its own (#379), headless: no real window ever opens. A renderer on the test's thread
//! runs the window's pass inside the main one with input of its own, as eframe does, and notes what the window was
//! built with, what it painted and where its keyboard was. A window that cannot open (egui embeds it: the web app, a
//! headless context) shows the composer docked.

use std::cell::{Cell, RefCell};

use egui::accesskit::Role;
use egui::{Context, Event, Id, Key, Modifiers, Pos2, RawInput, Rect, Shape, Vec2, ViewportEvent, ViewportInfo, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use inboxcraft_engine::Session;
use inboxcraft_engine::appearance::ComposerHost;
use inboxcraft_model::{AccountId, Host, Security, Server};
use inboxcraft_store::{Batch, Change, Storage as _};
use inboxcraft_time::{FakeClock, Timestamp};
use serde_json::{Value, json};

use super::{InboxApp, compose};
use crate::compose::Line;
use crate::keymap;
use crate::mailbox::Mailbox;
use crate::motion::Level;
use crate::sample;

/// 2026-11-05T09:00:00Z.
const NOW: i64 = 1_793_869_200;

/// What the window's last passes showed.
#[derive(Debug, Default, Clone)]
struct Seen {
    /// How many passes of the window ran.
    passes: usize,
    /// The window's title and inner size, as it was built.
    title: Option<String>,
    size: Option<Vec2>,
    /// Where the keyboard was at the end of each pass, newest last.
    focused: Vec<Option<Id>>,
    /// The texts the last pass painted.
    texts: Vec<String>,
}

thread_local! {
    /// The events of the window's next pass.
    static EVENTS: RefCell<Vec<Event>> = const { RefCell::new(Vec::new()) };
    /// The desktop asks the window to close in its next pass.
    static CLOSE: Cell<bool> = const { Cell::new(false) };
    /// The window is gone without a close (the desktop killed it): its passes do not run.
    static KILLED: Cell<bool> = const { Cell::new(false) };
    static SEEN: RefCell<Seen> = RefCell::new(Seen::default());
}

fn seen() -> Seen {
    SEEN.with(|seen| seen.borrow().clone())
}

/// Every text in `shapes`.
fn texts(shapes: &[egui::epaint::ClippedShape]) -> Vec<String> {
    fn open(shape: &Shape, out: &mut Vec<String>) {
        match shape {
            Shape::Text(text) => out.push(text.galley.text().to_owned()),
            Shape::Vec(shapes) => shapes.iter().for_each(|shape| open(shape, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    shapes.iter().for_each(|clipped| open(&clipped.shape, &mut out));
    out
}

/// Lets `ctx` open windows of their own, simulated on this thread.
fn windowing(ctx: &Context) {
    EVENTS.with(|events| events.borrow_mut().clear());
    CLOSE.set(false);
    KILLED.set(false);
    SEEN.with(|seen| *seen.borrow_mut() = Seen::default());
    Context::set_immediate_viewport_renderer(|ctx, mut viewport| {
        let this = viewport.ids.this;
        if KILLED.get() {
            // Gone without a close: the desktop shows nothing of it, and the window that opens again starts afresh,
            // its keyboard nowhere.
            let mut raw = RawInput { viewport_id: this, ..RawInput::default() };
            raw.viewports.insert(this, ViewportInfo::default());
            let mut output = ctx.run_ui(raw, |ui| {
                (viewport.viewport_ui_cb)(ui);
                if let Some(focused) = ui.ctx().memory(|memory| memory.focused()) {
                    ui.ctx().memory_mut(|memory| memory.surrender_focus(focused));
                }
            });
            output.textures_delta.clear();
            return;
        }
        let mut info = ViewportInfo::default();
        if CLOSE.replace(false) {
            info.events.push(ViewportEvent::Close);
        }
        let mut raw = RawInput { events: EVENTS.with(|events| std::mem::take(&mut *events.borrow_mut())), ..RawInput::default() };
        raw.viewport_id = this;
        // The desktop's clock, as the main window's.
        raw.time = Some(ctx.input_for(egui::ViewportId::ROOT, |input| input.time));
        raw.viewports.insert(this, info);
        raw.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, vec2(704.0, 720.0)));
        let mut focused = None;
        let mut output = ctx.run_ui(raw, |ui| {
            (viewport.viewport_ui_cb)(ui);
            focused = ui.ctx().memory(|memory| memory.focused());
        });
        output.textures_delta.clear();
        // As eframe: a window drawn inside its parent's pass asks the parent for the frame it wants.
        if let Some(asked) = output.viewport_output.get(&this).filter(|asked| asked.repaint_delay < std::time::Duration::MAX) {
            ctx.request_repaint_after_for(asked.repaint_delay, egui::ViewportId::ROOT);
        }
        SEEN.with(|seen| {
            let mut seen = seen.borrow_mut();
            seen.passes += 1;
            seen.title.clone_from(&viewport.builder.title);
            seen.size = viewport.builder.inner_size;
            seen.focused.push(focused);
            seen.texts = texts(&output.shapes);
        });
    });
    ctx.set_embed_viewports(false);
}

fn clock() -> FakeClock {
    FakeClock::new(Timestamp::from_unix(NOW, 0).unwrap())
}

/// The sample mail, its first account able to send when `sends`, the composer's host `host`; windows open when
/// `windows`.
fn app(sends: bool, host: ComposerHost, windows: bool) -> Harness<'static, InboxApp> {
    let clock = clock();
    let mut store = sample::mailbox(Timestamp::from_unix(NOW, 0).unwrap()).unwrap();
    if sends {
        let account = store.accounts().unwrap().into_iter().find(|account| account.id() == AccountId::new(1)).unwrap();
        let server = Server::new(Host::new("smtp.example.org").unwrap(), Security::Tls.submission_port(), Security::Tls);
        let mut batch = Batch::new();
        batch.push(Change::PutAccount(account.with_submission(Some(server))));
        store.apply(&batch).unwrap();
    }
    let mailbox = Mailbox::new(Session::new(Box::new(store), Box::new(clock.clone())).unwrap(), Box::new(clock));
    let mut harness = Harness::builder()
        .with_size(vec2(1300.0, 820.0))
        .with_max_steps(12)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), InboxApp::at_mail(mailbox));
    if windows {
        windowing(&harness.ctx);
    }
    harness.state_mut().set_composer_host(host);
    harness.run();
    harness
}

/// New message, as its key does.
fn new_message(harness: &mut Harness<'static, InboxApp>) {
    crate::draw::tests::new_message(harness);
    harness.run();
}

/// Writes to Odile with `subject` and `text`, kept; the draft's id.
fn write(harness: &mut Harness<'static, InboxApp>, subject: &str, text: &str) -> u64 {
    new_message(harness);
    let (Some(composer), mailbox) = harness.state_mut().composer_mut() else { panic!("no composer") };
    "Odile Brannock <odile@example.org>".clone_into(composer.typing_mut(Line::To).unwrap());
    composer.take(mailbox, Line::To).unwrap();
    subject.clone_into(composer.subject_mut());
    text.clone_into(composer.text_mut());
    composer.edited();
    composer.save(mailbox).unwrap();
    let draft = composer.draft();
    harness.run();
    draft
}

/// Writes the Composer page's quote to Odile, kept.
fn quote(harness: &mut Harness<'static, InboxApp>) {
    write(harness, "Quote for the shop front", "Two coats.");
}

/// The draft `id` as the store keeps it.
fn kept(harness: &mut Harness<'static, InboxApp>, id: u64) -> Option<Value> {
    harness.state_mut().mailbox_mut().run("compose.show", &json!({ "id": id })).ok()
}

/// Runs the frames with `events` in the window's next pass.
fn in_window(harness: &mut Harness<'static, InboxApp>, events: Vec<Event>) {
    EVENTS.with(|queued| *queued.borrow_mut() = events);
    harness.run();
}

fn key(key: Key, modifiers: Modifiers) -> Event {
    Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }
}

/// The keyboard in the main window is never on a field of the composer, which the main window does not draw.
fn main_keys_not_in_the_composer(harness: &Harness<'static, InboxApp>, when: &str) {
    let focused = harness.ctx.memory(|memory| memory.focused());
    assert!(!focused.is_some_and(compose::is_field), "{when}: the main window's keyboard is on a field it does not draw");
}

/// The keyboard of the window is somewhere once its frames settled.
fn window_keys_never_on_nothing(when: &str) {
    let seen = seen();
    assert!(seen.passes > 0, "{when}: the window drew");
    assert!(seen.focused.last().is_some_and(Option::is_some), "{when}: the window's keyboard on nothing: {:?}", seen.focused);
}

#[test]
fn the_composer_opens_in_its_window_with_the_frame_s_title_and_size_and_nothing_of_it_in_the_main_window() {
    let mut harness = app(false, ComposerHost::Window, true);
    // The main window's keyboard on a row of the list: New message leaves it there.
    harness.query_all_by_role(Role::ListBoxOption).next().unwrap().focus();
    harness.run();
    let row = harness.ctx.memory(|memory| memory.focused());
    assert!(row.is_some());
    new_message(&mut harness);
    assert_eq!(harness.ctx.memory(|memory| memory.focused()), row, "the main window's keyboard stays on its row");
    let shown = seen();
    assert!(shown.passes > 0, "the window drew");
    // `screen/composer-window/new`: 704 × 720, "New message" in the title bar.
    assert_eq!(shown.size, Some(vec2(704.0, 720.0)));
    assert_eq!(shown.title.as_deref(), Some("New message"));
    assert!(shown.texts.iter().any(|text| text == "SUBJECT"), "the composer is in the window: {:?}", shown.texts);
    assert!(!harness.state().docked() && !harness.state().composer_drawn(), "nothing of it in the main window");
    assert_eq!(compose::drawn_rect(&harness.ctx), None, "nothing of it painted in the main window");
    window_keys_never_on_nothing("opened");
    assert_eq!(shown.focused.last().copied().flatten(), Some(compose::field_id(Line::To)), "the keys in To");
    main_keys_not_in_the_composer(&harness, "opened");
    // The subject is the title.
    "Kiln rota".clone_into(harness.state_mut().composer_mut().0.unwrap().subject_mut());
    harness.run();
    assert_eq!(seen().title.as_deref(), Some("Kiln rota"));
}

#[test]
fn where_no_window_can_open_the_composer_shows_docked_and_the_status_line_says_why_once() {
    // A context that embeds its viewports: the web app's, a headless one.
    let mut harness = app(false, ComposerHost::Window, false);
    new_message(&mut harness);
    assert!(harness.state().docked(), "docked");
    harness.get_by_role_and_label(Role::TextInput, "Subject");
    harness.get_by_label_contains("No window of its own can open here");
    harness.state_mut().close_composer();
    harness.state_mut().status.forget_notices();
    harness.run();
    new_message(&mut harness);
    assert!(harness.state().docked());
    assert!(harness.query_by_label_contains("No window of its own can open here").is_none(), "said once");
}

#[test]
fn a_1_mib_draft_in_its_window_survives_every_way_out_and_sends_once_with_undo_in_the_main_window() {
    let mut harness = app(true, ComposerHost::Window, true);
    let text: String = "Bring gloves and the long hose.\n".repeat(1 << 15);
    assert_eq!(text.len(), 1 << 20);
    let draft = write(&mut harness, "Plot notes", &text);
    window_keys_never_on_nothing("written");
    // The desktop's close: the composer closes, the draft kept whole on the disk.
    CLOSE.set(true);
    harness.run();
    assert!(harness.state().composer().is_none(), "the desktop's close closes it");
    assert_eq!(kept(&mut harness, draft).unwrap()["text"].as_str().map(str::len), Some(text.len()), "kept whole");
    main_keys_not_in_the_composer(&harness, "closed");
    // Open again, then the window is gone without a close: the composer stays, all of it, and its window comes back.
    harness.state_mut().reopen(draft);
    harness.run();
    KILLED.set(true);
    for _ in 0..3 {
        harness.step();
    }
    assert_eq!(harness.state().composer().map(|composer| composer.text().len()), Some(text.len()), "nothing lost");
    KILLED.set(false);
    let before = seen().passes;
    harness.run();
    assert!(seen().passes > before, "the window opens again");
    // The motion level changes under it.
    for level in [Level::Full, Level::Off, Level::Reduced] {
        harness.state_mut().set_motion_level(level);
        harness.run();
        assert_eq!(harness.state().composer().map(|composer| composer.text().len()), Some(text.len()), "{level:?}");
    }
    window_keys_never_on_nothing("reopened");
    // Send from the window, by its key: one send, its countdown and Undo in the main window's status line.
    in_window(&mut harness, vec![key(Key::Enter, Modifiers::COMMAND)]);
    assert!(harness.state().composer().is_none(), "sent");
    assert_eq!(harness.state().mailbox().outbox().len(), 1, "one send");
    harness.get_by_role_and_label(Role::Button, "undo").click();
    harness.run();
    assert!(harness.state().mailbox().outbox().is_empty(), "taken back");
    let composer = harness.state().composer().expect("Undo opens the draft again");
    assert_eq!((composer.draft(), composer.text().len()), (draft, text.len()));
    let before = seen().passes;
    harness.run();
    assert!(seen().passes > before, "in its window again");
    main_keys_not_in_the_composer(&harness, "undone");
    // Sent again: still one.
    in_window(&mut harness, vec![key(Key::Enter, Modifiers::COMMAND)]);
    assert_eq!(harness.state().mailbox().outbox().len(), 1, "one send");
    assert!(harness.state().composer().is_none());
}

#[test]
fn esc_in_the_window_closes_the_composer_as_its_close_does() {
    let mut harness = app(false, ComposerHost::Window, true);
    let draft = write(&mut harness, "Kiln rota", "Friday at ten.");
    "Kiln rota for Friday".clone_into(harness.state_mut().composer_mut().0.unwrap().subject_mut());
    harness.state_mut().composer_mut().0.unwrap().edited();
    in_window(&mut harness, vec![key(Key::Escape, Modifiers::NONE)]);
    assert!(harness.state().composer().is_none(), "one Esc closes it");
    assert_eq!(kept(&mut harness, draft).unwrap()["subject"], "Kiln rota for Friday", "the newest words kept");
    harness.get_by_label("Draft saved");
}

#[test]
fn the_keyboard_walks_the_composer_in_its_window_and_never_leaves_it() {
    let mut harness = app(false, ComposerHost::Window, true);
    write(&mut harness, "Kiln rota", "Friday at ten.");
    let main = harness.ctx.memory(|memory| memory.focused());
    let mut walked = Vec::new();
    for modifiers in [Modifiers::NONE, Modifiers::SHIFT] {
        for _ in 0..16 {
            in_window(&mut harness, vec![key(Key::Tab, modifiers)]);
            walked.push(seen().focused.last().copied().flatten());
        }
    }
    assert!(walked.iter().all(Option::is_some), "the window's keyboard on nothing: {walked:?}");
    for field in [compose::field_id(Line::To), compose::subject_id(), compose::text_id()] {
        assert!(walked.contains(&Some(field)), "{field:?} reached: {walked:?}");
    }
    assert_eq!(harness.ctx.memory(|memory| memory.focused()), main, "the main window's keyboard stays where it was");
    assert!(harness.state().composer().is_some(), "Tab closes nothing");
}

#[test]
fn in_the_composer_s_window_no_key_of_the_main_window_acts_and_the_look_keys_do() {
    let mut harness = app(false, ComposerHost::Window, true);
    write(&mut harness, "Kiln rota", "Friday at ten.");
    let state = |harness: &Harness<'static, InboxApp>| {
        let app = harness.state();
        (app.screen(), app.mailbox().selected(), app.mailbox().inbox_view(), app.command().results(), app.settings.is_open(), app.mailbox().place())
    };
    let before = state(&harness);
    let keymap = harness.state().keymap().clone();
    for action in keymap::actions() {
        if action.id == keymap::SEND || crate::surface::WINDOW_KEYS.contains(&action.id) {
            continue;
        }
        // Esc is the window's own: it closes the window (its own test).
        let Some(shortcut) = keymap.shortcut(action.id).filter(|shortcut| shortcut.logical_key != Key::Escape) else { continue };
        in_window(&mut harness, vec![key(shortcut.logical_key, shortcut.modifiers)]);
        assert_eq!(state(&harness), before, "{}: nothing in the main window changed", action.id);
        assert!(harness.state().composer().is_some(), "{}: the composer stays", action.id);
    }
    // The look of the app is the window's too.
    let size = harness.state().text_scale();
    in_window(&mut harness, vec![key(Key::Plus, Modifiers::COMMAND)]);
    assert!(harness.state().text_scale().percent() > size.percent(), "the text grows from the window");
}

#[test]
fn the_desktop_s_grab_of_the_composer_s_window_gives_the_release_to_that_window() {
    let mut harness = app(false, ComposerHost::Window, true);
    new_message(&mut harness);
    let window = super::composer_window();
    // A press on the window's title bar, away from its controls, then a drag: the desktop moves the window.
    let at = Pos2::new(300.0, 22.0);
    in_window(
        &mut harness,
        vec![
            Event::PointerMoved(at),
            Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE },
        ],
    );
    in_window(&mut harness, vec![Event::PointerMoved(at + vec2(30.0, 0.0))]);
    // The desktop kept the release: the window's next pass hears the button come up, the main window's never.
    in_window(&mut harness, vec![]);
    assert!(!harness.ctx.input_for(window, |input| input.pointer.any_down()), "the window's button counts as up");
    assert!(!harness.ctx.input(|input| input.pointer.any_released()), "the main window heard no release");
}

/// The Composer page's window states, each drawn in the window: its title and what tells the state.
#[test]
fn every_window_state_of_the_composer_page_draws_in_its_window() {
    type Setup = fn(&mut Harness<'static, InboxApp>);
    let states: [(&str, bool, Setup, &str, &[&str]); 7] = [
        ("new", true, |harness| new_message(harness), "New message", &["FROM", "TO", "SUBJECT", "from Personal", "send"]),
        ("written", true, |harness| quote(harness), "Quote for the shop front", &["draft kept 09:00", "from Personal", "Two coats."]),
        (
            "bad-address",
            true,
            |harness| {
                quote(harness);
                let (Some(composer), mailbox) = harness.state_mut().composer_mut() else { panic!() };
                "ansel.example.org,".clone_into(composer.typing_mut(Line::To).unwrap());
                composer.typed(mailbox, Line::To).unwrap();
                harness.run();
            },
            "Quote for the shop front",
            &["ansel.example.org"],
        ),
        (
            "suggestions-open",
            true,
            |harness| {
                new_message(harness);
                let (Some(composer), mailbox) = harness.state_mut().composer_mut() else { panic!() };
                "te".clone_into(composer.typing_mut(Line::To).unwrap());
                composer.typed(mailbox, Line::To).unwrap();
                composer.step(true);
                harness.run();
            },
            "New message",
            &["Teodor Lisk"],
        ),
        (
            "reply-quote",
            true,
            |harness| {
                let id = harness.state().mailbox().messages().iter().find(|row| row.subject == "Allotment rota for November").map(|row| row.id);
                assert!(harness.state_mut().answer(id.unwrap(), crate::compose::Answer::Reply));
                harness.run();
            },
            "Re: Allotment rota for November",
            &["TO"],
        ),
        (
            "scheduled",
            true,
            |harness| {
                quote(harness);
                harness.state_mut().composer_mut().0.unwrap().set_later(Some(Timestamp::from_unix(NOW + 86_400, 0).unwrap()));
                harness.run();
            },
            "Quote for the shop front",
            &["schedule"],
        ),
        ("problem", false, |harness| quote(harness), "Quote for the shop front", &["Two coats."]),
    ];
    for (name, sends, setup, title, shown) in states {
        let mut harness = app(sends, ComposerHost::Window, true);
        setup(&mut harness);
        let seen = seen();
        assert_eq!(seen.title.as_deref(), Some(title), "{name}");
        for text in shown {
            assert!(seen.texts.iter().any(|painted| painted.contains(text)), "{name}: {text:?} not in {:?}", seen.texts);
        }
        window_keys_never_on_nothing(name);
        main_keys_not_in_the_composer(&harness, name);
    }
}

#[test]
fn the_floating_host_opens_a_new_composer_floating_and_its_dock_still_docks_it() {
    let mut harness = app(false, ComposerHost::Floating, true);
    new_message(&mut harness);
    assert!(harness.state().composer().is_some_and(crate::compose::Composer::floating), "floating");
    assert_eq!(seen().passes, 0, "no window");
    harness.get_by_role_and_label(Role::Button, "Back to the reader's place").click();
    harness.run();
    assert!(harness.state().docked(), "docked by its own button");
}

#[test]
fn the_layout_row_moves_the_open_composer_into_its_window_and_back() {
    let mut harness = app(false, ComposerHost::Docked, true);
    let draft = write(&mut harness, "Kiln rota", "Friday at ten.");
    assert!(harness.state().docked() && seen().passes == 0);
    harness.state_mut().set_composer_host(ComposerHost::Window);
    harness.run();
    assert!(!harness.state().docked() && seen().passes > 0, "in its window");
    harness.state_mut().set_composer_host(ComposerHost::Docked);
    harness.run();
    assert!(harness.state().docked(), "back in the reader's place");
    assert_eq!(harness.state().composer().map(|composer| (composer.draft(), composer.text().to_owned())), Some((draft, "Friday at ten.".to_owned())));
}

#[test]
fn typing_in_the_window_keeps_the_draft_at_most_once_a_while_not_at_every_key() {
    let mut harness = app(false, ComposerHost::Window, true);
    write(&mut harness, "Kiln rota", "Friday at ten.");
    harness.state_mut().window_focus = Some(compose::subject_id());
    harness.run();
    assert_eq!(seen().focused.last().copied().flatten(), Some(compose::subject_id()), "the keys in the subject");
    let mut waited = Vec::new();
    for typed in ["!", "?", "!", "?", "!"] {
        EVENTS.with(|queued| *queued.borrow_mut() = vec![Event::Text(typed.to_owned())]);
        harness.step();
        waited.push(harness.state().composer().is_some_and(crate::compose::Composer::changed));
    }
    assert!(harness.state().composer().unwrap().subject().ends_with("!?!?!"), "typed in the window");
    // A key typed soon after a keep waits for the next one: not every key is a write.
    assert!(waited.contains(&true), "kept at every key: {waited:?}");
}
