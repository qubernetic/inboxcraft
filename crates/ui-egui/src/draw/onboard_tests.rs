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
use super::tests::drew_icon;
use crate::iconset::IconId;
use crate::mailbox::Mailbox;
use crate::onboard::tests::{FakeWorkers, Kept, PASSWORD, filled, mailbox, proposal};
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
fn the_empty_window_is_one_card_in_the_middle() {
    let harness = harness(app(&Kept::default(), FakeWorkers::passing()), egui::Theme::Light);
    let title = harness.get_by_label("No accounts yet").rect();
    let add = harness.get_by_role_and_label(Role::Button, "Add account").rect();
    harness.get_by_label("Opens Settings · Accounts");
    assert!((add.center().x - title.center().x).abs() < 2.0, "{add:?} under {title:?}");
    // The window is 1100 by 760; right of the rail its middle is near x 580, and the card stands in the middle.
    assert!((500.0..660.0).contains(&title.center().x), "{title:?}");
    assert!(title.top() > 150.0 && add.bottom() < 610.0, "{title:?} {add:?}");
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
    harness.get_by_label("Check and add").scroll_to_me();
    harness.run();
    harness.get_by_label("Check and add").click();
    harness.run();
    harness.run();
    assert_eq!(kept.passwords.lock().unwrap().get(&1).map(String::as_str), Some(PASSWORD));
    harness.get_by_label("Checking carol@example.org");
    harness.get_by_label("Signed in. InboxCraft now fetches the folders and the mail.");
    harness.get_by_label("IMAP sign-in: passed");
    harness.get_by_label("Certificate: passed");
    harness.get_by_label("imap.example.org:993 · issued by Example Test CA");
    harness.get_by_label("SMTP sign-in: no server set");
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
    for id in [IconId::Done, IconId::Failed, IconId::Skipped] {
        assert!(drew_icon(harness.output(), id), "{id:?}");
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
        // The IMAP server's row comes first; the outgoing server's has its own pair.
        harness.get_all_by_label("STARTTLS").next().unwrap().click();
        harness.run();
        assert_eq!(harness.state().onboarding().form().security, Some(crate::onboard::Security::StartTls));
        assert_eq!(harness.state().onboarding().form().smtp_security, None);
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
        assert!(harness.state().onboarding().form().password.is_empty(), "{way}: the parked form holds no password");
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
        harness.state_mut().onboarding_mut().0.close();
        harness.state_mut().onboarding_mut().0.open();
        assert_eq!(harness.state().onboarding().form().address, "", "{way}");
    }
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

/// Leaves the form on the sheet by Escape, then opens the sheet on Accounts again.
fn leave_and_come_back(harness: &mut Harness<'static, InboxApp>) {
    harness.key_press(Key::Escape);
    harness.run();
    assert_eq!(harness.state().onboarding().stage(), Stage::Closed, "the form is left");
    harness.state_mut().open_accounts();
    harness.run();
}

#[test]
fn add_account_over_a_parked_edit_brings_the_edit_back_as_typed() {
    let mut harness = harness(app(&Kept::default(), FakeWorkers::passing()), egui::Theme::Dark);
    add_account(&mut harness, "carol@example.org");
    harness.state_mut().open_accounts();
    harness.run();
    harness.get_by_label("carol@example.org, carol@example.org").click();
    harness.run();
    assert_eq!(harness.state().onboarding().stage(), Stage::Edit(1));
    harness.state_mut().onboarding_mut().0.form_mut().label = "Typed label".to_owned();
    leave_and_come_back(&mut harness);
    on_sheet(&harness, "Add account").click();
    harness.run();
    let onboarding = harness.state().onboarding();
    assert_eq!(onboarding.stage(), Stage::Edit(1), "the parked edit comes back, not an empty add form");
    assert_eq!(onboarding.form().label, "Typed label");
}

#[test]
fn an_account_s_row_over_a_parked_add_form_brings_the_add_form_back_as_typed() {
    let mut harness = harness(app(&Kept::default(), FakeWorkers::passing()), egui::Theme::Dark);
    add_account(&mut harness, "carol@example.org");
    harness.state_mut().open_accounts();
    harness.run();
    on_sheet(&harness, "Add account").click();
    harness.run();
    assert_eq!(harness.state().onboarding().stage(), Stage::Form);
    harness.state_mut().onboarding_mut().0.form_mut().address = "dana@example.org".to_owned();
    leave_and_come_back(&mut harness);
    harness.get_by_label("carol@example.org, carol@example.org").click();
    harness.run();
    let onboarding = harness.state().onboarding();
    assert_eq!(onboarding.stage(), Stage::Form, "the parked add form comes back, not carol's edit");
    assert_eq!(onboarding.form().address, "dana@example.org");
}

#[test]
fn another_account_s_row_over_a_parked_edit_brings_that_edit_back_as_typed() {
    let mut harness = harness(app(&Kept::default(), FakeWorkers::passing()), egui::Theme::Dark);
    add_account(&mut harness, "carol@example.org");
    add_account(&mut harness, "dana@example.org");
    harness.state_mut().open_accounts();
    harness.run();
    harness.get_by_label("carol@example.org, carol@example.org").click();
    harness.run();
    assert_eq!(harness.state().onboarding().stage(), Stage::Edit(1));
    harness.state_mut().onboarding_mut().0.form_mut().label = "Typed label".to_owned();
    leave_and_come_back(&mut harness);
    harness.get_by_label("dana@example.org, dana@example.org").click();
    harness.run();
    let onboarding = harness.state().onboarding();
    assert_eq!(onboarding.stage(), Stage::Edit(1), "carol's parked edit comes back, not dana's");
    assert_eq!(onboarding.form().label, "Typed label");
    // Cancelled on purpose, the other account's form opens.
    harness.state_mut().onboarding_mut().0.close();
    harness.state_mut().open_accounts();
    harness.run();
    harness.get_by_label("dana@example.org, dana@example.org").click();
    harness.run();
    assert_eq!(harness.state().onboarding().stage(), Stage::Edit(2));
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

/// The line over a form that came back in place of the one asked for.
const INSTEAD: &str = "You left this form unfinished, so it came back in place of the one you asked for: Cancel it to open that one.";

#[test]
fn a_form_that_comes_back_in_place_of_the_one_asked_for_says_so() {
    let mut harness = harness(app(&Kept::default(), FakeWorkers::passing()), egui::Theme::Dark);
    add_account(&mut harness, "carol@example.org");
    add_account(&mut harness, "dana@example.org");
    harness.state_mut().open_accounts();
    harness.run();
    // Carol's edit, left and asked for again: the same form, nothing to say.
    harness.get_by_label("carol@example.org, carol@example.org").click();
    harness.run();
    assert!(harness.query_by_label(INSTEAD).is_none(), "a form asked for says nothing");
    leave_and_come_back(&mut harness);
    harness.get_by_label("carol@example.org, carol@example.org").click();
    harness.run();
    assert_eq!(harness.state().onboarding().stage(), Stage::Edit(1));
    assert!(harness.query_by_label(INSTEAD).is_none(), "the form asked for came back: nothing to say");
    // Dana's row, and Add account, over carol's left edit: carol's comes back and says why.
    for ask in ["dana@example.org, dana@example.org", "Add account"] {
        leave_and_come_back(&mut harness);
        if ask == "Add account" {
            on_sheet(&harness, ask).click()
        } else {
            harness.get_by_label(ask).click()
        }
        harness.run();
        assert_eq!(harness.state().onboarding().stage(), Stage::Edit(1), "{ask}");
        harness.get_by_label(INSTEAD);
    }
    // Cancel lets it go; then the form asked for opens, and says nothing.
    harness.get_by_label("Cancel").click();
    harness.run();
    on_sheet(&harness, "Add account").click();
    harness.run();
    assert_eq!(harness.state().onboarding().stage(), Stage::Form);
    assert!(harness.query_by_label(INSTEAD).is_none(), "the add form asked for says nothing");
    // The add form left, and carol's row asked for: the add form comes back and says why.
    leave_and_come_back(&mut harness);
    harness.get_by_label("carol@example.org, carol@example.org").click();
    harness.run();
    assert_eq!(harness.state().onboarding().stage(), Stage::Form);
    harness.get_by_label(INSTEAD);
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
