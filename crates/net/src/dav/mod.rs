// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The calendar jobs: `dav`'s state machines driven over HTTPS.
//!
//! A job runs at once on a connection of its own, kept alive for its requests and closed at its end: discovery from
//! the account's CalDAV address (the well-known path, the principal, the calendar home set), a sync of one calendar
//! from its last sync token or by entity tags, and the PUT or DELETE of one event with `If-Match` or
//! `If-None-Match: *`, so that a change made elsewhere comes back as [`CalendarReport::Conflict`] and is never
//! overwritten.
//!
//! The password goes with every request, as Basic auth, only to the origin of the account's CalDAV address and only
//! over TLS whose certificate verified: a job URL that is not `https` fails with [`ErrorKind::NotEncrypted`] and one
//! on another origin with [`ErrorKind::OtherServer`] before the password is asked for, and a redirect to another
//! origin with [`ErrorKind::OtherServer`]. A refused password is [`ErrorKind::LoginRefused`]; the worker then sends it to the
//! CalDAV server no more until the owner checks the account.

mod connection;

use inboxcraft_dav::{self as dav, Changes, DavError, Deleted, Discovery, Origin, Precondition, Sync, Written};
use inboxcraft_model::{Account, CalendarId, CalendarJob, CalendarListing, CalendarReport, ErrorKind, FetchedEvent, KnownEvent};

pub(crate) use connection::Session;

use crate::{Connector, Credentials, NetError, Password};

/// Runs `job` for `account` on a connection of its own, signed in with the account's password, asked for first: none
/// means no connection at all.
///
/// # Errors
///
/// [`ErrorKind::NoCalendarServer`] without an address, [`ErrorKind::NotEncrypted`] and [`ErrorKind::OtherServer`] for
/// a job URL that is not `https` or not on the address's origin, [`ErrorKind::CredentialMissing`], and the errors of
/// [`Session::exchange`] and of `dav` (see [`failure`]).
pub(crate) fn run(account: &Account, connector: &Connector, credentials: &dyn Credentials, job: &CalendarJob) -> Result<CalendarReport, NetError> {
    // No address, or a URL elsewhere: no password asked for.
    let caldav = account.caldav().ok_or_else(|| NetError::new(ErrorKind::NoCalendarServer, "no CalDAV address set"))?;
    let start = match job {
        CalendarJob::Discover { url } => url.as_deref().unwrap_or(caldav),
        CalendarJob::Sync { url, .. } | CalendarJob::Put { url, .. } | CalendarJob::Delete { url, .. } => url,
    };
    held_to(caldav, start)?;
    let password = credentials.password(account).ok_or_else(|| NetError::new(ErrorKind::CredentialMissing, "no password stored"))?;
    let mut session = session(connector, account.username(), &password)?;
    drop(password);
    let report = match job {
        CalendarJob::Discover { .. } => discover(&mut session, start).map(found),
        CalendarJob::Sync { calendar, url, token, known } => {
            sync(&mut session, url, token.as_deref(), known).map(|(origin, changes)| synced(*calendar, &origin, changes))
        }
        CalendarJob::Put { calendar, event, url, etag, ical } => put(&mut session, url, etag.as_deref(), ical).map(|written| match written {
            Written::Stored { etag } => CalendarReport::Written { calendar: *calendar, event: *event, etag },
            Written::Conflict => CalendarReport::Conflict { calendar: *calendar, event: *event },
        }),
        CalendarJob::Delete { calendar, event, url, etag } => delete(&mut session, url, etag.as_deref()).map(|deleted| match deleted {
            Deleted::Removed | Deleted::Gone => CalendarReport::Deleted { calendar: *calendar, event: *event },
            Deleted::Conflict => CalendarReport::Conflict { calendar: *calendar, event: *event },
        }),
    };
    session.close();
    report
}

/// Holds `url` to the origin of `caldav`, the account's CalDAV address: the password goes nowhere else.
///
/// # Errors
///
/// [`ErrorKind::NotEncrypted`] for a URL that is not `https`, [`ErrorKind::OtherServer`] for one on another origin,
/// and the errors of a URL that cannot be used (see [`failure`]).
fn held_to(caldav: &str, url: &str) -> Result<(), NetError> {
    let (origin, _) = Origin::parse(url).map_err(|error| failure(&error))?;
    if !origin.is_encrypted() {
        return Err(failure(&DavError::NotEncrypted));
    }
    let (own, _) = Origin::parse(caldav).map_err(|error| failure(&error))?;
    if origin == own { Ok(()) } else { Err(failure(&DavError::OtherOrigin { url: origin.to_string() })) }
}

/// A session signed in as `user` with `password`.
///
/// # Errors
///
/// [`ErrorKind::NoLoginMethod`] for a user name or password Basic auth cannot carry (a colon in the user name, a
/// control character).
pub(crate) fn session<'a>(connector: &'a Connector, user: &str, password: &Password) -> Result<Session<'a>, NetError> {
    let login = dav::Credentials::new(user, password.expose()).map_err(|error| NetError::new(ErrorKind::NoLoginMethod, error.to_string()))?;
    Ok(Session::new(connector, login))
}

/// Finds the calendars, starting at `url`.
pub(crate) fn discover(session: &mut Session<'_>, url: &str) -> Result<dav::Account, NetError> {
    let mut discovery = Discovery::new(url).map_err(|error| failure(&error))?;
    loop {
        let request = discovery.request().map_err(|error| failure(&error))?;
        let response = session.exchange(&request)?;
        if let Some(found) = discovery.receive(&response).map_err(|error| failure(&error))? {
            return Ok(found);
        }
    }
}

fn found(account: dav::Account) -> CalendarReport {
    let origin = &account.origin;
    let calendars = account
        .calendars
        .iter()
        .map(|calendar| CalendarListing {
            url: origin.url(&calendar.href),
            name: calendar.name.clone(),
            colour: calendar.color.clone(),
            ctag: calendar.ctag.clone(),
            sync_token: calendar.sync_token.clone(),
            events: calendar.holds_events(),
        })
        .collect();
    CalendarReport::Found { home: origin.url(&account.home), calendars }
}

/// Brings the calendar at `url` in step from `token`, knowing `known`.
fn sync(session: &mut Session<'_>, url: &str, token: Option<&str>, known: &[KnownEvent]) -> Result<(Origin, Changes), NetError> {
    let (origin, path) = Origin::parse(url).map_err(|error| failure(&error))?;
    let known = known.iter().map(|event| (event.url.clone(), event.etag.clone()));
    let mut sync = Sync::new(&origin, &path, token, known).map_err(|error| failure(&error))?;
    loop {
        let request = sync.request().map_err(|error| failure(&error))?;
        let response = session.exchange(&request)?;
        if let Some(changes) = sync.receive(&response).map_err(|error| failure(&error))? {
            return Ok((origin, changes));
        }
    }
}

fn synced(calendar: CalendarId, origin: &Origin, changes: Changes) -> CalendarReport {
    CalendarReport::Synced {
        calendar,
        changed: changes
            .changed
            .into_iter()
            .map(|resource| FetchedEvent { url: origin.url(&resource.href), etag: resource.etag, ical: resource.data })
            .collect(),
        removed: changes.removed.iter().map(|href| origin.url(href)).collect(),
        token: changes.token,
        full: changes.full,
    }
}

/// Writes `ical` at `url`: new where `etag` is `None`, over the copy with `etag` otherwise.
fn put(session: &mut Session<'_>, url: &str, etag: Option<&str>, ical: &str) -> Result<Written, NetError> {
    let (origin, path) = Origin::parse(url).map_err(|error| failure(&error))?;
    let precondition = etag.map_or(Precondition::Create, |etag| Precondition::Replace(etag.to_owned()));
    let request = dav::put(&origin, &path, ical, &precondition).map_err(|error| failure(&error))?;
    let response = session.exchange(&request)?;
    dav::put_outcome(&response).map_err(|error| failure(&error))
}

/// Removes the event at `url`, if it still has `etag`.
fn delete(session: &mut Session<'_>, url: &str, etag: Option<&str>) -> Result<Deleted, NetError> {
    let (origin, path) = Origin::parse(url).map_err(|error| failure(&error))?;
    let request = dav::delete(&origin, &path, etag).map_err(|error| failure(&error))?;
    let response = session.exchange(&request)?;
    dav::delete_outcome(&response).map_err(|error| failure(&error))
}

/// A `dav` error as the owner is told it: one [`ErrorKind`] per cause, never the server's words.
pub(crate) fn failure(error: &DavError) -> NetError {
    let kind = match error {
        DavError::NotEncrypted => ErrorKind::NotEncrypted,
        DavError::OtherOrigin { .. } => ErrorKind::OtherServer,
        DavError::Unauthorized => ErrorKind::LoginRefused,
        DavError::Refused { status: 404 | 410 } | DavError::BadUrl(_) => ErrorKind::NoCalendarServer,
        DavError::Missing(what) if *what != "redirect location" => ErrorKind::NoCalendarServer,
        DavError::Refused { .. } | DavError::BadArgument(_) => ErrorKind::CommandRefused,
        DavError::TooLarge { .. } => ErrorKind::TooLarge,
        _ => ErrorKind::Protocol,
    };
    NetError::new(kind, error.to_string())
}

#[cfg(test)]
mod tests;
