// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Home's model: built from the sample mailbox through `safe` commands, before the first account, bounded where it is
//! built, and its words.

use inboxcraft_model::Role;
use inboxcraft_time::{FakeClock, Timestamp};
use serde_json::json;

use super::{AGENDA, Buttons, Counted, DRAFTS, Greeting, Home, Kind, MOST, build, kept, sample};
use crate::mailbox::Mailbox;
use crate::sample as mail;

const NOW: i64 = 1_793_869_200;

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(NOW, 0).unwrap()))
}

fn mailbox() -> Mailbox {
    Mailbox::new(mail::session(clock()).unwrap(), clock())
}

#[test]
fn home_is_built_from_the_sample_mailbox_through_safe_commands() {
    let mut mailbox = mailbox();
    let home = build(&mut mailbox);
    assert_eq!(home.kind, Kind::Brief);
    let unread = mailbox.views().iter().find(|view| view.role == Role::Inbox).map(|view| view.unread);
    assert_eq!(home.counts[0].what, Counted::ToRead);
    assert_eq!(home.counts[0].count, unread, "to read: the inbox's unread mail");
    assert_eq!(home.counts[1].count, Some(mailbox.waiting().len() as u64), "to screen: the senders held back");
    assert!(home.counts[2].count.is_some(), "back today: read from the snoozed mail");
    assert_eq!(home.counts[3].count, Some(mailbox.outbox().len() as u64), "the outbox");
    let drafts = mailbox.run("compose.list", &json!({})).unwrap();
    let all = drafts["drafts"].as_array().unwrap().len() as u64;
    assert_eq!(home.drafts_total, all, "every draft counted");
    assert_eq!(home.drafts.len() as u64, all.min(DRAFTS as u64), "only the drafts Home shows are kept");
    assert!(!home.agenda.is_empty(), "the sample has events left today");
    assert!(home.agenda.len() <= AGENDA && home.agenda.len() as u64 <= home.agenda_total);
    assert!(home.agenda.windows(2).all(|pair| pair[0].time <= pair[1].time), "the agenda by its start");
    assert!(home.date.starts_with("Today · "), "{}", home.date);
    assert!(!home.lede.is_empty());
    let read = unread.unwrap_or(0);
    assert_eq!(home.buttons, if read > 0 { Buttons::ReadAndWrite } else { Buttons::WriteAndCalendar });
}

#[test]
fn back_today_counts_the_snoozed_mail_that_returns_today_only() {
    let mut mailbox = mailbox();
    let before = build(&mut mailbox).counts[2].count.unwrap();
    mailbox.run("mail.snooze", &json!({ "id": 2, "until": "2026-11-05T15:00:00Z" })).unwrap();
    mailbox.run("mail.snooze", &json!({ "id": 3, "until": "2026-11-06T09:00:00Z" })).unwrap();
    let home = build(&mut mailbox);
    assert_eq!(home.counts[2].what, Counted::BackToday);
    assert_eq!(home.counts[2].count, Some(before + 1), "one back today, one tomorrow");
}

#[test]
fn every_draft_is_counted_and_only_the_newest_kept() {
    let mut mailbox = mailbox();
    let before = build(&mut mailbox).drafts_total;
    let account = mailbox.accounts().first().unwrap().id;
    for _ in 0..40 {
        mailbox.run("compose.new", &json!({ "account": account })).unwrap();
    }
    let home = build(&mut mailbox);
    assert_eq!(home.drafts_total, before + 40);
    assert_eq!(home.drafts.len(), DRAFTS, "40 drafts more cost Home no more lines");
    assert!(home.drafts.iter().all(|draft| draft.subject.is_none() && draft.to == "no recipient yet"), "{:?}", home.drafts);
}

#[test]
fn home_asks_compose_list_for_the_drafts_it_shows_and_reads_the_total() {
    let mut mailbox = mailbox();
    let account = mailbox.accounts().first().unwrap().id;
    for _ in 0..5 {
        mailbox.run("compose.new", &json!({ "account": account })).unwrap();
    }
    let asked = mailbox.run("compose.list", &super::drafts_asked()).unwrap();
    assert_eq!(asked["drafts"].as_array().unwrap().len(), DRAFTS, "only what Home shows is answered");
    let total = asked["total"].as_u64().unwrap();
    assert!(total >= 5);
    assert_eq!(build(&mut mailbox).drafts_total, total, "the total, not the rows answered");
}

#[test]
fn before_the_first_account_home_is_the_first_run() {
    let store = inboxcraft_store::Memory::new(inboxcraft_store::StoreConfig::default());
    let session = inboxcraft_engine::Session::new(Box::new(store), clock()).unwrap();
    let mut mailbox = Mailbox::new(session, clock());
    let home = build(&mut mailbox);
    assert_eq!(home, Home::first_run());
    assert_eq!(home.buttons, Buttons::FirstRun);
    assert_eq!(home.headline(), ("Your mail,", Some("on your machine.")));
}

#[test]
fn an_outside_text_is_kept_plain_and_capped() {
    let hostile: String = "a\r\n\0\u{202e}b\u{2067}שלום مرحبا\u{2069} ".repeat(1_000);
    let kept = kept(&hostile);
    assert!(kept.chars().count() <= MOST + 1, "{}", kept.chars().count());
    assert!(kept.ends_with('…'));
    assert!(!kept.chars().any(|c| c.is_control() || ('\u{202a}'..='\u{202e}').contains(&c) || ('\u{2066}'..='\u{2069}').contains(&c)));
    assert!(kept.contains("שלום") && kept.contains("مرحبا"), "the words' own direction stays");
    assert_eq!(super::kept("Kiln loading"), "Kiln loading");
}

#[test]
fn the_greeting_follows_the_hour() {
    assert_eq!(Greeting::at(0), Greeting::Morning);
    assert_eq!(Greeting::at(11), Greeting::Morning);
    assert_eq!(Greeting::at(12), Greeting::Afternoon);
    assert_eq!(Greeting::at(17), Greeting::Afternoon);
    assert_eq!(Greeting::at(18), Greeting::Evening);
    assert_eq!(Greeting::at(23), Greeting::Evening);
    assert_eq!(Greeting::Afternoon.text(false), "Good afternoon.");
}

#[test]
fn the_lede_says_the_counts_and_the_next_event() {
    let next = super::AgendaLine { time: "15:00".into(), title: "Design review".into(), detail: String::new(), account: 0 };
    assert_eq!(super::lede(7, 3, 0, Some(&next), false), "7 to read, 3 waiting at the door, and Design review at 15:00.");
    assert_eq!(super::lede(1, 0, 0, None, false), "1 to read.");
    assert!(super::lede(0, 0, 0, None, true).starts_with("Nothing waits for you."));
    assert_eq!(super::length(30), "30 min");
    assert_eq!(super::length(60), "1 h");
    assert_eq!(super::length(90), "1 h 30 min");
}

#[test]
fn the_sample_states_are_the_five_frames() {
    for state in ["today", "nothing-to-do", "overflow-counts", "first-run", "everything-failing"] {
        assert!(sample(state).is_some(), "{state}");
    }
    assert!(sample("nothing").is_none());
    assert!(sample("nothing-to-do").unwrap().is_quiet());
    assert_eq!(sample("everything-failing").unwrap().counts[3].what, Counted::NotSent);
}
