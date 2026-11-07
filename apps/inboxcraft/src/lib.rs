// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The desktop app: reads the command line and builds the window's content; `main` opens the window.
//!
//! `inboxcraft --sample` shows the invented sample mailbox, `inboxcraft` alone the owner's mailbox, where they add
//! accounts; `--version` and `--help` print and leave. The sample lives in memory and connects nowhere: its accounts
//! point at `imap.example.org`, their workers are never started, and its remote pictures are drawn in code.
//!
//! The owner's mailbox is kept on disk ([`store`]): the SQLite store in the owner's data folder, with its accounts,
//! mail and job queue, so the next start shows what the last one held and runs the jobs it left. The window adds
//! accounts ([`accounts`]): each password goes into the OS credential store, Detect asks the system's name servers
//! and the web, and every account runs on a `net` worker of its own. Besides those, the one request the window makes
//! is for a remote picture the owner allowed, through `net`'s picture client.
//!
//! Both windows keep the owner's appearance choices through the [`Keep`] they are given; `main` gives them the file in
//! the owner's config folder ([`KeepFile::of_owner`]). The file holds no mail and nothing about an account.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use inboxcraft_engine::Session;
use inboxcraft_net::picture::PictureClient;
use inboxcraft_net::{Connector, Credentials, TlsSettings, Trust, WorkerSettings};
use inboxcraft_time::{Clock, SystemClock};
use inboxcraft_ui_egui::fetch::{Fetch, Get, Threaded, Wake};
use inboxcraft_ui_egui::keep::{Keep, KeepFile, NoKeep};
use inboxcraft_ui_egui::mailbox::Mailbox;
use inboxcraft_ui_egui::onboard::{self, Run};
use inboxcraft_ui_egui::save::{Downloads, NoSave, Save};
use inboxcraft_ui_egui::{InboxApp, UiError, sample};

pub mod accounts;
pub mod autostart;
pub mod desktop;
pub mod instance;
pub mod store;
#[cfg(target_os = "linux")]
pub mod tray;

use instance::{Claim, Instance, Request};
pub use store::Mail;

/// Which mailbox the window shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// The invented sample mailbox (`--sample`).
    Sample,
    /// The owner's mailbox, kept on disk; empty until they add an account.
    Owner,
}

/// What to do after reading the command line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Leave with this exit code.
    Exit(u8),
    /// Open the window on this mailbox.
    Window(Source),
    /// Run on the owner's mailbox without showing the window (`--background`), as a start at login does.
    Background,
    /// Ask the running InboxCraft to quit (`--quit`).
    Quit,
    /// Start InboxCraft at login in the background, or no longer (`--autostart`, `--no-autostart`).
    Autostart(bool),
}

/// What `--help` prints.
#[must_use]
pub fn usage() -> String {
    format!(
        "{name} {version}\n\
         \n\
         Usage: {name} [--sample | --background | --quit | --autostart | --no-autostart]\n\
         \n\
         \x20 --sample       show an invented sample mailbox (nothing connects anywhere)\n\
         \x20 --background   keep the mail in step without showing the window\n\
         \x20 --quit         end the running InboxCraft\n\
         \x20 --autostart    start in the background at login\n\
         \x20 --no-autostart no longer start at login\n\
         \x20 -V, --version  print the version\n\
         \x20 -h, --help     print this help\n\
         \n\
         Closing the window keeps the owner's mailbox running in the background; starting\n\
         {name} again brings the window back.\n",
        name = env!("CARGO_PKG_NAME"),
        version = env!("CARGO_PKG_VERSION"),
    )
}

/// Reads the command-line arguments (without the program name) and says what to do next; prints `--version` and
/// `--help` to `out` and refusals to `err`.
///
/// No arguments open the window on the owner's mailbox and `--sample` on the sample mailbox. `--version` (or `-V`) and
/// `--help` (or `-h`) print and leave with 0. Anything else, or more than one argument, is refused with 2. A failed
/// write leaves with 1.
pub fn run(args: impl IntoIterator<Item = OsString>, out: &mut dyn Write, err: &mut dyn Write) -> Step {
    let args: Vec<OsString> = args.into_iter().take(2).collect();
    let name = env!("CARGO_PKG_NAME");
    let printed = |result: std::io::Result<()>| Step::Exit(if result.is_ok() { 0 } else { 1 });
    match args.as_slice() {
        [] => Step::Window(Source::Owner),
        [flag] if flag == "--sample" => Step::Window(Source::Sample),
        [flag] if flag == "--background" => Step::Background,
        [flag] if flag == "--quit" => Step::Quit,
        [flag] if flag == "--autostart" => Step::Autostart(true),
        [flag] if flag == "--no-autostart" => Step::Autostart(false),
        [flag] if flag == "--version" || flag == "-V" => printed(writeln!(out, "{name} {}", env!("CARGO_PKG_VERSION"))),
        [flag] if flag == "--help" || flag == "-h" => printed(write!(out, "{}", usage())),
        _ => {
            let shown: Vec<String> = args.iter().map(|arg| arg.to_string_lossy().chars().filter(|c| !c.is_control()).take(80).collect()).collect();
            // A refusal either way; a closed stderr leaves nothing else to report on.
            let _ = writeln!(err, "{name}: unknown arguments: {}; try --help", shown.join(" "));
            Step::Exit(2)
        }
    }
}

/// How long one remote picture may take.
pub const PICTURE_TIMEOUT: Duration = Duration::from_secs(20);

/// The window's content over `mail` ([`store::mail`]), dated by the clocks `clock` makes (one for the session, one
/// for the list); `wake` draws the window again when a remote picture arrived; the appearance choices are kept in
/// `keep`.
///
/// The sample draws its remote pictures in code and connects nowhere. On a store, the session starts at once: the
/// jobs the last run left go out again, then every held account syncs its folder list. A remote picture the owner
/// allowed is fetched by `net`'s picture client (HTTPS only, a fixed user agent, no cookies, no referrer, public
/// addresses only) on threads of its own, and the window adds accounts: passwords into the OS credential store,
/// Detect on a thread, one `net` worker per account. Attachments are saved into the owner's downloads folder, never over a file.
///
/// # Errors
///
/// A sample or a session that could not be built.
pub fn app(mail: Mail, clock: impl Fn() -> Box<dyn Clock>, wake: Option<Wake>, keep: Box<dyn Keep>) -> Result<InboxApp, UiError> {
    let save: Box<dyn Save> = match Downloads::of_owner() {
        Some(downloads) => Box::new(downloads),
        None => Box::new(NoSave),
    };
    let owned = match mail {
        Mail::Sample => {
            let mailbox = Mailbox::new(sample::session(clock())?, clock());
            return Ok(InboxApp::with_outside(mailbox, Box::new(sample::Pictures::default()), save).with_keep(keep));
        }
        Mail::Store(owned) => owned,
    };
    let keychain = Arc::new(accounts::OsKeychain::new(&owned.id));
    let tls = TlsSettings::new(Trust::Platform, Arc::new(SystemClock::new()));
    let client = PictureClient::new(tls.clone(), PICTURE_TIMEOUT);
    let get: Get = Arc::new(move |url: &str| client.fetch(url).map(|picture| picture.bytes).map_err(|error| error.to_string()));
    let fetch: Box<dyn Fetch> = Box::new(Threaded::new(get, wake.clone()));
    let mut session = Session::new(owned.store, clock())?;
    session.start();
    let detecting = tls.clone();
    let run: Run = Arc::new(move |address: &str, ispdb: bool| accounts::detect(&detecting, address, ispdb));
    let detect = Box::new(onboard::Threaded::new(run, wake.clone()));
    let workers = Box::new(accounts::NetWorkers::new(WorkerSettings::new(Connector::new(tls)), Arc::clone(&keychain) as Arc<dyn Credentials>, wake));
    Ok(InboxApp::with_outside(Mailbox::new(session, clock()), fetch, save).with_accounts(keychain, detect, workers).with_keep(keep))
}

/// The folder a start on the owner's mailbox claims ([`instance`]): the one that holds the store.
#[must_use]
pub fn instance_dir() -> Option<PathBuf> {
    store::store_dir().and_then(|dir| dir.parent().map(Path::to_path_buf))
}

/// A start on the owner's mailbox claims `dir`: the first start gets the [`Instance`] and opens the store; a later one
/// brings the running window forward (or, starting `background` at login, leaves the running one alone) and leaves
/// with the exit code in `Err`. Refusals go to `err`.
///
/// # Errors
///
/// The exit code to leave with: 0 when the running start was reached or left alone, 1 when it was not or `dir` could
/// not be claimed.
pub fn arrive(dir: &Path, background: bool, err: &mut dyn Write) -> Result<Instance, u8> {
    arrive_with(dir, background, instance::PATIENCE, err)
}

/// [`arrive`], trying for `patience` to reach the running start.
///
/// # Errors
///
/// As [`arrive`].
pub fn arrive_with(dir: &Path, background: bool, patience: Duration, err: &mut dyn Write) -> Result<Instance, u8> {
    let name = env!("CARGO_PKG_NAME");
    match instance::claim(dir) {
        Ok(Claim::First(instance)) => Ok(instance),
        Ok(Claim::Running(_)) if background => Err(0),
        Ok(Claim::Running(running)) => match running.with_patience(patience).send(Request::Show) {
            Ok(()) => Err(0),
            Err(error) => {
                let _ = writeln!(err, "{name}: {error}");
                Err(1)
            }
        },
        Err(error) => {
            let _ = writeln!(err, "{name}: {error}");
            Err(1)
        }
    }
}

/// `--quit`: asks the start that holds `dir` to end. 0 when it was asked, 1 when none runs or it was not reached.
pub fn quit(dir: &Path, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let name = env!("CARGO_PKG_NAME");
    let refused = |err: &mut dyn Write, error: &dyn std::fmt::Display| {
        let _ = writeln!(err, "{name}: {error}");
        1
    };
    match instance::claim(dir) {
        Ok(Claim::First(_)) => refused(err, &"InboxCraft is not running"),
        Ok(Claim::Running(running)) => match running.send(Request::Quit) {
            Ok(()) => {
                let _ = writeln!(out, "{name}: asked InboxCraft to quit");
                0
            }
            Err(error) => refused(err, &error),
        },
        Err(error) => refused(err, &error),
    }
}

/// `--autostart` and `--no-autostart`: [`autostart::choose`] in `dir` (the `autostart` folder of the config folder),
/// through `portal` in the Flatpak. 0 when done, 1 when not.
pub fn set_autostart(
    on: bool,
    dir: Option<&Path>,
    program: &Path,
    portal: Option<autostart::Ask<'_>>,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    let name = env!("CARGO_PKG_NAME");
    let chosen = dir
        .ok_or_else(|| "there is no folder for the autostart entry: HOME (or XDG_CONFIG_HOME) is not set".to_owned())
        .and_then(|dir| autostart::choose(on, dir, program, portal));
    match chosen {
        Ok(said) => {
            let _ = writeln!(out, "{name}: {said}");
            0
        }
        Err(error) => {
            let _ = writeln!(err, "{name}: {error}");
            1
        }
    }
}

/// Where the owner's appearance choices are kept: the file in their config folder, or nowhere without one.
#[must_use]
pub fn owner_keep() -> Box<dyn Keep> {
    match KeepFile::of_owner() {
        Some(file) => Box::new(file),
        None => Box::new(NoKeep),
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[cfg(target_os = "linux")]
mod bus_tests;
