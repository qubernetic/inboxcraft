// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `cargo xtask flatpak-sources` and `flatpak-sources-check`: the crates of `Cargo.lock` as flatpak-builder sources.
//!
//! flatpak-builder builds without a network, so every crate the lock file names is a source of the manifest: the
//! `.crate` archive from crates.io with its checksum from the lock file, unpacked into `cargo/vendor/`, and a cargo
//! configuration that points crates.io there. `flatpak-sources` writes [`SOURCES`]; `flatpak-sources-check` (part of
//! `cargo xtask ci`) fails when the file is not what `Cargo.lock` gives, so a dependency change cannot land without
//! it. Only crates.io is allowed: a git or other registry source is refused. The lock file is read as hostile: a
//! name, version or checksum that could leave `cargo/vendor/` or break the JSON is refused.

use std::fmt::Write as _;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

use crate::{Error, count};

/// Where the sources go, relative to the workspace root.
pub(crate) const SOURCES: &str = "contrib/flatpak/cargo-sources.json";

/// The `source` values of crates.io in a lock file: the git index and the sparse index.
const CRATES_IO: &[&str] = &["registry+https://github.com/rust-lang/crates.io-index", "sparse+https://index.crates.io/"];

/// The cargo configuration of the build: crates.io comes from `cargo/vendor/`, nothing from the network. Relative
/// paths in `$CARGO_HOME/config.toml` start at the parent of `$CARGO_HOME`, the module's build directory.
const CONFIG: &str =
    "[source.vendored-sources]\ndirectory = \"cargo/vendor\"\n\n[source.crates-io]\nreplace-with = \"vendored-sources\"\n\n[net]\noffline = true\n";

/// One crate from crates.io.
struct Crate {
    name: String,
    version: String,
    checksum: String,
}

/// Whether `text` is a package name: ASCII letters, digits, `-` and `_`.
fn is_name(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Whether `text` is a version: ASCII letters, digits, `.`, `-` and `+`, starting with a digit.
fn is_version(text: &str) -> bool {
    text.bytes().next().is_some_and(|b| b.is_ascii_digit()) && text.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-' || b == b'+')
}

/// Whether `text` is a SHA-256 in lower-case hex.
fn is_checksum(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The crates.io crates of the lock file `text`, in its order; workspace crates (no source) are left out.
fn crates(text: &str) -> Result<Vec<Crate>, String> {
    let lock: toml::Table = text.parse().map_err(|error: toml::de::Error| format!("Cargo.lock: {}", error.message()))?;
    let Some(packages) = lock.get("package").and_then(toml::Value::as_array).filter(|packages| !packages.is_empty()) else {
        return Err("Cargo.lock lists no packages".to_owned());
    };
    let mut found = Vec::new();
    for (at, package) in packages.iter().enumerate() {
        let n = at.saturating_add(1);
        let package = package.as_table().ok_or_else(|| format!("Cargo.lock: package {n} is not a table"))?;
        let field = |key: &str| package.get(key).and_then(toml::Value::as_str);
        let name = field("name").ok_or_else(|| format!("Cargo.lock: package {n} has no name"))?;
        let version = field("version").ok_or_else(|| format!("Cargo.lock: package {n} has no version"))?;
        let source = match package.get("source") {
            None => continue,
            Some(source) => source.as_str().ok_or_else(|| format!("Cargo.lock: {name} {version} has a source that is not text"))?,
        };
        if !CRATES_IO.contains(&source) {
            return Err(format!("{name} {version} comes from {source}, not crates.io"));
        }
        if !is_name(name) {
            return Err(format!("{name:?} is not a crate name"));
        }
        if !is_version(version) {
            return Err(format!("{name}: {version:?} is not a version"));
        }
        let checksum = field("checksum").ok_or_else(|| format!("{name} {version} has no checksum"))?;
        if !is_checksum(checksum) {
            return Err(format!("{name} {version}: {checksum:?} is not a SHA-256 checksum"));
        }
        found.push(Crate { name: name.to_owned(), version: version.to_owned(), checksum: checksum.to_owned() });
    }
    Ok(found)
}

/// `text` as a JSON string, quotes included.
pub(crate) fn json_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len().saturating_add(2));
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c if u32::from(c) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// One JSON object of `fields`, indented as an element of the top-level array.
fn object(fields: &[(&str, String)]) -> String {
    let body: Vec<String> = fields.iter().map(|(key, value)| format!("    {}: {}", json_string(key), json_string(value))).collect();
    format!("  {{\n{}\n  }}", body.join(",\n"))
}

/// The flatpak-builder sources for the lock file `text`.
pub(crate) fn sources(text: &str) -> Result<String, String> {
    let crates = crates(text)?;
    let mut entries = Vec::with_capacity(crates.len().saturating_mul(2).saturating_add(1));
    for Crate { name, version, checksum } in &crates {
        let dest = format!("cargo/vendor/{name}-{version}");
        entries.push(object(&[
            ("type", "archive".to_owned()),
            ("archive-type", "tar-gzip".to_owned()),
            ("url", format!("https://static.crates.io/crates/{name}/{name}-{version}.crate")),
            ("sha256", checksum.clone()),
            ("dest", dest.clone()),
        ]));
        entries.push(object(&[
            ("type", "inline".to_owned()),
            ("contents", format!("{{\"package\": \"{checksum}\", \"files\": {{}}}}")),
            ("dest", dest),
            ("dest-filename", ".cargo-checksum.json".to_owned()),
        ]));
    }
    entries.push(object(&[
        ("type", "inline".to_owned()),
        ("contents", CONFIG.to_owned()),
        ("dest", "cargo".to_owned()),
        ("dest-filename", "config.toml".to_owned()),
    ]));
    Ok(format!("[\n{}\n]\n", entries.join(",\n")))
}

/// Writes the sources for `root`'s `Cargo.lock` (`check` false) or checks the written ones (`check` true), reports
/// to `out` and returns whether they match.
pub(crate) fn gate(root: &Path, check: bool, out: &mut dyn Write) -> Result<bool, Error> {
    let lock = root.join("Cargo.lock");
    let text = fs::read_to_string(&lock).map_err(|source| Error::Io { path: lock.clone(), source })?;
    let wanted = sources(&text).map_err(|message| Error::Config { path: lock.clone(), message })?;
    let crates = count(wanted.matches("\"archive-type\"").count(), "crate");
    let path = root.join(SOURCES);
    if !check {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir).map_err(|source| Error::Io { path: dir.to_path_buf(), source })?;
        }
        fs::write(&path, &wanted).map_err(|source| Error::Io { path: path.clone(), source })?;
        writeln!(out, "flatpak-sources: wrote {SOURCES} for {crates}")?;
        return Ok(true);
    }
    let written = match fs::read_to_string(&path) {
        Ok(written) => written,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            writeln!(out, "flatpak-sources-check: {SOURCES} is missing; run `just flatpak-sources` and commit it")?;
            return Ok(false);
        }
        Err(source) => return Err(Error::Io { path, source }),
    };
    if written == wanted {
        writeln!(out, "flatpak-sources-check: {SOURCES} matches Cargo.lock ({crates})")?;
        Ok(true)
    } else {
        writeln!(out, "flatpak-sources-check: {SOURCES} does not match Cargo.lock; run `just flatpak-sources` and commit it")?;
        Ok(false)
    }
}

#[cfg(test)]
#[path = "flatpak_tests.rs"]
mod tests;
