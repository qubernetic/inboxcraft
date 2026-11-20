// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::Arc;

use egui::{Color32, Galley, Ui, pos2, vec2};

use crate::theme::{Font, Mode, TextScale, Theme};

/// What `draw` lays out in a headless frame with the app's fonts (the second of two passes, so the fonts are in).
fn laid(draw: impl Fn(&Ui) -> Vec<Arc<Galley>>) -> Vec<Arc<Galley>> {
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    let mut found = Vec::new();
    for _ in 0..2 {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(1200.0, 400.0))), ..egui::RawInput::default() };
        let mut output = ctx.run_ui(input, |ui| found = draw(ui));
        output.textures_delta.clear();
    }
    found
}

/// Each row of `galley` as it is seen, left to right: the signs that take room (a letter, a space, the ellipsis), by
/// where they stand. egui gives every letter of a word shaped right to left in its visual place, with signs of no
/// width after it that stand for the rest of its cluster; those are left out.
fn seen(galley: &Galley) -> Vec<String> {
    galley
        .rows
        .iter()
        .map(|row| {
            let mut signs: Vec<(f32, char)> =
                row.glyphs.iter().filter(|glyph| glyph.advance_width > 0.0).map(|glyph| (glyph.pos.x, glyph.chr)).collect();
            signs.sort_by(|a, b| a.0.total_cmp(&b.0));
            signs.into_iter().map(|(_, chr)| chr).collect()
        })
        .collect()
}

/// [`seen`] of a galley of one row.
fn line(galley: &Galley) -> String {
    seen(galley).concat()
}

fn fonts() -> crate::theme::Fonts {
    Theme::new(Mode::Dark, TextScale::default()).fonts
}

fn body() -> Font {
    fonts().body
}

/// Turned round, character by character: how a right-to-left line of letters only (no marks) is seen.
fn backwards(text: &str) -> String {
    text.chars().rev().collect()
}

#[test]
fn a_right_to_left_phrase_stands_in_its_words_order() {
    let hebrew = "שלום עולם רב";
    let arabic = "مرحبا يا عالم";
    let found =
        laid(|ui| vec![crate::component::galley(ui, hebrew, body(), Color32::WHITE), crate::component::galley(ui, arabic, body(), Color32::WHITE)]);
    let [hebrew_line, arabic_line] = found.as_slice() else { panic!("two lines") };
    // The first word stands at the right, the last at the left.
    assert_eq!(line(hebrew_line), backwards(hebrew), "Hebrew words in the order they are read");
    // Arabic letters join, so only the words' order is held: the first word is the rightmost.
    let words: Vec<String> = line(arabic_line).split(' ').map(str::to_owned).collect();
    assert_eq!(words.len(), 3, "{words:?}");
    let first_right = words.last().is_some_and(|word| word.chars().any(|c| c == 'ح'));
    let last_left = words.first().is_some_and(|word| word.chars().any(|c| c == 'ع'));
    assert!(first_right && last_left, "Arabic words in the order they are read: {words:?}");
}

#[test]
fn a_cut_right_to_left_label_keeps_its_start() {
    let text = "שלום עולם רב מאוד מאוד";
    let kept = "שלום עולם…";
    let found = laid(|ui| {
        let room = crate::draw::paint::measured(&crate::component::galley(ui, kept, body(), Color32::WHITE)) + 0.5;
        vec![crate::component::cut(ui, text, body(), Color32::WHITE, room), crate::component::fitted(ui, text, body(), Color32::WHITE, room)]
    });
    for galley in &found {
        // The first two words, the ellipsis at the logical end: at the left.
        assert_eq!(line(galley), "…םלוע םולש", "cut at the logical end");
    }
    assert!(found.first().is_some_and(|cut| cut.elided), "a cut line is marked as cut, as egui marks one");
}

#[test]
fn a_cut_right_to_left_subject_on_two_rows_keeps_its_start() {
    let text = "שלום עולם רב מאוד מאוד מאוד מאוד מאוד";
    let found = laid(|ui| {
        let room = crate::draw::paint::measured(&crate::component::galley(ui, "שלום עולם רב", body(), Color32::WHITE)) + 0.5;
        vec![crate::component::wrapped(ui, text, body(), Color32::WHITE, room, 2, egui::Align::LEFT)]
    });
    let [galley] = found.as_slice() else { panic!("one galley") };
    let rows = seen(galley);
    assert_eq!(rows.first().map(String::as_str), Some(backwards("שלום עולם רב").as_str()), "the first row: the first words, in order");
    let second = rows.get(1).cloned().unwrap_or_default();
    assert!(second.starts_with('…') && second.ends_with('מ'), "the second row goes on with the next words and ends at the left: {second:?}");
    assert_eq!(rows.len(), 2);
}

#[test]
fn mixed_runs_and_numbers_stand_where_the_algorithm_puts_them() {
    let found = laid(|ui| {
        ["Re: שלום עולם 2026", "שלום (Kiln) עולם", "Year ٢٠٢٦ done", "Kiln rota 12:30"]
            .iter()
            .map(|text| crate::component::galley(ui, text, body(), Color32::WHITE))
            .collect()
    });
    let shown: Vec<String> = found.iter().map(|galley| line(galley)).collect();
    assert_eq!(
        shown,
        [
            // A number after right-to-left words goes with them (UAX #9 W7 to N1).
            "Re: 2026 םלוע םולש",
            // A right-to-left paragraph: the Latin word inside keeps its letters, its brackets turn round.
            "םלוע (Kiln) םולש",
            // Arabic digits in a left-to-right line read left to right.
            "Year ٢٠٢٦ done",
            // A line with nothing right to left is laid out as before.
            "Kiln rota 12:30",
        ]
    );
}

#[test]
fn bidi_controls_reorder_only_their_own_label_and_hide_nothing() {
    let font = body();
    let found = laid(|ui| {
        // Two sections on one line, as a row's name and subject are.
        let sections = |first: &str| {
            let mut job = egui::text::LayoutJob {
                wrap: egui::text::TextWrapping { max_width: 600.0, max_rows: 1, break_anywhere: true, overflow_character: Some('…') },
                break_on_newline: false,
                ..egui::text::LayoutJob::default()
            };
            // Two colours, so egui keeps them two sections.
            for (text, colour) in [(first, Color32::WHITE), ("Kiln rota", Color32::GRAY)] {
                job.append(&font.text(text), 0.0, font.format(colour));
            }
            job
        };
        [
            // An override left open, an isolate left open, an embedding: each in the first section only.
            "\u{202E}abc def",
            "\u{2067}abc def",
            "\u{202B}abc \u{200F}",
        ]
        .iter()
        .map(|first| crate::draw::paint::cut_within(ui.painter(), sections(first), 600.0))
        .collect()
    });
    for galley in &found {
        assert!(!galley.job.text.contains(['\u{202E}', '\u{2067}', '\u{202B}', '\u{200F}']), "no control is laid out: {:?}", galley.job.text);
    }
    let shown: Vec<String> = found.iter().map(|galley| line(galley)).collect();
    // The override turns round its own letters; in the isolate and the embedding the Latin words keep their order (the
    // embedding's space before its right-to-left mark goes to its right-to-left level, left of the word): no letter is
    // lost, no control is drawn, and the next section stays as written.
    assert_eq!(shown, ["fed cbaKiln rota", "abc defKiln rota", " abcKiln rota"]);
}

#[test]
fn ten_thousand_characters_of_right_to_left_text_stay_bounded() {
    let text: String = "שלום עולם ".chars().cycle().take(10_000).collect();
    let found = laid(|ui| {
        vec![
            crate::component::galley(ui, &text, body(), Color32::WHITE),
            crate::component::cut(ui, &text, body(), Color32::WHITE, 200.0),
            crate::component::wrapped(ui, &text, body(), Color32::WHITE, 200.0, 2, egui::Align::LEFT),
        ]
    });
    for galley in &found {
        let letters = galley.rows.iter().map(|row| row.glyphs.len()).sum::<usize>();
        // A glyph per character at most, and the signs egui adds for a cluster: twice the cap, over.
        assert!(letters <= 2 * super::MOST * 2, "{letters} glyphs");
    }
}
