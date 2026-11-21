// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The page of the reader's sets walked by the keyboard, left alone, and read for its names and its ground.

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::{page, size};
use crate::component::gallery::keymap;
use crate::component::tests::{builder, keyboard_walk, rests};
use crate::motion;
use crate::theme::{Mode, TextScale, Theme};

/// The boards the page holds, by title.
pub(crate) const BOARDS: [&str; 9] =
    ["Why line", "Content bar", "Thread row", "Quote fold", "Attachment", "Hairline table", "Invitation card", "Picture box", "Fitted message"];

/// The page in `mode`, motion reduced or not.
fn harness(mode: Mode, reduce: bool) -> Harness<'static> {
    let theme = Theme::new(mode, TextScale::default());
    let keymap = keymap();
    let mut harness = builder().with_size(size()).build_ui(move |ui| {
        motion::set_reduced(ui.ctx(), reduce);
        page(ui, &theme, &keymap);
    });
    harness.run_ok();
    harness
}

#[test]
fn every_control_of_the_reader_page_is_reached_by_tab() {
    let mut harness = harness(Mode::Light, true);
    let missed = keyboard_walk(&mut harness);
    assert!(missed.is_empty(), "the controls Tab never reaches: {missed:?}");
}

#[test]
fn the_reader_page_rests() {
    for reduce in [true, false] {
        let mut harness = harness(Mode::Light, reduce);
        assert!(rests(&mut harness, 8), "the page keeps asking for frames with motion reduced: {reduce}");
    }
}

#[test]
fn every_control_of_the_reader_page_has_a_name_and_fits_the_page() {
    let harness = harness(Mode::Dark, true);
    let page = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), size());
    for role in [Role::Button, Role::Label, Role::Group, Role::RadioGroup] {
        for node in harness.query_all_by_role(role) {
            let label = node.accesskit_node().label();
            if role == Role::Button {
                assert!(label.as_ref().is_some_and(|label| !label.is_empty()), "{role:?} without a name");
            }
            assert!(page.contains_rect(node.rect()), "{label:?} {:?} leaves the page", node.rect());
        }
    }
}

#[test]
fn the_reader_page_holds_every_board() {
    for mode in [Mode::Light, Mode::Dark] {
        let harness = harness(mode, true);
        for title in BOARDS {
            assert!(harness.query_all_by_label(title).count() >= 1, "{mode:?}: the {title} board");
        }
    }
}

#[test]
fn the_fitted_message_board_shows_its_quote_folded_and_open() {
    let harness = harness(Mode::Dark, true);
    harness.get_by_role_and_label(Role::Button, "Show 3 quoted lines");
    // The quote fold's own board has an open fold too.
    assert_eq!(harness.query_all_by_role_and_label(Role::Button, "Hide quoted text").count(), 2);
    assert_eq!(harness.query_all_by_role_and_label(Role::Link, "the full rota").count(), 2);
    harness.get_by_label("The shed key is under the pot.");
}

#[test]
fn the_invitation_board_shows_the_card_while_its_answer_changes() {
    // The design's `invite-card/state=changing`: Going (primary), Maybe, No and Keep my answer, 8 apart, in a cell of
    // its own after the request.
    let harness = harness(Mode::Dark, true);
    let changing = harness.get_by_label("changing").rect();
    let request = harness.get_by_label("request").rect();
    assert!(changing.top() > request.top(), "the changing cell stands after the request: {request:?} {changing:?}");
    let keep: Vec<_> = harness.query_all_by_role_and_label(Role::Button, "Keep my answer").map(|node| node.rect()).collect();
    let [keep] = keep[..] else { panic!("one Keep my answer on the page: {keep:?}") };
    assert!(keep.top() > changing.top(), "Keep my answer is in the changing cell: {changing:?} {keep:?}");
    let no = harness
        .query_all_by_role_and_label(Role::Button, "No")
        .map(|node| node.rect())
        .find(|rect| (rect.center().y - keep.center().y).abs() <= 0.5)
        .unwrap();
    assert!((keep.left() - no.right() - 8.0).abs() <= 0.5, "8 after No: {no:?} {keep:?}");
}
