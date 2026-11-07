// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::Path;

use inboxcraft_engine::{AgentClass, CommandSpec, Outcome, Registry, Session};
use inboxcraft_model::{AccountId, EmailAddress, Host, Job, JobKind, Security, Server};
use inboxcraft_store::{Batch, Change};
use inboxcraft_time::{Clock, FakeClock, Timestamp};

use super::{Mail, STORE_DIR, mail, open_store};
use crate::Source;

fn clock() -> Box<dyn Clock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

fn account() -> inboxcraft_model::Account {
    let imap = Server::new(Host::new("imap.example.org").unwrap(), Security::Tls.imap_port(), Security::Tls);
    inboxcraft_model::Account::new(AccountId::new(1), "Home", EmailAddress::new("anna@example.org").unwrap(), "anna", imap).unwrap()
}

/// A session on the store in `dir` with one account and a command that queues a kept job.
fn session_in(dir: &Path) -> Session {
    let mut store = open_store(dir).unwrap();
    if store.accounts().unwrap().is_empty() {
        store.apply(&Batch::new().with(Change::PutAccount(account()))).unwrap();
    }
    let mut registry = Registry::builtin().unwrap();
    // A command that queues a kept job, as the acts on mail do.
    let run: inboxcraft_engine::Run = |_, _| Ok(Outcome::new(Default::default()).with_kept_job(AccountId::new(1), JobKind::SyncFolderList));
    registry.register(CommandSpec { id: "test.keeps", class: Some(AgentClass::Safe), summary: "A test command", run }).unwrap();
    Session::with_registry(store, clock(), registry).unwrap()
}

#[test]
fn the_store_is_kept_between_runs() {
    let dir = tempfile::tempdir().unwrap();
    drop(session_in(dir.path()));
    let store = open_store(dir.path()).unwrap();
    assert_eq!(store.accounts().unwrap(), [account()]);
    assert!(dir.path().join(inboxcraft_store::DATABASE_FILE).is_file());
}

#[test]
fn queued_jobs_survive_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    let mut first = session_in(dir.path());
    first.run("test.keeps", &Default::default()).unwrap();
    let sent = first.take_jobs();
    assert_eq!(sent.len(), 1);
    // The app ends before the worker ran the job.
    drop(first);

    let mut second = session_in(dir.path());
    second.start();
    let jobs: Vec<Job> = second.take_jobs();
    assert_eq!(jobs.first(), sent.first(), "the job runs again, under its id");
    assert_eq!(jobs.get(1).map(|job| (job.id.get(), &job.kind)), Some((2, &JobKind::SyncFolderList)), "then the start's own, past it");
}

#[test]
fn a_store_that_does_not_open_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(inboxcraft_store::DATABASE_FILE), b"not a database, written by something else").unwrap();
    let error = open_store(dir.path()).err().unwrap();
    assert!(error.contains("could not be opened") && error.contains(&dir.path().display().to_string()), "{error}");
}

/// Set in the child run of [`child_opens_the_owner_s_mail`]; without it that test does nothing.
const CHILD_VAR: &str = "INBOXCRAFT_STORE_CHILD";

/// In a child run: opens the owner's mail and prints where it is kept as `mail=<what>`.
#[test]
fn child_opens_the_owner_s_mail() {
    if std::env::var_os(CHILD_VAR).is_none() {
        return;
    }
    for source in [Source::Sample, Source::Owner] {
        match mail(source) {
            Ok(Mail::Sample) => println!("mail=sample"),
            Ok(Mail::Store(store)) => println!("mail=store with {} accounts", store.accounts().unwrap().len()),
            Err(error) => println!("mail=error {error}"),
        }
    }
}

/// What a child run of this test binary prints, with only `vars` of the folder variables set.
fn child_with(vars: &[(&str, &Path)]) -> Vec<String> {
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command.args(["store::tests::child_opens_the_owner_s_mail", "--exact", "--nocapture", "--test-threads=1"]).env(CHILD_VAR, "1");
    for name in ["HOME", "USERPROFILE", "XDG_DATA_HOME", "LOCALAPPDATA"] {
        command.env_remove(name);
    }
    command.envs(vars.iter().copied());
    let output = command.output().unwrap();
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout).unwrap().lines().filter_map(|line| line.rsplit_once("mail=").map(|(_, mail)| mail.to_owned())).collect()
}

#[test]
fn the_owner_s_mail_is_kept_in_the_data_folder() {
    let data = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    assert_eq!(child_with(&[("XDG_DATA_HOME", data.path())]), ["sample", "store with 0 accounts"]);
    assert!(data.path().join(STORE_DIR).join(inboxcraft_store::DATABASE_FILE).is_file(), "in XDG_DATA_HOME, where the Flatpak points it");
    assert_eq!(child_with(&[("HOME", home.path()), ("XDG_DATA_HOME", Path::new("relative"))]), ["sample", "store with 0 accounts"]);
    let fallback = if cfg!(windows) || cfg!(target_os = "macos") { None } else { Some(home.path().join(".local/share").join(STORE_DIR)) };
    if let Some(fallback) = fallback {
        assert!(fallback.join(inboxcraft_store::DATABASE_FILE).is_file(), "a relative XDG_DATA_HOME is not taken");
    }
}

#[test]
fn without_a_data_folder_the_window_does_not_open() {
    let shown = child_with(&[]);
    assert_eq!(shown.first().map(String::as_str), Some("sample"), "the sample needs no folder");
    assert!(shown.get(1).is_some_and(|error| error.starts_with("error ") && error.contains("no folder")), "{shown:?}");
}
