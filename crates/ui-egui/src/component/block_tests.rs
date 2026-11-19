// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The block's sheet, state by state, in dark and light, at 100 % and 130 %; its press and its place in the Tab
//! order; hostile lines.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use egui::accesskit::Role;
use egui::{Color32, Id, Key, Rect, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::{Block, Inset, block, text_line};
use crate::component::Shown;
use crate::component::block_line::{BlockLine, block_line};
use crate::component::gallery::keymap;
use crate::component::gallery_home::{AGENDA, BLOCK_WIDTH, BlockState, agenda, block_states};
use crate::component::tests::{Look, builder, own, painted, staged};
use crate::motion;
use crate::theme::{Mode, TextScale, Theme};

/// The block of `state` on a `bg` stage in `theme`.
fn harness(theme: Theme, state: BlockState) -> Harness<'static> {
    staged(
        theme,
        |p| p.bg,
        vec2(BLOCK_WIDTH + 48.0, 220.0),
        move |ui, theme| {
            agenda(ui, theme, &state);
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

/// The block's name on the boards: its word, as a screen reader hears the button.
const NAME: &str = "Agenda";

#[test]
fn a_block_looks_as_its_sheet_says_in_every_state_at_100_and_130_percent() {
    for (mode, scale) in [Mode::Dark, Mode::Light].into_iter().flat_map(|mode| [(mode, TextScale::default()), (mode, TextScale::ALL[4])]) {
        let theme = Theme::new(mode, scale);
        let (p, f) = (theme.palette, theme.fonts);
        let head = theme.size(36.0);
        for state in block_states() {
            let mut harness = harness(theme, state);
            if state.shown == Shown::Focus {
                harness.get_by_role_and_label(Role::Button, NAME).focus();
                harness.run();
            }
            let rect = harness.get_by_role_and_label(Role::Button, NAME).rect();
            let at = format!("{mode:?} {:?} {}", theme.text_scale, state.name);
            let o = rect.min;
            // The head, its hairline, 12 over two lines 4 apart and 12 under: 36 + 1 + 12 + 20 + 4 + 20 + 12 = 105.
            assert_eq!(rect.size(), vec2(320.0, (head + 1.0 + 12.0 + f.mono.line * 2.0 + 4.0 + 12.0).round()), "{at}");
            if theme.text_scale == TextScale::default() {
                assert_eq!(rect.height(), 105.0, "the sheet's 320 × 105");
            }
            let paint = painted(&harness, NAME);
            let found = own(&paint, rect);
            let (fill, edge) = match state {
                BlockState { selected: true, .. } => (p.stage, p.cyan_edge),
                BlockState { shown: Shown::Hover, .. } => (p.surface, p.muted),
                _ => (p.surface, p.line),
            };
            assert_eq!((found.fill, found.edge), (Some(fill), Some((1.0, edge, false))), "{at}: {found:?}");
            assert_eq!(found.radius, 0.0, "{at}");
            let ring = (state.shown == Shown::Focus).then_some((rect.expand(3.0), 1.0, p.cyan_edge));
            assert_eq!(found.ring, ring, "the sheet's ring at -3,-3 326 × 111: {at}");
            // The hairline under the head, across the block.
            let hairline = paint
                .rects()
                .find(|shape| matches!(shape.look, Look::Rect { stroke: Some((_, c)), .. } if c == p.panel_line) && shape.rect.height() == 0.0);
            let hairline = hairline.map(|shape| shape.rect).unwrap_or_else(|| panic!("the hairline: {at}"));
            // A hairline's line runs through the middle of its pixels: half a pixel in from where it is drawn.
            assert_eq!((hairline.top() - o.y, hairline.left() - o.x, rect.right() - hairline.right()), (head + 0.5, 0.5, -0.5), "{at}");
            let texts = texts(&harness, NAME);
            let index = texts.iter().find(|t| t.1 == "[ 02 ]").cloned().unwrap_or_else(|| panic!("the label: {at}: {texts:?}"));
            assert_eq!((index.0.left() - o.x, index.2, index.3), (16.0, p.cyan_text, f.label.size), "{at}");
            assert!((index.0.center().y - o.y - head / 2.0).abs() <= 1.0, "on the head's middle: {at}: {:?}", index.0);
            for (n, line) in AGENDA.iter().enumerate() {
                let found = texts.iter().find(|t| t.1 == *line).cloned().unwrap_or_else(|| panic!("{line}: {at}"));
                let top = head + 1.0 + 12.0 + (f.mono.line + 4.0) * n as f32;
                assert_eq!((found.0.min - o, found.2, found.3), (vec2(16.0, top), p.ink, f.mono.size), "{line}: {at}");
            }
        }
    }
}

/// A block that presses, holding a line with a link, beside a block that does not; the presses of the first counted
/// in tens, the link's in ones.
fn walk_harness(presses: Arc<AtomicUsize>) -> Harness<'static> {
    let theme = Theme::new(Mode::Light, TextScale::default());
    let keymap = keymap();
    let mut harness = builder().with_size(vec2(400.0, 400.0)).build_ui(move |ui| {
        motion::set_reduced(ui.ctx(), true);
        ui.set_width(320.0);
        let first = Block { id: Id::unique("first"), index: "[ 02 ]", word: "Agenda", inset: Inset::Flush, presses: true, selected: false };
        let drawn = block(ui, &theme, &first, None, |ui| block_line(ui, &theme, &keymap, &BlockLine::More("all drafts (5) →"), None));
        presses.fetch_add(usize::from(drawn.pressed) * 10 + usize::from(drawn.inner), Ordering::SeqCst);
        ui.add_space(16.0);
        let second = Block { id: Id::unique("second"), index: "[ 03 ]", word: "Drafts", inset: Inset::Text, presses: false, selected: false };
        block(ui, &theme, &second, None, |ui| text_line(ui, &theme, "10:00  Workshop prep"));
    });
    harness.run();
    harness
}

#[test]
fn tab_reaches_a_block_that_presses_before_its_content_and_never_one_that_does_not() {
    let presses = Arc::new(AtomicUsize::new(0));
    let mut harness = walk_harness(Arc::clone(&presses));
    assert!(harness.query_by_role_and_label(Role::Button, "Drafts").is_none(), "a block that does not press is no button");
    harness.key_press(Key::Tab);
    harness.run();
    assert!(harness.get_by_role_and_label(Role::Button, "Agenda").accesskit_node().is_focused(), "the block first");
    harness.key_press(Key::Enter);
    harness.run();
    assert_eq!(presses.load(Ordering::SeqCst), 10, "Enter presses the block");
    harness.key_press(Key::Tab);
    harness.run();
    assert!(harness.get_by_label("all drafts (5) →").accesskit_node().is_focused(), "then its link");
    harness.key_press(Key::Space);
    harness.run();
    assert_eq!(presses.load(Ordering::SeqCst), 11, "Space presses the link, not the block");
    harness.key_press(Key::Tab);
    harness.run();
    assert!(harness.get_by_role_and_label(Role::Button, "Agenda").accesskit_node().is_focused(), "and round: the second block takes no stop");
    harness.key_press_modifiers(egui::Modifiers::SHIFT, Key::Tab);
    harness.run();
    assert!(harness.get_by_label("all drafts (5) →").accesskit_node().is_focused(), "Shift Tab goes back");
    harness.get_by_label("all drafts (5) →").click();
    harness.run();
    assert_eq!(presses.load(Ordering::SeqCst), 12, "a click on the link presses the link alone");
}

#[test]
fn a_block_rests_once_its_height_is_known() {
    let mut harness = walk_harness(Arc::new(AtomicUsize::new(0)));
    assert!(crate::component::tests::rests(&mut harness, 8));
}

/// Hostile lines: control characters, bidi overrides, 10 000 characters, 10 000 combining marks.
fn hostile() -> Vec<&'static str> {
    [format!("10:00\u{0}\r\n\u{202E}Work{}", "x".repeat(10_000)), format!("e{}", "\u{301}".repeat(10_000))]
        .into_iter()
        .map(|text| &*Box::leak(text.into_boxed_str()))
        .collect()
}

#[test]
fn a_hostile_line_stays_one_line_in_the_block_cut_once() {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    for line in hostile() {
        let harness = staged(
            theme,
            |p| p.bg,
            vec2(BLOCK_WIDTH + 48.0, 220.0),
            move |ui, theme| {
                ui.set_width(BLOCK_WIDTH);
                let shown = Block { id: Id::unique("hostile"), index: "[ 02 ]", word: "Agenda", inset: Inset::Text, presses: true, selected: false };
                block(ui, theme, &shown, None, |ui| text_line(ui, theme, line));
            },
        );
        let rect = harness.get_by_role_and_label(Role::Button, NAME).rect();
        assert_eq!(rect.height(), 36.0 + 1.0 + 12.0 + 20.0 + 12.0, "one line");
        let paint = painted(&harness, NAME);
        assert!(paint.shapes.iter().all(|shape| !shape.overflows), "nothing leaves the block");
        let lines: Vec<_> = texts(&harness, NAME).into_iter().filter(|t| t.1 != "[ 02 ]" && t.1 != "AGENDA").collect();
        assert_eq!(lines.len(), 1, "{lines:?}");
        for (at, words, _, _) in lines {
            let inside = Rect::from_min_max(rect.min + vec2(16.0, 36.0 + 1.0 + 12.0), rect.max - vec2(16.0, 12.0));
            assert!(inside.contains_rect(at), "{at:?} inside the padding {inside:?}");
            assert!(!words.contains(['\0', '\r', '\n', '\u{202E}']), "no control or bidi character is drawn");
            assert!(words.chars().count() <= crate::component::MOST + 1, "at most the cap is laid out");
            assert_eq!(words.matches('…').count(), 1, "cut once: {:?}", words.chars().rev().take(8).collect::<String>());
        }
    }
}
