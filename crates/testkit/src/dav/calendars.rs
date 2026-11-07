// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The fake's calendars: collections of iCalendar resources with entity tags, a change log for sync tokens, and a
//! handle tests share with the server to change things "elsewhere" and to look at what the client wrote.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// What every sync token of the fake starts with.
const TOKEN_PREFIX: &str = "http://example.org/ns/sync/";

/// The calendars of a [`crate::dav::FakeDav`], shared: a clone sees and changes the same calendars, also while the
/// server runs on its thread.
#[derive(Debug, Clone, Default)]
pub struct Calendars(Arc<Mutex<State>>);

#[derive(Debug, Default)]
pub(crate) struct State {
    pub(crate) collections: BTreeMap<String, Collection>,
    etags: u64,
    /// The change counter: every change gets the next number.
    seq: u64,
    /// Bumped by [`Calendars::forget_tokens`]: tokens of an earlier epoch are no longer known.
    epoch: u64,
}

/// One calendar collection.
#[derive(Debug, Clone, Default)]
pub(crate) struct Collection {
    pub(crate) name: String,
    pub(crate) color: String,
    pub(crate) components: Vec<String>,
    pub(crate) objects: BTreeMap<String, Object>,
    /// Removed resources and the change that removed them.
    gone: BTreeMap<String, u64>,
    /// The last change in the collection.
    pub(crate) changed: u64,
}

/// One iCalendar resource.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Object {
    pub(crate) etag: String,
    pub(crate) data: String,
    seq: u64,
}

/// What changed in a collection since a sync token, oldest first: the path and the resource, or `None` when it was
/// removed.
pub(crate) type Log = Vec<(String, Option<Object>)>;

impl Calendars {
    /// Adds an empty calendar collection at `path` (a leading and a trailing `/` are added when missing) with the
    /// display name `name`, taking the `components` (`VEVENT`, `VTODO`); a collection already there is kept.
    pub fn add(&self, path: &str, name: &str, components: &[&str]) -> String {
        let path = format!("/{}/", path.trim_matches('/'));
        let collection = Collection {
            name: name.to_owned(),
            color: "#3366CCFF".to_owned(),
            components: components.iter().map(|component| (*component).to_owned()).collect(),
            ..Collection::default()
        };
        self.lock().collections.entry(path.clone()).or_insert(collection);
        path
    }

    /// Writes `data` at `href`, as another client would, and returns its new entity tag; `None` when `href` is not
    /// in a calendar collection.
    pub fn put(&self, href: &str, data: &str) -> Option<String> {
        self.lock().put(href, data)
    }

    /// Removes the resource at `href`, as another client would; whether it was there.
    pub fn remove(&self, href: &str) -> bool {
        self.lock().remove(href)
    }

    /// The entity tag and the iCalendar text at `href`.
    #[must_use]
    pub fn get(&self, href: &str) -> Option<(String, String)> {
        let state = self.lock();
        let object = state.object(href)?;
        Some((object.etag.clone(), object.data.clone()))
    }

    /// The paths of the resources in the calendar at `calendar`, in order.
    #[must_use]
    pub fn hrefs(&self, calendar: &str) -> Vec<String> {
        self.lock().collections.get(calendar).map(|collection| collection.objects.keys().cloned().collect()).unwrap_or_default()
    }

    /// The calendar's sync token now.
    #[must_use]
    pub fn token(&self, calendar: &str) -> Option<String> {
        let state = self.lock();
        state.collections.contains_key(calendar).then(|| state.token(state.seq))
    }

    /// Forgets every sync token handed out so far, as a server does that lost its change log: a sync report from
    /// one of them is refused with `DAV:valid-sync-token`.
    pub fn forget_tokens(&self) {
        let mut state = self.lock();
        state.epoch = state.epoch.saturating_add(1);
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, State> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl State {
    pub(crate) fn put(&mut self, href: &str, data: &str) -> Option<String> {
        let (parent, _) = split(href)?;
        self.etags = self.etags.saturating_add(1);
        self.seq = self.seq.saturating_add(1);
        let etag = format!("\"{}\"", self.etags);
        let seq = self.seq;
        let collection = self.collections.get_mut(parent)?;
        collection.gone.remove(href);
        collection.objects.insert(href.to_owned(), Object { etag: etag.clone(), data: data.to_owned(), seq });
        collection.changed = seq;
        Some(etag)
    }

    pub(crate) fn remove(&mut self, href: &str) -> bool {
        let Some((parent, _)) = split(href) else { return false };
        let seq = self.seq.saturating_add(1);
        let Some(collection) = self.collections.get_mut(parent) else { return false };
        if collection.objects.remove(href).is_none() {
            return false;
        }
        collection.gone.insert(href.to_owned(), seq);
        collection.changed = seq;
        self.seq = seq;
        true
    }

    pub(crate) fn object(&self, href: &str) -> Option<&Object> {
        let (parent, _) = split(href)?;
        self.collections.get(parent)?.objects.get(href)
    }

    /// The number of the newest change.
    pub(crate) fn seq(&self) -> u64 {
        self.seq
    }

    pub(crate) fn token(&self, seq: u64) -> String {
        format!("{TOKEN_PREFIX}{}-{seq}", self.epoch)
    }

    /// What changed in the collection at `path` since `token` (empty: everything there now), oldest first, and the
    /// number of the newest change; `None` when the token is not one of the fake's or no longer known.
    pub(crate) fn since(&self, path: &str, token: &str) -> Option<(Log, u64)> {
        let collection = self.collections.get(path)?;
        let from = if token.is_empty() {
            None
        } else {
            let (epoch, seq) = token.strip_prefix(TOKEN_PREFIX)?.split_once('-')?;
            let (epoch, seq) = (epoch.parse::<u64>().ok()?, seq.parse::<u64>().ok()?);
            if epoch != self.epoch || seq > self.seq {
                return None;
            }
            Some(seq)
        };
        let mut log: Vec<(u64, String, Option<Object>)> = collection
            .objects
            .iter()
            .filter(|(_, object)| from.is_none_or(|from| object.seq > from))
            .map(|(href, object)| (object.seq, href.clone(), Some(object.clone())))
            .collect();
        if let Some(from) = from {
            log.extend(collection.gone.iter().filter(|(_, seq)| **seq > from).map(|(href, seq)| (*seq, href.clone(), None)));
        }
        log.sort_by_key(|(seq, _, _)| *seq);
        Some((log.into_iter().map(|(_, href, object)| (href, object)).collect(), self.seq))
    }

    /// The change number of an entry of a change log.
    pub(crate) fn seq_of(&self, path: &str, href: &str, object: Option<&Object>) -> u64 {
        match object {
            Some(object) => object.seq,
            None => self.collections.get(path).and_then(|collection| collection.gone.get(href).copied()).unwrap_or(self.seq),
        }
    }
}

/// Splits a resource path into its collection (with its trailing `/`) and its name.
fn split(href: &str) -> Option<(&str, &str)> {
    let at = href.rfind('/')?;
    let name = href.get(at.saturating_add(1)..)?;
    if name.is_empty() {
        return None;
    }
    Some((href.get(..=at)?, name))
}
