// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::cell::RefCell;
use std::ffi::{OsStr, OsString};
use std::io;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use super::{NATIVE_ONLY, TARGET, gate};
use crate::test_support::{Broken, BrokenAfter, manifest, run_in, write};
use crate::{Context, Error, FAILED, OK, Runner};

/// Records the arguments of every call and answers with `result`.
struct Recorder {
    calls: Rc<RefCell<Vec<String>>>,
    result: Result<bool, ()>,
}

impl Runner for Recorder {
    fn run(&mut self, _: &OsStr, args: &[&str], _: &Path) -> Result<bool, Error> {
        self.calls.borrow_mut().push(args.join(" "));
        self.result.map_err(|()| Error::Spawn { program: "cargo".to_owned(), source: io::Error::other("gone") })
    }
}

fn context(root: PathBuf, result: Result<bool, ()>) -> (Context, Rc<RefCell<Vec<String>>>) {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let runner = Recorder { calls: Rc::clone(&calls), result };
    (Context { root, cargo: OsString::from("cargo"), runner: Box::new(runner) }, calls)
}

fn fixture(names: &[&str]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n");
    for name in names {
        write(dir.path(), &format!("crates/{name}/Cargo.toml"), &manifest(name, ""));
    }
    dir
}

#[test]
fn builds_the_standalone_and_layer_0_to_5_crates_but_net() {
    let dir = fixture(&["inboxcraft-charset", "inboxcraft-engine", "inboxcraft-net", "inboxcraft-ui-egui", "inboxcraft", "xtask"]);
    let (ctx, calls) = context(dir.path().to_path_buf(), Ok(true));
    let (code, out, _) = run_in(ctx, &["wasm"]);
    assert_eq!(code, OK, "{out}");
    assert_eq!(*calls.borrow(), [format!("build --target {TARGET} -p inboxcraft-charset -p inboxcraft-engine")]);
    assert_eq!(out, format!("wasm: building 2 crates for {TARGET}\nwasm: 2 crates build for {TARGET}\n"));
    assert_eq!(NATIVE_ONLY, ["inboxcraft-net"]);
}

#[test]
fn a_crate_that_does_not_build_for_wasm_fails() {
    let dir = fixture(&["inboxcraft-time"]);
    let (ctx, _) = context(dir.path().to_path_buf(), Ok(false));
    let (code, out, _) = run_in(ctx, &["wasm"]);
    assert_eq!(code, FAILED);
    assert!(out.ends_with(&format!("wasm: the build for {TARGET} failed (1 crate); see the cargo output above\n")), "{out}");
}

#[test]
fn a_workspace_without_wasm_crates_passes_without_a_build() {
    let dir = fixture(&["inboxcraft-net", "xtask"]);
    let (ctx, calls) = context(dir.path().to_path_buf(), Ok(false));
    let (code, out, _) = run_in(ctx, &["wasm"]);
    assert_eq!(code, OK);
    assert!(calls.borrow().is_empty());
    assert_eq!(out, "wasm: no standalone or layer 0 to 5 crates to build\n");
}

#[test]
fn this_workspace_builds_thirteen_crates() {
    let (mut ctx, calls) = context(crate::default_root(), Ok(true));
    assert!(gate(&mut ctx, &mut Vec::new()).unwrap());
    let call = calls.borrow().first().cloned().unwrap();
    assert_eq!(call.matches(" -p ").count(), 13, "{call}");
    assert!(!call.contains("inboxcraft-net") && !call.contains("ui-egui"), "{call}");
}

#[test]
fn a_cargo_that_cannot_start_is_an_error() {
    let dir = fixture(&["inboxcraft-time"]);
    let (mut ctx, _) = context(dir.path().to_path_buf(), Err(()));
    assert!(matches!(gate(&mut ctx, &mut Vec::new()), Err(Error::Spawn { .. })));
}

#[test]
fn a_workspace_that_cannot_be_read_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let (mut ctx, _) = context(dir.path().to_path_buf(), Ok(true));
    assert!(matches!(gate(&mut ctx, &mut Vec::new()), Err(Error::Io { .. })));
}

#[test]
fn a_broken_report_stops_the_gate() {
    let dir = fixture(&["inboxcraft-time"]);
    let (mut ctx, calls) = context(dir.path().to_path_buf(), Ok(true));
    assert!(matches!(gate(&mut ctx, &mut Broken), Err(Error::Output(_))));
    let first = format!("wasm: building 1 crate for {TARGET}\n");
    assert!(matches!(gate(&mut ctx, &mut BrokenAfter::new(first.len())), Err(Error::Output(_))));
    assert_eq!(calls.borrow().len(), 1, "the build ran once, before the second report line");
    let dir = fixture(&[]);
    let (mut ctx, _) = context(dir.path().to_path_buf(), Ok(true));
    std::fs::create_dir_all(dir.path().join("crates")).unwrap();
    assert!(matches!(gate(&mut ctx, &mut Broken), Err(Error::Output(_))));
}
