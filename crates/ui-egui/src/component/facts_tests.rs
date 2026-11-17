// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::{Color32, Rect, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

use super::{Facts, facts, facts_of, spoken};
use crate::component::tests::{Look, Paint, builder, painted, staged};
use crate::keymap::Keymap;
use crate::theme::{Mode, TextScale, Theme};

const FROM: &str = "Teodor Vasquez  <teodor@example.org>";
const TO: &str = "studio@example.org";
const DATE: &str = "Mon 16 Nov 2026, 14:02";

const OPEN: Facts<'static> = Facts {
    from: FROM,
    to: TO,
    date: DATE,
    cc: "Marit Solheim  <marit@example.org>",
    reply_to: "teodor@example.org",
    checked: "example.org · SPF pass · DKIM pass",
    id: "<20261116.1402.7f3a@example.org>",
    recipients: None,
};

const LONG: Facts<'static> = Facts {
    from: "Annelise Vandenberghe-Okonkwo-Halvorssen  <annelise.vandenberghe-okonkwo-halvorssen@studio.example.org>",
    to: "studio@example.org, Marit Solheim, Ines Okafor, Pavel Hruska, Greta Lindqvist, Tobias Wren, Ada Brannock, Odile Brannock",
    date: DATE,
    cc: "",
    reply_to: "",
    checked: "",
    id: "",
    recipients: Some(12),
};

/// The texts painted, `(rect, text, colour, size)`.
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

/// The facts 560 wide on the reader's `surface`.
fn harness(theme: Theme, shown: Facts<'static>, open: bool) -> Harness<'static> {
    staged(
        theme,
        |p| p.surface,
        vec2(560.0 + 48.0, 260.0),
        move |ui, theme| {
            ui.set_width(560.0);
            facts_of(ui, theme, &Keymap::default(), &shown, open);
        },
    )
}

#[test]
fn the_facts_look_as_their_sheet_says_in_every_state() {
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::default());
        let p = theme.palette;
        let f = theme.fonts;
        let default = Facts { from: FROM, to: TO, date: DATE, ..Facts::default() };
        for (state, shown, open) in [("default", default, false), ("details-open", OPEN, true), ("long-text", LONG, false)] {
            let harness = harness(theme, shown, open);
            let name = spoken(&shown);
            let rect = harness.get_by_label(&name).rect();
            let paint = painted(&harness, &name);
            let at = format!("{mode:?} {state}: {paint:#?}");
            let rows: Vec<(&str, &str, bool)> = if open {
                vec![
                    ("FROM", shown.from, true),
                    ("TO", shown.to, true),
                    ("CC", shown.cc, true),
                    ("REPLY TO", shown.reply_to, true),
                    ("CHECKED", shown.checked, false),
                    ("ID", shown.id, false),
                    ("DATE", shown.date, false),
                ]
            } else {
                vec![("FROM", shown.from, true), ("TO", shown.to, true), ("DATE", shown.date, false)]
            };
            let high = rows.len() as f32 * 22.0 + (rows.len() - 1) as f32 * 6.0;
            assert_eq!(rect.size(), vec2(560.0, high), "{at}");
            assert!(
                paint.rects().all(|shape| !matches!(shape.look, Look::Rect { fill, .. } if fill.a() > 0 && fill != p.cyan_text && shape.rect.height() > 2.0)),
                "no fill but the link's caret: {at}"
            );
            let got = texts(&paint);
            for (index, (label, value, body)) in rows.iter().enumerate() {
                let top = index as f32 * 28.0;
                let label_at = got.iter().find(|text| text.1 == *label).map(|text| (text.0.min - rect.min, text.2, text.3));
                assert_eq!(label_at, Some((vec2(0.0, top + 3.0), p.muted, f.label.size)), "{label}: {at}");
                let value_at = got.iter().find(|text| value.starts_with(text.1.trim_end_matches('…')) && text.0.left() - rect.left() == 84.0);
                let (value_rect, _, ink, size) = value_at.cloned().unwrap_or_else(|| panic!("{label} {value}: {at}"));
                let (font, colour) = if *body { (f.body, p.ink) } else { (f.mono, p.muted) };
                assert_eq!((value_rect.top() - rect.top(), ink, size), (top, colour, font.size), "{label}: {at}");
                assert!(value_rect.height() <= font.line + 0.01, "one line: {label}: {at}");
            }
            let link = if open { "hide details ▴" } else { "details ▾" };
            let link_rect = harness.get_by_label(link).rect();
            assert_eq!((link_rect.right(), link_rect.top() - rect.top(), link_rect.height()), (rect.right(), 3.0, 16.0), "{at}");
            // The sheet's link hugs its words and caret: 59 (92 open) wide, so its words start at 501 (468).
            assert_eq!(link_rect.width(), if open { 92.0 } else { 59.0 }, "{at}");
            let words = got.iter().find(|text| text.1.starts_with(if open { "hide details" } else { "details" })).map(|text| text.0.left() - rect.left());
            assert_eq!(words, Some(if open { 468.0 } else { 501.0 }), "{at}");
            // The caret: the design's triangle 5.46 wide and high, its top 5.6 and its foot 0.12 over the baseline (15.125).
            let link_paint = painted(&harness, link);
            let caret = link_paint.shapes.iter().find(|shape| matches!(shape.look, Look::Rect { fill, .. } if fill == p.cyan_text));
            let caret = caret.map(|shape| shape.rect.translate(-rect.min.to_vec2())).unwrap_or_else(|| panic!("no caret: {at}"));
            let left = if open { 468.0 + 13.0 * 6.6 } else { 501.0 + 8.0 * 6.6 } + 0.033;
            let want = egui::Rect::from_min_max(egui::pos2(left, 15.125 - 5.599), egui::pos2(left + 5.456, 15.125 - 0.121));
            let near = |a: f32, b: f32| (a - b).abs() < 0.02;
            assert!(
                near(caret.left(), want.left()) && near(caret.right(), want.right()) && near(caret.top(), want.top()) && near(caret.bottom(), want.bottom()),
                "the caret {caret:?}, the design's {want:?}: {at}"
            );
            if state == "long-text" {
                let count = got.iter().find(|text| text.1 == "12 in all").map(|text| (text.0, text.2, text.3));
                let (count_rect, ink, size) = count.unwrap_or_else(|| panic!("{at}"));
                assert_eq!(
                    (count_rect.right().ceil(), count_rect.top() - rect.top(), ink, size),
                    (rect.right(), 31.0, p.muted, f.mono_s.size),
                    "{at}"
                );
                for (text_rect, words, ..) in &got {
                    if text_rect.top() - rect.top() < 50.0 && text_rect.left() - rect.left() == 84.0 {
                        let end = if words.starts_with("Annelise") { link_rect.left() - 12.0 } else { count_rect.left().floor() - 12.0 };
                        assert!(text_rect.right() <= end + 0.01, "{words} runs into what stands right of it: {at}");
                    }
                }
            }
        }
    }
}

#[test]
fn the_details_link_answers_a_click_enter_and_space() {
    let theme = Theme::new(Mode::Light, TextScale::default());
    for key in [None, Some(egui::Key::Enter), Some(egui::Key::Space)] {
        let mut harness = builder().with_size(vec2(700.0, 300.0)).build_ui_state(
            move |ui, open: &mut bool| {
                if facts(ui, &theme, &Keymap::default(), FROM, TO, DATE, *open).clicked() {
                    *open = !*open;
                }
            },
            false,
        );
        harness.run();
        match key {
            None => harness.get_by_label("details ▾").click(),
            Some(key) => {
                harness.get_by_label("details ▾").focus();
                harness.run();
                harness.key_press(key);
            }
        }
        harness.run();
        assert!(*harness.state(), "{key:?}");
        harness.get_by_label("hide details ▴");
    }
}

#[test]
fn hostile_facts_stay_on_their_lines() {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let hostile = "Mallory\u{0}\r\n<a@b>\t".repeat(100);
    let from: &'static str = Box::leak(hostile.into_boxed_str());
    let shown = Facts { from, to: from, date: from, cc: from, reply_to: from, checked: from, id: from, recipients: Some(usize::MAX) };
    let harness = harness(theme, shown, true);
    let name = spoken(&shown);
    let rect = harness.get_by_label(&name).rect();
    let paint = painted(&harness, &name);
    for (text_rect, words, ..) in texts(&paint) {
        assert!(rect.contains_rect(text_rect), "{words:?} at {text_rect:?} leaves {rect:?}");
        assert!(!words.contains(['\0', '\r', '\n', '\t']), "{words:?}");
    }
}

/// Whether `shown`, without its closing ellipsis, is a beginning of `full` that ends between grapheme clusters.
fn whole_prefix(full: &str, shown: &str) -> bool {
    use unicode_segmentation::UnicodeSegmentation as _;
    let kept = shown.strip_suffix('…').unwrap_or(shown).trim_end();
    full.starts_with(kept) && (kept.is_empty() || full.grapheme_indices(true).any(|(at, grapheme)| at + grapheme.len() == kept.len()))
}

#[test]
fn a_cut_value_keeps_its_graphemes_whole_and_is_capped() {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let marks: &'static str = Box::leak("e\u{301}\u{323}".repeat(300).into_boxed_str());
    let family: &'static str = Box::leak("\u{1F469}\u{200D}\u{1F469}\u{200D}\u{1F467} ".repeat(200).into_boxed_str());
    let long: &'static str = Box::leak("v".repeat(100_000).into_boxed_str());
    for value in [marks, family, long] {
        let shown = Facts { from: value, to: value, date: value, ..Facts::default() };
        let mut harness = builder().allow_missing_glyphs().with_size(vec2(560.0 + 48.0, 200.0)).build_ui(move |ui| {
            crate::component::tests::stage(ui, theme.palette.surface, |ui| {
                ui.set_width(560.0);
                facts_of(ui, &theme, &Keymap::default(), &shown, false);
            });
        });
        harness.run();
        let paint = painted(&harness, &spoken(&shown));
        let values: Vec<String> = texts(&paint).into_iter().map(|text| text.1).filter(|text| text.chars().next() == value.chars().next()).collect();
        assert_eq!(values.len(), 3, "the three values");
        for drawn in values {
            assert!(drawn.chars().count() <= crate::component::MOST + 1, "capped: {} characters", drawn.chars().count());
            assert!(drawn.ends_with('…') && whole_prefix(value, &drawn), "a whole-grapheme beginning: {drawn:?}");
        }
    }
}
