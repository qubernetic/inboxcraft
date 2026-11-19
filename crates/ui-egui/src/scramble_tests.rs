// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Scramble (M3): each character changes once, white space stays, a section keeps its characters; a button's label at
//! the three levels, frame by frame, a press during it, and rest.

use egui::text::{LayoutJob, TextFormat};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

use super::{Side, glyph, job, mixed, real};
use crate::component::button::TextButton;
use crate::component::tests::{builder, rests, stage, texts};
use crate::keymap::Keymap;
use crate::motion::{self, Level};
use crate::theme::{Mode, TextScale, Theme};

#[test]
fn a_glyph_is_the_same_in_every_frame_and_comes_from_the_code_set() {
    for index in 0..64 {
        assert_eq!(glyph(7, index), glyph(7, index));
        assert!(crate::field::CODE.contains(&(glyph(7, index) as u8)));
    }
    assert!((0..16).any(|index| glyph(1, index) != glyph(2, index)), "the seed changes the glyphs");
}

#[test]
fn each_character_changes_once_and_white_space_stays() {
    let text = "Good afternoon,\nOdile.";
    let count = text.chars().count();
    let frames: Vec<Vec<char>> = (0..=20).map(|step| mixed(text, real(count, step as f32 / 20.0, Side::Left), 3).chars().collect()).collect();
    for (index, original) in text.chars().enumerate() {
        let seen: Vec<char> = frames.iter().map(|frame| frame[index]).collect();
        let changes = seen.windows(2).filter(|pair| pair[0] != pair[1]).count();
        assert!(changes <= 1, "{original:?} at {index} changed {changes} times: {seen:?}");
        assert_eq!(seen.last(), Some(&original), "it ends as itself");
        if original.is_whitespace() {
            assert!(seen.iter().all(|ch| *ch == original), "white space stays");
        }
    }
    assert_eq!(real(10, 0.5, Side::Right), 0..5, "going out, the right half is glyphs");
    assert_eq!(real(10, f32::NAN, Side::Left), 0..10, "nonsense is the end");
    assert_eq!(real(10, -1.0, Side::Left), 0..0);
}

#[test]
fn a_scrambled_job_keeps_each_section_on_its_characters() {
    let mut laid = LayoutJob::default();
    laid.append("ég ", 0.0, TextFormat::default());
    laid.append("ünd", 0.0, TextFormat { italics: true, ..TextFormat::default() });
    let out = job(&laid, &(0..1), 9).unwrap();
    assert_eq!(out.text.chars().count(), 6);
    assert!(out.text.starts_with('é'));
    let second = out.sections[1].byte_range.clone();
    assert_eq!(out.text.get(second.start.0..second.end.0).unwrap().chars().count(), 3);
    assert!(out.sections[1].format.italics);
    let long = LayoutJob::single_section("x".repeat(super::MOST + 1), TextFormat::default());
    assert!(job(&long, &(0..0), 1).is_none(), "past the cap a text stays as it is");
}

/// A text button `Reply` alone at `level`, frames 10 ms apart.
fn reply(level: Level) -> Harness<'static> {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let mut harness = builder().with_size(egui::vec2(300.0, 100.0)).with_step_dt(0.01).build_ui(move |ui| {
        motion::set_level(ui.ctx(), level);
        stage(ui, theme.palette.panel, |ui| {
            TextButton::new("Reply").show(ui, &theme, &Keymap::default());
        });
    });
    harness.run();
    harness
}

/// The label as painted.
fn label(harness: &Harness<'static>) -> String {
    texts(harness).into_iter().next().unwrap_or_default()
}

/// Whether `shown` reads as `Reply` for its first `n` characters and is glyphs after them.
fn resolved(shown: &str, n: usize) -> bool {
    let real: Vec<char> = "Reply".chars().collect();
    shown.chars().count() == 5 && shown.chars().enumerate().all(|(index, ch)| (index < n) == (Some(&ch) == real.get(index)))
}

#[test]
fn at_full_a_label_resolves_from_the_left_on_hover_in_300_ms_and_rests() {
    let mut harness = reply(Level::Full);
    assert_eq!(label(&harness), "Reply", "at rest the label reads");
    harness.get_by_label("Reply").hover();
    harness.step();
    assert!(resolved(&label(&harness), 0), "0 ms: its label in glyphs, {:?}", label(&harness));
    harness.run_steps(13);
    assert!(resolved(&label(&harness), 2), "130 ms: two of five back, {:?}", label(&harness));
    harness.run_steps(17);
    assert_eq!(label(&harness), "Reply", "300 ms: it reads");
    harness.run_steps(20);
    assert_eq!(label(&harness), "Reply", "it does not run again while the pointer stays");
    harness.hover_at(egui::pos2(1.0, 1.0));
    assert!(rests(&mut harness, 30), "and rests once the pointer left");
}

#[test]
fn at_full_a_press_during_the_scramble_ends_it_in_that_frame() {
    let mut harness = reply(Level::Full);
    harness.get_by_label("Reply").hover();
    harness.run_steps(3);
    assert!(!resolved(&label(&harness), 5));
    harness.get_by_label("Reply").click();
    harness.step();
    assert_eq!(label(&harness), "Reply", "the press acts and the label reads at once");
}

#[test]
fn at_reduced_and_off_a_label_never_scrambles() {
    for level in [Level::Reduced, Level::Off] {
        let mut harness = reply(level);
        harness.get_by_label("Reply").hover();
        harness.step();
        assert_eq!(label(&harness), "Reply", "{level:?}: no hover scramble");
        harness.run_steps(10);
        assert_eq!(label(&harness), "Reply", "{level:?}: none later");
    }
}
