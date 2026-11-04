// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-FileCopyrightText: MailCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Structured header values with parameters: `Content-Type` (RFC 2045 section 5), `Content-Disposition` (RFC 2183)
//! and the RFC 2231 forms of parameter values.
//!
//! Parameter reading adapted from UCS-Care-Group/mailcraft `crates/mime/src/header.rs` (see `PROVENANCE.md`).

use inboxcraft_charset::decode_text;

use crate::header::{clean, text_of};

/// The most parameters, RFC 2231 pieces counted one by one, read from one header value; the rest are ignored.
pub const MAX_PARAMS: usize = 64;

/// A media type with its parameters, as a `Content-Type` field gives it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentType {
    kind: String,
    subtype: String,
    params: Params,
}

impl Default for ContentType {
    /// `text/plain; charset=us-ascii`, what a part without a `Content-Type` is (RFC 2045 section 5.2).
    fn default() -> Self {
        Self::new("text", "plain", Params::parse("; charset=us-ascii"))
    }
}

impl ContentType {
    pub(crate) fn new(kind: &str, subtype: &str, params: Params) -> Self {
        Self { kind: kind.to_owned(), subtype: subtype.to_owned(), params }
    }

    /// Reads a `Content-Type` value; `None` when it does not start with a `type/subtype` of RFC 2045 tokens.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        let (main, list) = value.split_once(';').unwrap_or((value, ""));
        let main = main.split(|c: char| c.is_ascii_whitespace() || c == '(').find(|t| !t.is_empty())?;
        let (kind, subtype) = main.split_once('/')?;
        (is_token(kind) && is_token(subtype)).then(|| Self::new(&kind.to_ascii_lowercase(), &subtype.to_ascii_lowercase(), Params::parse(list)))
    }

    /// The top-level type, lower case (`text`, `multipart`).
    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// The subtype, lower case (`plain`, `mixed`).
    #[must_use]
    pub fn subtype(&self) -> &str {
        &self.subtype
    }

    /// `type/subtype`, lower case.
    #[must_use]
    pub fn essence(&self) -> String {
        format!("{}/{}", self.kind, self.subtype)
    }

    /// Whether this is `kind/subtype`, in any letter case.
    #[must_use]
    pub fn is(&self, kind: &str, subtype: &str) -> bool {
        self.kind.eq_ignore_ascii_case(kind) && self.subtype.eq_ignore_ascii_case(subtype)
    }

    /// A parameter as written, quotes and escapes removed; the name in any letter case.
    #[must_use]
    pub fn param(&self, name: &str) -> Option<&str> {
        self.params.get(name)
    }

    /// Every parameter.
    #[must_use]
    pub fn params(&self) -> &Params {
        &self.params
    }
}

/// A `Content-Disposition` (RFC 2183): `inline` or `attachment`, and its parameters.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Disposition {
    kind: String,
    params: Params,
}

impl Disposition {
    /// Reads a `Content-Disposition` value. Anything is read; an unknown kind is neither inline nor attachment.
    #[must_use]
    pub fn parse(value: &str) -> Self {
        let (main, list) = value.split_once(';').unwrap_or((value, ""));
        Self { kind: main.trim().to_ascii_lowercase(), params: Params::parse(list) }
    }

    /// The kind, lower case (`attachment`, `inline`, or whatever the sender wrote).
    #[must_use]
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// Whether the part is to be shown as a file.
    #[must_use]
    pub fn is_attachment(&self) -> bool {
        self.kind == "attachment"
    }

    /// Whether the part is to be shown in the message.
    #[must_use]
    pub fn is_inline(&self) -> bool {
        self.kind == "inline"
    }

    /// The `filename` parameter as text ([`Params::text`]).
    #[must_use]
    pub fn file_name(&self) -> Option<String> {
        self.params.text("filename")
    }

    /// Every parameter.
    #[must_use]
    pub fn params(&self) -> &Params {
        &self.params
    }
}

/// The parameters of a structured header value, RFC 2231 continuations and character sets put together.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Params {
    list: Vec<Param>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Param {
    name: String,
    value: String,
    /// Came in the RFC 2231 form, which says its own character set.
    extended: bool,
}

/// One `name=value` piece of a parameter list, before RFC 2231 pieces are put together.
struct Piece {
    name: String,
    /// The position of a continuation (`name*1`).
    index: Option<u32>,
    /// Percent-encoded (`name*=` or `name*1*=`).
    encoded: bool,
    value: String,
}

impl Params {
    /// Reads a parameter list: the part of a header value from its first `;` on.
    #[must_use]
    pub fn parse(list: &str) -> Self {
        let pieces = pieces(list);
        let mut params: Vec<Param> = Vec::new();
        for piece in &pieces {
            if params.iter().any(|p| p.name == piece.name) {
                continue;
            }
            let mut extended: Vec<&Piece> = pieces.iter().filter(|p| p.name == piece.name && (p.encoded || p.index.is_some())).collect();
            if extended.is_empty() {
                params.push(Param { name: piece.name.clone(), value: piece.value.clone(), extended: false });
                continue;
            }
            // The RFC 2231 form wins over a plain parameter of the same name.
            extended.sort_by_key(|p| p.index.unwrap_or(0));
            params.push(Param { name: piece.name.clone(), value: join(&extended), extended: true });
        }
        Self { list: params }
    }

    /// A parameter as written, quotes and escapes removed, RFC 2231 pieces joined and decoded; the name in any
    /// letter case.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.find(name).map(|p| p.value.as_str())
    }

    /// A parameter that holds text for people, such as a file name: RFC 2231 decoded, or else RFC 2047 encoded
    /// words decoded (which many senders put there although they should not), control characters as spaces.
    #[must_use]
    pub fn text(&self, name: &str) -> Option<String> {
        self.find(name).map(|p| clean(&if p.extended { p.value.clone() } else { decode_text(&p.value).text }))
    }

    fn find(&self, name: &str) -> Option<&Param> {
        self.list.iter().find(|p| p.name.eq_ignore_ascii_case(name))
    }
}

/// The `name=value` pieces of a parameter list, at most [`MAX_PARAMS`].
fn pieces(list: &str) -> Vec<Piece> {
    let mut pieces = Vec::new();
    let mut chars = list.chars().peekable();
    while pieces.len() < MAX_PARAMS {
        while chars.next_if(|c| c.is_whitespace() || *c == ';').is_some() {}
        if chars.peek().is_none() {
            break;
        }
        let mut name = String::new();
        while let Some(c) = chars.next_if(|c| *c != '=' && *c != ';') {
            name.push(c);
        }
        let mut value = String::new();
        if chars.next_if_eq(&'=').is_some() {
            while chars.next_if(|c| c.is_whitespace()).is_some() {}
            if chars.next_if_eq(&'"').is_some() {
                while let Some(c) = chars.next() {
                    match c {
                        '"' => break,
                        // Only `\"` and `\\` are escapes; a lone backslash (a Windows path) stays.
                        '\\' => value.push(chars.next_if(|n| *n == '"' || *n == '\\').unwrap_or('\\')),
                        c => value.push(c),
                    }
                }
                // Anything between the closing quote and the next `;` is noise.
                while chars.next_if(|c| *c != ';').is_some() {}
            } else {
                while let Some(c) = chars.next_if(|c| *c != ';') {
                    value.push(c);
                }
                value.truncate(value.trim_end().len());
            }
        }
        let name = name.trim().to_ascii_lowercase();
        let mut parts = name.split('*');
        let base = parts.next().unwrap_or_default().to_owned();
        if base.is_empty() {
            continue;
        }
        let piece = match (parts.next(), parts.next()) {
            (None, _) => Piece { name: base, index: None, encoded: false, value },
            (Some(""), _) => Piece { name: base, index: None, encoded: true, value },
            (Some(n), star) => match n.parse::<u32>() {
                Ok(index) => Piece { name: base, index: Some(index), encoded: star == Some(""), value },
                Err(_) => continue,
            },
        };
        pieces.push(piece);
    }
    pieces
}

/// The RFC 2231 pieces of one parameter, in order, as one text: the first encoded piece may start with
/// `charset'language'`.
fn join(pieces: &[&Piece]) -> String {
    let mut charset = "";
    let mut bytes = Vec::new();
    for (i, piece) in pieces.iter().enumerate() {
        if !piece.encoded {
            bytes.extend_from_slice(piece.value.as_bytes());
            continue;
        }
        let mut text = piece.value.as_str();
        if i == 0 {
            let mut it = text.splitn(3, '\'');
            if let (Some(label), Some(_language), Some(rest)) = (it.next(), it.next(), it.next()) {
                charset = label;
                text = rest;
            }
        }
        bytes.extend(percent_decode(text));
    }
    match inboxcraft_charset::decode(charset, &bytes) {
        Ok(decoded) if !charset.is_empty() => decoded.text,
        _ => text_of(&bytes),
    }
}

fn percent_decode(s: &str) -> Vec<u8> {
    let b = s.as_bytes();
    let hex = |i: usize| b.get(i).and_then(|c| char::from(*c).to_digit(16)).and_then(|d| u8::try_from(d).ok());
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while let Some(&c) = b.get(i) {
        match (c, hex(i.saturating_add(1)), hex(i.saturating_add(2))) {
            (b'%', Some(h), Some(l)) => {
                out.push((h << 4) | l);
                i = i.saturating_add(3);
            }
            _ => {
                out.push(c);
                i = i.saturating_add(1);
            }
        }
    }
    out
}

/// An RFC 2045 token: printable ASCII without space and without the `tspecials`.
fn is_token(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_graphic() && !b"()<>@,;:\\\"/[]?=".contains(&b))
}

#[cfg(test)]
#[path = "params_tests.rs"]
mod tests;
