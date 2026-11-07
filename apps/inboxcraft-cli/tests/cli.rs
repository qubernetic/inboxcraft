// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The built binary, run the way a person or an agent runs it.

use std::process::{Command, Stdio};

use inboxcraft_testkit::store;

#[test]
fn the_binary_prints_its_version() {
    let output = Command::new(env!("CARGO_BIN_EXE_inboxcraft-cli")).arg("--version").output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), format!("inboxcraft-cli {}\n", env!("CARGO_PKG_VERSION")));
}

#[test]
fn the_binary_without_arguments_is_a_usage_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_inboxcraft-cli")).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("--help"));
}

#[test]
fn the_binary_runs_a_command_on_a_store_without_reading_stdin() {
    let dir = tempfile::tempdir().unwrap();
    store::fixture(dir.path()).unwrap();
    // A stdin that is never closed: a CLI that prompted or read it would hang here.
    let child = Command::new(env!("CARGO_BIN_EXE_inboxcraft-cli"))
        .args(["--store", dir.path().to_str().unwrap(), "--json", "mail.list"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let answer: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(answer["total"], 2);
}

#[test]
fn the_binary_refuses_an_owner_command_with_exit_code_1() {
    let dir = tempfile::tempdir().unwrap();
    store::fixture(dir.path()).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_inboxcraft-cli"))
        .args(["--store", dir.path().to_str().unwrap(), "account.remove", r#"{"account":1}"#])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("only safe commands run"));
}
