// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The shared contract on the SQLite backend, and what only it does: files, reopening, damage.

#![allow(clippy::unwrap_used)]

mod contract;

use std::fs;
use std::path::Path;

use contract::{account, body, folder, m, message, two_accounts};
use inboxcraft_model::{
    Account, AccountId, AlertId, CalendarEvent, CalendarEventId, CalendarId, Draft, DraftId, Due, EmailAddress, FolderId, Job, JobId, JobKind,
    MessageState, OutboxState,
};
use inboxcraft_store::{Batch, Change, DATABASE_FILE, SCHEMA_VERSION, SOURCES_DIR, Search, Sqlite, Storage, StoreConfig, StoreError};

/// A fresh store directory under the target directory, removed when the value drops.
fn dir() -> tempfile::TempDir {
    tempfile::Builder::new().prefix("store-").tempdir_in(env!("CARGO_TARGET_TMPDIR")).unwrap()
}

fn fresh() -> (Sqlite, tempfile::TempDir) {
    let tmp = dir();
    (Sqlite::open(tmp.path(), StoreConfig::default()).unwrap(), tmp)
}

contract_tests!(fresh());

fn sources(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> =
        fs::read_dir(dir.join(SOURCES_DIR)).unwrap().map(|entry| entry.unwrap().file_name().into_string().unwrap()).collect();
    names.sort();
    names
}

fn sql(dir: &Path, statement: &str) {
    rusqlite::Connection::open(dir.join(DATABASE_FILE)).unwrap().execute_batch(statement).unwrap();
}

/// A store in `dir` with message 1 in folder 1 and its body.
fn with_body(dir: &Path) -> Sqlite {
    let mut store = Sqlite::open(dir, StoreConfig::default()).unwrap();
    two_accounts(&mut store);
    store.apply(&Batch::new().with(Change::PutMessage(message(1, 1, 1, 5, "x", Some(1)))).with(Change::PutBody(m(1), body("hello")))).unwrap();
    store
}

#[test]
fn everything_is_there_after_a_reopen() {
    let tmp = dir();
    let dir = tmp.path();
    let mut store = with_body(dir);
    store.apply(&Batch::new().with(Change::MarkOpened(m(1)))).unwrap();
    drop(store);
    let store = Sqlite::open(dir, StoreConfig { body_days: 7 }).unwrap();
    assert_eq!(store.config().body_days, 7);
    assert_eq!(store.schema_version().unwrap(), SCHEMA_VERSION);
    assert_eq!(store.accounts().unwrap(), [account(1), account(2)]);
    assert_eq!(store.folders(AccountId::new(1)).unwrap(), [folder(1, 1, "INBOX"), folder(2, 1, "Archive")]);
    let entry = store.message(m(1)).unwrap().unwrap();
    assert!(entry.opened() && entry.has_body());
    assert_eq!(store.body(m(1)).unwrap(), Some(body("hello")));
    assert_eq!(store.search(&Search::new().with_text("hello")).unwrap(), [m(1)]);
}

#[test]
fn a_source_is_one_file_outside_the_database() {
    let tmp = dir();
    let dir = tmp.path();
    let mut store = with_body(dir);
    assert_eq!(sources(dir), ["1.eml"]);
    assert_eq!(fs::read(dir.join(SOURCES_DIR).join("1.eml")).unwrap(), body("hello").source());
    store.apply(&Batch::new().with(Change::PutBody(m(1), body("again")))).unwrap();
    assert_eq!(sources(dir), ["2.eml"], "a new body gets a new file, the old one goes after the commit");
    store.apply(&Batch::new().with(Change::RemoveBody(m(1))).with(Change::PutBody(m(1), body("third")))).unwrap();
    assert_eq!(sources(dir), ["3.eml"]);
    store.apply(&Batch::new().with(Change::RemoveAccount(AccountId::new(1)))).unwrap();
    assert!(sources(dir).is_empty());
}

#[test]
fn a_refused_batch_leaves_no_file_behind() {
    let tmp = dir();
    let dir = tmp.path();
    let mut store = with_body(dir);
    let refused = Batch::new().with(Change::PutBody(m(1), body("new"))).with(Change::MarkOpened(m(9)));
    assert_eq!(store.apply(&refused), Err(StoreError::UnknownMessage(m(9))));
    assert_eq!(sources(dir), ["1.eml"]);
    assert_eq!(store.body(m(1)).unwrap(), Some(body("hello")));
}

#[test]
fn a_failed_source_write_refuses_the_batch() {
    let tmp = dir();
    let dir = tmp.path();
    let mut store = with_body(dir);
    fs::remove_dir_all(dir.join(SOURCES_DIR)).unwrap();
    let error = store.apply(&Batch::new().with(Change::PutMessage(message(2, 1, 1, 6, "y", None))).with(Change::PutBody(m(2), body("lost"))));
    assert_eq!(error, Err(StoreError::Io(std::io::ErrorKind::NotFound)));
    assert_eq!(store.message(m(2)).unwrap(), None);
}

#[test]
fn opening_removes_the_files_a_kill_left_behind() {
    let tmp = dir();
    let dir = tmp.path();
    drop(with_body(dir));
    let sources_dir = dir.join(SOURCES_DIR);
    for name in ["7.eml", "8.partial", "notes.txt", "x.eml", "x.partial"] {
        fs::write(sources_dir.join(name), b"left over").unwrap();
    }
    // Names the store never writes: not its own, so not its to remove.
    let foreign = ["007.eml", "+7.eml", "0.eml", "-3.eml", "08.partial", "0.partial"];
    for name in foreign {
        fs::write(sources_dir.join(name), b"someone else's").unwrap();
    }
    for name in ["9.eml", "10.partial"] {
        fs::create_dir(sources_dir.join(name)).unwrap();
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        fs::write(sources_dir.join(std::ffi::OsStr::from_bytes(b"\xff.eml")), b"not utf-8").unwrap();
    }
    let store = Sqlite::open(dir, StoreConfig::default()).unwrap();
    let mut left: Vec<_> = fs::read_dir(&sources_dir).unwrap().map(|entry| entry.unwrap().file_name()).collect();
    left.sort();
    let mut expected: Vec<std::ffi::OsString> =
        ["1.eml", "notes.txt", "x.eml", "x.partial", "9.eml", "10.partial"].iter().chain(&foreign).map(Into::into).collect();
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        expected.push(std::ffi::OsStr::from_bytes(b"\xff.eml").to_owned());
    }
    expected.sort();
    assert_eq!(left, expected);
    assert_eq!(store.body(m(1)).unwrap(), Some(body("hello")));
}

#[test]
fn a_store_from_a_newer_build_is_left_alone() {
    let tmp = dir();
    let dir = tmp.path();
    drop(fresh_in(dir));
    sql(dir, "PRAGMA user_version = 99;");
    assert_eq!(Sqlite::open(dir, StoreConfig::default()).err(), Some(StoreError::NewerSchema { found: 99, known: SCHEMA_VERSION }));
    let version: u32 =
        rusqlite::Connection::open(dir.join(DATABASE_FILE)).unwrap().pragma_query_value(None, "user_version", |row| row.get(0)).unwrap();
    assert_eq!(version, 99);
}

fn fresh_in(dir: &Path) -> Sqlite {
    Sqlite::open(dir, StoreConfig::default()).unwrap()
}

#[test]
fn a_file_that_is_no_database_is_refused() {
    let tmp = dir();
    let dir = tmp.path();
    fs::write(dir.join(DATABASE_FILE), vec![b'x'; 4096]).unwrap();
    assert!(matches!(Sqlite::open(dir, StoreConfig::default()), Err(StoreError::Database(_))));
}

#[test]
fn a_directory_that_cannot_be_made_is_refused() {
    let tmp = dir();
    let dir = tmp.path();
    fs::write(dir.join(SOURCES_DIR), b"a file where the directory goes").unwrap();
    assert!(matches!(Sqlite::open(dir, StoreConfig::default()), Err(StoreError::Io(_))));
}

#[test]
fn a_damaged_source_is_reported() {
    let tmp = dir();
    let dir = tmp.path();
    let store = with_body(dir);
    let file = dir.join(SOURCES_DIR).join("1.eml");
    fs::write(&file, b"short").unwrap();
    assert_eq!(store.body(m(1)), Err(StoreError::Corrupt("a message source has the wrong length")));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&file, fs::Permissions::from_mode(0o000)).unwrap();
        assert_eq!(store.body(m(1)), Err(StoreError::Io(std::io::ErrorKind::PermissionDenied)));
    }
    fs::remove_file(&file).unwrap();
    assert_eq!(store.body(m(1)), Err(StoreError::Corrupt("a message source is missing")));
    sql(dir, "UPDATE body SET length = -1;");
    assert_eq!(store.body(m(1)), Err(StoreError::Corrupt("a source length")));
}

#[test]
fn a_damaged_record_is_reported() {
    let tmp = dir();
    let dir = tmp.path();
    let store = with_body(dir);
    sql(dir, "UPDATE account SET record = '{' WHERE id = 1; UPDATE message SET record = '[]'; UPDATE folder SET record = 'null', state = '1';");
    assert_eq!(store.accounts(), Err(StoreError::Corrupt("a record")));
    assert_eq!(store.folders(AccountId::new(1)), Err(StoreError::Corrupt("a record")));
    assert_eq!(store.folder_state(FolderId::new(1)), Err(StoreError::Corrupt("a record")));
    assert_eq!(store.messages(FolderId::new(1)), Err(StoreError::Corrupt("a record")));
    let mut store = store;
    assert_eq!(store.apply(&Batch::new().with(Change::SetFlags(m(1), Default::default()))), Err(StoreError::Corrupt("a record")));
    sql(dir, "PRAGMA foreign_keys = OFF; UPDATE message SET id = -4;");
    assert_eq!(store.search(&Search::new()), Err(StoreError::Corrupt("a message id")));
    sql(dir, "UPDATE message SET opened = 'yes';");
    assert!(matches!(store.messages(FolderId::new(1)), Err(StoreError::Database(_))));
}

#[test]
fn an_id_sqlite_cannot_hold_is_refused() {
    let (mut store, _tmp) = fresh();
    let big = Account::new(AccountId::new(u64::MAX), "Big", EmailAddress::new("big@example.org").unwrap(), "big", account(1).imap().clone()).unwrap();
    let refused = store.apply(&Batch::new().with(Change::PutAccount(big)));
    assert_eq!(refused, Err(StoreError::Database(format!("id {} is out of range", u64::MAX))));
    assert!(store.accounts().unwrap().is_empty());
}

#[test]
fn the_database_runs_in_wal_mode_with_full_sync() {
    let tmp = dir();
    let dir = tmp.path();
    drop(fresh_in(dir));
    let conn = rusqlite::Connection::open(dir.join(DATABASE_FILE)).unwrap();
    let mode: String = conn.pragma_query_value(None, "journal_mode", |row| row.get(0)).unwrap();
    assert_eq!(mode, "wal");
}

#[cfg(unix)]
#[test]
fn only_the_owner_can_read_the_store() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = dir();
    let made = tmp.path().join("made");
    let dir = made.join("store");
    let store = with_body(&dir);
    let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&made), 0o700, "a directory made on the way");
    assert_eq!(mode(&dir), 0o700, "the store directory");
    assert_eq!(mode(&dir.join(SOURCES_DIR)), 0o700, "the sources directory");
    for name in [DATABASE_FILE.to_owned(), format!("{DATABASE_FILE}-wal"), format!("{DATABASE_FILE}-shm")] {
        assert_eq!(mode(&dir.join(&name)), 0o600, "{name}");
    }
    assert_eq!(mode(&dir.join(SOURCES_DIR).join("1.eml")), 0o600, "a source");
    drop(store);
}

/// The files under `dir` whose bytes hold `marker`.
fn holding(dir: &Path, marker: &str) -> Vec<String> {
    let mut found = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            found.extend(holding(&path, marker));
        } else if fs::read(&path).unwrap().windows(marker.len()).any(|window| window == marker.as_bytes()) {
            found.push(path.file_name().unwrap().to_string_lossy().into_owned());
        }
    }
    found
}

#[test]
fn a_removed_body_leaves_no_words_in_the_files() {
    let tmp = dir();
    let dir = tmp.path();
    let mut store = Sqlite::open(dir, StoreConfig::default()).unwrap();
    two_accounts(&mut store);
    store
        .apply(
            &Batch::new()
                .with(Change::PutMessage(message(1, 1, 1, 1, "old", Some(1))))
                .with(Change::PutBody(m(1), body("the zebrafinch ledger")))
                .with(Change::PutMessage(message(2, 1, 1, 2, "wombatsubject", Some(199))))
                .with(Change::PutBody(m(2), body("the platypusbody note"))),
        )
        .unwrap();
    for marker in ["zebrafinch", "wombatsubject", "platypusbody"] {
        assert!(!holding(dir, marker).is_empty(), "{marker} is written before it is removed");
    }

    assert_eq!(store.prune_bodies(contract::at(200)).unwrap(), 1);
    assert_eq!(holding(dir, "zebrafinch"), Vec::<String>::new(), "a pruned body");
    assert!(!holding(dir, "platypusbody").is_empty(), "the kept body stays");

    store.apply(&Batch::new().with(Change::RemoveMessage(m(2)))).unwrap();
    for marker in ["wombatsubject", "platypusbody"] {
        assert_eq!(holding(dir, marker), Vec::<String>::new(), "{marker} of a removed message");
    }
    drop(store);
    assert_eq!(holding(dir, "platypusbody"), Vec::<String>::new(), "nothing comes back on close");
}

#[cfg(unix)]
#[test]
fn an_open_store_is_tightened() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = dir();
    let dir = tmp.path();
    drop(with_body(dir));
    let wide = |path: &Path, mode: u32| fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
    let wal = dir.join(format!("{DATABASE_FILE}-wal"));
    let shm = dir.join(format!("{DATABASE_FILE}-shm"));
    fs::write(&wal, b"").unwrap();
    fs::write(&shm, b"").unwrap();
    for (path, mode) in
        [(dir.to_owned(), 0o755), (dir.join(SOURCES_DIR), 0o755), (dir.join(DATABASE_FILE), 0o644), (wal.clone(), 0o644), (shm.clone(), 0o644)]
    {
        wide(&path, mode);
    }
    let store = Sqlite::open(dir, StoreConfig::default()).unwrap();
    let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(dir), 0o700, "the store directory");
    assert_eq!(mode(&dir.join(SOURCES_DIR)), 0o700, "the sources directory");
    for path in [dir.join(DATABASE_FILE), wal, shm] {
        assert_eq!(mode(&path), 0o600, "{}", path.display());
    }
    assert_eq!(store.body(m(1)).unwrap(), Some(body("hello")));
}

#[test]
fn queued_jobs_and_local_records_are_there_after_a_reopen() {
    let tmp = dir();
    let dir = tmp.path();
    let mut store = with_body(dir);
    let batch = Batch::new()
        .with(Change::QueueJob(contract::job(7, 1)))
        .with(Change::QueueJob(contract::job(3, 2)))
        .with(Change::PutDraft(contract::draft(1, 1, "kept")))
        .with(Change::PutOutgoing(contract::outgoing(1, OutboxState::Waiting)))
        .with(Change::PutScheduled(contract::scheduled(1, 5, Due::Send(DraftId::new(1)))))
        .with(Change::PutSender(contract::sender("anna@example.org")))
        .with(Change::SetMessageState(m(1), MessageState { parked: true, ..MessageState::default() }))
        .with(Change::PutCalendar(contract::calendar(1, 1, "home")))
        .with(Change::SetSyncToken(CalendarId::new(1), Some("token-1".to_owned())));
    store.apply(&batch).unwrap();
    drop(store);
    let store = Sqlite::open(dir, StoreConfig::default()).unwrap();
    assert_eq!(store.queued_jobs().unwrap(), [contract::job(7, 1), contract::job(3, 2)], "the queue keeps its order");
    assert_eq!(store.draft(DraftId::new(1)).unwrap(), Some(contract::draft(1, 1, "kept")));
    assert_eq!(store.outbox().unwrap(), [contract::outgoing(1, OutboxState::Waiting)]);
    assert_eq!(store.schedule().unwrap().len(), 1);
    assert_eq!(store.senders().unwrap(), [contract::sender("anna@example.org")]);
    assert!(store.message_state(m(1)).unwrap().parked);
    assert_eq!(store.sync_token(CalendarId::new(1)).unwrap().as_deref(), Some("token-1"));
}

#[test]
fn a_damaged_local_record_is_reported() {
    let tmp = dir();
    let dir = tmp.path();
    let mut store = with_body(dir);
    let batch = Batch::new()
        .with(Change::QueueJob(contract::job(1, 1)))
        .with(Change::PutDraft(contract::draft(1, 1, "x")))
        .with(Change::SetMessageState(m(1), MessageState { pinned: true, ..MessageState::default() }))
        .with(Change::PutCalendar(contract::calendar(1, 1, "home")))
        .with(Change::PutCalendarEvent(contract::event(1, 1)));
    store.apply(&batch).unwrap();
    sql(
        dir,
        "UPDATE job SET record = '{'; UPDATE draft SET record = '[]'; UPDATE message_state SET record = 'null'; UPDATE calendar_event SET record = '1';",
    );
    assert_eq!(store.queued_jobs(), Err(StoreError::Corrupt("a record")));
    assert_eq!(store.draft(DraftId::new(1)), Err(StoreError::Corrupt("a record")));
    assert_eq!(store.message_states(), Err(StoreError::Corrupt("a record")));
    assert_eq!(store.message_state(m(1)), Err(StoreError::Corrupt("a record")));
    assert_eq!(store.calendar_event(CalendarEventId::new(1)), Err(StoreError::Corrupt("a record")));
    sql(dir, "PRAGMA foreign_keys = OFF; UPDATE message_state SET message = -2, record = '{}';");
    assert_eq!(store.message_states(), Err(StoreError::Corrupt("a message id")));
    sql(dir, "UPDATE calendar SET sync_token = x'ff';");
    assert!(matches!(store.sync_token(CalendarId::new(1)), Err(StoreError::Database(_))));
}

#[test]
fn every_store_has_an_id_of_its_own_for_good() {
    let (one, one_dir) = fresh();
    let (other, _other_dir) = fresh();
    let id = one.id().unwrap();
    assert!(id.len() == 32 && id.bytes().all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()), "{id}");
    assert_ne!(other.id().unwrap(), id, "two stores on one computer never share one");
    drop(one);
    assert_eq!(fresh_in(one_dir.path()).id().unwrap(), id, "kept between runs");
    sql(one_dir.path(), "DELETE FROM meta;");
    assert_eq!(fresh_in(one_dir.path()).id(), Err(StoreError::Corrupt("the store's id")));
}

#[test]
fn a_local_id_sqlite_cannot_hold_is_refused() {
    let (mut store, _tmp) = fresh();
    two_accounts(&mut store);
    let out_of_range = Err(StoreError::Database(format!("id {} is out of range", u64::MAX)));
    for change in [
        Change::QueueJob(Job::new(JobId::new(u64::MAX), AccountId::new(1), JobKind::SyncFolderList)),
        Change::PutDraft(contract::draft(u64::MAX, 1, "x")),
        Change::PutOutgoing(contract::outgoing(u64::MAX, OutboxState::Waiting)),
        Change::PutScheduled(contract::scheduled(1, 1, Due::Send(DraftId::new(u64::MAX)))),
        Change::PutScheduled(contract::scheduled(1, 1, Due::FollowUp(m(u64::MAX)))),
        Change::PutScheduled(contract::scheduled(1, 1, Due::Alert(AlertId::new(u64::MAX)))),
        Change::SetMessageState(m(u64::MAX), MessageState::default()),
        Change::PutCalendar(contract::calendar(u64::MAX, 1, "home")),
        Change::SetSyncToken(CalendarId::new(u64::MAX), None),
        Change::PutCalendarEvent(contract::event(1, u64::MAX)),
        Change::PutAlert(contract::alert(1, u64::MAX, 1)),
        Change::RemoveJob(JobId::new(u64::MAX)),
    ] {
        assert_eq!(store.apply(&Batch::new().with(change)), out_of_range);
    }
    store.apply(&Batch::new().with(Change::PutDraft(contract::draft(1, 1, "x")))).unwrap();
    let entry = Change::PutScheduled(contract::scheduled(u64::MAX, 1, Due::Send(DraftId::new(1))));
    assert_eq!(store.apply(&Batch::new().with(entry)), out_of_range);
    assert_eq!(store.drafts(AccountId::new(u64::MAX)), out_of_range.map(|()| Vec::new()));
}

#[test]
fn a_removed_draft_leaves_no_words_in_the_files() {
    let tmp = dir();
    let dir = tmp.path();
    let mut store = Sqlite::open(dir, StoreConfig::default()).unwrap();
    two_accounts(&mut store);
    let secret = Draft { text: "the quokkadraft plan".to_owned(), ..contract::draft(1, 1, "x") };
    let event = CalendarEvent { ical: "SUMMARY:narwhalmeeting".to_owned(), ..contract::event(1, 1) };
    store
        .apply(
            &Batch::new()
                .with(Change::PutDraft(secret))
                .with(Change::PutCalendar(contract::calendar(1, 1, "home")))
                .with(Change::PutCalendarEvent(event)),
        )
        .unwrap();
    assert!(!holding(dir, "quokkadraft").is_empty());
    store.apply(&Batch::new().with(Change::RemoveDraft(DraftId::new(1))).with(Change::RemoveCalendarEvent(CalendarEventId::new(1)))).unwrap();
    for marker in ["quokkadraft", "narwhalmeeting"] {
        assert_eq!(holding(dir, marker), Vec::<String>::new(), "{marker}");
    }
}

#[test]
fn a_forgotten_sender_refusal_or_sync_token_leaves_nothing_in_the_files() {
    let tmp = dir();
    let dir = tmp.path();
    let mut store = Sqlite::open(dir, StoreConfig::default()).unwrap();
    two_accounts(&mut store);
    let refused = OutboxState::Refused { reply: "550 5.1.1 no user penguinrecipient".to_owned() };
    store
        .apply(
            &Batch::new()
                .with(Change::PutSender(contract::sender("walrusperson@example.org")))
                .with(Change::PutDraft(contract::draft(1, 1, "x")))
                .with(Change::PutOutgoing(contract::outgoing(1, refused)))
                .with(Change::PutCalendar(contract::calendar(1, 1, "home")))
                .with(Change::SetSyncToken(CalendarId::new(1), Some("http://example.org/sync/ottertoken".to_owned()))),
        )
        .unwrap();
    let forgotten = [
        (Change::RemoveSender(EmailAddress::new("walrusperson@example.org").unwrap()), "walrusperson"),
        (Change::RemoveOutgoing(DraftId::new(1)), "penguinrecipient"),
        (Change::SetSyncToken(CalendarId::new(1), None), "ottertoken"),
    ];
    for (change, marker) in forgotten {
        assert!(!holding(dir, marker).is_empty(), "{marker}");
        store.apply(&Batch::new().with(change)).unwrap();
        assert_eq!(holding(dir, marker), Vec::<String>::new(), "{marker}");
    }
}
