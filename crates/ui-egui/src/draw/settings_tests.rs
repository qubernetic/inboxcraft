// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The appearance settings page driven through its AccessKit tree: every control runs its `ui.*.set` command.

use egui::accesskit::{Role, Toggled};
use egui::{Key, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_engine::appearance::{Accent, Icons, Motion, ThemeMode, Typeface, WindowFrame};
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
    let app = InboxApp::new(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    let mut harness =
        Harness::builder().with_size(vec2(1200.0, 820.0)).with_theme(egui::Theme::Light).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

fn opened() -> Harness<'static, InboxApp> {
    let mut harness = harness();
    harness.get_by_label("Appearance settings").click();
    harness.run();
    assert!(harness.state().settings().is_open());
    harness
}

/// Presses the button called `label` and returns the id of the command the press ran.
fn press(harness: &mut Harness<'static, InboxApp>, label: &str) -> &'static str {
    harness.get_by_role_and_label(Role::Button, label).click();
    harness.run();
    harness.state().mailbox().last_run().unwrap_or("nothing")
}

/// Moves the pointer to `at`, then presses and releases the primary button there.
fn press_at(harness: &mut Harness<'static, InboxApp>, at: egui::Pos2) {
    harness.hover_at(at);
    harness.run();
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
        harness.run();
    }
}

fn selected(harness: &Harness<'static, InboxApp>, label: &str) -> bool {
    harness.get_by_role_and_label(Role::Button, label).accesskit_node().toggled() == Some(Toggled::True)
}

#[test]
fn the_sheet_opens_over_the_dimmed_window_a_press_beside_it_or_close_or_escape_leave_it() {
    let mut harness = opened();
    harness.get_by_role_and_label(Role::Dialog, "Settings");
    harness.get_by_label("Appearance");
    harness.get_by_label("Preview");
    // The sections not built yet are there, faded.
    assert!(harness.get_by_label("Calendar, settings page").accesskit_node().is_disabled());
    assert!(!harness.get_by_label("Accounts, settings page").accesskit_node().is_disabled());
    assert!(!harness.get_by_label("Appearance, settings page").accesskit_node().is_disabled());
    // The list lies under the sheet: a press there selects nothing, and the sheet stays.
    let before = harness.state().mailbox().selected();
    let row = harness.query_all_by_label_contains("Draft logo for the bakery").next().expect("the list is behind the sheet").rect();
    press_at(&mut harness, row.center());
    assert_eq!(harness.state().mailbox().selected(), before, "the sheet takes the press");
    assert!(harness.state().settings().is_open());
    // Beside the sheet the dim takes the press, and the sheet goes.
    let sheet = harness.get_by_role_and_label(Role::Dialog, "Settings").rect();
    press_at(&mut harness, egui::pos2(sheet.left() / 2.0, sheet.center().y));
    assert_eq!(harness.state().mailbox().selected(), before, "the dim takes the press");
    assert!(!harness.state().settings().is_open(), "a press beside the sheet closes it");
    harness.state_mut().open_settings();
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Close").click();
    harness.run();
    assert!(!harness.state().settings().is_open());
    assert!(harness.query_all_by_label_contains("Draft logo for the bakery").count() > 0, "the list is back");

    harness.get_by_label("Appearance settings").click();
    harness.run();
    harness.key_press(Key::Escape);
    harness.run();
    assert!(!harness.state().settings().is_open());
}

#[test]
fn every_control_runs_its_command() {
    let mut harness = opened();
    assert!(selected(&harness, "System"));
    assert_eq!(press(&mut harness, "Dark"), "ui.theme.set");
    assert_eq!(harness.state().appearance().theme, ThemeMode::Dark);
    assert_eq!(Mode::of(&harness.ctx), Mode::Dark);
    assert!(selected(&harness, "Dark"));

    assert_eq!(press(&mut harness, "Aurora"), "ui.accent.set");
    assert_eq!(harness.state().appearance().accent, Accent::Preset("aurora"));
    assert!(selected(&harness, "Aurora"));

    assert_eq!(press(&mut harness, "Inter"), "ui.font.set");
    assert_eq!(harness.state().appearance().font, Typeface::Inter);

    assert_eq!(press(&mut harness, "Lucide"), "ui.icons.set");
    assert_eq!(harness.state().appearance().icons, Icons::Lucide);

    // The slider: a press in its middle sets 110 %, the right arrow a step more.
    harness.get_by_role_and_label(Role::Slider, "Text size").click();
    harness.run();
    assert_eq!(harness.state().mailbox().last_run(), Some("ui.text_scale.set"));
    assert_eq!(harness.state().appearance().text_scale, 110);
    harness.key_press(Key::ArrowRight);
    harness.run();
    assert_eq!(harness.state().appearance().text_scale, 120);
    assert_eq!(harness.get_by_role_and_label(Role::Slider, "Text size").accesskit_node().numeric_value(), Some(120.0));

    let switch = harness.get_by_role_and_label(Role::Switch, "Reduce motion");
    assert_eq!(switch.accesskit_node().toggled(), Some(Toggled::False));
    // Below the fold at 120 %: pressed the way a screen reader presses it.
    switch.click_accesskit();
    harness.run();
    assert_eq!(harness.state().mailbox().last_run(), Some("ui.motion.set"));
    assert_eq!(harness.state().appearance().motion, Motion::Reduce);
    assert_eq!(harness.get_by_role_and_label(Role::Switch, "Reduce motion").accesskit_node().toggled(), Some(Toggled::True));

    // Custom takes the colours in use as its own and opens their fields.
    assert!(harness.query_by_label("Gradient start").is_none(), "no fields while a preset is in use");
    assert_eq!(press(&mut harness, "Custom"), "ui.accent.set");
    for (label, colour) in [("Gradient start", "#102030"), ("Gradient middle", "#405060"), ("Gradient end", "#708090"), ("Accent", "#ffd60a")] {
        let field = harness.get_by_role_and_label(Role::TextInput, label);
        field.click();
        harness.run();
        harness.key_press_modifiers(egui::Modifiers::COMMAND, Key::A);
        harness.get_by_role_and_label(Role::TextInput, label).type_text(colour);
        harness.run();
    }
    assert_eq!(press(&mut harness, "Use these colours"), "ui.accent.set");
    let Accent::Custom(custom) = harness.state().appearance().accent else { panic!("a custom accent") };
    assert_eq!(custom.hex(), ["#102030", "#405060", "#708090", "#ffd60a"].map(str::to_owned));
    assert!(selected(&harness, "Custom"));
}

#[test]
fn an_accent_the_engine_made_lighter_says_so_and_shows_the_colour_it_uses() {
    let mut harness = opened();
    harness.state_mut().settings_mut().colours_mut()[3] = "#3a1d8c".to_owned();
    harness.run();
    assert_eq!(press(&mut harness, "Use these colours"), "ui.accent.set");
    let Accent::Custom(custom) = harness.state().appearance().accent else { panic!("a custom accent") };
    assert_ne!(custom.accent.hex(), "#3a1d8c");
    assert_eq!(harness.state().settings().colours()[3], custom.accent.hex(), "the field shows the colour in use");
    let note = harness.state().settings().note().unwrap().to_owned();
    assert!(note.contains("#3a1d8c") && note.contains(&custom.accent.hex()), "{note}");
    harness.get_by_label(&note);
}

#[test]
fn the_custom_card_takes_the_typed_colours_and_the_preview_shows_them_first() {
    let mut harness = opened();
    let typed = ["#00a86b", "#0077ff", "#7a00ff", "#ffe066"];
    for (field, colour) in harness.state_mut().settings_mut().colours_mut().iter_mut().zip(typed) {
        *field = colour.to_owned();
    }
    harness.run();
    assert_eq!(harness.state().appearance().accent, Accent::default(), "typing changes only the preview");
    assert_eq!(harness.state().settings().draft().unwrap().gradient[0], egui::Color32::from_rgb(0x00, 0xa8, 0x6b));
    assert_eq!(press(&mut harness, "Custom"), "ui.accent.set");
    let Accent::Custom(custom) = harness.state().appearance().accent else { panic!("a custom accent") };
    assert_eq!(custom.hex(), typed.map(str::to_owned));
}

#[test]
fn colours_that_are_not_rrggbb_run_nothing_and_say_how_to_write_them() {
    let mut harness = opened();
    harness.state_mut().settings_mut().colours_mut()[1] = "blue; DROP".to_owned();
    harness.run();
    assert!(harness.state().settings().draft().is_none());
    harness.get_by_label("Write each colour as # and six hex digits, like #2f6bff.");
    let before = harness.state().mailbox().last_run();
    harness.get_by_role_and_label(Role::Button, "Use these colours").click();
    harness.run();
    assert_eq!(harness.state().mailbox().last_run(), before, "a disabled button runs nothing");
    harness.get_by_role_and_label(Role::Button, "Custom").click();
    harness.run();
    assert_eq!(harness.state().mailbox().last_run(), before, "nor does Custom");
    assert_eq!(harness.state().appearance().accent, Accent::default());
}

#[test]
fn the_add_account_form_opens_on_the_accounts_page_and_appearance_closes_it() {
    let mut harness = opened();
    harness.state_mut().onboarding_mut().0.open();
    harness.run();
    assert!(harness.state().settings().is_open(), "the form opens on the sheet");
    assert_eq!(harness.state().settings().section(), Section::Accounts);
    harness.state_mut().open_settings();
    harness.run();
    assert_eq!(harness.state().settings().section(), Section::Appearance);
    assert_eq!(harness.state().onboarding().stage(), crate::onboard::Stage::Closed);
}

#[test]
fn the_accounts_page_lists_every_account_with_its_address_and_the_nav_goes_back() {
    let mut harness = opened();
    assert!(selected(&harness, "Appearance, settings page"));
    harness.get_by_label("Accounts, settings page").click();
    harness.run();
    assert_eq!(harness.state().settings().section(), Section::Accounts);
    assert!(selected(&harness, "Accounts, settings page"));
    harness.get_by_label("Your accounts");
    harness.get_by_label("The mail accounts this computer reads and sends from.");
    let accounts = harness.state().mailbox().accounts().to_vec();
    assert!(!accounts.is_empty());
    for account in &accounts {
        harness.get_by_label(&format!("{}, {}", account.label, account.address));
    }
    assert!(harness.query_by_label("Mode").is_none(), "Appearance's rows are gone");
    // The sample window adds no account: it offers none.
    assert_eq!(harness.query_all_by_label("Add account").count(), 0);
    harness.check_accessibility();
    harness.get_by_label("Appearance, settings page").click();
    harness.run();
    assert_eq!(harness.state().settings().section(), Section::Appearance);
    harness.get_by_label("Mode");
}

#[test]
fn a_window_that_adds_accounts_offers_add_account_above_the_list() {
    let app = InboxApp::new(Mailbox::new(sample::session(clock()).unwrap(), clock())).with_accounts(
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
    let app = InboxApp::new(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    let mut harness =
        Harness::builder().with_size(vec2(1200.0, 1400.0)).with_theme(egui::Theme::Light).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness.get_by_label("Appearance settings").click();
    harness.run();
    assert!(selected(&harness, "Strip"), "the app's own strip by default");
    assert_eq!(press(&mut harness, "Top row"), "ui.frame.set");
    assert_eq!(harness.state().appearance().frame, WindowFrame::TopRow);
    assert_eq!(press(&mut harness, "Title bar"), "ui.frame.set");
    assert_eq!(harness.state().appearance().frame, WindowFrame::System);
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
