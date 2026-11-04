// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `cargo xtask layers` on a workspace with an upward dependency.

use std::fs;
use std::process::Command;

#[test]
fn layers_fails_on_an_upward_dependency() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("Cargo.toml"), "[workspace]\nmembers = [\"crates/*\"]\n").unwrap();
    for (name, deps) in [("inboxcraft-model", "inboxcraft-engine = { path = \"../inboxcraft-engine\" }\n"), ("inboxcraft-engine", "")] {
        fs::create_dir_all(dir.path().join("crates").join(name)).unwrap();
        let manifest = format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\n\n[dependencies]\n{deps}");
        fs::write(dir.path().join("crates").join(name).join("Cargo.toml"), manifest).unwrap();
    }
    let output = Command::new(env!("CARGO_BIN_EXE_xtask")).args(["layers", "--root"]).arg(dir.path()).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("`inboxcraft-model` (layer 1) depends on `inboxcraft-engine` (layer 4)"), "{stdout}");
}
