// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A point in time, UTC.

use std::time::Duration;

const NANOS_PER_SEC: u32 = 1_000_000_000;

/// A point in time: seconds and nanoseconds since 1970-01-01T00:00:00Z, without leap seconds (Unix time).
///
/// The range is 0000-01-01T00:00:00Z ([`Timestamp::MIN`]) to 10000-12-31T23:59:59.999999999Z ([`Timestamp::MAX`]),
/// proleptic Gregorian: any local time from year 1 to year 9999, at any offset a mail header can carry, fits.
/// Every constructor and every sum checks the range, so a value outside it never exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp {
    secs: i64,
    nanos: u32,
}

impl Timestamp {
    /// 0000-01-01T00:00:00Z.
    pub const MIN: Self = Self { secs: -62_167_219_200, nanos: 0 };

    /// 10000-12-31T23:59:59.999999999Z.
    pub const MAX: Self = Self { secs: 253_433_923_199, nanos: NANOS_PER_SEC - 1 };

    /// 1970-01-01T00:00:00Z.
    pub const UNIX_EPOCH: Self = Self { secs: 0, nanos: 0 };

    /// The timestamp `seconds` and `nanos` after the Unix epoch (`seconds` may be negative), or `None` outside the
    /// range or when `nanos` is a second or more.
    #[must_use]
    pub const fn from_unix(seconds: i64, nanos: u32) -> Option<Self> {
        if nanos >= NANOS_PER_SEC || seconds < Self::MIN.secs || seconds > Self::MAX.secs {
            return None;
        }
        Some(Self { secs: seconds, nanos })
    }

    /// Whole seconds since the Unix epoch, rounded towards the past.
    #[must_use]
    pub const fn unix_seconds(self) -> i64 {
        self.secs
    }

    /// The nanoseconds past [`Timestamp::unix_seconds`], below one second.
    #[must_use]
    pub const fn subsec_nanos(self) -> u32 {
        self.nanos
    }

    /// `self` moved forwards by `duration`, or `None` past [`Timestamp::MAX`].
    #[must_use]
    pub fn checked_add(self, duration: Duration) -> Option<Self> {
        let secs = i64::try_from(duration.as_secs()).ok()?;
        let mut secs = self.secs.checked_add(secs)?;
        let mut nanos = self.nanos.saturating_add(duration.subsec_nanos());
        if nanos >= NANOS_PER_SEC {
            nanos = nanos.saturating_sub(NANOS_PER_SEC);
            secs = secs.checked_add(1)?;
        }
        Self::from_unix(secs, nanos)
    }

    /// `self` moved forwards by `duration`, stopping at [`Timestamp::MAX`].
    #[must_use]
    pub fn saturating_add(self, duration: Duration) -> Self {
        self.checked_add(duration).unwrap_or(Self::MAX)
    }

    /// `self` moved backwards by `duration`, or `None` before [`Timestamp::MIN`].
    #[must_use]
    pub fn checked_sub(self, duration: Duration) -> Option<Self> {
        let secs = i64::try_from(duration.as_secs()).ok()?;
        let mut secs = self.secs.checked_sub(secs)?;
        let mut nanos = self.nanos;
        if nanos < duration.subsec_nanos() {
            nanos = nanos.saturating_add(NANOS_PER_SEC);
            secs = secs.checked_sub(1)?;
        }
        Self::from_unix(secs, nanos.saturating_sub(duration.subsec_nanos()))
    }

    /// How long after `earlier` `self` is, or `None` when `earlier` is later.
    #[must_use]
    pub fn duration_since(self, earlier: Self) -> Option<Duration> {
        if self < earlier {
            return None;
        }
        // Both are in range, so the difference fits and is not negative.
        let mut secs = self.secs.checked_sub(earlier.secs)?;
        let mut nanos = self.nanos;
        if nanos < earlier.nanos {
            nanos = nanos.saturating_add(NANOS_PER_SEC);
            secs = secs.checked_sub(1)?;
        }
        Some(Duration::new(u64::try_from(secs).ok()?, nanos.saturating_sub(earlier.nanos)))
    }
}

#[cfg(test)]
#[path = "timestamp_tests.rs"]
mod tests;
