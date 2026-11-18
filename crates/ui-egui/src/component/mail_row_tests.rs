// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The mail row's sheet, state by state, in dark and light, at 100 % and 130 %.

use egui::accesskit::Role;
use egui::{Color32, Rect, StrokeKind, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::Mark;
use crate::component::Shown;
use crate::component::gallery::keymap;
use crate::component::gallery_lists::{MailState, ROW_WIDTH, mail, mail_spoken, mail_states};
use crate::component::tag::TagKind;
use crate::component::tests::{Look, Paint, icons, own, painted, staged};
use crate::draw::paint::run_box;
use crate::theme::{Mode, TextScale, Theme};

/// The row of `state` on a `surface` stage in `theme`.
fn harness(theme: Theme, state: MailState) -> Harness<'static> {
    let keymap = keymap();
    staged(theme, |p| p.surface, vec2(ROW_WIDTH + 48.0, 140.0), move |ui, theme| {
        mail(ui, theme, &keymap, &state);
    })
}

/// The texts painted, `(rect, text, colour, family, size)`.
fn texts(paint: &Paint) -> Vec<(Rect, String, Color32, String, f32)> {
    paint
        .shapes
        .iter()
        .filter_map(|shape| match &shape.look {
            Look::Text { text, colour, family, size, .. } => Some((shape.rect, text.clone(), *colour, family.clone(), *size)),
            _ => None,
        })
        .collect()
}

/// The filled rects painted, `(rect, fill)`.
fn fills(paint: &Paint) -> Vec<(Rect, Color32)> {
    paint
        .rects()
        .filter_map(|shape| match shape.look {
            Look::Rect { fill, stroke: None, .. } if fill.a() > 0 => Some((shape.rect, fill)),
            _ => None,
        })
        .collect()
}

/// The family name of `font`.
fn family(font: crate::theme::Font) -> String {
    font.id().family.to_string()
}

#[test]
fn a_mail_row_looks_as_its_sheet_says_in_every_state() {
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::default());
        let (p, f) = (theme.palette, theme.fonts);
        for state in mail_states() {
            let mut harness = harness(theme, state);
            let name = mail_spoken(&state);
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
            let staged = state.current || state.checked || state.shown == Shown::Hover;
            // The board's own `surface` under the row is not the row's fill.
            assert_eq!(found.fill.filter(|fill| *fill != p.surface), staged.then_some(p.stage), "{at}");
            // Square; the key chips of the hover actions alone round their corners by 2.
            let radius = if state.shown == Shown::Hover { 2.0 } else { 0.0 };
            assert_eq!(found.radius, radius, "square: {at}");
            assert!(found.lines.contains(&p.panel_line), "the line under it: {at}");
            let rects = fills(&paint);
            let bar = Rect::from_min_size(o, vec2(2.0, 56.0));
            assert_eq!(rects.iter().any(|(r, c)| *r == bar && *c == p.cyan_edge), state.current, "the current row's bar: {at}");
            let square = Rect::from_min_size(o + vec2(16.0, 25.0), vec2(6.0, 6.0));
            let unread_square = state.unread && !state.checked;
            assert_eq!(rects.iter().any(|(r, c)| *r == square && *c == p.cyan_edge), unread_square, "the unread square: {at}");
            let words = texts(&paint);
            let find = |text: &str| words.iter().find(|t| t.1 == text || (t.1.ends_with('…') && text.starts_with(t.1.trim_end_matches('…'))));
            // Line one: the sender, its style and colour.
            let sender = find(state.sender).unwrap_or_else(|| panic!("the sender: {at}"));
            let font = if state.unread { f.strong } else { f.body };
            let ink = if state.done { p.muted } else { p.ink };
            assert_eq!((sender.0.min - o, sender.2, &sender.3, sender.4), (vec2(34.0, 7.0), ink, &family(font), font.size), "{at}");
            // Line two: subject and preview, `caption`, `ink` when unread.
            let line = find(state.line).unwrap_or_else(|| panic!("line two: {at}"));
            let words_ink = if state.unread { p.ink } else { p.muted };
            assert_eq!((line.0.min - o, line.2, &line.3, line.4), (vec2(34.0, 31.0), words_ink, &family(f.caption), f.caption.size), "{at}");
            assert!(line.0.right() <= o.x + 424.0 + 0.01, "line two inside the text: {at}");
            // The time at the right in `mono_s`, gone under the hover actions.
            let time = words.iter().find(|t| t.1 == state.time && t.3 == family(f.mono_s));
            if state.shown == Shown::Hover {
                assert!(time.is_none(), "the hover actions take the time's place: {at}");
                assert!(harness.query_by_label("Done").is_some(), "{at}");
            } else {
                let time = time.unwrap_or_else(|| panic!("the time: {at}"));
                let time_ink = if state.marks.contains(&Mark::Snoozed) {
                    p.warn
                } else if state.unread {
                    p.cyan_text
                } else {
                    p.muted
                };
                assert_eq!((time.0.min.y - o.y, time.2, time.4), (10.0, time_ink, f.mono_s.size), "{at}");
                assert!((time.0.left() + run_box(time.0.width()) - (o.x + 424.0)).abs() < 0.51, "the time ends at the text's right: {at}");
            }
            // The account bar 8 after the sender.
            let account = theme.account_colour(state.account);
            let bar_at = Rect::from_min_size(pos2(sender.0.left() + run_box(sender.0.width()) + 8.0, o.y + 12.0), vec2(3.0, 12.0));
            assert!(rects.iter().any(|(r, c)| (r.min - bar_at.min).length() < 0.51 && r.size() == bar_at.size() && *c == account), "the account bar: {at}");
            // The tag 8 after the bar, 20 high on the line's middle, in its kind's colours.
            if let Some((words, kind)) = state.tag {
                let tag = harness.get_by_label(words).rect();
                assert_eq!((tag.left(), tag.top() - o.y, tag.height()), (bar_at.right() + 8.0, 8.0, 20.0), "{at}");
                let edge = match kind {
                    TagKind::Accent => p.cyan_edge,
                    TagKind::Warn => p.warn,
                    TagKind::Minus => p.minus,
                    TagKind::Default | TagKind::Plus => p.line,
                };
                assert_eq!(own(&painted(&harness, words), tag).edge, Some((1.0, edge, false)), "{at}");
            }
            // The marks: 14 px, 8 before the time, in their colours.
            for (index, mark) in state.marks.iter().enumerate() {
                let (_, colour) = mark.look(&theme);
                let glyphs: Vec<_> = words.iter().filter(|t| t.3 == icons() && t.2 == colour && t.4 > 0.0).collect();
                assert!(!glyphs.is_empty(), "the {mark:?} mark in its colour: {at}");
                if index == 0 && state.shown != Shown::Hover {
                    let time = words.iter().find(|t| t.1 == state.time && t.3 == family(f.mono_s)).map_or(0.0, |t| t.0.left());
                    let mark_box = Rect::from_center_size(glyphs[0].0.center(), vec2(14.0, 14.0));
                    assert!((mark_box.right() + 8.0 - time).abs() < 1.01, "8 before the time: {at}");
                    assert!((mark_box.center().y - (o.y + 18.0)).abs() < 1.01, "on line one's middle: {at}");
                }
            }
            // The focus edge 2 px inside, above the line: the sheet's 2,2 436 × 51.
            let ring = paint.rects().filter_map(|shape| match shape.look {
                Look::Rect { stroke: Some((1.0, c)), stroke_kind: StrokeKind::Inside, .. } if c == p.cyan_edge && shape.rect.width() > 400.0 => {
                    Some(shape.rect)
                }
                _ => None,
            });
            let inside = Rect::from_min_max(o + vec2(2.0, 2.0), pos2(rect.right() - 2.0, rect.bottom() - 3.0));
            assert_eq!(ring.collect::<Vec<_>>(), if state.shown == Shown::Focus { vec![inside] } else { vec![] }, "{at}");
            // A checked row: the check box in the first column, on and coming soon.
            let check = harness.query_all_by_role(Role::CheckBox).next();
            assert_eq!(check.is_some(), state.checked, "{at}");
            if let Some(check) = check {
                let side = check.rect();
                assert!(check.accesskit_node().is_disabled(), "bulk select is coming soon: {at}");
                assert_eq!((side.min - o).x, 11.0, "{at}");
                let boxed = paint.shapes.iter().any(|shape| shape.rect == Rect::from_min_size(o + vec2(11.0, 20.0), vec2(16.0, 16.0)));
                assert!(boxed, "the 16 px box at 11,20: {at}");
            }
        }
    }
}

#[test]
fn a_mail_row_is_the_scaled_height_at_130_percent_and_keeps_its_columns() {
    let theme = Theme::new(Mode::Light, TextScale::ALL[4]);
    let states = mail_states();
    for state in [states[1], states[20]] {
        let harness = harness(theme, state);
        let name = mail_spoken(&state);
        let rect = harness.get_by_label(&name).rect();
        assert_eq!(rect.size(), vec2(440.0, theme.size(56.0)), "{}", state.name);
        let paint = painted(&harness, &name);
        let words = texts(&paint);
        let time = words.iter().find(|t| t.1 == state.time).map_or(Rect::NOTHING, |t| t.0);
        assert!((time.left() + run_box(time.width()) - (rect.left() + 424.0)).abs() < 0.51, "{}: {time:?}", state.name);
        assert!(words.iter().all(|t| t.0.right() <= rect.right() - 16.0 + 0.51), "{}: {words:?}", state.name);
    }
}

#[test]
fn the_sender_of_the_long_row_is_cut_short_of_its_tag_with_one_ellipsis() {
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::default());
        let state = mail_states()[20];
        let harness = harness(theme, state);
        let name = mail_spoken(&state);
        let words = texts(&painted(&harness, &name));
        let sender = words.iter().find(|t| state.sender.starts_with(t.1.trim_end_matches('…')) && t.1.len() > 4).map_or(Rect::NOTHING, |t| t.0);
        let tag = harness.get_by_label("New sender").rect();
        // Sender, 8, the bar's 3, 8, the tag.
        assert!(sender.left() + run_box(sender.width()) + 19.0 <= tag.left() + 0.01, "{sender:?} {tag:?}");
        let line = words.iter().filter(|t| t.1.ends_with('…')).count();
        assert!(line >= 1, "the long line is cut: {words:?}");
        assert!(words.iter().all(|t| t.1.matches('…').count() <= 1), "one ellipsis a line: {words:?}");
    }
}
