// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The draft row's sheet, state by state, in dark and light, at 100 % and 130 %.

use egui::{Color32, Rect, StrokeKind, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

use crate::component::Shown;
use crate::component::gallery::keymap;
use crate::component::gallery_lists::{DraftState, ROW_WIDTH, draft, draft_spoken, draft_states};
use crate::component::tests::{Look, icons, own, painted, staged};
use crate::draw::paint::run_box;
use crate::theme::{Mode, TextScale, Theme};

/// The row of `state` on a `surface` stage in `theme`.
fn harness(theme: Theme, state: DraftState) -> Harness<'static> {
    let keymap = keymap();
    staged(theme, |p| p.surface, vec2(ROW_WIDTH + 48.0, 120.0), move |ui, theme| draft(ui, theme, &keymap, &state))
}

/// The places of the row at the theme's text scale, from its top-left: its height, the mark's middle, the subject's
/// left and top, whom's top and the time's (and the links') top.
fn places(theme: &Theme) -> [f32; 6] {
    let f = theme.fonts;
    let high = theme.size(44.0);
    let middle = (high - 1.0) / 2.0;
    let at = |side: f32| (middle - side / 2.0).round();
    let icon = theme.boxes().icon;
    [high, at(icon) + icon / 2.0, 16.0 + icon + 12.0, at(f.body.line), at(f.caption.line), at(f.mono_s.line)]
}

#[test]
fn places_at_100_are_the_sheets() {
    assert_eq!(places(&Theme::new(Mode::Light, TextScale::default())), [44.0, 22.0, 44.0, 11.0, 13.0, 14.0]);
}

#[test]
fn a_draft_row_looks_as_its_sheet_says_in_every_state_at_100_and_130_percent() {
    for (mode, scale) in [Mode::Dark, Mode::Light].into_iter().flat_map(|mode| [(mode, TextScale::default()), (mode, TextScale::ALL[4])]) {
        let theme = Theme::new(mode, scale);
        let [high, mark_middle, subject_left, subject_top, to_top, when_top] = places(&theme);
        let (p, f) = (theme.palette, theme.fonts);
        for state in draft_states() {
            let mut harness = harness(theme, state);
            let name = draft_spoken(&state);
            if state.shown == Shown::Focus {
                harness.get_by_label(&name).focus();
                harness.run();
            }
            let rect = harness.get_by_label(&name).rect();
            let paint = painted(&harness, &name);
            let at = format!("{mode:?} {:?} {}: {paint:#?}", theme.text_scale, state.name);
            let o = rect.min;
            let hover = state.shown == Shown::Hover;
            assert_eq!(rect.size(), vec2(440.0, high), "{at}");
            let found = own(&paint, rect);
            assert_eq!(found.fill.filter(|fill| *fill != p.surface), hover.then_some(p.stage), "{at}");
            assert_eq!(found.radius, 0.0, "{at}");
            assert!(found.lines.contains(&p.panel_line), "{at}");
            let texts: Vec<(Rect, String, Color32, String, f32)> = paint
                .shapes
                .iter()
                .filter_map(|shape| match &shape.look {
                    Look::Text { text, colour, family, size, .. } => Some((shape.rect, text.clone(), *colour, family.clone(), *size)),
                    _ => None,
                })
                .collect();
            let find = |text: &str| {
                let cut =
                    |t: &&(Rect, String, Color32, String, f32)| t.1.ends_with('…') && t.1.len() > 4 && text.starts_with(t.1.trim_end_matches('…'));
                texts.iter().find(|t| t.1 == text || cut(t)).cloned().unwrap_or_else(|| panic!("{text}: {at}"))
            };
            let mark = texts.iter().find(|t| t.3 == icons()).cloned().unwrap_or_else(|| panic!("the mark: {at}"));
            assert_eq!(mark.2, if hover { p.ink } else { p.muted }, "{at}");
            assert!((mark.0.center() - (o + vec2(16.0 + theme.boxes().icon / 2.0, mark_middle))).length() < 1.01, "{:?}: {at}", mark.0);
            let (words, ink) = state.subject.map_or(("(no subject)", p.muted), |subject| (subject, p.ink));
            let subject = find(words);
            assert_eq!((subject.0.min - o, subject.2, subject.4), (vec2(subject_left, subject_top), ink, f.body.size), "{at}");
            let to = find(state.to);
            assert_eq!((to.0.top() - o.y, to.2, to.4), (to_top, p.muted, f.caption.size), "{at}");
            assert!(run_box(to.0.width()) <= 100.0, "at most 100 wide: {at}");
            let right = if hover {
                let open = harness.get_by_label("open").rect();
                let discard = harness.get_by_label("discard").rect();
                // The links' box (as high as a row's link) on the row's middle, as the time's text is.
                let link_top = ((high - 1.0) / 2.0 - theme.boxes().row_link / 2.0).round();
                assert_eq!((discard.right() - o.x, discard.top() - o.y, discard.left() - open.right()), (424.0, link_top, 16.0), "{at}");
                assert!(texts.iter().all(|t| t.1 != state.when), "the links take the time's place: {at}");
                open.left()
            } else {
                let when = find(state.when);
                assert_eq!((when.0.top() - o.y, when.2, when.4), (when_top, p.muted, f.mono_s.size), "{at}");
                assert!((when.0.left() + run_box(when.0.width()) - (o.x + 424.0)).abs() < 0.51, "{at}");
                when.0.left()
            };
            assert!((to.0.left() + run_box(to.0.width()) + 12.0 - right).abs() < 0.51, "whom 12 before the time: {at}");
            assert!(subject.0.left() + run_box(subject.0.width()) + 12.0 <= to.0.left() + 0.51, "{at}");
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
            assert_eq!(ring, if state.shown == Shown::Focus { vec![inside] } else { vec![] }, "the sheet's 2,2 436 × 39: {at}");
        }
    }
}

#[test]
fn a_draft_row_is_the_scaled_height_at_130_percent() {
    let theme = Theme::new(Mode::Light, TextScale::ALL[4]);
    let state = draft_states()[5];
    let harness = harness(theme, state);
    let rect = harness.get_by_label(&draft_spoken(&state)).rect();
    assert_eq!(rect.size(), vec2(440.0, theme.size(44.0)));
}

/// Two rows stacked in a top-down ui, the first `keyed`: where the second row stands, after the first is hovered when
/// `hover`.
fn second_top(theme: Theme, keyed: bool, hover: bool) -> f32 {
    use crate::component::draft_row::{DraftRow, draft_row};
    let keymap = keymap();
    let mut harness = staged(
        theme,
        |p| p.surface,
        vec2(ROW_WIDTH + 48.0, 200.0),
        move |ui, theme| {
            ui.set_width(ROW_WIDTH);
            for (index, spoken) in ["first draft", "second draft"].into_iter().enumerate() {
                let row = DraftRow {
                    id: egui::Id::unique(("stacked", index)),
                    spoken,
                    subject: Some(spoken),
                    to: "to Sunniva Aalto",
                    when: "edited 12:10",
                    last: index == 1,
                };
                draft_row(ui, theme, &keymap, &row, keyed && index == 0, None);
            }
        },
    );
    if hover {
        harness.get_by_label("first draft").hover();
        harness.run();
    }
    assert!(hover || keyed || harness.query_by_label("discard").is_none(), "the links show only when lit");
    let first = harness.get_by_label("first draft").rect();
    let second = harness.get_by_label("second draft").rect();
    assert!(second.top() >= first.bottom(), "the second row under the first: {first:?} {second:?}");
    second.top()
}

#[test]
fn a_lit_row_does_not_move_the_row_under_it_at_100_and_130_percent() {
    for scale in [TextScale::default(), TextScale::ALL[4]] {
        let theme = Theme::new(Mode::Light, scale);
        let still = second_top(theme, false, false);
        assert_eq!(second_top(theme, false, true), still, "hovered: {scale:?}");
        assert_eq!(second_top(theme, true, false), still, "keyed: {scale:?}");
    }
}

/// The id the next widget after a row takes when it asks `ui` for one, the row lit (`keyed`) or not.
fn next_auto_id(keyed: bool) -> egui::Id {
    use crate::component::draft_row::{DraftRow, draft_row};
    let keymap = keymap();
    let after = std::sync::Arc::new(std::sync::Mutex::new(None));
    let kept = std::sync::Arc::clone(&after);
    let _harness = staged(
        Theme::new(Mode::Light, TextScale::default()),
        |p| p.surface,
        vec2(ROW_WIDTH + 48.0, 200.0),
        move |ui, theme| {
            ui.set_width(ROW_WIDTH);
            let row =
                DraftRow { id: egui::Id::unique("auto"), spoken: "a draft", subject: None, to: "to Sunniva Aalto", when: "edited 12:10", last: true };
            draft_row(ui, theme, &keymap, &row, keyed, None);
            *kept.lock().unwrap() = Some(ui.next_auto_id());
        },
    );
    after.lock().unwrap().unwrap()
}

#[test]
fn a_lit_row_leaves_the_ids_after_it_as_they_are() {
    // A control after the row keeps its id when the row's links come and go, so the keyboard on it stays there.
    assert_eq!(next_auto_id(true), next_auto_id(false));
}
