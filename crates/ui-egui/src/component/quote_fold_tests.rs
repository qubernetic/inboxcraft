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
