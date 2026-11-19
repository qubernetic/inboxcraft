// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A change of place (M12) at the three levels, frame by frame from the mail to the calendar by Ctrl 2: the new place
//! in the frame of its key, the old body going and the new one coming, a key during it, and rest.

use std::collections::BTreeSet;

use egui::accesskit::{Role, Toggled};
use egui::{Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_time::{FakeClock, Timestamp};

use crate::mailbox::Mailbox;
use crate::motion::Level;
use crate::{InboxApp, sample};

/// The window at the mail at `level`, at rest, frames 10 ms apart.
fn window(level: Level) -> Harness<'static, InboxApp> {
    let clock = || Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()));
    let app = InboxApp::at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    let mut harness = crate::component::tests::builder()
        .with_size(vec2(1300.0, 820.0))
        .with_step_dt(0.01)
        .with_max_steps(100)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.state_mut().set_motion_level(level);
    harness.run();
    harness
}

/// The texts painted inside the body, and their alphas.
fn body(harness: &Harness<'static, InboxApp>) -> Vec<(String, u8)> {
    let Some(regions) = crate::draw::shell::laid(&harness.ctx) else { return Vec::new() };
    let mut out = Vec::new();
    for clipped in &harness.output().shapes {
        let mut add = |shape: &egui::Shape| {
            // An opaque ground over the whole body hides what was painted under it.
            if let egui::Shape::Rect(ground) = shape
                && ground.fill.a() == 255
                && ground.rect.contains_rect(regions.body)
            {
                out.clear();
            }
            if let egui::Shape::Text(text) = shape {
                let rect = text.galley.rect.translate(text.pos.to_vec2()).intersect(clipped.clip_rect);
                if rect.is_positive() && regions.body.contains(rect.center()) && text.galley.job.text.chars().count() >= 4 {
                    out.push((text.galley.job.text.clone(), (text.opacity_factor * f32::from(text.fallback_color.a())) as u8));
                }
            }
        };
        match &clipped.shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(&mut add),
            shape => add(shape),
        }
    }
    out
}

fn words(texts: &[(String, u8)]) -> BTreeSet<String> {
    texts.iter().map(|(text, _)| text.clone()).collect()
}

/// Whether `seen` is `word` partly in glyphs: as long, its first character its own, and no text either place draws at
/// rest (`known`).
fn partly(seen: &str, word: &str, known: &BTreeSet<String>) -> bool {
    seen.chars().count() == word.chars().count() && seen.chars().next() == word.chars().next() && !known.contains(seen)
}

/// The texts of the mail's body and the calendar's at rest, each without the other's; and both.
fn ends() -> (BTreeSet<String>, BTreeSet<String>, BTreeSet<String>) {
    let mut harness = window(Level::Off);
    let mail = words(&body(&harness));
    ctrl_2(&mut harness);
    harness.run();
    let calendar = words(&body(&harness));
    let known = mail.union(&calendar).cloned().collect();
    (mail.difference(&calendar).cloned().collect(), calendar.difference(&mail).cloned().collect(), known)
}

/// Ctrl 2 pressed: the frame of the key is the last drawn.
fn ctrl_2(harness: &mut Harness<'static, InboxApp>) {
    harness.event(egui::Event::Key { key: Key::Num2, pressed: true, modifiers: Modifiers::COMMAND, repeat: false, physical_key: None });
    harness.step();
}

fn on_calendar(harness: &Harness<'static, InboxApp>) -> bool {
    harness.get_by_role_and_label(Role::Button, "Calendar").accesskit_node().toggled() == Some(Toggled::True)
}

#[test]
fn at_off_the_new_place_is_there_in_the_frame_of_its_key() {
    let (_, calendar, _) = ends();
    let mut harness = window(Level::Off);
    ctrl_2(&mut harness);
    assert!(on_calendar(&harness), "the index is on Calendar in the frame of the key");
    let seen = words(&body(&harness));
    assert!(calendar.is_subset(&seen), "the owner never waits: the calendar's body in that frame, {calendar:?} {seen:?}");
    assert!(crate::component::tests::rests(&mut harness, 2));
}

#[test]
fn at_full_the_old_text_scrambles_out_in_125_ms_and_the_new_resolves_in_300() {
    let (mail, calendar, known) = ends();
    let mut harness = window(Level::Full);
    ctrl_2(&mut harness);
    assert!(on_calendar(&harness), "0 ms: the index is on Calendar already");
    let seen = words(&body(&harness));
    assert!(mail.is_subset(&seen), "0 ms: the old body stands, whole: {seen:?}");
    assert!(calendar.is_disjoint(&seen), "0 ms: over the new one");
    harness.run_steps(6);
    let seen = words(&body(&harness));
    assert!(mail.iter().any(|word| seen.iter().any(|text| partly(text, word, &known))), "60 ms: the old text half in glyphs, {seen:?}");
    assert!(seen.intersection(&mail).count() < mail.len() / 2, "60 ms: most of it going");
    assert!(calendar.is_disjoint(&seen), "60 ms: nothing of the calendar yet");
    harness.run_steps(21);
    let seen = words(&body(&harness));
    assert!(calendar.iter().any(|word| seen.iter().any(|text| partly(text, word, &known))), "270 ms: the new text half resolved, {seen:?}");
    assert!(mail.is_disjoint(&seen), "270 ms: the old body has gone");
    harness.run_steps(16);
    assert!(calendar.is_subset(&words(&body(&harness))), "430 ms: the calendar reads");
    assert!(crate::component::tests::rests(&mut harness, 2), "and rests");
}

#[test]
fn at_reduced_the_old_body_fades_out_over_the_new_in_125_ms() {
    let (mail, calendar, _) = ends();
    let mut harness = window(Level::Reduced);
    ctrl_2(&mut harness);
    assert!(on_calendar(&harness));
    harness.run_steps(6);
    let seen = body(&harness);
    let words_seen = words(&seen);
    assert!(calendar.is_subset(&words_seen), "60 ms: the calendar is there, its words its own");
    let old: Vec<u8> = seen.iter().filter(|(text, _)| mail.contains(text)).map(|(_, alpha)| *alpha).collect();
    assert!(!old.is_empty() && old.iter().all(|alpha| (60..200).contains(alpha)), "60 ms: the old body half faded, {old:?}");
    harness.run_steps(7);
    assert!(mail.is_disjoint(&words(&body(&harness))), "130 ms: gone");
    assert!(crate::component::tests::rests(&mut harness, 2), "and rests");
}

#[test]
fn a_key_during_the_change_ends_it_in_that_frame() {
    let (mail, calendar, _) = ends();
    for level in [Level::Full, Level::Reduced] {
        let mut harness = window(level);
        ctrl_2(&mut harness);
        harness.run_steps(4);
        harness.key_press(Key::Tab);
        harness.step();
        let seen = words(&body(&harness));
        assert!(calendar.is_subset(&seen) && mail.is_disjoint(&seen), "{level:?}: the key acts and the change is at its end, {seen:?}");
    }
}

#[test]
fn a_press_on_the_index_changes_the_place_in_the_frame_of_the_press() {
    let (mail, calendar, _) = ends();
    for level in [Level::Full, Level::Reduced] {
        let mut harness = window(level);
        harness.get_by_role_and_label(Role::Button, "Calendar").click();
        harness.step();
        assert!(on_calendar(&harness), "{level:?}: the index is on Calendar in the frame of the press");
        let seen = words(&body(&harness));
        assert!(mail.is_subset(&seen), "{level:?} 0 ms: the old body stands over the new, {seen:?}");
        assert!(crate::component::tests::rests(&mut harness, 60), "{level:?}: and rests");
        assert!(calendar.is_subset(&words(&body(&harness))), "{level:?}: the calendar at the end");
    }
}

#[test]
fn a_floating_composer_is_neither_painted_over_nor_scrambled_by_a_change() {
    let mut harness = window(Level::Full);
    let ctx = harness.ctx.clone();
    harness.state_mut().run_action(&ctx, crate::keymap::COMPOSE);
    harness.run();
    harness.get_by_role_and_label(Role::TextInput, "Subject").focus();
    harness.run();
    harness.get_by_role_and_label(Role::TextInput, "Subject").type_text("Seedlings");
    harness.run();
    harness.state_mut().run_action(&ctx, crate::keymap::CALENDAR);
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Back to the reader's place");
    if let Some(focused) = harness.ctx.memory(|memory| memory.focused()) {
        harness.ctx.memory_mut(|memory| memory.surrender_focus(focused));
    }
    harness.run();
    assert!(body(&harness).iter().any(|(text, _)| text == "Seedlings"), "at rest the draft's subject shows");
    harness.event(egui::Event::Key { key: Key::Num0, pressed: true, modifiers: Modifiers::COMMAND, repeat: false, physical_key: None });
    harness.step();
    for frame in 0..40 {
        let seen = body(&harness);
        assert!(
            seen.iter().any(|(text, alpha)| text == "Seedlings" && *alpha == 255),
            "frame {frame}: the draft's subject whole and in front, {seen:?}"
        );
        harness.step();
    }
}
