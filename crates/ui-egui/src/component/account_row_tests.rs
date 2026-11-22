// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The account row's sheet, state by state, in dark and light, at 100 % and 130 %.

use egui::{Rect, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

use super::Sync;
use crate::component::Shown;
use crate::component::gallery_index::{AccountState, account, account_spoken, account_states};
use crate::component::nav_row::tests::{texts, themes};
use crate::component::tests::{Look, icons, own, painted, staged};
use crate::draw::paint::run_box;
use crate::iconset::{IconId, glyph};

/// The row of `state` on a `panel` stage in `theme`.
fn harness(theme: crate::theme::Theme, state: AccountState) -> Harness<'static> {
    staged(theme, |p| p.panel, vec2(300.0, 100.0), move |ui, theme| account(ui, theme, &state))
}

/// The narrow row's bar and its mark, 4 apart, in the middle; no caret.
fn narrow_marks(
    texts: &[(Rect, String, egui::Color32, String)],
    bar: Rect,
    o: egui::Pos2,
    sync: Sync,
    mark: Option<(&str, egui::Color32)>,
    at: &str,
) {
    // The bar and its mark, 4 apart, in the middle.
    let right = match mark {
        Some((glyph, tone)) => {
            let shown = texts.iter().find(|t| t.1 == glyph).cloned().unwrap_or_else(|| panic!("{glyph}: {at}"));
            assert_eq!((shown.2, shown.0.left() - bar.right()), (tone, 4.0), "{at}");
            shown.0.left() + run_box(shown.0.width())
        }
        None if sync == Sync::Syncing => bar.right() + 14.0,
        None => bar.right(),
    };
    assert!(((bar.left() - o.x) - (o.x + 56.0 - right)).abs() <= 1.0, "in the middle: {at}: {bar:?} {right}");
    assert!(texts.iter().all(|t| t.3 != icons()), "no caret when narrow: {at}");
}

/// The name and the state's note after it, 10 before the caret; a long name cut once.
fn the_note(
    texts: &[(Rect, String, egui::Color32, String)],
    label: &(Rect, String, egui::Color32, String),
    o: egui::Pos2,
    note: Option<(&str, egui::Color32)>,
    mark: Option<(&str, egui::Color32)>,
    words: &str,
    at: &str,
) {
    match note {
        None => {
            assert_eq!(texts.len(), 2, "the name and the caret: {at}");
            if words.len() > 30 {
                assert!(label.1.ends_with('…') && label.1.matches('…').count() == 1, "{at}: {}", label.1);
                assert!(label.0.right() <= o.x + 196.0 + 0.51, "{at}");
            }
        }
        Some((word, tone)) => {
            let shown = texts.iter().find(|t| t.1 == word).cloned().unwrap_or_else(|| panic!("{word}: {at}"));
            assert_eq!(shown.2, tone, "{at}");
            assert!((shown.0.left() + run_box(shown.0.width()) - (o.x + 196.0)).abs() < 0.51, "10 before the caret: {at}");
            if let Some((glyph, tone)) = mark {
                let sign = texts.iter().find(|t| t.1 == glyph).cloned().unwrap_or_else(|| panic!("{glyph}: {at}"));
                assert_eq!(sign.2, tone, "{at}");
                assert!(sign.0.left() < shown.0.left(), "{at}");
            }
            assert!(label.0.right() < shown.0.left() - 10.0, "the name stops before the state: {at}");
        }
    }
}

#[test]
fn an_account_row_looks_as_its_sheet_says_in_every_state() {
    for theme in themes() {
        let p = theme.palette;
        for state in account_states() {
            let harness = harness(theme, state);
            let name = account_spoken(&state);
            let rect = harness.get_by_label(&name).rect();
            let at = format!("{:?} {:?} {}", theme.mode, theme.text_scale, state.name);
            let o = rect.min;
            let high = theme.size(28.0);
            assert_eq!(rect.size(), vec2(if state.narrow { 56.0 } else { 232.0 }, high), "{at}");
            let paint = painted(&harness, &name);
            let found = own(&paint, rect);
            let fill = (state.shown == Shown::Hover).then_some(p.grid);
            assert_eq!(found.fill.filter(|fill| *fill != p.panel), fill, "{at}");
            assert_eq!(found.radius, 0.0, "{at}");
            assert_eq!(found.edge, (state.shown == Shown::Focus).then_some((1.0, p.cyan_edge, false)), "{at}");
            let colour = theme.account_colour(1);
            let bars: Vec<Rect> = paint
                .rects()
                .filter(|shape| matches!(shape.look, Look::Rect { fill, stroke: None, .. } if fill == colour))
                .map(|shape| shape.rect)
                .collect();
            assert_eq!(bars.len(), 1, "one bar: {at}");
            let bar = bars[0];
            assert_eq!((bar.size(), bar.top() - o.y), (vec2(3.0, 12.0), ((high - 12.0) / 2.0).round()), "{at}");
            let dots = paint
                .rects()
                .filter(|shape| matches!(shape.look, Look::Rect { fill, .. } if fill == p.warn) && shape.rect.size() == vec2(2.0, 2.0))
                .count();
            assert_eq!(dots, if state.sync == Sync::Syncing { 8 } else { 0 }, "the syncing ring: {at}");
            let texts = texts(&harness, &name);
            let mark = match state.sync {
                Sync::Offline => Some(("○", p.warn)),
                Sync::Refused => Some(("●", p.minus)),
                _ => None,
            };
            if state.narrow {
                narrow_marks(&texts, bar, o, state.sync, mark, &at);
                continue;
            }
            assert_eq!(bar.left() - o.x, 50.0, "{at}");
            let strong = state.shown == Shown::Hover || state.open || state.sync == Sync::Refused;
            let ink = if strong { p.ink } else { p.muted };
            let caret = texts.iter().find(|t| t.3 == icons()).cloned().unwrap_or_else(|| panic!("the caret: {at}"));
            let want = if state.open { glyph(IconId::Caret) } else { glyph(IconId::Next) };
            assert_eq!((caret.1.clone(), caret.2), (want.to_string(), ink), "{at}");
            assert!((caret.0.center().x - (o.x + 212.0)).abs() < 1.01, "{at}: {:?}", caret.0);
            let label = texts.iter().find(|t| t.0.left() - o.x == 70.0).cloned().unwrap_or_else(|| panic!("the name: {at}: {texts:?}"));
            assert_eq!((label.0.top() - o.y, label.2), (((high - theme.fonts.mono_s.line) / 2.0).round(), ink), "{at}");
            let note = match state.sync {
                Sync::Quiet => None,
                Sync::Syncing => Some(("syncing", p.ink)),
                Sync::Offline => Some(("offline", p.ink)),
                Sync::Refused => Some(("refused", p.minus)),
            };
            the_note(&texts, &label, o, note, mark, state.label, &at);
        }
    }
}
