// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Commands by id on the `testkit` fixture store, headless, as an agent runs them.

use std::ffi::OsString;
use std::path::Path;

use inboxcraft_model::AccountId;
use inboxcraft_store::{Sqlite, Storage, StoreConfig};
use inboxcraft_testkit::store;
use serde_json::{Value, json};
use tempfile::TempDir;

fn fixture() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    store::fixture(dir.path()).unwrap();
    dir
}

fn cli(args: &[&str]) -> (u8, String, String) {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = inboxcraft_cli::run(args.iter().map(OsString::from), &mut out, &mut err);
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

fn on(dir: &Path, args: &[&str]) -> (u8, String, String) {
    let dir = dir.to_str().unwrap();
    let mut all = vec!["--store", dir];
    all.extend_from_slice(args);
    cli(&all)
}

fn json(dir: &Path, args: &[&str]) -> Value {
    let mut all = vec!["--json"];
    all.extend_from_slice(args);
    let (code, out, err) = on(dir, &all);
    assert_eq!((code, err.as_str()), (0, ""), "{out}");
    assert_eq!(out.matches('\n').count(), 1, "one line: {out}");
    serde_json::from_str(&out).unwrap()
}

#[test]
fn a_safe_command_runs_by_id_and_answers_in_json() {
    let dir = fixture();
    let accounts = json(dir.path(), &["account.list"]);
    assert_eq!(accounts["accounts"][0]["address"], "fixture@example.org");
    let inbox = json(dir.path(), &["mail.list", r#"{"folder":1}"#]);
    assert_eq!(inbox["total"], 2);
    assert_eq!(inbox["messages"][0]["subject"], "Lunch on Thursday");
}

#[test]
fn parameters_default_to_an_empty_object() {
    let dir = fixture();
    let inbox = json(dir.path(), &["mail.list"]);
    assert_eq!(inbox["view"], json!({ "role": "inbox" }));
}

#[test]
fn every_safe_read_command_runs_on_the_fixture() {
    let dir = fixture();
    let lunch = store::LUNCH.to_string();
    let read = format!(r#"{{"id":{lunch}}}"#);
    for (id, params) in [
        ("account.list", "{}"),
        ("folder.list", "{}"),
        ("mail.list", r#"{"role":"sent"}"#),
        ("mail.read", read.as_str()),
        ("mail.thread", read.as_str()),
        ("mail.search", r#"{"text":"allotment"}"#),
        ("remote.list", "{}"),
    ] {
        let answer = json(dir.path(), &[id, params]);
        assert!(answer.is_object(), "{id}: {answer}");
    }
    let found = json(dir.path(), &["mail.search", r#"{"text":"allotment"}"#]);
    assert_eq!(found["messages"].as_array().map(Vec::len), Some(1));
    let read = json(dir.path(), &["mail.read", &read]);
    assert!(read["message"]["body"]["text"].as_str().unwrap().contains("allotment"));
}

#[test]
fn a_change_a_safe_command_makes_is_kept_in_the_store() {
    let dir = fixture();
    let read = format!(r#"{{"id":{}}}"#, store::LUNCH);
    assert_eq!(json(dir.path(), &["mail.list"])["messages"][0]["opened"], false);
    json(dir.path(), &["mail.read", &read]);
    assert_eq!(json(dir.path(), &["mail.list"])["messages"][0]["opened"], true);
}

#[test]
fn text_output_is_one_field_a_line() {
    let dir = fixture();
    let (code, out, err) = on(dir.path(), &["account.list"]);
    assert_eq!((code, err.as_str()), (0, ""));
    assert!(out.contains("address: fixture@example.org\n"), "{out}");
    assert!(serde_json::from_str::<Value>(&out).is_err(), "text, not JSON: {out}");
    let (_, out, _) = on(dir.path(), &["mail.read", &format!(r#"{{"id":{}}}"#, store::LUNCH)]);
    assert!(out.contains("Shall we meet at the allotment cafe at noon on Thursday?\n"), "{out}");
}

#[test]
fn an_owner_command_is_refused_headless_and_changes_nothing() {
    let dir = fixture();
    let remove = format!(r#"{{"account":{}}}"#, store::ACCOUNT);
    let (code, out, err) = on(dir.path(), &["account.remove", &remove]);
    assert_eq!(code, 1);
    assert_eq!(out, "");
    assert!(err.contains("account.remove is owner"), "{err}");
    assert!(err.contains("headless"), "{err}");
    let (code, out, err) = on(dir.path(), &["--json", "account.remove", &remove]);
    assert_eq!((code, out.as_str()), (1, ""));
    let error: Value = serde_json::from_str(&err).unwrap();
    assert_eq!(error["error"]["code"], "not_allowed");
    let kept = Sqlite::open(dir.path(), StoreConfig::default()).unwrap();
    assert_eq!(kept.folders(AccountId::new(store::ACCOUNT)).unwrap().len(), 3);
}

#[test]
fn a_failed_command_is_one_json_error_object_and_a_non_zero_exit() {
    let dir = fixture();
    for (args, code, kind) in [
        (&["mail.read", r#"{"id":999}"#][..], 1, "not_found"),
        (&["mail.list", r#"{"typo":1}"#][..], 1, "bad_params"),
        (&["mail.nothing"][..], 1, "unknown_command"),
        (&["sample.mail.set", r#"{"id":1}"#][..], 1, "sample_only"),
        (&["mail.list", "{not json"][..], 2, "usage"),
        (&["mail.list", "{}", "extra"][..], 2, "usage"),
    ] {
        let mut all = vec!["--json"];
        all.extend_from_slice(args);
        let (got, out, err) = on(dir.path(), &all);
        assert_eq!((got, out.as_str()), (code, ""), "{args:?}: {err}");
        assert_eq!(err.matches('\n').count(), 1, "one line: {err}");
        let error: Value = serde_json::from_str(&err).unwrap();
        assert_eq!(error["error"]["code"], kind, "{args:?}: {err}");
        assert!(error["error"]["message"].as_str().is_some_and(|message| !message.is_empty()), "{err}");
    }
}

#[test]
fn a_directory_without_a_store_is_refused_and_left_empty() {
    let dir = tempfile::tempdir().unwrap();
    let (code, out, err) = on(dir.path(), &["--json", "account.list"]);
    assert_eq!((code, out.as_str()), (2, ""));
    let error: Value = serde_json::from_str(&err).unwrap();
    assert_eq!(error["error"]["code"], "no_store");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    let missing = dir.path().join("missing");
    let (code, _, err) = on(&missing, &["account.list"]);
    assert_eq!(code, 2);
    assert!(err.contains("no store"), "{err}");
    assert!(!missing.exists());
}

#[test]
fn a_store_that_does_not_open_is_a_store_error() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(inboxcraft_store::DATABASE_FILE), b"not a database, just bytes long enough to be read as a header").unwrap();
    let (code, out, err) = on(dir.path(), &["--json", "account.list"]);
    assert_eq!((code, out.as_str()), (1, ""), "{err}");
    let error: Value = serde_json::from_str(&err).unwrap();
    assert_eq!(error["error"]["code"], "store");
}
