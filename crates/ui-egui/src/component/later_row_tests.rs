// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The later row's sheet, state by state, in dark and light, at 100 % and 130 %.

use egui::{Color32, Rect, StrokeKind, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

use super::Pile;
use crate::component::Shown;
use crate::component::gallery::keymap;
use crate::component::gallery_lists::{LaterState, ROW_WIDTH, later, later_spoken, later_states};
use crate::component::tests::{Look, icons, own, painted, staged};
use crate::draw::paint::run_box;
use crate::theme::{Mode, TextScale, Theme};

/// The row of `state` on a `surface` stage in `theme`.
fn harness(theme: Theme, state: LaterState) -> Harness<'static> {
    let keymap = keymap();
    staged(theme, |p| p.surface, vec2(ROW_WIDTH + 48.0, 140.0), move |ui, theme| later(ui, theme, &keymap, &state))
}

#[test]
fn a_later_row_looks_as_its_sheet_says_in_every_state() {
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::default());
        let (p, f) = (theme.palette, theme.fonts);
        for state in later_states() {
            let mut harness = harness(theme, state);
            let name = later_spoken(&state);
            if state.shown == Shown::Focus {
                harness.get_by_label(&name).focus();
                harness.run();
            }
            let rect = harness.get_by_label(&name).rect();
            let paint = painted(&harness, &name);
            let at = format!("{mode:?} {}: {paint:#?}", state.name);
            let o = rect.min;
            assert_eq!(rect.size(), vec2(440.0, 56.0), "{at}");
            let found = own(&paint, rect);
            let staged = state.current || state.shown == Shown::Hover;
            assert_eq!(found.fill.filter(|fill| *fill != p.surface), staged.then_some(p.stage), "{at}");
            assert_eq!(found.radius, 0.0, "{at}");
            assert!(found.lines.contains(&p.panel_line), "{at}");
            let fills: Vec<(Rect, Color32)> = paint
                .rects()
                .filter_map(|shape| match shape.look {
                    Look::Rect { fill, stroke: None, .. } if fill.a() > 0 => Some((shape.rect, fill)),
                    _ => None,
                })
                .collect();
            let bar = Rect::from_min_size(o, vec2(2.0, 56.0));
            assert_eq!(fills.iter().any(|(r, c)| *r == bar && *c == p.cyan_edge), state.current, "{at}");
            let square = Rect::from_min_size(o + vec2(16.0, 25.0), vec2(6.0, 6.0));
            assert_eq!(fills.iter().any(|(r, c)| *r == square && *c == p.cyan_edge), state.unread, "{at}");
            let texts: Vec<(Rect, String, Color32, String, f32)> = paint
                .shapes
                .iter()
                .filter_map(|shape| match &shape.look {
                    Look::Text { text, colour, family, size, .. } => Some((shape.rect, text.clone(), *colour, family.clone(), *size)),
                    _ => None,
                })
                .collect();
            let find = |text: &str| texts.iter().find(|t| t.1 == text).cloned().unwrap_or_else(|| panic!("{text}: {at}"));
            let who = find(state.who);
            let font = if state.unread { f.strong } else { f.body };
            assert_eq!((who.0.min - o, who.2, who.4, &who.3), (vec2(34.0, 7.0), p.ink, font.size, &font.id().family.to_string()), "{at}");
            let words = if state.unread { p.ink } else { p.muted };
            let subject = find(state.subject);
            assert_eq!((subject.0.min - o, subject.2, subject.4), (vec2(34.0, 31.0), words, f.caption.size), "{at}");
            // The pile's mark at line one's right end, 14 px.
            let (_, mark, when_ink) = state.pile.look(&theme);
            let glyph = texts.iter().find(|t| t.3 == icons()).cloned().unwrap_or_else(|| panic!("the mark: {at}"));
            assert_eq!(glyph.2, mark, "{at}");
            assert!((glyph.0.center() - (o + vec2(417.0, 18.0))).length() < 1.01, "{:?}: {at}", glyph.0);
            if state.shown == Shown::Hover {
                let link = harness.get_by_label("bring back now").rect();
                assert_eq!((link.right() - o.x, link.top() - o.y, link.height()), (424.0, 32.0, 16.0), "{at}");
                assert!(texts.iter().all(|t| t.1 != state.when), "the link takes when's place: {at}");
            } else {
                let when = find(state.when);
                assert_eq!((when.0.top() - o.y, when.2, when.4), (32.0, when_ink, f.mono_s.size), "{at}");
                assert!((when.0.left() + run_box(when.0.width()) - (o.x + 424.0)).abs() < 0.51, "{at}");
            }
            if state.pile == Pile::Returned {
                let tag = harness.get_by_label("Returned").rect();
                assert_eq!((tag.top() - o.y, tag.height()), (8.0, 20.0), "{at}");
            }
            let ring: Vec<Rect> = paint
                .rects()
                .filter_map(|shape| match shape.look {
                    Look::Rect { stroke: Some((1.0, c)), stroke_kind: StrokeKind::Inside, .. } if c == p.cyan_edge && shape.rect.width() > 400.0 => {
                        Some(shape.rect)
                    }
                    _ => None,
                })
                .collect();
            let inside = Rect::from_min_max(o + vec2(2.0, 2.0), pos2(rect.right() - 2.0, rect.bottom() - 3.0));
            assert_eq!(ring, if state.shown == Shown::Focus { vec![inside] } else { vec![] }, "{at}");
        }
    }
}

#[test]
fn a_later_row_is_the_scaled_height_at_130_percent() {
    let theme = Theme::new(Mode::Light, TextScale::ALL[4]);
    let state = later_states()[3];
    let harness = harness(theme, state);
    let rect = harness.get_by_label(&later_spoken(&state)).rect();
    assert_eq!(rect.size(), vec2(440.0, theme.size(56.0)));
}
