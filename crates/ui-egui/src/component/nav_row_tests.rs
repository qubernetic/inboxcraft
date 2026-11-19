// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The place row's sheet, state by state, in dark and light, at 100 % and 130 %.

use egui::{Color32, Rect, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

use super::Standing;
use crate::component::Shown;
use crate::component::gallery_index::{NavState, nav, nav_spoken, nav_states};
use crate::component::tests::{Look, icons, own, painted, staged};
use crate::draw::paint::run_box;
use crate::theme::{Mode, TextScale, Theme};

/// The row of `state` on a `panel` stage in `theme`.
fn harness(theme: Theme, state: NavState) -> Harness<'static> {
    staged(theme, |p| p.panel, vec2(300.0, 100.0), move |ui, theme| nav(ui, theme, &state))
}

/// The texts `harness` painted for `label`: rect, words, colour, family.
pub(crate) fn texts(harness: &Harness<'_>, label: &str) -> Vec<(Rect, String, Color32, String)> {
    painted(harness, label)
        .shapes
        .iter()
        .filter_map(|shape| match &shape.look {
            Look::Text { text, colour, family, .. } => Some((shape.rect, text.clone(), *colour, family.clone())),
            _ => None,
        })
        .collect()
}

/// The themes a state test runs in: dark and light at 100 % and 130 %.
pub(crate) fn themes() -> Vec<Theme> {
    [Mode::Dark, Mode::Light].into_iter().flat_map(|mode| [Theme::new(mode, TextScale::default()), Theme::new(mode, TextScale::ALL[4])]).collect()
}

/// Whether the row of `rect` in `paint` has the shown row's bar.
pub(crate) fn has_bar(harness: &Harness<'_>, label: &str, rect: Rect, colour: Color32) -> bool {
    painted(harness, label).rects().any(|shape| {
        matches!(shape.look, Look::Rect { fill, stroke: None, .. } if fill == colour)
            && shape.rect == Rect::from_min_size(rect.min, vec2(2.0, rect.height()))
    })
}

#[test]
fn a_place_row_looks_as_its_sheet_says_in_every_state() {
    for theme in themes() {
        let p = theme.palette;
        let f = theme.fonts;
        for state in nav_states() {
            let mut harness = harness(theme, state);
            let name = nav_spoken(&state);
            if state.shown == Shown::Focus {
                harness.run();
            }
            let rect = harness.get_by_label(&name).rect();
            let at = format!("{:?} {:?} {}", theme.mode, theme.text_scale, state.name);
            let o = rect.min;
            let wide = if state.narrow { 56.0 } else { 232.0 };
            assert_eq!(rect.size(), vec2(wide, theme.size(34.0)), "{at}");
            let found = own(&painted(&harness, &name), rect);
            let active = state.standing == Standing::Active;
            let fill = if active { Some(p.stage) } else { (state.shown == Shown::Hover).then_some(p.grid) };
            assert_eq!(found.fill.filter(|fill| *fill != p.panel), fill, "{at}");
            assert_eq!(found.radius, 0.0, "{at}");
            assert_eq!(has_bar(&harness, &name, rect, p.cyan_edge), active, "{at}");
            assert_eq!(found.edge, (state.shown == Shown::Focus).then_some((1.0, p.cyan_edge, false)), "{at}");
            let lit = state.standing != Standing::Rest;
            let texts = texts(&harness, &name);
            if state.narrow {
                let icon = texts.iter().find(|t| t.3 == icons()).cloned().unwrap_or_else(|| panic!("the icon: {at}"));
                assert_eq!(icon.2, if lit { p.ink } else { p.muted }, "{at}");
                assert!((icon.0.center().x - rect.center().x).abs() < 1.01, "{at}: {:?}", icon.0);
                assert_eq!(texts.len(), 1, "only the icon when narrow: {at}");
                continue;
            }
            let find = |words: &str| texts.iter().find(|t| t.1 == words).cloned().unwrap_or_else(|| panic!("{words}: {at}: {texts:?}"));
            let top = ((theme.size(34.0) - f.label.line) / 2.0).round();
            let number = find("[ 01 ]");
            assert_eq!((number.0.left() - o.x, number.0.top() - o.y, number.2), (16.0, top, if lit { p.cyan_text } else { p.dim }), "{at}");
            let label = find("MAIL");
            let ink = if lit || state.shown == Shown::Hover { p.ink } else { p.muted };
            assert_eq!((label.0.left() - o.x, label.0.top() - o.y, label.2), (68.0, top, ink), "{at}");
            let count = crate::component::label::count(state.count);
            if count.is_empty() {
                assert_eq!(texts.len(), 2, "no count: {at}");
            } else {
                let shown = find(&count);
                assert_eq!(shown.2, if lit { p.cyan_text } else { p.muted }, "{at}");
                assert!((shown.0.left() + run_box(shown.0.width()) - (o.x + 218.0)).abs() < 0.51, "{at}: {:?}", shown.0);
            }
        }
    }
}

#[test]
fn an_overflowing_count_caps_at_9_999_plus() {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let state = nav_states()[6];
    let harness = harness(theme, state);
    let texts = texts(&harness, &nav_spoken(&state));
    assert!(texts.iter().any(|t| t.1 == "9\u{202f}999+"), "{texts:?}");
}

#[test]
fn a_place_row_inside_soon_takes_no_press_and_no_focus() {
    let theme = Theme::new(Mode::Light, TextScale::default());
    let state = nav_states()[0];
    let mut harness =
        staged(theme, |p| p.panel, vec2(300.0, 100.0), move |ui, theme| crate::component::soon::soon(ui, theme, |ui| nav(ui, theme, &state)));
    let missed = crate::component::tests::keyboard_walk(&mut harness);
    assert!(missed.is_empty(), "{missed:?}");
    harness.key_press(egui::Key::Tab);
    harness.run();
    use egui_kittest::kittest::NodeT as _;
    let spoken = nav_spoken(&state);
    let node = harness.get_by_label(&spoken);
    assert!(!node.accesskit_node().is_focused(), "Tab passes it by");
    assert!(node.accesskit_node().is_disabled(), "it reads as unavailable");
}
