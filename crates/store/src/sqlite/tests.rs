// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use rusqlite::config::DbConfig;

use super::{Sqlite, phrase};
use crate::StoreConfig;

#[test]
fn the_connection_trusts_nothing_in_the_file() {
    let tmp = tempfile::tempdir().unwrap();
    let store = Sqlite::open(tmp.path(), StoreConfig::default()).unwrap();
    assert!(store.conn.db_config(DbConfig::SQLITE_DBCONFIG_DEFENSIVE).unwrap(), "defensive mode");
    assert!(!store.conn.db_config(DbConfig::SQLITE_DBCONFIG_TRUSTED_SCHEMA).unwrap(), "no trusted schema");
    let secure_delete: i64 = store.conn.pragma_query_value(None, "secure_delete", |row| row.get(0)).unwrap();
    assert_eq!(secure_delete, 1);
}

#[test]
fn a_phrase_is_one_quoted_string() {
    assert_eq!(phrase(&[]), None);
    assert_eq!(phrase(&["near".to_owned(), "or".to_owned()]), Some("\"near or\"".to_owned()));
    assert_eq!(phrase(&["a\"b".to_owned(), "c".to_owned()]), Some("\"a\"\"b c\"".to_owned()), "a quote is doubled");
}
