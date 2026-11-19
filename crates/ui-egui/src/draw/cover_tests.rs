// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The palette coming over an open float, and the settings shown in its place's stead: a list, a menu or a picker open
//! closes (it picks, sends and changes nothing, and asks for no keyboard), so nothing stands over them or takes their
//! Esc; the held floats (the event editor, the floating composer) stay as they are, and show again with their place.

use egui::accesskit::Role;
use egui::{Key, LayerId, Modifiers, Order, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_time::{FakeClock, Timestamp};

use crate::calendar::View;
use crate::component::popup;
use crate::keymap;
use crate::mailbox::Mailbox;
use crate::{InboxApp, sample};

/// 2026-11-05T09:00:00Z.
const NOW: i64 = 1_793_869_200;

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(NOW, 0).unwrap()))
}

fn harness(calendar: bool) -> Harness<'static, InboxApp> {
    let mut app = InboxApp::new(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    if calendar {
        app.show_calendar();
        let (calendar, mailbox) = app.calendar_mut();
        calendar.set_view(mailbox, View::Week);
    }
    let mut harness = Harness::builder()
        .with_size(vec2(1440.0, 900.0))
        .with_theme(egui::Theme::Dark)
        .with_max_steps(40)
        .with_step_dt(1.0 / 60.0)
        .with_context_setup(crate::theme::install_fonts)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

/// The calendar's week with New event open, "Kiln" typed in its title and its Repeat list open.
fn editor_with_its_list_open() -> Harness<'static, InboxApp> {
    let mut harness = harness(true);
    harness.get_by_role_and_label(Role::Button, "New event").click();
    harness.run();
    harness.get_by_role_and_label(Role::TextInput, "Title").focus();
    harness.get_by_role_and_label(Role::TextInput, "Title").type_text("Kiln");
    harness.run();
    harness.get_by_role_and_label(Role::ComboBox, "Repeat").click();
    harness.run();
    assert!(repeat_list(&harness), "the Repeat list is open");
    harness
}

/// Whether the event editor's Repeat list is open.
fn repeat_list(harness: &Harness<'static, InboxApp>) -> bool {
    harness.query_by_label("Weekdays").is_some()
}

fn summary(harness: &Harness<'static, InboxApp>) -> Option<String> {
    harness.state().calendar().editor().map(|editor| editor.form().summary.clone())
}

/// The mail with a new message in the composer and its Send later picker open.
fn composer_with_its_picker_open() -> Harness<'static, InboxApp> {
    let mut harness = harness(false);
    let ctx = harness.ctx.clone();
    harness.state_mut().run_action(&ctx, keymap::COMPOSE);
    harness.run();
    harness.get_by_role_and_label(Role::TextInput, "Subject").focus();
    harness.get_by_role_and_label(Role::TextInput, "Subject").type_text("Plot");
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Send later").click();
    harness.run();
    assert!(picker_open(&harness), "the Send later picker is open");
    harness
}

fn picker_open(harness: &Harness<'static, InboxApp>) -> bool {
    popup::is_open(&harness.ctx, super::compose::later_popup())
}

/// Whether the Send later picker was drawn in the last frame, anywhere.
fn picker_drawn(harness: &Harness<'static, InboxApp>) -> bool {
    let layer = LayerId::new(Order::Foreground, super::compose::later_popup());
    harness.ctx.memory(|memory| memory.areas().is_visible(&layer))
}

fn subject(harness: &Harness<'static, InboxApp>) -> Option<String> {
    harness.state().composer().map(|composer| composer.subject().to_owned())
}

/// Shows the settings by their key.
fn open_settings(harness: &mut Harness<'static, InboxApp>) {
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Comma);
    harness.run();
    assert!(harness.state().settings().is_open());
}

/// The settings' key again: the place the owner came from shows.
fn open_settings_again(harness: &mut Harness<'static, InboxApp>) {
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Comma);
    harness.run();
    assert!(!harness.state().settings().is_open());
}

fn open_palette(harness: &mut Harness<'static, InboxApp>) {
    harness.key_press_modifiers(Modifiers::COMMAND, Key::K);
    harness.run();
    assert!(harness.state().palette().is_open());
}

fn esc(harness: &mut Harness<'static, InboxApp>) {
    harness.key_press(Key::Escape);
    harness.run();
}

/// Whether the control with the keyboard lies on the settings; no keyboard anywhere counts as on them.
fn keyboard_on_the_settings(harness: &Harness<'static, InboxApp>) -> bool {
    let settings = harness.get_by_role_and_label(Role::Pane, "Settings").rect();
    harness.root().children_recursive().find(|node| node.accesskit_node().is_focused()).is_none_or(|node| settings.contains_rect(node.rect()))
}

#[test]
fn the_settings_shown_from_the_editor_close_its_open_list_and_keep_its_typing() {
    let mut harness = editor_with_its_list_open();
    open_settings(&mut harness);
    assert!(!repeat_list(&harness), "the list of a place no longer drawn closed");
    assert!(!popup::any_open(&harness.ctx), "nothing says it is open");
    assert!(keyboard_on_the_settings(&harness), "the closed list holds no keyboard: {:?}", focused(&harness));
    esc(&mut harness);
    assert!(harness.state().settings().is_open(), "Esc closes no place");
    open_settings_again(&mut harness);
    assert!(!repeat_list(&harness), "back on the calendar the list stays closed");
    assert_eq!(summary(&harness).as_deref(), Some("Kiln"), "the editor keeps its typing");
    let editor = harness.state().calendar().editor();
    assert!(editor.is_some_and(|editor| !editor.warned()), "the editor is open and saw no Esc");
}

#[test]
fn the_settings_shown_from_the_composer_close_its_picker_and_keep_the_draft() {
    let mut harness = composer_with_its_picker_open();
    open_settings(&mut harness);
    assert!(!picker_drawn(&harness), "the picker of a place no longer drawn is not drawn over the settings");
    assert!(!popup::any_open(&harness.ctx), "nothing says it is open");
    assert_eq!(harness.ctx.memory(|memory| memory.top_modal_layer()), None, "a place holds no modal layer");
    assert!(keyboard_on_the_settings(&harness), "the closed picker holds no keyboard: {:?}", focused(&harness));
    esc(&mut harness);
    assert!(harness.state().settings().is_open(), "Esc closes no place");
    open_settings_again(&mut harness);
    assert!(!picker_open(&harness), "the picker stays closed");
    let composer = harness.state().composer();
    assert!(composer.is_some_and(|composer| composer.later().is_none()), "the closed picker picked no time");
    assert_eq!(subject(&harness).as_deref(), Some("Plot"), "and nothing typed is lost");
}

#[test]
fn the_palette_over_the_composer_closes_its_picker_and_one_esc_closes_the_palette() {
    let mut harness = composer_with_its_picker_open();
    open_palette(&mut harness);
    assert!(!picker_open(&harness), "the picker closed when the palette opened");
    assert!(!picker_drawn(&harness), "nothing stands over the palette");
    let modal = harness.ctx.memory(|memory| memory.top_modal_layer());
    assert_ne!(modal, Some(LayerId::new(Order::Foreground, super::compose::later_popup())), "the picker holds no modal layer");
    esc(&mut harness);
    assert!(!harness.state().palette().is_open(), "one Esc closes the palette");
    assert!(!picker_open(&harness), "the picker stays closed");
    let composer = harness.state().composer();
    assert!(composer.is_some_and(|composer| composer.later().is_none()), "the closed picker picked no time");
    assert_eq!(subject(&harness).as_deref(), Some("Plot"), "and nothing typed is lost");
}

/// The label of the control that has the keyboard, if one has it.
fn focused(harness: &Harness<'static, InboxApp>) -> Option<String> {
    harness.root().children_recursive().find(|node| node.accesskit_node().is_focused()).and_then(|node| node.accesskit_node().label())
}

/// The mail with its first message open in the reader and Snooze on Ctrl+H.
fn reading_with_keys_on_ctrl() -> Harness<'static, InboxApp> {
    let mut harness = harness(false);
    let first = harness.state().mailbox().messages().first().map(|row| row.id).unwrap();
    harness.state_mut().mailbox_mut().select(Some(first));
    let keymap = harness.state_mut().keymap_mut();
    keymap.bind(keymap::SNOOZE, egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::H));
    harness.run();
    assert!(harness.state().can_run(keymap::SNOOZE), "Snooze can run on the open message");
    harness
}

#[test]
fn on_the_settings_a_key_with_ctrl_runs_nothing_of_the_mail_and_new_message_leaves_them() {
    let mut harness = reading_with_keys_on_ctrl();
    let screen = format!("{:?}", harness.state().screen());
    open_settings(&mut harness);
    // Snooze acts on the mail, which the settings do not show: its key rests.
    harness.key_press_modifiers(Modifiers::COMMAND, Key::H);
    harness.run();
    harness.run();
    assert!(harness.state().settings().is_open(), "the settings stay");
    assert!(harness.query_by_label("Snooze until").is_none() && !popup::any_open(&harness.ctx), "no float opened");
    assert!(harness.state().composer().is_none(), "no message begun");
    assert_eq!(format!("{:?}", harness.state().screen()), screen, "the place left stays");
    assert!(keyboard_on_the_settings(&harness), "the keyboard is nowhere off the settings: {:?}", focused(&harness));
    // The palette comes over the settings, which stay under it.
    open_palette(&mut harness);
    assert!(harness.state().settings().is_open(), "the palette leaves the settings shown");
    esc(&mut harness);
    assert!(!harness.state().palette().is_open() && harness.state().settings().is_open(), "one Esc, the palette's");
    // New message, its button pressed, leaves the settings for the mail and begins a message.
    harness.get_by_role_and_label(Role::Button, "New message").click();
    harness.run();
    harness.run();
    assert!(!harness.state().settings().is_open(), "New message leaves the settings");
    assert!(harness.state().composer().is_some(), "and begins a message");
    // Off the settings the same Snooze key runs.
    let mut harness = reading_with_keys_on_ctrl();
    open_settings(&mut harness);
    open_settings_again(&mut harness);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::H);
    harness.run();
    harness.run();
    assert!(harness.query_by_label("Snooze until").is_some(), "Ctrl+H opens Snooze over the reader");
}

#[test]
fn the_new_message_button_on_the_settings_begins_a_message_with_the_default_keys_by_a_press_and_by_activate() {
    for activate in [false, true] {
        let mut harness = harness(false);
        open_settings(&mut harness);
        let button = harness.get_by_role_and_label(Role::Button, "New message");
        if activate {
            button.click_accesskit();
        } else {
            button.click();
        }
        harness.run();
        harness.run();
        assert!(!harness.state().settings().is_open(), "activate {activate}: New message leaves the settings");
        assert!(harness.state().composer().is_some(), "activate {activate}: and begins a message, its key C being bare");
    }
}

#[test]
fn on_the_settings_an_index_place_whose_key_is_bare_acts_by_a_press_and_by_activate() {
    let rows = [
        ("Mail", keymap::MAIL, Key::F5),
        ("Calendar", keymap::CALENDAR, Key::F6),
        ("Later", keymap::LATER, Key::F7),
        ("Settings", keymap::SETTINGS, Key::F8),
    ];
    for activate in [false, true] {
        for (name, action, key) in rows {
            let mut harness = harness(false);
            harness.state_mut().keymap_mut().bind(action, egui::KeyboardShortcut::new(Modifiers::NONE, key));
            let ctx = harness.ctx.clone();
            harness.state_mut().run_action(&ctx, keymap::SETTINGS);
            harness.run();
            assert!(harness.state().settings().is_open());
            let index = harness.get_by_role_and_label(Role::Pane, "Settings").rect();
            let row = harness.get_all_by_role_and_label(Role::Button, name).find(|node| !index.contains_rect(node.rect())).unwrap();
            if activate {
                row.click_accesskit();
            } else {
                row.click();
            }
            harness.run();
            harness.run();
            assert!(!harness.state().settings().is_open(), "{name} on {key:?}, activate {activate}: the row leaves the settings");
        }
    }
}

#[test]
fn on_the_settings_a_bare_key_typed_into_a_field_is_typed_and_runs_nothing() {
    let mut harness = harness(false);
    open_settings(&mut harness);
    super::tests::at_settings(harness.state_mut(), crate::settings::Section::Shortcuts);
    harness.run();
    harness.get_by_role_and_label(Role::TextInput, "Find a command").focus();
    harness.run();
    harness.key_press(Key::C);
    harness.event(egui::Event::Text("c".to_owned()));
    harness.run();
    harness.run();
    assert!(harness.state().settings().is_open(), "the settings stay");
    assert!(harness.state().composer().is_none(), "C, New message's key, begins nothing");
    assert_eq!(harness.state().settings().find(), "c", "it is typed into the field");
}

#[test]
fn j_walks_the_list_at_once_after_leaving_the_settings_by_their_key() {
    let mut harness = harness(false);
    harness.run();
    harness.key_press(Key::J);
    harness.run();
    harness.run();
    let first = harness.state().mailbox().selected();
    harness.key_press(Key::J);
    harness.run();
    harness.run();
    let second = harness.state().mailbox().selected();
    assert!(first.is_some() && second != first, "J walks before: {first:?} {second:?}");
    open_settings(&mut harness);
    open_settings_again(&mut harness);
    harness.key_press(Key::J);
    harness.run();
    harness.run();
    let third = harness.state().mailbox().selected();
    assert!(third != second, "J walks at once after leaving by the key: {second:?} -> {third:?}");
}

/// The mail's first message archived, its toast with Undo showing; then the settings shown.
fn archived_then_settings() -> (Harness<'static, InboxApp>, u64) {
    let mut harness = harness(false);
    let first = harness.state().mailbox().messages().first().map(|row| row.id).unwrap();
    harness.state_mut().mailbox_mut().act(first, crate::mailbox::Act::Archive);
    harness.run();
    assert!(harness.query_by_role_and_label(Role::Button, "Undo").is_some(), "the toast shows its Undo");
    open_settings(&mut harness);
    harness.ctx.memory_mut(|memory| {
        if let Some(focused) = memory.focused() {
            memory.surrender_focus(focused);
        }
    });
    harness.run();
    (harness, first)
}

fn archived(harness: &Harness<'static, InboxApp>, id: u64) -> bool {
    harness.state().mailbox().messages().iter().all(|row| row.id != id)
}

#[test]
fn on_the_settings_the_undo_key_takes_back_what_the_toast_shown_names() {
    let (mut harness, first) = archived_then_settings();
    assert!(archived(&harness, first));
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run();
    harness.run();
    assert!(!archived(&harness, first), "Ctrl+Z takes the archive back, as the toast's Undo would");
    assert!(harness.state().settings().is_open(), "the settings stay");
}

#[test]
fn on_the_settings_the_undo_key_in_a_field_is_the_field_s() {
    let (mut harness, first) = archived_then_settings();
    super::tests::at_settings(harness.state_mut(), crate::settings::Section::Shortcuts);
    harness.run();
    harness.get_by_role_and_label(Role::TextInput, "Find a command").focus();
    harness.run();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run();
    harness.run();
    assert!(archived(&harness, first), "a field with the keyboard keeps its Undo: the archive stays");
}
