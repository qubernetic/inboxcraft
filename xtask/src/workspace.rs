// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The members of a workspace and their direct dependencies, read from the manifests.
//!
//! Member patterns are literal directories or `<dir>/*`; other globs are refused rather than guessed at.

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use crate::Error;

/// How many members the reader accepts; far more than any real workspace.
pub(crate) const MAX_MEMBERS: usize = 512;

/// Which table a dependency is declared in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Normal,
    Dev,
    Build,
}

/// A direct dependency, by its package name (after `package = "..."` renames).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Dependency {
    pub(crate) name: String,
    pub(crate) kind: Kind,
}

/// A workspace member.
#[derive(Debug)]
pub(crate) struct Member {
    /// The package name.
    pub(crate) name: String,
    /// The member directory, relative to the workspace root (empty for a root package).
    pub(crate) dir: PathBuf,
    pub(crate) dependencies: Vec<Dependency>,
}

const TABLES: &[(&str, Kind)] = &[("dependencies", Kind::Normal), ("dev-dependencies", Kind::Dev), ("build-dependencies", Kind::Build)];

fn read(path: &Path) -> Result<toml::Table, Error> {
    let text = fs::read_to_string(path).map_err(|source| Error::Io { path: path.to_path_buf(), source })?;
    text.parse().map_err(|error| config_error(path, format!("not valid TOML: {error}")))
}

fn config_error(path: &Path, message: String) -> Error {
    Error::Config { path: path.to_path_buf(), message }
}

/// Every member of the workspace at `root`: a root package first, then the members in pattern order, each pattern's
/// matches sorted.
pub(crate) fn members(root: &Path) -> Result<Vec<Member>, Error> {
    let path = root.join("Cargo.toml");
    let manifest = read(&path)?;
    let workspace =
        manifest.get("workspace").and_then(toml::Value::as_table).ok_or_else(|| config_error(&path, "no `[workspace]` table".to_owned()))?;
    let shared = workspace.get("dependencies").and_then(toml::Value::as_table);
    let exclude = strings(workspace, "exclude", &path)?;
    let mut dirs = Vec::new();
    for pattern in strings(workspace, "members", &path)? {
        expand(root, &pattern, &path, &mut dirs)?;
        if dirs.len() > MAX_MEMBERS {
            return Err(config_error(&path, format!("more than {MAX_MEMBERS} workspace members")));
        }
    }
    dirs.retain(|dir| !exclude.iter().any(|ex| Path::new(ex) == dir));
    let mut found = Vec::new();
    if manifest.contains_key("package") {
        found.push(member(&manifest, PathBuf::new(), &path, shared)?);
    }
    for dir in dirs {
        let path = root.join(&dir).join("Cargo.toml");
        found.push(member(&read(&path)?, dir, &path, shared)?);
    }
    Ok(found)
}

fn strings(table: &toml::Table, key: &str, path: &Path) -> Result<Vec<String>, Error> {
    let Some(value) = table.get(key) else { return Ok(Vec::new()) };
    value
        .as_array()
        .and_then(|list| list.iter().map(|item| item.as_str().map(str::to_owned)).collect::<Option<Vec<_>>>())
        .ok_or_else(|| config_error(path, format!("`workspace.{key}` must be a list of strings")))
}

/// Adds the directories `pattern` names to `dirs`.
fn expand(root: &Path, pattern: &str, path: &Path, dirs: &mut Vec<PathBuf>) -> Result<(), Error> {
    let (base, glob) = match pattern.strip_suffix("/*") {
        Some(base) => (base, true),
        None => (pattern, false),
    };
    let plain = Path::new(base).components().all(|part| matches!(part, Component::Normal(_)));
    if !plain || base.contains(['*', '?', '[', ']', '{', '}']) {
        return Err(config_error(path, format!("unsupported member pattern `{pattern}`")));
    }
    if !glob {
        dirs.push(PathBuf::from(base));
        return Ok(());
    }
    let dir = root.join(base);
    let io_error = |source: io::Error| Error::Io { path: dir.clone(), source };
    let mut matches = Vec::new();
    for entry in fs::read_dir(&dir).map_err(io_error)? {
        let entry = entry.map_err(io_error)?;
        if entry.path().join("Cargo.toml").is_file() {
            matches.push(Path::new(base).join(entry.file_name()));
        }
        if matches.len() > MAX_MEMBERS {
            return Err(config_error(path, format!("more than {MAX_MEMBERS} workspace members")));
        }
    }
    matches.sort();
    dirs.append(&mut matches);
    Ok(())
}

fn member(manifest: &toml::Table, dir: PathBuf, path: &Path, shared: Option<&toml::Table>) -> Result<Member, Error> {
    let name = manifest
        .get("package")
        .and_then(|package| package.get("name"))
        .and_then(toml::Value::as_str)
        .ok_or_else(|| config_error(path, "no `package.name` string".to_owned()))?;
    let mut dependencies = Vec::new();
    collect(manifest, "", path, shared, &mut dependencies)?;
    if let Some(targets) = manifest.get("target") {
        let targets = targets.as_table().ok_or_else(|| config_error(path, "`target` must be a table".to_owned()))?;
        for (cfg, table) in targets {
            let table = table.as_table().ok_or_else(|| config_error(path, format!("`target.{cfg}` must be a table")))?;
            collect(table, &format!("target.{cfg}."), path, shared, &mut dependencies)?;
        }
    }
    Ok(Member { name: name.to_owned(), dir, dependencies })
}

fn collect(table: &toml::Table, prefix: &str, path: &Path, shared: Option<&toml::Table>, out: &mut Vec<Dependency>) -> Result<(), Error> {
    for (key, kind) in TABLES {
        let Some(deps) = table.get(*key) else { continue };
        let deps = deps.as_table().ok_or_else(|| config_error(path, format!("`{prefix}{key}` must be a table")))?;
        for (name, spec) in deps {
            let inherited = spec.get("workspace").and_then(toml::Value::as_bool) == Some(true);
            let source = if inherited { shared.and_then(|shared| shared.get(name)) } else { Some(spec) };
            let package = source.and_then(|spec| spec.get("package")).and_then(toml::Value::as_str).unwrap_or(name);
            out.push(Dependency { name: package.to_owned(), kind: *kind });
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "workspace_tests.rs"]
mod tests;
