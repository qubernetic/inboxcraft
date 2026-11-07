// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Mozilla's autoconfig file (`config-v1.1.xml`): the IMAP and submission servers a provider publishes, from the
//! provider's own web server or Mozilla's ISPDB.
//!
//! The file comes from the network and is hostile. The reader is a small XML reader of its own: elements,
//! attributes, text and the five named and the numeric character references; comments, processing instructions,
//! CDATA and the document type are skipped, and anything else ends the read. Input, depth, the number of elements
//! and of servers are capped, and every value is checked as a host name, a port or a user name before it is kept. A
//! server without TLS or STARTTLS is left out: encrypt or refuse.

use std::num::NonZeroU16;

use inboxcraft_model::{EmailAddress, Host, Security, Server};

/// The largest file read, in bytes.
pub const MAX_FILE: usize = 64 * 1024;

/// The most servers of each kind kept.
pub const MAX_SERVERS: usize = 8;

/// How deep elements may nest.
const MAX_DEPTH: usize = 16;

/// The most elements read.
const MAX_ELEMENTS: usize = 2_000;

/// The longest user name kept, in bytes.
const MAX_USERNAME: usize = 320;

/// A server the file names, with the user name to sign in with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    /// The server.
    pub server: Server,
    /// The user name, with the placeholders filled in for the address.
    pub username: String,
}

/// What the file offers, in its order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Autoconfig {
    /// IMAP servers.
    pub imap: Vec<Offer>,
    /// Submission (SMTP) servers.
    pub submission: Vec<Offer>,
}

/// The servers of the autoconfig file `xml` for `address`; `None` when it is not one (or larger than [`MAX_FILE`]).
///
/// ```
/// use inboxcraft_model::EmailAddress;
///
/// let xml = br#"<clientConfig version="1.1"><emailProvider id="example.org">
///   <incomingServer type="imap"><hostname>imap.%EMAILDOMAIN%</hostname><port>993</port>
///     <socketType>SSL</socketType><username>%EMAILADDRESS%</username></incomingServer>
/// </emailProvider></clientConfig>"#;
/// let address = EmailAddress::new("ada@example.org")?;
/// let found = inboxcraft_net::autoconfig::parse(xml, &address).ok_or("not autoconfig")?;
/// assert_eq!(found.imap[0].server.to_string(), "imap.example.org:993 (TLS)");
/// assert_eq!(found.imap[0].username, "ada@example.org");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn parse(xml: &[u8], address: &EmailAddress) -> Option<Autoconfig> {
    if xml.len() > MAX_FILE {
        return None;
    }
    let text = std::str::from_utf8(xml).ok()?;
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let root = Reader { text, at: 0, elements: 0 }.document()?;
    if root.name != "clientConfig" {
        return None;
    }
    let mut found = Autoconfig::default();
    for provider in root.children.iter().filter(|child| child.name == "emailProvider") {
        for server in &provider.children {
            let (kind, list) = match server.name.as_str() {
                "incomingServer" => ("imap", &mut found.imap),
                "outgoingServer" => ("smtp", &mut found.submission),
                _ => continue,
            };
            if server.attribute("type") != Some(kind) || list.len() >= MAX_SERVERS {
                continue;
            }
            if let Some(offer) = offer(server, address)
                && !list.contains(&offer)
            {
                list.push(offer);
            }
        }
    }
    Some(found)
}

/// One server element as an [`Offer`], if it is complete, encrypted and valid.
fn offer(server: &Element, address: &EmailAddress) -> Option<Offer> {
    let host = fill(server.child_text("hostname")?.trim(), address);
    let host = Host::new(&host).ok()?;
    let port = server.child_text("port")?.trim().parse::<u16>().ok().and_then(NonZeroU16::new)?;
    let security = match server.child_text("socketType")?.trim() {
        "SSL" | "TLS" => Security::Tls,
        "STARTTLS" => Security::StartTls,
        _ => return None,
    };
    let username = fill(server.child_text("username").unwrap_or("%EMAILADDRESS%"), address);
    let username = username.trim();
    if username.is_empty() || username.len() > MAX_USERNAME || username.chars().any(char::is_control) {
        return None;
    }
    Some(Offer { server: Server::new(host, port, security), username: username.to_owned() })
}

/// `text` with the placeholders of the address filled in.
fn fill(text: &str, address: &EmailAddress) -> String {
    text.replace("%EMAILADDRESS%", address.as_str()).replace("%EMAILLOCALPART%", address.local_part()).replace("%EMAILDOMAIN%", address.domain())
}

/// An element: its name, attributes, child elements and the text right inside it.
#[derive(Debug, Default)]
struct Element {
    name: String,
    attributes: Vec<(String, String)>,
    children: Vec<Element>,
    text: String,
}

impl Element {
    fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes.iter().find(|(key, _)| key == name).map(|(_, value)| value.as_str())
    }

    fn child_text(&self, name: &str) -> Option<&str> {
        self.children.iter().find(|child| child.name == name).map(|child| child.text.as_str())
    }
}

/// Reads one document.
struct Reader<'a> {
    text: &'a str,
    at: usize,
    elements: usize,
}

impl<'a> Reader<'a> {
    fn rest(&self) -> &'a str {
        self.text.get(self.at..).unwrap_or_default()
    }

    fn advance(&mut self, bytes: usize) {
        self.at = self.at.saturating_add(bytes).min(self.text.len());
    }

    /// Skips white space, comments, processing instructions and the document type between elements.
    fn skip_misc(&mut self) -> Option<()> {
        loop {
            let rest = self.rest();
            let trimmed = rest.trim_start();
            self.advance(rest.len().saturating_sub(trimmed.len()));
            if !self.skip_markup()? {
                return Some(());
            }
        }
    }

    /// Skips a comment, a processing instruction or a document type here; whether there was one. An unclosed one is
    /// an error.
    fn skip_markup(&mut self) -> Option<bool> {
        let rest = self.rest();
        let end = if rest.starts_with("<!--") {
            rest.find("-->")?.checked_add(3)?
        } else if rest.starts_with("<?") {
            rest.find("?>")?.checked_add(2)?
        } else if rest.starts_with("<!DOCTYPE") {
            // No internal subset: it could define entities.
            let close = rest.find('>')?;
            if rest.get(..close)?.contains('[') {
                return None;
            }
            close.checked_add(1)?
        } else {
            return Some(false);
        };
        self.advance(end);
        Some(true)
    }

    fn document(mut self) -> Option<Element> {
        self.skip_misc()?;
        let root = self.element(0)?;
        self.skip_misc()?;
        self.rest().is_empty().then_some(root)
    }

    /// The element starting here, at `depth`.
    fn element(&mut self, depth: usize) -> Option<Element> {
        if depth >= MAX_DEPTH {
            return None;
        }
        self.elements = self.elements.checked_add(1)?;
        if self.elements > MAX_ELEMENTS {
            return None;
        }
        self.advance(self.rest().strip_prefix('<').map(|_| 1)?);
        let mut element = Element { name: self.name()?, ..Element::default() };
        loop {
            let rest = self.rest();
            let trimmed = rest.trim_start();
            self.advance(rest.len().saturating_sub(trimmed.len()));
            if self.rest().starts_with("/>") {
                self.advance(2);
                return Some(element);
            }
            if self.rest().starts_with('>') {
                self.advance(1);
                break;
            }
            let key = self.name()?;
            let rest = self.rest().trim_start();
            let rest = rest.strip_prefix('=')?.trim_start();
            let quote = rest.chars().next().filter(|c| *c == '"' || *c == '\'')?;
            let value_and_more = rest.get(1..)?;
            let close = value_and_more.find(quote)?;
            let value = unescape(value_and_more.get(..close)?)?;
            let used = self.rest().len().saturating_sub(value_and_more.len()).checked_add(close)?.checked_add(1)?;
            self.advance(used);
            element.attributes.push((key, value));
        }
        self.content(&mut element, depth)?;
        Some(element)
    }

    /// The children and text of `element`, up to and with its end tag.
    fn content(&mut self, element: &mut Element, depth: usize) -> Option<()> {
        loop {
            let rest = self.rest();
            let text_end = rest.find('<')?;
            element.text.push_str(&unescape(rest.get(..text_end)?)?);
            self.advance(text_end);
            let rest = self.rest();
            if let Some(after) = rest.strip_prefix("</") {
                let close = after.find('>')?;
                if after.get(..close)?.trim_end() != element.name {
                    return None;
                }
                self.advance(close.checked_add(3)?);
                return Some(());
            }
            if let Some(after) = rest.strip_prefix("<![CDATA[") {
                let close = after.find("]]>")?;
                element.text.push_str(after.get(..close)?);
                self.advance(close.checked_add(12)?);
                continue;
            }
            if self.skip_markup()? {
                continue;
            }
            let child = self.element(depth.checked_add(1)?)?;
            element.children.push(child);
        }
    }

    /// A name: letters, digits, `_`, `-`, `.` and `:`, starting with a letter or `_`.
    fn name(&mut self) -> Option<String> {
        let rest = self.rest();
        let end = rest.find(|c: char| !(c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | ':'))).unwrap_or(rest.len());
        let name = rest.get(..end)?;
        if !name.chars().next().is_some_and(|c| c.is_alphabetic() || c == '_') {
            return None;
        }
        self.advance(end);
        Some(name.to_owned())
    }
}

/// `text` with its character references replaced; `None` for an unknown or broken one.
fn unescape(text: &str) -> Option<String> {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('&') {
        out.push_str(rest.get(..start)?);
        let after = rest.get(start.checked_add(1)?..)?;
        let end = after.find(';')?;
        let reference = after.get(..end)?;
        let c = match reference {
            "amp" => '&',
            "lt" => '<',
            "gt" => '>',
            "quot" => '"',
            "apos" => '\'',
            _ => {
                let number = if let Some(hex) = reference.strip_prefix("#x") {
                    u32::from_str_radix(hex, 16).ok()?
                } else {
                    reference.strip_prefix('#')?.parse::<u32>().ok()?
                };
                char::from_u32(number)?
            }
        };
        out.push(c);
        rest = after.get(end.checked_add(1)?..)?;
    }
    out.push_str(rest);
    Some(out)
}

#[cfg(test)]
#[path = "autoconfig_tests.rs"]
mod tests;
