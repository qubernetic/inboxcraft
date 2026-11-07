// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::Path;

use inboxcraft_time::Timestamp;

use super::{Problem, date_of, gate, read_fragment, today};
use crate::test_support::{Broken, context, run_in, write};
use crate::{FAILED, OK, USAGE};

const DATE: &str = "2026-11-07";

const CHANGELOG: &str = "\
# Changelog

Intro.

## [Unreleased]

### Added
- Old thing (#1)

### Fixed
- Old fix (#2)

## [0.1.0] - 2026-01-01

### Added
- Released (#0)
";

const ROADMAP: &str = "\
# Roadmap

> **Last reviewed:** <date> · **Last updated:** 2026-01-01 · **Change:** minor

## Progress log
| Date | What landed |
|---|---|
| 2026-01-01 | Old (#1) |

## Revision history
| Date | Change | Summary |
|---|---|---|
| 2026-01-01 | major | First version |
";

const GAPS: &str = "\
# Gaps

> **Last reviewed:** <date> · **Last updated:** 2026-01-01 · **Change:** minor

## Known rough edges (not ranked)
- Old gap (#1).

## Revision history
| Date | Change | Summary |
|---|---|---|
| 2026-01-01 | major | First version |
";

const GUIDE: &str = "\
# Guide

> **Last reviewed:** 2026-01-01 · **Last updated:** 2026-01-01 · **Change:** minor

Text.

## Revision history
| Date | Change | Summary |
|---|---|---|
| 2026-01-01 | major | First version |
";

/// A workspace with the four documents a fold writes into.
fn workspace() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "CHANGELOG.md", CHANGELOG);
    write(dir.path(), "ROADMAP.md", ROADMAP);
    write(dir.path(), "docs/gaps.md", GAPS);
    write(dir.path(), "docs/guide.md", GUIDE);
    dir
}

/// Writes the three fixture fragments, out of issue order on purpose.
fn three_fragments(root: &Path) {
    write(root, "changes/12.md", "## Changelog\n### Added\n- Twelfth thing (#12)\n### Changed\n- Twelfth change (#12)\n");
    write(
        root,
        "changes/3.md",
        "# 3\n\n## Changelog\n### Added\n- Third thing, wrapped\n  over two lines (#3)\n\n## Roadmap\n- Third landed, wrapped\n  in two lines (#3)\n\n## Revisions\n- ROADMAP.md | minor | Progress log: third (#3)\n",
    );
    write(
        root,
        "changes/7.md",
        "## Changelog\n### Security\n- Seventh guard (#7)\n### Fixed\n- Seventh fix (#7)\n\n## Gaps\n- Seventh gap, wrapped\n  in two lines (#7).\n\n## Revisions\n- docs/gaps.md | minor | Seventh gap (#7)\n- docs/guide.md | major | Rewritten (#7)\n",
    );
}

fn read(root: &Path, rel: &str) -> String {
    std::fs::read_to_string(root.join(rel)).unwrap()
}

fn check(root: &Path) -> (bool, String) {
    let mut out = Vec::new();
    let passed = gate(root, false, DATE, &mut out).unwrap();
    (passed, String::from_utf8(out).unwrap())
}

fn fold(root: &Path) -> (bool, String) {
    let mut out = Vec::new();
    let passed = gate(root, true, DATE, &mut out).unwrap();
    (passed, String::from_utf8(out).unwrap())
}

#[test]
fn three_fragments_fold_into_one_changelog() {
    let dir = workspace();
    let root = dir.path();
    three_fragments(root);
    let (passed, out) = fold(root);
    assert!(passed, "{out}");
    assert_eq!(
        read(root, "CHANGELOG.md"),
        "\
# Changelog

Intro.

## [Unreleased]

### Added
- Old thing (#1)
- Third thing, wrapped
  over two lines (#3)
- Twelfth thing (#12)

### Changed
- Twelfth change (#12)

### Fixed
- Old fix (#2)
- Seventh fix (#7)

### Security
- Seventh guard (#7)

## [0.1.0] - 2026-01-01

### Added
- Released (#0)
"
    );
    assert_eq!(out, "changes: folded 3 fragments (#3, #7, #12) into CHANGELOG.md, ROADMAP.md, docs/gaps.md, docs/guide.md\n");
    assert!(!root.join("changes/3.md").exists() && !root.join("changes/7.md").exists() && !root.join("changes/12.md").exists());
}

#[test]
fn the_fold_writes_the_roadmap_the_gaps_and_the_revision_tables() {
    let dir = workspace();
    let root = dir.path();
    three_fragments(root);
    assert!(fold(root).0);
    let roadmap = read(root, "ROADMAP.md");
    assert!(roadmap.contains("**Last updated:** 2026-11-07 · **Change:** minor\n"), "{roadmap}");
    assert!(roadmap.contains("| 2026-01-01 | Old (#1) |\n| 2026-11-07 | Third landed, wrapped in two lines (#3) |\n\n"), "{roadmap}");
    assert!(roadmap.ends_with("| 2026-01-01 | major | First version |\n| 2026-11-07 | minor | Progress log: third (#3) |\n"), "{roadmap}");
    let gaps = read(root, "docs/gaps.md");
    assert!(gaps.contains("- Old gap (#1).\n- Seventh gap, wrapped\n  in two lines (#7).\n\n## Revision history"), "{gaps}");
    assert!(gaps.ends_with("| 2026-11-07 | minor | Seventh gap (#7) |\n"), "{gaps}");
    let guide = read(root, "docs/guide.md");
    assert!(guide.contains("> **Last reviewed:** 2026-01-01 · **Last updated:** 2026-11-07 · **Change:** major\n"), "{guide}");
    assert!(guide.ends_with("| 2026-11-07 | major | Rewritten (#7) |\n"), "{guide}");
}

#[test]
fn the_highest_change_of_a_document_goes_into_its_status_line() {
    let dir = workspace();
    let root = dir.path();
    write(root, "changes/4.md", "## Revisions\n- docs/guide.md | major | Big (#4)\n");
    write(root, "changes/5.md", "## Revisions\n- docs/guide.md | patch | Small (#5)\n- docs/guide.md | minor | Middle (#5)\n");
    assert!(fold(root).0);
    let guide = read(root, "docs/guide.md");
    assert!(guide.contains("**Change:** major\n"), "{guide}");
    assert!(
        guide.ends_with("| 2026-11-07 | major | Big (#4) |\n| 2026-11-07 | patch | Small (#5) |\n| 2026-11-07 | minor | Middle (#5) |\n"),
        "{guide}"
    );
}

#[test]
fn a_changelog_without_kinds_gets_them_in_their_order() {
    let dir = workspace();
    let root = dir.path();
    write(root, "CHANGELOG.md", "# Changelog\n\n## [Unreleased]\n");
    write(root, "changes/9.md", "## Changelog\n### Removed\n- Gone (#9)\n### Added\n- New (#9)\n### Deprecated\n- Old (#9)\n");
    assert!(fold(root).0);
    assert_eq!(
        read(root, "CHANGELOG.md"),
        "# Changelog\n\n## [Unreleased]\n\n### Added\n- New (#9)\n\n### Deprecated\n- Old (#9)\n\n### Removed\n- Gone (#9)\n"
    );
}

#[test]
fn no_fragments_pass_and_fold_nothing() {
    let dir = workspace();
    let root = dir.path();
    assert_eq!(check(root), (true, "changes: no fragments\n".to_owned()));
    assert_eq!(fold(root), (true, "changes: no fragments to fold\n".to_owned()));
    std::fs::create_dir(root.join("changes")).unwrap();
    assert_eq!(check(root), (true, "changes: no fragments\n".to_owned()));
    assert_eq!(read(root, "CHANGELOG.md"), CHANGELOG);
}

#[test]
fn good_fragments_pass_the_check_and_stay() {
    let dir = workspace();
    let root = dir.path();
    three_fragments(root);
    assert_eq!(check(root), (true, "changes: 3 fragments checked\n".to_owned()));
    assert!(root.join("changes/3.md").exists());
    assert_eq!(read(root, "CHANGELOG.md"), CHANGELOG);
}

/// The problems the check finds in `text` as fragment `issue` of the workspace at `root`.
fn problems(root: &Path, issue: u64, text: &str) -> Vec<Problem> {
    read_fragment(root, issue, text).1
}

/// The problems the check finds in `text` as fragment 5 of the fixture workspace.
fn problems_of(text: &str) -> Vec<String> {
    let dir = workspace();
    problems(dir.path(), 5, text).into_iter().map(|(line, message)| format!("{line}: {message}")).collect()
}

#[test]
fn every_malformed_fragment_is_named_with_its_line() {
    let cases: &[(&str, &str)] = &[
        ("", "0: the fragment says nothing"),
        ("# Title\n", "0: the fragment says nothing"),
        ("- x (#5)\n", "1: an entry before the first `## ` section"),
        ("## Notes\n- x (#5)\n", "1: unknown section `Notes`"),
        ("## Changelog\n- x (#5)\n", "2: a Changelog entry outside a `### <kind>`"),
        ("## Changelog\n### Improved\n- x (#5)\n", "2: unknown kind `Improved`"),
        ("## Changelog\n### Added\n- x (#5)\n## Changelog\n- y (#5)\n", "5: a Changelog entry outside a `### <kind>`"),
        ("## Changelog\n### Added\n- x (#5)\n### Added\n- y (#5)\n", "4: a second `### Added`"),
        ("## Changelog\n### Added\n- x (#5)\n## Changelog\n### Fixed\n- y (#5)\n", "4: a second `## Changelog`"),
        ("## Changelog\n### Added\n", "2: `### Added` has no entry"),
        ("## Gaps\n\n## Changelog\n### Added\n- x (#5)\n", "1: `## Gaps` has no entry"),
        ("## Roadmap\n### Added\n- x (#5)\n", "2: `### ` kinds go under `## Changelog` only"),
        ("## Changelog\n### Added\n- x without a number\n", "3: the entry does not name #5"),
        ("## Changelog\n### Added\n- x (#55)\n", "3: the entry does not name #5"),
        ("## Changelog\n### Added\n- x (a#5)\n", "3: the entry does not name #5"),
        ("## Changelog\n### Added\n  stray (#5)\n", "3: a continuation line with no entry above it"),
        ("## Changelog\n### Added\n- x\n\n  late (#5)\n", "5: a continuation line with no entry above it"),
        ("## Changelog\n### Added\n- \n", "3: an empty entry"),
        ("## Changelog\n### Added\n- a\u{0} (#5)\n", "3: a control character"),
        ("## Changelog\n### Added\n- a\tb (#5)\n", "3: a control character"),
        ("## Changelog\n### Added\n- x (#5)\nloose text\n", "4: expected `## <section>`, `### <kind>`, `- <entry>` or a line indented by two spaces"),
        ("# One\n# Two\n## Changelog\n### Added\n- x (#5)\n", "2: a title after the first line"),
        ("## Roadmap\n- a | b (#5)\n## Revisions\n- ROADMAP.md | minor | r (#5)\n", "2: no `|` in a table row"),
        ("## Roadmap\n- x (#5)\n", "0: the Roadmap lines need a `ROADMAP.md` row under `## Revisions`"),
        ("## Gaps\n- x (#5).\n", "0: the Gaps lines need a `docs/gaps.md` row under `## Revisions`"),
        ("## Revisions\n- docs/guide.md minor x (#5)\n", "2: a revision is `- <doc> | <change> | <summary>`"),
        ("## Revisions\n- docs/guide.md | huge | x (#5)\n", "2: the change is major, minor or patch"),
        ("## Revisions\n- docs/guide.md | minor | x\n  more (#5)\n", "3: a revision is one line"),
        ("## Revisions\n- docs/guide.md | minor | a | b (#5)\n", "2: a revision is `- <doc> | <change> | <summary>`"),
        ("## Revisions\n- docs/guide.md | minor | x\n", "2: the entry does not name #5"),
        ("## Revisions\n- ../guide.md | minor | x (#5)\n", "2: `../guide.md` is not a Markdown file in the workspace"),
        ("## Revisions\n- /etc/x.md | minor | x (#5)\n", "2: `/etc/x.md` is not a Markdown file in the workspace"),
        ("## Revisions\n- docs/guide.txt | minor | x (#5)\n", "2: `docs/guide.txt` is not a Markdown file in the workspace"),
        ("## Revisions\n- docs/none.md | minor | x (#5)\n", "2: `docs/none.md` cannot be read"),
        ("## Revisions\n- CHANGELOG.md | minor | x (#5)\n", "2: `CHANGELOG.md` has no `## Revision history` table"),
    ];
    for (text, expected) in cases {
        let found = problems_of(text);
        assert!(found.iter().any(|problem| problem == expected), "{text:?}: expected {expected:?}, found {found:?}");
    }
}

#[test]
fn a_document_without_a_status_line_is_named() {
    let dir = workspace();
    write(dir.path(), "docs/bare.md", "# Bare\n\n## Revision history\n| Date | Change | Summary |\n|---|---|---|\n");
    let found = problems(dir.path(), 5, "## Revisions\n- docs/bare.md | minor | x (#5)\n");
    assert_eq!(found, vec![(2, "`docs/bare.md` has no `**Last updated:** ... · **Change:** ...` status line".to_owned())]);
}

#[test]
fn hostile_file_names_and_contents_fail_the_check() {
    let dir = workspace();
    let root = dir.path();
    write(root, "changes/abc.md", "## Changelog\n### Added\n- x (#5)\n");
    write(root, "changes/05.md", "");
    write(root, "changes/0.md", "");
    write(root, "changes/99999999999999999999999.md", "");
    write(root, "changes/notes.txt", "");
    write(root, "changes/sub/1.md", "");
    std::fs::write(root.join("changes/6.md"), [0xff, 0xfe]).unwrap();
    write(root, "changes/8.md", &format!("## Changelog\n### Added\n- {} (#8)\n", "x".repeat(70_000)));
    let (passed, out) = check(root);
    assert!(!passed);
    let name = "not a fragment: name it `<issue>.md`, the issue number without leading zeros";
    for line in [
        format!("changes: changes/0.md: {name}"),
        format!("changes: changes/05.md: {name}"),
        format!("changes: changes/99999999999999999999999.md: {name}"),
        format!("changes: changes/abc.md: {name}"),
        format!("changes: changes/notes.txt: {name}"),
        format!("changes: changes/sub: {name}"),
        "changes: changes/6.md: not UTF-8".to_owned(),
        "changes: changes/8.md: larger than 65536 bytes".to_owned(),
    ] {
        assert!(out.contains(&format!("{line}\n")), "{line}: {out}");
    }
    assert!(out.ends_with("changes: 8 problems in 8 files\n"), "{out}");
}

#[test]
fn a_malformed_fragment_fails_with_its_file_and_line() {
    let dir = workspace();
    let root = dir.path();
    write(root, "changes/5.md", "## Changelog\n### Added\n- x without a number\n");
    let (passed, out) = check(root);
    assert!(!passed);
    assert_eq!(out, "changes: changes/5.md:3: the entry does not name #5\nchanges: 1 problem in 1 file\n");
}

#[test]
fn the_fold_refuses_a_malformed_fragment_and_writes_nothing() {
    let dir = workspace();
    let root = dir.path();
    three_fragments(root);
    write(root, "changes/20.md", "## Notes\n");
    let (passed, out) = fold(root);
    assert!(!passed);
    assert!(out.ends_with("changes: nothing folded\n"), "{out}");
    assert_eq!(read(root, "CHANGELOG.md"), CHANGELOG);
    assert_eq!(read(root, "ROADMAP.md"), ROADMAP);
    assert!(root.join("changes/3.md").exists());
}

#[test]
fn the_fold_refuses_documents_without_their_anchor_and_writes_nothing() {
    for (rel, text, missing) in [
        ("CHANGELOG.md", "# Changelog\n", "CHANGELOG.md: no `## [Unreleased]` section"),
        (
            "ROADMAP.md",
            "# Roadmap\n\n> **Last updated:** x · **Change:** minor\n\n## Revision history\n|a|\n",
            "ROADMAP.md: no `## Progress log` table",
        ),
        (
            "ROADMAP.md",
            "> **Last updated:** x · **Change:** minor\n## Progress log\ntext\n## Revision history\n|a|\n",
            "ROADMAP.md: no `## Progress log` table",
        ),
        (
            "docs/gaps.md",
            "# Gaps\n\n> **Last updated:** x · **Change:** minor\n\n## Revision history\n|a|\n",
            "docs/gaps.md: no `## Known rough edges (not ranked)` section",
        ),
    ] {
        let dir = workspace();
        let root = dir.path();
        three_fragments(root);
        write(root, rel, text);
        let (passed, out) = fold(root);
        assert!(!passed, "{rel}");
        assert!(out.contains(&format!("changes: {missing}\n")) && out.ends_with("changes: nothing folded\n"), "{out}");
        assert_eq!(read(root, rel), text);
        assert_eq!(read(root, "docs/guide.md"), GUIDE);
        assert!(root.join("changes/7.md").exists());
    }
}

#[test]
fn a_missing_document_is_an_io_error_of_the_fold() {
    let dir = workspace();
    let root = dir.path();
    write(root, "changes/4.md", "## Changelog\n### Added\n- x (#4)\n");
    std::fs::remove_file(root.join("CHANGELOG.md")).unwrap();
    let error = gate(root, true, DATE, &mut Vec::new()).unwrap_err();
    assert!(error.to_string().contains("CHANGELOG.md"), "{error}");
}

#[test]
fn a_closed_report_is_an_output_error() {
    let dir = workspace();
    let root = dir.path();
    assert!(gate(root, false, DATE, &mut Broken).is_err());
    three_fragments(root);
    assert!(gate(root, false, DATE, &mut Broken).is_err());
    assert!(gate(root, true, DATE, &mut Broken).is_err());
    write(root, "changes/20.md", "## Notes\n");
    assert!(gate(root, false, DATE, &mut Broken).is_err());
}

#[test]
fn an_unreadable_changes_entry_is_an_io_error() {
    let dir = workspace();
    let root = dir.path();
    write(root, "changes", "a file, not a directory");
    assert!(gate(root, false, DATE, &mut Vec::new()).is_err());
}

#[test]
fn the_fold_date_is_the_utc_date() {
    let at = |seconds| date_of(Timestamp::from_unix(seconds, 0).unwrap());
    assert_eq!(at(0), "1970-01-01");
    assert_eq!(at(20_764 * 86_400 + 86_399), "2026-11-07");
    assert_eq!(at(-1), "1969-12-31");
    assert_eq!(date_of(Timestamp::MIN), "0000-01-01");
    assert_eq!(date_of(Timestamp::MAX), "10000-12-31");
    let today = today();
    assert_eq!(today.len(), 10, "{today}");
    assert!(today.as_str() > "2026-01-01", "{today}");
}

#[test]
fn xtask_changes_checks_and_folds_from_the_command_line() {
    let dir = workspace();
    let root = dir.path().to_str().unwrap();
    write(dir.path(), "changes/5.md", "## Notes\n");
    let (code, out, _) = run_in(context(dir.path().to_path_buf()), &["changes", "--root", root]);
    assert_eq!(code, FAILED, "{out}");
    write(dir.path(), "changes/5.md", "## Changelog\n### Added\n- x (#5)\n");
    let (code, out, _) = run_in(context(dir.path().to_path_buf()), &["changes", "--root", root]);
    assert_eq!((code, out.as_str()), (OK, "changes: 1 fragment checked\n"));
    let (code, out, _) = run_in(context(dir.path().to_path_buf()), &["changes", "--fold", "--root", root]);
    assert_eq!(code, OK, "{out}");
    assert!(read(dir.path(), "CHANGELOG.md").contains("- Old thing (#1)\n- x (#5)\n"));
    let (code, _, err) = run_in(context(dir.path().to_path_buf()), &["spdx", "--fold"]);
    assert_eq!(code, USAGE);
    assert!(err.contains("`--fold` only goes with `changes`"), "{err}");
}

#[test]
fn this_workspace_has_well_formed_fragments() {
    let (passed, out) = check(&crate::default_root());
    assert!(passed, "{out}");
}
