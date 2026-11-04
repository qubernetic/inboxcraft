// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Argument parsing and dispatch.

use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::{Context, Error, FAILED, OK, USAGE, spdx};

const HELP: &str = "\
usage: cargo xtask <command> [--root <dir>]

commands:
  spdx [--fix]  every Rust file starts with the SPDX lines; --fix adds the missing ones
  help          print this text

options:
  --root <dir>  the workspace to check (default: the one xtask was built from)

exit codes: 0 passed, 1 a gate failed, 2 usage or setup error
";

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Command {
    Help,
    Spdx { fix: bool },
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Invocation {
    pub(crate) command: Command,
    pub(crate) root: Option<PathBuf>,
}

/// The workspace xtask was built from: the parent of the `xtask` directory.
#[must_use]
pub fn default_root() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().unwrap_or(manifest).to_path_buf()
}

pub(crate) fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Invocation, String> {
    let mut args = args.into_iter().map(|arg| arg.into_string().map_err(|_| "arguments must be UTF-8".to_owned()));
    let command = match args.next().transpose()?.as_deref() {
        None | Some("help" | "--help" | "-h") => Command::Help,
        Some("spdx") => Command::Spdx { fix: false },
        Some(other) => return Err(format!("unknown command `{other}`")),
    };
    let mut invocation = Invocation { command, root: None };
    while let Some(arg) = args.next().transpose()? {
        match arg.as_str() {
            "--root" => {
                let dir = args.next().transpose()?.ok_or_else(|| "`--root` needs a directory".to_owned())?;
                invocation.root = Some(PathBuf::from(dir));
            }
            "--fix" => match &mut invocation.command {
                Command::Spdx { fix } => *fix = true,
                Command::Help => return Err("`--fix` only goes with `spdx`".to_owned()),
            },
            other => return Err(format!("unknown option `{other}`")),
        }
    }
    Ok(invocation)
}

/// Runs xtask with its arguments (without the program name) and returns the process exit code.
pub fn run(args: impl IntoIterator<Item = OsString>, ctx: &mut Context, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let invocation = match parse(args) {
        Ok(invocation) => invocation,
        Err(message) => {
            // A closed stderr leaves nothing else to report on; the exit code still says it.
            let _ = writeln!(err, "xtask: {message}; see `cargo xtask help`");
            return USAGE;
        }
    };
    if let Some(root) = invocation.root {
        ctx.root = root;
    }
    match dispatch(&invocation.command, ctx, out) {
        Ok(true) => OK,
        Ok(false) | Err(Error::Output(_)) => FAILED,
        Err(error) => {
            let _ = writeln!(err, "xtask: {error}");
            USAGE
        }
    }
}

fn dispatch(command: &Command, ctx: &Context, out: &mut dyn Write) -> Result<bool, Error> {
    match command {
        Command::Help => {
            out.write_all(HELP.as_bytes())?;
            Ok(true)
        }
        Command::Spdx { fix } => spdx::gate(&ctx.root, *fix, out),
    }
}

#[cfg(test)]
#[path = "cli_tests.rs"]
mod tests;
