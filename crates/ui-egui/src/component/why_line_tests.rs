// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::{spoken, why_line};
use crate::component::tests::{Look, own, painted, staged};
use crate::keymap::Keymap;
use crate::theme::{Mode, TextScale, Theme};

/// The sheet's four states and their words, reason by reason.
const STATES: [(&str, &[&str]); 4] = [
    ("group", &["let in by you on 3 Nov", "grouped as People", "synced 14:02 by IDLE"]),
    ("new-sender", &["first mail on 14 Nov", "let in by you today", "grouped as People"]),
    ("screener", &["first mail from this address", "SPF and DKIM pass", "held back"]),
    ("folder", &["moved by you on 9 Nov", "in Studio / Clients / Bakery"]),
];

/// The reasons as the line draws them when all fit.
fn joined(reasons: &[&str]) -> String {
    reasons.join(super::SEPARATOR)
}

/// The line `width` wide on the reader's `surface`.
fn harness_at(theme: Theme, reasons: &'static [&'static str], width: f32) -> Harness<'static> {
    staged(
        theme,
        |p| p.surface,
        vec2(width + 48.0, 100.0),
        move |ui, theme| {
            ui.set_width(width);
            why_line(ui, theme, &Keymap::default(), reasons);
        },
    )
}

/// The line 640 wide on the reader's `surface`.
fn harness(theme: Theme, reasons: &'static [&'static str]) -> Harness<'static> {
    harness_at(theme, reasons, 640.0)
}

/// The texts painted in the line that are not its own words (the label, the link, the tag).
fn reasons_painted(harness: &Harness<'static>, name: &str) -> Vec<(String, egui::Rect, bool)> {
    let paint = painted(harness, name);
    paint
        .shapes
        .iter()
        .filter_map(|shape| match &shape.look {
            Look::Text { text, .. } if !(text.is_empty() || text.starts_with("coming") || text.starts_with("WHY") || text.starts_with("open")) => {
                Some((text.clone(), shape.rect, shape.inside || shape.clipped))
            }
            _ => None,
        })
        .collect()
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
                    Look::Text { text, colour, size, .. } if text.starts_with(words) => {
                        Some((shape.rect.min - rect.min, text.clone(), *colour, *size))
                    }
                    _ => None,
                })
            };
            let (label_at, _, label_ink, label_size) = text("WHY HERE").unwrap_or_else(|| panic!("WHY HERE: {at}"));
            assert_eq!((label_at.x, label_ink, label_size), (12.0, p.cyan_text, f.label.size), "{at}");
            // The reasons 12 right of the label's box (59 wide), in mono_s and muted, on one line, the design's words.
            let (words_at, words, words_ink, words_size) = text(reasons[0]).unwrap_or_else(|| panic!("reasons: {at}"));
            assert_eq!((words_at.x, words.as_str(), words_ink, words_size), (83.0, joined(reasons).as_str(), p.muted, f.mono_s.size), "{at}");
            // The log is not built: its link stands unavailable at the right end, where the sheet puts it; the reasons
            // keep their room up to 12 before it (467 wide).
            let link = harness.get_by_label("open log →");
            assert!(link.accesskit_node().is_disabled(), "{at}");
            assert_eq!(link.rect().right(), rect.right() - 12.0, "{at}");
            assert_eq!(link.rect().left() - rect.left(), 562.0, "{at}");
            let said = harness.get_by_label(&reasons.join(", ")).rect();
            assert_eq!((said.left() - rect.left(), said.width()), (83.0, 467.0), "{at}");
            assert!(harness.query_all_by_label("coming soon").count() <= 1, "{at}");
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
fn a_screen_reader_hears_every_reason_as_one_sentence() {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    // In 360 only the first reason shows; the line is still heard whole.
    let reasons = STATES[1].1;
    let harness = harness_at(theme, reasons, 360.0);
    let name = "Why here: first mail on 14 Nov, let in by you today, grouped as People";
    assert_eq!(spoken(reasons), name);
    let line = harness.get_by_label(name);
    assert_eq!(line.accesskit_node().role(), egui::accesskit::Role::Group);
    let drawn = reasons_painted(&harness, name);
    assert_eq!(drawn.iter().map(|(text, ..)| text.as_str()).collect::<Vec<_>>(), [reasons[0]], "{drawn:?}");
}

#[test]
fn hostile_reasons_stay_on_one_cut_line_and_are_heard_whole() {
    let theme = Theme::new(Mode::Dark, TextScale::ALL[4]);
    let long: &'static str = Box::leak(format!("\u{202E}{}\u{0}folder", "Ż".repeat(600)).into_boxed_str());
    let reasons: &'static [&'static str] = Box::leak(vec![long, "let in by you"].into_boxed_slice());
    let harness = harness(theme, reasons);
    let name = spoken(reasons);
    assert!(!name.contains('\u{202E}') && !name.contains('\u{0}'), "no override or control in the name");
    let rect = harness.get_by_label(&name).rect();
    let drawn = reasons_painted(&harness, &name);
    assert_eq!(drawn.len(), 1, "one run of the reasons: {drawn:?}");
    let (text, at, inside) = &drawn[0];
    assert!(text.starts_with('Ż') && text.ends_with('…') && !text.contains("let in"), "the first cut with one ellipsis, the second gone: {text:?}");
    assert!(*inside && at.right() <= rect.right() && at.height() <= rect.height(), "the reasons stay inside the line: {at:?} {rect:?}");
}

#[test]
fn an_address_of_254_characters_is_cut_inside_the_line_and_never_widens_it() {
    let address: &'static str = Box::leak(format!("you put {}@example.org in People", "a".repeat(242)).into_boxed_str());
    let reasons: &'static [&'static str] = Box::leak(vec![address, "a sender you know", "in a thread of 3 messages", "in Inbox"].into_boxed_slice());
    for scale in [TextScale::default(), TextScale::ALL[4]] {
        for width in [640.0, 420.0] {
            let theme = Theme::new(Mode::Light, scale);
            let harness = harness_at(theme, reasons, width);
            let name = spoken(reasons);
            let rect = harness.get_by_label(&name).rect();
            assert_eq!(rect.width(), width, "{scale:?} {width}");
            let drawn = reasons_painted(&harness, &name);
            assert_eq!(drawn.len(), 1, "{drawn:?}");
            let (text, at, inside) = &drawn[0];
            assert!(text.starts_with("you put aaa") && text.ends_with('…') && !text.contains('·'), "{text:?}");
            assert!(*inside && at.right() <= rect.right() - 12.0, "{at:?} {rect:?}");
        }
    }
}

#[test]
fn four_reasons_leave_whole_from_the_end_as_the_line_narrows() {
    const FOUR: &[&str] = &["you put ana@example.org in People", "first mail from this address", "in a thread of 3 messages", "in Inbox"];
    for scale in [TextScale::default(), TextScale::ALL[4]] {
        let theme = Theme::new(Mode::Dark, scale);
        let mut shown_before = usize::MAX;
        for width in [1400.0, 1000.0, 800.0, 640.0, 520.0, 400.0, 300.0, 200.0, 120.0] {
            let harness = harness_at(theme, FOUR, width);
            let name = spoken(FOUR);
            let rect = harness.get_by_label(&name).rect();
            let at = format!("{scale:?} {width}");
            let drawn = reasons_painted(&harness, &name);
            assert!(drawn.len() <= 1, "{at}: {drawn:?}");
            // With no room left beside the label, nothing of them is drawn.
            let Some((text, place, inside)) = drawn.first() else {
                shown_before = 0;
                continue;
            };
            assert!(*inside && place.right() <= rect.right(), "{at}");
            // Whole reasons from the first, or the first alone cut with one ellipsis.
            let whole = (1..=FOUR.len()).map(|count| joined(&FOUR[..count])).position(|kept| &kept == text).map(|at| at + 1);
            let cut = text.ends_with('…') && FOUR[0].starts_with(text.trim_end_matches('…'));
            assert!(whole.is_some() || cut, "{at}: {text:?}");
            let shown = whole.unwrap_or(0);
            assert!(shown <= shown_before, "{at}: fewer as the line narrows");
            shown_before = shown;
        }
        assert_eq!(shown_before, 0, "{scale:?}: at 120 the first is cut or gone");
    }
    // Wide, all four show.
    let harness = harness_at(Theme::new(Mode::Dark, TextScale::default()), FOUR, 1400.0);
    assert_eq!(reasons_painted(&harness, &spoken(FOUR))[0].0, joined(FOUR));
}

#[test]
fn with_no_reason_the_tag_stands_where_the_reasons_go() {
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::default());
        let harness = harness(theme, &[]);
        let rect = harness.get_by_label(&spoken::<&str>(&[])).rect();
        let tag = harness.get_by_label("coming soon").rect();
        assert_eq!(tag.left() - rect.left(), 83.0, "{mode:?}: the tag in the reasons' place");
        assert_eq!(harness.get_by_label("open log →").rect().right(), rect.right() - 12.0, "{mode:?}: the link where the sheet puts it");
    }
}

#[test]
fn in_a_narrow_line_nothing_overlaps_and_nothing_paints_past_it() {
    for scale in [TextScale::default(), TextScale::ALL[4]] {
        let theme = Theme::new(Mode::Dark, scale);
        for reasons in [&[][..], STATES[3].1] {
            for width in [640.0, 400.0, 300.0, 240.0, 160.0, 100.0, 1.0] {
                let harness = harness_at(theme, reasons, width);
                let at = format!("{scale:?} {width} {reasons:?}");
                let rect = harness.get_by_label(&spoken(reasons)).rect();
                let link = harness.query_by_label("open log →").map(|node| node.rect());
                let tag = harness.query_by_label("coming soon").map(|node| node.rect());
                // With no reason the link (unavailable) goes before the tag.
                assert!(link.is_none() || tag.is_some() || !reasons.is_empty(), "the link stays only while the tag does: {at}");
                for part in [link, tag].into_iter().flatten() {
                    assert!(rect.contains_rect(part), "{part:?} inside {rect:?}: {at}");
                }
                if let (Some(link), Some(tag)) = (link, tag) {
                    assert!(tag.right() <= link.left(), "{tag:?} {link:?}: {at}");
                }
                for (text, place, inside) in reasons_painted(&harness, &spoken(reasons)) {
                    assert!(inside, "{text:?} paints past the line: {at}");
                    for part in [link, tag].into_iter().flatten() {
                        assert!(place.right() <= part.left() + 0.5 || place.left() >= part.right() - 0.5, "{text:?} under {part:?}: {at}");
                    }
                }
            }
        }
    }
}
