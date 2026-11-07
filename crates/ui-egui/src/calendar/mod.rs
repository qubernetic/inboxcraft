// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The calendar screen's state, behind the rail's Calendar button. The calendars and their events come with calendar
//! sync; until then the screen holds none and shows its empty state. The screen is drawn by `draw/calendar/`.

/// What the calendar screen shows.
#[derive(Debug, Default)]
pub struct Calendar {
    /// The calendars shown, by name; none until calendar sync fills them.
    calendars: Vec<String>,
}

impl Calendar {
    /// The calendars shown, by name.
    #[must_use]
    pub fn calendars(&self) -> &[String] {
        &self.calendars
    }

    /// Whether there is no calendar to show: the screen shows its empty state.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.calendars.is_empty()
    }
}

#[cfg(test)]
mod tests;
