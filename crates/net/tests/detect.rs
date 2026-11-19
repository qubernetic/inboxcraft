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
use inboxcraft_testkit::smtp::{Extension, FakeSmtp};
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

/// From the start.
const DRIP_AT_ONCE: &[u8] = &[];
/// From the client's first TLS record on (a handshake record starts with 0x16): after STARTTLS.
const DRIP_FROM_CLIENT_HELLO: &[u8] = &[0x16];
/// From the client's second flight on (a change of cipher spec, 0x14, or an encrypted record, 0x17): its Finished, so
/// the handshake is over and what drips is the encrypted greeting.
const DRIP_AFTER_HANDSHAKE: &[u8] = &[0x14, 0x17];

/// A relay in front of `server`: what the client sends goes on at once; what the server sends goes on one byte every
/// 100 ms, from the start or from the first piece the client sends that begins with a byte of `from`. It drips for at
/// most 20 seconds, then closes both sides. Its thread says whether it dripped at all.
fn drip_relay(server: SocketAddr, from: &'static [u8]) -> Result<(SocketAddr, thread::JoinHandle<bool>)> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let address = listener.local_addr()?;
    let relay = thread::spawn(move || {
        let Ok((client, _)) = listener.accept() else { return false };
        let Ok(upstream) = TcpStream::connect(server) else { return false };
        let dripping = Arc::new(std::sync::atomic::AtomicBool::new(from.is_empty()));
        let (Ok(mut from_client), Ok(mut to_server), Ok(closer_client), Ok(closer_server)) =
            (client.try_clone(), upstream.try_clone(), client.try_clone(), upstream.try_clone())
        else {
            return false;
        };
        let started = Arc::clone(&dripping);
        let up = thread::spawn(move || {
            let mut buffer = [0_u8; 4096];
            while let Ok(read) = io::Read::read(&mut from_client, &mut buffer) {
                let Some(bytes) = buffer.get(..read).filter(|bytes| !bytes.is_empty()) else { break };
                if bytes.first().is_some_and(|first| from.contains(first)) {
                    started.store(true, Ordering::SeqCst);
                }
                if to_server.write_all(bytes).is_err() {
                    break;
                }
            }
        });
        let (mut from_server, mut to_client) = (upstream, client);
        let mut byte = [0_u8; 1];
        let mut dripped = 0;
        while dripped < 200 {
            if dripping.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(100));
                dripped += 1;
            }
            match io::Read::read(&mut from_server, &mut byte) {
                Ok(1) if to_client.write_all(&byte).is_ok() => {}
                _ => break,
            }
        }
        let _ = closer_client.shutdown(std::net::Shutdown::Both);
        let _ = closer_server.shutdown(std::net::Shutdown::Both);
        let _ = up.join();
        dripped > 0
    });
    Ok((address, relay))
}

/// Probes `mail.example.org:<port>` through a [`drip_relay`] in front of `fake`: it must end at its 1.5-second
/// deadline, with the read timeout at 30 seconds.
fn probe_through_a_drip(pki: &TestPki, fake: &FakeServer, port: u16, security: Security, from: &'static [u8]) -> Result {
    let (address, relay) = drip_relay(fake.addr(), from)?;
    let connector = prober(pki, Names::default().at("mail.example.org", port, address), SLOW);
    let server = on("mail.example.org", port, security)?;
    let (result, took) = timed(|deadline| connector.probe(&server, Service::Imap, deadline), Duration::from_millis(1500));
    assert_eq!(result.map_err(|error| error.kind()), Err(ErrorKind::TimedOut), "{server}");
    assert!(took < Duration::from_secs(5), "{server}: {took:?}");
    drop(connector);
    assert!(relay.join().unwrap_or(false), "{server}: nothing dripped");
    Ok(())
}

/// A server whose TLS handshake comes one byte at a time, each well inside the read timeout, cannot hold the probe
/// past its deadline on 993: rustls reads in a loop of its own, so the deadline is in the socket under it.
#[test]
fn a_slow_drip_handshake_ends_at_the_probes_deadline() -> Result {
    let pki = TestPki::new()?;
    let fake = Listen::tls(pki.server_config(None)?).start_imap(FakeImap::new())?;
    probe_through_a_drip(&pki, &fake, 993, Security::Tls, DRIP_AT_ONCE)
}

/// A server that completes the TLS handshake at once and then sends its encrypted greeting one byte at a time cannot
/// hold the probe past its deadline either: rustls reads until a whole record has come, in a loop of its own.
#[test]
fn a_slow_drip_after_the_handshake_ends_at_the_probes_deadline() -> Result {
    let pki = TestPki::new()?;
    let fake = Listen::tls(pki.server_config(None)?).start_imap(FakeImap::new())?;
    probe_through_a_drip(&pki, &fake, 993, Security::Tls, DRIP_AFTER_HANDSHAKE)
}

/// The same on 143: after STARTTLS, the handshake and the encrypted greeting drip, and the probe still ends at its
/// deadline.
#[test]
fn a_slow_drip_after_starttls_ends_at_the_probes_deadline() -> Result {
    let pki = TestPki::new()?;
    let fake = Listen::plain().start_imap(FakeImap::new().starttls(pki.server_config(None)?))?;
    probe_through_a_drip(&pki, &fake, 143, Security::StartTls, DRIP_FROM_CLIENT_HELLO)
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

/// A server that greets over TLS but offers no sign-in the app can do is no candidate; nothing is sent but the
/// capability question and the goodbye.
#[test]
fn a_server_without_a_sign_in_is_no_candidate() -> Result {
    let pki = TestPki::new()?;
    let imap =
        Listen::tls(pki.server_config(None)?).start_imap(FakeImap::new().without(inboxcraft_testkit::imap::Capability::AuthPlain).no_sign_in())?;
    let smtp = Listen::tls(pki.server_config(None)?).start_smtp(FakeSmtp::new().without(Extension::AuthPlain).without(Extension::AuthLogin))?;
    let names = Names::default().at("mail.example.org", 993, imap.addr()).at("mail.example.org", 465, smtp.addr());
    let connector = prober(&pki, names, TIMEOUTS);
    let deadline = Deadline::new(wall(), Duration::from_secs(10));
    let kind = |result: std::result::Result<(), NetError>| result.map_err(|error| error.kind());
    assert_eq!(kind(connector.probe(&on("mail.example.org", 993, Security::Tls)?, Service::Imap, &deadline)), Err(ErrorKind::NoLoginMethod));
    assert_eq!(kind(connector.probe(&on("mail.example.org", 465, Security::Tls)?, Service::Submission, &deadline)), Err(ErrorKind::NoLoginMethod));
    no_credentials(imap)?;
    no_credentials(smtp)
}
