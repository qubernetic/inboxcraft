// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The add-account page driven through its AccessKit tree.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use egui::accesskit::Role;
use egui::{Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_model::{CheckStep, ErrorKind, StepStatus};
use inboxcraft_time::{FakeClock, Timestamp};

use super::InboxApp;
use crate::mailbox::Mailbox;
use crate::onboard::tests::{FakeWorkers, Kept, PASSWORD, filled, mailbox, proposal};
use crate::onboard::{MAX_PASSWORD_CHARS, Proposed, Secret, Security, Source, Stage, StepState};
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
    let mut harness = Harness::builder().with_size(vec2(1100.0, 900.0)).with_theme(theme).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
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

/// Presses the control called `label`, scrolled into view first, as a person would.
fn press(harness: &mut Harness<'static, InboxApp>, label: &str) {
    harness.get_by_label(label).scroll_to_me();
    harness.run();
    harness.get_by_label(label).click();
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
    harness.get_by_label("Add an account to read its mail here.");
    // Without an account the folder pane would hold only the app's name: it is left out.
    assert_eq!(harness.get_all_by_label("Add account").count(), 1, "on the empty window only");
    assert!(harness.query_by_label("Mailboxes").is_none() && harness.query_all_by_label("InboxCraft").count() <= 1, "no folder pane");
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
    press(&mut harness, "Cancel");
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
fn the_welcome_is_a_box_520_wide_in_the_middle_with_add_account_and_its_arrow() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let harness = harness(app(&Kept::default(), FakeWorkers::passing()), theme);
        let card = harness.get_by_role_and_label(Role::Group, "Welcome").rect();
        assert_eq!(card.width(), 520.0, "{card:?}");
        assert_eq!(card.height(), 294.0, "the frame's box: {card:?}");
        // Right of the rail the window's middle is near x 580 of 1100; the box stands in the middle, on whole points.
        assert!((card.center().x - 580.0).abs() < 40.0 && card.min.x.fract() == 0.0 && card.min.y.fract() == 0.0, "{card:?}");
        let label = harness.get_by_label("No account yet").rect();
        let title = harness.get_by_label("Add an account to read its mail here.").rect();
        let help = harness.get_by_label("Nothing leaves this computer before you do.").rect();
        let add = harness.get_by_role_and_label(Role::Button, "Add account").rect();
        let leads = harness.get_by_label("Opens Settings · Accounts").rect();
        // The empty state's lines 24 in from the edge, 40 under its top, 12 apart; the button 8 under the gap after the
        // help; where it leads 24 above the box's bottom.
        for (name, rect) in [("label", label), ("title", title), ("help", help), ("add", add), ("leads", leads)] {
            assert_eq!(rect.left(), card.left() + 25.0, "{name} {rect:?} in {card:?}");
        }
        assert_eq!(label.top(), card.top() + 41.0);
        assert_eq!(title.top(), label.bottom() + 12.0);
        assert_eq!(help.top(), title.bottom() + 12.0);
        assert_eq!(add.top(), help.bottom() + 20.0);
        assert_eq!(leads.bottom(), card.bottom() - 25.0);
        assert!(texts_in(harness.output(), add).iter().any(|text| text == "→"), "the primary button carries the arrow");
    }
}

/// The texts painted inside `rect` this frame, each galley's text whole.
fn texts_in(output: &egui::FullOutput, rect: egui::Rect) -> Vec<String> {
    fn walk(shape: &egui::Shape, rect: egui::Rect, found: &mut Vec<String>) {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, rect, found)),
            egui::Shape::Text(text) if rect.expand(1.0).contains_rect(text.visual_bounding_rect()) => found.push(text.galley.text().to_owned()),
            _ => {}
        }
    }
    let mut found = Vec::new();
    for clipped in &output.shapes {
        walk(&clipped.shape, rect, &mut found);
    }
    found
}

#[test]
fn a_window_that_cannot_add_says_how_to_look_around_in_the_same_box() {
    let harness = harness_empty();
    let card = harness.get_by_role_and_label(Role::Group, "Welcome").rect();
    assert_eq!((card.width(), card.height()), (520.0, 170.0), "{card:?}");
    harness.get_by_label("This window cannot add an account.");
    harness.get_by_label("Start InboxCraft with --sample to look around an invented mailbox.");
    assert!(harness.query_by_role_and_label(Role::Button, "Add account").is_none());
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
    // After Detect the form ends below the 780-point sheet (its fields and labels follow Plex's taller line): the
    // button is scrolled into view first, as a person would.
    press(&mut harness, "Check and add");
    harness.run();
    assert_eq!(kept.passwords.lock().unwrap().get(&1).map(String::as_str), Some(PASSWORD));
    harness.get_by_label("Checked carol@example.org");
    harness.get_by_label("Signed in. InboxCraft now fetches the folders and the mail.");
    harness.get_by_label("IMAP sign-in: passed");
    harness.get_by_label("Certificate: passed");
    let list = harness.get_by_label("Certificate: passed").rect();
    assert!(texts_in(harness.output(), list).iter().any(|text| text == "certificate · issued by Example Test CA"), "the issuer on its line");
    // And a screen reader hears where each step went and who vouched for the server, as the line shows it.
    let said = harness.get_by_label("imap.example.org:993 · issued by Example Test CA").rect();
    assert!(said.intersects(list), "said on the certificate's line: {said:?} {list:?}");
    let connection = harness.get_by_label("Connection (TCP): passed").rect();
    assert!(harness.get_all_by_label("imap.example.org:993").any(|node| node.rect().intersects(connection)), "the connection's server and port");
    harness.get_by_label("SMTP sign-in: no server");
    harness.check_accessibility();
    press(&mut harness, "Done");
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
    press(&mut harness, "Check and add");
    harness.run();
    // The reason stands under the title, as the design has it.
    let reason = harness.get_by_label(&format!("Connection (TCP) failed: {}", ErrorKind::MailPortsBlocked)).rect();
    let title = harness.get_by_label("Checked carol@example.org").rect();
    assert!(reason.top() >= title.bottom() - 1.0 && reason.top() - title.bottom() < 40.0, "{reason:?} under {title:?}");
    harness.get_by_label("Connection (TCP): failed");
    let line = harness.get_by_label("Connection (TCP): failed").rect();
    assert!(texts_in(harness.output(), line).iter().any(|text| *text == ErrorKind::MailPortsBlocked.to_string()), "the sentence under its line");
    harness.get_by_label("Encryption (TLS): skipped");
    let painted = texts_in(harness.output(), egui::Rect::EVERYTHING);
    for mark in ["[ ok ]", "[fail]", "[skip]"] {
        assert!(painted.iter().any(|text| text == mark), "{mark}");
    }
    harness.check_accessibility();
    press(&mut harness, "Check again");
    harness.get_by_label("Connection (TCP): failed");
    press(&mut harness, "Change settings");
    assert_eq!(harness.state().onboarding().stage(), Stage::Form);
    assert!(kept.passwords.lock().unwrap().is_empty());
    harness.get_by_label("Add an account");
    harness.check_accessibility();
}

#[test]
fn an_accounts_row_opens_its_settings_and_a_new_password_is_saved_and_checked() {
    let kept = Kept::default();
    let mut harness = harness(app(&kept, FakeWorkers::passing()), egui::Theme::Light);
    {
        let (onboarding, mailbox) = harness.state_mut().onboarding_mut();
        onboarding.open();
        filled(onboarding.form_mut());
        onboarding.add(mailbox);
        onboarding.close();
    }
    harness.state_mut().pump();
    harness.state_mut().open_accounts();
    harness.run();
    harness.get_by_label("carol@example.org, carol@example.org").click();
    harness.run();
    assert_eq!(harness.state().onboarding().stage(), Stage::Edit(1));
    harness.get_by_label("Account settings");
    harness.get_by_label("New password");
    harness.get_by_label("CalDAV address, for calendars");
    harness.get_by_label("Save");
    harness.check_accessibility();
    harness.state_mut().onboarding_mut().0.form_mut().password = Secret::new("new-Lantern-9");
    harness.run();
    harness.get_by_label("Save and check").click();
    harness.run();
    harness.run();
    assert_eq!(kept.passwords.lock().unwrap().get(&1).map(String::as_str), Some("new-Lantern-9"));
    assert_eq!(harness.state().onboarding().stage(), Stage::Checking(1));
    harness.get_by_label("Signed in. InboxCraft now fetches the folders and the mail.");
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
        // The IMAP server's row has its own select; the outgoing server's has another.
        press(&mut harness, "IMAP security");
        harness.get_by_role_and_label(Role::MenuItemRadio, "STARTTLS").click();
        harness.run();
        assert_eq!(harness.state().onboarding().form().security, Some(crate::onboard::Security::StartTls));
        assert_eq!(harness.state().onboarding().form().smtp_security, None);
        // The error stands under its field.
        press(&mut harness, "Check and add");
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
    press(&mut harness, "Check and add");
    harness.run();
    press(&mut harness, "Change settings");
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
    press(&mut harness, "Check and add");
    harness.run();
    press(&mut harness, "Done");
    harness.state_mut().onboarding_mut().0.open();
    harness.run();
    harness.state_mut().onboarding_mut().0.form_mut().address = "dave@example.org".to_owned();
    harness.state_mut().onboarding_mut().0.form_mut().host = "imap.example.org".to_owned();
    harness.run();
    undo_in_password(&mut harness);
    assert!(harness.state().onboarding().form().password.is_empty(), "undo restored the first account's password");
    press(&mut harness, "Check and add");
    harness.get_by_label("Enter the password.");
    assert_eq!(kept.passwords.lock().unwrap().get(&2), None, "no password for the second account");
}

#[test]
fn the_password_field_takes_no_more_than_its_limit_and_never_grows_its_buffer() {
    let mut harness = harness(app(&Kept::default(), FakeWorkers::passing()), egui::Theme::Light);
    harness.state_mut().onboarding_mut().0.open();
    harness.run();
    let capacity = harness.state_mut().onboarding_mut().0.form_mut().password.expose_mut().capacity();
    // Past the limit, what was typed is refused whole (never cut) and the field says so.
    type_password(&mut harness, &"é".repeat(MAX_PASSWORD_CHARS + 10));
    assert!(harness.state_mut().onboarding_mut().0.form_mut().password.expose().is_empty(), "refused whole");
    assert!(harness.query_by_label_contains("Too long").is_some(), "the field says why");
    type_password(&mut harness, &"é".repeat(MAX_PASSWORD_CHARS));
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
            press(&mut harness, "Check and add");
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
    press(&mut harness, "Check and add");
    harness.run();
    kept.refuse_forget.store(true, Ordering::SeqCst);
    press(&mut harness, "Change settings");
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

#[test]
fn the_back_link_rings_itself_outside_2_px_clear() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let mut harness = harness(app(&Kept::default(), FakeWorkers::passing()), theme);
        open_form(&mut harness);
        harness.get_by_label("Back to the accounts").focus();
        harness.run();
        let rect = harness.get_by_label("Back to the accounts").rect();
        let mode = if theme == egui::Theme::Dark { crate::theme::Mode::Dark } else { crate::theme::Mode::Light };
        let cyan = crate::theme::Theme::new(mode, crate::theme::TextScale::default()).palette.cyan_edge;
        let strokes = super::tests::strokes_round(harness.output(), cyan, rect);
        let near = |a: egui::Rect, b: egui::Rect| (a.min - b.min).length() < 0.51 && (a.max - b.max).length() < 0.51;
        assert!(
            strokes.iter().any(|(ring, width, whole)| near(*ring, rect.expand(3.0)) && *width == 1.0 && *whole),
            "{theme:?}: {strokes:?} {rect:?}"
        );
        assert!(!strokes.iter().any(|(ring, ..)| rect.contains_rect(*ring)), "{theme:?} no ring inside: {strokes:?} {rect:?}");
    }
}

#[test]
fn the_box_to_ask_mozilla_s_list_ticks_by_pointer_and_by_space() {
    const NAME: &str = "Also ask Mozilla's list of providers (tells Mozilla the domain)";
    let mut harness = harness(app(&Kept::default(), FakeWorkers::passing()), egui::Theme::Light);
    harness.state_mut().onboarding_mut().0.open();
    harness.run();
    let ticked = |harness: &Harness<'static, InboxApp>| harness.state().onboarding().form().ispdb;
    assert!(!ticked(&harness), "it starts off");
    harness.get_by_role_and_label(Role::CheckBox, NAME).click();
    harness.run();
    assert!(ticked(&harness), "a click ticks it");
    assert_eq!(harness.get_by_role_and_label(Role::CheckBox, NAME).accesskit_node().toggled(), Some(egui::accesskit::Toggled::True));
    harness.get_by_role_and_label(Role::CheckBox, NAME).focus();
    harness.run();
    harness.key_press(Key::Space);
    harness.run();
    assert!(!ticked(&harness), "Space takes the tick off");
}

/// Adds an account at `address` through the form, as Add account and Save would.
fn add_account(harness: &mut Harness<'static, InboxApp>, address: &str) {
    let (onboarding, mailbox) = harness.state_mut().onboarding_mut();
    onboarding.open();
    filled(onboarding.form_mut());
    onboarding.form_mut().address = address.to_owned();
    onboarding.add(mailbox);
    onboarding.close();
    harness.state_mut().pump();
}

#[test]
fn the_typed_form_outlasts_every_way_out_of_the_sheet() {
    for way in ["Escape", "Close", "beside", "Appearance", "back", "rail", "palette"] {
        let mut harness = harness(app(&Kept::default(), FakeWorkers::passing()), egui::Theme::Dark);
        open_form(&mut harness);
        type_in(&mut harness);
        match way {
            "Escape" => harness.key_press(Key::Escape),
            "Close" => on_sheet(&harness, "Close").click(),
            "beside" => {
                let sheet = harness.get_by_role_and_label(Role::Dialog, "Settings").rect();
                let at = egui::pos2(sheet.left() / 2.0, sheet.center().y);
                harness.hover_at(at);
                harness.run();
                for pressed in [true, false] {
                    harness.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
                }
            }
            "Appearance" => harness.get_by_label("Appearance, settings page").click(),
            "back" => harness.get_by_label("Back to the accounts").click(),
            "palette" => harness.key_press_modifiers(Modifiers::COMMAND, Key::K),
            _ => harness.state_mut().run_action(&egui::Context::default(), crate::keymap::CALENDAR),
        }
        harness.run();
        assert_eq!(harness.state().onboarding().stage(), Stage::Closed, "{way}: the form is left");
        assert!(harness.state().onboarding().form().password.is_empty(), "{way}: the kept form holds no password");
        if way == "palette" {
            assert!(!harness.state().settings().is_open(), "the palette closed the sheet");
            harness.key_press(Key::Escape);
            harness.run();
        }
        harness.state_mut().open_accounts();
        harness.run();
        on_sheet(&harness, "Add account").click();
        harness.run();
        let onboarding = harness.state().onboarding();
        assert_eq!(onboarding.stage(), Stage::Form, "{way}");
        assert_eq!(onboarding.form().address, "carol@example.org", "{way}: the address as typed");
        assert!(onboarding.form().password.is_empty(), "{way}: the password is dropped on the way out and typed again");
        // Cancel on the form itself lets it go: the next form is empty.
        // As a screen reader presses it: in this window the form's foot is under the sheet's view.
        harness.get_by_label("Cancel").click_accesskit();
        harness.run();
        assert_eq!(harness.state().onboarding().stage(), Stage::Closed, "{way}: Cancel closes the form");
        harness.state_mut().onboarding_mut().0.open();
        assert_eq!(harness.state().onboarding().form().address, "", "{way}: Cancel let it go");
    }
}

/// Leaves the form on the sheet by Escape, then opens the sheet on Accounts again.
fn leave_and_come_back(harness: &mut Harness<'static, InboxApp>) {
    harness.key_press(Key::Escape);
    harness.run();
    assert_eq!(harness.state().onboarding().stage(), Stage::Closed, "the form is left");
    harness.state_mut().open_accounts();
    harness.run();
}

/// Opens the form of the account whose row is `row`, and gives it the name `label`.
fn edit_typed(harness: &mut Harness<'static, InboxApp>, row: &str, id: u64, label: &str) {
    harness.get_by_label(row).click();
    harness.run();
    assert_eq!(harness.state().onboarding().stage(), Stage::Edit(id));
    harness.state_mut().onboarding_mut().0.form_mut().label = label.to_owned();
    harness.run();
}

const CAROL: &str = "carol@example.org, carol@example.org";
const DANA: &str = "dana@example.org, dana@example.org";

#[test]
fn add_account_over_a_kept_edit_opens_the_add_form_and_the_edit_stays_kept() {
    let mut harness = harness(app(&Kept::default(), FakeWorkers::passing()), egui::Theme::Dark);
    add_account(&mut harness, "carol@example.org");
    harness.state_mut().open_accounts();
    harness.run();
    edit_typed(&mut harness, CAROL, 1, "Typed label");
    leave_and_come_back(&mut harness);
    on_sheet(&harness, "Add account").click();
    harness.run();
    let onboarding = harness.state().onboarding();
    assert_eq!(onboarding.stage(), Stage::Form, "the add form asked for opens");
    assert_eq!(onboarding.form().label, "", "an empty add form, not the edit's typing");
    // The edit is not dropped by asking for the add form: its row brings it back as typed.
    harness.get_by_label("Back to the accounts").click();
    harness.run();
    harness.get_by_label(CAROL).click();
    harness.run();
    let onboarding = harness.state().onboarding();
    assert_eq!(onboarding.stage(), Stage::Edit(1));
    assert_eq!(onboarding.form().label, "Typed label", "the kept edit comes back on its own row");
    assert!(onboarding.form().password.is_empty());
}

#[test]
fn an_account_s_row_over_a_kept_add_form_opens_the_edit_and_the_add_form_stays_kept() {
    let mut harness = harness(app(&Kept::default(), FakeWorkers::passing()), egui::Theme::Dark);
    add_account(&mut harness, "carol@example.org");
    harness.state_mut().open_accounts();
    harness.run();
    on_sheet(&harness, "Add account").click();
    harness.run();
    assert_eq!(harness.state().onboarding().stage(), Stage::Form);
    harness.state_mut().onboarding_mut().0.form_mut().address = "dana@example.org".to_owned();
    harness.state_mut().onboarding_mut().0.form_mut().password = Secret::new(PASSWORD);
    leave_and_come_back(&mut harness);
    harness.get_by_label(CAROL).click();
    harness.run();
    let onboarding = harness.state().onboarding();
    assert_eq!(onboarding.stage(), Stage::Edit(1), "carol's form asked for opens");
    assert_eq!(onboarding.form().address, "carol@example.org");
    // The add form is not dropped by asking for carol's: Add account brings it back as typed, without its password.
    harness.get_by_label("Back to the accounts").click();
    harness.run();
    on_sheet(&harness, "Add account").click();
    harness.run();
    let onboarding = harness.state().onboarding();
    assert_eq!(onboarding.stage(), Stage::Form);
    assert_eq!(onboarding.form().address, "dana@example.org", "the kept add form comes back");
    assert!(onboarding.form().password.is_empty(), "its password is typed again");
}

#[test]
fn another_account_s_row_over_a_kept_edit_opens_that_edit_and_both_stay_kept() {
    let mut harness = harness(app(&Kept::default(), FakeWorkers::passing()), egui::Theme::Dark);
    add_account(&mut harness, "carol@example.org");
    add_account(&mut harness, "dana@example.org");
    harness.state_mut().open_accounts();
    harness.run();
    edit_typed(&mut harness, CAROL, 1, "Carol typed");
    leave_and_come_back(&mut harness);
    harness.get_by_label(DANA).click();
    harness.run();
    let onboarding = harness.state().onboarding();
    assert_eq!(onboarding.stage(), Stage::Edit(2), "dana's form asked for opens");
    assert_ne!(onboarding.form().label, "Carol typed", "with dana's own values");
    harness.state_mut().onboarding_mut().0.form_mut().label = "Dana typed".to_owned();
    harness.run();
    // Each edit is kept on its own: carol's row brings carol's typing back, dana's row dana's.
    harness.get_by_label("Back to the accounts").click();
    harness.run();
    harness.get_by_label(CAROL).click();
    harness.run();
    assert_eq!((harness.state().onboarding().stage(), harness.state().onboarding().form().label.as_str()), (Stage::Edit(1), "Carol typed"));
    harness.get_by_label("Back to the accounts").click();
    harness.run();
    harness.get_by_label(DANA).click();
    harness.run();
    assert_eq!((harness.state().onboarding().stage(), harness.state().onboarding().form().label.as_str()), (Stage::Edit(2), "Dana typed"));
    // Cancel lets dana's go; carol's stays.
    harness.get_by_label("Cancel").click_accesskit();
    harness.run();
    harness.state_mut().open_accounts();
    harness.run();
    harness.get_by_label(DANA).click();
    harness.run();
    assert_ne!(harness.state().onboarding().form().label, "Dana typed", "Cancel dropped dana's typing");
    harness.get_by_label("Back to the accounts").click();
    harness.run();
    harness.get_by_label(CAROL).click();
    harness.run();
    assert_eq!(harness.state().onboarding().form().label, "Carol typed", "and kept carol's");
}

#[test]
fn add_account_stands_at_the_right_end_of_the_accounts_title() {
    for accounts in [false, true] {
        let mut harness = harness(app(&Kept::default(), FakeWorkers::passing()), egui::Theme::Dark);
        if accounts {
            add_account(&mut harness, "carol@example.org");
        }
        harness.state_mut().open_accounts();
        harness.run();
        let sheet = harness.get_by_role_and_label(Role::Dialog, "Settings").rect();
        let add = on_sheet(&harness, "Add account").rect();
        let title = harness.get_by_role_and_label(Role::Heading, "Accounts").rect();
        // The page's column ends 40 in from the sheet's edge, its words 16 further in.
        assert!((add.right() - (sheet.right() - 1.0 - 40.0 - 16.0)).abs() <= 0.5, "{accounts}: {add:?} in {sheet:?}");
        assert!((add.center().y - title.center().y).abs() <= 1.0, "{accounts}: on the title's line: {add:?} {title:?}");
    }
}

/// The form scrolls itself to the control `Tab` comes to, as a settings page does: in a window too low for it, every
/// stop is in sight on the sheet, the last ones included.
#[test]
fn tab_through_the_form_in_a_low_window_keeps_each_stop_in_sight() {
    let app = app(&Kept::default(), FakeWorkers::passing());
    let mut harness = Harness::builder().with_size(vec2(1100.0, 600.0)).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    open_form(&mut harness);
    let sheet = harness.get_by_role_and_label(Role::Dialog, "Settings").rect();
    let mut seen = Vec::new();
    for _ in 0..40 {
        harness.key_press(Key::Tab);
        harness.run();
        let focused = harness.root().children_recursive().find(|node| node.accesskit_node().is_focused());
        if let Some(node) = focused {
            let label = node.accesskit_node().label().unwrap_or_default();
            assert!(sheet.contains_rect(node.rect()), "{label} {:?} has the keyboard out of sight of the sheet {sheet:?}", node.rect());
            seen.push(label);
        }
    }
    assert!(seen.iter().any(|label| label == "Check and add"), "Tab reaches the form's foot: {seen:?}");
}

/// The add form open over a window without accounts.
fn form_open(theme: egui::Theme, workers: FakeWorkers) -> Harness<'static, InboxApp> {
    let mut harness = harness(app(&Kept::default(), workers), theme);
    harness.state_mut().onboarding_mut().0.open();
    harness.run();
    harness
}

#[test]
fn a_field_s_error_stands_under_its_field() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let mut harness = form_open(theme, FakeWorkers::passing());
        // A port typed wrong says so at once, 6 under its row, from the row's left; nothing at the page's top.
        harness.state_mut().onboarding_mut().0.form_mut().port = "99999".to_owned();
        harness.run();
        let host = harness.get_by_label("IMAP server").rect();
        let port = harness.get_all_by_label("Port").next().unwrap().rect();
        let error = harness.get_by_label("The port is a number from 1 to 65535.").rect();
        assert_eq!((error.top(), error.left()), (port.bottom() + 6.0, host.left()), "{theme:?}: {error:?} under {port:?}");
        assert!(harness.query_by_label_contains("Error:").is_none(), "{theme:?}: no error banner for a field's error");
        // A CalDAV address the app does not take, under its field.
        harness.state_mut().onboarding_mut().0.form_mut().caldav = "http://dav.example.org/".to_owned();
        harness.run();
        let caldav = harness.get_by_label("CalDAV address, for calendars").rect();
        let said = harness.get_by_label("The CalDAV address starts with https:// and holds no user name.").rect();
        assert_eq!((said.top(), said.left()), (caldav.bottom() + 6.0, caldav.left()), "{theme:?}");
        // What is missing says so under its field when the form is sent, and goes when the field is typed in.
        press(&mut harness, "Check and add");
        let address = harness.get_by_label("Address").rect();
        let missing = harness.get_by_label("Enter the address.").rect();
        assert_eq!((missing.top(), missing.left()), (address.bottom() + 6.0, address.left()), "{theme:?}");
        harness.get_by_label("Address").focus();
        harness.run();
        harness.get_by_label("Address").type_text("carol@example.org");
        harness.run();
        harness.run();
        assert!(harness.query_by_label("Enter the address.").is_none(), "{theme:?}: typing takes the error away");
        assert_eq!(harness.state().onboarding().stage(), Stage::Form);
    }
}

#[test]
fn the_check_shows_steps_up_to_the_running_one() {
    let workers = FakeWorkers::answering(&[(CheckStep::Dns, StepStatus::Passed), (CheckStep::Tcp, StepStatus::Passed)]).unfinished();
    let mut harness = form_open(egui::Theme::Dark, workers);
    type_in(&mut harness);
    harness.state_mut().onboarding_mut().0.form_mut().host = "imap.example.org".to_owned();
    harness.run();
    press(&mut harness, "Check and add");
    harness.run();
    harness.get_by_label("Checking carol@example.org");
    harness.get_by_label("Each step is printed as it starts and gets its answer as it ends.");
    harness.get_by_label("Name lookup (DNS): passed");
    harness.get_by_label("Connection (TCP): passed");
    harness.get_by_label("Encryption (TLS): running");
    for later in ["Certificate", "IMAP sign-in", "SMTP sign-in", "CalDAV calendars"] {
        assert!(harness.query_by_label_contains(later).is_none(), "{later} is not printed before it starts");
    }
    let painted = texts_in(harness.output(), egui::Rect::EVERYTHING);
    assert!(painted.iter().any(|text| text == "[ .. ]") && painted.iter().filter(|text| *text == "[ ok ]").count() == 2, "{painted:?}");
    assert!(harness.query_by_label("Check again").is_none() && harness.query_by_label("Done").is_none(), "no button while it runs");
}

#[test]
fn printed_steps_end_at_the_one_under_way_and_a_check_that_ended_shows_all() {
    let row = |id: &str, state| crate::onboard::StepRow { id: id.to_owned(), state, server: None, issuer: None, message: None };
    let steps = [row("dns", StepState::Passed), row("tcp", StepState::Pending), row("tls", StepState::Pending)];
    let running = super::onboard::printed(&steps, true);
    assert_eq!(
        running.iter().map(|(row, state)| (row.id.as_str(), *state)).collect::<Vec<_>>(),
        [("dns", crate::component::check_step::StepState::Passed), ("tcp", crate::component::check_step::StepState::Running)]
    );
    let ended = super::onboard::printed(&steps, false);
    assert_eq!(ended.len(), 3);
    assert_eq!(ended[2].1, crate::component::check_step::StepState::Waiting);
    assert!(super::onboard::printed(&[], true).is_empty());
}

#[test]
fn no_control_offers_an_unencrypted_sign_in() {
    // There are two encryptions, and no third.
    for security in [Security::Tls, Security::StartTls] {
        match security {
            Security::Tls | Security::StartTls => {}
        }
    }
    let kept = Kept::default();
    let mut harness = harness(app(&kept, FakeWorkers::passing()), egui::Theme::Light);
    {
        let (onboarding, mailbox) = harness.state_mut().onboarding_mut();
        onboarding.open();
        filled(onboarding.form_mut());
        onboarding.add(mailbox);
        onboarding.close();
    }
    harness.state_mut().pump();
    for stage in ["add", "edit"] {
        if stage == "add" {
            harness.state_mut().onboarding_mut().0.open();
        } else {
            let (onboarding, mailbox) = harness.state_mut().onboarding_mut();
            onboarding.edit(mailbox, 1);
        }
        harness.run();
        for select in ["IMAP security", "Outgoing security"] {
            press(&mut harness, select);
            let mut offered: Vec<String> = harness.query_all_by_role(Role::MenuItemRadio).filter_map(|node| node.accesskit_node().label()).collect();
            offered.sort();
            assert_eq!(offered, ["STARTTLS", "TLS"], "{stage}: {select}");
            harness.get_by_role_and_label(Role::MenuItemRadio, "TLS").click();
            harness.run();
        }
        let controls =
            [Role::Button, Role::CheckBox, Role::RadioButton, Role::MenuItemRadio, Role::MenuItem, Role::ComboBox, Role::Link, Role::ListBoxOption];
        for node in harness.root().children_recursive().filter(|node| controls.contains(&node.accesskit_node().role())) {
            let name = node.accesskit_node().label().unwrap_or_default().to_lowercase();
            for way_round in ["unencrypted", "no encryption", "without encryption", "plain", "insecure", "none"] {
                assert!(!name.contains(way_round), "{stage}: {name}");
            }
        }
    }
}

/// Every property of every node of the AccessKit tree, in words.
fn tree(harness: &Harness<'static, InboxApp>) -> String {
    harness.root().children_recursive().map(|node| format!("{:?}", node.accesskit_node().data())).collect::<Vec<_>>().join("\n")
}

#[test]
fn a_password_is_never_in_the_accesskit_tree() {
    let failed = FakeWorkers::answering(&[(CheckStep::Tcp, StepStatus::Failed(ErrorKind::MailPortsBlocked))]);
    let mut harness = form_open(egui::Theme::Dark, failed);
    harness.state_mut().onboarding_mut().0.form_mut().address = "carol@example.org".to_owned();
    harness.state_mut().onboarding_mut().0.form_mut().host = "imap.example.org".to_owned();
    harness.run();
    type_password(&mut harness, PASSWORD);
    assert_eq!(harness.state().onboarding().form().password.expose(), PASSWORD, "typed");
    // Typed, with an error under the field, sent and checked, and back on the form: never in the tree.
    for step in ["typed", "error", "checked", "changed"] {
        match step {
            "error" => {
                harness.state_mut().onboarding_mut().0.form_mut().port = "0".to_owned();
                harness.run();
            }
            "checked" => {
                harness.state_mut().onboarding_mut().0.form_mut().port.clear();
                harness.run();
                press(&mut harness, "Check and add");
                harness.run();
            }
            "changed" => press(&mut harness, "Change settings"),
            _ => {}
        }
        let tree = tree(&harness);
        assert!(!tree.contains(PASSWORD) && !tree.contains("Meadow"), "{step}: the password is in the tree");
        assert!(step == "checked" || tree.contains("Password"), "{step}: the field is there, by name");
    }
}

#[test]
fn hostile_text_never_panics_and_nothing_is_cut_silently() {
    let mut harness = form_open(egui::Theme::Light, FakeWorkers::passing());
    {
        let form = harness.state_mut().onboarding_mut().0.form_mut();
        form.address = format!("{}@example.org", "\u{0}\r\n\t\u{202e}x".repeat(700));
        form.host = format!("{}.example.org", "h".repeat(4000));
        form.label = "é".repeat(3000);
        form.port = "70000".to_owned();
        form.smtp_port = "-1".to_owned();
        form.caldav = format!("https://{}", "d".repeat(4000));
    }
    harness.run();
    assert_eq!(harness.query_all_by_label("The port is a number from 1 to 65535.").count(), 2, "under each server row");
    // A host past what a field takes, typed: refused whole, with the field's sentence.
    harness.state_mut().onboarding_mut().0.form_mut().host.clear();
    harness.run();
    harness.get_by_label("IMAP server").focus();
    harness.run();
    harness.get_by_label("IMAP server").type_text(&"h".repeat(crate::component::field::MAX_LINE + 1));
    harness.run();
    assert!(harness.state().onboarding().form().host.is_empty(), "refused whole");
    assert!(harness.query_by_label_contains("Too long").is_some(), "and said so");
    // An engine's sentence of any length under a failed step: drawn wrapped, whole for a screen reader.
    let long = "the server said no ".repeat(600);
    let row = |id: &str, state, message: Option<&str>| crate::onboard::StepRow {
        id: id.to_owned(),
        state,
        server: Some(format!("{}:993", "h".repeat(3000))),
        issuer: Some("\u{202e}Example\u{0} CA".repeat(100)),
        message: message.map(str::to_owned),
    };
    let steps = vec![row("dns", StepState::Passed, None), row("certificate", StepState::Passed, None), row("tcp", StepState::Failed, Some(&long))];
    // On a page of its own: the form's scroll would keep the top of the check out of sight.
    let mut harness = form_open(egui::Theme::Light, FakeWorkers::passing());
    harness.state_mut().onboarding_mut().0.show_check(1, steps, false);
    harness.run();
    harness.run();
    let whole = format!("Connection (TCP) failed: {}", long.trim_end());
    // A label's words are its value in the tree.
    let labels: Vec<String> = harness
        .query_all_by_label_contains("the server said no")
        .filter_map(|node| node.accesskit_node().label().or_else(|| node.accesskit_node().value()))
        .collect();
    assert!(
        labels.iter().any(|label| label.starts_with(&whole)),
        "{:?}",
        labels.iter().map(|label| label.chars().take(80).collect::<String>()).collect::<Vec<_>>()
    );
}

/// The page in `scene`: `welcome`, `form`, `error` (a port typed wrong), `detected`, `running` (the check at its third
/// step), `failed` or `passed`.
fn scene(name: &str, theme: egui::Theme) -> Harness<'static, InboxApp> {
    let passed = |step| (step, StepStatus::Passed);
    let workers = match name {
        "running" => FakeWorkers::answering(&[passed(CheckStep::Dns), passed(CheckStep::Tcp)]).unfinished(),
        "failed" => FakeWorkers::answering(&[passed(CheckStep::Dns), (CheckStep::Tcp, StepStatus::Failed(ErrorKind::MailPortsBlocked))]),
        _ => FakeWorkers::passing(),
    };
    let mut harness = harness(app(&Kept::default(), workers), theme);
    if name == "welcome" {
        return harness;
    }
    harness.state_mut().onboarding_mut().0.open();
    type_in(&mut harness);
    match name {
        "error" => harness.state_mut().onboarding_mut().0.form_mut().port = "99999".to_owned(),
        "detected" => {
            harness.state_mut().onboarding_mut().0.detect();
            harness.run();
        }
        "running" | "failed" | "passed" => {
            harness.state_mut().onboarding_mut().0.form_mut().host = "imap.example.org".to_owned();
            let (onboarding, mailbox) = harness.state_mut().onboarding_mut();
            onboarding.add(mailbox);
            harness.state_mut().pump();
        }
        _ => {}
    }
    harness.run();
    harness.run();
    harness
}

#[test]
fn every_scene_of_the_page_rests_and_tab_reaches_its_controls() {
    for name in ["welcome", "form", "error", "detected", "running", "failed", "passed"] {
        for reduce in [false, true] {
            let mut harness = scene(name, egui::Theme::Dark);
            harness.state_mut().set_reduce_motion(reduce);
            harness.run();
            assert!(crate::component::tests::rests(&mut harness, 8), "{name}: asks for frames at rest (reduced motion {reduce})");
        }
        let mut harness = scene(name, egui::Theme::Light);
        // The page's own controls: on the sheet, or the welcome's box.
        let place = harness
            .query_by_role_and_label(Role::Dialog, "Settings")
            .or_else(|| harness.query_by_role_and_label(Role::Group, "Welcome"))
            .map(|node| node.rect())
            .unwrap();
        // What the window draws under the sheet (the sheet is wide enough to cover most of it) is not the page's: the
        // controls found in the same place with the sheet closed.
        let beneath: Vec<String> = if harness.query_by_role_and_label(Role::Dialog, "Settings").is_some() {
            let mut bare = scene(name, egui::Theme::Light);
            bare.state_mut().close_settings();
            bare.run();
            bare.root().children_recursive().filter_map(|node| node.accesskit_node().label()).collect()
        } else {
            Vec::new()
        };
        let own: Vec<String> = harness
            .root()
            .children_recursive()
            .filter(|node| {
                let inside = node.accesskit_node().bounding_box().is_some_and(|at| {
                    let at = egui::Rect::from_min_max(egui::pos2(at.x0 as f32, at.y0 as f32), egui::pos2(at.x1 as f32, at.y1 as f32));
                    place.contains_rect(at)
                });
                inside && node.accesskit_node().role() != Role::Label
            })
            .filter(|node| node.accesskit_node().data().supports_action(egui::accesskit::Action::Focus) && !node.accesskit_node().is_disabled())
            .filter_map(|node| node.accesskit_node().label())
            .filter(|label| !beneath.contains(label))
            .collect();
        assert!(!own.is_empty(), "{name}");
        let missed = crate::component::tests::keyboard_walk(&mut harness);
        let missed: Vec<&String> = missed.iter().filter(|label| own.contains(label)).collect();
        assert!(missed.is_empty(), "{name}: Tab never reaches {missed:?}");
    }
}

/// The form with a password typed and the IMAP security's list open over the sheet.
fn list_open(theme: egui::Theme) -> Harness<'static, InboxApp> {
    let mut harness = form_open(theme, FakeWorkers::passing());
    harness.state_mut().onboarding_mut().0.form_mut().password = Secret::new(PASSWORD);
    harness.state_mut().onboarding_mut().0.form_mut().host = "imap.example.org".to_owned();
    harness.run();
    harness.get_by_label("IMAP security").click();
    harness.run();
    harness.run();
    assert!(harness.query_by_label("STARTTLS").is_some(), "the list is open");
    harness
}

#[test]
fn esc_closes_the_open_list_and_only_the_list() {
    for theme in [egui::Theme::Dark, egui::Theme::Light] {
        let mut harness = list_open(theme);
        harness.key_press(Key::Escape);
        harness.run();
        harness.run();
        assert!(harness.query_by_label("STARTTLS").is_none(), "{theme:?}: the list closed");
        assert!(harness.state().settings().is_open(), "{theme:?}: the sheet stays");
        let onboarding = harness.state().onboarding();
        assert_eq!(onboarding.stage(), Stage::Form, "{theme:?}");
        assert!(!onboarding.form().password.is_empty(), "{theme:?}: nothing typed is dropped");
        assert_eq!(onboarding.form().host, "imap.example.org", "{theme:?}");
        // The next Esc is the sheet's.
        harness.key_press(Key::Escape);
        harness.run();
        harness.run();
        assert!(!harness.state().settings().is_open(), "{theme:?}");
        assert_eq!(harness.state().onboarding().stage(), Stage::Closed, "{theme:?}");
    }
}

#[test]
fn a_press_beside_the_sheet_closes_the_open_list_and_only_the_list() {
    let mut harness = list_open(egui::Theme::Dark);
    let sheet = harness.get_by_role_and_label(Role::Dialog, "Settings").rect();
    let beside = egui::pos2(sheet.left() / 2.0, sheet.center().y);
    assert!(!sheet.contains(beside));
    harness.hover_at(beside);
    harness.run();
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton { pos: beside, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
        harness.run();
    }
    harness.run();
    assert!(harness.query_by_label("STARTTLS").is_none(), "the list closed");
    assert!(harness.state().settings().is_open(), "the sheet stays");
    assert_eq!(harness.state().onboarding().stage(), Stage::Form);
    assert!(!harness.state().onboarding().form().password.is_empty(), "nothing typed is dropped");
    // The next press beside is the sheet's.
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton { pos: beside, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
        harness.run();
    }
    assert!(!harness.state().settings().is_open());
}

#[test]
fn detect_s_note_says_what_the_form_now_holds() {
    let mut harness = form_open(egui::Theme::Dark, FakeWorkers::passing());
    let form = harness.state_mut().onboarding_mut().0.form_mut();
    form.address = "carol@example.org".to_owned();
    form.host = "mail.typed.example".to_owned();
    harness.run();
    press(&mut harness, "Detect");
    harness.run();
    assert_eq!(harness.state().onboarding().form().host, "mail.typed.example");
    harness.get_by_label("Detect found imap.example.org; the IMAP server you typed stays.");
    assert!(harness.query_by_label("Found in your provider's settings file.").is_none(), "no server of Detect's is in the form");
}

/// Each way out of the page the window offers, by its name.
fn leave(harness: &mut Harness<'static, InboxApp>, way: &str) {
    match way {
        "Esc" => harness.key_press(Key::Escape),
        "a press beside" => {
            let sheet = harness.get_by_role_and_label(Role::Dialog, "Settings").rect();
            let beside = egui::pos2(sheet.left() / 2.0, sheet.center().y);
            harness.hover_at(beside);
            harness.run();
            for pressed in [true, false] {
                harness.event(egui::Event::PointerButton { pos: beside, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
                harness.run();
            }
        }
        "Back" => harness.get_by_label("Back to the accounts").click(),
        "another section" => harness.get_by_label("Sending, settings page").click(),
        "the calendar" => harness.state_mut().show_calendar(),
        "the settings' first page" => harness.state_mut().open_settings(),
        "the shortcuts" => harness.state_mut().open_shortcuts(),
        _ => panic!("{way}"),
    }
    harness.run();
    harness.run();
}

#[test]
fn no_way_out_keeps_a_password_in_the_forms_it_keeps() {
    let ways = ["Esc", "a press beside", "Back", "another section", "the calendar", "the settings' first page", "the shortcuts"];
    for way in ways {
        // The add form.
        let mut harness = form_open(egui::Theme::Dark, FakeWorkers::passing());
        let form = harness.state_mut().onboarding_mut().0.form_mut();
        form.address = "carol@example.org".to_owned();
        form.password = Secret::new(PASSWORD);
        harness.run();
        leave(&mut harness, way);
        assert_eq!(harness.state().onboarding().stage(), Stage::Closed, "{way}");
        assert_eq!(harness.state().onboarding().kept_forms(), (1, false), "{way}: the add form is kept, without its password");
        // An account's form, changed.
        let (onboarding, mailbox) = harness.state_mut().onboarding_mut();
        onboarding.open();
        crate::onboard::tests::filled(onboarding.form_mut());
        onboarding.add(mailbox);
        assert_eq!(onboarding.stage(), Stage::Checking(1), "{way}: {:?}", onboarding.error());
        onboarding.edit(mailbox, 1);
        onboarding.form_mut().host = "imap.typed.example.org".to_owned();
        onboarding.form_mut().password = Secret::new(PASSWORD);
        harness.run();
        leave(&mut harness, way);
        assert_eq!(harness.state().onboarding().stage(), Stage::Closed, "{way}");
        assert_eq!(harness.state().onboarding().kept_forms(), (1, false), "{way}: the account's form is kept, without its password");
    }
}

#[test]
fn going_from_one_form_to_the_other_keeps_no_password() {
    let kept = Kept::default();
    let (mut mailbox, mut onboarding) =
        (crate::onboard::tests::mailbox(), crate::onboard::Onboarding::new(Arc::new(kept), Box::new(Proposed::default())));
    onboarding.open();
    crate::onboard::tests::filled(onboarding.form_mut());
    onboarding.add(&mut mailbox);
    onboarding.close();
    // The add form, typed in, then an account's row.
    onboarding.open();
    onboarding.form_mut().address = "dave@example.org".to_owned();
    onboarding.form_mut().password = Secret::new(PASSWORD);
    onboarding.edit(&mut mailbox, 1);
    assert_eq!(onboarding.kept_forms(), (1, false), "the add form");
    // The account's form, changed, then Add account.
    onboarding.form_mut().host = "imap.typed.example.org".to_owned();
    onboarding.form_mut().password = Secret::new(PASSWORD);
    onboarding.open();
    assert!(onboarding.form().password.is_empty());
    assert_eq!(onboarding.kept_forms(), (1, false), "the account's form; the add form is open again");
    // And back to the account's form: the add form's password goes with it.
    onboarding.form_mut().password = Secret::new(PASSWORD);
    onboarding.edit(&mut mailbox, 1);
    assert!(onboarding.form().password.is_empty());
    assert_eq!(onboarding.kept_forms(), (1, false));
}

#[test]
fn one_press_names_every_missing_field_and_a_row_says_both_its_errors() {
    for theme in [egui::Theme::Dark, egui::Theme::Light] {
        let mut harness = form_open(theme, FakeWorkers::passing());
        harness.state_mut().onboarding_mut().0.form_mut().port = "99999".to_owned();
        harness.run();
        press(&mut harness, "Check and add");
        for said in ["Enter the address.", "Enter the password.", "Enter the IMAP server, or use Detect.", "The port is a number from 1 to 65535."] {
            assert!(harness.query_by_label(said).is_some(), "{theme:?}: {said}");
        }
        // The host's and the port's errors stand under their row, one under the other.
        let port = harness.get_all_by_label("Port").next().unwrap().rect();
        let host = harness.get_by_label("Enter the IMAP server, or use Detect.").rect();
        let wrong = harness.get_by_label("The port is a number from 1 to 65535.").rect();
        assert_eq!(host.top(), port.bottom() + 6.0, "{theme:?}");
        assert!(wrong.top() >= host.bottom(), "{theme:?}: {wrong:?} under {host:?}");
    }
}

#[test]
fn rows_without_an_error_stand_12_apart() {
    let harness = form_open(egui::Theme::Dark, FakeWorkers::passing());
    let imap = harness.get_by_label("IMAP server").rect();
    let smtp = harness.get_by_label("Outgoing server (SMTP), to send").rect();
    let caldav = harness.get_by_label("CalDAV address, for calendars").rect();
    // In the sheet's 720 column the SMTP host's caption takes a second line of 16 (a gaps line says so).
    assert_eq!((smtp.top() - imap.top(), caldav.top() - smtp.top()), (72.0 + 16.0, 72.0), "a 60-high row and 12 between");
}

#[test]
fn a_refused_press_takes_the_keyboard_to_the_first_faulty_field_which_says_its_error() {
    for theme in [egui::Theme::Dark, egui::Theme::Light] {
        let mut harness = form_open(theme, FakeWorkers::passing());
        press(&mut harness, "Check and add");
        harness.run();
        let address = harness.get_by_label("Address");
        assert!(address.is_focused(), "{theme:?}: the first faulty field has the keyboard");
        assert_eq!(address.accesskit_node().description().as_deref(), Some("Enter the address."), "{theme:?}: and says why");
        assert_eq!(harness.get_by_label("Password").accesskit_node().description().as_deref(), Some("Enter the password."), "{theme:?}");
        // Once the address and the password are there, the IMAP server is the one; scrolled into view.
        let (onboarding, _) = harness.state_mut().onboarding_mut();
        onboarding.form_mut().address = "carol@example.org".to_owned();
        onboarding.form_mut().password = Secret::new(PASSWORD);
        onboarding.edited(crate::onboard::Slot::Address);
        onboarding.edited(crate::onboard::Slot::Password);
        harness.run();
        assert_eq!(harness.get_by_label("Address").accesskit_node().description().as_deref(), None, "{theme:?}: the error went");
        press(&mut harness, "Check and add");
        harness.run();
        assert!(harness.get_by_label("IMAP server").is_focused(), "{theme:?}");
        assert!(!harness.get_by_label("Check and add").is_focused(), "{theme:?}");
        // The keyboard is put there once: Tab moves on from it.
        harness.key_press(Key::Tab);
        harness.run();
        assert!(!harness.get_by_label("IMAP server").is_focused(), "{theme:?}");
    }
}

/// The top of the field `label` with its label over it: 24 over its box (a 16-high label and 8).
fn field_top(harness: &Harness<'static, InboxApp>, label: &str) -> f32 {
    harness.get_by_label(label).rect().top() - 24.0
}

#[test]
fn the_head_and_the_form_are_spaced_as_the_frames_give_them() {
    const HELP: &str = "IMAP for the mail. Detect proposes the servers from the address; nothing is sent until you have seen them.";
    for theme in [egui::Theme::Dark, egui::Theme::Light] {
        // The add form: the title 1 down in a 32-high row, the help 6 under the row, the first field 16 under the help.
        let harness = scene("detected", theme);
        let title = harness.get_by_label("Add an account").rect();
        let help = harness.get_by_label(HELP).rect();
        assert_eq!(help.top() - title.top(), 37.0, "{theme:?}: title to help");
        assert_eq!(field_top(&harness, "Address") - help.bottom(), 16.0, "{theme:?}: help to the form");
        // The column stands 40 right of the form's fields (600 from their left at the form's full 560): the form's 16 at
        // its right, and 24.
        let column = harness.get_by_label_contains("What Detect asked").rect();
        let detect = harness.get_by_label("Detect").rect();
        assert_eq!(column.left() - detect.right(), 40.0, "{theme:?}: the column");
        assert_eq!(column.top(), field_top(&harness, "Address"), "{theme:?}: the column's label beside the address's");
        // The check: the sentence 16 under the title's row, 17 under its text.
        let harness = scene("running", theme);
        let title = harness.get_by_label("Checking carol@example.org").rect();
        let sentence = harness.get_by_label("Each step is printed as it starts and gets its answer as it ends.").rect();
        assert_eq!(sentence.top() - title.top(), 47.0, "{theme:?}: title to sentence");
        assert_eq!(sentence.top() - title.bottom(), 17.0, "{theme:?}");
    }
}

#[test]
fn an_account_s_form_starts_16_under_its_title_s_row() {
    let mut harness = form_open(egui::Theme::Dark, FakeWorkers::passing());
    let (onboarding, mailbox) = harness.state_mut().onboarding_mut();
    crate::onboard::tests::filled(onboarding.form_mut());
    onboarding.add(mailbox);
    onboarding.edit(mailbox, 1);
    harness.run();
    harness.run();
    let title = harness.get_by_label("Account settings").rect();
    assert_eq!(field_top(&harness, "Address") - title.top(), 47.0, "1 down in a 32-high row, then 16");
}

/// The add form on a sheet that makes itself the modal layer every frame, as the settings sheet may, with a password
/// typed; the IMAP security's list open when `list` is on.
fn modal_sheet(list: bool) -> Harness<'static, InboxApp> {
    let sheet = egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("settings-sheet"));
    let app = app(&Kept::default(), FakeWorkers::passing());
    let mut harness = Harness::builder().with_size(vec2(1100.0, 900.0)).build_ui_state(
        move |ui, app: &mut InboxApp| {
            app.ui(ui);
            if app.settings().is_open() {
                ui.ctx().memory_mut(|memory| memory.set_modal_layer(sheet));
            }
        },
        app,
    );
    harness.run();
    harness.state_mut().onboarding_mut().0.open();
    harness.run();
    harness.state_mut().onboarding_mut().0.form_mut().password = Secret::new(PASSWORD);
    harness.run();
    harness.run();
    if list {
        harness.get_by_label("IMAP security").click();
        harness.run();
        harness.run();
        assert!(harness.query_by_label("STARTTLS").is_some(), "the list is open");
    }
    harness
}

/// A press of the pointer beside the sheet, down and up.
fn press_beside(harness: &mut Harness<'static, InboxApp>) {
    let sheet = harness.get_by_role_and_label(Role::Dialog, "Settings").rect();
    let beside = egui::pos2(sheet.left() / 2.0, sheet.center().y);
    harness.hover_at(beside);
    harness.run();
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton { pos: beside, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
        harness.run();
    }
    harness.run();
}

#[test]
fn a_modal_sheet_closes_on_esc_and_on_a_press_beside_when_no_list_is_open() {
    let mut harness = modal_sheet(false);
    harness.key_press(Key::Escape);
    harness.run();
    harness.run();
    assert!(!harness.state().settings().is_open(), "Esc closes the sheet");
    let mut harness = modal_sheet(false);
    press_beside(&mut harness);
    assert!(!harness.state().settings().is_open(), "a press beside closes the sheet");
}

#[test]
fn on_a_modal_sheet_esc_and_a_press_beside_close_only_the_open_list() {
    let mut harness = modal_sheet(true);
    harness.key_press(Key::Escape);
    harness.run();
    harness.run();
    assert!(harness.query_by_label("STARTTLS").is_none(), "Esc closed the list");
    assert!(harness.state().settings().is_open(), "and only the list");
    assert!(!harness.state().onboarding().form().password.is_empty());
    let mut harness = modal_sheet(true);
    press_beside(&mut harness);
    assert!(harness.query_by_label("STARTTLS").is_none(), "a press beside closed the list");
    assert!(harness.state().settings().is_open(), "and only the list");
}

#[test]
fn a_line_under_the_select_says_detect_replaced_the_picked_encryption() {
    for theme in [egui::Theme::Dark, egui::Theme::Light] {
        let mut harness = form_open(theme, FakeWorkers::passing());
        let form = harness.state_mut().onboarding_mut().0.form_mut();
        form.address = "carol@example.org".to_owned();
        form.security = Some(Security::StartTls);
        harness.run();
        press(&mut harness, "Detect");
        harness.run();
        const SAID: &str = "Detect set the encryption to TLS for port 993.";
        let said = harness.get_by_label(SAID).rect();
        let select = harness.get_by_label("IMAP security").rect();
        assert_eq!(said.top(), select.bottom() + 6.0, "{theme:?}: under the row");
        // Picked again, the line goes.
        harness.state_mut().onboarding_mut().0.form_mut().security = Some(Security::StartTls);
        harness.run();
        assert!(harness.query_by_label(SAID).is_none(), "{theme:?}");
    }
}

#[test]
fn two_steps_of_the_same_id_each_say_where_they_went() {
    let mut harness = form_open(egui::Theme::Dark, FakeWorkers::passing());
    let row = |server: &str| crate::onboard::StepRow {
        id: "tcp".to_owned(),
        state: StepState::Passed,
        server: Some(server.to_owned()),
        issuer: None,
        message: None,
    };
    harness.state_mut().onboarding_mut().0.show_check(1, vec![row("a.example.org:993"), row("b.example.org:993")], false);
    harness.run();
    harness.run();
    harness.get_by_label("a.example.org:993");
    harness.get_by_label("b.example.org:993");
}
