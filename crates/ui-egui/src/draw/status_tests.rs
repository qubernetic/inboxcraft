// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The status line in the window: a strip of its own at the foot, so the reader's answers stay pressable while a
//! message with Undo shows (#296); Undo shows for the `undo` token's 5 s, and its key takes back the act of the message
//! shown; under the command line's results nothing of the line runs but a send's or a discard's Undo; the line rests;
//! and a hostile failure text and a narrow window push no key hint out.

use egui::accesskit::Role;
use egui::{Event, Key, Modifiers, PointerButton, Pos2, Rect, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use inboxcraft_time::{FakeClock, Timestamp};

use super::shell;
use crate::InboxApp;
use crate::mailbox::{Act, Mailbox};
use crate::sample;
use crate::status::UNDO_SECONDS;
use crate::theme::TextScale;

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

/// The sample window `size` big at `percent` text, its first message open in the reader; the message's id.
fn reading(size: (f32, f32), percent: u16) -> (Harness<'static, InboxApp>, u64) {
    let mut app = InboxApp::at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    app.set_text_scale(TextScale::from_percent(percent).unwrap());
    let id = app.mailbox().messages().first().map(|row| row.id).unwrap();
    app.mailbox_mut().select(Some(id));
    let mut harness = Harness::builder().with_size(vec2(size.0, size.1)).with_max_steps(12).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    (harness, id)
}

fn flagged(harness: &Harness<'static, InboxApp>, id: u64) -> bool {
    harness.state().mailbox().messages().iter().find(|row| row.id == id).is_some_and(|row| row.flagged)
}

/// A press and release of the primary button at `at`, and the frames after it.
fn press(harness: &mut Harness<'static, InboxApp>, at: Pos2) {
    harness.event(Event::PointerMoved(at));
    harness.event(Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    harness.event(Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    harness.step();
    harness.step();
}

fn line(harness: &Harness<'static, InboxApp>) -> Rect {
    shell::laid(&harness.ctx).unwrap().status
}

#[test]
fn a_press_on_reply_reply_all_and_forward_reaches_them_while_an_undo_shows() {
    // #296: the toast with Undo lay over the reader's answers and took their presses. The line is a strip of its own.
    for (size, percent) in [((1300.0, 872.0), 100), ((1000.0, 700.0), 130), ((800.0, 600.0), 100)] {
        for answer in ["Reply", "Reply all", "Forward"] {
            let (mut harness, id) = reading(size, percent);
            harness.state_mut().mailbox_mut().act(id, Act::Flag);
            harness.run();
            let undo = harness.get_by_role_and_label(Role::Button, "undo").rect();
            assert!(line(&harness).contains_rect(undo), "{size:?} {percent}: the Undo stands in the line");
            let at = harness.get_by_role_and_label(Role::Button, answer).rect();
            assert!(!at.intersects(line(&harness)), "{size:?} {percent}: {answer} {at:?} clear of the line");
            press(&mut harness, at.center());
            assert!(harness.state().composer().is_some(), "{size:?} {percent}: the press on {answer} answers");
            assert!(flagged(&harness, id), "{size:?} {percent}: and undid nothing");
        }
    }
}

#[test]
fn undo_shows_for_its_5_s_and_its_key_takes_the_act_back_within_them() {
    // A frame every quarter of a second: within its 5 s, Ctrl+Z takes the act back from the line.
    let (mut harness, id) = reading((1300.0, 872.0), 100);
    harness.state_mut().mailbox_mut().act(id, Act::Flag);
    let steps = (UNDO_SECONDS * 4.0) as usize;
    harness.run_steps(steps - 2);
    harness.get_by_role_and_label(Role::Button, "undo");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run();
    assert!(!flagged(&harness, id), "taken back by the key at 4.5 s");
    assert!(harness.query_by_role_and_label(Role::Button, "undo").is_none(), "the message went with it");
    // After its 5 s the message has gone, and its act is no longer the line's.
    let (mut harness, id) = reading((1300.0, 872.0), 100);
    harness.state_mut().mailbox_mut().act(id, Act::Flag);
    harness.run_steps(steps + 2);
    assert!(harness.query_by_role_and_label(Role::Button, "undo").is_none(), "gone after 5 s");
    assert!(harness.state().mailbox().undoable().is_none(), "its act is not the line's any more");
}

#[test]
fn undo_by_key_takes_back_the_act_shown_never_an_older_one() {
    let (mut harness, first) = reading((1300.0, 872.0), 100);
    // The third row, whose act the engine's history holds (the sample's second row's flag is not undoable).
    let second = harness.state().mailbox().messages().get(2).map(|row| row.id).unwrap();
    harness.state_mut().mailbox_mut().act(first, Act::Flag);
    harness.run();
    harness.state_mut().mailbox_mut().act(second, Act::Flag);
    harness.run();
    assert!(flagged(&harness, first) && flagged(&harness, second), "both flagged");
    assert!(harness.state().mailbox().undoable().is_some_and(|undo| undo.undoable && undo.id == second), "the second act shows");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run();
    assert!(!flagged(&harness, second), "the act shown is taken back: {} {:?}", flagged(&harness, first), harness.state().mailbox().error());
    assert!(flagged(&harness, first), "the older one stays");
}

#[test]
fn the_key_while_an_undo_shows_takes_back_that_message_s_act() {
    // The first act's message has gone after its 5 s; the second's shows. The key takes back the second, the one shown.
    let (mut harness, first) = reading((1300.0, 872.0), 100);
    let second = harness.state().mailbox().messages().get(2).map(|row| row.id).unwrap();
    harness.state_mut().mailbox_mut().act(first, Act::Flag);
    harness.run_steps((UNDO_SECONDS * 4.0) as usize + 2);
    assert!(harness.query_by_role_and_label(Role::Button, "undo").is_none(), "the first act's message has gone");
    harness.state_mut().mailbox_mut().act(second, Act::Flag);
    harness.run();
    harness.get_by_role_and_label(Role::Button, "undo");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run();
    assert!(!flagged(&harness, second), "the act shown is taken back");
    assert!(flagged(&harness, first), "the older one stays");
    // No message shows now: the key is the history's, and takes back the last act left.
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run();
    assert!(!flagged(&harness, first), "the key takes back the last act whether or not its message shows");
}

#[test]
fn nothing_of_the_line_runs_under_the_command_line_s_results() {
    // A press on its Undo, a screen reader's activate and Undo's key: none takes the act back while the results show.
    for how in ["press", "activate", "key"] {
        let (mut harness, id) = reading((1300.0, 872.0), 100);
        harness.state_mut().mailbox_mut().act(id, Act::Flag);
        harness.run();
        harness.key_press_modifiers(Modifiers::COMMAND, Key::K);
        harness.run();
        harness.state_mut().command_mut().set_text(">zz");
        harness.run();
        assert!(harness.state().command().results(), "{how}: the results show");
        match how {
            "press" => harness.get_by_role_and_label(Role::Button, "undo").click(),
            "activate" => harness.get_by_role_and_label(Role::Button, "undo").click_accesskit(),
            _ => harness.key_press_modifiers(Modifiers::COMMAND, Key::Z),
        }
        harness.run();
        assert!(flagged(&harness, id), "{how}: nothing taken back under the results");
    }
}

#[test]
fn a_discard_from_home_waits_as_long_as_its_undo_shows() {
    // The discard's wait and its Undo are one length, the `undo` token: Undo works a second before it ends (its press
    // takes two frames), and the discard is carried out just after.
    let steps = (UNDO_SECONDS * 4.0) as usize;
    for (wait, kept) in [(steps - 4, true), (steps + 1, false)] {
        let (mut harness, _) = reading((1300.0, 872.0), 100);
        let account = harness.state().mailbox().accounts().first().map(|account| account.id).unwrap();
        let draft = harness.state_mut().mailbox_mut().run("compose.new", &serde_json::json!({ "account": account })).unwrap();
        let draft = draft.get("id").and_then(serde_json::Value::as_u64).unwrap();
        harness.state_mut().mailbox_mut().run("compose.update", &serde_json::json!({ "id": draft, "subject": "Glaze order" })).unwrap();
        harness.state_mut().discard_draft(draft);
        harness.run_steps(wait);
        let undo = harness.query_by_role_and_label(Role::Button, "undo").map(|node| node.rect());
        assert_eq!(undo.is_some(), kept, "after {wait} quarter seconds");
        if undo.is_some() {
            harness.get_by_role_and_label(Role::Button, "undo").click();
        }
        harness.run();
        assert_eq!(harness.state().discarding(), None);
        let listed = harness.state_mut().mailbox_mut().run("compose.list", &serde_json::json!({})).unwrap();
        let listed = listed
            .get("drafts")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|drafts| drafts.iter().any(|row| row.get("id").and_then(serde_json::Value::as_u64) == Some(draft)));
        assert_eq!(listed, kept, "after {wait} quarter seconds: kept by Undo, else discarded");
    }
}

#[test]
fn a_discard_s_undo_keeps_the_draft_under_the_command_line_s_results() {
    // Its time runs out under the results too: one press, a screen reader's activate or the key keeps the draft.
    for how in ["press", "activate", "key"] {
        let (mut harness, _) = reading((1300.0, 872.0), 100);
        let account = harness.state().mailbox().accounts().first().map(|account| account.id).unwrap();
        let draft = harness.state_mut().mailbox_mut().run("compose.new", &serde_json::json!({ "account": account })).unwrap();
        let draft = draft.get("id").and_then(serde_json::Value::as_u64).unwrap();
        harness.state_mut().mailbox_mut().run("compose.update", &serde_json::json!({ "id": draft, "subject": "Glaze order" })).unwrap();
        harness.state_mut().discard_draft(draft);
        harness.run();
        harness.key_press_modifiers(Modifiers::COMMAND, Key::K);
        harness.run();
        harness.state_mut().command_mut().set_text(">zz");
        harness.run();
        assert!(harness.state().command().results(), "{how}: the results show");
        assert!(harness.state().discarding().is_some(), "{how}: the discard waits");
        match how {
            "press" => harness.get_by_role_and_label(Role::Button, "undo").click(),
            "activate" => harness.get_by_role_and_label(Role::Button, "undo").click_accesskit(),
            _ => harness.key_press_modifiers(Modifiers::COMMAND, Key::Z),
        }
        harness.run();
        assert_eq!(harness.state().discarding(), None, "{how}: the discard is dropped by one");
        let listed = harness.state_mut().mailbox_mut().run("compose.list", &serde_json::json!({})).unwrap();
        let kept = listed
            .get("drafts")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|drafts| drafts.iter().any(|row| row.get("id").and_then(serde_json::Value::as_u64) == Some(draft)));
        assert!(kept, "{how}: the draft is kept");
    }
}

#[test]
fn rests() {
    // At rest, with no message: the line asks for no frame sooner than a second.
    let (mut harness, _) = reading((1300.0, 872.0), 100);
    assert!(harness.query_by_role_and_label(Role::Button, "undo").is_none());
    assert!(crate::component::tests::rests(&mut harness, 4), "the line rests");
}

#[test]
fn no_fact_pushes_a_key_hint_out() {
    // Everything at once in an 800 wide window at 130 %: an error with a 10 000 character failure text from outside,
    // and an act's undo over it. The keys stand whole in the line, the message cut to what is left.
    let hostile = format!("refused\r\n\0\u{202E}{}", "x".repeat(10_000));
    for percent in [100, 130] {
        let (mut harness, id) = reading((800.0, 600.0), percent);
        harness.state_mut().mailbox_mut().fail(crate::UiError::Shot(hostile.clone()));
        harness.run();
        let status = line(&harness);
        let error = harness.query_by_label_contains("ui_shot: refused x").map(|node| node.rect()).expect("the error shows in the line");
        assert!(status.expand(0.5).contains_rect(error), "{percent}: the error {error:?} in the line {status:?}");
        harness.state_mut().mailbox_mut().act(id, Act::Flag);
        harness.run();
        let hints: Vec<Rect> = ["next", "previous", "done", "snooze", "commands"]
            .iter()
            .flat_map(|word| harness.query_all_by_label_contains(word).map(|node| node.rect()).collect::<Vec<_>>())
            .filter(|rect| status.contains_rect(*rect) || (status.intersects(*rect) && rect.height() < status.height()))
            .collect();
        assert!(!hints.is_empty(), "{percent}: the line shows its keys");
        for hint in &hints {
            assert!(hint.left() >= status.left() && hint.right() <= status.right(), "{percent}: a key {hint:?} out of the line {status:?}");
        }
        let undo = harness.get_by_role_and_label(Role::Button, "undo").rect();
        let first = hints.iter().map(|hint| hint.left()).fold(f32::INFINITY, f32::min);
        // The fact after the message (the sample signs in nowhere: offline) is cut before the keys, never under them.
        for fact in harness.query_all_by_label_contains("offline").map(|node| node.rect()).filter(|rect| status.intersects(*rect)) {
            let shown = fact.intersect(status);
            assert!(shown.right() <= first + 0.5 || shown.width() <= 0.0, "{percent}: the fact {fact:?} runs under the keys at {first}");
        }
        assert!(undo.right() <= first, "{percent}: the message {undo:?} ends before the keys at {first}");
    }
}
