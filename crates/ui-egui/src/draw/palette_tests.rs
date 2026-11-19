// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The command palette driven through its AccessKit tree: Ctrl+K opens it over every action, typing narrows it, the
//! arrows and Enter or a press run one, Escape or a press beside it close it.

use egui::accesskit::{Role, Toggled};
use egui::{Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_engine::appearance::ThemeMode;
use inboxcraft_time::{FakeClock, Timestamp};

use super::{InboxApp, Screen};
use crate::keymap;
use crate::mailbox::Mailbox;
use crate::sample;

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

fn harness(theme: egui::Theme) -> Harness<'static, InboxApp> {
    let app = InboxApp::new(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    let mut harness = Harness::builder()
        .with_size(vec2(1200.0, 820.0))
        .with_theme(theme)
        .with_max_steps(12)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

fn opened(theme: egui::Theme) -> Harness<'static, InboxApp> {
    let mut harness = harness(theme);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::K);
    harness.run();
    assert!(harness.state().palette().is_open());
    harness
}

fn type_in(harness: &mut Harness<'static, InboxApp>, text: &str) {
    harness.get_by_role_and_label(Role::TextInput, "Command").type_text(text);
    harness.run();
}

#[test]
fn command_k_opens_the_palette_over_every_action_with_its_key() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let harness = opened(theme);
        harness.get_by_role_and_label(Role::Dialog, "Command palette");
        assert!(harness.get_by_role_and_label(Role::TextInput, "Command").accesskit_node().is_focused(), "the field has the focus");
        let rows = harness.query_all_by_role(Role::MenuItem).count();
        // Every action of the window and of the mail screen it opens over; the calendar's own keys are not.
        let listed = keymap::actions().iter().filter(|action| keymap::scope(action.id).meets(keymap::Scope::Mail)).count();
        assert_eq!(listed, keymap::actions().len() - keymap::CALENDAR_KEYS.len());
        assert_eq!(rows, listed + 9, "every action of the mail screen is listed, and the settings' nine pages");
        harness.get_by_label("↑↓ to move · Enter to run · Esc to close");
        harness.check_accessibility();
    }
}

#[test]
fn typing_narrows_the_list_and_enter_runs_the_first() {
    let mut harness = opened(egui::Theme::Light);
    // On the mail screen the calendar's own keys are not listed: "calendar" finds Show the calendar, then the settings'
    // Calendar page.
    type_in(&mut harness, "calendar");
    assert_eq!(harness.query_all_by_role(Role::MenuItem).count(), 2);
    harness.key_press(Key::Enter);
    harness.run();
    assert!(!harness.state().palette().is_open(), "running an action closes the palette");
    assert_eq!(harness.state().screen(), Screen::Calendar);
}

#[test]
fn the_arrows_move_the_pick_and_a_text_that_matches_nothing_says_so() {
    let mut harness = opened(egui::Theme::Light);
    type_in(&mut harness, "mode");
    assert_eq!(harness.state().palette().chosen().map(|action| action.id), Some(keymap::SYSTEM_MODE));
    harness.key_press(Key::ArrowDown);
    harness.run();
    assert_eq!(harness.state().palette().picked(), 1);
    harness.key_press(Key::ArrowUp);
    harness.key_press(Key::ArrowUp);
    harness.run();
    let picked = harness.state().palette().chosen().unwrap();
    assert_eq!(harness.get_by_role_and_label(Role::MenuItem, &picked.name).accesskit_node().toggled(), Some(Toggled::True), "the pick is marked");
    harness.key_press(Key::Enter);
    harness.run();
    let wanted = match picked.id {
        keymap::LIGHT => ThemeMode::Light,
        keymap::DARK => ThemeMode::Dark,
        _ => ThemeMode::System,
    };
    assert_eq!(harness.state().appearance().theme, wanted);
    assert_eq!(harness.state().mailbox().last_run(), Some("ui.theme.set"), "the palette runs the engine's command");

    let mut harness = opened(egui::Theme::Light);
    let selected = harness.state().mailbox().selected();
    type_in(&mut harness, "zzz");
    harness.get_by_label("No command matches “zzz”.");
    harness.key_press(Key::Enter);
    harness.key_press(Key::ArrowDown);
    harness.run();
    assert!(harness.state().palette().is_open(), "Enter with nothing picked runs nothing");
    assert_eq!(harness.state().mailbox().selected(), selected, "the arrows are the palette's, not the list's");
}

#[test]
fn escape_or_a_press_beside_it_closes_the_palette_and_runs_nothing() {
    let mut harness = opened(egui::Theme::Light);
    type_in(&mut harness, "dark");
    harness.key_press(Key::Escape);
    harness.run();
    assert!(!harness.state().palette().is_open());
    assert_eq!(harness.state().palette().query(), "", "what was typed goes");
    assert_eq!(harness.state().appearance().theme, ThemeMode::System);

    harness.key_press_modifiers(Modifiers::COMMAND, Key::K);
    harness.run();
    let dialog = harness.get_by_role_and_label(Role::Dialog, "Command palette").rect();
    let beside = egui::pos2(dialog.center().x, 800.0);
    harness.hover_at(beside);
    harness.run();
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton { pos: beside, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
        harness.run();
    }
    assert!(!harness.state().palette().is_open());
}

#[test]
fn a_press_on_a_row_runs_it_and_an_action_that_cannot_run_here_is_faded() {
    let mut harness = opened(egui::Theme::Light);
    assert!(harness.state().mailbox().selected().is_none());
    assert!(harness.get_by_role_and_label(Role::MenuItem, "Done").accesskit_node().is_disabled(), "no message is selected");
    assert!(harness.get_by_role_and_label(Role::MenuItem, "Undo").accesskit_node().is_disabled());
    type_in(&mut harness, "done");
    harness.key_press(Key::Enter);
    harness.run();
    assert!(harness.state().palette().is_open(), "a faded action does not run");
    harness.key_press(Key::Escape);
    harness.run();

    harness.query_all_by_label_contains("Draft logo for the bakery").next().unwrap().click();
    harness.run();
    let selected = harness.state().mailbox().selected().unwrap();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::K);
    harness.run();
    type_in(&mut harness, "done");
    harness.get_by_role_and_label(Role::MenuItem, "Done").click();
    harness.run();
    assert!(!harness.state().palette().is_open());
    assert!(harness.state().mailbox().undoable().is_some_and(|undo| undo.id == selected), "Done acts on the selected message");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::K);
    harness.run();
    assert!(!harness.get_by_role_and_label(Role::MenuItem, "Undo").accesskit_node().is_disabled(), "the act can be undone");
}

#[test]
fn every_action_runs_through_run_action_and_an_unknown_one_does_nothing() {
    let mut harness = harness(egui::Theme::Light);
    let ctx = harness.ctx.clone();
    let app = harness.state_mut();
    app.run_action(&ctx, "mail.nothing");
    assert!(!app.can_run("mail.nothing"));
    app.run_action(&ctx, keymap::SHORTCUTS);
    assert_eq!(app.settings().section(), crate::settings::Section::Shortcuts);
    app.run_action(&ctx, keymap::SETTINGS);
    assert!(!app.settings().is_open(), "Settings again shows the place the owner came from");
    app.run_action(&ctx, keymap::SETTINGS);
    assert!(app.settings().is_open(), "and once more the settings");
    assert_eq!(app.settings().section(), crate::settings::Section::Shortcuts, "on the page last shown");
    app.run_action(&ctx, keymap::SENT);
    assert!(!app.settings().is_open());
    assert_eq!(app.mailbox().place(), crate::mailbox::Place::View(inboxcraft_model::Role::Sent));
    app.run_action(&ctx, keymap::MAIL);
    assert_eq!(app.mailbox().place(), crate::mailbox::Place::View(inboxcraft_model::Role::Inbox));
    app.run_action(&ctx, keymap::DARK);
    assert_eq!(app.appearance().theme, ThemeMode::Dark);
    app.run_action(&ctx, keymap::LIGHT);
    assert_eq!(app.appearance().theme, ThemeMode::Light);
    app.run_action(&ctx, keymap::SYSTEM_MODE);
    assert_eq!(app.appearance().theme, ThemeMode::System);
    let reduce = app.reduce_motion();
    app.run_action(&ctx, keymap::MOTION);
    assert_eq!(app.reduce_motion(), !reduce);
    assert!(!app.can_run(keymap::FULL_WIDTH), "no message is open");
    app.run_action(&ctx, keymap::CALENDAR);
    assert_eq!(app.screen(), Screen::Calendar);
    app.run_action(&ctx, keymap::FIND);
    assert_eq!(app.screen(), Screen::Mail, "the search shows the mail");
    harness.run();
    assert!(harness.ctx.memory(|memory| memory.has_focus(super::list::search_id())), "the search field has the focus");
    harness.query_all_by_label_contains("Draft logo for the bakery").next().unwrap().click();
    harness.run();
    let ctx = harness.ctx.clone();
    let app = harness.state_mut();
    assert!(app.can_run(keymap::FULL_WIDTH));
    app.run_action(&ctx, keymap::FULL_WIDTH);
    assert!(app.reader().full_width());
    app.run_action(&ctx, keymap::READ);
    assert!(app.mailbox().undoable().is_some(), "read or unread acts on the open message");
    app.run_action(&ctx, keymap::UNDO);
    assert!(app.mailbox().undoable().is_none(), "undone");
    app.run_action(&ctx, keymap::DELETE);
    assert!(app.mailbox().leaving().is_some(), "the deleted row slides out");
}

#[test]
fn the_palette_offers_no_action_only_a_coming_soon_control_offers() {
    for normal in [false, true] {
        let mut harness = harness(egui::Theme::Light);
        crate::component::tests::offer_soon(&harness.ctx, keymap::SHORTCUTS, normal);
        harness.run();
        assert!(harness.state().can_run(keymap::SHORTCUTS));
        let ctx = harness.ctx.clone();
        harness.state_mut().run_action(&ctx, keymap::PALETTE);
        harness.run();
        let shortcuts = harness.get_by_role_and_label(Role::MenuItem, "Shortcuts").accesskit_node().is_disabled();
        assert_eq!(shortcuts, !normal, "Shortcuts is runnable in the palette only when a usable control offers it too");
    }
}

#[test]
fn each_page_row_shows_the_settings_on_its_page() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        for section in crate::settings::Section::ALL.into_iter().filter(|section| !section.planned() || *section == crate::settings::Section::About) {
            let mut harness = opened(theme);
            let name = format!("Settings: {}", section.name());
            type_in(&mut harness, &name);
            let row = harness.get_by_role_and_label(Role::MenuItem, &name);
            assert!(!row.accesskit_node().is_disabled(), "{name}");
            harness.key_press(Key::Enter);
            harness.run();
            let app = harness.state();
            assert!(!app.palette().is_open(), "{name}");
            assert!(app.settings().is_open(), "{name}");
            assert_eq!(app.settings().section(), section, "{name}");
            assert_eq!(app.mailbox().last_run(), Some(keymap::SETTINGS), "through the command");
        }
    }
}

#[test]
fn a_page_row_pressed_while_the_settings_show_goes_to_its_page() {
    let mut harness = harness(egui::Theme::Light);
    harness.state_mut().open_settings_page(Some("about"));
    harness.run();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::K);
    harness.run();
    type_in(&mut harness, "settings: sending");
    harness.get_by_role_and_label(Role::MenuItem, "Settings: Sending").click();
    harness.run();
    assert!(harness.state().settings().is_open(), "a page never toggles back");
    assert_eq!(harness.state().settings().section(), crate::settings::Section::Sending);
}

#[test]
fn the_page_rows_rest_where_settings_rests() {
    let harness = opened(egui::Theme::Light);
    let settings = harness.get_by_role_and_label(Role::MenuItem, "Settings").accesskit_node().is_disabled();
    for name in ["Settings: Appearance", "Settings: Accounts", "Settings: About"] {
        assert_eq!(harness.get_by_role_and_label(Role::MenuItem, name).accesskit_node().is_disabled(), settings, "{name}");
    }
}

/// How many texts the last frame painted with their middle in `rect`: a row's icon, its name and its key's words.
fn texts_in(harness: &Harness<'static, InboxApp>, rect: egui::Rect) -> usize {
    fn open(shape: &egui::Shape, rect: egui::Rect, count: &mut usize) {
        match shape {
            egui::Shape::Vec(inner) => inner.iter().for_each(|shape| open(shape, rect, count)),
            egui::Shape::Text(text) if !text.galley.text().trim().is_empty() && rect.contains(text.visual_bounding_rect().center()) => *count += 1,
            _ => {}
        }
    }
    let mut count = 0;
    harness.output().shapes.iter().for_each(|clipped| open(&clipped.shape, rect, &mut count));
    count
}

#[test]
fn a_page_row_has_no_key_where_settings_shows_its_own() {
    let names = ["Settings: Appearance", "Settings: Accounts", "Settings: Notifications"];
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let mut harness = opened(theme);
        type_in(&mut harness, "settings");
        assert!(harness.state().keymap().chip(&egui::Context::default(), keymap::SETTINGS).is_some(), "Settings has a key");
        let bound: Vec<usize> = names.iter().map(|name| texts_in(&harness, harness.get_by_role_and_label(Role::MenuItem, name).rect())).collect();
        // Settings' key taken away: a page row draws what it drew, as it never showed the key.
        harness.state_mut().keymap_mut().unbind(keymap::SETTINGS);
        harness.run();
        let unbound: Vec<usize> = names.iter().map(|name| texts_in(&harness, harness.get_by_role_and_label(Role::MenuItem, name).rect())).collect();
        assert_eq!(bound, unbound, "no key column on a page row");
        assert!(bound.iter().all(|texts| *texts >= 2), "its icon and its name: {bound:?}");
    }
}
