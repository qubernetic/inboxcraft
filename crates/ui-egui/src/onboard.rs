// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Adding an account: the form, Detect's proposal, and the account check's steps as the worker reports them.
//!
//! Manual setup is the default: the owner types the address, the password and the IMAP server. Detect only fills in
//! a proposal, which the owner sees in the form before anything gets the password. "Add and check" runs
//! `account.add`, puts the password into the OS credential store ([`Keychain`]) under the new account's id and
//! nowhere else, and forgets it here; the account's worker reads it from there for the check `account.add` started.
//! When the credential store refuses it, the account is removed again: an account without its password is not kept.
//!
//! The steps show as they come, each with the host, port and (for the certificate) issuer it saw. After a failure the
//! owner can check again, or change the settings, which removes the account and its password and opens the form
//! again with everything but the password kept.
//!
//! The password lives in a [`Secret`]: no `Display`, a `Debug` that hides it, and its bytes overwritten when it is
//! cleared or dropped. It never goes into a command's parameters, an answer, an error or the store.

use std::fmt;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;

use serde_json::{Value, json};

use crate::UiError;
use crate::fetch::Wake;
use crate::mailbox::{Mailbox, shown};
use crate::tr;

/// Where a new account's password goes: the OS credential store (Secret Service, Keychain, Credential Manager) in the
/// app. The front end only ever writes or removes a password; it never reads one back.
pub trait Keychain: Send + Sync {
    /// Keeps `password` for the account `account`, replacing one kept before.
    ///
    /// # Errors
    ///
    /// Why the credential store refused, in words for the owner; never the password.
    fn store(&self, account: u64, password: &str) -> Result<(), String>;

    /// Removes the password of `account`; nothing when there is none.
    ///
    /// # Errors
    ///
    /// Why the credential store refused, in words for the owner.
    fn forget(&self, account: u64) -> Result<(), String>;
}

/// The keychain of a window that keeps no passwords: storing one is refused, so no account is added.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoKeychain;

impl Keychain for NoKeychain {
    fn store(&self, _account: u64, _password: &str) -> Result<(), String> {
        Err(tr!("This window keeps no passwords.").to_owned())
    }

    fn forget(&self, _account: u64) -> Result<(), String> {
        Ok(())
    }
}

/// How the encryption of a server is set in the form. There is no unencrypted choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Security {
    /// TLS from the first byte.
    Tls,
    /// STARTTLS before anything else is sent.
    StartTls,
}

impl Security {
    /// The usual IMAP port.
    #[must_use]
    pub const fn imap_port(self) -> u16 {
        match self {
            Self::Tls => 993,
            Self::StartTls => 143,
        }
    }

    /// The name `account.add` reads.
    const fn param(self) -> &'static str {
        match self {
            Self::Tls => "tls",
            Self::StartTls => "start_tls",
        }
    }

    /// The name the form shows.
    #[must_use]
    pub fn text(self) -> &'static str {
        match self {
            Self::Tls => tr!("TLS"),
            Self::StartTls => tr!("STARTTLS"),
        }
    }
}

/// Where Detect found a server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// The provider's autoconfig file, over HTTPS at the address's domain.
    Autoconfig,
    /// An autoconfig file reached through an unencrypted redirect: anyone on the network could have sent it.
    AutoconfigRedirect,
    /// Mozilla's ISPDB.
    Ispdb,
    /// An SRV record.
    Srv,
    /// A common host name: a guess.
    Guess,
}

impl Source {
    /// What the form says about a server from this source.
    #[must_use]
    pub fn text(self) -> &'static str {
        match self {
            Self::Autoconfig => tr!("Found in your provider's settings file."),
            Self::AutoconfigRedirect => {
                tr!("Found through an unencrypted redirect, which anyone on the network could have sent: make sure this is your provider's server.")
            }
            Self::Ispdb => tr!("Found in Mozilla's list of providers."),
            Self::Srv => tr!("Found in your domain's DNS records."),
            Self::Guess => tr!("A common server name: a guess, not something your provider said. Make sure it is right."),
        }
    }

    /// Whether the owner should look twice before using it.
    #[must_use]
    pub const fn doubtful(self) -> bool {
        matches!(self, Self::AutoconfigRedirect | Self::Guess)
    }
}

/// An IMAP server Detect proposes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    /// The host.
    pub host: String,
    /// The port.
    pub port: u16,
    /// The encryption.
    pub security: Security,
    /// The user name to sign in with.
    pub username: String,
    /// Where it came from.
    pub source: Source,
}

/// What Detect found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Proposal {
    /// IMAP servers, the best first.
    pub imap: Vec<Suggestion>,
    /// Every DNS query and URL asked, in order: what left the computer.
    pub asked: Vec<String>,
    /// The lookups that failed, each with its cause.
    pub failed: Vec<String>,
}

/// Runs Detect for the form.
pub trait Detect: Send {
    /// Starts Detect for `address`, asking Mozilla's ISPDB too when `ispdb` is on; the answer comes from
    /// [`Detect::finished`].
    fn start(&mut self, address: &str, ispdb: bool);

    /// The proposal, once Detect ended.
    fn finished(&mut self) -> Option<Proposal>;
}

/// The Detect of a window that asks nobody: it ends at once with nothing found.
#[derive(Debug, Default)]
pub struct NoDetect {
    done: Option<Proposal>,
}

impl Detect for NoDetect {
    fn start(&mut self, _address: &str, _ispdb: bool) {
        self.done = Some(Proposal { failed: vec![tr!("Detect does not run in this window.").to_owned()], ..Proposal::default() });
    }

    fn finished(&mut self) -> Option<Proposal> {
        self.done.take()
    }
}

/// The function a [`Threaded`] Detect runs: the address and whether the ISPDB is on, to the proposal.
pub type Run = Arc<dyn Fn(&str, bool) -> Proposal + Send + Sync>;

/// Runs Detect on a thread of its own, so the window does not wait for the name servers and the web; `wake` draws
/// the window again when it ended. The desktop app gives it `net`'s Detect.
pub struct Threaded {
    run: Run,
    wake: Option<Wake>,
    sender: Sender<Proposal>,
    receiver: Receiver<Proposal>,
}

impl fmt::Debug for Threaded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Threaded").finish_non_exhaustive()
    }
}

impl Threaded {
    /// A Detect that runs `run`, and calls `wake` after each answer.
    #[must_use]
    pub fn new(run: Run, wake: Option<Wake>) -> Self {
        let (sender, receiver) = channel();
        Self { run, wake, sender, receiver }
    }
}

impl Detect for Threaded {
    fn start(&mut self, address: &str, ispdb: bool) {
        let (run, wake, sender, address) = (Arc::clone(&self.run), self.wake.clone(), self.sender.clone(), address.to_owned());
        let spawned = thread::Builder::new().name("inboxcraft-detect".to_owned()).spawn(move || {
            // A closed window no longer listens; nothing to tell it then.
            let _ = sender.send(run(&address, ispdb));
            if let Some(wake) = wake {
                wake();
            }
        });
        if let Err(error) = spawned {
            let _ = self.sender.send(Proposal { failed: vec![error.to_string()], ..Proposal::default() });
        }
    }

    fn finished(&mut self) -> Option<Proposal> {
        self.receiver.try_recv().ok()
    }
}

/// A Detect that asks nobody and answers `proposal` at the next poll after each start; it writes down what it was
/// asked. For the `ui_shot` pictures and the tests.
#[derive(Debug, Default)]
pub struct Proposed {
    /// What it answers.
    pub proposal: Proposal,
    /// Each address it was asked for, and whether the ISPDB was on.
    pub asked: Arc<std::sync::Mutex<Vec<(String, bool)>>>,
    pending: bool,
}

impl Proposed {
    /// A Detect that answers `proposal`.
    #[must_use]
    pub fn new(proposal: Proposal) -> Self {
        Self { proposal, ..Self::default() }
    }
}

impl Detect for Proposed {
    fn start(&mut self, address: &str, ispdb: bool) {
        if let Ok(mut asked) = self.asked.lock() {
            asked.push((address.to_owned(), ispdb));
        }
        self.pending = true;
    }

    fn finished(&mut self) -> Option<Proposal> {
        std::mem::take(&mut self.pending).then(|| self.proposal.clone())
    }
}

/// A password in memory: it has no `Display`, its `Debug` hides it, and its bytes are overwritten when it is cleared
/// or dropped.
#[derive(Default)]
pub struct Secret(String);

impl Secret {
    /// The secret `text`.
    #[must_use]
    pub fn new(text: &str) -> Self {
        Self(text.to_owned())
    }

    /// Whether nothing was typed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Overwrites the bytes and leaves it empty.
    pub fn clear(&mut self) {
        let mut bytes = std::mem::take(&mut self.0).into_bytes();
        bytes.fill(0);
        std::hint::black_box(&bytes);
    }

    /// The text, for the password field and the credential store.
    pub(crate) fn expose(&self) -> &str {
        &self.0
    }

    /// The text, for the password field to edit.
    pub(crate) fn expose_mut(&mut self) -> &mut String {
        &mut self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(hidden)")
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        self.clear();
    }
}

/// What the owner typed.
#[derive(Debug, Default)]
pub struct Form {
    /// The account's address.
    pub address: String,
    /// The owner's name for the account; the address when empty.
    pub label: String,
    /// The owner's name on sent mail.
    pub display_name: String,
    /// The login name; the address when empty.
    pub username: String,
    /// The password.
    pub password: Secret,
    /// The IMAP server.
    pub host: String,
    /// The IMAP port; the usual one for the encryption when empty.
    pub port: String,
    /// The encryption.
    pub security: Option<Security>,
    /// Detect asks Mozilla's ISPDB too, which tells Mozilla the domain.
    pub ispdb: bool,
}

impl Form {
    /// The parameters of `account.add`: everything but the password.
    ///
    /// # Errors
    ///
    /// What is missing or wrong, in words for the owner.
    pub fn params(&self) -> Result<Value, String> {
        let address = self.address.trim();
        if address.is_empty() {
            return Err(tr!("Enter the address.").to_owned());
        }
        if self.password.is_empty() {
            return Err(tr!("Enter the password.").to_owned());
        }
        let host = self.host.trim();
        if host.is_empty() {
            return Err(tr!("Enter the IMAP server, or use Detect.").to_owned());
        }
        let security = self.security.unwrap_or(Security::Tls);
        let port = match self.port.trim() {
            "" => security.imap_port(),
            text => text.parse::<u16>().ok().filter(|port| *port > 0).ok_or_else(|| tr!("The port is a number from 1 to 65535.").to_owned())?,
        };
        let optional = |text: &str| Some(text.trim().to_owned()).filter(|text| !text.is_empty());
        Ok(json!({
            "address": address,
            "label": optional(&self.label),
            "display_name": optional(&self.display_name),
            "username": optional(&self.username),
            "imap": { "host": host, "port": port, "security": security.param() },
        }))
    }
}

/// How a step of the check stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepState {
    /// Not reported yet.
    Pending,
    /// It passed.
    Passed,
    /// It failed; [`StepRow::message`] says why.
    Failed,
    /// Not run, because an earlier step failed.
    Skipped,
    /// Not run: no password went out ([`StepRow::message`] says why).
    Held,
    /// Not built yet.
    NotBuilt,
}

/// One step of the check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepRow {
    /// The step's id (`imap_login`).
    pub id: String,
    /// How it stands.
    pub state: StepState,
    /// The host and port it tried.
    pub server: Option<String>,
    /// The certificate's issuer.
    pub issuer: Option<String>,
    /// Why it failed or did not run, in English from the engine.
    pub message: Option<String>,
}

impl StepRow {
    /// The step's name.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self.id.as_str() {
            "dns" => tr!("Name lookup (DNS)"),
            "tcp" => tr!("Connection (TCP)"),
            "tls" => tr!("Encryption (TLS)"),
            "certificate" => tr!("Certificate"),
            "imap_login" => tr!("IMAP sign-in"),
            "smtp_login" => tr!("SMTP sign-in"),
            "caldav_discovery" => tr!("CalDAV calendars"),
            _ => tr!("Another step"),
        }
    }
}

fn step_row(value: &Value) -> StepRow {
    let text = |key: &str| value.get(key).and_then(Value::as_str).map(shown);
    let state = match value.get("status").and_then(Value::as_str) {
        Some("passed") => StepState::Passed,
        Some("failed") => StepState::Failed,
        Some("skipped") => StepState::Skipped,
        Some("needs_review" | "needs_owner") => StepState::Held,
        Some("not_built") => StepState::NotBuilt,
        _ => StepState::Pending,
    };
    let server = text("host").map(|host| match value.get("port").and_then(Value::as_u64) {
        Some(port) => format!("{host}:{port}"),
        None => host,
    });
    StepRow { id: text("step").unwrap_or_default(), state, server, issuer: text("issuer"), message: text("message") }
}

/// Where the owner is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Onboarding is not open.
    Closed,
    /// The form.
    Form,
    /// The account was added and is being checked, or was checked.
    Checking(u64),
}

/// How the check of the added account ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Still running.
    Running,
    /// The sign-in passed: the account syncs.
    Passed,
    /// A step failed or held the sign-in back.
    Failed,
}

/// What Detect last found, as the form shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// Where the server in the form came from; `None` when nothing was found.
    pub source: Option<Source>,
    /// What Detect asked: what left the computer.
    pub asked: Vec<String>,
    /// The lookups that failed.
    pub failed: Vec<String>,
}

/// Onboarding's state.
pub struct Onboarding {
    stage: Stage,
    form: Form,
    detecting: bool,
    found: Option<Found>,
    steps: Vec<StepRow>,
    running: bool,
    error: Option<String>,
    keychain: Arc<dyn Keychain>,
    detect: Box<dyn Detect>,
}

impl fmt::Debug for Onboarding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Onboarding").field("stage", &self.stage).field("form", &self.form).field("steps", &self.steps).finish_non_exhaustive()
    }
}

impl Default for Onboarding {
    fn default() -> Self {
        Self::new(Arc::new(NoKeychain), Box::new(NoDetect::default()))
    }
}

impl Onboarding {
    /// Closed onboarding that keeps passwords in `keychain` and runs Detect through `detect`.
    #[must_use]
    pub fn new(keychain: Arc<dyn Keychain>, detect: Box<dyn Detect>) -> Self {
        Self {
            stage: Stage::Closed,
            form: Form::default(),
            detecting: false,
            found: None,
            steps: Vec::new(),
            running: false,
            error: None,
            keychain,
            detect,
        }
    }

    /// Opens an empty form.
    pub fn open(&mut self) {
        self.form = Form::default();
        self.found = None;
        self.error = None;
        self.steps.clear();
        self.stage = Stage::Form;
    }

    /// Closes onboarding; a password typed and not used is forgotten. An added account stays.
    pub fn close(&mut self) {
        self.form.password.clear();
        self.detecting = false;
        self.stage = Stage::Closed;
    }

    /// Where the owner is.
    #[must_use]
    pub fn stage(&self) -> Stage {
        self.stage
    }

    /// The form.
    #[must_use]
    pub fn form(&self) -> &Form {
        &self.form
    }

    /// The form, to type into.
    pub fn form_mut(&mut self) -> &mut Form {
        &mut self.form
    }

    /// Whether Detect runs.
    #[must_use]
    pub fn detecting(&self) -> bool {
        self.detecting
    }

    /// What Detect last found.
    #[must_use]
    pub fn found(&self) -> Option<&Found> {
        self.found.as_ref()
    }

    /// The steps of the check.
    #[must_use]
    pub fn steps(&self) -> &[StepRow] {
        &self.steps
    }

    /// What went wrong last, in words for the owner.
    #[must_use]
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// How the check stands.
    #[must_use]
    pub fn outcome(&self) -> Outcome {
        if self.steps.iter().any(|step| step.id == "imap_login" && step.state == StepState::Passed) {
            Outcome::Passed
        } else if self.running {
            Outcome::Running
        } else {
            Outcome::Failed
        }
    }

    /// Starts Detect for the address in the form.
    pub fn detect(&mut self) {
        let address = self.form.address.trim().to_owned();
        if address.is_empty() {
            self.error = Some(tr!("Enter the address, then Detect.").to_owned());
            return;
        }
        self.error = None;
        self.detecting = true;
        self.detect.start(&address, self.form.ispdb);
    }

    /// Takes Detect's answer when it came: its first IMAP server fills the form. Whether anything changed.
    pub fn poll(&mut self) -> bool {
        let Some(proposal) = self.detect.finished() else { return false };
        self.detecting = false;
        let source = proposal.imap.first().map(|best| {
            self.form.host = shown(&best.host);
            self.form.port = best.port.to_string();
            self.form.security = Some(best.security);
            self.form.username = shown(&best.username);
            best.source
        });
        let clean = |lines: Vec<String>| lines.iter().map(|line| shown(line)).collect();
        self.found = Some(Found { source, asked: clean(proposal.asked), failed: clean(proposal.failed) });
        true
    }

    /// Adds the account in the form through `mailbox` and keeps its password in the credential store; the check
    /// `account.add` started shows next. The password is forgotten here either way it ends.
    pub fn add(&mut self, mailbox: &mut Mailbox) {
        let params = match self.form.params() {
            Ok(params) => params,
            Err(error) => {
                self.error = Some(error);
                return;
            }
        };
        let id = match mailbox.run("account.add", &params) {
            Ok(answer) => answer.get("account").and_then(|account| account.get("id")).and_then(Value::as_u64),
            Err(error) => {
                self.error = Some(error.to_string());
                return;
            }
        };
        let Some(id) = id else {
            self.error = Some(UiError::Answer { command: "account.add", detail: "no account id".to_owned() }.to_string());
            return;
        };
        let stored = self.keychain.store(id, self.form.password.expose());
        self.form.password.clear();
        if let Err(refused) = stored {
            // An account without its password is not kept; removing it also drops the check it queued.
            let _ = mailbox.run("account.remove", &json!({ "account": id }));
            mailbox.refresh();
            self.error = Some(tr!(
                "The system's credential store did not take the password, so the account was not added: {reason}",
                reason = shown(&refused)
            ));
            return;
        }
        self.error = None;
        self.stage = Stage::Checking(id);
        mailbox.refresh();
        self.load(mailbox);
    }

    /// Reads the check's steps again from `account.list`.
    pub fn load(&mut self, mailbox: &mut Mailbox) {
        let Stage::Checking(id) = self.stage else { return };
        let answer = match mailbox.run("account.list", &json!({})) {
            Ok(answer) => answer,
            Err(error) => {
                self.error = Some(error.to_string());
                return;
            }
        };
        let accounts = answer.get("accounts").and_then(Value::as_array).cloned().unwrap_or_default();
        let Some(check) =
            accounts.iter().find(|account| account.get("id").and_then(Value::as_u64) == Some(id)).and_then(|account| account.get("check"))
        else {
            self.running = false;
            self.steps.clear();
            return;
        };
        self.running = check.get("running").and_then(Value::as_bool).unwrap_or(false);
        self.steps = check.get("steps").and_then(Value::as_array).map(|steps| steps.iter().map(step_row).collect()).unwrap_or_default();
    }

    /// Checks the added account again, as the owner.
    pub fn check_again(&mut self, mailbox: &mut Mailbox) {
        let Stage::Checking(id) = self.stage else { return };
        if let Err(error) = mailbox.run("account.check", &json!({ "account": id })) {
            self.error = Some(error.to_string());
        }
        self.load(mailbox);
    }

    /// Removes the added account and its password, and opens the form again with its settings; the password has to
    /// be typed again.
    pub fn change(&mut self, mailbox: &mut Mailbox) {
        let Stage::Checking(id) = self.stage else { return };
        let removed = mailbox.run("account.remove", &json!({ "account": id }));
        let forgotten = self.keychain.forget(id);
        mailbox.refresh();
        self.steps.clear();
        self.running = false;
        self.stage = Stage::Form;
        self.error = match (removed, forgotten) {
            (Err(error), _) => Some(error.to_string()),
            (Ok(_), Err(refused)) => {
                Some(tr!("The password could not be removed from the system's credential store: {reason}", reason = shown(&refused)))
            }
            (Ok(_), Ok(())) => None,
        };
    }
}

#[cfg(test)]
#[path = "onboard_tests.rs"]
pub(crate) mod tests;
