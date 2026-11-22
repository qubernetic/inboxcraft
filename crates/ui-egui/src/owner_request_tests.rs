// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The queue of owner requests, as plain state: eight wait at most, only the one in front is asked and runs out of
//! time, after two minutes from when it came to the front, and nothing leaves it but by a settlement.

use std::time::Duration;

use inboxcraft_time::Timestamp;
use serde_json::json;

use super::{MOST_NAME_CHARS, MOST_WAITING, OwnerRequests, PATIENCE, Settled, data};
use crate::component::hidden;

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_unix(1_793_869_200 + seconds, 0).unwrap()
}

#[test]
fn eight_wait_and_the_ninth_is_turned_away() {
    let mut requests = OwnerRequests::default();
    let tickets: Vec<_> = (0..8).map(|_| requests.push(None, "event.create", json!({}))).collect();
    assert!(tickets.iter().all(Option::is_some));
    assert_eq!(MOST_WAITING, 8);
    assert_eq!(requests.push(None, "event.create", json!({})), None);
    let mut seen = tickets.clone();
    seen.dedup();
    assert_eq!(seen.len(), 8, "every ticket is its own");
    requests.settle_front(Settled::Denied);
    assert!(requests.push(None, "event.create", json!({})).is_some(), "a settled request makes room");
}

#[test]
fn only_the_one_in_front_runs_out_of_time_and_after_two_minutes() {
    let mut requests = OwnerRequests::default();
    let (first, second) = (requests.push(None, "event.create", json!({})).unwrap(), requests.push(None, "event.create", json!({})).unwrap());
    requests.expire(at(0));
    requests.expire(at(119));
    assert!(requests.take_settled().is_empty());
    requests.expire(at(120));
    assert!(matches!(requests.take_settled().as_slice(), [(ticket, Settled::TimedOut)] if *ticket == first));
    requests.expire(at(239)); // the second's two minutes began at 120
    assert!(requests.take_settled().is_empty());
    requests.expire(at(240));
    assert!(matches!(requests.take_settled().as_slice(), [(ticket, Settled::TimedOut)] if *ticket == second));
    assert!(!requests.waiting());
}

#[test]
fn the_front_is_the_oldest_and_keeps_what_was_asked() {
    let mut requests = OwnerRequests::default();
    assert!(requests.front().is_none());
    let first = requests.push(Some("inboxcraft-cli"), "calendar.import", json!({ "calendar": 1 })).unwrap();
    requests.push(None, "event.create", json!({})).unwrap();
    let front = requests.front().unwrap();
    assert_eq!(front.ticket, first);
    assert_eq!(front.client.as_deref(), Some("inboxcraft-cli"));
    assert_eq!(front.command, "calendar.import");
    assert_eq!(front.params, json!({ "calendar": 1 }));
    assert_eq!(front.since, None);
}

#[test]
fn settling_answers_the_one_in_front_only() {
    let mut requests = OwnerRequests::default();
    requests.settle_front(Settled::Denied);
    assert!(requests.take_settled().is_empty(), "nothing waits, nothing is settled");
    let first = requests.push(None, "event.create", json!({})).unwrap();
    let second = requests.push(None, "event.create", json!({})).unwrap();
    requests.settle_front(Settled::Ran(Ok(json!({ "added": 1 }))));
    assert_eq!(requests.front().unwrap().ticket, second);
    let settled = requests.take_settled();
    assert!(matches!(settled.as_slice(), [(ticket, Settled::Ran(Ok(answer)))] if *ticket == first && answer["added"] == 1));
    assert!(requests.take_settled().is_empty(), "taken once");
}

#[test]
fn due_in_counts_down_the_front_s_two_minutes() {
    let mut requests = OwnerRequests::default();
    assert_eq!(requests.due_in(at(0)), None);
    requests.push(None, "event.create", json!({})).unwrap();
    assert_eq!(requests.due_in(at(0)), Some(Duration::ZERO), "not dated yet: look at once");
    requests.expire(at(0));
    assert_eq!(requests.due_in(at(0)), Some(PATIENCE));
    assert_eq!(requests.due_in(at(100)), Some(Duration::from_secs(20)));
    assert_eq!(requests.due_in(at(500)), Some(Duration::ZERO));
}

#[test]
fn a_clock_that_goes_back_does_not_date_the_front_again_and_never_ends_it_early() {
    let mut requests = OwnerRequests::default();
    let ticket = requests.push(None, "event.create", json!({})).unwrap();
    requests.expire(at(100));
    requests.expire(at(-1000));
    assert!(requests.take_settled().is_empty());
    assert_eq!(requests.due_in(at(-1000)), Some(PATIENCE), "a clock behind the date has waited nothing");
    requests.expire(at(-880));
    assert!(requests.take_settled().is_empty(), "not dated again at the clock's step back");
    requests.expire(at(219));
    assert!(requests.take_settled().is_empty());
    requests.expire(at(220));
    assert!(matches!(requests.take_settled().as_slice(), [(got, Settled::TimedOut)] if *got == ticket));
}

#[test]
fn a_withdrawn_request_leaves_the_queue_unanswered() {
    let mut requests = OwnerRequests::default();
    let first = requests.push(None, "event.create", json!({})).unwrap();
    let second = requests.push(None, "event.create", json!({})).unwrap();
    requests.expire(at(0));
    assert!(requests.withdraw(first));
    assert!(!requests.withdraw(first), "only once");
    assert_eq!(requests.front().map(|front| front.ticket), Some(second));
    assert_eq!(requests.front().and_then(|front| front.since), None, "its two minutes start when it is next looked at");
    assert!(requests.withdraw(second));
    assert!(!requests.waiting());
    assert!(requests.take_settled().is_empty());
}

#[test]
fn what_came_from_outside_is_drawn_plain_and_cut() {
    // Bidi overrides and isolates, zero-width characters, separators and the byte order mark are left out; a control
    // character is a space.
    let name = "Inbox\u{202E}Craft\u{2066}\u{200B}\u{2028}\u{FEFF}\nAllow";
    assert_eq!(data(name, MOST_NAME_CHARS), "InboxCraft Allow");
    for c in ['\u{202A}', '\u{202E}', '\u{2066}', '\u{2069}', '\u{200B}', '\u{200F}', '\u{2028}', '\u{2029}', '\u{FEFF}'] {
        assert!(hidden(c), "{c:?}");
    }
    assert!(!hidden('a') && !hidden('é'));
    let cut = data(&"x".repeat(700), MOST_NAME_CHARS);
    assert_eq!(cut.chars().count(), MOST_NAME_CHARS + 1);
    assert!(cut.ends_with('…'));
    assert_eq!(data("  \u{202E}\n ", MOST_NAME_CHARS), "");
}

#[test]
fn data_cannot_close_its_own_quote_and_has_one_space_between_words() {
    // The quotes the question puts around data stand for an apostrophe inside it.
    assert_eq!(data("cli” that you started, “x", MOST_NAME_CHARS), "cli' that you started, 'x");
    for quote in ['"', '“', '”', '„', '‟', '«', '»', '‹', '›', '‘', '’', '‚', '‛', '＂'] {
        assert_eq!(data(&format!("a{quote}b"), MOST_NAME_CHARS), "a'b", "{quote:?}");
    }
    // Every run of white space, no-break and wide spaces among it, is one space.
    assert_eq!(data("Wide \u{00A0}\u{2003}\u{3000}  \t\r\n gap", MOST_NAME_CHARS), "Wide gap");
    assert_eq!(data("\u{00A0} edges \u{2003}", MOST_NAME_CHARS), "edges");
}
