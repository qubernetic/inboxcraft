// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The computer's own fonts for the scripts neither IBM Plex, the shipped Noto faces (Hebrew, Arabic, Devanagari,
//! Thai, emoji) nor egui's bundled faces hold (Chinese, Japanese, Korean, the other Indic scripts, Sinhala, Lao,
//! Tibetan, Myanmar, Khmer, Ethiopic, Syriac, Thaana, Cherokee, Bopomofo: [`theme::FALLBACK_PROBES`]): found at start
//! on a worker thread and placed behind them ([`inboxcraft_ui_egui::theme::install_fallbacks`]). Where the computer
//! has none, the boxes stay.
//!
//! The search walks the system's font folders: on Linux those fontconfig's files name, `<remap-dir>` included, which
//! is how a Flatpak names the host's folders ([`crate::fontconfig`], read by the app itself and bounded). It walks
//! them itself rather than with `fontdb`'s own walk, which loops over, and in a debug build panics on, a collection
//! claiming billions of faces; `fontdb` only reads a face's names and style. Each font file is read at most once:
//! while the walk goes on, it keeps for each probe still open the best face so far with its file's bytes, and drops
//! a file's bytes as soon as it is no probe's best. Every file is hostile: one over [`MAX_FILE`] is skipped before it
//! is read, a collection of more than [`MAX_COLLECTION`] faces is skipped, at most 24 faces and 96 MiB are kept,
//! and a file that cannot be read or parsed is passed over. A face counts for a sign only when it has an outline for
//! it: egui draws outlines, not the colour bitmaps or layers of emoji faces.
//! The headless pictures, the tests and the CLI never search.

use std::collections::HashSet;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use eframe::egui;
use inboxcraft_ui_egui::theme::{self, Fallback};
use skrifa::MetadataProvider as _;
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::OutlinePen;

/// The largest font file read: 48 MiB.
pub(crate) const MAX_FILE: u64 = 48 * 1024 * 1024;

/// What the search keeps at most.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Limits {
    /// The largest file, in bytes.
    pub(crate) file: u64,
    /// All the kept files together, in bytes.
    pub(crate) total: u64,
    /// The number of faces.
    pub(crate) faces: usize,
    /// The font files looked at, in all folders together.
    pub(crate) files: usize,
}

/// The limits of the search at start.
pub(crate) const LIMITS: Limits = Limits { file: MAX_FILE, total: 96 * 1024 * 1024, faces: 24, files: 20_000 };

/// The most faces a collection file may claim: a font collection holds a few dozen at most.
pub(crate) const MAX_COLLECTION: u32 = 256;

/// How deep the walk goes below a font folder.
const MAX_DEPTH: usize = 16;

/// The most folder entries the walk looks at, in all folders together.
const MAX_ENTRIES: usize = 100_000;

/// The worker's stack: generous, so the XML parser has room to spare at the nesting the configuration files may
/// have (a defence beside the nesting count, which is what keeps a deep file away from the parser).
const WORKER_STACK: usize = 16 * 1024 * 1024;

/// The most probes one search takes: one face kept for each at most. A face's probes are bits of a `u32`.
pub(crate) const MAX_PROBES: usize = 32;

/// The longest family name kept in a fallback's name, in characters.
const MAX_NAME: usize = 64;

/// The faces on the computer for the `missing` probes: for each probe in order, the best face (in order of
/// preference) with an outline for its sign, unless a face already chosen has one; one face may serve several probes.
/// Preferred are faces whose family starts with `Noto Sans`, then the family the system names sans-serif, then the
/// order found; within those, upright before slanted and the weight nearest regular.
#[must_use]
pub fn system_fallbacks(missing: &[(&str, char)]) -> Vec<Fallback> {
    if missing.is_empty() {
        return Vec::new();
    }
    let (folders, sans) = system_folders();
    search_folders(&folders, sans.as_deref(), missing, &LIMITS).fallbacks
}

/// What a search found, and the font files it read.
pub(crate) struct Searched {
    /// The faces chosen.
    pub(crate) fallbacks: Vec<Fallback>,
    /// Every font file read, by real path, as often as it was read: what the tests count.
    #[cfg_attr(not(test), allow(dead_code, reason = "only the tests count the reads"))]
    pub(crate) reads: Vec<PathBuf>,
}

/// [`system_fallbacks`] in the font files (`.ttf`, `.otf`, `.ttc`, `.otc`) in and below `folders`, within `limits`,
/// with `sans` the family the system names sans-serif. The walk follows links, sees each folder and file once (so
/// link loops end), goes at most [`MAX_DEPTH`] deep, looks at most at [`MAX_ENTRIES`] entries and at most at
/// `limits.files` font files.
pub(crate) fn search_folders(folders: &[PathBuf], sans: Option<&str>, missing: &[(&str, char)], limits: &Limits) -> Searched {
    let mut search = Search::new(missing, sans, limits);
    let mut walk = Walk { seen: HashSet::new(), entries: 0, files: 0, reads: Vec::new() };
    if search.wants_any() {
        for folder in folders {
            walk.folder(&mut search, folder, 0);
        }
    }
    Searched { fallbacks: search.finish(), reads: walk.reads }
}

/// A walk through the font folders: what it has seen (folders and files, by their real path), how many entries, and
/// which files it read.
struct Walk {
    seen: HashSet<PathBuf>,
    entries: usize,
    files: usize,
    reads: Vec<PathBuf>,
}

impl Walk {
    fn folder(&mut self, search: &mut Search<'_>, folder: &Path, depth: usize) {
        if depth > MAX_DEPTH {
            return;
        }
        let Ok(real) = std::fs::canonicalize(folder) else { return };
        if !self.seen.insert(real.clone()) {
            return;
        }
        let Ok(entries) = std::fs::read_dir(&real) else { return };
        for entry in entries.flatten() {
            self.entries = self.entries.saturating_add(1);
            if self.entries > MAX_ENTRIES {
                return;
            }
            let path = entry.path();
            let Ok(meta) = std::fs::metadata(&path) else { continue };
            if meta.is_dir() {
                self.folder(search, &path, depth.saturating_add(1));
            } else if meta.is_file() && is_font_file(&path) && meta.len() <= search.limits.file {
                self.files = self.files.saturating_add(1);
                if self.files > search.limits.files {
                    return;
                }
                let Ok(real) = std::fs::canonicalize(&path) else { continue };
                if self.seen.insert(real.clone()) {
                    self.reads.push(real.clone());
                    if let Some(data) = read_file(&real, search.limits.file) {
                        search.offer(data);
                    }
                }
            }
        }
    }
}

/// Whether `path` names a font file by its extension.
fn is_font_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| ["ttf", "otf", "ttc", "otc"].iter().any(|font| extension.eq_ignore_ascii_case(font)))
}

/// The number of faces `data` claims if it is a font collection (`ttcf`), else 1.
fn collection_size(data: &[u8]) -> u32 {
    match (data.get(..4), data.get(8..12)) {
        (Some(b"ttcf"), Some(&[a, b, c, d])) => u32::from_be_bytes([a, b, c, d]),
        (Some(b"ttcf"), _) => 0,
        _ => 1,
    }
}

/// The folders the system keeps its fonts in, as fontconfig's files name them (else the usual ones), and the family
/// they name sans-serif.
#[cfg(all(unix, not(any(target_os = "macos", target_os = "ios", target_os = "android"))))]
fn system_folders() -> (Vec<PathBuf>, Option<String>) {
    let places = crate::fontconfig::Places::of_process();
    let top = std::env::var_os("FONTCONFIG_FILE").map_or_else(|| PathBuf::from("/etc/fonts/fonts.conf"), PathBuf::from);
    let found = crate::fontconfig::read(&[top], &places);
    let mut folders = found.folders;
    if folders.is_empty() {
        folders = vec![PathBuf::from("/usr/share/fonts"), PathBuf::from("/usr/local/share/fonts")];
        if let Some(home) = places.home {
            folders.extend([home.join(".fonts"), home.join(".local/share/fonts")]);
        }
    }
    (folders, found.sans)
}

/// The folders the system keeps its fonts in.
#[cfg(any(target_os = "macos", target_os = "ios"))]
fn system_folders() -> (Vec<PathBuf>, Option<String>) {
    let mut folders = vec![PathBuf::from("/Library/Fonts"), PathBuf::from("/System/Library/Fonts"), PathBuf::from("/Network/Library/Fonts")];
    if let Some(home) = std::env::var_os("HOME") {
        folders.push(PathBuf::from(home).join("Library/Fonts"));
    }
    (folders, None)
}

/// The folders the system keeps its fonts in.
#[cfg(target_os = "windows")]
fn system_folders() -> (Vec<PathBuf>, Option<String>) {
    let root = std::env::var_os("SYSTEMROOT").map_or_else(|| PathBuf::from("C:\\Windows"), PathBuf::from);
    let mut folders = vec![root.join("Fonts")];
    if let Some(profile) = std::env::var_os("USERPROFILE") {
        let profile = PathBuf::from(profile);
        folders.push(profile.join("AppData\\Local\\Microsoft\\Windows\\Fonts"));
        folders.push(profile.join("AppData\\Roaming\\Microsoft\\Windows\\Fonts"));
    }
    (folders, None)
}

/// No known font folders.
#[cfg(any(target_os = "android", not(any(unix, target_os = "windows"))))]
fn system_folders() -> (Vec<PathBuf>, Option<String>) {
    (Vec::new(), None)
}

/// A face that has an outline for one probe's sign or more, with its file's bytes.
struct Candidate {
    /// How much it is preferred, least first.
    rank: (u8, bool, u16, usize),
    /// The probes it serves, as bits.
    signs: u32,
    /// `system: ` and its family.
    name: String,
    /// Its index in the file.
    index: u32,
    /// The whole file, shared by the faces of one collection.
    data: Arc<Vec<u8>>,
}

/// The search's state: for each probe the best face found so far.
pub(crate) struct Search<'a> {
    signs: Vec<char>,
    sans: Option<&'a str>,
    limits: &'a Limits,
    best: Vec<Option<Arc<Candidate>>>,
    found: usize,
}

impl<'a> Search<'a> {
    /// A search for the `missing` probes (the first [`MAX_PROBES`]).
    pub(crate) fn new(missing: &[(&str, char)], sans: Option<&'a str>, limits: &'a Limits) -> Self {
        let signs: Vec<char> = missing.iter().map(|(_, sign)| *sign).take(MAX_PROBES).collect();
        let best = signs.iter().map(|_| None).collect();
        Self { signs, sans, limits, best, found: 0 }
    }

    /// Whether there is anything to look for.
    fn wants_any(&self) -> bool {
        !self.signs.is_empty() && self.limits.faces > 0
    }

    /// Looks at the faces of one font file, given whole. Its bytes stay only while a face of it is a probe's best.
    pub(crate) fn offer(&mut self, data: Vec<u8>) {
        if !self.wants_any() || u64::try_from(data.len()).unwrap_or(u64::MAX) > self.limits.file || collection_size(&data) > MAX_COLLECTION {
            return;
        }
        let data = Arc::new(data);
        let mut parsed = fontdb::Database::new();
        let shared: Arc<dyn AsRef<[u8]> + Send + Sync> = Arc::clone(&data) as _;
        parsed.load_font_source(fontdb::Source::Binary(shared));
        for face in parsed.faces() {
            self.consider(face, &data);
        }
    }

    /// Looks at `face` of the font file `data`.
    pub(crate) fn consider(&mut self, face: &fontdb::FaceInfo, data: &Arc<Vec<u8>>) {
        let signs = outlines(data, face.index, &self.signs);
        let order = self.found;
        self.found = self.found.saturating_add(1);
        if signs == 0 {
            return;
        }
        let candidate =
            Arc::new(Candidate { rank: preference(face, self.sans, order), signs, name: name_of(face), index: face.index, data: Arc::clone(data) });
        for at in 0..self.best.len() {
            if signs & (1 << at) == 0 {
                continue;
            }
            let better = self.best.get(at).and_then(Option::as_ref).is_none_or(|best| candidate.rank < best.rank);
            if better {
                let before = self.best.get_mut(at).and_then(|slot| slot.replace(Arc::clone(&candidate)));
                // Within the total: the bytes kept for all probes, each file once.
                if self.kept_bytes() > self.limits.total
                    && let Some(slot) = self.best.get_mut(at)
                {
                    *slot = before;
                }
            }
        }
    }

    /// The bytes the best faces hold, each file once.
    fn kept_bytes(&self) -> u64 {
        let mut files: Vec<&Arc<Vec<u8>>> = Vec::new();
        for candidate in self.best.iter().flatten() {
            if !files.iter().any(|file| Arc::ptr_eq(file, &candidate.data)) {
                files.push(&candidate.data);
            }
        }
        files.iter().map(|file| u64::try_from(file.len()).unwrap_or(u64::MAX)).fold(0, u64::saturating_add)
    }

    /// The faces chosen: for each probe in order its best face, unless one chosen already serves it; within the
    /// limits.
    pub(crate) fn finish(self) -> Vec<Fallback> {
        let mut chosen: Vec<Arc<Candidate>> = Vec::new();
        for (at, best) in self.best.iter().enumerate() {
            if chosen.iter().any(|candidate| candidate.signs & (1 << at) != 0) {
                continue;
            }
            if let Some(best) = best {
                chosen.push(Arc::clone(best));
            }
        }
        drop(self.best);
        let mut kept: Vec<Fallback> = Vec::new();
        let mut total: u64 = 0;
        for candidate in chosen {
            if kept.len() >= self.limits.faces {
                break;
            }
            let size = u64::try_from(candidate.data.len()).unwrap_or(u64::MAX);
            if total.saturating_add(size) > self.limits.total {
                continue;
            }
            total = total.saturating_add(size);
            let name = unique_name(&candidate.name, &kept);
            let index = candidate.index;
            let data = match Arc::try_unwrap(candidate) {
                Ok(candidate) => Arc::try_unwrap(candidate.data).unwrap_or_else(|shared| shared.as_ref().clone()),
                Err(shared) => shared.data.as_ref().clone(),
            };
            kept.push(Fallback { name, data, index });
        }
        kept
    }
}

/// How much `face` is preferred, least first: `Noto Sans`, then the `sans` family, then the rest; upright before
/// slanted; the weight nearest regular; then the order found.
fn preference(face: &fontdb::FaceInfo, sans: Option<&str>, order: usize) -> (u8, bool, u16, usize) {
    let names = || face.families.iter().map(|(name, _)| name.as_str());
    let tier = if names().any(|name| name.starts_with("Noto Sans")) {
        0
    } else if sans.is_some_and(|sans| names().any(|name| name.eq_ignore_ascii_case(sans))) {
        1
    } else {
        2
    };
    (tier, face.style != fontdb::Style::Normal, face.weight.0.abs_diff(fontdb::Weight::NORMAL.0), order)
}

/// The file at `path` if it is a regular file no larger than `file`: its type and size are looked at on the open file
/// before it is read, and no more than that is read.
fn read_file(path: &Path, file: u64) -> Option<Vec<u8>> {
    let opened = std::fs::File::open(path).ok()?;
    let meta = opened.metadata().ok()?;
    if !meta.is_file() || meta.len() > file {
        return None;
    }
    let mut data = Vec::with_capacity(usize::try_from(meta.len()).ok()?);
    opened.take(file.saturating_add(1)).read_to_end(&mut data).ok()?;
    (u64::try_from(data.len()).ok()? <= file).then_some(data)
}

/// The `signs` the face at `index` in `data` has an outline for, as bits; none where it does not parse.
fn outlines(data: &[u8], index: u32, signs: &[char]) -> u32 {
    let Ok(font) = skrifa::FontRef::from_index(data, index) else { return 0 };
    let (charmap, glyphs) = (font.charmap(), font.outline_glyphs());
    let mut mask = 0;
    for (at, sign) in signs.iter().enumerate().take(MAX_PROBES) {
        let Some(glyph) = charmap.map(*sign).and_then(|id| glyphs.get(id)) else { continue };
        let mut pen = Strokes(0);
        if glyph.draw((Size::unscaled(), LocationRef::default()), &mut pen).is_ok() && pen.0 > 0 {
            mask |= 1 << at;
        }
    }
    mask
}

/// Counts the strokes of an outline.
struct Strokes(usize);

impl OutlinePen for Strokes {
    fn move_to(&mut self, _x: f32, _y: f32) {}

    fn line_to(&mut self, _x: f32, _y: f32) {
        self.0 = self.0.saturating_add(1);
    }

    fn quad_to(&mut self, _cx0: f32, _cy0: f32, _x: f32, _y: f32) {
        self.0 = self.0.saturating_add(1);
    }

    fn curve_to(&mut self, _cx0: f32, _cy0: f32, _cx1: f32, _cy1: f32, _x: f32, _y: f32) {
        self.0 = self.0.saturating_add(1);
    }

    fn close(&mut self) {}
}

/// `system: ` and the face's family (cut to [`MAX_NAME`] characters).
fn name_of(face: &fontdb::FaceInfo) -> String {
    let family = face.families.first().map_or(face.post_script_name.as_str(), |(name, _)| name.as_str());
    format!("system: {}", family.chars().take(MAX_NAME).collect::<String>())
}

/// `base`, with a number where `kept` has the name already.
fn unique_name(base: &str, kept: &[Fallback]) -> String {
    let mut name = base.to_owned();
    let mut number: usize = 1;
    while kept.iter().any(|fallback| fallback.name == name) {
        number = number.saturating_add(1);
        name = format!("{base} #{number}");
    }
    name
}

/// The fallbacks of the search, shared by the worker and the windows.
#[derive(Default)]
pub struct Found(Mutex<Stage>);

/// Where the fallbacks are.
#[derive(Default)]
enum Stage {
    /// Not found (yet).
    #[default]
    Waiting,
    /// Found, not installed in a window yet.
    Found(Vec<Fallback>),
    /// Installed: the definitions, whose font bytes are shared, for a later window.
    Installed(egui::FontDefinitions),
}

impl Found {
    /// Hands over `fallbacks`; none leaves it waiting.
    pub(crate) fn put(&self, fallbacks: Vec<Fallback>) -> bool {
        if fallbacks.is_empty() {
            return false;
        }
        match self.0.lock() {
            Ok(mut stage) if matches!(*stage, Stage::Waiting) => {
                *stage = Stage::Found(fallbacks);
                true
            }
            _ => false,
        }
    }
}

/// Starts the search on a worker thread: the probes the app's fonts lack ([`theme::missing_probes`]), then
/// [`system_fallbacks`]. When it found any it puts them into the [`Found`] it returns and calls `wake`; when it found
/// none, or failed, nothing happens.
pub fn search(wake: impl Fn() + Send + 'static) -> Arc<Found> {
    let found = Arc::new(Found::default());
    let into = Arc::clone(&found);
    let started = std::thread::Builder::new().name("inboxcraft-fonts".to_owned()).stack_size(WORKER_STACK).spawn(move || {
        if into.put(system_fallbacks(&theme::missing_probes(&theme::FALLBACK_PROBES))) {
            wake();
        }
    });
    if let Err(error) = started {
        eprintln!("inboxcraft: the computer's fonts could not be searched: {error}");
    }
    found
}

/// Installs the found fallbacks in one window's context, once.
pub struct Fallbacks {
    found: Arc<Found>,
    installed: bool,
}

impl Fallbacks {
    /// Waits for `found`.
    #[must_use]
    pub fn new(found: Arc<Found>) -> Self {
        Self { found, installed: false }
    }

    /// Installs the fallbacks in `ctx` once they are found, not on its first pass: the app installs its own fonts
    /// then, which would replace them. Found by then, they ask for the pass after it. The first window takes their
    /// bytes without a copy; a later one shares them.
    pub fn install(&mut self, ctx: &egui::Context) {
        if self.installed {
            return;
        }
        let Ok(mut stage) = self.found.0.lock() else { return };
        if matches!(*stage, Stage::Waiting) {
            return;
        }
        if ctx.cumulative_pass_nr() == 0 {
            ctx.request_repaint();
            return;
        }
        match std::mem::take(&mut *stage) {
            Stage::Waiting => {}
            Stage::Found(fallbacks) => *stage = Stage::Installed(theme::install_fallbacks(ctx, fallbacks)),
            Stage::Installed(definitions) => {
                ctx.set_fonts(definitions.clone());
                ctx.request_repaint();
                *stage = Stage::Installed(definitions);
            }
        }
        self.installed = true;
    }
}

#[cfg(test)]
#[path = "fonts_tests.rs"]
mod tests;
