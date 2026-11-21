// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The text size's keys: Ctrl and Plus (or `=`), Ctrl and Minus, Ctrl and 0 step the text size of the settings
//! (`ui.text_scale.set`) by one of its five steps, or back to 100 %, and the status line says the size; at an end the
//! size stays and the line says so. Ctrl and 0 no longer shows Home, egui's own zoom stays off, and the Shortcuts page
//! lists the keys and gives them new ones as any other's.

use egui::accesskit::Role;
use egui::{Key, KeyboardShortcut, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use inboxcraft_time::{FakeClock, Timestamp};

use crate::draw::{InboxApp, Screen};
use crate::keymap::{self, Keymap};
use crate::mailbox::Mailbox;
use crate::sample;
use crate::theme::TextScale;

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

fn window() -> Harness<'static, InboxApp> {
    let app = InboxApp::at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    let mut harness = Harness::builder().with_size(vec2(1440.0, 900.0)).with_max_steps(12).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

/// Ctrl (the platform's command key) and `key`.
fn command(harness: &mut Harness<'static, InboxApp>, key: Key) {
    harness.key_press_modifiers(Modifiers::COMMAND, key);
    harness.run();
}

fn percent(harness: &Harness<'static, InboxApp>) -> u16 {
    harness.state().text_scale().percent()
}

#[test]
fn each_key_steps_the_text_size_by_one_and_the_status_line_says_the_size() {
    let mut harness = window();
    assert_eq!(percent(&harness), 100);
    command(&mut harness, Key::Plus);
    assert_eq!(percent(&harness), 110, "Ctrl Plus: one step larger");
    assert_eq!(harness.state().notice(), Some("Text size 110 %"));
    command(&mut harness, Key::Equals);
    assert_eq!(percent(&harness), 120, "Ctrl =, the Plus key without Shift: one step larger too");
    assert_eq!(harness.state().notice(), Some("Text size 120 %"));
    command(&mut harness, Key::Minus);
    assert_eq!(percent(&harness), 110, "Ctrl Minus: one step smaller");
    assert_eq!(harness.state().notice(), Some("Text size 110 %"));
    command(&mut harness, Key::Minus);
    command(&mut harness, Key::Minus);
    assert_eq!(percent(&harness), 90);
    command(&mut harness, Key::Num0);
    assert_eq!(percent(&harness), 100, "Ctrl 0: back to 100 %");
    assert_eq!(harness.state().notice(), Some("Text size 100 %"));
    assert_eq!(harness.ctx.zoom_factor(), 1.0, "egui's own zoom stays off");
    assert!(!harness.ctx.options(|options| options.zoom_with_keyboard));
}

/// A US keyboard sends Ctrl+Plus as Ctrl and Shift with the logical `+`: the extra Shift must not stop the step.
#[test]
fn ctrl_shift_plus_as_a_us_keyboard_sends_it_grows_the_text() {
    let mut harness = window();
    harness.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::Plus);
    harness.run();
    assert_eq!(percent(&harness), 110, "Ctrl Shift + (Ctrl Plus on a US keyboard): one step larger");
    assert_eq!(harness.state().notice(), Some("Text size 110 %"));
}

#[test]
fn ctrl_0_no_longer_shows_home_and_home_has_no_key_at_the_start() {
    let mut harness = window();
    assert_eq!(Keymap::default().shortcut(keymap::HOME), None);
    assert_eq!(Keymap::default().shortcut(keymap::TEXT_RESET), Some(KeyboardShortcut::new(Modifiers::COMMAND, Key::Num0)));
    harness.state_mut().set_text_scale(TextScale::from_percent(120).unwrap());
    harness.run();
    command(&mut harness, Key::Num0);
    assert_eq!(harness.state().screen(), Screen::Mail, "Ctrl 0 stays on the mail");
    assert_eq!(percent(&harness), 100);
    // Home is still a press on the index's HOME away.
    harness.get_by_role_and_label(Role::Button, "Home").click();
    // Home's emblem twinkles on by itself (#418): the window does not rest there.
    harness.run_steps(4);
    assert_eq!(harness.state().screen(), Screen::Home);
}

#[test]
fn at_an_end_the_size_stays_and_the_status_line_says_it_is_the_largest_or_the_smallest() {
    let mut harness = window();
    harness.state_mut().set_text_scale(TextScale::from_percent(130).unwrap());
    harness.run();
    command(&mut harness, Key::Plus);
    assert_eq!(percent(&harness), 130);
    assert_eq!(harness.state().notice(), Some("Text size 130 %, the largest"));
    command(&mut harness, Key::Equals);
    assert_eq!(percent(&harness), 130);
    harness.state_mut().set_text_scale(TextScale::from_percent(90).unwrap());
    harness.run();
    command(&mut harness, Key::Minus);
    assert_eq!(percent(&harness), 90);
    assert_eq!(harness.state().notice(), Some("Text size 90 %, the smallest"));
}

#[test]
fn a_newer_word_on_the_text_size_takes_the_place_of_the_older_in_the_status_line() {
    let mut harness = window();
    for _ in 0..4 {
        command(&mut harness, Key::Plus);
    }
    assert_eq!(percent(&harness), 130);
    assert_eq!(harness.state().notice(), Some("Text size 130 %, the largest"));
    harness.get_by_label("Text size 130 %, the largest");
    for older in ["Text size 110 %", "Text size 120 %", "Text size 130 %"] {
        assert!(harness.query_by_label(older).is_none(), "{older}: no older word waits to show first");
    }
}

#[test]
fn the_shortcuts_page_lists_the_keys_and_a_key_given_there_works_and_the_old_one_does_not() {
    let mut harness = window();
    let ctx = harness.ctx.clone();
    harness.state_mut().run_action(&ctx, keymap::SHORTCUTS);
    harness.run();
    harness.get_by_label("Key of Larger text: Ctrl Plus");
    harness.get_by_label("Key of Larger text (=): Ctrl Equals");
    harness.get_by_label("Key of Smaller text: Ctrl Minus");
    harness.get_by_label("Key of Text size 100 %: Ctrl 0");
    harness.get_by_label("Show Home has no key");
    harness.get_by_label("Key of Larger text: Ctrl Plus").click_accesskit();
    harness.run();
    assert_eq!(harness.state().settings().capturing(), Some(keymap::TEXT_LARGER));
    harness.key_press_modifiers(Modifiers::COMMAND, Key::L);
    harness.run();
    assert_eq!(harness.state().keymap().shortcut(keymap::TEXT_LARGER), Some(KeyboardShortcut::new(Modifiers::COMMAND, Key::L)));
    harness.state_mut().leave_settings();
    harness.run();
    command(&mut harness, Key::Plus);
    assert_eq!(percent(&harness), 100, "the old key does nothing now");
    command(&mut harness, Key::L);
    assert_eq!(percent(&harness), 110, "the new key steps the size");
}

#[test]
fn the_step_and_its_words_for_every_size() {
    for scale in TextScale::ALL {
        let (larger, _) = keymap::text_size_step(keymap::TEXT_LARGER, scale);
        let (smaller, _) = keymap::text_size_step(keymap::TEXT_SMALLER, scale);
        assert_eq!(larger, scale.larger().unwrap_or(scale));
        assert_eq!(smaller, scale.smaller().unwrap_or(scale));
        assert_eq!(keymap::text_size_step(keymap::TEXT_RESET, scale).0, TextScale::default());
        assert_eq!(keymap::text_size_step("ui.not.an.action", scale).0, scale);
    }
    let at = |percent| TextScale::from_percent(percent).unwrap();
    assert_eq!(at(100).larger(), Some(at(110)));
    assert_eq!(at(130).larger(), None);
    assert_eq!(at(100).smaller(), Some(at(90)));
    assert_eq!(at(90).smaller(), None);
    assert!(keymap::is_text_size_notice("Text size 120 %"));
    assert!(keymap::is_text_size_notice("Text size 90 %, the smallest"));
    assert!(!keymap::is_text_size_notice("Draft saved"));
}
