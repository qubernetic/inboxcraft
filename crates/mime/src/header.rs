// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-FileCopyrightText: MailCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Header fields (RFC 5322 section 2.2): splitting a header block from its body and unfolding the fields.
//!
//! Adapted from UCS-Care-Group/mailcraft `crates/mime/src/header.rs` (see `PROVENANCE.md`).

use inboxcraft_charset::decode_text;

use crate::MimeError;

/// The most fields one header block may have.
pub const MAX_FIELDS: usize = 2000;

/// The longest a header field may be, unfolded, name and value together, in bytes.
pub const MAX_FIELD_BYTES: usize = 256 * 1024;

/// One header field: its name as written and its value unfolded, white space around it gone, encoded words not
/// decoded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    name: String,
    value: String,
}

impl Field {
    /// The field name as written (`Subject`, `content-type`).
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The value, unfolded: the line breaks of folding are gone, the white space after them is kept, a CR that ends
    /// no line is a space, so no CR or LF is left. Raw 8-bit
    /// bytes are read as UTF-8 (RFC 6532) or, when they are not UTF-8, as windows-1252.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }
}

/// The header fields of a message or of one of its parts, in order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Headers {
    fields: Vec<Field>,
}

impl Headers {
    /// Reads a header block: the fields up to the first empty line, or up to the first line that is no field.
    ///
    /// # Errors
    ///
    /// [`MimeError::TooManyFields`] past [`MAX_FIELDS`], [`MimeError::FieldTooLong`] past [`MAX_FIELD_BYTES`].
    pub fn parse(bytes: &[u8]) -> Result<Self, MimeError> {
        split(bytes, false).map(|(headers, _)| headers)
    }

    /// Every field, in order.
    #[must_use]
    pub fn fields(&self) -> &[Field] {
        &self.fields
    }

    /// The value of the first field called `name`, in any letter case.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.get_all(name).next()
    }

    /// The values of every field called `name`, in any letter case, in order.
    pub fn get_all<'h>(&'h self, name: &str) -> impl Iterator<Item = &'h str> {
        self.fields.iter().filter(move |field| field.name.eq_ignore_ascii_case(name)).map(|field| field.value.as_str())
    }

    /// The first field called `name` as text for people: RFC 2047 encoded words decoded, every control character,
    /// CR, LF, NUL and tab included, a space, and the invisible direction and joiner marks dropped. Broken words
    /// stay as they are written.
    #[must_use]
    pub fn text(&self, name: &str) -> Option<String> {
        self.get(name).map(|value| clean(&decode_text(value).text))
    }
}

/// Every control character as a space, so text from a message cannot break a line or a header, and the invisible
/// direction and joiner marks dropped, so a file name written to read backwards cannot pass for another.
pub(crate) fn clean(text: &str) -> String {
    text.chars().filter(|c| !is_invisible_mark(*c)).map(|c| if c.is_control() { ' ' } else { c }).collect()
}

/// A mark that changes how text runs or joins without showing itself (bidi controls, zero-width characters).
fn is_invisible_mark(c: char) -> bool {
    matches!(c, '\u{061c}' | '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{2069}' | '\u{feff}')
}

/// The line that starts at `pos`, without its line break, and where the next one starts.
pub(crate) fn line_at(bytes: &[u8], pos: usize) -> (&[u8], usize) {
    let rest = bytes.get(pos..).unwrap_or_default();
    let (line, next) = match rest.iter().position(|b| *b == b'\n') {
        Some(i) => (rest.get(..i).unwrap_or_default(), pos.saturating_add(i).saturating_add(1)),
        None => (rest, bytes.len()),
    };
    (line.strip_suffix(b"\r").unwrap_or(line), next)
}

/// Splits a message or a part into its header fields and its body.
///
/// CRLF and bare LF line ends both work. The block ends at the first empty line, or at the first line that cannot
/// be a field (some senders forget the empty line); that line starts the body. At the `top` of a message a leading
/// mbox `From ` line is skipped.
pub(crate) fn split(bytes: &[u8], top: bool) -> Result<(Headers, &[u8]), MimeError> {
    let mut fields = Vec::new();
    let mut current: Option<(String, Vec<u8>)> = None;
    let mut pos = 0;
    let body = loop {
        if pos >= bytes.len() {
            break bytes.len();
        }
        let (line, next) = line_at(bytes, pos);
        if line.is_empty() {
            break next;
        }
        if matches!(line.first(), Some(b' ' | b'\t')) {
            let Some((name, value)) = &mut current else {
                // A continuation of nothing: this is no header block.
                break pos;
            };
            if name.len().saturating_add(value.len()).saturating_add(line.len()) > MAX_FIELD_BYTES {
                return Err(MimeError::FieldTooLong { limit: MAX_FIELD_BYTES });
            }
            value.extend_from_slice(line);
        } else if let Some((name, value)) = field(line) {
            push(&mut fields, current.take())?;
            if name.len().saturating_add(value.len()) > MAX_FIELD_BYTES {
                return Err(MimeError::FieldTooLong { limit: MAX_FIELD_BYTES });
            }
            current = Some((name, value.to_vec()));
        } else if !(top && pos == 0 && (line.starts_with(b"From ") || line.starts_with(b">From "))) {
            break pos;
        }
        pos = next;
    };
    push(&mut fields, current.take())?;
    Ok((Headers { fields }, bytes.get(body..).unwrap_or_default()))
}

fn push(fields: &mut Vec<Field>, current: Option<(String, Vec<u8>)>) -> Result<(), MimeError> {
    if let Some((name, mut value)) = current {
        if fields.len() >= MAX_FIELDS {
            return Err(MimeError::TooManyFields { limit: MAX_FIELDS });
        }
        // A CR that ends no line is no line break; it must not reach a value.
        for byte in value.iter_mut().filter(|b| **b == b'\r') {
            *byte = b' ';
        }
        fields.push(Field { name, value: text_of(value.trim_ascii()) });
    }
    Ok(())
}

/// Bytes from a header as text: UTF-8 when they are, windows-1252 when not.
pub(crate) fn text_of(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_owned(),
        Err(_) => inboxcraft_charset::decode("windows-1252", bytes).map_or_else(|_| String::from_utf8_lossy(bytes).into_owned(), |d| d.text),
    }
}

/// A header line as a name and a raw value, when it is one.
fn field(line: &[u8]) -> Option<(String, &[u8])> {
    let colon = line.iter().position(|b| *b == b':')?;
    let name = line.get(..colon)?.trim_ascii_end();
    let token = !name.is_empty() && name.iter().all(|b| (33..=126).contains(b));
    token.then(|| (String::from_utf8_lossy(name).into_owned(), line.get(colon.saturating_add(1)..).unwrap_or_default()))
}

#[cfg(test)]
#[path = "header_tests.rs"]
mod tests;
