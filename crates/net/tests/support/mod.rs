// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What the tests against the fake servers share: clocks, a connector that trusts only the test CA and dials the
//! fake's loopback address, and the account's server.

#![allow(dead_code)]

pub mod admin;
pub mod run;

use std::error::Error;
use std::net::SocketAddr;
use std::num::NonZeroU16;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use inboxcraft_model::{Host, Security, Server};
use inboxcraft_net::{Connector, Timeouts, TlsSettings, Trust};
use inboxcraft_testkit::{FAR_OFF, HOST, TestPki};
use inboxcraft_time::{Clock, FakeClock, Offset, SystemClock, Timestamp};

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

/// The wall clock's time `days` ago as IMAP writes an arrival time (`05-Nov-2026 10:00:00 +0000`).
pub fn days_ago(days: u64) -> Result<String> {
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    let then = SystemClock::new().now().checked_sub(Duration::from_secs(days * 86_400)).ok_or("clock out of range")?;
    let civil = then.to_civil(Offset::UTC).ok_or("clock out of range")?;
    let month = MONTHS.get(usize::from(civil.month()).saturating_sub(1)).ok_or("month")?;
    Ok(format!("{:02}-{month}-{} {:02}:{:02}:{:02} +0000", civil.day(), civil.year(), civil.hour(), civil.minute(), civil.second()))
}

/// The server [`HOST`] with `security`.
pub fn server(security: Security) -> Result<Server> {
    Ok(Server::new(Host::new(HOST)?, NonZeroU16::new(993).ok_or("port")?, security))
}

/// The computer's clock, which the test can set back as the owner or the network would; its monotonic time runs on.
#[derive(Debug, Default)]
pub struct SetBack {
    system: SystemClock,
    back: AtomicU64,
}

impl SetBack {
    /// Sets the wall clock `by` behind the computer's.
    pub fn set_back(&self, by: Duration) {
        self.back.store(by.as_secs(), Ordering::SeqCst);
    }
}

impl Clock for SetBack {
    fn now(&self) -> Timestamp {
        let back = Duration::from_secs(self.back.load(Ordering::SeqCst));
        self.system.now().checked_sub(back).unwrap_or(Timestamp::MIN)
    }

    fn monotonic(&self) -> Duration {
        self.system.monotonic()
    }
}
