// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A message opening (M7) at the three levels, frame by frame in the simple list, frames 10 ms apart: J moves the
//! keyboard to the next row and opens it; the message in the frame of its key, the key frames of the design's table,
//! a key during the movement, and rest.

use std::collections::BTreeSet;

use egui::{Key, Rect};
use egui_kittest::Harness;

use crate::InboxApp;
use crate::draw::list::rows_motion_tests::{press, window};
use crate::motion::Level;

/// Where the reader's card was drawn last.
pub(crate) fn reader_rect(harness: &Harness<'static, InboxApp>) -> Rect {
    harness.ctx.data(|data| data.get_temp::<Rect>(egui::Id::unique("reader-rect"))).unwrap_or(Rect::NOTHING)
}

/// The texts that show inside `rect` (what is cut away or covered by an opaque ground painted after it does not), each
/// with its alpha; icons and texts under four characters left out.
pub(crate) fn seen(harness: &Harness<'static, InboxApp>, rect: Rect) -> Vec<(String, u8, Rect)> {
    let icons = crate::theme::icon_font(1.0).family;
    let mut out: Vec<(String, u8, Rect)> = Vec::new();
    for clipped in &harness.output().shapes {
        let mut add = |shape: &egui::Shape| match shape {
            egui::Shape::Rect(ground) if ground.fill.a() == 255 => {
                let covers = ground.rect.intersect(clipped.clip_rect);
                out.retain(|(_, _, at)| !covers.contains_rect(*at));
            }
            egui::Shape::Text(text) if !text.galley.job.sections.iter().any(|section| section.format.font_id.family == icons) => {
                let at = text.galley.rect.translate(text.pos.to_vec2()).intersect(clipped.clip_rect);
                if at.is_positive() && rect.contains(at.center()) && text.galley.job.text.chars().count() >= 4 {
                    // An alpha of 0 to 255: the cast keeps it.
                    let alpha = (text.opacity_factor * f32::from(text.fallback_color.a())) as u8;
                    out.push((text.galley.job.text.clone(), alpha, at));
                }
            }
            _ => {}
        };
        match &clipped.shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(&mut add),
            shape => add(shape),
        }
    }
    out
}

fn words(seen: &[(String, u8, Rect)]) -> BTreeSet<String> {
    seen.iter().map(|(text, _, _)| text.clone()).collect()
}

/// The window with the first message open (J), at rest.
fn first_open(level: Level) -> Harness<'static, InboxApp> {
    let mut harness = window(level);
    press(&mut harness, Key::J);
    harness.run();
    harness
}

/// The texts of the first message's reader and the second's at rest, each without the other's.
fn ends() -> (BTreeSet<String>, BTreeSet<String>) {
    let mut harness = first_open(Level::Off);
    let first = words(&seen(&harness, reader_rect(&harness)));
    press(&mut harness, Key::J);
    harness.run();
    let second = words(&seen(&harness, reader_rect(&harness)));
    (first.difference(&second).cloned().collect(), second.difference(&first).cloned().collect())
}

#[test]
fn at_off_the_message_opened_is_there_in_the_frame_of_its_key() {
    let (first, second) = ends();
    assert!(!first.is_empty() && !second.is_empty());
    let mut harness = first_open(Level::Off);
    press(&mut harness, Key::J);
    let shown = words(&seen(&harness, reader_rect(&harness)));
    assert!(second.is_subset(&shown) && first.is_disjoint(&shown), "the owner never waits: the message in the frame of its key, {shown:?}");
    assert!(crate::component::tests::rests(&mut harness, 2), "and rests");
}

#[test]
fn at_full_the_message_wipes_in_from_the_top_over_the_old_one_in_300_ms() {
    let (first, second) = ends();
    let mut harness = first_open(Level::Full);
    press(&mut harness, Key::J);
    let reader = reader_rect(&harness);
    let shown = words(&seen(&harness, reader));
    assert!(first.is_subset(&shown) && second.is_disjoint(&shown), "0 ms: the old message whole, the line at the top: {shown:?}");
    harness.run_steps(2);
    let shown = seen(&harness, reader);
    let (new, old): (Vec<_>, Vec<_>) =
        shown.iter().filter(|(text, _, _)| first.contains(text) || second.contains(text)).partition(|(text, _, _)| second.contains(text));
    let line = reader.top() + reader.height() * 0.24;
    assert!(!new.is_empty() && new.iter().all(|(_, _, at)| at.center().y < line + 40.0), "20 ms: the new message above the line, {new:?}");
    assert!(!old.is_empty() && old.iter().all(|(_, _, at)| at.center().y > line - 40.0), "20 ms: the old one under it, {old:?}");
    harness.run_steps(10);
    let shown = words(&seen(&harness, reader));
    assert!(shown.intersection(&second).count() > shown.intersection(&first).count(), "120 ms: most of the new message, {shown:?}");
    harness.run_steps(19);
    let shown = words(&seen(&harness, reader));
    assert!(second.is_subset(&shown) && first.is_disjoint(&shown), "310 ms: the message open, {shown:?}");
    assert!(crate::component::tests::rests(&mut harness, 2), "and rests");
}

#[test]
fn at_reduced_the_old_message_fades_out_over_the_new_in_125_ms() {
    let (first, second) = ends();
    let mut harness = first_open(Level::Reduced);
    press(&mut harness, Key::J);
    let reader = reader_rect(&harness);
    let shown = seen(&harness, reader);
    assert!(first.is_subset(&words(&shown)), "0 ms: the old message whole");
    harness.run_steps(6);
    let shown = seen(&harness, reader);
    assert!(second.is_subset(&words(&shown)), "60 ms: the new message there under it, {shown:?}");
    let old: Vec<u8> = shown.iter().filter(|(text, _, _)| first.contains(text)).map(|(_, alpha, _)| *alpha).collect();
    assert!(!old.is_empty() && old.iter().all(|alpha| (80..180).contains(alpha)), "60 ms: the old one half gone, {old:?}");
    harness.run_steps(7);
    let shown = words(&seen(&harness, reader));
    assert!(second.is_subset(&shown) && first.is_disjoint(&shown), "130 ms: the message open, {shown:?}");
    assert!(crate::component::tests::rests(&mut harness, 2), "and rests");
}

#[test]
fn a_key_while_a_message_comes_in_acts_in_its_frame_and_the_movement_is_at_its_end() {
    let (first, second) = ends();
    for level in [Level::Full, Level::Reduced] {
        let mut harness = first_open(level);
        press(&mut harness, Key::J);
        harness.run_steps(3);
        press(&mut harness, Key::Tab);
        let shown = words(&seen(&harness, reader_rect(&harness)));
        assert!(second.is_subset(&shown) && first.is_disjoint(&shown), "{level:?}: the key ends it in its frame, {shown:?}");
    }
}

#[test]
fn a_press_while_a_message_comes_in_reaches_nothing_under_the_old_one() {
    // The new message's controls stand under the old one's picture: what lies over the reader takes the press instead.
    let shield = egui::LayerId::new(egui::Order::Background, egui::Id::unique("reader-shield"));
    for level in [Level::Full, Level::Reduced] {
        let mut harness = first_open(level);
        press(&mut harness, Key::J);
        harness.step();
        let reader = reader_rect(&harness);
        let low = egui::pos2(reader.center().x, reader.bottom() - 40.0);
        assert_eq!(harness.ctx.layer_id_at(low), Some(shield), "{level:?}: the picture's place takes a press");
        harness.run_steps(50);
        assert_ne!(harness.ctx.layer_id_at(low), Some(shield), "{level:?}: at rest the message takes it");
    }
}

#[test]
fn a_press_on_the_floating_composer_while_a_message_comes_in_reaches_it() {
    // The shield stands over the reader's place only: a float over it, the floating composer, takes its own presses,
    // and the keys that follow type there, not on the list.
    use egui::accesskit::Role;
    use egui_kittest::kittest::Queryable as _;
    for level in [Level::Full, Level::Reduced] {
        let mut harness = first_open(level);
        let ctx = harness.ctx.clone();
        harness.state_mut().run_action(&ctx, crate::keymap::COMPOSE);
        harness.run();
        harness.state_mut().composer_mut().0.unwrap().set_floating(true);
        harness.ctx.memory_mut(|memory| memory.stop_text_input());
        harness.run();
        assert!(harness.state().composer().is_some_and(crate::compose::Composer::floating), "{level:?}: the composer floats");
        let count = harness.state().mailbox().messages().len();
        press(&mut harness, Key::J);
        harness.step();
        let at = harness.get_by_role_and_label(Role::TextInput, "Subject").rect().center();
        super::send_motion_tests::click_at(&mut harness, at);
        assert_eq!(harness.ctx.memory(|memory| memory.focused()), Some(crate::draw::compose::subject_id()), "{level:?}: Subject has the keyboard");
        harness.event(egui::Event::Key { key: Key::D, pressed: true, modifiers: egui::Modifiers::NONE, repeat: false, physical_key: None });
        harness.event(egui::Event::Text("d".to_owned()));
        harness.run();
        assert_eq!(harness.state().composer().map(crate::compose::Composer::subject), Some("d"), "{level:?}: D typed there");
        assert_eq!(harness.state().mailbox().messages().len(), count, "{level:?}: the message stays");
    }
}
