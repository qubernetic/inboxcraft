// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{Look, PRIVATE, UNSHARED, kept};

fn folder(owner: u32, mode: u32) -> Look {
    Look { folder: true, owner, mode }
}

#[test]
fn a_folder_of_the_users_alone_is_kept() {
    assert!(kept(folder(1000, 0o700), 1000, PRIVATE));
    assert!(kept(folder(1000, 0o700), 1000, UNSHARED));
    assert!(kept(folder(0, 0o700), 0, PRIVATE));
}

#[test]
fn a_folder_of_another_user_is_not_kept() {
    assert!(!kept(folder(1001, 0o700), 1000, PRIVATE));
    assert!(!kept(folder(0, 0o700), 1000, UNSHARED));
    assert!(!kept(folder(1000, 0o700), 0, PRIVATE));
}

#[test]
fn a_private_folder_lets_nobody_else_in() {
    for mode in [0o710, 0o701, 0o750, 0o755, 0o770, 0o777, 0o1777] {
        assert!(!kept(folder(1000, mode), 1000, PRIVATE), "{mode:o}");
    }
}

#[test]
fn an_unshared_folder_lets_nobody_else_write() {
    assert!(kept(folder(1000, 0o755), 1000, UNSHARED));
    for mode in [0o720, 0o702, 0o775, 0o777, 0o1777] {
        assert!(!kept(folder(1000, mode), 1000, UNSHARED), "{mode:o}");
    }
}

#[test]
fn what_is_no_folder_is_never_kept() {
    let link = Look { folder: false, owner: 1000, mode: 0o700 };
    assert!(!kept(link, 1000, PRIVATE));
    assert!(!kept(link, 1000, UNSHARED));
}

#[cfg(unix)]
mod on_disk {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    use super::super::{PRIVATE, UNSHARED, check, effective_uid, look};

    /// The owner of a folder this process makes.
    fn uid() -> u32 {
        let probe = tempfile_dir("probe");
        let owner = std::fs::metadata(&probe).unwrap().uid();
        std::fs::remove_dir(&probe).unwrap();
        owner
    }

    /// A new folder of this process's, made without the tempfile crate (not a dependency here).
    fn tempfile_dir(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("inboxcraft-control-key-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir(&path).unwrap();
        path
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn the_effective_uid_owns_what_the_process_makes() {
        assert_eq!(effective_uid(), Some(uid()));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_folder_is_looked_at_without_following_a_link() {
        let dir = tempfile_dir("look");
        let inner = dir.join("inner");
        std::fs::create_dir(&inner).unwrap();
        std::fs::set_permissions(&inner, std::fs::Permissions::from_mode(0o700)).unwrap();
        let link = dir.join("link");
        std::os::unix::fs::symlink(&inner, &link).unwrap();
        let seen = look(&inner).unwrap();
        assert!(seen.folder);
        assert_eq!((seen.owner, seen.mode & 0o777), (uid(), 0o700));
        assert!(!look(&link).unwrap().folder);
        assert!(check(&inner, PRIVATE).is_ok());
        let refused = check(&link, PRIVATE).unwrap_err();
        assert!(refused.contains("not a folder of this user's alone"), "{refused}");
        std::fs::set_permissions(&inner, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(check(&inner, PRIVATE).is_err());
        assert!(check(&inner, UNSHARED).is_ok());
        std::fs::set_permissions(&inner, std::fs::Permissions::from_mode(0o777)).unwrap();
        assert!(check(&inner, UNSHARED).is_err());
        assert!(check(&dir.join("missing"), UNSHARED).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
