// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `cargo xtask cov`: line coverage per crate from `cargo llvm-cov nextest`, checked against the floors in
//! [`CONFIG`].
//!
//! The gate runs the tests under `cargo llvm-cov nextest` (profile `ci`) into an lcov report, `lcov.info` in the root,
//! which stays for the Codecov upload. In `cargo xtask ci` this is the only test run; its JUnit report (profile `ci`)
//! lands in nextest's store under the root, `target/nextest/ci/junit.xml`, whatever target directory llvm-cov builds
//! in. A failed run fails the gate before any floor is checked, and the gate says so. A line is a `DA` record; it is
//! covered when its count is not 0. A file belongs to the member whose directory holds it most closely; files outside
//! every member are not counted. Files matching
//! `exclude` count nowhere; files matching `workspace-exclude` count for their crate but not for the workspace total.
//! With `--check` a crate or the workspace below its floor fails the gate; a crate with no lines has nothing to fall
//! below.

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

use crate::workspace::{self, Member};
use crate::{Context, Error};

/// The floors, relative to the workspace root.
pub(crate) const CONFIG: &str = ".config/coverage.toml";

/// The report `cov` writes when the context names no other.
const REPORT: &str = "lcov.info";

const KEYS: &[&str] = &["workspace", "crates", "exclude", "workspace-exclude"];

/// The line coverage floors, in per cent.
#[derive(Debug)]
pub(crate) struct Floors {
    /// The floor of the workspace total.
    pub(crate) workspace: f64,
    /// The floors of single crates, by package name.
    pub(crate) crates: BTreeMap<String, f64>,
    /// Patterns of the files counted nowhere.
    pub(crate) exclude: Vec<String>,
    /// Patterns of the files counted for their crate but not for the workspace total.
    pub(crate) workspace_exclude: Vec<String>,
}

impl Floors {
    /// Reads [`CONFIG`] under `root`; every crate it names must be one of `members`.
    pub(crate) fn read(root: &Path, members: &[Member]) -> Result<Self, Error> {
        let path = root.join(CONFIG);
        let text = fs::read_to_string(&path).map_err(|source| Error::Io { path: path.clone(), source })?;
        let error = |message: String| Error::Config { path: path.clone(), message };
        let table: toml::Table = text.parse().map_err(|e| error(format!("not valid TOML: {e}")))?;
        if let Some(key) = table.keys().find(|key| !KEYS.contains(&key.as_str())) {
            return Err(error(format!("unknown key `{key}`")));
        }
        let workspace = percent(table.get("workspace")).ok_or_else(|| error("`workspace` must be a number from 0 to 100".to_owned()))?;
        let mut crates = BTreeMap::new();
        if let Some(value) = table.get("crates") {
            let list = value.as_table().ok_or_else(|| error("`crates` must be a table".to_owned()))?;
            for (name, value) in list {
                let floor = percent(Some(value)).ok_or_else(|| error(format!("`crates.{name}` must be a number from 0 to 100")))?;
                if !members.iter().any(|member| member.name == *name) {
                    return Err(error(format!("`crates.{name}` is not a workspace crate")));
                }
                crates.insert(name.clone(), floor);
            }
        }
        let exclude = patterns(&table, "exclude").map_err(error)?;
        let workspace_exclude = patterns(&table, "workspace-exclude").map_err(error)?;
        Ok(Self { workspace, crates, exclude, workspace_exclude })
    }
}

/// A number from 0 to 100.
fn percent(value: Option<&toml::Value>) -> Option<f64> {
    let number = match value? {
        toml::Value::Integer(n) => *n as f64,
        toml::Value::Float(n) => *n,
        _ => return None,
    };
    (0.0..=100.0).contains(&number).then_some(number)
}

/// The list of file patterns under `key`: `/`-separated components, each a name, `*` (one component) or, last, `**`
/// (one or more components).
fn patterns(table: &toml::Table, key: &str) -> Result<Vec<String>, String> {
    let Some(value) = table.get(key) else { return Ok(Vec::new()) };
    let list = value
        .as_array()
        .and_then(|list| list.iter().map(|item| item.as_str().map(str::to_owned)).collect::<Option<Vec<_>>>())
        .ok_or_else(|| format!("`{key}` must be a list of strings"))?;
    for pattern in &list {
        let parts: Vec<&str> = pattern.split('/').collect();
        let last = parts.len().saturating_sub(1);
        let valid = parts.iter().enumerate().all(|(at, part)| match *part {
            "" | "." | ".." => false,
            "**" => at == last,
            "*" => true,
            name => !name.contains('*'),
        });
        if !valid {
            return Err(format!("unsupported pattern `{pattern}` in `{key}`"));
        }
    }
    Ok(list)
}

/// Whether the workspace-relative `path` matches `pattern` (see [`patterns`]).
pub(crate) fn matches(pattern: &str, path: &Path) -> bool {
    let mut parts = path.components().map(|part| part.as_os_str().to_str());
    for glob in pattern.split('/') {
        if glob == "**" {
            return parts.next().is_some();
        }
        match parts.next() {
            Some(Some(part)) if glob == "*" || glob == part => {}
            _ => return false,
        }
    }
    parts.next().is_none()
}

/// The lines of every file of an lcov report: line number to covered, by file. Paths under `root` are made relative
/// to it; others stay as written.
pub(crate) fn parse_lcov(text: &str, path: &Path, root: &Path) -> Result<BTreeMap<PathBuf, BTreeMap<u64, bool>>, Error> {
    let canonical = fs::canonicalize(root).ok();
    let error = |n: usize, message: &str| Error::Config { path: path.to_path_buf(), message: format!("line {n}: {message}") };
    let mut report: BTreeMap<PathBuf, BTreeMap<u64, bool>> = BTreeMap::new();
    let mut current: Option<PathBuf> = None;
    for (index, line) in text.lines().enumerate() {
        let n = index.saturating_add(1);
        if let Some(file) = line.strip_prefix("SF:") {
            if file.is_empty() {
                return Err(error(n, "`SF` without a path"));
            }
            let file = Path::new(file);
            let relative = file.strip_prefix(root).ok().or_else(|| canonical.as_deref().and_then(|root| file.strip_prefix(root).ok()));
            current = Some(relative.unwrap_or(file).to_path_buf());
        } else if let Some(record) = line.strip_prefix("DA:") {
            let Some(file) = &current else { return Err(error(n, "`DA` outside a `SF` record")) };
            let mut fields = record.split(',').map(str::parse::<u64>);
            let (Some(Ok(number)), Some(Ok(count))) = (fields.next(), fields.next()) else { return Err(error(n, "bad `DA` record")) };
            let covered = report.entry(file.clone()).or_default().entry(number).or_insert(false);
            *covered |= count > 0;
        } else if line == "end_of_record" {
            current = None;
        }
    }
    Ok(report)
}

#[derive(Default, Clone, Copy)]
struct Lines {
    total: usize,
    covered: usize,
}

impl Lines {
    fn add(&mut self, other: Self) {
        self.total = self.total.saturating_add(other.total);
        self.covered = self.covered.saturating_add(other.covered);
    }

    fn below(self, floor: f64) -> bool {
        self.total > 0 && (self.covered as f64) * 100.0 < floor * self.total as f64
    }

    /// The coverage with two decimals, rounded down so a number below a floor never prints as the floor.
    fn cover(self) -> String {
        match self.covered.saturating_mul(10_000).checked_div(self.total) {
            Some(hundredths) => format!("{}.{:02} %", hundredths / 100, hundredths % 100),
            None => "-".to_owned(),
        }
    }
}

/// Runs the tests under coverage, reports per crate to `out` and, with `check`, returns whether every floor is met.
pub(crate) fn gate(ctx: &mut Context, check: bool, out: &mut dyn Write) -> Result<bool, Error> {
    let members = workspace::members(&ctx.root)?;
    let floors = Floors::read(&ctx.root, &members)?;
    let path = ctx.lcov.clone().unwrap_or_else(|| ctx.root.join(REPORT));
    let output = path.to_str().ok_or_else(|| Error::Config { path: path.clone(), message: "the report path must be UTF-8".to_owned() })?;
    match fs::remove_file(&path) {
        Err(source) if source.kind() != io::ErrorKind::NotFound => return Err(Error::Io { path, source }),
        _ => {}
    }
    writeln!(out, "cov: running cargo llvm-cov nextest (profile ci)")?;
    out.flush()?;
    let args = ["llvm-cov", "nextest", "--workspace", "--profile", "ci", "--lcov", "--output-path", output];
    if !ctx.runner.run(&ctx.cargo, &args, &ctx.root)? {
        writeln!(out, "cov: the tests failed (a failing test or a build error, see the nextest output above); no coverage floor was checked")?;
        return Ok(false);
    }
    let text = fs::read_to_string(&path).map_err(|source| Error::Io { path: path.clone(), source })?;
    let report = parse_lcov(&text, &path, &ctx.root)?;

    let mut rows = vec![Lines::default(); members.len()];
    let mut total = Lines::default();
    let mut in_workspace = false;
    for (file, lines) in &report {
        let relative = file.components().all(|part| matches!(part, Component::Normal(_)));
        let Some((index, _)) = workspace::owner(&members, file).filter(|_| relative) else { continue };
        in_workspace = true;
        if floors.exclude.iter().any(|pattern| matches(pattern, file)) {
            continue;
        }
        let count = Lines { total: lines.len(), covered: lines.values().filter(|covered| **covered).count() };
        if let Some(row) = rows.get_mut(index) {
            row.add(count);
        }
        if !floors.workspace_exclude.iter().any(|pattern| matches(pattern, file)) {
            total.add(count);
        }
    }
    if !report.is_empty() && !in_workspace {
        return Err(Error::Config { path, message: "no file of the report is in a workspace crate".to_owned() });
    }

    let width = members.iter().map(|member| member.name.len()).chain(["workspace".len()]).max().unwrap_or_default();
    writeln!(out, "cov: {:<width$}  {:>5}  {:>7}  {:>8}  floor", "crate", "lines", "covered", "cover")?;
    let mut below = Vec::new();
    let named = members.iter().map(|member| (member.name.as_str(), floors.crates.get(&member.name).copied()));
    for ((name, floor), lines) in named.zip(rows).chain([(("workspace", Some(floors.workspace)), total)]) {
        let short = floor.is_some_and(|floor| lines.below(floor));
        let floor = floor.map_or_else(|| "-".to_owned(), |floor| format!("{floor} %"));
        let mark = if short { "  below" } else { "" };
        writeln!(out, "cov: {name:<width$}  {:>5}  {:>7}  {:>8}  {floor:>5}{mark}", lines.total, lines.covered, lines.cover())?;
        if short {
            below.push(name);
        }
    }
    if below.is_empty() {
        writeln!(out, "cov: every floor met")?;
        return Ok(true);
    }
    let unchecked = if check { "" } else { " (not checked without --check)" };
    writeln!(out, "cov: {} below the floor: {}{unchecked}", below.len(), below.join(", "))?;
    Ok(!check)
}

#[cfg(test)]
#[path = "cov_tests.rs"]
mod tests;
