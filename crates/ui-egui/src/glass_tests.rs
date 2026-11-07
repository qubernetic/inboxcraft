// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::{Color32, FontId, Rect, pos2, vec2};

use super::{Layers, Under, backdrop, faded, fields, float};
use crate::gradient::Gradient;
use crate::shot::{Picture, paint};
use crate::theme::{GHOST, Mode, PRESETS, Theme, contrast, over};

/// Every preset in both modes.
fn all() -> Vec<Theme> {
    PRESETS.iter().flat_map(|preset| [Theme::new(Mode::Light, preset), Theme::new(Mode::Dark, preset)]).collect()
}

fn opaque(colour: Color32) -> Color32 {
    Color32::from_rgb(colour.r(), colour.g(), colour.b())
}

/// The largest contrast between the same pixels of `a` and `b` inside `area`.
fn largest_change(a: &Picture, b: &Picture, area: Rect) -> f32 {
    let mut largest: f32 = 1.0;
    for y in area.top() as usize..area.bottom() as usize {
        for x in area.left() as usize..area.right() as usize {
            if let (Some(one), Some(other)) = (a.pixel(x, y), b.pixel(x, y)) {
                largest = largest.max(contrast(opaque(one), opaque(other)));
            }
        }
    }
    largest
}

const WINDOW: [f32; 2] = [600.0, 300.0];
const CARD: Rect = Rect { min: pos2(20.0, 20.0), max: pos2(580.0, 280.0) };
/// A menu over the reader's sharp text.
const MENU: Rect = Rect { min: pos2(300.0, 40.0), max: pos2(560.0, 150.0) };
/// Hover actions over a row whose text fades before them.
const ACTIONS: Rect = Rect { min: pos2(400.0, 185.0), max: pos2(560.0, 230.0) };

/// What a scene shows.
#[derive(Clone, Copy)]
struct Scene {
    /// The sharp text under the menu.
    sharp: bool,
    /// The row text under the hover actions.
    row: bool,
    /// The text on the menu.
    on_float: bool,
    /// What the menu is drawn as.
    menu: Under,
}

const FULL: Scene = Scene { sharp: true, row: true, on_float: true, menu: Under::Sharp };

/// The fields, a glass card, the reader's text with a menu over it and a row whose text fades before hover actions.
fn scene(theme: &Theme, shown: Scene) -> Picture {
    let theme = *theme;
    paint(vec2(WINDOW[0], WINDOW[1]), 1.0, theme.mode, move |ui| {
        let p = theme.palette;
        let painter = ui.painter().clone();
        backdrop(&painter, &theme, Rect::from_min_size(pos2(0.0, 0.0), vec2(WINDOW[0], WINDOW[1])));
        let layers = Layers::begin(&painter);
        let mut layers = layers;
        layers.card(&mut ui.new_child(egui::UiBuilder::new().max_rect(CARD)), egui::Margin::ZERO, |_| ());
        layers.finish(&painter, &theme);
        let words = "Sharp reader text that cannot fade, MWMWMWMWMWMW";
        if shown.sharp {
            for (index, colour) in [p.text, p.muted, p.accent].into_iter().enumerate() {
                let galley = painter.layout_no_wrap(words.to_owned(), FontId::proportional(18.0), colour);
                painter.galley(pos2(36.0, 50.0 + 30.0 * index as f32), galley, colour);
            }
        }
        if shown.row {
            let galley = painter.layout_no_wrap(words.to_owned(), FontId::proportional(16.0), p.text);
            faded(&painter, pos2(36.0, 198.0), &galley, p.text, ACTIONS.left() - theme.spacing.xs, theme.spacing.xl);
        }
        float(&painter, &theme, MENU, shown.menu);
        float(&painter, &theme, ACTIONS, Under::Fading);
        if shown.on_float {
            let galley = painter.layout_no_wrap("Reply all".to_owned(), FontId::proportional(22.0), p.text);
            painter.galley(pos2(316.0, 56.0), galley, p.text);
        }
    })
    .unwrap()
}

#[test]
fn a_float_keeps_its_text_readable_and_the_content_under_it_quiet_for_every_preset_in_both_modes() {
    for theme in all() {
        let name = format!("{} {:?}", theme.preset.id, theme.mode);
        let full = scene(&theme, FULL);
        // The menu's own text: as clear as on any surface.
        let bare = scene(&theme, Scene { on_float: false, ..FULL });
        let text = largest_change(&full, &bare, Rect::from_min_max(pos2(316.0, 56.0), pos2(420.0, 84.0)));
        assert!(text >= 4.5, "{name}: the menu's text reads at {text:.2}");
        // The sharp text under the menu: barely there.
        let unread = scene(&theme, Scene { sharp: false, ..FULL });
        let under_menu = Rect::from_min_max(pos2(430.0, MENU.top() + 2.0), pos2(MENU.right() - 2.0, MENU.bottom() - 2.0));
        let ghost = largest_change(&full, &unread, under_menu);
        assert!(ghost <= GHOST + 0.02, "{name}: the reader's text shows through the menu at {ghost:.2}");
        // The row under the hover actions: faded out before them, so nothing shows through.
        let no_row = scene(&theme, Scene { row: false, ..FULL });
        let under_actions = largest_change(&full, &no_row, ACTIONS.shrink(1.0));
        assert!(under_actions <= 1.02, "{name}: the row shows through the hover actions at {under_actions:.2}");
        let before = largest_change(&full, &no_row, Rect::from_min_max(pos2(36.0, 196.0), pos2(300.0, 220.0)));
        assert!(before >= 4.5, "{name}: the row reads before it fades: {before:.2}");
    }
}

#[test]
fn the_plain_float_over_sharp_text_would_let_it_show() {
    // Why a float over sharp text goes more opaque: the 80 % float leaves the text under it plainly visible.
    let shown = all()
        .iter()
        .map(|theme| {
            let plain = scene(theme, Scene { menu: Under::Fading, ..FULL });
            let unread = scene(theme, Scene { sharp: false, menu: Under::Fading, ..FULL });
            largest_change(&plain, &unread, Rect::from_min_max(pos2(430.0, MENU.top() + 2.0), pos2(MENU.right() - 2.0, MENU.bottom() - 2.0)))
        })
        .fold(1.0, f32::max);
    assert!(shown > GHOST + 0.1, "the plain float shows the text at {shown:.2}");
}

#[test]
fn the_fields_glow_in_the_presets_stops_over_the_background() {
    for theme in all() {
        let Gradient::Fields { base, fields } = fields(&theme) else { panic!("fields") };
        assert_eq!(base, theme.palette.background);
        let colours: Vec<Color32> = fields.iter().map(|field| field.colour).collect();
        assert_eq!(colours, theme.palette.colour_fields.to_vec());
        let picture =
            paint(vec2(400.0, 300.0), 1.0, theme.mode, |ui| backdrop(ui.painter(), &theme, Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 300.0))))
                .unwrap();
        // Soft pictures are rasterised for sizes rounded up to 128 pixels: 512 by 384 here.
        let centre = fields[0].centre;
        let near_first = opaque(picture.pixel((centre.x * 512.0) as usize, (centre.y * 384.0) as usize).unwrap());
        let toward = over(theme.palette.colour_fields[0], theme.palette.background);
        assert!(
            contrast(near_first, toward) < contrast(theme.palette.background, toward),
            "{} {:?}: tinted by the first stop",
            theme.preset.id,
            theme.mode
        );
        assert_eq!(picture.pixel(200, 150).map(|pixel| pixel.a()), Some(255), "the fields are opaque");
    }
}

#[test]
fn a_layer_is_glass_with_a_light_top_edge_and_a_shadow_under_it() {
    for theme in all() {
        let p = theme.palette;
        let card = Rect::from_min_max(pos2(60.0, 60.0), pos2(340.0, 220.0));
        let draw = |with_card: bool| {
            paint(vec2(400.0, 300.0), 1.0, theme.mode, move |ui| {
                let painter = ui.painter().clone();
                backdrop(&painter, &theme, Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 300.0)));
                let mut layers = Layers::begin(&painter);
                if with_card {
                    layers.card(&mut ui.new_child(egui::UiBuilder::new().max_rect(card)), egui::Margin::ZERO, |_| ());
                    assert_eq!(layers.rects(), &[card]);
                }
                layers.finish(&painter, &theme);
            })
            .unwrap()
        };
        let (with, without) = (draw(true), draw(false));
        let name = format!("{} {:?}", theme.preset.id, theme.mode);
        let inside = opaque(with.pixel(200, 150).unwrap());
        let expected = over(p.glass, opaque(without.pixel(200, 150).unwrap()));
        assert!(contrast(inside, expected) < 1.02, "{name}: glass over the fields: {inside:?} {expected:?}");
        let edge = opaque(with.pixel(200, 60).unwrap());
        let below_edge = opaque(with.pixel(200, 64).unwrap());
        let light = |c: Color32| u16::from(c.r()) + u16::from(c.g()) + u16::from(c.b());
        assert!(light(edge) > light(below_edge), "{name}: a light top edge: {edge:?} {below_edge:?}");
        let shade = opaque(with.pixel(200, 225).unwrap());
        let bare = opaque(without.pixel(200, 225).unwrap());
        assert!(light(shade) < light(bare), "{name}: a shadow under the layer: {shade:?} {bare:?}");
    }
}

#[test]
fn a_float_over_sharp_text_casts_no_shadow_onto_it_and_one_over_fading_content_does() {
    for theme in all() {
        let name = format!("{} {:?}", theme.preset.id, theme.mode);
        let draw = |under: Option<Under>| {
            paint(vec2(WINDOW[0], WINDOW[1]), 1.0, theme.mode, move |ui| {
                let painter = ui.painter().clone();
                backdrop(&painter, &theme, Rect::from_min_size(pos2(0.0, 0.0), vec2(WINDOW[0], WINDOW[1])));
                if let Some(under) = under {
                    float(&painter, &theme, MENU, under);
                }
            })
            .unwrap()
        };
        let bare = draw(None);
        // Just outside the menu, where its shadow falls.
        let beside = Rect::from_min_max(pos2(MENU.left() + 20.0, MENU.bottom() + 1.0), pos2(MENU.right() - 20.0, MENU.bottom() + 8.0));
        let sharp = largest_change(&draw(Some(Under::Sharp)), &bare, beside);
        assert!(sharp <= 1.0, "{name}: the menu darkens the text beside it at {sharp:.3}");
        let fading = largest_change(&draw(Some(Under::Fading)), &bare, beside);
        assert!(fading > 1.0, "{name}: a float over fading content has a shadow");
    }
}
