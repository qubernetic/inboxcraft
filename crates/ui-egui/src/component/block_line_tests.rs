// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The block line's sheet, state by state, in dark and light, at 100 % and 130 %; the press of its link; a hostile
//! empty text.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use egui::accesskit::Role;
use egui::{Color32, Rect, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::BlockLine;
use crate::component::Shown;
use crate::component::gallery::keymap;
use crate::component::gallery_home::{BLOCK_WIDTH, LineState, line, line_states};
use crate::component::tests::{Look, builder, own, painted, staged};
use crate::motion;
use crate::theme::{Mode, TextScale, Theme};

/// Where the stage puts the line: its 16 in from the window's own 8.
const AT: egui::Pos2 = pos2(24.0, 24.0);

/// The line of `state` on a `bg` stage in `theme`, 320 wide.
fn harness(theme: Theme, state: LineState) -> Harness<'static> {
    let keymap = keymap();
    staged(
        theme,
        |p| p.bg,
        vec2(BLOCK_WIDTH + 48.0, 160.0),
        move |ui, theme| {
            line(ui, theme, &keymap, &state, BLOCK_WIDTH);
        },
    )
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
fn a_block_line_looks_as_its_sheet_says_in_every_state_at_100_and_130_percent() {
    for (mode, scale) in [Mode::Dark, Mode::Light].into_iter().flat_map(|mode| [(mode, TextScale::default()), (mode, TextScale::ALL[4])]) {
        let theme = Theme::new(mode, scale);
        let (p, f) = (theme.palette, theme.fonts);
        for state in line_states() {
            let mut harness = harness(theme, state);
            let at = format!("{mode:?} {:?} {}", theme.text_scale, state.name);
            match state.line {
                BlockLine::Empty(words) => {
                    let node = harness.get_by_label(words);
                    assert_eq!(node.accesskit_node().role(), Role::Label, "information, no Tab stop: {at}");
                    let rect = node.rect();
                    // Two lines of the caption at 320: the sheet's 320 × 60.
                    assert_eq!((rect.min, rect.size()), (AT, vec2(320.0, theme.size(60.0))), "{at}");
                    let texts = texts(&harness, words);
                    assert_eq!(texts.len(), 1, "{at}: {texts:?}");
                    let text = texts.first().cloned().unwrap_or_else(|| panic!("{at}"));
                    assert_eq!((text.0.left() - AT.x, text.2, text.3), (16.0, p.muted, f.caption.size), "{at}");
                    assert!((text.0.height() - f.caption.line * 2.0).abs() < 1.0, "two lines: {at}: {:?}", text.0);
                    assert!((text.0.center().y - rect.center().y).abs() <= 0.51, "on the line's middle: {at}: {:?}", text.0);
                    assert!(text.0.right() <= rect.right() - 16.0 + 0.51, "16 in at the right: {at}");
                }
                BlockLine::More(words) => {
                    if state.shown == Shown::Hover {
                        harness.get_by_label(words).hover();
                        harness.run();
                    }
                    let node = harness.get_by_label(words);
                    assert_eq!(node.accesskit_node().role(), Role::Button, "{at}");
                    let link = node.rect();
                    let high = theme.size(32.0);
                    let link_high = theme.boxes().row_link;
                    assert_eq!((link.left() - AT.x, link.top() - AT.y, link.height()), (16.0, ((high - link_high) / 2.0).round(), link_high), "{at}");
                    let found = own(&painted(&harness, words), link);
                    assert!(found.texts.iter().any(|t| t.0 == words && t.1 == p.cyan_text && t.3 == f.mono_s.size), "{at}: {found:?}");
                    assert_eq!(found.lines.contains(&p.cyan_text), state.shown == Shown::Hover, "underlined under the pointer: {at}: {found:?}");
                }
            }
        }
    }
}

#[test]
fn the_more_line_is_32_high_at_100_and_42_at_130_percent() {
    for (scale, high) in [(TextScale::default(), 32.0), (TextScale::ALL[4], 42.0)] {
        let theme = Theme::new(Mode::Light, scale);
        let state = line_states()[1];
        let harness = builder().with_size(vec2(400.0, 200.0)).build_ui(move |ui| {
            let keymap = keymap();
            ui.set_width(BLOCK_WIDTH);
            let top = ui.cursor().min.y;
            line(ui, &theme, &keymap, &state, BLOCK_WIDTH);
            ui.label(format!("{}", ui.cursor().min.y - top - ui.spacing().item_spacing.y));
        });
        assert!(harness.query_by_label(&format!("{high}")).is_some(), "{scale:?}");
    }
}

#[test]
fn the_more_link_returns_its_press_by_pointer_and_keyboard() {
    let presses = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&presses);
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let mut harness = builder().with_size(vec2(400.0, 200.0)).build_ui(move |ui| {
        motion::set_reduced(ui.ctx(), true);
        let keymap = keymap();
        if line(ui, &theme, &keymap, &line_states()[1], BLOCK_WIDTH) {
            counted.fetch_add(1, Ordering::SeqCst);
        }
    });
    harness.run();
    harness.get_by_label("all drafts (5) →").click();
    harness.run();
    assert_eq!(presses.load(Ordering::SeqCst), 1);
    harness.key_press(egui::Key::Tab);
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();
    assert_eq!(presses.load(Ordering::SeqCst), 2);
}

#[test]
fn a_hostile_empty_text_stays_in_the_line_on_three_lines_at_most_cut_once() {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let texts_of = [
        format!("No\u{0}\r\ndrafts \u{202E}{}", "x ".repeat(5_000)),
        format!("e{}", "\u{301}".repeat(10_000)),
        format!("\u{2067}{}\u{2069}", "abc ".repeat(2_500)),
    ];
    for text in texts_of {
        let text: &'static str = Box::leak(text.into_boxed_str());
        let state = LineState { name: "hostile", line: BlockLine::Empty(text), shown: Shown::Rest };
        let harness = harness(theme, state);
        let node = harness
            .root()
            .children_recursive()
            .find(|node| node.accesskit_node().role() == Role::Label)
            .map(|node| (node.rect(), node.accesskit_node().label().or_else(|| node.accesskit_node().value())));
        let (rect, name) = node.unwrap_or_else(|| panic!("the line"));
        let name = name.unwrap_or_default();
        assert!(rect.height() <= 12.0 * 2.0 + 18.0 * 3.0, "three lines at most: {rect:?}");
        assert!(name.chars().count() <= crate::component::MOST + 1, "what is spoken is capped");
        assert!(!name.contains(['\0', '\r', '\n', '\u{202E}', '\u{2067}']), "no control or bidi character");
        let found = texts(&harness, &name);
        assert!(painted(&harness, &name).shapes.iter().all(|shape| !shape.overflows), "nothing leaves the line");
        for (at, words, _, _) in found {
            assert!(rect.contains_rect(at), "{at:?} in {rect:?}");
            assert!(words.matches('…').count() <= 1, "cut once at most");
        }
    }
}
