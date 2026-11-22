// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::{StrokeKind, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

use super::{quote_fold, quote_fold_with, spoken};
use crate::component::Shown;
use crate::component::tests::{Look, painted_in, staged};
use crate::theme::{Mode, TextScale, Theme};

const SAID: &str = "On Mon 16 Nov 2026 at 14:02, Teodor Vasquez wrote:";
const QUOTED: &str = "Hi Odile, two directions attached. The first is close to the sign they have now. The second keeps the colours.";

/// The fold 560 wide on the reader's `surface`.
fn harness(theme: Theme, open: bool, shown: Shown) -> Harness<'static> {
    staged(
        theme,
        |p| p.surface,
        vec2(560.0 + 48.0, 200.0),
        move |ui, theme| {
            ui.set_width(560.0);
            quote_fold_with(ui, theme, 14, open, Some(SAID), QUOTED, Some(shown));
        },
    )
}

#[test]
fn the_quote_fold_looks_as_its_sheet_says_in_every_state() {
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::default());
        let p = theme.palette;
        let f = theme.fonts;
        for (state, open, shown) in
            [("folded", false, Shown::Rest), ("open", true, Shown::Rest), ("hover", false, Shown::Hover), ("focus", false, Shown::Focus)]
        {
            let harness = harness(theme, open, shown);
            let name = spoken(14, open);
            let head = harness.get_by_label(&name).rect();
            // The whole quote: the line and, open, the text under it.
            let paint = painted_in(&harness, &name, egui::Rect::from_min_size(head.min, vec2(560.0, 150.0)));
            let at = format!("{mode:?} {state}: {paint:#?}");
            assert_eq!(head.size(), vec2(560.0, 28.0), "{at}");
            // The 2 px line edge at the left, down the whole height.
            let edge = paint.rects().find(|shape| shape.rect.left() == head.left() && shape.rect.width() == 2.0);
            let edge = edge.unwrap_or_else(|| panic!("the edge: {at}"));
            assert!(matches!(edge.look, Look::Rect { fill, .. } if fill == p.line), "{at}");
            let stage = paint.rects().any(|shape| shape.rect == head && matches!(shape.look, Look::Rect { fill, .. } if fill == p.stage));
            assert_eq!(stage, shown == Shown::Hover, "{at}");
            let words = if open { " hide quoted text" } else { " 14 quoted lines" };
            let ink = if shown == Shown::Hover { p.ink } else { p.muted };
            let line = paint.shapes.iter().find_map(|shape| match &shape.look {
                Look::Text { text, colour, size, .. } if text == words => Some((shape.rect, *colour, *size)),
                _ => None,
            });
            let (rect, colour, size) = line.unwrap_or_else(|| panic!("the line's words: {at}"));
            assert_eq!((colour, size), (ink, f.mono_s.size), "{at}");
            assert!((rect.left() - (head.left() + 12.0 + 5.5)).abs() < 0.6, "after the caret 12 in: {at}");
            // The caret drawn in code 12 in, in the words' colour.
            assert!(
                paint
                    .rects()
                    .any(|shape| matches!(shape.look, Look::Rect { fill, .. } if fill == ink)
                        && (shape.rect.left() - (head.left() + 12.0)).abs() < 0.5),
                "the caret: {at}"
            );
            if open {
                let said = paint.shapes.iter().find(|shape| matches!(&shape.look, Look::Text { text, .. } if text.starts_with("On Mon")));
                let said = said.unwrap_or_else(|| panic!("the attribution: {at}"));
                assert!(matches!(&said.look, Look::Text { colour, size, .. } if *colour == p.muted && *size == f.caption.size), "{at}");
                assert_eq!((said.rect.left(), said.rect.top()), (head.left() + 12.0, head.bottom()), "{at}");
                assert!(edge.rect.height() > 28.0 + 18.0 + 22.0, "the edge runs down the quote: {at}");
            }
            let ring = paint
                .rects()
                .find(|shape| matches!(shape.look, Look::Rect { stroke: Some((1.0, c)), stroke_kind: StrokeKind::Inside, .. } if c == p.cyan_edge));
            assert_eq!(ring.map(|ring| ring.rect), (shown == Shown::Focus).then(|| head.shrink(2.0)), "{at}");
        }
    }
}

#[test]
fn a_click_on_the_line_opens_the_quote() {
    let theme = Theme::new(Mode::Light, TextScale::default());
    let clicked = std::rc::Rc::new(std::cell::Cell::new(false));
    let kept = clicked.clone();
    let mut harness = staged(
        theme,
        |p| p.surface,
        vec2(600.0, 120.0),
        move |ui, theme| {
            if quote_fold(ui, theme, 3, false, None, "quoted").clicked() {
                kept.set(true);
            }
        },
    );
    harness.get_by_label("Show 3 quoted lines").click();
    harness.run();
    assert!(clicked.get());
}

#[test]
fn the_fold_is_as_high_as_the_scaled_size_at_130_percent() {
    let theme = Theme::new(Mode::Dark, TextScale::ALL[4]);
    let harness = harness(theme, false, Shown::Rest);
    assert_eq!(harness.get_by_label(&spoken(14, false)).rect().height(), theme.size(28.0));
}

#[test]
fn an_open_quote_s_words_reach_a_screen_reader_whole() {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let harness = harness(theme, true, Shown::Rest);
    harness.get_by_label(SAID);
    harness.get_by_label(QUOTED);
    let folded = super::super::tests::staged(
        theme,
        |p| p.surface,
        vec2(608.0, 200.0),
        |ui, theme| {
            ui.set_width(560.0);
            quote_fold(ui, theme, 14, false, Some(SAID), QUOTED);
        },
    );
    assert!(folded.query_by_label(QUOTED).is_none(), "a folded quote says only how long it is");
}

#[test]
fn an_open_quote_keeps_its_line_breaks() {
    // A signature: three lines, each short enough to fit the line alone.
    const LINES: &str = "Odile Brannock\n12 Example Street\nexample.org";
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let harness = staged(
        theme,
        |p| p.surface,
        vec2(608.0, 200.0),
        |ui, theme| {
            ui.set_width(560.0);
            quote_fold(ui, theme, 3, true, None, LINES);
        },
    );
    let head = harness.get_by_label(&spoken(3, true)).rect();
    let paint = painted_in(&harness, &spoken(3, true), egui::Rect::from_min_size(head.min, vec2(560.0, 150.0)));
    let rows: Vec<(String, f32)> = paint
        .shapes
        .iter()
        .filter_map(|shape| match &shape.look {
            Look::Text { text, .. } if !text.contains("quoted") => Some((text.clone(), shape.rect.top())),
            _ => None,
        })
        .collect();
    let words: Vec<&str> = rows.iter().map(|(text, _)| text.as_str()).collect();
    assert_eq!(words, ["Odile Brannock", "12 Example Street", "example.org"], "{rows:?}");
    assert!(rows.windows(2).all(|pair| pair[0].1 < pair[1].1), "one under the other: {rows:?}");
}

#[test]
fn a_quote_s_lines_are_cleaned_but_kept() {
    use std::borrow::Cow;
    // A quote with only its line breaks is not copied.
    assert!(matches!(super::super::plain_lines("one\ntwo"), Cow::Borrowed("one\ntwo")));
    // Other controls become spaces and overrides go; the breaks stay.
    assert_eq!(super::super::plain_lines("a\u{202E}b\tc\r\nd\0e"), "ab c \nd e");
}

#[test]
fn an_open_quote_keeps_its_edge_when_its_head_has_scrolled_away() {
    let theme = Theme::new(Mode::Light, TextScale::default());
    let line = theme.palette.line;
    let text = "Quoted words of a long message, kept with their line breaks.\n".repeat(300);
    let mut harness =
        Harness::builder().with_size(vec2(800.0, 600.0)).with_max_steps(64).with_context_setup(crate::theme::install_fonts).build_ui(move |ui| {
            crate::component::scroll::show(ui, &theme, egui::ScrollArea::vertical().auto_shrink([false, false]), |ui| {
                quote_fold(ui, &theme, 300, true, Some(SAID), &text);
            });
        });
    harness.run();
    // The 2 px `line` edges painted in the last frame.
    let edges = |harness: &Harness<'_>| {
        fn walk(shape: &egui::Shape, line: egui::Color32, out: &mut Vec<egui::Rect>) {
            match shape {
                egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, line, out)),
                egui::Shape::Rect(rect) if rect.fill == line && rect.rect.width() == 2.0 => out.push(rect.rect),
                _ => {}
            }
        }
        let mut found = Vec::new();
        harness.output().shapes.iter().for_each(|clipped| walk(&clipped.shape, line, &mut found));
        found
    };
    assert!(!edges(&harness).is_empty(), "the edge at the top");
    harness.hover_at(egui::pos2(300.0, 300.0));
    harness.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: vec2(0.0, -2_000.0),
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Unknown,
        modifiers: egui::Modifiers::NONE,
    });
    harness.run();
    let after = edges(&harness);
    assert!(after.iter().any(|edge| edge.top() < 0.0 && edge.bottom() > 600.0), "the open quote's edge runs on past its head: {after:?}");
}
