// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The scenes on the frames' sample: synced at 14:02 unless they say otherwise, the drafts edited at their own times,
//! none with `drafts` `empty`, the keys the frames' sample holds already refused, and a sample the script does not
//! know refused by name. Every window draws the app's own frame unless the scene names the system's.

use inboxcraft_time::{Clock, FakeClock, Timestamp};
use serde_json::{Value, json};

use super::{FRAMES_WORDS, NoSave, PRESENT_WORDS, Shot, drafts, frame, keep_drafts, parse_script, shoot, sync, window};
use crate::mailbox::{Mailbox, Place};
use crate::{InboxApp, sample};

fn shot(json: &str) -> Shot {
    parse_script(json).unwrap().remove(0)
}

/// The window over the frames' sample at its time, as `shoot` sets it, and its clock.
fn app() -> (InboxApp, FakeClock) {
    let clock = FakeClock::new(Timestamp::from_unix(sample::frames::TIME, 0).unwrap());
    let session = sample::frames::session(Box::new(clock.clone())).unwrap();
    let mut app = InboxApp::with_outside(Mailbox::new(session, Box::new(clock.clone())), Box::new(sample::Pictures::default()), Box::new(NoSave));
    sample::frames::prepare(app.mailbox_mut(), &clock, sample::frames::Outbox::Waiting).unwrap();
    (app, clock)
}

fn sync_status(app: &mut InboxApp) -> (String, String) {
    let status = app.mailbox_mut().run("sync.status", &json!({})).unwrap();
    let state = status.get("state").and_then(Value::as_str).unwrap_or_default().to_owned();
    let last = status.get("last_synced").and_then(Value::as_str).and_then(crate::mailbox::parse_date);
    (state, last.map(|at| app.mailbox().short_date(at)).unwrap_or_default())
}

#[test]
fn a_frames_scene_is_synced_at_14_02_without_a_sync_of_its_own() {
    let (mut app, _) = app();
    sync(&mut app, &shot(r#"{"out": "a.png", "mode": "light", "sample": "frames"}"#)).unwrap();
    assert_eq!(sync_status(&mut app), ("synced".to_owned(), "14:02".to_owned()));
}

#[test]
fn a_frames_scene_that_says_offline_stays_offline() {
    let (mut app, _) = app();
    sync(&mut app, &shot(r#"{"out": "a.png", "mode": "light", "sample": "frames", "sync": "offline"}"#)).unwrap();
    assert_eq!(sync_status(&mut app).0, "offline");
}

#[test]
fn a_present_scene_stays_offline_without_a_sync() {
    let present = shot(r#"{"out": "a.png", "mode": "light"}"#);
    let (mut app, _) = window(&present).unwrap();
    assert_eq!(app.mailbox().accounts().first().map(|account| account.address.as_str()), Some("mira@example.org"));
    sync(&mut app, &present).unwrap();
    assert_eq!(sync_status(&mut app).0, "offline");
}

#[test]
fn the_drafts_are_edited_at_their_own_times() {
    let (mut app, clock) = app();
    let now = clock.now();
    keep_drafts(&mut app, &clock, "a.png", FRAMES_WORDS.drafts).unwrap();
    app.mailbox_mut().open(Place::View(inboxcraft_model::Role::Drafts));
    let mut when: Vec<String> = app.mailbox().drafts().map(|draft| draft.when.clone()).collect();
    when.sort();
    // The one in the outbox was written yesterday, the kept one at 12:10; the clock is back at its time.
    assert_eq!(when, ["edited 12:10", "edited Yesterday"]);
    assert_eq!(clock.now(), now);
}

#[test]
fn the_present_drafts_are_its_own_two_at_their_own_times() {
    let present = shot(r#"{"out": "a.png", "mode": "light", "drafts": "filled"}"#);
    let (mut app, clock) = window(&present).unwrap();
    drafts(&mut app, &clock, &present, "filled").unwrap();
    let mut kept: Vec<(String, String)> =
        app.mailbox().drafts().map(|draft| (draft.subject.clone().unwrap_or_default(), draft.when.clone())).collect();
    kept.sort();
    let mut words: Vec<&str> = PRESENT_WORDS.drafts.iter().map(|(_, subject, _)| *subject).collect();
    words.sort_unstable();
    assert_eq!(kept.iter().map(|(subject, _)| subject.as_str()).collect::<Vec<_>>(), words);
    // Edited yesterday and at 07:06: two times, not one.
    assert_ne!(kept.first().map(|(_, when)| when), kept.get(1).map(|(_, when)| when), "{kept:?}");
}

#[test]
fn a_frames_window_with_no_drafts_has_none_and_nothing_in_the_outbox() {
    let (mut app, _) = window(&shot(r#"{"out": "a.png", "mode": "light", "sample": "frames", "drafts": "empty"}"#)).unwrap();
    app.mailbox_mut().open(Place::View(inboxcraft_model::Role::Drafts));
    assert_eq!(app.mailbox().drafts().count(), 0);
    assert_eq!(app.mailbox().drafts_count(), "no drafts");
    assert!(app.mailbox().outbox().is_empty());
    assert_eq!(crate::draw::status::read(app.mailbox_mut(), None).outbox, 0);
    // Without the key, the frames' two: the message in the outbox and the price list.
    let (mut app, _) = window(&shot(r#"{"out": "a.png", "mode": "light", "sample": "frames"}"#)).unwrap();
    app.mailbox_mut().open(Place::View(inboxcraft_model::Role::Drafts));
    assert_eq!(app.mailbox().drafts().count(), 2);
    assert_eq!(app.mailbox().outbox().len(), 1);
}

#[test]
fn the_keys_the_frames_sample_holds_already_are_refused_by_name() {
    for key in ["put_away", "invitations"] {
        for value in ["true", "false"] {
            let script = format!(r#"{{"out": "a.png", "mode": "light", "sample": "frames", "{key}": {value}}}"#);
            let error = shoot(&shot(&script)).unwrap_err().to_string();
            assert!(error.contains(&format!("a.png: {key} is part of the frames' sample")), "{error}");
        }
    }
}

#[test]
fn a_window_draws_the_own_frame_unless_the_scene_names_the_system_one() {
    for sample in ["present", "frames"] {
        let (mut window, _) = app();
        frame(&mut window, &shot(&format!(r#"{{"out": "a.png", "mode": "light", "sample": "{sample}"}}"#))).unwrap();
        assert!(window.draws_frame(), "{sample}");
    }
    let (mut system, _) = app();
    frame(&mut system, &shot(r#"{"out": "a.png", "mode": "light", "frame": "system"}"#)).unwrap();
    assert!(!system.draws_frame());
    let (mut strip, _) = app();
    frame(&mut strip, &shot(r#"{"out": "a.png", "mode": "light", "frame": "strip"}"#)).unwrap();
    assert!(strip.draws_frame());
    let error = frame(&mut strip, &shot(r#"{"out": "a.png", "mode": "light", "frame": "glass"}"#)).unwrap_err().to_string();
    assert!(error.contains("frame glass is not own or system"), "{error}");
}

#[test]
fn a_scene_without_a_frame_draws_the_own_frame_s_controls_and_system_draws_none() {
    // The window controls sit at the top right of the own frame: a scene without the key and one with `system`
    // differ there, and only there in the top strip's right end.
    let own = shoot(&shot(r#"{"out": "a.png", "mode": "light", "sample": "frames", "size": [1000, 700]}"#)).unwrap();
    let system = shoot(&shot(r#"{"out": "a.png", "mode": "light", "sample": "frames", "size": [1000, 700], "frame": "system"}"#)).unwrap();
    assert_ne!(corner(&own), corner(&system));
}

/// The top right corner of `picture`, where the own frame draws the window controls: 120 by 32 points' pixels.
fn corner(picture: &super::Picture) -> Vec<egui::Color32> {
    let (width, height) = (picture.width(), picture.height());
    let scale = width / 1000;
    (0..(32 * scale).min(height))
        .flat_map(|y| (width.saturating_sub(120 * scale)..width).map(move |x| (x, y)))
        .flat_map(|(x, y)| picture.pixel(x, y))
        .collect()
}

#[test]
fn a_sample_the_script_does_not_know_is_refused_by_name() {
    let error = shoot(&shot(r#"{"out": "a.png", "mode": "light", "sample": "old"}"#)).unwrap_err().to_string();
    assert!(error.contains("sample \"old\" is not present or frames"), "{error}");
}
