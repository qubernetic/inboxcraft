// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::accesskit::{Role, Toggled};
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::pill;
use crate::component::tests::{builder, theme};

#[test]
fn a_pill_is_a_selectable_button_with_its_word() {
    let theme = theme();
    let mut harness = builder().build_ui_state(
        move |ui, chosen: &mut usize| {
            for (index, word) in ["All", "Unread", "Flagged"].into_iter().enumerate() {
                if pill(ui, &theme, word, *chosen == index).clicked() {
                    *chosen = index;
                }
            }
        },
        1,
    );
    harness.run();
    let unread = harness.get_by_label("Unread");
    assert_eq!(unread.accesskit_node().role(), Role::Button);
    assert_eq!(unread.accesskit_node().toggled(), Some(Toggled::True));
    harness.get_by_label("Flagged").click();
    harness.run();
    assert_eq!(*harness.state(), 2);
    assert_eq!(harness.get_by_label("Flagged").accesskit_node().toggled(), Some(Toggled::True));
}

#[test]
fn a_pill_is_as_wide_as_its_word_and_round() {
    let theme = theme();
    let mut harness = builder().build_ui(move |ui| {
        pill(ui, &theme, "A", false);
        pill(ui, &theme, "A much longer word", false);
    });
    harness.run();
    let (short, long) = (harness.get_by_label("A").rect(), harness.get_by_label("A much longer word").rect());
    assert!(long.width() > short.width() * 2.0);
    assert_eq!(short.height(), long.height());
    assert!(short.width() >= short.height() * 0.9, "{short:?}");
}
