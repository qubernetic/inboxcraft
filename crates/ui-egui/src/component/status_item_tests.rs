// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The status item on its own: every state of its sheet in dark and light at 100 % and 130 % text, its press by the
//! pointer and the keyboard, a fact that takes neither, and a long hostile text kept to one line inside it.

use egui::{Color32, Rect, Vec2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::{Kind, StatusItem, status_item};
use crate::component::Shown;
use crate::component::tests::{Look, Paint, builder, keyboard_walk, own, painted, rests, stage, staged};
use crate::motion;
use crate::theme::{Mode, Palette, TextScale, Theme};

/// A state of the sheet: the kind, the words, the width, the text's left, the mark's colour and the text's.
type State = (Kind, &'static str, f32, f32, Option<Color32>, Color32);

/// The sheet's states: the kind, the sheet's words and its width at 100 %, where its text starts, and its colours
/// (the mark's, the text's).
fn states(p: &Palette) -> [State; 6] {
    [
        (Kind::Plain, "2 accounts", 78.0, 6.0, None, p.muted),
        (Kind::Ok, "synced 14:02", 109.0, 23.0, Some(p.plus), p.muted),
        (Kind::Waiting, "outbox 1 · goes at 14:05", 187.0, 22.0, Some(p.warn), p.ink),
        (Kind::Offline, "offline since 14:10", 155.0, 23.0, Some(p.warn), p.ink),
        (Kind::Error, "Studio: sign-in refused", 181.0, 23.0, Some(p.minus), p.minus),
        (Kind::Accent, "next: Design review 15:00 · in 12 min", 257.0, 6.0, None, p.cyan_text),
    ]
}

/// A page with `item` on the status line's `panel`, as wide as `room`.
fn harness(theme: Theme, item: StatusItem<'static>, room: f32) -> Harness<'static> {
    staged(
        theme,
        |p| p.panel,
        vec2(room + 48.0, 120.0),
        move |ui, theme| {
            ui.set_max_width(room);
            status_item(ui, theme, item);
        },
    )
}

/// The texts of `paint`: where each stands against `rect`, its words and colour.
fn texts(paint: &Paint, rect: Rect) -> Vec<(Vec2, String, Color32, bool)> {
    paint
        .shapes
        .iter()
        .filter_map(|shape| match &shape.look {
            Look::Text { text, colour, .. } if !text.is_empty() => Some((shape.rect.min - rect.min, text.clone(), *colour, shape.inside)),
            _ => None,
        })
        .collect()
}

/// The fills of the marks of `paint`: the discs' and the ring's dots, and the circles' strokes.
/// (A disc at 130 % is 10.7 across.)
fn marks(paint: &Paint) -> Vec<Color32> {
    paint
        .rects()
        .filter_map(|shape| match shape.look {
            Look::Rect { fill, stroke, .. } if shape.rect.width() <= 12.0 => {
                if fill.a() > 0 {
                    Some(fill)
                } else {
                    stroke.map(|(_, colour)| colour)
                }
            }
            _ => None,
        })
        .collect()
}

#[test]
fn every_state_looks_as_its_sheet_says_in_dark_and_light() {
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::default());
        let p = &theme.palette;
        for (kind, words, wide, text_x, mark, ink) in states(p) {
            let harness = harness(theme, StatusItem::new(words, kind), 600.0);
            let rect = harness.get_by_label(words).rect();
            let paint = painted(&harness, words);
            let at = format!("{mode:?} {kind:?}: {paint:#?}");
            assert_eq!(rect.size(), vec2(wide, 26.0), "{at}");
            assert_eq!((rect.min.x.fract(), rect.min.y.fract()), (0.0, 0.0), "{at}");
            let look = own(&paint, rect);
            assert_eq!((look.fill, look.edge), (None, None), "at rest no fill and no edge: {at}");
            let texts = texts(&paint, rect);
            assert_eq!(texts.len(), 1, "{at}");
            let (offset, text, colour, inside) = &texts[0];
            assert_eq!((offset.x, text.as_str(), *colour, *inside), (text_x, words, ink, true), "{at}");
            let marks = marks(&paint);
            match mark {
                Some(colour) => {
                    let count = if kind == Kind::Waiting { 8 } else { 1 };
                    assert_eq!(marks, vec![colour; count], "{at}");
                }
                None => assert!(marks.is_empty(), "{at}"),
            }
        }
    }
}

#[test]
fn at_130_percent_the_item_is_34_high_and_its_paddings_stay() {
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::ALL[4]);
        let p = &theme.palette;
        for (kind, words, _, _, _, ink) in states(p) {
            let harness = harness(theme, StatusItem::new(words, kind), 800.0);
            let rect = harness.get_by_label(words).rect();
            let paint = painted(&harness, words);
            let texts = texts(&paint, rect);
            let at = format!("{mode:?} {kind:?}: {texts:?}");
            assert_eq!(rect.height(), 34.0, "{at}");
            let (offset, _, colour, _) = &texts[0];
            // 6 in; a sign as wide as the larger text (11 × 1.3, up to 15), the ring its fixed 10; 6 after it.
            let text_x = match kind {
                Kind::Plain | Kind::Accent => 6.0,
                Kind::Waiting => 22.0,
                Kind::Ok | Kind::Offline | Kind::Error => 27.0,
            };
            assert_eq!((offset.x, *colour), (text_x, ink), "{at}");
            assert_eq!(offset.y, ((34.0 - theme.fonts.mono_s.line) / 2.0).round(), "{at}");
            // 6 after the text: the box hugs it.
            let galley_right = rect.width() - 6.0;
            assert!(galley_right > text_x, "{at}");
        }
    }
}

#[test]
fn hover_focus_and_pressed_look_as_the_sheet_and_the_rule_say_at_100_and_130_percent() {
    for (mode, scale) in [Mode::Dark, Mode::Light].into_iter().flat_map(|mode| [(mode, TextScale::default()), (mode, TextScale::ALL[4])]) {
        let theme = Theme::new(mode, scale);
        let p = theme.palette;
        let looks = [
            (Shown::Hover, Some(p.grid), None, p.ink),
            (Shown::Focus, None, Some(p.cyan_edge), p.muted),
            (Shown::Pressed, Some(p.stage), Some(p.ink), p.ink),
        ];
        for (shown, fill, edge, ink) in looks {
            let words = "synced 14:02";
            let harness = harness(theme, StatusItem::new(words, Kind::Ok).pressable().shown(shown), 600.0);
            let rect = harness.get_by_label(words).rect();
            let paint = painted(&harness, words);
            let look = own(&paint, rect);
            let at = format!("{mode:?} {scale:?} {shown:?}: {look:#?}");
            if scale == TextScale::default() {
                assert_eq!(rect.size(), vec2(109.0, 26.0), "{at}");
            } else {
                // 34 high; 6 in, the sign 15 (11 × 1.3, up), 6 to the text, whose box ends 6 before the item's.
                assert_eq!(rect.height(), 34.0, "{at}");
                let (offset, ..) = &texts(&paint, rect)[0];
                assert_eq!(offset.x, 27.0, "{at}");
                let text = paint.shapes.iter().find(|shape| matches!(shape.look, Look::Text { .. })).map(|shape| shape.rect).unwrap();
                assert_eq!(rect.right() - 6.0, rect.left() + 27.0 + crate::draw::paint::run_box(text.width()), "{at}");
            }
            assert_eq!(look.fill, fill, "{at}");
            assert_eq!(look.edge, edge.map(|colour| (1.0, colour, false)), "{at}");
            assert_eq!(texts(&paint, rect)[0].2, ink, "{at}");
            assert!(marks(&paint).contains(&p.plus), "{at}");
        }
    }
}

#[test]
fn the_pointer_brings_the_hover_fill_and_the_keyboard_the_focus_edge() {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let p = theme.palette;
    let words = "synced 14:02";
    let mut harness = harness(theme, StatusItem::new(words, Kind::Ok).pressable(), 600.0);
    harness.get_by_label(words).hover();
    harness.run();
    let rect = harness.get_by_label(words).rect();
    assert_eq!(own(&painted(&harness, words), rect).fill, Some(p.grid));
    harness.hover_at(egui::pos2(1.0, 1.0));
    harness.key_press(egui::Key::Tab);
    harness.run();
    assert_eq!(own(&painted(&harness, words), rect).edge, Some((1.0, p.cyan_edge, false)));
}

/// A page of a pressable item and a fact beside it; the frames' presses of the item are counted in the state.
fn pressing(pressable: bool) -> Harness<'static, usize> {
    let theme = Theme::new(Mode::Light, TextScale::default());
    let mut harness = builder().with_size(vec2(400.0, 80.0)).build_ui_state(
        move |ui, presses: &mut usize| {
            motion::set_reduced(ui.ctx(), true);
            stage(ui, theme.palette.panel, |ui| {
                ui.horizontal(|ui| {
                    let item = StatusItem::new("2 accounts", Kind::Plain);
                    let item = if pressable { item.pressable() } else { item };
                    if status_item(ui, &theme, item).clicked() {
                        *presses += 1;
                    }
                    status_item(ui, &theme, StatusItem::new("outbox 1", Kind::Plain));
                });
            });
        },
        0,
    );
    harness.run();
    harness
}

#[test]
fn a_pressable_item_is_pressed_by_the_pointer_enter_and_space() {
    let mut harness = pressing(true);
    harness.get_by_label("2 accounts").click();
    harness.run();
    assert_eq!(*harness.state(), 1, "the pointer");
    for key in [egui::Key::Enter, egui::Key::Space] {
        harness.get_by_label("2 accounts").focus();
        harness.run();
        harness.key_press(key);
        harness.run();
    }
    assert_eq!(*harness.state(), 3, "Enter and Space");
    let missed = keyboard_walk(&mut harness);
    assert!(missed.is_empty(), "{missed:?}");
    assert!(rests(&mut harness, 8));
}

#[test]
fn a_fact_takes_neither_the_pointer_nor_the_keyboard() {
    let mut harness = pressing(false);
    let fact = harness.get_by_label("2 accounts");
    assert_eq!(fact.accesskit_node().role(), egui::accesskit::Role::Label);
    fact.click();
    harness.run();
    harness.key_press(egui::Key::Tab);
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();
    assert_eq!(*harness.state(), 0);
    assert!(harness.ctx.memory(|memory| memory.focused()).is_none(), "no Tab stop");
    assert!(rests(&mut harness, 8));
}

#[test]
fn a_text_wider_than_its_room_is_cut_with_one_ellipsis_and_never_past_420() {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let words = "Studio: sign-in refused, mail is not being fetched from the server at all today";
    for (room, most) in [(150.0, 150.0), (900.0, 6.0 + 11.0 + 6.0 + 420.0 + 6.0)] {
        let harness = harness(theme, StatusItem::new(words, Kind::Error).pressable(), room);
        let rect = harness.get_by_label(words).rect();
        let paint = painted(&harness, words);
        let (_, text, ..) = texts(&paint, rect).remove(0);
        assert!(rect.width() <= most, "{room}: {rect:?}");
        assert!(text.ends_with('…') && text.matches('…').count() == 1 && !text.ends_with(" …"), "{room}: {text:?}");
        assert!(!own(&paint, rect).overflows, "{room}");
        for shape in &paint.shapes {
            assert!(rect.expand(0.5).contains_rect(shape.rect), "{room}: {shape:?} leaves {rect:?}");
        }
    }
}

#[test]
fn a_hostile_10000_character_failure_is_one_line_inside_the_item_cleaned_or_not() {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let raw: &'static str = Box::leak(format!("Studio\r\n\0\u{202E}{}", "refused \u{2067}x\u{301}\u{301} ".repeat(800)).into_boxed_str());
    let cleaned: &'static str =
        Box::leak(raw.chars().filter(|c| !c.is_control() && !('\u{202A}'..='\u{2069}').contains(c)).collect::<String>().into_boxed_str());
    for text in [raw, cleaned] {
        assert!(text.chars().count() > 10_000 || text == cleaned);
        let harness = harness(theme, StatusItem::new(text, Kind::Error).pressable(), 600.0);
        let node = harness.query_all_by_role(egui::accesskit::Role::Button).next().unwrap();
        let label = node.accesskit_node().label().unwrap_or_default();
        assert!(label.chars().count() <= crate::component::MOST, "heard capped: {}", label.chars().count());
        assert!(!label.contains(['\r', '\n', '\0', '\u{202E}', '\u{2067}']), "heard as plain words");
        let rect = node.rect();
        assert_eq!(rect.height(), 26.0);
        let mut shown = Vec::new();
        fn open(shape: &egui::Shape, out: &mut Vec<(Rect, String, usize)>) {
            match shape {
                egui::Shape::Vec(inner) => inner.iter().for_each(|shape| open(shape, out)),
                egui::Shape::Text(text) => out.push((text.visual_bounding_rect(), text.galley.text().to_owned(), text.galley.rows.len())),
                _ => {}
            }
        }
        harness.output().shapes.iter().for_each(|clipped| open(&clipped.shape, &mut shown));
        let (bounds, line, rows) = shown.into_iter().find(|(_, text, _)| text.starts_with("Studio")).unwrap();
        assert_eq!(rows, 1, "one line");
        assert!(line.ends_with('…') && line.matches('…').count() == 1, "{line:?}");
        assert!(!line.contains(['\r', '\n', '\0', '\u{202E}', '\u{2067}']), "{line:?}");
        assert!(rect.expand(0.5).contains_rect(bounds), "{bounds:?} leaves {rect:?}");
    }
}

#[test]
fn a_line_looks_at_no_more_than_the_cap_of_a_10000_character_text() {
    let long = format!("Studio\r\n\0\u{202E}{}", "refused ".repeat(1300));
    assert!(long.chars().count() > 10_000);
    let (kept, cut) = super::kept(&long);
    assert!(cut);
    assert!(kept.chars().count() <= crate::component::MOST, "{}", kept.chars().count());
    assert!(!kept.contains(['\r', '\n', '\0', '\u{202E}']), "plain words");
    let (short, cut) = super::kept("synced 14:02");
    assert_eq!((short.as_ref(), cut), ("synced 14:02", false));
}

/// Every text, rect and disc the last frame of `harness` painted, as far as its clip shows it, and whether it is text.
fn shown_shapes<State>(harness: &Harness<'_, State>) -> Vec<(Rect, bool, bool)> {
    fn open(shape: &egui::Shape, clip: Rect, out: &mut Vec<(Rect, bool, bool)>) {
        match shape {
            egui::Shape::Vec(inner) => inner.iter().for_each(|shape| open(shape, clip, out)),
            egui::Shape::Text(text) => out.push((text.visual_bounding_rect().intersect(clip), true, false)),
            egui::Shape::Rect(rect) => out.push((rect.rect.intersect(clip), false, false)),
            egui::Shape::Circle(circle) => {
                out.push((Rect::from_center_size(circle.center, Vec2::splat(circle.radius * 2.0)).intersect(clip), false, true));
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    harness.output().shapes.iter().for_each(|clipped| open(&clipped.shape, clipped.clip_rect, &mut out));
    out
}

#[test]
fn an_item_is_never_wider_than_its_room_and_draws_only_what_fits_whole() {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    // In 20 the mark fits whole (6 + 11) and the text not at all; in 8 not even the mark; in 0 nothing.
    for (room, disc) in [(0.0, false), (8.0, false), (20.0, true)] {
        let words = "synced 14:02";
        let harness = harness(theme, StatusItem::new(words, Kind::Ok).pressable(), room);
        let rect = harness.get_by_label(words).rect();
        assert!(rect.width() <= room, "{room}: {rect:?}");
        let shapes: Vec<_> = shown_shapes(&harness)
            .into_iter()
            .filter(|(bounds, ..)| bounds.is_positive() && !(bounds.contains_rect(rect) && *bounds != rect))
            .collect();
        for (bounds, ..) in &shapes {
            assert!(rect.expand(0.01).contains_rect(*bounds), "{room}: {bounds:?} leaves {rect:?}");
        }
        assert!(!shapes.iter().any(|(_, text, _)| *text), "{room}: no text fits");
        assert_eq!(shapes.iter().any(|(_, _, circle)| *circle), disc, "{room}: the mark");
    }
}
