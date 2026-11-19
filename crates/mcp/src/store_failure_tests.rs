// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A tool whose store really fails answers `error store:` and the failure's fixed sentence, marked `isError`: none of
//! the database's own words (a table or column name, `sqlite`, a path).

use std::path::Path;

use inboxcraft_testkit::{store, store_fault};
use serde_json::{Value, json};

use crate::test_support::{call, fixture_server, text};

/// Makes `fault` in a fixture store under a running server, calls `tool` with `arguments`, and returns its text.
fn failed(fault: fn(&Path), tool: &str, arguments: Value) -> String {
    let (dir, mut server) = fixture_server();
    let mut words = store_fault::schema_words(dir.path()).unwrap();
    fault(dir.path());
    words.extend(store_fault::schema_words(dir.path()).unwrap_or_default());
    let (out, error) = text(&call(&mut server, tool, arguments));
    assert!(error, "{out}");
    assert_eq!(store_fault::leaks(&out, &words, dir.path()), Vec::<String>::new(), "{out}");
    out
}

#[test]
fn every_kind_of_failure_answers_its_own_sentence() {
    let list = || json!({ "folder": store::INBOX });
    let read = || json!({ "id": store::LUNCH });
    assert_eq!(
        failed(|dir| store_fault::drop_table(dir, "body").unwrap(), "mail_list", list()),
        "error store: the store could not be read or written"
    );
    assert_eq!(failed(|dir| store_fault::damage(dir, "message").unwrap(), "mail_list", list()), "error store: the store is damaged");
    assert_eq!(failed(|dir| store_fault::refuse_updates(dir, "message").unwrap(), "mail_read", read()), "error store: the store refused the change");
}

#[test]
fn a_held_store_is_busy() {
    let (dir, mut server) = fixture_server();
    let words = store_fault::schema_words(dir.path()).unwrap();
    let held = store_fault::hold(dir.path()).unwrap();
    let (out, error) = text(&call(&mut server, "mail_read", json!({ "id": store::LUNCH })));
    assert!(error, "{out}");
    assert_eq!(out, "error store: the store is busy, try again");
    assert_eq!(store_fault::leaks(&out, &words, dir.path()), Vec::<String>::new());
    held.release().unwrap();
}
