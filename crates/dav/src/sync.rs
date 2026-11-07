// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bringing one calendar in step: a sync report (RFC 6578) from the last sync token, or, where the server has none, a
//! listing of entity tags compared with what is known; then the changed resources fetched with
//! `calendar-multiget` (RFC 4791 section 7.9).

use std::collections::{BTreeMap, BTreeSet};

use crate::http::{Method, Request, Response};
use crate::multistatus::{self, Multistatus};
use crate::xml::{self, CALDAV, DAV};
use crate::{DavError, Origin};

/// How many resources one `calendar-multiget` asks for.
pub const MAX_MULTIGET: usize = 50;
/// How many sync reports in a row one sync sends while the server cuts its answers short (507).
pub const MAX_ROUNDS: usize = 16;
/// How many resources one calendar may hold.
pub const MAX_RESOURCES: usize = 100_000;

/// One calendar object resource, as fetched.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resource {
    /// Its path on the origin, in its one form.
    pub href: String,
    /// Its entity tag, for `If-Match` when it is changed or removed; `None` when the server gave none that can be sent.
    pub etag: Option<String>,
    /// Its iCalendar text, as the server sent it; `inboxcraft-ical` reads it.
    pub data: String,
}

/// What changed in a calendar since the last sync.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Changes {
    /// Resources that are new or changed, by path.
    pub changed: Vec<Resource>,
    /// Paths of resources that are gone.
    pub removed: Vec<String>,
    /// The token to start the next sync from. `None` when the server has none, or when a resource that changed could
    /// not be fetched: the next sync then compares everything again, so nothing is lost.
    pub token: Option<String>,
    /// Whether the whole calendar was compared (a first sync, a token the server no longer knows, or no sync report):
    /// then every known path not in `changed` and still there is unchanged, and `removed` lists all the others.
    pub full: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Stage {
    /// A sync report from this token (`None`: the first one).
    Report { token: Option<String> },
    /// A PROPFIND of every member's entity tag: the server has no sync report.
    Listing,
    /// `calendar-multiget` for these paths.
    Fetch { batch: Vec<String> },
}

/// Syncing one calendar as a state machine: send [`Sync::request`], give the answer to [`Sync::receive`], until it
/// returns the [`Changes`].
///
/// A sync token the server no longer knows (`DAV:valid-sync-token`) starts again without one; a server without the
/// sync report (403, 405, 501 or 400) is listed with PROPFIND instead. Resources whose entity tag is new or
/// different from the known one are fetched, [`MAX_MULTIGET`] at a time.
///
/// ```
/// use inboxcraft_dav::{Method, Origin, Sync};
///
/// let (origin, _) = Origin::parse("https://dav.example.org/")?;
/// let known = [("/calendars/ada/work/a.ics".to_owned(), "\"1\"".to_owned())];
/// let sync = Sync::new(&origin, "/calendars/ada/work/", Some("http://example.org/sync/1"), known)?;
/// let request = sync.request()?;
/// assert_eq!((request.method(), request.header("Depth")), (Method::Report, Some("0")));
/// assert!(String::from_utf8_lossy(request.body()).contains("<d:sync-token>http://example.org/sync/1</d:sync-token>"));
/// # Ok::<(), inboxcraft_dav::DavError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sync {
    origin: Origin,
    path: String,
    known: BTreeMap<String, String>,
    /// `None` once the changes were handed out or an error ended the sync.
    stage: Option<Stage>,
    /// Members the server listed, with their entity tag when it gave one that can be sent.
    listed: BTreeMap<String, Option<String>>,
    removed: BTreeSet<String>,
    full: bool,
    rounds: usize,
    token: Option<String>,
    /// Paths still to fetch, the next one last; `None` until the listing is done.
    queue: Option<Vec<String>>,
    changes: Vec<Resource>,
    left_out: bool,
}

impl Sync {
    /// Syncs the calendar at `href` on `origin` from `token`, the last sync's (`None` for a first sync), knowing the
    /// resources in `known` by path and entity tag.
    ///
    /// # Errors
    ///
    /// [`DavError::BadUrl`] or [`DavError::OtherOrigin`] when `href` is not a path on `origin`.
    pub fn new(origin: &Origin, href: &str, token: Option<&str>, known: impl IntoIterator<Item = (String, String)>) -> Result<Self, DavError> {
        let path = origin.resolve("/", href)?;
        let known = known.into_iter().filter_map(|(href, etag)| Some((origin.resolve("/", &href).ok()?, etag))).collect();
        let token = token.and_then(multistatus::token);
        Ok(Self {
            origin: origin.clone(),
            path,
            known,
            stage: Some(Stage::Report { token }),
            listed: BTreeMap::new(),
            removed: BTreeSet::new(),
            full: false,
            rounds: 0,
            token: None,
            queue: None,
            changes: Vec::new(),
            left_out: false,
        })
    }

    /// The request to send now.
    ///
    /// # Errors
    ///
    /// [`DavError::Finished`] once the changes were handed out.
    pub fn request(&self) -> Result<Request, DavError> {
        let xml_body = "application/xml; charset=utf-8";
        Ok(match self.stage.as_ref().ok_or(DavError::Finished)? {
            Stage::Report { token } => {
                let token = xml::escape(token.as_deref().unwrap_or_default());
                let body = format!(
                    "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<d:sync-collection xmlns:d=\"{DAV}\"><d:sync-token>{token}</d:sync-token>\
                     <d:sync-level>1</d:sync-level><d:prop><d:getetag/></d:prop></d:sync-collection>"
                );
                Request::new(Method::Report, &self.origin, &self.path).with_header("Depth", "0").with_body(xml_body, body)
            }
            Stage::Listing => Request::new(Method::Propfind, &self.origin, &self.path)
                .with_header("Depth", "1")
                .with_body(xml_body, multistatus::propfind("<d:getetag/><d:resourcetype/>")),
            Stage::Fetch { batch } => {
                let hrefs: String = batch.iter().map(|href| format!("<d:href>{}</d:href>", xml::escape(href))).collect();
                let body = format!(
                    "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<c:calendar-multiget xmlns:d=\"{DAV}\" xmlns:c=\"{CALDAV}\">\
                     <d:prop><d:getetag/><c:calendar-data/></d:prop>{hrefs}</c:calendar-multiget>"
                );
                Request::new(Method::Report, &self.origin, &self.path).with_header("Depth", "1").with_body(xml_body, body)
            }
        })
    }

    /// Takes the server's answer to [`Sync::request`]: `None` while there is another request to send.
    ///
    /// # Errors
    ///
    /// [`DavError::Unauthorized`] for a refused password, [`DavError::Refused`] for a status the sync cannot go on
    /// from, [`DavError::TooLarge`] past [`MAX_RESOURCES`], and the reading errors of a bad answer. The sync is over
    /// after an error; the last token stays good.
    pub fn receive(&mut self, response: &Response) -> Result<Option<Changes>, DavError> {
        let stage = self.stage.take().ok_or(DavError::Finished)?;
        if response.status() == 401 {
            return Err(DavError::Unauthorized);
        }
        match stage {
            Stage::Report { token } => self.reported(token, response)?,
            Stage::Listing => self.listing(response)?,
            Stage::Fetch { batch } => self.fetched(&batch, response)?,
        }
        if self.stage.is_none() {
            self.stage = self.next_batch();
        }
        Ok(if self.stage.is_none() { Some(self.changes()) } else { None })
    }

    fn reported(&mut self, token: Option<String>, response: &Response) -> Result<(), DavError> {
        if response.status() != 207 {
            if matches!(response.status(), 403 | 409) && token.is_some() && names_invalid_token(response.body()) {
                self.restart(Stage::Report { token: None });
                return Ok(());
            }
            if matches!(response.status(), 400 | 403 | 405 | 501) && !names_invalid_token(response.body()) {
                self.restart(Stage::Listing);
                return Ok(());
            }
            return Err(DavError::Refused { status: response.status() });
        }
        let answer = self.answer(response)?;
        let mut more = false;
        for entry in answer.entries {
            if entry.href == self.path {
                more |= entry.status == Some(507);
            } else if entry.status == Some(404) {
                self.listed.remove(&entry.href);
                self.removed.insert(entry.href);
            } else {
                self.removed.remove(&entry.href);
                let etag = entry.etag();
                self.list(entry.href, etag)?;
            }
        }
        if token.is_none() {
            self.full = true;
        }
        self.token = answer.sync_token;
        self.rounds = self.rounds.saturating_add(1);
        if more && self.rounds < MAX_ROUNDS && self.token.is_some() {
            self.stage = Some(Stage::Report { token: self.token.clone() });
        }
        Ok(())
    }

    fn listing(&mut self, response: &Response) -> Result<(), DavError> {
        if response.status() != 207 {
            return Err(DavError::Refused { status: response.status() });
        }
        let answer = self.answer(response)?;
        for entry in answer.entries {
            if entry.href != self.path && entry.status.is_none() && !entry.is_type(DAV, "collection") {
                let etag = entry.etag();
                self.list(entry.href, etag)?;
            }
        }
        self.full = true;
        self.token = None;
        Ok(())
    }

    fn fetched(&mut self, batch: &[String], response: &Response) -> Result<(), DavError> {
        if response.status() != 207 {
            return Err(DavError::Refused { status: response.status() });
        }
        let answer = self.answer(response)?;
        let mut answered = BTreeSet::new();
        for entry in answer.entries {
            if !batch.contains(&entry.href) || answered.contains(&entry.href) {
                continue;
            }
            if entry.status == Some(404) {
                answered.insert(entry.href.clone());
                self.listed.remove(&entry.href);
                self.removed.insert(entry.href);
            } else if let Some(data) = entry.prop(CALDAV, "calendar-data").map(xml::Element::text) {
                answered.insert(entry.href.clone());
                let etag = entry.etag().or_else(|| self.listed.get(&entry.href).cloned().flatten());
                self.changes.push(Resource { href: entry.href, etag, data });
            }
        }
        self.left_out |= answered.len() < batch.len();
        Ok(())
    }

    /// Reads a multistatus answer about this calendar.
    fn answer(&self, response: &Response) -> Result<Multistatus, DavError> {
        multistatus::read(&self.origin, &self.path, response.body())
    }

    fn list(&mut self, href: String, etag: Option<String>) -> Result<(), DavError> {
        if self.listed.len() >= MAX_RESOURCES && !self.listed.contains_key(&href) {
            return Err(DavError::TooLarge { what: "calendar", limit: MAX_RESOURCES });
        }
        self.listed.insert(href, etag);
        Ok(())
    }

    /// Forgets what was read so far and goes on at `stage`.
    fn restart(&mut self, stage: Stage) {
        self.full = false;
        self.listed.clear();
        self.removed.clear();
        self.stage = Some(stage);
    }

    /// The next `calendar-multiget`, once the listing is done; `None` when nothing is left to fetch.
    fn next_batch(&mut self) -> Option<Stage> {
        let known = &self.known;
        let listed = &self.listed;
        let queue = self.queue.get_or_insert_with(|| {
            listed.iter().filter(|(href, etag)| etag.is_none() || known.get(*href) != etag.as_ref()).map(|(href, _)| href.clone()).rev().collect()
        });
        let batch: Vec<String> = queue.split_off(queue.len().saturating_sub(MAX_MULTIGET)).into_iter().rev().collect();
        (!batch.is_empty()).then_some(Stage::Fetch { batch })
    }

    fn changes(&mut self) -> Changes {
        let mut removed = std::mem::take(&mut self.removed);
        if self.full {
            removed.extend(self.known.keys().filter(|href| !self.listed.contains_key(*href)).cloned());
        }
        removed.retain(|href| !self.listed.contains_key(href));
        Changes {
            changed: std::mem::take(&mut self.changes),
            removed: removed.into_iter().collect(),
            token: if self.left_out { None } else { self.token.take() },
            full: self.full,
        }
    }
}

/// Whether an error body names the `DAV:valid-sync-token` precondition (RFC 6578 section 3.2).
fn names_invalid_token(body: &[u8]) -> bool {
    xml::parse(body).is_ok_and(|root| root.is(DAV, "error") && root.child(DAV, "valid-sync-token").is_some())
}

#[cfg(test)]
#[path = "sync_tests.rs"]
mod tests;
