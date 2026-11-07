// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The calendar jobs and what the worker reports back: finding the owner's CalDAV calendars, syncing one, and
//! writing or removing one event, each with its entity tag.
//!
//! Every place on the server is an absolute `https` URL, as discovery found it: a calendar can live on another host
//! than the mail. The worker runs a calendar job at once on an HTTPS connection of its own, so it never waits in the
//! IMAP queue; what it finds comes back as one [`CalendarReport`], in the shape the store's calendar tables keep
//! (the calendar with its sync token, each event with its URL, entity tag and iCalendar text).

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::text::clean_line;
use crate::{CalendarEventId, CalendarId, ErrorKind};

/// The longest calendar name kept, in characters.
const MAX_NAME_CHARS: usize = 256;
/// The longest calendar colour kept, in characters.
const MAX_COLOUR_CHARS: usize = 32;

/// An event the store holds: its URL and the entity tag of the copy held, so a sync fetches only what changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnownEvent {
    /// Its URL.
    pub url: String,
    /// The entity tag of the copy held.
    pub etag: String,
}

/// What to do with the account's calendars. Every URL is held to the origin of the account's CalDAV address: one
/// elsewhere fails with [`ErrorKind::OtherServer`] before the password is asked for.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CalendarJob {
    /// Find the calendars (RFC 6764: the well-known path, the principal, the calendar home set), starting at `url`,
    /// or at the account's CalDAV address when `None`; answered with [`CalendarReport::Found`]. Without a CalDAV
    /// address it fails with [`ErrorKind::NoCalendarServer`] before the password
    /// is asked for.
    Discover {
        /// Where to start: a URL on the origin of the account's CalDAV address, as every calendar job's URL is.
        url: Option<String>,
    },
    /// Bring one calendar in step from `token`, the last sync's; answered with [`CalendarReport::Synced`].
    Sync {
        /// Our id of the calendar.
        calendar: CalendarId,
        /// Its collection's URL.
        url: String,
        /// Where the last sync ended (`None`: compare everything).
        token: Option<String>,
        /// The events held, with their entity tags.
        known: Vec<KnownEvent>,
    },
    /// Write one event: a new one (`etag` `None`), which must not be there yet, or the copy with `etag`, which must
    /// not have changed elsewhere; answered with [`CalendarReport::Written`] or [`CalendarReport::Conflict`].
    Put {
        /// Our id of the calendar.
        calendar: CalendarId,
        /// Our id of the event.
        event: CalendarEventId,
        /// Its URL.
        url: String,
        /// The entity tag of the copy it replaces; `None` for a new event.
        etag: Option<String>,
        /// The iCalendar text.
        ical: String,
    },
    /// Remove one event, if it still has `etag`; answered with [`CalendarReport::Deleted`] or
    /// [`CalendarReport::Conflict`].
    Delete {
        /// Our id of the calendar.
        calendar: CalendarId,
        /// Our id of the event.
        event: CalendarEventId,
        /// Its URL.
        url: String,
        /// The entity tag of the copy seen; `None` removes whatever is there.
        etag: Option<String>,
    },
}

impl CalendarJob {
    /// The calendar the job is about, if any.
    #[must_use]
    pub fn calendar(&self) -> Option<CalendarId> {
        match self {
            Self::Discover { .. } => None,
            Self::Sync { calendar, .. } | Self::Put { calendar, .. } | Self::Delete { calendar, .. } => Some(*calendar),
        }
    }
}

impl fmt::Debug for CalendarJob {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Discover { url } => f.debug_struct("Discover").field("url", url).finish(),
            Self::Sync { calendar, url, token, known } => {
                f.debug_struct("Sync").field("calendar", calendar).field("url", url).field("token", token).field("known", &known.len()).finish()
            }
            Self::Put { calendar, event, url, etag, ical } => f
                .debug_struct("Put")
                .field("calendar", calendar)
                .field("event", event)
                .field("url", url)
                .field("etag", etag)
                .field("ical_bytes", &ical.len())
                .finish(),
            Self::Delete { calendar, event, url, etag } => {
                f.debug_struct("Delete").field("calendar", calendar).field("event", event).field("url", url).field("etag", etag).finish()
            }
        }
    }
}

/// One calendar discovery found. Its name and colour come from the server and are cleaned, also when read back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "RawListing")]
pub struct CalendarListing {
    /// Its collection's URL.
    pub url: String,
    /// Its name, one clean line.
    pub name: Option<String>,
    /// Its colour as the server gives it, one clean line.
    pub colour: Option<String>,
    /// Its `getctag`: changes whenever anything in it changes.
    pub ctag: Option<String>,
    /// Its sync token now; the server has the sync report on it.
    pub sync_token: Option<String>,
    /// Whether it may hold events (`VEVENT`, or no component named).
    pub events: bool,
}

/// A [`CalendarListing`] as stored, before its name and colour are cleaned.
#[derive(Deserialize)]
struct RawListing {
    url: String,
    name: Option<String>,
    colour: Option<String>,
    ctag: Option<String>,
    sync_token: Option<String>,
    events: bool,
}

impl From<RawListing> for CalendarListing {
    fn from(raw: RawListing) -> Self {
        Self {
            url: raw.url,
            name: raw.name.and_then(|name| clean_line(&name, MAX_NAME_CHARS)),
            colour: raw.colour.and_then(|colour| clean_line(&colour, MAX_COLOUR_CHARS)),
            ctag: raw.ctag,
            sync_token: raw.sync_token,
            events: raw.events,
        }
    }
}

/// An event a sync fetched. `Debug` shows the length of its text, never the text.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FetchedEvent {
    /// Its URL.
    pub url: String,
    /// Its entity tag; `None` when the server gave none that can be sent back.
    pub etag: Option<String>,
    /// Its iCalendar text, as the server sent it.
    pub ical: String,
}

impl fmt::Debug for FetchedEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FetchedEvent").field("url", &self.url).field("etag", &self.etag).field("ical_bytes", &self.ical.len()).finish()
    }
}

/// What a calendar job found or did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CalendarReport {
    /// The calendar home set and the calendars in it, in the server's order.
    Found {
        /// The calendar home set's URL.
        home: String,
        /// The calendars.
        calendars: Vec<CalendarListing>,
    },
    /// What changed in a calendar since the last sync.
    Synced {
        /// The calendar.
        calendar: CalendarId,
        /// Events new or changed.
        changed: Vec<FetchedEvent>,
        /// URLs of events gone.
        removed: Vec<String>,
        /// Where the next sync starts; `None` compares everything next time.
        token: Option<String>,
        /// The whole calendar was compared: a held event neither changed nor listed in `removed` is unchanged.
        full: bool,
    },
    /// The server took the event.
    Written {
        /// The calendar.
        calendar: CalendarId,
        /// The event.
        event: CalendarEventId,
        /// Its new entity tag, when the server sent one (otherwise the next sync has it).
        etag: Option<String>,
    },
    /// The event is gone from the server: removed now, or gone already.
    Deleted {
        /// The calendar.
        calendar: CalendarId,
        /// The event.
        event: CalendarEventId,
    },
    /// The event was created or changed elsewhere since it was seen (412): nothing was overwritten. Sync, then
    /// decide.
    Conflict {
        /// The calendar.
        calendar: CalendarId,
        /// The event.
        event: CalendarEventId,
    },
    /// The job failed.
    Failed {
        /// The calendar it was about, if any.
        calendar: Option<CalendarId>,
        /// Why.
        kind: ErrorKind,
    },
}

impl CalendarReport {
    /// The calendar the report is about, if any.
    #[must_use]
    pub fn calendar(&self) -> Option<CalendarId> {
        match self {
            Self::Found { .. } => None,
            Self::Synced { calendar, .. } | Self::Written { calendar, .. } | Self::Deleted { calendar, .. } | Self::Conflict { calendar, .. } => {
                Some(*calendar)
            }
            Self::Failed { calendar, .. } => *calendar,
        }
    }
}

#[cfg(test)]
#[path = "calendar_tests.rs"]
mod tests;
