// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The [`Envelope`] of a message from its header block (`BODY.PEEK[HEADER]`), read the way RFC 5322 writes it.
//!
//! Headers are hostile: nothing here fails or panics. A field that cannot be read is left out, an address that is no
//! address is skipped, a list is read up to [`MAX_LIST_CHARS`] characters and [`MAX_ADDRESSES`] addresses, and
//! comments nest without recursion. The model cleans every text on the way in.

use inboxcraft_charset::decode_text;
use inboxcraft_mime::Headers;
use inboxcraft_model::{Address, EmailAddress, Envelope, MAX_ADDRESSES, MAX_REFERENCES, MessageRef};
use inboxcraft_time::MailDate;

/// Most characters of one address or id field read; the rest is left out.
pub const MAX_LIST_CHARS: usize = 64 * 1024;

/// The envelope of the header block `header`; an empty one for bytes that are no header.
#[must_use]
pub fn envelope(header: &[u8]) -> Envelope {
    let Ok(headers) = Headers::parse(header) else { return Envelope::default() };
    let list = |name: &str| headers.get(name).map(addresses).unwrap_or_default();
    let refs = |name: &str| headers.get(name).map(message_refs).unwrap_or_default();
    Envelope::new(&headers.text("subject").unwrap_or_default())
        .with_date(headers.get("date").and_then(|date| MailDate::parse(date).ok()).map(MailDate::timestamp))
        .with_from(list("from"))
        .with_reply_to(list("reply-to"))
        .with_to(list("to"))
        .with_cc(list("cc"))
        .with_bcc(list("bcc"))
        .with_message_id(refs("message-id").into_iter().next())
        .with_in_reply_to(refs("in-reply-to"))
        .with_references(refs("references"))
}

/// The addresses of an address list (RFC 5322 section 3.4): mailboxes and the members of groups, at most
/// [`MAX_ADDRESSES`].
#[must_use]
pub fn addresses(value: &str) -> Vec<Address> {
    let mut found = Vec::new();
    let mut piece = String::new();
    let mut scan = Scan::default();
    for c in value.chars().take(MAX_LIST_CHARS) {
        if found.len() >= MAX_ADDRESSES {
            break;
        }
        if scan.is_top() && matches!(c, ',' | ';') {
            found.extend(mailbox(&piece));
            piece.clear();
        } else if scan.is_top() && c == ':' {
            // The name of a group: its members follow.
            piece.clear();
        } else {
            scan.step(c);
            piece.push(c);
        }
    }
    if found.len() < MAX_ADDRESSES {
        found.extend(mailbox(&piece));
    }
    found.truncate(MAX_ADDRESSES);
    found
}

/// The message ids of `Message-ID`, `In-Reply-To` or `References`: each `<...>`, at most twice
/// [`MAX_REFERENCES`] (the model keeps the last of `References`).
#[must_use]
pub fn message_refs(value: &str) -> Vec<MessageRef> {
    let value = value.get(..floor_boundary(value, MAX_LIST_CHARS)).unwrap_or_default();
    let mut refs = Vec::new();
    let mut rest = value;
    while let Some(start) = rest.find('<') {
        let after = rest.get(start.saturating_add(1)..).unwrap_or_default();
        let Some(end) = after.find('>') else { break };
        if let Some(id) = after.get(..end).and_then(|id| MessageRef::new(id.trim())) {
            refs.push(id);
        }
        rest = after.get(end.saturating_add(1)..).unwrap_or_default();
    }
    if refs.len() > MAX_REFERENCES.saturating_mul(2) {
        refs.drain(..refs.len().saturating_sub(MAX_REFERENCES.saturating_mul(2)));
    }
    refs
}

/// The largest char boundary of `text` at or below `limit` bytes.
fn floor_boundary(text: &str, limit: usize) -> usize {
    if text.len() <= limit {
        return text.len();
    }
    (0..=limit).rev().find(|at| text.is_char_boundary(*at)).unwrap_or(0)
}

/// Where a scan of header text stands: inside a quoted string, a comment (nested to `depth`) or angle brackets.
#[derive(Default)]
struct Scan {
    quoted: bool,
    escaped: bool,
    depth: usize,
    angle: bool,
}

impl Scan {
    fn is_top(&self) -> bool {
        !self.quoted && self.depth == 0 && !self.angle
    }

    /// Moves past `c`; returns what `c` is in the text.
    fn step(&mut self, c: char) -> Token {
        if self.escaped {
            self.escaped = false;
            return if self.quoted { Token::Quoted(c) } else { Token::Comment(c) };
        }
        if self.quoted {
            match c {
                '\\' => self.escaped = true,
                '"' => self.quoted = false,
                _ => return Token::Quoted(c),
            }
            return Token::Syntax;
        }
        if self.depth > 0 {
            match c {
                '\\' => self.escaped = true,
                '(' => self.depth = self.depth.saturating_add(1),
                ')' => self.depth = self.depth.saturating_sub(1),
                _ => return Token::Comment(c),
            }
            return if self.depth == 0 { Token::CommentEnd } else { Token::Syntax };
        }
        if self.angle {
            if c == '>' {
                self.angle = false;
                return Token::Syntax;
            }
            return Token::Angle(c);
        }
        match c {
            '"' => self.quoted = true,
            '(' => self.depth = 1,
            '<' => self.angle = true,
            _ => return Token::Plain(c),
        }
        Token::Syntax
    }

    fn is_open(&self) -> bool {
        self.quoted || self.escaped || self.depth > 0 || self.angle
    }
}

/// One character of a mailbox, by where it stands.
enum Token {
    /// Outside quotes, comments and brackets.
    Plain(char),
    /// Inside a quoted string, unescaped.
    Quoted(char),
    /// Inside a comment.
    Comment(char),
    /// The end of the outermost comment.
    CommentEnd,
    /// Inside angle brackets.
    Angle(char),
    /// A quote, bracket or escape.
    Syntax,
}

/// One mailbox: `name <address>`, `address (name)` or `address`; `None` when it is none.
fn mailbox(text: &str) -> Option<Address> {
    let (mut phrase, mut comment, mut angle) = (String::new(), None::<String>, None::<String>);
    let mut in_comment = String::new();
    let mut scan = Scan::default();
    for c in text.chars() {
        match scan.step(c) {
            Token::Plain(c) | Token::Quoted(c) => phrase.push(c),
            Token::Comment(c) => in_comment.push(c),
            Token::CommentEnd => {
                // A comment separates words; only the first is kept, as a name for a bare address.
                phrase.push(' ');
                comment.get_or_insert_with(|| std::mem::take(&mut in_comment));
                in_comment.clear();
            }
            Token::Angle(c) => angle.get_or_insert_with(String::new).push(c),
            Token::Syntax => {}
        }
    }
    if scan.is_open() {
        return None;
    }
    let phrase = phrase.split_whitespace().collect::<Vec<_>>().join(" ");
    let (address, name) = match angle {
        // An obsolete route (`<@relay:a@example.org>`) is dropped.
        Some(angle) => (angle.rsplit(':').next().unwrap_or_default().trim().to_owned(), Some(phrase).filter(|p| !p.is_empty())),
        None => (phrase, comment.map(|c| c.trim().to_owned())),
    };
    let email = EmailAddress::new(&address).ok()?;
    let name = name.map(|name| decode_text(&name).text);
    Some(Address::new(name.as_deref(), email))
}

#[cfg(test)]
#[path = "envelope_tests.rs"]
mod tests;
