// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use inboxcraft_model::{ModSeq, Uid, UidValidity};

use super::{MAX_FLAG_LEN, MAX_FLAGS, MailboxStatus, flag_list, flag_words};
use crate::imap::value::{Status, StatusLine, Value};

fn note(status: &mut MailboxStatus, text: &str) {
    status.note_code(&StatusLine::new(Status::Ok, text));
}

#[test]
fn response_codes_fill_in_the_status() {
    let mut status = MailboxStatus::default();
    note(&mut status, "[UIDVALIDITY 3857529045] UIDs valid");
    note(&mut status, "[UIDNEXT 4392] Predicted next UID");
    note(&mut status, "[UNSEEN 12] first unseen");
    note(&mut status, "[HIGHESTMODSEQ 715194045007]");
    note(&mut status, r"[PERMANENTFLAGS (\Deleted \Seen \*)] Limited");
    note(&mut status, "[ALERT] not for the status");
    assert_eq!(
        status,
        MailboxStatus {
            uid_validity: UidValidity::new(3_857_529_045),
            uid_next: Uid::new(4392),
            unseen: Some(12),
            highest_modseq: ModSeq::new(715_194_045_007),
            permanent_flags: vec!["\\Deleted".into(), "\\Seen".into(), "\\*".into()],
            ..MailboxStatus::default()
        }
    );
    // Zero, junk and numbers out of range are no value.
    note(&mut status, "[UIDVALIDITY 0]");
    note(&mut status, "[UIDNEXT x]");
    note(&mut status, "[UNSEEN 99999999999]");
    note(&mut status, "[HIGHESTMODSEQ -1]");
    assert_eq!((status.uid_validity, status.uid_next, status.unseen, status.highest_modseq), (None, None, None, None));
}

#[test]
fn flags_are_atoms_capped_in_number_and_length() {
    let atoms = |words: &[&str]| words.iter().map(|w| Value::Atom((*w).into())).collect::<Vec<_>>();
    assert_eq!(flag_list(atoms(&["\\Seen", "$Forwarded", "\\", "a(b", "a]b", "\\*"])), ["\\Seen", "$Forwarded", "\\*"]);
    assert_eq!(flag_list(vec![Value::Str(b"\\Seen".to_vec()), Value::List(vec![])]), Vec::<String>::new());
    let long = format!("\\{}", "x".repeat(MAX_FLAG_LEN));
    let fits = "x".repeat(MAX_FLAG_LEN);
    assert_eq!(flag_list(atoms(&[&long, &fits])), [fits]);
    let many: Vec<String> = (0..MAX_FLAGS + 5).map(|n| format!("k{n}")).collect();
    assert_eq!(flag_list(many.iter().map(|w| Value::Atom(w.clone())).collect()).len(), MAX_FLAGS);
    assert_eq!(flag_words(&many.join(" ")).len(), MAX_FLAGS);
    assert_eq!(flag_words(" \\Seen  (x \u{7} "), ["\\Seen"]);
}
