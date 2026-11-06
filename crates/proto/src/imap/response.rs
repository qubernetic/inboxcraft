// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The kind of a whole response (RFC 9051 section 7): a continuation request, a tagged or untagged status, or data.

use super::capability::is_atom_char;
use super::value::{Piece, Status, StatusLine, Value, values};
use crate::ProtoError;

/// One response from the server.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Response {
    /// `+ ...`: the server is ready for the rest of the command.
    Continue,
    /// `A0001 OK ...`: the end of a command.
    Tagged { tag: String, line: StatusLine },
    /// `* OK ...`, `* BYE ...`, `* PREAUTH ...`.
    Status(StatusLine),
    /// Untagged data: `* 3 EXISTS`, `* LIST ...`, `* 3 FETCH (...)`.
    Data(Vec<Value>),
}

/// The first word of `bytes` and what follows the space after it.
fn split_word(bytes: &[u8]) -> (&[u8], &[u8]) {
    match bytes.iter().position(|b| *b == b' ') {
        Some(at) => (bytes.get(..at).unwrap_or_default(), bytes.get(at.saturating_add(1)..).unwrap_or_default()),
        None => (bytes, &[]),
    }
}

/// A response from its pieces (see `Framer`).
pub(crate) fn classify(mut pieces: Vec<Piece>) -> Result<Response, ProtoError> {
    let Some(Piece::Text(first)) = pieces.first_mut() else { return Err(ProtoError::Malformed("response")) };
    if first.first() == Some(&b'+') {
        return Ok(Response::Continue);
    }
    if let Some(rest) = first.strip_prefix(b"* ") {
        let (word, text) = split_word(rest);
        if let Some(status) = Status::parse(word) {
            return Ok(Response::Status(StatusLine::new(status, &String::from_utf8_lossy(text))));
        }
        *first = rest.to_vec();
        return values(pieces).map(Response::Data);
    }
    let (tag, after) = split_word(first);
    if tag.is_empty() || !tag.iter().all(|b| is_atom_char(*b) && *b != b'+') {
        return Err(ProtoError::Malformed("tag"));
    }
    let (word, text) = split_word(after);
    match Status::parse(word) {
        Some(status @ (Status::Ok | Status::No | Status::Bad)) => {
            Ok(Response::Tagged { tag: String::from_utf8_lossy(tag).into_owned(), line: StatusLine::new(status, &String::from_utf8_lossy(text)) })
        }
        _ => Err(ProtoError::Malformed("tagged response")),
    }
}

#[cfg(test)]
#[path = "response_tests.rs"]
mod tests;
