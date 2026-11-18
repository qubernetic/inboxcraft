// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The keyboard comes back to what opened a float of the calendar when it closes: New event or Edit for the editor,
//! "+N more" for the day's float. It is never left nowhere.

use egui::accesskit::Role;
use egui::{Key, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _, by};
use inboxcraft_time::{FakeClock, Timestamp};

use crate::calendar::{Day, EventRow, Moment, View};
use crate::mailbox::Mailbox;
use crate::{InboxApp, sample};

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

fn window(view: View) -> Harness<'static, InboxApp> {
    let mut app = InboxApp::new(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    app.show_calendar();
    let (calendar, mailbox) = app.calendar_mut();
    calendar.set_view(mailbox, view);
    let mut harness =
        Harness::builder().with_size(vec2(1440.0, 900.0)).with_theme(egui::Theme::Dark).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

/// The name of the node the keyboard is on.
fn focused(harness: &Harness<'static, InboxApp>) -> Option<String> {
    harness.root().children_recursive().find(|node| node.accesskit_node().is_focused()).and_then(|node| node.accesskit_node().label())
}

#[test]
fn the_editor_gives_the_keyboard_back_to_new_event() {
    let mut harness = window(View::Week);
    harness.get_by_role_and_label(Role::Button, "New event").focus();
    harness.run();
    harness.key_press(Key::Enter);
    harness.run();
    assert!(harness.state().calendar().editor().is_some(), "Enter on New event opens the editor");
    harness.key_press(Key::Escape);
    harness.run();
    assert!(harness.state().calendar().editor().is_none());
    assert_eq!(focused(&harness).as_deref(), Some("New event"), "Esc gives the keyboard back to New event");
    // Cancel too.
    harness.get_by_role_and_label(Role::Button, "New event").click();
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Cancel").click();
    harness.run();
    assert_eq!(focused(&harness).as_deref(), Some("New event"), "Cancel gives the keyboard back to New event");
}

#[test]
fn the_editor_opened_by_edit_gives_the_keyboard_back_to_edit() {
    let mut harness = window(View::Week);
    harness.query_all(by().role(Role::Button).label_contains("Studio stand-up")).next().unwrap().click();
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Edit").click();
    harness.run();
    assert!(harness.state().calendar().editor().is_some_and(|editor| editor.id().is_some()));
    harness.key_press(Key::Escape);
    harness.run();
    assert!(harness.state().calendar().editor().is_none());
    assert_eq!(focused(&harness).as_deref(), Some("Edit"), "Esc gives the keyboard back to Edit");
}

#[test]
fn the_day_s_float_gives_the_keyboard_back_to_its_link() {
    let mut harness = window(View::Month);
    let thursday = Day::new(2026, 11, 5).unwrap();
    let mut events: Vec<EventRow> = harness.state().calendar().events().cloned().collect();
    let model = events.first().cloned().unwrap();
    for index in 0..12_u64 {
        let mut event = model.clone();
        event.id = 30_000 + index;
        event.summary = format!("Crowd {index}");
        event.start = Moment { day: thursday, minute: 540 };
        event.end = Moment { day: thursday, minute: 600 };
        (event.first, event.last, event.all_day, event.repeat) = (thursday, thursday, false, None);
        events.push(event);
    }
    harness.state_mut().calendar_mut().0.set_events(events);
    harness.run();
    let link =
        harness.query_all(by().role(Role::Button)).find(|node| node.accesskit_node().label().is_some_and(|label| label.starts_with('+'))).unwrap();
    let name = link.accesskit_node().label().unwrap();
    link.click();
    harness.run();
    assert_eq!(harness.state().calendar().more(), Some(thursday));
    harness.key_press(Key::Escape);
    harness.run();
    assert_eq!(harness.state().calendar().more(), None);
    assert_eq!(focused(&harness).as_deref(), Some(name.as_str()), "Esc gives the keyboard back to +N more");
    // A row picked in it: the float closes, the keyboard goes back to the link as well.
    harness.query_all(by().role(Role::Button).label(name.as_str())).next().unwrap().click();
    harness.run();
    harness.query_all(by().role(Role::Button).label_contains("Crowd 3")).last().unwrap().click();
    harness.run();
    assert_eq!(harness.state().calendar().more(), None);
    assert_eq!(focused(&harness).as_deref(), Some(name.as_str()));
}

/// Picks a timed event of today, opens Edit on it and types into the title.
fn editing(harness: &mut Harness<'static, InboxApp>) -> u64 {
    let today = harness.state().calendar().today();
    let event = harness.state().calendar().events().find(|event| !event.all_day && event.start.day == today).map(EventRow::key);
    harness.state_mut().calendar_mut().0.pick(event);
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Edit").click();
    harness.run();
    harness.get_by_role_and_label(Role::TextInput, "Title").focus();
    harness.get_by_role_and_label(Role::TextInput, "Title").type_text(" moved");
    harness.run();
    harness.state().calendar().editor().and_then(crate::calendar::Editor::id).unwrap()
}

/// Cancel, by the model, and two frames for the keyboard to land.
fn cancel(harness: &mut Harness<'static, InboxApp>) {
    harness.state_mut().calendar_mut().0.close_editor();
    harness.run();
    harness.run();
}

#[test]
fn with_its_opener_gone_the_editor_gives_the_keyboard_to_the_grid_s_picked_day() {
    // Edit hidden by a narrow window: the week's head of the day the view is on (today) has the keyboard.
    let mut harness = window(View::Week);
    editing(&mut harness);
    harness.set_size(vec2(560.0, 900.0));
    harness.run();
    assert!(harness.query_by_role_and_label(Role::Button, "Edit").is_none(), "under 600 the details hide");
    cancel(&mut harness);
    assert_eq!(focused(&harness).as_deref(), Some("Thursday 5 November 2026, today"), "the grid's picked day");
    // The event deleted under the editor: the same.
    let mut harness = window(View::Week);
    let id = editing(&mut harness);
    {
        let (calendar, mailbox) = harness.state_mut().calendar_mut();
        mailbox.run("event.delete", &serde_json::json!({ "id": id })).unwrap();
        calendar.refresh(mailbox);
    }
    harness.run();
    cancel(&mut harness);
    assert_eq!(focused(&harness).as_deref(), Some("Thursday 5 November 2026, today"));
    // In the month: the picked day's number.
    let mut harness = window(View::Month);
    editing(&mut harness);
    harness.set_size(vec2(560.0, 900.0));
    harness.run();
    cancel(&mut harness);
    assert_eq!(focused(&harness).as_deref(), Some("Thursday 5 November 2026, today"));
    // The day view and the agenda draw no day's button: New event.
    for view in [View::Day, View::Agenda] {
        let mut harness = window(view);
        editing(&mut harness);
        harness.set_size(vec2(560.0, 900.0));
        harness.run();
        cancel(&mut harness);
        assert_eq!(focused(&harness).as_deref(), Some("New event"), "{view:?}: never nowhere");
    }
}
