// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The fake CalDAV server.
//!
//! [`FakeDav`] answers the requests of a CalDAV client (RFC 4791, RFC 4918, RFC 6578, RFC 6764) with its own small
//! HTTP and XML readers, never with `inboxcraft-dav`'s, so a parser bug there cannot hide behind the same bug here:
//! the well-known path, PROPFIND (depth 0 and 1) on the principal, the calendar home set, the calendars and their
//! resources, the reports `sync-collection`, `calendar-multiget` and `calendar-query` (its filter is not applied),
//! GET, PUT and DELETE with `If-Match` and `If-None-Match`, on one kept-alive connection.
//!
//! - It takes Basic auth over TLS only: over plain TCP it refuses every request with `403` and never asks for a
//!   password, unless [`FakeDav::auth_in_clear`] makes it a server that does. The transcript holds the head lines of
//!   every request, so a test sees whether a password went in the clear ([`crate::Transcript::in_clear`]).
//! - Its calendars are a shared [`Calendars`] handle: a test adds and changes resources "elsewhere" between syncs and
//!   looks at what the client wrote. Each change has a number; a sync token names the newest one it covers.
//!   [`Calendars::forget_tokens`] makes every earlier token unknown (`DAV:valid-sync-token`).
//! - [`FakeDav::without_sync`] makes a server without the sync report, [`FakeDav::page`] one that cuts sync answers
//!   short (`507`), [`FakeDav::without_well_known`] one without the well-known redirect, and [`FakeDav::on`] scripts
//!   answers and faults at a request.
//!
//! [`crate::Listen::start_dav`] serves it on a loopback port, over implicit TLS (or plain TCP, to test that nothing
//! goes in the clear), with the certificate faults of [`crate::CertFault`]. [`FakeDav::answer`] plays it without a
//! socket, as over TLS.
//!
//! ```
//! use inboxcraft_testkit::dav::{FakeDav, WORK};
//!
//! let fake = FakeDav::new();
//! fake.calendars().put(&format!("{WORK}e1.ics"), "BEGIN:VCALENDAR\r\nEND:VCALENDAR\r\n");
//! let out = fake.answer(
//!     b"PROPFIND /calendars/ada/work/ HTTP/1.1\r\nHost: dav.example.org\r\nDepth: 1\r\n\
//!       Authorization: Basic YWRhQGV4YW1wbGUub3JnOm5vdC1hLXJlYWwtcGFzc3dvcmQ=\r\n\r\n",
//! );
//! let out = String::from_utf8_lossy(&out);
//! assert!(out.starts_with("HTTP/1.1 207 Multi-Status\r\n"));
//! assert!(out.contains("<d:href>/calendars/ada/work/e1.ics</d:href>"));
//! ```

mod calendars;
mod props;
mod request;
mod rules;
mod session;
mod xml;

use std::io::{self, Read, Write};
use std::sync::atomic::AtomicBool;

pub use calendars::Calendars;
pub use request::MAX_REQUEST_BODY;
pub use rules::{Action, Reply, When};

use crate::Error;
use crate::server::{FakeServer, Listen};
use rules::Rules;
use session::Session;

/// The user name [`FakeDav::new`] takes.
pub const USER: &str = "ada@example.org";
/// The password [`FakeDav::new`] takes; invented, like every fixture.
pub const PASSWORD: &str = "not-a-real-password";
/// The well-known path (RFC 6764); it redirects to `/`.
pub const WELL_KNOWN: &str = "/.well-known/caldav";
/// The principal's path.
pub const PRINCIPAL: &str = "/principals/ada/";
/// The calendar home set's path.
pub const HOME: &str = "/calendars/ada/";
/// The calendar [`FakeDav::new`] has, for events.
pub const WORK: &str = "/calendars/ada/work/";

/// A fake CalDAV server: its user, its calendars, what it supports and the scripted points of its session.
#[derive(Debug, Clone)]
pub struct FakeDav {
    user: String,
    password: String,
    auth_in_clear: bool,
    well_known: bool,
    sync: bool,
    page: Option<usize>,
    rules: Rules,
    calendars: Calendars,
}

impl Default for FakeDav {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeDav {
    /// The user [`USER`] with [`PASSWORD`], Basic auth over TLS only, the well-known redirect, the sync report, and
    /// one empty calendar for events, [`WORK`], named `Work`.
    #[must_use]
    pub fn new() -> Self {
        let calendars = Calendars::default();
        calendars.add(WORK, "Work", &["VEVENT"]);
        Self {
            user: USER.to_owned(),
            password: PASSWORD.to_owned(),
            auth_in_clear: false,
            well_known: true,
            sync: true,
            page: None,
            rules: Rules::default(),
            calendars,
        }
    }

    /// The user name and password the server takes.
    #[must_use]
    pub fn login(self, user: &str, password: &str) -> Self {
        Self { user: user.to_owned(), password: password.to_owned(), ..self }
    }

    /// Asks for Basic auth and serves requests on a connection that is not encrypted, as a careless server does: a
    /// test that the client still sends no password in clear.
    #[must_use]
    pub fn auth_in_clear(self) -> Self {
        Self { auth_in_clear: true, ..self }
    }

    /// Answers the well-known path with `404` instead of a redirect.
    #[must_use]
    pub fn without_well_known(self) -> Self {
        Self { well_known: false, ..self }
    }

    /// A server without the sync report (RFC 6578): calendars have no `sync-token`, and the report is refused with
    /// `403` and `DAV:supported-report`.
    #[must_use]
    pub fn without_sync(self) -> Self {
        Self { sync: false, ..self }
    }

    /// Answers a sync report with at most `changes` changes (at least one), the rest left for the next report from
    /// the token it hands out, with `507` on the collection, as RFC 6578 section 3.6 lets a server do.
    #[must_use]
    pub fn page(self, changes: usize) -> Self {
        Self { page: Some(changes), ..self }
    }

    /// Plays `actions` at `when`. Each call is one rule, played once: two rules for the same request play at its
    /// first and its second occurrence.
    #[must_use]
    pub fn on(mut self, when: When, actions: impl IntoIterator<Item = Action>) -> Self {
        self.rules.add(when, actions.into_iter().collect());
        self
    }

    /// Answers with `reply` instead at `when`, once; what was asked does not happen.
    #[must_use]
    pub fn reply(self, when: When, reply: Reply) -> Self {
        self.on(when, [Action::Reply(reply)])
    }

    /// The server's calendars. The handle is shared: it sees and changes the calendars of every session of this
    /// fake, also those it serves on a socket after the handle was taken.
    #[must_use]
    pub fn calendars(&self) -> Calendars {
        self.calendars.clone()
    }

    /// Plays one session without a socket, as over TLS: reads the client's `input` and returns what the server sent.
    /// The session ends with the input or where the server closes the connection.
    #[must_use]
    pub fn answer(&self, input: &[u8]) -> Vec<u8> {
        let mut memory = Memory { input, output: Vec::new() };
        let mut session = Session::new(self.clone(), true);
        let _ended = session.run(&mut memory, &AtomicBool::new(false));
        memory.output
    }
}

impl Listen {
    /// Binds a free port on 127.0.0.1 and serves `fake` to the first client on a thread of its own.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] when the port cannot be bound or the thread cannot be started.
    pub fn start_dav(self, fake: FakeDav) -> Result<FakeServer, Error> {
        self.spawn(move |wire, stop, transcript| {
            let mut session = Session::new(fake, wire.is_tls()).recording(transcript);
            let ended = session.run(wire, stop);
            (session.transcript(), ended)
        })
    }

    /// Like [`Listen::start_dav`], but serves up to `connections` clients one after the other: the scripted points
    /// carry over, and a rule played on one connection is not played again on the next. Once one client has been
    /// served, a client that does not come within the accept timeout ends the server quietly.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] when the port cannot be bound or the thread cannot be started.
    pub fn start_dav_many(self, fake: FakeDav, connections: usize) -> Result<FakeServer, Error> {
        let mut state = Some(fake);
        self.spawn_many(connections, move |wire, stop, transcript| {
            let Some(fake) = state.take() else { return (transcript, Err(Error::Thread)) };
            let mut session = Session::new(fake, wire.is_tls()).recording(transcript);
            let ended = session.run(wire, stop);
            let transcript = session.transcript();
            state = Some(session.into_fake());
            (transcript, ended)
        })
    }
}

/// The client's input and the server's output of [`FakeDav::answer`].
struct Memory<'a> {
    input: &'a [u8],
    output: Vec<u8>,
}

impl Read for Memory<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.input.read(buf)
    }
}

impl Write for Memory<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.output.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
