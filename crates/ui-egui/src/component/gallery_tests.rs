// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::component::tests::builder;
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::{keymap, page, size};
use crate::motion;
use crate::sample;
use crate::theme::{Mode, TextScale, Theme};

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
fn the_gallery_shows_every_component_in_both_modes() {
    {
        for mode in [Mode::Light, Mode::Dark] {
            let harness = harness(Theme::new(mode, TextScale::default()));
            for name in ["Newsletters", "Screener: 3 new senders", "Unpin", "Unread", "People", "Marked as done", "Move to a folder"] {
                harness.get_by_label(name);
            }
            // The archive button itself: its two tooltips on the tooltip board carry the name too.
            harness.get_by_role_and_label(Role::Button, "Archive");
            assert!(harness.query_all_by_label("Done").count() >= 2, "the button and the hover action");
            for name in ["Snooze until", "Later today, 17:30", "Tomorrow morning, Sat 08:00", "Thursday 12 November, 09:00"] {
                harness.get_by_label(name);
            }
            assert!(harness.query_by_label_contains("This morning").is_none(), "a preset behind is hidden");
            harness.get_by_role_and_label(Role::Switch, "Notify me when it comes back");
            harness.get_by_label("Ctrl F");
            harness.get_by_label("Ctrl Z");
            for (name, _) in sample::people() {
                assert!(harness.get_all_by_label(name).any(|node| node.accesskit_node().role() == Role::Image), "{name}");
            }
        }
    }
}

#[test]
fn every_control_of_the_gallery_has_a_name_and_fits_the_page() {
    let harness = harness(Theme::new(Mode::Dark, TextScale::default()));
    let page = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), size());
    for role in [Role::Button, Role::MenuItem, Role::Image, Role::Switch, Role::TextInput] {
        for node in harness.query_all_by_role(role) {
            assert!(node.accesskit_node().label().is_some_and(|label| !label.is_empty()), "{role:?} without a name");
            assert!(page.contains_rect(node.rect()), "{:?} {:?} leaves the page", node.accesskit_node().label(), node.rect());
        }
    }
}

#[test]
fn the_gallery_keymap_has_a_key_for_every_action_it_shows() {
    let keymap = keymap();
    for action in
        ["mail.done", "mail.archive", "mail.delete", "mail.snooze", "mail.pin", "mail.move", "mail.read", "mail.send", "ui.undo", crate::keymap::FIND]
    {
        assert!(keymap.shortcut(action).is_some(), "{action}");
    }
}

#[test]
fn the_boards_say_what_the_design_boards_say_on_their_ground() {
    for mode in [Mode::Light, Mode::Dark] {
        let theme = Theme::new(mode, TextScale::default());
        let harness = harness(theme);
        // The boards stand on `bg`, as the design's do.
        for name in ["Draft", "New sender"] {
            assert_eq!(crate::component::tests::painted(&harness, name).ground, Some(theme.palette.bg), "{mode:?} {name}");
        }
        // The tooltip board's words.
        assert_eq!(harness.query_all_by_role_and_label(Role::Tooltip, "Archive").count(), 2, "{mode:?} default and with-key");
        harness
            .get_by_role_and_label(Role::Tooltip, "Snooze is not there for a message that is still being sent. Wait until it has left the outbox.");
        assert_eq!(harness.query_all_by_label("Empfangsbestätigung für diese Nachricht anfordern").count(), 2, "{mode:?} the long labels");
        // The segmented board's long label: its words, so its width (95 | 160 | 63 and four lines) and its cut.
        let long = harness.get_by_label("Gruppiert / Zusammenfassung des Posteingangs / Liste").rect();
        assert!((long.width() - 322.0).abs() < 0.01 && (long.height() - 32.0).abs() < 0.01, "{mode:?} {long:?}");
        // Each unavailable cell shows its reason under the control, 36 below its top, as the board does.
        for (name, reason) in [
            ("Pin", "Not yet: choose a message first"),
            ("Send", "Not yet: add a recipient first"),
            ("Reply all", "Not yet: the message has one recipient"),
            ("Delete", "Not yet: choose a message first"),
            ("Discard", "Not yet: nothing is written"),
        ] {
            let tip = harness
                .query_all_by_role_and_label(Role::Tooltip, name)
                .next()
                .unwrap_or_else(|| panic!("{mode:?} {name}: no reason tooltip"))
                .rect();
            let control = harness
                .query_all_by_role_and_label(Role::Button, name)
                .filter(|node| node.accesskit_node().is_disabled())
                .find(|node| (node.rect().top() + 36.0 - tip.top()).abs() < 0.5 && (node.rect().left() - tip.left()).abs() < 0.5);
            assert!(control.is_some(), "{mode:?} {name}: the tooltip at {tip:?} is not 36 under an unavailable {name}");
            assert!(harness.query_all_by_label(reason).count() >= 1, "{mode:?} {reason}");
        }
    }
}
