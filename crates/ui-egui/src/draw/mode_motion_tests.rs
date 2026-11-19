// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Dark and light (M14) at the three levels, frame by frame from dark to light by a key, frames 10 ms apart: the new mode
//! in the frame of its key, the old colours fading out over the new ones, a key during it, and rest.

use egui::{Key, KeyboardShortcut, Modifiers};
use egui_kittest::Harness;
use serde_json::json;

use crate::InboxApp;
use crate::draw::list::rows_motion_tests::window;
use crate::motion::Level;
use crate::theme::{Mode, Palette};

/// The window in dark at `level`, Light bound to Ctrl Shift L.
fn dark(level: Level) -> Harness<'static, InboxApp> {
    let mut harness = window(level);
    harness.state_mut().set_appearance("ui.theme.set", &json!({ "mode": "dark" })).unwrap();
    harness.state_mut().keymap_mut().bind(crate::keymap::LIGHT, KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, Key::L));
    harness.run();
    assert_eq!(Mode::of(&harness.ctx), Mode::Dark);
    harness
}

/// Ctrl Shift L as one event: the frame of the key is the last drawn.
fn light(harness: &mut Harness<'static, InboxApp>) {
    let modifiers = Modifiers::COMMAND | Modifiers::SHIFT;
    harness.event(egui::Event::Key { key: Key::L, pressed: true, modifiers, repeat: false, physical_key: None });
    harness.step();
}

/// The alpha of the dark window's ground painted over the whole window, the last one painted; none.
fn dark_over(harness: &Harness<'static, InboxApp>) -> Option<u8> {
    let window = harness.ctx.content_rect();
    let dark = Palette::of(Mode::Dark).bg;
    harness.output().shapes.iter().rev().find_map(|clipped| match &clipped.shape {
        egui::Shape::Rect(ground) if ground.rect.contains_rect(window.shrink(2.0)) && ground.fill.a() > 0 && near(ground.fill, dark) => {
            Some(ground.fill.a())
        }
        _ => None,
    })
}

/// Whether `seen`, faded or not, is `colour`.
fn near(seen: egui::Color32, colour: egui::Color32) -> bool {
    let [r, g, b, _] = seen.to_srgba_unmultiplied();
    [r, g, b].iter().zip([colour.r(), colour.g(), colour.b()]).all(|(a, b)| a.abs_diff(b) <= 3)
}

/// Whether the light window's ground is painted (the new mode drawn under whatever fades over it).
fn light_drawn(harness: &Harness<'static, InboxApp>) -> bool {
    let light = Palette::of(Mode::Light);
    harness
        .output()
        .shapes
        .iter()
        .any(|clipped| matches!(&clipped.shape, egui::Shape::Rect(ground) if ground.fill == light.bg || ground.fill == light.panel))
}

#[test]
fn at_off_light_is_there_in_the_frame_of_its_key() {
    let mut harness = dark(Level::Off);
    light(&mut harness);
    assert_eq!(Mode::of(&harness.ctx), Mode::Light, "the owner never waits: the mode in the frame of its key");
    assert!(light_drawn(&harness), "drawn light in that frame");
    assert_eq!(dark_over(&harness), None, "nothing of the dark one");
    assert!(crate::component::tests::rests(&mut harness, 2), "and rests");
}

#[test]
fn at_full_the_colours_cross_fade_over_300_ms() {
    let mut harness = dark(Level::Full);
    light(&mut harness);
    assert!(light_drawn(&harness), "0 ms: the light window drawn");
    assert_eq!(dark_over(&harness), Some(255), "0 ms: the dark one whole over it");
    harness.run_steps(15);
    let half = dark_over(&harness);
    assert!(half.is_some_and(|alpha| (100..160).contains(&alpha)), "150 ms: half way, {half:?}");
    harness.run_steps(16);
    assert_eq!(dark_over(&harness), None, "310 ms: light");
    assert!(crate::component::tests::rests(&mut harness, 2), "and rests");
}

#[test]
fn at_reduced_the_colours_cross_fade_over_125_ms() {
    let mut harness = dark(Level::Reduced);
    light(&mut harness);
    assert_eq!(dark_over(&harness), Some(255), "0 ms: the dark one whole over the light");
    harness.run_steps(6);
    let half = dark_over(&harness);
    assert!(half.is_some_and(|alpha| (100..160).contains(&alpha)), "60 ms: half way, {half:?}");
    harness.run_steps(7);
    assert_eq!(dark_over(&harness), None, "130 ms: light");
    assert!(crate::component::tests::rests(&mut harness, 2), "and rests");
}

#[test]
fn a_key_during_the_cross_fade_acts_and_ends_it_in_its_frame() {
    for level in [Level::Full, Level::Reduced] {
        let mut harness = dark(level);
        light(&mut harness);
        harness.run_steps(4);
        assert!(dark_over(&harness).is_some(), "{level:?}: 40 ms, still fading");
        harness.event(egui::Event::Key { key: Key::J, pressed: true, modifiers: Modifiers::NONE, repeat: false, physical_key: None });
        harness.step();
        assert_eq!(dark_over(&harness), None, "{level:?}: the key ends it in its frame");
        assert!(harness.state().mailbox().selected().is_some(), "{level:?}: and acts");
    }
}

#[test]
fn a_key_the_command_line_takes_ends_the_cross_fade_in_its_frame() {
    // ↓ is the command line's own while its results show; it ends the movement all the same.
    let mut harness = dark(Level::Full);
    light(&mut harness);
    // The results open with no key of their own (as a slash typed in a field would open them).
    harness.state_mut().command_mut().open_with(">");
    harness.run_steps(4);
    assert!(harness.state().command().results(), "the results show");
    assert!(dark_over(&harness).is_some(), "40 ms, still fading");
    let picked = harness.state().command().picked();
    harness.event(egui::Event::Key { key: Key::ArrowDown, pressed: true, modifiers: Modifiers::NONE, repeat: false, physical_key: None });
    harness.step();
    assert_ne!(harness.state().command().picked(), picked, "the command line took the key");
    assert_eq!(dark_over(&harness), None, "and the key ended the cross-fade in its frame");
}
