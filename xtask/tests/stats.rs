// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `cargo xtask stats` on this workspace.

use std::process::Command;

#[test]
fn stats_prints_a_line_per_crate() {
    let output = Command::new(env!("CARGO_BIN_EXE_xtask")).arg("stats").output().unwrap();
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.starts_with("stats: crate "), "{stdout}");
    assert!(stdout.lines().any(|line| line.starts_with("stats: inboxcraft-time ")), "{stdout}");
    assert!(stdout.lines().last().is_some_and(|line| line.starts_with("stats: total ")), "{stdout}");
}
