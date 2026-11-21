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

/// The sample mailbox at `at` (an ISO 8601 time), in `offsets` with the list's clock at `offset`, the week rule's first
/// day `first_day` (`week.set`).
fn mailbox_at(at: &str, first_day: &str, offsets: Option<inboxcraft_time::OffsetTable>, offset: Option<i16>) -> Mailbox {
    let clock = FakeClock::new(crate::mailbox::parse_date(at).unwrap());
    let mut session = mail::session(Box::new(clock.clone())).unwrap();
    if let Some(offsets) = offsets {
        session.set_offsets(Box::new(offsets));
    }
    let mut mailbox = Mailbox::new(session, Box::new(clock));
    if let Some(minutes) = offset {
        mailbox.set_offset(inboxcraft_time::Offset::from_minutes(minutes).unwrap());
    }
    mailbox.run("week.set", &json!({ "first_day": first_day, "first_week": "first_four_day_week" })).unwrap();
    mailbox
}

/// Makes an event `summary` from `start` to `end` in the first calendar.
fn event(mailbox: &mut Mailbox, summary: &str, start: &str, end: &str) {
    let calendars = mailbox.run("calendar.list", &json!({})).unwrap();
    let calendar = calendars["calendars"][0]["id"].as_u64().unwrap();
    mailbox.run("event.create", &json!({ "calendar": calendar, "summary": summary, "start": start, "end": end })).unwrap();
}

#[test]
fn tomorrow_starts_with_its_first_event_on_every_day_of_every_week_rule() {
    // 9 to 15 November 2026 is Monday to Sunday: each day is the last of the week under one of the presets (Sunday
    // under a Monday start, Saturday under a Sunday start, Friday under a Saturday start).
    for first_day in ["monday", "sunday", "saturday"] {
        for date in 9..=15 {
            let mut mailbox = mailbox_at(&format!("2026-11-{date:02}T23:50:00Z"), first_day, None, None);
            let next = date + 1;
            event(&mut mailbox, "Early kiln", &format!("2026-11-{next:02}T05:00:00Z"), &format!("2026-11-{next:02}T05:30:00Z"));
            let home = build(&mut mailbox);
            let at = format!("{first_day}, {date} November: {:?}", home.agenda);
            assert_eq!(home.agenda_total, 0, "{at}");
            assert_eq!(home.tomorrow, Some(("05:00".to_owned(), "Early kiln".to_owned())), "{at}");
        }
    }
}

#[test]
fn an_event_across_midnight_is_today_s_where_it_starts_and_tomorrow_s_only_when_it_starts_then() {
    // Tuesday 10 November, 22:00: an event from 23:00 to 01:00 is the rest of today, two hours long.
    let mut mailbox = mailbox_at("2026-11-10T22:00:00Z", "monday", None, None);
    event(&mut mailbox, "Night firing", "2026-11-10T23:00:00Z", "2026-11-11T01:00:00Z");
    let home = build(&mut mailbox);
    let line = home.agenda.iter().find(|line| line.title == "Night firing").unwrap_or_else(|| panic!("{:?}", home.agenda));
    assert_eq!((line.time.as_str(), line.detail.as_str()), ("23:00", "2 h"));
    // From the evening before, it is tomorrow's first: it starts tomorrow.
    let mut mailbox = mailbox_at("2026-11-09T23:50:00Z", "monday", None, None);
    event(&mut mailbox, "Night firing", "2026-11-10T03:00:00Z", "2026-11-10T04:00:00Z");
    assert_eq!(build(&mut mailbox).tomorrow, Some(("03:00".to_owned(), "Night firing".to_owned())));
    // One that started yesterday and ends tomorrow is neither the rest of today nor tomorrow's first.
    let mut mailbox = mailbox_at("2026-11-10T23:50:00Z", "monday", None, None);
    event(&mut mailbox, "Long firing", "2026-11-09T23:00:00Z", "2026-11-11T07:00:00Z");
    event(&mut mailbox, "Early kiln", "2026-11-11T05:00:00Z", "2026-11-11T05:30:00Z");
    let home = build(&mut mailbox);
    assert!(home.agenda.iter().all(|line| line.title != "Long firing"), "{:?}", home.agenda);
    assert_eq!(home.tomorrow, Some(("05:00".to_owned(), "Early kiln".to_owned())));
}

#[test]
fn the_day_summer_time_starts_keeps_its_evening_and_its_tomorrow() {
    // Sunday 29 March 2026: the clocks go from +01:00 to +02:00 at 01:00 UTC; the list's clock shows +02:00.
    let change = inboxcraft_time::Timestamp::from_unix(1_774_746_000, 0).unwrap();
    let zone = || {
        inboxcraft_time::OffsetTable::new(inboxcraft_time::Offset::from_minutes(60).unwrap())
            .with_change(change, inboxcraft_time::Offset::from_minutes(120).unwrap())
    };
    let mut mailbox = mailbox_at("2026-03-29T18:00:00Z", "monday", Some(zone()), Some(120));
    event(&mut mailbox, "Late glaze", "2026-03-29T20:00:00Z", "2026-03-29T21:00:00Z");
    event(&mut mailbox, "Early kiln", "2026-03-30T03:00:00Z", "2026-03-30T03:30:00Z");
    let home = build(&mut mailbox);
    assert!(home.date.contains("29 Mar"), "{}", home.date);
    let line = home.agenda.iter().find(|line| line.title == "Late glaze").unwrap_or_else(|| panic!("{:?}", home.agenda));
    assert_eq!((line.time.as_str(), line.detail.as_str()), ("22:00", "1 h"), "on the summer clock");
    let mut mailbox = mailbox_at("2026-03-29T21:30:00Z", "monday", Some(zone()), Some(120));
    event(&mut mailbox, "Early kiln", "2026-03-30T03:00:00Z", "2026-03-30T03:30:00Z");
    let home = build(&mut mailbox);
    assert_eq!(home.agenda_total, 0, "{:?}", home.agenda);
    assert_eq!(home.tomorrow, Some(("05:00".to_owned(), "Early kiln".to_owned())));
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
    assert_eq!(super::lede(Some(7), Some(3), Some(0), Some(&next), false), "7 to read, 3 waiting at the door, and Design review at 15:00.");
    assert_eq!(super::lede(Some(1), Some(0), Some(0), None, false), "1 to read.");
    assert!(super::lede(Some(0), Some(0), Some(0), None, true).starts_with("Nothing waits for you."));
    assert_eq!(super::length(30), "30 min");
    assert_eq!(super::length(60), "1 h");
    assert_eq!(super::length(90), "1 h 30 min");
}

/// A model whose counts are `read`, `screen`, `back` and `outbox`, with nothing left today and no draft.
fn counted(read: Option<u64>, screen: Option<u64>, back: Option<u64>, outbox: Option<u64>) -> Home {
    Home {
        counts: super::counts(read, screen, back, outbox, 0),
        agenda: Vec::new(),
        agenda_total: 0,
        drafts: Vec::new(),
        drafts_total: 0,
        ..sample("today").unwrap()
    }
}

#[test]
fn only_counts_read_as_0_make_home_quiet_and_an_unread_count_is_left_out_of_the_lede() {
    let quiet = "Nothing waits for you. The inbox is read, nobody is at the door, and the day's events are over.";
    let known = [Some(0), None];
    for read in known {
        for screen in known {
            for back in known {
                for outbox in known {
                    let home = counted(read, screen, back, outbox);
                    let all = [read, screen, back, outbox].iter().all(|count| *count == Some(0));
                    assert_eq!(home.is_quiet(), all, "{read:?} {screen:?} {back:?} {outbox:?}");
                    let lede = super::lede(read, screen, back, None, home.is_quiet());
                    let at = format!("{read:?} {screen:?} {back:?} {outbox:?}: {lede}");
                    assert_eq!(lede == quiet, all, "{at}");
                    assert_eq!(lede.contains("inbox is read") || lede.contains("in the inbox"), read == Some(0), "{at}");
                    assert_eq!(lede.to_lowercase().contains("nobody is at the door"), screen == Some(0) && (read.is_none() || all), "{at}");
                }
            }
        }
    }
    assert_eq!(super::lede(Some(0), None, Some(0), None, false), "The inbox is read, and the day's events are over.");
    assert_eq!(super::lede(None, Some(0), Some(0), None, false), "Nobody is at the door, and the day's events are over.");
    assert_eq!(super::lede(None, None, None, None, false), "The day's events are over.");
    assert_eq!(super::lede(Some(0), Some(0), None, None, false), "Nothing waits for you in the inbox.");
    assert_eq!(super::lede(None, Some(4), None, None, false), "4 waiting at the door.", "an unread count is left out");
    assert_eq!(super::lede(Some(2), None, Some(1), None, false), "2 to read, and 1 back today.");
}

#[test]
fn the_sample_states_are_the_five_frames_and_the_drafts_two() {
    for state in ["today", "nothing-to-do", "overflow-counts", "first-run", "everything-failing", "drafts", "drafts-empty"] {
        assert!(sample(state).is_some(), "{state}");
    }
    assert!(sample("nothing").is_none());
    assert!(sample("nothing-to-do").unwrap().is_quiet());
    assert_eq!(sample("everything-failing").unwrap().counts[3].what, Counted::NotSent);
}

/// The headline greets by the word of the sending name the owner chose (`greeting.set`): the first by default, the
/// last, the whole name, or none, where it greets without a comma; Home kept between frames is built again when the
/// choice changes.
#[test]
fn the_headline_greets_by_the_word_the_owner_chose() {
    use crate::mailbox::GreetBy;
    let mut mailbox = mailbox();
    assert_eq!(mailbox.accounts().first().map(|account| account.name.as_str()), Some("Mira Holloway"));
    let home = build(&mut mailbox);
    assert_eq!(home.headline().1, Some("Mira"), "the first word by default");
    assert!(home.headline().0.ends_with(','), "{}", home.headline().0);
    let mut kept = super::Kept::default();
    assert_eq!(kept.current(&mut mailbox, None).name.as_deref(), Some("Mira"));
    for (by, name) in
        [(GreetBy::Last, Some("Holloway")), (GreetBy::Full, Some("Mira Holloway")), (GreetBy::None, None), (GreetBy::First, Some("Mira"))]
    {
        mailbox.set_greet_by(by);
        assert_eq!(mailbox.greet_by(), by);
        assert_eq!(build(&mut mailbox).headline().1, name, "{by:?}");
        let home = kept.current(&mut mailbox, None);
        assert_eq!(home.name.as_deref(), name, "{by:?}: the kept Home is built again");
        assert_eq!(home.headline().0.ends_with('.'), name.is_none(), "{by:?}: {}", home.headline().0);
    }
    // A choice handed back from an earlier run runs no command and changes no revision: the kept Home still follows.
    mailbox.restore_greet_by(GreetBy::Last);
    assert_eq!(kept.current(&mut mailbox, None).name.as_deref(), Some("Holloway"), "the kept Home follows a restored choice");
}

/// Tells `account` of `mailbox` what its worker would: its connection `state`, and a sync round ended at `synced`.
fn told(mailbox: &mut Mailbox, account: u64, state: inboxcraft_model::ConnectionState, synced: Option<i64>) {
    use inboxcraft_model::{AccountId, Event, EventKind};
    let account = AccountId::new(account);
    if let Some(at) = synced {
        mailbox.tell(&Event::new(account, None, EventKind::Connection(inboxcraft_model::ConnectionState::Online))).unwrap();
        mailbox.tell(&Event::new(account, None, EventKind::SyncFinished { at: Timestamp::from_unix(at, 0).unwrap() })).unwrap();
    }
    mailbox.tell(&Event::new(account, None, EventKind::Connection(state))).unwrap();
}

/// Tells `account` of `mailbox` that its sign-in was refused.
fn refused(mailbox: &mut Mailbox, account: u64) {
    use inboxcraft_model::{AccountId, ErrorKind, Event, EventKind};
    let failed = EventKind::Failed { kind: ErrorKind::LoginRefused, folder: None };
    mailbox.tell(&Event::new(AccountId::new(account), None, failed)).unwrap();
}

fn accounts(mailbox: &Mailbox) -> Vec<(u64, String)> {
    mailbox.accounts().iter().map(|account| (account.id, account.label.clone())).collect()
}

const OFFLINE: inboxcraft_model::ConnectionState = inboxcraft_model::ConnectionState::Offline;
const ONLINE: inboxcraft_model::ConnectionState = inboxcraft_model::ConnectionState::Online;

#[test]
fn offline_says_since_the_last_sync_sync_status_names() {
    let mut mailbox = mailbox();
    assert_eq!(build(&mut mailbox).failing, None, "nothing told yet: the status line says offline, the banner nothing");
    let all = accounts(&mailbox);
    // Synced at 07:40 and 08:10 today, then every account offline: since the newest sync.
    for (index, (account, _)) in all.iter().enumerate() {
        told(&mut mailbox, *account, OFFLINE, Some(NOW - 80 * 60 + 30 * 60 * index as i64));
    }
    let failing = build(&mut mailbox).failing.unwrap();
    let since = mailbox.short_date(Timestamp::from_unix(NOW - 80 * 60 + 30 * 60 * (all.len() as i64 - 1), 0).unwrap());
    assert_eq!(failing.text, format!("Offline since {since}: what you see is what this computer holds."));
    assert!(!failing.account && !failing.outbox, "nothing to open for being offline");
    // One account back online: Home is no longer offline.
    told(&mut mailbox, all[0].0, ONLINE, None);
    assert_eq!(build(&mut mailbox).failing, None);
}

#[test]
fn a_refused_sign_in_is_named_by_its_account_and_offers_the_account() {
    let mut mailbox = mailbox();
    let all = accounts(&mailbox);
    let (studio, label) = all.last().unwrap().clone();
    for (account, _) in &all {
        told(&mut mailbox, *account, ONLINE, Some(NOW - 600));
    }
    refused(&mut mailbox, studio);
    let failing = build(&mut mailbox).failing.unwrap();
    assert_eq!(failing.text, format!("{label} refused the sign-in."));
    assert!(failing.account && !failing.outbox);
    // Every account refused: counted, not listed.
    for (account, _) in &all {
        refused(&mut mailbox, *account);
    }
    let failing = build(&mut mailbox).failing.unwrap();
    if all.len() > 1 {
        assert_eq!(failing.text, format!("{} accounts refused the sign-in.", all.len()));
    }
    // Their connections down as well: an account stopped for the owner is refused, not offline.
    for (account, _) in &all {
        told(&mut mailbox, *account, OFFLINE, None);
    }
    assert!(!build(&mut mailbox).failing.unwrap().text.contains("Offline"), "an account refused is not offline");
}

#[test]
fn everything_failing_says_offline_the_refused_sign_in_and_the_send_in_one_banner() {
    let mut mailbox = mailbox();
    let all = accounts(&mailbox);
    assert!(all.len() >= 2, "the sample has two accounts");
    let (studio, label) = all.last().unwrap().clone();
    for (account, _) in &all {
        told(&mut mailbox, *account, OFFLINE, Some(NOW - 50 * 60));
    }
    refused(&mut mailbox, studio);
    let since = mailbox.short_date(Timestamp::from_unix(NOW - 50 * 60, 0).unwrap());
    let failing = build(&mut mailbox).failing.unwrap();
    assert_eq!(failing.text, format!("Offline since {since}: what you see is what this computer holds. {label} refused the sign-in."));
    assert!(failing.account);
    // The words are the parts in their order, and a refused send joins them.
    let parts = super::failing_text(Some(&since), std::slice::from_ref(&label), 1, 1, Some("Odette Marchand"));
    assert_eq!(
        parts,
        format!(
            "Offline since {since}: what you see is what this computer holds. {label} refused the sign-in, and one message to Odette Marchand was not sent. Nothing is lost."
        )
    );
    assert_eq!(super::failing_text(None, &[], 0, 3, None), "3 messages were not sent. Nothing is lost.");
    assert_eq!(super::failing_text(None, &[], 0, 1, None), "One message was not sent. Nothing is lost.");
    assert_eq!(super::failing_text(None, &[], 0, 1, Some("Odette")), "One message to Odette was not sent. Nothing is lost.");
}

#[test]
fn an_account_s_name_in_the_banner_is_plain_and_capped() {
    let hostile = format!("\u{202e}Studio\r\n\u{0}{}", "x".repeat(10_000));
    let text = super::failing_text(None, &[super::kept(&hostile)], 1, 0, None);
    assert!(text.chars().count() <= MOST + 40, "{}", text.len());
    assert!(!text.contains('\u{202e}') && !text.contains('\n') && !text.contains('\u{0}'));
}


#[test]
fn of_every_error_kind_only_a_refused_sign_in_is_read_as_refused() {
    use inboxcraft_model::ErrorKind;
    for kind in ErrorKind::ALL {
        let mut mailbox = mailbox();
        let all = accounts(&mailbox);
        for (account, _) in &all {
            told(&mut mailbox, *account, ONLINE, Some(NOW - 600));
        }
        let (studio, _) = all.last().unwrap().clone();
        let failed = inboxcraft_model::EventKind::Failed { kind, folder: None };
        mailbox.tell(&inboxcraft_model::Event::new(inboxcraft_model::AccountId::new(studio), None, failed)).unwrap();
        // The name `account.list` gives the kind is read back as that kind.
        let listed = mailbox.run("account.list", &json!({})).unwrap();
        let row = listed["accounts"].as_array().unwrap().iter().find(|row| row["id"] == json!(studio)).unwrap().clone();
        let name = row["failure"]["kind"].as_str().unwrap();
        assert_eq!(super::error_kind(name), Some(kind), "{name}");
        let refused = build(&mut mailbox).failing.is_some_and(|failing| failing.account);
        assert_eq!(refused, kind == ErrorKind::LoginRefused, "{kind:?}");
    }
    assert_eq!(super::error_kind("LoginRefused"), None, "only the names the answers give");
    assert_eq!(super::error_kind(""), None);
}