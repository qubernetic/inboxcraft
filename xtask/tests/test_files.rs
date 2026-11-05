// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `cargo xtask test-files` on a workspace with an inline `mod tests`.

use std::fs;
use std::process::Command;

#[test]
fn test_files_fails_on_an_inline_mod_tests() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("lib.rs"), "#[cfg(test)]\nmod tests {\n}\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_xtask")).args(["test-files", "--root"]).arg(dir.path()).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stdout).contains("lib.rs:2: inline test module `tests`"));
}
