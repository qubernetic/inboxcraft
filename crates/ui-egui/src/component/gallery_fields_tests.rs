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
        ui.spacing_mut().scroll = crate::component::scroll::style(&theme);
        page(ui, &theme, &keymap);
    });
    harness.run_ok();
    harness
}

#[test]
fn every_control_of_the_fields_page_is_reached_by_tab() {
    let mut harness = harness(Theme::new(Mode::Light, TextScale::default()), true);
    assert_eq!(keyboard_walk(&mut harness), Vec::<String>::new(), "the controls Tab never reaches");
}

#[test]
fn the_fields_page_rests() {
    for reduce in [true, false] {
        let mut harness = harness(Theme::new(Mode::Light, TextScale::default()), reduce);
        assert!(rests(&mut harness, 8), "the page keeps asking for frames with motion reduced: {reduce}");
    }
}

#[test]
fn every_control_of_the_fields_page_has_a_name_and_fits_the_page() {
    let harness = harness(Theme::new(Mode::Dark, TextScale::default()), true);
    let whole = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), size());
    for role in [Role::Button, Role::MenuItem, Role::MenuItemRadio, Role::TextInput, Role::PasswordInput, Role::MultilineTextInput, Role::ListItem] {
        for node in harness.query_all_by_role(role) {
            assert!(node.accesskit_node().label().is_some_and(|label| !label.is_empty()), "{role:?} without a name");
            assert!(whole.contains_rect(node.rect()), "{:?} {:?} leaves the page", node.accesskit_node().label(), node.rect());
        }
    }
}

#[test]
fn the_fields_boards_say_what_the_design_boards_say_on_their_ground() {
    for mode in [Mode::Light, Mode::Dark] {
        let theme = Theme::new(mode, TextScale::default());
        let harness = harness(theme, true);
        // One of each board's own words, once, standing on `bg`.
        for name in ["Port", "Address", "CalDAV address, for calendars", "New password", "Track this parcel", "Every month", "Mark as read"] {
            assert_eq!(painted(&harness, name).ground, Some(theme.palette.bg), "{mode:?} {name}");
        }
        harness.get_by_role_and_label(Role::Switch, "Notify me when it comes back");
        // On the typed time's board and on the picker's typed-invalid.
        assert_eq!(harness.query_all_by_label("Not a time yet: try thu 9, 12 nov or in 3 days").count(), 2, "{mode:?}");
        harness.get_by_label("That time has passed: type one still ahead");
        harness.get_by_label("It counts as soon as it reads as a time: no Enter needed.");
        harness.get_by_label("teodor@ is not an address");
        assert_eq!(harness.query_all_by_role(Role::ListItem).filter(|chip| chip.accesskit_node().label().as_deref() == Some("Studio, everyone")).count(), 1);
    }
}
