// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Detect's probe: whether a candidate server is one the app can use, without signing in.
//!
//! A server counts only when it greets, the connection is encrypted (implicit TLS, or STARTTLS before anything
//! else) with a certificate that verifies, and, over the encrypted connection, it offers what the app needs: IMAP4rev1
//! or IMAP4rev2 with a sign-in the client can do (AUTH=PLAIN, or LOGIN not disabled); for submission, AUTH PLAIN or
//! AUTH LOGIN after EHLO. No user name and no password is sent: the probe ends with LOGOUT or QUIT.

use inboxcraft_model::{ErrorKind, Server};

use crate::{Connector, ImapConnection, NetError, SmtpConnection};

/// What a candidate is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Service {
    /// Reading mail: IMAP.
    Imap,
    /// Sending mail: submission (SMTP).
    Submission,
}

impl Service {
    /// The word for it in Detect's list of what was asked: `imap`, `submission`.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Imap => "imap",
            Self::Submission => "submission",
        }
    }
}

/// What probes a candidate: a [`Connector`], or a test's stand-in.
pub trait Probe {
    /// Whether `server` serves `service` as the module says; it never signs in.
    ///
    /// # Errors
    ///
    /// The cause: the name, the connection, TLS or STARTTLS, the certificate, the protocol, or a server that offers no
    /// sign-in the client can do ([`ErrorKind::NoLoginMethod`]).
    fn probe(&self, server: &Server, service: Service) -> Result<(), NetError>;
}

impl Probe for Connector {
    fn probe(&self, server: &Server, service: Service) -> Result<(), NetError> {
        let fits = match service {
            Service::Imap => {
                let connection = ImapConnection::open(server, self)?;
                let fits = connection.is_encrypted()
                    && connection.capabilities_known().is_some_and(|caps| {
                        (caps.has("IMAP4rev1") || caps.has("IMAP4rev2")) && (caps.has("AUTH=PLAIN") || !caps.has("LOGINDISABLED"))
                    });
                connection.logout();
                fits
            }
            Service::Submission => {
                let connection = SmtpConnection::open(server, self)?;
                let fits = connection.is_encrypted() && connection.offers_sign_in();
                connection.quit();
                fits
            }
        };
        if fits { Ok(()) } else { Err(NetError::new(ErrorKind::NoLoginMethod, format!("{server} offers no sign-in the app can do"))) }
    }
}
