// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A sync in the status line (M9) at the three levels: at full and reduced the bar of ten characters fills as the work
//! is done and the ring goes round; at off one print says a sync runs, no bar and no share, the ring still, and the
//! line rests.

use egui::vec2;
use egui_kittest::Harness;

use super::{Facts, Shown, Sync, line};
use crate::keymap::Keymap;
use crate::motion::Level;
use crate::theme::{Mode, TextScale, Theme};

/// The status line at `level` with a sync 30 % done.
fn syncing(level: Level) -> Harness<'static> {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let facts = Facts { sync: Sync::Syncing(0.3, "Personal · 30 %".to_owned()), accounts: 2, ..Facts::default() };
    let mut harness = crate::component::tests::builder().with_size(vec2(900.0, 40.0)).with_step_dt(0.01).build_ui(move |ui| {
        crate::motion::set_level(ui.ctx(), level);
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), vec2(900.0, 26.0));
        let shown = Shown {
            facts: &facts,
            message: None,
            now: 0.0,
            whole: 0.0,
            hints: &[],
            undo_key: false,
            rests: false,
            lifted: false,
            undo_taken: false,
            escape: false,
        };
        let _ = line(ui, &theme, &Keymap::default(), rect, &shown);
    });
    harness.run_steps(2);
    harness
}

fn texts(harness: &Harness<'static>) -> Vec<String> {
    let mut out = Vec::new();
    for clipped in &harness.output().shapes {
        if let egui::Shape::Text(text) = &clipped.shape {
            out.push(text.galley.job.text.clone());
        }
    }
    out
}

#[test]
fn at_full_and_reduced_the_bar_fills_as_the_work_is_done() {
    for level in [Level::Full, Level::Reduced] {
        let harness = syncing(level);
        let seen = texts(&harness);
        assert!(seen.iter().any(|text| text == "[===>      ]"), "{level:?}: a cell for each tenth done, {seen:?}");
        assert!(seen.iter().any(|text| text == "Personal · 30 %"), "{level:?}: the share, {seen:?}");
    }
}

#[test]
fn at_off_one_print_says_a_sync_runs_with_no_bar_and_the_line_rests() {
    let mut harness = syncing(Level::Off);
    let seen = texts(&harness);
    assert!(seen.iter().any(|text| text == "syncing…"), "one print, {seen:?}");
    assert!(!seen.iter().any(|text| text.starts_with('[') || text.contains('%')), "no bar, no share, {seen:?}");
    assert!(crate::component::tests::rests(&mut harness, 2), "the ring still: nothing asks for a frame");
}
