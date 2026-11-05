// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The desktop app: reads the command line and builds the window's content; `main` opens the window.
//!
//! `inboxcraft --sample` shows the invented sample mailbox, `inboxcraft` alone an empty one (adding an account comes
//! with onboarding), `--version` and `--help` print and leave. Nothing here connects anywhere: the sample's accounts
//! point at `imap.example.org` and their workers are never started.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::ffi::OsString;
use std::io::Write;

use inboxcraft_engine::Session;
use inboxcraft_store::{Memory, StoreConfig};
use inboxcraft_time::Clock;
use inboxcraft_ui_egui::mailbox::Mailbox;
use inboxcraft_ui_egui::{InboxApp, UiError, sample};

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

/// The window's content over `source`, dated by the clocks `clock` makes (one for the session, one for the list).
///
/// # Errors
///
/// A sample or a session that could not be built.
pub fn app(source: Source, clock: impl Fn() -> Box<dyn Clock>) -> Result<InboxApp, UiError> {
    let session = match source {
        Source::Sample => sample::session(clock())?,
        Source::Empty => Session::new(Box::new(Memory::new(StoreConfig::default())), clock())?,
    };
    Ok(InboxApp::new(Mailbox::new(session, clock())))
}

#[cfg(test)]
mod tests;
