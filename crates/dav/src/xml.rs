// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A small XML reader for the server's answers (XML 1.0 with namespaces), and escaping for the requests.
//!
//! Everything a server sends is hostile: the input must be UTF-8, a document type declaration is refused (so no
//! entity can grow, and nothing is fetched), and nesting, the number of nodes and of attributes and the length of a
//! name are capped. Only the five predefined entities and character references are read.

use crate::DavError;

/// The WebDAV namespace (RFC 4918).
pub const DAV: &str = "DAV:";
/// The CalDAV namespace (RFC 4791).
pub const CALDAV: &str = "urn:ietf:params:xml:ns:caldav";
/// The namespace of `getctag`, which many servers keep beside `sync-token`.
pub const CALENDARSERVER: &str = "http://calendarserver.org/ns/";
/// The namespace of `calendar-color`.
pub const APPLE_ICAL: &str = "http://apple.com/ns/ical/";

/// How deep elements may nest.
pub const MAX_DEPTH: usize = 64;
/// How many elements and pieces of text one document may hold.
pub const MAX_NODES: usize = 200_000;
/// How many attributes one element may carry.
pub const MAX_ATTRIBUTES: usize = 64;
/// How long a name, prefix included, may be, in bytes.
pub const MAX_NAME: usize = 256;

/// The namespace the `xml` prefix is bound to.
const XML_NAMESPACE: &str = "http://www.w3.org/XML/1998/namespace";

/// An element: its namespace and local name, its attributes and what it holds.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Element {
    /// The namespace URI, empty when it has none.
    pub namespace: String,
    /// The local name, without a prefix.
    pub name: String,
    /// The attributes other than namespace declarations, by their name as written (`name`, `xml:lang`).
    pub attributes: Vec<(String, String)>,
    /// What the element holds, in order.
    pub children: Vec<Node>,
}

/// A piece of an element's content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Node {
    /// A child element.
    Element(Element),
    /// Text, entities and CDATA sections read; neighbouring pieces are joined.
    Text(String),
}

impl Element {
    /// Whether the element is `name` in `namespace`.
    #[must_use]
    pub fn is(&self, namespace: &str, name: &str) -> bool {
        self.namespace == namespace && self.name == name
    }

    /// The child elements, in order.
    pub fn elements(&self) -> impl Iterator<Item = &Self> {
        self.children.iter().filter_map(|node| match node {
            Node::Element(element) => Some(element),
            Node::Text(_) => None,
        })
    }

    /// The child elements that are `name` in `namespace`.
    pub fn children_named<'a>(&'a self, namespace: &'a str, name: &'a str) -> impl Iterator<Item = &'a Self> {
        self.elements().filter(move |element| element.is(namespace, name))
    }

    /// The first child element that is `name` in `namespace`.
    #[must_use]
    pub fn child(&self, namespace: &str, name: &str) -> Option<&Self> {
        self.elements().find(|element| element.is(namespace, name))
    }

    /// The element's own text, without that of its children.
    #[must_use]
    pub fn text(&self) -> String {
        self.children
            .iter()
            .filter_map(|node| match node {
                Node::Text(text) => Some(text.as_str()),
                Node::Element(_) => None,
            })
            .collect()
    }

    /// The value of the attribute written as `name`.
    #[must_use]
    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes.iter().find(|(written, _)| written == name).map(|(_, value)| value.as_str())
    }
}

/// Reads one XML document and returns its root element.
///
/// # Errors
///
/// [`DavError::BadXml`] when the document is not well-formed or has a document type declaration, and
/// [`DavError::TooLarge`] when it goes past a cap.
///
/// ```
/// use inboxcraft_dav::xml::{DAV, parse};
///
/// let root = parse(br#"<d:prop xmlns:d="DAV:"><d:displayname>Work &amp; more</d:displayname></d:prop>"#)?;
/// assert_eq!(root.child(DAV, "displayname").map(|name| name.text()), Some("Work & more".to_owned()));
/// # Ok::<(), inboxcraft_dav::DavError>(())
/// ```
pub fn parse(input: &[u8]) -> Result<Element, DavError> {
    let text = std::str::from_utf8(input).map_err(|_| DavError::BadXml("not UTF-8"))?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    if !text.chars().all(allowed) {
        return Err(DavError::BadXml("a character XML does not allow"));
    }
    Reader { src: text, pos: 0, nodes: 0, open: Vec::new(), root: None }.run()
}

/// Escapes `text` for an XML element or a quoted attribute.
#[must_use]
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

/// An element whose end tag has not come yet.
struct Open {
    element: Element,
    /// Its name as written, for the end tag.
    written: String,
    /// The namespaces it declares: prefix (empty for the default) and URI.
    declared: Vec<(String, String)>,
}

/// One start tag, read but not resolved.
struct Tag {
    written: String,
    attributes: Vec<(String, String)>,
    closed: bool,
}

struct Reader<'a> {
    src: &'a str,
    pos: usize,
    nodes: usize,
    open: Vec<Open>,
    root: Option<Element>,
}

impl<'a> Reader<'a> {
    fn run(mut self) -> Result<Element, DavError> {
        let src = self.src;
        while let Some(rest) = src.get(self.pos..).filter(|rest| !rest.is_empty()) {
            if rest.starts_with("<?") {
                self.skip_past("?>")?;
            } else if rest.starts_with("<!--") {
                self.skip_past("-->")?;
            } else if rest.starts_with("<![CDATA[") {
                self.cdata()?;
            } else if rest.starts_with("<!") {
                return Err(DavError::BadXml("a document type declaration"));
            } else if rest.starts_with("</") {
                self.end_tag()?;
            } else if rest.starts_with('<') {
                self.start_tag()?;
            } else {
                self.text(rest)?;
            }
        }
        if !self.open.is_empty() {
            return Err(DavError::BadXml("an unclosed element"));
        }
        self.root.ok_or(DavError::BadXml("no root element"))
    }

    /// Moves past the next `end`.
    fn skip_past(&mut self, end: &str) -> Result<(), DavError> {
        let rest = self.src.get(self.pos..).unwrap_or_default();
        let at = rest.find(end).ok_or(DavError::BadXml("an unclosed section"))?;
        self.pos = self.pos.saturating_add(at).saturating_add(end.len());
        Ok(())
    }

    fn cdata(&mut self) -> Result<(), DavError> {
        let start = self.pos.saturating_add("<![CDATA[".len());
        self.skip_past("]]>")?;
        let inner = self.src.get(start..self.pos.saturating_sub(3)).unwrap_or_default().to_owned();
        self.push_text(inner)
    }

    fn text(&mut self, rest: &str) -> Result<(), DavError> {
        let len = rest.find('<').unwrap_or(rest.len());
        let raw = rest.get(..len).unwrap_or_default();
        self.pos = self.pos.saturating_add(len);
        if self.open.is_empty() {
            return if raw.trim().is_empty() { Ok(()) } else { Err(DavError::BadXml("text outside the root element")) };
        }
        if raw.contains("]]>") {
            return Err(DavError::BadXml("a ]]> in text"));
        }
        let text = unescape(raw)?;
        self.push_text(text)
    }

    fn push_text(&mut self, text: String) -> Result<(), DavError> {
        let Some(top) = self.open.last_mut() else { return Err(DavError::BadXml("text outside the root element")) };
        if text.is_empty() {
            return Ok(());
        }
        if let Some(Node::Text(last)) = top.element.children.last_mut() {
            last.push_str(&text);
            return Ok(());
        }
        self.nodes = self.nodes.saturating_add(1);
        if self.nodes > MAX_NODES {
            return Err(DavError::TooLarge { what: "XML node count", limit: MAX_NODES });
        }
        top.element.children.push(Node::Text(text));
        Ok(())
    }

    fn end_tag(&mut self) -> Result<(), DavError> {
        self.pos = self.pos.saturating_add(2);
        let name = self.name();
        self.skip_space();
        let rest = self.src.get(self.pos..).unwrap_or_default();
        let matches = rest.starts_with('>') && self.open.last().is_some_and(|top| top.written == name);
        let Some(top) = self.open.pop().filter(|_| matches) else { return Err(DavError::BadXml("an end tag that does not match")) };
        self.pos = self.pos.saturating_add(1);
        self.close(top.element);
        Ok(())
    }

    fn start_tag(&mut self) -> Result<(), DavError> {
        if self.root.is_some() {
            return Err(DavError::BadXml("a second root element"));
        }
        self.pos = self.pos.saturating_add(1);
        let tag = self.tag()?;
        if self.open.len() >= MAX_DEPTH {
            return Err(DavError::TooLarge { what: "XML nesting", limit: MAX_DEPTH });
        }
        self.nodes = self.nodes.saturating_add(1);
        if self.nodes > MAX_NODES {
            return Err(DavError::TooLarge { what: "XML node count", limit: MAX_NODES });
        }
        let open = self.resolve(tag.written, tag.attributes)?;
        if tag.closed {
            self.close(open.element);
        } else {
            self.open.push(open);
        }
        Ok(())
    }

    /// Reads a start tag after its `<`, up to and with its `>` or `/>`.
    fn tag(&mut self) -> Result<Tag, DavError> {
        let written = qualified(self.name())?;
        let mut attributes: Vec<(String, String)> = Vec::new();
        loop {
            self.skip_space();
            let rest = self.src.get(self.pos..).unwrap_or_default();
            if rest.starts_with("/>") {
                self.pos = self.pos.saturating_add(2);
                return Ok(Tag { written, attributes, closed: true });
            }
            if rest.starts_with('>') {
                self.pos = self.pos.saturating_add(1);
                return Ok(Tag { written, attributes, closed: false });
            }
            if rest.is_empty() || rest.starts_with('/') {
                return Err(DavError::BadXml("an unclosed tag"));
            }
            let (name, value) = self.attribute()?;
            if attributes.iter().any(|(seen, _)| *seen == name) {
                return Err(DavError::BadXml("an attribute given twice"));
            }
            if attributes.len() >= MAX_ATTRIBUTES {
                return Err(DavError::TooLarge { what: "XML attribute count", limit: MAX_ATTRIBUTES });
            }
            attributes.push((name, value));
        }
    }

    fn attribute(&mut self) -> Result<(String, String), DavError> {
        let name = qualified(self.name())?;
        self.skip_space();
        let rest = self.src.get(self.pos..).unwrap_or_default();
        let Some(rest) = rest.strip_prefix('=') else { return Err(DavError::BadXml("an attribute without a value")) };
        let after = rest.trim_start_matches(is_space);
        let quote = match after.chars().next() {
            Some(quote @ ('"' | '\'')) => quote,
            _ => return Err(DavError::BadXml("an attribute without quotes")),
        };
        let value_start = rest.len().saturating_sub(after.len()).saturating_add(1);
        let body = rest.get(value_start..).unwrap_or_default();
        let len = body.find(quote).ok_or(DavError::BadXml("an unclosed tag"))?;
        let raw = body.get(..len).unwrap_or_default();
        if raw.contains('<') {
            return Err(DavError::BadXml("a < in an attribute"));
        }
        self.pos = self.pos.saturating_add(1).saturating_add(value_start).saturating_add(len).saturating_add(1);
        Ok((name, unescape(raw)?))
    }

    /// Takes the namespace declarations out of the attributes and binds the names.
    fn resolve(&self, written: String, attributes: Vec<(String, String)>) -> Result<Open, DavError> {
        let mut declared = Vec::new();
        let mut kept = Vec::new();
        for (name, value) in attributes {
            if name == "xmlns" {
                declared.push((String::new(), value));
            } else if let Some(prefix) = name.strip_prefix("xmlns:") {
                if value.is_empty() {
                    return Err(DavError::BadXml("an empty namespace for a prefix"));
                }
                declared.push((prefix.to_owned(), value));
            } else {
                kept.push((name, value));
            }
        }
        for (name, _) in &kept {
            if let Some((prefix, _)) = name.split_once(':') {
                self.lookup(prefix, &declared)?;
            }
        }
        let (prefix, local) = written.split_once(':').unwrap_or(("", &written));
        let namespace = self.lookup(prefix, &declared)?;
        let element = Element { namespace, name: local.to_owned(), attributes: kept, children: Vec::new() };
        Ok(Open { element, written, declared })
    }

    /// The namespace `prefix` is bound to, the innermost declaration first.
    fn lookup(&self, prefix: &str, declared: &[(String, String)]) -> Result<String, DavError> {
        if prefix == "xml" {
            return Ok(XML_NAMESPACE.to_owned());
        }
        let scopes = std::iter::once(declared).chain(self.open.iter().rev().map(|open| open.declared.as_slice()));
        for scope in scopes {
            if let Some((_, uri)) = scope.iter().rev().find(|(bound, _)| bound == prefix) {
                return Ok(uri.clone());
            }
        }
        if prefix.is_empty() { Ok(String::new()) } else { Err(DavError::BadXml("a prefix without a namespace")) }
    }

    fn close(&mut self, element: Element) {
        match self.open.last_mut() {
            Some(parent) => parent.element.children.push(Node::Element(element)),
            None => self.root = Some(element),
        }
    }

    /// Takes the characters up to the next space, `/`, `>`, `=` or the end.
    fn name(&mut self) -> &'a str {
        let src = self.src;
        let rest = src.get(self.pos..).unwrap_or_default();
        let len = rest.find(|c: char| is_space(c) || matches!(c, '/' | '>' | '=')).unwrap_or(rest.len());
        self.pos = self.pos.saturating_add(len);
        rest.get(..len).unwrap_or_default()
    }

    fn skip_space(&mut self) {
        let rest = self.src.get(self.pos..).unwrap_or_default();
        self.pos = self.pos.saturating_add(rest.len().saturating_sub(rest.trim_start_matches(is_space).len()));
    }
}

/// Whether XML 1.0 allows `c`: every control character but tab, line feed and carriage return is refused.
fn allowed(c: char) -> bool {
    !matches!(c, '\0'..='\x08' | '\x0b' | '\x0c' | '\x0e'..='\x1f')
}

fn is_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r')
}

/// Checks a name with at most one prefix.
fn qualified(name: &str) -> Result<String, DavError> {
    let good = name.len() <= MAX_NAME && name.split(':').count() <= 2 && name.split(':').all(local_name);
    if good { Ok(name.to_owned()) } else { Err(DavError::BadXml("a bad name")) }
}

fn local_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(|first| first == '_' || first.is_alphabetic()) && chars.all(|c| c.is_alphanumeric() || matches!(c, '-' | '.' | '_'))
}

/// Reads the entities and character references of `raw`.
fn unescape(raw: &str) -> Result<String, DavError> {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(at) = rest.find('&') {
        out.push_str(rest.get(..at).unwrap_or_default());
        let after = rest.get(at.saturating_add(1)..).unwrap_or_default();
        let end = after.find(';').ok_or(DavError::BadXml("an unclosed entity"))?;
        out.push(entity(after.get(..end).unwrap_or_default())?);
        rest = after.get(end.saturating_add(1)..).unwrap_or_default();
    }
    out.push_str(rest);
    Ok(out)
}

fn entity(name: &str) -> Result<char, DavError> {
    let number = match name {
        "lt" => return Ok('<'),
        "gt" => return Ok('>'),
        "amp" => return Ok('&'),
        "apos" => return Ok('\''),
        "quot" => return Ok('"'),
        _ => match name.strip_prefix("#x") {
            Some(hex) => u32::from_str_radix(hex, 16).ok(),
            None => match name.strip_prefix('#') {
                Some(decimal) => decimal.parse::<u32>().ok(),
                None => return Err(DavError::BadXml("an unknown entity")),
            },
        },
    };
    number.and_then(char::from_u32).filter(|&c| allowed(c)).ok_or(DavError::BadXml("a character reference to no character"))
}

#[cfg(test)]
#[path = "xml_tests.rs"]
mod tests;
