// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The window driven through its AccessKit tree, as a screen reader sees it. The harness refuses any control
//! without an accessible name after every run.

use egui::accesskit::{Role, Toggled};
use egui::{Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_engine::Session;
use inboxcraft_model::Role as Folder;
use inboxcraft_store::{Memory, StoreConfig};
use inboxcraft_time::{FakeClock, Timestamp};

use super::InboxApp;
use crate::mailbox::{Mailbox, Place};
use crate::sample;
use crate::theme::{Mode, ModePreference, Preset, Theme};

/// 2026-11-05T09:00:00Z.
fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

fn sample_app() -> InboxApp {
    InboxApp::new(Mailbox::new(sample::session(clock()).unwrap(), clock()))
}

fn harness_with(app: InboxApp, theme: egui::Theme) -> Harness<'static, InboxApp> {
    let mut harness = Harness::builder().with_size(vec2(1100.0, 720.0)).with_theme(theme).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

fn harness() -> Harness<'static, InboxApp> {
    harness_with(sample_app(), egui::Theme::Light)
}

#[test]
fn the_unified_inbox_lists_its_rows_by_sender_subject_and_date() {
    let harness = harness();
    harness.get_by_label("Odile Brannock, Allotment rota for November, 08:35, unread");
    harness.get_by_label("Petra Harbour, Draft logo for the bakery, 08:10, unread, flagged");
    harness.get_by_label("Inbox");
    harness.get_by_label("All accounts · 11 messages");
    assert_eq!(harness.get_all_by_role(Role::ListBoxOption).count(), 11);
}

#[test]
fn every_control_has_an_accessible_name_in_both_modes() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let mut harness = harness_with(sample_app(), theme);
        harness.check_accessibility();
        harness.state_mut().search("logo");
        harness.run();
        harness.check_accessibility();
        harness.get_by_label("Accounts").click();
        harness.run();
        harness.check_accessibility();
    }
}

#[test]
fn the_theme_follows_the_mode() {
    let harness = harness_with(sample_app(), egui::Theme::Dark);
    assert_eq!(Mode::of(&harness.ctx), Mode::Dark);
    assert_eq!(harness.state().mode(), ModePreference::System);
    assert_eq!(*harness.state().preset(), Preset::default());
    assert_eq!(harness.ctx.style_of(egui::Theme::Dark).visuals.panel_fill, Theme::new(Mode::Dark, &Preset::default()).palette.panel);
}

#[test]
fn a_new_preset_and_mode_show_from_the_next_frame() {
    let mut harness = harness_with(sample_app(), egui::Theme::Light);
    let citrus = *Preset::named("citrus").unwrap();
    harness.state_mut().set_preset(citrus);
    harness.state_mut().set_mode(ModePreference::Dark);
    harness.run();
    assert_eq!(Mode::of(&harness.ctx), Mode::Dark);
    assert_eq!(harness.ctx.style_of(egui::Theme::Light).visuals.panel_fill, Theme::new(Mode::Light, &citrus).palette.panel);
    assert_eq!(harness.ctx.style_of(egui::Theme::Dark).visuals.panel_fill, Theme::new(Mode::Dark, &citrus).palette.panel);
    harness.state_mut().set_mode(ModePreference::Light);
    harness.run();
    assert_eq!(Mode::of(&harness.ctx), Mode::Light);
    harness.state_mut().set_mode(ModePreference::System);
    harness.run();
    assert_eq!(harness.ctx.options(|options| options.theme_preference), egui::ThemePreference::System);
}

#[test]
fn the_folder_pane_names_views_and_folders_with_their_unread_counts() {
    let harness = harness();
    harness.get_by_label("Inbox, 4 unread");
    harness.get_by_label("Junk, 1 unread");
    harness.get_by_label("Inbox in Personal, 2 unread");
    harness.get_by_label("Inbox in Studio, 2 unread");
    harness.get_by_label("Harbour Bakery in Studio");
    harness.get_by_label("mira@example.org");
    harness.get_by_label("holloway.studio@example.org");
}

#[test]
fn clicking_a_folder_or_a_view_opens_it() {
    let mut harness = harness();
    harness.get_by_label("Receipts in Personal").click();
    harness.run();
    assert_eq!(harness.state().mailbox().place(), Place::Folder(7));
    harness.get_by_label("Paper & Pine, Receipt for your order 4417, Friday");
    harness.get_by_role_and_label(Role::Button, "Sent").click();
    harness.run();
    assert_eq!(harness.state().mailbox().place(), Place::View(Folder::Sent));
    assert!(harness.get_by_role_and_label(Role::Button, "Sent").accesskit_node().toggled() == Some(Toggled::True));
}

#[test]
fn the_account_picker_narrows_to_one_account_with_its_address_in_the_header() {
    let mut harness = harness();
    harness.get_by_label("Accounts").click();
    harness.run();
    harness.get_by_label("Studio (holloway.studio@example.org)").click();
    harness.run();
    assert_eq!(harness.state().mailbox().narrowed(), Some(2));
    harness.get_by_label("holloway.studio@example.org · 4 messages");
    assert!(harness.query_by_label("Inbox in Personal, 2 unread").is_none());
    harness.get_by_label("Accounts").click();
    harness.run();
    harness.get_by_label("All accounts").click();
    harness.run();
    assert_eq!(harness.state().mailbox().narrowed(), None);
}

#[test]
fn typing_in_the_search_field_searches_and_clear_ends_it() {
    let mut harness = harness();
    harness.get_by_role(Role::TextInput).focus();
    harness.run();
    harness.get_by_role(Role::TextInput).type_text("logo");
    harness.run();
    assert_eq!(harness.state().mailbox().query(), "logo");
    harness.get_by_label("Search");
    harness.get_by_label("All accounts · 2 messages");
    harness.get_by_label("Clear").click();
    harness.run();
    assert_eq!(harness.state().mailbox().query(), "");
    assert_eq!(harness.state().search_text(), "");
    assert_eq!(harness.state().mailbox().messages().len(), 11);
}

#[test]
fn ctrl_f_focuses_the_search_and_escape_empties_it() {
    let mut harness = harness();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::F);
    harness.run();
    assert!(harness.get_by_role(Role::TextInput).is_focused());
    harness.get_by_role(Role::TextInput).type_text("odile");
    harness.run();
    assert_eq!(harness.state().mailbox().messages().len(), 3);
    harness.key_press(Key::Escape);
    harness.run();
    assert_eq!(harness.state().mailbox().query(), "");
}

#[test]
fn opening_a_folder_empties_the_search_field() {
    let mut harness = harness();
    harness.state_mut().search("logo");
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Archive").click();
    harness.run();
    assert_eq!(harness.state().search_text(), "");
    assert_eq!(harness.state().mailbox().query(), "");
}

#[test]
fn a_search_without_results_and_an_empty_folder_say_so() {
    let mut harness = harness();
    harness.state_mut().search("zeppelin");
    harness.run();
    harness.get_by_label("No messages match");
    harness.get_by_label("No subject, sender or recipient holds “zeppelin”.");
    harness.state_mut().mailbox_mut().open(Place::Folder(8));
    harness.run();
    harness.get_by_label("Nothing here");
    harness.get_by_label("All accounts · 0 messages");
}

#[test]
fn clicking_a_row_selects_it() {
    let mut harness = harness();
    harness.get_by_label("Teodor Lisk, Photos from the coast walk, 04:00, flagged").click();
    harness.run();
    let selected = harness.state().mailbox().selected().unwrap();
    let row = harness.state().mailbox().messages().iter().find(|row| row.id == selected).unwrap();
    assert_eq!(row.subject, "Photos from the coast walk");
    assert!(harness.get_by_label("Teodor Lisk, Photos from the coast walk, 04:00, flagged").accesskit_node().toggled() == Some(Toggled::True));
}

#[test]
fn hovering_rows_and_folders_draws_without_trouble() {
    let mut harness = harness();
    let row = harness.get_by_label("Odile Brannock, Allotment rota for November, 08:35, unread").rect().center();
    harness.hover_at(row);
    harness.run();
    let folder = harness.get_by_label("Receipts in Personal").rect().center();
    harness.hover_at(folder);
    harness.run();
    harness.get_by_label("Receipts in Personal").focus();
    harness.run();
    harness.check_accessibility();
}

#[test]
fn an_error_shows_in_a_banner_until_dismissed() {
    let mut harness = harness();
    harness.state_mut().mailbox_mut().open(Place::Folder(4242));
    harness.run();
    let error = harness.state().mailbox().error().unwrap().to_owned();
    harness.get_by_label(&error);
    harness.get_by_label("Dismiss").click();
    harness.run();
    assert_eq!(harness.state().mailbox().error(), None);
}

#[test]
fn without_accounts_the_window_says_how_to_look_around() {
    let session = Session::new(Box::new(Memory::new(StoreConfig::default())), clock()).unwrap();
    let harness = harness_with(InboxApp::new(Mailbox::new(session, clock())), egui::Theme::Dark);
    harness.get_by_label("No accounts yet");
    harness.get_by_label_contains("--sample");
    assert!(harness.query_by_label("Accounts").is_none());
}

#[test]
fn the_app_shows_its_debug_form_without_message_text() {
    let app = sample_app();
    let shown = format!("{app:?}");
    assert!(shown.starts_with("InboxApp { mailbox: Mailbox {"), "{shown}");
    assert!(!shown.contains("Allotment"), "{shown}");
}
