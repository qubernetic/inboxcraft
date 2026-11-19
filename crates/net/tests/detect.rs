// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Detect against the fake servers, with a fake resolver and a fake web: no real name is looked up and no real
//! connection is opened. The probe counts a server only when it greets and, encrypted, offers a sign-in; it sends no
//! credentials.

mod support;

use std::collections::HashMap;
use std::io::{self, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::num::NonZeroU16;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

use inboxcraft_model::{EmailAddress, ErrorKind, Host, Security, Server};
use inboxcraft_net::detect::{DetectConfig, Lookup, Outside, Proposal, Source, detect};
use inboxcraft_net::dns::Srv;
use inboxcraft_net::probe::{PROBE_ADDRESSES, Probe, Service};
use inboxcraft_net::web::{Web, WebAnswer};
use inboxcraft_net::{Connector, Deadline, Dialer, NetError, Resolve, Timeouts, TlsSettings, Trust};
use inboxcraft_testkit::imap::FakeImap;
use inboxcraft_testkit::smtp::FakeSmtp;
use inboxcraft_testkit::{FakeServer, Listen, TestPki};
use inboxcraft_time::{Clock, SystemClock};
use support::{Result, TIMEOUTS, wall};

/// The provider's host in the owner's case: a made-up name here, which only the fake resolver knows.
const PROVIDER_HOST: &str = "mail.privateemail.com";

/// A fake resolver: names and ports to local addresses; any other name has no address.
#[derive(Debug, Default)]
struct Names(HashMap<(String, u16), SocketAddr>);

impl Names {
    fn at(mut self, host: &str, port: u16, address: SocketAddr) -> Self {
        self.0.insert((host.to_owned(), port), address);
        self
    }
}

impl Resolve for Names {
    fn resolve(&self, host: &Host, port: u16) -> io::Result<Vec<SocketAddr>> {
        self.0.get(&(host.as_str().to_owned(), port)).map(|address| vec![*address]).ok_or_else(|| io::ErrorKind::NotFound.into())
    }
}

/// A fake DNS: the MX hosts, no SRV records.
struct Dns(Vec<&'static str>);

impl Lookup for Dns {
    fn mx(&self, _domain: &Host) -> std::result::Result<Vec<Host>, NetError> {
        self.0.iter().map(|name| Host::new(name).map_err(|_| NetError::new(ErrorKind::Protocol, "fake"))).collect()
    }

    fn srv(&self, _name: &str) -> std::result::Result<Vec<Srv>, NetError> {
        Ok(Vec::new())
    }
}

/// A fake web where every page is missing.
#[derive(Debug)]
struct NoPages;

impl Web for NoPages {
    fn get(&self, _url: &str) -> std::result::Result<WebAnswer, NetError> {
        Ok(WebAnswer { status: 404, location: None, body: Vec::new() })
    }
}

/// An address on 127.0.0.1 where nothing listens: a connection to it is refused.
fn nobody() -> Result<SocketAddr> {
    Ok(TcpListener::bind("127.0.0.1:0")?.local_addr()?)
}

fn run(pki: &TestPki, names: Names, mx: Vec<&'static str>) -> Result<Proposal> {
    let clock = wall();
    let connector =
        Connector::new(TlsSettings::new(Trust::Roots(vec![pki.ca()]), Arc::clone(&clock))).with_timeouts(TIMEOUTS).with_resolver(Arc::new(names));
    let address = EmailAddress::new("ada@example.org")?;
    Ok(detect(&address, &DetectConfig::default(), &Outside { dns: &Dns(mx), web: &NoPages, probe: &connector, clock, stop: None }))
}

fn no_credentials(fake: FakeServer) -> Result {
    let transcript = fake.finish()?;
    for line in transcript.received() {
        let upper = line.to_ascii_uppercase();
        assert!(!upper.contains("LOGIN") && !upper.contains("AUTH") && !line.contains("ada@example.org"), "{line}");
    }
    Ok(())
}

/// The owner's case: the domain answers nothing on the mail ports, its MX points into a provider, and `mail.` of the
/// provider answers with implicit TLS.
#[test]
fn a_domain_hosted_by_a_provider_is_found_with_implicit_tls() -> Result {
    let pki = TestPki::new()?;
    let imap = Listen::tls(pki.server_config_for(PROVIDER_HOST)?).start_imap(FakeImap::new())?;
    let smtp = Listen::tls(pki.server_config_for(PROVIDER_HOST)?).start_smtp(FakeSmtp::new())?;
    let refused = nobody()?;
    let mut names = Names::default().at(PROVIDER_HOST, 993, imap.addr()).at(PROVIDER_HOST, 465, smtp.addr());
    for host in ["mail.example.org", "imap.example.org", "smtp.example.org", "example.org"] {
        for port in [993, 143, 465, 587] {
            names = names.at(host, port, refused);
        }
    }
    let proposal = run(&pki, names, vec!["mx1.privateemail.com", "mx2.privateemail.com"])?;
    let found: Vec<String> =
        proposal.imap.iter().chain(&proposal.submission).map(|candidate| format!("{} {:?}", candidate.server, candidate.source)).collect();
    assert_eq!(found, ["mail.privateemail.com:993 (TLS) Provider", "mail.privateemail.com:465 (TLS) Provider"]);
    assert_eq!(proposal.imap[0].username, "ada@example.org");
    // Nothing detected gets a password before the owner has seen it.
    assert!(!proposal.imap[0].target().is_reviewed());
    no_credentials(imap)?;
    no_credentials(smtp)
}

/// A server that offers no TLS is not a candidate: not on the implicit TLS port, not on the STARTTLS port without
/// STARTTLS. Nothing else answers, so nothing is found.
#[test]
fn a_server_without_tls_is_no_candidate() -> Result {
    let pki = TestPki::new()?;
    let imap_plain = Listen::plain().start_imap(FakeImap::new())?;
    let imap_no_starttls = Listen::plain().start_imap(FakeImap::new())?;
    let smtp_plain = Listen::plain().start_smtp(FakeSmtp::new())?;
    let smtp_no_starttls = Listen::plain().start_smtp(FakeSmtp::new())?;
    let names = Names::default()
        .at("mail.example.org", 993, imap_plain.addr())
        .at("mail.example.org", 143, imap_no_starttls.addr())
        .at("mail.example.org", 465, smtp_plain.addr())
        .at("mail.example.org", 587, smtp_no_starttls.addr());
    let proposal = run(&pki, names, Vec::new())?;
    assert!(proposal.imap.is_empty() && proposal.submission.is_empty(), "{proposal:?}");
    let failed = |what: &str| proposal.failed.iter().find(|(asked, _)| asked == what).map(|(_, kind)| *kind);
    assert_eq!(failed("imap mail.example.org:143 (STARTTLS)"), Some(ErrorKind::StartTlsMissing));
    assert_eq!(failed("submission mail.example.org:587 (STARTTLS)"), Some(ErrorKind::StartTlsMissing));
    assert!(failed("imap mail.example.org:993 (TLS)").is_some());
    assert!(failed("submission mail.example.org:465 (TLS)").is_some());
    // The other names have no address here.
    assert_eq!(failed("imap imap.example.org:993 (TLS)"), Some(ErrorKind::HostNotFound));
    assert!(proposal.imap.iter().all(|candidate| candidate.source != Source::Guess));
    drop((imap_plain, smtp_plain));
    no_credentials(imap_no_starttls)?;
    no_credentials(smtp_no_starttls)
}

/// A connector for the probe tests: the test CA, the fake resolver `names`, and `timeouts`.
fn prober(pki: &TestPki, names: Names, timeouts: Timeouts) -> Connector {
    Connector::new(TlsSettings::new(Trust::Roots(vec![pki.ca()]), wall())).with_timeouts(timeouts).with_resolver(Arc::new(names))
}

fn on(host: &str, port: u16, security: Security) -> Result<Server> {
    Ok(Server::new(Host::new(host)?, NonZeroU16::new(port).ok_or("port")?, security))
}

/// Runs `probe` with a deadline `within` from now; its result and how long it took on the computer's clock.
fn timed(probe: impl FnOnce(&Deadline) -> std::result::Result<(), NetError>, within: Duration) -> (std::result::Result<(), NetError>, Duration) {
    let clock = SystemClock::new();
    let deadline = Deadline::new(wall(), within);
    let start = clock.monotonic();
    let result = probe(&deadline);
    (result, clock.monotonic().saturating_sub(start))
}

/// Long enough per read that only the probe's deadline can end these tests in time.
const SLOW: Timeouts = Timeouts { connect: Duration::from_secs(30), io: Duration::from_secs(30) };

/// A server that accepts and never greets: the probe ends at its deadline, not at the 30-second read timeout, both
/// on the implicit TLS port (no handshake answer) and on the STARTTLS port (no greeting).
#[test]
fn a_server_that_never_greets_ends_at_the_probes_deadline() -> Result {
    let pki = TestPki::new()?;
    let silent = TcpListener::bind("127.0.0.1:0")?;
    let names = Names::default().at("mail.example.org", 993, silent.local_addr()?).at("mail.example.org", 143, silent.local_addr()?);
    let connector = prober(&pki, names, SLOW);
    for server in [on("mail.example.org", 993, Security::Tls)?, on("mail.example.org", 143, Security::StartTls)?] {
        let (result, took) = timed(|deadline| connector.probe(&server, Service::Imap, deadline), Duration::from_millis(500));
        assert_eq!(result.map_err(|error| error.kind()), Err(ErrorKind::TimedOut), "{server}");
        assert!(took < Duration::from_secs(5), "{server}: {took:?}");
    }
    drop(silent);
    Ok(())
}

/// A server that sends its greeting one byte at a time, each well inside the read timeout, cannot hold the probe past
/// its deadline.
#[test]
fn a_slow_drip_greeting_ends_at_the_probes_deadline() -> Result {
    let pki = TestPki::new()?;
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let address = listener.local_addr()?;
    let drip = thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else { return };
        for byte in b"* OK ".iter().chain(std::iter::repeat_n(&b'x', 200)) {
            if stream.write_all(&[*byte]).is_err() {
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }
    });
    let connector = prober(&pki, Names::default().at("mail.example.org", 143, address), SLOW);
    let server = on("mail.example.org", 143, Security::StartTls)?;
    let (result, took) = timed(|deadline| connector.probe(&server, Service::Imap, deadline), Duration::from_millis(800));
    assert_eq!(result.map_err(|error| error.kind()), Err(ErrorKind::TimedOut));
    assert!(took < Duration::from_secs(5), "{took:?}");
    drop(connector);
    let _ = drip.join();
    Ok(())
}

/// A resolver that gives one name 16 addresses.
#[derive(Debug)]
struct Many;

impl Resolve for Many {
    fn resolve(&self, _host: &Host, port: u16) -> io::Result<Vec<SocketAddr>> {
        Ok((1..=16).map(|n| SocketAddr::from(([192, 0, 2, n], port))).collect())
    }
}

/// A network where every connection hangs until its timeout; it counts the attempts.
#[derive(Debug, Default)]
struct Hang(AtomicUsize);

impl Dialer for Hang {
    fn dial(&self, _address: SocketAddr, timeout: Duration) -> io::Result<TcpStream> {
        self.0.fetch_add(1, Ordering::SeqCst);
        thread::sleep(timeout);
        Err(io::ErrorKind::TimedOut.into())
    }
}

/// A name with 16 addresses that all hang: the probe tries at most [`PROBE_ADDRESSES`] of them, and stops at its
/// deadline however many are left.
#[test]
fn a_name_with_sixteen_hanging_addresses_is_capped() -> Result {
    let pki = TestPki::new()?;
    let server = on("mail.example.org", 993, Security::Tls)?;
    let quick = Timeouts { connect: Duration::from_millis(50), io: Duration::from_secs(1) };
    let hang = Arc::new(Hang::default());
    let connector = Connector::new(TlsSettings::new(Trust::Roots(vec![pki.ca()]), wall()))
        .with_timeouts(quick)
        .with_resolver(Arc::new(Many))
        .with_dialer(Arc::clone(&hang) as Arc<dyn Dialer>);
    let (result, _) = timed(|deadline| connector.probe(&server, Service::Imap, deadline), Duration::from_secs(10));
    assert_eq!(result.map_err(|error| error.kind()), Err(ErrorKind::TimedOut));
    assert_eq!(hang.0.load(Ordering::SeqCst), PROBE_ADDRESSES);

    // With a connect timeout longer than the deadline, the deadline ends the first attempt and no other starts.
    let hang = Arc::new(Hang::default());
    let connector = connector.with_timeouts(SLOW).with_dialer(Arc::clone(&hang) as Arc<dyn Dialer>);
    let (result, took) = timed(|deadline| connector.probe(&server, Service::Imap, deadline), Duration::from_millis(500));
    assert_eq!(result.map_err(|error| error.kind()), Err(ErrorKind::TimedOut));
    assert_eq!(hang.0.load(Ordering::SeqCst), 1);
    assert!(took < Duration::from_secs(3), "{took:?}");
    Ok(())
}
