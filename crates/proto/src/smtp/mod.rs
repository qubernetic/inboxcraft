// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! An SMTP submission client core without I/O (RFC 5321 and RFC 6409): the [`Client`] state machine and what it
//! reads.
//!
//! The commands of a submission: EHLO, STARTTLS (RFC 3207), AUTH PLAIN and AUTH LOGIN (RFC 4954, encrypted
//! connections only), MAIL FROM with SIZE (RFC 1870), `BODY=8BITMIME` (RFC 6152) and SMTPUTF8 (RFC 6531), RCPT TO,
//! DATA with the message dot-stuffed, RSET and QUIT; enhanced status codes (RFC 3463) from a server that offers
//! ENHANCEDSTATUSCODES (RFC 2034).
//!
//! ```
//! use inboxcraft_proto::ProtoError;
//! use inboxcraft_proto::smtp::{Answer, Client, Event, State, Transport, Verb};
//!
//! let mut client = Client::new(Transport::Plain);
//! client.receive(b"220 smtp.example.org ESMTP\r\n")?;
//! assert_eq!(client.next_event(), Some(Event::Greeted));
//! client.ehlo("client.example.org")?;
//! assert_eq!(client.take_output(), b"EHLO client.example.org\r\n");
//! client.receive(b"250-smtp.example.org\r\n250-STARTTLS\r\n250 AUTH PLAIN\r\n")?;
//! assert!(matches!(client.next_event(), Some(Event::Completed { command: Verb::Ehlo, outcome: Ok(Answer::Hello(_)) })));
//! // No password over a connection that is not encrypted, even when the server offers AUTH.
//! assert_eq!(client.auth_plain("ada@example.org", "secret"), Err(ProtoError::NotEncrypted));
//!
//! client.starttls()?;
//! assert_eq!(client.take_output(), b"STARTTLS\r\n");
//! client.receive(b"220 2.0.0 Ready to start TLS\r\n")?;
//! assert_eq!(client.state(), State::StartingTls);
//! # client.next_event();
//! // The caller runs the TLS handshake here, then:
//! client.tls_established()?;
//! assert_eq!(client.take_output(), b"EHLO client.example.org\r\n");
//! # Ok::<(), ProtoError>(())
//! ```

mod client;
mod data;
mod ehlo;
mod enhanced;
mod reply;

pub use client::{Answer, Client, Event, MailOptions, Refusal, State, Transport, Verb};
pub use data::{MAX_DOMAIN, MAX_PATH};
pub use ehlo::{Extensions, MAX_EXTENSIONS, MAX_KEYWORD_LEN};
pub use enhanced::EnhancedCode;
pub use reply::{MAX_REPLY_LINE, MAX_REPLY_LINES};
