// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use inboxcraft_time::Timestamp;

use super::{Envelope, MAX_ADDRESSES, MAX_PREVIEW_CHARS, MAX_REFERENCES, MAX_SUBJECT_CHARS, Message, MessageRef, Moving, Summary};
use crate::{AccountId, Address, EmailAddress, Flag, Flags, FolderId, MessageId, ModSeq, ThreadId, Uid, UidValidity};

fn address(text: &str) -> Address {
    Address::new(None, EmailAddress::new(text).unwrap())
}

fn reference(text: &str) -> MessageRef {
    MessageRef::new(text).unwrap()
}

#[test]
fn a_message_ref_drops_its_angle_brackets() {
    assert_eq!(reference("<abc.123@example.org>").as_str(), "abc.123@example.org");
    assert_eq!(reference("  abc@example.org ").as_str(), "abc@example.org");
    assert_eq!(reference("<abc@example.org>").to_string(), "<abc@example.org>");
}

#[test]
fn broken_message_refs_are_refused() {
    let long = format!("<{}@example.org>", "a".repeat(MessageRef::MAX_LEN));
    for text in ["", "<>", "  ", "<a b@example.org>", "<a\r\n@example.org>", "<<a@example.org>>", "a>b", long.as_str()] {
        assert_eq!(MessageRef::new(text), None, "{text:?}");
    }
    assert!(serde_json::from_str::<MessageRef>("\"a b\"").is_err());
    assert_eq!(serde_json::from_str::<MessageRef>("\"a@example.org\"").unwrap(), reference("a@example.org"));
}

#[test]
fn message_refs_compare_exactly() {
    assert_ne!(reference("A@example.org"), reference("a@example.org"));
    assert_eq!(reference("<a@example.org>"), reference("a@example.org"));
}

#[test]
fn an_envelope_cleans_its_subject() {
    let envelope = Envelope::new("  Re:\r\n  the\0 plan \u{202e} ");
    assert_eq!(envelope.subject(), "Re: the plan");
    assert_eq!(Envelope::new(&"é".repeat(MAX_SUBJECT_CHARS + 1)).subject().chars().count(), MAX_SUBJECT_CHARS);
    assert_eq!(Envelope::new("\r\n").subject(), "");
    assert_eq!(Envelope::default().subject(), "");
}

#[test]
fn an_envelope_holds_the_header_fields() {
    let date = Timestamp::from_unix(1_700_000_000, 0);
    let envelope = Envelope::new("Plan")
        .with_date(date)
        .with_from(vec![address("anna@example.org")])
        .with_reply_to(vec![address("list@example.org")])
        .with_to(vec![address("bela@example.org")])
        .with_cc(vec![address("cecil@example.org")])
        .with_bcc(vec![address("dora@example.org")])
        .with_message_id(Some(reference("m3@example.org")))
        .with_in_reply_to(vec![reference("m2@example.org")])
        .with_references(vec![reference("m1@example.org"), reference("m2@example.org")]);
    assert_eq!(envelope.date(), date);
    assert_eq!(envelope.from(), [address("anna@example.org")]);
    assert_eq!(envelope.reply_to(), [address("list@example.org")]);
    assert_eq!(envelope.to(), [address("bela@example.org")]);
    assert_eq!(envelope.cc(), [address("cecil@example.org")]);
    assert_eq!(envelope.bcc(), [address("dora@example.org")]);
    assert_eq!(envelope.message_id(), Some(&reference("m3@example.org")));
    assert_eq!(envelope.in_reply_to(), [reference("m2@example.org")]);
    assert_eq!(envelope.references(), [reference("m1@example.org"), reference("m2@example.org")]);
    let json = serde_json::to_string(&envelope).unwrap();
    assert_eq!(serde_json::from_str::<Envelope>(&json).unwrap(), envelope);
}

#[test]
fn hostile_envelopes_are_capped() {
    let many: Vec<Address> = (0..MAX_ADDRESSES + 10).map(|n| address(&format!("p{n}@example.org"))).collect();
    let refs: Vec<MessageRef> = (0..MAX_REFERENCES + 10).map(|n| reference(&format!("r{n}@example.org"))).collect();
    let envelope = Envelope::new("x").with_to(many.clone()).with_in_reply_to(refs.clone()).with_references(refs.clone());
    assert_eq!(envelope.to().len(), MAX_ADDRESSES);
    assert_eq!(envelope.to().first(), many.first());
    assert_eq!(envelope.in_reply_to().len(), MAX_REFERENCES);
    // The newest references are the ones that link a thread: the last ones are kept.
    assert_eq!(envelope.references().len(), MAX_REFERENCES);
    assert_eq!(envelope.references().last(), refs.last());
    assert_eq!(envelope.references().first(), refs.get(10));
    let json = serde_json::to_string(&Envelope::new("x").with_references(Vec::new())).unwrap();
    let long = json.replace("\"references\":[]", &format!("\"references\":{}", serde_json::to_string(&refs).unwrap()));
    assert_eq!(serde_json::from_str::<Envelope>(&long).unwrap().references().len(), MAX_REFERENCES);
}

#[test]
fn a_stored_subject_and_date_are_checked() {
    let json = serde_json::to_string(&Envelope::new("x").with_date(Timestamp::from_unix(5, 7))).unwrap();
    assert!(json.contains("\"date\":[5,7]"), "{json}");
    let hostile = json.replace("\"subject\":\"x\"", "\"subject\":\"a\\r\\nBcc: b\"");
    assert_eq!(serde_json::from_str::<Envelope>(&hostile).unwrap().subject(), "a Bcc: b");
    let out_of_range = json.replace("[5,7]", "[5,1000000000]");
    assert!(serde_json::from_str::<Envelope>(&out_of_range).is_err());
    let none = json.replace("[5,7]", "null");
    assert_eq!(serde_json::from_str::<Envelope>(&none).unwrap().date(), None);
}

fn summary() -> Summary {
    let mut flags = Flags::default();
    flags.insert(Flag::Seen);
    Summary::new(Uid::new(42).unwrap(), Envelope::new("Plan"))
        .with_flags(flags)
        .with_size(1234)
        .with_received(Timestamp::from_unix(1_700_000_100, 0))
        .with_preview(Some("  Hello,\r\n\r\nthe plan is\u{200b} ready. "))
        .with_modseq(ModSeq::new(9))
}

#[test]
fn a_summary_is_what_the_server_lists_for_a_message() {
    let summary = summary();
    assert_eq!(summary.uid(), Uid::new(42).unwrap());
    assert_eq!(summary.envelope().subject(), "Plan");
    assert!(summary.flags().is_seen());
    assert_eq!(summary.size(), 1234);
    assert_eq!(summary.received(), Timestamp::from_unix(1_700_000_100, 0));
    assert_eq!(summary.preview(), Some("Hello, the plan is ready."));
    assert_eq!(summary.modseq(), ModSeq::new(9));
    let json = serde_json::to_string(&summary).unwrap();
    assert_eq!(serde_json::from_str::<Summary>(&json).unwrap(), summary);
}

#[test]
fn a_preview_is_capped_and_cleaned_on_the_way_in() {
    let long = Summary::new(Uid::new(1).unwrap(), Envelope::default()).with_preview(Some(&"a".repeat(MAX_PREVIEW_CHARS * 2)));
    assert_eq!(long.preview().map(str::len), Some(MAX_PREVIEW_CHARS));
    let json = serde_json::to_string(&long.with_preview(Some("x"))).unwrap();
    let hostile = json.replace("\"preview\":\"x\"", "\"preview\":\"a\\u0000b\"");
    assert_eq!(serde_json::from_str::<Summary>(&hostile).unwrap().preview(), Some("ab"));
}

#[test]
fn a_message_is_a_summary_with_our_ids() {
    let message = Message::new(MessageId::new(7), AccountId::new(1), FolderId::new(2), summary());
    assert_eq!(message.id(), MessageId::new(7));
    assert_eq!(message.account(), AccountId::new(1));
    assert_eq!(message.folder(), FolderId::new(2));
    assert_eq!(message.thread(), None);
    assert_eq!(message.summary(), &summary());
    let threaded = message.with_thread(Some(ThreadId::new(5)));
    assert_eq!(threaded.thread(), Some(ThreadId::new(5)));
    let json = serde_json::to_string(&threaded).unwrap();
    assert_eq!(serde_json::from_str::<Message>(&json).unwrap(), threaded);
}

#[test]
fn a_moved_message_keeps_where_the_server_has_it_until_the_answer_comes() {
    let message = Message::new(MessageId::new(7), AccountId::new(1), FolderId::new(3), summary());
    assert_eq!(message.moving(), None);
    let json = serde_json::to_string(&message).unwrap();
    assert!(!json.contains("moving"), "a settled message is stored as before: {json}");
    let place = Moving { folder: FolderId::new(2), uid_validity: UidValidity::new(7).unwrap(), uid: Uid::new(42).unwrap(), flags: Flags::default() };
    let moving = message.clone().with_moving(Some(place.clone()));
    assert_eq!(moving.moving(), Some(&place));
    let json = serde_json::to_string(&moving).unwrap();
    assert_eq!(serde_json::from_str::<Message>(&json).unwrap(), moving);
    assert_eq!(moving.with_moving(None), message);
}

#[test]
fn a_summary_takes_another_uid() {
    let other = summary().with_uid(Uid::new(43).unwrap());
    assert_eq!(other.uid(), Uid::new(43).unwrap());
    assert_eq!(other.with_uid(Uid::new(42).unwrap()), summary());
}
