// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use tempfile::TempDir;

use std::path::Path;

use super::{MAX_PATTERN, gate, is_asset, matches, pinned, read_manifest, rows};
use crate::test_support::{Broken, context, run_in, write};
use crate::{Error, FAILED, OK};

const TABLE: &str = "\
# Asset attribution

| Asset | Author | Source | Licence | Notes |
|---|---|---|---|---|
| `assets/app-icon/source/inboxcraft.svg` | InboxCraft contributors | drawn | MIT OR Apache-2.0 | |
| `docs/images/<shot>.png` | InboxCraft contributors | ui_shot | MIT OR Apache-2.0 | |
";

/// A manifest that governs two folders and pins nothing.
const EMPTY_MANIFEST: &str = "roots = [\"assets/brand\", \"assets/fonts/plex-sans\"]\n";

/// The sha256 of `abc`.
const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

fn fixture(files: &[&str]) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "ATTRIBUTION.md", TABLE);
    write(dir.path(), ".config/assets.toml", EMPTY_MANIFEST);
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
         assets: 1 of 2 asset files have no row in ATTRIBUTION.md\n\
         assets: 0 pinned files, all match .config/assets.toml\n"
    );
}

#[test]
fn every_asset_with_a_row_passes() {
    let (passed, out) = check(&["assets/app-icon/source/inboxcraft.svg", "docs/images/inbox.png", "docs/images/dark.png", "src/lib.rs"]);
    assert!(passed, "{out}");
    assert_eq!(out, "assets: 3 asset files, all have a row in ATTRIBUTION.md\nassets: 0 pinned files, all match .config/assets.toml\n");
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
    assert_eq!(out, "assets: 0 asset files, all have a row in ATTRIBUTION.md\nassets: 0 pinned files, all match .config/assets.toml\n");
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

/// The problems with the pinned files under `root`, or the error that its manifest is.
fn pins(root: &Path) -> Result<Vec<String>, Error> {
    pinned(root, &read_manifest(root)?)
}

/// A tree with `manifest` as `.config/assets.toml` and the files `(path, contents)`.
fn pinned_tree(manifest: &str, files: &[(&str, &str)]) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), ".config/assets.toml", manifest);
    for (rel, text) in files {
        write(dir.path(), rel, text);
    }
    dir
}

fn manifest_pinning(path: &str, sha256: &str) -> String {
    format!(
        "roots = [\"assets/brand\", \"assets/fonts/plex-sans\"]\n\n\
         [[source]]\nid = \"qubernetic-brand\"\nversion = \"v0.3.0\"\n\n\
         [[file]]\npath = \"{path}\"\nsource = \"qubernetic-brand\"\nfrom = \"marks/x.svg\"\nsha256 = \"{sha256}\"\n"
    )
}

#[test]
fn an_unpinned_brand_file_fails() {
    let dir = pinned_tree(EMPTY_MANIFEST, &[("assets/brand/mark.svg", "abc"), ("assets/fonts/figtree/Figtree.ttf", "x")]);
    assert_eq!(pins(dir.path()).unwrap(), ["assets: assets/brand/mark.svg is not pinned in .config/assets.toml"]);
}

#[test]
fn a_changed_pinned_file_fails() {
    let dir = pinned_tree(&manifest_pinning("assets/brand/mark.svg", ABC), &[("assets/brand/mark.svg", "abd")]);
    assert_eq!(pins(dir.path()).unwrap(), ["assets: assets/brand/mark.svg differs from its pinned sha256"]);
}

#[test]
fn a_pinned_file_that_is_missing_fails() {
    let dir = pinned_tree(&manifest_pinning("assets/brand/mark.svg", ABC), &[]);
    assert_eq!(pins(dir.path()).unwrap(), ["assets: assets/brand/mark.svg is pinned but missing"]);
}

#[test]
fn a_pinned_tree_passes() {
    let dir = pinned_tree(&manifest_pinning("assets/brand/mark.svg", &ABC.to_ascii_uppercase()), &[("assets/brand/mark.svg", "abc")]);
    assert_eq!(pins(dir.path()).unwrap(), Vec::<String>::new());
    write(dir.path(), "ATTRIBUTION.md", "| `assets/brand/mark.svg` | x | y | z | |\n");
    let mut out = Vec::new();
    assert!(gate(dir.path(), &mut out).unwrap());
    assert_eq!(
        String::from_utf8(out).unwrap(),
        "assets: 1 asset files, all have a row in ATTRIBUTION.md\nassets: 1 pinned files, all match .config/assets.toml\n"
    );
}

#[test]
fn a_big_pinned_file_is_hashed_in_chunks() {
    let text = "abc".repeat(100_000);
    let dir = pinned_tree(
        &manifest_pinning("assets/brand/big.bin", "0000000000000000000000000000000000000000000000000000000000000000"),
        &[("assets/brand/big.bin", &text)],
    );
    assert_eq!(pins(dir.path()).unwrap(), ["assets: assets/brand/big.bin differs from its pinned sha256"]);
}

#[test]
fn a_manifest_with_a_bad_hash_length_is_an_error_not_a_panic() {
    let dir = pinned_tree(&manifest_pinning("assets/brand/mark.svg", "abc"), &[("assets/brand/mark.svg", "abc")]);
    let error = pins(dir.path()).unwrap_err();
    assert!(matches!(&error, Error::Config { .. }), "{error}");
    assert!(error.to_string().contains("assets/brand/mark.svg"), "{error}");
    let dir = pinned_tree(&manifest_pinning("assets/brand/mark.svg", &"g".repeat(64)), &[]);
    assert!(matches!(pins(dir.path()), Err(Error::Config { .. })));
}

#[test]
fn a_pinned_path_outside_the_repository_is_an_error() {
    for path in ["/etc/passwd", "assets/../../secret", "..", "", "assets/./brand/x.svg", "assets\\\\brand"] {
        let dir = pinned_tree(&manifest_pinning(path, ABC), &[]);
        assert!(matches!(pins(dir.path()), Err(Error::Config { .. })), "{path}");
    }
    for root in ["/assets", "assets/../..", ""] {
        let dir = pinned_tree(&format!("roots = [\"{root}\"]\n"), &[]);
        assert!(matches!(pins(dir.path()), Err(Error::Config { .. })), "{root}");
    }
}

#[test]
fn a_root_that_does_not_exist_yet_is_fine() {
    let dir = pinned_tree("roots = [\"assets/fonts/audiowide\"]\n", &[]);
    assert_eq!(pins(dir.path()).unwrap(), Vec::<String>::new());
}

#[test]
fn a_missing_or_broken_manifest_is_an_error_naming_the_file() {
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(pins(dir.path()), Err(Error::Io { .. })));
    for manifest in [
        "roots = [",
        "",
        "roots = \"assets/brand\"\n",
        "roots = [1]\n",
        "roots = []\nextra = 1\n",
        "roots = []\n[[file]]\npath = \"assets/brand/a\"\n",
        "roots = []\n[[file]]\nsha256 = \"x\"\n",
        "roots = []\nfile = 1\n",
        "roots = []\nfile = [1]\n",
        "roots = []\n[[file]]\npath = \"assets/brand/a\"\nsha256 = 1\n",
        "roots = []\n[[file]]\npath = \"assets/brand/a\"\nsource = \"nowhere\"\nsha256 = \"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad\"\n",
        "roots = []\n[[file]]\npath = \"assets/brand/a\"\nsha = \"x\"\n",
        "roots = []\n[[source]]\nversion = \"v1\"\n",
        "roots = []\nsource = [1]\n",
    ] {
        let dir = pinned_tree(manifest, &[]);
        let error = pins(dir.path()).unwrap_err();
        assert!(matches!(&error, Error::Config { .. }), "{manifest}: {error}");
        assert!(error.to_string().contains("assets.toml"), "{manifest}: {error}");
    }
}

#[test]
fn a_pinned_file_twice_is_an_error() {
    let mut manifest = manifest_pinning("assets/brand/mark.svg", ABC);
    manifest.push_str(&format!("\n[[file]]\npath = \"assets/brand/mark.svg\"\nsha256 = \"{ABC}\"\n"));
    let dir = pinned_tree(&manifest, &[]);
    assert!(matches!(pins(dir.path()), Err(Error::Config { .. })));
}

#[test]
fn the_gate_reports_pin_problems_and_fails() {
    let dir = fixture(&["assets/brand/mark.svg"]);
    write(dir.path(), "ATTRIBUTION.md", "| `assets/brand/mark.svg` | x | y | z | |\n");
    let mut out = Vec::new();
    assert!(!gate(dir.path(), &mut out).unwrap());
    assert_eq!(
        String::from_utf8(out).unwrap(),
        "assets: 1 asset files, all have a row in ATTRIBUTION.md\n\
         assets: assets/brand/mark.svg is not pinned in .config/assets.toml\n\
         assets: 1 problems with the pinned files in .config/assets.toml\n"
    );
    assert!(matches!(gate(dir.path(), &mut Broken), Err(Error::Output(_))));
    write(dir.path(), ".config/assets.toml", "roots = [");
    assert!(matches!(gate(dir.path(), &mut Vec::new()), Err(Error::Config { .. })));
}

#[test]
fn every_file_under_a_root_counts_whatever_its_directory_is_called() {
    let files = [("assets/brand/.cache/x.ttf", "x"), ("assets/brand/target/y.png", "y"), ("assets/brand/plan/z.png", "z")];
    let dir = pinned_tree(EMPTY_MANIFEST, &files);
    assert_eq!(
        pins(dir.path()).unwrap(),
        [
            "assets: assets/brand/.cache/x.ttf is not pinned in .config/assets.toml",
            "assets: assets/brand/plan/z.png is not pinned in .config/assets.toml",
            "assets: assets/brand/target/y.png is not pinned in .config/assets.toml",
        ]
    );
}

#[test]
fn a_root_that_is_a_file_is_an_error_naming_it() {
    let dir = pinned_tree(EMPTY_MANIFEST, &[("assets/brand", "not a folder")]);
    let error = pins(dir.path()).unwrap_err();
    assert!(matches!(&error, Error::Config { .. }), "{error}");
    assert!(error.to_string().contains("root `assets/brand` is not a directory"), "{error}");
}

#[test]
fn a_pinned_directory_is_missing() {
    let dir = pinned_tree(&manifest_pinning("assets/brand/mark.svg", ABC), &[("assets/brand/mark.svg/inner", "abc")]);
    assert_eq!(
        pins(dir.path()).unwrap(),
        ["assets: assets/brand/mark.svg/inner is not pinned in .config/assets.toml", "assets: assets/brand/mark.svg is pinned but missing"]
    );
}

#[cfg(unix)]
#[test]
fn a_symbolic_link_is_a_problem_never_followed() {
    use std::os::unix::fs::symlink;
    let outside = tempfile::tempdir().unwrap();
    write(outside.path(), "secret", "abc");
    write(outside.path(), "dir/secret", "abc");
    // A pinned link to a file outside, with the outside file's sha256.
    let dir = pinned_tree(&manifest_pinning("assets/brand/mark.svg", ABC), &[]);
    std::fs::create_dir_all(dir.path().join("assets/brand")).unwrap();
    symlink(outside.path().join("secret"), dir.path().join("assets/brand/mark.svg")).unwrap();
    assert_eq!(
        pins(dir.path()).unwrap(),
        ["assets: assets/brand/mark.svg is a symbolic link, not followed", "assets: assets/brand/mark.svg is a symbolic link, not followed",]
    );
    // A pin in a linked directory outside the roots.
    let dir = pinned_tree(&manifest_pinning("assets/other/secret", ABC), &[]);
    std::fs::create_dir_all(dir.path().join("assets")).unwrap();
    symlink(outside.path().join("dir"), dir.path().join("assets/other")).unwrap();
    assert_eq!(pins(dir.path()).unwrap(), ["assets: assets/other is a symbolic link, not followed"]);
    // A root that is a link, and a linked directory under a root.
    let dir = pinned_tree(EMPTY_MANIFEST, &[("assets/fonts/plex-sans/a.ttf", "a")]);
    symlink(outside.path().join("dir"), dir.path().join("assets/brand")).unwrap();
    symlink(outside.path().join("dir"), dir.path().join("assets/fonts/plex-sans/linked")).unwrap();
    assert_eq!(
        pins(dir.path()).unwrap(),
        [
            "assets: assets/brand is a symbolic link, not followed",
            "assets: assets/fonts/plex-sans/a.ttf is not pinned in .config/assets.toml",
            "assets: assets/fonts/plex-sans/linked is a symbolic link, not followed",
        ]
    );
}

#[cfg(unix)]
#[test]
fn a_file_name_that_is_not_utf8_is_reported_not_a_panic() {
    use std::os::unix::ffi::OsStrExt;
    let dir = pinned_tree(EMPTY_MANIFEST, &[]);
    std::fs::create_dir_all(dir.path().join("assets/brand")).unwrap();
    std::fs::write(dir.path().join("assets/brand").join(std::ffi::OsStr::from_bytes(b"x\xff.ttf")), "").unwrap();
    assert_eq!(pins(dir.path()).unwrap(), ["assets: assets/brand/x\u{fffd}.ttf is not pinned in .config/assets.toml"]);
}
