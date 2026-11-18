// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The ways out of the event editor with something typed: what was typed is thrown away only by an act that says so
//! (Cancel, or an Esc after the warning an Esc raised, with nothing changed since). A press outside, however often,
//! another New event or Edit, and a change of view or day keep it; their warning is not Esc's.

use egui::accesskit::Role;
use egui::{Event, Key, Modifiers, PointerButton, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _, by};
use inboxcraft_time::{FakeClock, Timestamp};

use crate::calendar::View;
use crate::mailbox::Mailbox;
use crate::{InboxApp, sample};

/// What the editor says while what was typed is not saved.
const WARNING: &str = "What you typed is not saved. Save keeps it; Cancel, or Esc again, throws it away.";

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

/// The sample window on the calendar's week.
fn window() -> Harness<'static, InboxApp> {
    let mut app = InboxApp::new(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    app.show_calendar();
    let (calendar, mailbox) = app.calendar_mut();
    calendar.set_view(mailbox, View::Week);
    let mut harness =
        Harness::builder().with_size(vec2(1440.0, 900.0)).with_theme(egui::Theme::Dark).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

/// Opens New event and types `text` into its title.
fn typed(harness: &mut Harness<'static, InboxApp>, text: &str) {
    harness.get_by_role_and_label(Role::Button, "New event").click();
    harness.run();
    type_more(harness, text);
}

fn type_more(harness: &mut Harness<'static, InboxApp>, text: &str) {
    harness.get_by_role_and_label(Role::TextInput, "Title").focus();
    harness.get_by_role_and_label(Role::TextInput, "Title").type_text(text);
    harness.run();
}

/// The editor's title, while it is open.
fn summary(harness: &Harness<'static, InboxApp>) -> Option<String> {
    harness.state().calendar().editor().map(|editor| editor.form().summary.clone())
}

fn warns(harness: &Harness<'static, InboxApp>) -> bool {
    harness.query_by_label(WARNING).is_some()
}

/// A press and release of the pointer at `x`, `y`, each in a frame of its own.
fn press_at(harness: &mut Harness<'static, InboxApp>, x: f32, y: f32) {
    let pos = pos2(x, y);
    harness.event(Event::PointerMoved(pos));
    harness.event(Event::PointerButton { pos, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    harness.step();
    harness.event(Event::PointerButton { pos, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    harness.step();
}

#[test]
fn a_press_outside_with_typing_warns_and_never_closes_however_often() {
    let mut harness = window();
    typed(&mut harness, "Kiln");
    press_at(&mut harness, 1300.0, 700.0);
    harness.run();
    assert_eq!(summary(&harness).as_deref(), Some("Kiln"), "one press outside keeps the typing");
    assert!(warns(&harness), "and says how to keep or throw it away");
    // A double click outside, then more presses: the typing stays.
    press_at(&mut harness, 1300.0, 700.0);
    press_at(&mut harness, 1300.0, 700.0);
    harness.run();
    for _ in 0..3 {
        press_at(&mut harness, 200.0, 600.0);
    }
    harness.run();
    assert_eq!(summary(&harness).as_deref(), Some("Kiln"), "presses outside never throw the typing away");
    assert!(warns(&harness));
    // Without typing, a press outside closes the editor.
    harness.state_mut().calendar_mut().0.close_editor();
    harness.run();
    harness.get_by_role_and_label(Role::Button, "New event").click();
    harness.run();
    press_at(&mut harness, 1300.0, 700.0);
    harness.run();
    assert!(summary(&harness).is_none(), "nothing typed: a press outside closes it");
}

#[test]
fn typing_after_the_warning_asks_again_before_esc_throws_it_away() {
    let mut harness = window();
    typed(&mut harness, "Kiln");
    harness.key_press(Key::Escape);
    harness.run();
    assert!(harness.state().calendar().editor().is_some_and(|editor| editor.warned()));
    assert!(warns(&harness));
    type_more(&mut harness, " and glaze");
    assert!(!harness.state().calendar().editor().is_some_and(|editor| editor.warned()), "a change resets the warning");
    assert!(!warns(&harness), "the warning goes with the change");
    harness.key_press(Key::Escape);
    harness.run();
    assert_eq!(summary(&harness).as_deref(), Some("Kiln and glaze"), "the Esc after more typing asks again");
    assert!(warns(&harness));
    harness.key_press(Key::Escape);
    harness.run();
    assert!(summary(&harness).is_none(), "a second Esc with nothing changed throws it away");
}

#[test]
fn after_the_warning_of_a_press_new_event_or_edit_the_first_esc_warns_as_esc_does() {
    // A press outside.
    let mut harness = window();
    typed(&mut harness, "Kiln");
    press_at(&mut harness, 1300.0, 700.0);
    harness.run();
    assert!(warns(&harness));
    harness.key_press(Key::Escape);
    harness.run();
    assert_eq!(summary(&harness).as_deref(), Some("Kiln"), "the first Esc after a press's warning keeps the typing");
    assert!(warns(&harness), "and warns as Esc does");
    harness.key_press(Key::Escape);
    harness.run();
    assert!(summary(&harness).is_none(), "the second Esc, nothing changed since, throws it away");
    // New event and Edit, asked while typing is held.
    for way in ["new", "edit"] {
        typed(&mut harness, "Kiln");
        let (calendar, _) = harness.state_mut().calendar_mut();
        if way == "new" {
            calendar.new_event();
        } else {
            let other = calendar.events().next().map(crate::calendar::EventRow::key);
            calendar.pick(other);
            calendar.edit_picked();
        }
        harness.run();
        assert!(warns(&harness), "{way} warns");
        harness.key_press(Key::Escape);
        harness.run();
        assert_eq!(summary(&harness).as_deref(), Some("Kiln"), "the first Esc after the warning of {way} keeps the typing");
        harness.key_press(Key::Escape);
        harness.run();
        assert!(summary(&harness).is_none(), "the second Esc after {way} throws it away");
    }
}

#[test]
fn a_save_refused_after_the_warning_shows_its_reason() {
    let mut harness = window();
    harness.query_all(by().role(Role::Button).label_contains("Studio stand-up")).next().unwrap().click();
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Edit").click();
    harness.run();
    let id = harness.state().calendar().editor().and_then(crate::calendar::Editor::id).unwrap();
    type_more(&mut harness, " moved");
    press_at(&mut harness, 1300.0, 700.0);
    harness.run();
    assert!(warns(&harness), "the press outside warns");
    // The event goes under the editor (another device deleted it), then Save is refused.
    {
        let (calendar, mailbox) = harness.state_mut().calendar_mut();
        mailbox.run("event.delete", &serde_json::json!({ "id": id })).unwrap();
        calendar.refresh(mailbox);
    }
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Save").click();
    harness.run();
    let problem = harness.state().calendar().editor().and_then(|editor| editor.problem().map(str::to_owned));
    assert!(problem.as_deref().is_some_and(|problem| problem != WARNING), "the refusal's reason, not the warning: {problem:?}");
    assert!(!warns(&harness), "the warning gives way to the reason");
    assert!(harness.query_by_label(problem.as_deref().unwrap_or_default()).is_some(), "the reason is shown");
    // A bad date, refused after Esc's warning: the reason shows too.
    harness.state_mut().calendar_mut().0.close_editor();
    harness.run();
    typed(&mut harness, "Kiln");
    harness.state_mut().calendar_mut().0.editor_mut().unwrap().form_mut().start = "soon".to_owned();
    harness.run();
    harness.key_press(Key::Escape);
    harness.run();
    assert!(warns(&harness));
    harness.get_by_role_and_label(Role::Button, "Save").click();
    harness.run();
    assert!(!warns(&harness), "a refused Save clears the warning");
    assert!(harness.query_by_label("Starts is no date and time like 2026-11-06 13:00").is_some());
}

#[test]
fn new_event_or_edit_of_another_event_keeps_the_typing() {
    let mut harness = window();
    typed(&mut harness, "Kiln");
    harness.state_mut().calendar_mut().0.new_event();
    harness.run();
    assert_eq!(summary(&harness).as_deref(), Some("Kiln"), "New event keeps the typing");
    assert!(warns(&harness));
    // Edit of another event: the pick changes under the editor, Edit keeps the typing.
    let other = harness.state().calendar().events().next().map(crate::calendar::EventRow::key);
    harness.state_mut().calendar_mut().0.pick(other);
    harness.state_mut().calendar_mut().0.edit_picked();
    harness.run();
    assert_eq!(summary(&harness).as_deref(), Some("Kiln"), "Edit of another event keeps the typing");
    // Without typing, New event and Edit open at once: the typing erased after the warning holds nothing to keep.
    harness.state_mut().calendar_mut().0.editor_mut().unwrap().form_mut().summary.clear();
    harness.state_mut().calendar_mut().0.edit_picked();
    harness.run();
    assert!(harness.state().calendar().editor().is_some_and(|editor| editor.id().is_some()), "Edit replaces an editor with nothing typed");
    assert!(!warns(&harness), "and opens without a word");
    harness.state_mut().calendar_mut().0.close_editor();
    harness.state_mut().calendar_mut().0.new_event();
    harness.state_mut().calendar_mut().0.edit_picked();
    harness.run();
    assert!(harness.state().calendar().editor().is_some_and(|editor| editor.id().is_some()), "Edit replaces a New event with nothing typed");
}

#[test]
fn changing_the_view_or_the_day_keeps_the_typing() {
    let mut harness = window();
    let month = harness.get_by_role_and_label(Role::RadioButton, "Month").rect().center();
    let next = harness.get_by_role_and_label(Role::Button, "Next week").rect().center();
    typed(&mut harness, "Kiln");
    // A press on Month or Next week is a press outside the editor: it warns, and reaches nothing under it.
    for at in [month, next, month, next] {
        press_at(&mut harness, at.x, at.y);
    }
    harness.run();
    assert_eq!(summary(&harness).as_deref(), Some("Kiln"), "a press on the toolbar keeps the typing");
    assert!(warns(&harness));
    assert_eq!(harness.state().calendar().view(), View::Week, "and the view stays");
    // From outside (an alert, a script): the view and the day change, the editor and its typing stay, and the new
    // view's toolbar is out of reach as well.
    let (calendar, mailbox) = harness.state_mut().calendar_mut();
    calendar.set_view(mailbox, View::Month);
    calendar.step(mailbox, true);
    calendar.go_today(mailbox);
    harness.run();
    assert_eq!(harness.state().calendar().view(), View::Month);
    assert_eq!(summary(&harness).as_deref(), Some("Kiln"), "a change of view or day keeps the typing");
    let (focus, next) = (harness.state().calendar().focus(), harness.get_by_role_and_label(Role::Button, "Next month").rect().center());
    press_at(&mut harness, next.x, next.y);
    harness.run();
    assert_eq!(harness.state().calendar().focus(), focus, "a press on the month's Next month reaches nothing");
    assert_eq!(summary(&harness).as_deref(), Some("Kiln"));
}

#[test]
fn cancel_throws_the_typing_away_at_once() {
    let mut harness = window();
    typed(&mut harness, "Kiln");
    harness.get_by_role_and_label(Role::Button, "Cancel").click();
    harness.run();
    assert!(summary(&harness).is_none());
    // After a press outside's warning and after Esc's: the press and the Esc kept it, Cancel alone throws it away,
    // with no second word.
    for esc in [false, true] {
        typed(&mut harness, "Kiln");
        if esc {
            harness.key_press(Key::Escape);
        } else {
            press_at(&mut harness, 1300.0, 700.0);
        }
        harness.run();
        assert_eq!(summary(&harness).as_deref(), Some("Kiln"), "the {} kept the typing", if esc { "Esc" } else { "press" });
        assert!(warns(&harness));
        harness.get_by_role_and_label(Role::Button, "Cancel").click();
        harness.run();
        assert!(summary(&harness).is_none(), "Cancel throws it away at once");
    }
}

/// Twelve more events at 09:00 on Thursday 5 November, past what the week shows side by side: a "+N more" link.
fn crowded(harness: &mut Harness<'static, InboxApp>) {
    use crate::calendar::{Day, EventRow, Moment};
    let thursday = Day::new(2026, 11, 5).unwrap();
    let mut events: Vec<EventRow> = harness.state().calendar().events().cloned().collect();
    let model = events.iter().find(|event| !event.all_day).cloned().unwrap();
    for index in 0..12_u64 {
        let mut event = model.clone();
        event.id = 40_000 + index;
        event.summary = format!("Crowd {index}");
        event.start = Moment { day: thursday, minute: 540 };
        event.end = Moment { day: thursday, minute: 600 };
        (event.first, event.last, event.all_day, event.repeat) = (thursday, thursday, false, None);
        events.push(event);
    }
    harness.state_mut().calendar_mut().0.set_events(events);
    harness.run();
}

/// The centre of the first button whose name holds `label`.
fn centre(harness: &Harness<'static, InboxApp>, label: &str) -> egui::Pos2 {
    harness.query_all(by().role(Role::Button).label_contains(label)).next().unwrap().rect().center()
}

/// What a press under the editor could change: the day, the view, the pick, the day's float, the events listed.
fn under(harness: &Harness<'static, InboxApp>) -> (crate::calendar::Day, View, Option<crate::calendar::Picked>, Option<crate::calendar::Day>, usize) {
    let calendar = harness.state().calendar();
    (calendar.focus(), calendar.view(), calendar.picked().map(crate::calendar::EventRow::key), calendar.more(), calendar.events().count())
}

#[test]
fn a_press_outside_the_editor_reaches_nothing_under_it() {
    let mut harness = window();
    // A day of this week other than today, so that Today would change it.
    {
        let (calendar, mailbox) = harness.state_mut().calendar_mut();
        calendar.go_to(mailbox, crate::calendar::Day::new(2026, 11, 3).unwrap());
    }
    harness.run();
    crowded(&mut harness);
    let month = harness.get_by_role_and_label(Role::RadioButton, "Month").rect().center();
    let next = harness.get_by_role_and_label(Role::Button, "Next week").rect().center();
    let today = harness.get_by_role_and_label(Role::Button, "Today").rect().center();
    let day = centre(&harness, "Friday");
    let event = centre(&harness, "Studio stand-up");
    let more = harness
        .query_all(by().role(Role::Button))
        .find(|node| node.accesskit_node().label().is_some_and(|label| label.starts_with('+')))
        .unwrap()
        .rect()
        .center();
    typed(&mut harness, "Kiln");
    let before = under(&harness);
    for (name, at) in [("Next week", next), ("Today", today), ("Month", month), ("a day", day), ("an event", event), ("+N more", more)] {
        press_at(&mut harness, at.x, at.y);
        harness.run();
        assert_eq!(under(&harness), before, "a press on {name} outside the editor changes nothing under it");
        assert_eq!(summary(&harness).as_deref(), Some("Kiln"), "and keeps the typing ({name})");
        assert!(warns(&harness), "it only warns ({name})");
    }
    // New event, the editor's own anchor: the typing stays and nothing else opens.
    let new = harness.get_by_role_and_label(Role::Button, "New event").rect().center();
    press_at(&mut harness, new.x, new.y);
    harness.run();
    assert_eq!(summary(&harness).as_deref(), Some("Kiln"));
    assert!(harness.state().calendar().editor().is_some_and(|editor| editor.id().is_none()));
    // An editor with nothing typed closes on the press, and the release presses nothing under it.
    harness.state_mut().calendar_mut().0.close_editor();
    harness.run();
    for (name, at) in [("Month", month), ("an event", event), ("New event", new)] {
        harness.get_by_role_and_label(Role::Button, "New event").click();
        harness.run();
        assert!(harness.state().calendar().editor().is_some());
        let before = under(&harness);
        press_at(&mut harness, at.x, at.y);
        harness.run();
        assert!(harness.state().calendar().editor().is_none(), "nothing typed: a press on {name} closes the editor");
        assert_eq!(under(&harness), before, "and does nothing else ({name})");
    }
    // The editor's own controls still take the pointer.
    typed(&mut harness, "Kiln");
    let cancel = harness.get_by_role_and_label(Role::Button, "Cancel").rect().center();
    press_at(&mut harness, cancel.x, cancel.y);
    harness.run();
    assert!(summary(&harness).is_none(), "a press on Cancel inside the editor is Cancel's");
}

#[test]
fn a_press_on_delete_beside_the_editor_never_deletes_the_event_edited() {
    let mut harness = window();
    harness.query_all(by().role(Role::Button).label_contains("Studio stand-up")).next().unwrap().click();
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Edit").click();
    harness.run();
    let id = harness.state().calendar().editor().and_then(crate::calendar::Editor::id).unwrap();
    type_more(&mut harness, " moved");
    let delete = harness.get_by_role_and_label(Role::Button, "Delete").rect().center();
    press_at(&mut harness, delete.x, delete.y);
    harness.run();
    assert!(harness.state().calendar().events().any(|event| event.id == id), "the event edited is still there");
    assert!(harness.state().calendar().picked().is_some_and(|event| event.id == id), "and still picked");
    assert!(summary(&harness).is_some_and(|summary| summary.ends_with(" moved")));
    assert!(warns(&harness));
}
