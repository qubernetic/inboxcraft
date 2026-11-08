// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Building messages (RFC 5322, RFC 2045 to 2049): header fields, text and HTML bodies, attachments, multiparts.
//!
//! The header injection guard lives here. Every header value is checked before it is written: text for people
//! (subject, display names, file names, the values of extra fields) loses CR, LF and NUL, and values with a grammar
//! of their own (addresses, message ids, field names, media types) are refused when they do not fit it, so no value
//! can end a line or start another field.

use inboxcraft_charset::encode_text;
use inboxcraft_time::MailDate;

use crate::encode::{b_words, base64_encode, crlf, fold, percent_pieces, qp_encode, strip_breaks};
use crate::header::is_invisible_mark;
use crate::params::is_token;
use crate::{MAX_MESSAGE_BYTES, MimeError};

/// The longest message id, angle brackets not counted.
const MAX_ID: usize = 250;

/// The longest name of an extra field (RFC 5322 section 2.1.1 keeps the whole line within 78).
const MAX_FIELD_NAME: usize = 76;

/// The longest type or subtype name of a media type (RFC 6838 section 4.2).
const MAX_MEDIA_NAME: usize = 127;

/// The longest word of a field value that is still folded as it is; an encoded word is at most 75.
const MAX_FOLDED_WORD: usize = 76;

/// The longest line of a body in `7bit` (RFC 5322 section 2.1.1), line break not counted.
const MAX_LINE: usize = 998;

/// The longest word of a display name written as a quoted string; a longer one is written as encoded words, which
/// can be folded.
const MAX_QUOTED_WORD: usize = 70;

/// Fields the builder writes itself; [`MessageBuilder::header`] refuses them, and every `Content-` field.
const OWN_FIELDS: [&str; 12] =
    ["date", "from", "sender", "reply-to", "to", "cc", "bcc", "subject", "message-id", "in-reply-to", "references", "mime-version"];

/// A person or a list: an address and, if wanted, a display name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mailbox {
    name: Option<String>,
    address: String,
}

impl Mailbox {
    /// An address without a display name. Nothing is checked yet: [`MessageBuilder::build`] refuses an address
    /// outside the RFC 5322 grammar.
    #[must_use]
    pub fn new(address: &str) -> Self {
        Self { name: None, address: address.to_owned() }
    }

    /// An address with a display name. The name is text: CR, LF and NUL are dropped when it is written.
    #[must_use]
    pub fn named(name: &str, address: &str) -> Self {
        Self { name: Some(name.to_owned()), address: address.to_owned() }
    }
}

/// A file to attach: its name, its media type and its bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attachment {
    file_name: String,
    media_type: String,
    data: Vec<u8>,
}

impl Attachment {
    /// A file called `file_name` (text; CR, LF and NUL are dropped, an empty name is none) of the media type
    /// `media_type` (`type/subtype`, checked by [`MessageBuilder::build`]).
    #[must_use]
    pub fn new(file_name: &str, media_type: &str, data: Vec<u8>) -> Self {
        Self { file_name: file_name.to_owned(), media_type: media_type.to_owned(), data }
    }
}

/// Builds a message, field by field and part by part; [`MessageBuilder::build`] checks every value and writes it.
///
/// The fields come out in a fixed order: `Date`, `From`, `Sender`, `Reply-To`, `To`, `Cc`, `Subject`, `Message-ID`,
/// `In-Reply-To`, `References`, the extra fields in the order given, then `MIME-Version` and the content fields. A
/// field that was not set is not written; nothing is required, so a draft can be built too. Blind copies are no
/// field: they belong to the SMTP envelope.
///
/// The body is the text alone (`text/plain; charset=utf-8`), the HTML alone, or both as `multipart/alternative`;
/// with attachments that goes first in a `multipart/mixed`. Text is `7bit` when it is printable ASCII in lines of at
/// most 998 characters without white space at their end, else `quoted-printable`; line ends become CRLF. Attachments are `base64` with
/// `Content-Disposition: attachment` and their file name, in RFC 2231 form when it is not plain ASCII. The message
/// is 7-bit ASCII throughout, but for addresses outside ASCII, which are written as given (RFC 6532).
///
/// ```
/// use inboxcraft_mime::{Attachment, Mailbox, MessageBuilder, MimeError, parse};
///
/// let bytes = MessageBuilder::new()
///     .from(Mailbox::named("Anna Kovács", "anna@example.org"))
///     .to(Mailbox::new("bela@example.org"))
///     .subject("Képek\r\nBcc: evil@example.org")
///     .text("Szia!\n")
///     .attach(Attachment::new("kép.png", "image/png", b"\x89PNG".to_vec()))
///     .build()?;
/// let message = parse(&bytes)?;
/// assert_eq!(message.headers().text("subject").as_deref(), Some("KépekBcc: evil@example.org"));
/// assert_eq!(message.headers().get("bcc"), None);
/// assert_eq!(message.text_body().map(|part| part.text()).transpose()?.as_deref(), Some("Szia!\r\n"));
/// assert_eq!(message.attachments()[0].file_name().as_deref(), Some("kép.png"));
///
/// let evil = MessageBuilder::new().to(Mailbox::new("bela@example.org\r\nBcc: evil@example.org")).build();
/// assert_eq!(evil, Err(MimeError::BadHeader("address")));
/// # Ok::<(), MimeError>(())
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MessageBuilder {
    date: Option<MailDate>,
    from: Vec<Mailbox>,
    sender: Option<Mailbox>,
    reply_to: Vec<Mailbox>,
    to: Vec<Mailbox>,
    cc: Vec<Mailbox>,
    subject: Option<String>,
    message_id: Option<String>,
    in_reply_to: Vec<String>,
    references: Vec<String>,
    extra: Vec<(String, String)>,
    text: Option<String>,
    html: Option<String>,
    calendar: Option<(String, String)>,
    attachments: Vec<Attachment>,
}

impl MessageBuilder {
    /// A builder with nothing set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The `Date`.
    #[must_use]
    pub fn date(mut self, date: MailDate) -> Self {
        self.date = Some(date);
        self
    }

    /// Adds an author to `From`.
    #[must_use]
    pub fn from(mut self, mailbox: Mailbox) -> Self {
        self.from.push(mailbox);
        self
    }

    /// The `Sender`, who sent the message for the authors.
    #[must_use]
    pub fn sender(mut self, mailbox: Mailbox) -> Self {
        self.sender = Some(mailbox);
        self
    }

    /// Adds an address to `Reply-To`.
    #[must_use]
    pub fn reply_to(mut self, mailbox: Mailbox) -> Self {
        self.reply_to.push(mailbox);
        self
    }

    /// Adds a recipient to `To`.
    #[must_use]
    pub fn to(mut self, mailbox: Mailbox) -> Self {
        self.to.push(mailbox);
        self
    }

    /// Adds a recipient to `Cc`.
    #[must_use]
    pub fn cc(mut self, mailbox: Mailbox) -> Self {
        self.cc.push(mailbox);
        self
    }

    /// The `Subject`: text, written as encoded words where it needs them, CR, LF and NUL dropped, white space around
    /// it trimmed.
    #[must_use]
    pub fn subject(mut self, subject: &str) -> Self {
        self.subject = Some(subject.to_owned());
        self
    }

    /// The `Message-ID`, with or without its angle brackets.
    #[must_use]
    pub fn message_id(mut self, id: &str) -> Self {
        self.message_id = Some(id.to_owned());
        self
    }

    /// Adds a message id to `In-Reply-To`.
    #[must_use]
    pub fn in_reply_to(mut self, id: &str) -> Self {
        self.in_reply_to.push(id.to_owned());
        self
    }

    /// Adds a message id to `References`.
    #[must_use]
    pub fn reference(mut self, id: &str) -> Self {
        self.references.push(id.to_owned());
        self
    }

    /// Adds an extra field: `name` must be a field name the builder does not write itself (no `Content-` field
    /// either), `value` is text as for [`MessageBuilder::subject`].
    #[must_use]
    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.extra.push((name.to_owned(), value.to_owned()));
        self
    }

    /// The text body.
    #[must_use]
    pub fn text(mut self, text: &str) -> Self {
        self.text = Some(text.to_owned());
        self
    }

    /// The HTML body.
    #[must_use]
    pub fn html(mut self, html: &str) -> Self {
        self.html = Some(html.to_owned());
        self
    }

    /// A calendar object (iMIP, RFC 6047) beside the text and the HTML, as `text/calendar` whose `method` is
    /// `method`, the `METHOD` the object names.
    #[must_use]
    pub fn calendar(mut self, method: &str, calendar: &str) -> Self {
        self.calendar = Some((method.to_owned(), calendar.to_owned()));
        self
    }

    /// Adds an attachment.
    #[must_use]
    pub fn attach(mut self, attachment: Attachment) -> Self {
        self.attachments.push(attachment);
        self
    }

    /// Checks every value and writes the message, CRLF line ends throughout.
    ///
    /// # Errors
    ///
    /// [`MimeError::BadHeader`] naming the kind of value that does not fit its grammar: `"address"` (RFC 5322
    /// `addr-spec`, at most 254 bytes), `"message id"` (`msg-id`, ASCII, at most 250 bytes), `"field name"` (printable
    /// ASCII without `:`, at most 76 characters, none the builder writes itself) or `"media type"` (`type/subtype`
    /// of RFC 2045 tokens of at most 127 characters, no multipart, no `message` type but `message/global`) or
    /// `"calendar method"` (an RFC 2045 token).
    /// [`MimeError::FieldTooLong`] and [`MimeError::TooLarge`] when the reader
    /// could not read the result back ([`crate::MAX_FIELD_BYTES`], [`MAX_MESSAGE_BYTES`]).
    pub fn build(&self) -> Result<Vec<u8>, MimeError> {
        let attached = self.attachments.iter().fold(0usize, |sum, a| sum.saturating_add(a.data.len().div_ceil(3).saturating_mul(4)));
        if attached > MAX_MESSAGE_BYTES {
            return Err(MimeError::TooLarge { limit: MAX_MESSAGE_BYTES });
        }
        let mut head = Vec::new();
        if let Some(date) = self.date {
            head.extend(fold("Date", &date.to_string())?);
        }
        address_field(&mut head, "From", &self.from)?;
        address_field(&mut head, "Sender", self.sender.as_slice())?;
        address_field(&mut head, "Reply-To", &self.reply_to)?;
        address_field(&mut head, "To", &self.to)?;
        address_field(&mut head, "Cc", &self.cc)?;
        if let Some(subject) = &self.subject {
            head.extend(fold("Subject", &unstructured(subject))?);
        }
        id_field(&mut head, "Message-ID", self.message_id.as_slice())?;
        id_field(&mut head, "In-Reply-To", &self.in_reply_to)?;
        id_field(&mut head, "References", &self.references)?;
        for (name, value) in &self.extra {
            head.extend(fold(&field_name(name)?, &unstructured(value))?);
        }
        head.extend_from_slice(b"MIME-Version: 1.0\r\n");
        within_limit(write(self.body()?, 0, head)?)
    }

    /// The tree of parts.
    fn body(&self) -> Result<Node, MimeError> {
        let text = self.text.as_deref().map(|text| text_part("plain", text));
        let html = self.html.as_deref().map(|html| text_part("html", html));
        let calendar = match &self.calendar {
            Some((method, _)) if !is_token(method) || method.len() > MAX_MEDIA_NAME => return Err(MimeError::BadHeader("calendar method")),
            Some((method, calendar)) => Some(text_part(&format!("calendar; method={method}"), calendar)),
            None => None,
        };
        let mut alternatives: Vec<Node> = [text, html, calendar].into_iter().flatten().collect();
        let main = if alternatives.len() > 1 { Some(Node::Multipart("alternative", alternatives)) } else { alternatives.pop() };
        if self.attachments.is_empty() {
            return Ok(main.unwrap_or_else(|| text_part("plain", "")));
        }
        let mut parts: Vec<Node> = main.into_iter().collect();
        for attachment in &self.attachments {
            parts.push(attachment_part(attachment)?);
        }
        Ok(Node::Multipart("mixed", parts))
    }
}

/// The message, when the reader takes it: at most [`MAX_MESSAGE_BYTES`].
fn within_limit(message: Vec<u8>) -> Result<Vec<u8>, MimeError> {
    if message.len() > MAX_MESSAGE_BYTES { Err(MimeError::TooLarge { limit: MAX_MESSAGE_BYTES }) } else { Ok(message) }
}

/// A part before it is written: its content fields and encoded body, or a multipart.
enum Node {
    Leaf(Vec<u8>, Vec<u8>),
    Multipart(&'static str, Vec<Self>),
}

/// Writes a part after the fields in `head`. A multipart's boundary is the first `=_inboxcraft_<depth>_<n>` that
/// is in none of its parts; `=_` cannot appear in base64 or quoted-printable, so the first one nearly always is.
fn write(node: Node, depth: usize, mut head: Vec<u8>) -> Result<Vec<u8>, MimeError> {
    match node {
        Node::Leaf(fields, body) => {
            head.extend(fields);
            head.extend_from_slice(b"\r\n");
            head.extend(body);
        }
        Node::Multipart(subtype, nodes) => {
            let parts = nodes.into_iter().map(|node| write(node, depth.saturating_add(1), Vec::new())).collect::<Result<Vec<_>, _>>()?;
            let mut n = 0usize;
            let boundary = loop {
                let boundary = format!("=_inboxcraft_{depth}_{n}");
                let delimiter = format!("--{boundary}");
                if !parts.iter().any(|part| part.windows(delimiter.len()).any(|w| w == delimiter.as_bytes())) {
                    break boundary;
                }
                n = n.saturating_add(1);
            };
            head.extend(fold("Content-Type", &format!("multipart/{subtype}; boundary=\"{boundary}\""))?);
            head.extend_from_slice(b"\r\n");
            for part in parts {
                head.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
                head.extend(part);
                head.extend_from_slice(b"\r\n");
            }
            head.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
        }
    }
    Ok(head)
}

/// A `text/<subtype>` part (the subtype with any parameters before the charset) in UTF-8: `7bit` when it can be, else quoted-printable. White space at the end of a
/// line makes it quoted-printable too, as mail servers may drop it.
fn text_part(subtype: &str, text: &str) -> Node {
    let text = crlf(text);
    let plain = text.bytes().all(|b| b.is_ascii_graphic() || matches!(b, b' ' | b'\t' | b'\r' | b'\n'))
        && text.split("\r\n").all(|line| line.len() <= MAX_LINE && !line.ends_with([' ', '\t']));
    let (encoding, body) = if plain { ("7bit", text.into_bytes()) } else { ("quoted-printable", qp_encode(text.as_bytes())) };
    let fields = format!("Content-Type: text/{subtype}; charset=utf-8\r\nContent-Transfer-Encoding: {encoding}\r\n");
    Node::Leaf(fields.into_bytes(), body)
}

fn attachment_part(attachment: &Attachment) -> Result<Node, MimeError> {
    let media_type = media_type(&attachment.media_type)?;
    let mut fields = fold("Content-Type", &media_type)?;
    fields.extend_from_slice(b"Content-Transfer-Encoding: base64\r\n");
    let name = strip_breaks(&attachment.file_name);
    let disposition = if name.is_empty() { "attachment".to_owned() } else { format!("attachment; {}", file_name_param(&name)) };
    fields.extend(fold("Content-Disposition", &disposition)?);
    Ok(Node::Leaf(fields, base64_encode(&attachment.data)))
}

/// The `filename` parameter: quoted when the name is short printable ASCII that cannot be taken for an encoded
/// word, else in RFC 2231 pieces of percent-encoded UTF-8.
fn file_name_param(name: &str) -> String {
    let plain = name.len() <= MAX_QUOTED_WORD && name.bytes().all(|b| b.is_ascii_graphic() || b == b' ') && !name.contains("=?");
    if plain {
        return format!("filename={}", quoted(name));
    }
    let pieces: Vec<String> = percent_pieces(name)
        .into_iter()
        .enumerate()
        .map(|(i, piece)| if i == 0 { format!("filename*0*=utf-8''{piece}") } else { format!("filename*{i}*={piece}") })
        .collect();
    pieces.join("; ")
}

/// A quoted string, `"` and `\` escaped.
fn quoted(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Text for a field: CR, LF and NUL dropped, then encoded words where needed. A text with a word too long to fold
/// is written as encoded words throughout, which can be folded anywhere.
fn unstructured(text: &str) -> String {
    let text = strip_breaks(text);
    let encoded = encode_text(&text);
    if encoded.split(' ').any(|word| word.len() > MAX_FOLDED_WORD) { b_words(&text).join(" ") } else { encoded }
}

/// Writes a field of mailboxes, when there are any.
fn address_field(head: &mut Vec<u8>, name: &str, mailboxes: &[Mailbox]) -> Result<(), MimeError> {
    if mailboxes.is_empty() {
        return Ok(());
    }
    let written = mailboxes.iter().map(mailbox).collect::<Result<Vec<_>, _>>()?;
    head.extend(fold(name, &written.join(", "))?);
    Ok(())
}

/// A mailbox as `name <address>`, or the address alone. A display name of short printable ASCII words is a quoted
/// string; any other is encoded words.
fn mailbox(mailbox: &Mailbox) -> Result<String, MimeError> {
    let address = address(&mailbox.address)?;
    let name = mailbox.name.as_deref().map(strip_breaks).unwrap_or_default();
    if name.is_empty() {
        return Ok(address.to_owned());
    }
    let plain =
        name.bytes().all(|b| b.is_ascii_graphic() || b == b' ') && !name.contains("=?") && name.split(' ').all(|word| word.len() <= MAX_QUOTED_WORD);
    let phrase = if plain { quoted(&name) } else { b_words(&name).join(" ") };
    Ok(format!("{phrase} <{address}>"))
}

/// An `addr-spec` (RFC 5322 section 3.4.1, RFC 6532 for UTF-8), white space around it trimmed: a dot-atom or a
/// quoted string, `@`, and a dot-atom or a domain literal.
fn address(text: &str) -> Result<&str, MimeError> {
    let bad = MimeError::BadHeader("address");
    let text = text.trim_matches(' ');
    if text.len() > 254 {
        return Err(bad);
    }
    let (local, domain) = text.rsplit_once('@').ok_or_else(|| bad.clone())?;
    let local_ok = if local.starts_with('"') { is_quoted_string(local) } else { is_dot_atom(local, true) };
    let domain_ok = if domain.starts_with('[') { is_literal(domain) } else { is_dot_atom(domain, true) };
    if local_ok && domain_ok { Ok(text) } else { Err(bad) }
}

/// RFC 5322 `atext`, and with `utf8` any character outside ASCII that is no control, space or invisible mark.
fn is_atext(c: char, utf8: bool) -> bool {
    if c.is_ascii() {
        c.is_ascii_alphanumeric() || "!#$%&'*+-/=?^_`{|}~".contains(c)
    } else {
        utf8 && !c.is_control() && !c.is_whitespace() && !is_invisible_mark(c)
    }
}

/// Atoms of `atext` joined by single dots.
fn is_dot_atom(text: &str, utf8: bool) -> bool {
    text.split('.').all(|atom| !atom.is_empty() && atom.chars().all(|c| is_atext(c, utf8)))
}

/// A quoted string: `"`, printable ASCII, spaces, UTF-8 (RFC 6532) and `\` pairs, `"`.
fn is_quoted_string(text: &str) -> bool {
    let Some(inner) = text.strip_prefix('"').and_then(|t| t.strip_suffix('"')) else {
        return false;
    };
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        let ok = match c {
            '\\' => chars.next().is_some_and(|n| n == ' ' || n.is_ascii_graphic()),
            '"' => false,
            c => c == ' ' || c.is_ascii_graphic() || is_atext(c, true),
        };
        if !ok {
            return false;
        }
    }
    true
}

/// A domain literal: `[`, printable ASCII but `[`, `]` and `\`, `]`.
fn is_literal(text: &str) -> bool {
    text.strip_prefix('[')
        .and_then(|t| t.strip_suffix(']'))
        .is_some_and(|inner| inner.bytes().all(|b| b.is_ascii_graphic() && !matches!(b, b'[' | b']' | b'\\')))
}

/// Writes a field of message ids, when there are any.
fn id_field(head: &mut Vec<u8>, name: &str, ids: &[String]) -> Result<(), MimeError> {
    if ids.is_empty() {
        return Ok(());
    }
    let written = ids.iter().map(|id| message_id(id)).collect::<Result<Vec<_>, _>>()?;
    head.extend(fold(name, &written.join(" "))?);
    Ok(())
}

/// A `msg-id` (RFC 5322 section 3.6.4) in angle brackets: ASCII `dot-atom-text`, `@`, and a `dot-atom-text` or a
/// domain literal.
fn message_id(text: &str) -> Result<String, MimeError> {
    let bad = MimeError::BadHeader("message id");
    let text = text.trim_matches(' ');
    let id = text.strip_prefix('<').and_then(|t| t.strip_suffix('>')).unwrap_or(text);
    if id.len() > MAX_ID {
        return Err(bad);
    }
    let (left, right) = id.split_once('@').ok_or_else(|| bad.clone())?;
    let right_ok = if right.starts_with('[') { is_literal(right) } else { is_dot_atom(right, false) };
    if is_dot_atom(left, false) && right_ok { Ok(format!("<{id}>")) } else { Err(bad) }
}

/// The name of an extra field: printable ASCII without `:`, at most 76 characters, not one the builder writes.
fn field_name(name: &str) -> Result<String, MimeError> {
    let lower = name.to_ascii_lowercase();
    let ok = (1..=MAX_FIELD_NAME).contains(&name.len())
        && name.bytes().all(|b| b.is_ascii_graphic() && b != b':')
        && !OWN_FIELDS.contains(&lower.as_str())
        && !lower.starts_with("content-");
    if ok { Ok(name.to_owned()) } else { Err(MimeError::BadHeader("field name")) }
}

/// Whether [`MessageBuilder`] writes `text` as a message id (in `Message-ID`, `In-Reply-To` or `References`): ASCII
/// `dot-atom-text`, `@` and a `dot-atom-text` or domain literal, at most 250 bytes, with or without angle brackets.
#[must_use]
pub fn is_message_id(text: &str) -> bool {
    message_id(text).is_ok()
}

/// Whether [`MessageBuilder`] takes `text` as the media type of an [`Attachment`].
#[must_use]
pub fn is_media_type(text: &str) -> bool {
    media_type(text).is_ok()
}

/// A media type for an attachment: `type/subtype` of RFC 2045 tokens of at most 127 characters each, lower case, no
/// multipart and no attached message but `message/global`: RFC 2046 section 5.2 allows the others only in an
/// identity encoding, and every attachment is base64.
fn media_type(text: &str) -> Result<String, MimeError> {
    let lower = text.trim_matches(' ').to_ascii_lowercase();
    let name = |part: &str| is_token(part) && part.len() <= MAX_MEDIA_NAME;
    match lower.split_once('/') {
        Some((kind, subtype)) if name(kind) && name(subtype) && kind != "multipart" && (kind != "message" || subtype == "global") => Ok(lower),
        _ => Err(MimeError::BadHeader("media type")),
    }
}

#[cfg(test)]
#[path = "build_tests.rs"]
mod tests;
