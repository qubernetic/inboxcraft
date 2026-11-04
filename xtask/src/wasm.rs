// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `cargo xtask wasm`: the standalone and layer 0 to 5 crates build for `wasm32-unknown-unknown`.
//!
//! The crates come from the workspace and the class table in `layers.rs`; [`NATIVE_ONLY`] crates are left out. One
//! `cargo build` builds them all, its output going to the terminal.

use std::io::Write;

use crate::layers::{self, Class};
use crate::{Context, Error, count, workspace};

/// The target the core crates must build for.
pub(crate) const TARGET: &str = "wasm32-unknown-unknown";

/// Crates of layers 0 to 5 that are native only: `net` owns the sockets.
pub(crate) const NATIVE_ONLY: &[&str] = &["inboxcraft-net"];

/// Builds the wasm crates of `ctx.root`, reports to `out` and returns whether the build passed.
pub(crate) fn gate(ctx: &mut Context, out: &mut dyn Write) -> Result<bool, Error> {
    let members = workspace::members(&ctx.root)?;
    let names: Vec<&str> = members
        .iter()
        .map(|member| member.name.as_str())
        .filter(|name| matches!(layers::class(name), Some(Class::Standalone | Class::Layer(0..=5))))
        .filter(|name| !NATIVE_ONLY.contains(name))
        .collect();
    if names.is_empty() {
        writeln!(out, "wasm: no standalone or layer 0 to 5 crates to build")?;
        return Ok(true);
    }
    let crates = count(names.len(), "crate");
    let mut args = vec!["build", "--target", TARGET];
    for name in &names {
        args.extend(["-p", name]);
    }
    writeln!(out, "wasm: building {crates} for {TARGET}")?;
    out.flush()?;
    let passed = ctx.runner.run(&ctx.cargo, &args, &ctx.root)?;
    if passed {
        writeln!(out, "wasm: {crates} build for {TARGET}")?;
    } else {
        writeln!(out, "wasm: the build for {TARGET} failed ({crates}); see the cargo output above")?;
    }
    Ok(passed)
}

#[cfg(test)]
#[path = "wasm_tests.rs"]
mod tests;
