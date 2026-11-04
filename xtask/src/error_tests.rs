// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::error::Error as _;
use std::io;
use std::path::PathBuf;

use super::Error;

#[test]
fn every_error_names_what_failed() {
    let io_error = Error::Io { path: PathBuf::from("a/b.rs"), source: io::Error::other("denied") };
    assert_eq!(io_error.to_string(), "a/b.rs: denied");
    assert!(io_error.source().is_some());

    let config = Error::Config { path: PathBuf::from("Cargo.toml"), message: "no licence".to_owned() };
    assert_eq!(config.to_string(), "Cargo.toml: no licence");
    assert!(config.source().is_none());

    let spawn = Error::Spawn { program: "cargo".to_owned(), source: io::Error::other("not found") };
    assert_eq!(spawn.to_string(), "cannot run `cargo`: not found");
    assert!(spawn.source().is_some());

    let output = Error::from(io::Error::other("closed"));
    assert_eq!(output.to_string(), "cannot write the report: closed");
    assert!(output.source().is_some());
}
