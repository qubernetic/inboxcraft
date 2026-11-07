// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `cargo xtask ci`: every gate, in the order CI runs them.
//!
//! Every gate runs even after one fails, so one run shows everything that is wrong; the summary names the failed
//! gates. The cargo tools write straight to the terminal, the in-process gates write their report.

use std::ffi::OsStr;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::{Context, Error, assets, changes, cov, flatpak, layers, spdx, stats, test_files, wasm};

/// Runs an external tool.
pub trait Runner {
    /// Runs `program` with `args` in `dir`, its output going to the terminal, and returns whether it exited with 0.
    ///
    /// # Errors
    ///
    /// [`Error::Spawn`] when the program cannot be started.
    fn run(&mut self, program: &OsStr, args: &[&str], dir: &Path) -> Result<bool, Error>;

    /// [`Runner::run`] with the variables of `env` set for the program. A runner that starts no process ignores them.
    ///
    /// # Errors
    ///
    /// [`Error::Spawn`] when the program cannot be started.
    fn run_with_env(&mut self, program: &OsStr, args: &[&str], env: &[(&str, &str)], dir: &Path) -> Result<bool, Error> {
        let _ = env;
        self.run(program, args, dir)
    }

    /// Runs `program` with `args` in `dir`, its standard error going to the terminal, and returns its standard
    /// output, or `None` when it did not exit with 0. A runner that captures no output answers `None`, a failed call.
    ///
    /// # Errors
    ///
    /// [`Error::Spawn`] when the program cannot be started.
    fn output(&mut self, program: &OsStr, args: &[&str], dir: &Path) -> Result<Option<Vec<u8>>, Error> {
        let _ = (program, args, dir);
        Ok(None)
    }
}

/// The [`Runner`] that starts real processes.
#[derive(Debug, Clone, Copy, Default)]
pub struct ProcessRunner;

impl Runner for ProcessRunner {
    fn run(&mut self, program: &OsStr, args: &[&str], dir: &Path) -> Result<bool, Error> {
        self.run_with_env(program, args, &[], dir)
    }

    fn run_with_env(&mut self, program: &OsStr, args: &[&str], env: &[(&str, &str)], dir: &Path) -> Result<bool, Error> {
        let status = Command::new(program)
            .args(args)
            .envs(env.iter().copied())
            .current_dir(dir)
            .status()
            .map_err(|source| Error::Spawn { program: program.to_string_lossy().into_owned(), source })?;
        Ok(status.success())
    }

    fn output(&mut self, program: &OsStr, args: &[&str], dir: &Path) -> Result<Option<Vec<u8>>, Error> {
        let output = Command::new(program)
            .args(args)
            .current_dir(dir)
            .stderr(Stdio::inherit())
            .output()
            .map_err(|source| Error::Spawn { program: program.to_string_lossy().into_owned(), source })?;
        Ok(output.status.success().then_some(output.stdout))
    }
}

pub(crate) enum Gate {
    Assets,
    /// `cargo` with these arguments.
    Cargo(&'static [&'static str]),
    /// `changes`, without `--fold`.
    Changes,
    /// `cov --check`.
    Cov,
    /// `flatpak-sources-check`.
    FlatpakSources,
    Layers,
    Spdx,
    Stats,
    TestFiles,
    Wasm,
}

/// The gates of `cargo xtask ci`, cheap ones first; `stats` only reports. `cov` is the one test run: the tests run
/// only under coverage.
pub(crate) const GATES: &[(&str, Gate)] = &[
    ("fmt", Gate::Cargo(&["fmt", "--all", "--", "--check"])),
    ("clippy", Gate::Cargo(&["clippy", "--workspace", "--all-targets", "--", "-D", "warnings"])),
    ("spdx", Gate::Spdx),
    ("test-files", Gate::TestFiles),
    ("assets", Gate::Assets),
    ("changes", Gate::Changes),
    ("layers", Gate::Layers),
    ("flatpak-sources-check", Gate::FlatpakSources),
    ("doctests", Gate::Cargo(&["test", "--workspace", "--doc"])),
    ("cov", Gate::Cov),
    ("wasm", Gate::Wasm),
    ("deny", Gate::Cargo(&["deny", "check"])),
    ("stats", Gate::Stats),
];

/// Runs every gate, reports to `out` and returns whether all passed.
pub(crate) fn gate(ctx: &mut Context, out: &mut dyn Write) -> Result<bool, Error> {
    let mut failed = Vec::new();
    for (name, gate) in GATES {
        writeln!(out, "ci: {name}")?;
        out.flush()?;
        let result = match gate {
            Gate::Cargo(args) => ctx.runner.run(&ctx.cargo, args, &ctx.root),
            Gate::Assets => assets::gate(&ctx.root, out),
            Gate::Changes => changes::gate(&ctx.root, false, &changes::today(), out),
            Gate::Cov => cov::gate(ctx, true, out),
            Gate::FlatpakSources => flatpak::gate(&ctx.root, true, out),
            Gate::Layers => layers::gate(&ctx.root, out),
            Gate::Spdx => spdx::gate(&ctx.root, false, out),
            Gate::Stats => stats::gate(&ctx.root, out),
            Gate::TestFiles => test_files::gate(&ctx.root, out),
            Gate::Wasm => wasm::gate(ctx, out),
        };
        match result {
            Ok(true) => {}
            Ok(false) => failed.push(*name),
            Err(Error::Output(source)) => return Err(Error::Output(source)),
            Err(error) => {
                writeln!(out, "ci: {name}: {error}")?;
                failed.push(*name);
            }
        }
    }
    if failed.is_empty() {
        writeln!(out, "ci: all {} gates passed", GATES.len())?;
    } else {
        writeln!(out, "ci: {} of {} gates failed: {}", failed.len(), GATES.len(), failed.join(", "))?;
    }
    Ok(failed.is_empty())
}

#[cfg(test)]
#[path = "ci_tests.rs"]
mod tests;
