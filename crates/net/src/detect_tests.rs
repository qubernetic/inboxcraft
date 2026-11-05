// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::HashMap;
use std::num::NonZeroU16;
use std::sync::Mutex;

use inboxcraft_model::{EmailAddress, ErrorKind, Host, Security};

use super::{DetectConfig, ISPDB, Lookup, Proposal, Source, base_domain, detect, https_host, within};
use crate::NetError;
use crate::dns::Srv;
use crate::web::{Web, WebAnswer};

/// A stand-in web: canned answers by URL, 404 for the rest, and every URL asked for.
#[derive(Debug, Default)]
struct FakeWeb {
    answers: HashMap<String, Result<WebAnswer, ErrorKind>>,
    asked: Mutex<Vec<String>>,
}

impl FakeWeb {
    fn page(mut self, url: &str, body: &str) -> Self {
        self.answers.insert(url.to_owned(), Ok(WebAnswer { status: 200, location: None, body: body.as_bytes().to_vec() }));
        self
    }

    fn redirect(mut self, url: &str, to: &str) -> Self {
        self.answers.insert(url.to_owned(), Ok(WebAnswer { status: 301, location: Some(to.to_owned()), body: Vec::new() }));
        self
    }

    fn failing(mut self, url: &str, kind: ErrorKind) -> Self {
        self.answers.insert(url.to_owned(), Err(kind));
        self
    }

    fn asked(&self) -> Vec<String> {
        self.asked.lock().unwrap().clone()
    }
}

impl Web for FakeWeb {
    fn get(&self, url: &str) -> Result<WebAnswer, NetError> {
        self.asked.lock().unwrap().push(url.to_owned());
        match self.answers.get(url) {
            Some(Ok(answer)) => Ok(answer.clone()),
            Some(Err(kind)) => Err(NetError::new(*kind, "fake")),
            None => Ok(WebAnswer { status: 404, location: None, body: Vec::new() }),
        }
    }
}

/// A stand-in DNS.
#[derive(Default)]
struct FakeDns {
    mx: Vec<&'static str>,
    mx_fails: bool,
    srv: HashMap<String, Vec<(&'static str, u16)>>,
}

impl Lookup for FakeDns {
    fn mx(&self, _domain: &Host) -> Result<Vec<Host>, NetError> {
        if self.mx_fails {
            return Err(NetError::new(ErrorKind::TimedOut, "fake"));
        }
        Ok(self.mx.iter().map(|host| Host::new(host).unwrap()).collect())
    }

    fn srv(&self, name: &str) -> Result<Vec<Srv>, NetError> {
        if name.starts_with("_submission._tcp") && self.mx_fails {
            return Err(NetError::new(ErrorKind::TimedOut, "fake"));
        }
        Ok(self
            .srv
            .get(name)
            .map(|records| {
                records
                    .iter()
                    .map(|(host, port)| Srv { host: Host::new(host).unwrap(), port: NonZeroU16::new(*port).unwrap(), priority: 0, weight: 0 })
                    .collect()
            })
            .unwrap_or_default())
    }
}

fn address() -> EmailAddress {
    EmailAddress::new("ada@example.org").unwrap()
}

fn config(host: &str, port: u16, socket: &str) -> String {
    format!(
        "<clientConfig><emailProvider><incomingServer type=\"imap\"><hostname>{host}</hostname><port>{port}</port><socketType>{socket}</socketType></incomingServer>\
         <outgoingServer type=\"smtp\"><hostname>smtp.{host}</hostname><port>465</port><socketType>SSL</socketType></outgoingServer></emailProvider></clientConfig>"
    )
}

fn imap(proposal: &Proposal) -> Vec<String> {
    proposal.imap.iter().map(|candidate| format!("{} {:?}", candidate.server, candidate.source)).collect()
}

const AUTOCONFIG: &str = "https://autoconfig.example.org/mail/config-v1.1.xml";
const WELL_KNOWN: &str = "https://example.org/.well-known/autoconfig/mail/config-v1.1.xml";
const PLAIN: &str = "http://autoconfig.example.org/mail/config-v1.1.xml";

#[test]
fn the_providers_autoconfig_comes_first_and_nothing_else_is_asked() {
    let web = FakeWeb::default().page(AUTOCONFIG, &config("imap.example.org", 993, "SSL"));
    let dns = FakeDns { mx: vec!["mx.example.org"], ..FakeDns::default() };
    let proposal = detect(&address(), &DetectConfig::default(), &dns, &web);
    assert_eq!(imap(&proposal), ["imap.example.org:993 (TLS) Autoconfig"]);
    assert_eq!(proposal.submission[0].server.to_string(), "smtp.imap.example.org:465 (TLS)");
    assert_eq!(proposal.mx, [Host::new("mx.example.org").unwrap()]);
    assert_eq!(web.asked(), [AUTOCONFIG]);
    assert_eq!(proposal.asked, ["dns MX example.org", AUTOCONFIG]);
    // A detected server gets no password until the owner has seen it.
    let target = proposal.imap[0].target();
    assert!(!target.is_reviewed());
    assert_eq!(target.username(), "ada@example.org");
}

#[test]
fn the_well_known_place_is_tried_next() {
    let web = FakeWeb::default().failing(AUTOCONFIG, ErrorKind::HostNotFound).page(WELL_KNOWN, &config("imap.example.org", 143, "STARTTLS"));
    let proposal = detect(&address(), &DetectConfig::default(), &FakeDns::default(), &web);
    assert_eq!(imap(&proposal), ["imap.example.org:143 (STARTTLS) Autoconfig"]);
    assert_eq!(proposal.failed, [(AUTOCONFIG.to_owned(), ErrorKind::HostNotFound)]);
}

#[test]
fn with_the_ispdb_off_it_gets_no_request() {
    let ispdb = format!("{ISPDB}example.org");
    let web = FakeWeb::default().page(&ispdb, &config("imap.example.org", 993, "SSL"));
    let dns = FakeDns { mx: vec!["mx1.provider.example.net"], ..FakeDns::default() };
    let proposal = detect(&address(), &DetectConfig::default(), &dns, &web);
    assert!(!DetectConfig::default().ispdb);
    assert!(web.asked().iter().all(|url| !url.starts_with(ISPDB)), "{:?}", web.asked());
    assert!(proposal.asked.iter().all(|url| !url.starts_with(ISPDB)));
    assert!(proposal.imap.iter().all(|candidate| candidate.source != Source::Ispdb));
}

#[test]
fn with_the_ispdb_on_it_is_asked_for_the_domain_then_the_mx_domain() {
    let config_on = DetectConfig { ispdb: true, ..DetectConfig::default() };
    let web = FakeWeb::default().page(&format!("{ISPDB}example.net"), &config("imap.example.net", 993, "SSL"));
    let dns = FakeDns { mx: vec!["mx1.mail.example.net"], ..FakeDns::default() };
    let proposal = detect(&address(), &config_on, &dns, &web);
    assert_eq!(imap(&proposal), ["imap.example.net:993 (TLS) Ispdb"]);
    assert_eq!(
        web.asked(),
        [AUTOCONFIG.to_owned(), WELL_KNOWN.to_owned(), PLAIN.to_owned(), format!("{ISPDB}example.org"), format!("{ISPDB}example.net")]
    );

    // Found for the domain: the MX domain is not asked.
    let web = FakeWeb::default().page(&format!("{ISPDB}example.org"), &config("imap.example.org", 993, "SSL"));
    let proposal = detect(&address(), &config_on, &dns, &web);
    assert_eq!(imap(&proposal), ["imap.example.org:993 (TLS) Ispdb"]);
    assert!(!web.asked().contains(&format!("{ISPDB}example.net")));
}

#[test]
fn no_request_carries_the_local_part_or_the_address() {
    let address = EmailAddress::new("zq-distinctive-local@example.org").unwrap();
    let config_on = DetectConfig { ispdb: true, ..DetectConfig::default() };
    let target = "https://autoconfig.provider.example.net/mail/config-v1.1.xml";
    let web = FakeWeb::default().failing(AUTOCONFIG, ErrorKind::TimedOut).redirect(PLAIN, target);
    let mut dns = FakeDns { mx: vec!["mx.mail.provider.example.net"], ..FakeDns::default() };
    dns.srv.insert("_imaps._tcp.example.org".to_owned(), vec![("imap.provider.example.net", 993)]);
    dns.srv.insert("_submission._tcp.example.org".to_owned(), vec![("smtp.provider.example.net", 587)]);
    let proposal = detect(&address, &config_on, &dns, &web);
    // Every source was asked: the failing autoconfig, the redirect and its target, the ISPDB and SRV.
    assert!(proposal.asked.iter().any(|asked| asked.starts_with(ISPDB)), "{:?}", proposal.asked);
    assert!(proposal.asked.contains(&target.to_owned()), "{:?}", proposal.asked);
    assert!(proposal.imap.iter().any(|candidate| candidate.source == Source::Srv));
    for asked in web.asked().iter().chain(&proposal.asked) {
        assert!(!asked.contains("zq-distinctive-local") && !asked.contains('@'), "{asked}");
    }
}

#[test]
fn plain_http_counts_only_as_a_redirect_to_https_on_the_mx_domain() {
    let dns = FakeDns { mx: vec!["mx.mail.provider.example.net"], ..FakeDns::default() };
    let target = "https://autoconfig.provider.example.net/mail/config-v1.1.xml?emailaddress=x";
    let web = FakeWeb::default().redirect(PLAIN, target).page(target, &config("imap.provider.example.net", 993, "SSL"));
    let proposal = detect(&address(), &DetectConfig::default(), &dns, &web);
    assert_eq!(imap(&proposal), ["imap.provider.example.net:993 (TLS) AutoconfigRedirect"]);
    assert_eq!(web.asked().last().map(String::as_str), Some(target));

    for elsewhere in [
        "https://evil.example.com/config-v1.1.xml",
        "http://autoconfig.provider.example.net/mail/config-v1.1.xml",
        "https://user@autoconfig.provider.example.net/x",
        "https://provider.example.net.evil.example.com/x",
    ] {
        let web = FakeWeb::default().redirect(PLAIN, elsewhere).page(elsewhere, &config("imap.evil.example.com", 993, "SSL"));
        let proposal = detect(&address(), &DetectConfig::default(), &dns, &web);
        assert!(proposal.imap.iter().all(|candidate| candidate.source == Source::Guess), "{elsewhere}");
        assert!(!web.asked().contains(&elsewhere.to_owned()), "{elsewhere}");
    }

    // A plain HTTP page itself is never read, and without MX no redirect is taken.
    let web = FakeWeb::default().page(PLAIN, &config("imap.example.org", 993, "SSL"));
    assert!(detect(&address(), &DetectConfig::default(), &dns, &web).imap.iter().all(|candidate| candidate.source == Source::Guess));
    let web = FakeWeb::default().redirect(PLAIN, target).page(target, &config("imap.provider.example.net", 993, "SSL"));
    assert!(detect(&address(), &DetectConfig::default(), &FakeDns::default(), &web).imap.iter().all(|candidate| candidate.source == Source::Guess));
}

#[test]
fn srv_records_come_next_the_encrypted_first() {
    let mut srv = HashMap::new();
    srv.insert("_imap._tcp.example.org".to_owned(), vec![("imap.example.org", 143)]);
    srv.insert("_imaps._tcp.example.org".to_owned(), vec![("imap.example.org", 993), ("imap.example.org", 993)]);
    srv.insert("_submission._tcp.example.org".to_owned(), vec![("smtp.example.org", 587)]);
    let dns = FakeDns { srv, ..FakeDns::default() };
    let proposal = detect(&address(), &DetectConfig::default(), &dns, &FakeWeb::default());
    assert_eq!(imap(&proposal), ["imap.example.org:993 (TLS) Srv", "imap.example.org:143 (STARTTLS) Srv"]);
    assert_eq!(proposal.submission.len(), 1);
    assert_eq!(proposal.submission[0].server.security(), Security::StartTls);
    assert_eq!(proposal.imap[0].username, "ada@example.org");
}

#[test]
fn common_host_names_are_a_hint_when_nothing_else_answers() {
    let dns = FakeDns { mx_fails: true, ..FakeDns::default() };
    let web = FakeWeb::default().failing(AUTOCONFIG, ErrorKind::TimedOut).page(WELL_KNOWN, "<html>not autoconfig</html>");
    let proposal = detect(&address(), &DetectConfig::default(), &dns, &web);
    assert_eq!(imap(&proposal), ["imap.example.org:993 (TLS) Guess", "mail.example.org:993 (TLS) Guess"]);
    let submission: Vec<String> = proposal.submission.iter().map(|candidate| candidate.server.to_string()).collect();
    assert_eq!(submission, ["smtp.example.org:465 (TLS)", "mail.example.org:587 (STARTTLS)"]);
    assert!(proposal.failed.contains(&("dns MX example.org".to_owned(), ErrorKind::TimedOut)));
    assert!(proposal.failed.contains(&(AUTOCONFIG.to_owned(), ErrorKind::TimedOut)));
    assert!(proposal.failed.contains(&(WELL_KNOWN.to_owned(), ErrorKind::Protocol)));
    assert!(proposal.failed.contains(&("dns SRV _submission._tcp.example.org".to_owned(), ErrorKind::TimedOut)));
    // Guessing asks nothing.
    assert_eq!(web.asked().len(), 3);
}

#[test]
fn a_plain_http_failure_is_noted() {
    let web = FakeWeb::default().failing(PLAIN, ErrorKind::ConnectionRefused);
    let proposal = detect(&address(), &DetectConfig::default(), &FakeDns::default(), &web);
    assert!(proposal.failed.contains(&(PLAIN.to_owned(), ErrorKind::ConnectionRefused)));
}

#[test]
fn the_mx_domain_is_the_last_two_labels_or_three_under_a_country() {
    let base = |host: &str| base_domain(&Host::new(host).unwrap());
    assert_eq!(base("aspmx.l.example.com").as_deref(), Some("example.com"));
    assert_eq!(base("mx.Example.co.uk").as_deref(), Some("example.co.uk"));
    assert_eq!(base("example.org").as_deref(), Some("example.org"));
    assert_eq!(base("mx.example.de").as_deref(), Some("example.de"));
    assert_eq!(base("localhost"), None);
}

#[test]
fn only_https_hosts_without_user_information_count() {
    assert_eq!(https_host("https://Autoconfig.Example.net/path?q#f").as_deref(), Some("autoconfig.example.net"));
    assert_eq!(https_host("HTTPS://example.net:8443/x").as_deref(), Some("example.net"));
    assert_eq!(https_host("https://example.net:bad/x"), None);
    assert_eq!(https_host("http://example.net/"), None);
    assert_eq!(https_host("https://a@example.net/"), None);
    assert_eq!(https_host("https:"), None);
    assert_eq!(https_host("https://"), None);
    for url in [
        "https://evil.example\\@mx.example.net/",
        "https://mx.example.net\\.evil.example/",
        "https://mx.\u{ff45}xample.net/",
        "https://[::1]/",
        "https://mx%2eexample.net/",
        "https://mx.example.net./",
        "https://.example.net/",
        "https://mx.example .net/",
    ] {
        assert_eq!(https_host(url), None, "{url}");
    }
    assert!(within("a.example.net", "example.net"));
    assert!(within("example.net", "example.net"));
    assert!(!within("badexample.net", "example.net"));
}
