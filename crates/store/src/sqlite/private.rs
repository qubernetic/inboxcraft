// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Mail is private: on unix the store makes its directories `0700` and its files `0600`, whatever the umask, so no
//! other account on the machine can read them. SQLite gives its `-wal` and `-shm` files the database file's mode.
//! Elsewhere the files inherit the ACL of the directory they are in, which for the app's profile directory is the
//! owner's alone.

use std::fs::{DirBuilder, File, OpenOptions};
use std::path::Path;

use crate::StoreError;

/// Creates `dir` and the directories above it that are missing, each one only the owner may enter.
pub(crate) fn create_dir(dir: &Path) -> Result<(), StoreError> {
    let mut builder = DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(dir)?;
    Ok(())
}

/// Opens `path` for writing, creating it, if it is new, so that only the owner may read it; a file there is kept.
pub(crate) fn create_file(path: &Path, truncate: bool) -> Result<File, StoreError> {
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(truncate);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    Ok(options.open(path)?)
}
