// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::HashMap;
use std::num::NonZeroU16;
use std::sync::Mutex;
use std::time::Duration;

use inboxcraft_model::{EmailAddress, ErrorKind, Host, Security, Server};
use inboxcraft_time::{Clock, FakeClock, Timestamp};

use super::{Checked, DetectConfig, ISPDB, Lookup, MAX_TRIES, Outside, Proposal, Source, base_domain, detect, https_host, within};
use crate::NetError;
use crate::candidates::{MAX_MX, MAX_PROVIDERS};
use crate::dns::Srv;
use crate::probe::{Probe, Service};
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
    many_mx: Vec<String>,
    mx_fails: bool,
    srv: HashMap<String, Vec<(&'static str, u16)>>,
}

impl Lookup for FakeDns {
    fn mx(&self, _domain: &Host) -> Result<Vec<Host>, NetError> {
        if self.mx_fails {
            return Err(NetError::new(ErrorKind::TimedOut, "fake"));
        }
        Ok(self.mx.iter().map(|host| Host::new(host).unwrap()).chain(self.many_mx.iter().map(|host| Host::new(host).unwrap())).collect())
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

/// A stand-in probe: the servers that answer (all of them when `answering` is `None`), every probe written down, and
/// a clock each probe moves on by `step`.
struct FakeProbe {
    answering: Option<Vec<&'static str>>,
    fails_with: ErrorKind,
    tried: Mutex<Vec<String>>,
    clock: FakeClock,
    step: Duration,
}

impl FakeProbe {
    fn all() -> Self {
        Self {
            answering: None,
            fails_with: ErrorKind::ConnectionRefused,
            tried: Mutex::default(),
            clock: FakeClock::new(Timestamp::UNIX_EPOCH),
            step: Duration::ZERO,
        }
    }

    fn only(answering: &[&'static str]) -> Self {
        Self { answering: Some(answering.to_vec()), ..Self::all() }
    }

    fn tried(&self) -> Vec<String> {
        self.tried.lock().unwrap().clone()
    }
}

impl Probe for FakeProbe {
    fn probe(&self, server: &Server, service: Service) -> Result<(), NetError> {
        self.tried.lock().unwrap().push(format!("{} {server}", service.word()));
        self.clock.advance(self.step);
        match &self.answering {
            Some(answering) if !answering.contains(&server.to_string().as_str()) => Err(NetError::new(self.fails_with, "fake")),
            _ => Ok(()),
        }
    }
}

/// Detect for `ada@example.org` with every probe answering.
fn run(config: &DetectConfig, dns: &FakeDns, web: &FakeWeb) -> Proposal {
    run_probed(config, dns, web, &FakeProbe::all())
}

fn run_probed(config: &DetectConfig, dns: &FakeDns, web: &FakeWeb, probe: &FakeProbe) -> Proposal {
    detect(&address(), config, &Outside { dns, web, probe, clock: &probe.clock })
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
    let proposal = run(&DetectConfig::default(), &dns, &web);
    assert_eq!(imap(&proposal), ["imap.example.org:993 (TLS) Autoconfig"]);
    assert_eq!(proposal.submission[0].server.to_string(), "smtp.imap.example.org:465 (TLS)");
    assert_eq!(proposal.mx, [Host::new("mx.example.org").unwrap()]);
    assert_eq!(web.asked(), [AUTOCONFIG]);
    // What left the computer: the MX query, the file, and one probe of each server it named; no other source.
    assert_eq!(proposal.asked, ["dns MX example.org", AUTOCONFIG, "imap imap.example.org:993 (TLS)", "submission smtp.imap.example.org:465 (TLS)"]);
    // A detected server gets no password until the owner has seen it.
    let target = proposal.imap[0].target();
    assert!(!target.is_reviewed());
    assert_eq!(target.username(), "ada@example.org");
}

#[test]
fn the_well_known_place_is_tried_next() {
    let web = FakeWeb::default().failing(AUTOCONFIG, ErrorKind::HostNotFound).page(WELL_KNOWN, &config("imap.example.org", 143, "STARTTLS"));
    let proposal = run(&DetectConfig::default(), &FakeDns::default(), &web);
    assert_eq!(imap(&proposal), ["imap.example.org:143 (STARTTLS) Autoconfig"]);
    assert_eq!(proposal.failed, [(AUTOCONFIG.to_owned(), ErrorKind::HostNotFound)]);
}

#[test]
fn with_the_ispdb_off_it_gets_no_request() {
    let ispdb = format!("{ISPDB}example.org");
    let web = FakeWeb::default().page(&ispdb, &config("imap.example.org", 993, "SSL"));
    let dns = FakeDns { mx: vec!["mx1.provider.example.net"], ..FakeDns::default() };
    let proposal = run(&DetectConfig::default(), &dns, &web);
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
    let proposal = run(&config_on, &dns, &web);
    assert_eq!(imap(&proposal), ["imap.example.net:993 (TLS) Ispdb"]);
    let urls = [AUTOCONFIG.to_owned(), WELL_KNOWN.to_owned(), PLAIN.to_owned(), format!("{ISPDB}example.org"), format!("{ISPDB}example.net")];
    assert_eq!(web.asked(), urls);
    let mut asked = vec!["dns MX example.org".to_owned()];
    asked.extend(urls);
    asked.extend(["imap imap.example.net:993 (TLS)".to_owned(), "submission smtp.imap.example.net:465 (TLS)".to_owned()]);
    assert_eq!(proposal.asked, asked);

    // Found for the domain: the MX domain is not asked.
    let web = FakeWeb::default().page(&format!("{ISPDB}example.org"), &config("imap.example.org", 993, "SSL"));
    let proposal = run(&config_on, &dns, &web);
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
    let proposal =
        detect(&address, &config_on, &Outside { dns: &dns, web: &web, probe: &FakeProbe::all(), clock: &FakeClock::new(Timestamp::UNIX_EPOCH) });
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
    let proposal = run(&DetectConfig::default(), &dns, &web);
    assert_eq!(imap(&proposal), ["imap.provider.example.net:993 (TLS) AutoconfigRedirect"]);
    assert_eq!(web.asked().last().map(String::as_str), Some(target));

    for elsewhere in [
        "https://evil.example.com/config-v1.1.xml",
        "http://autoconfig.provider.example.net/mail/config-v1.1.xml",
        "https://user@autoconfig.provider.example.net/x",
        "https://provider.example.net.evil.example.com/x",
    ] {
        let web = FakeWeb::default().redirect(PLAIN, elsewhere).page(elsewhere, &config("imap.evil.example.com", 993, "SSL"));
        let proposal = run(&DetectConfig::default(), &dns, &web);
        assert!(proposal.imap.iter().all(|candidate| not_a_file(candidate.source)), "{elsewhere}");
        assert!(!web.asked().contains(&elsewhere.to_owned()), "{elsewhere}");
    }

    // A plain HTTP page itself is never read, and without MX no redirect is taken.
    let web = FakeWeb::default().page(PLAIN, &config("imap.example.org", 993, "SSL"));
    assert!(run(&DetectConfig::default(), &dns, &web).imap.iter().all(|candidate| not_a_file(candidate.source)));
    let web = FakeWeb::default().redirect(PLAIN, target).page(target, &config("imap.provider.example.net", 993, "SSL"));
    assert!(run(&DetectConfig::default(), &FakeDns::default(), &web).imap.iter().all(|candidate| candidate.source == Source::Guess));
}

/// A source that is no autoconfig file.
fn not_a_file(source: Source) -> bool {
    matches!(source, Source::Srv | Source::Provider | Source::Guess)
}

#[test]
fn srv_records_come_next_the_encrypted_first() {
    let mut srv = HashMap::new();
    srv.insert("_imap._tcp.example.org".to_owned(), vec![("imap.example.org", 143)]);
    srv.insert("_imaps._tcp.example.org".to_owned(), vec![("imap.example.org", 993), ("imap.example.org", 993)]);
    srv.insert("_submission._tcp.example.org".to_owned(), vec![("smtp.example.org", 587)]);
    let dns = FakeDns { srv, ..FakeDns::default() };
    let probe = FakeProbe::only(&["imap.example.org:143 (STARTTLS)", "smtp.example.org:587 (STARTTLS)"]);
    let proposal = run_probed(&DetectConfig::default(), &dns, &FakeWeb::default(), &probe);
    assert_eq!(imap(&proposal), ["imap.example.org:143 (STARTTLS) Srv"]);
    // The encrypted record was probed first, once though it came twice.
    assert_eq!(probe.tried()[..2], ["imap imap.example.org:993 (TLS)", "imap imap.example.org:143 (STARTTLS)"]);
    assert_eq!(proposal.submission.len(), 1);
    assert_eq!(proposal.submission[0].server.security(), Security::StartTls);
    assert_eq!(proposal.imap[0].username, "ada@example.org");
}

#[test]
fn the_domains_own_mail_host_is_probed_when_nothing_else_answers() {
    let dns = FakeDns { mx_fails: true, ..FakeDns::default() };
    let web = FakeWeb::default().failing(AUTOCONFIG, ErrorKind::TimedOut).page(WELL_KNOWN, "<html>not autoconfig</html>");
    let probe = FakeProbe::only(&["mail.example.org:993 (TLS)", "smtp.example.org:587 (STARTTLS)"]);
    let proposal = run_probed(&DetectConfig::default(), &dns, &web, &probe);
    assert_eq!(imap(&proposal), ["mail.example.org:993 (TLS) Guess"]);
    let submission: Vec<String> = proposal.submission.iter().map(|candidate| candidate.server.to_string()).collect();
    assert_eq!(submission, ["smtp.example.org:587 (STARTTLS)"]);
    // The submission pairs in order, until one answered.
    let tried: Vec<String> = probe.tried().into_iter().filter(|tried| tried.starts_with("submission")).collect();
    assert_eq!(
        tried,
        [
            "submission mail.example.org:465 (TLS)",
            "submission mail.example.org:587 (STARTTLS)",
            "submission smtp.example.org:465 (TLS)",
            "submission smtp.example.org:587 (STARTTLS)"
        ]
    );
    assert!(proposal.failed.contains(&("submission mail.example.org:465 (TLS)".to_owned(), ErrorKind::ConnectionRefused)));
    assert!(proposal.failed.contains(&("dns MX example.org".to_owned(), ErrorKind::TimedOut)));
    assert!(proposal.failed.contains(&(AUTOCONFIG.to_owned(), ErrorKind::TimedOut)));
    assert!(proposal.failed.contains(&(WELL_KNOWN.to_owned(), ErrorKind::Protocol)));
    assert!(proposal.failed.contains(&("dns SRV _submission._tcp.example.org".to_owned(), ErrorKind::TimedOut)));
    // Without MX there is no provider domain to ask the web about.
    assert_eq!(web.asked().len(), 3);
}

#[test]
fn a_plain_http_failure_is_noted() {
    let web = FakeWeb::default().failing(PLAIN, ErrorKind::ConnectionRefused);
    let proposal = run(&DetectConfig::default(), &FakeDns::default(), &web);
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

/// The owner's case: the domain answers nothing, its MX points into a provider, and `mail.` of the provider answers.
#[test]
fn a_domain_hosted_by_a_provider_is_found_through_its_mx() {
    let dns = FakeDns { mx: vec!["mx1.privateemail.com", "mx2.privateemail.com"], ..FakeDns::default() };
    let probe = FakeProbe::only(&["mail.privateemail.com:993 (TLS)", "mail.privateemail.com:465 (TLS)"]);
    let web = FakeWeb::default();
    let proposal = run_probed(&DetectConfig::default(), &dns, &web, &probe);
    assert_eq!(imap(&proposal), ["mail.privateemail.com:993 (TLS) Provider"]);
    assert_eq!(proposal.submission[0].server.to_string(), "mail.privateemail.com:465 (TLS)");
    assert_eq!(proposal.submission[0].source, Source::Provider);
    assert_eq!(proposal.imap[0].username, "ada@example.org");
    // The provider's autoconfig was asked, once for both MX hosts; the provider came before the domain's own hosts.
    assert!(web.asked().contains(&"https://autoconfig.privateemail.com/mail/config-v1.1.xml".to_owned()));
    // The domain's own hosts come first: four refused IMAP and four refused submission pairs, then the provider.
    let tried = probe.tried();
    assert_eq!(tried.len(), 10);
    assert_eq!(tried[8], "imap mail.privateemail.com:993 (TLS)");
    assert_eq!(tried[9], "submission mail.privateemail.com:465 (TLS)");
    assert!(!proposal.failed.iter().any(|(what, _)| what.starts_with("detect")));
}

/// The owner's case when the domain's hosts drop every packet: each probe takes 20 seconds, the probe's own cap. A
/// host that timed out is not tried again, so the provider is reached within the run's 120 seconds.
#[test]
fn the_owners_case_fits_the_cap_when_the_domain_drops_packets() {
    let dns = FakeDns { mx: vec!["mx1.privateemail.com"], ..FakeDns::default() };
    let probe = FakeProbe {
        fails_with: ErrorKind::TimedOut,
        step: Duration::from_secs(20),
        ..FakeProbe::only(&["mail.privateemail.com:993 (TLS)", "mail.privateemail.com:465 (TLS)"])
    };
    let proposal = run_probed(&DetectConfig::default(), &dns, &FakeWeb::default(), &probe);
    assert_eq!(imap(&proposal), ["mail.privateemail.com:993 (TLS) Provider"]);
    assert_eq!(proposal.submission[0].server.to_string(), "mail.privateemail.com:465 (TLS)");
    assert_eq!(
        probe.tried(),
        [
            "imap mail.example.org:993 (TLS)",
            "imap imap.example.org:993 (TLS)",
            "submission smtp.example.org:465 (TLS)",
            "imap mail.privateemail.com:993 (TLS)",
            "submission mail.privateemail.com:465 (TLS)"
        ]
    );
    // Five probes of 20 seconds: 100 seconds in all.
    assert_eq!(probe.clock.monotonic(), Duration::from_secs(100));
}

#[test]
fn srv_records_win_over_the_provider_and_the_domain() {
    let mut dns = FakeDns { mx: vec!["mx1.privateemail.com"], ..FakeDns::default() };
    dns.srv.insert("_imaps._tcp.example.org".to_owned(), vec![("imap.provider.example.net", 993)]);
    dns.srv.insert("_submissions._tcp.example.org".to_owned(), vec![("smtp.provider.example.net", 465)]);
    let probe = FakeProbe::all();
    let proposal = run_probed(&DetectConfig::default(), &dns, &FakeWeb::default(), &probe);
    assert_eq!(imap(&proposal), ["imap.provider.example.net:993 (TLS) Srv"]);
    assert_eq!(proposal.submission[0].source, Source::Srv);
    assert_eq!(probe.tried().len(), 2);
}

#[test]
fn when_nothing_answers_nothing_is_found_within_the_cap() {
    let dns = FakeDns { mx: vec!["mx1.privateemail.com"], ..FakeDns::default() };
    let probe = FakeProbe::only(&[]);
    let proposal = run_probed(&DetectConfig::default(), &dns, &FakeWeb::default(), &probe);
    assert!(proposal.imap.is_empty() && proposal.submission.is_empty());
    // Four pairs on each of mail. and imap. (or smtp.) of the provider and of the domain, each probed once.
    assert_eq!(probe.tried().len(), 16);
    assert!(probe.tried().len() <= MAX_TRIES);
    assert!(probe.tried().iter().all(|tried| !tried.contains(":25 ")));
    assert_eq!(proposal.failed.iter().filter(|(what, _)| what.starts_with("imap ") || what.starts_with("submission ")).count(), 16);
}

#[test]
fn the_tries_are_capped() {
    let mut dns = FakeDns::default();
    let many: Vec<(&'static str, u16)> = (0..40).map(|n| (&*String::leak(format!("imap{n}.example.net")), 993)).collect();
    dns.srv.insert("_imaps._tcp.example.org".to_owned(), many);
    let probe = FakeProbe::only(&[]);
    let config = DetectConfig { tries: 5, ..DetectConfig::default() };
    let proposal = run_probed(&config, &dns, &FakeWeb::default(), &probe);
    assert_eq!(probe.tried().len(), 5);
    assert!(proposal.failed.contains(&("detect: the cap of 5 tries".to_owned(), ErrorKind::TooLarge)));
    // Nothing is asked after the cap; the first server the SRV records name stands, unchecked.
    assert_eq!(imap(&proposal), ["imap0.example.net:993 (TLS) Srv"]);
    assert_eq!(proposal.imap[0].checked, Checked::NoAnswer);
    assert!(probe.tried().iter().all(|tried| tried.contains(".example.net")));
}

#[test]
fn the_time_is_capped() {
    let dns = FakeDns { mx: vec!["mx1.privateemail.com"], ..FakeDns::default() };
    let probe = FakeProbe { step: Duration::from_secs(50), ..FakeProbe::only(&[]) };
    let proposal = run_probed(&DetectConfig::default(), &dns, &FakeWeb::default(), &probe);
    // Probes start at 0, 50 and 100 seconds; at 150 the 120 seconds are over.
    assert_eq!(probe.tried().len(), 3);
    assert!(proposal.failed.contains(&("detect: the cap of 120 s".to_owned(), ErrorKind::TimedOut)));
}

#[test]
fn ten_thousand_mx_records_are_capped_where_they_are_kept() {
    let dns = FakeDns { many_mx: (0..10_000).map(|n| format!("mx{n}.p{n}.example")).collect(), ..FakeDns::default() };
    let probe = FakeProbe::only(&[]);
    let web = FakeWeb::default();
    let proposal = run_probed(&DetectConfig::default(), &dns, &web, &probe);
    assert_eq!(proposal.mx.len(), MAX_MX);
    // Two provider domains at most, each with its two autoconfig places.
    let provider_urls = web.asked().iter().filter(|url| url.contains(".example/")).count();
    assert_eq!(provider_urls, 2 * MAX_PROVIDERS);
    assert!(probe.tried().len() <= MAX_TRIES);
}

#[test]
fn hostile_names_from_the_resolver_and_the_files_are_left_out() {
    let dns = FakeDns { mx: vec!["192.0.2.1", "[::1]", "localhost", "mx.\u{0435}xample.net"], ..FakeDns::default() };
    let long = "a".repeat(300);
    let file = format!(
        "<clientConfig><emailProvider>\
         <incomingServer type=\"imap\"><hostname>{long}.example.org</hostname><port>993</port><socketType>SSL</socketType></incomingServer>\
         <incomingServer type=\"imap\"><hostname>a&#0;b.example.org</hostname><port>993</port><socketType>SSL</socketType></incomingServer>\
         <incomingServer type=\"imap\"><hostname>a\r\nb.example.org</hostname><port>993</port><socketType>SSL</socketType></incomingServer>\
         <outgoingServer type=\"smtp\"><hostname>relay.example.org</hostname><port>25</port><socketType>STARTTLS</socketType></outgoingServer>\
         </emailProvider></clientConfig>"
    );
    let web = FakeWeb::default().page(AUTOCONFIG, &file);
    let probe = FakeProbe::all();
    let proposal = run_probed(&DetectConfig::default(), &dns, &web, &probe);
    assert!(proposal.mx.is_empty(), "{:?}", proposal.mx);
    for tried in probe.tried() {
        assert!(tried.contains("example.org:"), "{tried}");
        assert!(!tried.contains(":25 ") && !tried.contains('\0') && tried.len() < 100, "{tried}");
    }
    // Only the domain's own hosts were left to probe.
    assert_eq!(imap(&proposal), ["mail.example.org:993 (TLS) Guess"]);
}

/// What the domain names is never thrown away: every port is blocked, so its autoconfig server is proposed unchecked.
#[test]
fn a_named_server_no_probe_reached_is_proposed_unchecked() {
    let web = FakeWeb::default().page(AUTOCONFIG, &config("imap.example.org", 993, "SSL"));
    let dns = FakeDns { mx: vec!["mx1.privateemail.com"], ..FakeDns::default() };
    let probe = FakeProbe { fails_with: ErrorKind::TimedOut, ..FakeProbe::only(&[]) };
    let proposal = run_probed(&DetectConfig::default(), &dns, &web, &probe);
    assert_eq!(imap(&proposal), ["imap.example.org:993 (TLS) Autoconfig"]);
    assert_eq!(proposal.imap[0].checked, Checked::NoAnswer);
    assert_eq!(proposal.submission[0].server.to_string(), "smtp.imap.example.org:465 (TLS)");
    assert_eq!(proposal.submission[0].checked, Checked::NoAnswer);
    // No guess was probed in their place.
    assert_eq!(probe.tried(), ["imap imap.example.org:993 (TLS)", "submission smtp.imap.example.org:465 (TLS)"]);
}

/// A guessable provider host that answers does not replace the server the domain names.
#[test]
fn a_guess_that_answers_does_not_replace_a_named_server() {
    let web = FakeWeb::default().page(AUTOCONFIG, &config("imap.example.org", 993, "SSL"));
    let dns = FakeDns { mx: vec!["mx1.privateemail.com"], ..FakeDns::default() };
    let probe = FakeProbe {
        fails_with: ErrorKind::ConnectionRefused,
        ..FakeProbe::only(&["mail.privateemail.com:993 (TLS)", "mail.privateemail.com:465 (TLS)", "mail.example.org:993 (TLS)"])
    };
    let proposal = run_probed(&DetectConfig::default(), &dns, &web, &probe);
    assert_eq!(imap(&proposal), ["imap.example.org:993 (TLS) Autoconfig"]);
    assert_eq!(proposal.imap[0].checked, Checked::NoAnswer);
    assert!(probe.tried().iter().all(|tried| !tried.contains("privateemail") && !tried.contains("mail.example.org")), "{:?}", probe.tried());
}

/// A named server that answered and was wrong (no STARTTLS) is not proposed, and no guess takes its place.
#[test]
fn a_named_server_that_answered_wrong_is_not_proposed() {
    let web = FakeWeb::default().page(AUTOCONFIG, &config("imap.example.org", 143, "STARTTLS"));
    let probe = FakeProbe { fails_with: ErrorKind::StartTlsMissing, ..FakeProbe::only(&["mail.example.org:993 (TLS)"]) };
    let proposal = run_probed(&DetectConfig::default(), &FakeDns::default(), &web, &probe);
    assert!(proposal.imap.is_empty() && proposal.submission.is_empty(), "{proposal:?}");
    assert!(proposal.failed.contains(&("imap imap.example.org:143 (STARTTLS)".to_owned(), ErrorKind::StartTlsMissing)));
}
