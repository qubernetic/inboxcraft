// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::HashSet;

use egui::{Color32, Rect, pos2, vec2};

use super::{IconId, glyph, paint};
use crate::shot::{self, Picture};
use crate::theme::{self, Mode};

#[test]
fn all_lists_every_id_once_in_order() {
    for (index, id) in IconId::ALL.iter().enumerate() {
        assert_eq!(id.index(), index, "{id:?}");
    }
    let unique: HashSet<IconId> = IconId::ALL.into_iter().collect();
    assert_eq!(unique.len(), IconId::ALL.len());
}

#[test]
fn one_icon_family_one_weight() {
    let ctx = egui::Context::default();
    theme::install_fonts(&ctx);
    ctx.run_ui(egui::RawInput::default(), |_| {}).textures_delta.clear();
    let font = theme::icon_font(16.0);
    assert_eq!(font.family, egui::FontFamily::Name("Phosphor".into()));
    for id in IconId::ALL {
        assert!(ctx.fonts_mut(|fonts| fonts.has_glyph(&font, glyph(id))), "{id:?}");
    }
    let family = ctx.fonts(|fonts| fonts.definitions().families.get(&font.family).cloned());
    assert_eq!(family, Some(vec!["Phosphor".to_owned()]), "Phosphor Regular alone");
    // No code names another icon font: the names are put together so this file does not name them either.
    let gone = [["Phosphor", "-Fill"].concat(), ["Phosphor", "-Duotone"].concat(), ["Luc", "ide"].concat()];
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    for dir in ["src", "examples"] {
        rust_files(&root.join(dir), &mut files);
    }
    assert!(files.len() > 10, "the scan finds the sources");
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        for name in &gone {
            assert!(!text.contains(name.as_str()), "{} names {name}", file.display());
        }
    }
}

/// The `.rs` files under `dir`, tests included.
fn rust_files(dir: &std::path::Path, found: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_files(&path, found);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            found.push(path);
        }
    }
}

#[test]
fn ids_have_their_own_glyphs() {
    let glyphs: HashSet<char> = IconId::ALL.iter().map(|id| glyph(*id)).collect();
    assert_eq!(glyphs.len(), IconId::ALL.len());
}

/// `id` painted black into a `side` point square, at 2 pixels a point.
fn painted(id: IconId, side: f32) -> Picture {
    let mut first = true;
    shot::paint(vec2(side, side), 2.0, Mode::Light, |ui| {
        if first {
            theme::install_fonts(ui.ctx());
            first = false;
        }
        paint(ui.painter(), Rect::from_min_size(pos2(0.0, 0.0), vec2(side, side)), id, Color32::BLACK);
    })
    .unwrap()
}

/// The pixels with ink: their bounding box (left, top, right, bottom, in pixels) and the summed alpha.
fn ink(picture: &Picture) -> Option<([usize; 4], f32)> {
    let mut found: Option<[usize; 4]> = None;
    let mut sum = 0.0;
    for y in 0..picture.height() {
        for x in 0..picture.width() {
            let alpha = picture.pixel(x, y).unwrap().a();
            if alpha > 24 {
                sum += f32::from(alpha) / 255.0;
                found = Some(match found {
                    None => [x, y, x, y],
                    Some([l, t, r, b]) => [l.min(x), t.min(y), r.max(x), b.max(y)],
                });
            }
        }
    }
    found.map(|bounds| (bounds, sum))
}

#[test]
fn a_ring_sits_in_the_middle_of_its_square_and_fills_most_of_it_at_every_size() {
    for side in [14.0, 16.0, 20.0, 32.0] {
        let picture = painted(IconId::Waiting, side);
        let ([l, t, r, b], _) = ink(&picture).unwrap();
        let pixels = side * 2.0;
        let centre = |a: usize, b: usize| (a + b + 1) as f32 / 2.0;
        let off = (centre(l, r) - pixels / 2.0, centre(t, b) - pixels / 2.0);
        assert!(off.0.abs() <= 1.0 && off.1.abs() <= 1.0, "{side}: off by {off:?} pixels ({l} {t} {r} {b})");
        let width = (r + 1 - l) as f32 / pixels;
        assert!((0.7..=0.95).contains(&width), "{side}: the ring is {width} of the square");
    }
}

#[test]
fn nothing_is_painted_into_an_empty_or_unreal_square() {
    for rect in [Rect::NOTHING, Rect::from_min_size(pos2(4.0, 4.0), vec2(0.0, 10.0)), Rect::from_min_size(pos2(f32::NAN, 0.0), vec2(10.0, 10.0))] {
        let picture = shot::paint(vec2(16.0, 16.0), 1.0, Mode::Light, |ui| {
            theme::install_fonts(ui.ctx());
            paint(ui.painter(), rect, IconId::Inbox, Color32::BLACK);
        })
        .unwrap();
        assert!(ink(&picture).is_none(), "{rect:?}");
    }
}
