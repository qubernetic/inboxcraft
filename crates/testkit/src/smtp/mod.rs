// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The fake SMTP submission server.
//!
//! [`FakeSmtp`] answers the submission commands of RFC 5321 and RFC 6409 with its own small command reader, never with
//! `proto`'s parser, so a parser bug there cannot hide behind the same bug here: EHLO and HELO, STARTTLS (RFC 3207),
//! AUTH PLAIN and AUTH LOGIN (RFC 4954), MAIL FROM with SIZE (RFC 1870), `BODY=8BITMIME` (RFC 6152) and SMTPUTF8
//! (RFC 6531), RCPT TO, DATA with dot-stuffing, RSET, NOOP and QUIT, with enhanced status codes (RFC 3463, RFC 2034).
//!
//! - It signs in only over an encrypted connection: before STARTTLS it lists no AUTH and answers AUTH with
//!   `530 5.7.0`, unless [`FakeSmtp::auth_in_clear`] makes it a server that takes a password in clear.
//! - [`FakeSmtp::refuse`] scripts a refusal, with its enhanced code, at a [`When`]: the greeting, a command, one
//!   recipient, or the message after its final dot. [`FakeSmtp::on`] scripts faults and lines as they are.
//! - Every message it accepts is captured in its [`Outbox`] as a [`Submission`]: the envelope and the message as the
//!   client meant it, dot-stuffing undone.
//! - [`FakeSmtp::bounce`] makes it accept a message for one recipient and then report its failure: an RFC 3464
//!   delivery report, dropped into the fake IMAP server's mailbox through a [`Postbox`](crate::imap::Postbox).
//!
//! [`crate::Listen::start_smtp`] serves it on a loopback port, over plain TCP (with STARTTLS when
//! [`FakeSmtp::starttls`] gave it a configuration) or implicit TLS, with the certificate faults of
//! [`crate::CertFault`]. [`FakeSmtp::answer`] plays it without a socket, as over implicit TLS.
//!
//! ```
//! use inboxcraft_testkit::smtp::{FakeSmtp, Reply, When};
//!
//! let fake = FakeSmtp::new().refuse(When::rcpt("nobody@example.org"), Reply::new(550, "5.1.1", "No such user"));
//! let outbox = fake.outbox();
//! let out = fake.answer(
//!     b"EHLO c.example.org\r\nAUTH PLAIN AGFkYUBleGFtcGxlLm9yZwBub3QtYS1yZWFsLXBhc3N3b3Jk\r\n\
//!       MAIL FROM:<ada@example.org>\r\nRCPT TO:<nobody@example.org>\r\nRCPT TO:<bea@example.org>\r\n\
//!       DATA\r\nSubject: Hi\r\n\r\n..a dot\r\n.\r\nQUIT\r\n",
//! );
//! let out = String::from_utf8_lossy(&out);
//! assert!(out.contains("235 2.7.0 Authentication successful\r\n"));
//! assert!(out.contains("550 5.1.1 No such user\r\n"));
//! assert!(out.ends_with("250 2.0.0 Queued as 1\r\n221 2.0.0 Bye\r\n"));
//! let sent = outbox.messages();
//! assert_eq!(sent.first().map(|sent| sent.recipients().to_vec()), Some(vec!["bea@example.org".to_owned()]));
//! assert_eq!(sent.first().map(|sent| sent.data().to_vec()), Some(b"Subject: Hi\r\n\r\n.a dot\r\n".to_vec()));
//! ```

mod bounce;
mod extension;
mod outbox;
mod reader;
mod rules;
mod session;

use std::io::{self, Read, Write};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use rustls::ServerConfig;

pub use bounce::Bounce;
pub use extension::Extension;
pub use outbox::{Outbox, Submission};
pub use rules::{Action, Reply, When};

use crate::Error;
use crate::server::{Ending, FakeServer, Listen};
use crate::wire::Wire;
use rules::Rules;
use session::{Effect, Session};

/// The user name [`FakeSmtp::new`] takes.
pub const USER: &str = "ada@example.org";
/// The password [`FakeSmtp::new`] takes; invented, like every fixture.
pub const PASSWORD: &str = "not-a-real-password";
/// The server's own name, in its greeting, its EHLO answer and its delivery reports.
pub const DOMAIN: &str = "smtp.example.org";
/// The message size [`FakeSmtp::new`] takes at most, in bytes: 10 MiB.
pub const MAX_SIZE: usize = 10 * 1024 * 1024;

/// A fake SMTP submission server: its extensions, its user, its size limit and the scripted points of its session.
#[derive(Debug, Clone)]
pub struct FakeSmtp {
    extensions: Vec<Extension>,
    user: String,
    password: String,
    max_size: usize,
    auth_in_clear: bool,
    starttls: Option<Arc<ServerConfig>>,
    rules: Rules,
    bounces: Vec<Bounce>,
    outbox: Outbox,
}

impl Default for FakeSmtp {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeSmtp {
    /// Every extension, the user [`USER`] with [`PASSWORD`], messages up to [`MAX_SIZE`], AUTH only over an
    /// encrypted connection, and no STARTTLS.
    #[must_use]
    pub fn new() -> Self {
        Self {
            extensions: Extension::ALL.to_vec(),
            user: USER.to_owned(),
            password: PASSWORD.to_owned(),
            max_size: MAX_SIZE,
            auth_in_clear: false,
            starttls: None,
            rules: Rules::default(),
            bounces: Vec::new(),
            outbox: Outbox::default(),
        }
    }

    /// Switches `extension` off: it is not listed, and a command or parameter that needs it is refused.
    #[must_use]
    pub fn without(mut self, extension: Extension) -> Self {
        self.extensions.retain(|&offered| offered != extension);
        self
    }

    /// Switches `extension` on again.
    #[must_use]
    pub fn with(mut self, extension: Extension) -> Self {
        if !self.offers(extension) {
            self.extensions.push(extension);
        }
        self
    }

    /// The user name and password the server takes.
    #[must_use]
    pub fn login(self, user: &str, password: &str) -> Self {
        Self { user: user.to_owned(), password: password.to_owned(), ..self }
    }

    /// The largest message the server takes, in bytes, as listed after `SIZE`.
    #[must_use]
    pub fn max_size(self, bytes: usize) -> Self {
        Self { max_size: bytes, ..self }
    }

    /// Lists AUTH and signs in on a connection that is not encrypted, as a careless server does: a test that the
    /// client still sends no password in clear.
    #[must_use]
    pub fn auth_in_clear(self) -> Self {
        Self { auth_in_clear: true, ..self }
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

    /// Answers with `reply` instead at `when`, once; what was refused does not happen (no sign-in, no sender, no
    /// recipient, no message). A refused greeting leaves the server answering every command but QUIT with `503`.
    #[must_use]
    pub fn refuse(self, when: When, reply: Reply) -> Self {
        self.on(when, [Action::Reply(reply)])
    }

    /// Accepts mail for the recipient of `bounce`, then reports its failure: see [`Bounce`].
    #[must_use]
    pub fn bounce(mut self, bounce: Bounce) -> Self {
        self.bounces.push(bounce);
        self
    }

    /// Where the messages the server accepts are captured. The handle is shared: it sees the messages of every
    /// session of this fake, also those it serves on a socket after the handle was taken.
    #[must_use]
    pub fn outbox(&self) -> Outbox {
        self.outbox.clone()
    }

    /// Plays one session without a socket, as over implicit TLS: reads the client's `input` and returns what the
    /// server sent. The session ends with the input, at QUIT, or where a socket would change (STARTTLS).
    #[must_use]
    pub fn answer(&self, input: &[u8]) -> Vec<u8> {
        let mut memory = Memory { input, output: Vec::new() };
        let mut session = Session::new(self.clone(), true);
        let _ended = session.run(&mut memory, &AtomicBool::new(false));
        memory.output
    }

    fn offers(&self, extension: Extension) -> bool {
        self.extensions.contains(&extension)
    }
}

impl Listen {
    /// Binds a free port on 127.0.0.1 and serves `fake` to the first client on a thread of its own.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] when the port cannot be bound or the thread cannot be started.
    pub fn start_smtp(self, fake: FakeSmtp) -> Result<FakeServer, Error> {
        self.spawn(move |wire, stop, transcript| {
            let mut session = Session::new(fake, wire.is_tls()).recording(transcript);
            let ended = serve(&mut session, wire, stop);
            (session.transcript(), ended)
        })
    }

    /// Like [`Listen::start_smtp`], but serves up to `connections` clients one after the other, as a client that
    /// sends again meets the server: the scripted points carry over, and a rule played on one connection is not
    /// played again on the next. Once one client has been served, a client that does not come within the accept
    /// timeout ends the server quietly; the first error is reported at the end.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] when the port cannot be bound or the thread cannot be started.
    pub fn start_smtp_many(self, fake: FakeSmtp, connections: usize) -> Result<FakeServer, Error> {
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

/// Runs `session` on `wire`, turning the connection into TLS when the session asks.
fn serve(session: &mut Session, wire: &mut Wire, stop: &AtomicBool) -> Result<Ending, Error> {
    loop {
        match session.run(wire, stop)? {
            Effect::End(ending) => return Ok(ending),
            Effect::StartTls(config) => {
                session.heard_in_clear(wire.lines_before_handshake()?);
                wire.accept_tls(&config)?;
            }
        }
    }
}

/// The client's input and the server's output of [`FakeSmtp::answer`].
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
