// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Done, park, pin, snooze and follow-up reminders in the mailbox, through the engine's `mail.done`, `mail.park`,
//! `mail.pin`, `mail.snooze`, `mail.follow_up` and the lists `park.list`, `pin.list` and `mail.list`, over the sample.

use inboxcraft_engine::{MAX_HISTORY, Session};
use inboxcraft_model::{Message, MessageId, Role, Uid};
use inboxcraft_store::{Batch, Change, Storage};
use inboxcraft_time::{Clock, FakeClock, Timestamp};

use super::{Act, GroupKind, Mailbox, Pile, Place};
use crate::sample;

const ROTA: &str = "Allotment rota for November";
const LOAN: &str = "Your library loan is due on Friday";

/// Thursday 5 November 2026, 09:00 UTC.
const NOW: i64 = 1_793_869_200;

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(NOW, 0).unwrap()))
}

fn mailbox() -> Mailbox {
    Mailbox::new(sample::session(clock()).unwrap(), clock())
}

fn id_of(mailbox: &Mailbox, subject: &str) -> u64 {
    mailbox.messages().iter().find(|row| row.subject == subject).map(|row| row.id).unwrap()
}

fn subjects(mailbox: &Mailbox) -> Vec<&str> {
    mailbox.messages().iter().map(|row| row.subject.as_str()).collect()
}

#[test]
fn done_files_the_message_and_show_done_lists_it_in_its_old_place() {
    let mut mailbox = mailbox();
    let rota = id_of(&mailbox, ROTA);
    mailbox.act(rota, Act::Done);
    assert!(!subjects(&mailbox).contains(&ROTA), "done leaves the inbox");
    assert!(!mailbox.show_done());
    mailbox.set_show_done(true);
    let row = mailbox.messages().iter().find(|row| row.id == rota).unwrap();
    assert!(row.done, "shown in its old place, marked done");
    assert!(row.seen, "done marks it read");
    assert!(mailbox.messages().iter().filter(|row| row.id != rota).all(|row| !row.done));
    mailbox.set_show_done(false);
    assert!(!subjects(&mailbox).contains(&ROTA));
}

#[test]
fn undo_brings_a_done_message_back() {
    let mut mailbox = mailbox();
    let rota = id_of(&mailbox, ROTA);
    mailbox.act(rota, Act::Done);
    assert_eq!(mailbox.undoable().map(|undo| undo.text.as_str()), Some("Marked as done"));
    mailbox.undo();
    let row = mailbox.messages().iter().find(|row| row.id == rota).unwrap();
    assert!(!row.done && !row.seen, "back where it was, unread");
}

#[test]
fn not_done_brings_a_done_message_back_where_it_was() {
    let mut mailbox = mailbox();
    let rota = id_of(&mailbox, ROTA);
    mailbox.act(rota, Act::Done);
    mailbox.set_show_done(true);
    mailbox.act(rota, Act::Undone);
    assert_eq!(mailbox.undoable().map(|undo| undo.text.as_str()), Some("Back in its place"));
    mailbox.set_show_done(false);
    let row = mailbox.messages().iter().find(|row| row.id == rota).unwrap();
    assert!(!row.done);
    assert_eq!(mailbox.place(), Place::View(Role::Inbox));
}

#[test]
fn a_parked_message_leaves_the_list_for_the_park_pile() {
    let mut mailbox = mailbox();
    let rota = id_of(&mailbox, ROTA);
    assert_eq!(mailbox.parked(), 0);
    mailbox.act(rota, Act::Park(true));
    assert_eq!(mailbox.undoable().map(|undo| undo.text.as_str()), Some("Parked"));
    assert!(!subjects(&mailbox).contains(&ROTA));
    assert_eq!(mailbox.parked(), 1, "the inbox counts its parked mail");
    mailbox.open(Place::Later(Pile::Parked));
    assert_eq!(subjects(&mailbox), [ROTA]);
    assert_eq!(mailbox.title(), "Later");
    assert_eq!(mailbox.pile_count(Pile::Parked), 1);
    mailbox.act(rota, Act::Park(false));
    assert_eq!(mailbox.undoable().map(|undo| undo.text.as_str()), Some("Taken off the park pile"));
    assert!(mailbox.messages().is_empty());
    mailbox.undo();
    assert_eq!(subjects(&mailbox), [ROTA], "undo parks it again");
}

#[test]
fn pins_stay_in_the_list_and_gather_in_the_pins_pile() {
    let mut mailbox = mailbox();
    let rota = id_of(&mailbox, ROTA);
    mailbox.act(rota, Act::Pin(true));
    assert_eq!(mailbox.undoable().map(|undo| undo.text.as_str()), Some("Pinned"));
    assert!(mailbox.messages().iter().any(|row| row.id == rota && row.pinned), "a pinned message stays where it is");
    mailbox.open(Place::Later(Pile::Pins));
    assert_eq!(subjects(&mailbox), [ROTA]);
    assert_eq!((mailbox.pile_count(Pile::Snoozed), mailbox.pile_count(Pile::Pins), mailbox.pile_count(Pile::Parked)), (0, 1, 0));
    mailbox.act(rota, Act::Pin(false));
    assert_eq!(mailbox.undoable().map(|undo| undo.text.as_str()), Some("Pin taken off"));
    assert!(mailbox.messages().is_empty());
}

#[test]
fn the_piles_have_their_names() {
    let names: Vec<(&str, &str)> = Pile::ALL.iter().map(|pile| (pile.id(), pile.text())).collect();
    assert_eq!(names, [("snoozed", "Snoozed"), ("pins", "Pins"), ("parked", "Parked")]);
    assert_eq!(Pile::named("pins"), Some(Pile::Pins));
    assert_eq!(Pile::named("later"), None);
}

#[test]
fn a_pin_outlasts_done_when_the_owner_says_so() {
    let mut mailbox = mailbox();
    assert!(!mailbox.pins_outlast_done());
    mailbox.set_pins_outlast_done(true);
    assert!(mailbox.pins_outlast_done());
    let rota = id_of(&mailbox, ROTA);
    mailbox.act(rota, Act::Pin(true));
    mailbox.act(rota, Act::Done);
    mailbox.open(Place::Later(Pile::Pins));
    assert_eq!(subjects(&mailbox), [ROTA], "the pin stays on the done message");
    mailbox.set_pins_outlast_done(false);
    mailbox.open(Place::View(Role::Inbox));
    let loan = id_of(&mailbox, LOAN);
    mailbox.act(loan, Act::Pin(true));
    mailbox.act(loan, Act::Done);
    mailbox.open(Place::Later(Pile::Pins));
    assert_eq!(subjects(&mailbox), [ROTA], "done took the second pin off");
}

#[test]
fn a_snoozed_message_waits_in_the_snoozed_pile_with_its_time() {
    let mut mailbox = mailbox();
    let rota = id_of(&mailbox, ROTA);
    let until = Timestamp::from_unix(NOW + 2 * 86_400, 0).unwrap();
    mailbox.act(rota, Act::Snooze(until));
    assert_eq!(mailbox.undoable().map(|undo| undo.text.as_str()), Some("Snoozed until Saturday 7 November, 09:00"));
    assert!(!subjects(&mailbox).contains(&ROTA));
    mailbox.open(Place::Later(Pile::Snoozed));
    let row = mailbox.messages().iter().find(|row| row.id == rota).unwrap();
    assert_eq!(row.snoozed_until, Some(until));
    assert_eq!(mailbox.row_date(row), "Sat 09:00", "the pile shows when it comes back");
    assert!(mailbox.is_snoozed(rota));
    mailbox.act(rota, Act::Unsnooze);
    assert_eq!(mailbox.undoable().map(|undo| (undo.text.as_str(), undo.undoable)), Some(("Back in the inbox", false)));
    mailbox.open(Place::View(Role::Inbox));
    let row = mailbox.messages().iter().find(|row| row.id == rota).unwrap();
    assert!(row.returned.is_some(), "it came back with the returned mark");
    assert_eq!(mailbox.messages().first().map(|row| row.id), Some(rota), "listed as if it arrived now");
}

#[test]
fn undo_takes_a_snooze_back() {
    let mut mailbox = mailbox();
    let rota = id_of(&mailbox, ROTA);
    mailbox.act(rota, Act::Snooze(Timestamp::from_unix(NOW + 3_600, 0).unwrap()));
    mailbox.undo();
    let row = mailbox.messages().iter().find(|row| row.id == rota).unwrap();
    assert_eq!(row.snoozed_until, None);
}

#[test]
fn the_presets_come_from_the_engine_with_their_names() {
    let mut mailbox = mailbox();
    assert!(mailbox.presets().is_empty(), "read when a picker opens");
    mailbox.load_presets();
    let names: Vec<&str> = mailbox.presets().iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["Later today", "This evening", "Tomorrow morning", "This weekend", "Next week"]);
    assert!(mailbox.presets().iter().all(|(_, at)| *at > mailbox.now()));
    // Thursday 5 November 2026 09:00 UTC: Saturday 7 at 09:00, and the week from Monday 9 at 09:00 (not 08:00).
    let at = |name: &str| mailbox.presets().iter().find(|(shown, _)| shown == name).map(|(_, at)| at.unix_seconds());
    assert_eq!(at("This weekend"), Some(NOW + 2 * 86_400));
    assert_eq!(at("Next week"), Some(NOW + 4 * 86_400));
}

#[test]
fn with_saturday_first_this_weekend_and_next_week_both_show_at_the_same_time() {
    let mut mailbox = mailbox();
    mailbox.run("week.set", &serde_json::json!({ "first_day": "saturday" })).unwrap();
    mailbox.load_presets();
    let names: Vec<&str> = mailbox.presets().iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["Later today", "This evening", "Tomorrow morning", "This weekend", "Next week"], "both, in the engine's order");
    let at = |name: &str| mailbox.presets().iter().find(|(shown, _)| shown == name).map(|(_, at)| at.unix_seconds());
    // The week holding Thursday 12 November starts on Saturday 7: the same moment as this weekend.
    assert_eq!(at("This weekend"), Some(NOW + 2 * 86_400));
    assert_eq!(at("Next week"), Some(NOW + 2 * 86_400));
}

#[test]
fn a_follow_up_reminder_goes_on_sent_mail() {
    let mut mailbox = mailbox();
    mailbox.open(Place::View(Role::Sent));
    let plans = id_of(&mailbox, "Weekend plans");
    assert!(mailbox.is_sent(plans));
    let until = Timestamp::from_unix(NOW + 86_400, 0).unwrap();
    mailbox.act(plans, Act::FollowUp(until));
    assert_eq!(mailbox.undoable().map(|undo| undo.text.as_str()), Some("Reminder set for Friday 6 November, 09:00"));
    let row = mailbox.messages().iter().find(|row| row.id == plans).unwrap();
    assert_eq!(row.follow_up, Some(until));
    mailbox.act(plans, Act::CancelFollowUp);
    assert_eq!(mailbox.undoable().map(|undo| undo.text.as_str()), Some("Reminder taken off"));
    mailbox.undo();
    assert!(mailbox.messages().iter().any(|row| row.id == plans && row.follow_up == Some(until)));
    mailbox.open(Place::View(Role::Inbox));
    assert!(!mailbox.is_sent(id_of(&mailbox, ROTA)));
}

#[test]
fn a_reminder_on_mail_not_sent_is_refused_and_says_why() {
    let mut mailbox = mailbox();
    let rota = id_of(&mailbox, ROTA);
    mailbox.act(rota, Act::FollowUp(Timestamp::from_unix(NOW + 86_400, 0).unwrap()));
    assert!(mailbox.error().is_some());
    assert!(mailbox.undoable().is_none());
}

#[test]
fn done_all_files_every_message_of_a_group_with_one_undo() {
    let mut mailbox = mailbox();
    let ids: Vec<u64> = mailbox.messages().iter().filter(|row| row.group == Some(GroupKind::Updates)).map(|row| row.id).collect();
    assert_eq!(ids.len(), 3);
    mailbox.done_all(&ids);
    assert_eq!(mailbox.undoable().map(|undo| (undo.text.as_str(), undo.undoable)), Some(("Marked 3 as done", true)));
    assert!(mailbox.messages().iter().all(|row| !ids.contains(&row.id)));
    mailbox.undo();
    assert_eq!(mailbox.messages().iter().filter(|row| ids.contains(&row.id)).count(), 3, "one Undo brings all three back");
}

/// The sample mailbox with `copies` more messages from the sender of `LOAN`, all in the Updates group.
fn crowded(copies: u64) -> Mailbox {
    let loan = id_of(&mailbox(), LOAN);
    let mut store = sample::mailbox(clock().now()).unwrap();
    let message = store.message(MessageId::new(loan)).unwrap().unwrap().message().clone();
    let mut batch = Batch::new();
    for n in 1..=copies {
        let summary = message.summary().clone().with_uid(Uid::new(u32::try_from(1_000 + n).unwrap()).unwrap());
        batch.push(Change::PutMessage(Message::new(MessageId::new(1_000 + n), message.account(), message.folder(), summary)));
    }
    store.apply(&batch).unwrap();
    let mut session = Session::sample(Box::new(store), clock()).unwrap();
    session.run("screener.mode", &serde_json::json!({ "mode": "before" })).unwrap();
    Mailbox::new(session, clock())
}

#[test]
fn done_all_on_a_group_bigger_than_the_history_says_it_cannot_be_undone() {
    let mut mailbox = crowded(u64::try_from(MAX_HISTORY).unwrap());
    let ids: Vec<u64> = mailbox.messages().iter().filter(|row| row.group == Some(GroupKind::Updates)).map(|row| row.id).collect();
    assert!(ids.len() > MAX_HISTORY, "{}", ids.len());
    mailbox.done_all(&ids);
    let text = format!("Marked {} as done, too many to undo", ids.len());
    assert_eq!(mailbox.undoable().map(|undo| (undo.text.as_str(), undo.undoable)), Some((text.as_str(), false)));
    assert!(mailbox.messages().iter().all(|row| !ids.contains(&row.id)), "every one of them is done");
}

#[test]
fn done_all_within_the_history_keeps_its_undo() {
    let mut mailbox = crowded(20);
    let ids: Vec<u64> = mailbox.messages().iter().filter(|row| row.group == Some(GroupKind::Updates)).map(|row| row.id).collect();
    assert_eq!(ids.len(), 23);
    mailbox.done_all(&ids);
    assert_eq!(mailbox.undoable().map(|undo| undo.undoable), Some(true));
    mailbox.undo();
    assert_eq!(mailbox.messages().iter().filter(|row| ids.contains(&row.id)).count(), 23, "one Undo brings all of them back");
}

#[test]
fn done_all_of_nothing_does_nothing() {
    let mut mailbox = mailbox();
    mailbox.done_all(&[]);
    assert!(mailbox.undoable().is_none());
}

#[test]
fn a_returned_message_says_so_while_unread() {
    let mut mailbox = mailbox();
    let rota = id_of(&mailbox, ROTA);
    mailbox.act(rota, Act::Snooze(Timestamp::from_unix(NOW + 3_600, 0).unwrap()));
    mailbox.open(Place::Later(Pile::Snoozed));
    mailbox.act(rota, Act::Unsnooze);
    mailbox.open(Place::View(Role::Inbox));
    let row = mailbox.messages().iter().find(|row| row.id == rota).unwrap().clone();
    assert!(mailbox.returned(&row));
    assert!(mailbox.spoken(&row).ends_with("returned"), "{}", mailbox.spoken(&row));
    mailbox.act(rota, Act::Read);
    let row = mailbox.messages().iter().find(|row| row.id == rota).unwrap().clone();
    assert!(!mailbox.returned(&row), "read, it is an ordinary message again");
}

#[test]
fn bringing_back_a_snoozed_message_the_owner_deleted_says_only_its_snooze_went() {
    let mut mailbox = mailbox();
    let rota = id_of(&mailbox, ROTA);
    mailbox.act(rota, Act::Snooze(Timestamp::from_unix(NOW + 2 * 86_400, 0).unwrap()));
    mailbox.act(rota, Act::Delete);
    mailbox.act(rota, Act::Unsnooze);
    assert_eq!(mailbox.undoable().map(|undo| undo.text.as_str()), Some("Snooze taken off"), "it stays in Trash: never 'back in the inbox'");
}

/// The snoozed pile with four messages coming back at `NOW` plus each of `hours`, in the order the pile lists them.
fn snoozed_at(hours: [i64; 4]) -> (Mailbox, Vec<u64>) {
    let mut mailbox = mailbox();
    let ids: Vec<u64> = mailbox.messages().iter().take(4).map(|row| row.id).collect();
    for (id, hours) in ids.iter().zip(hours) {
        mailbox.act(*id, Act::Snooze(Timestamp::from_unix(NOW + hours * 3_600, 0).unwrap()));
    }
    mailbox.open(Place::Later(Pile::Snoozed));
    (mailbox, ids)
}

/// The heads of the pile, each with the ids under it.
fn heads(mailbox: &Mailbox) -> Vec<(String, Vec<u64>)> {
    mailbox.later_heads().into_iter().map(|group| (group.name, group.messages)).collect()
}

#[test]
fn the_snoozed_pile_stands_under_back_today_back_this_week_and_later() {
    // Thursday 5 November 09:00: today at 16:00; Saturday 7 at 09:00; Sunday 8 at 09:00; Monday 9 at 09:00.
    let (mut mailbox, ids) = snoozed_at([7, 48, 72, 96]);
    let (today, saturday, sunday, monday) = (ids[0], ids[1], ids[2], ids[3]);
    // Monday first (the default): the week ends as Sunday ends.
    assert_eq!(
        heads(&mailbox),
        [("Back today".to_owned(), vec![today]), ("Back this week".to_owned(), vec![saturday, sunday]), ("Later".to_owned(), vec![monday]),]
    );
    // Sunday first: Sunday 8 starts the next week, and so "Next week" of the presets.
    mailbox.run("week.set", &serde_json::json!({ "first_day": "sunday" })).unwrap();
    assert_eq!(
        heads(&mailbox),
        [("Back today".to_owned(), vec![today]), ("Back this week".to_owned(), vec![saturday]), ("Later".to_owned(), vec![sunday, monday]),]
    );
    mailbox.load_presets();
    let next_week = mailbox.presets().iter().find(|(name, _)| name == "Next week").map(|(_, at)| at.unix_seconds());
    assert_eq!(next_week, Some(NOW + 72 * 3_600), "the first row of Later is where Next week begins");
}

#[test]
fn a_head_with_no_row_is_left_out_and_other_piles_and_searches_have_none() {
    let (mut mailbox, ids) = snoozed_at([30, 31, 32, 33]);
    assert_eq!(heads(&mailbox), [("Back this week".to_owned(), ids.clone())], "only the head with rows");
    mailbox.search("rota");
    assert!(heads(&mailbox).is_empty(), "a search lists by time, under no head");
    mailbox.open(Place::Later(Pile::Pins));
    assert!(heads(&mailbox).is_empty(), "the pins have none");
    mailbox.open(Place::View(Role::Inbox));
    assert!(heads(&mailbox).is_empty(), "nor the inbox");
}

/// [`snoozed_at`] with the clock at `now` (Unix seconds) when the four are snoozed and the pile is read.
fn snoozed_from(now: i64, hours: [i64; 4]) -> (Mailbox, Vec<u64>) {
    let shared = FakeClock::new(Timestamp::from_unix(NOW, 0).unwrap());
    let mut mailbox = Mailbox::new(sample::session(Box::new(shared.clone())).unwrap(), Box::new(shared.clone()));
    shared.set(Timestamp::from_unix(now, 0).unwrap());
    let ids: Vec<u64> = mailbox.messages().iter().take(4).map(|row| row.id).collect();
    for (id, hours) in ids.iter().zip(hours) {
        mailbox.act(*id, Act::Snooze(Timestamp::from_unix(now + hours * 3_600, 0).unwrap()));
    }
    mailbox.open(Place::Later(Pile::Snoozed));
    (mailbox, ids)
}

#[test]
fn on_the_weeks_last_day_this_week_ends_with_today_and_its_head_is_left_out() {
    // Sunday 8 November 2026, 09:00 UTC: today at 16:00 and 23:00; Monday 9 at 05:00 and 09:00, the next week.
    let (mailbox, ids) = snoozed_from(1_794_128_400, [7, 14, 20, 24]);
    assert_eq!(heads(&mailbox), [("Back today".to_owned(), vec![ids[0], ids[1]]), ("Later".to_owned(), vec![ids[2], ids[3]])]);
}

#[test]
fn the_week_runs_on_across_the_years_turn() {
    // Thursday 31 December 2026, 09:00 UTC: the week runs to Sunday 3 January 2027.
    let (mailbox, ids) = snoozed_from(1_798_707_600, [7, 48, 72, 96]);
    assert_eq!(
        heads(&mailbox),
        [("Back today".to_owned(), vec![ids[0]]), ("Back this week".to_owned(), vec![ids[1], ids[2]]), ("Later".to_owned(), vec![ids[3]])]
    );
}

#[test]
fn a_junk_kept_week_rule_files_the_pile_by_the_default_week() {
    let (mut mailbox, ids) = snoozed_at([7, 48, 72, 96]);
    mailbox.run("week.set", &serde_json::json!({ "first_day": "sunday" })).unwrap();
    mailbox.restore_week(&serde_json::json!({ "first_day": 9, "first_week": [] }));
    // Monday first, as the default counts: Sunday 8 still in this week.
    assert_eq!(
        heads(&mailbox),
        [("Back today".to_owned(), vec![ids[0]]), ("Back this week".to_owned(), vec![ids[1], ids[2]]), ("Later".to_owned(), vec![ids[3]])]
    );
}

#[test]
fn where_the_calendar_cannot_say_when_today_ends_only_the_rows_whose_time_has_passed_are_back_today() {
    // Snoozed as the sample's week begins, then the clock set to 31 December 10000, the last day it counts: neither
    // today's nor the week's end can be told.
    let shared = FakeClock::new(Timestamp::from_unix(NOW, 0).unwrap());
    let mut mailbox = Mailbox::new(sample::session(Box::new(shared.clone())).unwrap(), Box::new(shared.clone()));
    let ids: Vec<u64> = mailbox.messages().iter().take(4).map(|row| row.id).collect();
    for (id, hours) in ids.iter().zip([7, 48, 72, 96]) {
        mailbox.act(*id, Act::Snooze(Timestamp::from_unix(NOW + hours * 3_600, 0).unwrap()));
    }
    let last_day = Timestamp::MAX.unix_seconds() - 15 * 3_600;
    shared.set(Timestamp::from_unix(last_day, 0).unwrap());
    mailbox.open(Place::Later(Pile::Snoozed));
    assert_eq!(mailbox.later_bounds(), (None, None), "no bounds");
    // Two come back later that day: the engine takes no time past the year 9999, so the rows are told so here.
    let ahead = [ids[2], ids[3]];
    for row in mailbox.messages.iter_mut().filter(|row| ahead.contains(&row.id)) {
        row.snoozed_until = Some(Timestamp::from_unix(last_day + 3 * 3_600, 0).unwrap());
    }
    assert_eq!(
        heads(&mailbox),
        [("Back today".to_owned(), vec![ids[0], ids[1]]), ("Later".to_owned(), vec![ids[2], ids[3]])],
        "the rows whose time has passed are back today; the others are not, with no bound to tell"
    );
}
