// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `cargo xtask wasm` with the real cargo on a one-crate workspace: it passes, then fails once the crate stops
//! building for wasm.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

/// Runs `xtask wasm` on `root` with this toolchain and none of the coverage instrumentation of the outer run.
fn wasm(root: &Path) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_xtask"));
    command.args(["wasm", "--root"]).arg(root).env("CARGO", env!("CARGO")).env("CARGO_TARGET_DIR", root.join("target"));
    for var in ["RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_BUILD_RUSTFLAGS", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER"] {
        command.env_remove(var);
    }
    command.output().unwrap()
}

#[test]
fn wasm_fails_on_a_crate_that_does_not_build_for_wasm() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    fs::copy(workspace.join("rust-toolchain.toml"), dir.path().join("rust-toolchain.toml")).unwrap();
    fs::write(dir.path().join("Cargo.toml"), "[workspace]\nmembers = [\"crates/*\"]\nresolver = \"3\"\n").unwrap();
    let krate = dir.path().join("crates/time");
    fs::create_dir_all(krate.join("src")).unwrap();
    fs::write(krate.join("Cargo.toml"), "[package]\nname = \"inboxcraft-time\"\nversion = \"0.1.0\"\nedition = \"2024\"\n").unwrap();
    fs::write(krate.join("src/lib.rs"), "pub fn now() {}\n").unwrap();

    let output = wasm(dir.path());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(0), "{stdout}{}", String::from_utf8_lossy(&output.stderr));
    assert!(stdout.contains("wasm: 1 crate build for wasm32-unknown-unknown"), "{stdout}");

    fs::write(krate.join("src/lib.rs"), "#[cfg(target_arch = \"wasm32\")]\ncompile_error!(\"native only\");\n").unwrap();
    let output = wasm(dir.path());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(1), "{stdout}");
    assert!(stdout.contains("wasm: the build for wasm32-unknown-unknown failed (1 crate)"), "{stdout}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("native only"));
}
