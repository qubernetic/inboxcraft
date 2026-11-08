// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The screener and the grouped inbox in the window, from the engine's data on the sample: the screener card and
//! "New senders", the "Let ... in?" card over a held message, the group cards, the grouped list and the group menu,
//! the Triage page, and the triage choices kept between runs.

use std::sync::{Arc, Mutex};

use egui::accesskit::{Role, Toggled};
use egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_engine::appearance::Appearance;
use inboxcraft_engine::{ScreenerMode, Session};
use inboxcraft_time::{FakeClock, Timestamp};

use super::InboxApp;
use crate::keep::{Keep, Triage};
use crate::mailbox::{InboxView, Mailbox, Place};
use crate::sample;
use crate::settings::Section;
use crate::theme::install_fonts;

const SWIM: &str = "Harbour Swim Club, Winter swim at the harbour steps, 06:00, unread, new sender";
const ODILE: &str = "Odile Brannock, Allotment rota for November, 08:35, unread";

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

fn app() -> InboxApp {
    let mut app = InboxApp::new(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    app.set_reduce_motion(true);
    app
}

fn harness_of(app: InboxApp) -> Harness<'static, InboxApp> {
    let mut harness =
        Harness::builder().with_size(vec2(1300.0, 900.0)).with_context_setup(install_fonts).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

fn rows(harness: &Harness<'static, InboxApp>) -> usize {
    harness.query_all_by_role(Role::ListBoxOption).count()
}

/// The sample window on "New senders", the swim club's message open.
fn previewing() -> Harness<'static, InboxApp> {
    let mut harness = harness_of(app());
    harness.get_by_role_and_label(Role::Button, "Review").click();
    harness.run();
    harness.get_by_label(SWIM).click();
    harness.run();
    harness
}

#[test]
fn the_inbox_shows_the_screener_card_and_a_card_for_each_group() {
    let harness = harness_of(app());
    harness.get_by_label("2 new senders");
    harness.get_by_label("People, 3 unread");
    harness.get_by_label("Updates, 1 unread");
    harness.get_by_label("Show all (8)");
    assert_eq!(rows(&harness), 6, "three rows of each group until Show all");
}

#[test]
fn review_opens_new_senders_with_the_held_mail() {
    let mut harness = harness_of(app());
    harness.get_by_role_and_label(Role::Button, "Review").click();
    harness.run();
    assert_eq!(harness.state().mailbox().place(), Place::Senders);
    harness.get_by_label("New senders");
    harness.get_by_label(SWIM);
    assert_eq!(rows(&harness), 2);
}

#[test]
fn a_held_message_opens_as_a_preview_that_asks_whether_to_let_the_sender_in() {
    let harness = previewing();
    harness.get_by_label("Let Harbour Swim Club in?");
    harness.get_by_label("Preview · nothing is accepted yet");
    assert!(harness.query_by_label("In People").is_none(), "held mail is in no group yet");
    for name in ["People", "Updates", "Newsletters", "Invitations", "Accept", "Accept and open", "Block"] {
        harness.get_by_role_and_label(Role::Button, name);
    }
    let app = harness.state();
    assert_eq!(app.mailbox().last_run(), Some("mail.read"), "read as a preview, not opened");
    assert!(!app.mailbox().can_undo(), "the preview decides nothing");
}

#[test]
fn accept_with_a_group_lets_the_sender_in_and_undo_holds_them_again() {
    let mut harness = previewing();
    harness.get_by_role_and_label(Role::Button, "Updates").click();
    harness.run();
    assert_eq!(harness.get_by_role_and_label(Role::Button, "Updates").accesskit_node().toggled(), Some(Toggled::True));
    harness.get_by_role_and_label(Role::Button, "Accept").click();
    harness.run();
    harness.get_by_label("Accepted Harbour Swim Club into Updates");
    assert_eq!(harness.state().mailbox().waiting().len(), 1);
    assert!(harness.query_by_label(SWIM).is_none());
    harness.get_by_role_and_label(Role::Button, "Undo").click();
    harness.run();
    assert_eq!(harness.state().mailbox().waiting().len(), 2);
    harness.get_by_label(SWIM);
}

#[test]
fn accept_and_open_shows_the_message_in_the_inbox() {
    let mut harness = previewing();
    harness.get_by_role_and_label(Role::Button, "Accept and open").click();
    harness.run();
    harness.run();
    harness.get_by_label("Accepted Harbour Swim Club");
    let mailbox = harness.state().mailbox();
    assert_eq!(mailbox.place(), Place::View(inboxcraft_model::Role::Inbox));
    let open = mailbox.selected().unwrap();
    assert!(mailbox.messages().iter().any(|row| row.id == open && row.subject == "Winter swim at the harbour steps" && row.seen));
    assert!(harness.query_by_label("Let Harbour Swim Club in?").is_none(), "let in, so no longer held");
}

#[test]
fn block_files_the_sender_away() {
    let mut harness = previewing();
    harness.get_by_role_and_label(Role::Button, "Block").click();
    harness.run();
    harness.get_by_label("Blocked Harbour Swim Club");
    assert_eq!(rows(&harness), 1);
}

#[test]
fn a_row_s_context_menu_moves_its_sender_to_another_group_with_undo() {
    let mut harness = harness_of(app());
    harness.get_by_label(ODILE).click_secondary();
    harness.run();
    harness.get_by_label("Moves all mail from odile@example.org, kept and future.");
    harness.get_by_role_and_label(Role::MenuItemRadio, "Newsletters").click();
    harness.run();
    harness.get_by_label("Odile Brannock now goes to Newsletters");
    harness.get_by_label("Newsletters, 1 unread");
    harness.get_by_role_and_label(Role::Button, "Undo").click();
    harness.run();
    assert!(harness.query_by_label("Newsletters, 1 unread").is_none());
}

#[test]
fn more_in_the_reader_moves_the_sender_to_another_group() {
    let mut harness = harness_of(app());
    harness.get_by_label(ODILE).click();
    harness.run();
    harness.get_by_role_and_label(Role::Button, "More").click();
    harness.run();
    harness.get_by_role_and_label(Role::MenuItemRadio, "Let InboxCraft sort it").click();
    harness.run();
    harness.get_by_label("Odile Brannock goes where InboxCraft sorts it");
}

#[test]
fn the_grouped_list_puts_each_group_under_its_heading_and_the_rest_under_other() {
    let mut app = app();
    app.mailbox_mut().set_inbox_view(InboxView::List);
    app.mailbox_mut().set_grouped(2, false);
    let harness = harness_of(app);
    harness.get_by_label("People, 1 unread");
    harness.get_by_label("Other, 2 unread");
    assert!(harness.query_by_label("Done all").is_none(), "no cards");
    assert_eq!(rows(&harness), 11);
}

fn settings_on_triage() -> Harness<'static, InboxApp> {
    let mut app = app();
    app.settings_mut().open_at(Section::Triage, &Appearance::default());
    harness_of(app)
}

#[test]
fn the_triage_page_changes_the_screener_the_view_and_the_grouping() {
    let mut harness = settings_on_triage();
    harness.get_by_label("Triage, settings page");
    harness.get_by_label("Grouped inbox");
    assert_eq!(harness.get_by_role_and_label(Role::Button, "Before the inbox").accesskit_node().toggled(), Some(Toggled::True));
    harness.get_by_role_and_label(Role::Button, "Inside, marked").click();
    harness.run();
    assert_eq!(harness.state().mailbox().screener_mode(), ScreenerMode::Inside);
    harness.get_by_role_and_label(Role::Button, "Grouped list").click();
    harness.run();
    assert_eq!(harness.state().mailbox().inbox_view(), InboxView::List);
    let switch = harness.get_by_label("Group the inbox of Personal");
    assert_eq!(switch.accesskit_node().toggled(), Some(Toggled::True));
    switch.click();
    harness.run();
    assert!(!harness.state().mailbox().account(1).unwrap().grouped);
    assert_eq!(harness.get_by_label("Group the inbox of Personal").accesskit_node().toggled(), Some(Toggled::False));
}

/// A keep that holds the triage choices in memory and records what it was given.
#[derive(Clone, Default)]
struct Held {
    triage: Arc<Mutex<Option<Triage>>>,
    stored: Arc<Mutex<Vec<Triage>>>,
    fail: bool,
}

impl Keep for Held {
    fn load(&self) -> Option<Appearance> {
        None
    }

    fn store(&mut self, _appearance: &Appearance) -> Result<(), String> {
        Ok(())
    }

    fn load_triage(&self) -> Option<Triage> {
        *self.triage.lock().unwrap()
    }

    fn store_triage(&mut self, triage: &Triage) -> Result<(), String> {
        self.stored.lock().unwrap().push(*triage);
        if self.fail { Err("the disk is full".to_owned()) } else { Ok(()) }
    }
}

fn real_app() -> InboxApp {
    let session = Session::new(Box::new(sample::mailbox(Timestamp::from_unix(1_793_869_200, 0).unwrap()).unwrap()), clock()).unwrap();
    InboxApp::new(Mailbox::new(session, clock()))
}

#[test]
fn the_kept_triage_choices_come_back_and_each_change_is_kept() {
    let keep = Held::default();
    *keep.triage.lock().unwrap() = Some(Triage { screener: ScreenerMode::Inside, view: InboxView::List, keep_pins: false, hold: None });
    let app = real_app().with_keep(Box::new(keep.clone()));
    assert_eq!(app.mailbox().screener_mode(), ScreenerMode::Inside);
    assert_eq!(app.mailbox().inbox_view(), InboxView::List);
    let mut harness = harness_of(app);
    assert!(keep.stored.lock().unwrap().is_empty(), "nothing changed yet");
    harness.state_mut().mailbox_mut().set_inbox_view(InboxView::Simple);
    harness.run();
    assert_eq!(*keep.stored.lock().unwrap(), [Triage { screener: ScreenerMode::Inside, view: InboxView::Simple, keep_pins: false, hold: Some(5) }]);
    harness.state_mut().mailbox_mut().set_screener_mode(ScreenerMode::Off);
    harness.run();
    assert_eq!(
        keep.stored.lock().unwrap().last(),
        Some(&Triage { screener: ScreenerMode::Off, view: InboxView::Simple, keep_pins: false, hold: Some(5) })
    );
}

#[test]
fn without_kept_choices_the_window_keeps_its_own_from_the_first_change() {
    let keep = Held::default();
    let mut harness = harness_of(real_app().with_keep(Box::new(keep.clone())));
    harness.run();
    assert!(keep.stored.lock().unwrap().is_empty());
    harness.state_mut().mailbox_mut().set_inbox_view(InboxView::List);
    harness.run();
    assert_eq!(*keep.stored.lock().unwrap(), [Triage { screener: ScreenerMode::Off, view: InboxView::List, keep_pins: false, hold: Some(5) }]);
}

#[test]
fn the_sample_takes_the_kept_view_but_never_keeps_its_screener_mode() {
    let keep = Held::default();
    *keep.triage.lock().unwrap() = Some(Triage { screener: ScreenerMode::Off, view: InboxView::Simple, keep_pins: false, hold: None });
    let app = app().with_keep(Box::new(keep.clone()));
    assert_eq!(app.mailbox().screener_mode(), ScreenerMode::Before, "the sample keeps its own");
    assert_eq!(app.mailbox().inbox_view(), InboxView::Simple);
    let mut harness = harness_of(app);
    harness.state_mut().mailbox_mut().set_screener_mode(ScreenerMode::Inside);
    harness.run();
    assert!(keep.stored.lock().unwrap().is_empty(), "the owner's mode is not the sample's to change");
    harness.state_mut().mailbox_mut().set_inbox_view(InboxView::Cards);
    harness.run();
    assert_eq!(*keep.stored.lock().unwrap(), [Triage { screener: ScreenerMode::Off, view: InboxView::Cards, keep_pins: false, hold: Some(5) }]);
    let fresh = Held::default();
    let mut harness = harness_of(self::app().with_keep(Box::new(fresh.clone())));
    harness.state_mut().mailbox_mut().set_inbox_view(InboxView::List);
    harness.run();
    assert_eq!(*fresh.stored.lock().unwrap(), [Triage { screener: ScreenerMode::Off, view: InboxView::List, keep_pins: false, hold: Some(5) }]);
}

#[test]
fn a_triage_choice_that_cannot_be_kept_shows_why_once() {
    let keep = Held { fail: true, ..Held::default() };
    let mut harness = harness_of(real_app().with_keep(Box::new(keep.clone())));
    harness.state_mut().mailbox_mut().set_inbox_view(InboxView::List);
    harness.run();
    harness.run();
    assert_eq!(keep.stored.lock().unwrap().len(), 1, "tried once per change");
    assert!(harness.state().mailbox().error().is_some_and(|error| error.contains("the disk is full")));
}
