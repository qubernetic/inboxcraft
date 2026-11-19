// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::num::NonZeroU16;

use inboxcraft_model::{EmailAddress, Host, Security, Server};

use super::{Candidates, Lookups, MAX_CANDIDATES, MAX_MX, Reached, SrvName, candidates, provider_domains, usable_host};
use crate::autoconfig::{Autoconfig, Offer};
use crate::detect::Source;
use crate::dns::Srv;

fn address() -> EmailAddress {
    EmailAddress::new("ada@example.org").unwrap()
}

fn host(name: &str) -> Host {
    Host::new(name).unwrap()
}

fn server(name: &str, port: u16, security: Security) -> Server {
    Server::new(host(name), NonZeroU16::new(port).unwrap(), security)
}

fn srv(name: &str, port: u16) -> Srv {
    Srv { host: host(name), port: NonZeroU16::new(port).unwrap(), priority: 0, weight: 0 }
}

fn offer(name: &str, port: u16, security: Security) -> Offer {
    Offer { server: server(name, port, security), username: "ada".to_owned() }
}

/// Each candidate as `server source`.
fn rows(list: &Candidates) -> (Vec<String>, Vec<String>) {
    let row =
        |candidates: &[crate::detect::Candidate]| candidates.iter().map(|candidate| format!("{} {:?}", candidate.server, candidate.source)).collect();
    (row(&list.imap), row(&list.submission))
}

/// The order and the de-duplication of the list, as a table: every source, a server repeated in a later source, a
/// relay port, a provider that is the address's own domain, and no guess where the domain names servers.
#[test]
fn the_list_is_ordered_and_without_repeats() {
    let mut lookups = Lookups::default();
    lookups.set_mx([host("mx1.privateemail.com"), host("mx2.privateemail.com"), host("mx.example.org")]);
    // The domain names IMAP servers; the only submission server it names is on the relay port.
    lookups.add_file(
        Source::Autoconfig,
        Autoconfig { imap: vec![offer("imap.example.org", 993, Security::Tls)], submission: vec![offer("smtp.example.org", 25, Security::StartTls)] },
    );
    lookups.add_srv(SrvName::Imaps, [srv("imap.example.org", 993), srv("IMAP.srv.example.net", 993)]);
    lookups.add_srv(SrvName::Imap, [srv("imap.srv.example.net", 143)]);
    lookups.add_srv(SrvName::Submissions, []);
    lookups.add_srv(SrvName::Submission, []);
    lookups.add_provider_file(
        "privateemail.com",
        Autoconfig {
            imap: vec![offer("imap.privateemail.com", 993, Security::Tls)],
            submission: vec![offer("mail.privateemail.com", 465, Security::Tls)],
        },
    );
    lookups.reach(Reached::Providers);
    let (imap, submission) = rows(&candidates(&address(), &lookups));
    // Named by the domain: no guess of the domain or of the provider stands beside them.
    let want_imap = ["imap.example.org:993 (TLS) Autoconfig", "imap.srv.example.net:993 (TLS) Srv", "imap.srv.example.net:143 (STARTTLS) Srv"];
    assert_eq!(imap, want_imap);
    let want_submission = [
        "mail.example.org:465 (TLS) Guess",
        "mail.example.org:587 (STARTTLS) Guess",
        "smtp.example.org:465 (TLS) Guess",
        "smtp.example.org:587 (STARTTLS) Guess",
        "mail.privateemail.com:465 (TLS) ProviderAutoconfig",
        "mail.privateemail.com:587 (STARTTLS) Provider",
        "smtp.privateemail.com:465 (TLS) Provider",
        "smtp.privateemail.com:587 (STARTTLS) Provider",
    ];
    assert_eq!(submission, want_submission);

    // Nothing named: the domain's own hosts, then the provider's file and hosts, without repeats.
    let mut lookups = Lookups::default();
    lookups.set_mx([host("mx1.privateemail.com")]);
    lookups
        .add_provider_file("privateemail.com", Autoconfig { imap: vec![offer("IMAP.privateemail.com", 993, Security::Tls)], submission: Vec::new() });
    lookups.reach(Reached::Providers);
    let want_imap = [
        "mail.example.org:993 (TLS) Guess",
        "mail.example.org:143 (STARTTLS) Guess",
        "imap.example.org:993 (TLS) Guess",
        "imap.example.org:143 (STARTTLS) Guess",
        "imap.privateemail.com:993 (TLS) ProviderAutoconfig",
        "mail.privateemail.com:993 (TLS) Provider",
        "mail.privateemail.com:143 (STARTTLS) Provider",
        "imap.privateemail.com:143 (STARTTLS) Provider",
    ];
    assert_eq!(rows(&candidates(&address(), &lookups)).0, want_imap);
}

#[test]
fn the_list_holds_only_the_sources_reached() {
    let mut lookups = Lookups::default();
    lookups.set_mx([host("mx1.privateemail.com")]);
    assert!(candidates(&address(), &lookups).imap.is_empty());
    lookups.reach(Reached::Srv);
    assert!(candidates(&address(), &lookups).imap.is_empty());
    lookups.reach(Reached::Own);
    let (imap, _) = rows(&candidates(&address(), &lookups));
    assert_eq!(imap.len(), 4);
    assert_eq!(imap[0], "mail.example.org:993 (TLS) Guess");
    lookups.reach(Reached::Providers);
    let (imap, _) = rows(&candidates(&address(), &lookups));
    assert_eq!(imap.len(), 8);
    assert_eq!(imap[4], "mail.privateemail.com:993 (TLS) Provider");
    // Never back.
    lookups.reach(Reached::Files);
    assert_eq!(candidates(&address(), &lookups).imap.len(), 8);
}

#[test]
fn every_list_is_capped_where_it_is_built() {
    let mut lookups = Lookups::default();
    lookups.set_mx((0..10_000).map(|n| host(&format!("mx{n}.example.net"))));
    assert_eq!(lookups.mx().len(), MAX_MX);
    let offers = (0..100).map(|n| offer(&format!("imap{n}.file.example.net"), 993, Security::Tls)).collect();
    lookups.add_file(Source::Autoconfig, Autoconfig { imap: offers, submission: Vec::new() });
    lookups.add_srv(SrvName::Imaps, (0..10_000).map(|n| srv(&format!("imap{n}.example.net"), 993)));
    lookups.reach(Reached::Own);
    let list = candidates(&address(), &lookups);
    assert_eq!(list.imap.len(), MAX_CANDIDATES);
    for n in 0..20 {
        lookups.add_file(Source::Autoconfig, Autoconfig::default());
        lookups.add_provider_file(&format!("p{n}.example.net"), Autoconfig::default());
    }
    assert_eq!(lookups.files.len() + lookups.provider_files.len(), super::MAX_FILES);
}

#[test]
fn each_mx_host_gives_one_provider_domain_never_above_its_mx_domain() {
    let domains = |hosts: &[&str]| provider_domains(&hosts.iter().map(|name| host(name)).collect::<Vec<_>>(), "example.org");
    assert_eq!(domains(&["mx1.privateemail.com", "mx2.privateemail.com"]), ["privateemail.com"]);
    assert_eq!(domains(&["aspmx.l.example.com"]), ["example.com"]);
    assert_eq!(domains(&["mx.example.co.uk"]), ["example.co.uk"]);
    assert_eq!(domains(&["Provider.Example.NET"]), ["example.net"]);
    assert_eq!(domains(&["example.net"]), ["example.net"]);
    assert_eq!(domains(&["mx.example.org"]), Vec::<String>::new());
    assert_eq!(domains(&["localhost"]), Vec::<String>::new());
    // Two MX hosts at two providers give both.
    assert_eq!(domains(&["mx.filter.example.net", "mx1.privateemail.com"]), ["example.net", "privateemail.com"]);
    assert_eq!(domains(&["mx.one.example", "mx.two.example", "mx.three.example"]), ["one.example", "two.example"]);
    // Without a public suffix list a short label under a country stays: `abc.de` is not tried (a gap).
    assert_eq!(domains(&["mx.abc.de"]), ["mx.abc.de"]);
}

#[test]
fn only_plain_names_are_usable_hosts() {
    assert_eq!(usable_host("Mail.Example.ORG").map(|host| host.as_str().to_owned()).as_deref(), Some("mail.example.org"));
    assert!(usable_host("_imaps.example.org").is_some());
    let long = format!("{}.example.org", "a".repeat(300));
    let too_long = format!("{}example.org", "abcdefghi.".repeat(25));
    for name in [
        long.as_str(),
        too_long.as_str(),
        "a\0b.example.org",
        "a\rb.example.org",
        "a\nb.example.org",
        "a b.example.org",
        "mail.\u{0435}xample.org",
        "[192.0.2.1]",
        "[::1]",
        "192.0.2.1",
        "localhost",
        "",
        ".",
        "a..example.org",
        "-a.example.org",
        "a\\000b.example.org",
    ] {
        assert_eq!(usable_host(name), None, "{name:?}");
    }
}
