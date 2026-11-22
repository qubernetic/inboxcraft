// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use inboxcraft_model::ErrorKind;
use inboxcraft_time::{FakeClock, Timestamp};

use super::Deadline;

#[test]
fn a_deadline_shortens_each_wait_and_ends_with_its_stop() {
    let clock = Arc::new(FakeClock::new(Timestamp::UNIX_EPOCH));
    let deadline = Deadline::new(clock.clone(), Duration::from_secs(10));
    assert_eq!(deadline.wait(Duration::from_secs(15)).unwrap(), Duration::from_secs(10));
    assert_eq!(deadline.sooner(Duration::from_secs(3)).left(), Some(Duration::from_secs(3)));
    assert_eq!(deadline.sooner(Duration::from_secs(30)).left(), Some(Duration::from_secs(10)));
    clock.advance(Duration::from_secs(9));
    assert_eq!(deadline.wait(Duration::from_secs(15)).unwrap(), Duration::from_secs(1));
    clock.advance(Duration::from_secs(1));
    assert_eq!(deadline.left(), None);
    assert_eq!(deadline.wait(Duration::from_secs(15)).unwrap_err().kind(), ErrorKind::TimedOut);

    let stop = Arc::new(AtomicBool::new(false));
    let seen = Arc::clone(&stop);
    let deadline = Deadline::new(clock, Duration::from_secs(10)).with_stop(Arc::new(move || seen.load(Ordering::SeqCst)));
    assert!(deadline.left().is_some());
    stop.store(true, Ordering::SeqCst);
    assert!(deadline.is_stopped() && deadline.left().is_none() && deadline.sooner(Duration::from_secs(1)).left().is_none());
}
