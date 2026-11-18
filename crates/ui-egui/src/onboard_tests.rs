// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use inboxcraft_engine::Session;
use inboxcraft_model::{Account, AccountId, CheckStep, ErrorKind, Host, Job, JobKind, StepStatus};
use inboxcraft_store::{Memory, Sqlite, Storage, StoreConfig};
use inboxcraft_time::{FakeClock, Timestamp};
use serde_json::json;

use super::{
    Form, Keychain, MAX_PASSWORD_CHARS, NoDetect, NoKeychain, Onboarding, Outcome, Proposal, Proposed, Secret, Security, Slot, Source, Stage,
    StepState, Suggestion,
};
use crate::mailbox::Mailbox;
use crate::workers::{NoWorkers, Scripted, Workers};

/// What the tests type as the password.
pub(crate) const PASSWORD: &str = "tulip-Meadow-7-quartz";

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

fn mailbox_over(store: Box<dyn Storage>) -> Mailbox {
    Mailbox::new(Session::new(store, clock()).unwrap(), clock())
}

pub(crate) fn mailbox() -> Mailbox {
    mailbox_over(Box::new(Memory::new(StoreConfig::default())))
}

/// A credential store in memory that writes down what it was given, or refuses.
#[derive(Clone, Default)]
pub(crate) struct Kept {
    pub(crate) passwords: Arc<Mutex<BTreeMap<u64, String>>>,
    pub(crate) refuse: bool,
    /// While on, only `forget` refuses.
    pub(crate) refuse_forget: Arc<AtomicBool>,
    /// Every account `forget` was asked for, in order.
    pub(crate) forgotten: Arc<Mutex<Vec<u64>>>,
}

impl Keychain for Kept {
    fn store(&self, account: u64, password: &str) -> Result<(), String> {
        if self.refuse {
            return Err("the Secret Service is locked\u{202e}".to_owned());
        }
        self.passwords.lock().unwrap().insert(account, password.to_owned());
        Ok(())
    }

    fn forget(&self, account: u64) -> Result<(), String> {
        self.forgotten.lock().unwrap().push(account);
        if self.refuse || self.refuse_forget.load(Ordering::SeqCst) {
            return Err("locked".to_owned());
        }
        self.passwords.lock().unwrap().remove(&account);
        Ok(())
    }
}

pub(crate) fn proposal(source: Source) -> Proposal {
    Proposal {
        submission: vec![],
        imap: vec![
            Suggestion { host: "imap.example.org".to_owned(), port: 993, security: Security::Tls, username: "carol@example.org".to_owned(), source },
            Suggestion { host: "mail.example.org".to_owned(), port: 143, security: Security::StartTls, username: "carol".to_owned(), source },
        ],
        asked: vec!["dns MX example.org".to_owned(), "https://autoconfig.example.org/mail/config-v1.1.xml\r\n".to_owned()],
        failed: vec![],
    }
}

pub(crate) type FakeWorkers = Scripted;

/// Account 1 added over `kept`, its check done, and onboarding over `kept` closed.
fn added(kept: &Kept) -> (Mailbox, Onboarding) {
    let mut mailbox = mailbox();
    let mut onboarding = onboarding(kept);
    filled(onboarding.form_mut());
    onboarding.form_mut().username = "carol".to_owned();
    onboarding.form_mut().smtp_host = "smtp.example.org".to_owned();
    onboarding.add(&mut mailbox);
    assert_eq!(onboarding.error(), None);
    let mut workers = Scripted::passing();
    mailbox.pump(&mut workers);
    onboarding.close();
    (mailbox, onboarding)
}

fn listed(mailbox: &mut Mailbox) -> serde_json::Value {
    mailbox.run("account.list", &json!({})).unwrap()["accounts"][0].clone()
}

#[test]
fn the_form_sends_the_outgoing_server_and_refuses_a_bad_caldav_address() {
    let mut form = Form::default();
    filled(&mut form);
    assert_eq!(form.params().unwrap()["submission"], serde_json::Value::Null);
    form.smtp_host = " smtp.example.org ".to_owned();
    form.smtp_security = Some(Security::StartTls);
    assert_eq!(form.params().unwrap()["submission"], json!({ "host": "smtp.example.org", "port": 587, "security": "start_tls" }));
    form.smtp_security = None;
    assert_eq!(form.params().unwrap()["submission"]["port"], json!(465));
    form.smtp_port = "x".to_owned();
    assert_eq!(form.params().unwrap_err(), "The port is a number from 1 to 65535.");
    form.smtp_port.clear();
    for bad in ["http://dav.example.org", "https://carol:secret@dav.example.org", "dav.example.org"] {
        form.caldav = bad.to_owned();
        let refused = form.params().unwrap_err();
        assert_eq!(refused, "The CalDAV address starts with https:// and holds no user name.", "{bad}");
        assert!(!refused.contains("secret"));
    }
}

#[test]
fn detect_fills_the_outgoing_server_and_flags_a_guess() {
    let guess =
        Suggestion { host: "smtp.example.org".to_owned(), port: 465, security: Security::Tls, username: String::new(), source: Source::Guess };
    let found = Proposal { submission: vec![guess], ..proposal(Source::Autoconfig) };
    let mut onboarding = Onboarding::new(Arc::new(NoKeychain), Box::new(Proposed::new(found)));
    onboarding.open();
    onboarding.form_mut().address = "carol@example.org".to_owned();
    onboarding.detect();
    assert!(onboarding.poll());
    assert_eq!(onboarding.form().smtp_host, "smtp.example.org");
    assert_eq!(onboarding.form().smtp_port, "465");
    assert_eq!(onboarding.form().smtp_security, Some(Security::Tls));
    assert_eq!(onboarding.found().unwrap().outgoing, Some(("smtp.example.org".to_owned(), Source::Guess)));
}

#[test]
fn adding_with_a_caldav_address_sets_it_and_a_store_refusal_keeps_nothing() {
    let kept = Kept::default();
    let mut mailbox = mailbox();
    let mut onboarding = onboarding(&kept);
    filled(onboarding.form_mut());
    onboarding.form_mut().caldav = "https://dav.example.org/".to_owned();
    onboarding.add(&mut mailbox);
    assert_eq!(onboarding.stage(), Stage::Checking(1));
    assert_eq!(listed(&mut mailbox)["caldav"], json!("https://dav.example.org/"));
}

#[test]
fn an_account_opens_with_all_but_its_password_and_cancel_changes_nothing() {
    let kept = Kept::default();
    let (mut mailbox, mut onboarding) = added(&kept);
    onboarding.edit(&mut mailbox, 1);
    assert_eq!(onboarding.stage(), Stage::Edit(1));
    let form = onboarding.form();
    assert_eq!(form.address, "carol@example.org");
    assert_eq!(form.username, "carol");
    assert_eq!(form.host, "imap.example.org");
    assert_eq!(form.port, "993");
    assert_eq!(form.smtp_host, "smtp.example.org");
    assert_eq!(form.smtp_port, "465");
    assert!(form.password.is_empty(), "never the stored password");
    assert!(!onboarding.save_checks());
    onboarding.form_mut().host = "other.example.org".to_owned();
    onboarding.close();
    assert_eq!(onboarding.stage(), Stage::Closed);
    assert_eq!(listed(&mut mailbox)["imap"]["host"], json!("imap.example.org"));
    onboarding.edit(&mut mailbox, 99);
    assert_eq!(onboarding.stage(), Stage::Closed);
    assert!(onboarding.error().is_some());
}

#[test]
fn saving_only_a_name_closes_and_sends_no_password_anywhere() {
    let kept = Kept::default();
    let (mut mailbox, mut onboarding) = added(&kept);
    onboarding.edit(&mut mailbox, 1);
    onboarding.form_mut().label = "Allotment".to_owned();
    assert!(!onboarding.save_checks());
    onboarding.save(&mut mailbox);
    assert_eq!(onboarding.error(), None);
    assert_eq!(onboarding.stage(), Stage::Closed);
    assert_eq!(listed(&mut mailbox)["label"], json!("Allotment"));
    let mut workers = Scripted::passing();
    mailbox.pump(&mut workers);
    assert!(workers.sent.is_empty(), "{:?}", workers.sent);
}

#[test]
fn saving_with_nothing_changed_just_closes() {
    let kept = Kept::default();
    let (mut mailbox, mut onboarding) = added(&kept);
    onboarding.edit(&mut mailbox, 1);
    onboarding.save(&mut mailbox);
    assert_eq!(onboarding.stage(), Stage::Closed);
}

#[test]
fn a_changed_server_says_where_the_sign_in_goes_and_checks_it_as_the_owners() {
    let kept = Kept::default();
    let (mut mailbox, mut onboarding) = added(&kept);
    onboarding.edit(&mut mailbox, 1);
    assert_eq!(onboarding.sign_in_note(), None);
    onboarding.form_mut().host = "moved.example.org".to_owned();
    onboarding.form_mut().smtp_host = "out.example.org".to_owned();
    onboarding.form_mut().caldav = "https://dav.example.org/".to_owned();
    assert!(onboarding.save_checks());
    let note = onboarding.sign_in_note().unwrap();
    for server in ["moved.example.org:993", "out.example.org:465", "https://dav.example.org/"] {
        assert!(note.contains(server), "{note}");
    }
    onboarding.save(&mut mailbox);
    assert_eq!(onboarding.error(), None);
    assert_eq!(onboarding.stage(), Stage::Checking(1));
    let account = listed(&mut mailbox);
    assert_eq!(account["imap"]["host"], json!("moved.example.org"));
    assert_eq!(account["submission"]["host"], json!("out.example.org"));
    assert_eq!(account["caldav"], json!("https://dav.example.org/"));
    let mut workers = Scripted::passing();
    mailbox.pump(&mut workers);
    assert!(workers.sent.iter().any(|(_, kind)| *kind == JobKind::Check { by_owner: true }), "{:?}", workers.sent);
}

#[test]
fn a_new_password_replaces_the_stored_one_and_is_checked() {
    let kept = Kept::default();
    let (mut mailbox, mut onboarding) = added(&kept);
    onboarding.edit(&mut mailbox, 1);
    onboarding.form_mut().password = Secret::new("new-Lantern-9");
    assert!(onboarding.save_checks());
    assert!(onboarding.sign_in_note().unwrap().contains("imap.example.org:993"));
    onboarding.save(&mut mailbox);
    assert_eq!(onboarding.error(), None);
    assert_eq!(onboarding.stage(), Stage::Checking(1));
    assert_eq!(kept.passwords.lock().unwrap().get(&1).map(String::as_str), Some("new-Lantern-9"));
    assert!(onboarding.form().password.is_empty(), "forgotten in the window");
    assert!(!listed(&mut mailbox).to_string().contains("new-Lantern-9"));
    let mut workers = Scripted::passing();
    mailbox.pump(&mut workers);
    let id = AccountId::new(1);
    assert_eq!(workers.sent, [(id, JobKind::Check { by_owner: true }), (id, JobKind::SyncFolderList)], "the sync starts again once it signed in");
}

#[test]
fn an_empty_password_field_keeps_the_stored_password() {
    let kept = Kept::default();
    let (mut mailbox, mut onboarding) = added(&kept);
    onboarding.edit(&mut mailbox, 1);
    onboarding.form_mut().host = "moved.example.org".to_owned();
    onboarding.save(&mut mailbox);
    assert_eq!(kept.passwords.lock().unwrap().get(&1).map(String::as_str), Some(PASSWORD));
}

#[test]
fn a_refused_form_replaces_no_password() {
    let kept = Kept::default();
    let (mut mailbox, mut onboarding) = added(&kept);
    onboarding.edit(&mut mailbox, 1);
    onboarding.form_mut().password = Secret::new("new-Lantern-9");
    onboarding.form_mut().host = "bad host\r\n".to_owned();
    onboarding.save(&mut mailbox);
    assert_eq!(onboarding.stage(), Stage::Edit(1));
    assert!(onboarding.error().is_some());
    assert_eq!(kept.passwords.lock().unwrap().get(&1).map(String::as_str), Some(PASSWORD), "the stored one stays");
    assert!(!onboarding.form().password.is_empty(), "kept to be corrected and sent again");
    onboarding.form_mut().host.clear();
    onboarding.save(&mut mailbox);
    assert_eq!(onboarding.error(), Some("Enter the IMAP server, or use Detect."));
    assert_eq!(kept.passwords.lock().unwrap().get(&1).map(String::as_str), Some(PASSWORD));
}

#[test]
fn when_the_credential_store_refuses_nothing_changes() {
    let mut kept = Kept::default();
    let (mut mailbox, mut onboarding) = added(&kept);
    kept.refuse = true;
    let mut onboarding_over = Onboarding::new(Arc::new(kept.clone()), Box::new(NoDetect::default()));
    onboarding.close();
    onboarding_over.edit(&mut mailbox, 1);
    onboarding_over.form_mut().host = "moved.example.org".to_owned();
    onboarding_over.form_mut().password = Secret::new("new-Lantern-9");
    onboarding_over.save(&mut mailbox);
    assert_eq!(onboarding_over.stage(), Stage::Edit(1));
    let error = onboarding_over.error().unwrap();
    assert!(error.contains("nothing was changed"), "{error}");
    assert!(!error.contains("new-Lantern-9"), "{error}");
    assert!(onboarding_over.form().password.is_empty());
    assert_eq!(listed(&mut mailbox)["imap"]["host"], json!("imap.example.org"));
    assert_eq!(kept.passwords.lock().unwrap().get(&1).map(String::as_str), Some(PASSWORD));
}

#[test]
fn a_check_still_running_holds_a_save_back_and_a_new_name_is_not() {
    let kept = Kept::default();
    let mut mailbox = mailbox();
    let mut onboarding = onboarding(&kept);
    filled(onboarding.form_mut());
    onboarding.add(&mut mailbox);
    onboarding.close();
    onboarding.edit(&mut mailbox, 1);
    onboarding.form_mut().password = Secret::new("new-Lantern-9");
    onboarding.save(&mut mailbox);
    assert_eq!(onboarding.stage(), Stage::Edit(1));
    assert!(onboarding.error().unwrap().contains("still running"));
    assert_eq!(kept.passwords.lock().unwrap().get(&1).map(String::as_str), Some(PASSWORD));
    onboarding.form_mut().password.clear();
    onboarding.form_mut().label = "Allotment".to_owned();
    onboarding.save(&mut mailbox);
    assert_eq!(onboarding.error(), None);
    assert_eq!(onboarding.stage(), Stage::Closed);
}

#[test]
fn changing_the_settings_of_an_edited_account_removes_nothing() {
    let kept = Kept::default();
    let (mut mailbox, mut onboarding) = added(&kept);
    onboarding.edit(&mut mailbox, 1);
    onboarding.form_mut().host = "moved.example.org".to_owned();
    onboarding.save(&mut mailbox);
    assert_eq!(onboarding.stage(), Stage::Checking(1));
    onboarding.change(&mut mailbox);
    assert_eq!(onboarding.stage(), Stage::Edit(1));
    assert_eq!(onboarding.form().host, "moved.example.org");
    assert_eq!(mailbox.accounts().len(), 1);
    assert!(kept.forgotten.lock().unwrap().is_empty());
}

pub(crate) fn filled(form: &mut Form) {
    form.address = "carol@example.org".to_owned();
    form.display_name = "Carol Example".to_owned();
    form.password = Secret::new(PASSWORD);
    form.host = "imap.example.org".to_owned();
}

fn onboarding(kept: &Kept) -> Onboarding {
    let mut onboarding = Onboarding::new(Arc::new(kept.clone()), Box::new(NoDetect::default()));
    onboarding.open();
    onboarding
}

#[test]
fn the_form_asks_for_what_is_missing_and_never_puts_the_password_in_the_parameters() {
    let mut form = Form::default();
    assert_eq!(form.params().unwrap_err(), "Enter the address.");
    form.address = " carol@example.org ".to_owned();
    assert_eq!(form.params().unwrap_err(), "Enter the password.");
    form.password = Secret::new(PASSWORD);
    assert_eq!(form.params().unwrap_err(), "Enter the IMAP server, or use Detect.");
    form.host = "imap.example.org".to_owned();
    let params = form.params().unwrap();
    assert_eq!(
        params,
        json!({
            "address": "carol@example.org",
            "label": null,
            "display_name": null,
            "username": null,
            "imap": { "host": "imap.example.org", "port": 993, "security": "tls" },
            "submission": null,
        })
    );
    assert!(!params.to_string().contains(PASSWORD));
    form.security = Some(Security::StartTls);
    form.username = " carol ".to_owned();
    let params = form.params().unwrap();
    assert_eq!(params["imap"]["port"], json!(143));
    assert_eq!(params["imap"]["security"], json!("start_tls"));
    assert_eq!(params["username"], json!("carol"));
    for port in ["0", "65536", "imap", "-1"] {
        form.port = port.to_owned();
        assert_eq!(form.params().unwrap_err(), "The port is a number from 1 to 65535.", "{port}");
    }
    form.port = " 1143 ".to_owned();
    assert_eq!(form.params().unwrap()["imap"]["port"], json!(1143));
}

#[test]
fn a_secret_hides_itself_and_is_overwritten_when_cleared() {
    let mut secret = Secret::new(PASSWORD);
    assert_eq!(format!("{secret:?}"), "Secret(hidden)");
    let mut form = Form::default();
    filled(&mut form);
    assert!(!format!("{form:?}").contains(PASSWORD));
    assert!(!secret.is_empty());
    secret.clear();
    assert!(secret.is_empty());
    assert_eq!(secret.expose(), "");
    secret.expose_mut().push('x');
    assert_eq!(secret.expose(), "x");
}

#[test]
fn a_secret_keeps_one_buffer_for_its_longest_password() {
    let mut secret = Secret::default();
    let capacity = secret.expose_mut().capacity();
    assert!(capacity >= MAX_PASSWORD_CHARS * 4);
    let long = Secret::new(&"\u{1F512}".repeat(MAX_PASSWORD_CHARS + 1));
    assert_eq!(long.expose().chars().count(), MAX_PASSWORD_CHARS, "cut to the field's limit");
    secret.expose_mut().push_str(long.expose());
    assert_eq!(secret.expose_mut().capacity(), capacity, "the longest password fits without a new buffer");
    secret.clear();
    assert!(secret.is_empty());
    assert_eq!(secret.expose_mut().capacity(), capacity, "clearing keeps the buffer for the next password");
}

#[test]
fn the_window_s_end_forgets_every_password_it_stored_and_keeps_what_the_store_refused() {
    let kept = Kept::default();
    let mut mailbox = mailbox();
    let mut onboarding = onboarding(&kept);
    for address in ["carol@example.org", "dave@example.org"] {
        onboarding.open();
        filled(onboarding.form_mut());
        onboarding.form_mut().address = address.to_owned();
        onboarding.add(&mut mailbox);
    }
    assert_eq!(kept.passwords.lock().unwrap().len(), 2);
    kept.refuse_forget.store(true, Ordering::SeqCst);
    onboarding.forget_passwords();
    assert_eq!(kept.passwords.lock().unwrap().len(), 2);
    kept.refuse_forget.store(false, Ordering::SeqCst);
    kept.forgotten.lock().unwrap().clear();
    onboarding.forget_passwords();
    assert_eq!(kept.forgotten.lock().unwrap().as_slice(), [1, 2], "both tried again");
    assert!(kept.passwords.lock().unwrap().is_empty());
    kept.forgotten.lock().unwrap().clear();
    onboarding.forget_passwords();
    assert!(kept.forgotten.lock().unwrap().is_empty(), "nothing left to forget");
}

#[test]
fn detect_fills_the_form_with_its_best_server_and_says_where_it_came_from() {
    let mut detect = Proposed::new(proposal(Source::Guess));
    let asked = Arc::clone(&detect.asked);
    detect.proposal.failed.push("dns SRV _imaps._tcp.example.org: timed out".to_owned());
    let mut onboarding = Onboarding::new(Arc::new(NoKeychain), Box::new(detect));
    onboarding.open();
    assert!(!onboarding.poll(), "nothing asked yet");
    onboarding.detect();
    assert_eq!(onboarding.error(), Some("Enter the address, then Detect."));
    assert!(asked.lock().unwrap().is_empty());
    onboarding.form_mut().address = " carol@example.org ".to_owned();
    onboarding.form_mut().ispdb = true;
    onboarding.detect();
    assert!(onboarding.detecting());
    assert_eq!(asked.lock().unwrap().as_slice(), [("carol@example.org".to_owned(), true)]);
    assert!(onboarding.poll());
    assert!(!onboarding.detecting());
    let form = onboarding.form();
    assert_eq!(
        (form.host.as_str(), form.port.as_str(), form.security, form.username.as_str()),
        ("imap.example.org", "993", Some(Security::Tls), "carol@example.org")
    );
    let found = onboarding.found().unwrap();
    assert_eq!(found.source, Some(Source::Guess));
    assert!(found.source.unwrap().doubtful());
    assert_eq!(found.asked[1], "https://autoconfig.example.org/mail/config-v1.1.xml  ", "line breaks from outside are shown as spaces");
    assert_eq!(found.failed.len(), 1);
    assert!(!onboarding.poll());
}

#[test]
fn detect_that_finds_nothing_leaves_the_form_as_it_was() {
    let mut onboarding = Onboarding::default();
    onboarding.open();
    onboarding.form_mut().address = "carol@example.org".to_owned();
    onboarding.form_mut().host = "typed.example.org".to_owned();
    onboarding.detect();
    assert!(onboarding.poll());
    assert_eq!(onboarding.form().host, "typed.example.org");
    let found = onboarding.found().unwrap();
    assert_eq!(found.source, None);
    assert_eq!(found.failed, ["Detect does not run in this window."]);
}

#[test]
fn every_source_has_its_words_and_only_redirects_and_guesses_are_doubtful() {
    for source in [Source::Autoconfig, Source::AutoconfigRedirect, Source::Ispdb, Source::Srv, Source::Guess] {
        assert!(!source.text().is_empty());
        assert_eq!(source.doubtful(), matches!(source, Source::AutoconfigRedirect | Source::Guess));
    }
    assert_eq!((Security::Tls.text(), Security::StartTls.text()), ("TLS", "STARTTLS"));
    assert_eq!((Security::Tls.imap_port(), Security::StartTls.imap_port()), (993, 143));
}

#[test]
fn adding_keeps_the_password_in_the_credential_store_only_and_shows_the_check() {
    let kept = Kept::default();
    let mut mailbox = mailbox();
    let mut onboarding = onboarding(&kept);
    filled(onboarding.form_mut());
    onboarding.add(&mut mailbox);
    assert_eq!(onboarding.error(), None);
    assert_eq!(onboarding.stage(), Stage::Checking(1));
    assert_eq!(kept.passwords.lock().unwrap().get(&1).map(String::as_str), Some(PASSWORD));
    assert!(onboarding.form().password.is_empty(), "the password is forgotten in the window");
    assert_eq!(mailbox.accounts().len(), 1);
    assert_eq!(onboarding.outcome(), Outcome::Running);
    let steps = onboarding.steps();
    assert_eq!(steps.len(), 7);
    assert!(steps.iter().all(|step| step.state == StepState::Pending));
    assert_eq!(steps.iter().map(super::StepRow::name).collect::<Vec<_>>()[4], "IMAP sign-in");
}

#[test]
fn the_steps_show_as_the_worker_reports_them_and_a_passed_sign_in_starts_the_sync() {
    let kept = Kept::default();
    let mut mailbox = mailbox();
    let mut onboarding = onboarding(&kept);
    filled(onboarding.form_mut());
    onboarding.add(&mut mailbox);
    let mut workers = FakeWorkers::passing();
    assert!(mailbox.pump(&mut workers));
    onboarding.load(&mut mailbox);
    assert_eq!(onboarding.outcome(), Outcome::Passed);
    let steps = onboarding.steps();
    assert_eq!(steps[0].state, StepState::Passed);
    assert_eq!(steps[0].server.as_deref(), Some("imap.example.org:993"));
    assert_eq!(steps[3].issuer.as_deref(), Some("Example Test CA"));
    assert_eq!(steps[6].state, StepState::NotSet);
    let account = AccountId::new(1);
    assert_eq!(workers.sent, [(account, JobKind::Check { by_owner: true }), (account, JobKind::SyncFolderList)]);
    assert_eq!(workers.kept, [account]);
    onboarding.close();
    assert_eq!(onboarding.stage(), Stage::Closed);
    assert_eq!(mailbox.accounts().len(), 1, "closing keeps the account");
}

#[test]
fn after_a_failed_step_the_owner_checks_again_or_changes_the_settings() {
    let kept = Kept::default();
    let mut mailbox = mailbox();
    let mut onboarding = onboarding(&kept);
    filled(onboarding.form_mut());
    onboarding.form_mut().label = "Allotment".to_owned();
    onboarding.add(&mut mailbox);
    let failed = [(CheckStep::Dns, StepStatus::Passed), (CheckStep::Tcp, StepStatus::Failed(ErrorKind::MailPortsBlocked))];
    let mut workers = FakeWorkers::answering(&failed);
    mailbox.pump(&mut workers);
    onboarding.load(&mut mailbox);
    assert_eq!(onboarding.outcome(), Outcome::Failed);
    assert_eq!(onboarding.steps()[1].state, StepState::Failed);
    assert_eq!(onboarding.steps()[1].message.as_deref(), Some(ErrorKind::MailPortsBlocked.to_string().as_str()));
    assert_eq!(onboarding.steps()[2].state, StepState::Skipped);

    onboarding.check_again(&mut mailbox);
    assert_eq!(onboarding.outcome(), Outcome::Running);
    mailbox.pump(&mut workers);
    assert_eq!(workers.sent.last().map(|(_, kind)| kind.clone()), Some(JobKind::Check { by_owner: true }));

    onboarding.change(&mut mailbox);
    assert_eq!(onboarding.stage(), Stage::Form);
    assert_eq!(onboarding.error(), None);
    assert!(mailbox.accounts().is_empty());
    assert!(kept.passwords.lock().unwrap().is_empty(), "the password goes with the account");
    let form = onboarding.form();
    assert_eq!((form.address.as_str(), form.label.as_str(), form.host.as_str()), ("carol@example.org", "Allotment", "imap.example.org"));
    assert!(form.password.is_empty());
    mailbox.pump(&mut workers);
    assert!(workers.kept.is_empty(), "the removed account's worker is stopped");
    // Typed again, the settings are added under a new id.
    onboarding.form_mut().password = Secret::new(PASSWORD);
    onboarding.add(&mut mailbox);
    assert_eq!(onboarding.stage(), Stage::Checking(2));
}

#[test]
fn a_held_sign_in_reads_as_held() {
    let mut mailbox = mailbox();
    let mut onboarding = onboarding(&Kept::default());
    filled(onboarding.form_mut());
    onboarding.add(&mut mailbox);
    let mut workers = FakeWorkers::answering(&[(CheckStep::ImapLogin, StepStatus::NeedsOwner)]);
    mailbox.pump(&mut workers);
    onboarding.load(&mut mailbox);
    assert_eq!(onboarding.steps()[4].state, StepState::Held);
    assert_eq!(onboarding.outcome(), Outcome::Failed);
}

#[test]
fn a_credential_store_that_refuses_adds_no_account() {
    let kept = Kept { refuse: true, ..Kept::default() };
    let mut mailbox = mailbox();
    let mut onboarding = onboarding(&kept);
    filled(onboarding.form_mut());
    onboarding.add(&mut mailbox);
    let error = onboarding.error().unwrap();
    assert!(error.starts_with("The system's credential store did not take the password"), "{error}");
    assert!(!error.contains('\u{202e}'));
    assert!(!error.contains(PASSWORD));
    assert_eq!(onboarding.stage(), Stage::Form);
    assert!(mailbox.accounts().is_empty());
    assert!(onboarding.form().password.is_empty());
    let mut workers = FakeWorkers::passing();
    mailbox.pump(&mut workers);
    assert!(workers.sent.is_empty(), "the check of the removed account never reaches a worker");
    // A window without a credential store says so.
    let mut plain = Onboarding::default();
    plain.open();
    filled(plain.form_mut());
    plain.add(&mut mailbox);
    assert!(plain.error().unwrap().contains("This window keeps no passwords."));
}

#[test]
fn bad_settings_stop_before_anything_is_added() {
    let kept = Kept::default();
    let mut mailbox = mailbox();
    let mut onboarding = onboarding(&kept);
    filled(onboarding.form_mut());
    onboarding.form_mut().host = "bad host!".to_owned();
    onboarding.add(&mut mailbox);
    assert!(onboarding.error().unwrap().contains("account.add"));
    assert_eq!(onboarding.stage(), Stage::Form);
    assert!(kept.passwords.lock().unwrap().is_empty());
    assert!(!onboarding.form().password.is_empty(), "the password stays typed for the next try");
    onboarding.form_mut().address.clear();
    onboarding.add(&mut mailbox);
    assert_eq!(onboarding.error(), Some("Enter the address."));
}

#[test]
fn removing_whose_password_the_store_keeps_says_so() {
    let kept = Kept::default();
    let mut mailbox = mailbox();
    let mut onboarding = onboarding(&kept);
    filled(onboarding.form_mut());
    onboarding.add(&mut mailbox);
    let stuck = Kept { refuse: true, passwords: Arc::clone(&kept.passwords), ..Kept::default() };
    let mut other = Onboarding::new(Arc::new(stuck), Box::new(NoDetect::default()));
    other.stage = Stage::Checking(1);
    other.change(&mut mailbox);
    assert!(other.error().unwrap().starts_with("The password could not be removed"));
    // The account is gone already: removing it again is the engine's refusal.
    other.stage = Stage::Checking(1);
    other.change(&mut mailbox);
    assert!(other.error().unwrap().contains("there is no account 1"));
}

#[test]
fn outside_the_check_the_check_actions_do_nothing() {
    let mut mailbox = mailbox();
    let mut onboarding = onboarding(&Kept::default());
    onboarding.load(&mut mailbox);
    onboarding.check_again(&mut mailbox);
    onboarding.change(&mut mailbox);
    assert_eq!(onboarding.stage(), Stage::Form);
    assert!(onboarding.steps().is_empty());
    // An account that is gone reads as a check with no steps.
    onboarding.stage = Stage::Checking(9);
    onboarding.running = true;
    onboarding.load(&mut mailbox);
    assert!(onboarding.steps().is_empty());
    assert_eq!(onboarding.outcome(), Outcome::Failed);
}

fn files_under(dir: &Path) -> Vec<Vec<u8>> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            found.extend(files_under(&path));
        } else {
            found.push(std::fs::read(&path).unwrap());
        }
    }
    found
}

fn holds(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|window| window == needle)
}

#[test]
fn no_secret_reaches_the_store_files_or_the_log() {
    let dir = tempfile::Builder::new().prefix("onboard-").tempdir().unwrap();
    let store = Sqlite::open(dir.path(), StoreConfig::default()).unwrap();
    let mut mailbox = mailbox_over(Box::new(store));
    let kept = Kept::default();
    let mut detect = Proposed::new(proposal(Source::Autoconfig));
    detect.proposal.imap[0].host = "imap.example.org".to_owned();
    let mut onboarding = Onboarding::new(Arc::new(kept.clone()), Box::new(detect));
    onboarding.open();
    // Everything the window writes down along the way: errors, answers, debug output.
    let mut log = Vec::new();
    onboarding.form_mut().address = "carol@example.org".to_owned();
    onboarding.form_mut().password = Secret::new(PASSWORD);
    log.push(format!("{onboarding:?}"));
    onboarding.detect();
    onboarding.poll();
    log.push(format!("{onboarding:?} {:?}", onboarding.found()));
    // A first try the engine refuses, a password typed into a field where it does not belong.
    onboarding.form_mut().port = PASSWORD.to_owned();
    onboarding.add(&mut mailbox);
    log.push(onboarding.error().unwrap_or_default().to_owned());
    onboarding.form_mut().port.clear();
    onboarding.add(&mut mailbox);
    log.push(format!("{onboarding:?} {:?}", onboarding.error()));
    let mut workers = FakeWorkers::passing();
    mailbox.pump(&mut workers);
    onboarding.load(&mut mailbox);
    log.push(format!("{onboarding:?} {mailbox:?} {:?}", mailbox.error()));
    log.push(mailbox.run("account.list", &json!({})).unwrap().to_string());
    log.push(format!("{:?}", workers.sent));
    assert_eq!(onboarding.outcome(), Outcome::Passed);
    assert_eq!(kept.passwords.lock().unwrap().get(&1).map(String::as_str), Some(PASSWORD), "the credential store has it");
    for line in &log {
        assert!(!line.contains(PASSWORD), "the log holds the password: {line}");
    }
    drop(mailbox);
    let files = files_under(dir.path());
    assert!(!files.is_empty());
    for file in &files {
        assert!(!holds(file, PASSWORD.as_bytes()), "a store file holds the password");
    }
    // The account itself is in the store.
    assert!(files.iter().any(|file| holds(file, b"carol@example.org")));
}

#[test]
fn no_workers_run_nothing_and_answer_nothing() {
    let mut workers = NoWorkers;
    let mut mailbox = mailbox();
    let mut onboarding = onboarding(&Kept::default());
    filled(onboarding.form_mut());
    onboarding.add(&mut mailbox);
    assert!(!mailbox.pump(&mut workers));
    let account = Account::new(
        AccountId::new(1),
        "",
        inboxcraft_model::EmailAddress::new("carol@example.org").unwrap(),
        "carol",
        inboxcraft_model::Server::new(
            Host::new("imap.example.org").unwrap(),
            inboxcraft_model::Security::Tls.imap_port(),
            inboxcraft_model::Security::Tls,
        ),
    )
    .unwrap();
    workers.send(&account, Job::new(inboxcraft_model::JobId::new(1), AccountId::new(1), JobKind::SyncFolderList));
    workers.keep(&[]);
    assert!(workers.events().is_empty());
}

#[test]
fn a_threaded_detect_answers_from_its_thread_and_wakes_the_window() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    let woken = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&woken);
    let run: super::Run = Arc::new(|address: &str, ispdb: bool| Proposal { asked: vec![format!("{address} {ispdb}")], ..Proposal::default() });
    let wake: crate::fetch::Wake = Arc::new(move || {
        counter.fetch_add(1, Ordering::SeqCst);
    });
    let mut detect = super::Threaded::new(run, Some(wake));
    assert!(format!("{detect:?}").starts_with("Threaded"));
    assert!(super::Detect::finished(&mut detect).is_none());
    super::Detect::start(&mut detect, 7, "carol@example.org", true);
    let mut answer = None;
    for _ in 0..2000 {
        answer = super::Detect::finished(&mut detect);
        if answer.is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let (run, answer) = answer.unwrap();
    assert_eq!((run, answer.asked), (7, vec!["carol@example.org true".to_owned()]));
    for _ in 0..2000 {
        if woken.load(Ordering::SeqCst) > 0 {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(woken.load(Ordering::SeqCst), 1);
}

#[test]
fn a_fault_names_its_field_typed_ones_as_they_are_typed() {
    let mut form = Form::default();
    assert!(form.typed_faults().is_empty(), "an empty form is not wrong, only unfinished");
    assert_eq!(form.fault(true).map(|(slot, _)| slot), Some(Slot::Address));
    assert_eq!(form.fault(false).map(|(slot, _)| slot), Some(Slot::Host), "an account's form has its address and password");
    filled(&mut form);
    assert_eq!(form.fault(true), None);
    form.port = "99999".to_owned();
    form.smtp_port = "smtp".to_owned();
    form.caldav = "http://dav.example.org/".to_owned();
    let typed: Vec<Slot> = form.typed_faults().into_iter().map(|(slot, _)| slot).collect();
    assert_eq!(typed, [Slot::Port, Slot::SmtpPort, Slot::Caldav]);
    // A port typed wrong stops the form even without its server: what is typed is never sent half-read.
    assert_eq!(form.fault(true), Some((Slot::Port, "The port is a number from 1 to 65535.".to_owned())));
    form.password.clear();
    assert_eq!(form.fault(true).map(|(slot, _)| slot), Some(Slot::Password));
}

#[test]
fn a_missing_field_s_error_stands_under_it_until_it_is_typed_in() {
    let kept = Kept::default();
    let mut mailbox = mailbox();
    let mut onboarding = onboarding(&kept);
    onboarding.form_mut().address = "carol@example.org".to_owned();
    onboarding.add(&mut mailbox);
    assert_eq!((onboarding.error(), onboarding.error_slot()), (Some("Enter the password."), Some(Slot::Password)));
    onboarding.edited(Slot::Host);
    assert_eq!(onboarding.error_slot(), Some(Slot::Password), "another field leaves it");
    onboarding.edited(Slot::Password);
    assert_eq!((onboarding.error(), onboarding.error_slot()), (None, None));
    // The engine's refusal belongs to no field.
    filled(onboarding.form_mut());
    onboarding.form_mut().host = "bad host!".to_owned();
    onboarding.add(&mut mailbox);
    assert!(onboarding.error().is_some());
    assert_eq!(onboarding.error_slot(), None);
    assert!(mailbox.accounts().is_empty());
}

#[test]
fn what_was_typed_is_kept_on_every_way_out_and_back_but_cancel() {
    let kept = Kept::default();
    let mut mailbox = mailbox();
    let mut onboarding = Onboarding::new(Arc::new(kept.clone()), Box::new(Proposed::new(proposal(Source::Srv))));
    onboarding.open();
    filled(onboarding.form_mut());
    onboarding.form_mut().label = "Allotment".to_owned();
    onboarding.form_mut().port = "1143".to_owned();
    onboarding.detect();
    assert!(onboarding.poll());
    let as_left = |onboarding: &Onboarding| {
        let form = onboarding.form();
        (form.address.clone(), form.label.clone(), form.display_name.clone(), form.host.clone(), form.port.clone())
    };
    let left = as_left(&onboarding);
    assert_eq!(left.3, "imap.example.org");
    // Back, Esc or another page: closed and opened again, all but the password is there, and what Detect found.
    onboarding.close();
    assert_eq!(onboarding.stage(), Stage::Closed);
    onboarding.open();
    assert_eq!(as_left(&onboarding), left);
    assert!(onboarding.form().password.is_empty(), "the password is dropped on the way out");
    assert!(onboarding.found().is_some());
    // An account's form in between leaves the add form as it was.
    filled(onboarding.form_mut());
    onboarding.form_mut().address = "dave@example.org".to_owned();
    onboarding.add(&mut mailbox);
    onboarding.close();
    onboarding.open();
    assert_eq!(onboarding.form().address, "", "an added account's form starts the next one empty");
    onboarding.form_mut().address = "erin@example.org".to_owned();
    onboarding.edit(&mut mailbox, 1);
    assert_eq!(onboarding.form().address, "dave@example.org");
    onboarding.close();
    onboarding.open();
    assert_eq!(onboarding.form().address, "erin@example.org");
    // Opened again while open, nothing changes.
    onboarding.open();
    assert_eq!(onboarding.form().address, "erin@example.org");
    // Cancel drops it.
    onboarding.cancel();
    onboarding.open();
    assert_eq!(onboarding.form().address, "");
    assert!(onboarding.found().is_none());
}

#[test]
fn detect_keeps_what_the_owner_typed_and_refills_what_it_put_there() {
    let found = Proposal {
        submission: vec![Suggestion {
            host: "smtp.example.org".to_owned(),
            port: 465,
            security: Security::Tls,
            username: String::new(),
            source: Source::Srv,
        }],
        ..proposal(Source::Srv)
    };
    let mut onboarding = Onboarding::new(Arc::new(NoKeychain), Box::new(Proposed::new(found)));
    onboarding.open();
    let form = onboarding.form_mut();
    form.address = "carol@example.org".to_owned();
    form.host = "mail.typed.example".to_owned();
    form.username = "carol.typed".to_owned();
    onboarding.detect();
    assert!(onboarding.poll());
    let form = onboarding.form();
    assert_eq!((form.host.as_str(), form.port.as_str(), form.security), ("mail.typed.example", "", None), "the typed row stays");
    assert_eq!(form.username, "carol.typed");
    assert_eq!((form.smtp_host.as_str(), form.smtp_port.as_str()), ("smtp.example.org", "465"), "the empty row is filled");
    // Detect again: what it put there it may change; the owner's change to its row is kept.
    onboarding.form_mut().smtp_port = "587".to_owned();
    onboarding.detect();
    assert!(onboarding.poll());
    assert_eq!(onboarding.form().smtp_port, "587");
    onboarding.form_mut().host.clear();
    onboarding.form_mut().username.clear();
    onboarding.detect();
    assert!(onboarding.poll());
    assert_eq!((onboarding.form().host.as_str(), onboarding.form().username.as_str()), ("imap.example.org", "carol@example.org"));
}

/// A Detect whose runs answer only when the test lets them: each start takes the next queued proposal, and
/// [`Held::answer`] hands the oldest started run's answer over at the next poll.
#[derive(Clone, Default)]
struct Held {
    queued: Arc<Mutex<std::collections::VecDeque<Proposal>>>,
    started: Arc<Mutex<std::collections::VecDeque<(u64, Proposal)>>>,
    ready: Arc<Mutex<std::collections::VecDeque<(u64, Proposal)>>>,
}

impl Held {
    fn new(answers: Vec<Proposal>) -> Self {
        let held = Self::default();
        held.queued.lock().unwrap().extend(answers);
        held
    }

    fn answer(&self) {
        let next = self.started.lock().unwrap().pop_front().unwrap();
        self.ready.lock().unwrap().push_back(next);
    }
}

impl super::Detect for Held {
    fn start(&mut self, run: u64, _address: &str, _ispdb: bool) {
        let next = self.queued.lock().unwrap().pop_front().unwrap_or_default();
        self.started.lock().unwrap().push_back((run, next));
    }

    fn finished(&mut self) -> Option<(u64, Proposal)> {
        self.ready.lock().unwrap().pop_front()
    }
}

/// What Detect finds at `host` (and `smtp`, when given) for the user `user`.
fn found_at(host: &str, smtp: Option<&str>, user: &str) -> Proposal {
    let suggestion =
        |host: &str, port, security| Suggestion { host: host.to_owned(), port, security, username: user.to_owned(), source: Source::Autoconfig };
    Proposal {
        imap: vec![suggestion(host, 993, Security::Tls)],
        submission: smtp.map(|host| vec![suggestion(host, 465, Security::Tls)]).unwrap_or_default(),
        asked: vec!["dns MX example.org".to_owned()],
        failed: vec![],
    }
}

#[test]
fn detect_s_late_answer_never_lands_in_an_account_s_form() {
    let kept = Kept::default();
    // An account with no outgoing server.
    let mut mailbox = mailbox();
    let mut first = onboarding(&kept);
    filled(first.form_mut());
    first.add(&mut mailbox);
    assert_eq!(first.stage(), Stage::Checking(1), "{:?}", first.error());
    let detect = Held::new(vec![found_at("imap.other.example.org", Some("smtp.other.example.org"), "x@other.example.org")]);
    let mut onboarding = Onboarding::new(Arc::new(kept.clone()), Box::new(detect.clone()));
    onboarding.open();
    onboarding.form_mut().address = "x@other.example.org".to_owned();
    onboarding.detect();
    // The account's row is pressed before Detect answers.
    onboarding.edit(&mut mailbox, 1);
    assert_eq!(onboarding.stage(), Stage::Edit(1));
    let before = (onboarding.form().host.clone(), onboarding.form().smtp_host.clone(), onboarding.form().username.clone());
    detect.answer();
    onboarding.poll();
    let after = (onboarding.form().host.clone(), onboarding.form().smtp_host.clone(), onboarding.form().username.clone());
    assert_eq!(after, before, "the account's form is as it was");
    assert!(!onboarding.save_checks(), "nothing to save, no sign-in anywhere");
    // Nor in the add form, once it is back: the page was left.
    onboarding.open();
    assert_eq!(onboarding.form().address, "x@other.example.org");
    assert_eq!(onboarding.form().host, "");
    assert!(!onboarding.detecting());
}

#[test]
fn detect_s_answer_after_the_page_was_left_is_thrown_away_and_the_next_one_counts() {
    let detect = Held::new(vec![
        found_at("imap.first.example.org", None, "carol@example.org"),
        found_at("imap.second.example.org", None, "carol@example.org"),
    ]);
    let mut onboarding = Onboarding::new(Arc::new(NoKeychain), Box::new(detect.clone()));
    onboarding.open();
    onboarding.form_mut().address = "carol@example.org".to_owned();
    onboarding.detect();
    onboarding.close();
    onboarding.open();
    assert!(!onboarding.detecting(), "Detect under way ended with the page");
    onboarding.detect();
    // The first run's answer comes now: it is the left page's, and thrown away.
    detect.answer();
    onboarding.poll();
    assert_eq!(onboarding.form().host, "");
    assert!(onboarding.detecting(), "the second run is still under way");
    detect.answer();
    assert!(onboarding.poll());
    assert_eq!(onboarding.form().host, "imap.second.example.org");
    assert!(!onboarding.detecting());
}

#[test]
fn detect_s_answer_for_an_address_no_longer_in_the_form_is_thrown_away() {
    let detect = Held::new(vec![found_at("imap.first.example.org", None, "a@first.example.org")]);
    let mut onboarding = Onboarding::new(Arc::new(NoKeychain), Box::new(detect.clone()));
    onboarding.open();
    onboarding.form_mut().address = "a@first.example.org".to_owned();
    onboarding.detect();
    onboarding.form_mut().address = "b@second.example.org".to_owned();
    detect.answer();
    onboarding.poll();
    assert_eq!((onboarding.form().host.as_str(), onboarding.form().username.as_str()), ("", ""));
    assert!(onboarding.found().is_none());
    assert!(!onboarding.detecting());
}
