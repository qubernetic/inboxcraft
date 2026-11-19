// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What a command asks the window to show. `ui.settings.open` (`safe`) asks for the settings, on one of
//! [`SETTINGS_PAGES`] when its `page` names one; the session keeps the asks ([`MAX_ASKED`] at most) until the front end
//! takes them ([`crate::Session::take_settings_asked`]). The engine shows nothing itself: headless an ask is answered
//! and goes nowhere.

use serde_json::Value;

/// The settings' pages a command may name, in the order the settings list them. The window's sections carry the
/// same ids (a test of the front end holds the two together); the planned pages are not among them.
pub const SETTINGS_PAGES: [&str; 9] = ["appearance", "accounts", "notifications", "triage", "sending", "calendar", "shortcuts", "privacy", "about"];

/// The longest page, in bytes, looked at: a longer text is no page before it is compared with any.
pub const MAX_PAGE: usize = 16;

/// The most asks kept for the front end; past it the oldest goes.
pub const MAX_ASKED: usize = 8;

/// The window is asked to show the settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SettingsAsked {
    /// The page to show, one of [`SETTINGS_PAGES`]; `None` for the page last shown.
    pub page: Option<&'static str>,
    /// No `page` was given (or `null`): the settings as their key does, shown on the page last shown or, when they
    /// show, the place the owner came from. A `page` that names none is not bare: the settings stay shown.
    pub bare: bool,
}

/// The page `value` names: one of [`SETTINGS_PAGES`] exactly; anything else (no text, a text over [`MAX_PAGE`] bytes,
/// another case, control characters) is no page.
pub(crate) fn page(value: Option<&Value>) -> Option<&'static str> {
    let text = value.and_then(Value::as_str).filter(|text| text.len() <= MAX_PAGE)?;
    SETTINGS_PAGES.into_iter().find(|page| *page == text)
}

#[cfg(test)]
#[path = "shown_tests.rs"]
mod tests;
