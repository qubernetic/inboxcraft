// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The add-account page driven through its AccessKit tree.

use std::sync::Arc;

use egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use inboxcraft_model::{CheckStep, ErrorKind, StepStatus};
use inboxcraft_time::{FakeClock, Timestamp};

use super::InboxApp;
use crate::mailbox::Mailbox;
use crate::onboard::tests::{FakeWorkers, Kept, PASSWORD, mailbox, proposal};
use crate::onboard::{Proposed, Secret, Source, Stage};
use crate::sample;

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

fn app(kept: &Kept, workers: FakeWorkers) -> InboxApp {
    let detect = Proposed::new(proposal(Source::Autoconfig));
    InboxApp::new(mailbox()).with_accounts(Arc::new(kept.clone()), Box::new(detect), Box::new(workers))
}

fn harness(app: InboxApp, theme: egui::Theme) -> Harness<'static, InboxApp> {
    let mut harness = Harness::builder().with_size(vec2(1100.0, 760.0)).with_theme(theme).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

fn type_in(harness: &mut Harness<'static, InboxApp>) {
    let (onboarding, _) = harness.state_mut().onboarding_mut();
    let form = onboarding.form_mut();
    form.address = "carol@example.org".to_owned();
    form.password = Secret::new(PASSWORD);
    harness.run();
}

#[test]
fn the_empty_window_offers_to_add_an_account_and_opens_the_form() {
    let mut harness = harness(app(&Kept::default(), FakeWorkers::passing()), egui::Theme::Light);
    harness.get_by_label("No accounts yet");
    assert_eq!(harness.get_all_by_label("Add account").count(), 2, "in the pane and in the empty list");
    harness.get_all_by_label("Add account").next().unwrap().click();
    harness.run();
    assert_eq!(harness.state().onboarding().stage(), Stage::Form);
    harness.get_by_label("Add an account");
    harness.get_by_label("Password");
    harness.get_by_label("Cancel").click();
    harness.run();
    assert_eq!(harness.state().onboarding().stage(), Stage::Closed);
}

#[test]
fn a_window_without_onboarding_offers_nothing() {
    let app = InboxApp::new(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    assert!(!app.can_add());
    let harness = harness(app, egui::Theme::Light);
    assert_eq!(harness.query_all_by_label("Add account").count(), 0);
    let empty = harness_empty();
    empty.get_by_label("Start InboxCraft with --sample to look around an invented mailbox.");
}

fn harness_empty() -> Harness<'static, InboxApp> {
    harness(InboxApp::new(mailbox()), egui::Theme::Light)
}

#[test]
fn detect_add_and_a_passing_check_show_every_step_with_its_server() {
    let kept = Kept::default();
    let mut harness = harness(app(&kept, FakeWorkers::passing()), egui::Theme::Dark);
    harness.get_all_by_label("Add account").next().unwrap().click();
    harness.run();
    type_in(&mut harness);
    harness.get_by_label("Detect").click();
    harness.run();
    harness.run();
    harness.get_by_label("Found in your provider's settings file.");
    harness.get_by_label("What Detect asked (2)");
    assert_eq!(harness.state().onboarding().form().host, "imap.example.org");
    harness.get_by_label("Add and check").click();
    harness.run();
    harness.run();
    assert_eq!(kept.passwords.lock().unwrap().get(&1).map(String::as_str), Some(PASSWORD));
    harness.get_by_label("Checking carol@example.org");
    harness.get_by_label("Signed in. InboxCraft now fetches the folders and the mail.");
    harness.get_by_label("IMAP sign-in: passed");
    harness.get_by_label("Certificate: passed");
    harness.get_by_label("imap.example.org:993 · issued by Example Test CA");
    harness.get_by_label("SMTP sign-in: not built yet");
    harness.check_accessibility();
    harness.get_by_label("Done").click();
    harness.run();
    assert_eq!(harness.state().onboarding().stage(), Stage::Closed);
    assert!(harness.query_all_by_label("carol@example.org").count() >= 1, "the account is in the pane");
}

#[test]
fn a_failed_step_offers_to_check_again_or_change_the_settings() {
    let failed = FakeWorkers::answering(&[(CheckStep::Dns, StepStatus::Passed), (CheckStep::Tcp, StepStatus::Failed(ErrorKind::MailPortsBlocked))]);
    let kept = Kept::default();
    let mut harness = harness(app(&kept, failed), egui::Theme::Light);
    harness.get_all_by_label("Add account").next().unwrap().click();
    harness.run();
    type_in(&mut harness);
    harness.state_mut().onboarding_mut().0.form_mut().host = "imap.example.org".to_owned();
    harness.get_by_label("Add and check").click();
    harness.run();
    harness.run();
    harness.get_by_label("A step failed. Check again, or change the settings.");
    harness.get_by_label("Connection (TCP): failed");
    harness.get_by_label(&ErrorKind::MailPortsBlocked.to_string());
    harness.get_by_label("Encryption (TLS): skipped");
    harness.check_accessibility();
    harness.get_by_label("Check again").click();
    harness.run();
    harness.get_by_label("Connection (TCP): failed");
    harness.get_by_label("Change settings").click();
    harness.run();
    assert_eq!(harness.state().onboarding().stage(), Stage::Form);
    assert!(kept.passwords.lock().unwrap().is_empty());
    harness.get_by_label("Add an account");
    harness.check_accessibility();
}

#[test]
fn every_control_of_the_form_has_a_name_in_both_modes() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let mut harness = harness(app(&Kept::default(), FakeWorkers::passing()), theme);
        harness.check_accessibility();
        harness.state_mut().onboarding_mut().0.open();
        harness.run();
        harness.check_accessibility();
        harness.get_by_label("STARTTLS").click();
        harness.run();
        assert_eq!(harness.state().onboarding().form().security, Some(crate::onboard::Security::StartTls));
        // An error shows above the form.
        harness.get_by_label("Add and check").click();
        harness.run();
        harness.get_by_label("Enter the address.");
    }
}
