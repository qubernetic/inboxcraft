// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The Rust source files of a workspace.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::Error;

/// How deep the walk goes before it gives up; far deeper than any real source tree.
pub(crate) const MAX_DEPTH: usize = 32;

/// Every `.rs` file under `root`, relative to it and sorted. Skips hidden directories, build output (`target`,
/// `target-*`) and the private `plan/`, and never follows symbolic links.
pub(crate) fn rust_files(root: &Path) -> Result<Vec<PathBuf>, Error> {
    let mut files = Vec::new();
    visit(root, Path::new(""), 0, &mut files)?;
    files.sort();
    Ok(files)
}

fn skipped(name: &str) -> bool {
    name.starts_with('.') || name == "target" || name.starts_with("target-") || name == "plan"
}

fn visit(root: &Path, rel: &Path, depth: usize, files: &mut Vec<PathBuf>) -> Result<(), Error> {
    let dir = root.join(rel);
    let io_error = |source| Error::Io { path: dir.clone(), source };
    if depth > MAX_DEPTH {
        return Err(io_error(io::Error::other(format!("directory tree deeper than {MAX_DEPTH} levels"))));
    }
    for entry in fs::read_dir(&dir).map_err(io_error)? {
        let entry = entry.map_err(io_error)?;
        let kind = entry.file_type().map_err(io_error)?;
        let name = entry.file_name();
        let path = rel.join(&name);
        if kind.is_dir() && !name.to_str().is_some_and(skipped) {
            visit(root, &path, depth + 1, files)?;
        } else if kind.is_file() && path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "walk_tests.rs"]
mod tests;
