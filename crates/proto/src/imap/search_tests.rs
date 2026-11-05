// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use inboxcraft_model::{Uid, UidSet};

use super::{MAX_SEARCH_DEPTH, MAX_SEARCH_KEYS, MAX_SET_RANGES, SearchDate, SearchKey, command, set_text};
use crate::ProtoError;

fn sent(key: &SearchKey) -> Result<(String, Vec<String>), ProtoError> {
    let (first, rest) = command(key)?.finish("A1");
    Ok((String::from_utf8_lossy(&first).into_owned(), rest.iter().map(|chunk| String::from_utf8_lossy(chunk).into_owned()).collect()))
}

fn line(key: &SearchKey) -> String {
    sent(key).map(|(first, _)| first).unwrap_or_default()
}

fn uids(ranges: &[(u32, u32)]) -> UidSet {
    ranges.iter().filter_map(|(a, b)| Some((Uid::new(*a)?, Uid::new(*b)?))).collect()
}

#[test]
fn days() {
    assert!(SearchDate::new(2024, 2, 29).is_some());
    for (y, m, d) in [(2023, 2, 29), (0, 1, 1), (10_000, 1, 1), (2024, 0, 1), (2024, 13, 1), (2024, 1, 0), (2024, 4, 31)] {
        assert_eq!(SearchDate::new(y, m, d), None, "{y}-{m}-{d}");
    }
}

#[test]
fn every_key_as_sent() {
    let day = SearchDate::new(1994, 2, 1).unwrap();
    for (key, text) in [
        (SearchKey::All, "ALL"),
        (SearchKey::Seen, "SEEN"),
        (SearchKey::Unseen, "UNSEEN"),
        (SearchKey::Flagged, "FLAGGED"),
        (SearchKey::Unflagged, "UNFLAGGED"),
        (SearchKey::Answered, "ANSWERED"),
        (SearchKey::Unanswered, "UNANSWERED"),
        (SearchKey::Deleted, "DELETED"),
        (SearchKey::Undeleted, "UNDELETED"),
        (SearchKey::Draft, "DRAFT"),
        (SearchKey::Since(day), "SINCE 1-Feb-1994"),
        (SearchKey::Before(SearchDate::new(2026, 12, 31).unwrap()), "BEFORE 31-Dec-2026"),
        (SearchKey::On(day), "ON 1-Feb-1994"),
        (SearchKey::Uid(uids(&[(4, 4), (7, 9)])), "UID 4,7:9"),
        (SearchKey::Larger(100), "LARGER 100"),
        (SearchKey::Smaller(5), "SMALLER 5"),
        (SearchKey::From("ada@example.org".into()), "FROM \"ada@example.org\""),
        (SearchKey::To("bob".into()), "TO \"bob\""),
        (SearchKey::Subject("say \"hi\"".into()), "SUBJECT \"say \\\"hi\\\"\""),
        (SearchKey::Body("x".into()), "BODY \"x\""),
        (SearchKey::Text("y".into()), "TEXT \"y\""),
        (SearchKey::Header("List-Id".into(), "dev".into()), "HEADER List-Id \"dev\""),
        (SearchKey::Not(Box::new(SearchKey::Seen)), "NOT SEEN"),
        (SearchKey::Or(Box::new(SearchKey::Seen), Box::new(SearchKey::Flagged)), "OR SEEN FLAGGED"),
        (SearchKey::And(vec![SearchKey::Unseen, SearchKey::Since(day)]), "UNSEEN SINCE 1-Feb-1994"),
        (SearchKey::Or(Box::new(SearchKey::And(vec![SearchKey::Seen, SearchKey::Draft])), Box::new(SearchKey::All)), "OR (SEEN DRAFT) ALL"),
        (SearchKey::Not(Box::new(SearchKey::And(vec![SearchKey::Seen]))), "NOT (SEEN)"),
    ] {
        assert_eq!(line(&key), format!("A1 UID SEARCH {text}\r\n"), "{key:?}");
    }
}

#[test]
fn a_text_that_is_not_ascii_goes_as_a_utf8_literal() {
    let key = SearchKey::And(vec![SearchKey::Subject("Árvíztűrő".into()), SearchKey::Seen]);
    assert_eq!(sent(&key), Ok(("A1 UID SEARCH CHARSET UTF-8 SUBJECT {13}\r\n".to_owned(), vec!["Árvíztűrő SEEN\r\n".to_owned()])));
}

#[test]
fn what_cannot_be_sent_is_refused() {
    for key in [
        SearchKey::Subject(String::new()),
        SearchKey::Subject("a\r\nb".into()),
        SearchKey::Subject("\0".into()),
        SearchKey::Body("x".repeat(1_001)),
        SearchKey::Header(String::new(), "x".into()),
        SearchKey::Header("To:".into(), "x".into()),
        SearchKey::Header("To Cc".into(), "x".into()),
        SearchKey::Header("x".repeat(101), "x".into()),
        SearchKey::Header("To".into(), String::new()),
        SearchKey::And(vec![]),
        SearchKey::Uid(UidSet::default()),
    ] {
        assert!(matches!(sent(&key), Err(ProtoError::BadArgument(_))), "{key:?}");
    }
    let mut deep = SearchKey::Seen;
    for _ in 0..=MAX_SEARCH_DEPTH {
        deep = SearchKey::Not(Box::new(deep));
    }
    assert!(matches!(sent(&deep), Err(ProtoError::BadArgument(_))));
    let many = SearchKey::And(vec![SearchKey::Seen; MAX_SEARCH_KEYS]);
    assert!(matches!(sent(&many), Err(ProtoError::BadArgument(_))));
    assert!(sent(&SearchKey::And(vec![SearchKey::Seen; MAX_SEARCH_KEYS - 1])).is_ok());
    assert!(sent(&SearchKey::Or(Box::new(SearchKey::Seen), Box::new(SearchKey::Subject("\n".into())))).is_err());
    assert!(sent(&SearchKey::Not(Box::new(SearchKey::Uid(UidSet::default())))).is_err());
}

#[test]
fn sets_as_sent() {
    assert_eq!(set_text(&uids(&[(1, 1)])), Ok("1".to_owned()));
    assert_eq!(set_text(&uids(&[(1, 3), (5, 5), (7, u32::MAX)])), Ok("1:3,5,7:4294967295".to_owned()));
    assert!(set_text(&UidSet::default()).is_err());
    let ranges: Vec<(u32, u32)> = (1..=MAX_SET_RANGES as u32).map(|n| (n * 2, n * 2)).collect();
    assert!(set_text(&uids(&ranges)).is_ok());
    let ranges: Vec<(u32, u32)> = (1..=MAX_SET_RANGES as u32 + 1).map(|n| (n * 2, n * 2)).collect();
    assert!(set_text(&uids(&ranges)).is_err());
}
