// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::cell::RefCell;
use std::ffi::{OsStr, OsString};
use std::io;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use super::{GATES, ProcessRunner, Runner};
use crate::test_support::{Broken, run_in};
use crate::{Context, Error, FAILED, OK};

/// Records every call and fails the calls whose first argument is in `failing`; `"spawn"` in `failing` makes the
/// call a spawn error instead.
#[derive(Default)]
struct Recorder {
    calls: Rc<RefCell<Vec<String>>>,
    failing: Vec<&'static str>,
}

impl Runner for Recorder {
    fn run(&mut self, program: &OsStr, args: &[&str], dir: &Path) -> Result<bool, Error> {
        self.calls.borrow_mut().push(format!("{} {} @ {}", program.to_string_lossy(), args.join(" "), dir.display()));
        let first = args.first().copied().unwrap_or_default();
        if self.failing.contains(&"spawn") {
            return Err(Error::Spawn { program: program.to_string_lossy().into_owned(), source: io::Error::other("gone") });
        }
        Ok(!self.failing.contains(&first))
    }
}

fn ci_context(failing: Vec<&'static str>) -> (Context, Rc<RefCell<Vec<String>>>) {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let runner = Recorder { calls: Rc::clone(&calls), failing };
    (Context { root: crate::default_root(), cargo: OsString::from("cargo"), runner: Box::new(runner) }, calls)
}

#[test]
fn ci_runs_every_gate_in_order() {
    let (ctx, calls) = ci_context(vec![]);
    let (code, out, _) = run_in(ctx, &["ci"]);
    assert_eq!(code, OK, "{out}");
    let root = crate::default_root();
    let wasm = ["charset", "control-key", "dav", "engine", "html", "ical", "mcp", "mime", "model", "proto", "store", "time", "vcard"]
        .map(|name| format!(" -p inboxcraft-{name}"))
        .concat();
    let wasm = format!("build --target wasm32-unknown-unknown{wasm}");
    let expected: Vec<String> = [
        "fmt --all -- --check",
        "clippy --workspace --all-targets -- -D warnings",
        "nextest run --workspace --profile ci",
        "test --workspace --doc",
        &wasm,
        "deny check",
    ]
    .iter()
    .map(|args| format!("cargo {args} @ {}", root.display()))
    .collect();
    assert_eq!(*calls.borrow(), expected);
    for name in ["fmt", "clippy", "spdx", "test-files", "assets", "layers", "nextest", "doctests", "wasm", "deny"] {
        assert!(out.contains(&format!("ci: {name}\n")), "{name}: {out}");
    }
    assert!(out.contains("spdx: "), "the in-process gates report: {out}");
    assert!(out.contains("layers: ") && out.contains("assets: "), "{out}");
    assert!(out.ends_with("ci: all 10 gates passed\n"), "{out}");
    assert_eq!(GATES.len(), 10);
}

#[test]
fn ci_runs_on_after_a_failure_and_names_every_failed_gate() {
    let (ctx, calls) = ci_context(vec!["clippy", "deny"]);
    let (code, out, _) = run_in(ctx, &["ci"]);
    assert_eq!(code, FAILED);
    assert_eq!(calls.borrow().len(), 6, "every gate still ran");
    assert!(out.ends_with("ci: 2 of 10 gates failed: clippy, deny\n"), "{out}");
}

#[test]
fn a_tool_that_cannot_start_fails_its_gate() {
    let (ctx, _) = ci_context(vec!["spawn"]);
    let (code, out, _) = run_in(ctx, &["ci"]);
    assert_eq!(code, FAILED);
    assert!(out.contains("ci: fmt: cannot run `cargo`: gone"), "{out}");
    assert!(out.ends_with("ci: 6 of 10 gates failed: fmt, clippy, nextest, doctests, wasm, deny\n"), "{out}");
}

#[test]
fn an_in_process_gate_that_cannot_run_fails_without_stopping_ci() {
    let dir = tempfile::tempdir().unwrap();
    let (mut ctx, calls) = ci_context(vec![]);
    ctx.root = dir.path().to_path_buf();
    let (code, out, _) = run_in(ctx, &["ci"]);
    assert_eq!(code, FAILED);
    assert!(out.contains("ci: spdx: "), "{out}");
    assert!(out.ends_with("ci: 4 of 10 gates failed: spdx, assets, layers, wasm\n"), "{out}");
    assert_eq!(calls.borrow().len(), 5);
}

#[test]
fn a_broken_report_stops_ci() {
    let (mut ctx, _) = ci_context(vec![]);
    assert!(matches!(super::gate(&mut ctx, &mut Broken), Err(Error::Output(_))));
    let (mut ctx, _) = ci_context(vec!["spawn"]);
    let mut out = crate::test_support::BrokenAfter::new(b"ci: fmt\n".len());
    assert!(matches!(super::gate(&mut ctx, &mut out), Err(Error::Output(_))));
    let (mut ctx, _) = ci_context(vec![]);
    let mut out = crate::test_support::BrokenAfter::new(b"ci: fmt\nci: clippy\nci: spdx\n".len());
    assert!(matches!(super::gate(&mut ctx, &mut out), Err(Error::Output(_))), "the spdx report fails");
}

#[test]
fn ci_takes_no_options_but_root() {
    let (ctx, calls) = ci_context(vec![]);
    let (code, _, err) = run_in(ctx, &["ci", "--fix"]);
    assert_eq!(code, crate::USAGE);
    assert!(err.contains("`--fix` only goes with `spdx`"), "{err}");
    assert!(calls.borrow().is_empty());
}

#[test]
fn the_process_runner_reports_the_exit_status() {
    let cargo = OsStr::new(env!("CARGO"));
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(ProcessRunner.run(cargo, &["--version"], here).unwrap());
    assert!(!ProcessRunner.run(cargo, &["inboxcraft-no-such-subcommand"], here).unwrap());
    let missing = PathBuf::from(here).join("inboxcraft-no-such-program");
    assert!(matches!(ProcessRunner.run(missing.as_os_str(), &[], here), Err(Error::Spawn { .. })));
}

#[test]
fn the_real_context_uses_the_cargo_that_runs_xtask() {
    let ctx = Context::from_env();
    assert_eq!(ctx.root, crate::default_root());
    assert!(!ctx.cargo.is_empty());
}
