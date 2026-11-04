// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Cleaning one line of text from outside (names, subjects, previews) before it is kept.

/// A mark that changes how text runs or joins without showing itself (bidi controls, zero-width characters).
fn is_invisible_mark(c: char) -> bool {
    matches!(c, '\u{061c}' | '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{2069}' | '\u{feff}')
}

/// `text` as one clean line: every run of white space (CR and LF included) becomes one space, other control
/// characters and the invisible direction and joiner marks are dropped, the ends are trimmed and the line is cut at
/// `max_chars` characters. `None` when nothing is left.
pub(crate) fn clean_line(text: &str, max_chars: usize) -> Option<String> {
    let mut cleaned = String::new();
    let mut kept = 0_usize;
    let mut space = false;
    for c in text.chars() {
        if kept >= max_chars {
            break;
        }
        if c.is_whitespace() {
            space = !cleaned.is_empty();
            continue;
        }
        if c.is_control() || is_invisible_mark(c) {
            continue;
        }
        if space {
            cleaned.push(' ');
            kept = kept.saturating_add(1);
            space = false;
            if kept >= max_chars {
                break;
            }
        }
        cleaned.push(c);
        kept = kept.saturating_add(1);
    }
    let cleaned = cleaned.trim_end().to_owned();
    (!cleaned.is_empty()).then_some(cleaned)
}
