// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The built binary, run the way a person runs it. The window itself is not opened here: tests have no display.

use std::process::Command;

#[test]
fn the_binary_prints_its_version() {
    let output = Command::new(env!("CARGO_BIN_EXE_inboxcraft")).arg("--version").output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), format!("inboxcraft {}\n", env!("CARGO_PKG_VERSION")));
}

#[test]
fn the_binary_explains_its_arguments() {
    let output = Command::new(env!("CARGO_BIN_EXE_inboxcraft")).arg("--help").output().unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("--sample"));
}

#[test]
fn the_binary_refuses_an_unknown_argument() {
    let output = Command::new(env!("CARGO_BIN_EXE_inboxcraft")).arg("--frobnicate").output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("--help"));
}
