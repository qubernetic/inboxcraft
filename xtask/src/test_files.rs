// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `cargo xtask test-files`: tests live in files of their own, never in an inline `mod tests { ... }`.
//!
//! A module with a body is an inline test module when it is called `tests` or `<name>_tests`, or when a
//! `#[cfg(test)]`-style attribute sits on it. Test files (`tests.rs`, `<name>_tests.rs`, anything under a `tests/`
//! directory) are not checked. The check reads lines, not a syntax tree: a module line inside a multi-line string
//! would count too.

use std::fs;
use std::io::Write;
use std::path::Path;

use crate::{Error, walk};

/// Whether `rel` is a file that holds tests and may therefore contain anything.
pub(crate) fn is_test_file(rel: &Path) -> bool {
    let name = rel.file_name().and_then(|name| name.to_str()).unwrap_or_default();
    name == "tests.rs" || name.ends_with("_tests.rs") || rel.parent().is_some_and(|dir| dir.iter().any(|part| part == "tests"))
}

/// Strips the leading attributes from `line`; returns the rest and whether one of them is a `cfg` that names `test`.
/// `None` when an attribute does not close on this line.
fn strip_attributes(mut line: &str) -> Option<(&str, bool)> {
    let mut cfg_test = false;
    while let Some(attr) = line.strip_prefix("#[") {
        let end = attr.find(']')?;
        let body = attr.get(..end).unwrap_or_default();
        cfg_test |= body.starts_with("cfg") && body.contains("test");
        line = attr.get(end.saturating_add(1)..).unwrap_or_default().trim_start();
    }
    Some((line, cfg_test))
}

/// Strips a leading `pub`, `pub(crate)`, `pub(in path)` and the like.
fn strip_visibility(line: &str) -> Option<&str> {
    let Some(rest) = line.strip_prefix("pub") else { return Some(line) };
    let rest = rest.trim_start();
    match rest.strip_prefix('(') {
        Some(inner) => inner.find(')').and_then(|end| inner.get(end.saturating_add(1)..)).map(str::trim_start),
        None => Some(rest),
    }
}

/// The name of the module that `line` opens with a body, if it does.
fn inline_module(line: &str) -> Option<&str> {
    let rest = strip_visibility(line)?.strip_prefix("mod ")?.trim_start();
    let end = rest.find(|c: char| !(c.is_alphanumeric() || c == '_')).unwrap_or(rest.len());
    let (name, after) = rest.split_at(end);
    (!name.is_empty() && after.trim_start().starts_with('{')).then_some(name)
}

/// Every inline test module in `text`, as its 1-based line number and name.
pub(crate) fn inline_test_modules(text: &str) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    let mut cfg_test_pending = false;
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        let Some((rest, cfg_test)) = strip_attributes(line) else { continue };
        cfg_test_pending |= cfg_test;
        if rest.is_empty() {
            continue;
        }
        if let Some(name) = inline_module(rest)
            && (cfg_test_pending || name == "tests" || name.ends_with("_tests"))
        {
            found.push((index.saturating_add(1), name.to_owned()));
        }
        cfg_test_pending = false;
    }
    found
}

/// Checks every Rust file under `root` that is not a test file, reports to `out` and returns whether all pass.
pub(crate) fn gate(root: &Path, out: &mut dyn Write) -> Result<bool, Error> {
    let files = walk::rust_files(root)?;
    let mut count = 0_usize;
    for rel in files.iter().filter(|rel| !is_test_file(rel)) {
        let path = root.join(rel);
        let text = fs::read_to_string(&path).map_err(|source| Error::Io { path: path.clone(), source })?;
        for (line, name) in inline_test_modules(&text) {
            count = count.saturating_add(1);
            writeln!(
                out,
                "test-files: {}:{line}: inline test module `{name}`; move it into a file of its own \
                 (`#[cfg(test)] mod tests;` next to a `tests.rs`)",
                rel.display()
            )?;
        }
    }
    if count == 0 {
        writeln!(out, "test-files: {} files checked, no inline test modules", files.len())?;
    } else {
        let noun = if count == 1 { "module" } else { "modules" };
        writeln!(out, "test-files: {count} inline test {noun} in {} files", files.len())?;
    }
    Ok(count == 0)
}

#[cfg(test)]
#[path = "test_files_tests.rs"]
mod tests;
