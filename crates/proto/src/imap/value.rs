// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-FileCopyrightText: MailCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The values of a response (atoms, strings, literals, parenthesised lists) and status lines.
//!
//! Adapted from UCS-Care-Group/mailcraft `crates/proto/src/imap/parse.rs` (see `PROVENANCE.md`).

use crate::ProtoError;

/// Deepest that parenthesised lists may nest.
pub const MAX_DEPTH: usize = 32;

/// A response as it came off the wire: lines of text with the literals between them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Piece {
    Text(Vec<u8>),
    Literal(Vec<u8>),
}

/// One value of a response.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Value {
    /// A bare word: a number, a flag, `NIL`, `BODY[]`.
    Atom(String),
    /// A quoted string or a literal.
    Str(Vec<u8>),
    List(Vec<Value>),
}

impl Value {
    pub(crate) fn atom(&self) -> Option<&str> {
        match self {
            Self::Atom(atom) => Some(atom),
            _ => None,
        }
    }

    /// A number as IMAP writes one: digits and nothing else, within `u32`.
    pub(crate) fn number(&self) -> Option<u32> {
        number(self.atom()?)
    }

    /// The text of a string or a bare word (what the grammar calls an `astring`).
    pub(crate) fn into_text(self) -> Option<String> {
        match self {
            Self::Atom(atom) => Some(atom),
            Self::Str(bytes) => Some(String::from_utf8_lossy(&bytes).into_owned()),
            Self::List(_) => None,
        }
    }
}

/// A number as IMAP writes one: digits and nothing else, within `u32`.
pub(crate) fn number(text: &str) -> Option<u32> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse().ok()
}

/// Where a line ends in a literal marker (`{123}`, also `{123+}`) outside any quoted string: the marker's position
/// and the size it names. A size beyond `u64` is `u64::MAX`, refused like any other that is too large.
pub(crate) fn literal_marker(line: &[u8]) -> Option<(usize, u64)> {
    let inner_end = line.len().checked_sub(1).filter(|last| line.get(*last) == Some(&b'}'))?;
    let open = line.iter().rposition(|b| *b == b'{')?;
    let inner = line.get(open.saturating_add(1)..inner_end)?;
    let digits = inner.strip_suffix(b"+").unwrap_or(inner);
    if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
        return None;
    }
    // A brace inside a quoted string is just text.
    let (mut quoted, mut escaped) = (false, false);
    for b in line.get(..open)? {
        if escaped {
            escaped = false;
        } else if quoted && *b == b'\\' {
            escaped = true;
        } else if *b == b'"' {
            quoted = !quoted;
        }
    }
    if quoted {
        return None;
    }
    let size = digits.iter().try_fold(0_u64, |n, d| n.checked_mul(10)?.checked_add(u64::from(d.wrapping_sub(b'0')))).unwrap_or(u64::MAX);
    Some((open, size))
}

/// The values of a response. Unbalanced parentheses are forgiven; lists nested deeper than [`MAX_DEPTH`] are not.
pub(crate) fn values(pieces: Vec<Piece>) -> Result<Vec<Value>, ProtoError> {
    // The list being filled is on top; the response itself is at the bottom.
    let mut stack: Vec<Vec<Value>> = vec![Vec::new()];
    let mut pieces = pieces.into_iter().peekable();
    while let Some(piece) = pieces.next() {
        match piece {
            Piece::Literal(bytes) => push(&mut stack, Value::Str(bytes)),
            Piece::Text(text) => {
                // The marker of the literal that follows is not a value.
                let end = match (pieces.peek(), literal_marker(&text)) {
                    (Some(Piece::Literal(_)), Some((at, _))) => at,
                    _ => text.len(),
                };
                scan(text.get(..end).unwrap_or_default(), &mut stack)?;
            }
        }
    }
    while stack.len() > 1 {
        close(&mut stack);
    }
    Ok(stack.pop().unwrap_or_default())
}

fn push(stack: &mut [Vec<Value>], value: Value) {
    if let Some(top) = stack.last_mut() {
        top.push(value);
    }
}

fn close(stack: &mut Vec<Vec<Value>>) {
    if stack.len() > 1
        && let Some(list) = stack.pop()
    {
        push(stack, Value::List(list));
    }
}

fn scan(text: &[u8], stack: &mut Vec<Vec<Value>>) -> Result<(), ProtoError> {
    // A `[` only opens a section (`BODY[HEADER.FIELDS (TO)]`) when a `]` follows somewhere.
    let last_close = text.iter().rposition(|b| *b == b']');
    let mut i = 0;
    // Every arm moves `i` forward.
    while let Some(&b) = text.get(i) {
        match b {
            b' ' | b'\t' => i += 1,
            b'(' => {
                if stack.len() > MAX_DEPTH {
                    return Err(ProtoError::TooDeep { limit: MAX_DEPTH });
                }
                stack.push(Vec::new());
                i += 1;
            }
            b')' => {
                close(stack);
                i += 1;
            }
            b'"' => {
                let (string, next) = quoted(text, i.saturating_add(1));
                push(stack, Value::Str(string));
                i = next;
            }
            _ => {
                let end = atom_end(text, i, last_close);
                push(stack, Value::Atom(String::from_utf8_lossy(text.get(i..end).unwrap_or_default()).into_owned()));
                i = end;
            }
        }
    }
    Ok(())
}

/// The contents of the quoted string that starts at `start` (after its opening quote), and where the text goes on.
/// A string the line ends in the middle of is taken as far as it goes.
fn quoted(text: &[u8], start: usize) -> (Vec<u8>, usize) {
    let mut out = Vec::new();
    let mut i = start;
    while let Some(&b) = text.get(i) {
        match b {
            b'"' => return (out, i.saturating_add(1)),
            b'\\' => {
                out.extend(text.get(i.saturating_add(1)));
                i = i.saturating_add(2);
            }
            _ => {
                out.push(b);
                i += 1;
            }
        }
    }
    (out, i)
}

/// Where the bare word starting at `start` ends: at a space or parenthesis outside `[...]`.
fn atom_end(text: &[u8], start: usize, last_close: Option<usize>) -> usize {
    let mut i = start;
    while let Some(&b) = text.get(i) {
        match b {
            b' ' | b'\t' | b'(' | b')' if i > start => break,
            b'[' if last_close.is_some_and(|close| close > i) => {
                let close = text.get(i..).and_then(|rest| rest.iter().position(|b| *b == b']')).unwrap_or(0);
                i = i.saturating_add(close).saturating_add(1);
            }
            _ => i += 1,
        }
    }
    i
}

/// The word of a status response.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Status {
    Ok,
    No,
    Bad,
    Bye,
    Preauth,
}

impl Status {
    pub(crate) fn parse(word: &[u8]) -> Option<Self> {
        [(b"OK".as_slice(), Self::Ok), (b"NO", Self::No), (b"BAD", Self::Bad), (b"BYE", Self::Bye), (b"PREAUTH", Self::Preauth)]
            .into_iter()
            .find_map(|(name, status)| word.eq_ignore_ascii_case(name).then_some(status))
    }
}

/// `OK [UIDVALIDITY 3857529045] UIDs valid`, tagged or not.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct StatusLine {
    pub(crate) status: Status,
    /// The name of the response code, upper-cased (`UIDVALIDITY`); empty when there is none.
    pub(crate) code: String,
    /// What follows the name inside the brackets.
    pub(crate) arg: String,
    /// The text after the response code.
    pub(crate) text: String,
}

impl StatusLine {
    pub(crate) fn new(status: Status, rest: &str) -> Self {
        let rest = rest.trim();
        let (inner, text) = match rest.strip_prefix('[').and_then(|r| r.split_once(']')) {
            Some((inner, text)) => (inner.trim(), text.trim()),
            None => ("", rest),
        };
        let (code, arg) = inner.split_once(' ').unwrap_or((inner, ""));
        Self { status, code: code.to_ascii_uppercase(), arg: arg.trim().to_owned(), text: text.to_owned() }
    }
}

#[cfg(test)]
#[path = "value_tests.rs"]
mod tests;
