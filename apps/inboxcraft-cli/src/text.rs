// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The text output: an answer as one field a line, nested by two spaces.
//!
//! Subjects, names and bodies are the sender's text. Nothing in them reaches the terminal as a control: control
//! characters (escape sequences, the bell, C1 controls) and the characters that turn the direction of text are
//! written as `\u{..}`; only a tab stays, and a line break starts a new line of a block.

use std::fmt::Write;

use serde_json::Value;

/// How far a nesting level is indented.
const INDENT: usize = 2;

/// `value` as text, ending with a line break.
pub(crate) fn render(value: &Value) -> String {
    let mut out = String::new();
    match value {
        Value::Object(_) | Value::Array(_) => block(&mut out, value, 0),
        scalar => line(&mut out, 0, "", scalar),
    }
    out
}

/// The fields of an object or the items of a list, each on its own line at `depth`.
fn block(out: &mut String, value: &Value, depth: usize) {
    match value {
        Value::Object(fields) => {
            for (key, field) in fields {
                line(out, depth, &format!("{}:", clean(key)), field);
            }
        }
        Value::Array(items) => {
            for item in items {
                line(out, depth, "-", item);
            }
        }
        _ => {}
    }
}

/// `label` and `value` at `depth`: a value on the same line, a nested one on the lines below it.
fn line(out: &mut String, depth: usize, label: &str, value: &Value) {
    let pad = " ".repeat(depth.saturating_mul(INDENT));
    let gap = if label.is_empty() { "" } else { " " };
    let _ = match value {
        Value::Object(fields) if fields.is_empty() => writeln!(out, "{pad}{label}{gap}{{}}"),
        Value::Array(items) if items.is_empty() => writeln!(out, "{pad}{label}{gap}[]"),
        Value::Object(_) | Value::Array(_) => {
            let _ = writeln!(out, "{pad}{label}");
            block(out, value, depth.saturating_add(1));
            Ok(())
        }
        Value::String(text) if text.contains('\n') => {
            let _ = writeln!(out, "{pad}{label}{gap}|");
            let inner = " ".repeat(depth.saturating_add(1).saturating_mul(INDENT));
            for part in text.strip_suffix('\n').unwrap_or(text).split('\n') {
                let _ = writeln!(out, "{inner}{}", clean(part.strip_suffix('\r').unwrap_or(part)));
            }
            Ok(())
        }
        Value::String(text) => writeln!(out, "{pad}{label}{gap}{}", clean(text)),
        scalar => writeln!(out, "{pad}{label}{gap}{scalar}"),
    };
}

/// `text` with every control and direction character but a tab written as `\u{..}`.
fn clean(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if c != '\t' && (c.is_control() || direction(c)) {
            let _ = write!(out, "\\u{{{:x}}}", u32::from(c));
        } else {
            out.push(c);
        }
    }
    out
}

/// The characters that change the direction text is shown in, or hide it: the marks, embeddings, overrides and
/// isolates, and the invisible joiners and byte-order mark.
fn direction(c: char) -> bool {
    matches!(c, '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{2069}' | '\u{feff}')
}

#[cfg(test)]
#[path = "text_tests.rs"]
mod tests;
