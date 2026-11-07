// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Whether a folder on the way to the socket is the user's alone. The app serves the channel only from such a folder,
//! and a client shows its token only to a socket in one: in a folder another account can write, that account could
//! put its own socket in the app's place and read the token.
//!
//! The decision ([`kept`]) takes the user id as a parameter. On Unix, [`look`] reads a path without following a
//! symbolic link, and [`effective_uid`] is the owner of `/proc/self` (Linux only: the standard library does not give
//! the effective user id, and the channel adds no crate for it); where it is not known, [`check`] refuses.

#[cfg(unix)]
use std::path::Path;

/// Mode bits nobody but the owner may have: a private folder lets nobody else read, write or enter it.
pub const PRIVATE: u32 = 0o077;

/// Mode bits nobody but the owner may have in an unshared folder: nobody else may write it.
pub const UNSHARED: u32 = 0o022;

/// What a look at a path found, without following a symbolic link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Look {
    /// The path is a folder itself, not a link to one, nor anything else.
    pub folder: bool,
    /// The user id of its owner.
    pub owner: u32,
    /// Its mode bits.
    pub mode: u32,
}

/// Whether `look` shows a folder owned by `uid` whose mode has none of the bits in `others` ([`PRIVATE`] or
/// [`UNSHARED`]).
#[must_use]
pub fn kept(look: Look, uid: u32, others: u32) -> bool {
    look.folder && look.owner == uid && look.mode & others == 0
}

/// The effective user id of this process: the owner of `/proc/self`. `None` where there is no `/proc`.
#[cfg(unix)]
#[must_use]
pub fn effective_uid() -> Option<u32> {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata("/proc/self").ok().map(|meta| meta.uid())
}

/// Looks at `path` without following a symbolic link.
///
/// # Errors
///
/// `path` cannot be looked at.
#[cfg(unix)]
pub fn look(path: &Path) -> std::io::Result<Look> {
    use std::os::unix::fs::MetadataExt;
    let meta = std::fs::symlink_metadata(path)?;
    Ok(Look { folder: meta.file_type().is_dir(), owner: meta.uid(), mode: meta.mode() })
}

/// Whether `folder` is a folder of this process's user that has none of the mode bits in `others`.
///
/// # Errors
///
/// Why not, in words for the owner: the user is not known, the folder cannot be looked at, or it is a link, not a
/// folder, another user's, or open to others.
#[cfg(unix)]
pub fn check(folder: &Path, others: u32) -> Result<(), String> {
    let uid = effective_uid().ok_or("the user this program runs as is not known on this system (no /proc/self)")?;
    let seen = look(folder).map_err(|error| format!("{} cannot be looked at: {error}", folder.display()))?;
    if kept(seen, uid, others) {
        Ok(())
    } else {
        Err(format!("{} is not a folder of this user's alone (a real folder, theirs, mode {:o} at most)", folder.display(), 0o777 & !others))
    }
}

#[cfg(test)]
#[path = "folder_tests.rs"]
mod tests;
