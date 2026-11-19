// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The count tile's sheet, state by state, in dark and light, at 100 % and 130 %; its press by the keyboard; hostile
//! labels.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use egui::{Color32, Key, Rect, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::{CountTile, Tone, count_tile, spoken, written};
use crate::component::Shown;
use crate::component::gallery_home::{TILE_WIDTH, TileState, tile, tile_of, tile_states};
use crate::component::tests::{Look, builder, own, painted, stage, staged};
use crate::motion;
use crate::theme::{Mode, TextScale, Theme};

/// The tile of `state` on a `surface` stage in `theme`.
fn harness(theme: Theme, state: TileState) -> Harness<'static> {
    staged(
        theme,
        |p| p.surface,
        vec2(TILE_WIDTH + 48.0, 160.0),
        move |ui, theme| {
            tile(ui, theme, &state);
        },
    )
}

/// [`harness`] that draws a letter its faces lack as a box instead of failing the test.
fn glyphless(theme: Theme, state: TileState) -> Harness<'static> {
    let mut harness = builder().allow_missing_glyphs().with_size(vec2(TILE_WIDTH + 48.0, 160.0)).build_ui(move |ui| {
        motion::set_reduced(ui.ctx(), true);
        stage(ui, theme.palette.surface, |ui| {
            tile(ui, &theme, &state);
        });
    });
    harness.run();
    harness
}

/// Every text the widget `name` painted: `(rect, text, colour, size)`.
fn texts(harness: &Harness<'_>, name: &str) -> Vec<(Rect, String, Color32, f32)> {
    painted(harness, name)
        .shapes
        .iter()
        .filter_map(|shape| match &shape.look {
            Look::Text { text, colour, size, .. } => Some((shape.rect, text.clone(), *colour, *size)),
            _ => None,
        })
        .collect()
}

#[test]
fn a_count_is_written_as_the_sheet_writes_it_and_caps_at_9_999_plus() {
    assert_eq!(written(0), "0");
    assert_eq!(written(7), "7");
    assert_eq!(written(1_204), "1\u{202f}204");
    assert_eq!(written(9_999), "9\u{202f}999");
    assert_eq!(written(10_000), "9\u{202f}999+");
    assert_eq!(written(u64::MAX), "9\u{202f}999+");
}

#[test]
fn a_count_tile_looks_as_its_sheet_says_in_every_state_at_100_and_130_percent() {
    for (mode, scale) in [Mode::Dark, Mode::Light].into_iter().flat_map(|mode| [(mode, TextScale::default()), (mode, TextScale::ALL[4])]) {
        let theme = Theme::new(mode, scale);
        let (p, f) = (theme.palette, theme.fonts);
        for state in tile_states() {
            let mut harness = harness(theme, state);
            let name = spoken(&tile_of(&state));
            if state.shown == Shown::Focus {
                harness.get_by_label(&name).focus();
                harness.run();
            }
            let rect = harness.get_by_label(&name).rect();
            let at = format!("{mode:?} {:?} {}", theme.text_scale, state.name);
            let o = rect.min;
            // The zero state's label takes two lines: the sheet's 92; the others 76.
            let sheet = if state.name == "zero" { 92.0 } else { 76.0 };
            assert_eq!(rect.size(), vec2(140.0, theme.size(sheet)), "{at}");
            let found = own(&painted(&harness, &name), rect);
            let hover = state.shown == Shown::Hover;
            assert_eq!(found.fill, hover.then_some(p.stage), "{at}: {found:?}");
            assert_eq!(found.edge, (state.shown == Shown::Focus).then_some((1.0, p.cyan_edge, false)), "{at}: {found:?}");
            assert_eq!(found.radius, 0.0, "{at}");
            let texts = texts(&harness, &name);
            let count = texts.iter().find(|t| t.3 == f.title.size).cloned().unwrap_or_else(|| panic!("the count: {at}: {texts:?}"));
            let ink = match (state.count, state.tone) {
                (0, _) => p.muted,
                (_, Tone::Plain) => p.ink,
                (_, Tone::Accent) => p.cyan_text,
                (_, Tone::Warn) => p.warn,
                (_, Tone::Minus) => p.minus,
            };
            assert_eq!((count.0.min - o, count.1.as_str(), count.2), (vec2(16.0, 14.0), written(state.count).as_str(), ink), "{at}");
            let labels: Vec<_> = texts.iter().filter(|t| t.3 == f.label.size).collect();
            assert_eq!(labels.len(), 1, "one label: {at}");
            let label = labels.first().map(|t| (*t).clone()).unwrap_or_else(|| panic!("{at}"));
            let label_top = (14.0 + f.title.line + 2.0).round();
            assert_eq!((label.0.min - o, label.2), (vec2(16.0, label_top), if hover { p.ink } else { p.muted }), "{at}");
            assert!(label.1.to_uppercase() == label.1 && label.1.replace('\n', " ") == state.label.to_uppercase(), "{at}: {:?}", label.1);
            assert!(label.0.right() <= rect.right() - 16.0 + 0.51, "{at}: the label stays 16 in");
            assert!(texts.iter().all(|t| rect.contains_rect(t.0)), "{at}: nothing leaves the tile");
        }
    }
}

#[test]
fn the_overflow_tile_reads_9_999_plus() {
    let state = tile_states()[5];
    assert_eq!(state.name, "overflow");
    let harness = harness(Theme::new(Mode::Dark, TextScale::default()), state);
    let name = spoken(&tile_of(&state));
    assert!(name.starts_with("9\u{202f}999+ "), "{name}");
    assert!(texts(&harness, &name).iter().any(|t| t.1 == "9\u{202f}999+"));
}

/// A tile that counts its presses, beside a second one, on `surface`.
fn pressed_harness(presses: Arc<AtomicUsize>) -> Harness<'static> {
    let theme = Theme::new(Mode::Light, TextScale::default());
    let mut harness = builder().with_size(vec2(400.0, 200.0)).build_ui(move |ui| {
        motion::set_reduced(ui.ctx(), true);
        ui.horizontal(|ui| {
            for (name, count) in [("first", 3), ("second", 9)] {
                ui.allocate_ui(vec2(TILE_WIDTH, 120.0), |ui| {
                    ui.set_width(TILE_WIDTH);
                    let tile = CountTile { id: egui::Id::unique(name), count, label: "To read", tone: Tone::Accent, row: None };
                    if count_tile(ui, &theme, &tile, None).clicked() {
                        presses.fetch_add(usize::try_from(count).unwrap_or(0), Ordering::SeqCst);
                    }
                });
            }
        });
    });
    harness.run();
    harness
}

#[test]
fn tab_reaches_the_tiles_in_the_drawn_order_and_enter_and_space_press_them() {
    let presses = Arc::new(AtomicUsize::new(0));
    let mut harness = pressed_harness(Arc::clone(&presses));
    harness.key_press(Key::Tab);
    harness.run();
    assert!(harness.get_by_label("3 To read").accesskit_node().is_focused(), "the first tile first");
    harness.key_press(Key::Enter);
    harness.run();
    assert_eq!(presses.load(Ordering::SeqCst), 3, "Enter presses the first");
    harness.key_press(Key::Tab);
    harness.run();
    assert!(harness.get_by_label("9 To read").accesskit_node().is_focused(), "then the second");
    harness.key_press(Key::Space);
    harness.run();
    assert_eq!(presses.load(Ordering::SeqCst), 12, "Space presses the second");
    harness.key_press_modifiers(egui::Modifiers::SHIFT, Key::Tab);
    harness.run();
    assert!(harness.get_by_label("3 To read").accesskit_node().is_focused(), "Shift Tab goes back");
    harness.get_by_label("9 To read").click();
    harness.run();
    assert_eq!(presses.load(Ordering::SeqCst), 21, "a click presses it");
}

/// Hostile labels: control characters, bidi overrides, 10 000 characters, 10 000 combining marks, right-to-left words
/// (the mono face holds no Hebrew or Arabic letters: drawn as boxes).
fn hostile() -> Vec<&'static str> {
    [
        format!("To\u{0}\r\nre\u{202E}ad {}", "x".repeat(10_000)),
        format!("e{}", "\u{301}".repeat(10_000)),
        "שלום עולם, ".repeat(1_000),
        "مرحبا ".repeat(2_000),
    ]
    .into_iter()
    .map(|text| &*Box::leak(text.into_boxed_str()))
    .collect()
}

#[test]
fn a_hostile_label_stays_in_the_tile_on_two_lines_at_most_cut_once() {
    let theme = Theme::new(Mode::Dark, TextScale::ALL[4]);
    for label in hostile() {
        let state = TileState { name: "hostile", count: 5, label, tone: Tone::Plain, shown: Shown::Rest };
        // The harness draws a letter its faces lack as a box, as the app does (egui's own policy).
        let harness = glyphless(theme, state);
        let name = spoken(&tile_of(&state));
        assert!(name.chars().count() <= crate::component::MOST + 8, "what is spoken is capped too");
        let rect = harness.get_by_label(&name).rect();
        assert!(rect.height() <= theme.size(92.0), "two lines at most: {rect:?}");
        let paint = painted(&harness, &name);
        assert!(paint.shapes.iter().all(|shape| !shape.overflows), "nothing leaves the tile");
        for (at, words, _, _) in texts(&harness, &name) {
            assert!(rect.contains_rect(at), "{at:?}");
            assert!(!words.contains(['\0', '\r', '\n', '\u{202E}']), "no control or bidi character is drawn");
            assert!(words.chars().count() <= crate::component::MOST + 2, "at most the cap is laid out");
        }
        let f = theme.fonts;
        let label: Vec<_> = texts(&harness, &name).into_iter().filter(|t| t.3 == f.label.size).collect();
        assert_eq!(label.len(), 1);
        assert!(
            label.iter().all(|t| t.1.matches('…').count() == 1),
            "cut once: {:?}",
            label.iter().map(|t| t.1.chars().rev().take(8).collect::<String>()).collect::<Vec<_>>()
        );
    }
}

#[test]
fn a_row_of_tiles_shares_the_tallest_tiles_height() {
    // Home's row of counts: four tiles, one with a two-line label; each takes the row's height, hovered fill and all.
    for scale in [TextScale::default(), TextScale::ALL[4]] {
        let theme = Theme::new(Mode::Dark, scale);
        let labels = ["To read", "To screen", "Nothing to read", "In the outbox"];
        let mut harness = builder().with_size(vec2(4.0 * TILE_WIDTH + 48.0, 200.0)).build_ui(move |ui| {
            motion::set_reduced(ui.ctx(), true);
            stage(ui, theme.palette.surface, |ui| {
                let row = labels.iter().map(|label| super::height(ui, &theme, label, TILE_WIDTH)).fold(0.0_f32, f32::max);
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    for (n, label) in labels.into_iter().enumerate() {
                        ui.allocate_ui(vec2(TILE_WIDTH, row), |ui| {
                            ui.set_width(TILE_WIDTH);
                            let tile = CountTile { id: egui::Id::unique(n), count: 3, label, tone: Tone::Plain, row: Some(row) };
                            count_tile(ui, &theme, &tile, None);
                        });
                    }
                });
            });
        });
        harness.run();
        harness.get_by_label("3 To read").hover();
        harness.run();
        let rects: Vec<Rect> = labels.iter().map(|label| harness.get_by_label(&format!("3 {label}")).rect()).collect();
        assert!(rects.iter().all(|rect| rect.size() == vec2(140.0, theme.size(92.0))), "{scale:?}: {rects:?}");
        let first = rects.first().copied().unwrap_or(Rect::NOTHING);
        assert_eq!(own(&painted(&harness, "3 To read"), first).fill, Some(theme.palette.stage), "the hover fills the row's height");
    }
}
