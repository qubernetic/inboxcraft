// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use inboxcraft_model::ErrorKind;
use inboxcraft_proto::ProtoError;

use super::proto;

#[test]
fn fatal_session_errors_have_their_kinds() {
    let cases = [
        (ProtoError::TooLarge { size: 2, limit: 1 }, ErrorKind::TooLarge),
        (ProtoError::LineTooLong { limit: 1 }, ErrorKind::TooLarge),
        (ProtoError::TooManyLines { limit: 1 }, ErrorKind::TooLarge),
        (ProtoError::NotOffered("STARTTLS"), ErrorKind::StartTlsMissing),
        (ProtoError::NotOffered("AUTH PLAIN"), ErrorKind::NoLoginMethod),
        (ProtoError::NotOffered("AUTH LOGIN"), ErrorKind::NoLoginMethod),
        (ProtoError::Bye(String::new()), ErrorKind::ConnectionDropped),
        (ProtoError::Closed, ErrorKind::ConnectionDropped),
        (ProtoError::PlaintextAfterStartTls, ErrorKind::Protocol),
        (ProtoError::Malformed("reply"), ErrorKind::Protocol),
    ];
    for (error, kind) in cases {
        assert_eq!(proto(&error).kind(), kind, "{error:?}");
    }
}
