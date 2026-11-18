// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The outbox row's sheet, state by state, in dark and light, at 100 % and 130 %.

use egui::accesskit::Role;
use egui::{Color32, Rect, StrokeKind, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

use super::Standing;
use crate::component::Shown;
use crate::component::gallery::keymap;
use crate::component::gallery_lists::{ROW_WIDTH, SEND_REPLY, SEND_SUBJECT, SEND_TO, SendState, send, send_spoken, send_states};
use crate::component::tests::{Look, icons, own, painted, staged};
use crate::draw::paint::run_box;
use crate::theme::{Mode, TextScale, Theme};

/// The row of `state` on a `surface` stage in `theme`.
fn harness(theme: Theme, state: SendState) -> Harness<'static> {
    let keymap = keymap();
    staged(theme, |p| p.surface, vec2(ROW_WIDTH + 48.0, 140.0), move |ui, theme| send(ui, theme, &keymap, &state))
}

/// The places of a row, refused or not, at the theme's text scale, from its top: its height, the top of its text,
/// the middle of its mark, the top of line two, of where it stands, of its buttons and of the reply.
fn places(theme: &Theme, refused: bool) -> [f32; 7] {
    let f = theme.fonts;
    let high = theme.size(if refused { 72.0 } else { 56.0 });
    let upper = f.body.line + 2.0 + f.caption.line;
    let block = if refused { upper + 2.0 + f.caption.line } else { upper };
    let top = ((high - block) / 2.0).round();
    let icon = theme.boxes().icon;
    let button = theme.boxes().button;
    [
        high,
        top,
        top + ((upper - icon) / 2.0).round() + icon / 2.0,
        top + f.body.line + 2.0,
        top + ((f.body.line - f.mono_s.line) / 2.0).round(),
        top + ((upper - button) / 2.0).round(),
        top + upper + 2.0,
    ]
}

#[test]
fn places_at_100_are_the_sheets() {
    let theme = Theme::new(Mode::Light, TextScale::default());
    assert_eq!(places(&theme, false), [56.0, 7.0, 28.0, 31.0, 10.0, 12.0, 51.0]);
    assert_eq!(places(&theme, true), [72.0, 5.0, 26.0, 29.0, 8.0, 10.0, 49.0]);
}

#[test]
fn an_outbox_row_looks_as_its_sheet_says_in_every_state_at_100_and_130_percent() {
    for (mode, scale) in [Mode::Dark, Mode::Light].into_iter().flat_map(|mode| [(mode, TextScale::default()), (mode, TextScale::ALL[4])]) {
        let theme = Theme::new(mode, scale);
        let (p, f) = (theme.palette, theme.fonts);
        for state in send_states() {
            let mut harness = harness(theme, state);
            let name = send_spoken(&state);
            if state.shown == Shown::Focus {
                harness.get_by_label(&name).focus();
                harness.run();
            }
            let rect = harness.get_by_label(&name).rect();
            let paint = painted(&harness, &name);
            let at = format!("{mode:?} {:?} {}: {paint:#?}", theme.text_scale, state.name);
            let o = rect.min;
            let refused = state.standing == Standing::Refused;
            let [high, top, mark_middle, two, standing, buttons_top, reply_top] = places(&theme, refused);
            assert_eq!(rect.size(), vec2(440.0, high), "{at}");
            let found = own(&paint, rect);
            let fill = found.fill.filter(|fill| *fill != p.surface);
            assert_eq!(fill, (state.shown == Shown::Hover).then_some(p.stage), "{at}");
            assert!(found.lines.contains(&p.panel_line), "{at}");
            let texts: Vec<(Rect, String, Color32, String, f32)> = paint
                .shapes
                .iter()
                .filter_map(|shape| match &shape.look {
                    Look::Text { text, colour, family, size, .. } => Some((shape.rect, text.clone(), *colour, family.clone(), *size)),
                    _ => None,
                })
                .collect();
            // A text as drawn: whole, or cut with one ellipsis (the refused row's room is narrow).
            let find = |text: &str| {
                let cut =
                    |t: &&(Rect, String, Color32, String, f32)| t.1.ends_with('…') && t.1.len() > 4 && text.starts_with(t.1.trim_end_matches('…'));
                texts.iter().find(|t| t.1 == text || cut(t)).cloned().unwrap_or_else(|| panic!("{text}: {at}"))
            };
            let (_, ink) = state.standing.look(&theme);
            // The mark: 16 px, 16 in, on the upper part's middle, in the standing's colour.
            let mark = texts.iter().find(|t| t.3 == icons()).cloned().unwrap_or_else(|| panic!("the mark: {at}"));
            assert_eq!(mark.2, ink, "{at}");
            assert!((mark.0.center() - (o + vec2(16.0 + theme.boxes().icon / 2.0, mark_middle))).length() < 1.01, "{:?}: {at}", mark.0);
            let to = find(SEND_TO);
            assert_eq!((to.0.min - o, to.2, to.4), (vec2(44.0, top), p.ink, f.body.size), "{at}");
            let subject = find(SEND_SUBJECT);
            assert_eq!((subject.0.min - o, subject.2, subject.4), (vec2(44.0, two), p.muted, f.caption.size), "{at}");
            let words = find(state.words);
            assert_eq!((words.0.top() - o.y, words.2, words.4), (standing, ink, f.mono_s.size), "{at}");
            let buttons: Vec<Rect> = state.buttons.iter().map(|button| harness.get_by_role_and_label(Role::Button, button).rect()).collect();
            let text_right = buttons.first().map_or(o.x + 424.0, |first| first.left() - 12.0);
            assert!((words.0.left() + run_box(words.0.width()) - text_right).abs() < 0.51, "where it stands ends at the text's right: {at}");
            if let Some(last) = buttons.last() {
                assert_eq!((last.right() - o.x, last.top() - o.y, last.height()), (424.0, buttons_top, theme.boxes().button), "{at}");
            }
            if buttons.len() == 2 {
                assert_eq!(buttons[1].left() - buttons[0].right(), 8.0, "{at}");
            }
            if refused {
                let reply = find(SEND_REPLY);
                assert_eq!((reply.0.min - o, reply.2, reply.4), (vec2(44.0, reply_top), p.minus, f.caption.size), "{at}");
            }
            // A held send draws the share of its hold left along its foot.
            // 2 high, its lower pixel row under the row's line, as the design draws its line over the bar.
            let bar = Rect::from_min_size(o + vec2(0.0, high - 2.0), vec2(352.0, 1.0));
            let barred =
                paint.rects().any(|shape| matches!(shape.look, Look::Rect { fill, stroke: None, .. } if fill == p.cyan_edge) && shape.rect == bar);
            assert_eq!(barred, matches!(state.standing, Standing::Countdown { .. }), "{at}");
            let ring: Vec<Rect> = paint
                .rects()
                .filter_map(|shape| match shape.look {
                    Look::Rect { stroke: Some((1.0, c)), stroke_kind: StrokeKind::Inside, .. } if c == p.cyan_edge && shape.rect.width() > 400.0 => {
                        Some(shape.rect)
                    }
                    _ => None,
                })
                .collect();
            // The sheet's 2,2 436 × 50: above the hold's bar.
            let inside = Rect::from_min_max(o + vec2(2.0, 2.0), pos2(rect.right() - 2.0, rect.bottom() - 4.0));
            assert_eq!(ring, if state.shown == Shown::Focus { vec![inside] } else { vec![] }, "{at}");
            assert_eq!(found.radius, 0.0, "{at}");
        }
    }
}

#[test]
fn an_outbox_row_is_the_scaled_height_at_130_percent() {
    let theme = Theme::new(Mode::Light, TextScale::ALL[4]);
    for state in [send_states()[0], send_states()[3]] {
        let harness = harness(theme, state);
        let rect = harness.get_by_label(&send_spoken(&state)).rect();
        let high = if state.standing == Standing::Refused { 72.0 } else { 56.0 };
        assert_eq!(rect.size(), vec2(440.0, theme.size(high)), "{}", state.name);
        for button in state.buttons {
            assert!(rect.contains_rect(harness.get_by_role_and_label(Role::Button, button).rect()), "{}", state.name);
        }
    }
}

#[test]
fn a_cut_recipient_keeps_its_whole_room() {
    // The board's refused row: "to" cut in its box of 120 (44..164), the bar 8 after the box (172), a spacer of 2 and a
    // gap before where it stands.
    let state = send_states().into_iter().find(|state| state.standing == Standing::Refused).unwrap();
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::default());
        let harness = harness(theme, state);
        let name = send_spoken(&state);
        let o = harness.get_by_label(&name).rect().min;
        let paint = painted(&harness, &name);
        let bar = paint.rects().find(|shape| shape.rect.size() == vec2(3.0, 12.0)).map(|shape| shape.rect);
        assert_eq!(bar.map(|bar| bar.left() - o.x), Some(172.0), "{mode:?}");
    }
}
