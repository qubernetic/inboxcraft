// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::Color32;

use super::{Laid, MAX_ATTRIBUTION, MAX_LINES, Role, layout, roles};
use crate::theme::{Mode, TextScale, Theme, install_fonts};

/// A reply's text as the engine writes it: a line over the quote, the quote, the signature.
const REPLY: &str = "Thanks, I can bring the hose.\n\nOdile Brannock wrote on 4 Nov 2026 at 18:12:\n> Hi all,\n>\n> The rota for November is below. The second week is still open, and someone with a car would help with the manure delivery on the Saturday.\n> Odile\n-- \nMira";

const WIDTH: f32 = 600.0;

/// `f` run with a ui of the light theme's fonts.
fn with_ui<R>(f: impl FnOnce(&egui::Ui, &Theme) -> R) -> R {
    let ctx = egui::Context::default();
    install_fonts(&ctx);
    let theme = Theme::new(Mode::Light, TextScale::default());
    let mut f = Some(f);
    let mut out = None;
    let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
        if let Some(f) = f.take() {
            out = Some(f(ui, &theme));
        }
    });
    output.textures_delta.clear();
    out.unwrap()
}

fn laid(text: &str) -> Option<Laid> {
    with_ui(|ui, theme| layout(ui, theme, &[], text, WIDTH))
}

/// The rows of the line `line` (counted from 0) in `galley`.
fn rows_of(galley: &egui::Galley, line: usize) -> Vec<&egui::epaint::text::PlacedRow> {
    let mut at = 0;
    let mut rows = Vec::new();
    for row in &galley.rows {
        if at == line {
            rows.push(row);
        }
        if row.ends_with_newline {
            at += 1;
        }
    }
    rows
}

/// The format the byte `at` of `galley`'s text is laid out in.
fn format_at(galley: &egui::Galley, at: usize) -> &egui::TextFormat {
    &galley.job.sections.iter().find(|section| section.byte_range.start.0 <= at && at < section.byte_range.end.0).unwrap().format
}

#[test]
fn the_line_over_a_quote_that_ends_with_a_colon_says_who_wrote_it() {
    let roles = roles(REPLY);
    assert_eq!(
        roles,
        vec![
            Role::Text,
            Role::Text,
            Role::Attribution,
            Role::Quoted { prefix: 2 },
            Role::Quoted { prefix: 1 },
            Role::Quoted { prefix: 2 },
            Role::Quoted { prefix: 2 },
            Role::Text,
            Role::Text,
        ]
    );
    assert_eq!(super::roles("Note:\n\n> a")[0], Role::Text, "a blank line between: not the attribution");
    assert_eq!(super::roles(&format!("{}:\n> a", "x".repeat(MAX_ATTRIBUTION - 1)))[0], Role::Attribution);
    assert_eq!(super::roles(&format!("{}:\n> a", "x".repeat(MAX_ATTRIBUTION)))[0], Role::Text, "too long to be one");
    assert_eq!(super::roles("  >> deeper")[0], Role::Quoted { prefix: 3 }, "one level hidden, the next shows");
    assert!(layout_none("No quote here.\n\nMira"), "a text that quotes nothing is laid out as before");
}

fn layout_none(text: &str) -> bool {
    laid(text).is_none()
}

#[test]
fn a_reply_s_quote_is_laid_out_as_the_block_and_its_text_stays_as_it_is() {
    let laid = laid(REPLY).unwrap();
    let galley = &laid.galley;
    assert_eq!(galley.text(), REPLY, "the layout is a view of the text, byte for byte");
    let theme = Theme::new(Mode::Light, TextScale::default());
    let indent = super::BAR + theme.spacing.s3;
    assert_eq!(indent, 14.0);
    // The owner's line: at the left, in `read`, in ink.
    let own = rows_of(galley, 0);
    assert_eq!(own[0].pos.x, 0.0);
    assert_eq!(format_at(galley, 0).font_id.size, theme.fonts.read.size);
    // The attribution: 14 in, `mono_s` in `muted`, its row 16 + 6.
    let at = REPLY.find("Odile Brannock wrote").unwrap();
    let attribution = rows_of(galley, 2);
    assert_eq!(attribution[0].pos.x, indent);
    assert_eq!(attribution[0].size.y, 22.0, "its 16 and the 6 under it");
    assert_eq!(format_at(galley, at).font_id.size, theme.fonts.mono_s.size);
    assert_eq!(format_at(galley, at).color, theme.palette.muted);
    // A quoted line: its `> ` hidden, its text 14 in, in `body` (14/22) in `muted`; a long one wraps inside the block.
    let at = REPLY.find("> The rota").unwrap();
    assert_eq!(format_at(galley, at).color, Color32::TRANSPARENT, "the quote mark is not drawn");
    assert_eq!(format_at(galley, at + 2).font_id.size, theme.fonts.body.size);
    assert_eq!(format_at(galley, at + 2).color, theme.palette.muted);
    let quoted = rows_of(galley, 5);
    assert!(quoted.len() > 1, "the long line wraps");
    let first_letter = quoted[0].pos.x + quoted[0].glyphs[2].pos.x;
    assert!((first_letter - indent).abs() < 0.5, "the text starts 14 in: {first_letter}");
    // The hidden `> ` stands in the room left of the text, right of the bar.
    assert!(quoted[0].pos.x >= super::BAR, "{:?}", quoted[0].rect());
    for row in &quoted {
        let left = if std::ptr::eq(*row, quoted[0]) { super::BAR } else { indent - 0.5 };
        assert!(row.pos.x >= left && row.pos.x + row.size.x <= WIDTH + 0.5, "inside the block: {:?}", row.rect());
        assert_eq!(row.size.y, 22.0);
    }
    // A long attribution wraps in rows of 16, the 6 under the last.
    let long = format!("{} wrote:\n> a", "Someone ".repeat(20));
    let wrapped = with_ui(|ui, theme| layout(ui, theme, &[], &long, WIDTH)).unwrap();
    let rows = rows_of(&wrapped.galley, 0);
    assert!(rows.len() > 1);
    assert!(rows[..rows.len() - 1].iter().all(|row| row.size.y == 16.0));
    assert_eq!(rows[rows.len() - 1].size.y, 22.0);
    // The empty quoted line keeps its row of 22.
    assert_eq!(rows_of(galley, 4)[0].size.y, 22.0);
    // One block, from the attribution's top to the last quoted line's bottom.
    let last = rows_of(galley, 6);
    assert_eq!(laid.blocks.len(), 1);
    assert_eq!(laid.blocks[0].min, attribution[0].pos.y);
    assert_eq!(laid.blocks[0].max, last[last.len() - 1].rect().bottom());
    // The signature under it, back at the left in `read`.
    assert_eq!(rows_of(galley, 8)[0].pos.x, 0.0);
}

#[test]
fn a_text_laid_out_line_by_line_puts_its_rows_where_one_layout_does() {
    // The lines that are not quoted keep egui's own layout: the same rows, at the same places, with the same letters,
    // so the caret walks them as before.
    let text = "Hi both,\n\nWe meet at the plot at ten and finish by noon. Bring gloves; the shed has spades and the wheelbarrow, and more.\n\n";
    with_ui(|ui, theme| {
        let marks = vec!["ten".to_owned()];
        let roles = vec![Role::Text; text.split('\n').count()];
        let ours = super::lay_out(ui, theme, &marks, text, &roles, WIDTH).galley;
        let theirs = super::super::marked(ui, theme, &marks, text, WIDTH);
        assert_eq!(ours.rows.len(), theirs.rows.len());
        for (a, b) in ours.rows.iter().zip(&theirs.rows) {
            assert_eq!(a.pos, b.pos);
            assert_eq!(a.size, b.size);
            assert_eq!(a.ends_with_newline, b.ends_with_newline);
            assert_eq!(a.glyphs.len(), b.glyphs.len());
        }
        assert_eq!(ours.text(), text);
    });
}

#[test]
fn hostile_quotes_are_bounded_and_wrap_inside_the_block() {
    // 10 000 quoted lines, long ones, deep `>>>>` and a run of bidi controls.
    let long = "word ".repeat(100);
    let deep = ">".repeat(300);
    let mut text = String::from("Someone wrote:\n");
    for at in 0..10_000 {
        match at % 3 {
            0 => text.push_str(&format!("> {long}\n")),
            1 => text.push_str(&format!("{deep} deep\n")),
            _ => text.push_str("> \u{202E}\u{2066}reversed?\u{0007}\n"),
        }
    }
    text.push_str("after");
    let roles = roles(&text);
    let quoted = roles.iter().filter(|role| matches!(role, Role::Quoted { .. })).count();
    assert_eq!(quoted, MAX_LINES, "at most {MAX_LINES} lines are drawn as a quote");
    let laid = laid(&text).unwrap();
    assert_eq!(laid.galley.text(), text);
    for row in &laid.galley.rows {
        assert!(row.pos.x >= 0.0 && row.pos.x + row.size.x <= WIDTH + 0.5, "inside the width: {:?}", row.rect());
    }
    assert_eq!(laid.blocks.len(), 1);
}

#[test]
fn a_bidi_control_in_a_quote_is_hidden_and_stays_in_its_line() {
    let text = "Odile wrote \u{202E}backwards:\n> abc\u{202E}def\u{2067}\nafter the quote";
    let laid = laid(text).unwrap();
    let galley = &laid.galley;
    for control in ['\u{202E}', '\u{2067}'] {
        for (at, _) in text.match_indices(control) {
            assert_eq!(format_at(galley, at).color, Color32::TRANSPARENT, "{control:?} at {at} is not drawn");
        }
    }
    // The line after the quote is laid out as it is alone: left to right from the left edge.
    let after = rows_of(galley, 2);
    let alone = with_ui(|ui, theme| super::super::marked(ui, theme, &[], "after the quote", WIDTH));
    let xs = |row: &egui::epaint::text::PlacedRow| row.glyphs.iter().map(|glyph| glyph.pos.x).collect::<Vec<_>>();
    assert_eq!(after[0].pos.x, 0.0);
    assert_eq!(xs(after[0]), xs(&alone.rows[0]));
}

#[test]
fn a_changed_text_is_laid_out_anew() {
    // Two texts of as many lines, one letter apart: the second is not the first one's layout.
    let first = "Odile wrote:\n> Hi all,";
    let second = "Odile wrote:\n> Hi alll,";
    with_ui(|ui, theme| {
        let a = layout(ui, theme, &[], first, WIDTH).unwrap();
        let b = layout(ui, theme, &[], second, WIDTH).unwrap();
        assert_eq!(a.galley.text(), first);
        assert_eq!(b.galley.text(), second);
        let c = layout(ui, theme, &[], second, WIDTH - 100.0).unwrap();
        assert_eq!(c.galley.job.wrap.max_width, WIDTH - 100.0, "nor at another width");
    });
}
