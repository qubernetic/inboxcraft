// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The workspace gates behind `cargo xtask`.
//!
//! Every gate is a library function with tests; `main.rs` only hands over the arguments. `cargo xtask help` lists the
//! commands.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod cli;
mod config;
mod error;
mod spdx;
mod test_files;
mod walk;

#[cfg(test)]
mod test_support;

use std::path::PathBuf;

pub use cli::{default_root, run};
pub use error::Error;

/// Exit code: every gate passed.
pub const OK: u8 = 0;
/// Exit code: a gate ran and failed.
pub const FAILED: u8 = 1;
/// Exit code: bad arguments, or a gate could not run at all.
pub const USAGE: u8 = 2;

/// Where xtask runs.
#[derive(Debug)]
pub struct Context {
    /// The workspace root; `--root` replaces it.
    pub root: PathBuf,
}

impl Context {
    /// The context `cargo xtask` runs in: this workspace.
    #[must_use]
    pub fn from_env() -> Self {
        Self { root: default_root() }
    }
}
