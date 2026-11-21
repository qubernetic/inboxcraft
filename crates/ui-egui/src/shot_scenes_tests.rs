// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The scenes of `shots.jsonl` show what their names say on the frames' sample: the error scenes keep the error of
//! the folder they open after the window is drawn, and the event editor's scenes pick an event the owner can edit or
//! one that is someone else's as their names say.

use egui::vec2;

use super::{Open, Shot, parse_script, render, sync, window};
use crate::mailbox::Place;
use crate::theme::Mode;
use crate::workers::NoWorkers;

/// The scenes of the script the pictures are taken from whose picture is one of `names` (without `-light.png` or
/// `-dark.png`), every one found.
fn scenes(names: &[&str]) -> Vec<Shot> {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/shots.jsonl")).unwrap();
    let scenes: Vec<Shot> = parse_script(&text)
        .unwrap()
        .into_iter()
        .filter(|shot| names.iter().any(|name| [format!("docs/images/{name}-light.png"), format!("docs/images/{name}-dark.png")].contains(&shot.out)))
        .collect();
    assert_eq!(scenes.len(), names.len() * 2, "every scene in light and dark");
    scenes
}

#[test]
fn the_frames_window_leaves_nothing_due_for_its_first_frame() {
    for drafts in ["", r#", "drafts": "empty""#] {
        let shot = parse_script(&format!(r#"{{"out": "a.png", "mode": "light", "sample": "frames"{drafts}}}"#)).unwrap().remove(0);
        let (mut app, _) = window(&shot).unwrap();
        assert!(!app.mailbox_mut().pump(&mut NoWorkers), "{drafts}: something was due");
    }
}

#[test]
fn the_error_scenes_keep_their_error_once_drawn() {
    for shot in scenes(&["error-banner", "error-banner-strip", "status-line-error"]) {
        let (mut app, _) = window(&shot).unwrap();
        app.show_mail();
        let Some(Open::Folder(id)) = shot.open else { panic!("{}: opens no folder", shot.out) };
        app.mailbox_mut().open(Place::Folder(id));
        sync(&mut app, &shot).unwrap();
        render(&mut app, vec2(1200.0, 760.0), 1.0, Mode::Light).unwrap();
        assert_eq!(app.mailbox().error(), Some("there is no folder 4242"), "{}", shot.out);
    }
}

#[test]
fn the_event_scenes_edit_an_own_event_and_read_someone_else_s() {
    for shot in scenes(&["event-edit", "event-read-only", "event-full-edit", "event-full-read-only"]) {
        let (mut app, _) = window(&shot).unwrap();
        app.show_calendar();
        super::calendar(&mut app, &shot).unwrap();
        let editor = app.calendar().editor();
        let read_only = editor.map(crate::calendar::editor::Editor::read_only);
        assert_eq!(read_only, Some(shot.out.contains("read-only")), "{}", shot.out);
    }
}
