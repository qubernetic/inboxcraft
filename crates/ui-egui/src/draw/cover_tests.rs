// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The settings sheet and the palette coming over an open float: a list, a menu or a picker open under them closes
//! (it picks, sends and changes nothing, and asks for no keyboard), so nothing under the sheet stands over it, keeps
//! `Tab` or takes its Esc and its press beside; the held floats (the event editor, the floating composer) stay as they
//! are, quiet under it.

use egui::accesskit::Role;
use egui::{Event, Id, Key, LayerId, Modifiers, Order, PointerButton, vec2};
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

fn open_sheet(harness: &mut Harness<'static, InboxApp>) {
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Comma);
    harness.run();
    assert!(harness.state().settings().is_open());
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

/// A press of the pointer beside the sheet, down and up.
fn press_beside(harness: &mut Harness<'static, InboxApp>) {
    let sheet = harness.get_by_role_and_label(Role::Dialog, "Settings").rect();
    let beside = egui::pos2(sheet.left() / 2.0, sheet.center().y);
    harness.hover_at(beside);
    harness.run();
    for pressed in [true, false] {
        harness.event(Event::PointerButton { pos: beside, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
        harness.run();
    }
    harness.run();
}

/// The label of the control with the keyboard, and whether it lies in the settings sheet.
fn focus(harness: &Harness<'static, InboxApp>) -> Option<(String, bool)> {
    let sheet = harness.get_by_role_and_label(Role::Dialog, "Settings").rect();
    let focused = harness.ctx.memory(|memory| memory.focused())?;
    let response = harness.ctx.read_response(focused)?;
    let label = harness.root().children_recursive().find(|node| node.accesskit_node().is_focused()).and_then(|node| node.accesskit_node().label());
    Some((label.unwrap_or_default(), sheet.contains_rect(response.rect)))
}

/// Walks `Tab` over the sheet: every stop lies in it, and the walk comes round to its first stop.
fn tab_goes_round_the_sheet(harness: &mut Harness<'static, InboxApp>) {
    let mut stops = Vec::new();
    for step in 0..40 {
        harness.key_press(Key::Tab);
        harness.run();
        let (label, inside) = focus(harness).unwrap_or_default();
        assert!(inside, "Tab {step} stopped outside the sheet, on {label:?}");
        stops.push(label);
    }
    let first = stops.first().cloned().unwrap_or_default();
    assert!(stops.iter().skip(1).any(|stop| *stop == first), "Tab comes round to {first:?}: {stops:?}");
}

#[test]
fn the_sheet_over_the_editor_closes_its_open_list_and_one_esc_closes_the_sheet() {
    let mut harness = editor_with_its_list_open();
    open_sheet(&mut harness);
    assert!(!repeat_list(&harness), "the list under the sheet closed when the sheet opened");
    assert!(!popup::any_open(&harness.ctx), "nothing under the sheet says it is open");
    let modal = harness.ctx.memory(|memory| memory.top_modal_layer());
    assert_eq!(modal, Some(LayerId::new(Order::Foreground, Id::unique("settings-sheet"))), "the sheet holds the modal layer");
    tab_goes_round_the_sheet(&mut harness);
    esc(&mut harness);
    assert!(!harness.state().settings().is_open(), "one Esc closes the sheet");
    assert!(!repeat_list(&harness), "and the list stays closed");
    assert_eq!(summary(&harness).as_deref(), Some("Kiln"), "the editor keeps its typing");
    let editor = harness.state().calendar().editor();
    assert!(editor.is_some_and(|editor| !editor.warned()), "the editor is open and that Esc was the sheet's");
}

#[test]
fn the_sheet_over_the_editor_with_its_list_open_closes_on_a_press_beside() {
    let mut harness = editor_with_its_list_open();
    open_sheet(&mut harness);
    press_beside(&mut harness);
    assert!(!harness.state().settings().is_open(), "a press beside closes the sheet");
    assert!(!repeat_list(&harness), "the list stays closed");
    assert_eq!(summary(&harness).as_deref(), Some("Kiln"), "the editor keeps its typing");
    assert!(harness.state().calendar().editor().is_some_and(|editor| !editor.warned()), "the press was the sheet's");
}

#[test]
fn the_sheet_over_the_composer_closes_its_picker_and_keeps_the_keyboard() {
    let mut harness = composer_with_its_picker_open();
    open_sheet(&mut harness);
    assert!(!picker_open(&harness), "the picker closed when the sheet opened");
    assert!(!picker_drawn(&harness), "nothing stands over the sheet");
    let modal = harness.ctx.memory(|memory| memory.top_modal_layer());
    assert_eq!(modal, Some(LayerId::new(Order::Foreground, Id::unique("settings-sheet"))), "the sheet holds the modal layer");
    assert!(focus(&harness).is_none_or(|(_, inside)| inside), "the closed picker asked for no keyboard under the sheet");
    tab_goes_round_the_sheet(&mut harness);
    esc(&mut harness);
    assert!(!harness.state().settings().is_open(), "one Esc closes the sheet");
    assert!(!picker_open(&harness), "the picker stays closed");
    let composer = harness.state().composer();
    assert!(composer.is_some_and(|composer| composer.later().is_none()), "the closed picker picked no time");
    assert_eq!(subject(&harness).as_deref(), Some("Plot"), "and nothing typed is lost");
    let mut harness = composer_with_its_picker_open();
    open_sheet(&mut harness);
    press_beside(&mut harness);
    assert!(!harness.state().settings().is_open(), "a press beside closes the sheet");
    assert!(!picker_open(&harness), "the picker stays closed");
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

/// The mail with its first message open in the reader, Snooze on Ctrl+H and New message on Ctrl+M.
fn reading_with_keys_on_ctrl() -> Harness<'static, InboxApp> {
    let mut harness = harness(false);
    let first = harness.state().mailbox().messages().first().map(|row| row.id).unwrap();
    harness.state_mut().mailbox_mut().select(Some(first));
    let keymap = harness.state_mut().keymap_mut();
    keymap.bind(keymap::SNOOZE, egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::H));
    keymap.bind(keymap::COMPOSE, egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::M));
    harness.run();
    assert!(harness.state().can_run(keymap::SNOOZE), "Snooze can run on the open message");
    harness
}

#[test]
fn under_the_sheet_a_key_with_ctrl_runs_no_command_of_the_window_under_it() {
    let mut harness = reading_with_keys_on_ctrl();
    let screen = format!("{:?}", harness.state().screen());
    open_sheet(&mut harness);
    // Snooze's picker and a new message: neither from a key under the sheet. (The keys to a place of the window, the
    // calendar's Ctrl+2 among them, close the sheet first, as before.)
    for (modifiers, key) in [(Modifiers::COMMAND, Key::H), (Modifiers::COMMAND, Key::M)] {
        harness.key_press_modifiers(modifiers, key);
        harness.run();
        harness.run();
        let case = format!("{modifiers:?} {key:?}");
        assert!(harness.state().settings().is_open(), "{case}: the sheet stays");
        assert!(harness.query_by_label("Snooze until").is_none() && !popup::any_open(&harness.ctx), "{case}: no float opened");
        assert!(harness.state().composer().is_none(), "{case}: no message begun");
        assert_eq!(format!("{:?}", harness.state().screen()), screen, "{case}: the screen under it stays");
        let modal = harness.ctx.memory(|memory| memory.top_modal_layer());
        assert_eq!(modal.map(|layer| layer.id), Some(Id::unique("settings-sheet")), "{case}: the sheet holds the modal layer");
        let sheet = harness.get_by_role_and_label(Role::Dialog, "Settings").rect();
        let inside =
            harness.root().children_recursive().find(|node| node.accesskit_node().is_focused()).is_none_or(|node| sheet.contains_rect(node.rect()));
        assert!(inside, "{case}: the keyboard is nowhere outside the sheet: {:?}", focused(&harness));
    }
    // Esc closes the sheet, as before (Ctrl+, over it keeps it open, as before); the palette's key opens the palette.
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Comma);
    harness.run();
    assert!(harness.state().settings().is_open(), "Ctrl+, keeps the sheet");
    esc(&mut harness);
    assert!(!harness.state().settings().is_open(), "Esc closes the sheet");
    open_sheet(&mut harness);
    open_palette(&mut harness);
    assert!(!harness.state().settings().is_open(), "the palette closes the sheet, as before");
    esc(&mut harness);
    // Without the sheet the same keys run.
    harness.key_press_modifiers(Modifiers::COMMAND, Key::H);
    harness.run();
    harness.run();
    assert!(harness.query_by_label("Snooze until").is_some(), "Ctrl+H opens Snooze over the reader");
}
