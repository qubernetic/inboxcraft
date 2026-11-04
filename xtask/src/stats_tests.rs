// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::gate;
use crate::test_support::{Broken, BrokenAfter, context, manifest, run_in, write};
use crate::{Error, OK};

fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "Cargo.toml", "[workspace]\nmembers = [\"crates/*\", \"xtask\"]\n");
    write(dir.path(), "crates/a/Cargo.toml", &manifest("inboxcraft-a", ""));
    write(dir.path(), "crates/a/src/lib.rs", "//! a\n\npub fn a() {}\n\n#[cfg(test)]\nmod tests;\n");
    write(dir.path(), "crates/a/src/tests.rs", "#[test]\nfn t() {}\n");
    write(dir.path(), "crates/a/src/parse_tests.rs", "#[test]\nfn p() {}\n");
    write(dir.path(), "crates/a/tests/it.rs", "#[test]\nfn it() {}\n");
    write(dir.path(), "crates/longer-name/Cargo.toml", &manifest("inboxcraft-longer-name", ""));
    write(dir.path(), "crates/longer-name/src/lib.rs", "\n\n");
    write(dir.path(), "xtask/Cargo.toml", &manifest("xtask", ""));
    write(dir.path(), "xtask/src/main.rs", "fn main() {}\n");
    write(dir.path(), "outside.rs", "fn not_counted() {}\n");
    dir
}

#[test]
fn stats_prints_one_line_per_crate_and_a_total() {
    let dir = fixture();
    let (code, out, _) = run_in(context(dir.path().to_path_buf()), &["stats"]);
    assert_eq!(code, OK);
    assert_eq!(
        out,
        "\
stats: crate                   files  code lines  test lines
stats: inboxcraft-a                4           4           6
stats: inboxcraft-longer-name      1           0           0
stats: xtask                       1           1           0
stats: total                       6           5           6
"
    );
}

#[test]
fn a_root_package_counts_only_the_files_outside_the_other_members() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "Cargo.toml", &format!("{}[workspace]\nmembers = [\"inner\"]\n", manifest("top", "")));
    write(dir.path(), "src/main.rs", "fn main() {}\n");
    write(dir.path(), "inner/Cargo.toml", &manifest("inner", ""));
    write(dir.path(), "inner/src/lib.rs", "pub fn x() {}\npub fn y() {}\n");
    let mut out = Vec::new();
    assert!(gate(dir.path(), &mut out).unwrap());
    let out = String::from_utf8(out).unwrap();
    assert!(out.contains("stats: top        1           1           0\n"), "{out}");
    assert!(out.contains("stats: inner      1           2           0\n"), "{out}");
}

#[test]
fn this_workspace_has_a_line_for_every_crate() {
    let mut out = Vec::new();
    assert!(gate(&crate::default_root(), &mut out).unwrap());
    let out = String::from_utf8(out).unwrap();
    for name in ["inboxcraft-engine", "inboxcraft-ui-egui", "inboxcraft-cli", "xtask"] {
        assert!(out.contains(&format!("stats: {name} ")), "{name}: {out}");
    }
}

#[test]
fn an_unreadable_file_is_an_io_error() {
    let dir = fixture();
    std::fs::write(dir.path().join("crates/a/src/bad.rs"), [0xff]).unwrap();
    assert!(matches!(gate(dir.path(), &mut Vec::new()), Err(Error::Io { .. })));
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(gate(dir.path(), &mut Vec::new()), Err(Error::Io { .. })));
}

#[test]
fn a_broken_report_stops_the_gate() {
    let dir = fixture();
    assert!(matches!(gate(dir.path(), &mut Broken), Err(Error::Output(_))));
    let header = "stats: crate                   files  code lines  test lines\n";
    assert!(matches!(gate(dir.path(), &mut BrokenAfter::new(header.len())), Err(Error::Output(_))));
}
