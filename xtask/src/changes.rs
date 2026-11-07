// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `cargo xtask changes [--fold]`: the change fragments in `changes/`.
//!
//! A slice writes its `CHANGELOG.md` lines, its `ROADMAP.md` progress line, its new `docs/gaps.md` lines and its
//! revision rows into `changes/<issue>.md` instead of those files, so slices that run side by side do not conflict.
//! `changes` checks every fragment and is part of `cargo xtask ci`; `changes --fold` writes them into the documents,
//! dated today in UTC, and deletes them. The format is in docs/development.md, "Change fragments". A fold with a
//! malformed fragment, or into a document without its section, writes nothing.

use std::collections::BTreeMap;
use std::collections::btree_map::Entry as Slot;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Component, Path};

use inboxcraft_time::{Clock, Offset, SystemClock, Timestamp};

use crate::{Error, count};

/// Where the fragments live, relative to the workspace root.
const DIR: &str = "changes";
/// The largest fragment read; a slice's record is a few lines.
const MAX_BYTES: u64 = 65_536;
/// The Keep a Changelog kinds, in their order in a release.
const KINDS: [&str; 6] = ["Added", "Changed", "Deprecated", "Removed", "Fixed", "Security"];
/// The kinds of change a revision row names, the largest first.
const CHANGES: [&str; 3] = ["major", "minor", "patch"];

const CHANGELOG: &str = "CHANGELOG.md";
const ROADMAP: &str = "ROADMAP.md";
const GAPS: &str = "docs/gaps.md";
const UNRELEASED: &str = "## [Unreleased]";
const PROGRESS: &str = "## Progress log";
const ROUGH: &str = "## Known rough edges (not ranked)";
const HISTORY: &str = "## Revision history";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    Changelog,
    Roadmap,
    Gaps,
    Revisions,
}

impl Section {
    fn named(name: &str) -> Option<Self> {
        match name {
            "Changelog" => Some(Self::Changelog),
            "Roadmap" => Some(Self::Roadmap),
            "Gaps" => Some(Self::Gaps),
            "Revisions" => Some(Self::Revisions),
            _ => None,
        }
    }
}

/// Where the parser is: before the first section, inside one it knows, or inside one it reported and skips.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Start,
    In(Section),
    Skip,
}

/// One `- ` entry and its continuation lines, as written.
#[derive(Debug)]
struct Entry {
    section: Section,
    /// The index into [`KINDS`]; Changelog entries only.
    kind: usize,
    line: usize,
    lines: Vec<String>,
}

impl Entry {
    /// The text without the `- ` and the indentation, on one line.
    fn joined(&self) -> String {
        let parts: Vec<&str> = self.lines.iter().map(|line| line.trim_start_matches("- ").trim()).collect();
        parts.join(" ")
    }
}

/// A revision row: the document, the index into [`CHANGES`] and the summary.
#[derive(Debug)]
struct Revision {
    doc: String,
    change: usize,
    summary: String,
}

/// A checked fragment.
#[derive(Debug)]
struct Fragment {
    issue: u64,
    entries: Vec<Entry>,
    revisions: Vec<Revision>,
}

/// A problem at a 1-based line; 0 is the whole fragment.
type Problem = (usize, String);

/// Reads a fragment line by line.
#[derive(Default)]
struct Parser {
    state: Option<State>,
    /// The Changelog kind entries go under; `None` before the first or after an unknown one.
    kind: Option<usize>,
    /// Whether the last `### ` heading named an unknown kind, already reported: its entries are skipped.
    unknown_kind: bool,
    sections: Vec<Section>,
    kinds: Vec<usize>,
    /// The last heading that has no entry yet: its line and how it reads.
    pending: Option<(usize, String)>,
    /// Whether the line before was part of an entry, so an indented line continues it.
    open: bool,
    entries: Vec<Entry>,
    problems: Vec<Problem>,
}

impl Parser {
    fn state(&self) -> State {
        self.state.unwrap_or(State::Start)
    }

    fn problem(&mut self, line: usize, message: impl Into<String>) {
        self.problems.push((line, message.into()));
    }

    fn close_pending(&mut self) {
        if let Some((line, heading)) = self.pending.take() {
            self.problem(line, format!("`{heading}` has no entry"));
        }
    }

    fn line(&mut self, number: usize, line: &str) {
        let open = std::mem::take(&mut self.open);
        if line.chars().any(char::is_control) {
            self.problem(number, "a control character");
        } else if line.trim().is_empty() {
        } else if let Some(name) = line.strip_prefix("### ") {
            self.kind_heading(number, name);
        } else if let Some(name) = line.strip_prefix("## ") {
            self.section_heading(number, name);
        } else if line.starts_with("# ") {
            if number != 1 {
                self.problem(number, "a title after the first line");
            }
        } else if let Some(text) = line.strip_prefix("- ") {
            self.entry(number, line, text);
        } else if line.starts_with("  ") {
            self.continuation(number, line, open);
        } else {
            self.problem(number, "expected `## <section>`, `### <kind>`, `- <entry>` or a line indented by two spaces");
        }
    }

    fn section_heading(&mut self, number: usize, name: &str) {
        self.close_pending();
        self.kind = None;
        self.unknown_kind = false;
        let Some(section) = Section::named(name) else {
            self.problem(number, format!("unknown section `{name}`"));
            self.state = Some(State::Skip);
            return;
        };
        if self.sections.contains(&section) {
            self.problem(number, format!("a second `## {name}`"));
        }
        self.sections.push(section);
        self.state = Some(State::In(section));
        self.pending = Some((number, format!("## {name}")));
    }

    fn kind_heading(&mut self, number: usize, name: &str) {
        if self.state() != State::In(Section::Changelog) {
            self.close_pending();
            self.problem(number, "`### ` kinds go under `## Changelog` only");
            self.state = Some(State::Skip);
            return;
        }
        self.pending = self.pending.take().filter(|(_, heading)| heading.starts_with("### "));
        self.close_pending();
        self.kind = KINDS.iter().position(|kind| *kind == name);
        self.unknown_kind = self.kind.is_none();
        match self.kind {
            None => self.problem(number, format!("unknown kind `{name}`")),
            Some(kind) if self.kinds.contains(&kind) => self.problem(number, format!("a second `### {name}`")),
            Some(kind) => self.kinds.push(kind),
        }
        if self.kind.is_some() {
            self.pending = Some((number, format!("### {name}")));
        }
    }

    fn entry(&mut self, number: usize, line: &str, text: &str) {
        let section = match self.state() {
            State::Start => return self.problem(number, "an entry before the first `## ` section"),
            State::Skip => return,
            State::In(section) => section,
        };
        let kind = match (section, self.kind) {
            (Section::Changelog, None) if self.unknown_kind => return,
            (Section::Changelog, None) => return self.problem(number, "a Changelog entry outside a `### <kind>`"),
            (_, kind) => kind.unwrap_or_default(),
        };
        if text.trim().is_empty() {
            return self.problem(number, "an empty entry");
        }
        self.pending = None;
        self.open = true;
        self.entries.push(Entry { section, kind, line: number, lines: vec![line.to_owned()] });
    }

    fn continuation(&mut self, number: usize, line: &str, open: bool) {
        if self.state() == State::Skip {
            return;
        }
        let Some(entry) = self.entries.last_mut().filter(|_| open) else {
            return self.problem(number, "a continuation line with no entry above it");
        };
        if entry.section == Section::Revisions {
            return self.problem(number, "a revision is one line");
        }
        entry.lines.push(line.to_owned());
        self.open = true;
    }

    fn finish(mut self) -> (Vec<Entry>, Vec<Problem>) {
        self.close_pending();
        (self.entries, self.problems)
    }
}

/// Whether `text` names issue `issue` as `#<issue>`, not as part of a longer number or word.
fn names_issue(text: &str, issue: u64) -> bool {
    let needle = format!("#{issue}");
    text.match_indices(&needle).any(|(at, _)| {
        let before = text.get(..at).and_then(|head| head.chars().next_back());
        let after = text.get(at.saturating_add(needle.len())..).and_then(|tail| tail.chars().next());
        !before.is_some_and(char::is_alphanumeric) && !after.is_some_and(|c| c.is_ascii_digit())
    })
}

/// Whether `doc` is a relative path to a Markdown file that stays inside the workspace.
fn inside(doc: &str) -> bool {
    let path = Path::new(doc);
    doc.ends_with(".md") && path.components().all(|part| matches!(part, Component::Normal(_)))
}

/// `line` with its status dated `date` and its change set to `change`; `None` when it is not a status line.
fn bump(line: &str, date: &str, change: &str) -> Option<String> {
    let (head, rest) = line.split_once("**Last updated:** ")?;
    let (_, rest) = rest.split_once(" · ")?;
    let (middle, rest) = rest.split_once("**Change:** ")?;
    let tail = rest.find(" · ").and_then(|at| rest.get(at..)).unwrap_or_default();
    Some(format!("{head}**Last updated:** {date} · {middle}**Change:** {change}{tail}"))
}

/// Why the document `doc` cannot take a revision row, if it cannot.
fn revision_target(root: &Path, doc: &str) -> Option<String> {
    if !inside(doc) {
        return Some(format!("`{doc}` is not a Markdown file in the workspace"));
    }
    let Ok(text) = fs::read_to_string(root.join(doc)) else { return Some(format!("`{doc}` cannot be read")) };
    let lines: Vec<String> = text.lines().map(str::to_owned).collect();
    if section(&lines, HISTORY).and_then(|(start, end)| after_table(&lines, start, end)).is_none() {
        return Some(format!("`{doc}` has no `{HISTORY}` table"));
    }
    if !lines.iter().any(|line| bump(line, "", "").is_some()) {
        return Some(format!("`{doc}` has no `**Last updated:** ... · **Change:** ...` status line"));
    }
    None
}

/// The revision an entry stands for, or why it is not one.
fn revision(root: &Path, entry: &Entry) -> Result<Revision, String> {
    let text = entry.joined();
    let parts: Vec<&str> = text.split('|').map(str::trim).collect();
    let [doc, change, summary] = parts.as_slice() else { return Err("a revision is `- <doc> | <change> | <summary>`".to_owned()) };
    if doc.is_empty() || summary.is_empty() {
        return Err("a revision is `- <doc> | <change> | <summary>`".to_owned());
    }
    let change = CHANGES.iter().position(|known| known == change).ok_or_else(|| "the change is major, minor or patch".to_owned())?;
    if let Some(problem) = revision_target(root, doc) {
        return Err(problem);
    }
    Ok(Revision { doc: (*doc).to_owned(), change, summary: (*summary).to_owned() })
}

/// Reads fragment `issue`, checks it against the workspace at `root` and returns it with its problems.
fn read_fragment(root: &Path, issue: u64, text: &str) -> (Fragment, Vec<Problem>) {
    let mut parser = Parser::default();
    for (index, line) in text.lines().enumerate() {
        parser.line(index.saturating_add(1), line);
    }
    let (entries, mut problems) = parser.finish();
    let mut revisions = Vec::new();
    for entry in &entries {
        if !names_issue(&entry.lines.join(" "), issue) {
            problems.push((entry.line, format!("the entry does not name #{issue}")));
        }
        match entry.section {
            Section::Roadmap if entry.joined().contains('|') => problems.push((entry.line, "no `|` in a table row".to_owned())),
            Section::Revisions => match revision(root, entry) {
                Ok(revision) => revisions.push(revision),
                Err(problem) => problems.push((entry.line, problem)),
            },
            _ => {}
        }
    }
    for (section, doc, name) in [(Section::Roadmap, ROADMAP, "Roadmap"), (Section::Gaps, GAPS, "Gaps")] {
        let has_lines = entries.iter().any(|entry| entry.section == section);
        let has_row = entries.iter().any(|entry| entry.section == Section::Revisions && entry.joined().split('|').next().map(str::trim) == Some(doc));
        if has_lines && !has_row {
            problems.push((0, format!("the {name} lines need a `{doc}` row under `## Revisions`")));
        }
    }
    if entries.is_empty() {
        problems.push((0, "the fragment says nothing".to_owned()));
    }
    (Fragment { issue, entries, revisions }, problems)
}

/// The issue a fragment file name stands for: `<issue>.md`, the number without leading zeros.
fn issue_of(name: &str) -> Option<u64> {
    let digits = name.strip_suffix(".md")?;
    let plain = !digits.is_empty() && !digits.starts_with('0') && digits.bytes().all(|byte| byte.is_ascii_digit());
    plain.then(|| digits.parse().ok()).flatten()
}

/// Reads at most [`MAX_BYTES`] and one byte more of `path`.
fn read_capped(path: &Path) -> Result<Vec<u8>, Error> {
    let file = fs::File::open(path).map_err(|source| Error::Io { path: path.to_path_buf(), source })?;
    let mut bytes = Vec::new();
    file.take(MAX_BYTES.saturating_add(1)).read_to_end(&mut bytes).map_err(|source| Error::Io { path: path.to_path_buf(), source })?;
    Ok(bytes)
}

/// The fragments in `changes/` sorted by issue, and the problems by file.
struct Found {
    fragments: Vec<Fragment>,
    /// The file, relative to the root, and its problems.
    problems: Vec<(String, Vec<Problem>)>,
}

fn find(root: &Path) -> Result<Found, Error> {
    let dir = root.join(DIR);
    let mut found = Found { fragments: Vec::new(), problems: Vec::new() };
    let listing = match fs::read_dir(&dir) {
        Ok(listing) => listing,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(found),
        Err(source) => return Err(Error::Io { path: dir, source }),
    };
    let mut names = Vec::new();
    for entry in listing {
        let entry = entry.map_err(|source| Error::Io { path: dir.clone(), source })?;
        let is_file = entry.file_type().map_err(|source| Error::Io { path: entry.path(), source })?.is_file();
        names.push((entry.file_name().to_string_lossy().into_owned(), is_file));
    }
    names.sort();
    for (name, is_file) in names {
        let rel = format!("{DIR}/{name}");
        let Some(issue) = issue_of(&name).filter(|_| is_file) else {
            found.problems.push((rel, vec![(0, "not a fragment: name it `<issue>.md`, the issue number without leading zeros".to_owned())]));
            continue;
        };
        let bytes = read_capped(&dir.join(&name))?;
        let text = match String::from_utf8(bytes) {
            Ok(text) if text.len() as u64 > MAX_BYTES => Err(format!("larger than {MAX_BYTES} bytes")),
            Ok(text) => Ok(text),
            Err(_) => Err("not UTF-8".to_owned()),
        };
        let (fragment, problems) = match text {
            Ok(text) => read_fragment(root, issue, &text),
            Err(problem) => {
                found.problems.push((rel, vec![(0, problem)]));
                continue;
            }
        };
        if problems.is_empty() {
            found.fragments.push(fragment);
        } else {
            found.problems.push((rel, problems));
        }
    }
    found.fragments.sort_by_key(|fragment| fragment.issue);
    Ok(found)
}

/// Checks the fragments of the workspace at `root`, or with `fold` writes them into the documents dated `date` and
/// deletes them; reports to `out` and returns whether all went well.
pub(crate) fn gate(root: &Path, fold: bool, date: &str, out: &mut dyn Write) -> Result<bool, Error> {
    let found = find(root)?;
    if !found.problems.is_empty() {
        let mut total = 0_usize;
        for (file, problems) in &found.problems {
            for (line, message) in problems {
                total = total.saturating_add(1);
                if *line == 0 {
                    writeln!(out, "changes: {file}: {message}")?;
                } else {
                    writeln!(out, "changes: {file}:{line}: {message}")?;
                }
            }
        }
        writeln!(out, "changes: {} in {}", count(total, "problem"), count(found.problems.len(), "file"))?;
        if fold {
            writeln!(out, "changes: nothing folded")?;
        }
        return Ok(false);
    }
    match (found.fragments.is_empty(), fold) {
        (true, false) => writeln!(out, "changes: no fragments")?,
        (true, true) => writeln!(out, "changes: no fragments to fold")?,
        (false, false) => writeln!(out, "changes: {} checked", count(found.fragments.len(), "fragment"))?,
        (false, true) => return fold_all(root, &found.fragments, date, out),
    }
    Ok(true)
}

/// The documents a fold changes, by path relative to the root, as lines.
struct Docs<'a> {
    root: &'a Path,
    docs: BTreeMap<String, Vec<String>>,
}

impl Docs<'_> {
    fn get(&mut self, rel: &str) -> Result<&mut Vec<String>, Error> {
        Ok(match self.docs.entry(rel.to_owned()) {
            Slot::Occupied(slot) => slot.into_mut(),
            Slot::Vacant(slot) => {
                let path = self.root.join(rel);
                let text = fs::read_to_string(&path).map_err(|source| Error::Io { path, source })?;
                slot.insert(text.lines().map(str::to_owned).collect())
            }
        })
    }
}

/// The first index in `from..to` whose line passes `test`.
fn position(lines: &[String], from: usize, to: usize, test: impl Fn(&str) -> bool) -> Option<usize> {
    let found = lines.get(from..to)?.iter().position(|line| test(line))?;
    Some(from.saturating_add(found))
}

/// The lines of the section that starts with `heading`: its heading line and the line after its end.
fn section(lines: &[String], heading: &str) -> Option<(usize, usize)> {
    let start = position(lines, 0, lines.len(), |line| line == heading)?;
    let end = position(lines, start.saturating_add(1), lines.len(), |line| line.starts_with("## ")).unwrap_or(lines.len());
    Some((start, end))
}

/// The index after the last line with text in `start..end`.
fn after_text(lines: &[String], start: usize, end: usize) -> usize {
    let last = lines.get(start..end).and_then(|part| part.iter().rposition(|line| !line.trim().is_empty())).unwrap_or_default();
    start.saturating_add(last).saturating_add(1)
}

/// The index after the last table row in `start..end`, if there is a table.
fn after_table(lines: &[String], start: usize, end: usize) -> Option<usize> {
    let last = lines.get(start..end)?.iter().rposition(|line| line.starts_with('|'))?;
    Some(start.saturating_add(last).saturating_add(1))
}

fn insert(lines: &mut Vec<String>, at: usize, new: Vec<String>) {
    let at = at.min(lines.len());
    lines.splice(at..at, new);
}

/// Puts a Changelog entry under its kind in `## [Unreleased]`, adding the kind in its place when it is missing.
fn add_changelog(lines: &mut Vec<String>, kind: usize, entry: &[String]) -> Result<(), String> {
    let (start, end) = section(lines, UNRELEASED).ok_or_else(|| format!("{CHANGELOG}: no `{UNRELEASED}` section"))?;
    let name = KINDS.get(kind).copied().unwrap_or_default();
    let heading = format!("### {name}");
    if let Some(at) = position(lines, start, end, |line| line == heading) {
        let stop = position(lines, at.saturating_add(1), end, |line| line.starts_with("### ")).unwrap_or(end);
        insert(lines, after_text(lines, at, stop), entry.to_vec());
        return Ok(());
    }
    let later = KINDS.get(kind.saturating_add(1)..).unwrap_or_default();
    let next = position(lines, start, end, |line| later.iter().any(|kind| line.strip_prefix("### ") == Some(kind)));
    let mut block = vec![heading];
    block.extend_from_slice(entry);
    match next {
        Some(at) => {
            block.push(String::new());
            insert(lines, at, block);
        }
        None => {
            block.insert(0, String::new());
            insert(lines, after_text(lines, start, end), block);
        }
    }
    Ok(())
}

/// Adds `row` after the table of the section `heading` of `doc`.
fn add_row(lines: &mut Vec<String>, doc: &str, heading: &str, row: String) -> Result<(), String> {
    let at = section(lines, heading).and_then(|(start, end)| after_table(lines, start, end)).ok_or_else(|| format!("{doc}: no `{heading}` table"))?;
    insert(lines, at, vec![row]);
    Ok(())
}

/// Adds `entry` at the end of the list in the rough edges section of `docs/gaps.md`.
fn add_gap(lines: &mut Vec<String>, entry: &[String]) -> Result<(), String> {
    let (start, end) = section(lines, ROUGH).ok_or_else(|| format!("{GAPS}: no `{ROUGH}` section"))?;
    insert(lines, after_text(lines, start, end), entry.to_vec());
    Ok(())
}

/// Folds one fragment into `docs`; the error names the document that lacks its place.
fn fold_one(docs: &mut Docs<'_>, fragment: &Fragment, date: &str) -> Result<Result<(), String>, Error> {
    for entry in &fragment.entries {
        let done = match entry.section {
            Section::Changelog => add_changelog(docs.get(CHANGELOG)?, entry.kind, &entry.lines),
            Section::Roadmap => add_row(docs.get(ROADMAP)?, ROADMAP, PROGRESS, format!("| {date} | {} |", entry.joined())),
            Section::Gaps => add_gap(docs.get(GAPS)?, &entry.lines),
            Section::Revisions => Ok(()),
        };
        if done.is_err() {
            return Ok(done);
        }
    }
    for revision in &fragment.revisions {
        let change = CHANGES.get(revision.change).copied().unwrap_or_default();
        let done = add_row(docs.get(&revision.doc)?, &revision.doc, HISTORY, format!("| {date} | {change} | {} |", revision.summary));
        if done.is_err() {
            return Ok(done);
        }
    }
    Ok(Ok(()))
}

/// Dates the status line of `lines` and gives it `change`.
fn bump_status(lines: &mut [String], date: &str, change: &str) {
    if let Some(line) = lines.iter_mut().find(|line| bump(line, date, change).is_some()) {
        *line = bump(line, date, change).unwrap_or_default();
    }
}

fn fold_all(root: &Path, fragments: &[Fragment], date: &str, out: &mut dyn Write) -> Result<bool, Error> {
    let mut docs = Docs { root, docs: BTreeMap::new() };
    for fragment in fragments {
        if let Err(problem) = fold_one(&mut docs, fragment, date)? {
            writeln!(out, "changes: {problem}")?;
            writeln!(out, "changes: nothing folded")?;
            return Ok(false);
        }
    }
    let mut largest: BTreeMap<&str, usize> = BTreeMap::new();
    for revision in fragments.iter().flat_map(|fragment| &fragment.revisions) {
        let change = largest.entry(&revision.doc).or_insert(revision.change);
        *change = (*change).min(revision.change);
    }
    for (doc, change) in largest {
        bump_status(docs.get(doc)?, date, CHANGES.get(change).copied().unwrap_or_default());
    }
    for (rel, lines) in &docs.docs {
        let path = root.join(rel);
        fs::write(&path, format!("{}\n", lines.join("\n"))).map_err(|source| Error::Io { path, source })?;
    }
    for fragment in fragments {
        let path = root.join(DIR).join(format!("{}.md", fragment.issue));
        fs::remove_file(&path).map_err(|source| Error::Io { path, source })?;
    }
    let issues: Vec<String> = fragments.iter().map(|fragment| format!("#{}", fragment.issue)).collect();
    let names: Vec<&str> = docs.docs.keys().map(String::as_str).collect();
    writeln!(out, "changes: folded {} ({}) into {}", count(fragments.len(), "fragment"), issues.join(", "), names.join(", "))?;
    Ok(true)
}

/// The UTC date of `at` as `YYYY-MM-DD`.
pub(crate) fn date_of(at: Timestamp) -> String {
    let (year, month, day) = at.to_civil(Offset::UTC).map_or((0, 1, 1), |date| (date.year(), date.month(), date.day()));
    format!("{year:04}-{month:02}-{day:02}")
}

/// Today in UTC as `YYYY-MM-DD`.
pub(crate) fn today() -> String {
    date_of(SystemClock::new().now())
}

#[cfg(test)]
#[path = "changes_tests.rs"]
mod tests;
