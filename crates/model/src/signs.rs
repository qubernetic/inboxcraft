// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The signs in a message's header that say what kind of mail it is, for the grouped inbox: list fields (RFC 2369,
//! RFC 2919), `Precedence`, `Auto-Submitted` (RFC 3834), a `text/calendar` media type, a delivery report (a
//! `multipart/report` of RFC 6522 whose `report-type` is `delivery-status`, RFC 3464, or `global-delivery-status`,
//! RFC 6533), and whether the body holds a `text/calendar` part. What they mean for the message's group is the engine's to decide; this only reads them.

use serde::{Deserialize, Serialize};

/// The `report-type` values of a delivery report, in lower case.
const DELIVERY_REPORTS: [&str; 2] = ["delivery-status", "global-delivery-status"];

/// The fields of a mailing list (RFC 2369 and RFC 2919), in lower case.
const LIST_FIELDS: [&str; 7] = ["list-id", "list-unsubscribe", "list-post", "list-help", "list-subscribe", "list-owner", "list-archive"];

/// The `Precedence` field: not in a standard, but list servers and bulk senders write it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Precedence {
    /// `bulk`.
    Bulk,
    /// `list`.
    List,
    /// `junk`, which automatic answers write.
    Junk,
    /// Any other word.
    Other,
}

/// What a message's header says about its kind. The default says nothing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signs {
    /// A list field.
    #[serde(default)]
    list: bool,
    /// `Precedence`.
    #[serde(default)]
    precedence: Option<Precedence>,
    /// `Auto-Submitted` with a value other than `no`.
    #[serde(default)]
    automatic: bool,
    /// The message itself is `text/calendar`.
    #[serde(default)]
    calendar_type: bool,
    /// The kept body holds a `text/calendar` part.
    #[serde(default)]
    calendar: bool,
    /// The message is a delivery report.
    #[serde(default)]
    report: bool,
}

impl Signs {
    /// The signs of a header given as its fields (name, unfolded value), in order. Of a field given more than once
    /// the first counts; a value is its first word (up to a `;` or a comment), told apart without case.
    ///
    /// ```
    /// use inboxcraft_model::{Precedence, Signs};
    ///
    /// let signs = Signs::read([("List-Id", "<news.example.org>"), ("Precedence", "Bulk")]);
    /// assert!(signs.list());
    /// assert_eq!(signs.precedence(), Some(Precedence::Bulk));
    /// ```
    #[must_use]
    pub fn read<'a>(fields: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        let mut signs = Self::default();
        let (mut automatic, mut media_type) = (None, None);
        for (name, value) in fields {
            let name = name.trim();
            if name.eq_ignore_ascii_case("precedence") {
                signs.precedence = signs.precedence.or_else(|| precedence(value));
            } else if name.eq_ignore_ascii_case("auto-submitted") {
                automatic = automatic.or_else(|| word(value).map(|word| !word.eq_ignore_ascii_case("no")));
            } else if name.eq_ignore_ascii_case("content-type") {
                media_type = media_type.or_else(|| word(value).map(|word| (word, value)));
            } else {
                signs.list |= LIST_FIELDS.iter().any(|list| name.eq_ignore_ascii_case(list));
            }
        }
        signs.automatic = automatic.unwrap_or(false);
        signs.calendar_type = media_type.is_some_and(|(word, _)| word.eq_ignore_ascii_case("text/calendar"));
        signs.report = media_type.is_some_and(|(word, value)| word.eq_ignore_ascii_case("multipart/report") && delivery_report(value));
        signs
    }

    /// The same signs, with whether the body holds a `text/calendar` part.
    #[must_use]
    pub fn with_calendar(mut self, calendar: bool) -> Self {
        self.calendar = calendar;
        self
    }

    /// Whether the header has a list field.
    #[must_use]
    pub fn list(&self) -> bool {
        self.list
    }

    /// `Precedence`, when the header has one with a word in it.
    #[must_use]
    pub fn precedence(&self) -> Option<Precedence> {
        self.precedence
    }

    /// Whether `Auto-Submitted` says the message was sent by a program (any value but `no`).
    #[must_use]
    pub fn automatic(&self) -> bool {
        self.automatic
    }

    /// Whether the message itself is `text/calendar` (its `Content-Type`).
    #[must_use]
    pub fn calendar_type(&self) -> bool {
        self.calendar_type
    }

    /// Whether the message is a delivery report: `multipart/report` with a `report-type` of `delivery-status` or
    /// `global-delivery-status`.
    ///
    /// ```
    /// use inboxcraft_model::Signs;
    ///
    /// assert!(Signs::read([("Content-Type", "multipart/report; report-type=delivery-status")]).report());
    /// assert!(!Signs::read([("Content-Type", "multipart/report; report-type=disposition-notification")]).report());
    /// ```
    #[must_use]
    pub fn report(&self) -> bool {
        self.report
    }

    /// Whether the body holds a `text/calendar` part; known once the body is kept.
    #[must_use]
    pub fn calendar(&self) -> bool {
        self.calendar
    }

    /// Whether nothing is known.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// The `Precedence` of `value`; `None` when it holds no word.
fn precedence(value: &str) -> Option<Precedence> {
    let word = word(value)?;
    let known = [("bulk", Precedence::Bulk), ("list", Precedence::List), ("junk", Precedence::Junk)];
    Some(known.into_iter().find(|(name, _)| word.eq_ignore_ascii_case(name)).map_or(Precedence::Other, |(_, precedence)| precedence))
}

/// Whether the `Content-Type` value `value` has a `report-type` parameter of a delivery report.
fn delivery_report(value: &str) -> bool {
    value.split(';').skip(1).filter_map(|parameter| parameter.split_once('=')).any(|(name, kind)| {
        name.trim().eq_ignore_ascii_case("report-type")
            && DELIVERY_REPORTS.iter().any(|report| kind.trim().trim_matches('"').eq_ignore_ascii_case(report))
    })
}

/// The first word of a field value: up to a `;` or a comment, white space around it gone; `None` when nothing is
/// left.
fn word(value: &str) -> Option<&str> {
    let end = value.find([';', '(']).unwrap_or(value.len());
    Some(value.get(..end)?.trim()).filter(|word| !word.is_empty())
}

#[cfg(test)]
#[path = "signs_tests.rs"]
mod tests;
