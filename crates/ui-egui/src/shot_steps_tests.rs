// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The script steps of `ui_shot`: what a line may ask, what a junk line gets, and what a step does to the window.

use std::cell::Cell;

use egui::{Key, KeyboardShortcut, Modifiers, Rect, pos2, vec2};
use inboxcraft_time::{FakeClock, Timestamp};

use super::{
    Do, MAX_FRAMES, MAX_LINE, MAX_NAME, MAX_NTH, MAX_SCRIPT, MAX_SHOTS, MAX_STEPS, MAX_TIMES, Picture, Pointer, Run, SHOT_TIME, out_path,
    parse_script, render_steps, script_text, shoot,
};
use crate::keymap;
use crate::mailbox::Mailbox;
use crate::theme::Mode;
use crate::{InboxApp, sample};

/// The accessible name of Teodor's row in the sample's inbox.
const ROW: &str = "Teodor Lisk, Photos from the coast walk, 04:00, flagged";

fn picture(line: &str) -> Picture {
    shoot(&parse_script(line).unwrap().remove(0)).unwrap()
}

fn refusal(line: &str) -> String {
    match parse_script(line) {
        Ok(mut shots) => shoot(&shots.remove(0)).unwrap_err().to_string(),
        Err(error) => error.to_string(),
    }
}

fn line(steps: &str) -> String {
    format!(r#"{{"out": "a.png", "mode": "light", "steps": {steps}}}"#)
}

#[test]
fn each_step_kind_is_read() {
    let shots =
        parse_script(&line(r#"[{"key": "Command+K"}, {"tab": 3}, {"shift_tab": 1}, {"focus": "Search mail"}, {"press": "Snooze", "nth": 2}]"#))
            .unwrap();
    let read: Vec<Do> = shots[0].steps.as_deref().unwrap().iter().map(|step| step.read().unwrap()).collect();
    assert_eq!(
        read,
        vec![
            Do::Key(KeyboardShortcut::new(Modifiers::COMMAND, Key::K)),
            Do::Tab { back: false, times: 3 },
            Do::Tab { back: true, times: 1 },
            Do::Focus { name: "Search mail".to_owned(), nth: None },
            Do::Press { name: "Snooze".to_owned(), nth: Some(2) },
        ]
    );
}

#[test]
fn a_step_the_script_cannot_take_is_refused_naming_its_line() {
    let long = "x".repeat(MAX_NAME + 1);
    let many = format!("[{}]", vec![r#"{"tab": 1}"#; MAX_STEPS + 1].join(", "));
    for (steps, says) in [
        (r#"[{"jump": 1}]"#.to_owned(), "jump"),
        (r#"[{"tab": 1, "key": "A"}]"#.to_owned(), "one of"),
        (r"[{}]".to_owned(), "one of"),
        (r#"[{"tab": 0}]"#.to_owned(), "tab 0"),
        (format!(r#"[{{"tab": {}}}]"#, MAX_TIMES + 1), "is not 1 to"),
        (r#"[{"shift_tab": -1}]"#.to_owned(), "-1"),
        (r#"[{"tab": 1e30}]"#.to_owned(), "floating point"),
        (r#"[{"tab": 1, "nth": 1}]"#.to_owned(), "nth goes with"),
        (r#"[{"focus": "Search mail", "nth": 0}]"#.to_owned(), "nth 0"),
        (format!(r#"[{{"press": "Snooze", "nth": {}}}]"#, MAX_NTH + 1), "is not 1 to"),
        (r#"[{"press": ""}]"#.to_owned(), "a name of 1 to"),
        (format!(r#"[{{"focus": "{long}"}}]"#), "a name of 1 to"),
        (r#"[{"key": "Ctrl+Nonsense"}]"#.to_owned(), "is not a key"),
        (r#"[{"key": "Hyper+K"}]"#.to_owned(), "is not a key"),
        (r#"[{"key": 7}]"#.to_owned(), "invalid type"),
        (r#"{"tab": 1}"#.to_owned(), "invalid type"),
        (many, "more than"),
    ] {
        let script = format!("# a comment\n\n{}\n", line(&steps));
        let error = parse_script(&script).unwrap_err().to_string();
        assert!(error.contains("line 3") && error.contains(says), "{steps}: {error}");
    }
}

#[test]
fn a_script_too_long_or_too_many_pictures_is_refused() {
    let long = format!(r#"{{"out": "{}", "mode": "light"}}"#, "a".repeat(MAX_LINE));
    assert!(parse_script(&long).unwrap_err().to_string().contains("line 1: longer than"));
    let many = vec![r#"{"out": "a.png", "mode": "light"}"#; MAX_SHOTS + 1].join("\n");
    let error = parse_script(&many).unwrap_err().to_string();
    assert!(error.contains(&format!("line {}: more than {MAX_SHOTS}", MAX_SHOTS + 1)), "{error}");
    assert!(parse_script(&" ".repeat(MAX_SCRIPT + 1)).unwrap_err().to_string().contains("longer than"));
}

#[test]
fn junk_lines_never_panic() {
    let junk = [
        "\u{0}\u{1}\u{2}",
        "{",
        "[]",
        "null",
        r#"{"out": "a.png", "mode": "light", "steps": null}"#,
        r#"{"out": "a.png", "mode": "light", "steps": []}"#,
        r#"{"out": "a.png", "mode": "light", "steps": [null]}"#,
        r#"{"out": "a.png", "mode": "light", "steps": [{"key": "Shift+Shift+Shift"}]}"#,
        r#"{"out": "a.png", "mode": "light", "steps": [{"key": "+"}]}"#,
        "{\"out\": \"a.png\", \"mode\": \"light\", \"steps\": [{\"key\": \"\\u202eK\"}]}",
        "{\"out\": \"a.png\", \"mode\": \"light\", \"steps\": [{\"focus\": \"\\u202e\\u0000\\r\\nRow\"}]}",
        r#"{"out": "a.png", "mode": "light", "steps": [{"press": "é́́́"}]}"#,
        r#"{"out": "a.png", "mode": "light", "steps": [{"press": "Snooze", "nth": 4294967295}]}"#,
        r#"{"out": "a.png", "mode": "light", "steps": [{"press": "Search"}]}"#,
        r#"{"out": "a.png", "mode": "light", "steps": [{"focus": "Search"}]}"#,
        r#"{"out": "a.png", "mode": "light", "pointer": [1, 1], "click": [2, 2], "steps": [{"shift_tab": 64}]}"#,
        r#"{"out": "a.png", "mode": "light", "steps": [{"key": "Escape"}, {"key": "Command+Z"}]}"#,
        r#"{"out": "a.png", "mode": "light", "lists": true, "steps": [{"tab": 1}]}"#,
    ];
    for text in junk {
        if let Ok(shots) = parse_script(text) {
            for shot in shots {
                // An error or a picture; never a panic.
                let _ = shoot(&shot);
            }
        }
    }
}

#[test]
fn a_name_no_control_has_or_several_have_is_an_error_not_a_silent_step() {
    let error = refusal(&line(r#"[{"focus": "Photos from the coast walk"}]"#));
    assert!(error.contains("step 1: no control that takes the focus is named"), "{error}");
    assert!(error.contains(ROW), "the names that hold it are offered: {error}");
    let selected = |steps: &str| format!(r#"{{"out": "a.png", "mode": "light", "step": 2, "steps": {steps}}}"#);
    let error = refusal(&selected(r#"[{"press": "Snooze"}]"#));
    assert!(error.contains("2 controls are named \"Snooze\"; nth says which"), "{error}");
    let error = refusal(&selected(r#"[{"press": "Snooze", "nth": 3}]"#));
    assert!(error.contains("nth 3, but only 2 controls"), "{error}");
    let error = refusal(&line(r#"[{"tab": 1}, {"press": "Nothing at all"}]"#));
    assert!(error.contains("step 2: no enabled control that can be pressed is named"), "{error}");
    let error = refusal(r#"{"out": "a.png", "mode": "light", "lists": true, "steps": [{"tab": 1}]}"#);
    assert!(error.contains("has no window"), "{error}");
}

#[test]
fn a_focus_step_rings_the_named_row_and_draws_the_same_bytes_every_time() {
    let focused = line(&format!(r#"[{{"focus": "{ROW}"}}]"#));
    let first = picture(&focused);
    assert_eq!(first, picture(&focused), "the same script draws the same bytes");
    assert_ne!(first, picture(r#"{"out": "a.png", "mode": "light"}"#), "the focus shows");
}

#[test]
fn tab_and_shift_tab_walk_the_focus_both_ways() {
    let one = picture(&line(r#"[{"tab": 1}]"#));
    assert_eq!(one, picture(&line(r#"[{"tab": 3}, {"shift_tab": 2}]"#)), "three on and two back is one on");
    assert_ne!(one, picture(&line(r#"[{"tab": 2}]"#)));
    assert_ne!(one, picture(r#"{"out": "a.png", "mode": "light"}"#));
}

#[test]
fn a_press_lands_on_a_hover_action_once_it_is_laid_out() {
    let pressed = r#"{"out": "a.png", "mode": "light", "size": [1400, 860], "step": 2, "steps": [{"press": "Snooze", "nth": 2}]}"#;
    let first = picture(pressed);
    assert_eq!(first, picture(pressed), "the same script draws the same bytes");
    assert_ne!(first, picture(r#"{"out": "a.png", "mode": "light", "size": [1400, 860], "step": 2}"#), "the picker opened");
}

#[test]
fn a_step_that_reaches_a_command_only_the_owner_runs_fails_the_picture() {
    for (key, command) in [("D", "mail.done"), ("Delete", "mail.delete")] {
        let error = refusal(&format!(r#"{{"out": "a.png", "mode": "light", "step": 2, "steps": [{{"key": "{key}"}}]}}"#));
        assert!(error.contains(command) && error.contains("safe commands only"), "{key}: {error}");
    }
    // Flag is safe.
    picture(r#"{"out": "a.png", "mode": "light", "step": 2, "steps": [{"key": "S"}]}"#);
}

/// The sample window at the pictures' time, the second row selected.
fn window() -> InboxApp {
    let clock = || Box::new(FakeClock::new(Timestamp::from_unix(SHOT_TIME, 0).unwrap()));
    let mut app = InboxApp::at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    app.set_reduce_motion(true);
    app.mailbox_mut().step(true);
    app.mailbox_mut().step(true);
    app
}

#[test]
fn a_key_goes_through_the_keymap_so_a_rebinding_is_honoured() {
    let delete = [Do::Key(KeyboardShortcut::new(Modifiers::NONE, Key::Delete))];
    let draw = |app: &mut InboxApp| render_steps(app, vec2(1200.0, 760.0), 1.0, Mode::Light, Pointer::default(), &delete);
    let mut app = window();
    assert!(draw(&mut app).unwrap_err().contains("mail.delete"));
    let mut app = window();
    let flagged = app.mailbox().messages().iter().filter(|row| row.flagged).count();
    app.keymap_mut().bind(keymap::FLAG, KeyboardShortcut::new(Modifiers::NONE, Key::Delete));
    draw(&mut app).unwrap();
    assert_ne!(app.mailbox().messages().iter().filter(|row| row.flagged).count(), flagged, "Delete flags now");
    assert_eq!(app.mailbox().refused(), None);
}

/// A bare window of 400 by 300 points with its AccessKit tree, no frame drawn yet.
fn bare() -> Run {
    let run = Run::new(vec2(400.0, 300.0), 1.0, Mode::Light).unwrap();
    run.ctx.enable_accesskit();
    run
}

#[test]
fn a_step_settles_the_window_until_nothing_moves() {
    let mut run = bare();
    let (on, value) = (Cell::new(false), Cell::new(0.0_f32));
    let mut draw = |ui: &mut egui::Ui| value.set(ui.ctx().animate_bool_with_time(egui::Id::unique("slide"), on.get(), 0.5));
    run.frame(Vec::new(), &mut draw).unwrap();
    on.set(true);
    run.frame(Vec::new(), &mut draw).unwrap();
    assert!(value.get() < 1.0, "half a second to go");
    run.settle(&mut draw).unwrap();
    assert!((value.get() - 1.0).abs() < f32::EPSILON, "settled where the motion ends: {}", value.get());
}

/// "Go" at `left` until the pointer comes into `zone`, then at `right`.
fn moving_button(ui: &mut egui::Ui, zone: Rect, pressed: &Cell<bool>) {
    let inside = ui.ctx().pointer_hover_pos().is_some_and(|at| zone.contains(at));
    let at = if inside { Rect::from_min_size(pos2(300.0, 30.0), vec2(60.0, 40.0)) } else { Rect::from_min_size(pos2(20.0, 30.0), vec2(60.0, 40.0)) };
    if ui.put(at, egui::Button::new("Go")).clicked() {
        pressed.set(true);
    }
}

#[test]
fn a_press_aims_again_when_the_pointer_moves_its_target() {
    let mut run = bare();
    let pressed = Cell::new(false);
    // A row that shows its action at the right while the pointer is over it, as a hover action does.
    let mut draw = |ui: &mut egui::Ui| moving_button(ui, Rect::from_min_size(pos2(0.0, 0.0), vec2(400.0, 100.0)), &pressed);
    run.settle(&mut draw).unwrap();
    let mut pointer = None;
    run.step(&Do::Press { name: "Go".to_owned(), nth: None }, &mut pointer, &mut draw).unwrap();
    assert!(pressed.get(), "pressed where it was laid out under the pointer");
    assert_eq!(pointer, Some(pos2(330.0, 50.0)));
}

#[test]
fn a_target_that_flees_the_pointer_is_an_error_after_a_few_tries() {
    let mut run = bare();
    let pressed = Cell::new(false);
    // Over the left place it goes right; over the right place it goes left.
    let mut draw = |ui: &mut egui::Ui| moving_button(ui, Rect::from_min_size(pos2(0.0, 0.0), vec2(200.0, 100.0)), &pressed);
    run.settle(&mut draw).unwrap();
    let error = run.step(&Do::Press { name: "Go".to_owned(), nth: None }, &mut None, &mut draw).unwrap_err();
    assert_eq!(error, "\"Go\" moved each time the pointer came over it");
    assert!(!pressed.get());
}

#[test]
fn an_out_under_out_dir_cannot_leave_it() {
    let dir = std::path::Path::new("/renders");
    assert_eq!(out_path(Some(dir), "docs/images/a.png").unwrap(), dir.join("docs/images/a.png"));
    assert_eq!(out_path(None, "../anywhere.png").unwrap(), std::path::PathBuf::from("../anywhere.png"), "without --out-dir as before");
    for out in ["", "/etc/a.png", "../a.png", "docs/../../a.png", "./a.png"] {
        let error = out_path(Some(dir), out).unwrap_err().to_string();
        assert!(error.contains("stays under --out-dir"), "{out:?}: {error}");
    }
}

#[test]
fn a_script_over_its_cap_is_refused_whole_even_mid_character() {
    let mut bytes = vec![b'#'; MAX_SCRIPT];
    // "é" is two bytes; a read that stops one byte past the cap splits it.
    bytes.push(0xC3);
    assert!(script_text(bytes).unwrap_err().to_string().contains(&format!("longer than {MAX_SCRIPT} bytes")));
    assert!(script_text(vec![0xFF, b'a']).unwrap_err().to_string().contains("not UTF-8"));
    assert_eq!(script_text("# é\n".as_bytes().to_vec()).unwrap(), "# é\n");
}

#[test]
fn a_huge_line_a_million_lines_and_deep_json_are_refused_quickly() {
    let huge = format!(r#"{{"out": "a.png", "mode": "light", "search": "{}"}}"#, "x".repeat(10 * 1024 * 1024));
    assert!(parse_script(&huge).unwrap_err().to_string().contains("longer than"));
    let million = format!("{}{}", "\n".repeat(1_000_000), r#"{"out": "a.png", "mode": "light"}"#);
    assert_eq!(parse_script(&million).unwrap().len(), 1);
    let comments = "#\n".repeat(1_000_000);
    assert!(parse_script(&comments).unwrap().is_empty());
    for depth in [200, 5000] {
        let deep = format!(r#"{{"out": "a.png", "mode": "light", "steps": {}{}}}"#, "[".repeat(depth), "]".repeat(depth));
        assert!(parse_script(&deep).is_err(), "{depth}");
    }
}

#[test]
fn a_picture_draws_at_most_its_frames() {
    let mut run = bare();
    let mut draw = |ui: &mut egui::Ui| {
        ui.label("Nothing moves");
    };
    let tab = Do::Tab { back: false, times: 64 };
    let error = (0..MAX_FRAMES).find_map(|_| run.step(&tab, &mut None, &mut draw).err()).unwrap();
    assert_eq!(error, format!("the picture has drawn its {MAX_FRAMES} frames"));
    assert_eq!(run.frames, MAX_FRAMES);
}

#[test]
fn a_press_aims_only_at_a_control_that_can_be_pressed() {
    let mut run = bare();
    let mut draw = |ui: &mut egui::Ui| {
        let at = Rect::from_min_size(pos2(20.0, 30.0), vec2(60.0, 40.0));
        ui.interact(at, egui::Id::unique("words"), egui::Sense::hover())
            .widget_info(|| egui::WidgetInfo::labeled(egui::accesskit::Role::Image, true, "Go"));
    };
    run.settle(&mut draw).unwrap();
    let error = run.step(&Do::Press { name: "Go".to_owned(), nth: None }, &mut None, &mut draw).unwrap_err();
    assert!(error.starts_with("no enabled control that can be pressed is named \"Go\""), "{error}");
}

#[test]
fn a_click_runs_safe_commands_only() {
    // The reader's Done, the first of the head's acts at its right (under the command line): mail.done is the owner's.
    let error = refusal(r#"{"out": "a.png", "mode": "light", "select": "Photos from the coast walk", "click": [943, 78]}"#);
    assert!(error.contains("the click reached mail.done, which only the owner runs"), "{error}");
}

#[test]
fn a_shot_can_ask_the_owner_and_shows_the_question() {
    let ics = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Example//Export//EN\r\nBEGIN:VEVENT\r\nUID:shot-1@example.org\r\nDTSTART:20261110T080000Z\r\nDTEND:20261110T090000Z\r\nSUMMARY:Dentist\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
    let asked = |command: &str, params: serde_json::Value| {
        let request = serde_json::json!({ "command": command, "client": "inboxcraft-cli", "params": params });
        format!(r#"{{"out": "a.png", "mode": "light", "calendar": true, "owner_request": {request}}}"#)
    };
    let shown = picture(&asked("calendar.import", serde_json::json!({ "calendar": 2, "ics": ics })));
    assert_eq!(shown, picture(&asked("calendar.import", serde_json::json!({ "calendar": 2, "ics": ics }))), "the same bytes every time");
    assert_ne!(shown, picture(r#"{"out": "a.png", "mode": "light", "calendar": true}"#), "the question shows");
    // A safe command needs no asking, an unknown one is none, and a request the engine refuses is no question.
    for (command, params) in [
        ("calendar.list", serde_json::json!({})),
        ("no.such", serde_json::json!({})),
        ("calendar.import", serde_json::json!({ "calendar": 2, "ics": "BEGIN:VCARD" })),
    ] {
        let error = refusal(&asked(command, params));
        assert!(error.contains("owner"), "{command}: {error}");
    }
    let error = refusal(r#"{"out": "a.png", "mode": "light", "owner_request": {"command": "calendar.import", "say": "allow"}}"#);
    assert!(error.contains("say"), "{error}");
}
