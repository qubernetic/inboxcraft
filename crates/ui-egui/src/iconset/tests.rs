// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::HashSet;

use egui::{Color32, Rect, pos2, vec2};

use super::{BACK_OPACITY, IconId, IconSet, Style};
use crate::shot::{self, Picture};
use crate::theme::{self, IconFont, Mode, Preset, Theme};

const STYLES: [Style; 3] = [Style::Regular, Style::Fill, Style::Duotone];

#[test]
fn all_lists_every_id_once_in_order() {
    for (index, id) in IconId::ALL.iter().enumerate() {
        assert_eq!(id.index(), index, "{id:?}");
    }
    let unique: HashSet<IconId> = IconId::ALL.into_iter().collect();
    assert_eq!(unique.len(), IconId::ALL.len());
}

#[test]
fn every_id_has_a_glyph_in_every_style_that_its_font_draws() {
    let ctx = egui::Context::default();
    theme::install_fonts(&ctx);
    ctx.run_ui(egui::RawInput::default(), |_| {}).textures_delta.clear();
    for set in IconSet::ALL {
        for style in STYLES {
            for id in IconId::ALL {
                let glyph = set.glyph(id, style);
                assert!(!glyph.layers.is_empty(), "{set:?} {style:?} {id:?}");
                let font = glyph.face.id(16.0);
                for (layer, _) in &glyph.layers {
                    assert!(ctx.fonts_mut(|fonts| fonts.has_glyph(&font, *layer)), "{set:?} {style:?} {id:?} {layer:?}");
                }
            }
        }
    }
}

#[test]
fn ids_have_their_own_glyphs() {
    for set in IconSet::ALL {
        for style in STYLES {
            let glyphs: HashSet<Vec<char>> = IconId::ALL.iter().map(|id| set.glyph(*id, style).layers.iter().map(|(c, _)| *c).collect()).collect();
            assert_eq!(glyphs.len(), IconId::ALL.len(), "{set:?} {style:?}");
        }
    }
}

#[test]
fn phosphor_draws_regular_and_fill_as_one_glyph_of_its_own_font_and_duotone_as_a_faint_back_under_the_line() {
    let set = IconSet::Phosphor;
    for id in IconId::ALL {
        assert_eq!(set.glyph(id, Style::Regular).layers.len(), 1);
        assert_eq!(set.glyph(id, Style::Fill).layers.len(), 1);
        let duotone = set.glyph(id, Style::Duotone);
        let opacities: Vec<f32> = duotone.layers.iter().map(|(_, opacity)| *opacity).collect();
        assert_eq!(opacities, vec![BACK_OPACITY, 1.0], "{id:?}");
        assert_ne!(set.glyph(id, Style::Regular).face, set.glyph(id, Style::Fill).face);
        assert!(STYLES.iter().all(|style| set.glyph(id, *style).disc == 0.0), "Phosphor has solid glyphs and needs no disc");
    }
}

#[test]
fn lucide_draws_lines_with_a_faint_disc_behind_duotone_only() {
    let set = IconSet::Lucide;
    for id in IconId::ALL {
        let [regular, fill, duotone] = STYLES.map(|style| set.glyph(id, style));
        for glyph in [&regular, &fill, &duotone] {
            assert_eq!(glyph.layers, regular.layers, "{id:?}: one line glyph in every style");
            assert_eq!(glyph.face, IconFont::Lucide);
        }
        assert_eq!(regular.disc, 0.0);
        assert_eq!(duotone.disc, BACK_OPACITY);
        // A disc behind an active state looked like a smudge; its colour and the selection behind it show the state.
        assert_eq!(fill.disc, 0.0, "{id:?}: no disc behind Fill");
    }
}

#[test]
fn the_owner_s_icon_choice_names_its_set() {
    use inboxcraft_engine::appearance::Icons;
    assert_eq!(IconSet::of(Icons::Phosphor), IconSet::Phosphor);
    assert_eq!(IconSet::of(Icons::Lucide), IconSet::Lucide);
    assert_eq!(IconSet::ALL.len(), Icons::ALL.len());
}

#[test]
fn the_theme_draws_phosphor_by_default() {
    assert_eq!(IconSet::default(), IconSet::Phosphor);
    assert_eq!(Theme::new(Mode::Light, &Preset::default()).icons, IconSet::Phosphor);
}

/// `id` in `style` painted black into a `side` point square, at 2 pixels a point.
fn painted(set: IconSet, id: IconId, style: Style, side: f32) -> Picture {
    let mut first = true;
    shot::paint(vec2(side, side), 2.0, Mode::Light, |ui| {
        if first {
            theme::install_fonts(ui.ctx());
            first = false;
        }
        set.paint(ui.painter(), Rect::from_min_size(pos2(0.0, 0.0), vec2(side, side)), id, style, Color32::BLACK);
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
    for set in IconSet::ALL {
        for style in [Style::Regular, Style::Fill] {
            for side in [16.0, 20.0, 32.0] {
                let picture = painted(set, IconId::Waiting, style, side);
                let ([l, t, r, b], _) = ink(&picture).unwrap();
                let pixels = side * 2.0;
                let centre = |a: usize, b: usize| (a + b + 1) as f32 / 2.0;
                let off = (centre(l, r) - pixels / 2.0, centre(t, b) - pixels / 2.0);
                assert!(off.0.abs() <= 1.0 && off.1.abs() <= 1.0, "{set:?} {style:?} {side}: off by {off:?} pixels ({l} {t} {r} {b})");
                let width = (r + 1 - l) as f32 / pixels;
                assert!((0.7..=0.95).contains(&width), "{set:?} {style:?} {side}: the ring is {width} of the square");
            }
        }
    }
}

#[test]
fn duotone_puts_more_ink_down_than_regular_and_fill_more_still_where_the_set_has_solid_glyphs() {
    for set in IconSet::ALL {
        let sum = |style| ink(&painted(set, IconId::Inbox, style, 24.0)).unwrap().1;
        let regular = sum(Style::Regular);
        assert!(sum(Style::Duotone) > regular * 1.1, "{set:?}");
        // Lucide's Fill is its line glyph; Phosphor's is solid.
        match set {
            IconSet::Phosphor => assert!(sum(Style::Fill) > sum(Style::Duotone), "{set:?}"),
            IconSet::Lucide => assert!((sum(Style::Fill) - regular).abs() < regular * 0.01, "{set:?}"),
        }
    }
}

#[test]
fn nothing_is_painted_into_an_empty_or_unreal_square() {
    for rect in [Rect::NOTHING, Rect::from_min_size(pos2(4.0, 4.0), vec2(0.0, 10.0)), Rect::from_min_size(pos2(f32::NAN, 0.0), vec2(10.0, 10.0))] {
        let picture = shot::paint(vec2(16.0, 16.0), 1.0, Mode::Light, |ui| {
            theme::install_fonts(ui.ctx());
            IconSet::Phosphor.paint(ui.painter(), rect, IconId::Inbox, Style::Fill, Color32::BLACK);
        })
        .unwrap();
        assert!(ink(&picture).is_none(), "{rect:?}");
    }
}
