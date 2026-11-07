// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The Shortcuts page of the settings sheet: every action with its key, a new key from the next press, the keys
//! kept between runs; and the sheet's one open state, forgotten by every way out.

use egui::accesskit::Role;
use egui::{Key, KeyboardShortcut, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_model::Role as FolderRole;
use inboxcraft_time::{FakeClock, Timestamp};

use super::InboxApp;
use crate::keep::{FILE, Keep, KeepFile};
use crate::keymap::{self, Keymap};
use crate::mailbox::{Mailbox, Place};
use crate::sample;
use crate::settings::Section;

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

fn build(app: InboxApp, theme: egui::Theme) -> Harness<'static, InboxApp> {
    let mut harness = Harness::builder()
        .with_size(vec2(1200.0, 820.0))
        .with_theme(theme)
        .with_max_steps(12)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

fn app() -> InboxApp {
    InboxApp::new(Mailbox::new(sample::session(clock()).unwrap(), clock()))
}

/// The sheet open on Shortcuts, through its section in the nav.
fn on_shortcuts(theme: egui::Theme) -> Harness<'static, InboxApp> {
    let mut harness = build(app(), theme);
    harness.get_by_label("Appearance settings").click();
    harness.run();
    harness.get_by_label("Shortcuts, settings page").click();
    harness.run();
    assert_eq!(harness.state().settings().section(), Section::Shortcuts);
    harness
}

/// Presses the key chip of the action `name` as a screen reader does; it may lie below the fold.
fn press_chip(harness: &mut Harness<'static, InboxApp>, starts: &str) {
    let chip = harness.query_all_by_role(Role::Button).find(|node| node.accesskit_node().label().is_some_and(|label| label.starts_with(starts)));
    chip.unwrap_or_else(|| panic!("no chip starting {starts:?}")).click_accesskit();
    harness.run();
}

fn press_beside_the_sheet(harness: &mut Harness<'static, InboxApp>) {
    let sheet = harness.get_by_role_and_label(Role::Dialog, "Settings").rect();
    let at = egui::pos2(sheet.left() / 2.0, sheet.center().y);
    harness.hover_at(at);
    harness.run();
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
        harness.run();
    }
}

#[test]
fn the_shortcuts_page_lists_every_action_with_its_key() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let harness = on_shortcuts(theme);
        assert!(!harness.get_by_label("Shortcuts, settings page").accesskit_node().is_disabled());
        harness.get_by_label("Key of Command palette: Ctrl+K");
        harness.get_by_label("Key of Search mail: Ctrl+F");
        harness.get_by_label("Show sent mail has no key");
        harness.get_by_role_and_label(Role::TextInput, "Find a command");
        harness.get_by_role_and_label(Role::Button, "Reset all keys");
        harness.get_by_role_and_label(Role::Button, "Remove the key of Command palette");
        harness.check_accessibility();
    }
}

#[test]
fn a_press_on_a_key_waits_for_the_next_key_which_becomes_the_action_s() {
    let mut harness = on_shortcuts(egui::Theme::Light);
    press_chip(&mut harness, "Show sent mail has no key");
    assert_eq!(harness.state().settings().capturing(), Some(keymap::SENT));
    harness.get_by_label("Show sent mail: press a key, or Escape to keep the old one");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Num3);
    harness.run();
    assert_eq!(harness.state().settings().capturing(), None);
    assert_eq!(harness.state().keymap().shortcut(keymap::SENT), Some(KeyboardShortcut::new(Modifiers::COMMAND, Key::Num3)));
    harness.get_by_label("Key of Show sent mail: Ctrl+3");
    assert!(harness.state().settings().is_open(), "the press only gave the key");
    // The new key works at once.
    harness.state_mut().close_settings();
    harness.run();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Num3);
    harness.run();
    assert_eq!(harness.state().mailbox().place(), Place::View(FolderRole::Sent));
}

#[test]
fn a_key_another_action_had_moves_and_the_page_says_so() {
    let mut harness = on_shortcuts(egui::Theme::Light);
    press_chip(&mut harness, "Show sent mail has no key");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::K);
    harness.run();
    assert_eq!(harness.state().keymap().shortcut(keymap::SENT), Some(KeyboardShortcut::new(Modifiers::COMMAND, Key::K)));
    assert_eq!(harness.state().keymap().shortcut(keymap::PALETTE), None, "one key runs one action");
    assert!(!harness.state().palette().is_open(), "the press gave a key and opened nothing");
    harness.get_by_label("Ctrl+K was the key of Command palette; it has none now.");
    harness.get_by_label("Command palette has no key");
    // Giving an action its own key again says nothing.
    press_chip(&mut harness, "Key of Show sent mail");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::K);
    harness.run();
    assert_eq!(harness.state().settings().keys_note(), None);
}

#[test]
fn escape_while_waiting_keeps_the_old_key_and_the_sheet() {
    let mut harness = on_shortcuts(egui::Theme::Light);
    press_chip(&mut harness, "Key of Command palette");
    harness.key_press(Key::Escape);
    harness.run();
    assert_eq!(harness.state().settings().capturing(), None);
    assert!(harness.state().settings().is_open(), "Escape stops the wait only");
    assert_eq!(harness.state().keymap(), &Keymap::default());
}

#[test]
fn remove_takes_a_key_away_and_reset_gives_every_action_its_first_key() {
    let mut harness = on_shortcuts(egui::Theme::Light);
    harness.get_by_role_and_label(Role::Button, "Remove the key of Command palette").click();
    harness.run();
    assert_eq!(harness.state().keymap().shortcut(keymap::PALETTE), None);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::K);
    harness.run();
    assert!(!harness.state().palette().is_open(), "a key taken away runs nothing");
    harness.get_by_role_and_label(Role::Button, "Reset all keys").click();
    harness.run();
    assert_eq!(harness.state().keymap(), &Keymap::default());
}

#[test]
fn the_field_narrows_the_list() {
    let mut harness = on_shortcuts(egui::Theme::Light);
    harness.get_by_role_and_label(Role::TextInput, "Find a command").click();
    harness.run();
    harness.get_by_role_and_label(Role::TextInput, "Find a command").type_text("sent");
    harness.run();
    assert_eq!(harness.state().settings().find(), "sent");
    harness.get_by_label("Show sent mail has no key");
    assert!(harness.query_by_label("Key of Command palette: Ctrl+K").is_none());
    harness.get_by_role_and_label(Role::TextInput, "Find a command").type_text(" zzz");
    harness.run();
    harness.get_by_label("No command matches “sent zzz”.");
}

#[test]
fn a_key_without_ctrl_or_alt_types_into_a_field_instead_of_running() {
    let mut harness = on_shortcuts(egui::Theme::Light);
    press_chip(&mut harness, "Key of Show the calendar");
    harness.key_press(Key::G);
    harness.run();
    assert_eq!(harness.state().keymap().shortcut(keymap::CALENDAR), Some(KeyboardShortcut::new(Modifiers::NONE, Key::G)));
    harness.key_press(Key::G);
    harness.run();
    assert!(harness.state().settings().is_open(), "G runs nothing while the sheet is open");
    harness.state_mut().close_settings();
    harness.run();
    harness.state_mut().search("");
    harness.ctx.memory_mut(|memory| memory.request_focus(super::list::search_id()));
    harness.run();
    harness.key_press(Key::G);
    harness.run();
    assert_eq!(harness.state().screen(), super::Screen::Mail, "G in the search field is typed");
    harness.ctx.memory_mut(|memory| memory.surrender_focus(super::list::search_id()));
    harness.run();
    harness.key_press(Key::G);
    harness.run();
    assert_eq!(harness.state().screen(), super::Screen::Calendar, "G shows the calendar where nothing is typed");
}

#[test]
fn the_keys_are_kept_between_runs() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(FILE);
    let mut harness = build(app().with_keep(Box::new(KeepFile::new(path.clone()))), egui::Theme::Light);
    harness.state_mut().open_settings();
    harness.run();
    harness.get_by_label("Shortcuts, settings page").click();
    harness.run();
    press_chip(&mut harness, "Show sent mail has no key");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Num3);
    harness.run();
    let kept = KeepFile::new(path.clone()).load_keys().unwrap();
    assert_eq!(kept.shortcut(keymap::SENT), Some(KeyboardShortcut::new(Modifiers::COMMAND, Key::Num3)));
    let again = app().with_keep(Box::new(KeepFile::new(path)));
    assert_eq!(again.keymap(), &kept, "the next run starts with them");
}

#[test]
fn keys_that_cannot_be_kept_show_in_the_error_banner() {
    let dir = tempfile::tempdir().unwrap();
    let blocker = dir.path().join("blocker");
    std::fs::write(&blocker, b"a file where the folder should be").unwrap();
    let mut harness = build(app().with_keep(Box::new(KeepFile::new(blocker.join(FILE)))), egui::Theme::Light);
    harness.state_mut().keymap_mut().unbind(keymap::PALETTE);
    harness.run();
    assert!(harness.state().mailbox().error().is_some_and(|error| error.contains("blocker")), "{:?}", harness.state().mailbox().error());
}

/// The sheet on Shortcuts in the state an open sheet can hold: a text in the field and an action waiting for a key.
fn busy_sheet() -> Harness<'static, InboxApp> {
    let mut harness = on_shortcuts(egui::Theme::Light);
    harness.state_mut().settings_mut().find_mut().push_str("show");
    harness.run();
    press_chip(&mut harness, "Key of Show the calendar");
    assert_eq!(harness.state().settings().capturing(), Some(keymap::CALENDAR));
    harness
}

fn assert_closed_clean(harness: &mut Harness<'static, InboxApp>) {
    let settings = harness.state().settings();
    assert!(!settings.is_open());
    assert_eq!((settings.capturing(), settings.find(), settings.keys_note()), (None, "", None), "the open state is forgotten");
    assert_eq!(harness.state().keymap(), &Keymap::default(), "no key was given on the way out");
    assert!(harness.query_by_role_and_label(Role::Dialog, "Settings").is_none());
    // The next opening is clean.
    harness.state_mut().open_settings();
    harness.run();
    assert_eq!(harness.state().settings().section(), Section::Appearance);
    assert_eq!(harness.state().settings().capturing(), None);
}

#[test]
fn the_close_button_forgets_the_open_state() {
    let mut harness = busy_sheet();
    harness.get_by_role_and_label(Role::Button, "Close").click();
    harness.run();
    assert_closed_clean(&mut harness);
}

#[test]
fn escape_forgets_the_open_state() {
    let mut harness = busy_sheet();
    // The first Escape stops the wait, the second leaves the sheet.
    harness.key_press(Key::Escape);
    harness.run();
    assert!(harness.state().settings().is_open());
    harness.key_press(Key::Escape);
    harness.run();
    assert_closed_clean(&mut harness);
}

#[test]
fn a_press_beside_the_sheet_forgets_the_open_state() {
    let mut harness = busy_sheet();
    press_beside_the_sheet(&mut harness);
    assert_closed_clean(&mut harness);
}

#[test]
fn the_rail_and_the_keys_that_show_another_place_forget_the_open_state() {
    for leave in ["Mail", "Calendar", "Sent mail", "Search mail", "Appearance settings"] {
        let mut harness = busy_sheet();
        harness.get_by_role_and_label(Role::Button, leave).click();
        harness.run();
        assert_closed_clean(&mut harness);
    }
    let mut harness = busy_sheet();
    harness.key_press(Key::Escape);
    harness.run();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::K);
    harness.run();
    assert!(harness.state().palette().is_open());
    harness.state_mut().palette_mut().close();
    harness.run();
    assert_closed_clean(&mut harness);
}
