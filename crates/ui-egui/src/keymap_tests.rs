// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::{Context, Event, Key, KeyboardShortcut, Modifiers, RawInput};

use super::{CALENDAR, Captured, DELETE, DONE, FIND, Keymap, MAIL, PALETTE, READ, SETTINGS, SHORTCUTS, UNDO, actions, captured, read, written};

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

#[test]
fn mail_is_command_1_and_the_settings_command_comma_by_default() {
    let keymap = Keymap::default();
    assert_eq!(keymap.shortcut(MAIL), Some(KeyboardShortcut::new(Modifiers::COMMAND, Key::Num1)));
    assert_eq!(keymap.shortcut(SETTINGS), Some(KeyboardShortcut::new(Modifiers::COMMAND, Key::Comma)));
}

#[test]
fn every_action_has_a_name_an_id_once_and_the_palette_is_command_k() {
    let actions = actions();
    let mut ids: Vec<_> = actions.iter().map(|action| action.id).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), actions.len(), "one entry per id");
    for action in &actions {
        assert!(!action.name.is_empty(), "{}", action.id);
        assert_eq!(super::action(action.id).map(|found| found.id), Some(action.id));
    }
    for id in [FIND, MAIL, SETTINGS, DONE, READ, DELETE, UNDO, PALETTE, CALENDAR, SHORTCUTS] {
        assert!(super::action(id).is_some(), "{id} is in the catalogue");
    }
    assert!(super::action("mail.nothing").is_none());
    let keymap = Keymap::default();
    assert_eq!(keymap.shortcut(PALETTE), Some(CTRL_K));
    assert_eq!(keymap.shortcut(CALENDAR), Some(KeyboardShortcut::new(Modifiers::COMMAND, Key::Num2)));
    assert_eq!(keymap.action_of(CTRL_K), Some(PALETTE));
    assert_eq!(keymap.action_of(KeyboardShortcut::new(Modifiers::ALT, Key::Q)), None);
}

#[test]
fn a_key_is_written_with_its_modifiers_and_read_back() {
    let shift_alt = Modifiers { alt: true, shift: true, ..Modifiers::COMMAND };
    for shortcut in [CTRL_K, KeyboardShortcut::new(Modifiers::NONE, Key::Delete), KeyboardShortcut::new(shift_alt, Key::Num1)] {
        let text = written(shortcut);
        assert_eq!(read(&text), Some(shortcut), "{text}");
    }
    assert_eq!(written(CTRL_K), "Command+K");
    assert_eq!(written(KeyboardShortcut::new(Modifiers::CTRL, Key::K)), "Ctrl+K");
    assert_eq!(written(KeyboardShortcut::new(Modifiers::MAC_CMD, Key::K)), "MacCmd+K");
    assert_eq!(read(" command + shift + f "), Some(KeyboardShortcut::new(Modifiers { shift: true, ..Modifiers::COMMAND }, Key::F)));
    for key in Key::ALL {
        let shortcut = KeyboardShortcut::new(Modifiers::NONE, *key);
        assert_eq!(read(&written(shortcut)), Some(shortcut), "{key:?}");
    }
}

#[test]
fn hostile_key_texts_read_as_nothing() {
    for text in ["", "+", "Command+", "Command", "Hyper+K", "K+Command", "Command+K+L", "\0", "Ctrl+\u{202e}K", &"Shift+".repeat(10_000)] {
        assert_eq!(read(text), None, "{text:?}");
    }
    assert_eq!(read(&format!("{}K", "Shift+".repeat(3))), Some(KeyboardShortcut::new(Modifiers::SHIFT, Key::K)), "a modifier twice is once");
    for text in ["Command+Ctrl+K", "MacCmd+Command+K"] {
        assert_eq!(read(text), Some(CTRL_K), "Command stands for Ctrl and ⌘: {text}");
    }
}

#[test]
fn the_keymap_is_kept_as_every_action_s_key_and_read_back_over_the_defaults() {
    let mut keymap = Keymap::default();
    keymap.bind(FIND, CTRL_K);
    keymap.unbind(DONE);
    let value = keymap.to_value();
    assert_eq!(value["keys"][FIND], "Command+K");
    assert_eq!(value["keys"][PALETTE], serde_json::Value::Null, "the palette lost its key to Find");
    assert_eq!(value["keys"].as_object().map(serde_json::Map::len), Some(actions().len()));
    assert_eq!(Keymap::from_value(&value), keymap);
    // An action missing from the file keeps its default; unknown ids, bad keys and wrong types are dropped.
    let partial = serde_json::json!({ "keys": { FIND: "Command+J", "mail.nothing": "Command+L", MAIL: 7, READ: "Hyper+R" } });
    let read = Keymap::from_value(&partial);
    assert_eq!(read.shortcut(FIND), Some(KeyboardShortcut::new(Modifiers::COMMAND, Key::J)));
    assert_eq!(read.shortcut("mail.nothing"), None);
    assert_eq!(read.shortcut(MAIL), Keymap::default().shortcut(MAIL));
    assert_eq!(read.shortcut(READ), Keymap::default().shortcut(READ));
    for hostile in [serde_json::json!(null), serde_json::json!([1, 2]), serde_json::json!({ "keys": "x" })] {
        assert_eq!(Keymap::from_value(&hostile), Keymap::default());
    }
}

#[test]
fn a_captured_key_is_taken_from_the_input_and_escape_cancels() {
    let ctx = Context::default();
    let key = |key, modifiers| Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers };
    let linux_ctrl = Modifiers { ctrl: true, command: true, ..Modifiers::NONE };
    let frames = [
        (vec![key(Key::J, linux_ctrl)], Some(Captured::Key(KeyboardShortcut::new(Modifiers::COMMAND, Key::J)))),
        // J is still down: egui counts this press as a repeat.
        (vec![key(Key::J, Modifiers::NONE)], None),
        (vec![key(Key::Escape, Modifiers::NONE)], Some(Captured::Cancel)),
        (vec![Event::Key { key: Key::J, physical_key: None, pressed: false, repeat: false, modifiers: Modifiers::NONE }], None),
        (vec![Event::Text("j".to_owned())], None),
    ];
    for (events, wanted) in frames {
        let mut seen = None;
        let mut left = true;
        let mut output = ctx.run_ui(RawInput { events, ..RawInput::default() }, |ui| {
            seen = captured(ui.ctx());
            left = ui.input(|input| input.key_pressed(Key::J) || input.key_pressed(Key::Escape));
        });
        output.textures_delta.clear();
        assert_eq!(seen, wanted);
        if wanted.is_some() {
            assert!(!left, "the press is taken");
        }
    }
}
