// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `ui_bench`: how long the sample window takes to build a frame, over the sample mailbox grown to 10 000 invented
//! messages, of which the list shows the engine's `MAX_LIST` (1 000). Four scenes of 300 frames each:
//! `inbox-default-rest` (the inbox in the view a new owner sees, today the cards), `inbox-list-rest` and
//! `inbox-list-scroll` (the grouped list, every listed row laid out, scrolling 40 points a frame: the stress case)
//! and `reader-html` (the reader open on an HTML message beside the default view); with `--large`, `reader-large-rest`
//! and `reader-large-scroll` instead: the reader open on an HTML message of two megabytes (as large as the cleaner
//! takes), left alone and scrolling 40 points a frame; then `reader-paragraph-*`, a message of one paragraph of two
//! megabytes, and `reader-links-*`, a message of one paragraph of as many links as the cleaner keeps (10 000), each at
//! rest and scrolling. A frame is the window's `ui` and the tessellation of what it
//! painted, on the CPU;
//! nothing is drawn on a GPU. Per scene it prints the median and the 95th percentile in milliseconds, the shapes and
//! the list rows the last frame built, and the budget's verdict: p95 at most 4 ms, or with `--pinned` (run under
//! `taskset -c 0` by `just bench-ui`) at most 16.6 ms, one frame at 60 Hz. A scene that misses either is a miss. It
//! also prints the CPU's model, its core count and the first core's frequency governor, so numbers from two machines
//! are never mixed up. The verdict never fails the run.
//!
//! With `--compose` it runs the composer's scenes instead: the composer open with a message text of 16 KiB, 64 KiB,
//! 256 KiB and 1 MiB (just under the composer's cap, so typing fits), its text focused, at rest and typing one
//! character a frame at its start, each without AccessKit and with it (`-ak`, as while a screen reader listens). Every
//! scene also prints its slowest frame (`max_ms`); a typing scene keeps its draft once a second, one frame in sixty that
//! the p95 does not see, so its verdict holds the slowest frame to the budget as well. The sample's store is in memory:
//! a keep on the SQLite store also waits for the disk, which no scene measures.
//!
//! With `--calendar` it runs the calendar's scenes instead: `calendar-week-200`, the week view holding 200 invented
//! events (overlapping, over whole days), left alone; and `calendar-week-5000`, `calendar-month-5000` and
//! `calendar-agenda-5000`, each view holding 5 000 invented events put in place of what was read (more than the
//! engine lists at once): hundreds at one hour, hundreds over the same days, the rest spread out.
//!
//! With `--settings` it runs the settings sheet's scenes instead, each at rest: Appearance, Shortcuts and Sending
//! over the sample's accounts, and Sending and Notifications with 300 accounts more (a signature field, or a row, an
//! account).
//!
//! With `--field` it runs the character field's scenes instead: `field-window-sweep`, a window of the field with an
//! emblem on its right half, Home's field at the full motion level, the pointer sweeping across it every frame; and
//! `field-4k-sweep`, the same over a 4K screen (3 840 × 2 160 points, some 69 000 cells).
//!
//! `home-window-sweep`, among the four scenes above and alone with `--home`: the window on Home, the start place, at the
//! full motion level, the pointer sweeping across its character field every frame.
//!
//! ```text
//! cargo run --release -p inboxcraft-ui-egui --example ui_bench [-- --pinned] [--compose | --calendar | --settings | --large | --field | --home]
//! ```

use std::process::ExitCode;

use egui::{Event, Modifiers, MouseWheelSource, MouseWheelUnit, Pos2, RawInput, Rect, Shape, TouchPhase, Vec2, pos2, vec2};
use inboxcraft_engine::MAX_LIST;
use inboxcraft_time::{Clock, FakeClock, SystemClock, Timestamp};
use inboxcraft_ui_egui::calendar::{Day, EventRow, Moment, View};
use inboxcraft_ui_egui::mailbox::{InboxView, Mailbox};
use inboxcraft_ui_egui::save::NoSave;
use inboxcraft_ui_egui::settings::Section;
use inboxcraft_ui_egui::{InboxApp, UiError, rows_built, sample};

/// The messages in the grown sample.
const MESSAGES: usize = 10_000;
/// The frames each scene runs, after its warm-up.
const FRAMES: usize = 300;
/// Frames run before the measuring starts, so the first layout and the font atlas are not counted.
const WARM_UP: usize = 30;
/// The window, in points.
const WINDOW: Vec2 = vec2(1280.0, 800.0);
/// The scroll of one frame, in points.
const SCROLL: f32 = 40.0;
/// The budget for the 95th percentile, in milliseconds, on the measuring machine with every core free.
const BUDGET_MS: f64 = 4.0;
/// The budget for the 95th percentile pinned to one core: one frame at 60 Hz.
const PINNED_BUDGET_MS: f64 = 16.6;
/// The time the sample is shown at: 2026-11-05T09:00:00Z, as `ui_shot` shows it.
const TIME: i64 = 1_793_869_200;
/// The accounts added to the sample's in the settings scenes that show one row or field an account.
const MORE_ACCOUNTS: usize = 300;

/// The HTML message the reader opens.
const HTML_SUBJECT: &str = "Your bike is due for a service";

/// The subject of the HTML message of two megabytes the large scenes open.
const LARGE_SUBJECT: &str = "The allotment minutes, every one of them";

/// What a large message is made of.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Large {
    /// Paragraphs with links and bold words, a table now and then and an open quote at its end.
    Paragraphs,
    /// One paragraph of two megabytes.
    Paragraph,
    /// One paragraph of as many links as the cleaner keeps.
    Links,
}

impl Large {
    const ALL: [Self; 3] = [Self::Paragraphs, Self::Paragraph, Self::Links];

    fn name(self) -> &'static str {
        match self {
            Self::Paragraphs => "large",
            Self::Paragraph => "paragraph",
            Self::Links => "links",
        }
    }

    /// The message's HTML, just under the cleaner's input cap.
    fn html(self) -> String {
        let room = inboxcraft_html::MAX_INPUT - 4096;
        match self {
            Self::Paragraphs => {
                let paragraph = "<p>The minutes of the meeting, read again for those who were not there: the <b>water butts</b> are \
                                 emptied before the frost, and <a href=\"https://allotments.example.org/rota\">the rota</a> is on the site.</p>";
                let table = "<table><tr><th>Week</th><th>Who</th><th>Plot</th></tr><tr><td>1</td><td>Odile Brannock</td><td>14</td></tr></table>";
                let mut html = String::new();
                while html.len() < room {
                    html.push_str(paragraph);
                    if html.len() % 40 < 2 {
                        html.push_str(table);
                    }
                }
                html.push_str("<blockquote><p>The earlier minutes.</p></blockquote>");
                html
            }
            Self::Paragraph => {
                let words = "The minutes of the allotment meeting, read again. ";
                format!("<p>{}</p>", words.repeat(room / words.len()))
            }
            Self::Links => format!("<p>{}</p>", "<a href=\"https://allotments.example.org/rota\">rota</a> ".repeat(inboxcraft_html::MAX_LINKS)),
        }
    }
}

/// Puts into `store` an HTML message of two megabytes made as `large` says.
fn put_large(store: &mut inboxcraft_store::Memory, time: Timestamp, large: Large) -> Result<(), UiError> {
    use inboxcraft_mime::{Mailbox as Sender, MessageBuilder};
    use inboxcraft_model::{AccountId, Address, EmailAddress, Envelope, FolderId, Message, MessageId, Summary, Uid};
    use inboxcraft_store::{Batch, Body, Change, Storage as _};
    let bad = |error: &dyn std::fmt::Display| UiError::Shot(error.to_string());
    let html = large.html();
    let id = 99_000_u64;
    let raw = MessageBuilder::new()
        .from(Sender::named("Odile Brannock", "odile@example.org"))
        .to(Sender::new("mira@example.org"))
        .subject(LARGE_SUBJECT)
        .text("The minutes.")
        .html(&html)
        .build()
        .map_err(|error| bad(&error))?;
    let from = Address::new(Some("Odile Brannock"), EmailAddress::new("odile@example.org").map_err(|error| bad(&error))?);
    let envelope = Envelope::new(LARGE_SUBJECT).with_from(vec![from]).with_date(Some(time));
    let uid = Uid::new(99_000).ok_or_else(|| UiError::Shot("no uid".to_owned()))?;
    let summary = Summary::new(uid, envelope).with_received(Some(time));
    let message = Message::new(MessageId::new(id), AccountId::new(1), FolderId::new(1), summary);
    let body = Body::new(raw, "The minutes.").map_err(|error| bad(&error))?;
    let batch = Batch::new().with(Change::PutMessage(message)).with(Change::PutBody(MessageId::new(id), body));
    store.apply(&batch).map_err(|error| bad(&error))
}

/// The sizes of the composer's message text its scenes measure, in KiB.
const BODIES_KIB: [usize; 4] = [16, 64, 256, 1024];

/// A scene: what it shows and what it does in each frame.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Scene {
    /// The inbox in the view a new owner sees (today the cards), left alone.
    DefaultRest,
    /// The inbox as the grouped list, every listed row laid out: the stress case, left alone.
    ListRest,
    /// The grouped list scrolling 40 points a frame.
    ListScroll,
    /// The reader open on an HTML message, the inbox in the default view beside it.
    ReaderHtml,
    /// The reader open on an HTML message of two megabytes made as `large` says; `scrolling` 40 points a frame.
    ReaderLarge { large: Large, scrolling: bool },
    /// The composer with a message text of `kib` KiB, focused; `typing` a character a frame; `accesskit` on.
    Compose { kib: usize, typing: bool, accesskit: bool },
    /// The calendar's week holding [`WEEK_EVENTS`] events, left alone.
    CalendarWeek,
    /// The calendar's `view` holding [`CROWD_EVENTS`] events, left alone.
    CalendarCrowd(View),
    /// The settings sheet open on `section`, `more` accounts added to the sample's, left alone.
    Settings { section: Section, more: usize },
    /// Home in the window at the full motion level, the pointer sweeping across its field.
    HomeSweep,
}

impl Scene {
    const ALL: [Self; 5] = [Self::DefaultRest, Self::ListRest, Self::ListScroll, Self::ReaderHtml, Self::HomeSweep];

    /// The composer's scenes, every size at rest and typing, without AccessKit and with it.
    fn compose() -> Vec<Self> {
        let mut scenes = Vec::new();
        for accesskit in [false, true] {
            for kib in BODIES_KIB {
                for typing in [false, true] {
                    scenes.push(Self::Compose { kib, typing, accesskit });
                }
            }
        }
        scenes
    }

    /// The settings sheet's scenes.
    fn settings() -> Vec<Self> {
        vec![
            Self::Settings { section: Section::Appearance, more: 0 },
            Self::Settings { section: Section::Shortcuts, more: 0 },
            Self::Settings { section: Section::Sending, more: 0 },
            Self::Settings { section: Section::Sending, more: MORE_ACCOUNTS },
            Self::Settings { section: Section::Notifications, more: MORE_ACCOUNTS },
        ]
    }

    fn name(self) -> String {
        match self {
            Self::DefaultRest => "inbox-default-rest".to_owned(),
            Self::ListRest => "inbox-list-rest".to_owned(),
            Self::ListScroll => "inbox-list-scroll".to_owned(),
            Self::ReaderHtml => "reader-html".to_owned(),
            Self::HomeSweep => "home-window-sweep".to_owned(),
            Self::CalendarWeek => "calendar-week-200".to_owned(),
            Self::CalendarCrowd(view) => format!("calendar-{}-{CROWD_EVENTS}", format!("{view:?}").to_lowercase()),
            Self::ReaderLarge { large, scrolling } => format!("reader-{}-{}", large.name(), if scrolling { "scroll" } else { "rest" }),
            Self::Compose { kib, typing, accesskit } => {
                format!("compose-{kib}k-{}{}", if typing { "typing" } else { "rest" }, if accesskit { "-ak" } else { "" })
            }
            Self::Settings { section, more } => {
                let more = if more > 0 { format!("-{more}more") } else { String::new() };
                format!("settings-{}{more}-rest", section.id())
            }
        }
    }

    /// The inbox view it sets; `None` keeps the default.
    fn view(self) -> Option<InboxView> {
        match self {
            Self::ListRest | Self::ListScroll => Some(InboxView::Grouped),
            Self::DefaultRest
            | Self::ReaderHtml
            | Self::ReaderLarge { .. }
            | Self::Compose { .. }
            | Self::CalendarWeek
            | Self::CalendarCrowd(_)
            | Self::Settings { .. }
            | Self::HomeSweep => None,
        }
    }
}

/// What one scene measured.
struct Measured {
    median_ms: f64,
    p95_ms: f64,
    /// The slowest frame: in a typing scene, one that keeps the draft.
    max_ms: f64,
    shapes: usize,
    rows_built: Option<usize>,
}

/// The shapes of `shape`, nested lists opened.
fn count(shape: &Shape) -> usize {
    match shape {
        Shape::Vec(shapes) => shapes.iter().map(count).sum(),
        _ => 1,
    }
}

/// The sample window over the grown mailbox, reduced motion on, as `scene` starts.
fn app(scene: Scene) -> Result<InboxApp, UiError> {
    let time = Timestamp::from_unix(TIME, 0).ok_or_else(|| UiError::Shot("no time".to_owned()))?;
    let clock = FakeClock::new(time);
    let mut store = sample::grown(time, MESSAGES)?;
    if let Scene::ReaderLarge { large, .. } = scene {
        put_large(&mut store, time, large)?;
    }
    let mut session = sample::session_over(store, Box::new(clock.clone()))?;
    if scene == Scene::CalendarWeek {
        week_of_events(&mut session)?;
    }
    if let Scene::Settings { more, .. } = scene {
        for n in 0..more {
            let account = serde_json::json!({
                "address": format!("bench{n}@example.org"),
                "label": format!("Bench account {n}"),
                "imap": { "host": "imap.example.org", "security": "tls" },
            });
            session.run("account.add", &account).map_err(UiError::Engine)?;
        }
    }
    let mailbox = Mailbox::new(session, Box::new(clock));
    let mut app = InboxApp::with_outside(mailbox, Box::new(sample::Pictures::default()), Box::new(NoSave));
    app.set_reduce_motion(true);
    if scene == Scene::HomeSweep {
        app.set_reduce_motion(false);
        app.show_home();
    } else {
        app.show_mail();
    }
    if let Some(view) = scene.view() {
        app.mailbox_mut().set_inbox_view(view);
    }
    if let Scene::Settings { section, .. } = scene {
        app.settings_mut().open_at(section);
    }
    if scene == Scene::ReaderHtml {
        let id = app.mailbox().messages().iter().find(|row| row.subject == HTML_SUBJECT).map(|row| row.id);
        let Some(id) = id else { return Err(UiError::Shot(format!("no row {HTML_SUBJECT:?}"))) };
        app.mailbox_mut().select(Some(id));
    }
    if scene == Scene::CalendarWeek {
        app.show_calendar();
        let shown = app.calendar().events().count();
        if shown < WEEK_EVENTS {
            return Err(UiError::Shot(format!("the week shows {shown} events, not {WEEK_EVENTS}")));
        }
    }
    if let Scene::CalendarCrowd(view) = scene {
        app.show_calendar();
        let (calendar, mailbox) = app.calendar_mut();
        calendar.set_view(mailbox, view);
        calendar.set_events(crowd()?);
    }
    if let Scene::ReaderLarge { .. } = scene {
        let id = app.mailbox().messages().iter().find(|row| row.subject == LARGE_SUBJECT).map(|row| row.id);
        let Some(id) = id else { return Err(UiError::Shot(format!("no row {LARGE_SUBJECT:?}"))) };
        app.mailbox_mut().select(Some(id));
    }
    if let Scene::Compose { kib, .. } = scene {
        app.compose();
        // Lines of a letter, just under the composer's cap at 1 MiB so that the typing fits.
        let size = (kib << 10).min(inboxcraft_ui_egui::component::field::MAX_BODY - 4096);
        let line = "We meet at the plot at ten; bring gloves, the long rake and something to drink.\n";
        let mut text = line.repeat(size / line.len() + 1);
        text.truncate(size);
        match app.composer_mut() {
            (Some(composer), _) => *composer.text_mut() = text,
            (None, _) => return Err(UiError::Shot("no composer".to_owned())),
        }
    }
    Ok(app)
}

/// The events of the calendar's scene, in the week the sample shows.
const WEEK_EVENTS: usize = 200;

/// Puts [`WEEK_EVENTS`] invented events into the sample's first calendar, in the week of 2 November 2026: every day,
/// every hour from 07:00, some overlapping, some over whole days.
fn week_of_events(session: &mut inboxcraft_engine::Session) -> Result<(), UiError> {
    let failed = |error: inboxcraft_engine::EngineError| UiError::Shot(format!("event.create: {error}"));
    let calendars = session.run("calendar.list", &serde_json::json!({})).map_err(failed)?;
    let calendar = calendars["calendars"][0]["id"].as_u64().ok_or_else(|| UiError::Shot("no calendar".to_owned()))?;
    for index in 0..WEEK_EVENTS {
        let day = 2 + index % 7;
        let hour = 7 + (index / 7) % 14;
        let all_day = index % 25 == 0;
        let (start, end) = if all_day {
            (format!("2026-11-{day:02}"), format!("2026-11-{:02}", day + 1))
        } else {
            (format!("2026-11-{day:02}T{hour:02}:{:02}:00Z", (index % 3) * 15), format!("2026-11-{day:02}T{:02}:30:00Z", hour + 1))
        };
        let params = serde_json::json!({
            "calendar": calendar, "summary": format!("Invented event {index} at the yard"), "start": start, "end": end,
            "all_day": all_day, "busy": true, "private": false, "repeat": "none", "guests": [],
        });
        session.run("event.create", &params).map_err(failed)?;
    }
    Ok(())
}

/// The events of the crowded scenes.
const CROWD_EVENTS: usize = 5_000;

/// [`CROWD_EVENTS`] invented events from 2 November 2026 on: a fifth at 09:00 on Thursday 5 November, a tenth all day
/// over 4 to 6 November, the rest spread over 42 days at every hour.
fn crowd() -> Result<Vec<EventRow>, UiError> {
    let day = |date: i64| Day::new(2026, 11, 2).map(|first| first.plus(date)).ok_or_else(|| UiError::Shot("no day".to_owned()));
    let mut events = Vec::with_capacity(CROWD_EVENTS);
    for index in 0..CROWD_EVENTS {
        let (first, last, minute, all_day) = match index % 10 {
            0 | 1 => (day(3)?, day(3)?, 540, false),
            2 => (day(2)?, day(4)?, 0, true),
            _ => {
                let at = day(i64::try_from(index % 42).unwrap_or(0))?;
                (at, at, u16::try_from(index % 24).unwrap_or(0) * 60, false)
            }
        };
        let end = if all_day { Moment { day: last.plus(1), minute: 0 } } else { Moment { day: first, minute: minute.saturating_add(45) } };
        events.push(EventRow {
            id: 100_000 + u64::try_from(index).unwrap_or(0),
            calendar: 1,
            summary: format!("Invented event {index} at the yard"),
            location: "The yard".to_owned(),
            description: String::new(),
            start: Moment { day: first, minute },
            end,
            first,
            last,
            all_day,
            busy: true,
            private: false,
            repeat: None,
            zone: None,
            guests: Vec::new(),
            untitled: false,
        });
    }
    events.sort_by_key(|event| (event.start.day, event.start.minute));
    Ok(events)
}

/// Where the composer's message text is, from the AccessKit tree of a frame (in points: the bench runs at 1 pixel a
/// point).
fn message_text(output: &egui::FullOutput) -> Option<Pos2> {
    let update = output.platform_output.accesskit_update.as_ref()?;
    update.nodes.iter().find_map(|(_, node)| {
        (node.label() == Some("Message text")).then(|| node.bounds()).flatten().map(|bounds| pos2(bounds.x0 as f32 + 8.0, bounds.y0 as f32 + 8.0))
    })
}

/// The value at `share` (0 to 1) of sorted `values`, nearest rank.
fn percentile(values: &[f64], share: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let last = values.len().saturating_sub(1);
    // The rank lies in 0..=last; the cast back is exact for any frame count this bench runs.
    let rank = (share * last as f64).round() as usize;
    values.get(rank.min(last)).copied().unwrap_or(0.0)
}

/// Runs `scene` for its warm-up and [`FRAMES`] frames.
fn measure(scene: Scene) -> Result<Measured, UiError> {
    let mut app = app(scene)?;
    let ctx = egui::Context::default();
    let screen = Rect::from_min_size(Pos2::ZERO, WINDOW);
    // The composer's text is found by its name and pressed, so it has the focus; then AccessKit stays on only where
    // the scene asks for it.
    let mut press = None;
    if let Scene::Compose { .. } = scene {
        ctx.enable_accesskit();
        let input = RawInput { screen_rect: Some(screen), ..RawInput::default() };
        let output = ctx.run_ui(input, |ui| app.ui(ui));
        press = Some(message_text(&output).ok_or_else(|| UiError::Shot("no message text".to_owned()))?);
    }
    let before = app.composer().map(|composer| composer.text().len());
    // Over the list, left of the reader and right of the folders; for the large message, over its text.
    let over_list = if let Scene::ReaderLarge { .. } = scene { pos2(WINDOW.x * 0.8, WINDOW.y * 0.6) } else { pos2(WINDOW.x * 0.35, WINDOW.y * 0.6) };
    let clock = SystemClock::new();
    let mut times = Vec::with_capacity(FRAMES);
    let (mut shapes, mut rows) = (0, None);
    for frame in 0..WARM_UP + FRAMES {
        // On Home: across its field (right of the 600 of the brief) and back, up and down, a new place every frame.
        let t = frame as f32 / 60.0;
        let over = if scene == Scene::HomeSweep {
            pos2(WINDOW.x * (0.82 + 0.15 * (t * 1.3).sin()), WINDOW.y * (0.5 + 0.4 * (t * 2.1).sin()))
        } else {
            press.unwrap_or(over_list)
        };
        let mut events = vec![Event::PointerMoved(over)];
        if let Some(at) = press
            && frame < 2
        {
            let pressed = frame == 0;
            events.push(Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
            if !pressed && !matches!(scene, Scene::Compose { accesskit: true, .. }) {
                ctx.disable_accesskit();
            }
        }
        if let Scene::Compose { typing: true, .. } = scene
            && frame >= WARM_UP
        {
            events.push(Event::Text("a".to_owned()));
        }
        if matches!(scene, Scene::ListScroll | Scene::ReaderLarge { scrolling: true, .. }) && frame >= WARM_UP {
            events.push(Event::MouseWheel {
                unit: MouseWheelUnit::Point,
                delta: vec2(0.0, -SCROLL),
                phase: TouchPhase::Move,
                source: MouseWheelSource::Unknown,
                modifiers: Modifiers::NONE,
            });
        }
        let input = RawInput { screen_rect: Some(screen), time: Some(frame as f64 / 60.0), predicted_dt: 1.0 / 60.0, events, ..RawInput::default() };
        let started = clock.monotonic();
        let output = ctx.run_ui(input, |ui| app.ui(ui));
        let primitives = ctx.tessellate(output.shapes.clone(), output.pixels_per_point);
        let took = clock.monotonic().saturating_sub(started);
        if frame >= WARM_UP {
            times.push(took.as_secs_f64() * 1000.0);
            shapes = output.shapes.iter().map(|clipped| count(&clipped.shape)).sum();
            rows = rows_built(&ctx).or(rows);
        }
        drop(primitives);
    }
    // A typing scene that typed nothing measured nothing: the press missed the text.
    let after = app.composer().map(|composer| composer.text().len());
    if let Scene::Compose { typing: true, .. } = scene
        && after.zip(before).is_none_or(|(after, before)| after != before + FRAMES)
    {
        return Err(UiError::Shot(format!("typing took {before:?} to {after:?} bytes")));
    }
    // A large scene that showed no large message measured nothing.
    if let Scene::ReaderLarge { large, .. } = scene {
        let shown = app.reader().open_id().and_then(|id| app.reader().opened(id)).map_or((0, 0, 0), |opened| match &opened.content {
            inboxcraft_ui_egui::reader::Content::Html(document) => {
                let letters = document.blocks.iter().map(|block| match block {
                    inboxcraft_html::Block::Text { spans, .. } => spans.iter().map(|span| span.text.len()).sum(),
                    _ => 0,
                });
                (document.blocks.len(), letters.max().unwrap_or(0), document.links.len())
            }
            _ => (0, 0, 0),
        });
        let (blocks, longest, links) = shown;
        let enough = match large {
            Large::Paragraphs => blocks >= 1_000,
            Large::Paragraph => longest >= 1_000_000,
            Large::Links => links >= inboxcraft_html::MAX_LINKS,
        };
        if !enough {
            return Err(UiError::Shot(format!("the large message shows {blocks} blocks, {longest} bytes at most, {links} links")));
        }
    }
    times.sort_by(f64::total_cmp);
    Ok(Measured { median_ms: percentile(&times, 0.5), p95_ms: percentile(&times, 0.95), max_ms: percentile(&times, 1.0), shapes, rows_built: rows })
}

/// Runs a field of `size` for the warm-up and [`FRAMES`] frames, the pointer sweeping across it.
fn measure_field(size: Vec2) -> Measured {
    use inboxcraft_ui_egui::component::gallery_field::placeholder;
    use inboxcraft_ui_egui::field::{self, Field, Level, Opts};
    use inboxcraft_ui_egui::theme::{Mode, TextScale, Theme, install_fonts};
    let ctx = egui::Context::default();
    install_fonts(&ctx);
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let screen = Rect::from_min_size(Pos2::ZERO, size);
    let opts = Opts { on_home: true, on_pane: false, emblem: Some(Rect::from_min_max(pos2(size.x / 2.0, 0.0), screen.max)) };
    let mut kept = Field::new(0).with_emblem(placeholder());
    let clock = SystemClock::new();
    let mut times = Vec::with_capacity(FRAMES);
    let mut shapes = 0;
    for frame in 0..WARM_UP + FRAMES {
        // Across the field and back, up and down: a new place every frame, over the field and the emblem.
        let t = frame as f32 / 60.0;
        let at = pos2(size.x * (0.5 + 0.45 * (t * 1.3).sin()), size.y * (0.5 + 0.4 * (t * 2.1).sin()));
        let events = vec![Event::PointerMoved(at)];
        let input = RawInput { screen_rect: Some(screen), time: Some(frame as f64 / 60.0), predicted_dt: 1.0 / 60.0, events, ..RawInput::default() };
        let started = clock.monotonic();
        let output = ctx.run_ui(input, |ui| field::show(ui, &theme, &mut kept, screen, Level::Full, opts));
        let primitives = ctx.tessellate(output.shapes.clone(), output.pixels_per_point);
        let took = clock.monotonic().saturating_sub(started);
        if frame >= WARM_UP {
            times.push(took.as_secs_f64() * 1000.0);
            shapes = output.shapes.iter().map(|clipped| count(&clipped.shape)).sum();
        }
        drop(primitives);
    }
    times.sort_by(f64::total_cmp);
    Measured { median_ms: percentile(&times, 0.5), p95_ms: percentile(&times, 0.95), max_ms: percentile(&times, 1.0), shapes, rows_built: None }
}

/// The CPU's model name and the count of cores the system shows, from `/proc/cpuinfo`.
fn cpu() -> (String, String) {
    let Ok(info) = std::fs::read_to_string("/proc/cpuinfo") else { return ("unknown".to_owned(), "unknown".to_owned()) };
    let model = info
        .lines()
        .find_map(|line| line.strip_prefix("model name").and_then(|rest| rest.split_once(':')).map(|(_, name)| name.trim().to_owned()))
        .unwrap_or_else(|| "unknown".to_owned());
    let cores = info.lines().filter(|line| line.starts_with("processor")).count();
    (model, if cores == 0 { "unknown".to_owned() } else { cores.to_string() })
}

/// The first core's frequency governor, `unknown` where the system does not show it.
fn governor() -> String {
    std::fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor")
        .map(|governor| governor.trim().to_owned())
        .unwrap_or_else(|_| "unknown".to_owned())
}

fn main() -> ExitCode {
    let pinned = std::env::args().any(|arg| arg == "--pinned");
    let (label, budget) = if pinned { ("pinned to core 0", PINNED_BUDGET_MS) } else { ("every core free", BUDGET_MS) };
    let listed = match app(Scene::DefaultRest) {
        Ok(app) => app.mailbox().messages().len(),
        Err(error) => {
            eprintln!("ui_bench: {error}");
            return ExitCode::FAILURE;
        }
    };
    let (model, cores) = cpu();
    let governor = governor();
    println!("ui_bench, {label}: {FRAMES} frames a scene, window {} x {} pt", WINDOW.x, WINDOW.y);
    println!("mailbox: {MESSAGES} messages held, {listed} listed (the engine lists at most {MAX_LIST})");
    println!("cpu: {model}");
    println!("cores: {cores}");
    if pinned {
        println!("governor: {governor} (the run pins the core, not the clock: the governor is left as it is)");
    } else {
        println!("governor: {governor}");
    }
    let clock = SystemClock::new();
    let asked = |flag: &str| std::env::args().any(|arg| arg == flag);
    if asked("--field") {
        for (name, size) in [("field-window-sweep", WINDOW), ("field-4k-sweep", vec2(3840.0, 2160.0))] {
            let measured = measure_field(size);
            let verdict = if measured.p95_ms <= budget { "PASS" } else { "MISS" };
            println!(
                "{name:<24} median_ms {:>8.3}  p95_ms {:>8.3}  max_ms {:>8.3}  shapes {:>6}  rows_built {:>6}  p95 <= {budget} ms: {verdict}",
                measured.median_ms, measured.p95_ms, measured.max_ms, measured.shapes, "n/a"
            );
        }
        println!("took {:.1} s", clock.monotonic().as_secs_f64());
        return ExitCode::SUCCESS;
    }
    let scenes = if asked("--compose") {
        Scene::compose()
    } else if asked("--calendar") {
        vec![Scene::CalendarWeek, Scene::CalendarCrowd(View::Week), Scene::CalendarCrowd(View::Month), Scene::CalendarCrowd(View::Agenda)]
    } else if asked("--settings") {
        Scene::settings()
    } else if asked("--home") {
        vec![Scene::HomeSweep]
    } else if asked("--large") {
        Large::ALL.into_iter().flat_map(|large| [false, true].map(|scrolling| Scene::ReaderLarge { large, scrolling })).collect()
    } else {
        Scene::ALL.to_vec()
    };
    for scene in scenes {
        match measure(scene) {
            Ok(measured) => {
                let rows = measured.rows_built.map_or_else(|| "n/a".to_owned(), |rows| rows.to_string());
                // Typing keeps the draft once a second, one frame in sixty that the p95 never sees: there the slowest
                // frame is held to the budget too.
                let keeps = matches!(scene, Scene::Compose { typing: true, .. });
                let within = measured.p95_ms <= budget && (!keeps || measured.max_ms <= budget);
                let verdict = if within { "PASS" } else { "MISS" };
                let rule = if keeps { "p95, max" } else { "p95" };
                println!(
                    "{:<24} median_ms {:>8.3}  p95_ms {:>8.3}  max_ms {:>8.3}  shapes {:>6}  rows_built {:>6}  {rule} <= {budget} ms: {verdict}",
                    scene.name(),
                    measured.median_ms,
                    measured.p95_ms,
                    measured.max_ms,
                    measured.shapes,
                    rows
                );
            }
            Err(error) => {
                eprintln!("ui_bench: {}: {error}", scene.name());
                return ExitCode::FAILURE;
            }
        }
    }
    println!("took {:.1} s", clock.monotonic().as_secs_f64());
    ExitCode::SUCCESS
}
