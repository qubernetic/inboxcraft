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
    let engine = UiError::from(EngineError::NotFound { what: "message", id: 7 });
    assert_eq!(engine.to_string(), EngineError::NotFound { what: "message", id: 7 }.to_string());
    assert!(engine.source().is_some());
}
