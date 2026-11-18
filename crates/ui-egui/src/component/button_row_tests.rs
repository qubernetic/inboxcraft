// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::vec2;

use super::{ButtonLines, button_lines};
use crate::component::button::TextButton;
use crate::component::tests::builder;
use crate::keymap::Keymap;
use crate::theme::{Mode, TextScale, Theme};

/// The lines three answers stand on in `room` points at `scale`.
fn lines_in(room: f32, scale: TextScale) -> ButtonLines {
    let theme = Theme::new(Mode::Dark, scale);
    let mut harness = builder().with_size(vec2(400.0, 100.0)).build_ui_state(
        move |ui, placed: &mut Option<ButtonLines>| {
            let buttons = [TextButton::new("Going").primary(), TextButton::new("Maybe").secondary(), TextButton::new("No").secondary()];
            *placed = Some(button_lines(ui, &theme, &Keymap::default(), &buttons, room, 8.0));
        },
        None,
    );
    harness.run();
    harness.state().clone().unwrap()
}

#[test]
fn buttons_stand_on_one_line_where_they_fit_and_wrap_in_order_where_not() {
    let side = 32.0;
    let wide = lines_in(600.0, TextScale::default());
    assert_eq!((wide.lines.clone(), wide.height), (vec![0, 0, 0], side));
    let narrow = lines_in(120.0, TextScale::default());
    assert_eq!((narrow.lines.clone(), narrow.height), (vec![0, 1, 1], side * 2.0 + 8.0), "Going alone, then Maybe and No");
    let tiny = lines_in(1.0, TextScale::ALL[4]);
    assert_eq!(tiny.lines, vec![0, 1, 2], "one a line in a room narrower than any");
    for room in [0.0, -5.0, f32::NAN, f32::INFINITY] {
        let placed = lines_in(room, TextScale::default());
        assert!(placed.height.is_finite() && placed.height >= side, "{room}: {placed:?}");
    }
}
