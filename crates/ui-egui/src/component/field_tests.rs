// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::accesskit::Role;
use egui_kittest::kittest::Queryable as _;

use super::{margin, search};
use crate::component::tests::{builder, drew_icon, theme};
use crate::iconset::{IconId, Style};

#[test]
fn the_search_pill_shows_a_magnifier_and_takes_typing() {
    let theme = theme();
    let mut harness = builder().build_ui_state(
        move |ui, text: &mut String| {
            search(ui, &theme, egui::Id::unique("search"), text, "Search", 200.0);
        },
        String::new(),
    );
    harness.run();
    assert!(drew_icon(harness.output(), IconId::Search, Style::Regular));
    harness.get_by_role(Role::TextInput).focus();
    harness.run();
    harness.get_by_role(Role::TextInput).type_text("odile");
    harness.run();
    assert_eq!(harness.state(), "odile");
}

#[test]
fn a_form_field_has_room_around_its_text() {
    let m = margin(&theme());
    assert!(m.top > 2 && m.left >= m.top, "{m:?}");
}
