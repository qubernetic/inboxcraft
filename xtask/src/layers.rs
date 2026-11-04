// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `cargo xtask layers`: the layering rules between the crates (`docs/architecture.md`).
//!
//! Every workspace crate is registered in [`CRATES`]. The gate reads the direct dependencies of every member from its
//! manifest (normal, dev and build, target-specific ones included) and fails on:
//!
//! - a member that is not registered;
//! - a dependency on a higher layer, or on a crate in the same layer that [`INTRA_LAYER_ORDER`] does not list;
//! - a standalone crate with any workspace dependency;
//! - `testkit` as a normal or build dependency (it is for dev-dependencies only);
//! - `testkit` with any dependency on a [`TESTKIT_NEVER`] crate, whose parser its fakes test;
//! - a [`UI_CRATES`] dependency anywhere but layer 6 and the apps;
//! - a [`NETWORK_CRATES`] dependency anywhere but `net`, `testkit` and the apps;
//! - a [`TESTKIT_ONLY_CRATES`] dependency anywhere but `testkit`, unless it is a dev-dependency;
//! - a dependency on an app or tool crate from anything but another app or tool crate.

use std::io::Write;
use std::path::Path;

use crate::workspace::{self, Kind, Member};
use crate::{Error, count};

/// How a crate takes part in the layering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Class {
    /// Layer `n`: depends only on lower layers and standalone crates.
    Layer(u8),
    /// No workspace dependencies at all, so it can leave the workspace any time.
    Standalone,
    /// The dev-only fake servers: a dev-dependency of anything, a normal dependency of nothing.
    Testkit,
    /// The apps and the tools: outside the layering.
    Exempt,
}

impl Class {
    fn describe(self) -> String {
        match self {
            Self::Layer(n) => format!("layer {n}"),
            Self::Standalone => "standalone".to_owned(),
            Self::Testkit => "testkit".to_owned(),
            Self::Exempt => "app or tool".to_owned(),
        }
    }
}

/// Every crate of the workspace and its class. A new crate is registered here in the commit that adds it.
pub(crate) const CRATES: &[(&str, Class)] = &[
    ("inboxcraft-control-key", Class::Standalone),
    ("inboxcraft-charset", Class::Standalone),
    ("inboxcraft-time", Class::Layer(0)),
    ("inboxcraft-model", Class::Layer(1)),
    ("inboxcraft-html", Class::Layer(1)),
    ("inboxcraft-store", Class::Layer(1)),
    ("inboxcraft-mime", Class::Layer(2)),
    ("inboxcraft-ical", Class::Layer(2)),
    ("inboxcraft-vcard", Class::Layer(2)),
    ("inboxcraft-proto", Class::Layer(3)),
    ("inboxcraft-dav", Class::Layer(3)),
    ("inboxcraft-net", Class::Layer(3)),
    ("inboxcraft-engine", Class::Layer(4)),
    ("inboxcraft-mcp", Class::Layer(5)),
    ("inboxcraft-ui-egui", Class::Layer(6)),
    ("inboxcraft-testkit", Class::Testkit),
    ("inboxcraft", Class::Exempt),
    ("inboxcraft-cli", Class::Exempt),
    ("inboxcraft-web", Class::Exempt),
    ("xtask", Class::Exempt),
];

/// The dependencies allowed inside one layer, as `(from, to)`.
pub(crate) const INTRA_LAYER_ORDER: &[(&str, &str)] =
    &[("inboxcraft-store", "inboxcraft-model"), ("inboxcraft-net", "inboxcraft-proto"), ("inboxcraft-net", "inboxcraft-dav")];

/// The crates whose parsers the fake servers test: `testkit` never depends on them, not even for its own tests, so a
/// parser bug cannot hide behind the same bug in the fake.
pub(crate) const TESTKIT_NEVER: &[&str] = &["inboxcraft-proto"];

/// Crates the fakes need and nothing that ships does: `testkit` only, or a dev-dependency. A crate that comes to need
/// one lifts it here as a reviewed choice.
pub(crate) const TESTKIT_ONLY_CRATES: &[&str] = &["rcgen", "flate2"];

/// The crate that does the network I/O.
const NET: &str = "inboxcraft-net";

/// Crates that belong to the front end only (layer 6 and the apps).
pub(crate) const UI_CRATES: &[&str] = &["egui", "eframe", "winit", "egui_kittest", "rfd"];

/// Crates that open sockets, speak TLS or make requests: `net` only (and `testkit` and the apps).
pub(crate) const NETWORK_CRATES: &[&str] = &[
    "hickory-resolver",
    "hyper",
    "hyper-util",
    "mio",
    "native-tls",
    "oauth2",
    "openssl",
    "reqwest",
    "rustls",
    "rustls-native-certs",
    "rustls-platform-verifier",
    "socket2",
    "tokio-native-tls",
    "tokio-rustls",
    "ureq",
    "webpki-roots",
];

/// The class `name` is registered with.
pub(crate) fn class(name: &str) -> Option<Class> {
    CRATES.iter().find(|(crate_name, _)| *crate_name == name).map(|(_, class)| *class)
}

/// Every rule `member` (of class `own`) breaks with its dependencies on the `members` of the workspace.
fn violations(member: &Member, own: Class, members: &[Member]) -> Vec<String> {
    let name = &member.name;
    let mut found = Vec::new();
    for dep in &member.dependencies {
        let target = &dep.name;
        if !members.iter().any(|m| &m.name == target) {
            if UI_CRATES.contains(&target.as_str()) && !matches!(own, Class::Layer(6) | Class::Exempt) {
                found.push(format!("`{name}` ({}) depends on the UI crate `{target}`; UI crates are for layer 6 only", own.describe()));
            }
            if NETWORK_CRATES.contains(&target.as_str()) && name != NET && !matches!(own, Class::Testkit | Class::Exempt) {
                found.push(format!("`{name}` depends on the network crate `{target}`; only `{NET}` does network I/O"));
            }
            if TESTKIT_ONLY_CRATES.contains(&target.as_str()) && own != Class::Testkit && dep.kind != Kind::Dev {
                let kind = if dep.kind == Kind::Build { "build" } else { "normal" };
                found.push(format!("`{name}` has `{target}` as a {kind} dependency; it is for `inboxcraft-testkit` and dev-dependencies only"));
            }
            continue;
        }
        // An unregistered member is reported on its own.
        let Some(theirs) = class(target) else { continue };
        if own == Class::Testkit && TESTKIT_NEVER.contains(&target.as_str()) {
            found.push(format!("`{name}` depends on `{target}`; a fake server never uses the parser it tests"));
        }
        if theirs == Class::Testkit && dep.kind != Kind::Dev {
            let kind = if dep.kind == Kind::Build { "build" } else { "normal" };
            found.push(format!("`{name}` has `{target}` as a {kind} dependency; testkit is for dev-dependencies only"));
        }
        match (own, theirs) {
            (Class::Exempt, _) | (_, Class::Testkit) => {}
            (Class::Standalone, _) => found.push(format!("`{name}` is standalone and depends on the workspace crate `{target}`")),
            (_, Class::Exempt) => found.push(format!("`{name}` depends on `{target}`, an app or tool crate")),
            (Class::Layer(a), Class::Layer(b)) if b > a => {
                found.push(format!("`{name}` (layer {a}) depends on `{target}` (layer {b}): a crate depends only on lower layers"))
            }
            (Class::Layer(a), Class::Layer(b)) if a == b && !INTRA_LAYER_ORDER.contains(&(name.as_str(), target.as_str())) => {
                found.push(format!(
                    "`{name}` (layer {a}) depends on `{target}` (layer {b}) in its own layer, which INTRA_LAYER_ORDER \
                     does not allow"
                ));
            }
            (Class::Layer(_) | Class::Testkit, Class::Layer(_) | Class::Standalone) => {}
        }
    }
    found
}

/// Checks every member of the workspace at `root`, reports to `out` and returns whether all pass.
pub(crate) fn gate(root: &Path, out: &mut dyn Write) -> Result<bool, Error> {
    let members = workspace::members(root)?;
    let mut total = 0_usize;
    for member in &members {
        let found = match class(&member.name) {
            Some(own) => violations(member, own, &members),
            None => vec![format!("{}: `{}` is not registered; add it to xtask/src/layers.rs", member.dir.display(), member.name)],
        };
        for message in &found {
            writeln!(out, "layers: {message}")?;
        }
        total = total.saturating_add(found.len());
    }
    let crates = count(members.len(), "crate");
    if total == 0 {
        writeln!(out, "layers: {crates} checked, no violations")?;
    } else {
        writeln!(out, "layers: {} in {crates}", count(total, "violation"))?;
    }
    Ok(total == 0)
}

#[cfg(test)]
#[path = "layers_tests.rs"]
mod tests;
