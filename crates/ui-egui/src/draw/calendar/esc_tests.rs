// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Esc goes to the surface on top alone (the palette, the settings sheet, a list, a float, then the editor, then the
//! picked event): one Esc never closes two things and never reaches what lies under the surface that has the keyboard.

use egui::accesskit::Role;
use egui::{Event, Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use inboxcraft_time::{FakeClock, Timestamp};

use crate::calendar::View;
use crate::mailbox::Mailbox;
use crate::{InboxApp, sample};

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

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

fn typed(harness: &mut Harness<'static, InboxApp>) {
    harness.get_by_role_and_label(Role::Button, "New event").click();
    harness.run();
    harness.get_by_role_and_label(Role::TextInput, "Title").focus();
    harness.get_by_role_and_label(Role::TextInput, "Title").type_text("Kiln");
    harness.run();
}

fn summary(harness: &Harness<'static, InboxApp>) -> Option<String> {
    harness.state().calendar().editor().map(|editor| editor.form().summary.clone())
}

fn warned(harness: &Harness<'static, InboxApp>) -> bool {
    harness.state().calendar().editor().is_some_and(crate::calendar::Editor::warned)
}

/// Esc pressed in a frame of its own and let go in the next, as a keyboard sends it.
fn esc(harness: &mut Harness<'static, InboxApp>) {
    for pressed in [true, false] {
        harness.event(Event::Key { key: Key::Escape, physical_key: None, pressed, repeat: false, modifiers: Modifiers::NONE });
        harness.step();
    }
    harness.run();
}

#[test]
fn esc_with_the_palette_over_the_editor_closes_the_palette_alone() {
    let mut harness = window();
    typed(&mut harness);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::K);
    harness.run();
    assert!(harness.state().palette().is_open());
    esc(&mut harness);
    assert!(!harness.state().palette().is_open(), "the palette, on top, takes the Esc");
    assert_eq!(summary(&harness).as_deref(), Some("Kiln"));
    assert!(!warned(&harness), "the editor under it never saw the Esc");
    // Then the editor is on top: Esc asks, a second Esc throws the typing away.
    esc(&mut harness);
    assert!(warned(&harness));
    esc(&mut harness);
    assert!(summary(&harness).is_none());
}

#[test]
fn esc_with_the_settings_sheet_over_the_calendar_closes_the_sheet_alone() {
    let mut harness = window();
    harness.key_press(Key::ArrowDown);
    harness.run();
    let picked = harness.state().calendar().picked().map(|event| event.summary.clone());
    assert!(picked.is_some());
    harness.state_mut().open_settings();
    harness.run();
    esc(&mut harness);
    assert!(!harness.state().settings().is_open(), "the sheet takes the Esc");
    assert_eq!(harness.state().calendar().picked().map(|event| event.summary.clone()), picked, "the pick under the sheet stays");
    // Then the pick is on top: Esc lets it go.
    esc(&mut harness);
    assert!(harness.state().calendar().picked().is_none());
}

#[test]
fn esc_with_the_settings_sheet_over_the_editor_closes_the_sheet_alone() {
    let mut harness = window();
    typed(&mut harness);
    harness.state_mut().open_settings();
    harness.run();
    esc(&mut harness);
    assert!(!harness.state().settings().is_open());
    assert_eq!(summary(&harness).as_deref(), Some("Kiln"));
    assert!(!warned(&harness), "the editor under the sheet never saw the Esc");
}

/// Twelve events at one hour on Thursday 5 November: the week shows four and "+N more".
fn crowded(harness: &mut Harness<'static, InboxApp>) {
    let thursday = crate::calendar::Day::new(2026, 11, 5).unwrap();
    let mut events: Vec<crate::calendar::EventRow> = harness.state().calendar().events().cloned().collect();
    let model = events.first().cloned().unwrap();
    for index in 0..12_u64 {
        let mut event = model.clone();
        event.id = 20_000 + index;
        event.summary = format!("Crowd {index}");
        event.start = crate::calendar::Moment { day: thursday, minute: 540 };
        event.end = crate::calendar::Moment { day: thursday, minute: 600 };
        (event.first, event.last, event.all_day, event.repeat) = (thursday, thursday, false, None);
        events.push(event);
    }
    harness.state_mut().calendar_mut().0.set_events(events);
    harness.run();
}

#[test]
fn esc_with_the_palette_over_the_day_s_float_closes_the_palette_alone() {
    let mut harness = window();
    crowded(&mut harness);
    let thursday = crate::calendar::Day::new(2026, 11, 5).unwrap();
    harness.state_mut().calendar_mut().0.set_more(Some(thursday));
    harness.run();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::K);
    harness.run();
    assert!(harness.state().palette().is_open());
    esc(&mut harness);
    assert!(!harness.state().palette().is_open(), "the palette, on top, takes the Esc");
    assert_eq!(harness.state().calendar().more(), Some(thursday), "the day's float under it stays");
    esc(&mut harness);
    assert_eq!(harness.state().calendar().more(), None, "then the float is on top and closes");
}
