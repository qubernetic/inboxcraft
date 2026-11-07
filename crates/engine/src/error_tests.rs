// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::error::Error as _;

use inboxcraft_model::AccountId;
use inboxcraft_store::StoreError;

use super::{EngineError, MAX_ERROR_TEXT, clean};
use crate::AgentClass;

#[test]
fn every_error_has_a_code_and_a_sentence() {
    let errors = [
        (EngineError::UnknownCommand("mail.lost".into()), "unknown_command", "there is no command \"mail.lost\""),
        (EngineError::Unclassified("mail.list"), "unclassified", "the command mail.list has no agent class, so it does not run"),
        (EngineError::DuplicateCommand("mail.list"), "duplicate_command", "the command mail.list is registered twice"),
        (EngineError::BadCommandId("Mail"), "bad_command_id", "\"Mail\" is not a command id"),
        (
            EngineError::NotAllowed { command: "mail.send", class: AgentClass::Owner },
            "not_allowed",
            "the command mail.send is owner: an agent may not run it",
        ),
        (EngineError::bad_params("mail.list", "unknown field"), "bad_params", "bad parameters for mail.list: unknown field"),
        (EngineError::NotFound { what: "message", id: 7 }, "not_found", "there is no message 7"),
        (EngineError::Store(StoreError::UnknownAccount(AccountId::new(3))), "store", "no account a3 in the store"),
        (EngineError::Panicked("mail.list"), "panicked", "the command mail.list failed inside; nothing was changed"),
        (EngineError::SampleOnly("sample.mail.set"), "sample_only", "the command sample.mail.set runs only on the sample mailbox (--sample)"),
        (EngineError::NothingToUndo, "nothing_to_undo", "there is nothing to undo"),
        (
            EngineError::StillMoving { id: 7 },
            "still_moving",
            "the message 7 is still on its way to its folder; it can be deleted for good once the server has it there",
        ),
    ];
    for (error, code, sentence) in errors {
        assert_eq!(error.code(), code);
        assert_eq!(error.to_string(), sentence);
    }
}

#[test]
fn only_a_store_error_has_a_source() {
    let store: EngineError = StoreError::TooLarge.into();
    assert_eq!(store.source().map(ToString::to_string).as_deref(), Some("the message source is too large to keep"));
    assert!(EngineError::Panicked("x.y").source().is_none());
}

#[test]
fn repeated_text_loses_control_characters_and_is_cut() {
    assert_eq!(clean("a\r\nb\0c\u{1b}[31m"), "abc[31m");
    let long = "é".repeat(MAX_ERROR_TEXT + 50);
    assert_eq!(clean(&long).chars().count(), MAX_ERROR_TEXT);
}
