// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Mail is private: on unix the store makes its directories `0700` and its files `0600`, whatever the umask, and
//! narrows any it finds wider, so no other account on the machine can read them. SQLite gives its `-wal` and `-shm`
//! files the database file's mode. Elsewhere the files inherit the ACL of the directory they are in, which for the
//! app's profile directory is the owner's alone.

use std::fs::{DirBuilder, File, OpenOptions};
use std::path::Path;

use crate::StoreError;

/// The mode of a store directory on unix.
#[cfg(unix)]
const DIR_MODE: u32 = 0o700;

/// The mode of a store file on unix.
#[cfg(unix)]
const FILE_MODE: u32 = 0o600;

/// Creates `dir` and the directories above it that are missing, each one only the owner may enter.
pub(crate) fn create_dir(dir: &Path) -> Result<(), StoreError> {
    let mut builder = DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, DIR_MODE);
    builder.create(dir)?;
    Ok(())
}

/// Opens `path` for writing, creating it, if it is new, so that only the owner may read it; a file there is kept.
pub(crate) fn create_file(path: &Path, truncate: bool) -> Result<File, StoreError> {
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(truncate);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, FILE_MODE);
    Ok(options.open(path)?)
}

/// Narrows the directories in `dirs` to `0700` and the files in `files` to `0600`; a path that is not there is left
/// out. A store made by an older build or copied in keeps no wider mode.
#[cfg(unix)]
pub(crate) fn tighten(dirs: &[&Path], files: &[&Path]) -> Result<(), StoreError> {
    let dirs = dirs.iter().map(|dir| (dir, DIR_MODE));
    for (path, mode) in dirs.chain(files.iter().map(|file| (file, FILE_MODE))) {
        set_mode(path, mode)?;
    }
    Ok(())
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) -> Result<(), StoreError> {
    use std::os::unix::fs::PermissionsExt;
    let held = match std::fs::metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        held => held?,
    };
    if held.permissions().mode() & 0o777 != mode {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))?;
    }
    Ok(())
}

/// Elsewhere the inherited ACL decides; see the module notes.
#[cfg(not(unix))]
pub(crate) fn tighten(_dirs: &[&Path], _files: &[&Path]) -> Result<(), StoreError> {
    Ok(())
}
