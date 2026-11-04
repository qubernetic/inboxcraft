// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Helpers shared by the unit tests.

use std::ffi::{OsStr, OsString};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::{Context, Error, Runner};

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

/// A writer that takes `left` bytes, then fails.
pub struct BrokenAfter {
    left: usize,
}

impl BrokenAfter {
    pub fn new(left: usize) -> Self {
        Self { left }
    }
}

impl Write for BrokenAfter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.len() > self.left {
            return Err(io::Error::other("closed"));
        }
        self.left -= buf.len();
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn broken_after_fails_once_full() {
    let mut out = BrokenAfter::new(2);
    out.write_all(b"ab").unwrap();
    out.flush().unwrap();
    out.write_all(b"c").unwrap_err();
}

/// Writes `text` to `root/rel`, creating the directories on the way.
pub fn write(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// A package manifest called `name`, followed by `rest` (dependency tables and the like).
pub fn manifest(name: &str, rest: &str) -> String {
    format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n{rest}")
}

#[test]
fn manifest_writes_a_package_table() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a/b/Cargo.toml", &manifest("x", "[dependencies]\n"));
    let text = std::fs::read_to_string(dir.path().join("a/b/Cargo.toml")).unwrap();
    assert!(text.starts_with("[package]\nname = \"x\"\n") && text.ends_with("[dependencies]\n"), "{text}");
}

/// A runner that starts nothing and passes every tool.
pub struct Passing;

impl Runner for Passing {
    fn run(&mut self, _: &OsStr, _: &[&str], _: &Path) -> Result<bool, Error> {
        Ok(true)
    }
}

#[test]
fn passing_passes_every_tool() {
    let (code, out, _) = run_xtask(&["ci"]);
    assert_eq!(code, crate::OK, "{out}");
}

/// A context rooted at `root` that starts no processes.
pub fn context(root: PathBuf) -> Context {
    Context { root, cargo: OsString::from("cargo"), runner: Box::new(Passing) }
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
