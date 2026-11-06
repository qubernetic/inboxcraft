// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::ffi::{OsStr, OsString};
use std::io;
use std::path::{Path, PathBuf};

use super::{is_ours, names, prune};
use crate::test_support::{Broken, run_in, write};
use crate::{Context, Error, FAILED, OK, Runner, USAGE};

const HASH: &str = "0123456789abcdef";

/// What `cargo metadata` answers: the JSON, a failed call (`None`) or a program that cannot start.
enum Metadata {
    Json(String),
    Fails,
    Gone,
}

impl Runner for Metadata {
    fn run(&mut self, _: &OsStr, _: &[&str], _: &Path) -> Result<bool, Error> {
        Ok(true)
    }

    fn output(&mut self, program: &OsStr, args: &[&str], _: &Path) -> Result<Option<Vec<u8>>, Error> {
        assert_eq!(args, ["metadata", "--no-deps", "--format-version", "1"]);
        match self {
            Self::Json(json) => Ok(Some(json.clone().into_bytes())),
            Self::Fails => Ok(None),
            Self::Gone => Err(Error::Spawn { program: program.to_string_lossy().into_owned(), source: io::Error::other("gone") }),
        }
    }
}

/// `cargo metadata` JSON for `packages`: each a name and its targets (name, kind).
fn metadata(packages: &[(&str, &[(&str, &str)])]) -> String {
    let packages: Vec<String> = packages
        .iter()
        .map(|(name, targets)| {
            let targets: Vec<String> = targets.iter().map(|(target, kind)| format!(r#"{{"name": "{target}", "kind": ["{kind}"]}}"#)).collect();
            format!(r#"{{"name": "{name}", "version": "0.1.0", "targets": [{}]}}"#, targets.join(", "))
        })
        .collect();
    format!(r#"{{"packages": [{}], "workspace_root": "/w", "version": 1}}"#, packages.join(", "))
}

/// The workspace of the fixture: `xtask` with a test target called `wasm`, a library with a build script, and two
/// plain libraries.
fn workspace() -> String {
    metadata(&[
        ("xtask", &[("xtask", "lib"), ("xtask", "bin"), ("wasm", "test")]),
        ("inboxcraft-web", &[("inboxcraft_web", "lib"), ("build-script-build", "custom-build")]),
        ("inboxcraft-model", &[("inboxcraft_model", "lib")]),
        ("inboxcraft-time", &[("inboxcraft_time", "lib"), ("ui_shot", "example")]),
    ])
}

/// Every file of the fixture `target/`, and whether the prune keeps it.
fn tree() -> Vec<(String, bool)> {
    let h = HASH;
    let mut files = vec![
        // Cargo's marker files.
        ("CACHEDIR.TAG".to_owned(), true),
        (".rustc_info.json".to_owned(), true),
        ("debug/.cargo-lock".to_owned(), true),
        ("debug/.cargo-build-lock".to_owned(), true),
        ("wasm32-unknown-unknown/debug/.cargo-artifact-lock".to_owned(), true),
        ("wasm32-unknown-unknown/CACHEDIR.TAG".to_owned(), true),
        // Third-party artifacts.
        (format!("debug/deps/libserde-{h}.rlib"), true),
        (format!("debug/deps/serde-{h}.d"), true),
        (format!("debug/deps/libwasm_bindgen-{h}.rlib"), true),
        (format!("debug/deps/wasm_bindgen-{h}.d"), true),
        (format!("debug/.fingerprint/wasm-bindgen-{h}/lib-wasm_bindgen"), true),
        (format!("debug/build/wasm-bindgen-{h}/out/x.rs"), true),
        (format!("debug/build/serde-{h}/build_script_build-{h}"), true),
        (format!("wasm32-unknown-unknown/debug/deps/libserde-{h}.rlib"), true),
        (format!("llvm-cov-target/debug/deps/libserde-{h}.rlib"), true),
        (format!("llvm-cov-target/debug/.fingerprint/serde-{h}/lib-serde"), true),
        // Ours, inside the kept folders.
        (format!("debug/deps/libinboxcraft_model-{h}.rlib"), false),
        (format!("debug/deps/inboxcraft_model-{h}.d"), false),
        (format!("debug/deps/xtask-{h}"), false),
        (format!("debug/deps/wasm-{h}"), false),
        (format!("debug/deps/ui_shot-{h}"), false),
        (format!("debug/.fingerprint/inboxcraft-model-{h}/lib-inboxcraft_model"), false),
        (format!("debug/.fingerprint/wasm-{h}/test-integration-test-wasm"), false),
        (format!("debug/build/inboxcraft-web-{h}/out/generated.rs"), false),
        (format!("debug/build/inboxcraft-web-{h}/build_script_build-{h}"), false),
        (format!("wasm32-unknown-unknown/debug/deps/libinboxcraft_time-{h}.rlib"), false),
        (format!("llvm-cov-target/debug/deps/inboxcraft_model-{h}"), false),
        // Everything else under target/.
        ("debug/inboxcraft".to_owned(), false),
        ("debug/inboxcraft.d".to_owned(), false),
        ("debug/examples/ui_shot".to_owned(), false),
        ("debug/incremental/x/y".to_owned(), false),
        ("llvm-cov-target/inboxcraft-1.profraw".to_owned(), false),
        ("llvm-cov-target/tmp/scratch".to_owned(), false),
        ("nextest/ci/junit.xml".to_owned(), false),
        ("tmp/scratch".to_owned(), false),
        ("doc/index.html".to_owned(), false),
        ("flatpak/repo/config".to_owned(), false),
        ("stray.txt".to_owned(), false),
    ];
    files.sort();
    files
}

/// A workspace root with the fixture `target/`.
fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for (file, _) in tree() {
        write(dir.path(), &format!("target/{file}"), "x");
    }
    dir
}

/// Every file under `dir`, relative to it, sorted.
fn files(dir: &Path) -> Vec<String> {
    fn visit(base: &Path, dir: &Path, found: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() && !path.is_symlink() {
                visit(base, &path, found);
            } else {
                found.push(path.strip_prefix(base).unwrap().to_string_lossy().into_owned());
            }
        }
    }
    let mut found = Vec::new();
    if dir.exists() {
        visit(dir, dir, &mut found);
    }
    found.sort();
    found
}

fn prune_in(root: &Path, metadata: Metadata) -> (u8, String, String) {
    let ctx = Context { root: PathBuf::from(root), cargo: OsString::from("cargo"), runner: Box::new(metadata), lcov: None };
    run_in(ctx, &["cache-prune"])
}

#[test]
fn the_prune_keeps_only_third_party_artifacts_and_cargo_markers() {
    let dir = fixture();
    let (code, out, err) = prune_in(dir.path(), Metadata::Json(workspace()));
    assert_eq!(code, OK, "{out}{err}");
    let kept: Vec<String> = tree().into_iter().filter(|(_, keep)| *keep).map(|(file, _)| file).collect();
    assert_eq!(files(&dir.path().join("target")), kept);
}

#[test]
fn the_prune_lists_what_it_kept_and_passes_its_check() {
    let dir = fixture();
    let (_, out, _) = prune_in(dir.path(), Metadata::Json(workspace()));
    for line in [
        "cache-prune: 10 workspace names",
        "cache-prune: kept target/debug: deps 4, build 2, .fingerprint 1",
        "cache-prune: kept target/llvm-cov-target/debug: deps 1, build 0, .fingerprint 1",
        "cache-prune: kept target/wasm32-unknown-unknown/debug: deps 1, build 0, .fingerprint 0",
        "cache-prune: removed target/flatpak",
        "cache-prune: removed target/llvm-cov-target/tmp",
        "cache-prune: removed target/debug/incremental",
        "cache-prune: removed 10 entries named after the workspace",
        "cache-prune: no path under target/ is named after a workspace package or target",
    ] {
        assert!(out.contains(&format!("{line}\n")), "{line}: {out}");
    }
}

#[test]
fn a_failing_metadata_call_deletes_nothing() {
    let dir = fixture();
    let before = files(&dir.path().join("target"));
    let (code, out, _) = prune_in(dir.path(), Metadata::Fails);
    assert_eq!(code, FAILED);
    assert!(out.contains("cache-prune: `cargo metadata` failed; nothing was deleted"), "{out}");
    assert_eq!(files(&dir.path().join("target")), before);
    let (code, _, err) = prune_in(dir.path(), Metadata::Gone);
    assert_eq!(code, USAGE);
    assert!(err.contains("cannot run `cargo`"), "{err}");
    assert_eq!(files(&dir.path().join("target")), before);
}

#[test]
fn a_name_list_without_xtask_or_bad_metadata_deletes_nothing() {
    let dir = fixture();
    let before = files(&dir.path().join("target"));
    for json in [
        metadata(&[]),
        metadata(&[("inboxcraft-model", &[("inboxcraft_model", "lib")])]),
        "not json".to_owned(),
        r#"{"packages": 3}"#.to_owned(),
        r#"{"packages": [{"targets": []}]}"#.to_owned(),
        r#"{"packages": [{"name": "xtask", "targets": [{"kind": ["lib"]}]}]}"#.to_owned(),
        metadata(&[("xtask", &[("../../etc", "bin")])]),
        metadata(&[("xtask", &[("", "bin")])]),
    ] {
        let (code, out, _) = prune_in(dir.path(), Metadata::Json(json.clone()));
        assert_eq!(code, FAILED, "{json}");
        assert!(out.contains("nothing was deleted"), "{json}: {out}");
        assert_eq!(files(&dir.path().join("target")), before, "{json}");
    }
}

#[test]
fn the_check_fails_on_a_workspace_name_at_any_depth() {
    let dir = fixture();
    write(dir.path(), &format!("target/debug/build/serde-{HASH}/out/nested/libxtask-{HASH}.rlib"), "x");
    let (code, out, _) = prune_in(dir.path(), Metadata::Json(workspace()));
    assert_eq!(code, FAILED, "{out}");
    assert!(
        out.contains(&format!("cache-prune: still named after the workspace: target/debug/build/serde-{HASH}/out/nested/libxtask-{HASH}.rlib")),
        "{out}"
    );
}

#[test]
fn a_tree_too_deep_for_the_check_is_an_error() {
    let dir = fixture();
    write(dir.path(), &format!("target/debug/build/serde-{HASH}/out/{}x", "d/".repeat(70)), "x");
    let (code, _, err) = prune_in(dir.path(), Metadata::Json(workspace()));
    assert_eq!(code, USAGE);
    assert!(err.contains("folders deep"), "{err}");
}

#[cfg(unix)]
#[test]
fn links_are_removed_and_never_followed() {
    let dir = fixture();
    let outside = tempfile::tempdir().unwrap();
    write(outside.path(), "keep/me", "x");
    std::os::unix::fs::symlink(outside.path().join("keep"), dir.path().join("target/linked")).unwrap();
    std::os::unix::fs::symlink(outside.path().join("keep"), dir.path().join(format!("target/debug/deps/xtask-{HASH}.link"))).unwrap();
    let (code, out, _) = prune_in(dir.path(), Metadata::Json(workspace()));
    assert_eq!(code, OK, "{out}");
    assert!(!dir.path().join("target/linked").exists() && !dir.path().join("target/linked").is_symlink());
    assert_eq!(files(outside.path()), ["keep/me"]);
}

#[test]
fn a_missing_target_folder_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let (code, _, err) = prune_in(dir.path(), Metadata::Json(workspace()));
    assert_eq!(code, USAGE);
    assert!(err.contains("target"), "{err}");
}

#[test]
fn a_closed_output_stops_the_prune() {
    let dir = fixture();
    let mut ctx =
        Context { root: PathBuf::from(dir.path()), cargo: OsString::from("cargo"), runner: Box::new(Metadata::Json(workspace())), lcov: None };
    assert!(matches!(prune(&mut ctx, &mut Broken), Err(Error::Output(_))));
}

#[test]
fn names_cover_hyphen_and_underscore_forms_without_build_scripts() {
    let found = names(workspace().as_bytes()).unwrap();
    for name in ["xtask", "wasm", "inboxcraft-web", "inboxcraft_web", "inboxcraft-model", "inboxcraft_model", "ui_shot", "ui-shot"] {
        assert!(found.contains(name), "{name}: {found:?}");
    }
    assert!(!found.iter().any(|name| name.contains("build")), "{found:?}");
}

#[test]
fn only_exact_names_with_a_hash_are_ours() {
    let found = names(workspace().as_bytes()).unwrap();
    for ours in ["xtask-0123456789abcdef", "libxtask-0123456789abcdef.rlib", "wasm-0123456789abcdef", "inboxcraft_model-0123456789abcdef.d"] {
        assert!(is_ours(ours, &found), "{ours}");
    }
    for theirs in [
        "wasm_bindgen-0123456789abcdef",
        "wasm-bindgen-0123456789abcdef",
        "libwasm_bindgen-0123456789abcdef.rlib",
        "xtask",
        "xtask-0123",
        "xtask-0123456789ABCDEF",
        "xtask-0123456789abcdefg",
        "libxtask.rlib",
        "xtask-é123456789abcde",
    ] {
        assert!(!is_ours(theirs, &found), "{theirs}");
    }
}
