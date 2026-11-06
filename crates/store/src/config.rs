// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The store's one setting: how long bodies are kept.

use std::time::Duration;

use inboxcraft_time::Timestamp;

/// How the store keeps bodies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoreConfig {
    /// Bodies of messages from the last this many days are kept, per folder; bodies of opened messages are kept
    /// whatever their age.
    pub body_days: u16,
}

impl Default for StoreConfig {
    /// 30 days.
    fn default() -> Self {
        Self { body_days: 30 }
    }
}

impl StoreConfig {
    /// The oldest date whose body is kept at `now`.
    ///
    /// ```
    /// use inboxcraft_store::StoreConfig;
    /// use inboxcraft_time::Timestamp;
    ///
    /// let now = Timestamp::from_unix(40 * 86_400, 0).ok_or("date")?;
    /// assert_eq!(StoreConfig::default().cutoff(now), Timestamp::from_unix(10 * 86_400, 0).ok_or("date")?);
    /// assert_eq!(StoreConfig::default().cutoff(Timestamp::MIN), Timestamp::MIN);
    /// # Ok::<(), &str>(())
    /// ```
    #[must_use]
    pub fn cutoff(&self, now: Timestamp) -> Timestamp {
        let days = Duration::from_secs(u64::from(self.body_days).saturating_mul(86_400));
        now.checked_sub(days).unwrap_or(Timestamp::MIN)
    }
}
