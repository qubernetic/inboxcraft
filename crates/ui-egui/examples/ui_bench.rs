// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `ui_bench`: how long the sample window takes to build a frame, over the sample mailbox grown to 10 000 invented
//! messages, of which the list shows the engine's `MAX_LIST` (1 000). Four scenes of 300 frames each:
//! `inbox-default-rest` (the inbox in the view a new owner sees, today the cards), `inbox-list-rest` and
//! `inbox-list-scroll` (the grouped list, every listed row laid out, scrolling 40 points a frame: the stress case)
//! and `reader-html` (the reader open on an HTML message beside the default view). A frame is the window's `ui` and the tessellation of what it painted, on the CPU;
//! nothing is drawn on a GPU. Per scene it prints the median and the 95th percentile in milliseconds, the shapes and
//! the list rows the last frame built, and the budget's verdict: p95 at most 4 ms, or with `--pinned` (run under
//! `taskset -c 0` by `just bench-ui`) at most 16.6 ms, one frame at 60 Hz. A scene that misses either is a miss. It
//! also prints the CPU's model, its core count and the first core's frequency governor, so numbers from two machines
//! are never mixed up. The verdict never fails the run.
//!
//! With `--compose` it runs the composer's scenes instead: the composer open with a message text of 16 KiB, 64 KiB,
//! 256 KiB and 1 MiB (just under the composer's cap, so typing fits), its text focused, at rest and typing one
//! character a frame at its start, each without AccessKit and with it (`-ak`, as while a screen reader listens).
//!
//! With `--calendar` it runs the calendar's scenes instead: `calendar-week-200`, the week view holding 200 invented
//! events (overlapping, over whole days), left alone; and `calendar-week-5000`, `calendar-month-5000` and
//! `calendar-agenda-5000`, each view holding 5 000 invented events put in place of what was read (more than the
//! engine lists at once): hundreds at one hour, hundreds over the same days, the rest spread out.
//!
//! ```text
//! cargo run --release -p inboxcraft-ui-egui --example ui_bench [-- --pinned] [--compose] [--calendar]
//! ```

use std::process::ExitCode;

use egui::{Event, Modifiers, MouseWheelSource, MouseWheelUnit, Pos2, RawInput, Rect, Shape, TouchPhase, Vec2, pos2, vec2};
use inboxcraft_engine::MAX_LIST;
use inboxcraft_time::{Clock, FakeClock, SystemClock, Timestamp};
use inboxcraft_ui_egui::calendar::{Day, EventRow, Moment, View};
use inboxcraft_ui_egui::mailbox::{InboxView, Mailbox};
use inboxcraft_ui_egui::save::NoSave;
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
/// The HTML message the reader opens.
const HTML_SUBJECT: &str = "Your bike is due for a service";

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
    /// The composer with a message text of `kib` KiB, focused; `typing` a character a frame; `accesskit` on.
    Compose { kib: usize, typing: bool, accesskit: bool },
    /// The calendar's week holding [`WEEK_EVENTS`] events, left alone.
    CalendarWeek,
    /// The calendar's `view` holding [`CROWD_EVENTS`] events, left alone.
    CalendarCrowd(View),
}

impl Scene {
    const ALL: [Self; 4] = [Self::DefaultRest, Self::ListRest, Self::ListScroll, Self::ReaderHtml];

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

    fn name(self) -> String {
        match self {
            Self::DefaultRest => "inbox-default-rest".to_owned(),
            Self::ListRest => "inbox-list-rest".to_owned(),
            Self::ListScroll => "inbox-list-scroll".to_owned(),
            Self::ReaderHtml => "reader-html".to_owned(),
            Self::CalendarWeek => "calendar-week-200".to_owned(),
            Self::CalendarCrowd(view) => format!("calendar-{}-{CROWD_EVENTS}", format!("{view:?}").to_lowercase()),
            Self::Compose { kib, typing, accesskit } => {
                format!("compose-{kib}k-{}{}", if typing { "typing" } else { "rest" }, if accesskit { "-ak" } else { "" })
            }
        }
    }

    /// The inbox view it sets; `None` keeps the default.
    fn view(self) -> Option<InboxView> {
        match self {
            Self::ListRest | Self::ListScroll => Some(InboxView::List),
            Self::DefaultRest | Self::ReaderHtml | Self::Compose { .. } | Self::CalendarWeek | Self::CalendarCrowd(_) => None,
        }
    }
}

/// What one scene measured.
struct Measured {
    median_ms: f64,
    p95_ms: f64,
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
    let store = sample::grown(time, MESSAGES)?;
    let mut session = sample::session_over(store, Box::new(clock.clone()))?;
    if scene == Scene::CalendarWeek {
        week_of_events(&mut session)?;
    }
    let mailbox = Mailbox::new(session, Box::new(clock));
    let mut app = InboxApp::with_outside(mailbox, Box::new(sample::Pictures::default()), Box::new(NoSave));
    app.set_reduce_motion(true);
    if let Some(view) = scene.view() {
        app.mailbox_mut().set_inbox_view(view);
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
    // Over the list, left of the reader and right of the folders.
    let over_list = pos2(WINDOW.x * 0.35, WINDOW.y * 0.6);
    let clock = SystemClock::new();
    let mut times = Vec::with_capacity(FRAMES);
    let (mut shapes, mut rows) = (0, None);
    for frame in 0..WARM_UP + FRAMES {
        let mut events = vec![Event::PointerMoved(press.unwrap_or(over_list))];
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
        if scene == Scene::ListScroll && frame >= WARM_UP {
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
    times.sort_by(f64::total_cmp);
    Ok(Measured { median_ms: percentile(&times, 0.5), p95_ms: percentile(&times, 0.95), shapes, rows_built: rows })
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
    let scenes = if std::env::args().any(|arg| arg == "--compose") {
        Scene::compose()
    } else if std::env::args().any(|arg| arg == "--calendar") {
        vec![Scene::CalendarWeek, Scene::CalendarCrowd(View::Week), Scene::CalendarCrowd(View::Month), Scene::CalendarCrowd(View::Agenda)]
    } else {
        Scene::ALL.to_vec()
    };
    for scene in scenes {
        match measure(scene) {
            Ok(measured) => {
                let rows = measured.rows_built.map_or_else(|| "n/a".to_owned(), |rows| rows.to_string());
                let verdict = if measured.p95_ms <= budget { "PASS" } else { "MISS" };
                println!(
                    "{:<24} median_ms {:>8.3}  p95_ms {:>8.3}  shapes {:>6}  rows_built {:>6}  p95 <= {budget} ms: {verdict}",
                    scene.name(),
                    measured.median_ms,
                    measured.p95_ms,
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
