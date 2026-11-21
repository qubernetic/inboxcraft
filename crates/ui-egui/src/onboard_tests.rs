// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};

use inboxcraft_engine::Session;
use inboxcraft_model::{Account, AccountId, CheckStep, ErrorKind, Host, Job, JobKind, StepStatus};
use inboxcraft_store::{Memory, Sqlite, Storage, StoreConfig};
use inboxcraft_time::{FakeClock, Timestamp};
use serde_json::json;

use super::{
    Checked, Form, Keychain, MAX_PASSWORD_CHARS, NoDetect, NoKeychain, Onboarding, Outcome, Proposal, Proposed, Secret, Security, Slot, Source,
    Stage, StepState, Suggestion,
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
            Suggestion {
                host: "imap.example.org".to_owned(),
                port: 993,
                security: Security::Tls,
                username: "carol@example.org".to_owned(),
                source,
                checked: Checked::Answered,
            },
            Suggestion {
                host: "mail.example.org".to_owned(),
                port: 143,
                security: Security::StartTls,
                username: "carol".to_owned(),
                source,
                checked: Checked::Answered,
            },
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
    let guess = Suggestion {
        host: "smtp.example.org".to_owned(),
        port: 465,
        security: Security::Tls,
        username: String::new(),
        source: Source::Guess,
        checked: Checked::Answered,
    };
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
    assert_eq!(onboarding_over.form().password.expose(), "new-Lantern-9", "kept in the form to try again");
    assert!(onboarding_over.save_summary().changed, "Save stays on");
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
    let every = [Source::Autoconfig, Source::AutoconfigRedirect, Source::Ispdb, Source::Srv, Source::Provider, Source::Guess];
    for source in every {
        assert!(!source.text().is_empty());
        assert_eq!(source.doubtful(), matches!(source, Source::AutoconfigRedirect | Source::Provider | Source::Guess));
        assert_eq!(every.iter().filter(|other| other.text() == source.text()).count(), 1, "{source:?} has words of its own");
    }
    // The MX provider's servers are a guess, said so, from the MX records.
    let provider = Source::Provider.text();
    assert!(provider.contains("guess") && provider.contains("MX") && provider.contains("check"), "{provider}");
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
    let run: super::Run =
        Arc::new(|address: &str, ispdb: bool, _stop: &super::RunStop| Proposal { asked: vec![format!("{address} {ispdb}")], ..Proposal::default() });
    let wake: crate::fetch::Wake = Arc::new(move || {
        counter.fetch_add(1, Ordering::SeqCst);
    });
    let mut detect = super::Threaded::new(run, Some(wake));
    assert!(format!("{detect:?}").starts_with("Threaded"));
    assert!(super::Detect::finished(&mut detect).is_none());
    super::Detect::start(&mut detect, 7, "carol@example.org", true, super::RunStop::default());
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
            checked: Checked::Answered,
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
pub(crate) struct Held {
    queued: Arc<Mutex<std::collections::VecDeque<Proposal>>>,
    started: Arc<Mutex<std::collections::VecDeque<(u64, Proposal)>>>,
    ready: Arc<Mutex<std::collections::VecDeque<(u64, Proposal)>>>,
    /// The stop of each run started, in order.
    pub(crate) stops: Arc<Mutex<Vec<super::RunStop>>>,
}

impl Held {
    pub(crate) fn new(answers: Vec<Proposal>) -> Self {
        let held = Self::default();
        held.queued.lock().unwrap().extend(answers);
        held
    }

    pub(crate) fn answer(&self) {
        let next = self.started.lock().unwrap().pop_front().unwrap();
        self.ready.lock().unwrap().push_back(next);
    }
}

impl super::Detect for Held {
    fn start(&mut self, run: u64, _address: &str, _ispdb: bool, stop: super::RunStop) {
        self.stops.lock().unwrap().push(stop);
        let next = self.queued.lock().unwrap().pop_front().unwrap_or_default();
        self.started.lock().unwrap().push_back((run, next));
    }

    fn finished(&mut self) -> Option<(u64, Proposal)> {
        self.ready.lock().unwrap().pop_front()
    }
}

/// What Detect finds at `host` (and `smtp`, when given) for the user `user`.
pub(crate) fn found_at(host: &str, smtp: Option<&str>, user: &str) -> Proposal {
    let suggestion = |host: &str, port, security| Suggestion {
        host: host.to_owned(),
        port,
        security,
        username: user.to_owned(),
        source: Source::Autoconfig,
        checked: Checked::Answered,
    };
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

/// Onboarding open on the add form over a Detect that answers `answers` in turn, at once.
fn detecting(answers: Vec<Proposal>) -> (Onboarding, Held) {
    let detect = Held::new(answers);
    let mut onboarding = Onboarding::new(Arc::new(NoKeychain), Box::new(detect.clone()));
    onboarding.open();
    (onboarding, detect)
}

/// Runs Detect for `address` and takes its answer.
fn detect_now(onboarding: &mut Onboarding, detect: &Held, address: &str) {
    onboarding.form_mut().address = address.to_owned();
    onboarding.detect();
    detect.answer();
    assert!(onboarding.poll());
}

#[test]
fn a_detect_that_finds_nothing_clears_what_an_earlier_detect_filled_and_keeps_what_was_typed() {
    let first = found_at("imap.first.example.org", Some("smtp.first.example.org"), "a@first.example.org");
    let (mut onboarding, detect) = detecting(vec![first.clone(), Proposal::default(), first, Proposal::default()]);
    detect_now(&mut onboarding, &detect, "a@first.example.org");
    assert_eq!(onboarding.form().host, "imap.first.example.org");
    detect_now(&mut onboarding, &detect, "b@second.example.org");
    let form = onboarding.form();
    assert_eq!(
        (form.host.as_str(), form.port.as_str(), form.security, form.username.as_str()),
        ("", "", None, ""),
        "the first domain's IMAP row and user name are gone"
    );
    assert_eq!((form.smtp_host.as_str(), form.smtp_port.as_str(), form.smtp_security), ("", "", None), "and its SMTP row");
    let found = onboarding.found().unwrap();
    assert_eq!((found.source, found.outgoing.as_ref()), (None, None), "the note says nothing was found, and nothing is in the form");
    // What the owner typed stays: the user name and the SMTP port here.
    detect_now(&mut onboarding, &detect, "a@first.example.org");
    onboarding.form_mut().username = "a.typed".to_owned();
    onboarding.form_mut().smtp_port = "587".to_owned();
    detect_now(&mut onboarding, &detect, "b@second.example.org");
    let form = onboarding.form();
    assert_eq!((form.host.as_str(), form.username.as_str()), ("", "a.typed"));
    assert_eq!((form.smtp_host.as_str(), form.smtp_port.as_str()), ("", "587"));
}

#[test]
fn a_security_picked_before_detect_never_keeps_the_host_from_being_filled() {
    let (mut onboarding, detect) = detecting(vec![found_at("imap.example.org", Some("smtp.example.org"), "a@example.org")]);
    onboarding.form_mut().security = Some(Security::StartTls);
    onboarding.form_mut().smtp_security = Some(Security::Tls);
    detect_now(&mut onboarding, &detect, "a@example.org");
    let form = onboarding.form();
    assert_eq!(form.host, "imap.example.org");
    assert_eq!(form.smtp_host, "smtp.example.org");
    assert_eq!((form.port.as_str(), form.security), ("993", Some(Security::Tls)), "the encryption goes with Detect's port");
    let found = onboarding.found().unwrap();
    assert_eq!(found.source, Some(Source::Autoconfig), "the note's server is the form's");
}

#[test]
fn a_host_cleared_by_hand_in_a_row_detect_filled_is_filled_again() {
    let answer = found_at("imap.example.org", None, "a@example.org");
    let (mut onboarding, detect) = detecting(vec![answer.clone(), answer]);
    detect_now(&mut onboarding, &detect, "a@example.org");
    onboarding.form_mut().host.clear();
    detect_now(&mut onboarding, &detect, "a@example.org");
    assert_eq!(onboarding.form().host, "imap.example.org");
}

#[test]
fn a_server_the_owner_typed_stays_and_the_note_says_so() {
    let (mut onboarding, detect) = detecting(vec![found_at("imap.example.org", Some("smtp.example.org"), "a@example.org")]);
    onboarding.form_mut().host = "mail.typed.example".to_owned();
    onboarding.form_mut().smtp_host = "out.typed.example".to_owned();
    detect_now(&mut onboarding, &detect, "a@example.org");
    let form = onboarding.form();
    assert_eq!((form.host.as_str(), form.smtp_host.as_str()), ("mail.typed.example", "out.typed.example"));
    let found = onboarding.found().unwrap();
    assert_eq!((found.source, found.outgoing.as_ref()), (None, None), "Detect put no server into the form");
    assert_eq!((found.kept_imap.as_deref(), found.kept_outgoing.as_deref()), (Some("imap.example.org"), Some("smtp.example.org")));
}

#[test]
fn after_change_settings_detect_still_knows_what_it_filled() {
    let kept = Kept::default();
    let mut mailbox = mailbox();
    let detect =
        Held::new(vec![found_at("imap.example.org", None, "carol@example.org"), found_at("imap.new.example.org", None, "carol@new.example.org")]);
    let mut onboarding = Onboarding::new(Arc::new(kept), Box::new(detect.clone()));
    onboarding.open();
    onboarding.form_mut().password = Secret::new(PASSWORD);
    detect_now(&mut onboarding, &detect, "carol@example.org");
    onboarding.add(&mut mailbox);
    assert!(matches!(onboarding.stage(), Stage::Checking(_)), "{:?}", onboarding.error());
    onboarding.change(&mut mailbox);
    assert_eq!(onboarding.stage(), Stage::Form);
    detect_now(&mut onboarding, &detect, "carol@new.example.org");
    assert_eq!((onboarding.form().host.as_str(), onboarding.form().username.as_str()), ("imap.new.example.org", "carol@new.example.org"));
}

#[test]
fn an_account_s_form_that_cannot_open_leaves_the_add_form_as_it_was() {
    let mut mailbox = mailbox();
    let mut onboarding = onboarding(&Kept::default());
    onboarding.form_mut().address = "carol@example.org".to_owned();
    onboarding.form_mut().host = "imap.example.org".to_owned();
    onboarding.edit(&mut mailbox, 4242);
    assert_eq!(onboarding.stage(), Stage::Form);
    assert!(onboarding.error().is_some(), "it says why");
    assert_eq!((onboarding.form().address.as_str(), onboarding.form().host.as_str()), ("carol@example.org", "imap.example.org"));
    onboarding.close();
    onboarding.open();
    assert_eq!(onboarding.form().address, "carol@example.org");
}

/// Accounts 1 (carol) and 2 (dave) added over `kept`, onboarding closed.
fn two_added(kept: &Kept) -> (Mailbox, Onboarding) {
    let (mut mailbox, mut onboarding) = added(kept);
    onboarding.open();
    filled(onboarding.form_mut());
    onboarding.form_mut().address = "dave@example.org".to_owned();
    onboarding.add(&mut mailbox);
    assert_eq!(onboarding.stage(), Stage::Checking(2), "{:?}", onboarding.error());
    onboarding.close();
    (mailbox, onboarding)
}

#[test]
fn an_account_s_form_keeps_what_was_typed_on_every_way_out_and_back_but_cancel() {
    let kept = Kept::default();
    let (mut mailbox, mut onboarding) = two_added(&kept);
    let typed = |onboarding: &mut Onboarding| {
        onboarding.form_mut().host = "imap.typed.example.org".to_owned();
        onboarding.form_mut().label = "Allotment".to_owned();
        onboarding.form_mut().password = Secret::new(PASSWORD);
    };
    let as_typed = |onboarding: &Onboarding| {
        let form = onboarding.form();
        assert_eq!((form.host.as_str(), form.label.as_str()), ("imap.typed.example.org", "Allotment"));
        assert!(form.password.is_empty(), "the password is dropped on the way out");
        assert!(onboarding.save_checks(), "the changed server is still a change");
    };
    // Add account in between.
    onboarding.edit(&mut mailbox, 1);
    typed(&mut onboarding);
    onboarding.open();
    assert_eq!(onboarding.form().address, "", "the add form");
    onboarding.edit(&mut mailbox, 1);
    as_typed(&onboarding);
    // Another account's form in between.
    typed(&mut onboarding);
    onboarding.edit(&mut mailbox, 2);
    assert_eq!((onboarding.form().address.as_str(), onboarding.form().host.as_str()), ("dave@example.org", "imap.example.org"));
    onboarding.edit(&mut mailbox, 1);
    as_typed(&onboarding);
    // Esc, Back, another page.
    typed(&mut onboarding);
    onboarding.close();
    onboarding.edit(&mut mailbox, 1);
    as_typed(&onboarding);
    // The store is as it was all along.
    assert_eq!(listed(&mut mailbox)["imap"]["host"], json!("imap.example.org"));
    // Cancel drops it, and only it.
    onboarding.open();
    onboarding.form_mut().address = "erin@example.org".to_owned();
    onboarding.edit(&mut mailbox, 1);
    onboarding.cancel();
    onboarding.edit(&mut mailbox, 1);
    assert_eq!(onboarding.form().host, "imap.example.org");
    onboarding.open();
    assert_eq!(onboarding.form().address, "erin@example.org", "the add form is still kept");
}

#[test]
fn a_saved_account_s_form_opens_as_the_account_now_stands() {
    let kept = Kept::default();
    let (mut mailbox, mut onboarding) = added(&kept);
    onboarding.edit(&mut mailbox, 1);
    onboarding.form_mut().label = "Allotment".to_owned();
    onboarding.save(&mut mailbox);
    assert_eq!(onboarding.stage(), Stage::Closed);
    assert!(onboarding.kept_edits.is_empty(), "a saved form is not kept as typed: the account holds it now");
    onboarding.edit(&mut mailbox, 1);
    assert_eq!(onboarding.form().label, "Allotment");
    assert!(!onboarding.save_checks());
    onboarding.form_mut().label = "Shed".to_owned();
    onboarding.form_mut().display_name.clear();
    onboarding.save(&mut mailbox);
    assert_eq!(listed(&mut mailbox)["label"], json!("Shed"));
    onboarding.edit(&mut mailbox, 1);
    assert_eq!(onboarding.form().label, "Shed");
}

#[test]
fn every_missing_or_wrong_field_is_named_at_one_press() {
    let mut mailbox = mailbox();
    let mut onboarding = onboarding(&Kept::default());
    onboarding.form_mut().port = "99999".to_owned();
    onboarding.add(&mut mailbox);
    let named: Vec<Slot> = onboarding.field_errors().iter().map(|(slot, _)| *slot).collect();
    assert_eq!(named, [Slot::Address, Slot::Password, Slot::Host, Slot::Port]);
    assert_eq!((onboarding.error(), onboarding.error_slot()), (Some("Enter the address."), Some(Slot::Address)), "the first leads");
    assert_eq!(onboarding.page_error(), None, "each error is its field's");
    // Typing in one field takes its error alone away.
    onboarding.form_mut().address = "carol@example.org".to_owned();
    onboarding.edited(Slot::Address);
    let named: Vec<Slot> = onboarding.field_errors().iter().map(|(slot, _)| *slot).collect();
    assert_eq!(named, [Slot::Password, Slot::Host, Slot::Port]);
    // An account's form names its missing server and its wrong port at once.
    let (mut mailbox, mut onboarding) = added(&Kept::default());
    onboarding.edit(&mut mailbox, 1);
    onboarding.form_mut().host.clear();
    onboarding.form_mut().smtp_port = "smtp".to_owned();
    onboarding.save(&mut mailbox);
    let named: Vec<Slot> = onboarding.field_errors().iter().map(|(slot, _)| *slot).collect();
    assert_eq!(named, [Slot::Host, Slot::SmtpPort]);
}

#[test]
fn detect_s_lines_are_capped_and_matched_once_when_they_come() {
    let many = Proposal {
        asked: (0..10_000).map(|n| format!("dns MX {n}.example.org")).collect(),
        failed: (0..10_000).map(|n| format!("dns MX {n}.example.org: timed out")).chain(["https://example.org: refused".to_owned()]).collect(),
        ..Proposal::default()
    };
    let (mut onboarding, detect) = detecting(vec![many]);
    detect_now(&mut onboarding, &detect, "carol@example.org");
    let found = onboarding.found().unwrap();
    assert_eq!(found.asked_count(), 10_000, "the count says all of them");
    assert!(found.asked.len() <= super::MOST_LINES && found.failed.len() <= super::MOST_LINES, "{} {}", found.asked.len(), found.failed.len());
    let column = found.column();
    assert!(column.len() <= super::MOST_LINES);
    assert_eq!(column[0], ("dns MX 0.example.org".to_owned(), Some("timed out".to_owned())), "each question with why it failed");
    assert_eq!(found.alone(), ["https://example.org: refused"], "a lookup that failed for no question, however far down");
}

#[test]
fn detect_says_when_it_replaced_an_encryption_the_owner_picked() {
    let (mut onboarding, detect) = detecting(vec![found_at("imap.example.org", Some("smtp.example.org"), "a@example.org")]);
    onboarding.form_mut().security = Some(Security::StartTls);
    onboarding.form_mut().smtp_security = Some(Security::Tls);
    detect_now(&mut onboarding, &detect, "a@example.org");
    let found = onboarding.found().unwrap();
    assert_eq!(found.encryption_set, [Some(Security::Tls), None], "the IMAP pick was replaced; the outgoing one was Detect's already");
}

#[test]
fn the_form_says_its_servers_come_from_an_earlier_detect_only_while_they_do() {
    let first = found_at("imap.first.example.org", Some("smtp.first.example.org"), "a@first.example.org");
    let (mut onboarding, detect) = detecting(vec![first.clone(), first]);
    assert!(!onboarding.detect_older(), "nothing detected yet");
    detect_now(&mut onboarding, &detect, "a@first.example.org");
    assert!(!onboarding.detect_older(), "Detect answered for the address in the form");
    // The owner changes the address: the servers below are the earlier answer's.
    onboarding.form_mut().address = "b@second.example.org".to_owned();
    assert!(onboarding.detect_older());
    // Spaces around the same address are no change: Detect asks about the trimmed address.
    onboarding.form_mut().address = " a@first.example.org ".to_owned();
    assert!(!onboarding.detect_older());
    onboarding.form_mut().address = "b@second.example.org".to_owned();
    // A server field edited by hand ends it: the IMAP row's port, then the outgoing row's encryption alone.
    onboarding.form_mut().port = "1993".to_owned();
    assert!(!onboarding.detect_older(), "the IMAP port typed by hand");
    onboarding.form_mut().port = String::new();
    let was = onboarding.form().port.clone();
    assert_eq!(was, "", "the row as Detect left it but its port");
    onboarding.form_mut().port = "993".to_owned();
    assert!(onboarding.detect_older(), "the row as Detect put it again");
    onboarding.form_mut().smtp_security = Some(Security::StartTls);
    assert!(!onboarding.detect_older(), "the outgoing encryption picked by hand");
    onboarding.form_mut().smtp_security = Some(Security::Tls);
    assert!(onboarding.detect_older());
    // Detect runs again: while it runs and after it answered for the address now in the form, nothing is said.
    onboarding.detect();
    assert!(!onboarding.detect_older(), "while Detect runs");
    detect.answer();
    assert!(onboarding.poll());
    assert!(!onboarding.detect_older(), "Detect answered for the new address");
}

#[test]
fn a_form_filled_by_hand_never_says_its_servers_come_from_detect() {
    let mut onboarding = onboarding(&Kept::default());
    filled(onboarding.form_mut());
    onboarding.form_mut().smtp_host = "smtp.example.org".to_owned();
    assert!(!onboarding.detect_older());
    onboarding.form_mut().address = "dave@example.org".to_owned();
    assert!(!onboarding.detect_older(), "the address changed, but no server came from Detect");
}

#[test]
fn the_registered_ports_say_which_encryption_is_usual_and_others_say_nothing() {
    use crate::onboard::unusual_encryption;
    // IMAP 143 STARTTLS, 993 TLS; submission 587 STARTTLS, 465 TLS; 25 STARTTLS: the other encryption is unusual.
    for (port, usual) in [(143, Security::StartTls), (993, Security::Tls), (587, Security::StartTls), (465, Security::Tls), (25, Security::StartTls)]
    {
        let other = if usual == Security::Tls { Security::StartTls } else { Security::Tls };
        assert_eq!(unusual_encryption(&port.to_string(), Some(usual), usual_port), None, "{port} with {usual:?}");
        assert_eq!(unusual_encryption(&port.to_string(), Some(other), usual_port), Some((port, usual)), "{port} with {other:?}");
    }
    // No port typed: the usual one for the encryption, never unusual; no encryption picked is TLS, as the form sends it.
    assert_eq!(unusual_encryption("", Some(Security::StartTls), usual_port), None);
    assert_eq!(unusual_encryption(" 143 ", None, usual_port), Some((143, Security::StartTls)));
    // An unknown port, 0, the highest port and junk warn of nothing (the field's own error speaks for junk).
    for port in ["0", "65535", "1993", "8443", "abc", "99999", "-1", "\u{202e}993"] {
        for security in [Some(Security::Tls), Some(Security::StartTls), None] {
            assert_eq!(unusual_encryption(port, security, usual_port), None, "{port:?} {security:?}");
        }
    }
}

fn usual_port(security: Security) -> u16 {
    security.imap_port()
}

#[test]
fn the_earlier_detect_line_names_the_user_name_while_it_is_still_detects() {
    let first = found_at("imap.first.example.org", None, "a@first.example.org");
    let (mut onboarding, detect) = detecting(vec![first]);
    detect_now(&mut onboarding, &detect, "a@first.example.org");
    assert_eq!(onboarding.form().username, "a@first.example.org", "Detect filled the user name");
    assert!(!onboarding.detect_older_user_name(), "Detect answered for the address in the form");
    onboarding.form_mut().address = "b@second.example.org".to_owned();
    assert!(onboarding.detect_older() && onboarding.detect_older_user_name());
    // The user name typed by hand: the servers are still the earlier Detect's, the user name is not.
    onboarding.form_mut().username = "b".to_owned();
    assert!(onboarding.detect_older() && !onboarding.detect_older_user_name());
    // A server edited by hand: nothing is said, whatever the user name.
    onboarding.form_mut().username = "a@first.example.org".to_owned();
    onboarding.form_mut().host = "imap.second.example.org".to_owned();
    assert!(!onboarding.detect_older() && !onboarding.detect_older_user_name());
}

#[test]
fn a_server_detect_did_not_reach_says_why_beside_its_row() {
    for (checked, imap, outgoing) in [
        (Checked::NoAnswer, "Not checked: imap.example.org did not answer from here.", "Not checked: smtp.example.org did not answer from here."),
        (
            Checked::Refused,
            "Not checked: imap.example.org refused the connection from here.",
            "Not checked: smtp.example.org refused the connection from here.",
        ),
        (Checked::NoName, "Not checked: the name imap.example.org was not found.", "Not checked: the name smtp.example.org was not found."),
        (
            Checked::Clock,
            "Not checked: the certificate of imap.example.org does not fit this computer's clock, which may be what is off.",
            "Not checked: the certificate of smtp.example.org does not fit this computer's clock, which may be what is off.",
        ),
    ] {
        let mut answer = found_at("imap.example.org", Some("smtp.example.org"), "carol@example.org");
        answer.imap[0].checked = checked;
        answer.submission[0].checked = checked;
        let (mut onboarding, detect) = detecting(vec![answer]);
        detect_now(&mut onboarding, &detect, "carol@example.org");
        // The fields hold what the domain names, for the owner to see and edit.
        assert_eq!((onboarding.form().host.as_str(), onboarding.form().smtp_host.as_str()), ("imap.example.org", "smtp.example.org"));
        assert_eq!(onboarding.unchecked(), [Some(imap.to_owned()), Some(outgoing.to_owned())], "{checked:?}");
    }
    // A server that answered says nothing.
    let (mut onboarding, detect) = detecting(vec![found_at("imap.example.org", Some("smtp.example.org"), "carol@example.org")]);
    detect_now(&mut onboarding, &detect, "carol@example.org");
    assert_eq!(onboarding.unchecked(), [None, None]);
}

/// Detect answers for carol@example.org with both servers not reached; the form open and filled.
fn unchecked_both() -> (Onboarding, Held) {
    let mut answer = found_at("imap.example.org", Some("smtp.example.org"), "carol@example.org");
    answer.imap[0].checked = Checked::NoAnswer;
    answer.submission[0].checked = Checked::Refused;
    let (mut onboarding, detect) = detecting(vec![answer, Proposal::default()]);
    detect_now(&mut onboarding, &detect, "carol@example.org");
    assert!(onboarding.unchecked().iter().all(Option::is_some));
    (onboarding, detect)
}

/// An edit of the form, named.
type Edit = (&'static str, fn(&mut Form));

#[test]
fn the_unchecked_line_goes_when_its_row_is_edited() {
    let edits: [Edit; 6] = [
        ("imap host", |form| form.host = "imap2.example.org".to_owned()),
        ("imap port", |form| form.port = "994".to_owned()),
        ("imap security", |form| form.security = Some(Security::StartTls)),
        ("smtp host", |form| form.smtp_host = "smtp2.example.org".to_owned()),
        ("smtp port", |form| form.smtp_port = "466".to_owned()),
        ("smtp security", |form| form.smtp_security = Some(Security::StartTls)),
    ];
    for (at, (what, edit)) in edits.into_iter().enumerate() {
        let (mut onboarding, _detect) = unchecked_both();
        edit(onboarding.form_mut());
        let lines = onboarding.unchecked();
        // The edited row's line goes; the other row's stays.
        let row = at / 3;
        assert!(lines[row].is_none(), "{what}: {lines:?}");
        assert!(lines[1 - row].is_some(), "{what}: {lines:?}");
    }
}

#[test]
fn the_unchecked_line_goes_when_detect_runs_again_even_if_that_run_is_left() {
    let (mut onboarding, detect) = unchecked_both();
    onboarding.detect();
    assert_eq!(onboarding.unchecked(), [None, None], "not while Detect asks");
    // The owner leaves while it asks, and comes back: the earlier answer's lines do not come back with the form.
    onboarding.close();
    onboarding.open();
    assert_eq!(onboarding.form().host, "imap.example.org");
    assert_eq!(onboarding.unchecked(), [None, None]);
    drop(detect);
}

#[test]
fn a_hostile_host_name_in_the_unchecked_line_is_cleaned_and_cut_with_one_ellipsis() {
    let host = format!("\u{202e}evil\u{0}\r\n{}.example.org\u{2066}", "a".repeat(300));
    let mut answer = found_at(&host, None, "carol@example.org");
    answer.imap[0].checked = Checked::NoName;
    let (mut onboarding, detect) = detecting(vec![answer]);
    detect_now(&mut onboarding, &detect, "carol@example.org");
    let [Some(line), None] = onboarding.unchecked() else { panic!("{:?}", onboarding.unchecked()) };
    assert_eq!(line.matches('…').count(), 1, "{line}");
    assert!(!line.chars().any(|c| c.is_control() || matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')), "{line:?}");
    let named = line.trim_start_matches("Not checked: the name ").trim_end_matches(" was not found.");
    assert_eq!(named.chars().count(), super::MOST_HOST_CHARS, "{named}");
    assert!(named.starts_with("evil "), "{named}");
}

#[test]
fn cancel_while_detect_asks_stops_the_run_and_its_late_answer_is_dropped() {
    let (mut onboarding, detect) = detecting(vec![
        found_at("imap.late.example.org", Some("smtp.late.example.org"), "carol@example.org"),
        found_at("imap.example.org", None, "carol@example.org"),
    ]);
    onboarding.form_mut().address = "carol@example.org".to_owned();
    onboarding.detect();
    assert!(onboarding.detecting());
    let stop = detect.stops.lock().unwrap()[0].clone();
    assert!(!stop.is_stopped());
    onboarding.cancel();
    assert!(stop.is_stopped(), "Cancel stops the run");
    assert!(!onboarding.detecting());
    // The stopped run's answer comes late: nothing of it is written anywhere.
    detect.answer();
    assert!(!onboarding.poll());
    onboarding.open();
    assert!(!onboarding.detecting(), "Detect and Check and add are back");
    assert_eq!((onboarding.form().host.as_str(), onboarding.form().smtp_host.as_str()), ("", ""));
    assert!(onboarding.found().is_none());
    // A second Detect works, with a stop of its own.
    detect_now(&mut onboarding, &detect, "carol@example.org");
    assert_eq!(onboarding.form().host, "imap.example.org");
    let stops = detect.stops.lock().unwrap();
    assert_eq!(stops.len(), 2);
    assert!(!stops[1].is_stopped());
}

#[test]
fn leaving_the_form_while_detect_asks_stops_the_run_and_its_late_answer_is_dropped() {
    let (mut onboarding, detect) = detecting(vec![found_at("imap.late.example.org", None, "carol@example.org")]);
    onboarding.form_mut().address = "carol@example.org".to_owned();
    onboarding.detect();
    let stop = detect.stops.lock().unwrap()[0].clone();
    onboarding.close();
    assert!(stop.is_stopped(), "leaving stops the run");
    detect.answer();
    assert!(!onboarding.poll());
    onboarding.open();
    assert!(!onboarding.detecting());
    assert_eq!((onboarding.form().address.as_str(), onboarding.form().host.as_str()), ("carol@example.org", ""));
}

#[test]
fn a_newer_detect_stops_the_run_before_it() {
    let (mut onboarding, detect) = detecting(vec![Proposal::default(), Proposal::default()]);
    onboarding.form_mut().address = "carol@example.org".to_owned();
    onboarding.detect();
    onboarding.detect();
    let stops = detect.stops.lock().unwrap();
    assert!(stops[0].is_stopped() && !stops[1].is_stopped());
}

/// A threaded Detect whose run asks one question per permit, and looks at its stop before each.
fn gated() -> (Onboarding, mpsc::Sender<()>, Arc<Mutex<Vec<String>>>) {
    let log = Arc::new(Mutex::new(Vec::new()));
    let (permit, gate) = mpsc::channel::<()>();
    let gate = Mutex::new(gate);
    let written = Arc::clone(&log);
    let run: super::Run = Arc::new(move |_address: &str, _ispdb: bool, stop: &super::RunStop| {
        for n in 1..=100 {
            if gate.lock().unwrap().recv().is_err() {
                written.lock().unwrap().push("closed".to_owned());
                break;
            }
            if stop.is_stopped() {
                written.lock().unwrap().push("stopped".to_owned());
                break;
            }
            written.lock().unwrap().push(format!("lookup {n}"));
        }
        found_at("imap.example.org", None, "carol@example.org")
    });
    let mut onboarding = Onboarding::new(Arc::new(NoKeychain), Box::new(super::Threaded::new(run, None)));
    onboarding.open();
    onboarding.form_mut().address = "carol@example.org".to_owned();
    (onboarding, permit, log)
}

fn wait_until(mut what: impl FnMut() -> bool) {
    for _ in 0..2000 {
        if what() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("never came");
}

#[test]
fn cancel_stops_a_threaded_run_before_its_next_lookup_and_a_second_detect_works() {
    let (mut onboarding, permit, log) = gated();
    onboarding.detect();
    permit.send(()).unwrap();
    permit.send(()).unwrap();
    wait_until(|| log.lock().unwrap().len() == 2);
    onboarding.cancel();
    // The run comes to its next lookup: it sees the stop and asks nothing more.
    permit.send(()).unwrap();
    wait_until(|| log.lock().unwrap().len() == 3);
    assert_eq!(*log.lock().unwrap(), ["lookup 1", "lookup 2", "stopped"]);
    // Its answer is dropped when it comes.
    for _ in 0..40 {
        assert!(!onboarding.poll());
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    onboarding.open();
    assert_eq!(onboarding.form().host, "");
    // A second Detect after the Cancel runs and fills the form.
    onboarding.form_mut().address = "carol@example.org".to_owned();
    onboarding.detect();
    drop(permit);
    wait_until(|| onboarding.poll());
    assert_eq!(onboarding.form().host, "imap.example.org");
    assert_eq!(log.lock().unwrap().last().map(String::as_str), Some("closed"));
}

#[test]
fn a_server_detect_left_out_for_the_owners_says_why_it_was_not_checked() {
    for (checked, cause) in [
        (Checked::NoAnswer, "it did not answer from here"),
        (Checked::Refused, "it refused the connection from here"),
        (Checked::NoName, "its name was not found"),
        (Checked::Clock, "its certificate does not fit this computer's clock, which may be what is off"),
    ] {
        let mut answer = found_at("imap.example.org", Some("smtp.example.org"), "carol@example.org");
        answer.imap[0].checked = checked;
        answer.submission[0].checked = checked;
        let (mut onboarding, detect) = detecting(vec![answer]);
        onboarding.form_mut().host = "mail.mine.example.org".to_owned();
        onboarding.form_mut().smtp_host = "smtp.mine.example.org".to_owned();
        detect_now(&mut onboarding, &detect, "carol@example.org");
        // The owner's servers stay, and nothing is said beside their rows: they are not Detect's.
        assert_eq!(onboarding.form().host, "mail.mine.example.org");
        assert_eq!(onboarding.unchecked(), [None, None]);
        let found = onboarding.found().unwrap();
        assert_eq!(found.kept_unchecked, [Some(checked), Some(checked)]);
        assert_eq!(
            super::kept_line(found.kept_imap.as_deref().unwrap(), found.kept_unchecked[0], false),
            format!("Detect found imap.example.org but did not check it: {cause}. The IMAP server you typed stays.")
        );
        assert_eq!(
            super::kept_line(found.kept_outgoing.as_deref().unwrap(), found.kept_unchecked[1], true),
            format!("Detect found smtp.example.org but did not check it: {cause}. The outgoing server you typed stays.")
        );
    }
    // One that answered says only that the owner's stays.
    let (mut onboarding, detect) = detecting(vec![found_at("imap.example.org", None, "carol@example.org")]);
    onboarding.form_mut().host = "mail.mine.example.org".to_owned();
    detect_now(&mut onboarding, &detect, "carol@example.org");
    let found = onboarding.found().unwrap();
    assert_eq!(found.kept_unchecked, [None, None]);
    assert_eq!(super::kept_line("imap.example.org", None, false), "Detect found imap.example.org; the IMAP server you typed stays.");
    // A hostile host is cleaned and cut as in the line under a row.
    let host = format!("\u{202e}{}.example.org\u{0}", "h".repeat(300));
    let said = super::kept_line(&super::shown(&host), Some(Checked::NoName), false);
    assert_eq!(said.matches('…').count(), 1, "{said}");
    assert!(!said.contains('\u{202e}') && !said.contains('\u{0}'), "{said:?}");
}

#[test]
fn an_account_s_name_in_the_remove_question_is_one_plain_capped_line() {
    let kept = Kept::default();
    let mut onboarding = onboarding(&kept);
    let long = "A".repeat(10_000);
    onboarding.form_mut().address = format!("Stu\r\ndio\u{0}\u{202e}\u{2066} {long}");
    let name = onboarding.removed_name();
    assert!(!name.chars().any(|c| c.is_control() || matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')), "{name:?}");
    assert!(name.starts_with("Stu"), "{name:?}");
    assert!(name.ends_with('…') && name.chars().count() <= 61, "cut with one ellipsis: {}", name.chars().count());
    onboarding.form_mut().address = "studio@example.org".to_owned();
    assert_eq!(onboarding.removed_name(), "studio@example.org", "a short name stays whole");
}

#[test]
fn the_save_summary_sees_a_calendar_a_server_or_a_port_and_not_a_change_undone() {
    let kept = Kept::default();
    let (mut mailbox, mut onboarding) = added(&kept);
    onboarding.edit(&mut mailbox, 1);
    assert!(!onboarding.save_summary().changed, "nothing typed");
    onboarding.form_mut().caldav = "https://dav.example.org/".to_owned();
    let summary = onboarding.save_summary();
    assert!(summary.changed, "the CalDAV address alone is something to save");
    assert!(summary.checks);
    onboarding.form_mut().caldav.clear();
    onboarding.form_mut().host = "moved.example.org".to_owned();
    assert!(onboarding.save_summary().changed, "a server");
    onboarding.form_mut().host = "imap.example.org".to_owned();
    onboarding.form_mut().port = "143".to_owned();
    assert!(onboarding.save_summary().changed, "a port");
    onboarding.form_mut().port = "993".to_owned();
    onboarding.form_mut().label = "Allotment".to_owned();
    assert!(onboarding.save_summary().changed, "a name");
    let held = listed(&mut mailbox)["label"].as_str().unwrap_or_default().to_owned();
    onboarding.form_mut().label = format!("  {held} ");
    assert!(!onboarding.save_summary().changed, "a name set back, with spaces round it");
    onboarding.form_mut().password = Secret::new("new-Lantern-9");
    assert!(onboarding.save_summary().changed, "a new password");
    onboarding.form_mut().password.clear();
    let summary = onboarding.save_summary();
    assert!(!summary.changed, "a password typed then cleared");
    assert_eq!(summary.note, None);
}

/// A store over [`Memory`] that refuses every write while `refuse` is on.
#[derive(Debug)]
struct Refusing(Memory, Arc<AtomicBool>);

#[rustfmt::skip]
impl Storage for Refusing {
    fn config(&self) -> StoreConfig { self.0.config() }
    fn apply(&mut self, batch: &inboxcraft_store::Batch) -> Result<(), inboxcraft_store::StoreError> {
        if self.1.load(Ordering::SeqCst) { return Err(inboxcraft_store::StoreError::UnknownAccount(AccountId::new(1))); }
        self.0.apply(batch)
    }
    fn accounts(&self) -> Result<Vec<Account>, inboxcraft_store::StoreError> { self.0.accounts() }
    fn folders(&self, a: AccountId) -> Result<Vec<inboxcraft_model::Folder>, inboxcraft_store::StoreError> { self.0.folders(a) }
    fn folder_state(&self, f: inboxcraft_model::FolderId) -> Result<Option<inboxcraft_model::FolderState>, inboxcraft_store::StoreError> { self.0.folder_state(f) }
    fn messages(&self, f: inboxcraft_model::FolderId) -> Result<Vec<inboxcraft_store::Entry>, inboxcraft_store::StoreError> { self.0.messages(f) }
    fn message(&self, m: inboxcraft_model::MessageId) -> Result<Option<inboxcraft_store::Entry>, inboxcraft_store::StoreError> { self.0.message(m) }
    fn linked(&self, l: &inboxcraft_model::MessageRef, k: &std::collections::BTreeSet<inboxcraft_model::MessageId>) -> Result<Vec<inboxcraft_store::Entry>, inboxcraft_store::StoreError> { self.0.linked(l, k) }
    fn body(&self, m: inboxcraft_model::MessageId) -> Result<Option<inboxcraft_store::Body>, inboxcraft_store::StoreError> { self.0.body(m) }
    fn search(&self, s: &inboxcraft_store::Search) -> Result<Vec<inboxcraft_model::MessageId>, inboxcraft_store::StoreError> { self.0.search(s) }
    fn queued_jobs(&self) -> Result<Vec<Job>, inboxcraft_store::StoreError> { self.0.queued_jobs() }
    fn drafts(&self, a: AccountId) -> Result<Vec<inboxcraft_model::Draft>, inboxcraft_store::StoreError> { self.0.drafts(a) }
    fn newest_drafts(&self, a: AccountId, n: usize) -> Result<(u64, Vec<inboxcraft_model::Draft>), inboxcraft_store::StoreError> { self.0.newest_drafts(a, n) }
    fn draft(&self, d: inboxcraft_model::DraftId) -> Result<Option<inboxcraft_model::Draft>, inboxcraft_store::StoreError> { self.0.draft(d) }
    fn outbox(&self) -> Result<Vec<inboxcraft_model::Outgoing>, inboxcraft_store::StoreError> { self.0.outbox() }
    fn schedule(&self) -> Result<Vec<inboxcraft_model::Scheduled>, inboxcraft_store::StoreError> { self.0.schedule() }
    fn sender(&self, e: &inboxcraft_model::EmailAddress) -> Result<Option<inboxcraft_model::Sender>, inboxcraft_store::StoreError> { self.0.sender(e) }
    fn senders(&self) -> Result<Vec<inboxcraft_model::Sender>, inboxcraft_store::StoreError> { self.0.senders() }
    fn message_state(&self, m: inboxcraft_model::MessageId) -> Result<inboxcraft_model::MessageState, inboxcraft_store::StoreError> { self.0.message_state(m) }
    fn message_states(&self) -> Result<Vec<(inboxcraft_model::MessageId, inboxcraft_model::MessageState)>, inboxcraft_store::StoreError> { self.0.message_states() }
    fn calendars(&self, a: AccountId) -> Result<Vec<inboxcraft_model::Calendar>, inboxcraft_store::StoreError> { self.0.calendars(a) }
    fn sync_token(&self, c: inboxcraft_model::CalendarId) -> Result<Option<String>, inboxcraft_store::StoreError> { self.0.sync_token(c) }
    fn calendar_events(&self, c: inboxcraft_model::CalendarId) -> Result<Vec<inboxcraft_model::CalendarEvent>, inboxcraft_store::StoreError> { self.0.calendar_events(c) }
    fn calendar_event(&self, e: inboxcraft_model::CalendarEventId) -> Result<Option<inboxcraft_model::CalendarEvent>, inboxcraft_store::StoreError> { self.0.calendar_event(e) }
    fn alerts(&self, e: inboxcraft_model::CalendarEventId) -> Result<Vec<inboxcraft_model::Alert>, inboxcraft_store::StoreError> { self.0.alerts(e) }
    fn alert(&self, a: inboxcraft_model::AlertId) -> Result<Option<inboxcraft_model::Alert>, inboxcraft_store::StoreError> { self.0.alert(a) }
    fn stale_bodies(&self, t: Timestamp) -> Result<Vec<inboxcraft_model::MessageId>, inboxcraft_store::StoreError> { self.0.stale_bodies(t) }
    fn missing_bodies(&self, f: inboxcraft_model::FolderId, t: Timestamp) -> Result<Vec<inboxcraft_model::MessageId>, inboxcraft_store::StoreError> { self.0.missing_bodies(f, t) }
}

#[test]
fn a_save_the_store_refuses_keeps_the_new_password_typed_and_can_be_tried_again() {
    let kept = Kept::default();
    let refuse = Arc::new(AtomicBool::new(false));
    let mut mailbox = mailbox_over(Box::new(Refusing(Memory::new(StoreConfig::default()), Arc::clone(&refuse))));
    let mut onboarding = onboarding(&kept);
    filled(onboarding.form_mut());
    onboarding.add(&mut mailbox);
    assert_eq!(onboarding.error(), None);
    mailbox.pump(&mut Scripted::passing());
    onboarding.close();
    onboarding.edit(&mut mailbox, 1);
    onboarding.form_mut().password = Secret::new("new-Lantern-9");
    refuse.store(true, Ordering::SeqCst);
    onboarding.save(&mut mailbox);
    assert_eq!(onboarding.stage(), Stage::Edit(1));
    let error = onboarding.error().unwrap();
    assert!(error.contains("were not saved"), "{error}");
    assert!(!error.contains("new-Lantern-9"), "{error}");
    assert_eq!(onboarding.form().password.expose(), "new-Lantern-9", "kept in the form to try again");
    assert!(onboarding.save_summary().changed, "Save stays on");
    refuse.store(false, Ordering::SeqCst);
    onboarding.save(&mut mailbox);
    assert_eq!(onboarding.error(), None);
    assert_eq!(onboarding.stage(), Stage::Checking(1));
    assert!(onboarding.form().password.is_empty(), "forgotten in the window once saved");
    assert_eq!(kept.passwords.lock().unwrap().get(&1).map(String::as_str), Some("new-Lantern-9"));
}
