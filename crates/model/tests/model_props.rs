// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Property tests of the model: no text from outside makes a check panic or lets CR, LF or NUL through, UID sets
//! hold exactly the UIDs they were made of, and threads hold every message once.

use inboxcraft_model::{
    AccountId, Address, EmailAddress, Envelope, Flag, Flags, FolderId, FolderListing, FolderPath, Host, Keyword, Message, MessageId, MessageRef,
    Summary, Uid, UidSet, detect_role, group_threads, labels,
};
use proptest::prelude::*;

/// Text from a hostile header: ASCII, the characters addresses and flags are made of, white space, controls, bidi
/// marks and any character.
fn hostile_text() -> impl Strategy<Value = String> {
    proptest::collection::vec(
        prop_oneof![
            4 => proptest::char::range('a', 'z'),
            2 => prop::sample::select(vec!['@', '.', '<', '>', '"', '\\', '[', ']', '/', '$', '-', ' ', '\t']),
            1 => prop::sample::select(vec!['\r', '\n', '\0', '\u{7f}', '\u{202e}', '\u{200b}', 'é', 'ő', '日']),
            1 => any::<char>(),
        ],
        0..80,
    )
    .prop_map(|chars| chars.into_iter().collect())
}

fn clean(text: &str) -> bool {
    !text.contains(['\r', '\n', '\0'])
}

proptest! {
    #[test]
    fn no_text_panics_a_check_or_gets_a_line_break_through(text in hostile_text(), name in hostile_text()) {
        if let Ok(email) = EmailAddress::new(&text) {
            prop_assert!(clean(email.as_str()));
            let address = Address::new(Some(&name), email);
            prop_assert!(address.name().is_none_or(clean));
            for label in labels(&[address.clone(), address.with_name(Some(&text))]) {
                prop_assert!(clean(&label));
            }
        }
        if let Ok(flag) = Flag::parse(&text) {
            prop_assert!(clean(&flag.to_string()));
        }
        if let Ok(keyword) = Keyword::new(&text) {
            prop_assert!(keyword.as_str().bytes().all(|b| b.is_ascii_graphic()));
        }
        if let Ok(host) = Host::new(&text) {
            prop_assert!(clean(host.as_str()));
        }
        if let Some(reference) = MessageRef::new(&text) {
            prop_assert!(clean(reference.as_str()));
        }
        prop_assert!(clean(Envelope::new(&text).subject()));
        if let Ok(path) = FolderPath::new(&text, name.chars().next()) {
            prop_assert!(path.components().count() <= text.len() + 1);
            let _ = detect_role(&FolderListing::new(path, Vec::new(), true));
        }
    }

    #[test]
    fn a_uid_set_holds_exactly_its_uids(ranges in proptest::collection::vec((1_u32..200, 1_u32..200), 0..20), probe in 1_u32..210) {
        let set: UidSet = ranges.iter().filter_map(|&(a, b)| Some((Uid::new(a)?, Uid::new(b)?))).collect();
        let naive = ranges.iter().any(|&(a, b)| (a.min(b)..=a.max(b)).contains(&probe));
        prop_assert_eq!(set.contains(Uid::new(probe).unwrap()), naive);
        let count = (1..210).filter(|n| ranges.iter().any(|&(a, b)| (a.min(b)..=a.max(b)).contains(n))).count();
        prop_assert_eq!(set.len(), count as u64);
        for pair in set.ranges().windows(2) {
            prop_assert!(pair[0].1.get() + 1 < pair[1].0.get());
        }
    }

    #[test]
    fn every_message_is_in_one_thread(links in proptest::collection::vec((0_u8..12, proptest::collection::vec(0_u8..12, 0..4)), 0..30)) {
        let messages: Vec<Message> = links
            .iter()
            .enumerate()
            .map(|(n, (own, refs))| {
                let envelope = Envelope::new("x")
                    .with_message_id(MessageRef::new(&format!("m{own}@example.org")))
                    .with_references(refs.iter().filter_map(|r| MessageRef::new(&format!("m{r}@example.org"))).collect());
                let summary = Summary::new(Uid::new(1).unwrap(), envelope).with_flags(Flags::default());
                Message::new(MessageId::new(n as u64), AccountId::new(1), FolderId::new(1), summary)
            })
            .collect();
        let threads = group_threads(&messages);
        let mut seen: Vec<u64> = threads.iter().flatten().map(|id| id.get()).collect();
        seen.sort_unstable();
        prop_assert_eq!(seen, (0..messages.len() as u64).collect::<Vec<_>>());
        prop_assert!(threads.iter().all(|thread| !thread.is_empty()));
    }
}
