// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `cargo xtask assets` on a workspace with a picture that `ATTRIBUTION.md` does not list.

use std::fs;
use std::process::Command;

#[test]
fn assets_fails_on_a_file_without_a_row() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("ATTRIBUTION.md"), "| Asset |\n|---|\n| `docs/listed.png` |\n").unwrap();
    fs::create_dir_all(dir.path().join("docs")).unwrap();
    fs::write(dir.path().join("docs/listed.png"), "").unwrap();
    fs::write(dir.path().join("docs/unlisted.svg"), "").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_xtask")).args(["assets", "--root"]).arg(dir.path()).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("assets: docs/unlisted.svg has no row in ATTRIBUTION.md"), "{stdout}");
    assert!(!stdout.contains("listed.png has no row"), "{stdout}");
}
