// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The frames' sample: what its scenes need is in it, and everyone in it is invented on `example.org`.

use inboxcraft_store::Storage;
use inboxcraft_time::{Clock, FakeClock, Offset, Timestamp};

use super::{BOUNCED, EMPTY_FOLDER, INVITATIONS, Invited, MESSAGES, Outbox, TIME, invitation_uid, mailbox, prepare, session, showcase};
use crate::mailbox::{Mailbox, Pile, Place};

fn now() -> Timestamp {
    Timestamp::from_unix(TIME, 0).unwrap()
}

/// The window's mailbox over the frames' sample at its time, put as the frames show it.
fn prepared() -> Mailbox {
    let clock = FakeClock::new(now());
    let mut mailbox = Mailbox::new(session(Box::new(clock.clone())).unwrap(), Box::new(clock.clone()));
    prepare(&mut mailbox, &clock, Outbox::Waiting).unwrap();
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
fn one_message_waits_in_the_outbox_with_no_time_of_its_own_and_no_toast_shows() {
    let mut mailbox = prepared();
    mailbox.open(Place::Outbox);
    let outbox = mailbox.outbox();
    assert_eq!(outbox.len(), 1);
    let row = outbox.first().unwrap();
    assert_eq!(row.subject, "Thank-you cards: the proof");
    // Handed to the worker, which nothing answers in a picture: no time it goes at, and none it was late.
    assert_eq!(row.state, crate::mailbox::SendState::Sending);
    assert!(mailbox.undoable().is_none());
    let facts = crate::draw::status::read(&mut mailbox, None);
    assert_eq!((facts.outbox, facts.outbox_goes), (1, None));
}

#[test]
fn the_outbox_message_is_still_a_draft_until_it_goes() {
    let mut mailbox = prepared();
    mailbox.open(Place::View(inboxcraft_model::Role::Drafts));
    let subjects: Vec<Option<&str>> = mailbox.drafts().map(|draft| draft.subject.as_deref()).collect();
    assert_eq!(subjects, [Some("Thank-you cards: the proof")]);
}

#[test]
fn no_message_waits_in_the_outbox_when_the_window_asks_for_none() {
    let clock = FakeClock::new(now());
    let mut mailbox = Mailbox::new(session(Box::new(clock.clone())).unwrap(), Box::new(clock.clone()));
    prepare(&mut mailbox, &clock, Outbox::Empty).unwrap();
    mailbox.refresh();
    assert!(mailbox.outbox().is_empty());
    assert_eq!(clock.now(), now());
}

#[test]
fn the_scheduled_page_lists_the_message_for_tomorrow_at_08_00() {
    let clock = FakeClock::new(now());
    let mut mailbox = Mailbox::new(session(Box::new(clock.clone())).unwrap(), Box::new(clock.clone()));
    prepare(&mut mailbox, &clock, Outbox::Scheduled).unwrap();
    mailbox.refresh();
    let at = mailbox.outbox().first().map(|row| row.state.clone());
    let tuesday = Timestamp::from_unix(TIME - 14 * 3_600 - 4 * 60 + 86_400 + 8 * 3_600, 0).unwrap();
    assert_eq!(at, Some(crate::mailbox::SendState::Waiting(Some(tuesday))));
}

#[test]
fn two_invitations_in_one_folder_get_two_uids() {
    let both = [Invited { folder: 10, ..INVITATIONS[0] }, Invited { folder: 10, ..INVITATIONS[1] }];
    assert_ne!(invitation_uid(&both, 0), invitation_uid(&both, 1));
    // The inbox holds the update after its own messages.
    let inbox = MESSAGES.iter().filter(|sample| sample.folder == 10).count();
    assert_eq!(invitation_uid(&both, 1), u32::try_from(inbox).unwrap() + 2);
}

#[test]
fn the_invitations_name_the_day_of_the_clock() {
    // A day later the review is on Tuesday, and so are the subjects.
    let store = mailbox(Timestamp::from_unix(TIME + 86_400, 0).unwrap()).unwrap();
    let mut subjects = Vec::new();
    for folder in [10, 12] {
        for entry in store.messages(inboxcraft_model::FolderId::new(folder)).unwrap() {
            let subject = entry.message().summary().envelope().subject().to_owned();
            if subject.contains("Design review") {
                subjects.push(subject);
            }
        }
    }
    subjects.sort();
    assert_eq!(subjects, ["Invitation: Design review, Tue 17 Nov 14:00", "Updated invitation: Design review, Tue 17 Nov 15:00"]);
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
    // The frames' set is a sample of its own: the present one keeps its people, its time and its invitations.
    let time = Timestamp::from_unix(crate::shot::SHOT_TIME, 0).unwrap();
    let mut present = crate::sample::mailbox(time).unwrap();
    let accounts = present.accounts().unwrap();
    assert_eq!(
        accounts.iter().map(|account| account.address().as_str().to_owned()).collect::<Vec<_>>(),
        ["mira@example.org", "holloway.studio@example.org"]
    );
    let batch = crate::sample::invitations(time).unwrap().into_iter().fold(inboxcraft_store::Batch::new(), inboxcraft_store::Batch::with);
    present.apply(&batch).unwrap();
    let mut invitations = Vec::new();
    for account in &accounts {
        for folder in present.folders(account.id()).unwrap() {
            for entry in present.messages(folder.id()).unwrap() {
                let Some(body) = present.body(entry.message().id()).unwrap() else { continue };
                // The calendar part is quoted-printable: its soft line breaks and its `=3D` undone.
                let source = String::from_utf8_lossy(body.source()).replace("=\r\n", "").replace("=\n", "").replace("=3D", "=");
                if source.contains("METHOD:REQUEST") {
                    invitations.push((entry.message().summary().envelope().subject().to_owned(), source));
                }
            }
        }
    }
    invitations.sort();
    let subjects: Vec<&str> = invitations.iter().map(|(subject, _)| subject.as_str()).collect();
    assert_eq!(subjects, ["Invitation: Lunch with Odile", "Updated invitation: Lunch with Odile"]);
    for (_, source) in &invitations {
        assert!(source.contains("ORGANIZER;CN=Odile Brannock:mailto:odile@example.org"), "{source}");
        assert!(source.contains("ATTENDEE;CN=Mira Holloway;PARTSTAT=NEEDS-ACTION;RSVP=TRUE:mailto:mira@example.org"), "{source}");
    }
}
