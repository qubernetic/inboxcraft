// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `207 Multi-Status` answer (RFC 4918 section 13) and the request bodies that ask for one.

use crate::xml::{self, APPLE_ICAL, CALDAV, CALENDARSERVER, DAV, Element};
use crate::{DavError, Origin};

/// The longest sync token or entity tag kept, in bytes; a longer one is treated as absent.
pub const MAX_TOKEN: usize = 1024;

/// One `207 Multi-Status` answer: its entries and, for a sync report, the new sync token.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Multistatus {
    /// The entries, in the server's order; those about another origin are left out.
    pub entries: Vec<Entry>,
    /// The `DAV:sync-token` of a sync report.
    pub sync_token: Option<String>,
}

/// One `DAV:response`: a resource and what the server says of it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Entry {
    /// The resource's path, resolved and in its one form ([`Origin::resolve`]).
    pub href: String,
    /// The status of the whole resource, when the server gave one instead of properties (`404` for a member removed
    /// since the sync token, `507` when a sync report was cut short).
    pub status: Option<u16>,
    /// The properties, each with its status.
    pub propstats: Vec<(u16, Element)>,
}

impl Entry {
    /// The property `name` in `namespace`, when the server found it (a 2xx propstat).
    #[must_use]
    pub fn prop(&self, namespace: &str, name: &str) -> Option<&Element> {
        self.propstats.iter().filter(|(status, _)| (200..300).contains(status)).find_map(|(_, prop)| prop.child(namespace, name))
    }

    /// The text of the property, cleaned ([`clean`]), when it is there and not empty.
    #[must_use]
    pub fn text(&self, namespace: &str, name: &str, max_chars: usize) -> Option<String> {
        self.prop(namespace, name).map(|prop| clean(&prop.text(), max_chars)).filter(|text| !text.is_empty())
    }

    /// The `DAV:href` inside the property, resolved against `base` on `origin`.
    #[must_use]
    pub fn href_in(&self, namespace: &str, name: &str, origin: &Origin, base: &str) -> Option<String> {
        let href = self.prop(namespace, name)?.child(DAV, "href")?.text();
        origin.resolve(base, href.trim()).ok()
    }

    /// The entity tag, when it can be sent back in `If-Match` ([`token`]).
    #[must_use]
    pub fn etag(&self) -> Option<String> {
        self.prop(DAV, "getetag").and_then(|etag| token(&etag.text()))
    }

    /// Whether `DAV:resourcetype` names `name` in `namespace`.
    #[must_use]
    pub fn is_type(&self, namespace: &str, name: &str) -> bool {
        self.prop(DAV, "resourcetype").is_some_and(|kind| kind.child(namespace, name).is_some())
    }
}

/// Reads a `207 Multi-Status` body; references resolve against `base`, the request's path on `origin`.
///
/// # Errors
///
/// [`DavError::BadXml`] or [`DavError::TooLarge`] from the XML reader, and [`DavError::Missing`] when the root is
/// not `DAV:multistatus`.
pub fn read(origin: &Origin, base: &str, body: &[u8]) -> Result<Multistatus, DavError> {
    let root = xml::parse(body)?;
    if !root.is(DAV, "multistatus") {
        return Err(DavError::Missing("multistatus"));
    }
    let mut entries = Vec::new();
    for response in root.children_named(DAV, "response") {
        let status = response.child(DAV, "status").and_then(|status| status_code(&status.text()));
        let propstats: Vec<(u16, Element)> = response
            .children_named(DAV, "propstat")
            .filter_map(|propstat| {
                let code = status_code(&propstat.child(DAV, "status")?.text())?;
                Some((code, propstat.child(DAV, "prop")?.clone()))
            })
            .collect();
        let hrefs = response.children_named(DAV, "href").filter_map(|href| origin.resolve(base, href.text().trim()).ok());
        let hrefs: Vec<String> = if propstats.is_empty() { hrefs.collect() } else { hrefs.take(1).collect() };
        for href in hrefs {
            entries.push(Entry { href, status, propstats: propstats.clone() });
        }
    }
    let sync_token = root.child(DAV, "sync-token").and_then(|sync| token(&sync.text()));
    Ok(Multistatus { entries, sync_token })
}

/// The status code of an HTTP status line, `HTTP/1.1 404 Not Found`.
fn status_code(line: &str) -> Option<u16> {
    let code = line.split_whitespace().nth(1)?;
    code.parse::<u16>().ok().filter(|status| code.len() == 3 && (100..600).contains(status))
}

/// A sync token or entity tag as the server sent it, trimmed, when it can go into a request as it is: printable
/// ASCII and at most [`MAX_TOKEN`] bytes.
#[must_use]
pub fn token(text: &str) -> Option<String> {
    let text = text.trim();
    let good = !text.is_empty() && text.len() <= MAX_TOKEN && text.bytes().all(|byte| (0x20..0x7f).contains(&byte));
    good.then(|| text.to_owned())
}

/// Text from the server for showing: control characters dropped, space trimmed, at most `max_chars` characters.
#[must_use]
pub fn clean(text: &str, max_chars: usize) -> String {
    text.chars().filter(|c| !c.is_control()).collect::<String>().trim().chars().take(max_chars).collect::<String>().trim_end().to_owned()
}

/// A PROPFIND body asking for `props`, written with the prefixes `d` (DAV), `c` (CalDAV), `cs` (calendarserver) and
/// `a` (Apple iCal).
#[must_use]
pub fn propfind(props: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<d:propfind xmlns:d=\"{DAV}\" xmlns:c=\"{CALDAV}\" xmlns:cs=\"{CALENDARSERVER}\" \
         xmlns:a=\"{APPLE_ICAL}\"><d:prop>{props}</d:prop></d:propfind>"
    )
}

#[cfg(test)]
#[path = "multistatus_tests.rs"]
mod tests;
