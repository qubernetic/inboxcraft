// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What the tests against the fake servers share: clocks, a connector that trusts only the test CA and dials the
//! fake's loopback address, and the account's server.

#![allow(dead_code)]

use std::error::Error;
use std::net::SocketAddr;
use std::num::NonZeroU16;
use std::sync::Arc;
use std::time::Duration;

use inboxcraft_model::{Host, Security, Server};
use inboxcraft_net::{Connector, Timeouts, TlsSettings, Trust};
use inboxcraft_testkit::{FAR_OFF, HOST, TestPki};
use inboxcraft_time::{Clock, FakeClock, SystemClock};

pub type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;

/// Short enough for a test, long enough for a slow machine.
pub const TIMEOUTS: Timeouts = Timeouts { connect: Duration::from_secs(5), io: Duration::from_secs(10) };

/// The computer's clock: the test certificates are made for it.
pub fn wall() -> Arc<dyn Clock> {
    Arc::new(SystemClock::new())
}

/// The computer's clock moved [`FAR_OFF`] ahead.
pub fn far_off() -> Result<Arc<dyn Clock>> {
    let now = SystemClock::new().now();
    let ahead = now.checked_add(Duration::from_secs(u64::try_from(FAR_OFF)?)).ok_or("clock out of range")?;
    Ok(Arc::new(FakeClock::new(ahead)))
}

/// A connector that trusts only the test CA, reads `clock` and dials `addr`.
pub fn connector(pki: &TestPki, clock: Arc<dyn Clock>, addr: SocketAddr) -> Connector {
    Connector::new(TlsSettings::new(Trust::Roots(vec![pki.ca()]), clock)).with_timeouts(TIMEOUTS).with_addresses(vec![addr])
}

/// The server [`HOST`] with `security`.
pub fn server(security: Security) -> Result<Server> {
    Ok(Server::new(Host::new(HOST)?, NonZeroU16::new(993).ok_or("port")?, security))
}
