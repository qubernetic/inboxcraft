// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-FileCopyrightText: MailCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Messages and their parts (RFC 5322, RFC 2045 and 2046): the tree of multiparts and attached messages.
//!
//! The multipart splitter is adapted from UCS-Care-Group/mailcraft `crates/mime/src/parse.rs` (see
//! `PROVENANCE.md`).

use std::borrow::Cow;

use inboxcraft_charset::Charset;

use crate::header::{clean, line_at, split};
use crate::{ContentType, Disposition, Headers, MimeError, Params, TransferEncoding};

/// The longest message read, in bytes.
pub const MAX_MESSAGE_BYTES: usize = 128 * 1024 * 1024;

/// How deep multiparts and attached messages may be nested: a message of `MAX_DEPTH` multiparts one inside the
/// other is read, one more is [`MimeError::TooDeep`].
pub const MAX_DEPTH: usize = 20;

/// The most parts a message may have, itself and every multipart and attached message counted.
pub const MAX_PARTS: usize = 1000;

/// The longest multipart boundary (RFC 2046 section 5.1.1).
const MAX_BOUNDARY: usize = 70;

/// Reads a message: its header, and the tree of its parts.
///
/// A leading mbox `From ` line is skipped. Nothing is decoded yet: [`Part::decoded`] and [`Part::text`] undo the
/// transfer encoding and the character set of one part when asked.
///
/// # Errors
///
/// [`MimeError::TooLarge`] past [`MAX_MESSAGE_BYTES`]; the header errors of [`Headers::parse`] for any part;
/// [`MimeError::TooDeep`] and [`MimeError::TooManyParts`] past [`MAX_DEPTH`] and [`MAX_PARTS`]; for a multipart
/// [`MimeError::MissingBoundary`], [`MimeError::BadBoundary`] or [`MimeError::BoundaryNotFound`].
pub fn parse(bytes: &[u8]) -> Result<Part<'_>, MimeError> {
    if bytes.len() > MAX_MESSAGE_BYTES {
        return Err(MimeError::TooLarge { limit: MAX_MESSAGE_BYTES });
    }
    let mut count = 0;
    Part::read(bytes, true, 0, ContentType::default(), &mut count)
}

/// A message or one of its parts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Part<'a> {
    headers: Headers,
    content_type: ContentType,
    encoding: TransferEncoding,
    raw: &'a [u8],
    body: Body<'a>,
}

/// What a part holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Body<'a> {
    /// A body of its own: text, a picture, a file ([`Part::decoded`], [`Part::text`]).
    Leaf,
    /// More parts.
    Multipart(Multipart<'a>),
    /// An attached message (`message/rfc822` or `message/global`, in an identity encoding).
    Message(Box<Part<'a>>),
}

/// The parts of a multipart.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Multipart<'a> {
    parts: Vec<Part<'a>>,
    closed: bool,
}

impl<'a> Multipart<'a> {
    /// The parts, in order.
    #[must_use]
    pub fn parts(&self) -> &[Part<'a>] {
        &self.parts
    }

    /// Whether the closing delimiter was there; a message cut short has none, and its last part ends where the
    /// message does.
    #[must_use]
    pub fn closed(&self) -> bool {
        self.closed
    }
}

impl<'a> Part<'a> {
    fn read(bytes: &'a [u8], top: bool, depth: usize, default: ContentType, count: &mut usize) -> Result<Self, MimeError> {
        *count = count.saturating_add(1);
        if *count > MAX_PARTS {
            return Err(MimeError::TooManyParts { limit: MAX_PARTS });
        }
        let (headers, raw) = split(bytes, top)?;
        let content_type = headers.get("content-type").and_then(ContentType::parse).unwrap_or(default);
        let encoding = headers.get("content-transfer-encoding").map(TransferEncoding::parse).unwrap_or_default();
        let identity = matches!(encoding, TransferEncoding::SevenBit | TransferEncoding::EightBit | TransferEncoding::Binary);
        let container = content_type.kind() == "multipart" || (identity && is_message(&content_type));
        if container && depth >= MAX_DEPTH {
            return Err(MimeError::TooDeep { limit: MAX_DEPTH });
        }
        let body = if content_type.kind() == "multipart" {
            let default =
                if content_type.subtype() == "digest" { ContentType::new("message", "rfc822", Params::default()) } else { ContentType::default() };
            let (pieces, closed) = split_multipart(raw, &content_type, MAX_PARTS.saturating_sub(*count))?;
            let parts = pieces
                .into_iter()
                .map(|piece| Self::read(piece, false, depth.saturating_add(1), default.clone(), count))
                .collect::<Result<_, _>>()?;
            Body::Multipart(Multipart { parts, closed })
        } else if container {
            Body::Message(Box::new(Self::read(raw, false, depth.saturating_add(1), ContentType::default(), count)?))
        } else {
            Body::Leaf
        };
        Ok(Self { headers, content_type, encoding, raw, body })
    }

    /// The header fields.
    #[must_use]
    pub fn headers(&self) -> &Headers {
        &self.headers
    }

    /// The media type: the `Content-Type` field, or the default of where the part is (`text/plain`, in a digest
    /// `message/rfc822`) when it has none or one that cannot be read.
    #[must_use]
    pub fn content_type(&self) -> &ContentType {
        &self.content_type
    }

    /// The `Content-Transfer-Encoding`.
    #[must_use]
    pub fn transfer_encoding(&self) -> &TransferEncoding {
        &self.encoding
    }

    /// The `Content-Disposition`, when there is one.
    #[must_use]
    pub fn disposition(&self) -> Option<Disposition> {
        self.headers.get("content-disposition").map(Disposition::parse)
    }

    /// The body as written, transfer encoding not undone.
    #[must_use]
    pub fn raw_body(&self) -> &'a [u8] {
        self.raw
    }

    /// What the part holds.
    #[must_use]
    pub fn body(&self) -> &Body<'a> {
        &self.body
    }

    /// The body with its transfer encoding undone.
    ///
    /// # Errors
    ///
    /// The errors of [`TransferEncoding::decode`].
    pub fn decoded(&self) -> Result<Cow<'a, [u8]>, MimeError> {
        self.encoding.decode(self.raw)
    }

    /// The body as text: transfer encoding undone, then read in the part's `charset`. Without one, or with
    /// `us-ascii`, UTF-8 when the bytes are UTF-8 and windows-1252 when not. A byte order mark at the start goes;
    /// line ends stay as written.
    ///
    /// # Errors
    ///
    /// The errors of [`Part::decoded`], and [`MimeError::Charset`] for a `charset` that cannot be read.
    pub fn text(&self) -> Result<String, MimeError> {
        let bytes = self.decoded()?;
        let label = self.content_type.param("charset").map(str::trim).unwrap_or_default();
        if !(label.is_empty() || label.eq_ignore_ascii_case("us-ascii") || label.eq_ignore_ascii_case("ascii")) {
            return Ok(inboxcraft_charset::decode(label, &bytes)?.text);
        }
        let charset = if std::str::from_utf8(&bytes).is_ok() { Ok(Charset::UTF_8) } else { Charset::for_label("windows-1252") };
        Ok(charset?.decode(&bytes).text)
    }

    /// The file name: the disposition's `filename`, else the content type's `name` ([`Params::text`]).
    #[must_use]
    pub fn file_name(&self) -> Option<String> {
        self.disposition().and_then(|d| d.file_name()).or_else(|| self.content_type.params().text("name")).filter(|name| !name.trim().is_empty())
    }

    /// The `Content-ID`, without its angle brackets, control characters as spaces.
    #[must_use]
    pub fn content_id(&self) -> Option<String> {
        let id = self.headers.get("content-id")?.trim().trim_start_matches('<').trim_end_matches('>').trim();
        (!id.is_empty()).then(|| clean(id))
    }

    /// Whether the part is a file to show as an attachment: one with `Content-Disposition: attachment` or a file
    /// name, or one that is no text body and is not shown inline (no disposition and no `Content-ID`). Multiparts
    /// never are; an attached message is. A `text/plain` or `text/html` part marked `inline` is a body, file name
    /// or not.
    #[must_use]
    pub fn is_attachment(&self) -> bool {
        if matches!(self.body, Body::Multipart(_)) {
            return false;
        }
        let disposition = self.disposition();
        if disposition.as_ref().is_some_and(Disposition::is_inline) && self.is_text_body_type() {
            return false;
        }
        if disposition.as_ref().is_some_and(Disposition::is_attachment) || self.file_name().is_some() {
            return true;
        }
        disposition.is_none() && self.content_id().is_none() && !self.is_text_body_type()
    }

    fn is_text_body_type(&self) -> bool {
        self.content_type.is("text", "plain") || self.content_type.is("text", "html")
    }

    /// The part and every part inside it, depth first, attached messages included.
    #[must_use]
    pub fn parts(&self) -> Vec<&Self> {
        self.walk(true)
    }

    /// The attachments ([`Part::is_attachment`]) of this message, depth first; the parts of an attached message are
    /// not looked into, the attached message is one attachment.
    #[must_use]
    pub fn attachments(&self) -> Vec<&Self> {
        self.walk(false).into_iter().filter(|part| part.is_attachment()).collect()
    }

    /// The first `text/plain` part that is no attachment, outside attached messages.
    #[must_use]
    pub fn text_body(&self) -> Option<&Self> {
        self.body_of("plain")
    }

    /// The first `text/html` part that is no attachment, outside attached messages.
    #[must_use]
    pub fn html_body(&self) -> Option<&Self> {
        self.body_of("html")
    }

    fn body_of(&self, subtype: &str) -> Option<&Self> {
        self.walk(false).into_iter().find(|part| part.content_type.is("text", subtype) && !part.is_attachment())
    }

    /// Depth first, without recursion; into attached messages only when `messages`.
    fn walk(&self, messages: bool) -> Vec<&Self> {
        let mut out = Vec::new();
        let mut stack = vec![self];
        while let Some(part) = stack.pop() {
            out.push(part);
            match &part.body {
                Body::Multipart(multipart) => stack.extend(multipart.parts.iter().rev()),
                Body::Message(inner) if messages => stack.push(inner),
                Body::Message(_) | Body::Leaf => {}
            }
        }
        out
    }
}

fn is_message(content_type: &ContentType) -> bool {
    content_type.is("message", "rfc822") || content_type.is("message", "global")
}

/// The parts of a multipart body: what lies between the delimiter lines, without the line break that belongs to the
/// delimiter. The preamble and the epilogue are not kept. A body cut short gives its last part as it is.
fn split_multipart<'b>(body: &'b [u8], content_type: &ContentType, max: usize) -> Result<(Vec<&'b [u8]>, bool), MimeError> {
    let boundary = content_type.param("boundary").ok_or(MimeError::MissingBoundary)?;
    let valid =
        (1..=MAX_BOUNDARY).contains(&boundary.len()) && boundary.bytes().all(|b| b == b' ' || b.is_ascii_graphic()) && !boundary.ends_with(' ');
    if !valid {
        return Err(MimeError::BadBoundary);
    }
    let delimiter = [b"--", boundary.as_bytes()].concat();
    let mut parts = Vec::new();
    let mut start: Option<usize> = None;
    let mut pos = 0;
    while pos < body.len() {
        let (line, next) = line_at(body, pos);
        if let Some(rest) = line.strip_prefix(delimiter.as_slice()) {
            let (closing, tail) = rest.strip_prefix(b"--").map_or((false, rest), |tail| (true, tail));
            if tail.iter().all(|b| matches!(b, b' ' | b'\t')) {
                if let Some(from) = start.take() {
                    let piece = body.get(from..pos).unwrap_or_default();
                    push(&mut parts, piece.strip_suffix(b"\r\n").or_else(|| piece.strip_suffix(b"\n")).unwrap_or(piece), max)?;
                }
                if closing {
                    return Ok((parts, true));
                }
                start = Some(next);
            }
        }
        pos = next;
    }
    // Without a closing delimiter `start` is set once any delimiter was seen.
    let from = start.ok_or(MimeError::BoundaryNotFound)?;
    push(&mut parts, body.get(from..).unwrap_or_default(), max)?;
    Ok((parts, false))
}

fn push<'b>(parts: &mut Vec<&'b [u8]>, piece: &'b [u8], max: usize) -> Result<(), MimeError> {
    if parts.len() >= max {
        return Err(MimeError::TooManyParts { limit: MAX_PARTS });
    }
    parts.push(piece);
    Ok(())
}

#[cfg(test)]
#[path = "part_tests.rs"]
mod tests;
