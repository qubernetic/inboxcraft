// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The desktop app: reads the command line and builds the window's content; `main` opens the window.
//!
//! `inboxcraft --sample` shows the invented sample mailbox, `inboxcraft` alone an empty one where the owner adds
//! accounts, `--version` and `--help` print and leave. The sample connects nowhere: its accounts point at
//! `imap.example.org`, their workers are never started, and its remote pictures are drawn in code.
//!
//! The empty window adds accounts ([`accounts`]): each password goes into the OS credential store, Detect asks the
//! system's name servers and the web, and every account runs on a `net` worker of its own. Besides those, the one
//! request the window makes is for a remote picture the owner allowed, through `net`'s picture client.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::ffi::OsString;
use std::io::Write;
use std::sync::Arc;
use std::time::Duration;

use inboxcraft_engine::Session;
use inboxcraft_net::picture::PictureClient;
use inboxcraft_net::{Connector, TlsSettings, Trust, WorkerSettings};
use inboxcraft_store::{Memory, StoreConfig};
use inboxcraft_time::{Clock, SystemClock};
use inboxcraft_ui_egui::fetch::{Fetch, Get, Threaded, Wake};
use inboxcraft_ui_egui::mailbox::Mailbox;
use inboxcraft_ui_egui::onboard::{self, Run};
use inboxcraft_ui_egui::save::{Downloads, NoSave, Save};
use inboxcraft_ui_egui::{InboxApp, UiError, sample};

pub mod accounts;

/// Which mailbox the window shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// The invented sample mailbox (`--sample`).
    Sample,
    /// An empty mailbox: no accounts yet.
    Empty,
}

/// What to do after reading the command line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Leave with this exit code.
    Exit(u8),
    /// Open the window on this mailbox.
    Window(Source),
}

/// What `--help` prints.
#[must_use]
pub fn usage() -> String {
    format!(
        "{name} {version}\n\
         \n\
         Usage: {name} [--sample]\n\
         \n\
         \x20 --sample      show an invented sample mailbox (nothing connects anywhere)\n\
         \x20 -V, --version print the version\n\
         \x20 -h, --help    print this help\n",
        name = env!("CARGO_PKG_NAME"),
        version = env!("CARGO_PKG_VERSION"),
    )
}

/// Reads the command-line arguments (without the program name) and says what to do next; prints `--version` and
/// `--help` to `out` and refusals to `err`.
///
/// No arguments open the window on an empty mailbox and `--sample` on the sample mailbox. `--version` (or `-V`) and
/// `--help` (or `-h`) print and leave with 0. Anything else, or more than one argument, is refused with 2. A failed
/// write leaves with 1.
pub fn run(args: impl IntoIterator<Item = OsString>, out: &mut dyn Write, err: &mut dyn Write) -> Step {
    let args: Vec<OsString> = args.into_iter().take(2).collect();
    let name = env!("CARGO_PKG_NAME");
    let printed = |result: std::io::Result<()>| Step::Exit(if result.is_ok() { 0 } else { 1 });
    match args.as_slice() {
        [] => Step::Window(Source::Empty),
        [flag] if flag == "--sample" => Step::Window(Source::Sample),
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

/// The window's content over `source`, dated by the clocks `clock` makes (one for the session, one for the list);
/// `wake` draws the window again when a remote picture arrived.
///
/// The sample draws its remote pictures in code and connects nowhere. Otherwise a remote picture the owner allowed
/// is fetched by `net`'s picture client (HTTPS only, a fixed user agent, no cookies, no referrer, public addresses
/// only) on threads of its own, and the window adds accounts: passwords into the OS credential store, Detect on a
/// thread, one `net` worker per account. Attachments are saved into the owner's downloads folder, never over a file.
///
/// # Errors
///
/// A sample or a session that could not be built.
pub fn app(source: Source, clock: impl Fn() -> Box<dyn Clock>, wake: Option<Wake>) -> Result<InboxApp, UiError> {
    let save: Box<dyn Save> = match Downloads::of_owner() {
        Some(downloads) => Box::new(downloads),
        None => Box::new(NoSave),
    };
    if source == Source::Sample {
        let mailbox = Mailbox::new(sample::session(clock())?, clock());
        return Ok(InboxApp::with_outside(mailbox, Box::new(sample::Pictures::default()), save));
    }
    let tls = TlsSettings::new(Trust::Platform, Arc::new(SystemClock::new()));
    let client = PictureClient::new(tls.clone(), PICTURE_TIMEOUT);
    let get: Get = Arc::new(move |url: &str| client.fetch(url).map(|picture| picture.bytes).map_err(|error| error.to_string()));
    let fetch: Box<dyn Fetch> = Box::new(Threaded::new(get, wake.clone()));
    let session = Session::new(Box::new(Memory::new(StoreConfig::default())), clock())?;
    let detecting = tls.clone();
    let run: Run = Arc::new(move |address: &str, ispdb: bool| accounts::detect(&detecting, address, ispdb));
    let detect = Box::new(onboard::Threaded::new(run, wake.clone()));
    let workers = Box::new(accounts::NetWorkers::new(WorkerSettings::new(Connector::new(tls)), Arc::new(accounts::OsKeychain), wake));
    Ok(InboxApp::with_outside(Mailbox::new(session, clock()), fetch, save).with_accounts(Arc::new(accounts::OsKeychain), detect, workers))
}

#[cfg(test)]
mod tests;
