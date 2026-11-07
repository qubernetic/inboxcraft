// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::cell::RefCell;
use std::ffi::{OsStr, OsString};
use std::io;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use super::{CONFIG, Floors, gate, matches, parse_lcov};
use crate::test_support::{Broken, BrokenAfter, TESTS_FAILED, manifest, run_in, write};
use crate::{Context, Error, FAILED, OK, Runner, USAGE};

const PASS: &str = include_str!("../tests/fixtures/cov/pass.info");
const CORE_BELOW: &str = include_str!("../tests/fixtures/cov/core-below.info");
const WORKSPACE_BELOW: &str = include_str!("../tests/fixtures/cov/workspace-below.info");

const FLOORS: &str = "\
workspace = 95
workspace-exclude = [\"apps/*/src/main.rs\"]
exclude = [\"xtask/src/main.rs\"]

[crates]
inboxcraft-time = 98
inboxcraft-ui-egui = 80
";

/// Plays `cargo llvm-cov`: records the arguments, writes `lcov` to the `--output-path` and exits with `passes`.
struct FakeLlvmCov {
    calls: Rc<RefCell<Vec<String>>>,
    lcov: Option<&'static str>,
    passes: bool,
}

impl Runner for FakeLlvmCov {
    fn run(&mut self, program: &OsStr, args: &[&str], dir: &Path) -> Result<bool, Error> {
        self.run_with_env(program, args, &[], dir)
    }

    fn run_with_env(&mut self, _: &OsStr, args: &[&str], env: &[(&str, &str)], _: &Path) -> Result<bool, Error> {
        let vars = env.iter().map(|(key, value)| format!("{key}={value} "));
        self.calls.borrow_mut().push(vars.collect::<String>() + &args.join(" "));
        let output = args.iter().position(|arg| *arg == "--output-path").and_then(|at| args.get(at + 1));
        if let (Some(path), Some(lcov)) = (output, self.lcov) {
            std::fs::write(path, lcov).unwrap();
        }
        Ok(self.passes)
    }
}

/// A workspace with a core crate, a UI crate, a crate without a floor, an app and xtask.
fn workspace(floors: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "Cargo.toml", "[workspace]\nmembers = [\"crates/*\", \"apps/*\", \"xtask\"]\n");
    for (rel, name) in [
        ("crates/html", "inboxcraft-html"),
        ("crates/time", "inboxcraft-time"),
        ("crates/ui-egui", "inboxcraft-ui-egui"),
        ("apps/inboxcraft", "inboxcraft"),
        ("xtask", "xtask"),
    ] {
        write(dir.path(), &format!("{rel}/Cargo.toml"), &manifest(name, ""));
    }
    write(dir.path(), CONFIG, floors);
    dir
}

fn cov_context(root: &Path, lcov: Option<&'static str>, passes: bool) -> (Context, Rc<RefCell<Vec<String>>>) {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let runner = FakeLlvmCov { calls: Rc::clone(&calls), lcov, passes };
    let ctx = Context { root: root.to_path_buf(), cargo: OsString::from("cargo"), runner: Box::new(runner), lcov: None };
    (ctx, calls)
}

fn cov(lcov: &'static str, args: &[&str]) -> (u8, String, String) {
    let dir = workspace(FLOORS);
    let (ctx, _) = cov_context(dir.path(), Some(lcov), true);
    run_in(ctx, args)
}

#[test]
fn cov_runs_llvm_cov_nextest_into_the_lcov_file() {
    let dir = workspace(FLOORS);
    let (ctx, calls) = cov_context(dir.path(), Some(PASS), true);
    let (code, out, err) = run_in(ctx, &["cov", "--check"]);
    assert_eq!(code, OK, "{out}{err}");
    let lcov = dir.path().join("lcov.info");
    assert_eq!(
        *calls.borrow(),
        [format!("CARGO_INCREMENTAL=1 llvm-cov nextest --workspace --profile ci --lcov --output-path {}", lcov.display())],
        "an incremental build, whatever the caller's environment: see `INCREMENTAL`"
    );
    assert_eq!(std::fs::read_to_string(lcov).unwrap(), PASS, "the report stays for Codecov");
}

#[test]
fn every_floor_met_passes_with_a_table_per_crate() {
    let (code, out, _) = cov(PASS, &["cov", "--check"]);
    assert_eq!(code, OK, "{out}");
    let table = out.lines().filter(|line| !line.starts_with("cov: running")).collect::<Vec<_>>().join("\n");
    assert_eq!(
        table,
        "\
cov: crate               lines  covered     cover  floor
cov: inboxcraft-html        20       20  100.00 %      -
cov: inboxcraft-time        50       50  100.00 %   98 %
cov: inboxcraft-ui-egui     10        8   80.00 %   80 %
cov: inboxcraft              4        0    0.00 %      -
cov: xtask                  20       19   95.00 %      -
cov: workspace             100       97   97.00 %   95 %
cov: every floor met"
    );
}

#[test]
fn a_core_crate_below_its_floor_fails() {
    let (code, out, _) = cov(CORE_BELOW, &["cov", "--check"]);
    assert_eq!(code, FAILED, "{out}");
    assert!(out.contains("cov: inboxcraft-time        50       48   96.00 %   98 %  below\n"), "{out}");
    assert!(out.contains("cov: workspace             100       95   95.00 %   95 %\n"), "on the floor is enough: {out}");
    assert!(out.ends_with("cov: 1 below the floor: inboxcraft-time\n"), "{out}");
}

#[test]
fn the_workspace_below_its_floor_fails() {
    let (code, out, _) = cov(WORKSPACE_BELOW, &["cov", "--check"]);
    assert_eq!(code, FAILED, "{out}");
    assert!(out.contains("cov: inboxcraft-html        20       14   70.00 %      -\n"), "{out}");
    assert!(out.contains("cov: workspace             100       91   91.00 %   95 %  below\n"), "{out}");
    assert!(out.ends_with("cov: 1 below the floor: workspace\n"), "{out}");
}

#[test]
fn without_check_cov_only_reports() {
    let (code, out, _) = cov(CORE_BELOW, &["cov"]);
    assert_eq!(code, OK, "{out}");
    assert!(out.ends_with("cov: 1 below the floor: inboxcraft-time (not checked without --check)\n"), "{out}");
}

#[test]
fn a_failed_test_run_fails_the_gate() {
    let dir = workspace(FLOORS);
    let (ctx, _) = cov_context(dir.path(), None, false);
    let (code, out, _) = run_in(ctx, &["cov", "--check"]);
    assert_eq!(code, FAILED);
    assert!(out.ends_with(&format!("{TESTS_FAILED}\n")), "{out}");
    assert!(!out.contains("below the floor"), "{out}");
}

#[test]
fn a_stale_report_is_removed_before_the_run() {
    let dir = workspace(FLOORS);
    write(dir.path(), "lcov.info", PASS);
    let (ctx, _) = cov_context(dir.path(), None, true);
    let (code, _, err) = run_in(ctx, &["cov", "--check"]);
    assert_eq!(code, USAGE, "a run that writes no report is a setup error, not the old numbers");
    assert!(err.contains("lcov.info"), "{err}");
}

#[test]
fn the_report_path_can_be_set() {
    let dir = workspace(FLOORS);
    let elsewhere = tempfile::tempdir().unwrap();
    let (mut ctx, calls) = cov_context(dir.path(), Some(PASS), true);
    ctx.lcov = Some(elsewhere.path().join("x.info"));
    assert!(gate(&mut ctx, true, &mut Vec::new()).unwrap());
    assert!(calls.borrow()[0].ends_with(&format!("--output-path {}", elsewhere.path().join("x.info").display())));
    assert!(!dir.path().join("lcov.info").exists());
}

#[test]
fn check_only_goes_with_cov() {
    let (code, _, err) = run_in(crate::test_support::context(PathBuf::from(".")), &["stats", "--check"]);
    assert_eq!(code, USAGE);
    assert!(err.contains("`--check` only goes with `cov`"), "{err}");
}

#[test]
fn absolute_paths_in_the_report_are_made_relative_to_the_root() {
    let dir = workspace(FLOORS);
    let report = parse_lcov(
        &format!("SF:{}\nDA:1,1\nDA:2,0\nend_of_record\n", dir.path().join("crates/time/src/lib.rs").display()),
        Path::new("lcov.info"),
        dir.path(),
    )
    .unwrap();
    assert_eq!(report.get(Path::new("crates/time/src/lib.rs")).map(|lines| lines.len()), Some(2));
}

#[test]
fn a_file_reported_twice_counts_each_line_once() {
    let text = "SF:a.rs\nDA:1,0\nDA:2,0\nend_of_record\nSF:a.rs\nDA:1,4\nend_of_record\n";
    let report = parse_lcov(text, Path::new("lcov.info"), Path::new("/w")).unwrap();
    let lines = report.get(Path::new("a.rs")).unwrap();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines.values().filter(|hit| **hit).count(), 1);
}

#[test]
fn a_broken_report_is_a_config_error_naming_the_line() {
    for (text, message) in [
        ("DA:1,1\n", "line 1: `DA` outside a `SF` record"),
        ("SF:a.rs\nDA:x,1\n", "line 2: bad `DA` record"),
        ("SF:a.rs\nDA:1\n", "line 2: bad `DA` record"),
        ("SF:a.rs\nDA:1,-1\n", "line 2: bad `DA` record"),
        ("SF:\n", "line 1: `SF` without a path"),
    ] {
        match parse_lcov(text, Path::new("lcov.info"), Path::new("/w")) {
            Err(Error::Config { message: got, .. }) => assert_eq!(got, message, "{text:?}"),
            other => panic!("{text:?}: expected a config error, got {other:?}"),
        }
    }
}

#[test]
fn a_report_with_no_workspace_file_is_a_config_error() {
    let dir = workspace(FLOORS);
    let (ctx, _) = cov_context(dir.path(), Some("SF:/elsewhere/a.rs\nDA:1,1\nend_of_record\n"), true);
    let (code, _, err) = run_in(ctx, &["cov"]);
    assert_eq!(code, USAGE);
    assert!(err.contains("no file of the report is in a workspace crate"), "{err}");
}

#[test]
fn an_empty_report_passes_with_nothing_to_cover() {
    let (code, out, _) = cov("", &["cov", "--check"]);
    assert_eq!(code, OK, "{out}");
    assert!(out.contains("cov: inboxcraft-time         0        0         -   98 %\n"), "{out}");
    assert!(out.contains("cov: workspace               0        0         -   95 %\n"), "{out}");
}

fn floors_error(floors: &str) -> String {
    let dir = workspace(floors);
    match Floors::read(dir.path(), &crate::workspace::members(dir.path()).unwrap()) {
        Err(Error::Config { message, .. }) => message,
        other => panic!("{floors:?}: expected a config error, got {other:?}"),
    }
}

#[test]
fn a_broken_floors_file_is_a_config_error() {
    for (floors, message) in [
        ("workspace = \n", "not valid TOML"),
        ("", "`workspace` must be a number from 0 to 100"),
        ("workspace = \"95\"\n", "`workspace` must be a number from 0 to 100"),
        ("workspace = 101\n", "`workspace` must be a number from 0 to 100"),
        ("workspace = -1.5\n", "`workspace` must be a number from 0 to 100"),
        ("workspace = nan\n", "`workspace` must be a number from 0 to 100"),
        ("workspace = 95\n[crates]\ninboxcraft-time = 200\n", "`crates.inboxcraft-time` must be a number from 0 to 100"),
        ("workspace = 95\n[crates]\ninboxcraft-nope = 98\n", "`crates.inboxcraft-nope` is not a workspace crate"),
        ("workspace = 95\ncrates = 1\n", "`crates` must be a table"),
        ("workspace = 95\nexclude = \"x\"\n", "`exclude` must be a list of strings"),
        ("workspace = 95\nexclude = [1]\n", "`exclude` must be a list of strings"),
        ("workspace = 95\nworkspace-exclude = [\"src/*.rs\"]\n", "unsupported pattern `src/*.rs`"),
        ("workspace = 95\nexclude = [\"**/main.rs\"]\n", "unsupported pattern `**/main.rs`"),
        ("workspace = 95\nexclude = [\"../x.rs\"]\n", "unsupported pattern `../x.rs`"),
        ("workspace = 95\nexclude = [\"\"]\n", "unsupported pattern ``"),
        ("workspace = 95\nfloor = 3\n", "unknown key `floor`"),
    ] {
        let got = floors_error(floors);
        assert!(got.starts_with(message), "{floors:?}: {got}");
    }
}

#[test]
fn a_missing_floors_file_is_an_io_error() {
    let dir = workspace(FLOORS);
    std::fs::remove_file(dir.path().join(CONFIG)).unwrap();
    assert!(matches!(Floors::read(dir.path(), &[]), Err(Error::Io { .. })));
}

#[test]
fn patterns_match_whole_components() {
    for (pattern, path, expected) in [
        ("xtask/src/main.rs", "xtask/src/main.rs", true),
        ("xtask/src/main.rs", "xtask/src/lib.rs", false),
        ("apps/*/src/main.rs", "apps/inboxcraft/src/main.rs", true),
        ("apps/*/src/main.rs", "apps/a/b/src/main.rs", false),
        ("apps/*/src/main.rs", "apps/src/main.rs", false),
        ("crates/ui-egui/src/draw/**", "crates/ui-egui/src/draw/a/b.rs", true),
        ("crates/ui-egui/src/draw/**", "crates/ui-egui/src/draw/b.rs", true),
        ("crates/ui-egui/src/draw/**", "crates/ui-egui/src/draw", false),
        ("crates/ui-egui/src/draw/**", "crates/ui-egui/src/lib.rs", false),
    ] {
        assert_eq!(matches(pattern, Path::new(path)), expected, "{pattern} {path}");
    }
}

#[test]
fn a_broken_output_stops_the_gate() {
    let dir = workspace(FLOORS);
    let (mut ctx, _) = cov_context(dir.path(), Some(PASS), true);
    assert!(matches!(gate(&mut ctx, true, &mut Broken), Err(Error::Output(_))));
    for left in [60, 120, 400] {
        let (mut ctx, _) = cov_context(dir.path(), Some(PASS), true);
        assert!(matches!(gate(&mut ctx, true, &mut BrokenAfter::new(left)), Err(Error::Output(_))), "{left}");
    }
    let (mut ctx, _) = cov_context(dir.path(), None, false);
    let running = "cov: running cargo llvm-cov nextest (profile ci)\n".len();
    assert!(matches!(gate(&mut ctx, true, &mut BrokenAfter::new(running)), Err(Error::Output(_))));
}

#[test]
fn an_unreadable_root_is_an_io_error() {
    let dir = tempfile::tempdir().unwrap();
    let (mut ctx, _) = cov_context(dir.path(), Some(PASS), true);
    assert!(matches!(gate(&mut ctx, true, &mut Vec::new()), Err(Error::Io { .. })));
}

#[test]
fn a_report_path_that_is_not_utf8_is_a_config_error() {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        let dir = workspace(FLOORS);
        let (mut ctx, calls) = cov_context(dir.path(), Some(PASS), true);
        ctx.lcov = Some(dir.path().join(OsString::from_vec(vec![b'x', 0xff])));
        assert!(matches!(gate(&mut ctx, true, &mut Vec::new()), Err(Error::Config { .. })));
        assert!(calls.borrow().is_empty());
    }
}

#[test]
fn an_unreadable_report_is_an_io_error() {
    let dir = workspace(FLOORS);
    let (mut ctx, _) = cov_context(dir.path(), None, true);
    ctx.lcov = Some(dir.path().join("dir.info"));
    std::fs::create_dir(dir.path().join("dir.info")).unwrap();
    assert!(matches!(gate(&mut ctx, true, &mut Vec::new()), Err(Error::Io { .. })));
}

#[test]
fn this_workspace_names_every_core_crate_in_its_floors() {
    let root = crate::default_root();
    let floors = Floors::read(&root, &crate::workspace::members(&root).unwrap()).unwrap();
    assert_eq!(floors.workspace, 95.0);
    for name in ["model", "mime", "charset", "time", "ical", "vcard", "proto", "dav", "store", "engine", "testkit"] {
        assert_eq!(floors.crates.get(&format!("inboxcraft-{name}")), Some(&98.0), "{name}");
    }
    for name in ["ui-egui", "net"] {
        assert_eq!(floors.crates.get(&format!("inboxcraft-{name}")), Some(&80.0), "{name}");
    }
    assert_eq!(floors.crates.len(), 13);
    assert_eq!(floors.exclude, ["xtask/src/main.rs"]);
    assert_eq!(floors.workspace_exclude, ["apps/*/src/main.rs", "crates/ui-egui/src/draw/**"]);
}

#[test]
fn the_io_error_of_a_spawn_reaches_the_caller() {
    struct Gone;
    impl Runner for Gone {
        fn run(&mut self, program: &OsStr, _: &[&str], _: &Path) -> Result<bool, Error> {
            Err(Error::Spawn { program: program.to_string_lossy().into_owned(), source: io::Error::other("gone") })
        }
    }
    let dir = workspace(FLOORS);
    let mut ctx = Context { root: dir.path().to_path_buf(), cargo: OsString::from("cargo"), runner: Box::new(Gone), lcov: None };
    assert!(matches!(gate(&mut ctx, true, &mut Vec::new()), Err(Error::Spawn { .. })));
}
