// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::BTreeSet;

use inboxcraft_model::Uid;

use super::free_uid;

fn taken(list: &[u32]) -> BTreeSet<Uid> {
    list.iter().filter_map(|n| Uid::new(*n)).collect()
}

#[test]
fn our_uids_come_from_the_top_down_past_every_one_taken() {
    assert_eq!(free_uid(&taken(&[])), Uid::new(u32::MAX));
    assert_eq!(free_uid(&taken(&[1, 2, 3])), Uid::new(u32::MAX));
    assert_eq!(free_uid(&taken(&[5, u32::MAX])), Uid::new(u32::MAX - 1));
    assert_eq!(free_uid(&taken(&[5, u32::MAX - 2, u32::MAX - 1, u32::MAX])), Uid::new(u32::MAX - 3));
    assert_eq!(free_uid(&taken(&[u32::MAX - 1, u32::MAX])), Uid::new(u32::MAX - 2));
}
