// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Detect against the fake servers, with a fake resolver and a fake web: no real name is looked up and no real
//! connection is opened. The probe counts a server only when it greets and, encrypted, offers a sign-in; it sends no
//! credentials.

mod support;

use std::collections::HashMap;
use std::io;
use std::net::{SocketAddr, TcpListener};
use std::sync::Arc;

use inboxcraft_model::{EmailAddress, ErrorKind, Host};
use inboxcraft_net::detect::{DetectConfig, Lookup, Outside, Proposal, Source, detect};
use inboxcraft_net::dns::Srv;
use inboxcraft_net::web::{Web, WebAnswer};
use inboxcraft_net::{Connector, NetError, Resolve, TlsSettings, Trust};
use inboxcraft_testkit::imap::FakeImap;
use inboxcraft_testkit::smtp::FakeSmtp;
use inboxcraft_testkit::{FakeServer, Listen, TestPki};
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
    Ok(detect(&address, &DetectConfig::default(), &Outside { dns: &Dns(mx), web: &NoPages, probe: &connector, clock: clock.as_ref() }))
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
