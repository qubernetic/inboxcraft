// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::io;

use inboxcraft_model::ErrorKind;
use inboxcraft_proto::ProtoError;
use inboxcraft_time::Timestamp;
use rustls::CertificateError;

use super::NetError;

fn io_kind(kind: io::ErrorKind) -> ErrorKind {
    NetError::io(&io::Error::from(kind), Timestamp::MAX).kind()
}

#[test]
fn socket_errors_have_their_kinds() {
    assert_eq!(io_kind(io::ErrorKind::TimedOut), ErrorKind::TimedOut);
    assert_eq!(io_kind(io::ErrorKind::WouldBlock), ErrorKind::TimedOut);
    assert_eq!(io_kind(io::ErrorKind::ConnectionRefused), ErrorKind::ConnectionRefused);
    for kind in [io::ErrorKind::UnexpectedEof, io::ErrorKind::ConnectionReset, io::ErrorKind::BrokenPipe, io::ErrorKind::Other] {
        assert_eq!(io_kind(kind), ErrorKind::ConnectionDropped);
    }
}

#[test]
fn a_tls_error_inside_a_socket_error_is_classified() {
    let tls = rustls::Error::InvalidCertificate(CertificateError::UnknownIssuer);
    let error = io::Error::new(io::ErrorKind::InvalidData, tls);
    assert_eq!(NetError::io(&error, Timestamp::MAX).kind(), ErrorKind::CertificateUnknownIssuer);
}

#[test]
fn proto_errors_have_their_kinds() {
    let kind = |error: ProtoError| NetError::proto(&error).kind();
    assert_eq!(kind(ProtoError::LineTooLong { limit: 1 }), ErrorKind::TooLarge);
    assert_eq!(kind(ProtoError::LiteralTooLarge { size: 2, limit: 1 }), ErrorKind::TooLarge);
    assert_eq!(kind(ProtoError::TooManyLiterals { limit: 1 }), ErrorKind::TooLarge);
    assert_eq!(kind(ProtoError::TooDeep { limit: 1 }), ErrorKind::TooLarge);
    assert_eq!(kind(ProtoError::NotOffered("STARTTLS")), ErrorKind::StartTlsMissing);
    assert_eq!(kind(ProtoError::NotOffered("LOGIN")), ErrorKind::NoLoginMethod);
    assert_eq!(kind(ProtoError::NotOffered("AUTH=PLAIN")), ErrorKind::NoLoginMethod);
    assert_eq!(kind(ProtoError::NotOffered("PREVIEW")), ErrorKind::Protocol);
    assert_eq!(kind(ProtoError::Bye(String::new())), ErrorKind::ConnectionDropped);
    assert_eq!(kind(ProtoError::Closed), ErrorKind::ConnectionDropped);
    assert_eq!(kind(ProtoError::PreauthWithoutTls), ErrorKind::Protocol);
    assert_eq!(kind(ProtoError::Malformed("x")), ErrorKind::Protocol);
}

#[test]
fn what_ends_a_connection() {
    for kind in [ErrorKind::CommandRefused, ErrorKind::FolderGone, ErrorKind::MessageGone, ErrorKind::CredentialMissing] {
        assert!(!NetError::new(kind, "").ends_connection(), "{kind:?}");
    }
    for kind in [ErrorKind::ConnectionDropped, ErrorKind::Protocol, ErrorKind::TooLarge, ErrorKind::LoginRefused] {
        assert!(NetError::new(kind, "").ends_connection(), "{kind:?}");
    }
}

#[test]
fn an_error_says_its_cause_and_detail() {
    let error = NetError::new(ErrorKind::TimedOut, "read");
    assert_eq!(error.detail(), "read");
    assert_eq!(error.to_string(), "the server did not answer in time (read)");
}
