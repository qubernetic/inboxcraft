// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Finding the owner's calendars (RFC 6764 section 6, RFC 4791 section 6.2.1, RFC 5397): the context path, the
//! principal, the calendar home set, the calendars in it.

use crate::http::{Method, Request, Response};
use crate::multistatus::{self, Entry};
use crate::xml::{APPLE_ICAL, CALDAV, CALENDARSERVER, DAV};
use crate::{DavError, Origin};

/// How many redirects in a row an exchange follows.
pub const MAX_REDIRECTS: usize = 5;

/// The longest calendar name kept, in characters.
const MAX_NAME_CHARS: usize = 256;
/// The longest calendar colour kept, in characters.
const MAX_COLOR_CHARS: usize = 32;
/// How many component names of one calendar are kept.
const MAX_COMPONENTS: usize = 16;

/// The well-known path of a CalDAV service (RFC 6764 section 5).
pub const WELL_KNOWN: &str = "/.well-known/caldav";

/// One calendar collection in the calendar home set.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Calendar {
    /// The collection's path on the origin, in its one form.
    pub href: String,
    /// Its `DAV:displayname`, cleaned.
    pub name: Option<String>,
    /// Its colour (`calendar-color`), as the server wrote it, cleaned.
    pub color: Option<String>,
    /// Its `getctag`: changes whenever anything in it changes.
    pub ctag: Option<String>,
    /// Its `DAV:sync-token` (RFC 6578): the server supports sync reports on it.
    pub sync_token: Option<String>,
    /// The components it takes (`VEVENT`, `VTODO`), empty when the server did not say (then it takes any).
    pub components: Vec<String>,
}

impl Calendar {
    /// Whether it may hold events: it names `VEVENT`, or names nothing.
    #[must_use]
    pub fn holds_events(&self) -> bool {
        self.components.is_empty() || self.components.iter().any(|component| component == "VEVENT")
    }

    fn from_entry(entry: &Entry) -> Self {
        let components = entry
            .prop(CALDAV, "supported-calendar-component-set")
            .map(|set| {
                set.children_named(CALDAV, "comp")
                    .filter_map(|comp| comp.attribute("name"))
                    .map(|name| multistatus::clean(name, 32).to_ascii_uppercase())
                    .filter(|name| !name.is_empty())
                    .take(MAX_COMPONENTS)
                    .collect()
            })
            .unwrap_or_default();
        Self {
            href: entry.href.clone(),
            name: entry.text(DAV, "displayname", MAX_NAME_CHARS),
            color: entry.text(APPLE_ICAL, "calendar-color", MAX_COLOR_CHARS),
            ctag: entry.prop(CALENDARSERVER, "getctag").and_then(|ctag| multistatus::token(&ctag.text())),
            sync_token: entry.prop(DAV, "sync-token").and_then(|token| multistatus::token(&token.text())),
            components,
        }
    }
}

/// What discovery found: where the owner's principal and calendar home set are, and the calendars in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account {
    /// The origin every path is on.
    pub origin: Origin,
    /// The principal's path.
    pub principal: String,
    /// The calendar home set's path.
    pub home: String,
    /// The calendar collections in the home set, in the server's order.
    pub calendars: Vec<Calendar>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Stage {
    /// Asking the context path (the well-known path, or the path the owner gave) for the principal.
    Context { path: String, well_known: bool },
    /// Asking the principal for the calendar home set.
    Principal { path: String },
    /// Listing the calendar home set.
    Home { principal: String, path: String },
}

impl Stage {
    fn path(&self) -> &str {
        match self {
            Self::Context { path, .. } | Self::Principal { path } | Self::Home { path, .. } => path,
        }
    }
}

/// Discovery as a state machine: send [`Discovery::request`], give the answer to [`Discovery::receive`], until it
/// returns the [`Account`].
///
/// Redirects are followed on the same origin only, at most [`MAX_REDIRECTS`] in a row; one to another origin ends
/// discovery with [`DavError::OtherOrigin`]. When the well-known path is missing (404, 405, 403 or 501), the
/// context path `/` is asked instead.
///
/// ```
/// use inboxcraft_dav::{Discovery, Method, Response};
///
/// let mut discovery = Discovery::new("https://dav.example.org")?;
/// let request = discovery.request()?;
/// assert_eq!((request.method(), request.path()), (Method::Propfind, "/.well-known/caldav"));
/// let moved = Response::new(301).with_header("Location", "/dav/");
/// assert_eq!(discovery.receive(&moved)?, None);
/// assert_eq!(discovery.request()?.path(), "/dav/");
/// # Ok::<(), inboxcraft_dav::DavError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Discovery {
    origin: Origin,
    /// `None` once the account was found or an error ended discovery.
    stage: Option<Stage>,
    redirects: usize,
}

impl Discovery {
    /// Starts at `url`: an `https` (or, without a password, `http`) URL. Without a path, or with `/`, discovery
    /// starts at the well-known path.
    ///
    /// # Errors
    ///
    /// [`DavError::BadUrl`] when `url` cannot be used.
    pub fn new(url: &str) -> Result<Self, DavError> {
        let (origin, path) = Origin::parse(url)?;
        let stage =
            if path == "/" { Stage::Context { path: WELL_KNOWN.to_owned(), well_known: true } } else { Stage::Context { path, well_known: false } };
        Ok(Self { origin, stage: Some(stage), redirects: 0 })
    }

    /// The origin requests go to.
    #[must_use]
    pub fn origin(&self) -> &Origin {
        &self.origin
    }

    /// The request to send now.
    ///
    /// # Errors
    ///
    /// [`DavError::Finished`] once the account was found.
    pub fn request(&self) -> Result<Request, DavError> {
        let Some(stage) = &self.stage else { return Err(DavError::Finished) };
        let (path, depth, props) = match stage {
            Stage::Context { path, .. } => (path, "0", "<d:current-user-principal/><c:calendar-home-set/><d:resourcetype/>"),
            Stage::Principal { path } => (path, "0", "<c:calendar-home-set/>"),
            Stage::Home { path, .. } => {
                (path, "1", "<d:resourcetype/><d:displayname/><cs:getctag/><d:sync-token/><c:supported-calendar-component-set/><a:calendar-color/>")
            }
        };
        Ok(Request::new(Method::Propfind, &self.origin, path)
            .with_header("Depth", depth)
            .with_body("application/xml; charset=utf-8", multistatus::propfind(props)))
    }

    /// Takes the server's answer to [`Discovery::request`]: `None` while there is another request to send.
    ///
    /// # Errors
    ///
    /// [`DavError::Unauthorized`] for a refused password, [`DavError::Missing`] when the server does not name the
    /// principal or the calendar home set, [`DavError::OtherOrigin`] and [`DavError::TooManyRedirects`] for
    /// redirects not followed, [`DavError::Refused`] for any other status, and the reading errors of a bad answer.
    /// Discovery is over after an error.
    pub fn receive(&mut self, response: &Response) -> Result<Option<Account>, DavError> {
        let Some(stage) = self.stage.take() else { return Err(DavError::Finished) };
        let path = stage.path().to_owned();
        if response.status() == 401 {
            return Err(DavError::Unauthorized);
        }
        if matches!(response.status(), 301 | 302 | 303 | 307 | 308) {
            self.stage = Some(self.redirect(stage, &path, response)?);
            return Ok(None);
        }
        self.redirects = 0;
        if response.status() != 207 {
            if let Stage::Context { well_known: true, .. } = stage
                && matches!(response.status(), 403..=405 | 501)
            {
                self.stage = Some(Stage::Context { path: "/".to_owned(), well_known: false });
                return Ok(None);
            }
            return Err(DavError::Refused { status: response.status() });
        }
        let answer = multistatus::read(&self.origin, &path, response.body())?;
        match stage {
            Stage::Context { .. } => self.stage = Some(context(&self.origin, &path, &answer.entries)?),
            Stage::Principal { .. } => {
                let home =
                    first_href(&self.origin, &path, &answer.entries, CALDAV, "calendar-home-set").ok_or(DavError::Missing("calendar home set"))?;
                self.stage = Some(Stage::Home { principal: path, path: home });
            }
            Stage::Home { principal, .. } => {
                let calendars =
                    answer.entries.iter().filter(|entry| entry.href != path && entry.is_type(CALDAV, "calendar")).map(Calendar::from_entry).collect();
                return Ok(Some(Account { origin: self.origin.clone(), principal, home: path, calendars }));
            }
        }
        Ok(None)
    }

    /// The stage after a redirect from `path`: the same question at the new place.
    fn redirect(&mut self, stage: Stage, path: &str, response: &Response) -> Result<Stage, DavError> {
        self.redirects = self.redirects.saturating_add(1);
        if self.redirects > MAX_REDIRECTS {
            return Err(DavError::TooManyRedirects);
        }
        let location = response.header("Location").ok_or(DavError::Missing("redirect location"))?;
        let to = self.origin.resolve(path, location)?;
        Ok(match stage {
            Stage::Context { well_known, .. } => Stage::Context { path: to, well_known },
            Stage::Principal { .. } => Stage::Principal { path: to },
            Stage::Home { principal, .. } => Stage::Home { principal, path: to },
        })
    }
}

/// The stage after the context path answered: the principal, or the home set when the context names it.
fn context(origin: &Origin, path: &str, entries: &[Entry]) -> Result<Stage, DavError> {
    if entries.iter().any(|entry| entry.prop(DAV, "current-user-principal").is_some_and(|prop| prop.child(DAV, "unauthenticated").is_some())) {
        return Err(DavError::Unauthorized);
    }
    if let Some(principal) = first_href(origin, path, entries, DAV, "current-user-principal") {
        return Ok(Stage::Principal { path: principal });
    }
    match first_href(origin, path, entries, CALDAV, "calendar-home-set") {
        Some(home) => Ok(Stage::Home { principal: path.to_owned(), path: home }),
        None => Err(DavError::Missing("current user principal")),
    }
}

fn first_href(origin: &Origin, base: &str, entries: &[Entry], namespace: &str, name: &str) -> Option<String> {
    entries.iter().find_map(|entry| entry.href_in(namespace, name, origin, base))
}

#[cfg(test)]
#[path = "discovery_tests.rs"]
mod tests;
