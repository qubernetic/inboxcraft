// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A store killed while it writes loses no committed batch and keeps no half of one.
//!
//! The test runs this test binary again as a child (`child_writes_until_killed`), reads the number of each batch the
//! child committed, kills it at some point inside the next write, and opens the store.

#![allow(clippy::unwrap_used)]

mod contract;

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};

use contract::{body, message, two_accounts};
use inboxcraft_model::{FolderId, MessageId};
use inboxcraft_store::{Batch, Change, DATABASE_FILE, SOURCES_DIR, Sqlite, Storage, StoreConfig};

/// The store directory of the child; without it the child test does nothing.
const DIR_VAR: &str = "INBOXCRAFT_STORE_KILL_DIR";

/// How many batches the child commits at most, so a child nobody kills still ends.
const MAX_BATCHES: u64 = 100_000;

fn text(n: u64) -> String {
    format!("message number {n} {}", "padding ".repeat(usize::try_from(n % 50).unwrap()))
}

/// One batch: message `n` in INBOX with its body, and a flag change on the message before it.
fn batch(n: u64) -> Batch {
    let mut batch = Batch::new()
        .with(Change::PutMessage(message(n, 1, 1, u32::try_from(n).unwrap(), &format!("Subject {n}"), Some(1))))
        .with(Change::PutBody(MessageId::new(n), body(&text(n))));
    if n > 1 {
        batch.push(Change::MarkOpened(MessageId::new(n - 1)));
    }
    batch
}

#[test]
fn child_writes_until_killed() {
    let Some(dir) = std::env::var_os(DIR_VAR) else { return };
    let mut store = Sqlite::open(Path::new(&dir), StoreConfig::default()).unwrap();
    let first = u64::try_from(store.messages(FolderId::new(1)).unwrap().len()).unwrap() + 1;
    let mut out = std::io::stdout().lock();
    for n in first..first + MAX_BATCHES {
        store.apply(&batch(n)).unwrap();
        writeln!(out, "committed {n}").unwrap();
        out.flush().unwrap();
    }
}

/// A fresh store directory under the target directory, removed when the value drops.
fn dir() -> tempfile::TempDir {
    tempfile::Builder::new().prefix("kill-").tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap()
}

/// Runs the child until it has committed `more` batches, kills it, and returns the last batch it reported.
fn run_and_kill(dir: &Path, more: u64) -> u64 {
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["child_writes_until_killed", "--exact", "--nocapture", "--test-threads=1"])
        .env(DIR_VAR, dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let mut reported = Vec::new();
    for line in lines {
        // The first one may share its line with the test harness's `test ... ` announcement.
        if let Some((_, n)) = line.unwrap().rsplit_once("committed ") {
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

/// Every batch up to `committed` is there whole; the child may have committed more before the kill, also whole.
fn check(dir: &Path, committed: u64) -> u64 {
    let store = Sqlite::open(dir, StoreConfig::default()).unwrap();
    let entries = store.messages(FolderId::new(1)).unwrap();
    let held = u64::try_from(entries.len()).unwrap();
    assert!(held >= committed, "{held} messages held, {committed} reported committed");
    for (n, entry) in (1..).zip(&entries) {
        assert_eq!(entry.message().id(), MessageId::new(n));
        assert!(entry.has_body(), "message {n} without its body");
        assert_eq!(store.body(MessageId::new(n)).unwrap(), Some(body(&text(n))), "body {n}");
        assert_eq!(entry.opened(), n < held, "message {n} opened by the batch after it");
    }
    let files = std::fs::read_dir(dir.join(SOURCES_DIR)).unwrap().count();
    assert_eq!(files as u64, held, "one source file per kept body, no orphan");
    let conn = rusqlite::Connection::open(dir.join(DATABASE_FILE)).unwrap();
    let integrity: String = conn.query_row("PRAGMA integrity_check", [], |row| row.get(0)).unwrap();
    assert_eq!(integrity, "ok");
    held
}

#[test]
fn a_kill_loses_no_committed_batch() {
    let tmp = dir();
    let dir = tmp.path();
    two_accounts(&mut Sqlite::open(dir, StoreConfig::default()).unwrap());
    let mut held = 0;
    for more in [1, 7, 30, 3, 60] {
        let committed = run_and_kill(dir, more);
        assert_eq!(committed, held + more);
        held = check(dir, committed);
    }
}
