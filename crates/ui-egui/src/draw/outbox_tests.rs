// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::time::Duration;

use egui::accesskit::{Role, Toggled};
use egui::{Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_engine::Session;
use inboxcraft_model::{AccountId, Host, Role as FolderRole, Security, Server};
use inboxcraft_store::{Batch, Change, Storage as _};
use inboxcraft_time::{FakeClock, Timestamp};

use super::InboxApp;
use crate::compose::Line;
use crate::mailbox::{Mailbox, Place, SendState};
use crate::sample;
use crate::workers::Scripted;

/// 2026-11-05T09:00:00Z, a Thursday.
const NOW: i64 = 1_793_869_200;

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_unix(NOW + seconds, 0).unwrap()
}

/// The window over the sample mail in a real session whose first account can send, on `clock`.
fn sending(clock: &FakeClock, theme: egui::Theme) -> Harness<'static, InboxApp> {
    let mut store = sample::mailbox(Timestamp::from_unix(NOW, 0).unwrap()).unwrap();
    let account = store.accounts().unwrap().into_iter().find(|account| account.id() == AccountId::new(1)).unwrap();
    let server = Server::new(Host::new("smtp.example.org").unwrap(), Security::Tls.submission_port(), Security::Tls);
    let mut batch = Batch::new();
    batch.push(Change::PutAccount(account.with_submission(Some(server))));
    store.apply(&batch).unwrap();
    let mailbox = Mailbox::new(Session::new(Box::new(store), Box::new(clock.clone())).unwrap(), Box::new(clock.clone()));
    let mut harness = Harness::builder()
        .with_size(vec2(1300.0, 820.0))
        .with_theme(theme)
        .with_max_steps(12)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), InboxApp::new(mailbox));
    harness.run();
    harness
}

fn clock() -> FakeClock {
    FakeClock::new(Timestamp::from_unix(NOW, 0).unwrap())
}

/// Writes a message to Odile with `subject` in the composer, from the pane's New message.
fn write(harness: &mut Harness<'static, InboxApp>, subject: &str) {
    harness.get_by_role_and_label(Role::Button, "New message").click();
    harness.run();
    let app = harness.state_mut();
    let (Some(composer), mailbox) = app.composer_mut() else { panic!("no composer") };
    "Odile Brannock <odile@example.org>".clone_into(composer.typing_mut(Line::To).unwrap());
    composer.take(mailbox, Line::To).unwrap();
    subject.clone_into(composer.subject_mut());
    composer.edited();
    composer.save(mailbox).unwrap();
    harness.run();
}

/// [`write`], then Send.
fn send(harness: &mut Harness<'static, InboxApp>, subject: &str) -> u64 {
    write(harness, subject);
    let draft = harness.state().composer().unwrap().draft();
    harness.get_by_role_and_label(Role::Button, "Send").click();
    harness.run();
    draft
}

#[test]
fn a_send_counts_down_in_its_toast_and_undo_opens_the_draft_again() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let clock = clock();
        let mut harness = sending(&clock, theme);
        let draft = send(&mut harness, "Work party on Saturday");
        assert!(harness.state().composer().is_none(), "sent");
        harness.get_by_label("Sending “Work party on Saturday” in 5 s");
        assert_eq!(harness.state().counting(), Some((draft, 5)));
        clock.advance(Duration::from_secs(2));
        harness.run();
        harness.get_by_label("Sending “Work party on Saturday” in 3 s");
        harness.get_by_role_and_label(Role::Button, "Undo").click();
        harness.run();
        assert!(harness.state().mailbox().outbox().is_empty(), "taken back");
        let composer = harness.state().composer().expect("the draft opens again");
        assert_eq!((composer.draft(), composer.subject()), (draft, "Work party on Saturday"));
        assert!(harness.query_by_label_contains("Sending “").is_none(), "the countdown is gone");
    }
}

#[test]
fn ctrl_z_takes_the_send_back_while_it_counts_down() {
    let clock = clock();
    let mut harness = sending(&clock, egui::Theme::Light);
    let draft = send(&mut harness, "Work party on Saturday");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run();
    assert!(harness.state().mailbox().outbox().is_empty());
    assert_eq!(harness.state().composer().map(crate::compose::Composer::draft), Some(draft));
}

#[test]
fn the_toast_goes_once_the_hold_is_over_and_the_send_is_on_its_way() {
    let clock = clock();
    let mut harness = sending(&clock, egui::Theme::Light);
    send(&mut harness, "Work party on Saturday");
    clock.advance(Duration::from_secs(6));
    harness.run();
    assert_eq!(harness.state().counting(), None);
    assert!(harness.query_by_label_contains("Sending “").is_none());
    let state = harness.state().mailbox().outbox().first().map(|row| row.state.clone());
    assert_eq!(state, Some(SendState::Sending), "handed to the worker");
}

#[test]
fn the_outbox_shows_in_the_pane_while_anything_waits_and_lists_it() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let clock = clock();
        let mut harness = sending(&clock, theme);
        assert!(harness.query_by_label_contains("Outbox").is_none(), "nothing waits");
        let draft = send(&mut harness, "Work party on Saturday");
        harness.get_by_label("Outbox, 1 send").click();
        harness.run();
        assert_eq!(harness.state().mailbox().place(), Place::Outbox);
        harness.get_by_label("1 waiting");
        harness.get_by_label("To Odile Brannock, Work party on Saturday, Sending in 5 s");
        harness.get_by_role_and_label(Role::Button, "Undo the send of “Work party on Saturday”").click();
        harness.run();
        assert_eq!(harness.state().composer().map(crate::compose::Composer::draft), Some(draft));
        assert_eq!(harness.state().mailbox().place(), Place::View(FolderRole::Inbox), "the outbox is empty: it gives way");
        assert!(harness.query_by_label("Outbox, 1 send").is_none());
    }
}

#[test]
fn a_time_picked_makes_send_a_schedule_and_the_outbox_says_when() {
    let clock = clock();
    let mut harness = sending(&clock, egui::Theme::Light);
    write(&mut harness, "Re: Draft logo for the bakery");
    harness.get_by_role_and_label(Role::Button, "Send later").click();
    harness.run();
    assert_eq!(harness.query_all_by_label("Send later").count(), 2, "the caret and the picker's title");
    harness.get_by_label_contains("Tomorrow morning").click();
    harness.run();
    assert_eq!(harness.state().composer().and_then(crate::compose::Composer::later), Some(at(86_400 - 3_600)));
    harness.get_by_role_and_label(Role::Button, "Schedule · Fri 08:00").click();
    harness.run();
    assert!(harness.state().composer().is_none());
    assert_eq!(harness.state().notice(), Some("Scheduled for Friday 6 November, 08:00"));
    assert_eq!(harness.state().counting(), None, "no countdown for a send later");
    harness.get_by_label("Outbox, 1 send").click();
    harness.run();
    harness.get_by_label("To Odile Brannock, Re: Draft logo for the bakery, Scheduled · Fri 08:00");
    harness.get_by_role_and_label(Role::Button, "Take “Re: Draft logo for the bakery” back and edit it").click();
    harness.run();
    assert!(harness.state().mailbox().outbox().is_empty());
    assert!(harness.state().composer().is_some(), "the draft is open again");
}

#[test]
fn send_now_instead_takes_the_time_away() {
    let clock = clock();
    let mut harness = sending(&clock, egui::Theme::Light);
    write(&mut harness, "Hello");
    let app = harness.state_mut();
    let (Some(composer), _) = app.composer_mut() else { panic!("no composer") };
    composer.set_later(Some(at(3_600)));
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Send later").click();
    harness.run();
    harness.get_by_label("Send now instead").click();
    harness.run();
    assert_eq!(harness.state().composer().and_then(crate::compose::Composer::later), None);
    harness.get_by_role_and_label(Role::Button, "Send");
}

#[test]
fn a_time_picked_that_has_passed_blocks_send_and_says_so() {
    let clock = clock();
    let mut harness = sending(&clock, egui::Theme::Light);
    write(&mut harness, "Hello");
    let app = harness.state_mut();
    let (Some(composer), _) = app.composer_mut() else { panic!("no composer") };
    composer.set_later(Some(at(60)));
    clock.advance(Duration::from_secs(120));
    harness.run();
    harness.get_by_label("The time picked has passed: pick another");
    assert!(harness.state().composer().is_some());
}

#[test]
fn a_refused_send_shows_the_reply_and_can_be_edited_or_sent_again() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let clock = clock();
        let mut harness = sending(&clock, theme);
        let draft = send(&mut harness, "Work party on Saturday");
        clock.advance(Duration::from_secs(6));
        harness.state_mut().mailbox_mut().pump(&mut Scripted::default().refusing("550 5.1.1 No such user here"));
        harness.state_mut().mailbox_mut().open(Place::Outbox);
        harness.run();
        harness.get_by_label("1 not sent");
        harness.get_by_label("To Odile Brannock, Work party on Saturday, Not sent: 550 5.1.1 No such user here");
        harness.get_by_role_and_label(Role::Button, "Send “Work party on Saturday” again").click();
        harness.run();
        assert_eq!(harness.state().counting(), Some((draft, 5)), "held again, counted down");
        harness.get_by_label("Sending “Work party on Saturday” in 5 s");
        clock.advance(Duration::from_secs(6));
        harness.state_mut().mailbox_mut().pump(&mut Scripted::default().refusing("451 4.3.0 Try again later"));
        harness.run();
        harness.get_by_role_and_label(Role::Button, "Edit “Work party on Saturday”").click();
        harness.run();
        assert_eq!(harness.state().composer().map(crate::compose::Composer::draft), Some(draft));
        assert_eq!(harness.state().mailbox().outbox().len(), 1, "still refused until it is sent again");
    }
}

#[test]
fn a_send_that_went_late_says_so_once() {
    let clock = clock();
    let mut harness = sending(&clock, egui::Theme::Light);
    write(&mut harness, "Work party on Saturday");
    let app = harness.state_mut();
    let (Some(composer), _) = app.composer_mut() else { panic!("no composer") };
    composer.set_later(Some(at(3_600)));
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Schedule · 10:00").click();
    harness.run();
    clock.advance(Duration::from_secs(7_200));
    harness.run();
    assert_eq!(harness.state().notice(), Some("Sent late, InboxCraft was not running at its time: Work party on Saturday"));
}

#[test]
fn the_sending_page_sets_how_long_a_send_is_held() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let clock = clock();
        let mut harness = sending(&clock, theme);
        let appearance = *harness.state().appearance();
        harness.state_mut().settings_mut().open_at(crate::settings::Section::Sending, &appearance);
        harness.run();
        harness.get_by_label("Undo send");
        let toggled = |harness: &Harness<'static, InboxApp>, name: &str| harness.get_by_role_and_label(Role::Button, name).accesskit_node().toggled();
        assert_eq!(toggled(&harness, "5 s"), Some(Toggled::True), "five seconds until the owner chooses");
        harness.get_by_role_and_label(Role::Button, "30 s").click();
        harness.run();
        assert_eq!(harness.state().mailbox().hold_seconds(), 30);
        assert_eq!(toggled(&harness, "30 s"), Some(Toggled::True));
        harness.get_by_label_contains("Now 30 s.");
    }
}

#[test]
fn a_bounced_message_is_marked_in_its_row_and_its_reader_says_why() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let clock = clock();
        let mut store = sample::mailbox(Timestamp::from_unix(NOW, 0).unwrap()).unwrap();
        sample::bounce(&mut store).unwrap();
        let mailbox = Mailbox::new(Session::sample(Box::new(store), Box::new(clock.clone())).unwrap(), Box::new(clock.clone()));
        let mut harness = Harness::builder()
            .with_size(vec2(1300.0, 820.0))
            .with_theme(theme)
            .with_max_steps(12)
            .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), InboxApp::new(mailbox));
        harness.state_mut().mailbox_mut().open(Place::View(FolderRole::Sent));
        harness.run();
        harness.get_by_label_contains("not delivered").click();
        harness.run();
        harness.get_by_label_contains("Not delivered: petra@example.org: 550 5.1.1");
    }
}

/// An answer to an invitation the server refused can be sent again, but not edited: its draft is the answer, which
/// changes only from the invitation.
#[test]
fn a_refused_invitation_answer_offers_no_edit() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let clock = clock();
        let mut store = sample::mailbox(Timestamp::from_unix(NOW, 0).unwrap()).unwrap();
        let draft = inboxcraft_model::Draft {
            to: vec!["odile@example.org".to_owned()],
            subject: "Accepted: Lunch with Odile".to_owned(),
            calendar: Some("BEGIN:VCALENDAR\r\nMETHOD:REPLY\r\nEND:VCALENDAR\r\n".to_owned()),
            ..inboxcraft_model::Draft::new(inboxcraft_model::DraftId::new(90), AccountId::new(1), at(0))
        };
        let refused = inboxcraft_model::OutboxState::Refused { reply: "550 5.1.1 No such user here".to_owned() };
        let account = store.accounts().unwrap().into_iter().find(|account| account.id() == AccountId::new(1)).unwrap();
        let server = Server::new(Host::new("smtp.example.org").unwrap(), Security::Tls.submission_port(), Security::Tls);
        let mut batch = Batch::new();
        batch.push(Change::PutAccount(account.with_submission(Some(server))));
        batch.push(Change::PutDraft(draft));
        batch.push(Change::PutOutgoing(inboxcraft_model::Outgoing::new(inboxcraft_model::DraftId::new(90), refused)));
        store.apply(&batch).unwrap();
        let mailbox = Mailbox::new(Session::new(Box::new(store), Box::new(clock.clone())).unwrap(), Box::new(clock.clone()));
        let mut harness = Harness::builder()
            .with_size(vec2(1300.0, 820.0))
            .with_theme(theme)
            .with_max_steps(12)
            .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), InboxApp::new(mailbox));
        harness.state_mut().mailbox_mut().open(Place::Outbox);
        harness.run();
        assert!(harness.state().mailbox().outbox().first().is_some_and(|row| row.invite_answer));
        assert!(harness.query_by_label("Edit “Accepted: Lunch with Odile”").is_none());
        harness.get_by_role_and_label(Role::Button, "Send “Accepted: Lunch with Odile” again").click();
        harness.run();
        // It goes at once: no hold, no countdown, no Undo.
        assert_eq!(harness.state().counting(), None);
        assert!(harness.query_by_label_contains("Sending “").is_none());
        let state = harness.state().mailbox().outbox().first().map(|row| row.state.clone());
        assert_eq!(state, Some(SendState::Sending));
    }
}
