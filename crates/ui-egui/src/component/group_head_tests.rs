// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The group head's sheet, state by state, in dark and light, at 100 % and 130 %.

use egui::{Rect, StrokeKind, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::spoken;
use crate::component::Shown;
use crate::component::gallery::keymap;
use crate::component::gallery_lists::{HEAD_NAME, HeadState, ROW_WIDTH, head, head_states};
use crate::component::tests::{Look, own, painted, staged};
use crate::draw::paint::run_box;
use crate::theme::{Mode, TextScale, Theme};

/// The head of `state` on a `surface` stage in `theme`.
fn harness(theme: Theme, state: HeadState) -> Harness<'static> {
    let keymap = keymap();
    staged(theme, |p| p.surface, vec2(ROW_WIDTH + 48.0, 100.0), move |ui, theme| head(ui, theme, &keymap, &state))
}

/// The head's height, the top of its words and of its link, at the theme's text scale (32, 8 and 8 at 100 %).
fn places(theme: &Theme) -> (f32, f32, f32, f32) {
    let high = theme.size(32.0);
    let words = ((high - theme.fonts.label.line) / 2.0).round();
    let link = theme.boxes().row_link;
    (high, words, ((high - link) / 2.0).round(), link)
}

#[test]
fn places_at_100_are_the_sheets() {
    assert_eq!(places(&Theme::new(Mode::Light, TextScale::default())), (32.0, 8.0, 8.0, 16.0));
}

#[test]
fn a_group_head_looks_as_its_sheet_says_in_every_state_at_100_and_130_percent() {
    for (mode, scale) in [Mode::Dark, Mode::Light].into_iter().flat_map(|mode| [(mode, TextScale::default()), (mode, TextScale::ALL[4])]) {
        let theme = Theme::new(mode, scale);
        let (high, words, link_top, link_high) = places(&theme);
        let (p, f) = (theme.palette, theme.fonts);
        for state in head_states() {
            let mut harness = harness(theme, state);
            let name = spoken(HEAD_NAME, state.unread);
            if state.shown == Shown::Focus {
                harness.get_by_label(&name).focus();
                harness.run();
            }
            let rect = harness.get_by_label(&name).rect();
            let paint = painted(&harness, &name);
            let at = format!("{mode:?} {:?} {}: {paint:#?}", theme.text_scale, state.name);
            let o = rect.min;
            assert_eq!(rect.size(), vec2(440.0, high), "{at}");
            let found = own(&paint, rect);
            let fill = if state.shown == Shown::Hover { p.grid } else { p.panel };
            assert_eq!(found.fill, Some(fill), "{at}");
            assert_eq!(found.radius, 0.0, "{at}");
            assert!(found.lines.contains(&p.panel_line), "the line under it: {at}");
            let texts: Vec<(Rect, String, egui::Color32, f32)> = paint
                .shapes
                .iter()
                .filter_map(|shape| match &shape.look {
                    Look::Text { text, colour, size, .. } => Some((shape.rect, text.clone(), *colour, *size)),
                    _ => None,
                })
                .collect();
            let find = |text: &str| texts.iter().find(|t| t.1 == text).cloned().unwrap_or_else(|| panic!("{text}: {at}"));
            let index = find("[ B ]");
            assert_eq!((index.0.min - o, index.2, index.3), (vec2(16.0, words), p.cyan_text, f.label.size), "{at}");
            let group = find(&HEAD_NAME.to_uppercase());
            let ink = if state.shown == Shown::Hover { p.ink } else { p.muted };
            let name_left = index.0.left() + run_box(index.0.width()) + 8.0;
            assert_eq!((group.0.left(), group.0.top() - o.y, group.2, group.3), (name_left, words, ink, f.label.size), "{at}");
            // The count at the right, 16 in; "nothing to read" when none is unread.
            let count = if state.unread == 0 { "nothing to read".to_owned() } else { state.unread.to_string() };
            let count = find(&count);
            assert_eq!(
                (count.2, count.3, count.0.top() - o.y),
                (p.muted, f.mono_s.size, words + ((f.label.line - f.mono_s.line) / 2.0).round()),
                "{at}"
            );
            assert!((count.0.left() + run_box(count.0.width()) - (o.x + 424.0)).abs() < 0.51, "{at}");
            // The link 8 before the count, and "3 of 6 ·" 8 before the link.
            // "done all" shows hovered and, so that `Tab` reaches it, keyed (the sheet's focus state draws none).
            let lit = matches!(state.shown, Shown::Hover | Shown::Focus);
            let link = state.link.or(lit.then_some("done all"));
            assert_eq!(harness.query_by_label("done all").is_some(), lit, "{at}");
            if let Some(link) = link {
                let link = harness.get_by_label(link).rect();
                assert_eq!((link.right() + 8.0, link.top() - o.y, link.height()), (count.0.left(), link_top, link_high), "{at}");
                if let Some(rows) = state.shown_rows {
                    let rows = find(rows);
                    assert!((rows.0.left() + run_box(rows.0.width()) + 8.0 - link.left()).abs() < 0.51, "{at}");
                    assert_eq!(rows.2, p.muted, "{at}");
                }
            }
            assert!(group.0.right() <= count.0.left() - 8.0 + 0.51, "the name stops short of the right part: {at}");
            // The focus edge 2 inside, above the line: the sheet's 2,2 436 × 27.
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
fn a_group_head_is_the_scaled_height_at_130_percent() {
    let theme = Theme::new(Mode::Light, TextScale::ALL[4]);
    let state = head_states()[3];
    let harness = harness(theme, state);
    let rect = harness.get_by_label(&spoken(HEAD_NAME, state.unread)).rect();
    assert_eq!(rect.size(), vec2(440.0, theme.size(32.0)));
    let link = harness.get_by_label("show all").rect();
    assert!(link.right() <= rect.right() - 16.0 && rect.contains_rect(link), "{link:?} in {rect:?}");
}

#[test]
fn the_keyboard_on_a_head_shows_done_all_and_tab_reaches_it() {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let mut harness = harness(theme, head_states()[0]);
    assert!(harness.query_by_label("done all").is_none(), "only hovered or keyed");
    harness.key_press(egui::Key::Tab);
    harness.run();
    assert!(harness.get_by_label(&spoken(HEAD_NAME, 6)).accesskit_node().is_focused(), "Tab comes to the head");
    assert!(harness.query_by_label("done all").is_some(), "the keyed head shows it");
    harness.key_press(egui::Key::Tab);
    harness.run();
    let link = harness.query_by_label("done all").map(|node| node.accesskit_node().is_focused());
    assert_eq!(link, Some(true), "Tab goes on to it, and it stays");
}

#[test]
fn a_head_without_a_link_shows_none_hovered_or_keyed() {
    use super::{GroupHead, group_head};
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let keymap = keymap();
    let mut harness = staged(
        theme,
        |p| p.surface,
        vec2(ROW_WIDTH + 48.0, 100.0),
        move |ui, theme| {
            let head =
                GroupHead { id: egui::Id::unique("bare"), index: "[ A ]", name: "Empty", unread: 0, shown: None, link: None, hover_link: None };
            ui.set_width(ROW_WIDTH);
            group_head(ui, theme, &keymap, &head, None);
        },
    );
    harness.get_by_label(&spoken("Empty", 0)).hover();
    harness.run();
    assert!(harness.query_by_label("done all").is_none());
}
