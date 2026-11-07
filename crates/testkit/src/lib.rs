// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The in-process fake server frame for tests.
//!
//! Dev-only: a dev-dependency of any crate, a normal dependency of none (`cargo xtask layers` enforces it, and that
//! `testkit` never depends on `proto`). The [`imap`] module is the fake IMAP server and the [`smtp`] module the fake
//! SMTP submission server; the fake CalDAV server of a later slice follows.
//!
//! - [`Script`] is what the server does: lines to send, lines to expect, and the faults of [`Fault`] (drop, delay,
//!   garbage).
//! - [`Listen`] starts a [`FakeServer`] on a free loopback port, over plain TCP or implicit TLS. It serves one client
//!   and [`FakeServer::finish`] returns the [`Transcript`] of what the client sent; while it runs,
//!   [`FakeServer::wait_received`] waits for the lines the client has sent so far.
//! - [`TestPki`] generates a test CA the client trusts and a stranger CA it does not, in memory at test time; no key
//!   or certificate file exists anywhere. It makes the server configurations for the certificate faults of
//!   [`CertFault`]: a chain from the stranger CA, an expired certificate, a certificate for the wrong host name, and a
//!   valid chain met by a client whose [`Clock`] is far off.
//! - [`TestClient`] drives a server and reports each failure with its own [`ErrorKind`]. It stands in for the real
//!   client until `net` exists; later slices drive `net` against the same servers.
//!
//! The [`store`] module writes the fixture store: a small invented mailbox on disk, for tests that open a store the
//! way the apps do.
//!
//! The [`live`] module is the harness of the opt-in live tests the owner runs by hand on two throwaway test
//! accounts (`just it-live`); without their credentials, and always in the agent sandbox and in CI, they skip.
//!
//! Servers listen on 127.0.0.1 only and certificates name `example.org` hosts ([`HOST`], [`WRONG_HOST`]).
//!
//! ```
//! use std::time::Duration;
//! use inboxcraft_testkit::{CertFault, ErrorKind, Listen, Script, TestClient, TestPki};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let server = Listen::plain().start(Script::new().send("* OK ready").drop_connection())?;
//! let mut client = TestClient::plain(server.addr(), Duration::from_secs(5))?;
//! assert_eq!(client.read_line()?, "* OK ready");
//! assert_eq!(client.read_line().map_err(|err| err.kind()), Err(ErrorKind::Dropped));
//! server.finish()?;
//!
//! let pki = TestPki::new()?;
//! let fault = CertFault::Expired;
//! let server = Listen::tls(pki.server_config(Some(fault))?).start(Script::new().send("* OK ready"))?;
//! let err = TestClient::tls(server.addr(), &pki.client(fault.clock())?, Duration::from_secs(5)).err();
//! assert_eq!(err.map(|err| err.kind()), Some(ErrorKind::Expired));
//! assert!(server.finish().is_err());
//! # Ok(())
//! # }
//! ```

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod base64;
mod client;
mod deflate;
mod error;
pub mod imap;
mod line;
pub mod live;
mod pki;
mod script;
mod server;
pub mod smtp;
pub mod store;
mod wire;

pub use client::{ClientError, ErrorKind, TestClient};
pub use error::Error;
pub use line::MAX_LINE;
pub use pki::{CertFault, Clock, FAR_OFF, HOST, TestPki, TlsClient, WRONG_HOST};
pub use script::{Fault, Script, Step};
pub use server::{FakeServer, Listen, Transcript};
