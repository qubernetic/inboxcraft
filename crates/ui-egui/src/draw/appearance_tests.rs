// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The appearance commands as the window applies and keeps them.

use std::path::Path;

use egui::vec2;
use egui_kittest::Harness;
use inboxcraft_engine::appearance::{Appearance, Motion, MotionLevel, ThemeMode, WindowFrame};
use inboxcraft_time::{FakeClock, Timestamp};
use serde_json::json;

use super::InboxApp;
use crate::keep::{FILE, Keep, KeepFile};
use crate::mailbox::Mailbox;
use crate::motion;
use crate::sample;
use crate::theme::{Look, Mode, ModePreference, TextScale, Theme};

/// 2026-11-05T09:00:00Z.
fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

/// The sample window keeping its appearance in `file`, after its first frame.
fn window(file: &Path) -> Harness<'static, InboxApp> {
    let app = InboxApp::at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock())).with_keep(Box::new(KeepFile::new(file.to_path_buf())));
    let mut harness = Harness::builder().with_size(vec2(1100.0, 720.0)).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

fn run(harness: &mut Harness<'static, InboxApp>, id: &'static str, params: serde_json::Value) -> serde_json::Value {
    let answer = harness.state_mut().set_appearance(id, &params).unwrap();
    harness.run();
    answer
}

#[test]
fn every_choice_persists_across_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("inboxcraft").join(FILE);
    let mut harness = window(&file);
    assert_eq!(*harness.state().appearance(), Appearance::default());
    run(&mut harness, "ui.theme.set", json!({ "mode": "dark" }));
    // An old client's frame: the top row of older versions is the own frame now.
    run(&mut harness, "ui.frame.set", json!({ "frame": "system" }));
    run(&mut harness, "ui.frame.set", json!({ "frame": "top_row" }));
    run(&mut harness, "ui.text_scale.set", json!({ "percent": 120 }));
    run(&mut harness, "ui.motion.set", json!({ "reduce": "on" }));
    let chosen = *harness.state().appearance();
    drop(harness);

    let restarted = window(&file);
    let app = restarted.state();
    assert_eq!(*app.appearance(), chosen);
    assert_eq!(app.mode(), ModePreference::Dark);
    assert_eq!(app.appearance().frame, WindowFrame::Own);
    assert_eq!(app.text_scale(), TextScale::from_percent(120).unwrap());
    assert_eq!(app.appearance().motion_level(), MotionLevel::Off);
    // The restarted window draws with them from its first frame.
    assert_eq!(Mode::of(&restarted.ctx), Mode::Dark);
    let expected = Theme::of(Mode::Dark, &Look::of(&chosen)).style();
    assert_eq!(restarted.ctx.style_of(egui::Theme::Dark).text_styles, expected.text_styles);
    assert_eq!(restarted.ctx.style_of(egui::Theme::Dark).visuals.panel_fill, expected.visuals.panel_fill);
    assert_eq!(motion::level(&restarted.ctx), motion::Level::Off);
}

#[test]
fn an_appearance_kept_by_the_old_look_loads_and_draws_the_one_palette() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(FILE);
    let old = json!({ "theme": "dark", "accent": { "preset": "aurora" }, "font": "inter", "icons": "lucide", "text_scale": 110 });
    std::fs::write(&file, old.to_string()).unwrap();
    let harness = window(&file);
    assert_eq!(*harness.state().appearance(), Appearance { theme: ThemeMode::Dark, text_scale: 110, ..Appearance::default() });
    let dark = Theme::new(Mode::Dark, TextScale::from_percent(110).unwrap()).style();
    assert_eq!(harness.ctx.style_of(egui::Theme::Dark).visuals, dark.visuals, "one palette, whatever the old file named");
    assert_eq!(harness.ctx.style_of(egui::Theme::Dark).text_styles, dark.text_styles, "IBM Plex, whatever the old file named");
}

#[test]
fn the_window_s_own_setters_run_the_commands_and_are_kept_too() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(FILE);
    let mut harness = window(&file);
    let app = harness.state_mut();
    app.set_mode(ModePreference::Light);
    app.set_frame(WindowFrame::System);
    app.set_text_scale(TextScale::from_percent(90).unwrap());
    app.set_motion_level(motion::Level::Full);
    harness.run();
    let kept = KeepFile::new(file.clone()).load().unwrap();
    assert_eq!(kept, *harness.state().appearance());
    assert_eq!((kept.motion_level(), kept.motion_chosen()), (MotionLevel::Full, true));
    assert_eq!(kept.text_scale, 90);
    assert_eq!(kept.frame, WindowFrame::System);
}

#[test]
fn a_refused_choice_changes_nothing_and_the_window_shows_why_a_change_could_not_be_kept() {
    let dir = tempfile::tempdir().unwrap();
    let blocker = dir.path().join("blocker");
    std::fs::write(&blocker, b"a file where the folder should be").unwrap();
    let mut harness = window(&blocker.join(FILE));
    assert!(harness.state_mut().set_appearance("ui.text_scale.set", &json!({ "percent": 95 })).is_err());
    assert_eq!(*harness.state().appearance(), Appearance::default());
    assert_eq!(harness.state().mailbox().error(), None);
    run(&mut harness, "ui.theme.set", json!({ "mode": "dark" }));
    assert_eq!(Mode::of(&harness.ctx), Mode::Dark, "the change shows even when it cannot be kept");
    let error = harness.state().mailbox().error().unwrap();
    assert!(error.contains("the appearance could not be kept"), "{error}");
}

#[test]
fn a_change_from_outside_the_window_is_kept_from_the_next_frame() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(FILE);
    let mut harness = window(&file);
    assert_eq!(KeepFile::new(file.clone()).load(), None, "nothing is written while nothing changed");
    harness.state_mut().mailbox_mut().run("ui.motion.set", &json!({ "reduce": "on" })).unwrap();
    assert_eq!(KeepFile::new(file.clone()).load(), None);
    harness.run();
    assert_eq!(KeepFile::new(file).load().map(|kept| kept.motion), Some(Motion::Reduce));
}
