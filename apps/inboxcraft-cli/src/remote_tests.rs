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
