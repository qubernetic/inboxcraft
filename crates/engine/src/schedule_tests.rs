// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The scheduler on the fake clock, and the time presets across a change of the offset.

use std::time::Duration;

use inboxcraft_model::{AccountId, Draft, DraftId, Due, MessageId, MessageState, ScheduleId, Scheduled};
use inboxcraft_store::{Batch, Change, Memory, Storage};
use inboxcraft_time::{CivilDateTime, FakeClock, Offset, OffsetTable, Offsets, Timestamp};
use serde_json::{Value, json};

use super::{Fired, LATE, Preset, presets};
use crate::test_support::{NOW, at, mailbox};
use crate::{AgentClass, CommandSpec, Context, EngineError, Outcome, Registry, Session};

fn hours(hours: i16) -> Offset {
    Offset::from_minutes(hours * 60).unwrap()
}

fn utc(year: i32, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> Timestamp {
    CivilDateTime::new(year, month, day, hour, minute, second).unwrap().to_timestamp(Offset::UTC).unwrap()
}

/// Central Europe in 2026: +01:00, +02:00 from 29 March 01:00 UTC, +01:00 again from 25 October 01:00 UTC.
fn europe() -> OffsetTable {
    OffsetTable::new(hours(1)).with_change(utc(2026, 3, 29, 1, 0, 0), hours(2)).with_change(utc(2026, 10, 25, 1, 0, 0), hours(1))
}

fn find(list: &[(Preset, Timestamp)], preset: Preset) -> Option<Timestamp> {
    list.iter().find(|(name, _)| *name == preset).map(|(_, at)| *at)
}

#[test]
fn the_presets_in_the_morning_are_later_today_this_evening_and_tomorrow_morning() {
    let list = presets(utc(2026, 11, 5, 9, 0, 0), &Offset::UTC);
    assert_eq!(
        list,
        vec![
            (Preset::LaterToday, utc(2026, 11, 5, 12, 0, 0)),
            (Preset::ThisEvening, utc(2026, 11, 5, 20, 0, 0)),
            (Preset::TomorrowMorning, utc(2026, 11, 6, 8, 0, 0)),
        ]
    );
    assert_eq!(Preset::ALL.map(Preset::name), ["later_today", "this_evening", "tomorrow_morning"]);
}

#[test]
fn later_today_is_three_hours_on_rounded_to_the_nearest_half_hour() {
    for ((minute, second), expected) in [((0, 0), (12, 0)), ((14, 59), (12, 0)), ((15, 0), (12, 30)), ((44, 59), (12, 30)), ((45, 0), (13, 0))] {
        let now = utc(2026, 11, 5, 9, minute, second);
        let later = find(&presets(now, &Offset::UTC), Preset::LaterToday);
        assert_eq!(later, Some(utc(2026, 11, 5, expected.0, expected.1, 0)), "at 09:{minute:02}:{second:02}");
    }
}

#[test]
fn a_preset_that_is_not_ahead_is_hidden() {
    let at_evening = presets(utc(2026, 11, 5, 20, 0, 0), &Offset::UTC);
    assert_eq!(find(&at_evening, Preset::ThisEvening), None, "20:00 is not ahead at 20:00");
    assert_eq!(find(&at_evening, Preset::LaterToday), Some(utc(2026, 11, 5, 23, 0, 0)));
    let before = presets(utc(2026, 11, 5, 19, 59, 59), &Offset::UTC);
    assert_eq!(find(&before, Preset::ThisEvening), Some(utc(2026, 11, 5, 20, 0, 0)));
}

#[test]
fn later_today_that_falls_tomorrow_is_hidden() {
    let late = presets(utc(2026, 11, 5, 21, 0, 0), &Offset::UTC);
    assert_eq!(late, vec![(Preset::TomorrowMorning, utc(2026, 11, 6, 8, 0, 0))]);
    let early = presets(utc(2026, 11, 6, 0, 10, 0), &Offset::UTC);
    assert_eq!(find(&early, Preset::TomorrowMorning), Some(utc(2026, 11, 7, 8, 0, 0)), "after midnight tomorrow is the next day");
    assert_eq!(find(&early, Preset::LaterToday), Some(utc(2026, 11, 6, 3, 0, 0)));
}

#[test]
fn the_presets_are_in_the_computers_offset() {
    // 03:40 UTC is 09:10 at +05:30; three hours on is 12:10, rounded to 12:00 there.
    let india = Offset::from_minutes(330).unwrap();
    let list = presets(utc(2026, 11, 5, 3, 40, 0), &india);
    assert_eq!(find(&list, Preset::LaterToday), Some(utc(2026, 11, 5, 6, 30, 0)));
    assert_eq!(find(&list, Preset::ThisEvening), Some(utc(2026, 11, 5, 14, 30, 0)));
    assert_eq!(find(&list, Preset::TomorrowMorning), Some(utc(2026, 11, 6, 2, 30, 0)));
    // 23:30 UTC on the 5th is 18:30 on the 5th at -05:00.
    let list = presets(utc(2026, 11, 5, 23, 30, 0), &hours(-5));
    assert_eq!(find(&list, Preset::LaterToday), Some(utc(2026, 11, 6, 2, 30, 0)), "21:30 there");
    assert_eq!(find(&list, Preset::ThisEvening), Some(utc(2026, 11, 6, 1, 0, 0)));
    assert_eq!(find(&list, Preset::TomorrowMorning), Some(utc(2026, 11, 6, 13, 0, 0)));
}

#[test]
fn tomorrow_morning_across_the_spring_change_is_eight_on_the_new_offset() {
    // Saturday 28 March, 21:00 at +01:00. Tomorrow 08:00 is at +02:00.
    let list = presets(utc(2026, 3, 28, 20, 0, 0), &europe());
    assert_eq!(find(&list, Preset::TomorrowMorning), Some(utc(2026, 3, 29, 6, 0, 0)));
    assert_eq!(find(&list, Preset::LaterToday), None, "midnight is tomorrow");
    assert_eq!(find(&list, Preset::ThisEvening), None);
}

#[test]
fn later_today_across_the_spring_change_is_three_real_hours() {
    // 00:30 at +01:00 on 29 March; three hours on is 02:30 UTC, which the clock shows as 04:30.
    let now = utc(2026, 3, 28, 23, 30, 0);
    let table = europe();
    let list = presets(now, &table);
    let later = find(&list, Preset::LaterToday).unwrap();
    assert_eq!(later, utc(2026, 3, 29, 2, 30, 0));
    assert_eq!(table.civil(later), CivilDateTime::new(2026, 3, 29, 4, 30, 0));
    assert_eq!(find(&list, Preset::ThisEvening), Some(utc(2026, 3, 29, 18, 0, 0)), "20:00 at +02:00");
    assert_eq!(find(&list, Preset::TomorrowMorning), Some(utc(2026, 3, 30, 6, 0, 0)));
}

#[test]
fn later_today_across_the_autumn_change_is_three_real_hours_not_the_first_0230() {
    // 00:30 at +02:00 on 25 October; three hours on is 01:30 UTC, the second 02:30 of the night.
    let now = utc(2026, 10, 24, 22, 30, 0);
    let list = presets(now, &europe());
    assert_eq!(find(&list, Preset::LaterToday), Some(utc(2026, 10, 25, 1, 30, 0)));
    assert_eq!(find(&list, Preset::ThisEvening), Some(utc(2026, 10, 25, 19, 0, 0)), "20:00 at +01:00");
    assert_eq!(find(&list, Preset::TomorrowMorning), Some(utc(2026, 10, 26, 7, 0, 0)));
}

#[test]
fn no_preset_past_the_end_of_time() {
    assert!(presets(Timestamp::MAX, &Offset::UTC).is_empty());
    assert!(presets(utc(10_000, 12, 31, 12, 0, 0), &Offset::UTC).iter().all(|(preset, _)| *preset != Preset::TomorrowMorning));
    assert!(presets(Timestamp::MIN, &Offset::from_minutes(-5999).unwrap()).is_empty(), "no wall clock before year 0");
}

// The scheduler.

const DRAFT: DraftId = DraftId::new(7);

fn entry(id: u64, due: i64, what: Due) -> Scheduled {
    Scheduled { id: ScheduleId::new(id), due: at(due), what }
}

/// The test mailbox with a draft and `entries` in its schedule, and message 1 snoozed and followed up as given.
fn store_with(entries: &[Scheduled], state: MessageState) -> Memory {
    let mut store = mailbox();
    let mut batch =
        Batch::new().with(Change::PutDraft(Draft::new(DRAFT, AccountId::new(1), at(NOW)))).with(Change::SetMessageState(MessageId::new(1), state));
    for entry in entries {
        batch.push(Change::PutScheduled(entry.clone()));
    }
    store.apply(&batch).unwrap();
    store
}

fn session_at(store: Memory, clock: &FakeClock) -> Session {
    Session::new(Box::new(store), Box::new(clock.clone())).unwrap()
}

#[test]
fn nothing_fires_before_its_time_and_the_next_time_is_known() {
    let clock = FakeClock::new(at(NOW));
    let mut session = session_at(store_with(&[entry(1, NOW + 60, Due::Send(DRAFT))], MessageState::default()), &clock);
    assert_eq!(session.tick().unwrap(), vec![]);
    assert_eq!(session.next_due().unwrap(), Some(at(NOW + 60)));
    assert_eq!(session.store().schedule().unwrap().len(), 1);
    assert!(session.take_jobs().is_empty());
}

#[test]
fn an_entry_fires_once_at_its_time_and_leaves_the_schedule() {
    let clock = FakeClock::new(at(NOW));
    let send = entry(1, NOW + 60, Due::Send(DRAFT));
    let mut session = session_at(store_with(std::slice::from_ref(&send), MessageState::default()), &clock);
    clock.advance(Duration::from_secs(60));
    assert_eq!(session.tick().unwrap(), vec![Fired { entry: send, missed: false }]);
    assert_eq!(session.tick().unwrap(), vec![], "only once");
    assert_eq!(session.next_due().unwrap(), None);
    assert!(session.store().schedule().unwrap().is_empty());
    assert!(session.store().draft(DRAFT).unwrap().is_some(), "the draft stays for the sending to take");
}

#[test]
fn entries_fire_in_time_order() {
    let clock = FakeClock::new(at(NOW));
    let entries = [entry(1, NOW - 10, Due::FollowUp(MessageId::new(2))), entry(2, NOW - 20, Due::Send(DRAFT))];
    let mut session = session_at(store_with(&entries, MessageState::default()), &clock);
    let fired: Vec<u64> = session.tick().unwrap().into_iter().map(|fired| fired.entry.id.get()).collect();
    assert_eq!(fired, vec![2, 1]);
}

#[test]
fn an_entry_handled_late_is_reported_missed() {
    let clock = FakeClock::new(at(NOW));
    let late_secs = i64::try_from(LATE.as_secs()).unwrap();
    let entries = [entry(1, NOW - late_secs, Due::Send(DRAFT)), entry(2, NOW - late_secs - 1, Due::FollowUp(MessageId::new(2)))];
    let mut session = session_at(store_with(&entries, MessageState::default()), &clock);
    let missed: Vec<(u64, bool)> = session.tick().unwrap().into_iter().map(|fired| (fired.entry.id.get(), fired.missed)).collect();
    assert_eq!(missed, vec![(2, true), (1, false)]);
}

#[test]
fn an_entry_due_while_inboxcraft_was_off_is_reported_missed_at_start() {
    let clock = FakeClock::new(at(NOW));
    let entries = [entry(1, NOW - 5, Due::Send(DRAFT)), entry(2, NOW + 5, Due::FollowUp(MessageId::new(2)))];
    let mut session = session_at(store_with(&entries, MessageState::default()), &clock);
    session.start();
    let fired = session.tick().unwrap();
    assert_eq!(fired, vec![Fired { entry: entries[0].clone(), missed: true }]);
    clock.advance(Duration::from_secs(5));
    assert_eq!(session.tick().unwrap(), vec![Fired { entry: entries[1].clone(), missed: false }], "due after the start");
}

#[test]
fn a_returned_snooze_and_a_fired_follow_up_clear_their_times_and_nothing_else() {
    let clock = FakeClock::new(at(NOW));
    let state = MessageState { pinned: true, snoozed_until: Some(at(NOW)), follow_up: Some(at(NOW)), ..MessageState::default() };
    let entries = [entry(1, NOW, Due::Unsnooze(MessageId::new(1))), entry(2, NOW, Due::FollowUp(MessageId::new(1)))];
    let mut session = session_at(store_with(&entries, state), &clock);
    assert_eq!(session.tick().unwrap().len(), 2);
    assert_eq!(session.store().message_state(MessageId::new(1)).unwrap(), MessageState { pinned: true, ..MessageState::default() });
}

#[test]
fn an_entry_for_another_time_than_the_message_shows_leaves_the_message_alone() {
    let clock = FakeClock::new(at(NOW));
    let state = MessageState { snoozed_until: Some(at(NOW + 3600)), follow_up: Some(at(NOW + 3600)), ..MessageState::default() };
    let entries = [entry(1, NOW, Due::Unsnooze(MessageId::new(1))), entry(2, NOW, Due::FollowUp(MessageId::new(1)))];
    let mut session = session_at(store_with(&entries, state.clone()), &clock);
    assert_eq!(session.tick().unwrap().len(), 2);
    assert_eq!(session.store().message_state(MessageId::new(1)).unwrap(), state);
}

#[test]
fn a_new_entry_gets_an_id_past_every_entry_held() {
    fn add(context: &Context<'_>, _: &Value) -> Result<Outcome, EngineError> {
        let id = context.next_schedule_id();
        let due = context.now().saturating_add(Duration::from_secs(60));
        Ok(Outcome::new(json!({ "id": id.get() })).with_change(Change::PutScheduled(Scheduled { id, due, what: Due::Send(DRAFT) })))
    }
    let mut registry = Registry::new();
    registry.register(CommandSpec { id: "test.add", class: Some(AgentClass::Owner), summary: "A test command", run: add }).unwrap();
    let clock = FakeClock::new(at(NOW));
    let store = store_with(&[entry(5, NOW + 600, Due::Send(DRAFT))], MessageState::default());
    let mut session = Session::with_registry(Box::new(store), Box::new(clock), registry).unwrap();
    assert_eq!(session.run("test.add", &json!({})).unwrap(), json!({ "id": 6 }));
    assert_eq!(session.run("test.add", &json!({})).unwrap(), json!({ "id": 7 }));
    assert_eq!(session.next_due().unwrap(), Some(at(NOW + 60)));
}

#[test]
fn the_session_works_in_the_offsets_it_is_given() {
    fn offset(context: &Context<'_>, _: &Value) -> Result<Outcome, EngineError> {
        Ok(Outcome::new(json!(context.offsets().offset_at(context.now()).minutes())))
    }
    let mut registry = Registry::new();
    registry.register(CommandSpec { id: "test.offset", class: Some(AgentClass::Safe), summary: "A test command", run: offset }).unwrap();
    let mut session = Session::with_registry(Box::new(mailbox()), Box::new(FakeClock::new(at(NOW))), registry).unwrap();
    assert_eq!(session.run("test.offset", &json!({})).unwrap(), json!(0), "UTC until told");
    session.set_offsets(Box::new(hours(2)));
    assert_eq!(session.run("test.offset", &json!({})).unwrap(), json!(120));
}
