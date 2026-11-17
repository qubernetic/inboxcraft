// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::Path;
use std::sync::Arc;

use eframe::egui;

use super::{Fallbacks, Found, LIMITS, Limits, MAX_COLLECTION, MAX_FILE, fallbacks_in, fallbacks_within, load_folders};

const PLEX_SANS: &[u8] = include_bytes!("../../../assets/fonts/plex-sans/IBMPlexSans-Regular.ttf");
const PLEX_MONO: &[u8] = include_bytes!("../../../assets/fonts/plex-mono/IBMPlexMono-Regular.ttf");

const LATIN: (&str, char) = ("latin", 'A');

/// A database of the font files in `dir` only, never the computer's.
fn database_of(dir: &Path) -> fontdb::Database {
    let mut db = fontdb::Database::new();
    load_folders(&mut db, &[dir.to_owned()]);
    db
}

/// A folder holding the shipped Plex Sans and Plex Mono.
fn plex_folder() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("IBMPlexSans-Regular.ttf"), PLEX_SANS).unwrap();
    std::fs::write(dir.path().join("IBMPlexMono-Regular.ttf"), PLEX_MONO).unwrap();
    dir
}

/// A face of Plex Sans in memory, under the family `family` at `weight` and `style`.
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

#[test]
fn the_first_face_with_the_glyph_is_chosen() {
    let dir = plex_folder();
    let fallbacks = fallbacks_in(&database_of(dir.path()), &[LATIN]);
    assert_eq!(fallbacks.len(), 1, "one face is enough");
    let chosen = &fallbacks[0];
    assert!(chosen.name.starts_with("system: IBM Plex"), "{}", chosen.name);
    assert_eq!(chosen.index, 0);
    assert!(chosen.data == PLEX_SANS || chosen.data == PLEX_MONO, "the whole file");
}

#[test]
fn one_face_serves_several_probes() {
    let dir = plex_folder();
    let fallbacks = fallbacks_in(&database_of(dir.path()), &[LATIN, ("digits", '7'), ("latin", 'é')]);
    assert_eq!(fallbacks.len(), 1);
}

#[test]
fn a_probe_no_font_covers_gives_nothing() {
    let dir = plex_folder();
    assert!(fallbacks_in(&database_of(dir.path()), &[("han", '日')]).is_empty());
    assert!(fallbacks_in(&database_of(dir.path()), &[]).is_empty());
    assert!(fallbacks_in(&fontdb::Database::new(), &[LATIN]).is_empty());
}

#[test]
fn a_glyph_without_an_outline_is_not_enough() {
    // Plex maps the space to a glyph with no outline, as a colour emoji face maps its emoji: egui would draw nothing.
    let dir = plex_folder();
    assert!(fallbacks_in(&database_of(dir.path()), &[("space", ' ')]).is_empty());
}

#[test]
fn a_file_over_the_size_limit_is_skipped() {
    let dir = tempfile::tempdir().unwrap();
    let mut big = PLEX_SANS.to_vec();
    big.resize(usize::try_from(MAX_FILE).unwrap() + 1, 0);
    std::fs::write(dir.path().join("big.ttf"), &big).unwrap();
    assert!(database_of(dir.path()).is_empty(), "not even read");
    let mut db = fontdb::Database::new();
    db.load_font_file(dir.path().join("big.ttf")).unwrap();
    assert_eq!(db.len(), 1, "the padded file is still a font");
    assert!(fallbacks_in(&db, &[LATIN]).is_empty(), "nor taken when another walk found it");
    let mut db = fontdb::Database::new();
    db.load_font_source(fontdb::Source::Binary(Arc::new(big)));
    assert!(fallbacks_in(&db, &[LATIN]).is_empty(), "nor in memory");
}

#[test]
fn the_faces_and_the_total_are_capped() {
    let dir = plex_folder();
    let db = database_of(dir.path());
    let size = u64::try_from(PLEX_SANS.len().max(PLEX_MONO.len())).unwrap();
    assert!(fallbacks_within(&db, &[LATIN], &Limits { faces: 0, ..LIMITS }).is_empty());
    assert!(fallbacks_within(&db, &[LATIN], &Limits { total: size / 2, ..LIMITS }).is_empty());
    assert!(fallbacks_within(&db, &[LATIN], &Limits { file: 16, ..LIMITS }).is_empty());
    assert_eq!(fallbacks_within(&db, &[LATIN], &Limits { faces: 1, total: size, ..LIMITS }).len(), 1);
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
    std::fs::create_dir(dir.path().join("loop")).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(dir.path(), dir.path().join("loop").join("back")).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(dir.path().join("plex.ttf"), dir.path().join("again.ttf")).unwrap();
    std::fs::create_dir(dir.path().join("x.ttf")).unwrap();
    std::fs::write(dir.path().join("plex.ttf"), PLEX_SANS).unwrap();
    let db = database_of(dir.path());
    assert_eq!(db.len(), 1, "Plex once, through the link too");
    let fallbacks = fallbacks_in(&db, &[("han", '日'), LATIN, ("emoji", '🙂')]);
    assert_eq!(fallbacks.len(), 1, "only Plex is a font");
    assert_eq!(fallbacks[0].data, PLEX_SANS);
    // A face whose file went away after the search found it, and faces with nonsense indexes.
    let gone = tempfile::tempdir().unwrap();
    std::fs::write(gone.path().join("plex.ttf"), PLEX_SANS).unwrap();
    let db = database_of(gone.path());
    drop(gone);
    assert!(fallbacks_in(&db, &[LATIN]).is_empty());
    let mut db = fontdb::Database::new();
    db.push_face_info(fontdb::FaceInfo { index: 7, ..face("Noto Sans Far", 400, fontdb::Style::Normal) });
    db.push_face_info(fontdb::FaceInfo { source: fontdb::Source::Binary(Arc::new(noise)), ..face("Noise", 400, fontdb::Style::Normal) });
    assert!(fallbacks_in(&db, &[LATIN]).is_empty());
}

#[test]
fn noto_sans_comes_first_then_the_sans_serif_then_regular_and_upright() {
    let normal = fontdb::Style::Normal;
    let mut db = fontdb::Database::new();
    db.set_sans_serif_family("Owner Sans");
    db.push_face_info(face("Some Serif", 400, normal));
    db.push_face_info(face("Owner Sans", 400, normal));
    db.push_face_info(face("Noto Sans Bold Only", 700, normal));
    db.push_face_info(face("Noto Sans Italic", 400, fontdb::Style::Italic));
    db.push_face_info(face("Noto Sans Test", 400, normal));
    let names = |db: &fontdb::Database| fallbacks_in(db, &[LATIN]).into_iter().map(|f| f.name).collect::<Vec<_>>();
    assert_eq!(names(&db), ["system: Noto Sans Test"]);
    let mut db = fontdb::Database::new();
    db.set_sans_serif_family("Owner Sans");
    db.push_face_info(face("Some Serif", 400, normal));
    db.push_face_info(face("Owner Sans", 700, normal));
    db.push_face_info(face("Owner Sans", 400, normal));
    assert_eq!(names(&db), ["system: Owner Sans"]);
    let mut db = fontdb::Database::new();
    db.push_face_info(face("Some Serif", 400, normal));
    db.push_face_info(face("Other Serif", 400, normal));
    assert_eq!(names(&db), ["system: Some Serif"], "else the first found");
}

#[test]
fn a_long_name_is_cut() {
    let normal = fontdb::Style::Normal;
    let mut db = fontdb::Database::new();
    let long = "Noto Sans ".to_owned() + &"x".repeat(500);
    db.push_face_info(face(&long, 400, normal));
    let fallbacks = fallbacks_in(&db, &[LATIN]);
    assert_eq!(fallbacks.len(), 1);
    assert!(fallbacks[0].name.chars().count() <= 80, "{}", fallbacks[0].name);
}

#[test]
fn the_fallbacks_go_in_after_the_app_s_first_pass_and_once() {
    let ctx = egui::Context::default();
    let found = Found::default();
    let mut fallbacks = Fallbacks::new(Arc::clone(&found));
    let plex =
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
    assert_eq!(plex(&ctx).as_deref(), Some("egui-icons"), "nothing found, nothing installed");
    assert!(found.set(fallbacks_in(&database_of(plex_folder().path()), &[LATIN])).is_ok());
    // Found before a window's first pass: installed on a later one, which that pass asks for.
    let early = egui::Context::default();
    let mut fallbacks_early = Fallbacks::new(Arc::clone(&found));
    pass(&early, &mut fallbacks_early);
    assert!(early.has_requested_repaint());
    settle(&early, &mut fallbacks_early);
    assert!(plex(&early).is_some_and(|name| name.starts_with("system: ")), "{:?}", plex(&early));
    // Found while a window rests: the wake brings a pass, which installs them and asks for one more to draw them.
    pass(&ctx, &mut fallbacks);
    assert!(ctx.has_requested_repaint(), "one frame to draw with them");
    settle(&ctx, &mut fallbacks);
    assert!(plex(&ctx).is_some_and(|name| name.starts_with("system: ")));
    let installed = ctx.fonts(|fonts| fonts.definitions().font_data.len());
    pass(&ctx, &mut fallbacks);
    assert!(!ctx.has_requested_repaint(), "then the app rests: they go in once");
    assert_eq!(ctx.fonts(|fonts| fonts.definitions().font_data.len()), installed);
}
