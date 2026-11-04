// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `inboxcraft` binary: hands its arguments to [`inboxcraft::run`].

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::process::ExitCode;

fn main() -> ExitCode {
    ExitCode::from(inboxcraft::run(std::env::args_os().skip(1), &mut std::io::stdout(), &mut std::io::stderr()))
}
