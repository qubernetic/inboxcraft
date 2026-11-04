// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The built binary, run the way a person runs it.

use std::process::Command;

#[test]
fn the_binary_prints_its_version() {
    let output = Command::new(env!("CARGO_BIN_EXE_inboxcraft-web")).arg("--version").output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), format!("inboxcraft-web {}\n", env!("CARGO_PKG_VERSION")));
}

#[test]
fn the_binary_refuses_to_pretend() {
    let output = Command::new(env!("CARGO_BIN_EXE_inboxcraft-web")).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
}
