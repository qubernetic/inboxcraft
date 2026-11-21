// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The scenes on the frames' sample: synced at 14:02 unless they say otherwise, the drafts edited at their own times,
//! and a sample the script does not know refused by name.

use inboxcraft_time::{Clock, FakeClock, Timestamp};
use serde_json::{Value, json};

use super::{FRAMES_WORDS, NoSave, PRESENT_WORDS, Shot, keep_drafts, parse_script, shoot, sync};
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
    let (mut app, _) = app();
    sync(&mut app, &shot(r#"{"out": "a.png", "mode": "light"}"#)).unwrap();
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
    // The present sample's two drafts differ as well.
    let times: Vec<u64> = PRESENT_WORDS.drafts.iter().map(|(.., minutes)| *minutes).collect();
    assert!(times.windows(2).all(|pair| pair.first() != pair.get(1)), "{times:?}");
}

#[test]
fn a_sample_the_script_does_not_know_is_refused_by_name() {
    let error = shoot(&shot(r#"{"out": "a.png", "mode": "light", "sample": "old"}"#)).unwrap_err().to_string();
    assert!(error.contains("sample \"old\" is not present or frames"), "{error}");
}
