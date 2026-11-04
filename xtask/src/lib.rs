// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The workspace gates behind `cargo xtask`.
//!
//! Every gate is a library function with tests; `main.rs` only hands over the arguments. `cargo xtask help` lists the
//! commands.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod ci;
mod cli;
mod config;
mod error;
mod layers;
mod spdx;
mod test_files;
mod walk;
mod wasm;
mod workspace;

#[cfg(test)]
mod test_support;

#[cfg(test)]
mod tests;

use std::ffi::OsString;
use std::path::PathBuf;

pub use ci::{ProcessRunner, Runner};
pub use cli::{default_root, run};
pub use error::Error;

/// Exit code: every gate passed.
pub const OK: u8 = 0;
/// Exit code: a gate ran and failed.
pub const FAILED: u8 = 1;
/// Exit code: bad arguments, or a gate could not run at all.
pub const USAGE: u8 = 2;

/// `n` and `noun`, the noun in the plural unless `n` is 1: `1 crate`, `3 crates`.
pub(crate) fn count(n: usize, noun: &str) -> String {
    if n == 1 { format!("1 {noun}") } else { format!("{n} {noun}s") }
}

/// Where xtask runs and how it starts the cargo tools.
pub struct Context {
    /// The workspace root; `--root` replaces it.
    pub root: PathBuf,
    /// The `cargo` to run the tools with.
    pub cargo: OsString,
    /// Starts the tools.
    pub runner: Box<dyn Runner>,
}

impl Context {
    /// The context `cargo xtask` runs in: this workspace, the `cargo` that started xtask (`$CARGO`, else `cargo`)
    /// and real processes.
    #[must_use]
    pub fn from_env() -> Self {
        Self { root: default_root(), cargo: cargo_or_default(std::env::var_os("CARGO")), runner: Box::new(ProcessRunner) }
    }
}

/// `$CARGO` when it is set, else `cargo` from the `PATH`.
fn cargo_or_default(var: Option<OsString>) -> OsString {
    match var {
        Some(cargo) => cargo,
        None => OsString::from("cargo"),
    }
}
