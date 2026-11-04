// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use tempfile::TempDir;

use super::{MAX_PATTERN, gate, is_asset, matches, rows};
use crate::test_support::{Broken, context, run_in, write};
use crate::{Error, FAILED, OK};

const TABLE: &str = "\
# Asset attribution

| Asset | Author | Source | Licence | Notes |
|---|---|---|---|---|
| `assets/app-icon/source/inboxcraft.svg` | InboxCraft contributors | drawn | MIT OR Apache-2.0 | |
| `docs/images/<shot>.png` | InboxCraft contributors | ui_shot | MIT OR Apache-2.0 | |
";

fn fixture(files: &[&str]) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "ATTRIBUTION.md", TABLE);
    for rel in files {
        write(dir.path(), rel, "");
    }
    dir
}

fn check(files: &[&str]) -> (bool, String) {
    let dir = fixture(files);
    let mut out = Vec::new();
    let passed = gate(dir.path(), &mut out).unwrap();
    (passed, String::from_utf8(out).unwrap())
}

#[test]
fn an_asset_without_a_row_fails() {
    let (passed, out) = check(&["crates/ui-egui/icons/star.png", "docs/images/inbox.png"]);
    assert!(!passed);
    assert_eq!(
        out,
        "assets: crates/ui-egui/icons/star.png has no row in ATTRIBUTION.md\n\
         assets: 1 of 2 asset files have no row in ATTRIBUTION.md\n"
    );
}

#[test]
fn every_asset_with_a_row_passes() {
    let (passed, out) = check(&["assets/app-icon/source/inboxcraft.svg", "docs/images/inbox.png", "docs/images/dark.png", "src/lib.rs"]);
    assert!(passed, "{out}");
    assert_eq!(out, "assets: 3 asset files, all have a row in ATTRIBUTION.md\n");
}

#[test]
fn anything_under_assets_is_an_asset() {
    let (passed, out) = check(&["assets/sounds/new-mail.raw"]);
    assert!(!passed);
    assert!(out.contains("assets/sounds/new-mail.raw has no row"), "{out}");
}

#[test]
fn ignored_build_and_local_data_directories_are_skipped() {
    let (passed, out) = check(&["dist/x.png", "corpus/y.png", "craft-fonts/z.ttf", "target/a.png", "plan/observe/b.png"]);
    assert!(passed, "{out}");
    assert_eq!(out, "assets: 0 asset files, all have a row in ATTRIBUTION.md\n");
}

#[test]
fn asset_kinds_are_known_by_extension() {
    for rel in ["a.PNG", "b.jpeg", "c.svg", "d.woff2", "e.ogg", "f.ico", "assets/readme.txt"] {
        assert!(is_asset(std::path::Path::new(rel)), "{rel}");
    }
    for rel in ["a.rs", "b.md", "Cargo.toml", "noext", "x/assets/readme.txt"] {
        assert!(!is_asset(std::path::Path::new(rel)), "{rel}");
    }
}

#[test]
fn rows_are_the_backticked_first_cells() {
    let text = "| `a.png` | x |\n|`b/<n>.png`|y|\n| Asset | Author |\n|---|---|\n| `` | z |\n| `c.png\nnot a row `d.png`\n";
    assert_eq!(rows(text), ["a.png", "b/<n>.png"]);
}

#[test]
fn a_placeholder_matches_one_or_more_characters_within_one_directory() {
    assert!(matches("docs/images/<shot>.png", "docs/images/inbox.png"));
    assert!(matches("a/<x>-<y>.png", "a/b-c-d.png"));
    assert!(matches("exact.png", "exact.png"));
    assert!(!matches("docs/images/<shot>.png", "docs/images/.png"));
    assert!(!matches("docs/images/<shot>.png", "docs/images/sub/inbox.png"));
    assert!(!matches("docs/images/<shot>.png", "docs/images/inbox.svg"));
    assert!(!matches("exact.png", "exact.pngx"));
    assert!(!matches("a<unclosed.png", "abc.png"));
    assert!(matches("a<unclosed.png", "a<unclosed.png"));
}

#[test]
fn a_hostile_pattern_is_ignored_not_a_slow_match() {
    let long = "<a>".repeat(MAX_PATTERN);
    assert!(!matches(&long, &"a".repeat(MAX_PATTERN)));
    let many = "<a>".repeat(40);
    assert!(!matches(&format!("{many}!"), &"a".repeat(60)));
}

#[test]
fn a_missing_attribution_file_is_an_io_error() {
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(gate(dir.path(), &mut Vec::new()), Err(Error::Io { .. })));
}

#[test]
fn a_broken_report_stops_the_gate() {
    let dir = fixture(&["x.png"]);
    assert!(matches!(gate(dir.path(), &mut Broken), Err(Error::Output(_))));
    let dir = fixture(&[]);
    assert!(matches!(gate(dir.path(), &mut Broken), Err(Error::Output(_))));
}

#[test]
fn assets_runs_from_the_command_line() {
    let dir = fixture(&["x.png"]);
    let (code, out, _) = run_in(context(dir.path().to_path_buf()), &["assets"]);
    assert_eq!(code, FAILED, "{out}");
    let (code, out, _) = crate::test_support::run_xtask(&["assets"]);
    assert_eq!(code, OK, "{out}");
}
