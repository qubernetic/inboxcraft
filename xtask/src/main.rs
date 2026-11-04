// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `cargo xtask`: hands its arguments to [`xtask::run`].

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::process::ExitCode;

fn main() -> ExitCode {
    let mut ctx = xtask::Context::from_env();
    ExitCode::from(xtask::run(std::env::args_os().skip(1), &mut ctx, &mut std::io::stdout(), &mut std::io::stderr()))
}
