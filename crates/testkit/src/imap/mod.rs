// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The fake IMAP server.
//!
//! [`FakeImap`] answers IMAP4rev1 (RFC 3501) commands with its own small command reader, never with `proto`'s
//! parser, so a parser bug there cannot hide behind the same bug here. A test switches capabilities off with
//! [`FakeImap::without`], and scripts faults and server-side lines with [`FakeImap::on`].
//!
//! Besides the read path it answers CREATE, DELETE and APPEND, so a test can change the mailboxes the way another
//! client would. [`crate::Listen::start_imap_many`] serves several connections one after the other, for a client
//! that reconnects.
//!
//! [`crate::Listen::start_imap`] serves it on a loopback port, over plain TCP (with STARTTLS when
//! [`FakeImap::starttls`] gave it a configuration) or implicit TLS, with the certificate faults of
//! [`crate::CertFault`]. [`FakeImap::answer`] plays it without a socket, as over implicit TLS.
//!
//! ```
//! use inboxcraft_testkit::imap::{Capability, FakeImap};
//!
//! let out = FakeImap::new().without(Capability::Move).answer(b"a1 LOGOUT\r\n");
//! let out = String::from_utf8_lossy(&out);
//! assert!(out.starts_with("* OK [CAPABILITY IMAP4rev1 AUTH=PLAIN"));
//! assert!(!out.contains("MOVE"));
//! assert!(out.ends_with("* BYE Logging out\r\na1 OK LOGOUT completed\r\n"));
//! ```

mod capability;
mod fetch;
mod fixture;
mod happen;
mod mailbox;
mod manage;
mod notify;
mod postbox;
mod reader;
mod search;
mod session;
mod set;
mod store;
mod sync;
mod update;
mod write;

use std::io::{self, Read, Write};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rustls::ServerConfig;

pub use capability::Capability;
pub use fixture::{Mailbox, Message};
pub use happen::{Action, When};
pub use postbox::Postbox;

use crate::Error;
use crate::server::{Ending, FakeServer, Listen};
use crate::wire::Wire;
use happen::Rules;
use session::{Effect, Session};

/// The user name [`FakeImap::new`] takes.
pub const USER: &str = "ada@example.org";
/// The password [`FakeImap::new`] takes; invented, like every fixture.
pub const PASSWORD: &str = "not-a-real-password";

/// A fake IMAP server: its capabilities, its user and the scripted points of its session.
#[derive(Debug, Clone)]
pub struct FakeImap {
    caps: Vec<Capability>,
    user: String,
    password: String,
    preauth: bool,
    starttls: Option<Arc<ServerConfig>>,
    rules: Rules,
    mailboxes: Vec<Mailbox>,
    /// The UIDVALIDITY of the mailbox CREATE made last.
    last_created: u32,
    postbox: Postbox,
}

impl Default for FakeImap {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeImap {
    /// Every capability, the user [`USER`] with [`PASSWORD`], and no STARTTLS.
    #[must_use]
    pub fn new() -> Self {
        Self {
            caps: Capability::ALL.to_vec(),
            user: USER.to_owned(),
            password: PASSWORD.to_owned(),
            preauth: false,
            starttls: None,
            rules: Rules::default(),
            mailboxes: vec![Mailbox::new("INBOX")],
            last_created: 0,
            postbox: Postbox::default(),
        }
    }

    /// [`FakeImap::new`] with the invented sample mailboxes: INBOX with three messages; Sent, Drafts, Archive, Junk
    /// and Trash with their SPECIAL-USE attributes; `Lists` with `Lists/Rust` under it; and `Elküldött`, a Hungarian
    /// name without an attribute.
    #[must_use]
    pub fn sample() -> Self {
        Self { mailboxes: fixture::sample(), ..Self::new() }
    }

    /// Adds `mailbox`, or puts it in place of the mailbox with its name.
    #[must_use]
    pub fn mailbox(mut self, mailbox: Mailbox) -> Self {
        match self.mailboxes.iter_mut().find(|own| own.name == mailbox.name) {
            Some(own) => *own = mailbox,
            None => self.mailboxes.push(mailbox),
        }
        self
    }

    /// Switches `cap` off.
    #[must_use]
    pub fn without(mut self, cap: Capability) -> Self {
        self.caps.retain(|&offered| offered != cap);
        self
    }

    /// Switches `cap` on again.
    #[must_use]
    pub fn with(mut self, cap: Capability) -> Self {
        if !self.offers(cap) {
            self.caps.push(cap);
        }
        self
    }

    /// The user name and password the server takes.
    #[must_use]
    pub fn login(self, user: &str, password: &str) -> Self {
        Self { user: user.to_owned(), password: password.to_owned(), ..self }
    }

    /// Greets with `PREAUTH`: the session starts signed in.
    #[must_use]
    pub fn preauth(self) -> Self {
        Self { preauth: true, ..self }
    }

    /// Offers STARTTLS on a plain connection, with `config`, usually from [`crate::TestPki::server_config`].
    #[must_use]
    pub fn starttls(self, config: Arc<ServerConfig>) -> Self {
        Self { starttls: Some(config), ..self }
    }

    /// Plays `actions` at `when`. Each call is one rule, played once: two rules for the same point play at its first
    /// and its second occurrence.
    #[must_use]
    pub fn on(mut self, when: When, actions: impl IntoIterator<Item = Action>) -> Self {
        self.rules.add(when, actions.into_iter().collect());
        self
    }

    /// Where mail from outside a session arrives, such as the reports of [`crate::smtp::Bounce`]. The handle is
    /// shared: what it takes reaches every session of this fake, also those on a socket started after it was taken.
    #[must_use]
    pub fn postbox(&self) -> Postbox {
        self.postbox.clone()
    }

    /// Plays one session without a socket, as over implicit TLS: reads the client's `input` and returns what the
    /// server sent. The session ends with the input, at `LOGOUT`, or where a socket would change (STARTTLS,
    /// COMPRESS).
    #[must_use]
    pub fn answer(&self, input: &[u8]) -> Vec<u8> {
        let mut memory = Memory { input, output: Vec::new() };
        let mut session = Session::new(self.clone(), true);
        let _ended = session.run(&mut memory, &AtomicBool::new(false));
        memory.output
    }

    fn offers(&self, cap: Capability) -> bool {
        self.caps.contains(&cap)
    }
}

impl Listen {
    /// Binds a free port on 127.0.0.1 and serves `fake` to the first client on a thread of its own.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] when the port cannot be bound or the thread cannot be started.
    pub fn start_imap(self, fake: FakeImap) -> Result<FakeServer, Error> {
        self.spawn(move |wire, stop, transcript| {
            let mut session = Session::new(fake, wire.is_tls()).recording(transcript);
            let ended = serve(&mut session, wire, stop);
            (session.transcript(), ended)
        })
    }
}

impl Listen {
    /// Like [`Listen::start_imap`], but serves up to `connections` clients one after the other, as a client that
    /// reconnects meets the server: the mailboxes and the scripted points carry over, and a rule played on one
    /// connection is not played again on the next. Once one client has been served, a client that does not come within
    /// the accept timeout ends the server quietly. A failed connection does not stop it; the first error is reported
    /// at the end.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] when the port cannot be bound or the thread cannot be started.
    pub fn start_imap_many(self, fake: FakeImap, connections: usize) -> Result<FakeServer, Error> {
        let mut state = Some(fake);
        self.spawn_many(connections, move |wire, stop, transcript| {
            let Some(fake) = state.take() else { return (transcript, Err(Error::Thread)) };
            let mut session = Session::new(fake, wire.is_tls()).recording(transcript);
            let ended = serve(&mut session, wire, stop);
            let transcript = session.transcript();
            state = Some(session.into_fake());
            (transcript, ended)
        })
    }
}

/// Runs `session` on `wire`, turning the connection into TLS or compressing it when the session asks.
fn serve(session: &mut Session, wire: &mut Wire, stop: &AtomicBool) -> Result<Ending, Error> {
    loop {
        match session.run(wire, stop)? {
            Effect::End(ending) => return Ok(ending),
            Effect::StartTls(config) => {
                session.heard_in_clear(wire.lines_before_handshake()?);
                wire.accept_tls(&config)?;
            }
            Effect::Compress => wire.compress(),
        }
    }
}

/// The client's input and the server's output of [`FakeImap::answer`].
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
