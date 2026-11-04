// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `cargo xtask spdx [--fix]`: every Rust source file starts with the SPDX lines.
//!
//! The lines are a block of `// SPDX-` comments at the very top: at least one `SPDX-FileCopyrightText` and one
//! `SPDX-License-Identifier` carrying the workspace licence (`workspace.package.license`). `--fix` adds what is
//! missing and keeps what is there; a file under another licence is reported and never changed.

use std::fs;
use std::io::Write;
use std::path::Path;

use crate::{Error, config, walk};

const COPYRIGHT_TAG: &str = "// SPDX-FileCopyrightText: ";
const LICENCE_TAG: &str = "// SPDX-License-Identifier: ";
/// The holder `--fix` writes into a new copyright line.
const HOLDER: &str = "Qubernetic and the InboxCraft contributors";

/// What is wrong with a file's SPDX lines.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Finding {
    /// No licence line at the top (and maybe no copyright line either).
    Missing,
    /// The licence line is there, the copyright line is not.
    MissingCopyright,
    /// The licence line names another licence.
    WrongLicence(String),
}

impl Finding {
    pub(crate) fn describe(&self, licence: &str) -> String {
        match self {
            Self::Missing => "no SPDX lines at the top".to_owned(),
            Self::MissingCopyright => "no SPDX-FileCopyrightText line".to_owned(),
            Self::WrongLicence(found) => {
                format!("licence is `{found}`, the workspace licence is `{licence}`; change it by hand")
            }
        }
    }
}

/// Splits `text` into the leading block of `// SPDX-` lines (each with its line ending) and the rest.
fn split_header(text: &str) -> (Vec<&str>, &str) {
    let mut block = Vec::new();
    let mut rest = text;
    for line in text.split_inclusive('\n') {
        if !line.starts_with("// SPDX-") {
            break;
        }
        block.push(line);
        rest = rest.get(line.len()..).unwrap_or_default();
    }
    (block, rest)
}

fn value<'a>(line: &'a str, tag: &str) -> Option<&'a str> {
    line.strip_prefix(tag).map(str::trim)
}

/// Checks the SPDX lines of one file against the workspace `licence`.
pub(crate) fn check(text: &str, licence: &str) -> Option<Finding> {
    let (block, _) = split_header(text);
    let has_copyright = block.iter().any(|line| line.starts_with(COPYRIGHT_TAG));
    match block.iter().find_map(|line| value(line, LICENCE_TAG)) {
        None => Some(Finding::Missing),
        Some(found) if found != licence => Some(Finding::WrongLicence(found.to_owned())),
        Some(_) if !has_copyright => Some(Finding::MissingCopyright),
        Some(_) => None,
    }
}

/// The file with the missing SPDX lines added, or `None` when it needs no change or cannot be fixed.
pub(crate) fn fix(text: &str, licence: &str) -> Option<String> {
    match check(text, licence) {
        None | Some(Finding::WrongLicence(_)) => return None,
        Some(Finding::Missing | Finding::MissingCopyright) => {}
    }
    let (block, rest) = split_header(text);
    let mut fixed = String::with_capacity(text.len().saturating_add(128));
    if !block.iter().any(|line| line.starts_with(COPYRIGHT_TAG)) {
        fixed.push_str(&format!("{COPYRIGHT_TAG}{HOLDER}\n"));
    }
    for line in &block {
        fixed.push_str(line);
        if !line.ends_with('\n') {
            fixed.push('\n');
        }
    }
    if !block.iter().any(|line| line.starts_with(LICENCE_TAG)) {
        fixed.push_str(&format!("{LICENCE_TAG}{licence}\n"));
    }
    if block.is_empty() && !rest.is_empty() && !rest.starts_with('\n') && !rest.starts_with("\r\n") {
        fixed.push('\n');
    }
    fixed.push_str(rest);
    Some(fixed)
}

/// Checks (or with `fix`, repairs) every Rust file under `root`, reports to `out` and returns whether all pass.
pub(crate) fn gate(root: &Path, fix_files: bool, out: &mut dyn Write) -> Result<bool, Error> {
    let licence = config::licence(root)?;
    let files = walk::rust_files(root)?;
    let mut failing = 0_usize;
    for rel in &files {
        let path = root.join(rel);
        let text = fs::read_to_string(&path).map_err(|source| Error::Io { path: path.clone(), source })?;
        let Some(finding) = check(&text, &licence) else { continue };
        if fix_files && let Some(fixed) = fix(&text, &licence) {
            fs::write(&path, fixed).map_err(|source| Error::Io { path: path.clone(), source })?;
            writeln!(out, "spdx: fixed {}", rel.display())?;
            continue;
        }
        failing = failing.saturating_add(1);
        writeln!(out, "spdx: {}: {}", rel.display(), finding.describe(&licence))?;
    }
    if failing == 0 {
        writeln!(out, "spdx: {} files checked, all have the SPDX lines", files.len())?;
    } else {
        writeln!(out, "spdx: {failing} of {} files need the SPDX lines; `cargo xtask spdx --fix` adds them", files.len())?;
    }
    Ok(failing == 0)
}

#[cfg(test)]
#[path = "spdx_tests.rs"]
mod tests;
