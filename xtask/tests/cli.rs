// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The built `xtask` binary, run the way `cargo xtask` runs it.

use std::process::{Command, Output};

fn xtask(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_xtask")).args(args).output().unwrap()
}

#[test]
fn help_succeeds() {
    let output = xtask(&["help"]);
    assert_eq!(output.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&output.stdout).contains("usage: cargo xtask <command>"));
}

#[test]
fn an_unknown_command_exits_with_the_usage_code() {
    let output = xtask(&["frobnicate"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown command `frobnicate`"));
}
