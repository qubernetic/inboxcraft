// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The add-account page driven through its AccessKit tree.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use egui::accesskit::Role;
use egui::{Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use inboxcraft_model::{CheckStep, ErrorKind, StepStatus};
use inboxcraft_time::{FakeClock, Timestamp};

use super::InboxApp;
use super::tests::drew_icon;
use crate::iconset::{IconId, Style};
use crate::mailbox::Mailbox;
use crate::onboard::tests::{FakeWorkers, Kept, PASSWORD, mailbox, proposal};
use crate::onboard::{MAX_PASSWORD_CHARS, Proposed, Secret, Source, Stage};
use crate::sample;
use crate::settings::Section;

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

/// Presses Add account on the empty window, then on the sheet's Accounts page: the form opens there.
fn open_form(harness: &mut Harness<'static, InboxApp>) {
    harness.get_by_role_and_label(Role::Button, "Add account").click();
    harness.run();
    on_sheet(harness, "Add account").click();
    harness.run();
}

/// The button called `label` on the settings sheet: the window under it may hold one of the same name, and the sheet
/// comes after it in the tree.
fn on_sheet<'h>(harness: &'h Harness<'static, InboxApp>, label: &'h str) -> egui_kittest::Node<'h> {
    let sheet = harness.get_by_role_and_label(Role::Dialog, "Settings").rect();
    harness.get_all_by_role_and_label(Role::Button, label).rfind(|node| sheet.contains_rect(node.rect())).unwrap()
}

#[test]
fn the_empty_window_offers_add_account_which_opens_settings_on_accounts_where_the_form_opens() {
    let mut harness = harness(app(&Kept::default(), FakeWorkers::passing()), egui::Theme::Light);
    harness.get_by_label("No accounts yet");
    // Without an account the folder pane would hold only the app's name: it is left out.
    assert_eq!(harness.get_all_by_label("Add account").count(), 1, "on the empty window only");
    assert!(harness.query_by_label("MAILBOXES").is_none() && harness.query_all_by_label("InboxCraft").count() <= 1, "no folder pane");
    harness.get_by_role_and_label(Role::Button, "Add account").click();
    harness.run();
    assert!(harness.state().settings().is_open());
    assert_eq!(harness.state().settings().section(), Section::Accounts);
    assert_eq!(harness.state().onboarding().stage(), Stage::Closed, "the list first");
    harness.get_by_label("No account yet. Add one to read and send mail here.");
    harness.get_by_label("Your accounts show up here");
    harness.check_accessibility();
    on_sheet(&harness, "Add account").click();
    harness.run();
    assert_eq!(harness.state().onboarding().stage(), Stage::Form);
    let sheet = harness.get_by_role_and_label(Role::Dialog, "Settings").rect();
    assert!(sheet.contains_rect(harness.get_by_label("Add an account").rect()), "the form is on the sheet");
    harness.get_by_label("Password");
    harness.check_accessibility();
    harness.get_by_label("Cancel").click();
    harness.run();
    assert_eq!(harness.state().onboarding().stage(), Stage::Closed);
    assert_eq!(harness.state().settings().section(), Section::Accounts, "Cancel leads back to the list");
    on_sheet(&harness, "Add account").click();
    harness.run();
    harness.get_by_label("Back to the accounts").click();
    harness.run();
    assert_eq!(harness.state().onboarding().stage(), Stage::Closed);
    harness.get_by_label("Your accounts show up here");
}

#[test]
fn closing_the_sheet_closes_the_form_on_it() {
    let mut harness = harness(app(&Kept::default(), FakeWorkers::passing()), egui::Theme::Light);
    open_form(&mut harness);
    harness.key_press(Key::Escape);
    harness.run();
    assert!(!harness.state().settings().is_open());
    assert_eq!(harness.state().onboarding().stage(), Stage::Closed, "the form would open the sheet again");
}

#[test]
fn opening_the_form_from_outside_opens_it_on_the_sheet() {
    let mut harness = harness(app(&Kept::default(), FakeWorkers::passing()), egui::Theme::Light);
    harness.state_mut().onboarding_mut().0.open();
    harness.run();
    assert!(harness.state().settings().is_open());
    assert_eq!(harness.state().settings().section(), Section::Accounts);
    harness.get_by_label("Add an account");
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
    open_form(&mut harness);
    type_in(&mut harness);
    harness.get_by_label("Detect").click();
    harness.run();
    harness.run();
    harness.get_by_label("Found in your provider's settings file.");
    harness.get_by_label("What Detect asked (2)");
    assert_eq!(harness.state().onboarding().form().host, "imap.example.org");
    harness.get_by_label("Check and add").click();
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
    harness.get_by_label("carol@example.org, carol@example.org");
}

#[test]
fn a_failed_step_offers_to_check_again_or_change_the_settings() {
    let failed = FakeWorkers::answering(&[(CheckStep::Dns, StepStatus::Passed), (CheckStep::Tcp, StepStatus::Failed(ErrorKind::MailPortsBlocked))]);
    let kept = Kept::default();
    let mut harness = harness(app(&kept, failed), egui::Theme::Light);
    open_form(&mut harness);
    type_in(&mut harness);
    harness.state_mut().onboarding_mut().0.form_mut().host = "imap.example.org".to_owned();
    harness.get_by_label("Check and add").click();
    harness.run();
    harness.run();
    // The reason stands under the title, as the design has it.
    let reason = harness.get_by_label(&format!("Connection (TCP) failed: {}", ErrorKind::MailPortsBlocked)).rect();
    let title = harness.get_by_label("Checking carol@example.org").rect();
    assert!(reason.top() >= title.bottom() - 1.0 && reason.top() - title.bottom() < 40.0, "{reason:?} under {title:?}");
    harness.get_by_label("Connection (TCP): failed");
    harness.get_by_label(&ErrorKind::MailPortsBlocked.to_string());
    harness.get_by_label("Encryption (TLS): skipped");
    for (id, style) in [(IconId::Done, Style::Fill), (IconId::Failed, Style::Fill), (IconId::Skipped, Style::Regular)] {
        assert!(drew_icon(harness.output(), id, style), "{id:?} {style:?}");
    }
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
        harness.get_by_label("Check and add").click();
        harness.run();
        harness.get_by_label("Enter the address.");
    }
}

/// Types `password` into the password field through the keyboard, as the owner would.
fn type_password(harness: &mut Harness<'static, InboxApp>, password: &str) {
    harness.get_by_role_and_label(Role::PasswordInput, "Password").focus();
    harness.run();
    harness.get_by_role_and_label(Role::PasswordInput, "Password").type_text(password);
    harness.run();
}

/// Presses Ctrl+Z (Cmd+Z on a Mac) in the password field.
fn undo_in_password(harness: &mut Harness<'static, InboxApp>) {
    harness.get_by_role_and_label(Role::PasswordInput, "Password").focus();
    harness.run();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run();
    harness.run();
}

#[test]
fn undo_brings_no_password_back_after_change_settings() {
    let failed = FakeWorkers::answering(&[(CheckStep::Tcp, StepStatus::Failed(ErrorKind::MailPortsBlocked))]);
    let kept = Kept::default();
    let mut harness = harness(app(&kept, failed), egui::Theme::Light);
    harness.state_mut().onboarding_mut().0.open();
    harness.run();
    harness.state_mut().onboarding_mut().0.form_mut().address = "carol@example.org".to_owned();
    harness.state_mut().onboarding_mut().0.form_mut().host = "imap.example.org".to_owned();
    harness.run();
    type_password(&mut harness, PASSWORD);
    assert_eq!(harness.state().onboarding().form().password.expose(), PASSWORD);
    harness.get_by_label("Check and add").click();
    harness.run();
    harness.run();
    harness.get_by_label("Change settings").click();
    harness.run();
    assert_eq!(harness.state().onboarding().stage(), Stage::Form);
    undo_in_password(&mut harness);
    assert!(harness.state().onboarding().form().password.is_empty(), "undo restored the password");
}

#[test]
fn undo_brings_no_password_of_an_earlier_account_into_the_next() {
    let kept = Kept::default();
    let mut harness = harness(app(&kept, FakeWorkers::passing()), egui::Theme::Light);
    harness.state_mut().onboarding_mut().0.open();
    harness.run();
    harness.state_mut().onboarding_mut().0.form_mut().address = "carol@example.org".to_owned();
    harness.state_mut().onboarding_mut().0.form_mut().host = "imap.example.org".to_owned();
    harness.run();
    type_password(&mut harness, PASSWORD);
    harness.get_by_label("Check and add").click();
    harness.run();
    harness.run();
    harness.get_by_label("Done").click();
    harness.run();
    harness.state_mut().onboarding_mut().0.open();
    harness.run();
    harness.state_mut().onboarding_mut().0.form_mut().address = "dave@example.org".to_owned();
    harness.state_mut().onboarding_mut().0.form_mut().host = "imap.example.org".to_owned();
    harness.run();
    undo_in_password(&mut harness);
    assert!(harness.state().onboarding().form().password.is_empty(), "undo restored the first account's password");
    harness.get_by_label("Check and add").click();
    harness.run();
    harness.get_by_label("Enter the password.");
    assert_eq!(kept.passwords.lock().unwrap().get(&2), None, "no password for the second account");
}

#[test]
fn the_password_field_takes_no_more_than_its_limit_and_never_grows_its_buffer() {
    let mut harness = harness(app(&Kept::default(), FakeWorkers::passing()), egui::Theme::Light);
    harness.state_mut().onboarding_mut().0.open();
    harness.run();
    let capacity = harness.state_mut().onboarding_mut().0.form_mut().password.expose_mut().capacity();
    type_password(&mut harness, &"é".repeat(MAX_PASSWORD_CHARS + 10));
    let password = &mut harness.state_mut().onboarding_mut().0.form_mut().password;
    assert_eq!(password.expose().chars().count(), MAX_PASSWORD_CHARS);
    assert_eq!(password.expose_mut().capacity(), capacity, "typing reallocated the password");
}

#[test]
fn closing_the_window_forgets_the_password_of_every_account_added_in_it() {
    for workers in [FakeWorkers::passing(), FakeWorkers::answering(&[(CheckStep::Tcp, StepStatus::Failed(ErrorKind::MailPortsBlocked))])] {
        let kept = Kept::default();
        let mut harness = harness(app(&kept, workers), egui::Theme::Light);
        for address in ["carol@example.org", "dave@example.org"] {
            harness.state_mut().onboarding_mut().0.open();
            type_in(&mut harness);
            harness.state_mut().onboarding_mut().0.form_mut().address = address.to_owned();
            harness.state_mut().onboarding_mut().0.form_mut().host = "imap.example.org".to_owned();
            harness.run();
            harness.get_by_label("Check and add").click();
            harness.run();
            harness.run();
            let close = if harness.query_by_label("Done").is_some() { "Done" } else { "Back to the accounts" };
            harness.get_by_label(close).click();
            harness.run();
        }
        assert_eq!(kept.passwords.lock().unwrap().len(), 2);
        harness.state_mut().on_exit();
        assert!(kept.passwords.lock().unwrap().is_empty(), "a password outlived the window");
    }
}

#[test]
fn a_password_the_store_did_not_forget_in_change_settings_is_forgotten_at_exit() {
    let failed = FakeWorkers::answering(&[(CheckStep::Tcp, StepStatus::Failed(ErrorKind::MailPortsBlocked))]);
    let kept = Kept::default();
    let mut harness = harness(app(&kept, failed), egui::Theme::Light);
    harness.state_mut().onboarding_mut().0.open();
    type_in(&mut harness);
    harness.state_mut().onboarding_mut().0.form_mut().host = "imap.example.org".to_owned();
    harness.run();
    harness.get_by_label("Check and add").click();
    harness.run();
    harness.run();
    kept.refuse_forget.store(true, Ordering::SeqCst);
    harness.get_by_label("Change settings").click();
    harness.run();
    assert!(harness.state().onboarding().error().unwrap().starts_with("The password could not be removed"));
    assert_eq!(kept.passwords.lock().unwrap().len(), 1);
    kept.refuse_forget.store(false, Ordering::SeqCst);
    kept.forgotten.lock().unwrap().clear();
    harness.state_mut().on_exit();
    assert_eq!(kept.forgotten.lock().unwrap().as_slice(), [1], "the exit tries that account again");
    assert!(kept.passwords.lock().unwrap().is_empty());
}

#[test]
fn the_form_says_what_detect_sends_before_it_is_pressed() {
    let mut harness = harness(app(&Kept::default(), FakeWorkers::passing()), egui::Theme::Light);
    harness.state_mut().onboarding_mut().0.open();
    harness.run();
    harness.get_by_label(
        "Detect asks your name servers and your domain's web server (once without encryption) about your domain; it sends no address, user name or password.",
    );
}
