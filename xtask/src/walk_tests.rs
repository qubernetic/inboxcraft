// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fs;
use std::path::{Path, PathBuf};

use super::{MAX_DEPTH, rust_files};
use crate::Error;

fn touch(root: &Path, rel: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, "").unwrap();
}

#[test]
fn finds_rust_files_sorted_and_relative() {
    let dir = tempfile::tempdir().unwrap();
    for rel in ["b/src/lib.rs", "a/main.rs", "a/README.md", "build.rs"] {
        touch(dir.path(), rel);
    }
    let files = rust_files(dir.path()).unwrap();
    assert_eq!(files, [PathBuf::from("a/main.rs"), PathBuf::from("b/src/lib.rs"), PathBuf::from("build.rs")]);
}

#[test]
fn skips_build_output_hidden_and_private_directories() {
    let dir = tempfile::tempdir().unwrap();
    for rel in ["target/debug/x.rs", "target-wasm/x.rs", ".git/x.rs", "plan/x.rs", "crates/target/x.rs", "keep.rs"] {
        touch(dir.path(), rel);
    }
    assert_eq!(rust_files(dir.path()).unwrap(), [PathBuf::from("keep.rs")]);
}

#[test]
fn does_not_follow_symbolic_links() {
    #[cfg(unix)]
    {
        let dir = tempfile::tempdir().unwrap();
        touch(dir.path(), "real/a.rs");
        std::os::unix::fs::symlink(dir.path().join("real"), dir.path().join("link")).unwrap();
        std::os::unix::fs::symlink(dir.path().join("real/a.rs"), dir.path().join("b.rs")).unwrap();
        assert_eq!(rust_files(dir.path()).unwrap(), [PathBuf::from("real/a.rs")]);
    }
}

#[test]
fn a_missing_root_is_an_io_error() {
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(rust_files(&dir.path().join("absent")), Err(Error::Io { .. })));
}

#[test]
fn a_tree_deeper_than_the_limit_is_an_error_not_a_stack_overflow() {
    let dir = tempfile::tempdir().unwrap();
    let mut deep = dir.path().to_path_buf();
    for _ in 0..=MAX_DEPTH {
        deep.push("d");
    }
    fs::create_dir_all(&deep).unwrap();
    match rust_files(dir.path()) {
        Err(Error::Io { source, .. }) => assert!(source.to_string().contains("deeper than"), "{source}"),
        other => panic!("expected an io error, got {other:?}"),
    }
}
