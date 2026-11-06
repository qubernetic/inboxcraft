// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The appearance settings page driven through its AccessKit tree: every control runs its `ui.*.set` command.

use egui::accesskit::{Role, Toggled};
use egui::{Key, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_engine::appearance::{Accent, Icons, Motion, ThemeMode, Typeface};
use inboxcraft_time::{FakeClock, Timestamp};

use super::InboxApp;
use crate::mailbox::Mailbox;
use crate::sample;
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

fn selected(harness: &Harness<'static, InboxApp>, label: &str) -> bool {
    harness.get_by_role_and_label(Role::Button, label).accesskit_node().toggled() == Some(Toggled::True)
}

#[test]
fn the_pane_opens_the_page_in_place_of_the_list_and_close_or_escape_leave_it() {
    let mut harness = opened();
    harness.get_by_label("Appearance");
    harness.get_by_label("Preview");
    assert_eq!(harness.query_all_by_label_contains("Draft logo for the bakery").count(), 0, "the list makes room for the page");
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

    assert_eq!(press(&mut harness, "120 %"), "ui.text_scale.set");
    assert_eq!(harness.state().appearance().text_scale, 120);
    assert!(selected(&harness, "120 %"));

    let switch = harness.get_by_role_and_label(Role::Switch, "Reduce motion");
    assert_eq!(switch.accesskit_node().toggled(), Some(Toggled::False));
    // Below the fold at 120 %: pressed the way a screen reader presses it.
    switch.click_accesskit();
    harness.run();
    assert_eq!(harness.state().mailbox().last_run(), Some("ui.motion.set"));
    assert_eq!(harness.state().appearance().motion, Motion::Reduce);
    assert_eq!(harness.get_by_role_and_label(Role::Switch, "Reduce motion").accesskit_node().toggled(), Some(Toggled::True));

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
fn the_custom_pill_takes_the_typed_colours_and_the_preview_shows_them_first() {
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
fn opening_the_page_closes_the_add_account_page_and_the_other_way_round() {
    let mut harness = opened();
    harness.state_mut().onboarding_mut().0.open();
    harness.run();
    assert!(!harness.state().settings().is_open(), "the add-account page takes the place");
    harness.state_mut().open_settings();
    harness.run();
    assert!(harness.state().settings().is_open());
    assert_eq!(harness.state().onboarding().stage(), crate::onboard::Stage::Closed);
}
