// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use inboxcraft_store::Storage;
use inboxcraft_time::{FakeClock, Timestamp};
use serde_json::{Value, json};

use super::{FOLDERS, MESSAGES, mailbox, people, session};
use crate::component::avatar;
use crate::theme::{AVATARS, Mode, PRESETS, Theme};

/// 2026-11-05T09:00:00Z.
fn now() -> Timestamp {
    Timestamp::from_unix(1_793_869_200, 0).unwrap()
}

fn run(command: &str, params: &Value) -> Value {
    session(Box::new(FakeClock::new(now()))).unwrap().run(command, params).unwrap()
}

#[test]
fn every_person_and_account_is_on_example_org() {
    let store = mailbox(now()).unwrap();
    let mut seen = 0;
    for account in store.accounts().unwrap() {
        assert!(account.address().as_str().ends_with("@example.org"), "{}", account.address());
        assert_eq!(account.imap().host().as_str(), "imap.example.org");
        for folder in store.folders(account.id()).unwrap() {
            for entry in store.messages(folder.id()).unwrap() {
                let envelope = entry.message().summary().envelope();
                for address in envelope.from().iter().chain(envelope.to()) {
                    assert!(address.email().as_str().ends_with("@example.org"), "{}", address.email());
                    seen += 1;
                }
            }
        }
    }
    assert_eq!(seen, MESSAGES.len() * 2);
}

#[test]
fn two_accounts_with_their_addresses_and_names() {
    let accounts = run("account.list", &json!({}));
    let shown: Vec<(&str, &str, &str)> = accounts["accounts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|account| (account["label"].as_str().unwrap(), account["address"].as_str().unwrap(), account["display_name"].as_str().unwrap()))
        .collect();
    assert_eq!(shown, [("Personal", "mira@example.org", "Mira Holloway"), ("Studio", "holloway.studio@example.org", "Mira Holloway")]);
}

#[test]
fn every_folder_role_is_there_and_the_unified_views_span_both_accounts() {
    let folders = run("folder.list", &json!({}));
    assert_eq!(folders["folders"].as_array().unwrap().len(), FOLDERS.len());
    let roles: Vec<&str> = folders["folders"].as_array().unwrap().iter().filter(|f| f["account"] == 1).filter_map(|f| f["role"].as_str()).collect();
    for role in ["inbox", "drafts", "sent", "archive", "junk", "trash"] {
        assert!(roles.contains(&role), "{role} missing from {roles:?}");
    }
    let inbox = folders["views"].as_array().unwrap().iter().find(|view| view["role"] == "inbox").unwrap();
    assert_eq!(inbox["folders"], json!([1, 10]));
    assert!(folders["roles_to_ask"].as_array().unwrap().iter().all(|ask| ask["account"] == 2), "{}", folders["roles_to_ask"]);
    let nested = folders["folders"].as_array().unwrap().iter().find(|f| f["name"] == "Clients/Harbour Bakery").unwrap();
    assert_eq!(nested["leaf"], "Harbour Bakery");
}

#[test]
fn the_unified_inbox_is_recent_newest_first_and_partly_unread() {
    let list = run("mail.list", &json!({ "role": "inbox" }));
    let messages = list["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 11);
    let dates: Vec<&str> = messages.iter().map(|m| m["date"].as_str().unwrap()).collect();
    let mut sorted = dates.clone();
    sorted.sort_unstable_by(|a, b| b.cmp(a));
    assert_eq!(dates, sorted);
    assert_eq!(dates[0], "2026-11-05T08:35:00Z", "25 minutes before now");
    assert!(messages.iter().any(|m| m["account"] == 1) && messages.iter().any(|m| m["account"] == 2));
    assert_eq!(messages.iter().filter(|m| m["seen"] == false).count(), 4);
    assert!(messages.iter().any(|m| m["flagged"] == true));
}

#[test]
fn the_search_finds_sample_mail_by_subject_and_sender() {
    let found = run("mail.search", &json!({ "subject": "logo" }));
    assert_eq!(found["total"], 2);
    let found = run("mail.search", &json!({ "from": "odile" }));
    assert_eq!(found["total"], 2);
}

#[test]
fn the_dates_follow_now() {
    let later = Timestamp::from_unix(now().unix_seconds() + 86_400, 0).unwrap();
    let list = session(Box::new(FakeClock::new(later))).unwrap().run("mail.list", &json!({ "limit": 1 })).unwrap();
    assert_eq!(list["messages"][0]["date"], "2026-11-06T08:35:00Z");
}

#[test]
fn a_mailbox_at_the_start_of_time_still_builds() {
    // Dates before the earliest timestamp are left out rather than refused.
    let store = mailbox(Timestamp::MIN).unwrap();
    assert_eq!(store.accounts().unwrap().len(), 2);
}

#[test]
fn every_sample_message_has_a_body_to_read() {
    let mut session = session(Box::new(FakeClock::new(now()))).unwrap();
    for id in 1..=MESSAGES.len() {
        let answer = session.run("mail.read", &json!({ "id": id })).unwrap();
        assert_eq!(answer["fetching"], json!(false), "message {id}");
        let body = &answer["message"]["body"];
        assert_eq!(body["readable"], json!(true), "message {id}");
        assert!(body["text"].as_str().is_some_and(|text| !text.trim().is_empty()), "message {id}");
    }
    assert!(session.take_jobs().is_empty(), "the sample fetches nothing");
}

#[test]
fn the_replies_share_a_thread_with_what_they_answer() {
    let mut session = session(Box::new(FakeClock::new(now()))).unwrap();
    let found = session.run("mail.search", &json!({ "subject": "Re: Allotment rota" })).unwrap();
    let reply = found["messages"][0]["id"].as_u64().unwrap();
    let thread = session.run("mail.thread", &json!({ "id": reply })).unwrap();
    let subjects: Vec<&str> = thread["messages"].as_array().unwrap().iter().map(|m| m["subject"].as_str().unwrap()).collect();
    assert_eq!(subjects, ["Allotment rota for November", "Re: Allotment rota for November"]);
    let text = session.run("mail.read", &json!({ "id": reply })).unwrap()["message"]["body"]["text"].as_str().unwrap().to_owned();
    assert!(text.contains("Odile Brannock wrote:\r\n> Hi all,"), "{text}");
}

#[test]
fn links_and_pictures_stay_on_invented_hosts() {
    let mut session = session(Box::new(FakeClock::new(now()))).unwrap();
    let mut html_seen = 0;
    for id in 1..=MESSAGES.len() {
        let body = session.run("mail.read", &json!({ "id": id })).unwrap()["message"]["body"].clone();
        for text in [&body["text"], &body["html"]].into_iter().filter_map(Value::as_str) {
            for (at, _) in text.match_indices("://") {
                let host: String = text[at + 3..].chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '-').collect();
                assert!(host.ends_with("example.org") || host.ends_with("example.net"), "{host} in message {id}");
            }
        }
        html_seen += usize::from(body["html"].is_string());
    }
    assert_eq!(html_seen, 2);
}

#[test]
fn the_logo_sketches_are_pictures_drawn_in_code() {
    let mut session = session(Box::new(FakeClock::new(now()))).unwrap();
    let found = session.run("mail.search", &json!({ "subject": "Draft logo", "from": "petra" })).unwrap();
    let id = found["messages"][0]["id"].as_u64().unwrap();
    let attachments = session.run("mail.read", &json!({ "id": id })).unwrap()["message"]["body"]["attachments"].clone();
    let names: Vec<&str> = attachments.as_array().unwrap().iter().map(|a| a["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["sketch-1.png", "sketch-2.png", "sketch-3.png"]);
    assert!(attachments.as_array().unwrap().iter().all(|a| a["type"] == "image/png" && a["size"].as_u64().unwrap() > 100));
}

#[test]
fn the_sample_draws_its_remote_pictures_and_asks_nobody() {
    use crate::fetch::Fetch as _;
    let mut pictures = super::Pictures::default();
    assert!(pictures.finished().is_empty());
    pictures.start("https://pictures.example.org/cycles/workshop.png");
    pictures.start("https://track.example.net/open.png?id=4417");
    pictures.start("https://elsewhere.example.com/a.png");
    pictures.start("http://pictures.example.org/plain.png");
    let done = pictures.finished();
    assert_eq!(done.len(), 4);
    for (url, answer) in done.iter().take(2) {
        let picture = crate::picture::decode(answer.as_ref().unwrap()).unwrap();
        assert_eq!((picture.width, picture.height), (480, 200), "{url}");
    }
    assert!(done.iter().skip(2).all(|(_, answer)| answer.is_err()));
}

#[test]
fn everyone_who_writes_or_is_written_to_has_an_avatar_from_the_avatar_palette() {
    let people = people();
    for sample in MESSAGES {
        for person in [sample.from, sample.to] {
            assert!(people.contains(&(person.name, person.address)), "{} is in people()", person.address);
        }
    }
    for preset in &PRESETS {
        for mode in [Mode::Light, Mode::Dark] {
            let theme = Theme::new(mode, preset);
            for (name, address) in &people {
                assert!(AVATARS.contains(&avatar::colour(&theme, address)), "{name}");
            }
        }
    }
}

#[test]
fn the_screener_holds_two_new_senders_before_the_inbox() {
    let answer = run("screener.list", &json!({}));
    assert_eq!(answer["mode"], "before");
    let senders: Vec<&str> = answer["senders"].as_array().unwrap().iter().map(|sender| sender["address"].as_str().unwrap()).collect();
    assert_eq!(senders, ["club@example.org", "plants@example.org"]);
    let held = run("mail.list", &json!({ "role": "screener" }));
    assert_eq!(held["total"], 2);
    let inbox = run("mail.list", &json!({ "role": "inbox" }));
    assert!(inbox["messages"].as_array().unwrap().iter().all(|row| row["new_sender"] == false), "the inbox holds no new sender");
}

#[test]
fn the_inbox_is_grouped_into_people_and_updates() {
    let inbox = run("mail.list", &json!({ "role": "inbox", "limit": 100 }));
    let group = |subject: &str| {
        let rows = inbox["messages"].as_array().unwrap();
        rows.iter().find(|row| row["subject"] == subject).map(|row| row["group"].clone()).unwrap()
    };
    assert_eq!(group("Allotment rota for November"), "people");
    assert_eq!(group("Draft logo for the bakery"), "people");
    assert_eq!(group("Your library loan is due on Friday"), "updates");
    assert_eq!(group("Your bike is due for a service"), "updates");
    assert_eq!(group("Invoice 2026-031 is paid"), "updates");
    let newsletters = run("mail.list", &json!({ "role": "trash", "group": "newsletters" }));
    assert_eq!(newsletters["total"], 2, "both newsletters carry their list's fields");
}

#[test]
fn put_away_fills_every_pile_of_later_and_leaves_a_done_and_a_returned_message() {
    use crate::mailbox::{Mailbox, Pile, Place};

    let mut mailbox = Mailbox::new(session(Box::new(FakeClock::new(now()))).unwrap(), Box::new(FakeClock::new(now())));
    super::put_away(&mut mailbox).unwrap();
    assert!(mailbox.undoable().is_none(), "no toast left over");
    let subjects = |mailbox: &Mailbox| mailbox.messages().iter().map(|row| row.subject.clone()).collect::<Vec<_>>();
    mailbox.open(Place::Later(Pile::Snoozed));
    assert_eq!(
        subjects(&mailbox),
        ["Your ferry booking is confirmed", "A question about the font licence", "Tenancy renewal"],
        "the first back on top"
    );
    mailbox.open(Place::Later(Pile::Pins));
    assert_eq!(mailbox.messages().len(), 2);
    mailbox.open(Place::Later(Pile::Parked));
    assert_eq!(mailbox.messages().len(), 2);
    mailbox.open(Place::View(inboxcraft_model::Role::Inbox));
    let choir = mailbox.messages().iter().find(|row| row.subject == "Choir practice moves to Thursday").unwrap();
    assert!(mailbox.returned(choir));
    assert!(mailbox.messages().iter().all(|row| row.subject != "Re: Soup recipe"), "done");
    mailbox.set_show_done(true);
    assert!(mailbox.messages().iter().any(|row| row.subject == "Re: Soup recipe" && row.done));
}
