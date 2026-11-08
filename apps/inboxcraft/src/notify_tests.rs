// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use inboxcraft_engine::notify::{About, Notice};
use inboxcraft_model::{AccountId, DraftId, MessageId};

use super::{Button, Click, Shown, clicked, escape, shown};

fn about(who: &str, subject: &str) -> About {
    About { account: AccountId::new(1), message: MessageId::new(7), who: who.to_owned(), subject: subject.to_owned() }
}

#[test]
fn new_mail_shows_who_it_is_from_and_its_subject_with_done_and_mark_read() {
    let notice = Notice::Mail(about("Anna Example", "Lunch on Friday"));
    assert_eq!(
        shown(&notice, None),
        Shown {
            key: "mail-7".to_owned(),
            title: "Anna Example".to_owned(),
            body: "Lunch on Friday".to_owned(),
            buttons: vec![Button::Done, Button::Read],
            mail: true
        }
    );
    let bare = shown(&Notice::Mail(about("", "")), None);
    assert_eq!((bare.title.as_str(), bare.body.as_str()), ("New mail", "(no subject)"));
}

#[test]
fn a_count_names_its_account() {
    let notice = Notice::Mails { account: AccountId::new(2), count: 5 };
    let shown = shown(&notice, Some("anna@example.org"));
    assert_eq!((shown.key.as_str(), shown.title.as_str(), shown.body.as_str()), ("mails-2", "5 new messages", "anna@example.org"));
    assert!(shown.buttons.is_empty() && shown.mail);
    assert_eq!(super::shown(&notice, None).body, "");
}

#[test]
fn a_returned_snooze_and_a_reminder_say_what_happened() {
    let back = shown(&Notice::Returned { about: about("Anna", "Lunch"), missed: false }, None);
    assert_eq!((back.key.as_str(), back.title.as_str(), back.body.as_str()), ("returned-7", "Back from snooze", "Anna: Lunch"));
    assert_eq!((back.buttons.as_slice(), back.mail), ([Button::Done].as_slice(), false));
    let late = shown(&Notice::Returned { about: about("", ""), missed: true }, None);
    assert_eq!((late.title.as_str(), late.body.as_str()), ("Back from snooze while InboxCraft was off", "(no subject)"));
    let reminder = shown(&Notice::Reminder { about: about("Bob", "Budget"), missed: false }, None);
    assert_eq!((reminder.key.as_str(), reminder.title.as_str(), reminder.body.as_str()), ("reminder-7", "No reply yet", "Bob: Budget"));
    assert!(reminder.buttons.is_empty());
    assert_eq!(shown(&Notice::Reminder { about: about("Bob", "Budget"), missed: true }, None).title, "No reply yet, reminded late");
}

#[test]
fn a_send_that_went_late_says_so_and_a_click_on_it_does_nothing() {
    let late = shown(&Notice::SentLate { account: AccountId::new(1), draft: DraftId::new(4), subject: "Budget".to_owned() }, None);
    assert_eq!(
        (late.key.as_str(), late.title.as_str(), late.body.as_str()),
        ("sent-4", "Sent late: InboxCraft was not running at its time", "Budget")
    );
    assert!(late.buttons.is_empty() && !late.mail);
    assert_eq!(shown(&Notice::SentLate { account: AccountId::new(1), draft: DraftId::new(4), subject: String::new() }, None).body, "(no subject)");
    assert_eq!(clicked("sent-4", "default"), None);
}

#[test]
fn a_click_is_read_from_the_key_and_the_action() {
    assert_eq!(clicked("mail-7", "open"), Some(Click::Open(7)));
    assert_eq!(clicked("mail-7", "default"), Some(Click::Open(7)));
    assert_eq!(clicked("mail-7", "done"), Some(Click::Done(7)));
    assert_eq!(clicked("mail-7", "read"), Some(Click::Read(7)));
    assert_eq!(clicked("returned-8", "done"), Some(Click::Done(8)));
    assert_eq!(clicked("reminder-9", "open"), Some(Click::Open(9)));
    assert_eq!(clicked("mails-1", "default"), Some(Click::Show));
    for (key, action) in [
        ("reminder-9", "done"),
        ("returned-8", "read"),
        ("mails-1", "done"),
        ("mail-0", "open"),
        ("mail--1", "open"),
        ("mail-18446744073709551616", "open"),
        ("mail-7x", "open"),
        ("mail", "open"),
        ("calendar-7", "open"),
        ("", ""),
        ("mail-7", "Open"),
        ("mail-7", "rm -rf"),
        ("mail-+7", "open"),
        ("mail-007", "open"),
        ("mails-+1", "default"),
        ("mail- 7", "open"),
    ] {
        assert_eq!(clicked(key, action), None, "{key:?} {action:?}");
    }
}

proptest::proptest! {
    #[test]
    fn a_click_is_taken_only_on_a_key_written_the_way_it_is_made(key in "(mails?|returned|reminder|x)?-?[+0 ]{0,2}[0-9]{0,4}[a-z-]{0,2}", action in "(open|default|done|read|.{0,8})") {
        if let Some(click) = clicked(&key, &action) {
            let (kind, id) = key.split_once('-').unwrap();
            let id: u64 = id.parse().unwrap();
            proptest::prop_assert_eq!(format!("{kind}-{id}"), key.clone());
            proptest::prop_assert!(id > 0);
            proptest::prop_assert!(matches!(click, Click::Show) || matches!(click, Click::Open(n) | Click::Done(n) | Click::Read(n) if n == id));
        }
    }
}

#[test]
fn markup_is_escaped_for_a_desktop_that_reads_it() {
    assert_eq!(escape("a < b && c > \"d\" 'é'"), "a &lt; b &amp;&amp; c &gt; \"d\" 'é'");
    assert_eq!(escape(""), "");
}

#[test]
fn privacy_names_what_a_notification_hands_on() {
    let privacy = include_str!("../../../PRIVACY.md").split_whitespace().collect::<Vec<_>>().join(" ");
    for point in [
        "the sender's name, or their address when the mail gives no name, and the subject",
        "the name or address of the first person you wrote to",
        "the number of new messages and the account's address",
        "may keep notifications in its list until you clear it",
        "its own settings for InboxCraft decide whether they show on the lock screen",
        "keeps these choices in `notify.json`",
        "names each account by its number only",
        "the work still queued for it and its notification choice in `notify.json`",
    ] {
        assert!(privacy.contains(point), "PRIVACY.md does not say: {point}");
    }
}

#[test]
fn the_buttons_have_names_on_the_bus_and_text() {
    assert_eq!(Button::Done.action(), "done");
    assert_eq!(Button::Read.label(), "Mark read");
}
