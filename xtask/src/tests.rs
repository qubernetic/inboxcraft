// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::ffi::OsString;

use super::cargo_or_default;

#[test]
fn cargo_comes_from_the_environment_or_the_path() {
    assert_eq!(cargo_or_default(Some(OsString::from("/opt/cargo"))), "/opt/cargo");
    assert_eq!(cargo_or_default(None), "cargo");
}
