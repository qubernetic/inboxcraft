// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A deadline on a clock's monotonic time, with an optional stop: what Detect gives each lookup and each probe, so
//! that the run ends at its cap, or at once when it is stopped.

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use inboxcraft_model::ErrorKind;
use inboxcraft_time::Clock;

use crate::NetError;

/// Asked before each step: `true` ends the work, as a passed deadline does. The app's stop of a run the owner left.
pub type Stop = Arc<dyn Fn() -> bool + Send + Sync>;

/// A moment on `clock`'s monotonic time, and a stop.
#[derive(Clone)]
pub struct Deadline {
    clock: Arc<dyn Clock>,
    at: Duration,
    stop: Option<Stop>,
}

impl fmt::Debug for Deadline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Deadline").field("at", &self.at).field("stop", &self.stop.is_some()).finish_non_exhaustive()
    }
}

impl Deadline {
    /// `within` from now on `clock`.
    #[must_use]
    pub fn new(clock: Arc<dyn Clock>, within: Duration) -> Self {
        let at = clock.monotonic().saturating_add(within);
        Self { clock, at, stop: None }
    }

    /// With `stop` asked before each step.
    #[must_use]
    pub fn with_stop(self, stop: Stop) -> Self {
        Self { stop: Some(stop), ..self }
    }

    /// This deadline, or `within` from now when that comes sooner; the same stop.
    #[must_use]
    pub fn sooner(&self, within: Duration) -> Self {
        let at = self.at.min(self.clock.monotonic().saturating_add(within));
        Self { at, ..self.clone() }
    }

    /// Whether the stop says to end.
    #[must_use]
    pub fn is_stopped(&self) -> bool {
        self.stop.as_ref().is_some_and(|stop| stop())
    }

    /// The time left; `None` once it has passed or the stop says to end.
    #[must_use]
    pub fn left(&self) -> Option<Duration> {
        if self.is_stopped() {
            return None;
        }
        self.at.checked_sub(self.clock.monotonic()).filter(|left| !left.is_zero())
    }

    /// How long one wait may last: `most`, or the time left when that is shorter.
    ///
    /// # Errors
    ///
    /// [`ErrorKind::TimedOut`] once the deadline has passed or the stop says to end.
    pub fn wait(&self, most: Duration) -> Result<Duration, NetError> {
        self.left().map(|left| left.min(most)).ok_or_else(|| NetError::new(ErrorKind::TimedOut, "the time for this step is over"))
    }

    /// The clock it runs on.
    #[must_use]
    pub fn clock(&self) -> &Arc<dyn Clock> {
        &self.clock
    }
}

#[cfg(test)]
#[path = "deadline_tests.rs"]
mod tests;
