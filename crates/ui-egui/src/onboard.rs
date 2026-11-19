// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Adding an account: the form, Detect's proposal, and the account check's steps as the worker reports them.
//!
//! Manual setup is the default: the owner types the address, the password and the IMAP server. Detect only fills in
//! a proposal, which the owner sees in the form before anything gets the password. "Add and check" runs
//! `account.add`, puts the password into the OS credential store ([`Keychain`]) under the new account's id and
//! nowhere else, and forgets it here; the account's worker reads it from there for the check `account.add` started.
//! When the credential store refuses it, the account is removed again: an account without its password is not kept.
//! The password stays in the credential store as long as the account stays in the store, and leaves with it. A window
//! over a store that is not kept removes the passwords stored in it when it closes ([`Onboarding::forget_passwords`]),
//! as they would outlive their accounts; the desktop app keeps its store, and so the passwords.
//!
//! The steps show as they come, each with the host, port and (for the certificate) issuer it saw. After a failure the
//! owner can check again, or change the settings, which removes the account and its password and opens the form
//! again with everything but the password kept.
//!
//! An account that is already there has the same form, from the Accounts page, without Detect and with a "New password"
//! field that starts empty: empty keeps the stored password, and a typed one replaces it the same way the add form
//! stores one, for the worker's next sign-in. The old password is never read: the [`Keychain`] has no way to. A changed server
//! or user name, or a new password, runs the owner's check, and the form says first where the sign-in goes.
//!
//! What the owner typed in the add form, or changed in an account's form and did not save, is kept on every way out
//! and back but Cancel: Back, Esc, another page, the window's welcome, or the other form keep it
//! ([`Onboarding::close`]), and the form opens with it again; only the password is dropped on the way out. Detect's answer belongs to the add form and the address that asked: one that comes after the page
//! was left, or for an address no longer in the form, is thrown away. Detect fills a server row and the user name
//! where the owner typed nothing, or kept what Detect put there before; a host the owner typed keeps the row theirs,
//! and the note says Detect's server was left out. A Detect that finds nothing takes out what an earlier one put there.
//! While the server rows still hold what Detect put there and the address has changed since it answered, a line says
//! they come from an earlier Detect (and the user name, while it is Detect's too), until Detect runs again or a server
//! field is edited by hand. A server Detect proposes without having reached it ([`Checked`]) says why beside its row,
//! until that row is edited or Detect runs again. Leaving the add form any way, Cancel too, while Detect asks sets the
//! run's [`RunStop`]: it starts no further lookup or probe, and its answer is thrown away when it comes.
//! A field whose text is wrong says so under itself ([`Slot`]): a port or a CalDAV address as it is
//! typed, a missing address, password or IMAP server when the form is sent.
//!
//! The password lives in a [`Secret`]: no `Display`, a `Debug` that hides it, and its bytes overwritten when it is
//! cleared or dropped. It never goes into a command's parameters, an answer, an error or the store.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;

use inboxcraft_model::Account;
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

    /// The usual submission (SMTP) port.
    #[must_use]
    pub const fn submission_port(self) -> u16 {
        match self {
            Self::Tls => 465,
            Self::StartTls => 587,
        }
    }

    /// The name `account.add` reads.
    const fn param(self) -> &'static str {
        match self {
            Self::Tls => "tls",
            Self::StartTls => "start_tls",
        }
    }

    /// The security `account.list` names.
    fn from_param(text: &str) -> Option<Self> {
        match text {
            "tls" => Some(Self::Tls),
            "start_tls" | "starttls" => Some(Self::StartTls),
            _ => None,
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
    /// The mail provider the domain's MX records point to: its settings file or its usual host names. A guess.
    Provider,
    /// A common host name of the domain: a guess.
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
            Self::Provider => tr!("A guess from the mail provider your domain's MX records point to: check the servers before you add."),
            Self::Guess => tr!("A common server name: a guess, not something your provider said. Make sure it is right."),
        }
    }

    /// Whether the owner should look twice before using it.
    #[must_use]
    pub const fn doubtful(self) -> bool {
        matches!(self, Self::AutoconfigRedirect | Self::Provider | Self::Guess)
    }
}

/// Whether Detect reached a server it proposes, and why not when it did not. Fixed ids: the form words them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Checked {
    /// It answered Detect's probe as a mail server of its kind.
    #[default]
    Answered,
    /// The domain names it, and it did not answer from here.
    NoAnswer,
    /// It refused the connection from here.
    Refused,
    /// Its name was not found.
    NoName,
    /// Its certificate does not fit this computer's clock.
    Clock,
}

/// The most characters of a host name a line about it shows; a longer one is cut with one `…`.
pub const MOST_HOST_CHARS: usize = 48;

/// `host` made fit for a line of the form: cleaned as [`shown`] does, and cut to [`MOST_HOST_CHARS`] with one `…`.
fn host_in_line(host: &str) -> String {
    let host = shown(host);
    if host.chars().count() <= MOST_HOST_CHARS {
        return host;
    }
    host.chars().take(MOST_HOST_CHARS.saturating_sub(1)).chain(std::iter::once('…')).collect()
}

/// What the form says beside a server Detect proposes without having reached it: why, with the host named. `None` for
/// a server that answered.
#[must_use]
pub fn unchecked_line(checked: Checked, host: &str) -> Option<String> {
    let host = host_in_line(host);
    Some(match checked {
        Checked::Answered => return None,
        Checked::NoAnswer => tr!("Not checked: {host} did not answer from here.", host = host),
        Checked::Refused => tr!("Not checked: {host} refused the connection from here.", host = host),
        Checked::NoName => tr!("Not checked: the name {host} was not found.", host = host),
        Checked::Clock => {
            tr!("Not checked: the certificate of {host} does not fit this computer's clock, which may be what is off.", host = host)
        }
    })
}

/// What the form says of a server Detect found at `host` and left out because the owner typed another (the outgoing one
/// when `outgoing`): that the owner's stays, and, when Detect did not reach it (`unchecked`), that it was not checked and
/// why, the host then cut as in [`unchecked_line`].
#[must_use]
pub fn kept_line(host: &str, unchecked: Option<Checked>, outgoing: bool) -> String {
    let cause = match unchecked {
        None | Some(Checked::Answered) => None,
        Some(Checked::NoAnswer) => Some(tr!("it did not answer from here")),
        Some(Checked::Refused) => Some(tr!("it refused the connection from here")),
        Some(Checked::NoName) => Some(tr!("its name was not found")),
        Some(Checked::Clock) => Some(tr!("its certificate does not fit this computer's clock, which may be what is off")),
    };
    match (cause, outgoing) {
        (None, false) => tr!("Detect found {host}; the IMAP server you typed stays.", host = host),
        (None, true) => tr!("Detect found {host}; the outgoing server you typed stays.", host = host),
        (Some(cause), false) => {
            tr!("Detect found {host} but did not check it: {cause}. The IMAP server you typed stays.", host = host_in_line(host), cause = cause)
        }
        (Some(cause), true) => {
            tr!("Detect found {host} but did not check it: {cause}. The outgoing server you typed stays.", host = host_in_line(host), cause = cause)
        }
    }
}

/// The stop of one Detect run: once set, the run starts no further lookup or probe and ends as soon as it can.
#[derive(Debug, Clone, Default)]
pub struct RunStop(Arc<AtomicBool>);

impl RunStop {
    /// Stops the run.
    pub fn stop(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// Whether the run was stopped.
    #[must_use]
    pub fn is_stopped(&self) -> bool {
        self.0.load(Ordering::SeqCst)
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
    /// Whether Detect reached it.
    pub checked: Checked,
}

/// What Detect found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Proposal {
    /// IMAP servers, the best first.
    pub imap: Vec<Suggestion>,
    /// Submission (SMTP) servers, the best first.
    pub submission: Vec<Suggestion>,
    /// Every DNS query and URL asked, in order: what left the computer.
    pub asked: Vec<String>,
    /// The lookups that failed, each with its cause.
    pub failed: Vec<String>,
}

/// Runs Detect for the form.
pub trait Detect: Send {
    /// Starts Detect's run `run` for `address`, asking Mozilla's ISPDB too when `ispdb` is on; the answer comes from
    /// [`Detect::finished`], with the run it answers. Once `stop` is set, the run starts no further lookup or probe.
    fn start(&mut self, run: u64, address: &str, ispdb: bool, stop: RunStop);

    /// A run that ended: its number and its proposal. Each run answers once.
    fn finished(&mut self) -> Option<(u64, Proposal)>;
}

/// The Detect of a window that asks nobody: it ends at once with nothing found.
#[derive(Debug, Default)]
pub struct NoDetect {
    done: Option<u64>,
}

impl Detect for NoDetect {
    fn start(&mut self, run: u64, _address: &str, _ispdb: bool, _stop: RunStop) {
        self.done = Some(run);
    }

    fn finished(&mut self) -> Option<(u64, Proposal)> {
        let run = self.done.take()?;
        Some((run, Proposal { failed: vec![tr!("Detect does not run in this window.").to_owned()], ..Proposal::default() }))
    }
}

/// The function a [`Threaded`] Detect runs: the address, whether the ISPDB is on and the run's stop, to the proposal.
pub type Run = Arc<dyn Fn(&str, bool, &RunStop) -> Proposal + Send + Sync>;

/// Runs Detect on a thread of its own, so the window does not wait for the name servers and the web; `wake` draws
/// the window again when it ended. The desktop app gives it `net`'s Detect.
pub struct Threaded {
    run: Run,
    wake: Option<Wake>,
    sender: Sender<(u64, Proposal)>,
    receiver: Receiver<(u64, Proposal)>,
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
    fn start(&mut self, number: u64, address: &str, ispdb: bool, stop: RunStop) {
        let (run, wake, sender, address) = (Arc::clone(&self.run), self.wake.clone(), self.sender.clone(), address.to_owned());
        let spawned = thread::Builder::new().name("inboxcraft-detect".to_owned()).spawn(move || {
            // A closed window no longer listens; nothing to tell it then.
            let _ = sender.send((number, run(&address, ispdb, &stop)));
            if let Some(wake) = wake {
                wake();
            }
        });
        if let Err(error) = spawned {
            let _ = self.sender.send((number, Proposal { failed: vec![error.to_string()], ..Proposal::default() }));
        }
    }

    fn finished(&mut self) -> Option<(u64, Proposal)> {
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
    /// The stop of each run started, in order.
    pub stops: Arc<std::sync::Mutex<Vec<RunStop>>>,
    /// The runs started and not answered yet, the oldest first.
    pending: std::collections::VecDeque<u64>,
}

impl Proposed {
    /// A Detect that answers `proposal`.
    #[must_use]
    pub fn new(proposal: Proposal) -> Self {
        Self { proposal, ..Self::default() }
    }
}

impl Detect for Proposed {
    fn start(&mut self, run: u64, address: &str, ispdb: bool, stop: RunStop) {
        if let Ok(mut asked) = self.asked.lock() {
            asked.push((address.to_owned(), ispdb));
        }
        if let Ok(mut stops) = self.stops.lock() {
            // Bounded like the runs a test starts: a picture starts one.
            if stops.len() < MOST_LINES {
                stops.push(stop);
            }
        }
        self.pending.push_back(run);
    }

    fn finished(&mut self) -> Option<(u64, Proposal)> {
        self.pending.pop_front().map(|run| (run, self.proposal.clone()))
    }
}

/// The most lines of what Detect asked, and of what failed, the form keeps and shows.
pub const MOST_LINES: usize = 32;

/// The most answers of Detect [`Onboarding::poll`] takes in one call.
const MOST_ANSWERS: usize = 8;

/// The most characters the password field takes.
pub const MAX_PASSWORD_CHARS: usize = crate::component::field::MAX_SECRET;

/// The bytes a password of [`MAX_PASSWORD_CHARS`] characters can take.
const SECRET_CAPACITY: usize = MAX_PASSWORD_CHARS * 4;

/// A password in memory: it has no `Display`, its `Debug` hides it, and its bytes are overwritten when it is cleared
/// or dropped. Its buffer is made once, big enough for [`MAX_PASSWORD_CHARS`] characters, so typing never moves the
/// text and leaves no copy of its own behind in a freed buffer (egui's per-frame copies are outside its reach).
pub struct Secret(String);

impl Default for Secret {
    fn default() -> Self {
        Self(String::with_capacity(SECRET_CAPACITY))
    }
}

impl Secret {
    /// The secret `text`, cut to [`MAX_PASSWORD_CHARS`] characters.
    #[must_use]
    pub fn new(text: &str) -> Self {
        let mut secret = Self::default();
        secret.0.extend(text.chars().take(MAX_PASSWORD_CHARS));
        secret
    }

    /// Whether nothing was typed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Overwrites every byte of the buffer, the unused ones too, and leaves it empty, with the same buffer.
    pub fn clear(&mut self) {
        let mut bytes = std::mem::take(&mut self.0).into_bytes();
        // Within the capacity: no new buffer.
        bytes.resize(bytes.capacity(), 0);
        bytes.fill(0);
        std::hint::black_box(&bytes);
        bytes.clear();
        self.0 = String::from_utf8(bytes).unwrap_or_default();
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
    /// The outgoing (SMTP) server; none when empty.
    pub smtp_host: String,
    /// The outgoing port; the usual one for the encryption when empty.
    pub smtp_port: String,
    /// The outgoing server's encryption.
    pub smtp_security: Option<Security>,
    /// The CalDAV address; none when empty.
    pub caldav: String,
    /// Detect asks Mozilla's ISPDB too, which tells Mozilla the domain.
    pub ispdb: bool,
}

/// A field of the form that can be wrong: its error stands under it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    /// The address.
    Address,
    /// The password.
    Password,
    /// The IMAP server.
    Host,
    /// The IMAP port.
    Port,
    /// The outgoing server's port.
    SmtpPort,
    /// The CalDAV address.
    Caldav,
}

/// The servers a form names, as the commands read them.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Servers {
    imap: Value,
    submission: Option<Value>,
}

/// The port typed as `text`: `None` for none (the usual one), else a number from 1 to 65535 or the error.
pub(crate) fn port_typed(text: &str) -> Result<Option<u16>, String> {
    match text.trim() {
        "" => Ok(None),
        text => text.parse::<u16>().ok().filter(|port| *port > 0).map(Some).ok_or_else(|| tr!("The port is a number from 1 to 65535.").to_owned()),
    }
}

/// The encryption a registered mail port is for: IMAP 143 STARTTLS and 993 TLS, submission 587 STARTTLS and 465 TLS,
/// 25 STARTTLS; `None` for any other port.
const fn registered(port: u16) -> Option<Security> {
    match port {
        993 | 465 => Some(Security::Tls),
        143 | 587 | 25 => Some(Security::StartTls),
        _ => None,
    }
}

/// Whether the encryption of a server row is unusual for its port: the port as it is typed (`usual` for the
/// encryption when empty) and the encryption the form sends (TLS when none is picked). The port and the encryption that
/// port is registered for, when it is the other one; `None` for the usual pair, an unregistered port or a port typed
/// wrong. It only says so: saving is not held back and nothing is changed.
#[must_use]
pub fn unusual_encryption(port: &str, security: Option<Security>, usual: fn(Security) -> u16) -> Option<(u16, Security)> {
    let security = security.unwrap_or(Security::Tls);
    let port = port_typed(port).ok()?.unwrap_or_else(|| usual(security));
    registered(port).filter(|registered| *registered != security).map(|registered| (port, registered))
}

/// `{host, port, security}` of a server the form names, `None` for an empty host; the usual port when none was typed.
fn server(host: &str, port: &str, security: Option<Security>, usual: fn(Security) -> u16) -> Result<Option<Value>, String> {
    let host = host.trim();
    if host.is_empty() {
        return Ok(None);
    }
    let security = security.unwrap_or(Security::Tls);
    let port = port_typed(port)?.unwrap_or_else(|| usual(security));
    Ok(Some(json!({ "host": host, "port": port, "security": security.param() })))
}

/// Trimmed `text`, `None` when that leaves nothing.
fn optional(text: &str) -> Option<String> {
    Some(text.trim().to_owned()).filter(|text| !text.is_empty())
}

/// `host:port` of a server the commands read.
fn where_to(server: &Value) -> String {
    let host = server.get("host").and_then(Value::as_str).unwrap_or_default();
    match server.get("port").and_then(Value::as_u64) {
        Some(port) => format!("{host}:{port}"),
        None => host.to_owned(),
    }
}

impl Form {
    /// What is wrong in what was typed, field by field, as it is typed: a port that is no port, a CalDAV address the
    /// app does not take. A field left empty is not wrong here.
    #[must_use]
    pub fn typed_faults(&self) -> Vec<(Slot, String)> {
        let mut faults = Vec::new();
        for (slot, text) in [(Slot::Port, &self.port), (Slot::SmtpPort, &self.smtp_port)] {
            if let Err(error) = port_typed(text) {
                faults.push((slot, error));
            }
        }
        if !Account::takes_caldav(&self.caldav) {
            faults.push((Slot::Caldav, tr!("The CalDAV address starts with https:// and holds no user name.").to_owned()));
        }
        faults
    }

    /// Everything that keeps the form from being sent, each with the field it stands under, in the form's order: what
    /// is missing (the address and the password only when `adding`), then what is wrong.
    #[must_use]
    pub fn faults(&self, adding: bool) -> Vec<(Slot, String)> {
        let mut faults = Vec::new();
        if adding && self.address.trim().is_empty() {
            faults.push((Slot::Address, tr!("Enter the address.").to_owned()));
        }
        if adding && self.password.is_empty() {
            faults.push((Slot::Password, tr!("Enter the password.").to_owned()));
        }
        if self.host.trim().is_empty() {
            faults.push((Slot::Host, tr!("Enter the IMAP server, or use Detect.").to_owned()));
        }
        faults.extend(self.typed_faults());
        faults
    }

    /// The first of [`Self::faults`].
    #[must_use]
    pub fn fault(&self, adding: bool) -> Option<(Slot, String)> {
        self.faults(adding).into_iter().next()
    }

    fn servers(&self) -> Result<Servers, String> {
        if let Some((_, error)) = self.fault(false) {
            return Err(error);
        }
        let imap = server(&self.host, &self.port, self.security, Security::imap_port)?
            .ok_or_else(|| tr!("Enter the IMAP server, or use Detect.").to_owned())?;
        let submission = server(&self.smtp_host, &self.smtp_port, self.smtp_security, Security::submission_port)?;
        if !Account::takes_caldav(&self.caldav) {
            return Err(tr!("The CalDAV address starts with https:// and holds no user name.").to_owned());
        }
        Ok(Servers { imap, submission })
    }

    /// The parameters of `account.add`: everything but the password.
    ///
    /// # Errors
    ///
    /// What is missing or wrong, in words for the owner.
    pub fn params(&self) -> Result<Value, String> {
        if let Some((_, error)) = self.fault(true) {
            return Err(error);
        }
        let address = self.address.trim();
        let Servers { imap, submission } = self.servers()?;
        Ok(json!({
            "address": address,
            "label": optional(&self.label),
            "display_name": optional(&self.display_name),
            "username": optional(&self.username),
            "imap": imap,
            "submission": submission,
        }))
    }

    /// The parameters of `account.update` for the account `id`: everything but the password. An empty name for the
    /// account or user name means the address, as in the add form.
    fn update_params(&self, id: u64, dry_run: bool, recheck: bool) -> Result<Value, String> {
        let Servers { imap, submission } = self.servers()?;
        let username = optional(&self.username).unwrap_or_else(|| self.address.trim().to_owned());
        Ok(json!({
            "account": id,
            "label": self.label.trim(),
            "display_name": optional(&self.display_name),
            "username": username,
            "imap": imap,
            "submission": submission,
            "dry_run": dry_run,
            "recheck": recheck,
        }))
    }
}

/// What the account being edited held when its form opened, to see what the owner changed.
#[derive(Debug, Clone)]
struct Held {
    label: String,
    display_name: String,
    username: String,
    servers: Option<Servers>,
    caldav: String,
}

/// What the owner changed in the form of the account being edited.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Changes {
    /// Anything `account.update` writes.
    settings: bool,
    /// Where or as whom it signs in.
    sign_in: bool,
    /// The CalDAV address.
    caldav: bool,
    /// Where the password would newly go, `host:port` or the CalDAV address.
    goes_to: Vec<String>,
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
    /// Not run: the account has no server for it (no submission server for the SMTP sign-in).
    NotSet,
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
    /// What kind of failure the check reported (`login_refused`), as the engine names it; `None` when it reported
    /// none.
    pub kind: Option<String>,
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
        Some("not_set") => StepState::NotSet,
        Some("not_built") => StepState::NotBuilt,
        _ => StepState::Pending,
    };
    let server = text("host").map(|host| match value.get("port").and_then(Value::as_u64) {
        Some(port) => format!("{host}:{port}"),
        None => host,
    });
    StepRow { id: text("step").unwrap_or_default(), state, server, issuer: text("issuer"), message: text("message"), kind: text("kind") }
}

/// Where the owner is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Onboarding is not open.
    Closed,
    /// The form.
    Form,
    /// The form of an account already there.
    Edit(u64),
    /// The account was added or changed and is being checked, or was checked.
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

/// A question Detect asked, with why it failed when it did.
pub type AskedLine = (String, Option<String>);

/// What Detect last found, as the form shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// Where the server in the form came from; `None` when nothing was found.
    pub source: Option<Source>,
    /// The outgoing server in the form and where it came from, when Detect found one.
    pub outgoing: Option<(String, Source)>,
    /// The IMAP server Detect found and left out, because the owner typed another.
    pub kept_imap: Option<String>,
    /// The outgoing server Detect found and left out, because the owner typed another.
    pub kept_outgoing: Option<String>,
    /// Why Detect did not reach the server it left out (IMAP, outgoing), when it did not.
    pub kept_unchecked: [Option<Checked>; 2],
    /// The encryption Detect set in the IMAP row and in the outgoing row where it replaced one the owner had picked.
    pub encryption_set: [Option<Security>; 2],
    /// What Detect asked: what left the computer; the first [`MOST_LINES`].
    pub asked: Vec<String>,
    /// The lookups that failed; the first [`MOST_LINES`].
    pub failed: Vec<String>,
    /// How many questions Detect asked, all of them.
    asked_count: usize,
    /// Each question shown, with why it failed when it did: matched once, when the answer came.
    column: Vec<AskedLine>,
    /// The lookups that failed for no question asked, the first [`MOST_LINES`].
    alone: Vec<String>,
}

impl Found {
    /// How many questions Detect asked, all of them.
    #[must_use]
    pub fn asked_count(&self) -> usize {
        self.asked_count
    }

    /// The questions to show, each with why it failed when it did.
    #[must_use]
    pub fn column(&self) -> &[AskedLine] {
        &self.column
    }

    /// The lookups that failed for no question asked.
    #[must_use]
    pub fn alone(&self) -> &[String] {
        &self.alone
    }

    /// What Detect asked and what failed, cleaned for the screen, kept to [`MOST_LINES`] each and matched once.
    fn lines(asked: &[String], failed: &[String]) -> (Vec<String>, Vec<String>, Vec<AskedLine>, Vec<String>) {
        let clean = |lines: &[String]| lines.iter().take(MOST_LINES).map(|line| shown(line)).collect::<Vec<_>>();
        let questions: BTreeSet<&str> = asked.iter().map(String::as_str).collect();
        // Whether a failed line names a question: its text before one of its ": " is a question asked.
        let named = |line: &str| line.match_indices(": ").any(|(at, _)| line.get(..at).is_some_and(|question| questions.contains(question)));
        let column = asked
            .iter()
            .take(MOST_LINES)
            .map(|question| {
                let cause = failed.iter().find_map(|line| line.strip_prefix(question.as_str()).and_then(|rest| rest.strip_prefix(": ")));
                (shown(question), cause.map(shown))
            })
            .collect();
        let alone = failed.iter().filter(|line| !named(line)).take(MOST_LINES).map(|line| shown(line)).collect();
        (clean(asked), clean(failed), column, alone)
    }
}

/// What Detect last put into the form, to tell it from what the owner typed: a server row as host, port and encryption,
/// and the user name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Detected {
    imap: Option<Row>,
    submission: Option<Row>,
    username: Option<String>,
    /// The address (trimmed) Detect last answered for: a server row still as Detect put it under another address in
    /// the form came from an earlier Detect ([`Onboarding::detect_older`]).
    address: Option<String>,
    /// The rows (IMAP, outgoing) whose encryption the owner picked since Detect last answered: the line that Detect set
    /// it is not said there, whichever encryption the owner picked.
    picked: [bool; 2],
    /// The rows (IMAP, outgoing) Detect filled with a server it did not reach, and why: said beside the row while it
    /// stands as Detect put it ([`Onboarding::unchecked`]); a new Detect takes them away.
    unchecked: [Option<(Checked, String)>; 2],
}

/// The add form as the owner left it, kept while onboarding is closed or on an account's form: the form, what Detect
/// found and put into it. The password is never kept.
#[derive(Debug, Default)]
struct Kept {
    form: Form,
    found: Option<Arc<Found>>,
    detected: Detected,
}

/// A server row of the form as Detect fills it: host, port and encryption.
type Row = (String, String, Option<Security>);

/// What [`fill`] did with a server row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Filled {
    /// Detect's server is in the row.
    Detect,
    /// The owner typed the row's host: the row stays theirs, Detect's server is left out.
    Owner,
    /// Detect found none: what it put there before is taken out.
    Nothing,
}

/// Puts Detect's `row` into a server row of the form, or, for none, takes out what Detect put there `before`; what the
/// owner typed stays. A host the owner typed makes the row theirs, and Detect leaves it. Otherwise the host is
/// Detect's, and its port too unless the owner typed one; the encryption goes with Detect's port. A picked encryption
/// alone never keeps Detect out: the select always shows one. Returns what it did, and what of the row is Detect's now.
fn fill(form: (&mut String, &mut String, &mut Option<Security>), before: Option<&Row>, row: Option<&Row>) -> (Filled, Option<Row>) {
    let (host, port, security) = form;
    let host_detects = before.is_some_and(|(was, _, _)| was == host);
    let port_detects = before.is_some_and(|(_, was, _)| was == port);
    let security_detects = before.is_some_and(|(_, _, was)| was == security);
    let Some((new_host, new_port, new_security)) = row else {
        if host_detects {
            host.clear();
        }
        if port_detects {
            port.clear();
        }
        if security_detects {
            *security = None;
        }
        return (Filled::Nothing, None);
    };
    if !host.trim().is_empty() && !host_detects {
        return (Filled::Owner, before.cloned());
    }
    host.clone_from(new_host);
    if port.trim().is_empty() || port_detects {
        port.clone_from(new_port);
        *security = *new_security;
    }
    (Filled::Detect, Some((host.clone(), port.clone(), *security)))
}

/// Onboarding's state.
pub struct Onboarding {
    stage: Stage,
    form: Form,
    /// Detect's run under way for the add form, and the address it asked about: only its answer is taken, and only
    /// while the add form is open and still holds that address.
    asking: Option<(u64, String)>,
    /// The stop of the run under way: set when the add form is left or cancelled, or another run starts.
    stop: Option<RunStop>,
    /// The number of Detect's last run.
    runs: u64,
    found: Option<Arc<Found>>,
    detected: Detected,
    /// The add form as the owner left it, while another form or none is open.
    kept: Option<Kept>,
    /// Each account's form the owner changed and left unsaved, with the account as it stood when the form opened; its
    /// password is never kept.
    kept_edits: BTreeMap<u64, (Form, Held)>,
    steps: Vec<StepRow>,
    running: bool,
    error: Option<String>,
    /// The errors of single fields the owner's last try left, each with the field it stands under, in the form's
    /// order; [`Self::error`] is the page's own.
    sent: Vec<(Slot, String)>,
    /// The field to take the keyboard next frame: the first of [`Self::sent`], once, after a refused try.
    focus: Option<Slot>,
    keychain: Arc<dyn Keychain>,
    /// The accounts whose password this window put into the credential store and has not removed yet.
    stored: BTreeSet<u64>,
    detect: Box<dyn Detect>,
    /// The account being edited as it stood: set from [`Onboarding::edit`] until [`Onboarding::close`], through the
    /// check a save starts, so "Change settings" comes back here and removes nothing.
    held: Option<Held>,
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
            asking: None,
            stop: None,
            runs: 0,
            found: None,
            detected: Detected::default(),
            kept: None,
            kept_edits: BTreeMap::new(),
            steps: Vec::new(),
            running: false,
            error: None,
            sent: Vec::new(),
            focus: None,
            keychain,
            stored: BTreeSet::new(),
            detect,
            held: None,
        }
    }

    /// Opens the add form: as the owner left it on the way out ([`Self::close`]), else empty. Open already, it stays
    /// as it is.
    pub fn open(&mut self) {
        if self.stage == Stage::Form {
            return;
        }
        self.keep();
        let Kept { form, found, detected } = self.kept.take().unwrap_or_default();
        self.form = form;
        self.found = found;
        self.detected = detected;
        self.error = None;
        self.sent.clear();
        self.steps.clear();
        self.held = None;
        self.stage = Stage::Form;
    }

    /// Closes onboarding; a password typed and not used is forgotten. The add form is kept as the owner left it, for
    /// [`Self::open`]; an added account stays, and an edited one stays as it was unless it was saved.
    pub fn close(&mut self) {
        self.form.password.clear();
        self.keep();
        self.held = None;
        self.stage = Stage::Closed;
    }

    /// The Accounts page is not shown (another page of the settings, or another place): the form stays open as the owner
    /// left it, its password forgotten, and Detect under way ends, as on every way out of the form; a check goes on and
    /// shows its result when the page shows again.
    pub fn leave_page(&mut self) {
        if matches!(self.stage, Stage::Form | Stage::Edit(_)) {
            self.form.password.clear();
            self.end_run();
        }
    }

    /// Whether the check of an added or changed account is under way.
    #[must_use]
    pub fn checking(&self) -> bool {
        matches!(self.stage, Stage::Checking(_)) && self.outcome() == Outcome::Running
    }

    /// Closes onboarding as [`Self::close`] does, and drops what the form open held: the owner's Cancel.
    pub fn cancel(&mut self) {
        let stage = self.stage;
        self.close();
        match stage {
            Stage::Edit(id) => {
                self.kept_edits.remove(&id);
            }
            Stage::Form | Stage::Closed => self.kept = None,
            Stage::Checking(_) => {}
        }
    }

    /// Keeps the form open as the owner left it while another form or none is open, its password forgotten: the add
    /// form, or an account's form the owner changed.
    fn keep(&mut self) {
        if let Stage::Edit(id) = self.stage {
            self.form.password.clear();
            let changes = self.changes();
            match self.held.take() {
                Some(held) if changes.settings || changes.caldav => {
                    self.kept_edits.insert(id, (std::mem::take(&mut self.form), held));
                }
                _ => {
                    self.kept_edits.remove(&id);
                }
            }
            return;
        }
        if self.stage != Stage::Form {
            return;
        }
        // Detect under way ends with the page: it is stopped, and its answer is thrown away if one still comes.
        self.end_run();
        self.form.password.clear();
        let form = std::mem::take(&mut self.form);
        self.kept = Some(Kept { form, found: self.found.take(), detected: std::mem::take(&mut self.detected) });
    }

    /// Opens the form of the account `id` with everything `account.list` shows of it. The password is not among
    /// it: the field starts empty, and an empty field keeps the stored one.
    pub fn edit(&mut self, mailbox: &mut Mailbox, id: u64) {
        self.error = None;
        self.sent.clear();
        let answer = match mailbox.run("account.list", &json!({})) {
            Ok(answer) => answer,
            Err(error) => {
                self.error = Some(error.to_string());
                return;
            }
        };
        let accounts = answer.get("accounts").and_then(Value::as_array).cloned().unwrap_or_default();
        let Some(account) = accounts.iter().find(|account| account.get("id").and_then(Value::as_u64) == Some(id)) else {
            self.error = Some(UiError::Answer { command: "account.list", detail: "no such account".to_owned() }.to_string());
            return;
        };
        // Found: only now is the form on the page left for the account's, so one that cannot open leaves it as it was.
        self.keep();
        let text = |value: Option<&Value>| value.and_then(Value::as_str).unwrap_or_default().to_owned();
        let port = |server: Option<&Value>| {
            server.and_then(|server| server.get("port")).and_then(Value::as_u64).map(|port| port.to_string()).unwrap_or_default()
        };
        let security =
            |server: Option<&Value>| server.and_then(|server| server.get("security")).and_then(Value::as_str).and_then(Security::from_param);
        // A form of an account no longer there is not kept.
        self.kept_edits.retain(|kept, _| accounts.iter().any(|account| account.get("id").and_then(Value::as_u64) == Some(*kept)));
        if let Some((form, held)) = self.kept_edits.remove(&id) {
            self.form = form;
            self.held = Some(held);
            self.found = None;
            self.steps.clear();
            self.stage = Stage::Edit(id);
            return;
        }
        let (imap, submission) = (account.get("imap"), account.get("submission").filter(|server| !server.is_null()));
        self.form = Form {
            address: text(account.get("address")),
            label: text(account.get("label")),
            display_name: text(account.get("display_name")),
            username: text(account.get("username")),
            password: Secret::default(),
            host: text(imap.and_then(|server| server.get("host"))),
            port: port(imap),
            security: security(imap),
            smtp_host: text(submission.and_then(|server| server.get("host"))),
            smtp_port: port(submission),
            smtp_security: security(submission),
            caldav: text(account.get("caldav")),
            ispdb: false,
        };
        self.held = Some(Held {
            label: self.form.label.clone(),
            display_name: self.form.display_name.clone(),
            username: self.form.username.clone(),
            servers: self.form.servers().ok(),
            caldav: self.form.caldav.trim().to_owned(),
        });
        self.found = None;
        self.steps.clear();
        self.stage = Stage::Edit(id);
    }

    /// What the owner changed in the form against the account as it stood.
    fn changes(&self) -> Changes {
        let Some(held) = &self.held else { return Changes::default() };
        let now = self.form.servers().ok();
        let caldav = self.form.caldav.trim();
        let username = self.form.username.trim() != held.username.trim();
        let moved_imap = now.as_ref().map(|servers| &servers.imap) != held.servers.as_ref().map(|servers| &servers.imap);
        let moved_submission = now.as_ref().map(|servers| &servers.submission) != held.servers.as_ref().map(|servers| &servers.submission);
        let caldav_changed = caldav != held.caldav;
        let sign_in = username || moved_imap || moved_submission;
        let mut goes_to = Vec::new();
        if let Some(servers) = &now {
            if moved_imap {
                goes_to.push(where_to(&servers.imap));
            }
            if let Some(submission) = servers.submission.as_ref().filter(|_| moved_submission) {
                goes_to.push(where_to(submission));
            }
        }
        if caldav_changed && !caldav.is_empty() {
            goes_to.push(caldav.to_owned());
        }
        let named = self.form.label.trim() != held.label.trim() || self.form.display_name.trim() != held.display_name.trim();
        Changes { settings: named || sign_in, sign_in, caldav: caldav_changed, goes_to }
    }

    /// Whether saving the form starts the account's check: a server or the user name changed, or a new password was typed.
    #[must_use]
    pub fn save_checks(&self) -> bool {
        self.checks_with(&self.changes())
    }

    /// [`Self::save_checks`] and [`Self::sign_in_note`] from one look at what changed: for the page, each frame.
    #[must_use]
    pub fn save_summary(&self) -> (bool, Option<String>) {
        let changes = self.changes();
        (self.checks_with(&changes), self.note_with(changes))
    }

    fn checks_with(&self, changes: &Changes) -> bool {
        changes.sign_in || changes.caldav || !self.form.password.is_empty()
    }

    /// What the form says before it is saved: which server the sign-in will go to, when it goes anywhere new.
    #[must_use]
    pub fn sign_in_note(&self) -> Option<String> {
        self.note_with(self.changes())
    }

    fn note_with(&self, changes: Changes) -> Option<String> {
        let goes_to = changes.goes_to;
        let new_password = !self.form.password.is_empty();
        if !goes_to.is_empty() {
            let servers = shown(&goes_to.join(", "));
            return Some(if new_password {
                tr!("Saving signs in to {servers} with the new password, to check them.", servers = servers)
            } else {
                tr!("Saving signs in to {servers} with your stored password, to check them.", servers = servers)
            });
        }
        if !new_password {
            return None;
        }
        let imap = self.form.servers().ok().map(|servers| where_to(&servers.imap))?;
        Some(tr!("Saving signs in to {server} with the new password.", server = shown(&imap)))
    }

    /// Saves the account being edited: the settings through `account.update` and `calendar.server`, and a typed new
    /// password into the credential store, under the account's id, replacing the old one. The worker reads it at its
    /// next sign-in. The commands are validated before the password goes in, so a form that is refused replaces
    /// nothing. When the sign-in moved or the password is new, the owner's check starts and shows next; otherwise the
    /// form closes. A check still running is left to end first, or it would read the password as it was.
    pub fn save(&mut self, mailbox: &mut Mailbox) {
        let Stage::Edit(id) = self.stage else { return };
        let changes = self.changes();
        let password = !self.form.password.is_empty();
        if !changes.settings && !changes.caldav && !password {
            self.close();
            return;
        }
        let checks = changes.sign_in || changes.caldav || password;
        if checks && self.check_running(mailbox, id) {
            self.error = Some(tr!("A check of this account is still running. Wait for it to end, then save.").to_owned());
            return;
        }
        let recheck = (changes.caldav || password) && !changes.sign_in;
        self.sent.clear();
        self.sent = self.form.faults(false);
        if !self.sent.is_empty() {
            self.error = None;
            self.focus = self.error_slot();
            return;
        }
        let (dry, params) = match (self.form.update_params(id, true, recheck), self.form.update_params(id, false, recheck)) {
            (Ok(dry), Ok(params)) => (dry, params),
            (Err(error), _) | (_, Err(error)) => {
                self.error = Some(error);
                return;
            }
        };
        if let Err(error) = mailbox.run("account.update", &dry) {
            self.error = Some(error.to_string());
            return;
        }
        if password {
            let stored = self.keychain.store(id, self.form.password.expose());
            self.form.password.clear();
            if let Err(refused) = stored {
                self.error = Some(tr!(
                    "The system's credential store did not take the new password, so nothing was changed: {reason}",
                    reason = shown(&refused)
                ));
                return;
            }
            self.stored.insert(id);
        }
        self.error = None;
        if let Err(error) = mailbox.run("account.update", &params) {
            self.error = Some(if password {
                tr!("The new password was stored, but the settings were not saved: {reason}", reason = error.to_string())
            } else {
                error.to_string()
            });
            mailbox.refresh();
            return;
        }
        if changes.caldav
            && let Err(error) = mailbox.run("calendar.server", &json!({ "account": id, "url": optional(&self.form.caldav) }))
        {
            self.error = Some(error.to_string());
        }
        mailbox.refresh();
        if checks {
            self.stage = Stage::Checking(id);
            self.load(mailbox);
        } else if self.error.is_none() {
            // Saved: nothing of the form is left to keep.
            self.held = None;
            self.close();
        }
    }

    /// Whether a check of the account `id` runs now.
    fn check_running(&self, mailbox: &mut Mailbox, id: u64) -> bool {
        let Ok(answer) = mailbox.run("account.list", &json!({})) else { return false };
        answer
            .get("accounts")
            .and_then(Value::as_array)
            .and_then(|accounts| accounts.iter().find(|account| account.get("id").and_then(Value::as_u64) == Some(id)))
            .and_then(|account| account.get("check"))
            .and_then(|check| check.get("running"))
            .and_then(Value::as_bool)
            .unwrap_or(false)
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
        self.asking.is_some()
    }

    /// What Detect last found.
    #[must_use]
    pub fn found(&self) -> Option<&Found> {
        self.found.as_deref()
    }

    /// What Detect last found, shared: for the page to hold while it draws the form.
    #[must_use]
    pub fn found_shared(&self) -> Option<Arc<Found>> {
        self.found.clone()
    }

    /// The steps of the check.
    #[must_use]
    pub fn steps(&self) -> &[StepRow] {
        &self.steps
    }

    /// What went wrong last, in words for the owner.
    #[must_use]
    pub fn error(&self) -> Option<&str> {
        self.sent.first().map(|(_, error)| error.as_str()).or(self.error.as_deref())
    }

    /// The field [`Self::error`] stands under, when it is one field's; `None` for an error of the whole page.
    #[must_use]
    pub fn error_slot(&self) -> Option<Slot> {
        self.sent.first().map(|(slot, _)| *slot)
    }

    /// The errors of single fields the owner's last try left, each with its field, in the form's order.
    #[must_use]
    pub fn field_errors(&self) -> &[(Slot, String)] {
        &self.sent
    }

    /// What went wrong last that belongs to no single field.
    #[must_use]
    pub fn page_error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// The field to give the keyboard to now, once: the first one a refused try named.
    pub fn take_focus(&mut self) -> Option<Slot> {
        self.focus.take()
    }

    /// The owner changed the field `slot`: the error under it goes, until the form is sent again.
    pub fn edited(&mut self, slot: Slot) {
        self.sent.retain(|(at, _)| *at != slot);
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
        if self.stage != Stage::Form {
            return;
        }
        let address = self.form.address.trim().to_owned();
        if address.is_empty() {
            self.error = None;
            self.sent = vec![(Slot::Address, tr!("Enter the address, then Detect.").to_owned())];
            self.focus = Some(Slot::Address);
            return;
        }
        self.error = None;
        self.sent.clear();
        // A run still under way asks no more: only the new one's answer is taken.
        self.end_run();
        // What the last answer said of the servers it did not reach goes with it.
        self.detected.unchecked = [None, None];
        self.runs = self.runs.wrapping_add(1);
        let stop = RunStop::default();
        self.detect.start(self.runs, &address, self.form.ispdb, stop.clone());
        self.asking = Some((self.runs, address));
        self.stop = Some(stop);
    }

    /// Ends Detect's run under way, if one is: its stop is set, so it starts no further lookup or probe, and its answer
    /// is thrown away when it comes.
    fn end_run(&mut self) {
        if let Some(stop) = self.stop.take() {
            stop.stop();
        }
        self.asking = None;
    }

    /// Takes Detect's answer when it came: its first IMAP and outgoing server fill their rows of the form, and its user
    /// name the user name, each only where the owner typed nothing or kept what Detect put there before. An answer is
    /// the add form's only for the run under way, while that form is open and holds the address asked about; any other
    /// (a run of a page left since, an address changed since) is thrown away. Whether anything changed.
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        // A few answers at most a frame: each run answers once, and only one is under way at a time.
        for _ in 0..MOST_ANSWERS {
            let Some((run, proposal)) = self.detect.finished() else { break };
            let Some((asked, address)) = self.asking.take() else { continue };
            if run != asked {
                self.asking = Some((asked, address));
                continue;
            }
            changed = true;
            self.stop = None;
            if self.stage == Stage::Form && self.form.address.trim() == address {
                self.take(proposal);
                self.detected.address = Some(address);
            }
        }
        changed
    }

    /// Puts Detect's `proposal` into the add form, as [`Self::poll`] says.
    fn take(&mut self, proposal: Proposal) {
        let form = &mut self.form;
        let detected = &mut self.detected;
        let row = |best: &Suggestion| (shown(&best.host), best.port.to_string(), Some(best.security));
        let best = proposal.imap.first();
        // An encryption the owner picked: one there that Detect did not put there.
        let picked = |security: Option<Security>, before: Option<&Row>| security.filter(|_| before.is_none_or(|(_, _, was)| *was != security));
        let replaced = |picked: Option<Security>, now: Option<Security>| picked.filter(|picked| now != Some(*picked)).and(now);
        let imap_picked = picked(form.security, detected.imap.as_ref());
        let (imap, now) = fill((&mut form.host, &mut form.port, &mut form.security), detected.imap.as_ref(), best.map(row).as_ref());
        detected.imap = now;
        let imap_set = replaced(imap_picked, form.security);
        // The user name: Detect's where the owner typed none, taken out with Detect's server when it found none.
        let username_detects = detected.username.as_deref() == Some(form.username.as_str());
        match best.map(|best| shown(&best.username)) {
            Some(username) if form.username.trim().is_empty() || username_detects => {
                form.username.clone_from(&username);
                detected.username = Some(username);
            }
            Some(_) => {}
            None => {
                if username_detects {
                    form.username.clear();
                }
                detected.username = None;
            }
        }
        let outgoing_best = proposal.submission.first();
        let outgoing_picked = picked(form.smtp_security, detected.submission.as_ref());
        let (outgoing, now) =
            fill((&mut form.smtp_host, &mut form.smtp_port, &mut form.smtp_security), detected.submission.as_ref(), outgoing_best.map(row).as_ref());
        detected.submission = now;
        let encryption_set = [imap_set, replaced(outgoing_picked, form.smtp_security)];
        let source = best.filter(|_| imap == Filled::Detect).map(|best| best.source);
        let kept_imap = best.filter(|_| imap == Filled::Owner).map(|best| shown(&best.host));
        let in_form = outgoing_best.filter(|_| outgoing == Filled::Detect).map(|best| (shown(&best.host), best.source));
        let kept_outgoing = outgoing_best.filter(|_| outgoing == Filled::Owner).map(|best| shown(&best.host));
        let kept_why = |best: Option<&Suggestion>, filled: Filled| {
            best.filter(|best| filled == Filled::Owner && best.checked != Checked::Answered).map(|best| best.checked)
        };
        let kept_unchecked = [kept_why(best, imap), kept_why(outgoing_best, outgoing)];
        // A server Detect put into a row without having reached it: the row says so, and why.
        let unchecked = |best: Option<&Suggestion>, filled: Filled| {
            best.filter(|best| filled == Filled::Detect && best.checked != Checked::Answered).map(|best| (best.checked, host_in_line(&best.host)))
        };
        self.detected.unchecked = [unchecked(best, imap), unchecked(outgoing_best, outgoing)];
        let (asked, failed, column, alone) = Found::lines(&proposal.asked, &proposal.failed);
        let asked_count = proposal.asked.len();
        self.found = Some(Arc::new(Found {
            source,
            outgoing: in_form,
            kept_imap,
            kept_outgoing,
            kept_unchecked,
            encryption_set,
            asked,
            failed,
            asked_count,
            column,
            alone,
        }));
        self.detected.picked = [false; 2];
    }

    /// Whether the add form's server rows come from an earlier Detect, for another address than the one in the form now:
    /// Detect filled at least one row, every row it filled stands as it put it (no server field edited by hand since),
    /// the address changed after it answered, and no Detect runs. A form filled by hand never says so.
    #[must_use]
    pub fn detect_older(&self) -> bool {
        if self.stage != Stage::Form || self.detecting() {
            return false;
        }
        let detected = &self.detected;
        if detected.address.as_deref().is_none_or(|address| address == self.form.address.trim()) {
            return false;
        }
        let form = &self.form;
        let rows = [
            (detected.imap.as_ref(), (&form.host, &form.port, form.security)),
            (detected.submission.as_ref(), (&form.smtp_host, &form.smtp_port, form.smtp_security)),
        ];
        let mut filled = rows.iter().filter_map(|(row, now)| row.map(|row| (row, now))).peekable();
        filled.peek().is_some()
            && filled.all(|((host, port, security), (now_host, now_port, now_security))| {
                host == *now_host && port == *now_port && security == now_security
            })
    }

    /// Whether the user name, too, is the earlier Detect's that [`Self::detect_older`] says the servers are: Detect
    /// filled it and it stands as Detect put it.
    #[must_use]
    pub fn detect_older_user_name(&self) -> bool {
        self.detect_older() && self.detected.username.as_deref().is_some_and(|username| !username.is_empty() && username == self.form.username)
    }

    /// The encryption Detect set in each server row (IMAP, outgoing) where it replaced one the owner had picked, as long
    /// as the owner has not picked that row's encryption since: what the line under the row says.
    #[must_use]
    pub fn encryption_said(&self) -> [Option<Security>; 2] {
        let set = self.found.as_ref().map_or([None, None], |found| found.encryption_set);
        let [imap, outgoing] = self.detected.picked;
        [set[0].filter(|_| !imap), set[1].filter(|_| !outgoing)]
    }

    /// What each server row (IMAP, outgoing) says of the server Detect put there without having reached it: why it was
    /// not checked. Only while the add form is open, no Detect runs, and the row's host, port and encryption stand as
    /// Detect put them; an edit of any of them takes the line away.
    #[must_use]
    pub fn unchecked(&self) -> [Option<String>; 2] {
        if self.stage != Stage::Form || self.detecting() {
            return [None, None];
        }
        let form = &self.form;
        let detected = &self.detected;
        let rows = [
            (detected.imap.as_ref(), (&form.host, &form.port, form.security)),
            (detected.submission.as_ref(), (&form.smtp_host, &form.smtp_port, form.smtp_security)),
        ];
        let mut lines = [None, None];
        for ((line, (row, (host, port, security))), said) in detected.unchecked.iter().zip(rows).zip(lines.iter_mut()) {
            let stands = row.is_some_and(|(was_host, was_port, was_security)| was_host == host && was_port == port && *was_security == security);
            *said = line.as_ref().filter(|_| stands).and_then(|(checked, host)| unchecked_line(*checked, host));
        }
        lines
    }

    /// The owner picked the encryption of server row `row` (0 IMAP, 1 outgoing): Detect's line there is not said again.
    pub fn encryption_picked(&mut self, row: usize) {
        if let Some(picked) = self.detected.picked.get_mut(row) {
            *picked = true;
        }
    }

    /// Adds the account in the form through `mailbox` and keeps its password in the credential store; the check
    /// `account.add` started shows next. The password is forgotten here either way it ends.
    pub fn add(&mut self, mailbox: &mut Mailbox) {
        self.sent.clear();
        self.sent = self.form.faults(true);
        if !self.sent.is_empty() {
            self.error = None;
            self.focus = self.error_slot();
            return;
        }
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
        self.stored.insert(id);
        self.error = None;
        // What the form held is an account now: the next add form starts empty. What Detect put into it is still
        // known, for Change settings, which brings this form back; the next add form starts without it.
        self.kept = None;
        // Checked by the form, so this answers an error only when the store fails; the check goes on without it.
        if let Some(url) = optional(&self.form.caldav)
            && let Err(error) = mailbox.run("calendar.server", &json!({ "account": id, "url": url }))
        {
            self.error = Some(error.to_string());
        }
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
    /// be typed again. An account that was edited is not removed: its form opens again as it now stands.
    pub fn change(&mut self, mailbox: &mut Mailbox) {
        let Stage::Checking(id) = self.stage else { return };
        self.sent.clear();
        if self.held.is_some() {
            self.edit(mailbox, id);
            return;
        }
        let removed = mailbox.run("account.remove", &json!({ "account": id }));
        let forgotten = self.keychain.forget(id);
        if forgotten.is_ok() {
            // One the store refused is tried again by `forget_passwords`.
            self.stored.remove(&id);
        }
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

    /// Shows the check of the account `id` with `steps`, `running` or not: a test's way to any check at all.
    #[cfg(test)]
    pub(crate) fn show_check(&mut self, id: u64, steps: Vec<StepRow>, running: bool) {
        self.stage = Stage::Checking(id);
        self.steps = steps;
        self.running = running;
    }

    /// How many forms are kept on the way out, and whether any of them holds a password: a test's look inside.
    #[cfg(test)]
    pub(crate) fn kept_forms(&self) -> (usize, bool) {
        let forms: Vec<&Form> = self.kept.iter().map(|kept| &kept.form).chain(self.kept_edits.values().map(|(form, _)| form)).collect();
        (forms.len(), forms.iter().any(|form| !form.password.is_empty()))
    }

    /// Removes from the credential store the password of every account added here, for the end of a window over a
    /// store that is not kept, whose accounts the passwords would outlive. A window over a kept store does not call it:
    /// a password stays until its account is removed. One the credential store refuses again stays behind.
    pub fn forget_passwords(&mut self) {
        let keychain = &self.keychain;
        self.stored.retain(|id| keychain.forget(*id).is_err());
    }
}

#[cfg(test)]
#[path = "onboard_tests.rs"]
pub(crate) mod tests;
