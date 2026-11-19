// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The page of the status line's sets walked by the keyboard, left alone, and read for its states.

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::{page, size};
use crate::component::gallery::keymap;
use crate::component::tests::{builder, keyboard_walk, rests};
use crate::motion;
use crate::theme::{Mode, TextScale, Theme};

fn harness(mode: Mode) -> Harness<'static> {
    let theme = Theme::new(mode, TextScale::default());
    let keymap = keymap();
    let mut harness = builder().with_size(size()).build_ui(move |ui| {
        motion::set_reduced(ui.ctx(), true);
        page(ui, &theme, &keymap);
    });
    harness.run_ok();
    harness
}

#[test]
fn every_state_of_both_sets_is_on_the_page_and_tab_reaches_each_press() {
    for mode in [Mode::Light, Mode::Dark] {
        let mut harness = harness(mode);
        for words in
            ["2 accounts", "outbox 1 · goes at 14:05", "offline since 14:10", "Studio: sign-in refused", "next: Design review 15:00 · in 12 min"]
        {
            harness.get_by_label(words);
        }
        assert_eq!(harness.query_all_by_label("synced 14:02").count(), 3, "ok, hover and focus");
        for words in [
            "done · Draft logo for the bakery",
            "sending · Re: Thursday workshop",
            "Link copied: https://example.org/open-day",
            "Studio: sign-in refused, mail is not being fetched",
            "sent 14:02 · Re: Thursday workshop",
        ] {
            assert_eq!(harness.get_by_label_contains(words).rect().width(), 560.0, "{words}");
        }
        let pressed = |name: &str| {
            harness.query_all_by_role(egui::accesskit::Role::Button).filter(|node| node.accesskit_node().label().as_deref() == Some(name)).count()
        };
        assert_eq!((pressed("undo"), pressed("dismiss"), pressed("synced 14:02")), (2, 1, 2), "the presses: undo twice, dismiss, hover and focus");
        let page = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), size());
        for node in harness.query_all_by_role(egui::accesskit::Role::Button) {
            assert!(page.contains_rect(node.rect()), "{:?} leaves the page", node.accesskit_node().label());
        }
        let missed = keyboard_walk(&mut harness);
        assert!(missed.is_empty(), "the controls Tab never reaches: {missed:?}");
        assert!(rests(&mut harness, 8), "{mode:?}");
    }
}
