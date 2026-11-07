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
    assert_eq!(names, ["Later today", "This evening", "Tomorrow morning"]);
    assert!(mailbox.presets().iter().all(|(_, at)| *at > mailbox.now()));
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
