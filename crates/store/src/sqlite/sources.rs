// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The message sources, one file per body row outside the database: `sources/<body id>.eml`.
//!
//! The crash rule: a source file is written and synced before the row that names it commits, and removed only after
//! the row is gone for good. A kill in between leaves a file no row names, which [`sweep`] removes at the next open.

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};

use super::private;
use crate::{MAX_SOURCE_BYTES, StoreError};

const EXTENSION: &str = ".eml";
const PARTIAL: &str = ".partial";

fn path(dir: &Path, id: i64) -> PathBuf {
    dir.join(format!("{id}{EXTENSION}"))
}

/// Writes the source of body row `id`: into a partial file, synced, renamed into place, the directory synced.
pub(crate) fn write(dir: &Path, id: i64, source: &[u8]) -> Result<(), StoreError> {
    let partial = dir.join(format!("{id}{PARTIAL}"));
    let mut file = private::create_file(&partial, true)?;
    file.write_all(source)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&partial, path(dir, id))?;
    sync_dir(dir)
}

/// Reads the source of body row `id`, which must be `length` bytes long.
pub(crate) fn read(dir: &Path, id: i64, length: i64) -> Result<Vec<u8>, StoreError> {
    let length = usize::try_from(length).ok().filter(|length| *length <= MAX_SOURCE_BYTES).ok_or(StoreError::Corrupt("a source length"))?;
    let file = File::open(path(dir, id)).map_err(|error| match error.kind() {
        ErrorKind::NotFound => StoreError::Corrupt("a message source is missing"),
        kind => StoreError::Io(kind),
    })?;
    let mut source = Vec::with_capacity(length);
    file.take(u64::try_from(length).unwrap_or(u64::MAX).saturating_add(1)).read_to_end(&mut source)?;
    if source.len() == length { Ok(source) } else { Err(StoreError::Corrupt("a message source has the wrong length")) }
}

/// Removes the source of body row `id`; a failure is left to the next [`sweep`].
pub(crate) fn remove(dir: &Path, id: i64) {
    let _ = fs::remove_file(path(dir, id));
}

/// Removes the source and partial files no body row in `kept` names; other files are left alone.
pub(crate) fn sweep(dir: &Path, kept: &BTreeSet<i64>) -> Result<(), StoreError> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let orphan = match (name.strip_suffix(EXTENSION), name.strip_suffix(PARTIAL)) {
            (Some(id), _) => id.parse::<i64>().is_ok_and(|id| !kept.contains(&id)),
            (_, Some(id)) => id.parse::<i64>().is_ok(),
            _ => false,
        };
        if orphan {
            fs::remove_file(entry.path())?;
        }
    }
    sync_dir(dir)
}

#[cfg(unix)]
fn sync_dir(dir: &Path) -> Result<(), StoreError> {
    File::open(dir)?.sync_all()?;
    Ok(())
}

/// Windows has no directory handle to sync; NTFS journals the rename.
#[cfg(not(unix))]
fn sync_dir(_dir: &Path) -> Result<(), StoreError> {
    Ok(())
}
