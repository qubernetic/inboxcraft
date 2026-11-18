// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::cell::RefCell;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use super::{SCRIPT, compare, target_dir};
use crate::cli::{Command, parse};
use crate::test_support::{Broken, run_in, write};
use crate::{Context, Error, FAILED, OK, Runner};

/// The pictures of a workspace and its renders: `committed` under `root/docs/images`, `rendered` under the renders.
fn pictures(committed: &[(&str, &str)], rendered: &[(&str, &str)]) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    for (name, bytes) in committed {
        write(dir.path(), &format!("root/docs/images/{name}"), bytes);
    }
    for (name, bytes) in rendered {
        write(dir.path(), &format!("renders/docs/images/{name}"), bytes);
    }
    let renders = dir.path().join("renders");
    (dir, renders)
}

fn check(dir: &Path, renders: &Path, scenes: &[&str], fix: bool) -> (bool, String) {
    let mut out = Vec::new();
    let passed = compare(&dir.join("root"), renders, scenes, fix, &mut out).unwrap();
    (passed, String::from_utf8(out).unwrap())
}

#[test]
fn pictures_equal_to_their_renders_pass() {
    let (dir, renders) = pictures(&[("a.png", "A"), ("b.png", "B"), ("notes.txt", "x")], &[("a.png", "A"), ("b.png", "B")]);
    let (passed, out) = check(dir.path(), &renders, &["docs/images/a.png", "docs/images/b.png"], false);
    assert!(passed, "{out}");
    assert_eq!(out, "shots: 2 scenes, every committed picture is the render\n");
}

#[test]
fn one_byte_of_difference_a_missing_picture_and_a_stray_fail_by_name() {
    let (dir, renders) =
        pictures(&[("a.png", "A"), ("b.png", "B1"), ("old.png", "O"), ("old.PNG", "O")], &[("a.png", "A"), ("b.png", "B2"), ("c.png", "C")]);
    let scenes = ["docs/images/a.png", "docs/images/b.png", "docs/images/c.png"];
    let (passed, out) = check(dir.path(), &renders, &scenes, false);
    assert!(!passed);
    assert_eq!(
        out,
        "shots: docs/images/b.png differs from the render\n\
         shots: docs/images/c.png has no picture\n\
         shots: docs/images/old.PNG belongs to no scene\n\
         shots: docs/images/old.png belongs to no scene\n\
         shots: 3 scenes: 1 differ, 1 without a picture, 2 of no scene, 0 written by more than one\n"
    );
    assert_eq!(std::fs::read(dir.path().join("root/docs/images/b.png")).unwrap(), b"B1", "a check writes nothing");
}

#[test]
fn fix_writes_the_renders_and_deletes_the_strays() {
    let (dir, renders) = pictures(&[("a.png", "A1"), ("old.png", "O")], &[("a.png", "A2"), ("c.png", "C")]);
    let scenes = ["docs/images/a.png", "docs/images/c.png"];
    let (passed, out) = check(dir.path(), &renders, &scenes, true);
    assert!(passed, "{out}");
    assert!(out.contains("shots: docs/images/a.png differs from the render: written\n"), "{out}");
    assert!(out.contains("shots: docs/images/old.png belongs to no scene: deleted\n"), "{out}");
    assert!(out.ends_with("; 3 fixed\n"), "{out}");
    let images = dir.path().join("root/docs/images");
    assert_eq!(std::fs::read(images.join("a.png")).unwrap(), b"A2");
    assert_eq!(std::fs::read(images.join("c.png")).unwrap(), b"C");
    assert!(!images.join("old.png").exists());
    assert!(check(dir.path(), &renders, &scenes, false).0, "fixed, the check passes");
}

#[test]
fn two_scenes_writing_one_picture_fail_even_with_fix() {
    let (dir, renders) = pictures(&[("a.png", "A")], &[("a.png", "A")]);
    for fix in [false, true] {
        let (passed, out) = check(dir.path(), &renders, &["docs/images/a.png", "docs/images/a.png"], fix);
        assert!(!passed);
        assert!(out.starts_with("shots: docs/images/a.png: 2 scenes write it; only the last one shows\n"), "{out}");
    }
}

#[test]
fn a_scene_outside_the_workspace_is_refused_and_a_missing_render_is_an_error() {
    let (dir, renders) = pictures(&[], &[]);
    let (passed, out) = check(dir.path(), &renders, &["../x.png", "/etc/x.png"], false);
    assert!(!passed);
    assert!(out.contains("shots: ../x.png: not a path under the workspace") && out.contains("shots: /etc/x.png: not a path"), "{out}");
    let mut out = Vec::new();
    let error = compare(&dir.path().join("root"), &renders, &["docs/images/a.png"], false, &mut out).unwrap_err();
    assert!(matches!(error, Error::Io { .. }), "{error}");
}

#[test]
fn a_broken_report_is_an_error() {
    let (dir, renders) = pictures(&[("a.png", "A")], &[("a.png", "A")]);
    assert!(compare(&dir.path().join("root"), &renders, &["docs/images/a.png"], false, &mut Broken).is_err());
}

#[test]
fn the_renders_go_under_the_target_directory() {
    let root = Path::new("/w");
    assert_eq!(target_dir(root, None), PathBuf::from("/w/target"));
    assert_eq!(target_dir(root, Some(PathBuf::new())), PathBuf::from("/w/target"));
    assert_eq!(target_dir(root, Some(PathBuf::from("t/x"))), PathBuf::from("/w/t/x"));
    assert_eq!(target_dir(root, Some(PathBuf::from("/abs"))), PathBuf::from("/abs"));
}

#[test]
fn shots_parses_check_and_fix() {
    let args = |list: &[&str]| list.iter().map(OsString::from).collect::<Vec<_>>();
    assert_eq!(parse(args(&["shots"])).unwrap().command, Command::Shots { fix: false });
    assert_eq!(parse(args(&["shots", "--check"])).unwrap().command, Command::Shots { fix: false });
    assert_eq!(parse(args(&["shots", "--fix"])).unwrap().command, Command::Shots { fix: true });
    assert!(parse(args(&["shots", "--fix", "--check"])).is_err());
    assert!(parse(args(&["layers", "--check"])).is_err());
}

/// Answers `ui_shot` with `printed`, after writing `rendered` where it was told to; records the arguments.
struct Shooter {
    printed: Option<&'static str>,
    rendered: Vec<(&'static str, &'static str)>,
    calls: Rc<RefCell<Vec<String>>>,
}

impl Runner for Shooter {
    fn run(&mut self, _: &OsStr, _: &[&str], _: &Path) -> Result<bool, Error> {
        Ok(true)
    }

    fn output(&mut self, program: &OsStr, args: &[&str], _: &Path) -> Result<Option<Vec<u8>>, Error> {
        self.calls.borrow_mut().push(format!("{} {}", program.to_string_lossy(), args.join(" ")));
        let dir = args.iter().position(|arg| *arg == "--out-dir").and_then(|at| args.get(at + 1)).unwrap();
        for (name, bytes) in &self.rendered {
            write(Path::new(dir), name, bytes);
        }
        Ok(self.printed.map(|printed| printed.as_bytes().to_vec()))
    }
}

/// A context rooted at `root` whose renders go to `root/renders/shots`.
fn shooter_context(root: &Path, printed: Option<&'static str>, rendered: Vec<(&'static str, &'static str)>) -> (Context, Rc<RefCell<Vec<String>>>) {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let runner = Shooter { printed, rendered, calls: Rc::clone(&calls) };
    let shots = Some(root.join("renders/shots"));
    (Context { root: root.to_path_buf(), cargo: OsString::from("cargo"), runner: Box::new(runner), lcov: None, shots }, calls)
}

#[test]
fn shots_runs_ui_shot_as_just_shot_does_and_compares_what_it_printed() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "docs/images/a.png", "A");
    let (ctx, calls) = shooter_context(dir.path(), Some("docs/images/a.png\n"), vec![("docs/images/a.png", "A")]);
    let (code, out, _) = run_in(ctx, &["shots", "--check"]);
    assert_eq!(code, OK, "{out}");
    let call = calls.borrow()[0].clone();
    assert!(call.starts_with("cargo run -q -p inboxcraft-ui-egui --example ui_shot -- --out-dir "), "{call}");
    assert!(call.ends_with(&format!("/shots {SCRIPT}")), "{call}");
    let (ctx, _) = shooter_context(dir.path(), Some("docs/images/a.png\n"), vec![("docs/images/a.png", "B")]);
    let (code, out, _) = run_in(ctx, &["shots"]);
    assert_eq!(code, FAILED, "{out}");
    assert!(out.contains("docs/images/a.png differs from the render"), "{out}");
}

#[test]
fn shots_fails_when_ui_shot_fails() {
    let dir = tempfile::tempdir().unwrap();
    let (ctx, _) = shooter_context(dir.path(), None, vec![]);
    let (code, out, _) = run_in(ctx, &["shots"]);
    assert_eq!(code, FAILED);
    assert_eq!(out, "shots: ui_shot failed (see above); no picture was compared\n");
}
