// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The in-process fake server frame for tests.
//!
//! Dev-only: a dev-dependency of any crate, a normal dependency of none (`cargo xtask layers` enforces it). The fake
//! IMAP, SMTP and CalDAV servers of later slices are scripts played by this frame.
//!
//! - [`Script`] is what the server does: lines to send, lines to expect, and the faults of [`Fault`] (drop, delay,
//!   garbage).
//! - [`Listen`] starts a [`FakeServer`] on a free loopback port, over plain TCP or implicit TLS. It serves one client
//!   and [`FakeServer::finish`] returns the [`Transcript`] of what the client sent.
//! - [`TestClient`] drives a server and reports each failure with its own [`ErrorKind`].
//!
//! ```
//! use std::time::Duration;
//! use inboxcraft_testkit::{ErrorKind, Listen, Script, TestClient};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let server = Listen::plain().start(Script::new().send("* OK ready").drop_connection())?;
//! let mut client = TestClient::plain(server.addr(), Duration::from_secs(5))?;
//! assert_eq!(client.read_line()?, "* OK ready");
//! assert_eq!(client.read_line().map_err(|err| err.kind()), Err(ErrorKind::Dropped));
//! server.finish()?;
//! # Ok(())
//! # }
//! ```

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod client;
mod error;
mod line;
mod script;
mod server;

pub use client::{ClientError, ErrorKind, TestClient};
pub use error::Error;
pub use script::{Fault, Script, Step};
pub use server::{FakeServer, Listen, Transcript};
