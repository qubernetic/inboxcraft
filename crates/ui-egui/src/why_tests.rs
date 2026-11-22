// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use inboxcraft_time::{FakeClock, Timestamp};
use serde_json::{Value, json};

use super::{MAX_REASONS, ask, words};
use crate::mailbox::{GroupKind, Mailbox};
use crate::sample;

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

fn mailbox() -> Mailbox {
    Mailbox::new(sample::session(clock()).unwrap(), clock())
}

fn one(text_id: &str, args: Value) -> Value {
    json!({ "reasons": [{ "kind": "x", "text_id": text_id, "args": args }] })
}

/// The engine's thirteen ids, each with args it could give.
fn thirteen() -> Vec<(&'static str, Value)> {
    vec![
        ("why.snooze.returned", json!({ "at": "2026-11-05T09:00:00Z" })),
        ("why.snooze.until", json!({ "until": "2026-11-10T09:00:00Z" })),
        ("why.rule.group", json!({ "group": "newsletters", "rule": "list" })),
        ("why.sender.group", json!({ "address": "ana@example.org", "group": "people" })),
        ("why.sender.accepted", json!({ "address": "ana@example.org" })),
        ("why.sender.blocked", json!({ "address": "ana@example.org" })),
        ("why.screener.held", json!({ "address": null })),
        ("why.screener.blocked", json!({ "address": null })),
        ("why.screener.new", json!({ "address": "ana@example.org" })),
        ("why.screener.known", json!({ "address": "ana@example.org" })),
        ("why.thread.yours", json!({ "messages": 3 })),
        ("why.thread.other", json!({ "messages": 5 })),
        ("why.default", json!({ "folder": 999_999, "role": null })),
    ]
}

#[test]
fn every_id_of_the_engine_is_one_sentence_in_the_windows_words() {
    let mailbox = mailbox();
    let expected = [
        "back from a snooze, ",
        "snoozed until ",
        "grouped as Newsletters: sent through a list",
        "you put ana@example.org in People",
        "let in by you",
        "blocked by you",
        "held back by the screener",
        "in Blocked",
        "first mail from this address",
        "a sender you know",
        "in a thread you wrote in, 3 messages",
        "in a thread of 5 messages",
        "in this folder",
    ];
    for ((text_id, args), expected) in thirteen().into_iter().zip(expected) {
        let said = words(&mailbox, 0, &one(text_id, args));
        assert_eq!(said.len(), 1, "{text_id}: {said:?}");
        assert!(said[0].starts_with(expected), "{text_id}: {said:?}");
        assert!(!said[0].contains("why.") && !said[0].contains('{'), "{text_id}: {said:?}");
    }
}

#[test]
fn an_id_the_window_does_not_know_is_left_out_and_never_shown() {
    let mailbox = mailbox();
    let answer = json!({ "reasons": [
        { "text_id": "why.sender.accepted", "args": {} },
        { "text_id": "why.something.new", "args": { "address": "x@example.org" } },
        { "text_id": 7, "args": {} },
        { "args": {} },
        "why.thread.other",
        { "text_id": "why.thread.other", "args": { "messages": "lots" } },
        { "text_id": "why.rule.group", "args": { "group": "Subject line of the mail" } },
    ]});
    assert_eq!(words(&mailbox, 0, &answer), ["let in by you"]);
    for junk in [json!(null), json!([]), json!({ "reasons": {} }), json!({ "reasons": "why.default" })] {
        assert!(words(&mailbox, 0, &junk).is_empty(), "{junk}");
    }
}

#[test]
fn at_most_four_reasons_in_the_engines_order() {
    let mailbox = mailbox();
    let reasons: Vec<Value> = thirteen().into_iter().map(|(text_id, args)| json!({ "text_id": text_id, "args": args })).collect();
    let said = words(&mailbox, 0, &json!({ "reasons": reasons }));
    assert_eq!(said.len(), MAX_REASONS);
    assert!(said[0].starts_with("back from a snooze") && said[3].starts_with("you put"), "{said:?}");
}

#[test]
fn an_address_of_254_characters_with_controls_comes_out_clean_and_whole() {
    let mailbox = mailbox();
    let local = "a".repeat(240);
    let address = format!("{local}@example.org\u{202E}\r\n\u{0}");
    let said = words(&mailbox, 0, &one("why.sender.group", json!({ "address": address, "group": "people" })));
    assert_eq!(said.len(), 1);
    assert!(said[0].contains(&format!("{local}@example.org")), "{said:?}");
    assert!(!said[0].chars().any(|c| c.is_control() || c == '\u{202E}'), "{said:?}");
}

#[test]
fn a_folder_is_named_as_the_index_names_it() {
    let mut mailbox = mailbox();
    let folders: Vec<(u64, Option<inboxcraft_model::Role>, String)> = mailbox
        .accounts()
        .iter()
        .flat_map(|account| mailbox.folders_of(account.id).into_iter().map(|folder| (folder.id, folder.role, folder.name.clone())))
        .collect();
    let (with_role, role, _) = folders.iter().find(|folder| folder.1.is_some()).unwrap();
    let said = words(&mailbox, 0, &one("why.default", json!({ "folder": with_role })));
    assert_eq!(said, [format!("in {}", crate::mailbox::role_text(role.unwrap()))]);
    if let Some((plain, _, name)) = folders.iter().find(|folder| folder.1.is_none()) {
        assert_eq!(words(&mailbox, 0, &one("why.default", json!({ "folder": plain }))), [format!("in {name}")]);
    }
    // A message gone is no reason and no error.
    assert!(ask(&mut mailbox, u64::MAX).is_empty());
}

#[test]
fn every_message_of_the_sample_has_one_to_four_reasons_in_words() {
    let mut mailbox = mailbox();
    let ids: Vec<u64> = mailbox.messages().iter().map(|row| row.id).collect();
    assert!(!ids.is_empty());
    for id in ids {
        let said = ask(&mut mailbox, id);
        assert!((1..=MAX_REASONS).contains(&said.len()), "{id}: {said:?}");
        assert!(said.iter().all(|text| !text.is_empty() && !text.contains("why.")), "{id}: {said:?}");
    }
}

fn reader() -> crate::reader::Reader {
    crate::reader::Reader::new(Box::new(crate::reader::tests::Recorder::default()), Box::new(crate::save::NoSave))
}

#[test]
fn the_engine_is_asked_once_when_a_message_opens_and_again_only_when_its_place_changes() {
    let mut mailbox = mailbox();
    let mut reader = reader();
    let id = mailbox.messages().first().unwrap().id;
    reader.open(&mut mailbox, Some(id));
    assert_eq!(reader.asked_why, 1);
    assert!(!reader.why().is_empty());
    // Nothing moved: the cards are read again, the reasons are kept.
    for _ in 0..5 {
        reader.refresh_cards_and_why(&mut mailbox);
    }
    assert_eq!(reader.asked_why, 1);
    // Snoozed while open: asked again, and it says so.
    mailbox.run("mail.snooze", &json!({ "id": id, "until": "2026-11-06T18:00:00Z" })).unwrap();
    mailbox.refresh();
    reader.refresh_cards_and_why(&mut mailbox);
    assert_eq!(reader.asked_why, 2);
    reader.refresh_cards_and_why(&mut mailbox);
    assert_eq!(reader.asked_why, 2, "once per change of place");
    // Closed: nothing to say, nothing asked.
    reader.open(&mut mailbox, None);
    assert!(reader.why().is_empty());
    assert_eq!(reader.asked_why, 2);
}

#[test]
fn a_message_moved_while_open_says_where_it_went() {
    let mut mailbox = mailbox();
    let mut reader = reader();
    let id = mailbox.messages().iter().find(|row| row.subject == "Photos from the coast walk").unwrap().id;
    reader.open(&mut mailbox, Some(id));
    let before = reader.why().to_vec();
    let moved = mailbox.run("mail.archive", &json!({ "id": id }));
    mailbox.refresh();
    reader.refresh_cards_and_why(&mut mailbox);
    let archive = crate::mailbox::role_text(inboxcraft_model::Role::Archive);
    assert_eq!(reader.asked_why, 2, "{moved:?}");
    assert!(reader.why().iter().any(|reason| reason == &format!("in {archive}")), "{before:?} {:?} {moved:?}", reader.why());
}

#[test]
fn a_message_the_engine_cannot_find_has_no_reasons_and_no_error() {
    let mut mailbox = mailbox();
    let mut reader = reader();
    reader.open(&mut mailbox, Some(u64::MAX));
    assert!(reader.why().is_empty());
    assert_eq!(reader.asked_why, 1);
}

#[test]
fn an_invitation_is_grouped_as_the_list_and_the_head_show_it() {
    use inboxcraft_store::{Batch, Storage as _};
    let now = Timestamp::from_unix(1_793_869_200, 0).unwrap();
    let mut store = sample::mailbox(now).unwrap();
    let mut batch = Batch::new();
    for change in sample::invitations(now).unwrap() {
        batch.push(change);
    }
    store.apply(&batch).unwrap();
    let mut mailbox = Mailbox::new(sample::session_over(store, clock()).unwrap(), clock());
    let row = mailbox.messages().iter().find(|row| row.subject == "Updated invitation: Lunch with Odile").cloned().unwrap();
    assert_eq!(row.group, Some(GroupKind::Invitations));
    let shown = GroupKind::shown_for(GroupKind::Invitations, &row.address).text();
    assert_ne!(shown, GroupKind::Invitations.text());
    let said = ask(&mut mailbox, row.id);
    assert_eq!(said.first(), Some(&format!("grouped as {shown}: it holds an invitation")), "{said:?}");
    assert!(said.iter().all(|reason| !reason.contains(GroupKind::Invitations.text())), "{said:?}");
    // A message the list does not hold: no group the list never shows.
    let calendar = one("why.rule.group", json!({ "group": "invitations", "rule": "calendar" }));
    assert_eq!(words(&mailbox, u64::MAX, &calendar), ["it holds an invitation"]);
    let sender = one("why.sender.group", json!({ "address": "noreply@example.org", "group": "invitations" }));
    assert_eq!(
        words(&mailbox, u64::MAX, &sender),
        [format!("you put noreply@example.org in {}", GroupKind::shown_for(GroupKind::Invitations, "noreply@example.org").text())]
    );
}
