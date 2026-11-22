// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::error::Error as _;

use inboxcraft_engine::EngineError;

use super::UiError;

#[test]
fn every_error_reads_as_a_sentence_and_only_an_engine_error_has_a_source() {
    let sample = UiError::sample("bad address");
    assert_eq!(sample.to_string(), "the sample mailbox could not be built: bad address");
    assert!(sample.source().is_none());
    let answer = UiError::Answer { command: "mail.list", detail: "no messages".to_owned() };
    assert_eq!(answer.to_string(), "mail.list answered in an unexpected shape: no messages");
    assert!(answer.source().is_none());
    let shot = UiError::Shot("line 2: bad".to_owned());
    assert_eq!(shot.to_string(), "ui_shot: line 2: bad");
    assert!(shot.source().is_none());
    let picture = UiError::Picture("only PNG and JPEG pictures are shown".to_owned());
    assert_eq!(picture.to_string(), "only PNG and JPEG pictures are shown");
    assert!(picture.source().is_none());
    let desk = UiError::Desk("the choice could not be kept".to_owned());
    assert_eq!(desk.to_string(), "the choice could not be kept");
    assert!(desk.source().is_none());
    let engine = UiError::from(EngineError::NotFound { what: "message", id: 7 });
    assert_eq!(engine.to_string(), EngineError::NotFound { what: "message", id: 7 }.to_string());
    assert!(engine.source().is_some());
}

#[test]
fn a_store_failure_s_words_go_to_the_log_once_and_only_its_words() {
    use inboxcraft_store::StoreError;

    use super::store_log;
    let mut logged = None;
    let locked = EngineError::Store(StoreError::DatabaseBusy("database is locked".to_owned()));
    assert_eq!(
        store_log(&locked, &mut logged).as_deref(),
        Some("inboxcraft: a command failed: the store is busy, try again (the database said: database is locked)")
    );
    assert_eq!(store_log(&locked, &mut logged), None, "the same words again are not written again");
    let gone = EngineError::Store(StoreError::Database("no such table: body".to_owned()));
    assert!(store_log(&gone, &mut logged).is_some_and(|line| line.ends_with("(the database said: no such table: body)")));
    assert_eq!(store_log(&EngineError::Store(StoreError::Corrupt("a record")), &mut logged), None, "no words of the database");
    assert_eq!(store_log(&EngineError::NothingToUndo, &mut logged), None);
    assert_eq!(logged.as_deref(), Some("no such table: body"));
}
