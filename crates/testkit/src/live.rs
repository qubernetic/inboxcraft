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
//! - The two accounts are called `live-imap` and `live-google` in output; their addresses, user names and secrets
//!   never appear in a [`fmt::Debug`] or [`fmt::Display`] of anything here. A [`Secret`] prints `[redacted]`.
//! - [`LiveConfig::check_recipient`] is the recipient allow-list: exactly the two addresses given at run time. A
//!   live test that sends mail checks every recipient with it first.
//! - [`LiveConfig::run_folder`] names the per-run folder `InboxCraft-Test/<run id>`, and [`RunFolder`] creates it and
//!   removes it again, also when the test fails.
//! - [`LiveConfig::leaked_secret`] scans a log or panic output for the secret values.
//!
//! | Variable | Meaning |
//! |---|---|
//! | `INBOXCRAFT_LIVE_{IMAP,GOOGLE}_ADDRESS` | the account's mail address, one of the two allowed recipients |
//! | `INBOXCRAFT_LIVE_{IMAP,GOOGLE}_HOST` | the server's host name |
//! | `INBOXCRAFT_LIVE_{IMAP,GOOGLE}_USER` | the login name |
//! | `INBOXCRAFT_LIVE_{IMAP,GOOGLE}_SECRET` | the password or token |
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

const GOOGLE: AccountVars = AccountVars {
    name: "live-google",
    address: "INBOXCRAFT_LIVE_GOOGLE_ADDRESS",
    host: "INBOXCRAFT_LIVE_GOOGLE_HOST",
    user: "INBOXCRAFT_LIVE_GOOGLE_USER",
    secret: "INBOXCRAFT_LIVE_GOOGLE_SECRET",
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
}

impl fmt::Display for LiveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Incomplete { missing } => {
                write!(f, "live test configuration is incomplete, missing: {}", missing.join(", "))
            }
            Self::Invalid { variable, why } => write!(f, "{variable} {why}"),
            Self::SameAddress => f.write_str("live-imap and live-google have the same address"),
            Self::RecipientRefused => f.write_str("recipient refused: not one of the two live test accounts"),
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
    /// The name in output: `live-imap` or `live-google`.
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
        let names = IMAP.all().into_iter().chain(GOOGLE.all());
        let missing: Vec<&'static str> = names.clone().filter(|name| get(name).is_none()).collect();
        if missing.len() == names.count() {
            return Ok(Self::Skip(SkipReason::NoCredentials));
        }
        if !missing.is_empty() {
            return Err(LiveError::Incomplete { missing });
        }
        let imap = account(&IMAP, &get)?;
        let google = account(&GOOGLE, &get)?;
        if same_address(&imap.address, &google.address) {
            return Err(LiveError::SameAddress);
        }
        let run_id = match get(RUN_ID) {
            Some(id) => run_id(id)?,
            None => made_run_id(),
        };
        Ok(Self::Run(Box::new(LiveConfig { imap, google, run_id })))
    }
}

/// The configuration of a live run: the two accounts and the run id.
#[derive(Debug, Clone)]
pub struct LiveConfig {
    imap: Account,
    google: Account,
    run_id: String,
}

impl LiveConfig {
    /// The plain IMAP account, `live-imap`.
    #[must_use]
    pub fn imap(&self) -> &Account {
        &self.imap
    }

    /// The Google account, `live-google`.
    #[must_use]
    pub fn google(&self) -> &Account {
        &self.google
    }

    /// Both accounts, `live-imap` first.
    #[must_use]
    pub fn accounts(&self) -> [&Account; 2] {
        [&self.imap, &self.google]
    }

    /// The run id: letters, digits, `-` and `_`.
    #[must_use]
    pub fn run_id(&self) -> &str {
        &self.run_id
    }

    /// The per-run folder, `InboxCraft-Test<delimiter><run id>`, with the server's hierarchy delimiter.
    #[must_use]
    pub fn run_folder(&self, delimiter: char) -> String {
        format!("{RUN_ROOT}{delimiter}{}", self.run_id)
    }

    /// The recipient allow-list: admits exactly the two run-time addresses, the domain compared without case.
    ///
    /// # Errors
    ///
    /// [`LiveError::RecipientRefused`] for any other address; the error does not repeat it.
    pub fn check_recipient(&self, recipient: &str) -> Result<(), LiveError> {
        if self.accounts().iter().any(|account| same_address(&account.address, recipient)) { Ok(()) } else { Err(LiveError::RecipientRefused) }
    }

    /// The account whose secret appears in `text` (a log or panic output), or `None` when no secret does.
    #[must_use]
    pub fn leaked_secret(&self, text: &str) -> Option<&'static str> {
        self.accounts().into_iter().find(|account| text.contains(account.secret.expose())).map(|account| account.name)
    }
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

/// The per-run folder: created by [`RunFolder::create`], deleted by [`RunFolder::remove`] or, when a failing test
/// never gets there, on drop.
pub struct RunFolder<'a, F: Folders> {
    folders: &'a mut F,
    name: String,
    open: bool,
}

impl<'a, F: Folders> RunFolder<'a, F> {
    /// Creates the folder `name`, usually [`LiveConfig::run_folder`].
    ///
    /// # Errors
    ///
    /// The error of [`Folders::create`]; nothing is left to remove then.
    pub fn create(folders: &'a mut F, name: String) -> Result<Self, F::Error> {
        folders.create(&name)?;
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

/// One `@` with something on both sides, and no white space.
fn is_address(text: &str) -> bool {
    match text.split_once('@') {
        Some((local, domain)) => !local.is_empty() && !domain.is_empty() && !domain.contains('@') && !text.chars().any(char::is_whitespace),
        None => false,
    }
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
