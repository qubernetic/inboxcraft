// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::{Path, PathBuf};

use egui::{Context, Id, RawInput};

use super::{LONGEST, MOTION, appear, reduced, set_reduced, system_default, toggle};

/// One frame.
pub(crate) const FRAME: f64 = 1.0 / 60.0;

/// Runs one frame of `ctx` at `time` seconds and returns what `f` gave.
pub(crate) fn frame<R>(ctx: &Context, time: f64, f: impl FnOnce(&Context) -> R) -> R {
    let mut out = None;
    let mut f = Some(f);
    let input = RawInput { time: Some(time), predicted_dt: FRAME as f32, ..RawInput::default() };
    ctx.run_ui(input, |ui| {
        if let Some(f) = f.take() {
            out = Some(f(ui.ctx()));
        }
    })
    .textures_delta
    .clear();
    out.unwrap()
}

#[test]
fn every_motion_token_is_short_and_real() {
    assert_eq!(MOTION.all().len(), 6);
    for seconds in MOTION.all() {
        assert!(seconds > 0.0 && seconds <= LONGEST, "{seconds}");
    }
    const { assert!(LONGEST <= 0.25) };
}

#[test]
fn motion_is_on_unless_the_system_asks_for_less() {
    assert!(!system_default(None));
    assert!(!system_default(Some(false)));
    assert!(system_default(Some(true)));
    let ctx = Context::default();
    assert!(!reduced(&ctx));
    set_reduced(&ctx, true);
    assert!(reduced(&ctx));
    set_reduced(&ctx, false);
    assert!(!reduced(&ctx));
}

#[test]
fn reduced_motion_stops_egui_s_own_animations_too() {
    let ctx = Context::default();
    set_reduced(&ctx, true);
    assert_eq!(ctx.global_style().animation_time, 0.0);
    assert_eq!(ctx.global_style().scroll_animation, egui::style::ScrollAnimation::none());
    set_reduced(&ctx, false);
    assert_eq!(ctx.global_style().animation_time, MOTION.tooltip);
    assert_eq!(ctx.global_style().scroll_animation, egui::style::ScrollAnimation::default());
    let mut style = egui::Style::default();
    super::tune(&mut style);
    assert_eq!(style.animation_time, MOTION.tooltip);
}

/// The value of a toggle that turns on at the second frame, frame by frame for half a second.
fn toggled(reduce: bool, seconds: f32) -> Vec<f32> {
    let ctx = Context::default();
    set_reduced(&ctx, reduce);
    let id = Id::unique("toggle");
    (0..30).map(|n| frame(&ctx, f64::from(n) * FRAME, |ctx| toggle(ctx, id, n > 0, seconds))).collect()
}

#[test]
fn a_toggle_moves_over_its_token_and_lands_on_its_end() {
    for seconds in MOTION.all() {
        let values = toggled(false, seconds);
        assert_eq!(values.first().copied(), Some(0.0));
        assert!(values.iter().any(|value| *value > 0.0 && *value < 1.0), "{seconds}: {values:?}");
        assert_eq!(values.last().copied(), Some(1.0), "{seconds}");
        assert!(values.windows(2).all(|pair| pair[0] <= pair[1]), "{values:?}");
    }
}

#[test]
fn reduced_motion_stops_every_toggle() {
    for seconds in MOTION.all() {
        let values = toggled(true, seconds);
        assert_eq!(values.first().copied(), Some(0.0));
        assert!(values.iter().skip(1).all(|value| *value == 1.0), "{seconds}: {values:?}");
    }
}

/// How far something shown from the third frame on has appeared, frame by frame.
fn appeared(reduce: bool, seconds: f32, gap: bool) -> Vec<f32> {
    let ctx = Context::default();
    set_reduced(&ctx, reduce);
    let id = Id::unique("appear");
    (0..40).map(|n| frame(&ctx, f64::from(n) * FRAME, |ctx| if n < 2 || (gap && n == 25) { -1.0 } else { appear(ctx, id, seconds) })).collect()
}

#[test]
fn something_new_appears_over_its_token() {
    for seconds in MOTION.all() {
        let values = appeared(false, seconds, false);
        assert_eq!(values.get(2).copied(), Some(0.0), "{values:?}");
        assert!(values.iter().any(|value| *value > 0.0 && *value < 1.0), "{seconds}: {values:?}");
        assert_eq!(values.last().copied(), Some(1.0));
    }
}

#[test]
fn something_shown_again_after_a_frame_away_appears_again() {
    let values = appeared(false, MOTION.toast, true);
    assert_eq!(values.get(24).copied(), Some(1.0));
    assert_eq!(values.get(26).copied(), Some(0.0), "{values:?}");
}

#[test]
fn reduced_motion_shows_something_new_at_once() {
    for seconds in MOTION.all() {
        let values = appeared(true, seconds, false);
        assert!(values.iter().skip(2).all(|value| *value == 1.0), "{seconds}: {values:?}");
    }
}

#[test]
fn nothing_asks_for_a_repaint_once_it_has_landed() {
    let ctx = Context::default();
    let id = Id::unique("still");
    for n in 0..40 {
        frame(&ctx, f64::from(n) * FRAME, |ctx| {
            appear(ctx, id, MOTION.tooltip);
            toggle(ctx, id, true, MOTION.card);
        });
    }
    let input = RawInput { time: Some(1.0), predicted_dt: FRAME as f32, ..RawInput::default() };
    let mut output = ctx.run_ui(input, |ui| {
        appear(ui.ctx(), id, MOTION.tooltip);
        toggle(ui.ctx(), id, true, MOTION.card);
    });
    output.textures_delta.clear();
    let repaint = output.viewport_output.values().map(|viewport| viewport.repaint_delay).min().unwrap();
    assert!(repaint > std::time::Duration::from_secs(1), "{repaint:?}");
}

#[test]
fn nonsense_durations_land_at_once() {
    for seconds in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        let values = toggled(false, seconds);
        assert!(values.iter().skip(1).all(|value| *value == 1.0), "{seconds}: {values:?}");
        let values = appeared(false, seconds, false);
        assert!(values.iter().skip(2).all(|value| *value == 1.0), "{seconds}: {values:?}");
    }
}

/// The `.rs` files under `dir`, test files left out.
fn sources(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().and_then(|name| name.to_str()).unwrap_or_default().to_owned();
        if path.is_dir() {
            sources(&path, found);
        } else if name.ends_with(".rs") && name != "tests.rs" && !name.ends_with("_tests.rs") {
            found.push(path);
        }
    }
}

#[test]
fn every_animation_goes_through_the_motion_module() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    sources(&root.join("src"), &mut files);
    assert!(files.len() > 10, "the walk found the sources");
    let motion = root.join("src").join("motion.rs");
    let mut found = Vec::new();
    for file in files.iter().filter(|file| **file != motion) {
        let text = std::fs::read_to_string(file).unwrap();
        for (index, line) in text.lines().enumerate() {
            let code = line.split("//").next().unwrap_or_default();
            if code.contains("animate_") || code.contains("animation_time") || code.contains("CollapsingState") {
                found.push(format!("{}:{}: {}", file.strip_prefix(root).unwrap().display(), index + 1, line.trim()));
            }
        }
    }
    assert!(found.is_empty(), "animations read the reduced-motion flag in motion.rs:\n{}", found.join("\n"));
}

#[test]
fn the_design_curve_runs_from_0_to_1_as_cubic_bezier_2_7_2_1() {
    use super::ease;
    assert_eq!(ease(0.0), 0.0);
    assert_eq!(ease(1.0), 1.0);
    // Measured apart from the code: x(t) sampled at two million points and inverted by interpolation.
    for (share, wanted) in [(0.1, 0.359_123), (0.25, 0.727_373), (0.5, 0.929_579), (0.75, 0.987_468), (0.9, 0.998_287)] {
        assert!((ease(share) - wanted).abs() < 1e-4, "{share}: {} for {wanted}", ease(share));
    }
    // Rising, and safe with what is not a share.
    let mut last = 0.0;
    for step in 0..=100 {
        let at = ease(step as f32 / 100.0);
        assert!(at >= last && (0.0..=1.0).contains(&at), "{step}: {at}");
        last = at;
    }
    assert_eq!((ease(-1.0), ease(2.0), ease(f32::NAN), ease(f32::INFINITY)), (0.0, 1.0, 1.0, 1.0));
}

#[test]
fn css_ease_runs_from_0_to_1_as_cubic_bezier_25_1_25_1() {
    use super::css_ease;
    assert_eq!((css_ease(0.0), css_ease(1.0)), (0.0, 1.0));
    // Measured apart from the code: x(t) inverted by halving 80 times in double precision.
    for (share, wanted) in [(0.1, 0.094_796), (0.25, 0.408_511), (0.5, 0.802_403), (0.75, 0.960_459), (0.9, 0.994_316)] {
        assert!((css_ease(share) - wanted).abs() < 1e-4, "{share}: {} for {wanted}", css_ease(share));
    }
    let mut last = 0.0;
    for step in 0..=100 {
        let at = css_ease(step as f32 / 100.0);
        assert!(at >= last && (0.0..=1.0).contains(&at), "{step}: {at}");
        last = at;
    }
    assert_eq!((css_ease(-1.0), css_ease(2.0), css_ease(f32::NAN), css_ease(f32::NEG_INFINITY)), (0.0, 1.0, 1.0, 0.0));
}

#[test]
fn the_elapsed_share_runs_evenly_and_lands() {
    use super::elapsed;
    let ctx = Context::default();
    let id = Id::unique("elapsed-test");
    let shares: Vec<f32> = (0..20).map(|n| frame(&ctx, 1.0 + n as f64 * FRAME, |ctx| elapsed(ctx, id, 0.18))).collect();
    assert_eq!(shares[0], 0.0);
    assert!((shares[3] - (3.0 * FRAME as f32 / 0.18)).abs() < 1e-3, "{shares:?}");
    assert_eq!(shares.last().copied(), Some(1.0));
}
