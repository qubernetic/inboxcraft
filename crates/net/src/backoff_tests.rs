// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::time::Duration;

use super::{Backoff, whole_seconds};

#[test]
fn the_wait_doubles_up_to_the_cap() {
    let backoff = Backoff::default();
    let waits: Vec<u64> = (0..12).map(|attempt| backoff.delay(attempt).as_secs()).collect();
    assert_eq!(waits, [0, 2, 4, 8, 16, 32, 64, 128, 256, 300, 300, 300]);
}

#[test]
fn many_attempts_stay_at_the_cap() {
    let backoff = Backoff { first: Duration::from_millis(10), cap: Duration::from_secs(60) };
    assert_eq!(backoff.delay(32), Duration::from_secs(60));
    assert_eq!(backoff.delay(33), Duration::from_secs(60));
    assert_eq!(backoff.delay(u32::MAX), Duration::from_secs(60));
    let huge = Backoff { first: Duration::MAX, cap: Duration::from_secs(1) };
    assert_eq!(huge.delay(2), Duration::from_secs(1));
}

#[test]
fn a_cap_below_the_first_wait_wins() {
    let backoff = Backoff { first: Duration::from_secs(10), cap: Duration::from_secs(3) };
    assert_eq!(backoff.delay(1), Duration::from_secs(3));
}

#[test]
fn seconds_are_rounded_up() {
    assert_eq!(whole_seconds(Duration::ZERO), 0);
    assert_eq!(whole_seconds(Duration::from_millis(1)), 1);
    assert_eq!(whole_seconds(Duration::from_secs(2)), 2);
    assert_eq!(whole_seconds(Duration::from_millis(2_500)), 3);
    assert_eq!(whole_seconds(Duration::MAX), u64::MAX);
}
