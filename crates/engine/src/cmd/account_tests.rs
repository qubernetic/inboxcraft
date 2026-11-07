// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use inboxcraft_model::{AccountId, CheckStep, ErrorKind, Event, EventKind, Host, JobId, JobKind, StepReport, StepStatus};
use serde_json::{Value, json};

use crate::test_support::session;
use crate::{Applied, Caller, Session};

fn report(step: CheckStep, status: StepStatus) -> StepReport {
    StepReport::new(step, status).with_server(Host::new("imap.example.org").unwrap(), 993)
}

fn step_event(job: JobId, step: CheckStep, status: StepStatus) -> Event {
    Event::new(AccountId::new(1), Some(job), EventKind::CheckStep(report(step, status)))
}

/// Starts a check of account 1 as an agent; the id of the Check job it queued.
fn start(session: &mut Session) -> JobId {
    let answer = session.run_as(Caller::Agent, "account.check", &json!({ "account": 1 })).unwrap();
    assert_eq!(answer["started"], json!(true));
    let jobs = session.take_jobs();
    assert_eq!(jobs.len(), 1);
    assert_eq!((jobs[0].account, &jobs[0].kind), (AccountId::new(1), &JobKind::Check { by_owner: false }));
    jobs[0].id
}

fn listed_check(session: &mut Session) -> Value {
    session.run("account.list", &json!({})).unwrap()["accounts"][0]["check"].clone()
}

#[test]
fn a_check_lists_every_step_and_queues_one_job() {
    let mut session = session();
    let answer = session.run_as(Caller::Agent, "account.check", &json!({ "account": 1 })).unwrap();
    assert_eq!(answer["account"], json!(1));
    let steps = answer["steps"].as_array().unwrap();
    let names: Vec<&str> = steps.iter().map(|step| step["step"].as_str().unwrap()).collect();
    assert_eq!(names, ["dns", "tcp", "tls", "certificate", "imap_login", "smtp_login", "caldav_discovery"]);
    let statuses: Vec<&str> = steps.iter().map(|step| step["status"].as_str().unwrap()).collect();
    assert_eq!(statuses, ["pending", "pending", "pending", "pending", "pending", "pending", "pending"]);
    assert_eq!(session.take_jobs().iter().map(|job| job.kind.clone()).collect::<Vec<_>>(), [JobKind::Check { by_owner: false }]);
    assert_eq!(listed_check(&mut session)["running"], json!(true));
}

#[test]
fn the_steps_land_as_the_worker_reports_them_and_the_last_ends_the_check() {
    let mut session = session();
    let job = start(&mut session);
    assert_eq!(session.apply(&step_event(job, CheckStep::Dns, StepStatus::Passed)).unwrap(), Applied::Changed);
    let failed = StepReport::new(CheckStep::Tcp, StepStatus::Failed(ErrorKind::MailPortsBlocked));
    session.apply(&Event::new(AccountId::new(1), Some(job), EventKind::CheckStep(failed))).unwrap();
    let check = listed_check(&mut session);
    assert_eq!(check["running"], json!(true));
    assert_eq!(check["steps"][0]["status"], json!("passed"));
    assert_eq!(check["steps"][0]["host"], json!("imap.example.org"));
    assert_eq!(check["steps"][0]["port"], json!(993));
    assert_eq!(
        check["steps"][1],
        json!({
            "step": "tcp",
            "status": "failed",
            "kind": "mail_ports_blocked",
            "message": ErrorKind::MailPortsBlocked.to_string(),
            "host": null,
            "port": null,
            "issuer": null,
        })
    );
    for step in [CheckStep::Tls, CheckStep::Certificate, CheckStep::ImapLogin] {
        session.apply(&step_event(job, step, StepStatus::Skipped)).unwrap();
    }
    session.apply(&step_event(job, CheckStep::SmtpLogin, StepStatus::NotSet)).unwrap();
    assert_eq!(listed_check(&mut session)["running"], json!(true));
    session.apply(&step_event(job, CheckStep::CaldavDiscovery, StepStatus::NotBuilt)).unwrap();
    let check = listed_check(&mut session);
    assert_eq!(check["running"], json!(false));
    assert_eq!(check["steps"][2]["message"], json!("skipped, because an earlier step failed"));
    // Done: a new check starts, with every step pending again; the worker holds its sign-in when the last one was
    // refused (see `a_check_after_a_refused_sign_in_signs_in_only_for_the_owner`).
    let again = start(&mut session);
    assert_ne!(again, job);
    assert_eq!(listed_check(&mut session)["steps"][0]["status"], json!("pending"));
}

#[test]
fn a_running_check_is_not_started_twice() {
    let mut session = session();
    let job = start(&mut session);
    session.apply(&step_event(job, CheckStep::Dns, StepStatus::Passed)).unwrap();
    let answer = session.run_as(Caller::Agent, "account.check", &json!({ "account": 1 })).unwrap();
    assert_eq!(answer["started"], json!(false));
    assert_eq!(answer["steps"][0]["status"], json!("passed"));
    assert!(session.take_jobs().is_empty());
    // Another account checks on its own.
    let other = session.run_as(Caller::Agent, "account.check", &json!({ "account": 2 })).unwrap();
    assert_eq!(other["started"], json!(true));
    assert_eq!(session.take_jobs().len(), 1);
}

#[test]
fn steps_of_another_job_or_without_a_check_are_dropped() {
    let mut session = session();
    // No check runs yet.
    assert_eq!(session.apply(&step_event(JobId::new(99), CheckStep::Dns, StepStatus::Passed)).unwrap(), Applied::Dropped);
    let job = start(&mut session);
    assert_eq!(session.apply(&step_event(JobId::new(99), CheckStep::Dns, StepStatus::Passed)).unwrap(), Applied::Dropped);
    let unsolicited = Event::new(AccountId::new(1), None, EventKind::CheckStep(report(CheckStep::Dns, StepStatus::Passed)));
    assert_eq!(session.apply(&unsolicited).unwrap(), Applied::Dropped);
    let other_account = Event::new(AccountId::new(2), Some(job), EventKind::CheckStep(report(CheckStep::Dns, StepStatus::Passed)));
    assert_eq!(session.apply(&other_account).unwrap(), Applied::Dropped);
    assert_eq!(listed_check(&mut session)["steps"][0]["status"], json!("pending"));
}

#[test]
fn a_detected_server_not_seen_yet_reads_as_such() {
    let mut session = session();
    let job = start(&mut session);
    session.apply(&step_event(job, CheckStep::ImapLogin, StepStatus::NeedsReview)).unwrap();
    let step = &listed_check(&mut session)["steps"][4];
    assert_eq!(step["status"], json!("needs_review"));
    assert!(step["message"].as_str().unwrap().contains("no password was sent"));
}

#[test]
fn a_check_after_a_refused_sign_in_signs_in_only_for_the_owner() {
    let mut session = session();
    let refused = Event::new(AccountId::new(1), None, EventKind::Failed { kind: ErrorKind::LoginRefused, folder: None });
    session.apply(&refused).unwrap();
    let job = start(&mut session);
    session.apply(&step_event(job, CheckStep::ImapLogin, StepStatus::NeedsOwner)).unwrap();
    let step = &listed_check(&mut session)["steps"][4];
    assert_eq!(step["status"], json!("needs_owner"));
    assert!(step["message"].as_str().unwrap().contains("only a check the owner starts"));
    for step in [CheckStep::SmtpLogin, CheckStep::CaldavDiscovery] {
        session.apply(&step_event(job, step, StepStatus::NotBuilt)).unwrap();
    }
    assert_eq!(session.run("account.check", &json!({ "account": 1 })).unwrap()["started"], json!(true));
    assert_eq!(session.take_jobs().iter().map(|job| job.kind.clone()).collect::<Vec<_>>(), [JobKind::Check { by_owner: true }]);
}

#[test]
fn a_check_of_an_unknown_account_or_with_bad_params_is_refused() {
    let mut session = session();
    assert_eq!(session.run("account.check", &json!({ "account": 9 })).unwrap_err().code(), "not_found");
    for params in [json!({}), json!({ "account": "1" }), json!({ "account": 1, "typo": true }), json!([1])] {
        assert_eq!(session.run("account.check", &params).unwrap_err().code(), "bad_params", "{params}");
    }
    assert!(session.take_jobs().is_empty());
}

#[test]
fn a_new_start_forgets_a_check_whose_worker_ended() {
    let mut session = session();
    let job = start(&mut session);
    session.apply(&step_event(job, CheckStep::Dns, StepStatus::Passed)).unwrap();
    // The workers are started again: the old check's last step never comes.
    session.start();
    assert!(session.take_jobs().iter().all(|job| job.kind == JobKind::SyncFolderList));
    let check = listed_check(&mut session);
    assert_eq!(check["running"], json!(false));
    assert_eq!(check["steps"][0]["status"], json!("passed"));
    assert_eq!(session.apply(&step_event(job, CheckStep::Tcp, StepStatus::Passed)).unwrap(), Applied::Dropped);
    let again = start(&mut session);
    assert_ne!(again, job);
}
