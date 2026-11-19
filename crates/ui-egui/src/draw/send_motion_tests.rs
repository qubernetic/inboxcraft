// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Send (M10) at the three levels, frame by frame from the docked composer, frames 10 ms apart: the status line prints
//! in the frame of Ctrl Enter, the composer folds onto it (full) or fades (reduced), a key during it, and rest.

use egui::accesskit::Role;
use egui::{Key, Modifiers, Rect, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use inboxcraft_engine::Session;
use inboxcraft_model::{AccountId, Host, Security, Server};
use inboxcraft_store::{Batch, Change, Storage as _};
use inboxcraft_time::{FakeClock, Timestamp};

use super::reader_motion_tests::seen;
use crate::mailbox::Mailbox;
use crate::motion::Level;
use crate::{InboxApp, sample};

const SUBJECT: &str = "Plot notes";

/// The sample mail in a real session whose first account can send, at `level`, a draft written in the docked composer
/// with the keys in its subject; frames 10 ms apart.
fn written(level: Level) -> Harness<'static, InboxApp> {
    written_over(level, false).0
}

/// [`written`], over the first message opened first (J) when `opened`, and the clock the mail keeps.
fn written_over(level: Level, opened: bool) -> (Harness<'static, InboxApp>, FakeClock) {
    let shared = FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap());
    let clock = || Box::new(shared.clone());
    let mut store = sample::mailbox(Timestamp::from_unix(1_793_869_200, 0).unwrap()).unwrap();
    let account = store.accounts().unwrap().into_iter().find(|account| account.id() == AccountId::new(1)).unwrap();
    let server = Server::new(Host::new("smtp.example.org").unwrap(), Security::Tls.submission_port(), Security::Tls);
    let mut batch = Batch::new();
    batch.push(Change::PutAccount(account.with_submission(Some(server))));
    store.apply(&batch).unwrap();
    let mailbox = Mailbox::new(Session::new(Box::new(store), clock()).unwrap(), clock());
    let mut harness = crate::component::tests::builder()
        .with_size(vec2(1300.0, 820.0))
        .with_step_dt(0.01)
        .with_max_steps(100)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), InboxApp::at_mail(mailbox));
    harness.state_mut().set_motion_level(level);
    harness.run();
    if opened {
        harness.event(egui::Event::Key { key: Key::J, pressed: true, modifiers: Modifiers::NONE, repeat: false, physical_key: None });
        harness.run();
        assert!(harness.state().mailbox().selected().is_some(), "a message open");
    }
    let ctx = harness.ctx.clone();
    harness.state_mut().run_action(&ctx, crate::keymap::COMPOSE);
    harness.run();
    for (field, text) in [("To", "odile@example.org,"), ("Subject", SUBJECT)] {
        harness.get_by_role_and_label(Role::TextInput, field).focus();
        harness.run();
        harness.get_by_role_and_label(Role::TextInput, field).type_text(text);
        harness.run();
    }
    assert!(harness.state().composer().is_some_and(|composer| composer.sendable()), "ready to go");
    (harness, shared)
}

/// Ctrl Enter as one event: the frame of the key is the last drawn.
fn send(harness: &mut Harness<'static, InboxApp>) {
    harness.event(egui::Event::Key { key: Key::Enter, pressed: true, modifiers: Modifiers::COMMAND, repeat: false, physical_key: None });
    harness.step();
}

fn regions(harness: &Harness<'static, InboxApp>) -> (Rect, Rect) {
    crate::draw::shell::laid(&harness.ctx).map_or((Rect::NOTHING, Rect::NOTHING), |regions| (regions.body, regions.status))
}

/// Where the draft's subject shows in the body, and its alpha.
fn subject(harness: &Harness<'static, InboxApp>) -> Option<(u8, Rect)> {
    seen(harness, regions(harness).0).into_iter().find(|(text, _, _)| text == SUBJECT).map(|(_, alpha, at)| (alpha, at))
}

/// Whether the status line prints the send's hold.
fn sending(harness: &Harness<'static, InboxApp>) -> bool {
    seen(harness, regions(harness).1).iter().any(|(text, _, _)| text.contains("sending"))
}

#[test]
fn at_off_the_status_line_prints_in_the_frame_of_the_send_and_the_composer_is_gone() {
    let (mut harness, clock) = written_over(Level::Off, false);
    assert!(subject(&harness).is_some() && !sending(&harness));
    let before = harness.ctx.cumulative_pass_nr();
    send(&mut harness);
    assert!(harness.state().composer().is_none(), "sent");
    let asked = harness.ctx.data(|data| data.get_temp::<u64>(egui::Id::unique("reader-discard")));
    assert!(asked.is_some_and(|pass| pass >= before), "the reader asked the frame of the key drawn again: {asked:?} {before}");
    assert!(sending(&harness), "the owner never waits: the status line prints in the frame of the key");
    assert_eq!(subject(&harness), None, "the composer is gone in that frame");
    // Past the send's 5 s hold (the mail's clock) and a double click's time (the window's), it rests.
    clock.advance(std::time::Duration::from_secs(6));
    harness.run_steps(40);
    assert!(!sending(&harness), "the hold is over");
    assert!(crate::component::tests::rests(&mut harness, 2), "and it rests");
}

#[test]
fn at_full_the_composer_folds_onto_the_status_line_in_300_ms_then_125() {
    let mut harness = written(Level::Full);
    let (_, before) = subject(&harness).unwrap();
    send(&mut harness);
    assert!(sending(&harness), "0 ms: the status line prints sending");
    let (_, at) = subject(&harness).expect("0 ms: the composer stands whole");
    assert!((at.top() - before.top()).abs() < 1.0, "0 ms: where it was, {at:?} {before:?}");
    harness.run_steps(6);
    let (_, at) = subject(&harness).expect("60 ms: its head and first lines on the falling edge");
    assert!(at.top() > before.top() + 150.0, "60 ms: falling, {at:?}");
    harness.run_steps(24);
    assert_eq!(subject(&harness), None, "300 ms: one line left at the foot, the subject cut away");
    harness.run_steps(13);
    let (body, status) = regions(&harness);
    let kept = seen(&harness, Rect::from_min_max(egui::pos2(body.left(), status.top() - 40.0), status.right_top()));
    assert!(kept.iter().all(|(text, _, _)| !text.contains("New message")), "430 ms: the line has gone into the status line");
    assert!(sending(&harness));
}

#[test]
fn at_reduced_the_composer_fades_out_over_the_reader_in_125_ms() {
    let mut harness = written(Level::Reduced);
    send(&mut harness);
    assert!(sending(&harness), "0 ms: the status line prints sending");
    assert_eq!(subject(&harness).map(|(alpha, _)| alpha), Some(255), "0 ms: the composer whole");
    harness.run_steps(6);
    let alpha = subject(&harness).map(|(alpha, _)| alpha);
    assert!(alpha.is_some_and(|alpha| (80..180).contains(&alpha)), "60 ms: half gone, {alpha:?}");
    harness.run_steps(7);
    assert_eq!(subject(&harness), None, "130 ms: gone");
}

#[test]
fn a_key_while_the_composer_folds_acts_in_its_frame_and_it_is_gone() {
    for level in [Level::Full, Level::Reduced] {
        let mut harness = written(level);
        send(&mut harness);
        harness.run_steps(3);
        assert!(subject(&harness).is_some(), "{level:?}: 30 ms, still moving");
        let before = harness.ctx.memory(|memory| memory.focused());
        harness.event(egui::Event::Key { key: Key::Tab, pressed: true, modifiers: Modifiers::NONE, repeat: false, physical_key: None });
        harness.step();
        assert_eq!(subject(&harness), None, "{level:?}: the key ends it in its frame");
        assert_ne!(harness.ctx.memory(|memory| memory.focused()), before, "{level:?}: and acts: Tab moves the keyboard");
    }
}

/// A click at `at`: pressed in one frame, let go in the next.
pub(crate) fn click_at(harness: &mut Harness<'static, InboxApp>, at: egui::Pos2) {
    let button = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
    harness.event(egui::Event::PointerMoved(at));
    harness.event(button(true));
    harness.step();
    harness.event(button(false));
    harness.step();
}

#[test]
fn a_second_click_on_send_reaches_nothing_under_it() {
    // A double click on Send: the first sends, the second lands where Send stood, over the message the reader shows
    // again, which must not take it (it would open a reply).
    for level in [Level::Full, Level::Reduced, Level::Off] {
        let (mut harness, _) = written_over(level, true);
        let at = harness.get_by_role_and_label(Role::Button, "Send").rect().center();
        click_at(&mut harness, at);
        assert!(harness.state().composer().is_none(), "{level:?}: the first click sends");
        harness.step();
        click_at(&mut harness, at);
        harness.run();
        assert!(harness.state().composer().is_none(), "{level:?}: the second click opened nothing");
    }
}

#[test]
fn a_draft_scheduled_from_the_docked_composer_leaves_in_the_frame_of_its_key() {
    // Schedule plays no M10; the list it reloads draws the frame of the key again, so the owner never waits.
    for level in [Level::Off, Level::Full] {
        let (mut harness, _) = written_over(level, true);
        let later = Timestamp::from_unix(1_793_869_200 + 2 * 86_400, 0).unwrap();
        harness.state_mut().composer_mut().0.unwrap().set_later(Some(later));
        harness.run();
        send(&mut harness);
        assert!(harness.state().composer().is_none(), "{level:?}: scheduled");
        assert_eq!(subject(&harness), None, "{level:?}: the composer is gone in that frame");
        let status = seen(&harness, regions(&harness).1);
        assert!(status.iter().any(|(text, _, _)| text.starts_with("Scheduled for")), "{level:?}: the status line says so in that frame");
    }
}
