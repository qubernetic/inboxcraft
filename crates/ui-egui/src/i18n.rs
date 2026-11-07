// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The text the person reads, through [`tr!`](crate::tr).
//!
//! The message id is the English text itself, as in gettext: `tr!("Search mail")`. Placeholders are named,
//! `tr!("{count} unread", count = 3)`, so a translation may put them in another order. English is the only language
//! so far, so a text is its own translation; the catalogues come with the Hungarian translation. Command ids, the CLI
//! and MCP never see this text.

use std::fmt::{Display, Write as _};

/// The text of `msgid` in the person's language.
#[must_use]
pub fn text(msgid: &'static str) -> &'static str {
    msgid
}

/// The text of `msgid` with each `{name}` replaced by the value of that name. An unknown name or a lone brace stays
/// as written, and a value is never read for placeholders: message text in a value is shown as it is.
#[must_use]
pub fn format(msgid: &'static str, args: &[(&str, &dyn Display)]) -> String {
    let template = text(msgid);
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        let (before, from_brace) = rest.split_at(open);
        out.push_str(before);
        let after = from_brace.get(1..).unwrap_or_default();
        let value = after.find('}').and_then(|close| {
            let name = after.get(..close)?;
            let (_, value) = args.iter().find(|(arg, _)| *arg == name)?;
            Some((close, value))
        });
        match value {
            Some((close, value)) => {
                // Writing into a String cannot fail.
                let _ = write!(out, "{value}");
                rest = after.get(close.saturating_add(1)..).unwrap_or_default();
            }
            None => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// The text of an English message id in the person's language, with its named placeholders filled in.
///
/// ```
/// use inboxcraft_ui_egui::tr;
///
/// assert_eq!(tr!("Inbox"), "Inbox");
/// assert_eq!(tr!("{count} unread", count = 2), "2 unread");
/// ```
#[macro_export]
macro_rules! tr {
    ($msgid:literal) => {
        $crate::i18n::text($msgid)
    };
    ($msgid:literal, $($name:ident = $value:expr),+ $(,)?) => {
        $crate::i18n::format($msgid, &[$((stringify!($name), &$value as &dyn ::std::fmt::Display)),+])
    };
}

#[cfg(test)]
#[path = "i18n_tests.rs"]
mod tests;
