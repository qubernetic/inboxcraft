// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The Notifications page: each account's level, returned snoozes and reminders through `notify.set`, kept between
//! runs; running in the background and starting with the computer through the [`Desk`] the app hands in.

use std::sync::{Arc, Mutex};

use egui::accesskit::{Role, Toggled};
use egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_engine::Session;
use inboxcraft_engine::appearance::Appearance;
use inboxcraft_engine::notify::{Level, Settings as NotifySettings};
use inboxcraft_model::AccountId;
use inboxcraft_store::{Memory, StoreConfig};
use inboxcraft_time::{FakeClock, Timestamp};

use super::InboxApp;
use crate::desk::Desk;
use crate::keep::Keep;
use crate::mailbox::Mailbox;
use crate::sample;
use crate::settings::Section;
use crate::theme::install_fonts;

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

fn app() -> InboxApp {
    let mut app = InboxApp::new(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    app.set_reduce_motion(true);
    app
}

/// The sample's mail in a session that is not the sample's, as the owner's own mail would be.
fn real_app() -> InboxApp {
    let session = Session::new(Box::new(sample::mailbox(Timestamp::from_unix(1_793_869_200, 0).unwrap()).unwrap()), clock()).unwrap();
    let mut app = InboxApp::new(Mailbox::new(session, clock()));
    app.set_reduce_motion(true);
    app
}

fn harness_of(app: InboxApp, theme: egui::Theme) -> Harness<'static, InboxApp> {
    let mut harness = Harness::builder()
        .with_size(vec2(1300.0, 900.0))
        .with_theme(theme)
        .with_context_setup(install_fonts)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

fn on_the_page(mut app: InboxApp) -> Harness<'static, InboxApp> {
    app.settings_mut().open_at(Section::Notifications, &Appearance::default());
    harness_of(app, egui::Theme::Light)
}

fn toggled(harness: &Harness<'static, InboxApp>, role: Role, label: &str) -> Option<Toggled> {
    harness.get_by_role_and_label(role, label).accesskit_node().toggled()
}

fn disabled(harness: &Harness<'static, InboxApp>, label: &str) -> bool {
    harness.get_by_role_and_label(Role::Switch, label).accesskit_node().is_disabled()
}

/// What a fake desk was asked, and what it answers.
#[derive(Debug, Default)]
struct Asked {
    background: Option<bool>,
    login: Option<bool>,
    asking: bool,
    /// The answer the next `answer` gives.
    answer: Option<Result<(), String>>,
    /// Every call, in order.
    calls: Vec<String>,
    /// `set_background` fails with this.
    refuse: Option<String>,
}

/// A desk whose state the test holds.
#[derive(Debug, Clone, Default)]
struct Fake(Arc<Mutex<Asked>>);

impl Fake {
    fn with(background: Option<bool>, login: Option<bool>) -> Self {
        Self(Arc::new(Mutex::new(Asked { background, login, ..Asked::default() })))
    }

    fn calls(&self) -> Vec<String> {
        self.0.lock().unwrap().calls.clone()
    }
}

impl Desk for Fake {
    fn background(&self) -> Option<bool> {
        self.0.lock().unwrap().background
    }

    fn set_background(&mut self, on: bool) -> Result<(), String> {
        let mut asked = self.0.lock().unwrap();
        asked.calls.push(format!("background {on}"));
        if let Some(why) = asked.refuse.clone() {
            return Err(why);
        }
        asked.background = Some(on);
        Ok(())
    }

    fn login(&self) -> Option<bool> {
        self.0.lock().unwrap().login
    }

    fn asking(&self) -> bool {
        self.0.lock().unwrap().asking
    }

    fn set_login(&mut self, on: bool) {
        let mut asked = self.0.lock().unwrap();
        asked.calls.push(format!("login {on}"));
        asked.asking = true;
    }

    fn answer(&mut self) -> Option<Result<(), String>> {
        let mut asked = self.0.lock().unwrap();
        let answer = asked.answer.take()?;
        asked.asking = false;
        Some(answer)
    }
}

#[test]
fn the_page_lists_every_account_with_its_level_and_a_press_changes_it() {
    let mut harness = on_the_page(app());
    harness.get_by_label("Notifications, settings page");
    harness.get_by_label("Personal · mira@example.org");
    assert_eq!(toggled(&harness, Role::Button, "All for Personal"), Some(Toggled::True));
    assert_eq!(toggled(&harness, Role::Button, "People only for Studio"), Some(Toggled::False));
    harness.get_by_role_and_label(Role::Button, "People only for Studio").click();
    harness.run();
    assert_eq!(harness.state().mailbox().notify_level(2), Level::People);
    assert_eq!(harness.state().mailbox().notify_level(1), Level::All);
    assert_eq!(toggled(&harness, Role::Button, "People only for Studio"), Some(Toggled::True));
    harness.get_by_role_and_label(Role::Button, "None for Personal").click();
    harness.run();
    assert_eq!(harness.state().mailbox().notify_level(1), Level::None);
}

#[test]
fn returned_snoozes_and_reminders_switch_on_and_off() {
    let mut harness = on_the_page(app());
    assert_eq!(toggled(&harness, Role::Switch, "Returned snoozes"), Some(Toggled::True));
    harness.get_by_role_and_label(Role::Switch, "Returned snoozes").click();
    harness.run();
    assert!(!harness.state().mailbox().notify_settings().returns);
    assert_eq!(toggled(&harness, Role::Switch, "Returned snoozes"), Some(Toggled::False));
    harness.get_by_role_and_label(Role::Switch, "Follow-up reminders").click();
    harness.run();
    assert!(!harness.state().mailbox().notify_settings().reminders);
    assert!(!harness.state().mailbox().notify_settings().returns);
}

#[test]
fn without_a_desk_the_background_switches_say_they_are_not_available_here() {
    let mut harness = on_the_page(app());
    assert!(disabled(&harness, "Run in the background"));
    assert!(disabled(&harness, "Start with the computer"));
    assert!(harness.query_all_by_label("Not available in this window").count() >= 1);
    harness.get_by_role_and_label(Role::Switch, "Run in the background").click();
    harness.run();
    assert!(harness.state().settings().is_open());
}

#[test]
fn the_background_switch_asks_the_desk_and_turning_it_off_ends_starting_with_the_computer() {
    let desk = Fake::with(Some(true), Some(true));
    let mut harness = on_the_page(app().with_desk(Box::new(desk.clone())));
    assert_eq!(toggled(&harness, Role::Switch, "Run in the background"), Some(Toggled::True));
    assert_eq!(toggled(&harness, Role::Switch, "Start with the computer"), Some(Toggled::True));
    assert!(!disabled(&harness, "Start with the computer"));
    harness.get_by_role_and_label(Role::Switch, "Run in the background").click();
    harness.run();
    assert_eq!(desk.calls(), ["background false", "login false"]);
    assert_eq!(toggled(&harness, Role::Switch, "Run in the background"), Some(Toggled::False));
    harness.run();
    harness.get_by_label("Waiting for the desktop's answer…");
    desk.0.lock().unwrap().login = Some(false);
    desk.0.lock().unwrap().answer = Some(Ok(()));
    harness.run();
    harness.run();
    assert!(harness.query_by_label("Waiting for the desktop's answer…").is_none());
    assert!(disabled(&harness, "Start with the computer"), "starting with the computer needs the background");
    harness.get_by_label("Turn on Run in the background first: a start at login opens no window.");
    harness.get_by_role_and_label(Role::Switch, "Run in the background").click();
    harness.run();
    assert_eq!(desk.calls().last().map(String::as_str), Some("background true"));
    assert!(!disabled(&harness, "Start with the computer"));
}

#[test]
fn starting_with_the_computer_waits_for_the_desktop_and_says_why_it_was_not_done() {
    let desk = Fake::with(Some(true), Some(false));
    let mut harness = on_the_page(app().with_desk(Box::new(desk.clone())));
    harness.get_by_role_and_label(Role::Switch, "Start with the computer").click();
    harness.run();
    assert_eq!(desk.calls(), ["login true"]);
    harness.run();
    harness.get_by_label("Waiting for the desktop's answer…");
    assert!(disabled(&harness, "Start with the computer"), "one question at a time");
    desk.0.lock().unwrap().answer = Some(Err("the desktop did not allow InboxCraft to start at login".to_owned()));
    harness.run();
    harness.run();
    harness.get_by_label("Not changed: the desktop did not allow InboxCraft to start at login.");
    assert_eq!(toggled(&harness, Role::Switch, "Start with the computer"), Some(Toggled::False));
    // The note goes with the sheet.
    harness.state_mut().close_settings();
    harness.run();
    harness.state_mut().settings_mut().open_at(Section::Notifications, &Appearance::default());
    harness.run();
    assert!(harness.query_by_label("Not changed: the desktop did not allow InboxCraft to start at login.").is_none());
}

#[test]
fn a_background_choice_the_desk_refuses_shows_in_the_error_banner() {
    let desk = Fake::with(Some(true), None);
    desk.0.lock().unwrap().refuse = Some("the choice could not be kept".to_owned());
    let mut harness = on_the_page(app().with_desk(Box::new(desk.clone())));
    assert!(disabled(&harness, "Start with the computer"), "a desk that cannot start at login");
    harness.get_by_role_and_label(Role::Switch, "Run in the background").click();
    harness.run();
    assert_eq!(desk.calls(), ["background false"], "nothing to turn off at login");
    assert!(harness.state().mailbox().error().is_some_and(|error| error.contains("the choice could not be kept")));
    assert_eq!(toggled(&harness, Role::Switch, "Run in the background"), Some(Toggled::True));
}

#[test]
fn without_accounts_the_page_says_to_add_one_first() {
    let session = Session::new(Box::new(Memory::new(StoreConfig::default())), clock()).unwrap();
    let harness = on_the_page(InboxApp::new(Mailbox::new(session, clock())));
    harness.get_by_label("Add an account first: each account chooses which of its new mail notifies you.");
    harness.get_by_role_and_label(Role::Switch, "Returned snoozes");
}

#[test]
fn every_control_on_the_page_has_an_accessible_name_in_both_modes() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let mut app = app().with_desk(Box::new(Fake::with(Some(true), Some(false))));
        app.settings_mut().open_at(Section::Notifications, &Appearance::default());
        let harness = harness_of(app, theme);
        harness.check_accessibility();
    }
}

/// A keep that holds the notification choices in memory and records what it was given.
#[derive(Clone, Default)]
struct Held {
    notify: Arc<Mutex<Option<NotifySettings>>>,
    stored: Arc<Mutex<Vec<NotifySettings>>>,
    fail: bool,
}

impl Keep for Held {
    fn load(&self) -> Option<Appearance> {
        None
    }

    fn store(&mut self, _appearance: &Appearance) -> Result<(), String> {
        Ok(())
    }

    fn load_notify(&self) -> Option<NotifySettings> {
        self.notify.lock().unwrap().clone()
    }

    fn store_notify(&mut self, settings: &NotifySettings) -> Result<(), String> {
        self.stored.lock().unwrap().push(settings.clone());
        if self.fail { Err("the disk is full".to_owned()) } else { Ok(()) }
    }
}

#[test]
fn the_kept_notification_choices_come_back_and_each_change_is_kept() {
    let keep = Held::default();
    let mut kept = NotifySettings::default();
    kept.set_level(AccountId::new(1), Level::None);
    kept.reminders = false;
    *keep.notify.lock().unwrap() = Some(kept.clone());
    let app = real_app().with_keep(Box::new(keep.clone()));
    assert_eq!(app.mailbox().notify_settings(), &kept);
    let mut harness = harness_of(app, egui::Theme::Light);
    assert!(keep.stored.lock().unwrap().is_empty(), "nothing changed yet");
    harness.state_mut().mailbox_mut().set_notify_level(2, Level::People);
    harness.run();
    let mut changed = kept;
    changed.set_level(AccountId::new(2), Level::People);
    assert_eq!(*keep.stored.lock().unwrap(), [changed]);
    harness.run();
    assert_eq!(keep.stored.lock().unwrap().len(), 1, "kept once per change");
}

#[test]
fn a_kept_level_of_an_account_not_held_is_dropped() {
    let keep = Held::default();
    let mut kept = NotifySettings::default();
    kept.set_level(AccountId::new(1), Level::People);
    kept.set_level(AccountId::new(7), Level::None);
    *keep.notify.lock().unwrap() = Some(kept);
    let app = real_app().with_keep(Box::new(keep.clone()));
    assert_eq!(app.mailbox().notify_level(7), Level::All, "account 7 is not held");
    assert_eq!(app.mailbox().notify_level(1), Level::People);
    let mut harness = harness_of(app, egui::Theme::Light);
    harness.run();
    let mut held = NotifySettings::default();
    held.set_level(AccountId::new(1), Level::People);
    assert_eq!(*keep.stored.lock().unwrap(), [held], "the next store leaves account 7 out");
}

#[test]
fn the_sample_neither_takes_nor_keeps_the_owner_s_notification_choices() {
    let keep = Held::default();
    let mut kept = NotifySettings::default();
    kept.set_level(AccountId::new(1), Level::None);
    *keep.notify.lock().unwrap() = Some(kept);
    let app = app().with_keep(Box::new(keep.clone()));
    assert_eq!(app.mailbox().notify_level(1), Level::All, "account 1 of the owner is not the sample's");
    let mut harness = harness_of(app, egui::Theme::Light);
    harness.state_mut().mailbox_mut().set_notify_level(2, Level::None);
    harness.run();
    assert!(keep.stored.lock().unwrap().is_empty());
}

#[test]
fn a_notification_choice_that_cannot_be_kept_shows_why_once() {
    let keep = Held { fail: true, ..Held::default() };
    let mut harness = harness_of(real_app().with_keep(Box::new(keep.clone())), egui::Theme::Light);
    harness.state_mut().mailbox_mut().set_notify_returns(false);
    harness.run();
    harness.run();
    assert_eq!(keep.stored.lock().unwrap().len(), 1, "tried once per change");
    assert!(harness.state().mailbox().error().is_some_and(|error| error.contains("the disk is full")));
}

#[test]
fn a_refusal_that_comes_after_the_sheet_closed_shows_in_the_error_banner() {
    let desk = Fake::with(Some(true), Some(false));
    let mut harness = on_the_page(app().with_desk(Box::new(desk.clone())));
    harness.get_by_role_and_label(Role::Switch, "Start with the computer").click();
    harness.run();
    harness.state_mut().close_settings();
    harness.run();
    desk.0.lock().unwrap().answer = Some(Err("the Background portal did not answer".to_owned()));
    harness.run();
    assert!(harness.state().mailbox().error().is_some_and(|error| error.contains("the Background portal did not answer")));
}
