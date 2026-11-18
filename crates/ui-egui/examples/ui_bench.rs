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
//! ```text
//! cargo run --release -p inboxcraft-ui-egui --example ui_bench [-- --pinned]
//! ```

use std::process::ExitCode;

use egui::{Event, Modifiers, MouseWheelSource, MouseWheelUnit, Pos2, RawInput, Rect, Shape, TouchPhase, Vec2, pos2, vec2};
use inboxcraft_engine::MAX_LIST;
use inboxcraft_time::{Clock, FakeClock, SystemClock, Timestamp};
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
}

impl Scene {
    const ALL: [Self; 4] = [Self::DefaultRest, Self::ListRest, Self::ListScroll, Self::ReaderHtml];

    fn name(self) -> &'static str {
        match self {
            Self::DefaultRest => "inbox-default-rest",
            Self::ListRest => "inbox-list-rest",
            Self::ListScroll => "inbox-list-scroll",
            Self::ReaderHtml => "reader-html",
        }
    }

    /// The inbox view it sets; `None` keeps the default.
    fn view(self) -> Option<InboxView> {
        match self {
            Self::ListRest | Self::ListScroll => Some(InboxView::Grouped),
            Self::DefaultRest | Self::ReaderHtml => None,
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
    let session = sample::session_over(store, Box::new(clock.clone()))?;
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
    Ok(app)
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
    // Over the list, left of the reader and right of the folders.
    let over_list = pos2(WINDOW.x * 0.35, WINDOW.y * 0.6);
    let clock = SystemClock::new();
    let mut times = Vec::with_capacity(FRAMES);
    let (mut shapes, mut rows) = (0, None);
    for frame in 0..WARM_UP + FRAMES {
        let mut events = vec![Event::PointerMoved(over_list)];
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
    for scene in Scene::ALL {
        match measure(scene) {
            Ok(measured) => {
                let rows = measured.rows_built.map_or_else(|| "n/a".to_owned(), |rows| rows.to_string());
                let verdict = if measured.p95_ms <= budget { "PASS" } else { "MISS" };
                println!(
                    "{:<20} median_ms {:>8.3}  p95_ms {:>8.3}  shapes {:>6}  rows_built {:>6}  p95 <= {budget} ms: {verdict}",
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
