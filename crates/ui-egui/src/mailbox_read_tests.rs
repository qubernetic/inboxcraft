// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A message the reader opens is marked read in the list where it stands: its row and the unread counts change as a
//! refresh would change them, without the list and the folders read again (#406).

use inboxcraft_time::{FakeClock, Timestamp};

use super::{FolderRow, Mailbox, MessageRow, ViewRow};
use crate::sample;

/// 2026-11-05T09:00:00Z.
fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

fn shown(mailbox: &Mailbox) -> (Vec<MessageRow>, Vec<FolderRow>, Vec<ViewRow>) {
    (mailbox.messages().to_vec(), mailbox.folders.clone(), mailbox.views().to_vec())
}

#[test]
fn an_opened_message_is_read_in_its_row_and_counts_as_a_refresh_shows() {
    let mut mailbox = Mailbox::new(sample::session(clock()).unwrap(), clock());
    let unread: Vec<u64> = mailbox.messages().iter().filter(|row| !row.seen && !mailbox.held(row.id)).map(|row| row.id).collect();
    assert!(unread.len() >= 2, "the sample's inbox has unread mail");
    for id in unread {
        let (rows, revision) = (mailbox.rows_serial(), mailbox.revision());
        let answer = mailbox.open_message(id).unwrap();
        assert_eq!(answer["marked_seen"], true, "{id}");
        // One change of what the list shows, not the four reads of a refresh.
        assert_eq!(mailbox.revision(), revision.wrapping_add(1), "{id}: the list was read again");
        assert_eq!(mailbox.rows_serial(), rows.wrapping_add(1), "{id}");
        let updated = shown(&mailbox);
        assert!(updated.0.iter().any(|row| row.id == id && row.seen), "{id}");
        mailbox.refresh();
        assert_eq!(updated, shown(&mailbox), "{id}: the row or a count is not what a refresh gives");
    }
}

#[test]
fn a_message_the_list_does_not_show_is_read_again_with_everything() {
    let mut mailbox = Mailbox::new(sample::session(clock()).unwrap(), clock());
    let listed: Vec<u64> = mailbox.messages().iter().map(|row| row.id).collect();
    let elsewhere = (1..=200_u64)
        .find(|id| {
            !listed.contains(id) && mailbox.run("mail.read", &serde_json::json!({ "id": id })).is_ok_and(|answer| answer["message"]["seen"] == false)
        })
        .unwrap();
    let answer = mailbox.open_message(elsewhere).unwrap();
    assert_eq!(answer["marked_seen"], true);
    let updated = shown(&mailbox);
    mailbox.refresh();
    assert_eq!(updated, shown(&mailbox));
}
