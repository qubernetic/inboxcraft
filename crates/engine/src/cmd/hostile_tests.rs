// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `hostile_params_never_panic`: every command, run with junk parameters, answers or refuses and never panics.
//!
//! The commands come from `cmd::all` and the parameter names from each module's `FIELDS`, so a new command is in
//! here without anyone adding it. A panic would be caught by `Session::run` as `EngineError::Panicked`; that counts
//! as a failure here, because the safety net is not a licence. Each runs in a session over a store and in a sample
//! session, where the `sample.*` commands run too.
//!
//! The junk alone seldom names a thing that exists: its `1` removes account 1 before any `{"id": 1}` runs, and no
//! junk value names account 2's mail. `commands_on_things_that_exist_never_panic` therefore runs every command on
//! every account, folder, calendar, message, draft, outbox item, schedule entry and event a richer world holds,
//! alone, with each junk value in each other parameter its module reads, and with a second thing that exists. The
//! world is built again whenever a run took away a thing the next run names, so each run meets what it names.

use std::collections::BTreeSet;

use inboxcraft_model::{
    AccountId, CalendarEventId, DraftId, ErrorKind, Event, EventKind, Folder, FolderId, FolderListing, FolderState, Message, MessageId, MessageRef,
    SpecialUse, Summary, Uid,
};
use inboxcraft_store::{Batch, Body, Change, Memory, Storage};
use proptest::prelude::*;
use serde_json::{Map, Value, json};

use super::MODULES;
use super::calendar_tests::{WORK, call, days_off, event, lunch, standup};
use super::compose_tests::validity;
use super::send_tests::store as sending_store;
use crate::arrive::invite::tests::{mail, object};
use crate::test_support::{NOW, address, at, clock, folder, message, sample_session, send_now, session};
use crate::{Caller, EngineError, Registry, Session};

/// Names no command reads.
const STRANGERS: [&str; 4] = ["message", "view", "", "\u{0}"];

/// Every parameter name a module says its commands read, and a few no command reads.
fn fields() -> Vec<&'static str> {
    let mut fields: Vec<&'static str> = MODULES.iter().flat_map(|module| module.fields).copied().collect();
    fields.extend(STRANGERS);
    fields.sort_unstable();
    fields.dedup();
    fields
}

fn junk() -> Vec<Value> {
    let mut deep = json!(1);
    for _ in 0..100 {
        deep = json!([deep]);
    }
    vec![
        json!(null),
        json!(true),
        json!(0),
        json!(1),
        json!(-1),
        json!(u64::MAX),
        json!(i64::MAX.unsigned_abs() + 1),
        json!(i64::MAX),
        json!(i64::MIN),
        json!(1.5),
        json!(1e308),
        json!(""),
        json!("x".repeat(100_000)),
        json!("\r\n\u{0}\u{1b}[2J\u{202e}"),
        json!("inbox"),
        json!("2026-11-05"),
        json!("2026-02-30T25:61:61Z"),
        json!("9999999999-01-01"),
        json!([]),
        json!([1, 2, 3]),
        json!({}),
        json!({ "nested": { "deeper": [null] } }),
        deep,
    ]
}

fn run_everything(session: &mut Session, params: &Value) {
    let ids: Vec<&'static str> = session.registry().specs().map(|spec| spec.id).collect();
    for id in ids {
        for caller in [Caller::Owner, Caller::Agent] {
            let result = session.run_as(caller, id, params);
            assert!(!matches!(result, Err(EngineError::Panicked(_))), "{id} panicked on {params}");
        }
    }
}

#[test]
fn hostile_params_never_panic() {
    for mut session in [session(), sample_session(), super::calendar_tests::session()] {
        for value in junk() {
            run_everything(&mut session, &value);
            for field in fields() {
                let params = Value::Object(Map::from_iter([(field.to_owned(), value.clone())]));
                run_everything(&mut session, &params);
            }
        }
    }
}

fn hostile_value() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(|n| json!(n)),
        any::<u64>().prop_map(|n| json!(n)),
        any::<f64>().prop_map(|n| json!(n)),
        ".{0,40}".prop_map(Value::String),
    ];
    leaf.prop_recursive(4, 32, 6, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..6).prop_map(Value::Array),
            prop::collection::vec((prop::sample::select(fields()), inner), 0..6)
                .prop_map(|pairs| Value::Object(pairs.into_iter().map(|(key, value)| (key.to_owned(), value)).collect())),
        ]
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn random_params_never_panic(params in hostile_value()) {
        for mut session in [session(), sample_session()] {
            run_everything(&mut session, &params);
        }
    }
}

#[test]
fn calendar_writes_with_one_hostile_field_never_panic() {
    let bases = [
        ("event.create", json!({ "calendar": 1, "summary": "A", "start": "2026-11-12T10:00:00Z" })),
        ("event.update", json!({ "id": 1, "summary": "A" })),
        ("event.update", json!({ "id": 4, "start": "2026-11-20" })),
        ("event.list", json!({ "from": "2026-11-05", "to": "2026-11-12" })),
        ("calendar.server", json!({ "account": 1, "url": "https://dav.example.org/" })),
    ];
    let mut session = super::calendar_tests::session();
    for (id, base) in bases {
        for field in fields() {
            for value in junk() {
                let mut params = base.clone();
                if let Value::Object(map) = &mut params {
                    map.insert(field.to_owned(), value.clone());
                }
                let result = session.run(id, &params);
                assert!(!matches!(result, Err(EngineError::Panicked(_))), "{id} panicked on {params}");
            }
        }
    }
}

/// Account 1's folders past the sending store's (INBOX 1, Sent 2, Archive 3, Drafts 6; account 2: INBOX 4, Sent 5).
const TRASH: u64 = 7;
const JUNK: u64 = 8;
const PROJECTS: u64 = 9;
const SCREENER: u64 = 10;
const BLOCKED: u64 = 11;
const SNOOZED: u64 = 12;

/// A message with an inline picture by Content-ID and an attached file.
const ATTACHED: &[u8] = b"From: Anna Example <anna@example.org>\r\nSubject: Budget for 2026\r\n\
Content-Type: multipart/mixed; boundary=outer\r\n\r\n\
--outer\r\nContent-Type: multipart/related; boundary=inner\r\n\r\n\
--inner\r\nContent-Type: text/html; charset=utf-8\r\n\r\n<p>The <b>numbers</b><img src=\"cid:logo@example.org\"></p>\r\n\
--inner\r\nContent-Type: image/png\r\nContent-ID: <logo@example.org>\r\nContent-Transfer-Encoding: base64\r\n\r\niVBORw==\r\n\
--inner--\r\n--outer\r\nContent-Type: text/csv; name=\"rota.csv\"\r\nContent-Disposition: attachment; filename=\"rota.csv\"\r\n\r\n\
week,name\r\n--outer--\r\n";

/// Message `id` of `account` in `folder` with UID `uid`, from `from`, with `envelope`'s subject and links.
fn put(id: u64, account: u64, folder: u64, uid: u32, from: &str, envelope: inboxcraft_model::Envelope) -> Change {
    let envelope = envelope.with_from(vec![address("Someone", from)]).with_to(vec![address("Anna", "anna@example.org")]);
    let summary = Summary::new(Uid::new(uid).unwrap(), envelope).with_received(Some(at(NOW - 600)));
    Change::PutMessage(Message::new(MessageId::new(id), AccountId::new(account), FolderId::new(folder), summary))
}

/// The world's store: the sending store with a CalDAV address, three calendars and four events, a folder of each role
/// with a message in it, a thread of two, a message with attachments and an invitation.
fn world_store() -> Memory {
    let mut store = sending_store();
    let account = store.accounts().unwrap().into_iter().find(|account| account.id() == AccountId::new(1)).unwrap();
    let calendar = |id: u64, owner: u64, href: &str, name: &str| {
        Change::PutCalendar(inboxcraft_model::Calendar {
            id: inboxcraft_model::CalendarId::new(id),
            account: AccountId::new(owner),
            href: href.to_owned(),
            name: name.to_owned(),
            colour: None,
        })
    };
    let noselect = Folder::new(
        FolderId::new(PROJECTS),
        AccountId::new(1),
        FolderListing::new(folder(PROJECTS, 1, "Projects", vec![]).listing().path().clone(), vec![], false),
    );
    let plain = || inboxcraft_model::Envelope::new("Hello");
    let mut batch = Batch::new()
        .with(Change::PutAccount(account.with_caldav(Some("https://dav.example.org/")).unwrap()))
        .with(calendar(1, 1, WORK, "Work"))
        .with(calendar(2, 1, "https://dav.example.org/calendars/anna/home/", "Home"))
        .with(calendar(3, 2, "https://dav.example.net/calendars/bob/default/", "Bob's"))
        .with(Change::PutCalendarEvent(event(1, 1, "standup", Some("\"s1\""), &standup())))
        .with(Change::PutCalendarEvent(event(2, 1, "lunch", Some("\"l1\""), &lunch())))
        .with(Change::PutCalendarEvent(event(3, 1, "call", Some("\"c1\""), &call())))
        .with(Change::PutCalendarEvent(event(4, 2, "off", Some("\"o1\""), &days_off())))
        .with(Change::PutFolder(folder(TRASH, 1, "Trash", vec![SpecialUse::Trash])))
        .with(Change::PutFolder(folder(JUNK, 1, "Junk", vec![SpecialUse::Junk])))
        .with(Change::PutFolder(noselect))
        .with(Change::PutFolder(folder(SCREENER, 1, "InboxCraft/Screener", vec![])))
        .with(Change::PutFolder(folder(BLOCKED, 1, "InboxCraft/Blocked", vec![])))
        .with(Change::PutFolder(folder(SNOOZED, 1, "InboxCraft/Snoozed", vec![])))
        .with(put(20, 1, 3, 1, "anna@example.org", plain()))
        .with(put(21, 1, 6, 1, "anna@example.org", plain()))
        .with(put(22, 1, TRASH, 1, "carol@example.org", plain()))
        .with(put(23, 1, JUNK, 1, "spam@example.org", plain()))
        .with(put(24, 1, SCREENER, 1, "erin@example.org", plain()))
        .with(put(25, 1, BLOCKED, 1, "frank@example.org", plain()))
        .with(put(26, 1, SNOOZED, 1, "dave@example.org", plain()))
        .with(put(27, 2, 5, 1, "bob@example.net", plain()))
        .with(put(28, 1, 1, 30, "odile@example.org", plain().with_message_id(MessageRef::new("plans@example.org"))))
        .with(put(29, 1, 2, 30, "anna@example.org", plain().with_in_reply_to(vec![MessageRef::new("plans@example.org").unwrap()])))
        .with(Change::PutMessage(message(30, 1, 1, 31, "Invitation: Lunch with Odile", Some(NOW - 300))))
        .with(Change::PutBody(MessageId::new(1), Body::new(ATTACHED.to_vec(), "The numbers").unwrap()))
        .with(Change::PutBody(
            MessageId::new(30),
            Body::new(
                mail("Invitation: Lunch with Odile", "lunch@example.org", &object("REQUEST", "lunch-1", 0, "20261107T113000Z", "")),
                "Odile invites you.",
            )
            .unwrap(),
        ));
    for id in [1, 2, 3, 6, TRASH, JUNK, SCREENER, BLOCKED, SNOOZED] {
        batch.push(Change::SetFolderState(FolderId::new(id), FolderState::new(validity())));
    }
    store.apply(&batch).unwrap();
    store
}

/// Runs `command` with `params` for the owner, which must answer; its answer.
fn must(session: &mut Session, command: &str, params: &Value) -> Value {
    session.run(command, params).unwrap_or_else(|error| panic!("{command} {params}: {error:?}"))
}

/// A draft of account 1 to Béla; its id.
fn draft(session: &mut Session, subject: &str) -> u64 {
    let id = must(session, "compose.new", &json!({ "account": 1 }))["id"].as_u64().unwrap();
    must(session, "compose.update", &json!({ "id": id, "to": ["Béla <bela@example.org>"], "subject": subject }));
    id
}

/// The world the sweep runs in: [`world_store`], and through the commands a draft, a reply, a refused send, a send in
/// its hold, a send later, a snooze, a follow-up, a pin, a parked message, a template, a sender allowed remote
/// pictures, a blocked sender and a sender's group, with the screener inside the inbox. Every job it makes is taken and
/// dropped: no worker is ever started.
pub(super) fn world() -> Session {
    let clock = clock();
    let mut session = Session::with_registry(Box::new(world_store()), Box::new(clock.clone()), Registry::builtin().unwrap()).unwrap();
    session.apply(&Event::new(AccountId::new(1), None, EventKind::Abilities { uid_expunge: true })).unwrap();
    must(&mut session, "screener.mode", &json!({ "mode": "inside" }));
    draft(&mut session, "Plain");
    must(&mut session, "compose.reply", &json!({ "id": 28 }));
    let refused = draft(&mut session, "Refused");
    send_now(&mut session, &clock, "mail.send", refused);
    let kind =
        EventKind::SendFailed { draft: DraftId::new(refused), kind: ErrorKind::MessageRefused, reply: Some("550 5.1.1 No such user".to_owned()) };
    session.apply(&Event::new(AccountId::new(1), None, kind)).unwrap();
    let held = draft(&mut session, "Held");
    must(&mut session, "mail.send", &json!({ "id": held }));
    let later = draft(&mut session, "Later");
    must(&mut session, "mail.send_later", &json!({ "id": later, "preset": "tomorrow_morning" }));
    must(&mut session, "mail.snooze", &json!({ "id": 2, "until": "2026-11-06T09:00:00Z" }));
    must(&mut session, "mail.follow_up", &json!({ "id": 5, "until": "2026-11-07T09:00:00Z" }));
    must(&mut session, "mail.pin", &json!({ "id": 1, "pinned": true }));
    must(&mut session, "mail.park", &json!({ "id": 28, "parked": true }));
    must(&mut session, "template.save", &json!({ "account": 1, "name": "Thanks", "subject": "Thanks", "text": "Thank you." }));
    must(&mut session, "remote.allow", &json!({ "address": "odile@example.org" }));
    must(&mut session, "screener.block", &json!({ "address": "frank@example.org" }));
    must(&mut session, "group.set", &json!({ "address": "carol@example.org", "group": "updates" }));
    drop(session.take_jobs());
    session
}

/// The kinds of thing a parameter names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Account,
    Folder,
    Calendar,
    Message,
    Draft,
    Outgoing,
    Scheduled,
    Event,
}

const KINDS: [Kind; 8] = [Kind::Account, Kind::Folder, Kind::Calendar, Kind::Message, Kind::Draft, Kind::Outgoing, Kind::Scheduled, Kind::Event];

/// The kinds `field` names; none for a parameter that names no thing.
fn kinds(field: &str) -> &'static [Kind] {
    match field {
        "account" => &[Kind::Account],
        "folder" => &[Kind::Folder],
        "calendar" => &[Kind::Calendar],
        "answers" => &[Kind::Message],
        "id" => &[Kind::Message, Kind::Draft, Kind::Outgoing, Kind::Scheduled, Kind::Event],
        _ => &[],
    }
}

/// The ids of every thing of `kind` the session's store holds.
fn held(session: &Session, kind: Kind) -> BTreeSet<u64> {
    let store = session.store();
    let accounts: Vec<AccountId> = store.accounts().unwrap().iter().map(inboxcraft_model::Account::id).collect();
    let folders = || accounts.iter().flat_map(|account| store.folders(*account).unwrap()).map(|folder| folder.id());
    let calendars = || accounts.iter().flat_map(|account| store.calendars(*account).unwrap()).map(|calendar| calendar.id);
    match kind {
        Kind::Account => accounts.iter().map(|account| account.get()).collect(),
        Kind::Folder => folders().map(FolderId::get).collect(),
        Kind::Calendar => calendars().map(|calendar| calendar.get()).collect(),
        Kind::Message => folders().flat_map(|folder| store.messages(folder).unwrap()).map(|entry| entry.message().id().get()).collect(),
        Kind::Draft => accounts.iter().flat_map(|account| store.drafts(*account).unwrap()).map(|draft| draft.id.get()).collect(),
        Kind::Outgoing => store.outbox().unwrap().iter().map(|item| item.draft.get()).collect(),
        Kind::Scheduled => store.schedule().unwrap().iter().map(|entry| entry.id.get()).collect(),
        Kind::Event => calendars().flat_map(|calendar| store.calendar_events(calendar).unwrap()).map(|event| event.id.get()).collect(),
    }
}

/// Whether the session still holds `value` as a thing of `kind`.
fn holds(session: &Session, kind: Kind, value: u64) -> bool {
    let store = session.store();
    match kind {
        Kind::Message => store.message(MessageId::new(value)).unwrap().is_some(),
        Kind::Draft => store.draft(DraftId::new(value)).unwrap().is_some(),
        Kind::Event => store.calendar_event(CalendarEventId::new(value)).unwrap().is_some(),
        _ => held(session, kind).contains(&value),
    }
}

/// A parameter naming a thing that exists: `field` set to `value`, a thing of each of `kinds`.
#[derive(Clone, Debug)]
struct Named {
    field: &'static str,
    value: u64,
    kinds: Vec<Kind>,
}

impl Named {
    fn holds(&self, session: &Session) -> bool {
        self.kinds.iter().all(|kind| holds(session, *kind, self.value))
    }
}

/// Every parameter naming a thing `session` holds, of every field that names things, in a fixed order.
fn named(session: &Session) -> Vec<Named> {
    let mut named = Vec::new();
    for field in ["account", "answers", "calendar", "folder", "id"] {
        let mut values: Vec<(u64, Vec<Kind>)> = Vec::new();
        for kind in kinds(field) {
            for value in held(session, *kind) {
                match values.iter_mut().find(|(seen, _)| *seen == value) {
                    Some((_, kinds)) => kinds.push(*kind),
                    None => values.push((value, vec![*kind])),
                }
            }
        }
        values.sort_by_key(|(value, _)| *value);
        named.extend(values.into_iter().map(|(value, kinds)| Named { field, value, kinds }));
    }
    named
}

/// Values that mean something to a command, for the parameters beside a named thing.
fn meaningful() -> Vec<Value> {
    [
        "anna@example.org",
        "odile@example.org",
        "frank@example.org",
        "Thanks",
        "logo@example.org",
        "2026-11-06T09:00:00Z",
        "tomorrow_morning",
        "accepted",
        "updates",
        "inside",
    ]
    .into_iter()
    .map(|text| json!(text))
    .chain([json!(2), json!(3), json!(TRASH), json!(SNOOZED), json!(false)])
    .collect()
}

/// A session that is built again from `build` before any run whose named things it no longer holds.
struct Sweep {
    build: fn() -> Session,
    session: Session,
    runs: usize,
    builds: usize,
}

impl Sweep {
    fn new(build: fn() -> Session) -> Self {
        Self { build, session: build(), runs: 0, builds: 1 }
    }

    /// Runs each of `commands` with `params`, for an agent and for the owner, on a session that holds every `named`.
    fn run(&mut self, commands: &[&'static str], params: &Value, named: &[&Named]) {
        for command in commands {
            for caller in [Caller::Agent, Caller::Owner] {
                if !named.iter().all(|named| named.holds(&self.session)) {
                    self.session = (self.build)();
                    self.builds += 1;
                    assert!(named.iter().all(|named| named.holds(&self.session)), "the world lacks {named:?}");
                }
                let result = self.session.run_as(caller, command, params);
                assert!(!matches!(result, Err(EngineError::Panicked(_))), "{command} panicked on {params} as {caller:?}");
                // The jobs are dropped here: the engine does no I/O and no worker runs in a test.
                drop(self.session.take_jobs());
                self.runs += 1;
            }
        }
    }
}

/// Every command, on every thing that exists, with the junk and the meaningful values in each other parameter its
/// module reads, and with a second thing that exists.
fn sweep_things_that_exist(build: fn() -> Session) -> Sweep {
    let mut sweep = Sweep::new(build);
    let things = named(&sweep.session);
    let every: Vec<&'static str> = sweep.session.registry().specs().map(|spec| spec.id).collect();
    let others: Vec<Value> = junk().into_iter().chain(meaningful()).collect();
    for thing in &things {
        sweep.run(&every, &json!({ thing.field: thing.value }), &[thing]);
    }
    for module in MODULES {
        let commands: Vec<&'static str> = module.commands.iter().map(|spec| spec.id).collect();
        let mine: Vec<&Named> = things.iter().filter(|thing| module.fields.contains(&thing.field)).collect();
        for thing in &mine {
            for field in module.fields.iter().filter(|field| **field != thing.field) {
                for value in &others {
                    let params = Value::Object(Map::from_iter([(thing.field.to_owned(), json!(thing.value)), ((*field).to_owned(), value.clone())]));
                    sweep.run(&commands, &params, &[thing]);
                }
            }
            for second in mine.iter().filter(|second| second.field > thing.field) {
                let params = json!({ thing.field: thing.value, second.field: second.value });
                sweep.run(&commands, &params, &[thing, second]);
            }
        }
    }
    sweep
}

#[test]
fn the_world_holds_a_thing_of_every_kind() {
    let mut session = world();
    for kind in KINDS {
        assert!(!held(&session, kind).is_empty(), "{kind:?}");
    }
    must(&mut session, "invite.show", &json!({ "id": 30 }));
    must(&mut session, "mail.attachment", &json!({ "id": 1, "index": 0 }));
    assert_eq!(must(&mut session, "mail.thread", &json!({ "id": 28 }))["messages"].as_array().map(Vec::len), Some(2));
    let states: Vec<Value> =
        must(&mut session, "outbox.list", &json!({}))["items"].as_array().unwrap().iter().map(|item| item["state"].clone()).collect();
    assert!(states.contains(&json!("refused")), "{states:?}");
}

#[test]
fn commands_on_things_that_exist_never_panic() {
    for build in [world as fn() -> Session, sample_session] {
        let sweep = sweep_things_that_exist(build);
        assert!(sweep.runs > 0 && sweep.builds > 1, "{} runs, {} builds", sweep.runs, sweep.builds);
        println!("{} runs, {} builds", sweep.runs, sweep.builds);
    }
}
