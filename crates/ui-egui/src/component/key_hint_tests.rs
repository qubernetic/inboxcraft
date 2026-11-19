// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::{Color32, Rect, vec2};
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::{key_hint, spoken};
use crate::component::Shown;
use crate::component::tests::{Look, Paint, builder, keyboard_walk, painted, rests, stage, staged};
use crate::theme::{Mode, TextScale, Theme};

/// The texts painted, `(rect, text, colour, size)`.
fn texts(paint: &Paint) -> Vec<(Rect, String, Color32, f32)> {
    paint
        .shapes
        .iter()
        .filter_map(|shape| match &shape.look {
            Look::Text { text, colour, size, .. } => Some((shape.rect, text.clone(), *colour, *size)),
            _ => None,
        })
        .collect()
}

/// The fills painted, `(rect, colour)`.
fn fills(paint: &Paint) -> Vec<(Rect, Color32)> {
    paint
        .rects()
        .filter_map(|shape| match shape.look {
            Look::Rect { fill, .. } if fill.a() > 0 => Some((shape.rect, fill)),
            _ => None,
        })
        .collect()
}

#[test]
fn a_key_hint_looks_as_its_sheet_says_in_both_states() {
    for scale in [TextScale::default(), TextScale::ALL[4]] {
        for mode in [Mode::Dark, Mode::Light] {
            let theme = Theme::new(mode, scale);
            let p = theme.palette;
            for (state, forced) in [("default", None), ("hover", Some(Shown::Hover))] {
                let harness = staged(
                    theme,
                    |p| p.panel,
                    vec2(300.0, 100.0),
                    move |ui, theme| {
                        key_hint(ui, theme, "D", "done", forced);
                    },
                );
                let name = spoken("D", "done");
                let rect = harness.get_by_label(&name).rect();
                let paint = painted(&harness, &name);
                let at = format!("{mode:?} {}% {state}: {paint:#?}", scale.percent());
                let high = theme.boxes().chip;
                // 17 for the chip of "D", 8, 27 for the word: the sheet's 52 × 20; at 130 % the text is wider.
                if scale == TextScale::default() {
                    assert_eq!(rect.size(), vec2(52.0, 20.0), "{at}");
                } else {
                    assert_eq!(rect.height(), 26.0, "{at}");
                }
                assert_eq!(rect.height(), high, "{at}");
                assert_eq!(rect.min, rect.min.round(), "on whole pixels: {at}");
                let words = texts(&paint);
                let word = words.iter().find(|text| text.1 == "done").cloned();
                let Some((word_rect, _, colour, size)) = word else { panic!("no word: {at}") };
                let chip = words.iter().find(|text| text.1 == "D").map_or(Rect::NOTHING, |text| text.0);
                assert_eq!(colour, if forced.is_some() { p.ink } else { p.muted }, "{at}");
                assert_eq!(size, theme.fonts.mono_s.size, "{at}");
                assert!((word_rect.left() - (rect.left() + rect.width() - word_rect.width().ceil())).abs() <= 1.0, "{at}");
                assert!(chip.right() < word_rect.left(), "{at}");
                // The word stands in the middle of the hint's height.
                assert!((word_rect.center().y - rect.center().y).abs() <= 1.0, "{at}");
                let filled = fills(&paint);
                if forced.is_some() {
                    assert_eq!(filled, vec![(rect.expand2(vec2(4.0, 2.0)), p.grid)], "{at}");
                } else {
                    assert!(filled.is_empty(), "{at}");
                }
                assert!(!paint.shapes.iter().any(|shape| shape.overflows), "{at}");
            }
        }
    }
}

#[test]
fn a_key_hint_takes_no_tab_stop_and_rests() {
    let theme = Theme::new(Mode::Light, TextScale::default());
    let mut harness = staged(
        theme,
        |p| p.panel,
        vec2(300.0, 100.0),
        |ui, theme| {
            key_hint(ui, theme, "Ctrl K", "commands", None);
        },
    );
    assert!(keyboard_walk(&mut harness).is_empty());
    harness.key_press(egui::Key::Tab);
    harness.run();
    assert!(!harness.get_by_label(&spoken("Ctrl K", "commands")).accesskit_node().is_focused(), "information takes no Tab stop");
    assert!(rests(&mut harness, 8));
}

#[test]
fn a_click_on_the_hint_is_its_press_and_hover_lights_the_word() {
    use std::cell::Cell;
    use std::rc::Rc;
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let pressed = Rc::new(Cell::new(0_u32));
    let seen = Rc::clone(&pressed);
    let mut harness = builder().with_size(vec2(300.0, 100.0)).build_ui(move |ui| {
        stage(ui, theme.palette.panel, |ui| {
            if key_hint(ui, &theme, "D", "done", None).1 {
                seen.set(seen.get() + 1);
            }
        });
    });
    harness.run();
    let name = spoken("D", "done");
    harness.get_by_label(&name).hover();
    harness.run();
    let lit = texts(&painted(&harness, &name)).into_iter().find(|text| text.1 == "done").map(|text| text.2);
    assert_eq!(lit, Some(theme.palette.ink), "hovered, the word is in ink");
    harness.get_by_label(&name).click();
    harness.run();
    assert_eq!(pressed.get(), 1);
}

#[test]
fn a_long_word_is_cut_inside_the_room_and_costs_no_more() {
    let theme = Theme::new(Mode::Light, TextScale::default());
    let long: &'static str = Box::leak("w\u{202e}\u{0}\r\n".repeat(2_500).into_boxed_str());
    let mut harness = builder().allow_missing_glyphs().with_size(vec2(300.0, 100.0)).build_ui(move |ui| {
        stage(ui, theme.palette.panel, |ui| {
            ui.set_max_width(200.0);
            key_hint(ui, &theme, "D", long, None);
        });
    });
    harness.run();
    // A label's words are its value.
    let node =
        harness.query_all_by_role(egui::accesskit::Role::Label).find(|node| node.accesskit_node().value().is_some_and(|l| l.starts_with("D w")));
    let Some(node) = node else { panic!("no hint") };
    let rect = node.rect();
    assert!(rect.width() <= 200.0, "{rect:?}");
    let label = node.accesskit_node().value().unwrap_or_default();
    let paint = painted(&harness, &label);
    for (text, shown, ..) in texts(&paint) {
        assert!(rect.contains_rect(text), "{shown:?} at {text:?} leaves {rect:?}");
    }
    assert!(label.chars().count() <= crate::component::MOST + 2, "capped: {}", label.chars().count());
}
