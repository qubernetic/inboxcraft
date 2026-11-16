// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use inboxcraft_model::{AccountId, JobKind};
use serde_json::{Value, json};

use crate::test_support::session;
use crate::{Caller, Session};

fn params() -> Value {
    json!({
        "address": "carol@example.org",
        "label": "Allotment",
        "display_name": "Carol Example",
        "username": "carol",
        "imap": { "host": "imap.example.org", "port": 143, "security": "starttls" },
    })
}

fn listed(session: &mut Session, id: u64) -> Value {
    let accounts = session.run("account.list", &json!({})).unwrap()["accounts"].as_array().unwrap().clone();
    accounts.into_iter().find(|account| account["id"] == json!(id)).unwrap()
}

/// Account 3, added and its first check out of the queue.
fn with_added() -> Session {
    let mut session = session();
    session.run("account.add", &params()).unwrap();
    session.take_jobs();
    session
}

#[test]
fn an_update_changes_what_it_names_and_keeps_the_rest() {
    let mut session = with_added();
    let answer = session.run("account.update", &json!({ "account": 3, "label": "Garden", "display_name": null })).unwrap();
    assert_eq!(answer["account"]["label"], json!("Garden"));
    assert_eq!(answer["started"], json!(false), "a new name sends no password anywhere");
    assert!(session.take_jobs().is_empty());
    let account = listed(&mut session, 3);
    assert_eq!(account["label"], json!("Garden"));
    assert_eq!(account["display_name"], Value::Null);
    assert_eq!(account["username"], json!("carol"));
    assert_eq!(account["imap"]["port"], json!(143));
}

#[test]
fn an_update_that_moves_the_sign_in_checks_it_again_as_the_owners() {
    for change in [
        json!({ "imap": { "host": "other.example.org", "port": 143, "security": "starttls" } }),
        json!({ "imap": { "host": "imap.example.org", "port": 1143, "security": "starttls" } }),
        json!({ "imap": { "host": "imap.example.org", "port": 143, "security": "tls" } }),
        json!({ "submission": { "host": "smtp.example.org", "security": "tls" } }),
        json!({ "username": "carol2" }),
    ] {
        let mut session = with_added();
        let mut params = change.clone();
        params["account"] = json!(3);
        let answer = session.run("account.update", &params).unwrap();
        assert_eq!(answer["started"], json!(true), "{change}");
        let jobs = session.take_jobs();
        assert_eq!(jobs.len(), 1, "{change}");
        assert_eq!((jobs[0].account, &jobs[0].kind), (AccountId::new(3), &JobKind::Check { by_owner: true }), "{change}");
        assert_eq!(listed(&mut session, 3)["check"]["running"], json!(true), "{change}");
    }
}

#[test]
fn the_submission_server_is_set_on_its_usual_port_and_cleared_with_null() {
    let mut session = with_added();
    session.run("account.update", &json!({ "account": 3, "submission": { "host": "smtp.example.org", "security": "tls" } })).unwrap();
    assert_eq!(listed(&mut session, 3)["submission"], json!({ "host": "smtp.example.org", "port": 465, "security": "tls" }));
    session.take_jobs();
    session.run("account.update", &json!({ "account": 3, "submission": null })).unwrap();
    assert_eq!(listed(&mut session, 3)["submission"], Value::Null);
}

#[test]
fn a_dry_run_changes_nothing_and_starts_nothing() {
    let mut session = with_added();
    let before = listed(&mut session, 3);
    let answer = session.run("account.update", &json!({ "account": 3, "username": "new", "dry_run": true })).unwrap();
    assert_eq!(answer["started"], json!(false));
    assert!(session.take_jobs().is_empty());
    assert_eq!(listed(&mut session, 3), before);
}

#[test]
fn an_update_is_validated_like_an_add_and_says_nothing_of_what_it_was_given() {
    let mut session = with_added();
    for bad in [
        json!({ "account": 3, "imap": { "host": "evil\r\nA1 LOGIN", "security": "tls" } }),
        json!({ "account": 3, "username": "carol\r\nA1 LOGIN" }),
        json!({ "account": 3, "username": "" }),
        json!({ "account": 3, "imap": { "host": "imap.example.org", "security": "none" } }),
        json!({ "account": 3, "password": "tulip-Meadow-7-quartz" }),
    ] {
        let error = session.run("account.update", &bad).unwrap_err().to_string();
        assert!(!error.contains("tulip") && !error.contains("LOGIN"), "{error}");
    }
    assert!(session.take_jobs().is_empty());
    let unknown = session.run("account.update", &json!({ "account": 99, "label": "x" })).unwrap_err().to_string();
    assert!(unknown.contains("99"), "{unknown}");
}

#[test]
fn a_recheck_asks_the_owners_check_and_its_sign_in_starts_the_sync_again() {
    use inboxcraft_model::{CheckStep, ErrorKind, StepStatus};
    let mut session = session();
    let answer = session.run("account.update", &json!({ "account": 1, "recheck": true })).unwrap();
    assert_eq!(answer["started"], json!(true));
    let jobs = session.take_jobs();
    assert_eq!(jobs.iter().map(|job| job.kind.clone()).collect::<Vec<_>>(), [JobKind::Check { by_owner: true }]);
    // A refused sign-in starts nothing, and the next check that signs in still does.
    step(&mut session, 1, jobs[0].id, CheckStep::ImapLogin, StepStatus::Failed(ErrorKind::LoginRefused));
    step(&mut session, 1, jobs[0].id, CheckStep::CaldavDiscovery, StepStatus::NotSet);
    assert!(session.take_jobs().is_empty());
    session.run("account.check", &json!({ "account": 1 })).unwrap();
    let job = session.take_jobs()[0].id;
    step(&mut session, 1, job, CheckStep::ImapLogin, StepStatus::Passed);
    step(&mut session, 1, job, CheckStep::CaldavDiscovery, StepStatus::NotSet);
    assert_eq!(session.take_jobs().iter().map(|job| job.kind.clone()).collect::<Vec<_>>(), [JobKind::SyncFolderList]);
    // Once only: the next check of an account with folders is an ordinary one.
    session.run("account.check", &json!({ "account": 1 })).unwrap();
    let job = session.take_jobs()[0].id;
    step(&mut session, 1, job, CheckStep::ImapLogin, StepStatus::Passed);
    assert!(session.take_jobs().is_empty());
}

#[test]
fn an_update_cannot_become_another_account() {
    let mut session = with_added();
    session
        .run("account.add", &json!({ "address": "dan@example.org", "username": "dan", "imap": { "host": "imap.example.org", "security": "tls" } }))
        .unwrap();
    let error = session.run("account.update", &json!({ "account": 3, "username": "dan" })).unwrap_err().to_string();
    assert!(error.contains("already there"), "{error}");
    // Its own settings again are no clash.
    session.run("account.update", &json!({ "account": 3, "username": "carol" })).unwrap();
}

#[test]
fn an_agent_may_not_update_an_account() {
    let mut session = with_added();
    let error = session.run_as(Caller::Agent, "account.update", &json!({ "account": 3, "label": "x" })).unwrap_err().to_string();
    assert!(error.contains("owner"), "{error}");
    assert!(session.take_jobs().is_empty());
}

#[test]
fn an_added_account_gets_the_next_id_and_its_first_check_by_the_owner() {
    let mut session = session();
    let answer = session.run("account.add", &params()).unwrap();
    assert_eq!(answer["account"]["id"], json!(3));
    assert_eq!(answer["account"]["label"], json!("Allotment"));
    assert_eq!(answer["account"]["imap"], json!({ "host": "imap.example.org", "port": 143, "security": "start_tls" }));
    assert_eq!(answer["started"], json!(true));
    assert_eq!(
        answer["steps"][0],
        json!({ "step": "dns", "status": "pending", "kind": null, "message": null, "host": null, "port": null, "issuer": null })
    );
    let jobs = session.take_jobs();
    assert_eq!(jobs.len(), 1);
    assert_eq!((jobs[0].account, &jobs[0].kind), (AccountId::new(3), &JobKind::Check { by_owner: true }));
    let account = listed(&mut session, 3);
    assert_eq!(account["address"], json!("carol@example.org"));
    assert_eq!(account["display_name"], json!("Carol Example"));
    assert_eq!(account["username"], json!("carol"));
    assert_eq!(account["check"]["running"], json!(true));
}

#[test]
fn the_user_name_label_and_port_have_defaults() {
    let mut session = session();
    let answer =
        session.run("account.add", &json!({ "address": "dan@example.org", "imap": { "host": "mail.example.org", "security": "tls" } })).unwrap();
    let account = &answer["account"];
    assert_eq!(account["username"], json!("dan@example.org"));
    assert_eq!(account["label"], json!("dan@example.org"));
    assert_eq!(account["display_name"], json!(null));
    assert_eq!(account["imap"]["port"], json!(993));
    let starttls =
        session.run("account.add", &json!({ "address": "eve@example.org", "imap": { "host": "mail.example.org", "security": "starttls" } })).unwrap();
    assert_eq!(starttls["account"]["imap"]["port"], json!(143));
}

#[test]
fn ids_go_on_after_the_highest_held() {
    let mut session = session();
    session.run("account.add", &params()).unwrap();
    let next =
        session.run("account.add", &json!({ "address": "dan@example.org", "imap": { "host": "imap.example.org", "security": "tls" } })).unwrap();
    assert_eq!(next["account"]["id"], json!(4));
}

#[test]
fn an_agent_may_not_add_an_account() {
    let mut session = session();
    let error = session.run_as(Caller::Agent, "account.add", &params()).unwrap_err();
    assert_eq!(error.code(), "not_allowed");
    assert_eq!(session.registry().get("account.add").and_then(|spec| spec.class).map(|class| class.name()), Some("owner"));
    assert!(session.take_jobs().is_empty());
    assert_eq!(session.run("account.list", &json!({})).unwrap()["accounts"].as_array().unwrap().len(), 2);
}

#[test]
fn the_same_sign_in_on_the_same_server_is_not_added_twice() {
    let mut session = session();
    session.run("account.add", &params()).unwrap();
    session.take_jobs();
    let mut again = params();
    again["imap"]["host"] = json!("IMAP.example.org.");
    again["label"] = json!("Another name");
    let error = session.run("account.add", &again).unwrap_err();
    assert_eq!(error.code(), "bad_params");
    assert!(error.to_string().contains("already"), "{error}");
    assert!(session.take_jobs().is_empty());
    // Another user name on the same server is another account.
    again["username"] = json!("carol.allotment");
    assert_eq!(session.run("account.add", &again).unwrap()["account"]["id"], json!(4));
}

#[test]
fn bad_settings_are_refused_and_change_nothing() {
    let mut session = session();
    let cases = [
        json!({}),
        json!({ "address": "not an address", "imap": { "host": "imap.example.org", "security": "tls" } }),
        json!({ "address": "carol@example.org", "imap": { "host": "bad host!", "security": "tls" } }),
        json!({ "address": "carol@example.org", "imap": { "host": "imap.example.org", "security": "none" } }),
        json!({ "address": "carol@example.org", "imap": { "host": "imap.example.org", "security": "tls", "port": 0 } }),
        json!({ "address": "carol@example.org", "imap": { "host": "imap.example.org", "security": "tls", "port": 70000 } }),
        json!({ "address": "carol@example.org", "username": "", "imap": { "host": "imap.example.org", "security": "tls" } }),
        json!({ "address": "carol@example.org", "username": "carol\r\nA1 LOGOUT", "imap": { "host": "imap.example.org", "security": "tls" } }),
        json!({ "address": "carol@example.org", "username": "x".repeat(321), "imap": { "host": "imap.example.org", "security": "tls" } }),
        json!({ "address": "carol@example.org", "imap": { "host": "imap.example.org", "security": "tls", "typo": 1 } }),
        json!({ "address": "carol@example.org", "imap": "imap.example.org" }),
    ];
    for params in cases {
        assert_eq!(session.run("account.add", &params).unwrap_err().code(), "bad_params", "{params}");
    }
    assert!(session.take_jobs().is_empty());
    assert_eq!(session.run("account.list", &json!({})).unwrap()["accounts"].as_array().unwrap().len(), 2);
}

#[test]
fn a_password_is_never_a_parameter_and_is_not_repeated() {
    let mut session = session();
    let secret = "correct-horse-battery-staple";
    for field in ["password", "secret", "token"] {
        let mut with = params();
        with[field] = json!(secret);
        let error = session.run("account.add", &with).unwrap_err();
        assert_eq!(error.code(), "bad_params");
        assert!(!error.to_string().contains(secret), "{error}");
        assert!(!format!("{error:?}").contains(secret), "{error:?}");
    }
    // A secret where a setting belongs is not repeated either.
    let mut misplaced = params();
    misplaced["imap"]["port"] = json!(secret);
    let error = session.run("account.add", &misplaced).unwrap_err();
    assert!(!format!("{error} {error:?}").contains(secret), "{error:?}");
    assert!(session.take_jobs().is_empty());
}

#[test]
fn display_names_and_labels_are_cleaned() {
    let mut session = session();
    let mut dirty = params();
    dirty["label"] = json!("  Work\r\n\u{202e}  ");
    dirty["display_name"] = json!("Carol\u{0}\r\nBcc: x@example.net");
    let account = session.run("account.add", &dirty).unwrap()["account"].clone();
    let label = account["label"].as_str().unwrap();
    let name = account["display_name"].as_str().unwrap();
    for text in [label, name] {
        assert!(!text.chars().any(|c| c.is_control() || c == '\u{202e}'), "{text:?}");
    }
}

fn step(session: &mut Session, account: u64, job: inboxcraft_model::JobId, step: inboxcraft_model::CheckStep, status: inboxcraft_model::StepStatus) {
    use inboxcraft_model::{Event, EventKind, StepReport};
    let event = Event::new(AccountId::new(account), Some(job), EventKind::CheckStep(StepReport::new(step, status)));
    assert_eq!(session.apply(&event).unwrap(), crate::Applied::Changed);
}

#[test]
fn a_new_account_starts_syncing_once_its_first_check_signs_in() {
    use inboxcraft_model::{CheckStep, ErrorKind, StepStatus};
    let mut session = session();
    session.run("account.add", &params()).unwrap();
    let job = session.take_jobs()[0].id;
    for done in [CheckStep::Dns, CheckStep::Tcp, CheckStep::Tls, CheckStep::Certificate] {
        step(&mut session, 3, job, done, StepStatus::Passed);
    }
    assert!(session.take_jobs().is_empty(), "nothing syncs before the sign-in passed");
    step(&mut session, 3, job, CheckStep::ImapLogin, StepStatus::Passed);
    let jobs = session.take_jobs();
    assert_eq!(jobs.iter().map(|job| (job.account, job.kind.clone())).collect::<Vec<_>>(), [(AccountId::new(3), JobKind::SyncFolderList)]);

    // A refused sign-in starts nothing; the owner fixes the settings first.
    let mut other = params();
    other["username"] = json!("carol2");
    session.run("account.add", &other).unwrap();
    let job = session.take_jobs()[0].id;
    step(&mut session, 4, job, CheckStep::ImapLogin, StepStatus::Failed(ErrorKind::LoginRefused));
    assert!(session.take_jobs().is_empty());
}

#[test]
fn a_check_of_an_account_with_folders_starts_no_sync() {
    use inboxcraft_model::{CheckStep, StepStatus};
    let mut session = session();
    session.run("account.check", &json!({ "account": 1 })).unwrap();
    let job = session.take_jobs()[0].id;
    step(&mut session, 1, job, CheckStep::ImapLogin, StepStatus::Passed);
    assert!(session.take_jobs().is_empty());
}

#[test]
fn a_removed_account_goes_with_its_mail_its_queued_jobs_and_its_state() {
    use inboxcraft_model::{CheckStep, StepStatus};
    let mut session = session();
    // The check `account.add` queued is still waiting for the worker.
    session.run("account.add", &params()).unwrap();
    let answer = session.run("account.remove", &json!({ "account": 3 })).unwrap();
    assert_eq!(answer, json!({ "account": 3, "removed": true }));
    assert!(session.take_jobs().is_empty(), "the queued check of the removed account is dropped");
    let ids: Vec<Value> = session.run("account.list", &json!({})).unwrap()["accounts"].as_array().unwrap().iter().map(|a| a["id"].clone()).collect();
    assert_eq!(ids, [json!(1), json!(2)]);
    // A late step of its check names an account the session no longer holds.
    let late = inboxcraft_model::Event::new(
        AccountId::new(3),
        Some(inboxcraft_model::JobId::new(1)),
        inboxcraft_model::EventKind::CheckStep(inboxcraft_model::StepReport::new(CheckStep::Dns, StepStatus::Passed)),
    );
    assert_eq!(session.apply(&late).unwrap(), crate::Applied::Dropped);

    // An account with mail loses its folders and messages too, and the others keep theirs.
    session.run("account.remove", &json!({ "account": 1 })).unwrap();
    assert!(session.store().folders(AccountId::new(1)).unwrap().is_empty());
    assert_eq!(session.run("mail.list", &json!({ "role": "inbox" })).unwrap()["messages"].as_array().unwrap().len(), 2);
    // The same settings can be added again, under an id no account had before.
    assert_eq!(session.run("account.add", &params()).unwrap()["account"]["id"], json!(4));
}

#[test]
fn only_the_owner_removes_an_account_that_is_there() {
    let mut session = session();
    assert_eq!(session.run_as(Caller::Agent, "account.remove", &json!({ "account": 1 })).unwrap_err().code(), "not_allowed");
    assert_eq!(session.run("account.remove", &json!({ "account": 9 })).unwrap_err().code(), "not_found");
    for params in [json!({}), json!({ "account": "1" }), json!({ "account": 1, "typo": 1 })] {
        assert_eq!(session.run("account.remove", &params).unwrap_err().code(), "bad_params", "{params}");
    }
    assert_eq!(session.run("account.list", &json!({})).unwrap()["accounts"].as_array().unwrap().len(), 2);
}

#[test]
fn an_account_takes_a_submission_server_with_its_usual_port() {
    let mut session = session();
    let mut with = params();
    with["submission"] = json!({ "host": "smtp.example.org", "security": "starttls" });
    let answer = session.run("account.add", &with).unwrap();
    assert_eq!(answer["account"]["submission"], json!({ "host": "smtp.example.org", "port": 587, "security": "start_tls" }));
    let mut tls = json!({ "address": "dan@example.org", "imap": { "host": "imap.example.org", "security": "tls" } });
    tls["submission"] = json!({ "host": "smtp.example.org", "security": "tls" });
    assert_eq!(session.run("account.add", &tls).unwrap()["account"]["submission"]["port"], json!(465));
    let mut bad = json!({ "address": "eve@example.org", "imap": { "host": "imap.example.org", "security": "tls" } });
    bad["submission"] = json!({ "host": "smtp example", "security": "tls" });
    assert!(session.run("account.add", &bad).is_err());
    let none = listed(&mut session, 3);
    assert_eq!(none["submission"]["host"], json!("smtp.example.org"));
}
