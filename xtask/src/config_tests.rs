// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fs;

use super::licence;
use crate::Error;

#[test]
fn the_licence_comes_from_the_workspace_manifest() {
    assert_eq!(licence(&crate::default_root()).unwrap(), "MIT OR Apache-2.0");
}

#[test]
fn a_missing_manifest_is_an_io_error() {
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(licence(dir.path()), Err(Error::Io { .. })));
}

fn config_error(manifest: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("Cargo.toml"), manifest).unwrap();
    match licence(dir.path()) {
        Err(Error::Config { message, .. }) => message,
        other => panic!("expected a config error, got {other:?}"),
    }
}

#[test]
fn a_broken_manifest_is_a_config_error() {
    assert!(config_error("[workspace\n").starts_with("not valid TOML"));
}

#[test]
fn a_manifest_without_the_licence_is_a_config_error() {
    for manifest in ["", "[workspace]\n", "[workspace.package]\n", "[workspace.package]\nlicense = 3\n", "workspace = 1\n"] {
        assert_eq!(config_error(manifest), "no `workspace.package.license` string", "{manifest:?}");
    }
}

#[test]
fn a_licence_that_cannot_go_into_a_comment_line_is_a_config_error() {
    for value in ["\"\"", "\"MIT\\nOR Apache-2.0\"", "\"  \""] {
        let manifest = format!("[workspace.package]\nlicense = {value}\n");
        assert_eq!(config_error(&manifest), "`workspace.package.license` must be one line of text", "{value}");
    }
}
