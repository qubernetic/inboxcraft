// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! How long a worker waits before it connects again: doubling from a first wait up to a cap.

use std::time::Duration;

/// The waits between attempts to connect: [`first`](Self::first) after the first failure, twice as long after each
/// further one, never longer than [`cap`](Self::cap).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Backoff {
    /// The wait after the first failed attempt.
    pub first: Duration,
    /// The longest wait.
    pub cap: Duration,
}

impl Default for Backoff {
    /// 2 seconds at first, at most 5 minutes.
    fn default() -> Self {
        Self { first: Duration::from_secs(2), cap: Duration::from_secs(300) }
    }
}

impl Backoff {
    /// The wait after `attempt` failed attempts in a row: none before the first failure.
    ///
    /// ```
    /// use std::time::Duration;
    /// use inboxcraft_net::Backoff;
    ///
    /// let backoff = Backoff { first: Duration::from_secs(2), cap: Duration::from_secs(10) };
    /// let waits: Vec<u64> = (0..6).map(|attempt| backoff.delay(attempt).as_secs()).collect();
    /// assert_eq!(waits, [0, 2, 4, 8, 10, 10]);
    /// ```
    #[must_use]
    pub fn delay(&self, attempt: u32) -> Duration {
        if attempt == 0 {
            return Duration::ZERO;
        }
        let factor = 2_u32.checked_pow(attempt - 1).unwrap_or(u32::MAX);
        self.first.checked_mul(factor).unwrap_or(self.cap).min(self.cap)
    }
}

/// `wait` in whole seconds, rounded up, as [`inboxcraft_model::ConnectionState::BackingOff`] tells it.
pub(crate) fn whole_seconds(wait: Duration) -> u64 {
    wait.as_secs().saturating_add(u64::from(wait.subsec_nanos() > 0))
}

#[cfg(test)]
#[path = "backoff_tests.rs"]
mod tests;
