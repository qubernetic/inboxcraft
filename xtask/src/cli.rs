// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Argument parsing and dispatch.

use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::{Context, Error, FAILED, OK, USAGE, assets, ci, layers, spdx, stats, test_files, wasm};

const HELP: &str = "\
usage: cargo xtask <command> [--root <dir>]

commands:
  ci            every gate: fmt, clippy -D warnings, spdx, test-files, assets, layers, nextest (ci profile), doctests, wasm, deny, stats
  assets        every asset file has a row in ATTRIBUTION.md
  layers        the layering rules between the crates (xtask/src/layers.rs)
  spdx [--fix]  every Rust file starts with the SPDX lines; --fix adds the missing ones
  stats         Rust files, code lines and test lines per crate
  test-files    no inline `mod tests { ... }`: tests live in files of their own
  wasm          build the standalone and layer 0 to 5 crates for wasm32-unknown-unknown (not net)
  help          print this text

options:
  --root <dir>  the workspace to check (default: the one xtask was built from)

exit codes: 0 passed, 1 a gate failed, 2 usage or setup error
";

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Command {
    Assets,
    Ci,
    Help,
    Layers,
    Spdx { fix: bool },
    Stats,
    TestFiles,
    Wasm,
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
        Some("assets") => Command::Assets,
        Some("ci") => Command::Ci,
        Some("layers") => Command::Layers,
        Some("spdx") => Command::Spdx { fix: false },
        Some("stats") => Command::Stats,
        Some("test-files") => Command::TestFiles,
        Some("wasm") => Command::Wasm,
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
                Command::Assets | Command::Ci | Command::Help | Command::Layers | Command::Stats | Command::TestFiles | Command::Wasm => {
                    return Err("`--fix` only goes with `spdx`".to_owned());
                }
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

fn dispatch(command: &Command, ctx: &mut Context, out: &mut dyn Write) -> Result<bool, Error> {
    match command {
        Command::Assets => assets::gate(&ctx.root, out),
        Command::Ci => ci::gate(ctx, out),
        Command::Help => {
            out.write_all(HELP.as_bytes())?;
            Ok(true)
        }
        Command::Layers => layers::gate(&ctx.root, out),
        Command::Spdx { fix } => spdx::gate(&ctx.root, *fix, out),
        Command::Stats => stats::gate(&ctx.root, out),
        Command::TestFiles => test_files::gate(&ctx.root, out),
        Command::Wasm => wasm::gate(ctx, out),
    }
}

#[cfg(test)]
#[path = "cli_tests.rs"]
mod tests;
