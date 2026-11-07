// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::Path;

use super::{APP_ID, ENTRY, SERVICE, SOCKET_FILE, name, socket, sockets};

#[test]
fn name_is_the_package_name() {
    assert_eq!(name(), "inboxcraft-control-key");
}

#[test]
fn the_token_is_kept_under_the_app_id() {
    assert_eq!(SERVICE, APP_ID);
    assert_eq!(APP_ID, "io.github.qubernetic.inboxcraft");
    assert_eq!(ENTRY, "control-token");
}

#[test]
fn the_socket_lives_in_a_folder_of_its_own_in_the_runtime_folder() {
    let runtime = Path::new("/run/user/1000");
    assert_eq!(socket(runtime, false), runtime.join("inboxcraft").join(SOCKET_FILE));
}

#[test]
fn in_the_flatpak_the_socket_lives_in_the_folder_the_host_shares() {
    let runtime = Path::new("/run/user/1000");
    assert_eq!(socket(runtime, true), runtime.join("app").join(APP_ID).join(SOCKET_FILE));
}

#[test]
fn a_client_looks_for_a_build_of_its_own_first_then_the_flatpak() {
    let runtime = Path::new("/run/user/1000");
    assert_eq!(sockets(runtime), [socket(runtime, false), socket(runtime, true)]);
}
