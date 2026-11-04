// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::name;

#[test]
fn name_is_the_package_name() {
    assert_eq!(name(), "inboxcraft-time");
}
