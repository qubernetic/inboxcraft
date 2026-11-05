// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::time::Duration;

use super::{Clock, FakeClock, SystemClock, since_epoch};
use crate::Timestamp;

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_unix(seconds, 0).unwrap()
}

#[test]
fn a_fake_clock_stands_still_until_moved() {
    let clock = FakeClock::new(at(1_000));
    assert_eq!(clock.now(), at(1_000));
    assert_eq!(clock.now(), at(1_000));
    assert_eq!(clock.monotonic(), Duration::ZERO);
}

#[test]
fn advancing_moves_the_wall_and_the_monotonic_time() {
    let clock = FakeClock::new(at(1_000));
    clock.advance(Duration::from_millis(1_500));
    assert_eq!(clock.now(), Timestamp::from_unix(1_001, 500_000_000).unwrap());
    assert_eq!(clock.monotonic(), Duration::from_millis(1_500));
}

#[test]
fn setting_moves_the_wall_time_only() {
    let clock = FakeClock::new(at(1_000));
    clock.advance(Duration::from_secs(2));
    clock.set(at(10));
    assert_eq!(clock.now(), at(10));
    assert_eq!(clock.monotonic(), Duration::from_secs(2));
}

#[test]
fn clones_share_one_time() {
    let clock = FakeClock::new(at(0));
    let other = clock.clone();
    other.advance(Duration::from_secs(5));
    assert_eq!(clock.now(), at(5));
    assert_eq!(clock.monotonic(), Duration::from_secs(5));
}

#[test]
fn advancing_past_the_last_timestamp_stops_there() {
    let clock = FakeClock::new(Timestamp::MAX);
    clock.advance(Duration::from_secs(1));
    assert_eq!(clock.now(), Timestamp::MAX);
    clock.advance(Duration::MAX);
    clock.advance(Duration::MAX);
    assert_eq!(clock.monotonic(), Duration::MAX);
}

#[test]
fn a_fake_clock_works_behind_the_trait() {
    let clock = FakeClock::new(at(42));
    let shared: std::sync::Arc<dyn Clock> = std::sync::Arc::new(clock.clone());
    clock.advance(Duration::from_secs(1));
    assert_eq!(shared.now(), at(43));
    assert!(format!("{shared:?}").contains("FakeClock"));
}

// The system clock's own tests are the only ones that read the computer's clock.
#[test]
fn the_system_clock_reads_a_plausible_time() {
    let clock = SystemClock::default();
    // After 2020-01-01 and before 3000-01-01.
    let now = clock.now().unix_seconds();
    assert!((1_577_836_800..32_503_680_000).contains(&now), "{now}");
    let first = clock.monotonic();
    assert!(clock.monotonic() >= first);
}

#[test]
fn system_times_outside_the_range_are_clamped() {
    assert_eq!(since_epoch(Ok(Duration::from_secs(5))), at(5));
    assert_eq!(since_epoch(Err(Duration::from_secs(5))), at(-5));
    assert_eq!(since_epoch(Ok(Duration::MAX)), Timestamp::MAX);
    assert_eq!(since_epoch(Err(Duration::MAX)), Timestamp::MIN);
}
