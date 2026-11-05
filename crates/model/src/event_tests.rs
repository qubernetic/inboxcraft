// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{ConnectionState, ErrorKind, Event, EventKind, FlagChange};
use crate::{
    AccountId, CheckStep, Envelope, Flag, Flags, FolderId, FolderListing, FolderPath, FolderState, JobId, ModSeq, StepReport, StepStatus, Summary,
    Uid, UidSet, UidValidity,
};

fn uid(n: u32) -> Uid {
    Uid::new(n).unwrap()
}

fn validity() -> UidValidity {
    UidValidity::new(7).unwrap()
}

const EVERY_ERROR: [ErrorKind; 22] = [
    ErrorKind::HostNotFound,
    ErrorKind::ConnectionRefused,
    ErrorKind::TimedOut,
    ErrorKind::PortTimedOut,
    ErrorKind::MailPortsBlocked,
    ErrorKind::ConnectionDropped,
    ErrorKind::CertificateUnknownIssuer,
    ErrorKind::CertificateExpired,
    ErrorKind::CertificateNotYetValid,
    ErrorKind::CertificateWrongHost,
    ErrorKind::ClockFarOff,
    ErrorKind::Tls,
    ErrorKind::StartTlsMissing,
    ErrorKind::LoginRefused,
    ErrorKind::LoginUnanswered,
    ErrorKind::NoLoginMethod,
    ErrorKind::CredentialMissing,
    ErrorKind::CommandRefused,
    ErrorKind::Protocol,
    ErrorKind::TooLarge,
    ErrorKind::FolderGone,
    ErrorKind::MessageGone,
];

#[test]
fn every_event_kind_round_trips_and_names_its_folder() {
    let folder = FolderId::new(2);
    let mut flags = Flags::default();
    flags.insert(Flag::Seen);
    let kinds = [
        (EventKind::Folders { listings: vec![FolderListing::new(FolderPath::new("INBOX", Some('/')).unwrap(), vec![], true)] }, None),
        (EventKind::MessagesAdded { folder, uid_validity: validity(), messages: vec![Summary::new(uid(1), Envelope::new("Hi"))] }, Some(folder)),
        (
            EventKind::MessagesChanged { folder, uid_validity: validity(), changes: vec![FlagChange { uid: uid(1), flags, modseq: ModSeq::new(3) }] },
            Some(folder),
        ),
        (EventKind::MessagesExpunged { folder, uid_validity: validity(), uids: [uid(1), uid(2)].into_iter().collect::<UidSet>() }, Some(folder)),
        (EventKind::UidValidityReset { folder, old: Some(validity()), new: UidValidity::new(8).unwrap() }, Some(folder)),
        (EventKind::BodyFetched { folder, uid_validity: validity(), uid: uid(1), raw: b"Subject: Hi\r\n\r\nHello".to_vec() }, Some(folder)),
        (EventKind::FolderSynced { folder, state: FolderState::new(validity()) }, Some(folder)),
        (EventKind::Connection(ConnectionState::Online), None),
        (EventKind::Failed { kind: ErrorKind::FolderGone, folder: Some(folder) }, Some(folder)),
        (EventKind::Failed { kind: ErrorKind::TimedOut, folder: None }, None),
        (EventKind::CheckStep(StepReport::new(CheckStep::Tcp, StepStatus::Failed(ErrorKind::MailPortsBlocked))), None),
    ];
    for (kind, expected) in kinds {
        assert_eq!(kind.folder(), expected, "{kind:?}");
        let event = Event::new(AccountId::new(1), Some(JobId::new(4)), kind);
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(serde_json::from_str::<Event>(&json).unwrap(), event);
    }
}

#[test]
fn connection_states_round_trip() {
    for state in [
        ConnectionState::Offline,
        ConnectionState::Connecting,
        ConnectionState::Online,
        ConnectionState::Watching,
        ConnectionState::BackingOff { attempt: 3, retry_in_secs: 60 },
    ] {
        let event = Event::new(AccountId::new(1), None, EventKind::Connection(state));
        assert_eq!(event.job, None);
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(serde_json::from_str::<Event>(&json).unwrap(), event);
    }
}

#[test]
fn only_passing_network_trouble_is_worth_a_retry() {
    let retried: Vec<ErrorKind> = EVERY_ERROR.into_iter().filter(|kind| kind.is_retryable()).collect();
    assert_eq!(retried, [ErrorKind::HostNotFound, ErrorKind::ConnectionRefused, ErrorKind::TimedOut, ErrorKind::ConnectionDropped]);
}

#[test]
fn every_error_kind_reads_as_a_sentence_and_round_trips() {
    let mut seen = std::collections::HashSet::new();
    for kind in EVERY_ERROR {
        let text = kind.to_string();
        assert!(text.starts_with("the "), "{text}");
        assert!(seen.insert(text), "{kind:?} has the same text as another kind");
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(serde_json::from_str::<ErrorKind>(&json).unwrap(), kind);
    }
}
