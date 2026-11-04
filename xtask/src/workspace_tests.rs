// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fs;
use std::path::PathBuf;

use super::{Dependency, Kind, MAX_MEMBERS, members};
use crate::Error;
use crate::test_support::{manifest, write};

fn dep(name: &str, kind: Kind) -> Dependency {
    Dependency { name: name.to_owned(), kind }
}

#[test]
fn expands_the_member_globs_sorted() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "Cargo.toml", "[workspace]\nmembers = [\"crates/*\", \"xtask\"]\n");
    write(dir.path(), "crates/b/Cargo.toml", &manifest("inboxcraft-b", ""));
    write(dir.path(), "crates/a/Cargo.toml", &manifest("inboxcraft-a", ""));
    write(dir.path(), "crates/not-a-crate/README.md", "");
    write(dir.path(), "crates/file.txt", "");
    write(dir.path(), "xtask/Cargo.toml", &manifest("xtask", ""));
    let found = members(dir.path()).unwrap();
    let names: Vec<_> = found.iter().map(|m| (m.name.as_str(), m.dir.clone())).collect();
    assert_eq!(names, [("inboxcraft-a", PathBuf::from("crates/a")), ("inboxcraft-b", PathBuf::from("crates/b")), ("xtask", PathBuf::from("xtask"))]);
}

#[test]
fn honours_exclude_and_a_root_package() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "Cargo.toml", "[package]\nname = \"top\"\n\n[workspace]\nmembers = [\"crates/*\"]\nexclude = [\"crates/skip\"]\n");
    write(dir.path(), "crates/keep/Cargo.toml", &manifest("keep", ""));
    write(dir.path(), "crates/skip/Cargo.toml", &manifest("skip", ""));
    let names: Vec<_> = members(dir.path()).unwrap().into_iter().map(|m| (m.name, m.dir)).collect();
    assert_eq!(names, [("top".to_owned(), PathBuf::new()), ("keep".to_owned(), PathBuf::from("crates/keep"))]);
}

#[test]
fn reads_every_kind_of_dependency_with_renames_and_workspace_inheritance() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "Cargo.toml",
        "[workspace]\nmembers = [\"a\"]\n\n[workspace.dependencies]\nren = { package = \"real-ws\", version = \"1\" }\nplain = \"1\"\n",
    );
    write(
        dir.path(),
        "a/Cargo.toml",
        &manifest(
            "a",
            "[dependencies]\nx = \"1\"\nalias = { package = \"y\", version = \"1\" }\nren.workspace = true\n\
             plain = { workspace = true }\n\n[dev-dependencies]\nd = \"1\"\n\n[build-dependencies]\nb = \"1\"\n\n\
             [target.'cfg(unix)'.dependencies]\nt = \"1\"\n\n[target.'cfg(unix)'.dev-dependencies]\ntd = \"1\"\n",
        ),
    );
    let found = members(dir.path()).unwrap();
    let mut deps = found.first().unwrap().dependencies.clone();
    deps.sort_by(|a, b| a.name.cmp(&b.name));
    assert_eq!(
        deps,
        [
            dep("b", Kind::Build),
            dep("d", Kind::Dev),
            dep("plain", Kind::Normal),
            dep("real-ws", Kind::Normal),
            dep("t", Kind::Normal),
            dep("td", Kind::Dev),
            dep("x", Kind::Normal),
            dep("y", Kind::Normal),
        ]
    );
}

#[test]
fn a_workspace_without_members_has_only_its_root_package() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "Cargo.toml", "[workspace]\n");
    assert!(members(dir.path()).unwrap().is_empty());
}

#[test]
fn a_missing_manifest_is_an_io_error() {
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(members(dir.path()), Err(Error::Io { .. })));
    write(dir.path(), "Cargo.toml", "[workspace]\nmembers = [\"gone\"]\n");
    assert!(matches!(members(dir.path()), Err(Error::Io { .. })));
    write(dir.path(), "Cargo.toml", "[workspace]\nmembers = [\"gone/*\"]\n");
    assert!(matches!(members(dir.path()), Err(Error::Io { .. })));
}

#[test]
fn broken_manifests_are_config_errors() {
    let cases = [
        ("not toml [", "not valid TOML"),
        ("[package]\nname = \"x\"\n", "no `[workspace]` table"),
        ("[workspace]\nmembers = \"crates/*\"\n", "`workspace.members` must be a list of strings"),
        ("[workspace]\nmembers = [1]\n", "`workspace.members` must be a list of strings"),
        ("[workspace]\nmembers = [\"c*/x\"]\n", "unsupported member pattern `c*/x`"),
        ("[workspace]\nmembers = [\"../out\"]\n", "unsupported member pattern `../out`"),
        ("[workspace]\nmembers = [\"/abs\"]\n", "unsupported member pattern `/abs`"),
        ("[workspace]\nexclude = [2]\n", "`workspace.exclude` must be a list of strings"),
        ("[package]\nversion = \"1\"\n[workspace]\n", "no `package.name` string"),
    ];
    for (text, message) in cases {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "Cargo.toml", text);
        match members(dir.path()) {
            Err(Error::Config { message: got, .. }) => assert!(got.contains(message), "{text:?}: {got}"),
            other => panic!("{text:?}: expected a config error, got {other:?}"),
        }
    }
}

#[test]
fn a_dependency_table_of_the_wrong_shape_is_a_config_error() {
    for (top, deps) in [("dependencies = 3\n", ""), ("target = 1\n", ""), ("", "[target.'cfg(unix)']\ndependencies = []\n")] {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "Cargo.toml", "[workspace]\nmembers = [\"a\"]\n");
        write(dir.path(), "a/Cargo.toml", &format!("{top}{}", manifest("a", deps)));
        assert!(matches!(members(dir.path()), Err(Error::Config { .. })), "{deps:?}");
    }
}

#[test]
fn too_many_members_is_a_config_error_not_an_unbounded_read() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "Cargo.toml", "[workspace]\nmembers = [\"c/*\"]\n");
    for index in 0..=MAX_MEMBERS {
        fs::create_dir_all(dir.path().join(format!("c/{index}"))).unwrap();
        fs::write(dir.path().join(format!("c/{index}/Cargo.toml")), "").unwrap();
    }
    match members(dir.path()) {
        Err(Error::Config { message, .. }) => assert!(message.contains("more than"), "{message}"),
        other => panic!("expected a config error, got {other:?}"),
    }
}

#[test]
fn this_workspace_has_every_crate() {
    let found = members(&crate::default_root()).unwrap();
    assert!(found.iter().any(|m| m.name == "inboxcraft-engine"));
    assert!(found.iter().any(|m| m.name == "xtask" && m.dependencies.iter().any(|d| d.name == "toml")));
}
