// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::{spoken, why_line};
use crate::component::tests::{Look, own, painted, staged};
use crate::keymap::Keymap;
use crate::theme::{Mode, TextScale, Theme};

/// The sheet's four states and their words.
const STATES: [(&str, &str); 4] = [
    ("group", "let in by you on 3 Nov  ·  grouped as People  ·  synced 14:02 by IDLE"),
    ("new-sender", "first mail on 14 Nov  ·  let in by you today  ·  grouped as People"),
    ("screener", "first mail from this address  ·  SPF and DKIM pass  ·  held back"),
    ("folder", "moved by you on 9 Nov  ·  in Studio / Clients / Bakery"),
];

/// The line 640 wide on the reader's `surface`.
fn harness(theme: Theme, reasons: &'static str) -> Harness<'static> {
    staged(
        theme,
        |p| p.surface,
        vec2(640.0 + 48.0, 100.0),
        move |ui, theme| {
            ui.set_width(640.0);
            why_line(ui, theme, &Keymap::default(), reasons);
        },
    )
}

#[test]
fn the_why_line_looks_as_its_sheet_says_in_every_state() {
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::default());
        let p = theme.palette;
        let f = theme.fonts;
        for (state, reasons) in STATES {
            let harness = harness(theme, reasons);
            let name = spoken(reasons);
            let rect = harness.get_by_label(&name).rect();
            let paint = painted(&harness, &name);
            let at = format!("{mode:?} {state}: {paint:#?}");
            assert_eq!(rect.size(), vec2(640.0, 32.0), "{at}");
            let look = own(&paint, rect);
            assert_eq!(look.fill, Some(p.panel), "{at}");
            assert_eq!(look.edge, Some((1.0, p.panel_line, false)), "{at}");
            assert_eq!(look.radius, 0.0, "{at}");
            let text = |words: &str| {
                paint.shapes.iter().find_map(|shape| match &shape.look {
                    Look::Text { text, colour, size, .. } if text.starts_with(words) => Some((shape.rect.min - rect.min, *colour, *size)),
                    _ => None,
                })
            };
            let (label_at, label_ink, label_size) = text("WHY HERE").unwrap_or_else(|| panic!("WHY HERE: {at}"));
            assert_eq!((label_at.x, label_ink, label_size), (12.0, p.cyan_text, f.label.size), "{at}");
            // The reasons 12 right of the label's box (59 wide), in mono_s and muted, on one line.
            let (words_at, words_ink, words_size) = text(&reasons[..12]).unwrap_or_else(|| panic!("reasons: {at}"));
            assert_eq!((words_at.x, words_ink, words_size), (83.0, p.muted, f.mono_s.size), "{at}");
            // The log is not built: its link stands unavailable, with the coming-soon tag.
            let link = harness.get_by_label("open log →");
            assert!(link.accesskit_node().is_disabled(), "{at}");
            assert_eq!(link.rect().right(), rect.right() - 12.0, "{at}");
            assert_eq!(harness.query_all_by_label("coming soon").count(), 1, "{at}");
        }
    }
}

#[test]
fn the_why_line_is_as_high_as_the_scaled_size_at_130_percent() {
    let theme = Theme::new(Mode::Dark, TextScale::ALL[4]);
    let reasons = STATES[0].1;
    let harness = harness(theme, reasons);
    let rect = harness.get_by_label(&spoken(reasons)).rect();
    assert_eq!(rect.size(), vec2(640.0, theme.size(32.0)));
}

#[test]
fn hostile_reasons_stay_on_one_cut_line_and_are_heard_whole() {
    let theme = Theme::new(Mode::Dark, TextScale::ALL[4]);
    let long: &'static str = Box::leak(format!("\u{202E}{}\u{0}folder", "Ż".repeat(600)).into_boxed_str());
    let harness = harness(theme, long);
    let name = spoken(long);
    assert!(!name.contains('\u{202E}') && !name.contains('\u{0}'), "no override or control in the name");
    let whole = crate::component::spoken(long);
    let words = harness.get_by_label(&whole).rect();
    let rect = harness.get_by_label(&name).rect();
    let paint = painted(&harness, &name);
    let drawn: Vec<&String> = paint
        .shapes
        .iter()
        .filter_map(|shape| match &shape.look {
            Look::Text { text, .. } if text.starts_with('Ż') => Some(text),
            _ => None,
        })
        .collect();
    assert_eq!(drawn.len(), 1, "one run of the reasons");
    assert!(drawn.iter().all(|text| text.ends_with('…')), "cut with one ellipsis: {drawn:?}");
    assert!(words.right() <= rect.right(), "the reasons stay inside the line");
}
