// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The computer's own fonts for the scripts IBM Plex and egui's bundled faces lack (Chinese, Japanese, Korean,
//! Arabic, Hebrew, Thai, Devanagari, emoji): found at start on a worker thread and placed behind Plex
//! ([`inboxcraft_ui_egui::theme::install_fallbacks`]). No font ships for them; where the computer has none, the boxes
//! stay.
//!
//! The search walks the system's font folders (on Linux those fontconfig's files name, so a Flatpak sees the host's
//! fonts through its runtime) and keeps what it finds in a `fontdb` database. It walks them itself rather than with
//! `fontdb`'s own walk, which loops over, and in a debug build panics on, a collection claiming billions of faces.
//! Every file is hostile: one over [`MAX_FILE`] is skipped before it is read, a collection of more than
//! [`MAX_COLLECTION`] faces is skipped, at most eight faces and 96 MiB are kept, and a file that cannot be read or
//! parsed is passed over. A face counts for a sign only when it has an outline for it: egui draws outlines, not the
//! colour bitmaps or layers of emoji faces.
//! The headless pictures, the tests and the CLI never search.

use std::collections::HashSet;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

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
}

/// The limits of the search at start.
pub(crate) const LIMITS: Limits = Limits { file: MAX_FILE, total: 96 * 1024 * 1024, faces: 8 };

/// The most faces a collection file may claim: a font collection holds a few dozen at most.
pub(crate) const MAX_COLLECTION: u32 = 256;

/// How deep the walk goes below a font folder.
const MAX_DEPTH: usize = 16;

/// The most folder entries the walk looks at, in all folders together.
const MAX_ENTRIES: usize = 100_000;

/// The most probes one search takes (one bit each).
const MAX_PROBES: usize = 32;

/// The longest family name kept in a fallback's name, in characters.
const MAX_NAME: usize = 64;

/// The faces on the computer for the `missing` probes: for each probe in order, the first face (in order of
/// preference) with an outline for its sign, unless a face already chosen has one; one face may serve several probes.
/// Preferred are faces whose family starts with `Noto Sans`, then the family the system names sans-serif, then the
/// order found; within those, upright before slanted and the weight nearest regular.
#[must_use]
pub fn system_fallbacks(missing: &[(&str, char)]) -> Vec<Fallback> {
    if missing.is_empty() {
        return Vec::new();
    }
    let (folders, sans) = system_folders();
    let mut db = fontdb::Database::new();
    if let Some(sans) = sans {
        db.set_sans_serif_family(sans);
    }
    load_folders(&mut db, &folders);
    fallbacks_in(&db, missing)
}

/// Puts the faces of the font files (`.ttf`, `.otf`, `.ttc`, `.otc`) in and below `folders` into `db`, each file once,
/// following links; a file too large, a collection of too many faces and a file that does not parse are passed over.
/// The faces keep their file's path: their data is read again only when one is chosen.
pub(crate) fn load_folders(db: &mut fontdb::Database, folders: &[PathBuf]) {
    let mut walk = Walk { seen: HashSet::new(), entries: 0 };
    for folder in folders {
        walk.folder(db, folder, 0);
    }
}

/// A walk through the font folders: what it has seen (folders and files, by their real path) and how many entries.
struct Walk {
    seen: HashSet<PathBuf>,
    entries: usize,
}

impl Walk {
    fn folder(&mut self, db: &mut fontdb::Database, folder: &Path, depth: usize) {
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
                self.folder(db, &path, depth.saturating_add(1));
            } else if meta.is_file() && is_font_file(&path) && meta.len() <= MAX_FILE {
                let Ok(real) = std::fs::canonicalize(&path) else { continue };
                if self.seen.insert(real.clone()) {
                    load_file(db, &real);
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

/// Puts the faces of the font file at `path` into `db`: parsed from the bytes read once, so the file cannot change
/// between the checks and the parse.
fn load_file(db: &mut fontdb::Database, path: &Path) {
    let Some(data) = read_file(path, MAX_FILE) else { return };
    if collection_size(&data) > MAX_COLLECTION {
        return;
    }
    let mut parsed = fontdb::Database::new();
    parsed.load_font_source(fontdb::Source::Binary(Arc::new(data)));
    for face in parsed.faces() {
        db.push_face_info(fontdb::FaceInfo { source: fontdb::Source::File(path.to_owned()), ..face.clone() });
    }
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
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let mut config = fontconfig_parser::FontConfig::default();
    if let Some(file) = std::env::var_os("FONTCONFIG_FILE") {
        let _ = config.merge_config(Path::new(&file));
    } else {
        let user = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from).or_else(|| home.as_ref().map(|home| home.join(".config")));
        let without_user = user.is_none_or(|user| config.merge_config(&user.join("fontconfig/fonts.conf")).is_err());
        if without_user {
            let _ = config.merge_config(Path::new("/etc/fonts/local.conf"));
        }
        let _ = config.merge_config(Path::new("/etc/fonts/fonts.conf"));
    }
    let sans = config
        .aliases
        .iter()
        .filter(|alias| ["sans-serif", "sans serif"].iter().any(|name| alias.alias.eq_ignore_ascii_case(name)))
        .find_map(|alias| alias.prefer.first().or_else(|| alias.accept.first()).or_else(|| alias.default.first()).cloned());
    let mut folders: Vec<PathBuf> = config
        .dirs
        .into_iter()
        .filter_map(|dir| match dir.path.strip_prefix("~") {
            Ok(rest) => home.as_ref().map(|home| home.join(rest)),
            Err(_) => Some(dir.path),
        })
        .collect();
    if folders.is_empty() {
        folders = vec![PathBuf::from("/usr/share/fonts"), PathBuf::from("/usr/local/share/fonts")];
        if let Some(home) = home {
            folders.extend([home.join(".fonts"), home.join(".local/share/fonts")]);
        }
    }
    (folders, sans)
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

/// [`system_fallbacks`] in the faces of `db`.
pub(crate) fn fallbacks_in(db: &fontdb::Database, missing: &[(&str, char)]) -> Vec<Fallback> {
    fallbacks_within(db, missing, &LIMITS)
}

/// [`fallbacks_in`] within `limits`.
pub(crate) fn fallbacks_within(db: &fontdb::Database, missing: &[(&str, char)], limits: &Limits) -> Vec<Fallback> {
    let signs: Vec<char> = missing.iter().map(|(_, sign)| *sign).take(MAX_PROBES).collect();
    if signs.is_empty() || limits.faces == 0 {
        return Vec::new();
    }
    let sans = db.family_name(&fontdb::Family::SansSerif).to_owned();
    let mut faces: Vec<&fontdb::FaceInfo> = db.faces().collect();
    faces.sort_by_key(|face| preference(face, &sans));
    let covering = coverage_of(&faces, &signs, limits.file);
    // For each probe the first face that covers it, unless one chosen already does.
    let mut chosen: Vec<(&fontdb::FaceInfo, u32)> = Vec::new();
    for bit in (0..signs.len()).map(|at| 1_u32 << at) {
        if chosen.iter().any(|(_, mask)| mask & bit != 0) {
            continue;
        }
        if let Some(found) = covering.iter().find(|(_, mask)| mask & bit != 0) {
            chosen.push(*found);
        }
    }
    let mut kept: Vec<Fallback> = Vec::new();
    let mut total: u64 = 0;
    for (face, _) in chosen {
        if kept.len() >= limits.faces {
            break;
        }
        let Some(data) = read(&face.source, limits.file) else { continue };
        let size = u64::try_from(data.len()).unwrap_or(u64::MAX);
        if total.saturating_add(size) > limits.total {
            continue;
        }
        total = total.saturating_add(size);
        let name = unique_name(face, &kept);
        let data = Arc::try_unwrap(data).unwrap_or_else(|shared| shared.as_ref().clone());
        kept.push(Fallback { name, data, index: face.index });
    }
    kept
}

/// The faces with an outline for one of `signs` or more, in the order of `faces`, each with its signs as bits; the
/// walk stops once every sign has a face.
fn coverage_of<'a>(faces: &[&'a fontdb::FaceInfo], signs: &[char], file: u64) -> Vec<(&'a fontdb::FaceInfo, u32)> {
    let all = signs.iter().enumerate().fold(0_u32, |all, (at, _)| all | (1 << at));
    let mut found = 0_u32;
    let mut covering = Vec::new();
    // The file read last: the faces of a collection share one.
    let mut last: Option<(PathBuf, Arc<Vec<u8>>)> = None;
    for face in faces {
        if found == all {
            break;
        }
        let data = match &face.source {
            fontdb::Source::File(path) => match &last {
                Some((read_path, data)) if read_path == path => Some(Arc::clone(data)),
                _ => {
                    let data = read_file(path, file).map(Arc::new);
                    last = data.as_ref().map(|data| (path.clone(), Arc::clone(data)));
                    data
                }
            },
            source => read(source, file),
        };
        let Some(data) = data else { continue };
        let mask = outlines(&data, face.index, signs);
        if mask != 0 {
            found |= mask;
            covering.push((*face, mask));
        }
    }
    covering
}

/// How much `face` is preferred, least first: `Noto Sans`, then the `sans` family, then the rest; upright before
/// slanted; the weight nearest regular.
fn preference(face: &fontdb::FaceInfo, sans: &str) -> (u8, bool, u16) {
    let names = || face.families.iter().map(|(name, _)| name.as_str());
    let tier = if names().any(|name| name.starts_with("Noto Sans")) {
        0
    } else if names().any(|name| name.eq_ignore_ascii_case(sans)) {
        1
    } else {
        2
    };
    (tier, face.style != fontdb::Style::Normal, face.weight.0.abs_diff(fontdb::Weight::NORMAL.0))
}

/// The bytes of `source` if they are no larger than `file`.
fn read(source: &fontdb::Source, file: u64) -> Option<Arc<Vec<u8>>> {
    if let fontdb::Source::File(path) = source {
        read_file(path, file).map(Arc::new)
    } else if let fontdb::Source::Binary(data) = source {
        let data: &[u8] = (**data).as_ref();
        (u64::try_from(data.len()).ok()? <= file).then(|| Arc::new(data.to_vec()))
    } else {
        None
    }
}

/// The file at `path` if it is no larger than `file`: its size is looked at before it is read, and no more is read.
fn read_file(path: &Path, file: u64) -> Option<Vec<u8>> {
    let opened = std::fs::File::open(path).ok()?;
    let size = opened.metadata().ok()?.len();
    if size > file {
        return None;
    }
    let mut data = Vec::with_capacity(usize::try_from(size).ok()?);
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

/// `system: ` and the face's family (cut to [`MAX_NAME`] characters), with a number where `kept` has the name already.
fn unique_name(face: &fontdb::FaceInfo, kept: &[Fallback]) -> String {
    let family = face.families.first().map_or(face.post_script_name.as_str(), |(name, _)| name.as_str());
    let base = format!("system: {}", family.chars().take(MAX_NAME).collect::<String>());
    let mut name = base.clone();
    let mut number: usize = 1;
    while kept.iter().any(|fallback| fallback.name == name) {
        number = number.saturating_add(1);
        name = format!("{base} #{number}");
    }
    name
}

/// The fallbacks, once the search found any.
pub type Found = Arc<OnceLock<Vec<Fallback>>>;

/// Starts the search on a worker thread: the probes the app's fonts lack ([`theme::missing_probes`]), then
/// [`system_fallbacks`]. When it found any it puts them into the [`Found`] it returns and calls `wake`; when it found
/// none, or failed, nothing happens.
pub fn search(wake: impl Fn() + Send + 'static) -> Found {
    let found = Found::default();
    let into = Arc::clone(&found);
    let started = std::thread::Builder::new().name("inboxcraft-fonts".to_owned()).spawn(move || {
        let fallbacks = system_fallbacks(&theme::missing_probes(&theme::FALLBACK_PROBES));
        if !fallbacks.is_empty() && into.set(fallbacks).is_ok() {
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
    found: Found,
    installed: bool,
}

impl Fallbacks {
    /// Waits for `found`.
    #[must_use]
    pub fn new(found: Found) -> Self {
        Self { found, installed: false }
    }

    /// Installs the fallbacks in `ctx` once they are found, not on its first pass: the app installs its own fonts
    /// then, which would replace them. Found by then, they ask for the pass after it.
    pub fn install(&mut self, ctx: &egui::Context) {
        if self.installed {
            return;
        }
        let Some(fallbacks) = self.found.get() else { return };
        if ctx.cumulative_pass_nr() == 0 {
            ctx.request_repaint();
            return;
        }
        theme::install_fallbacks(ctx, fallbacks);
        self.installed = true;
    }
}

#[cfg(test)]
#[path = "fonts_tests.rs"]
mod tests;
