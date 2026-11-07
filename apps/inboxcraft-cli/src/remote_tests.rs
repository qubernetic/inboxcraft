// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{Reach, os_token};

#[test]
fn without_a_credential_store_there_is_no_token() {
    // Only where no credential store runs (the container, CI): elsewhere this would read the owner's.
    if keyring::Entry::store_status().is_err() {
        assert_eq!(os_token().unwrap_err().code, "no_token");
    }
}

#[test]
fn a_reach_shows_its_folder_but_not_its_token() {
    let reach = Reach::new(Some("/run/user/1000".into()), Box::new(|| Err(inboxcraft_control_key::Failure::new("no_token", "none"))));
    assert_eq!(format!("{reach:?}"), "Reach { runtime: Some(\"/run/user/1000\"), .. }");
}

#[cfg(unix)]
#[test]
fn a_socket_is_trusted_only_when_it_and_its_folder_are_the_users_alone() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let runtime = tempfile::tempdir().unwrap();
    let socket = inboxcraft_control_key::socket(runtime.path(), false);
    let folder = socket.parent().unwrap();
    std::fs::create_dir(folder).unwrap();
    std::fs::set_permissions(folder, std::fs::Permissions::from_mode(0o700)).unwrap();
    let _listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
    let uid = std::fs::metadata(&socket).unwrap().uid();
    assert!(super::trusted(&socket, uid));
    // Another user: neither the folder nor the socket is theirs.
    assert!(!super::trusted(&socket, uid.wrapping_add(1)));
    // A folder that lets others in.
    std::fs::set_permissions(folder, std::fs::Permissions::from_mode(0o750)).unwrap();
    assert!(!super::trusted(&socket, uid));
    std::fs::set_permissions(folder, std::fs::Permissions::from_mode(0o700)).unwrap();
    // A plain file in the socket's place, then nothing there.
    std::fs::remove_file(&socket).unwrap();
    std::fs::write(&socket, b"").unwrap();
    assert!(!super::trusted(&socket, uid));
    std::fs::remove_file(&socket).unwrap();
    assert!(!super::trusted(&socket, uid));
}

#[cfg(unix)]
#[test]
fn the_runtime_folder_must_be_the_users_alone() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let runtime = tempfile::tempdir().unwrap();
    std::fs::set_permissions(runtime.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let uid = std::fs::metadata(runtime.path()).unwrap().uid();
    assert!(super::private(runtime.path(), uid));
    assert!(!super::private(runtime.path(), uid.wrapping_add(1)));
    std::fs::set_permissions(runtime.path(), std::fs::Permissions::from_mode(0o711)).unwrap();
    assert!(!super::private(runtime.path(), uid));
}
