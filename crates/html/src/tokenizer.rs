// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-FileCopyrightText: MailCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A tolerant HTML tokenizer: text, start tags and end tags.
//!
//! It follows the general syntax of the HTML Living Standard closely enough for mail, and reads
//! anything: a `<` that starts no tag is text, a tag or comment that never ends runs to the end of
//! the input. Comments, doctypes, processing instructions and the content of `script`, `style`,
//! `title` and friends never leave this module.

use crate::entities::unescape;

/// Most attributes kept per tag; the rest are read and dropped.
pub(crate) const MAX_ATTRS: usize = 32;

/// Longest tag name kept; a longer one is an unknown tag.
const MAX_TAG_NAME: usize = 24;

/// Elements whose content is raw text and never shown.
const DROPPED: [&str; 6] = ["script", "style", "title", "iframe", "noembed", "noframes"];

/// Elements whose content is text, not markup, and is shown.
const RAW_TEXT: [&str; 2] = ["textarea", "xmp"];

/// A start tag.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Tag<'a> {
    /// Lower case; empty for a name too long to be a real element.
    pub name: String,
    /// Names and values as written.
    pub attrs: Vec<(&'a str, &'a str)>,
}

impl Tag<'_> {
    /// An attribute's value with character references decoded; the first one wins, as in browsers.
    pub fn attr(&self, name: &str) -> Option<String> {
        self.attrs.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, v)| unescape(v))
    }

    /// Whether the tag has the attribute, with or without a value.
    pub fn has(&self, name: &str) -> bool {
        self.attrs.iter().any(|(n, _)| n.eq_ignore_ascii_case(name))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Token<'a> {
    /// Character data as written: references are not decoded yet.
    Text(&'a str),
    Open(Tag<'a>),
    /// An end tag's name, lower case.
    Close(String),
}

pub(crate) struct Tokenizer<'a> {
    src: &'a str,
    pos: usize,
    /// Set after the start tag of a raw-text element: its content comes next.
    raw: Option<&'static str>,
}

impl<'a> Tokenizer<'a> {
    pub fn new(src: &'a str) -> Tokenizer<'a> {
        Tokenizer { src, pos: 0, raw: None }
    }

    fn bytes(&self) -> &'a [u8] {
        self.src.as_bytes()
    }

    fn slice(&self, start: usize, end: usize) -> &'a str {
        self.src.get(start..end).unwrap_or("")
    }

    /// Whether the `<` at `i` starts a tag, a comment or a declaration (rather than being text).
    fn markup_at(&self, i: usize) -> bool {
        let b = self.bytes();
        match b.get(i.saturating_add(1)) {
            Some(c) if c.is_ascii_alphabetic() => true,
            Some(b'!' | b'?') => true,
            Some(b'/') => matches!(b.get(i.saturating_add(2)), Some(c) if c.is_ascii_alphabetic() || *c == b'>'),
            _ => false,
        }
    }

    /// The end of the text run that starts at `self.pos`: the next `<` that starts markup.
    fn text_end(&self) -> usize {
        let b = self.bytes();
        let mut i = self.pos.saturating_add(1);
        loop {
            match find_byte(b, i, b'<') {
                Some(j) if self.markup_at(j) => return j,
                Some(j) => i = j.saturating_add(1),
                None => return b.len(),
            }
        }
    }

    /// Skip a comment, doctype or processing instruction at `self.pos`.
    fn skip_declaration(&mut self) {
        let b = self.bytes();
        let end = if b.get(self.pos..self.pos.saturating_add(4)) == Some(b"<!--") {
            let body = self.pos.saturating_add(4);
            if b.get(body) == Some(&b'>') {
                body.saturating_add(1)
            } else if b.get(body..body.saturating_add(2)) == Some(b"->") {
                body.saturating_add(2)
            } else {
                find_sub(b, body, b"-->").map_or(b.len(), |i| i.saturating_add(3))
            }
        } else {
            find_byte(b, self.pos, b'>').map_or(b.len(), |i| i.saturating_add(1))
        };
        self.pos = end.min(b.len());
    }

    /// Read the end tag at `self.pos` (`</name ...>`).
    fn close_tag(&mut self) -> Option<Token<'a>> {
        let b = self.bytes();
        let start = self.pos.saturating_add(2);
        let i = name_end(b, start);
        let name = self.slice(start, i);
        self.pos = find_byte(b, i, b'>').map_or(b.len(), |j| j.saturating_add(1));
        // `</>` is nothing at all.
        (!name.is_empty()).then(|| Token::Close(tag_name(name)))
    }

    /// Read the start tag at `self.pos` (`<name attr=value ...>`).
    fn open_tag(&mut self) -> Tag<'a> {
        let b = self.bytes();
        let start = self.pos.saturating_add(1);
        let mut i = name_end(b, start);
        let name = tag_name(self.slice(start, i));
        let mut attrs = Vec::new();
        loop {
            while matches!(b.get(i), Some(c) if c.is_ascii_whitespace() || *c == b'/') {
                i = i.saturating_add(1);
            }
            match b.get(i) {
                None => break,
                Some(b'>') => {
                    i = i.saturating_add(1);
                    break;
                }
                Some(_) => {}
            }
            let (attr, value, next) = self.attribute(i);
            i = next;
            if attrs.len() < MAX_ATTRS {
                attrs.push((attr, value));
            }
        }
        self.pos = i.min(b.len());
        Tag { name, attrs }
    }

    /// Read one attribute starting at `i`: its name, its value as written and where reading goes on.
    fn attribute(&self, i: usize) -> (&'a str, &'a str, usize) {
        let b = self.bytes();
        // An attribute name runs to white space, `/`, `>` or `=`; a leading `=` is part of it.
        let name_start = i;
        let mut i = i.saturating_add(1);
        while let Some(c) = b.get(i) {
            if c.is_ascii_whitespace() || matches!(*c, b'/' | b'>' | b'=') {
                break;
            }
            i = i.saturating_add(1);
        }
        let attr = self.slice(name_start, i);
        let mut j = skip_space(b, i);
        if b.get(j) != Some(&b'=') {
            return (attr, "", j);
        }
        j = skip_space(b, j.saturating_add(1));
        match b.get(j) {
            Some(&q) if q == b'"' || q == b'\'' => {
                let end = find_byte(b, j.saturating_add(1), q).unwrap_or(b.len());
                (attr, self.slice(j.saturating_add(1), end), end.saturating_add(1).min(b.len()))
            }
            _ => {
                let mut end = j;
                while let Some(c) = b.get(end) {
                    if c.is_ascii_whitespace() || *c == b'>' {
                        break;
                    }
                    end = end.saturating_add(1);
                }
                (attr, self.slice(j, end), end)
            }
        }
    }
}

impl<'a> Iterator for Tokenizer<'a> {
    type Item = Token<'a>;

    fn next(&mut self) -> Option<Token<'a>> {
        loop {
            let b = self.bytes();
            if self.pos >= b.len() {
                return None;
            }
            if let Some(name) = self.raw.take() {
                let end = find_end_tag(b, self.pos, name).unwrap_or(b.len());
                let text = self.slice(self.pos, end);
                self.pos = end;
                if !text.is_empty() {
                    return Some(Token::Text(text));
                }
                continue;
            }
            if b.get(self.pos) != Some(&b'<') || !self.markup_at(self.pos) {
                let start = self.pos;
                self.pos = self.text_end();
                return Some(Token::Text(self.slice(start, self.pos)));
            }
            match b.get(self.pos.saturating_add(1)) {
                Some(b'!' | b'?') => self.skip_declaration(),
                Some(b'/') => {
                    if let Some(token) = self.close_tag() {
                        return Some(token);
                    }
                }
                _ => {
                    let tag = self.open_tag();
                    if DROPPED.contains(&tag.name.as_str()) {
                        // Skip the content and the end tag.
                        let end = find_end_tag(b, self.pos, &tag.name).unwrap_or(b.len());
                        self.pos = find_byte(b, end, b'>').map_or(b.len(), |i| i.saturating_add(1));
                        continue;
                    }
                    self.raw = RAW_TEXT.iter().copied().find(|n| *n == tag.name);
                    return Some(Token::Open(tag));
                }
            }
        }
    }
}

/// A tag name in lower case; empty when it is too long to be a real element.
fn tag_name(raw: &str) -> String {
    if raw.len() > MAX_TAG_NAME { String::new() } else { raw.to_ascii_lowercase() }
}

/// Where a tag name that starts at `start` ends: at white space, `/`, `>` or the end.
fn name_end(b: &[u8], start: usize) -> usize {
    let mut i = start;
    while let Some(c) = b.get(i) {
        if c.is_ascii_whitespace() || *c == b'/' || *c == b'>' {
            break;
        }
        i = i.saturating_add(1);
    }
    i
}

fn skip_space(b: &[u8], mut i: usize) -> usize {
    while matches!(b.get(i), Some(c) if c.is_ascii_whitespace()) {
        i = i.saturating_add(1);
    }
    i
}

fn find_byte(b: &[u8], from: usize, byte: u8) -> Option<usize> {
    b.get(from..)?.iter().position(|c| *c == byte).map(|i| i.saturating_add(from))
}

fn find_sub(b: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    b.get(from..)?.windows(needle.len()).position(|w| w == needle).map(|i| i.saturating_add(from))
}

/// Where `</name` starts (any letter case, followed by white space, `/`, `>` or the end), at or after `from`.
fn find_end_tag(b: &[u8], from: usize, name: &str) -> Option<usize> {
    let mut i = from;
    while let Some(lt) = find_byte(b, i, b'<') {
        let after = lt.saturating_add(2).saturating_add(name.len());
        let named =
            b.get(lt.saturating_add(1)) == Some(&b'/') && b.get(lt.saturating_add(2)..after).is_some_and(|n| n.eq_ignore_ascii_case(name.as_bytes()));
        if named && b.get(after).is_none_or(|c| c.is_ascii_whitespace() || matches!(*c, b'/' | b'>')) {
            return Some(lt);
        }
        i = lt.saturating_add(1);
    }
    None
}

#[cfg(test)]
#[path = "tokenizer_tests.rs"]
mod tests;
