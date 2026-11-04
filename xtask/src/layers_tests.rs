// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use tempfile::TempDir;

use super::{CRATES, Class, INTRA_LAYER_ORDER, NETWORK_CRATES, UI_CRATES, class, gate};
use crate::test_support::{Broken, context, manifest, run_in, run_xtask, write};
use crate::{Error, FAILED, OK, count};

/// A workspace with `crates/<name>` for every `(name, dependency tables)`.
fn fixture(crates: &[(&str, &str)]) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n");
    for (name, deps) in crates {
        write(dir.path(), &format!("crates/{name}/Cargo.toml"), &manifest(name, deps));
    }
    dir
}

fn check(crates: &[(&str, &str)]) -> (bool, String) {
    let dir = fixture(crates);
    let mut out = Vec::new();
    let passed = gate(dir.path(), &mut out).unwrap();
    (passed, String::from_utf8(out).unwrap())
}

fn fails_with(crates: &[(&str, &str)], message: &str) {
    let (passed, out) = check(crates);
    assert!(!passed, "{crates:?} should fail: {out}");
    assert!(out.contains(message), "{crates:?}: expected {message:?} in {out}");
}

fn passes(crates: &[(&str, &str)]) {
    let (passed, out) = check(crates);
    assert!(passed, "{crates:?} should pass: {out}");
    assert!(out.ends_with(&format!("layers: {} checked, no violations\n", count(crates.len(), "crate"))), "{out}");
}

fn dep(name: &str) -> String {
    format!("[dependencies]\n{name} = {{ path = \"../{name}\" }}\n")
}

#[test]
fn an_unregistered_crate_fails() {
    fails_with(&[("inboxcraft-frob", "")], "layers: crates/inboxcraft-frob: `inboxcraft-frob` is not registered; add it to xtask/src/layers.rs");
}

#[test]
fn an_upward_dependency_fails() {
    fails_with(
        &[("inboxcraft-mime", &dep("inboxcraft-engine")), ("inboxcraft-engine", "")],
        "`inboxcraft-mime` (layer 2) depends on `inboxcraft-engine` (layer 4): a crate depends only on lower layers",
    );
}

#[test]
fn a_dependency_on_a_lower_layer_or_a_standalone_crate_passes() {
    passes(&[
        ("inboxcraft-engine", &format!("{}inboxcraft-control-key = {{ path = \"x\" }}\n", dep("inboxcraft-model"))),
        ("inboxcraft-model", &dep("inboxcraft-time")),
        ("inboxcraft-time", ""),
        ("inboxcraft-control-key", ""),
    ]);
}

#[test]
fn a_dependency_inside_one_layer_passes_only_in_the_intra_layer_order() {
    passes(&[("inboxcraft-net", &dep("inboxcraft-proto")), ("inboxcraft-proto", "")]);
    passes(&[("inboxcraft-net", &dep("inboxcraft-dav")), ("inboxcraft-dav", "")]);
    fails_with(
        &[("inboxcraft-proto", &dep("inboxcraft-net")), ("inboxcraft-net", "")],
        "`inboxcraft-proto` (layer 3) depends on `inboxcraft-net` (layer 3) in its own layer, which INTRA_LAYER_ORDER does not allow",
    );
    fails_with(&[("inboxcraft-model", &dep("inboxcraft-html")), ("inboxcraft-html", "")], "in its own layer");
}

#[test]
fn a_standalone_crate_with_a_workspace_dependency_fails() {
    fails_with(
        &[("inboxcraft-charset", &dep("inboxcraft-time")), ("inboxcraft-time", "")],
        "`inboxcraft-charset` is standalone and depends on the workspace crate `inboxcraft-time`",
    );
    let dev = "[dev-dependencies]\ninboxcraft-control-key = { path = \"x\" }\n";
    fails_with(&[("inboxcraft-charset", dev), ("inboxcraft-control-key", "")], "is standalone");
    passes(&[("inboxcraft-charset", "[dependencies]\nserde = \"1\"\n")]);
}

#[test]
fn testkit_as_a_normal_or_build_dependency_fails() {
    fails_with(
        &[("inboxcraft-proto", &dep("inboxcraft-testkit")), ("inboxcraft-testkit", "")],
        "`inboxcraft-proto` has `inboxcraft-testkit` as a normal dependency; testkit is for dev-dependencies only",
    );
    let build = "[build-dependencies]\ninboxcraft-testkit = { path = \"x\" }\n";
    fails_with(&[("inboxcraft", build), ("inboxcraft-testkit", "")], "as a build dependency");
    let dev = "[dev-dependencies]\ninboxcraft-testkit = { path = \"x\" }\n";
    passes(&[("inboxcraft-mime", dev), ("inboxcraft-testkit", &dep("inboxcraft-mime"))]);
}

#[test]
fn testkit_depending_on_a_parser_its_fakes_test_fails() {
    let message = "`inboxcraft-testkit` depends on `inboxcraft-proto`; a fake server never uses the parser it tests";
    fails_with(&[("inboxcraft-testkit", &dep("inboxcraft-proto")), ("inboxcraft-proto", "")], message);
    let dev = "[dev-dependencies]\ninboxcraft-proto = { path = \"x\" }\n";
    fails_with(&[("inboxcraft-testkit", dev), ("inboxcraft-proto", "")], message);
}

#[test]
fn a_ui_crate_below_layer_6_fails() {
    for ui in UI_CRATES {
        fails_with(
            &[("inboxcraft-engine", &format!("[dev-dependencies]\n{ui} = \"1\"\n"))],
            &format!("`inboxcraft-engine` (layer 4) depends on the UI crate `{ui}`; UI crates are for layer 6 only"),
        );
    }
    fails_with(&[("inboxcraft-time", "[dependencies]\nwinit = \"1\"\n")], "the UI crate `winit`");
    fails_with(&[("inboxcraft-charset", "[dependencies]\negui = \"1\"\n")], "`inboxcraft-charset` (standalone)");
    passes(&[("inboxcraft-ui-egui", "[dependencies]\neframe = \"1\"\n"), ("inboxcraft", "[dependencies]\nrfd = \"1\"\n")]);
}

#[test]
fn every_class_describes_itself() {
    let described = [Class::Layer(3), Class::Standalone, Class::Testkit, Class::Exempt].map(Class::describe);
    assert_eq!(described, ["layer 3", "standalone", "testkit", "app or tool"]);
    fails_with(&[("inboxcraft-testkit", "[dependencies]\nrfd = \"1\"\n")], "`inboxcraft-testkit` (testkit) depends on the UI crate");
}

#[test]
fn a_network_crate_outside_net_fails() {
    for net in NETWORK_CRATES {
        fails_with(
            &[("inboxcraft-proto", &format!("[dependencies]\n{net} = \"1\"\n"))],
            &format!("`inboxcraft-proto` depends on the network crate `{net}`; only `inboxcraft-net` does network I/O"),
        );
    }
    let renamed = "[target.'cfg(unix)'.dependencies]\ntls = { package = \"rustls\", version = \"1\" }\n";
    fails_with(&[("inboxcraft-engine", renamed)], "the network crate `rustls`");
    passes(&[
        ("inboxcraft-net", "[dependencies]\nrustls = \"1\"\n"),
        ("inboxcraft-testkit", "[dependencies]\nrustls = \"1\"\n"),
        ("inboxcraft-cli", "[dependencies]\nureq = \"1\"\n"),
    ]);
}

#[test]
fn a_dependency_on_an_app_or_tool_crate_fails() {
    fails_with(&[("inboxcraft-engine", &dep("xtask")), ("xtask", "")], "`inboxcraft-engine` depends on `xtask`, an app or tool crate");
    passes(&[("inboxcraft", &dep("inboxcraft-cli")), ("inboxcraft-cli", &dep("inboxcraft-engine")), ("inboxcraft-engine", "")]);
}

#[test]
fn every_violation_is_reported_and_counted() {
    let (passed, out) = check(&[("inboxcraft-frob", ""), ("inboxcraft-time", "[dependencies]\negui = \"1\"\nrustls = \"1\"\n")]);
    assert!(!passed);
    assert!(out.ends_with("layers: 3 violations in 2 crates\n"), "{out}");
    let (_, out) = check(&[("inboxcraft-frob", "")]);
    assert!(out.ends_with("layers: 1 violation in 1 crate\n"), "{out}");
}

#[test]
fn a_dependency_on_an_unregistered_member_is_reported_once() {
    let (passed, out) = check(&[("inboxcraft-engine", &dep("inboxcraft-frob")), ("inboxcraft-frob", "")]);
    assert!(!passed);
    assert_eq!(out.matches("inboxcraft-frob").count(), 2, "{out}");
}

#[test]
fn the_table_is_consistent() {
    for (index, (name, _)) in CRATES.iter().enumerate() {
        assert!(CRATES.iter().skip(index + 1).all(|(other, _)| other != name), "{name} twice");
    }
    for (from, to) in INTRA_LAYER_ORDER {
        assert!(matches!((class(from), class(to)), (Some(Class::Layer(a)), Some(Class::Layer(b))) if a == b));
    }
    assert_eq!(class("inboxcraft-ui-egui"), Some(Class::Layer(6)));
    assert_eq!(class("inboxcraft-testkit"), Some(Class::Testkit));
    assert_eq!(class("nope"), None);
}

#[test]
fn this_workspace_passes() {
    let mut out = Vec::new();
    assert!(gate(&crate::default_root(), &mut out).unwrap(), "{}", String::from_utf8_lossy(&out));
}

#[test]
fn layers_runs_from_the_command_line() {
    let dir = fixture(&[("inboxcraft-frob", "")]);
    let (code, out, _) = run_in(context(dir.path().to_path_buf()), &["layers"]);
    assert_eq!(code, FAILED, "{out}");
    let (code, out, _) = run_xtask(&["layers"]);
    assert_eq!(code, OK, "{out}");
}

#[test]
fn a_workspace_that_cannot_be_read_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(gate(dir.path(), &mut Vec::new()), Err(Error::Io { .. })));
}

#[test]
fn a_broken_report_stops_the_gate() {
    let dir = fixture(&[("inboxcraft-frob", "")]);
    assert!(matches!(gate(dir.path(), &mut Broken), Err(Error::Output(_))));
    let dir = fixture(&[("inboxcraft-time", "")]);
    assert!(matches!(gate(dir.path(), &mut Broken), Err(Error::Output(_))));
}
