// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A command whose store really fails answers the failure's fixed sentence under the code `store`, for the owner and
//! for an agent: no table, column, `sqlite` or path of the database's own words. The words stay in the error's source.

use std::path::Path;

use inboxcraft_store::{Sqlite, StoreConfig, StoreError};
use inboxcraft_testkit::{store, store_fault};
use inboxcraft_time::FakeClock;
use serde_json::{Value, json};

use crate::{Caller, EngineError, Session};

fn session_on(dir: &Path) -> Session {
    store::fixture(dir).unwrap();
    let opened = Sqlite::open(dir, StoreConfig::default()).unwrap();
    Session::new(Box::new(opened), Box::new(FakeClock::new(store::NOW))).unwrap()
}

/// Makes `fault` in a fixture store, runs `id` with `params` as `caller`, and returns the failure, checked clean.
fn failure(fault: fn(&Path), caller: Caller, id: &str, params: &Value) -> EngineError {
    let tmp = tempfile::tempdir().unwrap();
    let mut session = session_on(tmp.path());
    // The names before the fault, and after it those it added (a damaged schema does not read back).
    let mut words = store_fault::schema_words(tmp.path()).unwrap();
    fault(tmp.path());
    words.extend(store_fault::schema_words(tmp.path()).unwrap_or_default());
    let error = session.run_as(caller, id, params).unwrap_err();
    assert_eq!(error.code(), "store", "{error:?}");
    let detail = std::error::Error::source(&error).and_then(|source| source.downcast_ref::<StoreError>()).and_then(StoreError::detail);
    assert!(detail.is_some_and(|detail| !detail.is_empty()), "the detail is kept: {error:?}");
    let text = error.to_string();
    assert_eq!(store_fault::leaks(&text, &words, tmp.path()), Vec::<String>::new(), "{text}");
    error
}

fn read_lunch() -> Value {
    json!({ "id": store::LUNCH })
}

#[test]
fn a_store_failure_of_no_kind_of_its_own_is_one_sentence() {
    for caller in [Caller::Owner, Caller::Agent] {
        let error = failure(|dir| store_fault::drop_table(dir, "body").unwrap(), caller, "mail.list", &json!({ "folder": store::INBOX }));
        assert!(matches!(error, EngineError::Store(StoreError::Database(_))), "{error:?}");
        assert_eq!(error.to_string(), "the store could not be read or written");
    }
}

#[test]
fn a_damaged_store_is_one_sentence() {
    for caller in [Caller::Owner, Caller::Agent] {
        let error = failure(|dir| store_fault::damage(dir, "message").unwrap(), caller, "mail.list", &json!({ "folder": store::INBOX }));
        assert!(matches!(error, EngineError::Store(StoreError::DatabaseDamaged(_))), "{error:?}");
        assert_eq!(error.to_string(), "the store is damaged");
    }
}

#[test]
fn a_refused_change_is_one_sentence_without_the_refusal_s_words() {
    for caller in [Caller::Owner, Caller::Agent] {
        let error = failure(|dir| store_fault::refuse_updates(dir, "message").unwrap(), caller, "mail.read", &read_lunch());
        assert!(matches!(error, EngineError::Store(StoreError::DatabaseRefused(_))), "{error:?}");
        assert_eq!(error.to_string(), "the store refused the change");
        assert!(!error.to_string().contains(store_fault::REFUSAL));
    }
}

#[test]
fn a_busy_store_is_one_sentence() {
    let tmp = tempfile::tempdir().unwrap();
    let mut session = session_on(tmp.path());
    let words = store_fault::schema_words(tmp.path()).unwrap();
    let held = store_fault::hold(tmp.path()).unwrap();
    let error = session.run_as(Caller::Agent, "mail.read", &read_lunch()).unwrap_err();
    assert!(matches!(error, EngineError::Store(StoreError::DatabaseBusy(_))), "{error:?}");
    assert_eq!((error.code(), error.to_string().as_str()), ("store", "the store is busy, try again"));
    assert_eq!(store_fault::leaks(&error.to_string(), &words, tmp.path()), Vec::<String>::new());
    held.release().unwrap();
    assert!(session.run_as(Caller::Agent, "mail.read", &read_lunch()).is_ok(), "once let go, it runs");
}

#[test]
fn a_full_store_and_a_failed_migration_are_one_sentence_each() {
    // Neither can be made under a running command without filling a disk; the engine passes them on as it does the rest.
    for (error, text) in [
        (StoreError::DatabaseFull("database or disk is full".to_owned()), "the store is full: there is no room left on the disk"),
        (StoreError::Migration("table meta already exists".to_owned()), "the store could not be brought up to this version"),
    ] {
        let error = EngineError::from(error);
        assert_eq!((error.code(), error.to_string().as_str()), ("store", text));
    }
}
