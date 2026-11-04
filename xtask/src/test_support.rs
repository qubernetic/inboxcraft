// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Helpers shared by the unit tests.

use std::ffi::OsString;
use std::io::{self, Write};
use std::path::PathBuf;

use crate::Context;

/// A writer that always fails, like a closed pipe.
pub struct Broken;

impl Write for Broken {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("closed"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::other("closed"))
    }
}

#[test]
fn broken_fails_every_call() {
    Broken.write_all(b"x").unwrap_err();
    Broken.flush().unwrap_err();
}

/// A context rooted at `root`.
pub fn context(root: PathBuf) -> Context {
    Context { root }
}

/// Runs xtask with `args` against this workspace and returns the exit code, stdout and stderr.
pub fn run_xtask(args: &[&str]) -> (u8, String, String) {
    run_in(context(crate::default_root()), args)
}

/// Runs xtask with `args` in `ctx` and returns the exit code, stdout and stderr.
pub fn run_in(mut ctx: Context, args: &[&str]) -> (u8, String, String) {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = crate::run(args.iter().map(OsString::from), &mut ctx, &mut out, &mut err);
    (code, String::from_utf8_lossy(&out).into_owned(), String::from_utf8_lossy(&err).into_owned())
}
