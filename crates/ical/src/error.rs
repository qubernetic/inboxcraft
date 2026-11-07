// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The crate's error type.

use std::fmt;

/// The longest property name an error repeats; a longer one is cut.
pub const MAX_NAME_IN_ERROR: usize = 64;

/// Why a calendar, a value or a rule cannot be read or written.
///
/// Every limit is part of the error, so the caller can say what was too much. Names taken from the input are capped
/// at [`MAX_NAME_IN_ERROR`] characters and hold no control character.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum IcalError {
    /// The input is longer than `limit` bytes.
    TooLarge {
        /// The cap, [`crate::MAX_BYTES`].
        limit: usize,
    },
    /// The input has more than `limit` properties and components together.
    TooManyItems {
        /// The cap, [`crate::MAX_ITEMS`].
        limit: usize,
    },
    /// Components are nested deeper than `limit`.
    TooDeep {
        /// The cap, [`crate::MAX_DEPTH`].
        limit: usize,
    },
    /// Content line `line` (from 1, after unfolding) is no `name *(";" param) ":" value`.
    BadLine {
        /// The line, counted after unfolding and without the empty lines.
        line: usize,
    },
    /// Content line `line` ends a component that is not open, or a property stands outside every component.
    Unbalanced {
        /// The line, counted as in [`IcalError::BadLine`].
        line: usize,
    },
    /// A component is not ended before the input ends.
    Unterminated,
    /// The input holds no component, more than one at the top, or not the one asked for (`VCALENDAR`).
    NotACalendar,
    /// A property's value cannot be read as its type.
    BadValue {
        /// The property's name.
        property: String,
    },
    /// A component lacks a property it must have.
    Missing {
        /// The component's name.
        component: &'static str,
        /// The property's name.
        property: &'static str,
    },
    /// A name to be written is no `iana-token` or `x-name` (letters, digits and hyphens).
    BadName {
        /// The name.
        name: String,
    },
}

impl IcalError {
    /// [`IcalError::BadValue`] for `property`, its name capped and cleaned.
    pub(crate) fn bad_value(property: &str) -> Self {
        Self::BadValue { property: clean(property) }
    }

    /// [`IcalError::BadName`] for `name`, capped and cleaned.
    pub(crate) fn bad_name(name: &str) -> Self {
        Self::BadName { name: clean(name) }
    }
}

/// `text` without control characters, at most [`MAX_NAME_IN_ERROR`] characters.
fn clean(text: &str) -> String {
    text.chars().filter(|c| !c.is_control()).take(MAX_NAME_IN_ERROR).collect()
}

impl fmt::Display for IcalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { limit } => write!(f, "the calendar is longer than {limit} bytes"),
            Self::TooManyItems { limit } => write!(f, "the calendar has more than {limit} properties and components"),
            Self::TooDeep { limit } => write!(f, "components are nested deeper than {limit}"),
            Self::BadLine { line } => write!(f, "content line {line} cannot be read"),
            Self::Unbalanced { line } => write!(f, "content line {line} ends no open component"),
            Self::Unterminated => f.write_str("a component is not ended"),
            Self::NotACalendar => f.write_str("the input holds no single calendar"),
            Self::BadValue { property } => write!(f, "the value of {property} cannot be read"),
            Self::Missing { component, property } => write!(f, "{component} has no {property}"),
            Self::BadName { name } => write!(f, "{name:?} is no property or component name"),
        }
    }
}

impl std::error::Error for IcalError {}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
