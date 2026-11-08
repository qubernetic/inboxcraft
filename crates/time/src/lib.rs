// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Clocks, dates and time zones, the same on native and wasm.
//!
//! Layer 0 (see `docs/architecture.md`).
//!
//! - [`Clock`] is the one way InboxCraft asks for the time. [`SystemClock`] reads the computer's clock;
//!   [`FakeClock`] stands still until a test moves it, and is the only clock tests use. `std::time::Instant::now`
//!   and `SystemTime::now` are banned everywhere else (`clippy.toml`): they trap on wasm and make tests depend on
//!   the hour they run.
//! - [`Timestamp`] is a point in time, UTC, from year 0 to year 10000. [`CivilDateTime`] is what a wall clock shows at
//!   an [`Offset`] from UTC.
//! - [`Offsets`] is the computer's offset over time, summer time included: a fixed [`Offset`], or an
//!   [`OffsetTable`] of changes. [`Offsets::resolve`] finds the moment a wall-clock time stands for across a change.
//! - [`MailDate`] reads and writes the dates of mail headers (RFC 5322), obsolete forms included.
//!
//! ```
//! use std::time::Duration;
//! use inboxcraft_time::{Clock, FakeClock, Timestamp};
//!
//! let clock = FakeClock::new(Timestamp::UNIX_EPOCH);
//! clock.advance(Duration::from_secs(90));
//! assert_eq!(clock.now().unix_seconds(), 90);
//! ```

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod civil;
mod clock;
mod rfc5322;
mod timestamp;
mod zone;

pub use civil::{CivilDateTime, Offset, Weekday, days_in_month, is_leap_year};
pub use clock::{Clock, FakeClock, SystemClock};
pub use rfc5322::{DateError, MAX_COMMENT_DEPTH, MAX_LEN, MailDate, Zone};
pub use timestamp::Timestamp;
pub use zone::{OffsetTable, Offsets};
