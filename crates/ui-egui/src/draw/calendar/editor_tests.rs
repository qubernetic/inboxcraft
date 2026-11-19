// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The event editor on the calendar screen: New event, Edit and Delete, the form's controls, the notify-guests prompt
//! only for an event with guests, and the toast whose Undo (or Ctrl+Z) takes a change back.

use egui::accesskit::{Role, Toggled};
use egui::{Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _, by};
use inboxcraft_time::{FakeClock, Timestamp};

use crate::mailbox::Mailbox;
use crate::{InboxApp, sample};

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

/// The sample window on the calendar screen.
fn on_calendar(theme: egui::Theme) -> Harness<'static, InboxApp> {
    let app = InboxApp::at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    let mut harness = Harness::builder().with_size(vec2(1200.0, 820.0)).with_theme(theme).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    press(&mut harness, "Calendar");
    harness
}

fn press(harness: &mut Harness<'static, InboxApp>, name: &str) {
    // A button, or a segmented control's option.
    harness.query_by_role_and_label(Role::RadioButton, name).unwrap_or_else(|| harness.get_by_role_and_label(Role::Button, name)).click();
    harness.run();
}

/// Presses the first event whose name holds `text`.
fn pick(harness: &mut Harness<'static, InboxApp>, text: &str) {
    harness.query_all(by().role(Role::Button).label_contains(text)).next().unwrap().click();
    harness.run();
}

fn shows(harness: &Harness<'static, InboxApp>, text: &str) -> bool {
    harness.query_all_by_label_contains(text).next().is_some()
}

/// Opens Repeat's list and takes `name` in it.
fn choose_repeat(harness: &mut Harness<'static, InboxApp>, name: &str) {
    press_field(harness, "Repeat");
    harness.get_by_role_and_label(Role::MenuItemRadio, name).click();
    harness.run();
}

fn press_field(harness: &mut Harness<'static, InboxApp>, name: &str) {
    harness.get_by_role_and_label(Role::ComboBox, name).click();
    harness.run();
}

fn editing(harness: &Harness<'static, InboxApp>) -> bool {
    harness.state().calendar().editor().is_some()
}

fn type_into(harness: &mut Harness<'static, InboxApp>, field: &str, text: &str) {
    harness.get_by_role_and_label(Role::TextInput, field).focus();
    harness.run();
    harness.get_by_role_and_label(Role::TextInput, field).type_text(text);
    harness.run();
}

#[test]
fn new_event_opens_the_editor_and_saving_adds_the_event_with_an_undo_toast() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let mut harness = on_calendar(theme);
        press(&mut harness, "New event");
        assert!(editing(&harness));
        for field in ["Title", "Starts", "Ends", "Guests"] {
            harness.get_by_role_and_label(Role::TextInput, field);
        }
        assert!(!shows(&harness, "Tell the"), "a new event asks nothing of its guests");
        harness.check_accessibility();
        type_into(&mut harness, "Title", "Pottery class");
        type_into(&mut harness, "Guests", "kai@example.org");
        assert!(!shows(&harness, "Tell the"), "not even with guests typed");
        press(&mut harness, "Save");
        assert!(!editing(&harness), "the editor closes");
        assert!(shows(&harness, "Pottery class, Thursday 5 November, 09:00 – 10:00"));
        harness.get_by_label("Event added");
        harness.check_accessibility();
        press(&mut harness, "undo");
        assert!(!shows(&harness, "Pottery class"), "undo takes the new event away");
    }
}

#[test]
fn the_notify_prompt_shows_only_when_the_event_has_guests() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let mut harness = on_calendar(theme);
        pick(&mut harness, "Dentist");
        press(&mut harness, "Edit");
        assert!(editing(&harness));
        assert!(!shows(&harness, "Tell the"), "the dentist has no guests");
        press(&mut harness, "Cancel");
        assert!(!editing(&harness));
        pick(&mut harness, "Allotment work party");
        press(&mut harness, "Edit");
        harness.get_by_label("Tell the 1 guest about this change?");
        let quiet = harness.get_by_role_and_label(Role::RadioButton, "Don't tell them");
        assert_eq!(quiet.accesskit_node().toggled(), Some(Toggled::True), "the default is not to tell them");
        harness.check_accessibility();
        harness.get_by_role_and_label(Role::RadioButton, "Send them the update").click();
        harness.run();
        assert!(harness.state().calendar().editor().is_some_and(|editor| editor.notify()));
        harness.state_mut().calendar_mut().0.editor_mut().unwrap().form_mut().guests.clear();
        harness.run();
        assert!(!shows(&harness, "Tell the"), "without guests the prompt goes");
    }
}

#[test]
fn an_edit_with_its_guests_told_says_so_in_the_toast() {
    let mut harness = on_calendar(egui::Theme::Light);
    pick(&mut harness, "Allotment work party");
    press(&mut harness, "Edit");
    type_into(&mut harness, "Title", " and soup");
    harness.get_by_role_and_label(Role::RadioButton, "Send them the update").click();
    harness.run();
    press(&mut harness, "Save");
    harness.get_by_label("Event saved; the guests are told");
    assert!(shows(&harness, "Allotment work party and soup"));
}

#[test]
fn the_form_s_controls_change_the_event() {
    let mut harness = on_calendar(egui::Theme::Light);
    press(&mut harness, "New event");
    press_field(&mut harness, "Calendar");
    harness.get_by_role_and_label(Role::MenuItemRadio, "Studio").click();
    harness.run();
    press(&mut harness, "Free");
    choose_repeat(&mut harness, "Weekly");
    for name in ["All day", "Private"] {
        harness.get_by_role_and_label(Role::CheckBox, name).click();
        harness.run();
    }
    let form = harness.state().calendar().editor().unwrap().form().clone();
    assert_eq!((form.calendar, form.repeat.as_str(), form.busy, form.all_day, form.private), (2, "weekly", false, true, true));
    assert_eq!(form.start, "2026-11-05");
    harness.get_by_role_and_label(Role::CheckBox, "All day").click();
    harness.run();
    assert_eq!(harness.state().calendar().editor().unwrap().form().start, "2026-11-05 09:00");
    press(&mut harness, "Busy");
    // Opened a second time, the list stands over the editor still.
    choose_repeat(&mut harness, "Never");
    let form = harness.state().calendar().editor().unwrap().form().clone();
    assert_eq!((form.repeat.as_str(), form.busy), ("none", true));
}

#[test]
fn a_custom_repeat_shows_as_custom_and_cannot_be_picked() {
    let mut harness = on_calendar(egui::Theme::Light);
    pick(&mut harness, "Dentist");
    press(&mut harness, "Edit");
    let shown = |harness: &Harness<'static, InboxApp>| harness.get_by_role_and_label(Role::ComboBox, "Repeat").accesskit_node().value();
    assert_eq!(shown(&harness).as_deref(), Some("Never"), "only a repeat the presets do not name shows Custom");
    harness.state_mut().calendar_mut().0.editor_mut().unwrap().form_mut().repeat = "custom".to_owned();
    harness.run();
    assert_eq!(shown(&harness).as_deref(), Some("Custom"));
    press_field(&mut harness, "Repeat");
    assert!(harness.query_by_role_and_label(Role::MenuItemRadio, "Custom").is_none(), "Custom is no choice to pick");
    harness.key_press(Key::Escape);
    harness.run();
    assert_eq!(harness.state().calendar().editor().unwrap().form().repeat, "custom");
    assert!(harness.query_by_role_and_label(Role::ComboBox, "Calendar").is_none(), "an edit picks no calendar");
}

#[test]
fn what_cannot_be_saved_is_said_in_the_editor() {
    let mut harness = on_calendar(egui::Theme::Light);
    press(&mut harness, "New event");
    harness.state_mut().calendar_mut().0.editor_mut().unwrap().form_mut().start = "soon".to_owned();
    harness.run();
    press(&mut harness, "Save");
    assert!(editing(&harness));
    harness.get_by_label("Starts is no date and time like 2026-11-06 13:00");
    harness.check_accessibility();
}

#[test]
fn escape_closes_the_open_list_first_and_only_then_the_editor() {
    let mut harness = on_calendar(egui::Theme::Light);
    press(&mut harness, "New event");
    press_field(&mut harness, "Repeat");
    assert!(harness.query_by_role_and_label(Role::MenuItemRadio, "Daily").is_some());
    harness.key_press(Key::Escape);
    harness.run();
    assert!(editing(&harness), "the form stays");
    assert!(harness.query_by_role_and_label(Role::MenuItemRadio, "Daily").is_none());
    harness.key_press(Key::Escape);
    harness.run();
    assert!(!editing(&harness), "the second Escape closes the editor");
}

#[test]
fn a_new_event_opens_with_the_list_closed_after_an_escape_left_one_open() {
    let mut harness = on_calendar(egui::Theme::Light);
    press(&mut harness, "New event");
    press_field(&mut harness, "Repeat");
    // Leaving the screen closes the editor while its list is open.
    harness.state_mut().calendar_mut().0.close_editor();
    harness.run();
    press(&mut harness, "New event");
    assert!(harness.query_by_role_and_label(Role::MenuItemRadio, "Daily").is_none(), "a fresh editor keeps its lists closed");
}

#[test]
fn a_new_event_in_a_narrow_window_still_offers_every_control() {
    let app = InboxApp::at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    let mut harness = Harness::builder().with_size(vec2(760.0, 620.0)).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    press(&mut harness, "Calendar");
    press(&mut harness, "New event");
    for name in ["Busy", "Free"] {
        harness.get_by_role_and_label(Role::RadioButton, name);
    }
    harness.get_by_role_and_label(Role::ComboBox, "Repeat");
}

#[test]
fn calendars_that_do_not_fit_side_by_side_are_a_select_instead() {
    let mut harness = on_calendar(egui::Theme::Light);
    press(&mut harness, "New event");
    let many: Vec<(u64, String)> = (1..=12).map(|id| (id, format!("A calendar with a long name {id}"))).collect();
    let day = crate::calendar::Day::new(2026, 11, 5).unwrap();
    *harness.state_mut().calendar_mut().0.editor_mut().unwrap() = crate::calendar::Editor::new_event(3, day, many);
    harness.run();
    assert!(harness.query_by_role_and_label(Role::Button, "A calendar with a long name 3").is_none());
    press_field(&mut harness, "Calendar");
    harness.get_by_role_and_label(Role::MenuItemRadio, "A calendar with a long name 7").click();
    harness.run();
    assert_eq!(harness.state().calendar().editor().unwrap().form().calendar, 7);
    assert_eq!(harness.get_by_role_and_label(Role::ComboBox, "Calendar").accesskit_node().value().as_deref(), Some("A calendar with a long name 7"));
}

#[test]
fn escape_closes_the_editor_and_leaves_the_calendar() {
    let mut harness = on_calendar(egui::Theme::Light);
    press(&mut harness, "New event");
    harness.key_press(Key::Escape);
    harness.run();
    assert!(!editing(&harness));
    harness.get_by_label("2 – 8 November 2026");
}

#[test]
fn delete_shows_a_toast_and_ctrl_z_brings_the_event_back() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let mut harness = on_calendar(theme);
        pick(&mut harness, "Dentist");
        press(&mut harness, "Delete");
        assert!(!shows(&harness, "Dentist"));
        harness.get_by_label("Event deleted");
        harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
        harness.run();
        assert!(shows(&harness, "Dentist"), "the toast's key brings it back on the calendar screen");
    }
}

#[test]
fn after_the_toast_ctrl_z_still_takes_an_event_s_change_back() {
    let mut harness = on_calendar(egui::Theme::Light);
    pick(&mut harness, "Dentist");
    press(&mut harness, "Delete");
    harness.state_mut().mailbox_mut().forget_undo();
    harness.run();
    assert!(harness.query_by_label("Event deleted").is_none());
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run();
    assert!(shows(&harness, "Dentist"), "the history's Undo reads the calendar again");
}

/// Every filled rect of the last frame and its fill.
fn flat_rects(harness: &Harness<'static, InboxApp>) -> Vec<(egui::Rect, egui::Color32)> {
    fn open(shape: &egui::Shape, out: &mut Vec<(egui::Rect, egui::Color32)>) {
        match shape {
            egui::Shape::Vec(inner) => inner.iter().for_each(|shape| open(shape, out)),
            egui::Shape::Rect(rect) => out.push((rect.rect, rect.fill)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    harness.output().shapes.iter().for_each(|clipped| open(&clipped.shape, &mut out));
    out
}

/// The sample's Allotment work party (one guest) open in the editor by Edit.
fn editing_the_party(theme: egui::Theme) -> Harness<'static, InboxApp> {
    let mut harness = on_calendar(theme);
    pick(&mut harness, "Allotment work party");
    press(&mut harness, "Edit");
    // The editor stands where it was measured the frame before.
    harness.run();
    harness
}

#[test]
fn the_edit_editor_stands_over_the_details_at_the_card_s_bottom_right() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let p = crate::theme::Palette::of(if theme == egui::Theme::Dark { crate::theme::Mode::Dark } else { crate::theme::Mode::Light });
        let harness = editing_the_party(theme);
        let found = flat_rects(&harness);
        // The card: the `line` down between the view and the side panel runs its height; the panel is 252 wide.
        let (edge, _) = found
            .iter()
            .copied()
            .filter(|(rect, fill)| *fill == p.line && rect.width() == 1.0 && rect.height() > 500.0 && rect.left() > 600.0)
            .min_by(|a, b| a.0.left().total_cmp(&b.0.left()))
            .unwrap();
        let card = egui::Rect::from_min_max(egui::pos2(edge.right(), edge.top()), egui::pos2(edge.right() + 252.0, edge.bottom()));
        let (editor, _) = found.iter().copied().find(|(rect, fill)| *fill == p.surface && rect.width() == 432.0).unwrap();
        // The frame: its right edge 3 inside the card's, its bottom on the card's bottom edge.
        assert_eq!(editor.right(), card.right() - 3.0, "{editor:?} in {card:?}");
        assert_eq!(editor.bottom(), card.bottom() + 1.0, "{editor:?} in {card:?}");
    }
}

#[test]
fn the_question_to_the_guests_stands_in_its_box() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let p = crate::theme::Palette::of(if theme == egui::Theme::Dark { crate::theme::Mode::Dark } else { crate::theme::Mode::Light });
        let harness = editing_the_party(theme);
        let question = harness.get_by_label("Tell the 1 guest about this change?").rect();
        let found = flat_rects(&harness);
        // The frame: `panel`, a 1 px `cyan_edge`, 398 × 78, the question 13 in from its left and top.
        let (boxed, _) = found
            .iter()
            .copied()
            .filter(|(rect, fill)| *fill == p.panel && rect.contains_rect(question))
            .min_by(|a, b| a.0.area().total_cmp(&b.0.area()))
            .unwrap();
        assert_eq!((boxed.width(), boxed.height()), (398.0, 78.0), "{boxed:?}");
        assert_eq!((question.left() - boxed.left(), question.top() - boxed.top()), (13.0, 13.0), "{question:?} in {boxed:?}");
        let side =
            |rect: &egui::Rect| (rect.width() == boxed.width() && rect.height() == 1.0) || (rect.height() == boxed.height() && rect.width() == 1.0);
        let edges = found.iter().filter(|(rect, fill)| *fill == p.cyan_edge && boxed.contains_rect(*rect) && side(rect)).count();
        assert_eq!(edges, 4, "four sides of cyan_edge");
    }
}
