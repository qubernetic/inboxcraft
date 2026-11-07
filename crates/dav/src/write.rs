// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Writing and removing one calendar object resource (RFC 4791 section 5.3.2), with entity tags so that a change
//! made elsewhere in the meantime is never overwritten (RFC 9110 section 13.1).

use crate::http::{Method, Request, Response};
use crate::multistatus;
use crate::{DavError, Origin};

/// The largest iCalendar text written, in bytes.
pub const MAX_PUT: usize = 4 * 1024 * 1024;

/// What a PUT may overwrite.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Precondition {
    /// A new resource: nothing may be there yet (`If-None-Match: *`).
    Create,
    /// The resource as last seen, with this entity tag (`If-Match`).
    Replace(String),
}

/// How a PUT ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Written {
    /// The server took it; its new entity tag, when it sent one that can be sent back (otherwise the next sync has it).
    Stored {
        /// The new entity tag.
        etag: Option<String>,
    },
    /// The precondition failed (412): the resource was created or changed elsewhere. Sync, then decide.
    Conflict,
}

/// How a DELETE ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Deleted {
    /// The server removed it.
    Removed,
    /// It was gone already (404 or 410).
    Gone,
    /// It was changed elsewhere since its entity tag was seen (412). Sync, then decide.
    Conflict,
}

/// A PUT of `calendar`, iCalendar text, to `href` on `origin`.
///
/// # Errors
///
/// [`DavError::BadUrl`] or [`DavError::OtherOrigin`] for an `href` that is not a path on `origin`,
/// [`DavError::BadArgument`] for an entity tag that cannot be sent, and [`DavError::TooLarge`] for a calendar longer
/// than [`MAX_PUT`].
///
/// ```
/// use inboxcraft_dav::{Origin, Precondition, Response, Written, put, put_outcome};
///
/// let (origin, _) = Origin::parse("https://dav.example.org/")?;
/// let request = put(&origin, "/cal/work/e1.ics", "BEGIN:VCALENDAR\r\nEND:VCALENDAR\r\n", &Precondition::Create)?;
/// assert_eq!(request.header("If-None-Match"), Some("*"));
/// let outcome = put_outcome(&Response::new(201).with_header("ETag", "\"2\""))?;
/// assert_eq!(outcome, Written::Stored { etag: Some("\"2\"".into()) });
/// # Ok::<(), inboxcraft_dav::DavError>(())
/// ```
pub fn put(origin: &Origin, href: &str, calendar: &str, precondition: &Precondition) -> Result<Request, DavError> {
    let path = origin.resolve("/", href)?;
    if calendar.len() > MAX_PUT {
        return Err(DavError::TooLarge { what: "calendar to write", limit: MAX_PUT });
    }
    let request = Request::new(Method::Put, origin, &path);
    let request = match precondition {
        Precondition::Create => request.with_header("If-None-Match", "*"),
        Precondition::Replace(etag) => request.with_header("If-Match", sendable(etag)?),
    };
    Ok(request.with_body("text/calendar; charset=utf-8", calendar.as_bytes().to_vec()))
}

/// How the PUT that got `response` ended.
///
/// # Errors
///
/// [`DavError::Unauthorized`] for a refused password, [`DavError::Refused`] for any other status that is not a
/// success.
pub fn put_outcome(response: &Response) -> Result<Written, DavError> {
    match response.status() {
        412 => Ok(Written::Conflict),
        401 => Err(DavError::Unauthorized),
        _ if response.is_success() => Ok(Written::Stored { etag: response.header("ETag").and_then(multistatus::token) }),
        status => Err(DavError::Refused { status }),
    }
}

/// A DELETE of `href` on `origin`, only if it still has `etag` when one is given.
///
/// # Errors
///
/// [`DavError::BadUrl`] or [`DavError::OtherOrigin`] for an `href` that is not a path on `origin`, and
/// [`DavError::BadArgument`] for an entity tag that cannot be sent.
pub fn delete(origin: &Origin, href: &str, etag: Option<&str>) -> Result<Request, DavError> {
    let request = Request::new(Method::Delete, origin, &origin.resolve("/", href)?);
    Ok(match etag {
        Some(etag) => request.with_header("If-Match", sendable(etag)?),
        None => request,
    })
}

/// How the DELETE that got `response` ended.
///
/// # Errors
///
/// [`DavError::Unauthorized`] for a refused password, [`DavError::Refused`] for any other status that is not a
/// success.
pub fn delete_outcome(response: &Response) -> Result<Deleted, DavError> {
    match response.status() {
        412 => Ok(Deleted::Conflict),
        404 | 410 => Ok(Deleted::Gone),
        401 => Err(DavError::Unauthorized),
        _ if response.is_success() => Ok(Deleted::Removed),
        status => Err(DavError::Refused { status }),
    }
}

/// An entity tag that can go into `If-Match` as it is.
fn sendable(etag: &str) -> Result<String, DavError> {
    multistatus::token(etag).ok_or(DavError::BadArgument("an entity tag that cannot be sent"))
}

#[cfg(test)]
#[path = "write_tests.rs"]
mod tests;
