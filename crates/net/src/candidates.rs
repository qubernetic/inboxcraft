// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Detect's candidate list, without I/O: from the address and what the lookups returned ([`Lookups`]), the ordered,
//! de-duplicated, capped servers to probe for IMAP and for submission ([`candidates`]).
//!
//! The order, as far as the lookups have [`Reached`]:
//!
//! 1. The autoconfig files at the address's domain, through the redirect and from the ISPDB, as they were read.
//! 2. SRV records (RFC 6186): `_imaps._tcp`, `_imap._tcp`, `_submissions._tcp`, `_submission._tcp`.
//! 3. `mail.` and `imap.`, or `mail.` and `smtp.`, of the address's own domain.
//! 4. For each provider domain of the MX hosts ([`provider_domains`]), last because it is a guess about a third
//!    party: its autoconfig file, then `mail.` and `imap.` (IMAP) or `mail.` and `smtp.` (submission) of it.
//!
//! A host made up from a domain gets the registered pairs only: IMAP 993 with implicit TLS, then 143 with STARTTLS;
//! submission 465 with implicit TLS, then 587 with STARTTLS. There is no candidate without encryption, none on port
//! 25, none whose host is not a plain ASCII name of at least two labels ([`usable_host`]), and at most
//! [`MAX_CANDIDATES`] of each kind; the first of two equal servers keeps its place. While the domain names servers
//! of a kind (its autoconfig, the ISPDB or SRV: [`Source::is_named`]), the guesses of that kind (its own common host
//! names and the MX providers': [`Source::is_guess`]) are left out.

use std::num::NonZeroU16;

use inboxcraft_model::{EmailAddress, Host, Security, Server};

use crate::autoconfig::Autoconfig;
use crate::detect::{Candidate, Checked, Source, base_domain};
use crate::dns::{MAX_RECORDS, Srv};
use crate::probe::Service;

/// The most candidates of each kind.
pub const MAX_CANDIDATES: usize = 24;

/// The most MX hosts kept.
pub const MAX_MX: usize = MAX_RECORDS;

/// The most provider domains taken from the MX hosts.
pub const MAX_PROVIDERS: usize = 2;

/// The most autoconfig files kept.
pub const MAX_FILES: usize = 8;

/// The SMTP relay port, never a candidate: submission is 465 or 587.
const RELAY_PORT: u16 = 25;

/// An SRV name Detect asks (RFC 6186, RFC 8314).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SrvName {
    /// `_imaps._tcp`: IMAP with implicit TLS.
    Imaps,
    /// `_imap._tcp`: IMAP with STARTTLS.
    Imap,
    /// `_submissions._tcp`: submission with implicit TLS.
    Submissions,
    /// `_submission._tcp`: submission with STARTTLS.
    Submission,
}

impl SrvName {
    /// Every name, in the order asked.
    pub const ALL: [Self; 4] = [Self::Imaps, Self::Imap, Self::Submissions, Self::Submission];

    /// The service label: `_imaps`.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Imaps => "_imaps",
            Self::Imap => "_imap",
            Self::Submissions => "_submissions",
            Self::Submission => "_submission",
        }
    }

    /// What it is for.
    #[must_use]
    pub fn service(self) -> Service {
        match self {
            Self::Imaps | Self::Imap => Service::Imap,
            Self::Submissions | Self::Submission => Service::Submission,
        }
    }

    /// Its encryption.
    #[must_use]
    pub fn security(self) -> Security {
        match self {
            Self::Imaps | Self::Submissions => Security::Tls,
            Self::Imap | Self::Submission => Security::StartTls,
        }
    }
}

/// How far the lookups have come: the list holds the sources up to this one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Reached {
    /// The autoconfig files at the domain and the ISPDB.
    #[default]
    Files,
    /// The SRV records.
    Srv,
    /// The address's own domain.
    Own,
    /// The provider domains of the MX hosts.
    Providers,
}

/// What the lookups returned, each list capped as it is filled.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Lookups {
    mx: Vec<Host>,
    files: Vec<(Source, Autoconfig)>,
    srv: Vec<(SrvName, Vec<Srv>)>,
    provider_files: Vec<(String, Autoconfig)>,
    reached: Reached,
}

impl Lookups {
    /// The MX hosts, by preference: the usable ones, without repeats, at most [`MAX_MX`].
    pub fn set_mx(&mut self, hosts: impl IntoIterator<Item = Host>) {
        self.mx.clear();
        for host in hosts.into_iter().filter_map(|host| usable_host(host.as_str())) {
            if self.mx.len() >= MAX_MX {
                break;
            }
            if !self.mx.contains(&host) {
                self.mx.push(host);
            }
        }
    }

    /// The MX hosts kept.
    #[must_use]
    pub fn mx(&self) -> &[Host] {
        &self.mx
    }

    /// An autoconfig file read at the domain, through the redirect or from the ISPDB; past [`MAX_FILES`] it is left
    /// out.
    pub fn add_file(&mut self, source: Source, file: Autoconfig) {
        if self.files.len() + self.provider_files.len() < MAX_FILES {
            self.files.push((source, file));
        }
    }

    /// Whether a file read so far names an IMAP server.
    #[must_use]
    pub fn files_name_imap(&self) -> bool {
        self.files.iter().any(|(_, file)| !file.imap.is_empty())
    }

    /// The SRV records of `name`, at most [`MAX_RECORDS`].
    pub fn add_srv(&mut self, name: SrvName, records: impl IntoIterator<Item = Srv>) {
        self.srv.push((name, records.into_iter().take(MAX_RECORDS).collect()));
    }

    /// The autoconfig file of the provider domain `provider`; past [`MAX_FILES`] it is left out.
    pub fn add_provider_file(&mut self, provider: &str, file: Autoconfig) {
        if self.files.len() + self.provider_files.len() < MAX_FILES {
            self.provider_files.push((provider.to_owned(), file));
        }
    }

    /// The lookups have come to `reached`.
    pub fn reach(&mut self, reached: Reached) {
        self.reached = self.reached.max(reached);
    }
}

/// The candidates of each kind, the best first.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Candidates {
    /// IMAP servers.
    pub imap: Vec<Candidate>,
    /// Submission servers.
    pub submission: Vec<Candidate>,
}

impl Candidates {
    /// The list of `service`.
    #[must_use]
    pub fn of(&self, service: Service) -> &[Candidate] {
        match service {
            Service::Imap => &self.imap,
            Service::Submission => &self.submission,
        }
    }

    fn push(&mut self, service: Service, server: Server, username: &str, source: Source) {
        let list = match service {
            Service::Imap => &mut self.imap,
            Service::Submission => &mut self.submission,
        };
        if list.len() >= MAX_CANDIDATES || server.port().get() == RELAY_PORT {
            return;
        }
        let Some(host) = usable_host(server.host().as_str()) else { return };
        let server = Server::new(host, server.port(), server.security());
        if list.iter().any(|held| held.server == server) {
            return;
        }
        list.push(Candidate { server, username: username.to_owned(), source, checked: Checked::NoAnswer });
    }

    fn file(&mut self, file: &Autoconfig, source: Source) {
        for offer in &file.imap {
            self.push(Service::Imap, offer.server.clone(), &offer.username, source);
        }
        for offer in &file.submission {
            self.push(Service::Submission, offer.server.clone(), &offer.username, source);
        }
    }

    /// `mail.`, `imap.` and `smtp.` of `domain`, with the registered pairs.
    fn hosts(&mut self, domain: &str, username: &str, source: Source) {
        let services = [
            (Service::Imap, ["mail", "imap"], [(993, Security::Tls), (143, Security::StartTls)]),
            (Service::Submission, ["mail", "smtp"], [(465, Security::Tls), (587, Security::StartTls)]),
        ];
        for (service, names, pairs) in services {
            for name in names {
                let Ok(host) = Host::new(&format!("{name}.{domain}")) else { continue };
                for (port, security) in pairs {
                    if let Some(port) = NonZeroU16::new(port) {
                        self.push(service, Server::new(host.clone(), port, security), username, source);
                    }
                }
            }
        }
    }
}

/// The candidates for `address` from `lookups`, as the module says.
#[must_use]
pub fn candidates(address: &EmailAddress, lookups: &Lookups) -> Candidates {
    let mut list = Candidates::default();
    let username = address.as_str();
    for (source, file) in &lookups.files {
        list.file(file, *source);
    }
    if lookups.reached >= Reached::Srv {
        for (name, records) in &lookups.srv {
            for record in records {
                list.push(name.service(), Server::new(record.host.clone(), record.port, name.security()), username, Source::Srv);
            }
        }
    }
    let own = address.domain().to_lowercase();
    if lookups.reached >= Reached::Own {
        list.hosts(&own, username, Source::Guess);
    }
    if lookups.reached >= Reached::Providers {
        for provider in provider_domains(&lookups.mx, &own) {
            for (_, file) in lookups.provider_files.iter().filter(|(of, _)| *of == provider) {
                list.file(file, Source::ProviderAutoconfig);
            }
            list.hosts(&provider, username, Source::Provider);
        }
    }
    // While the domain names servers of a kind, no guess stands in their place.
    for kind in [&mut list.imap, &mut list.submission] {
        if kind.iter().any(|candidate| candidate.source.is_named()) {
            kind.retain(|candidate| !candidate.source.is_guess());
        }
    }
    list
}

/// The provider domains of the MX hosts, lowercase, by preference: one for each MX host, its MX domain by the rule of
/// thumb in [`crate::detect`] (the last two labels, or three under a two-letter country label), so that two MX hosts
/// at different providers give two; never `own` (the address's domain), never twice, at most [`MAX_PROVIDERS`].
/// Without a public suffix list this is careful: `mx1.mail.example.net` gives `example.net`, `mx.example.co.uk`
/// gives `example.co.uk` and never `co.uk`; but `mx.abc.de` gives `mx.abc.de`, not `abc.de`.
#[must_use]
pub fn provider_domains(mx: &[Host], own: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for top in mx.iter().filter_map(base_domain) {
        if found.len() >= MAX_PROVIDERS {
            break;
        }
        if top != own && !found.contains(&top) {
            found.push(top);
        }
    }
    found
}

/// `name` as a host Detect may connect to, lowercase: a valid host name ([`Host`]: labels of 1 to 63 bytes, at most
/// 253 in all, no space, CR, LF or NUL) of plain ASCII letters, digits, `-` and `_`, with at least two labels, and not
/// an address: no IP literal in brackets, and a last label that is not all digits.
#[must_use]
pub fn usable_host(name: &str) -> Option<Host> {
    let host = Host::new(name).ok()?;
    let lower = host.as_str().to_ascii_lowercase();
    let plain = lower.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.');
    let last = lower.rsplit('.').next()?;
    if !plain || !lower.contains('.') || last.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Host::new(&lower).ok()
}

#[cfg(test)]
#[path = "candidates_tests.rs"]
mod tests;
