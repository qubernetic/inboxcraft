// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A row of LAST COMMANDS recalls a command and is not a control of it: it never makes an action that only a
//! coming-soon control offers look built (#397b).

use crate::component::button::TextButton;
use crate::component::soon::{blocked, soon};
use crate::component::tests::{builder, stage};
use crate::keymap::{self, Keymap};

use super::{Listed, command_row};

#[test]
fn a_last_commands_row_leaves_a_coming_soon_action_blocked() {
    let theme = crate::theme::Theme::new(crate::theme::Mode::Light, crate::theme::TextScale::default());
    let keymap = Keymap::default();
    let row = crate::palette::rows("").into_iter().find(|row| row.id == keymap::ARCHIVE).unwrap();
    let listed = Listed { row, refused: None };
    let mut harness = builder().build_ui_state(
        move |ui, (): &mut ()| {
            stage(ui, theme.palette.surface, |ui| {
                soon(ui, &theme, |ui| TextButton::new("Archive later").action(keymap::ARCHIVE).show(ui, &theme, &keymap));
                let _ = command_row(ui, &theme, &keymap, &listed, false, true);
            });
        },
        (),
    );
    harness.run();
    assert!(blocked(&harness.ctx, keymap::ARCHIVE), "only the coming-soon button offers it");
}
