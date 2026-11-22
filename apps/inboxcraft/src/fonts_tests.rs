// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use eframe::egui;
use inboxcraft_ui_egui::theme::Fallback;

use super::{Fallbacks, Found, LIMITS, Limits, MAX_COLLECTION, MAX_FILE, MAX_PROBES, Search, search_folders, search_with};

const PLEX_SANS: &[u8] = include_bytes!("../../../assets/fonts/plex-sans/IBMPlexSans-Regular.ttf");
const PLEX_MONO: &[u8] = include_bytes!("../../../assets/fonts/plex-mono/IBMPlexMono-Regular.ttf");
const AUDIOWIDE: &[u8] = include_bytes!("../../../assets/fonts/audiowide/Audiowide-Regular.ttf");

const LATIN: (&str, char) = ("latin", 'A');

/// The faces chosen in the font files of `dir` only, never the computer's.
fn search_in(dir: &Path, missing: &[(&str, char)]) -> Vec<Fallback> {
    search_folders(&[dir.to_owned()], None, missing, &LIMITS).fallbacks
}

/// A folder holding the shipped Plex Sans and Plex Mono.
fn plex_folder() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("IBMPlexSans-Regular.ttf"), PLEX_SANS).unwrap();
    std::fs::write(dir.path().join("IBMPlexMono-Regular.ttf"), PLEX_MONO).unwrap();
    dir
}

/// A face of Plex Sans under the family `family` at `weight` and `style`.
fn face(family: &str, weight: u16, style: fontdb::Style) -> fontdb::FaceInfo {
    fontdb::FaceInfo {
        id: fontdb::ID::dummy(),
        source: fontdb::Source::Binary(Arc::new(PLEX_SANS)),
        index: 0,
        families: vec![(family.to_owned(), fontdb::Language::English_UnitedStates)],
        post_script_name: family.replace(' ', ""),
        style,
        weight: fontdb::Weight(weight),
        stretch: fontdb::Stretch::Normal,
        monospaced: false,
    }
}

/// The names chosen among `faces` (all Plex Sans underneath), with `sans` the system's sans-serif.
fn chosen(faces: &[fontdb::FaceInfo], sans: Option<&str>) -> Vec<String> {
    let mut search = Search::new(&[LATIN], sans, &LIMITS);
    let data = Arc::new(PLEX_SANS.to_vec());
    for face in faces {
        search.consider(face, &data);
    }
    search.finish().into_iter().map(|fallback| fallback.name).collect()
}

#[test]
fn the_first_face_with_the_glyph_is_chosen() {
    let dir = plex_folder();
    let fallbacks = search_in(dir.path(), &[LATIN]);
    assert_eq!(fallbacks.len(), 1, "one face is enough");
    let chosen = &fallbacks[0];
    assert!(chosen.name.starts_with("system: IBM Plex"), "{}", chosen.name);
    assert_eq!(chosen.index, 0);
    assert!(chosen.data == PLEX_SANS || chosen.data == PLEX_MONO, "the whole file");
}

#[test]
fn one_face_serves_several_probes() {
    let dir = plex_folder();
    assert_eq!(search_in(dir.path(), &[LATIN, ("digits", '7'), ("latin", 'é')]).len(), 1);
}

#[test]
fn a_probe_no_font_covers_gives_nothing() {
    let dir = plex_folder();
    assert!(search_in(dir.path(), &[("han", '日')]).is_empty());
    assert!(search_in(dir.path(), &[]).is_empty());
    assert!(search_folders(&[], None, &[LATIN], &LIMITS).fallbacks.is_empty());
}

#[test]
fn a_glyph_without_an_outline_is_not_enough() {
    // Plex maps the space to a glyph with no outline, as a colour emoji face maps its emoji: egui would draw nothing.
    let dir = plex_folder();
    assert!(search_in(dir.path(), &[("space", ' ')]).is_empty());
}

#[test]
fn a_file_over_the_size_limit_is_skipped() {
    let dir = tempfile::tempdir().unwrap();
    let mut big = PLEX_SANS.to_vec();
    big.resize(usize::try_from(MAX_FILE).unwrap() + 1, 0);
    std::fs::write(dir.path().join("big.ttf"), &big).unwrap();
    let searched = search_folders(&[dir.path().to_owned()], None, &[LATIN], &LIMITS);
    assert!(searched.fallbacks.is_empty());
    assert!(searched.reads.is_empty(), "not even read");
    let mut search = Search::new(&[LATIN], None, &LIMITS);
    search.offer(big);
    assert!(search.finish().is_empty(), "nor taken when given");
}

#[test]
fn the_faces_and_the_total_are_capped() {
    let dir = plex_folder();
    let size = u64::try_from(PLEX_SANS.len().max(PLEX_MONO.len())).unwrap();
    let within = |limits: Limits| search_folders(&[dir.path().to_owned()], None, &[LATIN], &limits).fallbacks;
    assert!(within(Limits { faces: 0, ..LIMITS }).is_empty());
    assert!(within(Limits { total: size / 2, ..LIMITS }).is_empty());
    assert!(within(Limits { file: 16, ..LIMITS }).is_empty());
    assert_eq!(within(Limits { faces: 1, total: size, ..LIMITS }).len(), 1);
}

#[test]
fn each_font_file_is_read_once() {
    let dir = plex_folder();
    std::fs::write(dir.path().join("Audiowide-Regular.ttf"), AUDIOWIDE).unwrap();
    std::fs::create_dir(dir.path().join("more")).unwrap();
    std::fs::write(dir.path().join("more").join("copy.otf"), PLEX_SANS).unwrap();
    std::fs::write(dir.path().join("notes.txt"), "not a font").unwrap();
    // A probe nothing covers keeps the walk going to the end; the chosen file is not read again.
    let searched = search_folders(&[dir.path().to_owned(), dir.path().join("more")], None, &[LATIN, ("han", '日')], &LIMITS);
    assert_eq!(searched.fallbacks.len(), 1);
    let mut reads = searched.reads.clone();
    reads.sort();
    reads.dedup();
    assert_eq!(reads.len(), searched.reads.len(), "no file twice: {:?}", searched.reads);
    assert_eq!(reads.len(), 4, "every font file once, nothing else: {reads:?}");
}

#[test]
fn junk_files_never_panic() {
    let dir = tempfile::tempdir().unwrap();
    let mut noise = Vec::new();
    let mut state: u32 = 0x9e37_79b9;
    for _ in 0..64 * 1024 {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        noise.push(state.to_le_bytes()[0]);
    }
    std::fs::write(dir.path().join("noise.ttf"), &noise).unwrap();
    std::fs::write(dir.path().join("empty.ttf"), b"").unwrap();
    std::fs::write(dir.path().join("cut.ttf"), &PLEX_SANS[..PLEX_SANS.len() / 3]).unwrap();
    // A collection claiming four billion faces: `fontdb`'s own walk would try every one of them.
    std::fs::write(dir.path().join("collection.ttc"), b"ttcf\0\x01\0\0\xff\xff\xff\xff").unwrap();
    std::fs::write(dir.path().join("short.TTC"), b"ttcf").unwrap();
    let mut many = b"ttcf\0\x01\0\0".to_vec();
    many.extend_from_slice(&(MAX_COLLECTION + 1).to_be_bytes());
    many.extend_from_slice(PLEX_SANS);
    std::fs::write(dir.path().join("many.otc"), &many).unwrap();
    std::fs::write(dir.path().join("not-a-font.txt"), PLEX_SANS).unwrap();
    std::fs::create_dir(dir.path().join("x.ttf")).unwrap();
    std::fs::create_dir(dir.path().join("loop")).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(dir.path(), dir.path().join("loop").join("back")).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(dir.path().join("plex.ttf"), dir.path().join("again.ttf")).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink("/dev/zero", dir.path().join("zero.ttf")).unwrap();
    std::fs::write(dir.path().join("plex.ttf"), PLEX_SANS).unwrap();
    let searched = search_folders(&[dir.path().to_owned()], None, &[("han", '日'), LATIN, ("emoji", '🙂')], &LIMITS);
    assert_eq!(searched.fallbacks.len(), 1, "only Plex is a font");
    assert_eq!(searched.fallbacks[0].data, PLEX_SANS);
    assert_eq!(searched.reads.iter().filter(|path| path.ends_with("plex.ttf")).count(), 1, "Plex once, through the link too");
    // A face with a nonsense index, and noise given as a font.
    let mut search = Search::new(&[LATIN], None, &LIMITS);
    search.consider(&fontdb::FaceInfo { index: 7, ..face("Noto Sans Far", 400, fontdb::Style::Normal) }, &Arc::new(PLEX_SANS.to_vec()));
    search.offer(noise);
    search.offer(Vec::new());
    assert!(search.finish().is_empty());
}

#[test]
fn noto_sans_comes_first_then_the_sans_serif_then_regular_and_upright() {
    let normal = fontdb::Style::Normal;
    let faces = [
        face("Some Serif", 400, normal),
        face("Owner Sans", 400, normal),
        face("Noto Sans Bold Only", 700, normal),
        face("Noto Sans Italic", 400, fontdb::Style::Italic),
        face("Noto Sans Test", 400, normal),
    ];
    assert_eq!(chosen(&faces, Some("Owner Sans")), ["system: Noto Sans Test"]);
    let faces = [face("Some Serif", 400, normal), face("Owner Sans", 700, normal), face("Owner Sans", 400, normal)];
    assert_eq!(chosen(&faces, Some("owner sans")), ["system: Owner Sans"]);
    let faces = [face("Some Serif", 400, normal), face("Other Serif", 400, normal)];
    assert_eq!(chosen(&faces, None), ["system: Some Serif"], "else the first found");
}

#[test]
fn a_long_name_is_cut() {
    let long = "Noto Sans ".to_owned() + &"x".repeat(500);
    let names = chosen(&[face(&long, 400, fontdb::Style::Normal)], None);
    assert_eq!(names.len(), 1);
    assert!(names[0].chars().count() <= 80, "{}", names[0]);
}

#[cfg(all(unix, not(any(target_os = "macos", target_os = "ios", target_os = "android"))))]
#[test]
fn a_remap_dir_s_folder_is_searched() {
    // How a Flatpak names the host's fonts: `as-path` is the name in fontconfig's cache, the text the folder.
    let fonts = plex_folder();
    let config = tempfile::tempdir().unwrap();
    let file = config.path().join("font-dirs.xml");
    let remap = format!("<fontconfig><remap-dir as-path=\"/usr/share/fonts\">{}</remap-dir></fontconfig>", fonts.path().display());
    std::fs::write(&file, remap).unwrap();
    let found = crate::fontconfig::read(&[file], &crate::fontconfig::Places::default());
    assert_eq!(found.folders, [fonts.path().to_owned()]);
    let searched = search_folders(&found.folders, found.sans.as_deref(), &[LATIN], &LIMITS);
    assert_eq!(searched.fallbacks.len(), 1);
    assert!(searched.reads.iter().all(|read| read.starts_with(fonts.path().canonicalize().unwrap_or_else(|_| PathBuf::new()))));
}

#[test]
fn the_fallbacks_go_in_after_the_app_s_first_pass_and_once() {
    let ctx = egui::Context::default();
    let found = Arc::new(Found::default());
    let mut fallbacks = Fallbacks::new(Arc::clone(&found));
    let last =
        |ctx: &egui::Context| ctx.fonts(|fonts| fonts.definitions().families.get(&egui::FontFamily::Proportional).and_then(|f| f.last()).cloned());
    let pass = |ctx: &egui::Context, fallbacks: &mut Fallbacks| {
        ctx.run_ui(egui::RawInput::default(), |ui| {
            fallbacks.install(ui.ctx());
            if ui.ctx().cumulative_pass_nr() == 0 {
                // What the app does on its first frame.
                inboxcraft_ui_egui::theme::install_fonts(ui.ctx());
            }
        })
        .textures_delta
        .clear();
    };
    // Passes until the context rests; it must within a few.
    let settle = |ctx: &egui::Context, fallbacks: &mut Fallbacks| {
        for _ in 0..8 {
            if !ctx.has_requested_repaint() {
                return;
            }
            pass(ctx, fallbacks);
        }
        panic!("the context never rests");
    };
    pass(&ctx, &mut fallbacks);
    settle(&ctx, &mut fallbacks);
    assert_eq!(
        last(&ctx).as_deref(),
        Some(inboxcraft_ui_egui::theme::SHIPPED_FALLBACKS[4]),
        "nothing found, nothing installed: the shipped emoji face is last"
    );
    assert!(!found.put(Vec::new()), "nothing found stays waiting");
    let dir = plex_folder();
    let plex = search_in(dir.path(), &[LATIN]);
    let bytes = plex[0].data.as_ptr();
    assert!(found.put(plex));
    // Found before a window's first pass: installed on a later one, which that pass asks for.
    let early = egui::Context::default();
    let mut fallbacks_early = Fallbacks::new(Arc::clone(&found));
    pass(&early, &mut fallbacks_early);
    assert!(early.has_requested_repaint());
    settle(&early, &mut fallbacks_early);
    assert!(last(&early).is_some_and(|name| name.starts_with("system: ")), "{:?}", last(&early));
    // Found while a window rests: the wake brings a pass, which installs them and asks for one more to draw them.
    pass(&ctx, &mut fallbacks);
    assert!(ctx.has_requested_repaint(), "one frame to draw with them");
    settle(&ctx, &mut fallbacks);
    let name = last(&ctx).unwrap();
    assert!(name.starts_with("system: "));
    for window in [&early, &ctx] {
        assert_eq!(window.fonts(|fonts| fonts.definitions().font_data[&name].bytes().as_ptr()), bytes, "one copy of the bytes, shared");
    }
    let installed = ctx.fonts(|fonts| fonts.definitions().font_data.len());
    pass(&ctx, &mut fallbacks);
    assert!(!ctx.has_requested_repaint(), "then the app rests: they go in once");
    assert_eq!(ctx.fonts(|fonts| fonts.definitions().font_data.len()), installed);
}

#[test]
fn the_walk_looks_at_a_limited_number_of_font_files() {
    let dir = tempfile::tempdir().unwrap();
    for at in 0..5 {
        std::fs::write(dir.path().join(format!("{at}.ttf")), b"").unwrap();
    }
    let searched = search_folders(&[dir.path().to_owned()], None, &[LATIN], &Limits { files: 2, ..LIMITS });
    assert_eq!(searched.reads.len(), 2);
    assert_eq!(LIMITS.files, 20_000);
}

#[test]
fn the_search_takes_every_probe_and_keeps_a_face_for_each() {
    // Every probe the app's fonts lack is looked for; one face each fits the cap, and a face's probes fit its bits.
    let probes = inboxcraft_ui_egui::theme::FALLBACK_PROBES;
    assert!(MAX_PROBES >= probes.len() && MAX_PROBES <= 32, "{MAX_PROBES}");
    assert!(LIMITS.faces >= inboxcraft_ui_egui::theme::missing_probes(&probes).len(), "{}", LIMITS.faces);
    assert_eq!((LIMITS.faces, LIMITS.total), (24, 96 * 1024 * 1024));
    // The last probe counts: Plex serves a Latin probe set after 31 others no font holds.
    let dir = plex_folder();
    let mut missing: Vec<(&str, char)> = (0..31).map(|at| ("none", char::from_u32(0xE000 + at).unwrap())).collect();
    missing.push(LATIN);
    assert_eq!(search_in(dir.path(), &missing).len(), 1, "the 32nd probe is looked for");
}

#[test]
fn probes_past_the_cap_are_not_looked_for() {
    // Forty probes: the search takes the first 32. Plex serves the first; the 40th, which Plex also holds, is not
    // looked for, and no probe past the 32nd reaches a bit (a shift past a `u32` panics in a debug build).
    let dir = plex_folder();
    let mut missing: Vec<(&str, char)> = vec![LATIN];
    missing.extend((0..38).map(|at| ("none", char::from_u32(0xE000 + at).unwrap())));
    missing.push(("latin", 'B'));
    assert_eq!(missing.len(), 40);
    assert_eq!(search_in(dir.path(), &missing).len(), 1, "one face, for the first probe");
    // Alone past the cap, the 40th finds no face.
    missing[0] = ("none", '\u{F8FF}');
    assert!(search_in(dir.path(), &missing).is_empty(), "a probe past the cap was looked for");
}

/// One frame of a window: the fallbacks first, as the app's `logic` does, the app's own fonts on its first pass.
fn window_frame(ctx: &egui::Context, fallbacks: &mut Fallbacks) {
    ctx.run_ui(egui::RawInput::default(), |ui| {
        fallbacks.install(ui.ctx());
        if ui.ctx().cumulative_pass_nr() == 0 {
            inboxcraft_ui_egui::theme::install_fonts(ui.ctx());
        }
        ui.label("Mail");
    })
    .textures_delta
    .clear();
}

/// The computer's faces installed in `ctx`.
fn system_faces(ctx: &egui::Context) -> usize {
    ctx.fonts(|fonts| fonts.definitions().font_data.keys().filter(|name| name.starts_with("system: ")).count())
}

#[test]
fn the_first_frame_does_not_wait_on_the_search() {
    let dir = plex_folder();
    let plex = search_in(dir.path(), &[LATIN]);
    let (release, held) = std::sync::mpsc::channel::<()>();
    let (called, done, woken) = (Arc::new(AtomicBool::new(false)), Arc::new(AtomicBool::new(false)), Arc::new(AtomicUsize::new(0)));
    let (c, d, w) = (Arc::clone(&called), Arc::clone(&done), Arc::clone(&woken));
    // The search is held until the test lets it go (or five seconds pass, so a mutation fails rather than hangs).
    let find = move || {
        c.store(true, Ordering::SeqCst);
        let _ = held.recv_timeout(Duration::from_secs(5));
        d.store(true, Ordering::SeqCst);
        plex
    };
    let found = search_with(find, move || {
        w.fetch_add(1, Ordering::SeqCst);
    });
    let ctx = egui::Context::default();
    let mut fallbacks = Fallbacks::new(Arc::clone(&found));
    window_frame(&ctx, &mut fallbacks);
    found.join();
    assert!(!called.load(Ordering::SeqCst), "the search started before the first frame was drawn");
    // The frames after it draw while the search is held.
    for _ in 0..3 {
        window_frame(&ctx, &mut fallbacks);
    }
    // The worker may not have reached the finding yet: give it time.
    for _ in 0..5000 {
        if called.load(Ordering::SeqCst) {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(called.load(Ordering::SeqCst), "the search starts once the first frame is drawn");
    assert!(!done.load(Ordering::SeqCst), "a frame waited on the search");
    assert_eq!(system_faces(&ctx), 0);
    release.send(()).unwrap();
    found.join();
    assert_eq!(woken.load(Ordering::SeqCst), 1, "a found face wakes the window");
    for _ in 0..4 {
        window_frame(&ctx, &mut fallbacks);
    }
    assert_eq!(system_faces(&ctx), 1, "the found faces arrive after the first frame");
}

#[test]
fn a_search_that_fails_or_panics_leaves_the_shipped_faces() {
    for panics in [false, true] {
        let woken = Arc::new(AtomicUsize::new(0));
        let w = Arc::clone(&woken);
        let find = move || {
            assert!(!panics, "a font file broke the search");
            Vec::new()
        };
        let found = search_with(find, move || {
            w.fetch_add(1, Ordering::SeqCst);
        });
        let ctx = egui::Context::default();
        let mut fallbacks = Fallbacks::new(Arc::clone(&found));
        for _ in 0..2 {
            window_frame(&ctx, &mut fallbacks);
        }
        found.join();
        assert_eq!(found.failed(), panics, "a panic is caught and told, panics: {panics}");
        for _ in 0..4 {
            window_frame(&ctx, &mut fallbacks);
        }
        assert_eq!(system_faces(&ctx), 0, "panics: {panics}");
        let last = ctx.fonts(|fonts| fonts.definitions().families.get(&egui::FontFamily::Proportional).and_then(|f| f.last()).cloned());
        assert_eq!(last.as_deref(), Some(inboxcraft_ui_egui::theme::SHIPPED_FALLBACKS[4]), "the shipped faces stay, panics: {panics}");
        assert_eq!(woken.load(Ordering::SeqCst), 0, "nothing to wake for, panics: {panics}");
    }
}
