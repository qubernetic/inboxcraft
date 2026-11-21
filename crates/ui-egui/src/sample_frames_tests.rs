// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The frames' sample: what its scenes need is in it, and everyone in it is invented on `example.org`.

use inboxcraft_store::Storage;
use inboxcraft_time::{FakeClock, Offset, Timestamp};

use super::{BOUNCED, EMPTY_FOLDER, MESSAGES, TIME, mailbox, prepare, session, showcase};
use crate::mailbox::{Mailbox, Pile, Place};

fn now() -> Timestamp {
    Timestamp::from_unix(TIME, 0).unwrap()
}

/// The window's mailbox over the frames' sample at its time, put as the frames show it.
fn prepared() -> Mailbox {
    let clock = FakeClock::new(now());
    let mut mailbox = Mailbox::new(session(Box::new(clock.clone())).unwrap(), Box::new(clock.clone()));
    prepare(&mut mailbox, &clock).unwrap();
    // As the window does before it draws.
    mailbox.refresh();
    mailbox
}

#[test]
fn the_time_is_monday_16_november_2026_at_14_04() {
    let civil = now().to_civil(Offset::UTC).unwrap();
    assert_eq!((civil.year(), civil.month(), civil.day(), civil.hour(), civil.minute()), (2026, 11, 16, 14, 4));
}

#[test]
fn everyone_is_on_example_org() {
    let store = mailbox(now()).unwrap();
    let on_example = |address: &str| address.ends_with("@example.org") || address.ends_with(".example.org");
    let mut seen = 0;
    for account in store.accounts().unwrap() {
        assert!(on_example(account.address().as_str()), "{}", account.address());
        for folder in store.folders(account.id()).unwrap() {
            for entry in store.messages(folder.id()).unwrap() {
                let envelope = entry.message().summary().envelope();
                for address in envelope.from().iter().chain(envelope.to()) {
                    assert!(on_example(address.email().as_str()), "{}", address.email());
                    seen += 1;
                }
            }
        }
    }
    // Every message and the two invitations, one sender and one recipient each.
    assert_eq!(seen, (MESSAGES.len() + 2) * 2);
}

#[test]
fn the_inbox_holds_the_frames_thirteen_rows_seven_to_read() {
    let mailbox = prepared();
    let rows = mailbox.messages();
    assert_eq!(rows.len(), 13, "{:?}", rows.iter().map(|row| &row.subject).collect::<Vec<_>>());
    assert_eq!(rows.iter().filter(|row| !row.seen).count(), 7);
    assert_eq!(rows.first().map(|row| (row.sender.as_str(), row.subject.as_str())), Some(("Teodor Vasquez", "Draft logo for the bakery")));
    assert_eq!(rows.last().map(|row| row.subject.as_str()), Some("The Quiet Web"));
}

#[test]
fn three_new_senders_wait_in_the_screener() {
    let mailbox = prepared();
    let names: Vec<&str> = mailbox.waiting().iter().map(|waiting| waiting.name.as_str()).collect();
    assert_eq!(names, ["Lucian Albrecht", "Deals Daily", "Nora Keane"]);
    assert_eq!(showcase(&mailbox).screener.len(), 3);
}

#[test]
fn four_snoozed_three_pinned_two_parked_and_three_done() {
    let mut mailbox = prepared();
    // "Later" counts its piles when it is read.
    mailbox.open(Place::Later(Pile::Snoozed));
    assert_eq!([mailbox.pile_count(Pile::Snoozed), mailbox.pile_count(Pile::Pins), mailbox.pile_count(Pile::Parked)], [4, 3, 2]);
    // Two come back today, two this week.
    assert_eq!(mailbox.later_heads().iter().map(|head| head.messages.len()).collect::<Vec<_>>(), [2, 2]);
    mailbox.open(Place::View(inboxcraft_model::Role::Inbox));
    mailbox.set_show_done(true);
    assert_eq!(mailbox.messages().iter().filter(|row| row.done).count(), 3);
}

#[test]
fn one_message_waits_in_the_outbox_for_tomorrow_morning_and_no_toast_shows() {
    let mut mailbox = prepared();
    mailbox.open(Place::Outbox);
    let outbox = mailbox.outbox();
    assert_eq!(outbox.len(), 1);
    assert_eq!(outbox.first().map(|row| row.subject.as_str()), Some("Thank-you cards: the proof"));
    assert!(mailbox.undoable().is_none());
}

#[test]
fn the_shelving_message_bounced_and_the_kiln_rota_folder_is_empty() {
    let store = mailbox(now()).unwrap();
    let bounced = (1..=u64::try_from(MESSAGES.len()).unwrap())
        .filter(|id| store.message_state(inboxcraft_model::MessageId::new(*id)).unwrap().bounced.is_some())
        .collect::<Vec<_>>();
    let index = MESSAGES.iter().position(|sample| sample.subject == BOUNCED).unwrap();
    assert_eq!(bounced, [u64::try_from(index).unwrap() + 1]);
    assert!(store.messages(inboxcraft_model::FolderId::new(EMPTY_FOLDER)).unwrap().is_empty());
}

#[test]
fn the_week_holds_the_design_review_at_15_00_in_room_3() {
    let store = mailbox(now()).unwrap();
    let events = store.calendar_events(inboxcraft_model::CalendarId::new(2)).unwrap();
    let review = events.iter().find(|event| event.ical.contains("SUMMARY:Design review")).unwrap();
    assert!(review.ical.contains("DTSTART:20261116T150000Z"), "{}", review.ical);
    assert!(review.ical.contains("LOCATION:Room 3"));
}

#[test]
fn the_present_sample_is_not_touched() {
    // The frames' set is a sample of its own: the present one keeps its people and its time.
    let present = crate::sample::mailbox(Timestamp::from_unix(crate::shot::SHOT_TIME, 0).unwrap()).unwrap();
    let accounts = present.accounts().unwrap();
    assert_eq!(
        accounts.iter().map(|account| account.address().as_str().to_owned()).collect::<Vec<_>>(),
        ["mira@example.org", "holloway.studio@example.org"]
    );
}
