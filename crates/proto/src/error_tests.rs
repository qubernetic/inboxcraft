// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::ProtoError;
use crate::imap::State;

#[test]
fn refusals_before_sending_are_not_fatal_and_server_faults_are() {
    let local = [
        ProtoError::NotEncrypted,
        ProtoError::AlreadyEncrypted,
        ProtoError::AlreadyCompressed,
        ProtoError::WrongState { command: "LOGIN", state: State::Authenticated },
        ProtoError::Busy,
        ProtoError::NotOffered("STARTTLS"),
        ProtoError::CapabilitiesUnknown,
        ProtoError::NotEnabled("QRESYNC"),
        ProtoError::BadArgument("mailbox name"),
        ProtoError::ReadOnly,
    ];
    let fatal = [
        ProtoError::LineTooLong { limit: 1 },
        ProtoError::LiteralTooLarge { size: 2, limit: 1 },
        ProtoError::TooManyLiterals { limit: 1 },
        ProtoError::TooDeep { limit: 1 },
        ProtoError::Malformed("tag"),
        ProtoError::Unexpected("tagged response"),
        ProtoError::PreauthWithoutTls,
        ProtoError::PlaintextAfterStartTls,
        ProtoError::Bye(String::new()),
        ProtoError::Bye("shutting down".into()),
        ProtoError::Closed,
    ];
    for error in &local {
        assert!(!error.is_fatal(), "{error:?}");
    }
    for error in &fatal {
        assert!(error.is_fatal(), "{error:?}");
    }
    let texts: Vec<String> = local.iter().chain(&fatal).map(ToString::to_string).collect();
    for text in &texts {
        assert!(!text.is_empty() && !text.ends_with('.'), "{text}");
    }
    let mut unique = texts.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), texts.len());
    assert_eq!(
        ProtoError::WrongState { command: "LOGIN", state: State::Authenticated }.to_string(),
        "LOGIN cannot be sent in the authenticated state"
    );
    assert_eq!(ProtoError::Bye("shutting down".into()).to_string(), "the server ended the session: shutting down");
    let error: &dyn std::error::Error = &ProtoError::Closed;
    assert!(error.source().is_none());
}

#[test]
fn every_state_has_a_name() {
    let states =
        [State::Greeting, State::NotAuthenticated, State::Authenticated, State::Selected, State::StartingTls, State::LoggingOut, State::Closed];
    let mut names: Vec<String> = states.iter().map(ToString::to_string).collect();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), states.len());
}
