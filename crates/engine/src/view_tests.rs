// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use inboxcraft_model::Role;

use super::{ROLES, role, role_name};

#[test]
fn every_role_has_a_name_that_reads_back() {
    for each in ROLES {
        assert_eq!(role(&role_name(each)), Some(each));
    }
    assert_eq!(role("INBOX"), Some(Role::Inbox));
    assert_eq!(role("spam"), None);
    assert_eq!(role(""), None);
}
