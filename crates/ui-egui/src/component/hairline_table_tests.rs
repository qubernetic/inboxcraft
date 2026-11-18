// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::{Color32, Rect, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

use super::{Table, hairline_table, spoken};
use crate::component::tests::{Look, Paint, own, painted, staged};
use crate::theme::{Mode, TextScale, Theme};

const HEADS: [&str; 3] = ["Item", "Qty", "EUR"];
const WIDTHS: [f32; 2] = [40.0, 72.0];
const TOTAL: [&str; 3] = ["Total", "", "61.90"];

/// The board's rows, with the long cell or not.
fn rows(long: bool) -> Vec<&'static [&'static str]> {
    let second: &'static [&'static str] = if long {
        &["Loose leaf Assam, second flush, from the Halmari estate, 250 g in a tin that can be sent back for a refill.", "2", "19.00"]
    } else {
        &["Loose leaf Assam 250 g", "2", "19.00"]
    };
    vec![&["Stoneware teapot", "1", "38.00"], second, &["Delivery", "", "4.90"]]
}

/// The table 560 wide on the reader's `surface`.
fn harness(theme: Theme, long: bool) -> Harness<'static> {
    staged(
        theme,
        |p| p.surface,
        vec2(560.0 + 48.0, 260.0),
        move |ui, theme| {
            ui.set_width(560.0);
            let rows = rows(long);
            hairline_table(ui, theme, &Table { heads: &HEADS, widths: &WIDTHS, rows: &rows, total: Some(&TOTAL) });
        },
    )
}

/// The texts painted: `(rect, text, colour, size)`.
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

#[test]
fn the_hairline_table_looks_as_its_sheet_says_in_both_states() {
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::default());
        let p = theme.palette;
        let f = theme.fonts;
        for (state, long, high) in [("default", false, 164.0), ("long-cell", true, 188.0)] {
            let harness = harness(theme, long);
            let rows = rows(long);
            let table = Table { heads: &HEADS, widths: &WIDTHS, rows: &rows, total: Some(&TOTAL) };
            let name = spoken(&table);
            let rect = harness.get_by_label(&name).rect();
            let paint = painted(&harness, &name);
            let at = format!("{mode:?} {state}: {paint:#?}");
            assert_eq!(rect.size(), vec2(560.0, high), "{at}");
            assert_eq!(own(&paint, rect).edge, Some((1.0, p.line, false)), "{at}");
            let head = Rect::from_min_size(rect.min, vec2(560.0, 28.0));
            assert!(paint.rects().any(|shape| shape.rect == head && matches!(shape.look, Look::Rect { fill, .. } if fill == p.panel)), "{at}");
            let got = texts(&paint);
            let find = |words: &str| got.iter().find(|text| text.1.starts_with(words)).cloned().unwrap_or_else(|| panic!("{words}: {at}"));
            let item = find("ITEM");
            assert_eq!((item.0.left() - rect.left(), item.2, item.3), (12.0, p.muted, f.label.size), "{at}");
            for (label, right) in [("QTY", 464.0), ("EUR", 548.0)] {
                let found = find(label);
                assert!((found.0.right() - (rect.left() + right)).abs() <= 1.0, "{label} right-aligned at {right}: {at}");
            }
            let teapot = find("Stoneware teapot");
            assert_eq!((teapot.0.left() - rect.left(), teapot.2, teapot.3), (12.0, p.ink, f.body.size), "{at}");
            let price = find("38.00");
            assert_eq!((price.2, price.3), (p.ink, f.mono.size), "{at}");
            assert!((price.0.right() - (rect.left() + 548.0)).abs() <= 1.0, "{at}");
            let total = find("Total");
            assert_eq!((total.2, total.3), (p.ink, f.strong.size), "{at}");
            // Hairlines: panel_line under the head and between rows, line over the total.
            let lines: Vec<(f32, Color32)> = paint
                .shapes
                .iter()
                .filter_map(|shape| match shape.look {
                    Look::Rect { stroke: Some((1.0, colour)), .. } if shape.rect.height() == 0.0 => Some((shape.rect.top() - rect.top(), colour)),
                    _ => None,
                })
                .collect();
            let want: Vec<(f32, Color32)> = if long {
                vec![(28.5, p.panel_line), (61.5, p.panel_line), (118.5, p.panel_line), (151.5, p.line)]
            } else {
                vec![(28.5, p.panel_line), (61.5, p.panel_line), (94.5, p.panel_line), (127.5, p.line)]
            };
            assert_eq!(lines, want, "{at}");
            if long {
                let assam = find("Loose leaf Assam");
                assert_eq!(assam.0.top() - rect.top(), 68.0, "a wrapped cell 6 under its row's top: {at}");
                assert!(got.iter().filter(|text| text.1.contains("Halmari") || text.1.starts_with("Loose")).count() >= 1, "{at}");
                let qty = got.iter().find(|text| text.1 == "2").cloned().unwrap_or_else(|| panic!("{at}"));
                assert_eq!(qty.0.top() - rect.top(), 68.0, "{at}");
            }
            assert!(paint.rects().all(|shape| !matches!(shape.look, Look::Rect { radius, .. } if radius > 0.0)), "{at}");
        }
    }
}

#[test]
fn a_hostile_cell_stays_in_its_column() {
    let theme = Theme::new(Mode::Dark, TextScale::ALL[4]);
    let price: &'static str = Box::leak(format!("\u{202E}{}", "9".repeat(300)).into_boxed_str());
    let harness = staged(
        theme,
        |p| p.surface,
        vec2(608.0, 300.0),
        move |ui, theme| {
            ui.set_width(560.0);
            let row: &[&str] = &["Teapot", "1", price];
            hairline_table(ui, theme, &Table { heads: &HEADS, widths: &WIDTHS, rows: &[row], total: None });
        },
    );
    let row: &[&str] = &["Teapot", "1", price];
    let name = spoken(&Table { heads: &HEADS, widths: &WIDTHS, rows: &[row], total: None });
    assert!(!name.contains('\u{202E}'));
    let rect = harness.get_by_label(&name).rect();
    let paint = painted(&harness, &name);
    let cut = texts(&paint).into_iter().find(|text| text.1.starts_with('9')).unwrap_or_else(|| panic!("{paint:#?}"));
    assert!(cut.1.ends_with('…') && cut.0.width() <= 72.0 && cut.0.right() <= rect.right() - 12.0 + 1.0, "{cut:?}");
}
