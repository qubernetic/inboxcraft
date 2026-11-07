// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{FolderState, Job, JobKind};
use crate::{AccountId, DraftId, FolderId, FolderPath, JobId, MessageId, ModSeq, Uid, UidValidity};

fn inbox() -> FolderPath {
    FolderPath::new("INBOX", Some('/')).unwrap()
}

fn state() -> FolderState {
    FolderState::new(UidValidity::new(7).unwrap()).with_uid_next(Uid::new(100)).with_highest_modseq(ModSeq::new(555))
}

#[test]
fn a_folder_state_is_what_a_resync_starts_from() {
    let state = state();
    assert_eq!(state.uid_validity(), UidValidity::new(7).unwrap());
    assert_eq!(state.uid_next(), Uid::new(100));
    assert_eq!(state.highest_modseq(), ModSeq::new(555));
    let bare = FolderState::new(UidValidity::new(1).unwrap());
    assert_eq!((bare.uid_next(), bare.highest_modseq()), (None, None));
}

#[test]
fn every_job_names_its_account_and_folder() {
    let account = AccountId::new(1);
    let folder = FolderId::new(2);
    let jobs = [
        (Job::new(JobId::new(1), account, JobKind::SyncFolderList), None),
        (Job::new(JobId::new(2), account, JobKind::SyncFolder { folder, path: inbox(), known: Some(state()) }), Some(folder)),
        (
            Job::new(
                JobId::new(3),
                account,
                JobKind::FetchBody { folder, path: inbox(), uid_validity: UidValidity::new(7).unwrap(), uid: Uid::new(9).unwrap() },
            ),
            Some(folder),
        ),
        (Job::new(JobId::new(4), account, JobKind::Idle { folder, path: inbox() }), Some(folder)),
        (Job::new(JobId::new(5), account, JobKind::Check { by_owner: false }), None),
    ];
    for (job, expected) in jobs {
        assert_eq!(job.account, account);
        assert_eq!(job.kind.folder(), expected, "{job:?}");
        let json = serde_json::to_string(&job).unwrap();
        assert_eq!(serde_json::from_str::<Job>(&json).unwrap(), job);
    }
}

#[test]
fn a_job_from_storage_is_checked() {
    let json = r#"{"id":1,"account":1,"kind":{"Idle":{"folder":2,"path":{"name":"a\u0000","delimiter":"/"}}}}"#;
    assert!(serde_json::from_str::<Job>(json).is_err());
    let json = r#"{"id":1,"account":1,"kind":{"FetchBody":{"folder":2,"path":{"name":"INBOX","delimiter":"/"},"uid_validity":7,"uid":0}}}"#;
    assert!(serde_json::from_str::<Job>(json).is_err());
}

#[test]
fn every_write_job_names_its_folder_and_round_trips() {
    let account = AccountId::new(1);
    let folder = FolderId::new(2);
    let validity = UidValidity::new(7).unwrap();
    let uids: crate::UidSet = [Uid::new(9).unwrap(), Uid::new(10).unwrap()].into_iter().collect();
    let seen: crate::Flags = [crate::Flag::Seen].into_iter().collect();
    let archive = FolderPath::new("Archive", Some('/')).unwrap();
    let jobs = [
        (
            JobKind::StoreFlags {
                folder,
                path: inbox(),
                uid_validity: validity,
                uids: uids.clone(),
                add: seen.clone(),
                remove: crate::Flags::default(),
            },
            Some(folder),
        ),
        (
            JobKind::Move { folder, path: inbox(), uid_validity: validity, uids: uids.clone(), to: FolderId::new(3), to_path: archive.clone() },
            Some(folder),
        ),
        (JobKind::Expunge { folder, path: inbox(), uid_validity: validity, uids }, Some(folder)),
        (
            JobKind::Append { folder, path: inbox(), flags: seen.clone(), raw: b"Subject: Hi\r\n\r\nHello".to_vec(), draft: None, message: None },
            Some(folder),
        ),
        (
            JobKind::Append {
                folder,
                path: inbox(),
                flags: seen.clone(),
                raw: b"Subject: Hi\r\n\r\nHello".to_vec(),
                draft: Some(DraftId::new(4)),
                message: None,
            },
            Some(folder),
        ),
        (
            JobKind::Append {
                folder,
                path: inbox(),
                flags: seen,
                raw: b"Subject: Hi\r\n\r\nHello".to_vec(),
                draft: None,
                message: Some(MessageId::new(5)),
            },
            Some(folder),
        ),
        (JobKind::CreateFolder { path: archive }, None),
    ];
    for (id, (kind, expected)) in (10..).zip(jobs) {
        assert!(kind.writes(), "{kind:?}");
        let job = Job::new(JobId::new(id), account, kind);
        assert_eq!(job.kind.folder(), expected, "{job:?}");
        let json = serde_json::to_string(&job).unwrap();
        assert_eq!(serde_json::from_str::<Job>(&json).unwrap(), job);
    }
    // An append kept in the queue by an earlier build, before it named a message, reads back.
    let append = JobKind::Append { folder, path: inbox(), flags: crate::Flags::default(), raw: b"A".to_vec(), draft: None, message: None };
    let older = serde_json::to_string(&Job::new(JobId::new(3), account, append)).unwrap().replace(",\"message\":null", "");
    assert!(!older.contains("message"), "{older}");
    let read = serde_json::from_str::<Job>(&older);
    let Ok(Job { kind: JobKind::Append { message, .. }, .. }) = read else { panic!("{read:?}") };
    assert_eq!(message, None);
    for kind in [JobKind::SyncFolderList, JobKind::Check { by_owner: true }, JobKind::Idle { folder, path: inbox() }] {
        assert!(!kind.writes(), "{kind:?}");
    }
}
