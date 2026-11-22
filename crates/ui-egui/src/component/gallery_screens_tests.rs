// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::{page, size};
use crate::component::gallery::keymap;
use crate::component::tests::{builder, keyboard_walk, painted, rests};
use crate::motion;
use crate::theme::{Mode, TextScale, Theme};

/// The page in `theme`, motion reduced or not.
fn harness(theme: Theme, reduce: bool) -> Harness<'static> {
    let keymap = keymap();
    let mut harness = builder().with_size(size()).build_ui(move |ui| {
        motion::set_reduced(ui.ctx(), reduce);
        page(ui, &theme, &keymap);
    });
    harness.run_ok();
    harness
}

#[test]
fn every_control_of_the_screens_page_is_reached_by_tab() {
    let mut harness = harness(Theme::new(Mode::Light, TextScale::default()), true);
    assert_eq!(keyboard_walk(&mut harness), Vec::<String>::new(), "the controls Tab never reaches");
}

#[test]
fn the_screens_page_rests() {
    for reduce in [true, false] {
        let mut harness = harness(Theme::new(Mode::Dark, TextScale::default()), reduce);
        assert!(rests(&mut harness, 8), "the page keeps asking for frames with motion reduced: {reduce}");
    }
}

#[test]
fn the_draft_file_board_is_coming_soon_and_fits_the_page() {
    for mode in [Mode::Light, Mode::Dark] {
        let theme = Theme::new(mode, TextScale::default());
        let harness = harness(theme, true);
        harness.get_by_label("coming soon");
        let whole = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), size());
        let files: Vec<_> = harness.query_all_by_role(Role::ListItem).collect();
        assert_eq!(files.len(), 4, "{mode:?}: the sheet's four states");
        for file in files {
            assert!(whole.contains_rect(file.rect().expand(3.0)), "{:?} leaves the page", file.accesskit_node().label());
        }
        assert_eq!(painted(&harness, "open-day-all.zip, 48 MB, too large to send").ground, Some(theme.palette.bg), "{mode:?}");
    }
}
