// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::accesskit::Role;
use egui::{Color32, Rect, StrokeKind, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::{settings_row, settings_row_soon, spoken};
use crate::component::Shown;
use crate::component::segmented::segmented;
use crate::component::tests::{Look, Paint, bring, own, painted, rests, staged};
use crate::theme::{Mode, TextScale, Theme};

const TITLE: &str = "Motion";
const HELP: &str = "Reduced keeps what explains a change and drops the text effects. Off: everything at once.";
const SOON_TITLE: &str = "Calendar alerts";
const SOON_REASON: &str = "Comes with the Calendar settings page.";

/// The motion choice of the sheet: full, reduced (chosen), off.
fn motion_choice(ui: &mut egui::Ui, theme: &Theme) {
    let f = theme.fonts.button;
    segmented(ui, theme, &[(0, "full", f), (1, "reduced", f), (2, "off", f)], 1);
}

/// A settings page's `surface` with one row of the sheet.
fn harness(theme: Theme, help: bool) -> Harness<'static> {
    staged(
        theme,
        |p| p.surface,
        vec2(640.0 + 48.0, 140.0),
        move |ui, theme| {
            settings_row(ui, theme, TITLE, help.then_some(HELP), |ui| motion_choice(ui, theme));
        },
    )
}

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

/// The focus edges: 1 px `cyan_edge` strokes inside their rect.
fn edges(paint: &Paint, colour: Color32) -> Vec<Rect> {
    paint
        .rects()
        .filter_map(|shape| match shape.look {
            Look::Rect { stroke: Some((width, c)), stroke_kind: StrokeKind::Inside, dashed: false, .. } if width == 1.0 && c == colour => {
                Some(shape.rect)
            }
            _ => None,
        })
        .collect()
}

#[test]
fn a_settings_row_looks_as_its_sheet_says_by_default_hovered_focused_and_with_help() {
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::default());
        let p = theme.palette;
        let f = theme.fonts;
        for (state, help, shown) in
            [("default", false, Shown::Rest), ("hover", false, Shown::Hover), ("focus", false, Shown::Focus), ("with-help", true, Shown::Rest)]
        {
            let mut harness = harness(theme, help);
            match shown {
                Shown::Focus => {
                    harness.get_by_role(Role::RadioGroup).focus();
                    harness.run();
                }
                _ => bring(&mut harness, &spoken(TITLE, help.then_some(HELP)), shown),
            }
            let name = spoken(TITLE, help.then_some(HELP));
            let rect = harness.get_by_label(&name).rect();
            let paint = painted(&harness, &name);
            let at = format!("{mode:?} {state}: {paint:#?}");
            let high = if help { 84.0 } else { 56.0 };
            assert_eq!(rect.size(), vec2(640.0, high), "{at}");
            let found = own(&paint, rect);
            assert_eq!(found.fill, if shown == Shown::Hover { Some(p.stage) } else { None }, "{at}");
            assert!(found.lines.contains(&p.panel_line), "the line under it: {at}");
            let line = paint.rects().find(|shape| matches!(shape.look, Look::Rect { stroke: Some((_, c)), .. } if c == p.panel_line));
            assert_eq!(line.map(|shape| shape.rect.top()), Some(rect.bottom() - 0.5), "{at}");
            let words = texts(&paint);
            let title = words.iter().find(|text| text.1 == TITLE).map_or(Rect::NOTHING, |text| text.0);
            let title_top = if help { 12.0 } else { 17.0 };
            assert_eq!(title.min - rect.min, vec2(16.0, title_top), "{at}");
            assert!(words.iter().any(|text| text.1 == TITLE && text.2 == p.ink && text.3 == f.strong.size), "{at}");
            if help {
                let help = words.iter().find(|text| text.1 == HELP).map_or(Rect::NOTHING, |text| text.0);
                assert_eq!((help.min - rect.min, help.height()), (vec2(16.0, 36.0), 36.0), "two lines of help: {at}");
                assert!(words.iter().any(|text| text.1 == HELP && text.2 == p.muted && text.3 == f.caption.size), "{at}");
            }
            let control = harness.get_by_role(Role::RadioGroup).rect();
            let control_top = if help { 26.0 } else { 12.0 };
            assert_eq!((control.min - rect.min, control.size()), (vec2(437.0, control_top), vec2(187.0, 32.0)), "{at}");
            let ring = edges(&paint, p.cyan_edge).into_iter().filter(|edge| edge.width() > 600.0).collect::<Vec<_>>();
            if shown == Shown::Focus {
                let inside = Rect::from_min_max(rect.min + vec2(2.0, 2.0), pos2(rect.right() - 2.0, rect.bottom() - 3.0));
                assert_eq!(ring, vec![inside], "the focus edge 2,2 636 × 51: {at}");
            } else {
                assert!(ring.is_empty(), "{at}");
            }
        }
    }
}

#[test]
fn an_unavailable_settings_row_looks_as_its_sheet_says() {
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::default());
        let p = theme.palette;
        let harness = staged(theme, |p| p.surface, vec2(640.0 + 48.0, 140.0), |ui, theme| settings_row_soon(ui, theme, SOON_TITLE, SOON_REASON));
        let name = spoken(SOON_TITLE, Some(SOON_REASON));
        let rect = harness.get_by_label(&name).rect();
        let paint = painted(&harness, &name);
        let at = format!("{mode:?}: {paint:#?}");
        assert_eq!(rect.size(), vec2(640.0, 66.0), "{at}");
        let words = texts(&paint);
        let place = |words: &[(Rect, String, Color32, f32)], wanted: &str| words.iter().find(|text| text.1 == wanted).map(|text| (text.0, text.2));
        let (title, ink) = place(&words, SOON_TITLE).unwrap_or((Rect::NOTHING, Color32::TRANSPARENT));
        assert_eq!((title.min - rect.min, ink), (vec2(16.0, 12.0), p.muted), "{at}");
        let (help, ink) = place(&words, SOON_REASON).unwrap_or((Rect::NOTHING, Color32::TRANSPARENT));
        assert_eq!((help.min - rect.min, ink), (vec2(16.0, 36.0), p.muted), "{at}");
        let tag = harness.get_by_label("coming soon").rect();
        assert_eq!((tag.min - rect.min, tag.size()), (vec2(16.0 + title.width().ceil() + 8.0, 13.0), vec2(85.0, 20.0)), "{at}");
        let switch = harness.get_by_role(Role::Switch);
        assert!(switch.accesskit_node().is_disabled(), "{at}");
        assert_eq!((switch.rect().min - rect.min, switch.rect().size()), (vec2(596.0, 25.0), vec2(28.0, 16.0)), "{at}");
        assert!(paint.shapes.iter().any(|shape| matches!(shape.look, Look::Rect { dashed: true, .. }) && shape.rect == switch.rect()), "{at}");
    }
}

#[test]
fn a_settings_row_hovers_at_once_and_rests() {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let mut harness = crate::component::tests::staged_motion(
        theme,
        false,
        |p| p.surface,
        vec2(700.0, 140.0),
        |ui, theme| {
            settings_row(ui, theme, TITLE, None, |ui| motion_choice(ui, theme));
        },
    );
    let rect = harness.get_by_label(TITLE).rect();
    harness.get_by_label(TITLE).hover();
    harness.step();
    assert_eq!(own(&painted(&harness, TITLE), rect).fill, Some(theme.palette.stage), "in the first frame");
    assert!(rests(&mut harness, 8));
}

#[test]
fn a_settings_row_is_at_least_the_scaled_height_at_130_percent() {
    let theme = Theme::new(Mode::Light, TextScale::ALL[4]);
    let harness = harness(theme, false);
    let rect = harness.get_by_label(TITLE).rect();
    assert!(rect.height() >= theme.boxes().settings_row, "{rect:?}");
    assert_eq!(rect.height(), rect.height().round(), "whole points: {rect:?}");
}
