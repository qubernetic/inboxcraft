// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::{Id, Rect, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

use super::{Action, Shown, actions};
use crate::component::tests::{builder, theme};
use crate::iconset::IconId;
use crate::keymap::Keymap;

const ROW: Rect = Rect { min: pos2(0.0, 40.0), max: pos2(500.0, 80.0) };

const ACTIONS: [Action<'static>; 3] = [
    Action { icon: IconId::Done, name: "Done", action: Some("mail.done") },
    Action { icon: IconId::Notify, name: "Mark as read", action: None },
    Action { icon: IconId::Delete, name: "Delete", action: None },
];

/// The group over a row, shown when the state's flag says so; the state keeps what the group drew.
fn harness(shown: bool) -> Harness<'static, (bool, Option<Shown>)> {
    let theme = theme();
    let mut harness = builder().build_ui_state(
        move |ui, (shown, drawn): &mut (bool, Option<Shown>)| {
            *drawn = Some(actions(ui, &theme, &Keymap::default(), Id::unique("row"), ROW, *shown, &ACTIONS));
        },
        (shown, None),
    );
    harness.run();
    harness
}

#[test]
fn a_hidden_group_draws_nothing_and_leaves_the_row_its_width() {
    let harness = harness(false);
    assert!(harness.query_by_label("Done").is_none());
    assert_eq!(harness.state().1, Some(Shown { clicked: None, left: ROW.right(), openness: 0.0 }));
}

#[test]
fn a_shown_group_offers_its_actions_at_the_row_s_right_end() {
    let harness = harness(true);
    let shown = harness.state().1.unwrap();
    assert_eq!(shown.openness, 1.0);
    assert!(shown.left > ROW.center().x && shown.left < ROW.right(), "{shown:?}");
    for name in ["Done", "Mark as read", "Delete"] {
        let rect = harness.get_by_label(name).rect();
        assert!(rect.left() >= shown.left && rect.right() <= ROW.right() && ROW.contains(rect.center()), "{name} {rect:?}");
    }
}

#[test]
fn the_group_fades_in_and_out_over_the_hover_token() {
    let mut harness = harness(false);
    harness.state_mut().0 = true;
    harness.step();
    let first = harness.state().1.unwrap().openness;
    assert!(first > 0.0 && first < 1.0, "{first}");
    harness.run();
    assert_eq!(harness.state().1.unwrap().openness, 1.0);
    harness.state_mut().0 = false;
    harness.step();
    let fading = harness.state().1.unwrap().openness;
    assert!(fading > 0.0 && fading < 1.0, "{fading}");
    harness.run();
    assert!(harness.query_by_label("Done").is_none());
}

#[test]
fn a_press_on_an_action_names_it() {
    let theme = theme();
    let mut harness = builder().build_ui_state(
        move |ui, clicked: &mut Vec<usize>| {
            let shown = actions(ui, &theme, &Keymap::default(), Id::unique("row"), ROW, true, &ACTIONS);
            clicked.extend(shown.clicked);
        },
        Vec::new(),
    );
    harness.run();
    harness.get_by_label("Mark as read").click();
    harness.run();
    assert_eq!(harness.state(), &[1]);
}
