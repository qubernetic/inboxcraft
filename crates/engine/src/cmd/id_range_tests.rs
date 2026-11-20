// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! An id past `i64::MAX`, the largest key SQLite takes, names nothing: every command that takes an id of any kind
//! answers it as it answers an id that is simply unknown, on the SQLite store the app runs on. Never "the store
//! database failed". A write changes nothing and emits no job.

use inboxcraft_model::{AccountId, CalendarEventId, FolderId};
use inboxcraft_store::{Batch, Change, Sqlite, Storage, StoreConfig};
use serde_json::{Value, json};
use tempfile::TempDir;

use super::hostile_tests::world;
use crate::test_support::clock;
use crate::{Caller, Registry, Session};

/// `u64::MAX`, the first id past `i64::MAX`, and `i64::MAX` itself, which is in range and simply unknown.
const PAST: [u64; 3] = [u64::MAX, i64::MAX.unsigned_abs() + 1, i64::MAX.unsigned_abs()];

/// An id in range that no thing of any kind holds in the world.
const UNKNOWN: u64 = 999_999;

/// `from`'s records, put into `to` in the order their links need.
fn copy(from: &dyn Storage, to: &mut Sqlite) {
    let accounts = from.accounts().unwrap();
    let mut batch = Batch::new();
    let mut later = Batch::new();
    for account in &accounts {
        batch.push(Change::PutAccount(account.clone()));
    }
    for account in &accounts {
        for folder in from.folders(account.id()).unwrap() {
            batch.push(Change::PutFolder(folder.clone()));
            if let Some(state) = from.folder_state(folder.id()).unwrap() {
                batch.push(Change::SetFolderState(folder.id(), state));
            }
            for entry in from.messages(folder.id()).unwrap() {
                let id = entry.message().id();
                later.push(Change::PutMessage(entry.message().clone()));
                if let Some(body) = from.body(id).unwrap() {
                    later.push(Change::PutBody(id, body));
                }
                if entry.opened() {
                    later.push(Change::MarkOpened(id));
                }
            }
        }
    }
    to.apply(&batch).unwrap();
    to.apply(&later).unwrap();
    let mut batch = Batch::new();
    for account in &accounts {
        for draft in from.drafts(account.id()).unwrap() {
            batch.push(Change::PutDraft(draft));
        }
        for calendar in from.calendars(account.id()).unwrap() {
            let events = from.calendar_events(calendar.id).unwrap();
            batch.push(Change::PutCalendar(calendar));
            for event in events {
                let alerts = from.alerts(event.id).unwrap();
                batch.push(Change::PutCalendarEvent(event));
                for alert in alerts {
                    batch.push(Change::PutAlert(alert));
                }
            }
        }
    }
    for item in from.outbox().unwrap() {
        batch.push(Change::PutOutgoing(item));
    }
    for entry in from.schedule().unwrap() {
        batch.push(Change::PutScheduled(entry));
    }
    for (message, state) in from.message_states().unwrap() {
        batch.push(Change::SetMessageState(message, state));
    }
    for sender in from.senders().unwrap() {
        batch.push(Change::PutSender(sender));
    }
    for job in from.queued_jobs().unwrap() {
        batch.push(Change::QueueJob(job));
    }
    to.apply(&batch).unwrap();
}

/// The hostile sweep's world, on SQLite: every kind of thing a command names exists in it.
fn sqlite_world() -> (Session, TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Sqlite::open(dir.path(), StoreConfig::default()).unwrap();
    copy(world().store(), &mut store);
    let session = Session::with_registry(Box::new(store), Box::new(clock()), Registry::builtin().unwrap()).unwrap();
    (session, dir)
}

/// Everything the store holds, written out, to see that nothing changed.
fn everything(store: &dyn Storage) -> String {
    let mut all =
        format!("{:?} {:?} {:?} {:?} {:?}", store.accounts(), store.outbox(), store.schedule(), store.message_states(), store.queued_jobs());
    for account in store.accounts().unwrap() {
        all.push_str(&format!(" {:?} {:?}", store.drafts(account.id()), store.calendars(account.id())));
        for folder in store.folders(account.id()).unwrap() {
            all.push_str(&format!(" {:?} {:?} {:?}", folder, store.folder_state(folder.id()), store.messages(folder.id())));
        }
        for calendar in store.calendars(account.id()).unwrap() {
            for event in store.calendar_events(calendar.id).unwrap() {
                all.push_str(&format!(" {event:?} {:?}", store.alerts(event.id)));
            }
        }
    }
    all
}

/// `answer` with the id `id` written as [`UNKNOWN`], so it can be set beside the answer for an unknown id.
fn as_unknown(answer: &Result<Value, crate::EngineError>, id: u64) -> String {
    let written = match answer {
        Ok(value) => format!("ok {value}"),
        Err(error) => format!("{} {error}", error.code()),
    };
    written.replace(&id.to_string(), &UNKNOWN.to_string())
}

/// Commands that name a thing of each kind, with `$`'s place taken by the id; reads and writes alike.
fn commands() -> Vec<(&'static str, Value)> {
    let until = "2026-11-06T09:00:00Z";
    vec![
        // A message, and the thread it is in.
        ("mail.read", json!({ "id": "$" })),
        ("mail.why", json!({ "id": "$" })),
        ("mail.row", json!({ "id": "$" })),
        ("mail.thread", json!({ "id": "$" })),
        ("mail.attachment", json!({ "id": "$", "index": 0 })),
        ("invite.show", json!({ "id": "$" })),
        ("compose.reply", json!({ "id": "$" })),
        ("mail.flag", json!({ "id": "$", "flagged": true })),
        ("mail.move", json!({ "id": "$", "folder": 3 })),
        ("mail.delete", json!({ "id": "$" })),
        ("mail.snooze", json!({ "id": "$", "until": until })),
        ("mail.pin", json!({ "id": "$", "pinned": true })),
        // A folder.
        ("mail.list", json!({ "folder": "$" })),
        ("mail.search", json!({ "folder": "$", "text": "Hello" })),
        ("mail.move", json!({ "id": 1, "folder": "$" })),
        // An account.
        ("folder.list", json!({ "account": "$" })),
        ("compose.new", json!({ "account": "$" })),
        ("calendar.list", json!({ "account": "$" })),
        ("account.remove", json!({ "account": "$" })),
        // A draft, and an outbox item, which is named by its draft.
        ("compose.show", json!({ "id": "$" })),
        ("compose.update", json!({ "id": "$", "subject": "Far" })),
        ("compose.discard", json!({ "id": "$" })),
        ("mail.send", json!({ "id": "$" })),
        ("outbox.retry", json!({ "id": "$" })),
        // A schedule entry.
        ("schedule.cancel", json!({ "id": "$" })),
        // A calendar, and an event.
        ("event.create", json!({ "calendar": "$", "summary": "Far", "start": "2026-11-12T10:00:00Z" })),
        ("event.update", json!({ "id": "$", "summary": "Far" })),
        ("event.delete", json!({ "id": "$" })),
    ]
}

/// `params` with `"$"` set to `id`.
fn with_id(params: &Value, id: u64) -> Value {
    let Value::Object(map) = params else { return params.clone() };
    Value::Object(map.iter().map(|(name, value)| (name.clone(), if value == "$" { json!(id) } else { value.clone() })).collect())
}

#[test]
fn the_unknown_id_names_nothing_in_the_world() {
    let (session, _dir) = sqlite_world();
    let store = session.store();
    assert!(store.message(inboxcraft_model::MessageId::new(UNKNOWN)).unwrap().is_none());
    assert!(store.folder_state(FolderId::new(UNKNOWN)).unwrap().is_none());
    assert!(store.folders(AccountId::new(UNKNOWN)).unwrap().is_empty());
    assert!(store.calendar_event(CalendarEventId::new(UNKNOWN)).unwrap().is_none());
    assert!(store.outbox().unwrap().iter().all(|item| item.draft.get() != UNKNOWN));
    assert!(store.schedule().unwrap().iter().all(|entry| entry.id.get() != UNKNOWN));
    assert!(!everything(store).contains(&UNKNOWN.to_string()));
}

#[test]
fn an_id_past_the_largest_key_is_answered_as_an_unknown_id() {
    let (mut session, _dir) = sqlite_world();
    let before = everything(session.store());
    for (command, params) in commands() {
        let unknown = session.run_as(Caller::Owner, command, &with_id(&params, UNKNOWN));
        assert!(session.take_jobs().is_empty(), "{command} on an unknown id emitted a job");
        assert!(unknown.is_err(), "{command} answered an unknown id: {unknown:?}");
        for id in PAST {
            let answer = session.run_as(Caller::Owner, command, &with_id(&params, id));
            let written = as_unknown(&answer, id);
            assert!(!written.contains("database"), "{command} {id}: {written}");
            assert_eq!(written, as_unknown(&unknown, UNKNOWN), "{command} {id}");
            assert!(session.take_jobs().is_empty(), "{command} {id} emitted a job");
        }
        assert_eq!(everything(session.store()), before, "{command} changed the store");
    }
}

#[test]
fn an_id_past_the_largest_key_is_not_found_for_an_agent_too() {
    let (mut session, _dir) = sqlite_world();
    for id in PAST {
        for command in ["mail.read", "mail.why", "mail.thread"] {
            let error = session.run_as(Caller::Agent, command, &json!({ "id": id })).unwrap_err();
            assert_eq!((error.code(), error.to_string()), ("not_found", format!("there is no message {id}")), "{command}");
        }
    }
}

/// The parameters that name a thing of some kind, as the hostile sweep reads them.
const ID_FIELDS: [&str; 5] = ["id", "account", "folder", "calendar", "answers"];

/// A value that a command can take for `field` when `field` is not the one under test; `None` for the fields of no
/// use here.
fn filler(field: &str) -> Option<Value> {
    Some(match field {
        "id" | "account" | "calendar" => json!(1),
        "folder" => json!(3),
        "answers" => json!(28),
        "flagged" | "seen" | "done" | "parked" | "pinned" | "keep_on_done" | "all_day" | "grouped" => json!(true),
        "from" => json!("2026-11-05"),
        "to" => json!("2026-11-12"),
        "level" => json!("people"),
        "index" => json!(0),
        "summary" | "subject" | "label" => json!("Far"),
        "text" => json!("Hello"),
        "name" => json!("Thanks"),
        "start" => json!("2026-11-12T10:00:00Z"),
        "until" => json!("2026-11-06T09:00:00Z"),
        "at" => json!("2026-11-20T09:00:00Z"),
        "preset" => json!("tomorrow_morning"),
        "answer" => json!("accepted"),
        "group" => json!("updates"),
        "address" => json!("anna@example.org"),
        "content_id" => json!("logo@example.org"),
        "url" => json!("https://dav.example.org/"),
        _ => return None,
    })
}

/// The parameters to try `command` with: `field` set to `id`, alone and with one or two of the module's other
/// fields filled, so that a command which needs more than the id reaches its lookup.
fn tries(fields: &[&'static str], field: &str, id: u64) -> Vec<Value> {
    let others: Vec<(&str, Value)> =
        fields.iter().filter(|other| **other != field).filter_map(|other| filler(other).map(|value| (*other, value))).collect();
    let mut tries = vec![json!({ field: id })];
    for (at, (first, one)) in others.iter().enumerate() {
        tries.push(json!({ field: id, *first: one }));
        for (second, two) in others.iter().skip(at + 1) {
            tries.push(json!({ field: id, *first: one, *second: two }));
        }
    }
    tries
}

/// Every command whose module reads a parameter that names a thing, with that parameter set to each far id (alone and
/// beside one or two other parameters filled), answers as it answers the unknown id, on the SQLite store, and emits as
/// many jobs. A new command joins by itself.
#[test]
fn every_command_that_reads_an_id_answers_a_far_id_as_an_unknown_one() {
    let (mut session, _dir) = sqlite_world();
    let (mut runs, mut reached) = (0_usize, std::collections::BTreeSet::new());
    let mut stopped = std::collections::BTreeSet::new();
    for module in super::MODULES {
        for field in module.fields.iter().filter(|field| ID_FIELDS.contains(field)) {
            for spec in module.commands {
                let mut any = false;
                for (at, params) in tries(module.fields, field, UNKNOWN).into_iter().enumerate() {
                    let unknown = session.run_as(Caller::Owner, spec.id, &params);
                    let jobs = session.take_jobs().len();
                    if matches!(&unknown, Err(error) if error.code() == "bad_params") {
                        continue;
                    }
                    any = true;
                    for id in PAST {
                        let far = tries(module.fields, field, id).swap_remove(at);
                        let answer = session.run_as(Caller::Owner, spec.id, &far);
                        let written = as_unknown(&answer, id);
                        assert!(!written.contains("database"), "{} {far}: {written}", spec.id);
                        assert_eq!(written, as_unknown(&unknown, UNKNOWN), "{} {far}", spec.id);
                        assert_eq!(session.take_jobs().len(), jobs, "{} {far}", spec.id);
                        runs += 1;
                    }
                }
                if any {
                    reached.insert((spec.id, *field))
                } else {
                    stopped.insert((spec.id, *field))
                };
            }
        }
    }
    println!("{runs} runs; reached {reached:?}; not {stopped:?}");
    assert!(reached.len() >= 64, "{} reached: {reached:?}", reached.len());
}
