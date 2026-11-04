// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::ffi::OsString;

use super::{cargo_or_default, count};

#[test]
fn cargo_comes_from_the_environment_or_the_path() {
    assert_eq!(cargo_or_default(Some(OsString::from("/opt/cargo"))), "/opt/cargo");
    assert_eq!(cargo_or_default(None), "cargo");
}

#[test]
fn count_puts_the_noun_in_the_plural_unless_there_is_one() {
    assert_eq!(count(0, "crate"), "0 crates");
    assert_eq!(count(1, "crate"), "1 crate");
    assert_eq!(count(2, "violation"), "2 violations");
}
