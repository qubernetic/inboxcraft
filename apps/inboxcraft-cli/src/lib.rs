// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The command-line client.
//!
//! Nothing is built yet: the binary answers `--version` and refuses everything else, so no one mistakes the skeleton
//! for a mail program.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::ffi::OsString;
use std::io::Write;

/// Runs the app with its command-line arguments (without the program name) and returns the process exit code.
///
/// `--version` (or `-V`) as the only argument prints the name and version and returns 0. Anything else, including no
/// arguments, says that nothing is built yet and returns 1. A failed write also returns 1.
pub fn run(args: impl IntoIterator<Item = OsString>, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let args: Vec<OsString> = args.into_iter().take(2).collect();
    let name = env!("CARGO_PKG_NAME");
    let version = env!("CARGO_PKG_VERSION");
    if let [flag] = args.as_slice()
        && (flag == "--version" || flag == "-V")
    {
        return if writeln!(out, "{name} {version}").is_ok() { 0 } else { 1 };
    }
    // Both outcomes are failures; a closed stderr leaves nothing else to report on.
    let _ = writeln!(err, "{name} {version}: nothing is built yet; only --version works");
    1
}

#[cfg(test)]
mod tests;
