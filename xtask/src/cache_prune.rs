// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `cargo xtask cache-prune`: what CI leaves in `target/` before it saves the dependency cache.
//!
//! An allow list: under `target/` only cargo's marker files and the `deps`, `build` and `.fingerprint` folders of
//! each profile (`target/<profile>/` or `target/<triple>/<profile>/`) stay, and in those folders only the entries
//! that are not named after a package or target of this workspace. Everything else goes: binaries, examples,
//! incremental data, reports, coverage data, `llvm-cov-target/tmp` and every folder the list does not know. The
//! names come from `cargo metadata`; when that call fails, or the list is empty or lacks `xtask`, nothing is
//! deleted. Afterwards every path under `target/`, at any depth, is checked against the names. Links are removed,
//! never followed.

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::{Context, Error, count};

/// The folders of a profile that hold third-party artifacts.
const KEPT: &[&str] = &["deps", "build", ".fingerprint"];

/// Cargo's marker files, kept in `target/`, a triple folder and a profile folder.
const MARKERS: &[&str] = &["CACHEDIR.TAG", ".rustc_info.json", ".cargo-lock", ".cargo-artifact-lock", ".cargo-build-lock"];

/// The length of the hash cargo puts after an artifact's name.
const HASH: usize = 16;

/// How deep the check walks before it gives up; far deeper than any real `target/`.
const MAX_DEPTH: usize = 64;

/// Whether `text` can be a package or target name: ASCII letters, digits, `-` and `_`.
fn is_name(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Every package and target name of the workspace in the `cargo metadata` JSON `json`, each in its hyphen and its
/// underscore form. Build scripts are left out: their target name is the same in every package, and the package
/// name already covers their folders.
pub(crate) fn names(json: &[u8]) -> Result<BTreeSet<String>, String> {
    let value: serde_json::Value = serde_json::from_slice(json).map_err(|error| format!("`cargo metadata` gave no JSON: {error}"))?;
    let packages = value.get("packages").and_then(serde_json::Value::as_array).ok_or("`cargo metadata` lists no packages")?;
    let mut found = BTreeSet::new();
    for package in packages {
        let name = package.get("name").and_then(serde_json::Value::as_str).ok_or("a package without a name")?;
        found.insert(name.to_owned());
        let targets = package.get("targets").and_then(serde_json::Value::as_array).ok_or_else(|| format!("{name} lists no targets"))?;
        for target in targets {
            let kinds = target.get("kind").and_then(serde_json::Value::as_array);
            if kinds.is_some_and(|kinds| kinds.iter().any(|kind| kind.as_str() == Some("custom-build"))) {
                continue;
            }
            let target = target.get("name").and_then(serde_json::Value::as_str).ok_or_else(|| format!("{name}: a target without a name"))?;
            found.insert(target.to_owned());
        }
    }
    if let Some(bad) = found.iter().find(|name| !is_name(name)) {
        return Err(format!("{bad:?} is not a package or target name"));
    }
    let both: BTreeSet<String> = found.iter().flat_map(|name| [name.replace('-', "_"), name.replace('_', "-")]).collect();
    if !both.contains("xtask") {
        return Err("the name list lacks `xtask`".to_owned());
    }
    Ok(both)
}

/// Whether the file name `name` is an artifact of one of `names`: `[lib]<name>-<16 hex digits>`, then the end or a
/// `.` and an extension. A target `wasm` does not take `wasm-bindgen-<hash>` with it.
pub(crate) fn is_ours(name: &str, names: &BTreeSet<String>) -> bool {
    names.iter().any(|ours| ["", "lib"].iter().any(|lib| hashed(name, lib, ours)))
}

/// Whether `name` is `<lib><ours>-<hash>` with nothing or `.<extension>` after the hash.
fn hashed(name: &str, lib: &str, ours: &str) -> bool {
    let Some(rest) = name.strip_prefix(lib).and_then(|rest| rest.strip_prefix(ours)).and_then(|rest| rest.strip_prefix('-')) else {
        return false;
    };
    let Some(hash) = rest.get(..HASH) else { return false };
    hash.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        && rest.get(HASH..).is_some_and(|after| after.is_empty() || after.starts_with('.'))
}

fn io_error(path: &Path) -> impl Fn(io::Error) -> Error + '_ {
    move |source| Error::Io { path: path.to_path_buf(), source }
}

/// The entries of `dir`, sorted, with whether each is a real folder (not a link).
fn entries(dir: &Path) -> Result<Vec<(PathBuf, bool)>, Error> {
    let mut found = Vec::new();
    for entry in fs::read_dir(dir).map_err(io_error(dir))? {
        let entry = entry.map_err(io_error(dir))?;
        let kind = entry.file_type().map_err(io_error(&entry.path()))?;
        found.push((entry.path(), kind.is_dir()));
    }
    found.sort();
    Ok(found)
}

/// Removes `path`: a folder with all it holds, a file or a link (never what it points to).
fn remove(path: &Path, is_dir: bool) -> Result<(), Error> {
    if is_dir { fs::remove_dir_all(path) } else { fs::remove_file(path) }.map_err(io_error(path))
}

fn file_name(path: &Path) -> &str {
    path.file_name().and_then(OsStr::to_str).unwrap_or_default()
}

/// Whether the folder `dir` is a profile: it holds one of the [`KEPT`] folders.
fn is_profile(dir: &Path) -> bool {
    KEPT.iter().any(|kept| dir.join(kept).symlink_metadata().is_ok_and(|meta| meta.is_dir()))
}

/// What the prune did, for the report.
#[derive(Default)]
struct Pruned {
    /// Each profile kept, workspace-relative, with how many entries of each [`KEPT`] folder stayed.
    kept: Vec<(String, [usize; 3])>,
    /// The paths outside the allow list that went, workspace-relative.
    removed: Vec<String>,
    /// How many entries named after the workspace went.
    ours: usize,
}

struct Walk<'a> {
    root: &'a Path,
    names: &'a BTreeSet<String>,
    pruned: Pruned,
}

impl Walk<'_> {
    fn rel(&self, path: &Path) -> String {
        path.strip_prefix(self.root).unwrap_or(path).display().to_string()
    }

    /// Keeps the markers and the folders `keep` accepts in `dir`, removes everything else.
    fn level(&mut self, dir: &Path, keep: &mut dyn FnMut(&mut Self, &Path) -> Result<bool, Error>) -> Result<(), Error> {
        for (path, is_dir) in entries(dir)? {
            let kept = if is_dir { keep(self, &path)? } else { MARKERS.contains(&file_name(&path)) };
            if !kept {
                remove(&path, is_dir)?;
                self.pruned.removed.push(self.rel(&path));
            }
        }
        Ok(())
    }

    /// `target/`: profiles, and triple folders that hold profiles.
    fn target(&mut self, target: &Path) -> Result<(), Error> {
        self.level(target, &mut |walk, dir| {
            if is_profile(dir) {
                return walk.profile(dir).map(|()| true);
            }
            let profiles: Vec<PathBuf> =
                entries(dir)?.into_iter().filter(|(path, is_dir)| *is_dir && is_profile(path)).map(|(path, _)| path).collect();
            if profiles.is_empty() {
                return Ok(false);
            }
            walk.level(dir, &mut |walk, sub| {
                if profiles.iter().any(|profile| profile == sub) { walk.profile(sub).map(|()| true) } else { Ok(false) }
            })?;
            Ok(true)
        })
    }

    /// A profile: its [`KEPT`] folders without our entries.
    fn profile(&mut self, profile: &Path) -> Result<(), Error> {
        let mut counts = [0_usize; 3];
        self.level(profile, &mut |walk, dir| {
            let Some(slot) = KEPT.iter().position(|kept| *kept == file_name(dir)) else { return Ok(false) };
            for (path, is_dir) in entries(dir)? {
                if is_ours(file_name(&path), walk.names) {
                    remove(&path, is_dir)?;
                    walk.pruned.ours = walk.pruned.ours.saturating_add(1);
                } else if let Some(count) = counts.get_mut(slot) {
                    *count = count.saturating_add(1);
                }
            }
            Ok(true)
        })?;
        let rel = self.rel(profile);
        self.pruned.kept.push((rel, counts));
        Ok(())
    }
}

/// Every path under `dir`, at any depth, whose name is one of ours.
fn leftovers(dir: &Path, names: &BTreeSet<String>, depth: usize, found: &mut Vec<PathBuf>) -> Result<(), Error> {
    if depth > MAX_DEPTH {
        return Err(Error::Io { path: dir.to_path_buf(), source: io::Error::other(format!("more than {MAX_DEPTH} folders deep")) });
    }
    for (path, is_dir) in entries(dir)? {
        if is_ours(file_name(&path), names) {
            found.push(path.clone());
        }
        if is_dir {
            leftovers(&path, names, depth.saturating_add(1), found)?;
        }
    }
    Ok(())
}

/// Prunes `target/` under the workspace root and reports to `out`; fails when the names cannot be read or a path
/// named after the workspace is left.
pub(crate) fn prune(ctx: &mut Context, out: &mut dyn Write) -> Result<bool, Error> {
    let Some(json) = ctx.runner.output(&ctx.cargo, &["metadata", "--no-deps", "--format-version", "1"], &ctx.root)? else {
        writeln!(out, "cache-prune: `cargo metadata` failed; nothing was deleted")?;
        return Ok(false);
    };
    let names = match names(&json) {
        Ok(names) => names,
        Err(message) => {
            writeln!(out, "cache-prune: {message}; nothing was deleted")?;
            return Ok(false);
        }
    };
    writeln!(out, "cache-prune: {}", count(names.len(), "workspace name"))?;
    let target = ctx.root.join("target");
    fs::symlink_metadata(&target).map_err(io_error(&target))?;
    let mut walk = Walk { root: &ctx.root, names: &names, pruned: Pruned::default() };
    walk.target(&target)?;
    let Pruned { kept, removed, ours } = walk.pruned;
    for (profile, [deps, build, fingerprint]) in kept {
        writeln!(out, "cache-prune: kept {profile}: deps {deps}, build {build}, .fingerprint {fingerprint}")?;
    }
    for path in removed {
        writeln!(out, "cache-prune: removed {path}")?;
    }
    let entries = if ours == 1 { "entry" } else { "entries" };
    writeln!(out, "cache-prune: removed {ours} {entries} named after the workspace")?;
    let mut left = Vec::new();
    leftovers(&target, &names, 0, &mut left)?;
    for path in &left {
        writeln!(out, "cache-prune: still named after the workspace: {}", path.strip_prefix(&ctx.root).unwrap_or(path).display())?;
    }
    if left.is_empty() {
        writeln!(out, "cache-prune: no path under target/ is named after a workspace package or target")?;
    }
    Ok(left.is_empty())
}

#[cfg(test)]
#[path = "cache_prune_tests.rs"]
mod tests;
