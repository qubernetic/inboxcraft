// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The injectable clock: the computer's clock, and a fake one for tests.

use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
use std::time::{Instant, SystemTime, UNIX_EPOCH};
// `std` has no clock on wasm32-unknown-unknown (it traps); `web-time` reads the browser's.
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
use web_time::{Instant, SystemTime, UNIX_EPOCH};

use crate::Timestamp;

/// Where InboxCraft gets the time from. Code that needs the time takes a clock instead of asking the system, so
/// tests can run it at any hour on a [`FakeClock`].
pub trait Clock: fmt::Debug + Send + Sync {
    /// The wall-clock time. It can jump, forwards or backwards, when the owner or the network sets the clock.
    fn now(&self) -> Timestamp;

    /// The time since the clock was made, for timeouts and back-off. It never goes backwards.
    fn monotonic(&self) -> Duration;
}

/// The computer's clock.
#[derive(Debug, Clone, Copy)]
pub struct SystemClock {
    origin: Instant,
}

impl SystemClock {
    /// A clock whose [`Clock::monotonic`] time starts now.
    #[must_use]
    pub fn new() -> Self {
        #[allow(clippy::disallowed_methods, reason = "the one place that reads the monotonic clock")]
        let origin = Instant::now();
        Self { origin }
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        #[allow(clippy::disallowed_methods, reason = "the one place that reads the wall clock")]
        let now = SystemTime::now();
        since_epoch(now.duration_since(UNIX_EPOCH).map_err(|before| before.duration()))
    }

    fn monotonic(&self) -> Duration {
        self.origin.elapsed()
    }
}

/// The timestamp of a system time given as its distance after (`Ok`) or before (`Err`) the Unix epoch, clamped to
/// the range of [`Timestamp`].
fn since_epoch(distance: Result<Duration, Duration>) -> Timestamp {
    match distance {
        Ok(after) => Timestamp::UNIX_EPOCH.saturating_add(after),
        Err(before) => Timestamp::UNIX_EPOCH.checked_sub(before).unwrap_or(Timestamp::MIN),
    }
}

/// A clock that stands still until it is moved: the only clock tests use.
///
/// Clones share one time, so a test keeps a clone and moves the clock the code under test holds.
#[derive(Debug, Clone)]
pub struct FakeClock {
    state: Arc<Mutex<FakeState>>,
}

#[derive(Debug)]
struct FakeState {
    now: Timestamp,
    monotonic: Duration,
}

impl FakeClock {
    /// A clock showing `now`, with a monotonic time of zero.
    #[must_use]
    pub fn new(now: Timestamp) -> Self {
        Self { state: Arc::new(Mutex::new(FakeState { now, monotonic: Duration::ZERO })) }
    }

    /// Moves the wall clock and the monotonic time forwards by `by`; the wall clock stops at [`Timestamp::MAX`] and
    /// the monotonic time at [`Duration::MAX`].
    pub fn advance(&self, by: Duration) {
        let mut state = self.lock();
        state.now = state.now.saturating_add(by);
        state.monotonic = state.monotonic.saturating_add(by);
    }

    /// Sets the wall clock to `now`, as when the owner sets the computer's clock; the monotonic time stays.
    pub fn set(&self, now: Timestamp) {
        self.lock().now = now;
    }

    fn lock(&self) -> MutexGuard<'_, FakeState> {
        // The state is two plain values that are always valid, so a panic elsewhere cannot leave it broken.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Clock for FakeClock {
    fn now(&self) -> Timestamp {
        self.lock().now
    }

    fn monotonic(&self) -> Duration {
        self.lock().monotonic
    }
}

#[cfg(test)]
#[path = "clock_tests.rs"]
mod tests;
