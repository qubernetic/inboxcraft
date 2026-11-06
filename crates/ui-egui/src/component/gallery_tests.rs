// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::component::tests::builder;
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::{keymap, page, size};
use crate::motion;
use crate::sample;
use crate::theme::{Mode, PRESETS, Theme};

fn harness(theme: Theme) -> Harness<'static> {
    let keymap = keymap();
    let mut harness = builder().with_size(size()).build_ui(move |ui| {
        motion::set_reduced(ui.ctx(), true);
        page(ui, &theme, &keymap);
    });
    harness.run();
    harness
}

#[test]
fn the_gallery_shows_every_component_in_every_preset_and_mode() {
    for preset in &PRESETS {
        for mode in [Mode::Light, Mode::Dark] {
            let harness = harness(Theme::new(mode, preset));
            for name in ["Newsletters", "Screener: 3 new senders", "Archive", "Unpin", "Unread", "People", "Marked as done", "Move to a folder"] {
                harness.get_by_label(name);
            }
            assert!(harness.query_all_by_label("Done").count() >= 2, "the button and the hover action");
            harness.get_by_label("Ctrl+F");
            harness.get_by_label("Ctrl+Z");
            for (name, _) in sample::people() {
                assert!(harness.get_all_by_label(name).any(|node| node.accesskit_node().role() == Role::Image), "{name}");
            }
        }
    }
}

#[test]
fn every_control_of_the_gallery_has_a_name_and_fits_the_page() {
    let harness = harness(Theme::new(Mode::Dark, &PRESETS[0]));
    let page = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), size());
    for role in [Role::Button, Role::MenuItem, Role::Image] {
        for node in harness.query_all_by_role(role) {
            assert!(node.accesskit_node().label().is_some_and(|label| !label.is_empty()), "{role:?} without a name");
            assert!(page.contains_rect(node.rect()), "{:?} {:?} leaves the page", node.accesskit_node().label(), node.rect());
        }
    }
}

#[test]
fn the_gallery_keymap_has_a_key_for_every_action_it_shows() {
    let keymap = keymap();
    for action in ["mail.done", "mail.archive", "mail.delete", "mail.snooze", "mail.pin", "mail.move", "mail.read", "ui.undo", crate::keymap::FIND] {
        assert!(keymap.shortcut(action).is_some(), "{action}");
    }
}
