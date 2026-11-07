// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The opt-in live test harness: tests against the owner's two throwaway test accounts.
//!
//! Agents never run these tests and never hold the credentials; the owner runs them by hand with `just it-live`,
//! which reads the credentials from the OS credential store into the container for that run only. Everywhere else
//! they skip cleanly:
//!
//! - [`start`] reads the `INBOXCRAFT_LIVE_*` variables. Without any of them, and always when `AGENT_HORDE` (the
//!   agent sandbox) or `CI` is set, it prints the reason and returns `None`, and the test returns early. A partial
//!   or malformed configuration is an error, which names the variable and never its value.
//! - The two accounts, both on the reference provider, are called `live-imap` and `live-imap2` in output; their addresses, user names and secrets
//!   never appear in a [`fmt::Debug`] or [`fmt::Display`] of anything here. A [`Secret`] prints `[redacted]`.
//!   Values a test prints itself, or passes to a failing `assert_eq!`, are not redacted.
//! - [`LiveConfig::check_recipient`] is the recipient allow-list: exactly the two addresses given at run time, each
//!   a plain dot-atom address. A live test that sends mail checks every recipient with it first.
//! - [`LiveConfig::run_folder`] names the per-run folder `InboxCraft-Test/<run id>`, and [`RunFolder`] creates it and
//!   removes it again, also when the test fails. [`RunFolder`] takes no other name, so it deletes nothing else.
//! - [`LiveConfig::leaked_secret`] scans a log or panic output for the secret values, and
//!   [`LiveConfig::leaked_identifier`] for the addresses and login names, each raw and as `{:?}` prints it.
//!
//! | Variable | Meaning |
//! |---|---|
//! | `INBOXCRAFT_LIVE_{IMAP,IMAP2}_ADDRESS` | the account's mail address, one of the two allowed recipients |
//! | `INBOXCRAFT_LIVE_{IMAP,IMAP2}_HOST` | the server's host name |
//! | `INBOXCRAFT_LIVE_{IMAP,IMAP2}_USER` | the login name |
//! | `INBOXCRAFT_LIVE_{IMAP,IMAP2}_SECRET` | the password or token |
//! | `INBOXCRAFT_LIVE_RUN_ID` | optional: the run id; without it, the UTC time and the process id |
//!
//! ```
//! use inboxcraft_testkit::live::Live;
//!
//! # fn main() -> Result<(), inboxcraft_testkit::live::LiveError> {
//! let live = Live::from_vars(|_| None)?;
//! assert!(matches!(live, Live::Skip(_)));
//! # Ok(())
//! # }
//! ```

use std::fmt;

use time::OffsetDateTime;

/// The folder every run folder lives under.
pub const RUN_ROOT: &str = "InboxCraft-Test";

/// The longest value read from a variable, in bytes.
const MAX_VALUE: usize = 4096;
/// The longest run id, in bytes.
const MAX_RUN_ID: usize = 64;
/// The variable naming the run id.
const RUN_ID: &str = "INBOXCRAFT_LIVE_RUN_ID";

/// The variables of one account.
struct AccountVars {
    name: &'static str,
    address: &'static str,
    host: &'static str,
    user: &'static str,
    secret: &'static str,
}

const IMAP: AccountVars = AccountVars {
    name: "live-imap",
    address: "INBOXCRAFT_LIVE_IMAP_ADDRESS",
    host: "INBOXCRAFT_LIVE_IMAP_HOST",
    user: "INBOXCRAFT_LIVE_IMAP_USER",
    secret: "INBOXCRAFT_LIVE_IMAP_SECRET",
};

const IMAP2: AccountVars = AccountVars {
    name: "live-imap2",
    address: "INBOXCRAFT_LIVE_IMAP2_ADDRESS",
    host: "INBOXCRAFT_LIVE_IMAP2_HOST",
    user: "INBOXCRAFT_LIVE_IMAP2_USER",
    secret: "INBOXCRAFT_LIVE_IMAP2_SECRET",
};

impl AccountVars {
    fn all(&self) -> [&'static str; 4] {
        [self.address, self.host, self.user, self.secret]
    }
}

/// Why the live configuration cannot be used. No variant carries a value read from the environment.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum LiveError {
    /// Some `INBOXCRAFT_LIVE_*` variables are set and these are not.
    Incomplete {
        /// The names of the missing variables.
        missing: Vec<&'static str>,
    },
    /// A variable holds a value the harness refuses.
    Invalid {
        /// The variable's name.
        variable: &'static str,
        /// What is wrong with it.
        why: &'static str,
    },
    /// Both accounts have the same address.
    SameAddress,
    /// A recipient is not one of the two run-time addresses.
    RecipientRefused,
    /// The hierarchy delimiter could make the run folder name something other than a folder under [`RUN_ROOT`].
    Delimiter,
}

impl fmt::Display for LiveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Incomplete { missing } => {
                write!(f, "live test configuration is incomplete, missing: {}", missing.join(", "))
            }
            Self::Invalid { variable, why } => write!(f, "{variable} {why}"),
            Self::SameAddress => f.write_str("live-imap and live-imap2 have the same address"),
            Self::RecipientRefused => f.write_str("recipient refused: not one of the two live test accounts"),
            Self::Delimiter => f.write_str("the hierarchy delimiter is a letter, a digit, -, _, white space, a control character, * or %"),
        }
    }
}

impl std::error::Error for LiveError {}

/// A password or token. It prints as `[redacted]`; only [`Secret::expose`] gives the value.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    /// The value, for the one place that sends it to the server.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[redacted]")
    }
}

impl fmt::Display for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[redacted]")
    }
}

/// One live test account. Its debug output shows the name only.
#[derive(Clone)]
pub struct Account {
    name: &'static str,
    address: String,
    host: String,
    user: String,
    secret: Secret,
}

impl Account {
    /// The name in output: `live-imap` or `live-imap2`.
    #[must_use]
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// The mail address.
    #[must_use]
    pub fn address(&self) -> &str {
        &self.address
    }

    /// The server's host name.
    #[must_use]
    pub fn host(&self) -> &str {
        &self.host
    }

    /// The login name.
    #[must_use]
    pub fn user(&self) -> &str {
        &self.user
    }

    /// The password or token.
    #[must_use]
    pub fn secret(&self) -> &Secret {
        &self.secret
    }
}

impl fmt::Debug for Account {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Account").field("name", &self.name).finish_non_exhaustive()
    }
}

/// Whether the live tests run, and with what.
#[derive(Debug)]
pub enum Live {
    /// They skip, for this reason.
    Skip(SkipReason),
    /// They run with this configuration.
    Run(Box<LiveConfig>),
}

/// Why the live tests skip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SkipReason {
    /// `AGENT_HORDE` is set: the agent sandbox.
    AgentSandbox,
    /// `CI` is set.
    Ci,
    /// No `INBOXCRAFT_LIVE_*` credential is set.
    NoCredentials,
}

impl fmt::Display for SkipReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::AgentSandbox => "the agent sandbox never runs live tests (AGENT_HORDE is set)",
            Self::Ci => "CI never runs live tests (CI is set)",
            Self::NoCredentials => "no INBOXCRAFT_LIVE_* credentials are set; the owner runs these with `just it-live`",
        })
    }
}

impl Live {
    /// Reads the configuration from the process environment.
    ///
    /// # Errors
    ///
    /// As [`Live::from_vars`]; a variable that is not valid UTF-8 counts as empty and is refused.
    pub fn from_env() -> Result<Self, LiveError> {
        Self::from_vars(|name| std::env::var_os(name).map(|value| value.into_string().unwrap_or_default()))
    }

    /// Reads the configuration through `get`, which returns a variable's value or `None` when it is not set.
    ///
    /// # Errors
    ///
    /// [`LiveError::Incomplete`] when some credentials are set and others are not, [`LiveError::Invalid`] for an
    /// empty, overlong or malformed value, [`LiveError::SameAddress`] when the two addresses are one.
    pub fn from_vars(get: impl Fn(&str) -> Option<String>) -> Result<Self, LiveError> {
        let set = |name: &str| get(name).is_some_and(|value| !value.is_empty());
        if set("AGENT_HORDE") {
            return Ok(Self::Skip(SkipReason::AgentSandbox));
        }
        if set("CI") {
            return Ok(Self::Skip(SkipReason::Ci));
        }
        let names = IMAP.all().into_iter().chain(IMAP2.all());
        let missing: Vec<&'static str> = names.clone().filter(|name| get(name).is_none()).collect();
        if missing.len() == names.count() {
            return Ok(Self::Skip(SkipReason::NoCredentials));
        }
        if !missing.is_empty() {
            return Err(LiveError::Incomplete { missing });
        }
        let imap = account(&IMAP, &get)?;
        let imap2 = account(&IMAP2, &get)?;
        if same_address(&imap.address, &imap2.address) {
            return Err(LiveError::SameAddress);
        }
        let run_id = match get(RUN_ID) {
            Some(id) => run_id(id)?,
            None => made_run_id(),
        };
        Ok(Self::Run(Box::new(LiveConfig { imap, imap2, run_id })))
    }
}

/// The configuration of a live run: the two accounts and the run id.
#[derive(Debug, Clone)]
pub struct LiveConfig {
    imap: Account,
    imap2: Account,
    run_id: String,
}

impl LiveConfig {
    /// The first account, `live-imap`.
    #[must_use]
    pub fn imap(&self) -> &Account {
        &self.imap
    }

    /// The second account on the reference provider, `live-imap2`.
    #[must_use]
    pub fn imap2(&self) -> &Account {
        &self.imap2
    }

    /// Both accounts, `live-imap` first.
    #[must_use]
    pub fn accounts(&self) -> [&Account; 2] {
        [&self.imap, &self.imap2]
    }

    /// The run id: letters, digits, `-` and `_`.
    #[must_use]
    pub fn run_id(&self) -> &str {
        &self.run_id
    }

    /// The per-run folder, `InboxCraft-Test<delimiter><run id>`, with the server's hierarchy delimiter.
    ///
    /// # Errors
    ///
    /// [`LiveError::Delimiter`] for a delimiter that could run into the root or the run id, or that a server reads
    /// as white space or a wildcard: a letter, a digit, `-`, `_`, white space, a control character, `*` or `%`.
    pub fn run_folder(&self, delimiter: char) -> Result<String, LiveError> {
        if delimiter.is_alphanumeric() || delimiter.is_whitespace() || delimiter.is_control() || matches!(delimiter, '-' | '_' | '*' | '%') {
            return Err(LiveError::Delimiter);
        }
        Ok(format!("{RUN_ROOT}{delimiter}{}", self.run_id))
    }

    /// The `number`th invented message of the run, for a live test to APPEND to the per-run folder: from and to
    /// `live-test@example.org`, its subject naming the run and the number, CRLF line ends. Nothing in it is about a
    /// real person, and APPEND sends it to nobody.
    #[must_use]
    pub fn invented_message(&self, number: u32) -> String {
        let run = &self.run_id;
        [
            "From: InboxCraft live test <live-test@example.org>".to_owned(),
            "To: InboxCraft live test <live-test@example.org>".to_owned(),
            format!("Subject: {} {run} message {number}", self.message_subject_prefix()),
            "Date: Thu, 05 Nov 2026 12:00:00 +0000".to_owned(),
            format!("Message-ID: <{run}.{number}@live-test.example.org>"),
            "MIME-Version: 1.0".to_owned(),
            "Content-Type: text/plain; charset=utf-8".to_owned(),
            String::new(),
            format!("An invented message of the InboxCraft live tests, run {run}. It can be deleted."),
            String::new(),
        ]
        .join("\r\n")
    }

    /// The start of the subject of every [`LiveConfig::invented_message`].
    #[must_use]
    pub fn message_subject_prefix(&self) -> &'static str {
        "InboxCraft live test"
    }

    /// The recipient allow-list: admits exactly the two run-time addresses, the domain compared without case.
    ///
    /// # Errors
    ///
    /// [`LiveError::RecipientRefused`] for any other address; the error does not repeat it.
    pub fn check_recipient(&self, recipient: &str) -> Result<(), LiveError> {
        if self.accounts().iter().any(|account| same_address(&account.address, recipient)) { Ok(()) } else { Err(LiveError::RecipientRefused) }
    }

    /// The account whose secret appears in `text` (a log or panic output), raw or as `{:?}` prints it, or `None`
    /// when no secret does.
    #[must_use]
    pub fn leaked_secret(&self, text: &str) -> Option<&'static str> {
        self.accounts().into_iter().find(|account| appears(text, account.secret.expose())).map(|account| account.name)
    }

    /// The account and the field (`address` or `user`) whose value appears in `text`, raw or as `{:?}` prints it,
    /// or `None` when neither address nor login name does.
    #[must_use]
    pub fn leaked_identifier(&self, text: &str) -> Option<(&'static str, &'static str)> {
        self.accounts().into_iter().find_map(|account| {
            [("address", account.address.as_str()), ("user", account.user.as_str())]
                .into_iter()
                .find(|(_, value)| appears(text, value))
                .map(|(field, _)| (account.name, field))
        })
    }
}

/// Whether `value` appears in `text` as it is or as `{:?}` prints it, without the quotes.
fn appears(text: &str, value: &str) -> bool {
    let debug = format!("{value:?}");
    let escaped = debug.strip_prefix('"').and_then(|rest| rest.strip_suffix('"')).unwrap_or(&debug);
    text.contains(value) || text.contains(escaped)
}

/// Reads the live configuration for the test `test`: `Some` to run, `None` after printing why it skips.
///
/// # Errors
///
/// As [`Live::from_env`].
pub fn start(test: &str) -> Result<Option<LiveConfig>, LiveError> {
    start_from(test, |name| std::env::var_os(name).map(|value| value.into_string().unwrap_or_default()))
}

/// [`start`] with the variables read through `get`, as [`Live::from_vars`].
///
/// # Errors
///
/// As [`Live::from_vars`].
pub fn start_from(test: &str, get: impl Fn(&str) -> Option<String>) -> Result<Option<LiveConfig>, LiveError> {
    match Live::from_vars(get)? {
        Live::Run(config) => Ok(Some(*config)),
        Live::Skip(reason) => {
            eprintln!("SKIP {test}: {reason}");
            Ok(None)
        }
    }
}

/// Creating and deleting folders on a live account; the IMAP client of `net` implements it.
pub trait Folders {
    /// Why a folder operation failed.
    type Error;

    /// Creates the folder `name`.
    ///
    /// # Errors
    ///
    /// The server refused or the connection failed.
    fn create(&mut self, name: &str) -> Result<(), Self::Error>;

    /// Deletes the folder `name` and what is in it.
    ///
    /// # Errors
    ///
    /// The server refused or the connection failed.
    fn delete(&mut self, name: &str) -> Result<(), Self::Error>;
}

/// Why [`RunFolder::create`] made no folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunFolderError<E> {
    /// The harness refused before asking the server: see [`LiveConfig::run_folder`].
    Refused(LiveError),
    /// The error of [`Folders::create`].
    Folders(E),
}

impl<E: fmt::Display> fmt::Display for RunFolderError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(err) => err.fmt(f),
            Self::Folders(err) => err.fmt(f),
        }
    }
}

impl<E: fmt::Debug + fmt::Display> std::error::Error for RunFolderError<E> {}

/// The per-run folder: created by [`RunFolder::create`], deleted by [`RunFolder::remove`] or, when a failing test
/// never gets there, on drop. Its name is always [`LiveConfig::run_folder`], so it can delete nothing else.
pub struct RunFolder<'a, F: Folders> {
    folders: &'a mut F,
    name: String,
    open: bool,
}

impl<'a, F: Folders> RunFolder<'a, F> {
    /// Creates the run's folder, [`LiveConfig::run_folder`] with the server's hierarchy delimiter.
    ///
    /// # Errors
    ///
    /// [`RunFolderError::Refused`] for a delimiter [`LiveConfig::run_folder`] refuses, before the server is asked;
    /// [`RunFolderError::Folders`] with the error of [`Folders::create`]. Nothing is left to remove then.
    pub fn create(folders: &'a mut F, config: &LiveConfig, delimiter: char) -> Result<Self, RunFolderError<F::Error>> {
        let name = config.run_folder(delimiter).map_err(RunFolderError::Refused)?;
        folders.create(&name).map_err(RunFolderError::Folders)?;
        Ok(Self { folders, name, open: true })
    }

    /// The folder's name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The account, for the test's own work while the folder exists.
    pub fn folders(&mut self) -> &mut F {
        self.folders
    }

    /// Deletes the folder.
    ///
    /// # Errors
    ///
    /// The error of [`Folders::delete`]; the folder is not tried again on drop.
    pub fn remove(mut self) -> Result<(), F::Error> {
        self.open = false;
        self.folders.delete(&self.name)
    }
}

impl<F: Folders> Drop for RunFolder<'_, F> {
    fn drop(&mut self) {
        if self.open {
            // Best effort: a drop cannot report, and the next run uses a new folder anyway.
            let _ = self.folders.delete(&self.name);
        }
    }
}

fn account(vars: &AccountVars, get: &impl Fn(&str) -> Option<String>) -> Result<Account, LiveError> {
    let address = value(vars.address, get)?;
    if !is_address(&address) {
        return Err(LiveError::Invalid { variable: vars.address, why: "is not a single mail address" });
    }
    Ok(Account {
        name: vars.name,
        address,
        host: plain_value(vars.host, get)?,
        user: plain_value(vars.user, get)?,
        secret: Secret(value(vars.secret, get)?),
    })
}

/// A value that is set, not empty, not overlong and without control characters.
fn value(variable: &'static str, get: &impl Fn(&str) -> Option<String>) -> Result<String, LiveError> {
    let value = get(variable).unwrap_or_default();
    if value.is_empty() {
        Err(LiveError::Invalid { variable, why: "is empty or not valid UTF-8" })
    } else if value.len() > MAX_VALUE {
        Err(LiveError::Invalid { variable, why: "is longer than 4096 bytes" })
    } else if value.chars().any(char::is_control) {
        Err(LiveError::Invalid { variable, why: "contains a control character" })
    } else {
        Ok(value)
    }
}

/// A [`value`] that has no white space either: a host or a login name.
fn plain_value(variable: &'static str, get: &impl Fn(&str) -> Option<String>) -> Result<String, LiveError> {
    let value = value(variable, get)?;
    if value.chars().any(char::is_whitespace) {
        return Err(LiveError::Invalid { variable, why: "contains white space" });
    }
    Ok(value)
}

/// A plain dot-atom address, `local@domain`: no quotes, brackets, comments or address lists, so that every later
/// layer reads it as this one address.
fn is_address(text: &str) -> bool {
    match text.split_once('@') {
        Some((local, domain)) => is_dot_atom(local) && is_domain(domain),
        None => false,
    }
}

/// RFC 5322 `dot-atom-text` in ASCII: `atext` runs joined by single dots.
fn is_dot_atom(local: &str) -> bool {
    local.split('.').all(|atom| !atom.is_empty() && atom.chars().all(|c| c.is_ascii_alphanumeric() || "!#$%&'*+-/=?^_`{|}~".contains(c)))
}

/// Two or more letter-digit-hyphen labels of 1 to 63 bytes, no label starting or ending with `-`, no trailing dot.
fn is_domain(domain: &str) -> bool {
    domain.contains('.')
        && domain.split('.').all(|label| {
            (1..=63).contains(&label.len())
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        })
}

/// The same address: the local part exactly, the domain without case.
fn same_address(a: &str, b: &str) -> bool {
    match (a.rsplit_once('@'), b.rsplit_once('@')) {
        (Some((a_local, a_domain)), Some((b_local, b_domain))) => a_local == b_local && a_domain.eq_ignore_ascii_case(b_domain),
        _ => false,
    }
}

fn run_id(id: String) -> Result<String, LiveError> {
    let why = if id.is_empty() {
        "is empty"
    } else if id.len() > MAX_RUN_ID {
        "is longer than 64 bytes"
    } else if !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        "may hold only letters, digits, - and _"
    } else {
        return Ok(id);
    };
    Err(LiveError::Invalid { variable: RUN_ID, why })
}

/// `YYYYMMDDTHHMMSSZ-<process id>`.
fn made_run_id() -> String {
    let now = OffsetDateTime::now_utc();
    format!(
        "{:04}{:02}{:02}T{:02}{:02}{:02}Z-{}",
        now.year(),
        u8::from(now.month()),
        now.day(),
        now.hour(),
        now.minute(),
        now.second(),
        std::process::id()
    )
}
