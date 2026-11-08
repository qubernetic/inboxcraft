// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Which arriving mail tells the owner: the decision table, then the same through events, commands and the
//! scheduler.

use super::{Facts, Level, tells};

/// New mail from a person the owner knows, unread, in the inbox, after the first sync: what tells at every level but
/// `none`.
fn fresh(level: Level) -> Facts {
    Facts { level, inbox: true, first_sync: false, stays: true, seen: false, own: false, person: true, quiet: false }
}

#[test]
fn the_decision_table() {
    type Case = (&'static str, fn(&mut Facts), [bool; 3]);
    let cases: [Case; 8] = [
        ("fresh mail from a person", |_| {}, [true, true, false]),
        ("from a sender the owner does not know", |facts| facts.person = false, [true, false, false]),
        ("the folder's first sync", |facts| facts.first_sync = true, [false, false, false]),
        ("in another folder than the inbox", |facts| facts.inbox = false, [false, false, false]),
        ("held or blocked by an earlier stage", |facts| facts.stays = false, [false, false, false]),
        ("read already, elsewhere", |facts| facts.seen = true, [false, false, false]),
        ("from one of the owner's own addresses", |facts| facts.own = true, [false, false, false]),
        ("the owner's own move, back from the server", |facts| facts.quiet = true, [false, false, false]),
    ];
    for (case, change, expected) in cases {
        for (level, tell) in Level::ALL.into_iter().zip(expected) {
            let mut facts = fresh(level);
            change(&mut facts);
            assert_eq!(tells(&facts), tell, "{case} at {}", level.name());
        }
    }
}

mod session {
    use inboxcraft_model::{
        AccountId, Address, AlertId, DraftId, Due, EmailAddress, Envelope, Event, EventKind, Flag, Group, JobKind, MessageId, MessageRef, ScheduleId,
        Scheduled, Screening, Sender, Summary,
    };
    use inboxcraft_store::Change;
    use serde_json::{Value, json};

    use crate::arrive::screener::tests::{arrive, arrived, decided, from, known, mode, settle};
    use crate::cmd::act_tests::{session, uid};
    use crate::notify::{self, About, Level, MAX_PENDING, MAX_TEXT, Notice, Settings, line};
    use crate::schedule::Fired;
    use crate::test_support::{NOW, at, change};
    use crate::{Caller, Session};

    /// A message from `sender` with UID `n` and the Message-ID `<n@example.com>`.
    fn with_id(n: u32, sender: &str) -> Summary {
        let envelope = from(n, sender).envelope().clone().with_message_id(MessageRef::new(&format!("<{n}@example.com>")));
        Summary::new(uid(n), envelope)
    }

    fn mail(session: &Session, n: u32, who: &str, subject: &str) -> Notice {
        let message = arrived(session, n);
        Notice::Mail(About { account: AccountId::new(1), message: message.id(), who: who.to_owned(), subject: subject.to_owned() })
    }

    fn level(session: &mut Session, account: u64, level: &str) {
        session.run("notify.set", &json!({ "account": account, "level": level })).unwrap();
    }

    #[test]
    fn new_mail_in_the_inbox_tells_the_owner_who_it_is_from_and_what_about() {
        let mut session = session();
        arrive(&mut session, 1, vec![from(20, "new@example.com")]);
        assert_eq!(session.take_notices(), vec![mail(&session, 20, "Someone", "Hello")]);
        assert_eq!(session.take_notices(), vec![], "each notice is handed over once");
    }

    #[test]
    fn a_sender_without_a_name_is_told_by_address() {
        let mut session = session();
        let envelope =
            Envelope::new("  Two\r\n lines\u{202e} ").with_from(vec![Address::new(Some(" "), EmailAddress::new("new@example.com").unwrap())]);
        arrive(&mut session, 1, vec![Summary::new(uid(20), envelope)]);
        assert_eq!(session.take_notices(), vec![mail(&session, 20, "new@example.com", "Two lines")]);
    }

    #[test]
    fn the_first_sync_of_a_folder_tells_nothing() {
        let mut session = crate::test_support::session();
        arrive(&mut session, 1, vec![from(20, "new@example.com")]);
        assert_eq!(session.take_notices(), vec![]);
    }

    #[test]
    fn mail_in_another_folder_read_mail_and_mail_from_the_owner_tell_nothing() {
        let mut session = session();
        arrive(&mut session, 3, vec![from(20, "new@example.com")]);
        let read = from(21, "new@example.com").with_flags([Flag::Seen].into_iter().collect());
        arrive(&mut session, 1, vec![read, from(22, "Anna@Example.org"), from(23, "bob@example.net")]);
        assert_eq!(session.take_notices(), vec![]);
    }

    #[test]
    fn people_tells_of_the_senders_the_owner_knows_or_accepted() {
        let mut session = session();
        level(&mut session, 1, "people");
        change(&mut session, known("friend@example.com"));
        change(&mut session, decided("accepted@example.com", Screening::Accepted));
        let updates = Sender { known: true, group: Some(Group::Updates), ..Sender::new(EmailAddress::new("shop@example.com").unwrap()) };
        change(&mut session, Change::PutSender(updates));
        arrive(
            &mut session,
            1,
            vec![from(20, "new@example.com"), from(21, "friend@example.com"), from(22, "accepted@example.com"), from(23, "shop@example.com")],
        );
        assert_eq!(session.take_notices(), vec![mail(&session, 21, "Someone", "Hello"), mail(&session, 22, "Someone", "Hello")]);
    }

    #[test]
    fn people_follows_the_classifier_unless_the_owner_chose_a_group() {
        let mut session = session();
        level(&mut session, 1, "people");
        change(&mut session, known("news@example.com"));
        change(&mut session, known("club@example.com"));
        let people = Sender { known: true, group: Some(Group::People), ..Sender::new(EmailAddress::new("club@example.com").unwrap()) };
        change(&mut session, Change::PutSender(people));
        let list = |n: u32, sender: &str| {
            let summary = from(n, sender);
            let envelope = summary.envelope().clone().with_signs(inboxcraft_model::Signs::read([("List-Id", "<list.example.com>")]));
            summary.with_envelope(envelope)
        };
        arrive(&mut session, 1, vec![list(20, "news@example.com"), list(21, "club@example.com")]);
        assert_eq!(session.take_notices(), vec![mail(&session, 21, "Someone", "Hello")], "a known sender's newsletter tells nothing");
    }

    #[test]
    fn none_tells_nothing_of_that_account() {
        let mut session = session();
        level(&mut session, 1, "none");
        arrive(&mut session, 1, vec![from(20, "new@example.com")]);
        assert_eq!(session.take_notices(), vec![]);
        assert_eq!(session.notify_settings().level(AccountId::new(2)), Level::All, "another account keeps its own");
    }

    #[test]
    fn mail_the_screener_holds_or_blocks_tells_nothing() {
        let mut session = session();
        change(&mut session, decided("blocked@example.com", Screening::Blocked));
        arrive(&mut session, 1, vec![from(20, "blocked@example.com")]);
        mode(&mut session, "before");
        arrive(&mut session, 1, vec![from(21, "new@example.com")]);
        assert_eq!(session.take_notices(), vec![]);
        mode(&mut session, "inside");
        arrive(&mut session, 1, vec![from(22, "other@example.com")]);
        assert_eq!(session.take_notices(), vec![mail(&session, 22, "Someone", "Hello")], "marked inside the inbox, it is there");
    }

    #[test]
    fn accepting_a_sender_lets_the_held_mail_in_without_a_burst() {
        let mut session = session();
        mode(&mut session, "before");
        arrive(&mut session, 1, vec![with_id(20, "new@example.com"), with_id(21, "new@example.com")]);
        settle(&mut session);
        session.run("screener.accept", &json!({ "address": "new@example.com" })).unwrap();
        // A server without UIDPLUS names no UID for the moves back: the next sync of the inbox brings them anew.
        for job in session.take_jobs() {
            if let JobKind::Move { folder, uid_validity, uids, to, .. } = job.kind {
                session.apply(&Event::new(job.account, Some(job.id), EventKind::Moved { folder, uid_validity, uids, to, copied: None })).unwrap();
            }
        }
        let back: Vec<Summary> = [20, 21].map(|n| with_id(n, "new@example.com").with_uid(uid(n + 10))).to_vec();
        arrive(&mut session, 1, back);
        assert_eq!(session.take_notices(), vec![], "the owner's own move");
        arrive(&mut session, 1, vec![with_id(40, "new@example.com")]);
        assert_eq!(session.take_notices(), vec![mail(&session, 40, "Someone", "Hello")], "new mail of the accepted sender tells");
    }

    #[test]
    fn many_new_messages_at_once_are_one_notice_that_counts_them() {
        let mut session = session();
        arrive(&mut session, 1, (20..23).map(|n| from(n, "new@example.com")).collect());
        assert_eq!(session.take_notices().len(), 3);
        arrive(&mut session, 1, (30..35).map(|n| from(n, "new@example.com")).collect());
        assert_eq!(session.take_notices(), vec![Notice::Mails { account: AccountId::new(1), count: 5 }]);
    }

    #[test]
    fn the_queue_keeps_the_newest_notices_for_a_front_end_that_takes_none() {
        let mut session = session();
        for n in 0..150 {
            arrive(&mut session, 1, vec![from(100 + n, "new@example.com")]);
        }
        assert_eq!(session.take_notices(), vec![Notice::Mails { account: AccountId::new(1), count: MAX_PENDING }]);
    }

    #[test]
    fn a_removed_account_leaves_no_notices_and_no_choice_behind() {
        let mut session = session();
        level(&mut session, 1, "people");
        arrive(&mut session, 1, vec![from(20, "new@example.com")]);
        change(&mut session, known("friend@example.com"));
        arrive(&mut session, 1, vec![from(21, "friend@example.com")]);
        session.run("account.remove", &json!({ "account": 1 })).unwrap();
        assert_eq!(session.take_notices(), vec![]);
        assert_eq!(session.notify_settings(), &Settings::default());
    }

    fn scheduled(id: u64, due: i64, what: Due) -> Change {
        Change::PutScheduled(Scheduled { id: ScheduleId::new(id), due: at(due), what })
    }

    #[test]
    fn a_returned_snooze_and_a_reminder_tell_the_owner() {
        let mut session = session();
        change(&mut session, scheduled(1, NOW - 10, Due::Unsnooze(MessageId::new(1))));
        change(&mut session, scheduled(2, NOW - 3600, Due::FollowUp(MessageId::new(5))));
        session.tick().unwrap();
        let about = |message: u64, who: &str, subject: &str| About {
            account: AccountId::new(1),
            message: MessageId::new(message),
            who: who.to_owned(),
            subject: subject.to_owned(),
        };
        let sent = session.store().message(MessageId::new(5)).unwrap().unwrap();
        let envelope = sent.message().summary().envelope();
        assert_eq!((envelope.from()[0].name(), envelope.to()[0].name()), (Some("Anna Example"), Some("Bob")));
        assert_eq!(
            session.take_notices(),
            vec![
                // A reminder names the one written to, not the writer.
                Notice::Reminder { about: about(5, "Bob", "Re: Budget for 2026"), missed: true },
                Notice::Returned { about: about(1, "Anna Example", "Budget for 2026"), missed: false },
            ]
        );
    }

    #[test]
    fn a_message_the_store_no_longer_holds_and_other_entries_tell_nothing() {
        let session = session();
        let fired = |what: Due| Fired { entry: Scheduled { id: ScheduleId::new(1), due: at(NOW), what }, missed: false };
        let entries = [
            fired(Due::Unsnooze(MessageId::new(99))),
            fired(Due::FollowUp(MessageId::new(99))),
            fired(Due::Alert(AlertId::new(1))),
            fired(Due::Send(DraftId::new(1))),
        ];
        assert_eq!(notify::fired(session.store(), &Settings::default(), &entries, &[]).unwrap(), vec![]);
    }

    #[test]
    fn returns_and_reminders_tell_nothing_once_turned_off() {
        let mut session = session();
        let answer = session.run("notify.set", &json!({ "returns": false, "reminders": false })).unwrap();
        assert_eq!((answer["returns"].clone(), answer["reminders"].clone()), (json!(false), json!(false)));
        change(&mut session, scheduled(1, NOW - 10, Due::Unsnooze(MessageId::new(1))));
        change(&mut session, scheduled(2, NOW - 10, Due::FollowUp(MessageId::new(5))));
        session.tick().unwrap();
        assert_eq!(session.take_notices(), vec![]);
    }

    #[test]
    fn the_settings_list_every_account() {
        let mut session = session();
        let answer = session.run_as(Caller::Agent, "notify.settings", &json!({})).unwrap();
        assert_eq!(
            answer,
            json!({
                "accounts": [
                    { "account": 1, "address": "anna@example.org", "level": "all" },
                    { "account": 2, "address": "bob@example.net", "level": "all" },
                ],
                "levels": ["all", "people", "none"],
                "returns": true,
                "reminders": true,
            })
        );
        let answer = session.run("notify.set", &json!({ "account": 2, "level": "people", "returns": false })).unwrap();
        assert_eq!(answer["accounts"][1]["level"], json!("people"));
        assert_eq!(answer["returns"], json!(false));
        assert_eq!(answer["reminders"], json!(true));
        assert_eq!(session.run("notify.settings", &json!({})).unwrap(), answer);
        assert_eq!(session.notify_settings().level(AccountId::new(2)), Level::People);
    }

    #[test]
    fn changing_the_settings_is_the_owner_s() {
        let mut session = session();
        let refused = session.run_as(Caller::Agent, "notify.set", &json!({ "account": 1, "level": "none" })).unwrap_err();
        assert_eq!(refused.code(), "not_allowed");
        assert_eq!(session.notify_settings(), &Settings::default());
    }

    #[test]
    fn bad_settings_are_refused_and_change_nothing() {
        let mut session = session();
        for params in [
            json!({}),
            json!({ "account": 1 }),
            json!({ "level": "none" }),
            json!({ "account": 1, "level": "loud" }),
            json!({ "returns": "yes" }),
            json!({ "account": 1, "level": "none", "volume": 3 }),
        ] {
            assert_eq!(session.run("notify.set", &params).unwrap_err().code(), "bad_params", "{params}");
        }
        assert_eq!(session.run("notify.set", &json!({ "account": 9, "level": "none" })).unwrap_err().code(), "not_found");
        assert_eq!(session.run("notify.settings", &json!({ "account": 1 })).unwrap_err().code(), "bad_params");
        assert_eq!(session.notify_settings(), &Settings::default());
    }

    #[test]
    fn the_settings_are_kept_as_json_and_read_back_leniently() {
        let mut settings = Settings::default();
        settings.set_level(AccountId::new(1), Level::None);
        settings.set_level(AccountId::new(3), Level::People);
        settings.set_level(AccountId::new(3), Level::All);
        settings.reminders = false;
        let kept = settings.to_value();
        assert_eq!(kept, json!({ "accounts": [{ "account": 1, "level": "none" }], "returns": true, "reminders": false }));
        assert_eq!(Settings::from_value(&kept), settings);
        let junk = json!({
            "accounts": [{ "account": 0, "level": "none" }, { "account": 2, "level": "loud" }, { "account": "2" }, 7, { "account": 4, "level": "people" }],
            "returns": "no",
        });
        let mut expected = Settings::default();
        expected.set_level(AccountId::new(4), Level::People);
        assert_eq!(Settings::from_value(&junk), expected);
        for value in [Value::Null, json!([]), json!("x"), json!({ "accounts": {} })] {
            assert_eq!(Settings::from_value(&value), Settings::default(), "{value}");
        }
        let mut session = session();
        session.set_notify_settings(expected.clone());
        assert_eq!(session.notify_settings(), &Settings::default(), "account 4 is not held");
        expected.set_level(AccountId::new(1), Level::None);
        session.set_notify_settings(expected);
        let mut held = Settings::default();
        held.set_level(AccountId::new(1), Level::None);
        assert_eq!(session.notify_settings(), &held, "only the held account's level is taken");
    }

    #[test]
    fn a_notice_s_text_is_one_short_clean_line() {
        assert_eq!(line("  a\tb\r\n\u{0}c\u{1b}[2J \u{200f}d "), "a b c[2J d");
        let long = line(&"é".repeat(1000));
        assert_eq!(long.chars().count(), MAX_TEXT);
        assert_eq!(line(&format!("{} b", "a".repeat(MAX_TEXT - 1))), "a".repeat(MAX_TEXT - 1), "no space at the end");
        assert_eq!(line(""), "");
    }

    #[test]
    fn a_notice_names_its_account_and_message() {
        let about = About { account: AccountId::new(2), message: MessageId::new(7), who: String::new(), subject: String::new() };
        assert_eq!(Notice::Mail(about.clone()).message(), Some(MessageId::new(7)));
        assert_eq!(Notice::Returned { about: about.clone(), missed: false }.account(), AccountId::new(2));
        assert_eq!(Notice::Reminder { about, missed: true }.message(), Some(MessageId::new(7)));
        let count = Notice::Mails { account: AccountId::new(3), count: 4 };
        assert_eq!((count.account(), count.message()), (AccountId::new(3), None));
        assert_eq!(Level::from_name("loud"), None);
    }
}
