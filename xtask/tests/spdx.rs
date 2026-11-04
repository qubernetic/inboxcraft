// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `cargo xtask spdx` on a workspace with a file that lacks the SPDX lines.

use std::fs;
use std::process::Command;

#[test]
fn spdx_fails_on_a_file_without_the_lines_and_fix_adds_them() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("Cargo.toml"), "[workspace.package]\nlicense = \"MIT OR Apache-2.0\"\n").unwrap();
    fs::write(dir.path().join("lib.rs"), "pub fn f() {}\n").unwrap();
    let root = dir.path().to_str().unwrap();
    let xtask = |args: &[&str]| Command::new(env!("CARGO_BIN_EXE_xtask")).args(args).arg("--root").arg(root).output().unwrap();

    let check = xtask(&["spdx"]);
    assert_eq!(check.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&check.stdout).contains("lib.rs: no SPDX lines"));

    assert_eq!(xtask(&["spdx", "--fix"]).status.code(), Some(0));
    assert!(
        fs::read_to_string(dir.path().join("lib.rs"))
            .unwrap()
            .starts_with("// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors\n// SPDX-License-Identifier: MIT OR Apache-2.0\n")
    );
    assert_eq!(xtask(&["spdx"]).status.code(), Some(0));
}
