// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Text typed into a draft before a kill is in the draft after a restart.
//!
//! The test runs this test binary again as a child (`child_types_until_killed`), which opens a session over a store on
//! disk and types into one draft, a word per `compose.update`, saying after each how many words the answer holds. The
//! test kills the child at some point, opens a session over the same store, and finds every word the child was told
//! about.

#![allow(clippy::unwrap_used)]

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};

use inboxcraft_engine::Session;
use inboxcraft_model::{Account, AccountId, EmailAddress, Host, Security, Server};
use inboxcraft_store::{Batch, Change, Sqlite, Storage, StoreConfig};
use inboxcraft_time::{FakeClock, Timestamp};
use serde_json::{Value, json};

/// The store directory of the child; without it the child test does nothing.
const DIR_VAR: &str = "INBOXCRAFT_DRAFT_KILL_DIR";

/// How many words the child types at most, so a child nobody kills still ends.
const MAX_WORDS: u64 = 100_000;

fn session(dir: &Path) -> Session {
    let store = Sqlite::open(dir, StoreConfig::default()).unwrap();
    Session::new(Box::new(store), Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))).unwrap()
}

/// The one draft, made by the first child.
fn the_draft(session: &mut Session) -> Option<Value> {
    session.run("compose.list", &json!({})).unwrap()["drafts"].as_array().unwrap().first().cloned()
}

fn words(text: &str) -> u64 {
    text.split_whitespace().filter(|word| word.starts_with('w')).count() as u64
}

#[test]
fn child_types_until_killed() {
    let Some(dir) = std::env::var_os(DIR_VAR) else { return };
    let mut session = session(Path::new(&dir));
    let draft = match the_draft(&mut session) {
        Some(draft) => draft,
        None => session.run("compose.new", &json!({})).unwrap(),
    };
    let id = draft["id"].as_u64().unwrap();
    let mut text = draft["text"].as_str().unwrap().to_owned();
    let mut out = std::io::stdout().lock();
    for _ in 0..MAX_WORDS {
        text.push_str(&format!(" w{}", words(&text) + 1));
        let answer = session.run("compose.update", &json!({ "id": id, "text": text })).unwrap();
        writeln!(out, "typed {}", words(answer["text"].as_str().unwrap())).unwrap();
        out.flush().unwrap();
    }
}

/// A fresh store directory under the target directory, removed when the value drops.
fn dir() -> tempfile::TempDir {
    tempfile::Builder::new().prefix("draft-kill-").tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap()
}

/// Runs the child until it has typed `more` words, kills it, and returns the count it last reported.
fn run_and_kill(dir: &Path, more: u64) -> u64 {
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["child_types_until_killed", "--exact", "--nocapture", "--test-threads=1"])
        .env(DIR_VAR, dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let mut reported = Vec::new();
    for line in lines {
        // The first one may share its line with the test harness's `test ... ` announcement.
        if let Some((_, n)) = line.unwrap().rsplit_once("typed ") {
            reported.push(n.parse::<u64>().unwrap());
            if reported.len() as u64 == more {
                break;
            }
        }
    }
    child.kill().unwrap();
    child.wait().unwrap();
    assert_eq!(reported.len() as u64, more, "the child stopped before it was killed");
    *reported.last().unwrap()
}

#[test]
fn text_typed_before_a_kill_is_in_the_draft_after_a_restart() {
    let tmp = dir();
    let dir = tmp.path();
    let imap = Server::new(Host::new("imap.example.org").unwrap(), Security::Tls.imap_port(), Security::Tls);
    let account =
        Account::new(AccountId::new(1), "Work", EmailAddress::new("anna@example.org").unwrap(), "anna", imap).unwrap().with_signature(Some("Anna"));
    Sqlite::open(dir, StoreConfig::default()).unwrap().apply(&Batch::new().with(Change::PutAccount(account))).unwrap();
    let mut typed = 0;
    for more in [1, 5, 40, 2] {
        let reported = run_and_kill(dir, more);
        assert!(reported >= typed + more, "{reported} words reported, {typed} kept before");
        let mut session = session(dir);
        let draft = the_draft(&mut session).unwrap();
        let text = draft["text"].as_str().unwrap();
        typed = words(text);
        assert!(typed >= reported, "{typed} words kept, {reported} reported typed");
        assert!(text.starts_with("\n\n-- \nAnna w1"), "{text:?}");
        assert!(text.ends_with(&format!(" w{typed}")), "{text:?}");
    }
}
