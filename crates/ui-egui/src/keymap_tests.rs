// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::{Context, Event, Key, KeyboardShortcut, Modifiers, RawInput};

use super::{FIND, Keymap};

const CTRL_K: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::K);

#[test]
fn find_is_command_f_by_default() {
    let keymap = Keymap::default();
    assert_eq!(keymap.shortcut(FIND), Some(KeyboardShortcut::new(Modifiers::COMMAND, Key::F)));
    assert_eq!(keymap.shortcut("mail.nothing"), None);
    assert!(Keymap::empty().shortcut(FIND).is_none());
}

#[test]
fn binding_a_key_replaces_the_old_one_and_takes_it_from_any_other_action() {
    let mut keymap = Keymap::default();
    keymap.bind(FIND, CTRL_K);
    assert_eq!(keymap.shortcut(FIND), Some(CTRL_K));
    keymap.bind("mail.done", CTRL_K);
    assert_eq!(keymap.shortcut("mail.done"), Some(CTRL_K));
    assert_eq!(keymap.shortcut(FIND), None, "one key runs one action");
    keymap.unbind("mail.done");
    assert_eq!(keymap.shortcut("mail.done"), None);
}

#[test]
fn the_chip_reads_the_key_as_the_platform_writes_it() {
    let ctx = Context::default();
    let mut keymap = Keymap::default();
    assert_eq!(keymap.chip(&ctx, FIND).as_deref(), Some("Ctrl+F"));
    keymap.bind(FIND, CTRL_K);
    assert_eq!(keymap.chip(&ctx, FIND).as_deref(), Some("Ctrl+K"));
    assert_eq!(keymap.chip(&ctx, "mail.nothing"), None);
}

#[test]
fn pressed_takes_the_key_of_the_action_from_the_input() {
    let ctx = Context::default();
    let keymap = Keymap::default();
    let press = Event::Key { key: Key::F, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::COMMAND };
    let mut seen = (false, false);
    let mut output = ctx.run_ui(RawInput { events: vec![press], ..RawInput::default() }, |ui| {
        seen = (keymap.pressed(ui.ctx(), "mail.nothing"), keymap.pressed(ui.ctx(), FIND));
        assert!(!keymap.pressed(ui.ctx(), FIND), "taken once");
    });
    output.textures_delta.clear();
    assert_eq!(seen, (false, true));
}

#[test]
fn hostile_action_ids_are_only_strings() {
    let mut keymap = Keymap::default();
    for id in ["", "\0", "\u{202e}find", &"x".repeat(10_000)] {
        keymap.bind(id, CTRL_K);
        assert_eq!(keymap.shortcut(id), Some(CTRL_K));
        keymap.unbind(id);
    }
    assert_eq!(keymap.shortcut(FIND), Some(KeyboardShortcut::new(Modifiers::COMMAND, Key::F)));
}
