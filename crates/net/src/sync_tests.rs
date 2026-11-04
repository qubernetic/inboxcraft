// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use inboxcraft_model::{Flag, Uid};
use inboxcraft_proto::imap::{Event, FetchItem, Section};
use inboxcraft_time::Timestamp;

use super::{SyncConfig, full_body, header_summary};

fn uid(value: u32) -> Uid {
    Uid::new(value).unwrap()
}

fn item(number: u32, section: Option<(Section, &[u8])>) -> Event {
    Event::Fetched(FetchItem {
        seq: 1,
        uid: Uid::new(number),
        flags: Some(vec!["\\Seen".to_owned(), "bad flag".to_owned(), "$Label1".to_owned()]),
        internal_date: Timestamp::from_unix(100, 0),
        size: Some(42),
        preview: Some("Hello".to_owned()),
        section: section.map(|(section, bytes)| (section, bytes.to_vec())),
        ..FetchItem::default()
    })
}

#[test]
fn the_default_keeps_thirty_days() {
    let config = SyncConfig::default();
    assert_eq!(config.body_days, 30);
    let now = Timestamp::from_unix(31 * 86_400, 0).unwrap();
    assert_eq!(config.cutoff(now), Timestamp::from_unix(86_400, 0).unwrap());
    assert_eq!(config.cutoff(Timestamp::MIN), Timestamp::MIN);
}

#[test]
fn a_header_fetch_response_is_a_summary() {
    let summary = header_summary(item(7, Some((Section::Header, b"Subject: Hi\r\n\r\n"))), uid(1)).unwrap();
    assert_eq!(summary.uid(), uid(7));
    assert_eq!(summary.envelope().subject(), "Hi");
    assert_eq!(summary.preview(), Some("Hello"));
    assert_eq!(summary.size(), 42);
    assert_eq!(summary.received(), Timestamp::from_unix(100, 0));
    assert!(summary.flags().contains(&Flag::Seen));
    assert_eq!(summary.flags().iter().count(), 2);
}

#[test]
fn other_responses_are_no_summary() {
    assert!(header_summary(item(7, Some((Section::Header, b""))), uid(8)).is_none());
    assert!(header_summary(item(7, None), uid(1)).is_none());
    assert!(header_summary(item(7, Some((Section::Full, b"x"))), uid(1)).is_none());
    assert!(header_summary(Event::Exists(3), uid(1)).is_none());
    let mut no_uid = item(7, Some((Section::Header, b"")));
    if let Event::Fetched(fetched) = &mut no_uid {
        fetched.uid = None;
    }
    assert!(header_summary(no_uid, uid(1)).is_none());
}

#[test]
fn a_full_fetch_response_is_a_body() {
    assert_eq!(full_body(item(3, Some((Section::Full, b"raw")))), Some((uid(3), b"raw".to_vec())));
    assert_eq!(full_body(item(3, Some((Section::Header, b"raw")))), None);
    assert_eq!(full_body(item(3, None)), None);
    assert_eq!(full_body(Event::Expunged(1)), None);
}
