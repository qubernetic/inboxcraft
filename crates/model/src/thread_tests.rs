// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use inboxcraft_time::Timestamp;

use super::{Thread, group_threads};
use crate::{AccountId, Envelope, FolderId, Message, MessageId, MessageRef, Summary, ThreadId, Uid};

struct Mail {
    id: u64,
    account: u64,
    date: Option<i64>,
    message_id: Option<&'static str>,
    in_reply_to: &'static [&'static str],
    references: &'static [&'static str],
}

const fn mail(id: u64, date: i64, message_id: &'static str) -> Mail {
    Mail { id, account: 1, date: Some(date), message_id: Some(message_id), in_reply_to: &[], references: &[] }
}

fn refs(texts: &[&str]) -> Vec<MessageRef> {
    texts.iter().map(|text| MessageRef::new(text).unwrap()).collect()
}

fn build(mail: &Mail) -> Message {
    let envelope = Envelope::new("x")
        .with_date(mail.date.and_then(|date| Timestamp::from_unix(date, 0)))
        .with_message_id(mail.message_id.and_then(MessageRef::new))
        .with_in_reply_to(refs(mail.in_reply_to))
        .with_references(refs(mail.references));
    let summary = Summary::new(Uid::new(1).unwrap(), envelope);
    Message::new(MessageId::new(mail.id), AccountId::new(mail.account), FolderId::new(1), summary)
}

fn groups(mails: &[Mail]) -> Vec<Vec<u64>> {
    let messages: Vec<Message> = mails.iter().map(build).collect();
    group_threads(&messages).iter().map(|group| group.iter().map(|id| id.get()).collect()).collect()
}

#[test]
fn replies_join_the_message_they_answer_oldest_first() {
    let mails = [
        Mail { in_reply_to: &["a@example.org"], ..mail(2, 200, "b@example.org") },
        mail(1, 100, "a@example.org"),
        Mail { references: &["a@example.org", "b@example.org"], ..mail(3, 300, "c@example.org") },
        mail(4, 150, "other@example.org"),
    ];
    assert_eq!(groups(&mails), [vec![1, 2, 3], vec![4]]);
}

#[test]
fn a_reply_to_a_message_we_do_not_have_still_joins_its_siblings() {
    let mails = [
        Mail { in_reply_to: &["missing@example.org"], ..mail(1, 100, "x@example.org") },
        Mail { references: &["missing@example.org"], ..mail(2, 200, "y@example.org") },
    ];
    assert_eq!(groups(&mails), [vec![1, 2]]);
}

#[test]
fn the_sent_copy_and_the_received_copy_are_one_thread() {
    let mails = [
        Mail { account: 1, ..mail(1, 100, "same@example.org") },
        Mail { account: 2, ..mail(2, 100, "same@example.org") },
        Mail { account: 2, in_reply_to: &["same@example.org"], ..mail(3, 200, "reply@example.org") },
    ];
    assert_eq!(groups(&mails), [vec![1, 2, 3]]);
}

#[test]
fn messages_without_links_stand_alone_and_threads_list_newest_first() {
    let mails = [Mail { message_id: None, ..mail(1, 100, "") }, Mail { message_id: None, ..mail(2, 300, "") }, mail(3, 200, "c@example.org")];
    assert_eq!(groups(&mails), [vec![2], vec![3], vec![1]]);
}

#[test]
fn undated_messages_sort_first_and_ties_go_by_id() {
    let mails = [
        Mail { references: &["t@example.org"], ..mail(5, 100, "e@example.org") },
        Mail { references: &["t@example.org"], ..mail(3, 100, "d@example.org") },
        Mail { date: None, references: &["t@example.org"], ..mail(9, 0, "f@example.org") },
    ];
    assert_eq!(groups(&mails), [vec![9, 3, 5]]);
}

#[test]
fn link_cycles_do_not_hang() {
    let mails = [
        Mail { in_reply_to: &["b@example.org"], ..mail(1, 100, "a@example.org") },
        Mail { in_reply_to: &["a@example.org"], ..mail(2, 200, "b@example.org") },
        Mail { references: &["c@example.org"], ..mail(3, 300, "c@example.org") },
    ];
    assert_eq!(groups(&mails), [vec![3], vec![1, 2]]);
}

#[test]
fn a_long_chain_links_without_recursion() {
    let ids: Vec<String> = (0..20_000).map(|n| format!("m{n}@example.org")).collect();
    let messages: Vec<Message> = (0..20_000_usize)
        .map(|n| {
            let mut envelope = Envelope::new("x").with_message_id(MessageRef::new(&ids[n]));
            if n > 0 {
                envelope = envelope.with_in_reply_to(vec![MessageRef::new(&ids[n - 1]).unwrap()]);
            }
            let summary = Summary::new(Uid::new(1).unwrap(), envelope);
            Message::new(MessageId::new(n as u64), AccountId::new(1), FolderId::new(1), summary)
        })
        .collect();
    let threads = group_threads(&messages);
    assert_eq!(threads.len(), 1);
    assert_eq!(threads[0].len(), 20_000);
}

#[test]
fn a_thread_holds_its_messages() {
    let thread = Thread::new(ThreadId::new(1), vec![MessageId::new(1), MessageId::new(2)]);
    assert_eq!(thread.id(), ThreadId::new(1));
    assert_eq!(thread.messages(), [MessageId::new(1), MessageId::new(2)]);
    let json = serde_json::to_string(&thread).unwrap();
    assert_eq!(serde_json::from_str::<Thread>(&json).unwrap(), thread);
}

#[test]
fn no_messages_no_threads() {
    assert!(group_threads(&[]).is_empty());
}
