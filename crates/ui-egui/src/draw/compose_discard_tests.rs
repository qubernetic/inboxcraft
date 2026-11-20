// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The composer's own Discard (#340): the composer closes behind "Draft discarded" with Undo, as a discard from Home
//! or the drafts place does, the draft kept as it stood until the toast ends; Undo brings the composer back as it
//! was, in the reader's place or floating; when the toast ends, the window closes or the app ends the draft goes. A
//! new message nothing was written in goes at once, without an Undo. A second discard carries out the first, an Undo
//! over a new composer keeps that one first, and a send's countdown keeps the line while the discard waits under it.

use std::time::Duration;

use egui::accesskit::Role;
use egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use inboxcraft_engine::Session;
use inboxcraft_model::{AccountId, Host, Security, Server};
use inboxcraft_store::{Batch, Change, Storage as _};
use inboxcraft_time::{FakeClock, Timestamp};
use serde_json::{Value, json};

use super::{InboxApp, Screen};
use crate::compose::Line;
use crate::keymap;
use crate::mailbox::Mailbox;
use crate::sample;

/// 2026-11-05T09:00:00Z.
const NOW: i64 = 1_793_869_200;

fn clock() -> FakeClock {
    FakeClock::new(Timestamp::from_unix(NOW, 0).unwrap())
}

fn window(mailbox: Mailbox, theme: egui::Theme) -> Harness<'static, InboxApp> {
    let mut harness = Harness::builder()
        .with_size(vec2(1300.0, 820.0))
        .with_theme(theme)
        .with_max_steps(40)
        .with_step_dt(1.0 / 60.0)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), InboxApp::at_mail(mailbox));
    harness.run();
    harness
}

fn sample_window() -> Harness<'static, InboxApp> {
    let clock = clock();
    window(Mailbox::new(sample::session(Box::new(clock.clone())).unwrap(), Box::new(clock)), egui::Theme::Light)
}

/// Opens a new message from the pane's New message and writes in it: a chip for Odile and `te` still typed in To,
/// `subject` and a line of text, the last change not kept yet. The draft's id.
fn write(harness: &mut Harness<'static, InboxApp>, subject: &str) -> u64 {
    crate::draw::tests::new_message(harness);
    harness.run();
    let (Some(composer), mailbox) = harness.state_mut().composer_mut() else { panic!("no composer") };
    "Odile Brannock <odile@example.org>, te".clone_into(composer.typing_mut(Line::To).unwrap());
    composer.typed(mailbox, Line::To).unwrap();
    subject.clone_into(composer.subject_mut());
    let text = format!("Bring gloves.{}", composer.text());
    *composer.text_mut() = text;
    composer.edited();
    let draft = composer.draft();
    harness.run();
    draft
}

fn press_discard(harness: &mut Harness<'static, InboxApp>) {
    harness.get_by_role_and_label(Role::Button, "Discard the draft").click();
    harness.run();
}

fn press_undo(harness: &mut Harness<'static, InboxApp>) {
    harness.get_by_role_and_label(Role::Button, "undo").click();
    harness.run();
}

fn kept(harness: &mut Harness<'static, InboxApp>, id: u64) -> Option<Value> {
    harness.state_mut().mailbox_mut().run("compose.show", &json!({ "id": id })).ok()
}

/// Runs the frames until the toast's time is over.
fn wait_out_the_toast(harness: &mut Harness<'static, InboxApp>) {
    let later = harness.ctx.input(|input| input.time) + crate::status::UNDO_SECONDS + 1.0;
    while harness.ctx.input(|input| input.time) < later {
        harness.step();
    }
    harness.run();
}

#[test]
fn discard_closes_the_composer_behind_its_undo_and_undo_brings_it_back_as_it_was() {
    let mut harness = sample_window();
    let draft = write(&mut harness, "Kiln rota");
    // The last words, not kept by any frame yet.
    let (Some(composer), _) = harness.state_mut().composer_mut() else { panic!("no composer") };
    "Kiln rota for Friday".clone_into(composer.subject_mut());
    composer.edited();
    let text = composer.text().to_owned();
    press_discard(&mut harness);
    assert!(harness.state().composer().is_none(), "the composer closes");
    assert_eq!(harness.state().discarding(), Some(draft));
    harness.get_by_label("Draft discarded");
    // Nothing is gone yet, and the draft waits as it stood: after a crash it is still there, newest words and all.
    let held = kept(&mut harness, draft).expect("the discard waits for its toast");
    assert_eq!(held["subject"], "Kiln rota for Friday");
    // What was typed in To became a chip as the field was left, as it does on every way out of the line.
    let to: Vec<&str> = held["to"].as_array().unwrap().iter().filter_map(|chip| chip["text"].as_str()).collect();
    assert_eq!(to, ["Odile Brannock <odile@example.org>", "te"]);
    press_undo(&mut harness);
    assert_eq!(harness.state().discarding(), None);
    let composer = harness.state().composer().expect("Undo brings the composer back");
    assert_eq!(composer.draft(), draft);
    let chips: Vec<&str> = composer.chips(Line::To).iter().map(|chip| chip.text.as_str()).collect();
    assert_eq!(chips, to);
    assert_eq!((composer.subject(), composer.text()), ("Kiln rota for Friday", text.as_str()));
    assert!(!composer.floating(), "in the reader's place, as it was");
    assert_eq!(harness.state().screen(), Screen::Mail);
    assert!(kept(&mut harness, draft).is_some(), "and the draft stays");
    harness.get_by_role_and_label(Role::TextInput, "Subject");
}

#[test]
fn undo_brings_a_floating_composer_back_floating() {
    let mut harness = sample_window();
    let draft = write(&mut harness, "Kiln rota");
    let ctx = harness.ctx.clone();
    harness.state_mut().composer_mut().0.unwrap().set_floating(true);
    harness.state_mut().run_action(&ctx, keymap::CALENDAR);
    harness.run();
    assert_eq!(harness.state().screen(), Screen::Calendar);
    press_discard(&mut harness);
    assert!(harness.state().composer().is_none());
    press_undo(&mut harness);
    let composer = harness.state().composer().expect("Undo brings the composer back");
    assert_eq!(composer.draft(), draft);
    assert!(composer.floating(), "floating, as it was");
    assert_eq!(harness.state().screen(), Screen::Calendar, "over the calendar, where the owner is");
}

#[test]
fn when_the_toast_ends_the_draft_goes() {
    let mut harness = sample_window();
    let draft = write(&mut harness, "Kiln rota");
    press_discard(&mut harness);
    wait_out_the_toast(&mut harness);
    assert_eq!(harness.state().discarding(), None);
    assert!(kept(&mut harness, draft).is_none(), "carried out when the toast ends");
    assert!(harness.state().composer().is_none(), "and nothing comes back");
}

#[test]
fn a_new_message_nothing_was_written_in_just_goes_without_an_undo() {
    let mut harness = sample_window();
    crate::draw::tests::new_message(&mut harness);
    harness.run();
    let draft = harness.state().composer().unwrap().draft();
    press_discard(&mut harness);
    assert!(harness.state().composer().is_none());
    assert_eq!(harness.state().discarding(), None, "nothing to take back");
    assert!(kept(&mut harness, draft).is_none(), "gone at once");
    assert!(harness.query_by_role_and_label(Role::Button, "undo").is_none(), "no Undo offered");
}

#[test]
fn closing_the_window_inside_the_toast_carries_the_discard_out() {
    let mut harness = sample_window();
    let draft = write(&mut harness, "Kiln rota");
    press_discard(&mut harness);
    harness.input_mut().viewports.entry(egui::ViewportId::ROOT).or_default().events.push(egui::ViewportEvent::Close);
    harness.step();
    assert_eq!(harness.state().discarding(), None);
    assert!(kept(&mut harness, draft).is_none(), "carried out, as a discard from Home is");
}

#[test]
fn the_end_of_the_app_inside_the_toast_carries_the_discard_out() {
    let dir = tempfile::tempdir().unwrap();
    let now = Timestamp::from_unix(NOW, 0).unwrap();
    let mut store = inboxcraft_store::Sqlite::open(dir.path(), inboxcraft_store::StoreConfig::default()).unwrap();
    let mut batch = Batch::new();
    for account in sample::mailbox(now).unwrap().accounts().unwrap() {
        batch.push(Change::PutAccount(account));
    }
    store.apply(&batch).unwrap();
    let clock = clock();
    let mut harness = window(Mailbox::new(Session::new(Box::new(store), Box::new(clock.clone())).unwrap(), Box::new(clock)), egui::Theme::Light);
    let draft = write(&mut harness, "Kiln rota");
    let account = harness.state().composer().unwrap().account();
    press_discard(&mut harness);
    assert_eq!(harness.state().discarding(), Some(draft));
    drop(harness);
    let store = inboxcraft_store::Sqlite::open(dir.path(), inboxcraft_store::StoreConfig::default()).unwrap();
    let drafts = store.drafts(AccountId::new(account)).unwrap();
    assert!(drafts.iter().all(|held| held.id.get() != draft), "carried out as the app ends, as a discard from Home is");
}

#[test]
fn a_second_discard_carries_out_the_first() {
    let mut harness = sample_window();
    let first = write(&mut harness, "Kiln rota");
    press_discard(&mut harness);
    let second = write(&mut harness, "Glaze order");
    assert_ne!(first, second);
    press_discard(&mut harness);
    assert!(kept(&mut harness, first).is_none(), "the first discard is carried out");
    assert_eq!(harness.state().discarding(), Some(second));
    press_undo(&mut harness);
    let composer = harness.state().composer().expect("Undo brings the second back");
    assert_eq!((composer.draft(), composer.subject()), (second, "Glaze order"));
}

#[test]
fn undo_over_a_new_composer_keeps_that_one_and_brings_the_discarded_one_back() {
    let mut harness = sample_window();
    let first = write(&mut harness, "Kiln rota");
    press_discard(&mut harness);
    let second = write(&mut harness, "Glaze order");
    let (Some(composer), _) = harness.state_mut().composer_mut() else { panic!("no composer") };
    assert_eq!(composer.draft(), second);
    "Glaze order for May".clone_into(composer.subject_mut());
    composer.edited();
    press_undo(&mut harness);
    let composer = harness.state().composer().expect("the discarded one comes back");
    assert_eq!((composer.draft(), composer.subject()), (first, "Kiln rota"));
    let held = kept(&mut harness, second).expect("the new one is kept");
    assert_eq!(held["subject"], "Glaze order for May", "with its newest words");
}

/// The sample mail in a real session whose first account can send, on `clock`.
fn sending(clock: &FakeClock) -> Harness<'static, InboxApp> {
    let mut store = sample::mailbox(Timestamp::from_unix(NOW, 0).unwrap()).unwrap();
    let account = store.accounts().unwrap().into_iter().find(|account| account.id() == AccountId::new(1)).unwrap();
    let server = Server::new(Host::new("smtp.example.org").unwrap(), Security::Tls.submission_port(), Security::Tls);
    let mut batch = Batch::new();
    batch.push(Change::PutAccount(account.with_submission(Some(server))));
    store.apply(&batch).unwrap();
    window(Mailbox::new(Session::new(Box::new(store), Box::new(clock.clone())).unwrap(), Box::new(clock.clone())), egui::Theme::Light)
}

#[test]
fn a_send_s_countdown_keeps_the_line_while_a_discard_waits_under_it() {
    let clock = clock();
    let mut harness = sending(&clock);
    crate::draw::tests::new_message(&mut harness);
    harness.run();
    let (Some(composer), mailbox) = harness.state_mut().composer_mut() else { panic!("no composer") };
    "Odile Brannock <odile@example.org>".clone_into(composer.typing_mut(Line::To).unwrap());
    composer.take(mailbox, Line::To).unwrap();
    "Work party".clone_into(composer.subject_mut());
    composer.edited();
    let sent = composer.draft();
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Send").click();
    harness.run();
    assert_eq!(harness.state().counting().map(|(draft, _)| draft), Some(sent));
    let discarded = write(&mut harness, "Kiln rota");
    press_discard(&mut harness);
    assert_eq!(harness.state().discarding(), Some(discarded), "the discard waits");
    harness.get_by_label("sending · Work party, 5 s");
    // Undo is the send's while its countdown has the line: the send comes back, the discard still waits.
    press_undo(&mut harness);
    assert_eq!(harness.state().counting(), None);
    assert_eq!(harness.state().composer().map(crate::compose::Composer::draft), Some(sent));
    assert_eq!(harness.state().discarding(), Some(discarded));
    assert!(kept(&mut harness, discarded).is_some());
    clock.advance(Duration::from_secs(1));
    wait_out_the_toast(&mut harness);
    assert!(kept(&mut harness, discarded).is_none(), "carried out when its time ends");
    assert_eq!(harness.state().composer().map(crate::compose::Composer::draft), Some(sent), "the send's draft stays open");
}

#[test]
fn a_discard_keeps_the_newest_words_first_so_nothing_is_gone_before_the_toast_ends() {
    let mut harness = sample_window();
    let draft = write(&mut harness, "Kiln rota");
    let (Some(composer), _) = harness.state_mut().composer_mut() else { panic!("no composer") };
    "Kiln rota for Friday".clone_into(composer.subject_mut());
    composer.edited();
    // Without a frame between: the discard itself keeps them.
    harness.state_mut().discard_composer();
    assert!(harness.state().composer().is_none());
    assert!(harness.state().mailbox().unkept_drafts().is_empty(), "on the disk");
    assert_eq!(kept(&mut harness, draft).unwrap()["subject"], "Kiln rota for Friday");
}
