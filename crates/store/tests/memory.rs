// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The shared contract on the memory backend, and what only it does.

#![allow(clippy::unwrap_used)]

mod contract;

use inboxcraft_store::{Body, MAX_SOURCE_BYTES, MAX_TEXT_BYTES, Memory, Storage, StoreConfig, StoreError};

contract_tests!(Memory::new(StoreConfig::default()));

#[test]
fn the_config_is_the_one_given() {
    assert_eq!(Memory::new(StoreConfig { body_days: 3 }).config(), StoreConfig { body_days: 3 });
    assert_eq!(Memory::default().config(), StoreConfig::default());
}

#[test]
fn a_source_over_the_cap_is_refused() {
    assert_eq!(Body::new(vec![0; MAX_SOURCE_BYTES + 1], ""), Err(StoreError::TooLarge));
    assert_eq!(Body::new(vec![0; MAX_SOURCE_BYTES], "").map(|body| body.source().len()), Ok(MAX_SOURCE_BYTES));
}

#[test]
fn a_long_text_is_cut_at_a_character_boundary() {
    let text = format!("a{}", "é".repeat(MAX_TEXT_BYTES));
    let body = Body::new(Vec::new(), &text).unwrap();
    assert_eq!(body.text().len(), MAX_TEXT_BYTES - 1);
    assert!(body.text().ends_with('é'));
}
