// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The appearance settings page driven through its AccessKit tree: every control runs its `ui.*.set` command.

use egui::accesskit::{Role, Toggled};
use egui::{Key, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_engine::appearance::{Motion, ThemeMode, WindowFrame};
use inboxcraft_time::{FakeClock, Timestamp};

use super::InboxApp;
use crate::mailbox::Mailbox;
use crate::sample;
use crate::settings::Section;
use crate::theme::Mode;

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

fn harness() -> Harness<'static, InboxApp> {
    let app = InboxApp::at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    let mut harness =
        Harness::builder().with_size(vec2(1200.0, 820.0)).with_theme(egui::Theme::Light).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

fn opened() -> Harness<'static, InboxApp> {
    let mut harness = harness();
    super::tests::at_settings(harness.state_mut(), crate::settings::Section::Appearance);
    harness.run();
    assert!(harness.state().settings().is_open());
    harness
}

/// Presses the button called `label` and returns the id of the command the press ran.
fn press(harness: &mut Harness<'static, InboxApp>, label: &str) -> &'static str {
    // A button, or a segmented control's option.
    harness.query_by_role_and_label(Role::RadioButton, label).unwrap_or_else(|| harness.get_by_role_and_label(Role::Button, label)).click();
    harness.run();
    harness.state().mailbox().last_run().unwrap_or("nothing")
}

fn selected(harness: &Harness<'static, InboxApp>, label: &str) -> bool {
    let node = harness.query_by_role_and_label(Role::RadioButton, label).unwrap_or_else(|| harness.get_by_role_and_label(Role::Button, label));
    node.accesskit_node().toggled() == Some(Toggled::True)
}

#[test]
fn the_settings_take_the_body_beside_the_index_and_their_row_again_shows_the_mail() {
    let mut harness = opened();
    let place = harness.get_by_role_and_label(Role::Pane, "Settings").rect();
    harness.get_by_label("Appearance");
    // Every section opens; those not built yet show their page as coming soon.
    assert!(!harness.get_by_label("Calendar, settings page").accesskit_node().is_disabled());
    assert!(!harness.get_by_label("Accounts, settings page").accesskit_node().is_disabled());
    assert!(!harness.get_by_label("Appearance, settings page").accesskit_node().is_disabled());
    // The settings stand in the mail's place: no list, no reader; the index beside them.
    assert!(harness.query_all_by_label_contains("Draft logo for the bakery").next().is_none(), "the list is not drawn");
    let index = harness.get_by_role_and_label(Role::Button, "Settings").rect();
    assert!(index.right() <= place.left(), "the index {index:?} beside the settings {place:?}");
    assert!(harness.query_by_role_and_label(Role::Button, "Close").is_none(), "a place has no Close");
    // Esc closes no place; the index's SETTINGS again shows the mail.
    harness.key_press(Key::Escape);
    harness.run();
    assert!(harness.state().settings().is_open(), "Esc closes no place");
    harness.get_by_role_and_label(Role::Button, "Settings").click();
    harness.run();
    assert!(!harness.state().settings().is_open());
    assert!(harness.query_all_by_label_contains("Draft logo for the bakery").count() > 0, "the list is back");
}

#[test]
fn every_control_runs_its_command() {
    let mut harness = opened();
    assert!(selected(&harness, "Auto"));
    assert_eq!(press(&mut harness, "Dark"), "ui.theme.set");
    assert_eq!(harness.state().appearance().theme, ThemeMode::Dark);
    assert_eq!(Mode::of(&harness.ctx), Mode::Dark);
    assert!(selected(&harness, "Dark"));

    assert!(harness.query_by_label("Aurora").is_none(), "one palette: no accent to choose");

    // The slider: a press in its middle sets 110 %, the right arrow a step more.
    harness.get_by_role_and_label(Role::Slider, "Text size").click();
    harness.run();
    assert_eq!(harness.state().mailbox().last_run(), Some("ui.text_scale.set"));
    assert_eq!(harness.state().appearance().text_scale, 110);
    harness.key_press(Key::ArrowRight);
    harness.run();
    assert_eq!(harness.state().appearance().text_scale, 120);
    assert_eq!(harness.get_by_role_and_label(Role::Slider, "Text size").accesskit_node().numeric_value(), Some(120.0));

    // Motion, the design's three levels: Full is not built yet; Reduced is today's motion, Off today's reduced one.
    let level = |harness: &Harness<'static, InboxApp>, name: &str| harness.get_by_role_and_label(Role::RadioButton, name).accesskit_node().toggled();
    assert!(harness.get_by_role_and_label(Role::RadioButton, "Full for Motion").accesskit_node().is_disabled(), "Full comes later");
    assert_eq!(level(&harness, "Reduced for Motion"), Some(Toggled::True), "until the owner chooses, the motion that runs");
    // Below the fold at 120 %: pressed the way a screen reader presses it.
    harness.get_by_role_and_label(Role::RadioButton, "Off for Motion").click_accesskit();
    harness.run();
    assert_eq!(harness.state().mailbox().last_run(), Some("ui.motion.set"));
    assert_eq!(harness.state().appearance().motion, Motion::Reduce);
    assert_eq!(level(&harness, "Off for Motion"), Some(Toggled::True));
    harness.get_by_role_and_label(Role::RadioButton, "Reduced for Motion").click_accesskit();
    harness.run();
    assert_eq!(harness.state().appearance().motion, Motion::Full);
    assert!(harness.query_by_label("Custom").is_none());
}

#[test]
fn the_appearance_page_has_no_accent_typeface_or_icon_choice() {
    let harness = opened();
    for gone in ["Accent", "Custom", "Typeface", "Icons"] {
        assert!(harness.query_by_label(gone).is_none(), "{gone}");
    }
    for kept in ["Mode", "Text size", "Motion", "Window frame"] {
        assert!(harness.query_all_by_label_contains(kept).count() > 0, "{kept}");
    }
    // The mode reads Auto, Dark, Light, left to right; Auto is the system's.
    let x = |label: &str| harness.get_by_role_and_label(Role::RadioButton, label).rect().center().x;
    assert!(x("Auto") < x("Dark") && x("Dark") < x("Light"));
    assert!(harness.query_by_label("Auto / Dark / Light").is_some(), "the mode's Auto is not called System");
}

#[test]
fn the_add_account_form_forces_no_page_and_another_page_keeps_it_without_its_password() {
    let mut harness = opened();
    harness.state_mut().onboarding_mut().0.open();
    harness.state_mut().onboarding_mut().0.form_mut().password = crate::onboard::Secret::new("hunter2");
    harness.run();
    assert_eq!(harness.state().settings().section(), Section::Appearance, "nothing forces the Accounts page");
    assert!(harness.state().onboarding().form().password.is_empty(), "the form's page does not show: no password kept");
    harness.state_mut().open_accounts();
    harness.run();
    assert_eq!(harness.state().onboarding().stage(), crate::onboard::Stage::Form, "the form shows on Accounts as it was");
    harness.state_mut().onboarding_mut().0.form_mut().password = crate::onboard::Secret::new("hunter2");
    harness.get_by_label("Appearance, settings page").click();
    harness.run();
    assert_eq!(harness.state().onboarding().stage(), crate::onboard::Stage::Form, "another page keeps the form");
    assert!(harness.state().onboarding().form().password.is_empty(), "and drops its password");
}

#[test]
fn the_accounts_page_lists_every_account_with_its_address_and_the_nav_goes_back() {
    let mut harness = opened();
    assert!(selected(&harness, "Appearance, settings page"));
    harness.get_by_label("Accounts, settings page").click();
    harness.run();
    assert_eq!(harness.state().settings().section(), Section::Accounts);
    assert!(selected(&harness, "Accounts, settings page"));
    harness.get_by_label("[ A ] Your accounts · 2");
    harness.get_by_label("The mail accounts this computer reads and sends from.");
    let accounts = harness.state().mailbox().accounts().to_vec();
    assert!(!accounts.is_empty());
    for account in &accounts {
        harness.get_by_label(&format!("{}, {}", account.label, account.address));
    }
    assert!(harness.query_all_by_label_contains("Mode. ").next().is_none(), "Appearance's rows are gone");
    // The sample window adds no account: it offers none.
    assert_eq!(harness.query_all_by_label("Add account").count(), 0);
    harness.check_accessibility();
    harness.get_by_label("Appearance, settings page").click();
    harness.run();
    assert_eq!(harness.state().settings().section(), Section::Appearance);
    assert!(harness.query_all_by_label_contains("Mode. ").next().is_some());
}

#[test]
fn a_window_that_adds_accounts_offers_add_account_above_the_list() {
    let app = InboxApp::at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock())).with_accounts(
        std::sync::Arc::new(crate::onboard::NoKeychain),
        Box::<crate::onboard::NoDetect>::default(),
        Box::new(crate::workers::NoWorkers),
    );
    let mut harness =
        Harness::builder().with_size(vec2(1200.0, 820.0)).with_theme(egui::Theme::Dark).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness.state_mut().open_accounts();
    harness.run();
    let add = harness.get_by_role_and_label(Role::Button, "Add account").rect();
    let first = harness.state().mailbox().accounts().first().map(|a| format!("{}, {}", a.label, a.address)).unwrap();
    assert!(add.bottom() <= harness.get_by_label(&first).rect().top(), "Add account heads the list");
    harness.check_accessibility();
    harness.get_by_role_and_label(Role::Button, "Add account").click();
    harness.run();
    assert_eq!(harness.state().onboarding().stage(), crate::onboard::Stage::Form);
    harness.get_by_label("Add an account");
}

#[test]
fn the_window_frame_row_runs_ui_frame_set() {
    // Tall enough that the last row of the page is in view.
    let app = InboxApp::at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    let mut harness =
        Harness::builder().with_size(vec2(1200.0, 1400.0)).with_theme(egui::Theme::Light).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness.get_by_role_and_label(egui::accesskit::Role::Button, "Settings").click();
    harness.run();
    // The design's two frames: InboxCraft's own (the strip by default) and the desktop's.
    assert!(selected(&harness, "Own"), "the app's own frame by default");
    assert_eq!(press(&mut harness, "System"), "ui.frame.set");
    assert_eq!(harness.state().appearance().frame, WindowFrame::System);
    assert_eq!(press(&mut harness, "Own"), "ui.frame.set");
    assert_eq!(harness.state().appearance().frame, WindowFrame::Strip, "Own brings back the strip, the own frame last on");
    // With the top row on (set by its command), the desktop's frame and back bring back the top row: no own frame a
    // person had is out of reach.
    harness.state_mut().set_frame(WindowFrame::TopRow);
    harness.run();
    assert!(selected(&harness, "Own"), "the top row is the app's own frame");
    assert_eq!(press(&mut harness, "System"), "ui.frame.set");
    assert_eq!(press(&mut harness, "Own"), "ui.frame.set");
    assert_eq!(harness.state().appearance().frame, WindowFrame::TopRow, "Own brings back the top row, the own frame last on");
}

#[test]
fn the_sending_page_saves_each_accounts_signature_for_its_new_messages() {
    let mut harness = opened();
    harness.get_by_label("Sending, settings page").click();
    harness.run();
    assert_eq!(harness.state().settings().section(), Section::Sending);
    assert!(harness.get_by_role_and_label(Role::Button, "Save the signature of Personal").accesskit_node().is_disabled(), "nothing typed");
    let field = harness.get_by_role_and_label(Role::MultilineTextInput, "Signature of Personal");
    field.focus();
    harness.run();
    harness.get_by_role_and_label(Role::MultilineTextInput, "Signature of Personal").type_text("Mira\nPlot 14\n");
    harness.run();
    press(&mut harness, "Save the signature of Personal");
    assert_eq!(
        harness.state().mailbox().account(1).map(|account| account.signature.as_str()),
        Some("Mira\nPlot 14"),
        "kept without the line after it"
    );
    assert!(harness.get_by_role_and_label(Role::Button, "Save the signature of Personal").accesskit_node().is_disabled(), "saved");
    let draft = harness.state_mut().mailbox_mut().run("compose.new", &serde_json::json!({ "account": 1 })).unwrap();
    assert_eq!(draft["text"], serde_json::json!("\n\n-- \nMira\nPlot 14"));
    harness.get_by_role_and_label(Role::MultilineTextInput, "Signature of Personal").focus();
    harness.run();
    for _ in 0.."Mira\nPlot 14".chars().count() {
        harness.key_press(Key::Backspace);
    }
    harness.run();
    press(&mut harness, "Save the signature of Personal");
    assert_eq!(harness.state().mailbox().account(1).map(|account| account.signature.as_str()), Some(""), "an empty field clears it");
}

#[test]
fn a_signature_past_its_2000_characters_is_refused_whole_and_said() {
    let most = inboxcraft_model::Account::MAX_SIGNATURE_CHARS;
    let mut harness = opened();
    harness.get_by_label("Sending, settings page").click();
    harness.run();
    let field = || (Role::MultilineTextInput, "Signature of Personal");
    harness.get_by_role_and_label(field().0, field().1).focus();
    harness.run();
    harness.key_press_modifiers(egui::Modifiers::COMMAND, Key::A);
    harness.run();
    harness.key_press(Key::Backspace);
    harness.run();
    // Two bytes a character: within the field's bytes, past the model's characters.
    harness.event(egui::Event::Paste("é".repeat(most + 1)));
    harness.run();
    assert_eq!(harness.state().settings().signature(1), Some(""), "refused whole, nothing cut in");
    let said = harness.query_all(egui_kittest::kittest::by().label_contains("Too long")).count();
    assert_eq!(said, 1, "the field says why");
    harness.event(egui::Event::Paste("é".repeat(most)));
    harness.run();
    assert_eq!(harness.state().settings().signature(1).map(|typed| typed.chars().count()), Some(most), "exactly the cap goes in");
}

#[test]
fn a_middle_click_paste_cannot_cut_a_signature_silently() {
    let most = inboxcraft_model::Account::MAX_SIGNATURE_CHARS;
    let mut harness = opened();
    harness.get_by_label("Sending, settings page").click();
    harness.run();
    let field = || (Role::MultilineTextInput, "Signature of Personal");
    harness.get_by_role_and_label(field().0, field().1).focus();
    harness.run();
    harness.key_press_modifiers(egui::Modifiers::COMMAND, Key::A);
    harness.run();
    harness.key_press(Key::Backspace);
    harness.run();
    let own = "a".repeat(most - 10);
    harness.event(egui::Event::Paste(own.clone()));
    harness.run();
    assert_eq!(harness.state().settings().signature(1), Some(own.as_str()));
    // The field lets go of the keyboard; the PRIMARY selection is pasted where the middle button goes down, at its start.
    harness.ctx.memory_mut(|memory| {
        if let Some(focused) = memory.focused() {
            memory.surrender_focus(focused);
        }
    });
    harness.run();
    assert!(!harness.get_by_role_and_label(field().0, field().1).is_focused());
    let rect = harness.get_by_role_and_label(field().0, field().1).rect();
    let at = egui::pos2(rect.left() + 2.0, rect.top().max(0.0) + 8.0);
    harness.hover_at(at);
    harness.run();
    let button = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Middle, pressed, modifiers: egui::Modifiers::NONE };
    harness.input_mut().events.extend([button(true), egui::Event::MiddleClickPaste { pos: at, text: "é".repeat(20) }, button(false)]);
    harness.step();
    harness.run();
    assert_eq!(harness.state().settings().signature(1), Some(own.as_str()), "refused whole, nothing of the person's text cut");
    let said = harness.query_all(egui_kittest::kittest::by().label_contains("Too long")).count();
    assert_eq!(said, 1, "the field says why");
}
