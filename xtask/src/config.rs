// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Values read from the workspace manifest.

use std::fs;
use std::path::Path;

use crate::Error;

/// The workspace licence, `workspace.package.license` in the root `Cargo.toml`: the one place it is written down.
pub(crate) fn licence(root: &Path) -> Result<String, Error> {
    let path = root.join("Cargo.toml");
    let text = fs::read_to_string(&path).map_err(|source| Error::Io { path: path.clone(), source })?;
    let config_error = |message: String| Error::Config { path: path.clone(), message };
    let manifest: toml::Table = text.parse().map_err(|error| config_error(format!("not valid TOML: {error}")))?;
    let licence = manifest
        .get("workspace")
        .and_then(|workspace| workspace.get("package"))
        .and_then(|package| package.get("license"))
        .and_then(toml::Value::as_str)
        .ok_or_else(|| config_error("no `workspace.package.license` string".to_owned()))?;
    if licence.trim().is_empty() || licence.chars().any(char::is_control) {
        return Err(config_error("`workspace.package.license` must be one line of text".to_owned()));
    }
    Ok(licence.to_owned())
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
