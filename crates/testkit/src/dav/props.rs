// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The fake's resources and their properties, and the `207 Multi-Status` answers it writes.

use crate::dav::calendars::{Collection, Object, State};
use crate::dav::{HOME, PRINCIPAL};

/// The WebDAV namespace.
pub(crate) const DAV: &str = "DAV:";
/// The CalDAV namespace.
pub(crate) const CALDAV: &str = "urn:ietf:params:xml:ns:caldav";
/// The namespace of `getctag`.
const CALENDARSERVER: &str = "http://calendarserver.org/ns/";
/// The namespace of `calendar-color`.
const APPLE_ICAL: &str = "http://apple.com/ns/ical/";

/// The properties listed when a PROPFIND names none (an empty body, `allprop`, `propname`).
pub(crate) const DEFAULT_PROPS: [(&str, &str); 5] =
    [(DAV, "resourcetype"), (DAV, "displayname"), (DAV, "current-user-principal"), (DAV, "getetag"), (DAV, "getcontenttype")];

/// What a path names.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Kind<'a> {
    Root,
    Principal,
    Home,
    Calendar(&'a Collection),
    Object(&'a Object),
}

impl<'a> Kind<'a> {
    pub(crate) fn of(state: &'a State, path: &str) -> Option<Self> {
        match path {
            "/" => Some(Self::Root),
            PRINCIPAL => Some(Self::Principal),
            HOME => Some(Self::Home),
            _ => state.collections.get(path).map(Self::Calendar).or_else(|| state.object(path).map(Self::Object)),
        }
    }
}

/// The value of property `name` in `ns` of `kind`, as XML inside the property's element; `None` when it has none.
pub(crate) fn value(kind: Kind<'_>, ns: &str, name: &str, token: Option<&str>) -> Option<String> {
    let principal = format!("<d:href>{PRINCIPAL}</d:href>");
    match (kind, ns, name) {
        (Kind::Object(_), DAV, "current-user-principal") => None,
        (_, DAV, "current-user-principal") => Some(principal),
        (Kind::Principal, CALDAV, "calendar-home-set") => Some(format!("<d:href>{HOME}</d:href>")),
        (Kind::Root | Kind::Home, DAV, "resourcetype") => Some("<d:collection/>".to_owned()),
        (Kind::Principal, DAV, "resourcetype") => Some("<d:principal/>".to_owned()),
        (Kind::Calendar(_), DAV, "resourcetype") => Some("<d:collection/><c:calendar/>".to_owned()),
        (Kind::Object(_), DAV, "resourcetype") => Some(String::new()),
        (Kind::Principal, DAV, "displayname") => Some("Ada".to_owned()),
        (Kind::Home, DAV, "displayname") => Some("Calendars".to_owned()),
        (Kind::Calendar(calendar), DAV, "displayname") => Some(escape(&calendar.name)),
        (Kind::Calendar(calendar), CALENDARSERVER, "getctag") => Some(format!("\"{}\"", calendar.changed)),
        (Kind::Calendar(_), DAV, "sync-token") => token.map(escape),
        (Kind::Calendar(calendar), CALDAV, "supported-calendar-component-set") => {
            Some(calendar.components.iter().map(|component| format!("<c:comp name=\"{}\"/>", escape(component))).collect())
        }
        (Kind::Calendar(calendar), APPLE_ICAL, "calendar-color") => Some(escape(&calendar.color)),
        (Kind::Object(object), DAV, "getetag") => Some(escape(&object.etag)),
        (Kind::Object(_), DAV, "getcontenttype") => Some("text/calendar; charset=utf-8".to_owned()),
        (Kind::Object(object), CALDAV, "calendar-data") => Some(escape(&object.data)),
        _ => None,
    }
}

/// A `207 Multi-Status` body being written.
pub(crate) struct Multistatus {
    body: String,
}

impl Multistatus {
    pub(crate) fn new() -> Self {
        Self {
            body: format!(
                "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<d:multistatus xmlns:d=\"{DAV}\" xmlns:c=\"{CALDAV}\" \
                 xmlns:cs=\"{CALENDARSERVER}\" xmlns:a=\"{APPLE_ICAL}\">"
            ),
        }
    }

    /// An entry for `href` with the `props` asked for: those it has with `200`, the others with `404`.
    pub(crate) fn props(&mut self, href: &str, kind: Kind<'_>, props: &[(String, String)], token: Option<&str>) {
        let mut found = String::new();
        let mut missing = String::new();
        for (ns, name) in props {
            match value(kind, ns, name, token) {
                Some(inner) => found.push_str(&element(ns, name, &inner)),
                None => missing.push_str(&element(ns, name, "")),
            }
        }
        self.body.push_str(&format!("<d:response><d:href>{}</d:href>", escape(href)));
        for (status, props) in [("200 OK", found), ("404 Not Found", missing)] {
            if !props.is_empty() {
                self.body.push_str(&format!("<d:propstat><d:prop>{props}</d:prop><d:status>HTTP/1.1 {status}</d:status></d:propstat>"));
            }
        }
        self.body.push_str("</d:response>");
    }

    /// An entry for `href` with a status and no properties.
    pub(crate) fn status(&mut self, href: &str, status: &str) {
        self.body.push_str(&format!("<d:response><d:href>{}</d:href><d:status>HTTP/1.1 {status}</d:status></d:response>", escape(href)));
    }

    /// The body, with the sync token of a sync report.
    pub(crate) fn finish(mut self, token: Option<&str>) -> Vec<u8> {
        if let Some(token) = token {
            self.body.push_str(&format!("<d:sync-token>{}</d:sync-token>", escape(token)));
        }
        self.body.push_str("</d:multistatus>");
        self.body.into_bytes()
    }
}

/// A `DAV:error` body naming one precondition.
pub(crate) fn error(ns: &str, name: &str) -> Vec<u8> {
    format!("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<d:error xmlns:d=\"{DAV}\" xmlns:c=\"{CALDAV}\">{}</d:error>", element(ns, name, ""))
        .into_bytes()
}

/// The element `name` in `ns` around `inner`; a namespace without a prefix of the root is declared on it.
fn element(ns: &str, name: &str, inner: &str) -> String {
    let (prefix, declare) = match ns {
        DAV => ("d", String::new()),
        CALDAV => ("c", String::new()),
        CALENDARSERVER => ("cs", String::new()),
        APPLE_ICAL => ("a", String::new()),
        _ => ("x", format!(" xmlns:x=\"{}\"", escape(ns))),
    };
    if inner.is_empty() { format!("<{prefix}:{name}{declare}/>") } else { format!("<{prefix}:{name}{declare}>{inner}</{prefix}:{name}>") }
}

/// Escapes text for XML.
pub(crate) fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\r', "&#13;")
}
