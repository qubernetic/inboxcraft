// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The mailbox row's sheet, state by state, in dark and light, at 100 % and 130 %.

use egui::{Color32, Rect, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

use crate::component::Shown;
use crate::component::gallery_index::{MailboxState, mailbox, mailbox_spoken, mailbox_states};
use crate::component::nav_row::tests::{has_bar, texts, themes};
use crate::component::tests::{Look, icons, own, painted, staged};
use crate::draw::paint::run_box;
use crate::theme::{Mode, TextScale, Theme};

/// The row of `state` on a `panel` stage in `theme`.
fn harness(theme: Theme, state: MailboxState) -> Harness<'static> {
    staged(theme, |p| p.panel, vec2(300.0, 100.0), move |ui, theme| mailbox(ui, theme, &state))
}

#[test]
fn a_mailbox_row_looks_as_its_sheet_says_in_every_state() {
    for theme in themes() {
        let p = theme.palette;
        let f = theme.fonts;
        for state in mailbox_states() {
            let harness = harness(theme, state);
            let name = mailbox_spoken(&state);
            let rect = harness.get_by_label(&name).rect();
            let at = format!("{:?} {:?} {}", theme.mode, theme.text_scale, state.name);
            let o = rect.min;
            let high = theme.size(30.0);
            assert_eq!(rect.size(), vec2(if state.narrow { 56.0 } else { 232.0 }, high), "{at}");
            let paint = painted(&harness, &name);
            let found = own(&paint, rect);
            let fill = if state.active { Some(p.stage) } else { (state.shown == Shown::Hover).then_some(p.grid) };
            assert_eq!(found.fill.filter(|fill| *fill != p.panel), fill, "{at}");
            assert_eq!(found.radius, 0.0, "{at}");
            assert_eq!(has_bar(&harness, &name, rect, p.cyan_edge), state.active, "{at}");
            assert_eq!(found.edge, (state.shown == Shown::Focus).then_some((1.0, p.cyan_edge, false)), "{at}");
            let ink = if state.active || state.shown == Shown::Hover { p.ink } else { p.muted };
            let texts = texts(&harness, &name);
            let icon = texts.iter().find(|t| t.3 == icons()).cloned().unwrap_or_else(|| panic!("the icon: {at}"));
            assert_eq!(icon.2, ink, "{at}");
            assert!((icon.0.center().y - rect.center().y).abs() < 1.01, "{at}");
            let square = Rect::from_min_size(o + vec2(40.0, 6.0), vec2(4.0, 4.0));
            let squared = paint.rects().any(|shape| shape.rect == square && matches!(shape.look, Look::Rect { fill, .. } if fill == p.cyan_edge));
            if state.narrow {
                assert!((icon.0.center().x - rect.center().x).abs() < 1.01, "{at}");
                assert_eq!(texts.len(), 1, "only the icon when narrow: {at}");
                assert_eq!(squared, state.count > 0, "the square while anything waits: {at}");
                continue;
            }
            assert!(!squared, "{at}");
            assert!((icon.0.center().x - (o.x + 52.0)).abs() < 1.01, "{at}: {:?}", icon.0);
            let words: Vec<_> = texts.iter().filter(|t| t.3 != icons()).cloned().collect();
            let label = words.iter().find(|t| t.0.left() - o.x == 70.0).cloned().unwrap_or_else(|| panic!("the name: {at}: {words:?}"));
            assert_eq!((label.0.top() - o.y, label.2), (((high - f.body.line) / 2.0).round(), ink), "{at}");
            let count = crate::component::label::count(state.count);
            if count.is_empty() {
                assert_eq!(words.len(), 1, "no count: {at}");
                assert_eq!(label.1, state.label, "{at}");
            } else {
                let shown = words.iter().find(|t| t.1 == count).cloned().unwrap_or_else(|| panic!("the count: {at}"));
                assert_eq!(shown.2, if state.active { p.cyan_text } else { p.muted }, "{at}");
                assert!((shown.0.left() + run_box(shown.0.width()) - (o.x + 218.0)).abs() < 0.51, "{at}");
                // The name stops 10 before the count, cut with one ellipsis when it does not fit.
                assert!(label.0.right() <= shown.0.left() - 10.0 + 0.51, "{at}: {label:?} {shown:?}");
                if state.label.len() > 40 {
                    assert!(label.1.ends_with('…') && label.1.matches('…').count() == 1, "{at}: {}", label.1);
                }
            }
        }
    }
}

#[test]
fn a_mailbox_count_caps_at_9_999_plus_and_a_60_character_name_is_cut_once() {
    let theme = Theme::new(Mode::Light, TextScale::ALL[4]);
    let long = "Clients and suppliers of the bakery on the corner, both years";
    assert_eq!(long.chars().count(), 61);
    let state = MailboxState { label: long, count: 12_000, ..mailbox_states()[0] };
    let harness = harness(theme, state);
    let rect = harness.get_by_label(&mailbox_spoken(&state)).rect();
    let texts = texts(&harness, &mailbox_spoken(&state));
    assert!(texts.iter().any(|t| t.1 == "9\u{202f}999+"), "{texts:?}");
    let name = texts.iter().find(|t| t.1.starts_with("Clients")).cloned().unwrap_or_else(|| panic!("{texts:?}"));
    assert_eq!(name.1.matches('…').count(), 1, "{}", name.1);
    assert!(name.0.right() <= rect.right() - 14.0, "{name:?}");
    let _: Color32 = name.2;
}
