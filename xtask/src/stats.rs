// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `cargo xtask stats`: Rust files and lines per crate.
//!
//! A line is a line that is not blank. Test lines are those in test files (`tests.rs`, `<name>_tests.rs`, anything
//! under a `tests/` directory); code lines are the rest. A file belongs to the member whose directory holds it most
//! closely; files outside every member are not counted. The gate only reports and never fails on the numbers.

use std::fs;
use std::io::Write;
use std::path::Path;

use crate::test_files::is_test_file;
use crate::{Error, walk, workspace};

#[derive(Default, Clone, Copy)]
struct Counts {
    files: usize,
    code: usize,
    test: usize,
}

impl Counts {
    fn add(&mut self, other: Self) {
        self.files = self.files.saturating_add(other.files);
        self.code = self.code.saturating_add(other.code);
        self.test = self.test.saturating_add(other.test);
    }
}

/// Counts the files and lines of every member of the workspace at `root` and reports them to `out`.
pub(crate) fn gate(root: &Path, out: &mut dyn Write) -> Result<bool, Error> {
    let members = workspace::members(root)?;
    let mut counts = vec![Counts::default(); members.len()];
    for rel in walk::rust_files(root)? {
        let owner =
            members.iter().enumerate().filter(|(_, member)| rel.starts_with(&member.dir)).max_by_key(|(_, member)| member.dir.components().count());
        let Some((index, member)) = owner else { continue };
        let path = root.join(&rel);
        let text = fs::read_to_string(&path).map_err(|source| Error::Io { path: path.clone(), source })?;
        let lines = text.lines().filter(|line| !line.trim().is_empty()).count();
        let in_crate = rel.strip_prefix(&member.dir).unwrap_or(&rel);
        let file = if is_test_file(in_crate) { Counts { files: 1, code: 0, test: lines } } else { Counts { files: 1, code: lines, test: 0 } };
        if let Some(slot) = counts.get_mut(index) {
            slot.add(file);
        }
    }
    let width = members.iter().map(|member| member.name.len()).chain([5]).max().unwrap_or(5);
    writeln!(out, "stats: {:<width$}  files  code lines  test lines", "crate")?;
    let mut total = Counts::default();
    for (member, count) in members.iter().zip(&counts) {
        writeln!(out, "stats: {:<width$}  {:>5}  {:>10}  {:>10}", member.name, count.files, count.code, count.test)?;
        total.add(*count);
    }
    writeln!(out, "stats: {:<width$}  {:>5}  {:>10}  {:>10}", "total", total.files, total.code, total.test)?;
    Ok(true)
}

#[cfg(test)]
#[path = "stats_tests.rs"]
mod tests;
