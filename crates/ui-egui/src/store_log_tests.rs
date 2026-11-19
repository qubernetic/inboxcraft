// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A store failure of a command the window runs: the banner says the fixed sentence, the database's own words are
//! kept for the app's standard error, once while they repeat.

use inboxcraft_engine::Session;
use inboxcraft_store::{DATABASE_FILE, Sqlite, StoreConfig};
use inboxcraft_time::{FakeClock, Timestamp};
use serde_json::json;

use super::Mailbox;

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

#[test]
fn a_window_command_s_store_failure_logs_the_database_s_words_once_and_never_in_the_banner() {
    let dir = tempfile::tempdir().unwrap();
    let store = Sqlite::open(dir.path(), StoreConfig::default()).unwrap();
    let mut mailbox = Mailbox::new(Session::new(Box::new(store), clock()).unwrap(), clock());
    assert_eq!(mailbox.logged, None);
    // The database file damaged under the open store: every read fails with the database's own words.
    for name in [DATABASE_FILE.to_owned(), format!("{DATABASE_FILE}-wal"), format!("{DATABASE_FILE}-shm")] {
        std::fs::write(dir.path().join(name), vec![b'x'; 65_536]).unwrap();
    }
    let error = mailbox.run("account.list", &json!({})).unwrap_err();
    let banner = error.to_string();
    let logged = mailbox.logged.clone();
    assert!(logged.as_deref().is_some_and(|words| !words.is_empty()), "the words are kept for the log: {error:?}");
    assert!(!banner.contains(logged.as_deref().unwrap_or_default()), "the banner holds no words of the database: {banner}");
    assert!(mailbox.run("account.list", &json!({})).is_err());
    assert_eq!(mailbox.logged, logged, "the same words again are written once");
}
