// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use inboxcraft_model::{ModSeq, Uid, UidValidity};

use super::{MAX_NOTIFY_MAILBOXES, MailboxCounts, Notify, NotifyMailboxes};
use crate::ProtoError;
use crate::imap::list::MAX_MAILBOX_NAME;
use crate::imap::value::{Piece, values};

fn sent(notify: &Notify) -> Result<String, ProtoError> {
    let (first, rest) = notify.command(false)?.finish("A1");
    let mut bytes = first;
    rest.into_iter().for_each(|chunk| bytes.extend(chunk));
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn counts(line: &str) -> Option<MailboxCounts> {
    MailboxCounts::from_values(values(vec![Piece::Text(line.as_bytes().to_vec())]).unwrap_or_default())
}

#[test]
fn a_name_quotes_cannot_carry_goes_as_a_literal() {
    let notify = Notify { mailboxes: Some(NotifyMailboxes::Mailboxes(vec!["Árvíz".into()])), ..Notify::default() };
    assert_eq!(sent(&notify).as_deref(), Ok("A1 NOTIFY SET (MAILBOXES {7}\r\nÁrvíz (MessageNew MessageExpunge))\r\n"));
}

#[test]
fn names_that_cannot_be_sent_are_refused() {
    let too_many = vec!["a".to_owned(); MAX_NOTIFY_MAILBOXES + 1];
    let long = "x".repeat(MAX_MAILBOX_NAME + 1);
    for names in [vec![], too_many, vec!["ok".into(), String::new()], vec!["a\r\nb".into()], vec![long]] {
        for mailboxes in [NotifyMailboxes::Subtree(names.clone()), NotifyMailboxes::Mailboxes(names.clone())] {
            let notify = Notify { selected: true, mailboxes: Some(mailboxes), ..Notify::default() };
            assert!(matches!(sent(&notify), Err(ProtoError::BadArgument(_))), "{notify:?}");
        }
    }
    let most = vec!["a".to_owned(); MAX_NOTIFY_MAILBOXES];
    assert!(sent(&Notify { mailboxes: Some(NotifyMailboxes::Subtree(most)), ..Notify::default() }).is_ok());
}

#[test]
fn status_items_in_any_order() {
    assert_eq!(
        counts("STATUS blurdybloop (UIDNEXT 44292 MESSAGES 231 UNSEEN 3 UIDVALIDITY 7 HIGHESTMODSEQ 90060115205545359)"),
        Some(MailboxCounts {
            raw: "blurdybloop".into(),
            name: "blurdybloop".into(),
            messages: Some(231),
            uid_next: Uid::new(44292),
            uid_validity: UidValidity::new(7),
            unseen: Some(3),
            highest_modseq: ModSeq::new(90_060_115_205_545_359),
        })
    );
    assert_eq!(counts("status inbox ()").map(|c| c.name), Some("INBOX".into()));
    // Values that cannot be read are left out, and so is an item with no value after it.
    let empty = |raw: &str| Some(MailboxCounts { raw: raw.into(), name: raw.into(), ..MailboxCounts::default() });
    assert_eq!(counts("STATUS a (MESSAGES x UIDNEXT 0 UIDVALIDITY -1 UNSEEN () HIGHESTMODSEQ 0 (X) 1 MESSAGES)"), empty("a"));
    for bad in ["", "STATUS", "STATUS a", "STATUS a MESSAGES", "STATUS \"\" ()", "STATUS (a) ()", "LIST a ()"] {
        assert_eq!(counts(bad), None, "{bad:?}");
    }
}
