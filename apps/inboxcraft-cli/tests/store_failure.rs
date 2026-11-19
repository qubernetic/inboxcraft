// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Headless on a store that really fails: the error object holds the code `store` and the failure's fixed sentence,
//! none of the database's own words (a table or column name, `sqlite`, a path).

use std::ffi::OsString;
use std::path::Path;

use inboxcraft_testkit::{store, store_fault};
use serde_json::Value;
use tempfile::TempDir;

fn fixture() -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    store::fixture(dir.path()).unwrap();
    dir
}

/// Runs `args` with `--store <dir> --json` and returns the error's message, checked: exit 1, nothing on stdout, one
/// error object with the code `store`, and nothing of the store in its words.
fn store_error(dir: &Path, words: &std::collections::BTreeSet<String>, args: &[&str]) -> String {
    let mut all = vec!["--store", dir.to_str().unwrap(), "--json"];
    all.extend_from_slice(args);
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = inboxcraft_cli::run(all.iter().map(OsString::from), &mut out, &mut err);
    let err = String::from_utf8(err).unwrap();
    assert_eq!((code, out.as_slice()), (1, &b""[..]), "{err}");
    let error: Value = serde_json::from_str(&err).unwrap();
    assert_eq!(error["error"]["code"], "store", "{err}");
    let message = error["error"]["message"].as_str().unwrap().to_owned();
    assert_eq!(store_fault::leaks(&message, words, dir), Vec::<String>::new(), "{err}");
    message
}

const LIST: [&str; 2] = ["mail.list", r#"{"folder":1}"#];

/// A fault, the command line that meets it, and the sentence it answers.
type Case<'a> = (fn(&Path), &'a [&'a str], &'a str);

#[test]
fn every_kind_of_failure_answers_its_own_sentence() {
    let read = format!(r#"{{"id":{}}}"#, store::LUNCH);
    let faults: [Case<'_>; 4] = [
        (|dir| store_fault::drop_table(dir, "body").unwrap(), &LIST, "the store could not be read or written"),
        (|dir| store_fault::damage(dir, "message").unwrap(), &LIST, "the store is damaged"),
        (|dir| store_fault::refuse_updates(dir, "message").unwrap(), &["mail.read", &read], "the store refused the change"),
        (|dir| store_fault::stale_version(dir).unwrap(), &LIST, "the store could not be brought up to this version"),
    ];
    for (fault, args, sentence) in faults {
        let dir = fixture();
        let words = store_fault::schema_words(dir.path()).unwrap();
        fault(dir.path());
        let mut words = words;
        words.extend(store_fault::schema_words(dir.path()).unwrap_or_default());
        assert_eq!(store_error(dir.path(), &words, args), sentence);
    }
}

#[test]
fn a_held_store_is_busy() {
    let dir = fixture();
    let words = store_fault::schema_words(dir.path()).unwrap();
    let held = store_fault::hold(dir.path()).unwrap();
    assert_eq!(store_error(dir.path(), &words, &LIST), "the store is busy, try again");
    held.release().unwrap();
}

#[test]
fn a_file_that_is_no_database_is_damaged() {
    let dir = fixture();
    let words = store_fault::schema_words(dir.path()).unwrap();
    std::fs::write(dir.path().join(inboxcraft_store::DATABASE_FILE), b"not a database, just bytes long enough to be read as a header").unwrap();
    assert_eq!(store_error(dir.path(), &words, &LIST), "the store is damaged");
}
