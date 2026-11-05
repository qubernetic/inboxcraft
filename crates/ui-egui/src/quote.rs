// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Folded quotes: a plain-text body split into what the sender wrote and what they quoted.
//!
//! A quote is a run of lines starting with `>` (blank lines inside it included), shown without one level of `>`; the
//! line just before it (at most one blank line away) that ends with a colon, such as "On Thursday, Odile wrote:", is
//! its attribution and folds with it. A line `-----Original Message-----` folds everything after it. The text is the
//! sender's: control characters other than the line break and the tab, and the bidi controls that could reorder what
//! is shown, are removed, and it is cut at [`MAX_TEXT`] bytes and [`MAX_SEGMENTS`] segments.

/// The most bytes of a body the reader shows.
pub const MAX_TEXT: usize = 256 * 1024;

/// The most segments a body is split into; what is left shows as text.
pub const MAX_SEGMENTS: usize = 200;

/// The longest attribution line, in characters.
const MAX_ATTRIBUTION: usize = 200;

/// The line Outlook and others put above a quoted message.
const ORIGINAL: &str = "-----Original Message-----";

/// A piece of a plain-text body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    /// What the sender wrote.
    Text(String),
    /// What they quoted, folded until the owner opens it.
    Quote {
        /// The line that says whom it quotes.
        attribution: Option<String>,
        /// The quoted text without one level of `>`.
        text: String,
        /// How many lines it has.
        lines: usize,
    },
}

/// `text` without the characters that must not reach the screen: control characters other than the line break and
/// the tab (a carriage return included), and the bidi controls.
pub(crate) fn clean(text: &str) -> String {
    text.chars()
        .filter(|c| !matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{200E}' | '\u{200F}' | '\u{061C}'))
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect()
}

/// The first `max` bytes of `text`, cut back to a character boundary.
pub(crate) fn cut(text: &str, max: usize) -> &str {
    let mut end = max.min(text.len());
    while !text.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    text.get(..end).unwrap_or_default()
}

fn is_quoted(line: &str) -> bool {
    line.trim_start().starts_with('>')
}

fn is_blank(line: &str) -> bool {
    line.trim().is_empty()
}

/// A quoted line without one level of `>` and the space after it; a blank line inside a quote is empty.
fn unquote(line: &str) -> &str {
    match line.trim_start().strip_prefix('>') {
        Some(rest) => rest.strip_prefix(' ').unwrap_or(rest),
        None => "",
    }
}

/// `lines` without blank lines at either end.
fn trimmed<'a>(lines: &'a [&'a str]) -> &'a [&'a str] {
    let start = lines.iter().position(|line| !is_blank(line)).unwrap_or(lines.len());
    let end = lines.iter().rposition(|line| !is_blank(line)).map_or(start, |end| end.saturating_add(1)).max(start);
    lines.get(start..end).unwrap_or_default()
}

/// Ends `pending` as a text segment, if it holds more than blank lines.
fn flush(segments: &mut Vec<Segment>, pending: &mut Vec<&str>) {
    let lines = trimmed(pending);
    if !lines.is_empty() {
        segments.push(Segment::Text(lines.join("\n")));
    }
    pending.clear();
}

/// Takes the attribution off the end of `pending`: its last line that is not blank, at most one blank line before the
/// quote, when it ends with a colon and is not too long.
fn attribution(pending: &mut Vec<&str>) -> Option<String> {
    let blanks = pending.iter().rev().take_while(|line| is_blank(line)).count();
    if blanks > 1 {
        return None;
    }
    let at = pending.len().checked_sub(blanks.saturating_add(1))?;
    let line = pending.get(at)?.trim();
    if !line.ends_with(':') || line.chars().count() > MAX_ATTRIBUTION {
        return None;
    }
    let line = line.to_owned();
    pending.truncate(at);
    Some(line)
}

/// `text` split into what the sender wrote and the quotes, in order.
#[must_use]
pub fn split(text: &str) -> Vec<Segment> {
    let cleaned = clean(cut(text, MAX_TEXT));
    let lines: Vec<&str> = cleaned.split('\n').collect();
    let mut segments = Vec::new();
    let mut pending: Vec<&str> = Vec::new();
    let mut at = 0;
    while let Some(line) = lines.get(at) {
        if segments.len() >= MAX_SEGMENTS.saturating_sub(2) {
            // Room for the text before and the rest after: shown as it is.
            pending.extend(lines.get(at..).unwrap_or_default());
            break;
        }
        if line.trim().eq_ignore_ascii_case(ORIGINAL) {
            flush(&mut segments, &mut pending);
            let rest = trimmed(lines.get(at.saturating_add(1)..).unwrap_or_default());
            segments.push(Segment::Quote { attribution: Some(ORIGINAL.to_owned()), text: rest.join("\n"), lines: rest.len() });
            break;
        }
        if !is_quoted(line) {
            pending.push(line);
            at = at.saturating_add(1);
            continue;
        }
        let mut end = at;
        for (probe, next) in lines.iter().enumerate().skip(at) {
            if is_quoted(next) {
                end = probe;
            } else if !is_blank(next) {
                break;
            }
        }
        let block = lines.get(at..=end).unwrap_or_default();
        let attribution = attribution(&mut pending);
        flush(&mut segments, &mut pending);
        segments.push(Segment::Quote {
            attribution,
            text: block.iter().map(|line| unquote(line)).collect::<Vec<_>>().join("\n"),
            lines: block.len(),
        });
        at = end.saturating_add(1);
    }
    flush(&mut segments, &mut pending);
    segments
}

#[cfg(test)]
#[path = "quote_tests.rs"]
mod tests;
