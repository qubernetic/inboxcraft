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
use crate::mailbox::{After, GreetBy, InboxView, Mailbox, Place};
use crate::sample;
use crate::settings::Section;
use crate::theme::install_fonts;

const SWIM: &str = "Harbour Swim Club, Winter swim at the harbour steps, 06:00, unread, new sender";
const ODILE: &str = "Odile Brannock, Allotment rota for November, 08:35, unread";

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

fn app() -> InboxApp {
    let mut app = InboxApp::at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock()));
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
fn the_inbox_shows_the_screener_card_and_a_head_for_each_group() {
    let harness = harness_of(app());
    harness.get_by_label("2 new senders");
    harness.get_by_label("People, 3 unread");
    harness.get_by_label("Notifications, 1 unread");
    harness.get_by_role_and_label(Role::Button, "show all");
    assert_eq!(rows(&harness), 6, "three rows of each group until show all");
}

#[test]
fn review_opens_new_senders_with_the_held_mail() {
    let mut harness = harness_of(app());
    harness.get_by_role_and_label(Role::Button, "Review").click();
    harness.run();
    assert_eq!(harness.state().mailbox().place(), Place::Senders);
    // The list's head says New senders; so does the index's row, shown.
    assert!(harness.query_all_by_label("New senders").any(|node| node.accesskit_node().role() != Role::Button), "the list's head");
    harness.get_by_label(SWIM);
    // A queue of one card a sender, each with Let in and Keep out; the first is the current one.
    assert_eq!(harness.query_all_by_role_and_label(Role::Button, "Let in").count(), 2);
    assert_eq!(harness.query_all_by_role_and_label(Role::Button, "Keep out").count(), 2);
    harness.get_by_label("Who gets in?");
}

#[test]
fn let_in_and_keep_out_answer_the_sender_of_their_card() {
    let mut harness = harness_of(app());
    harness.get_by_role_and_label(Role::Button, "Review").click();
    harness.run();
    // Keep out on the first card: its sender is blocked, one card is left.
    harness.query_all_by_role_and_label(Role::Button, "Keep out").next().unwrap().click();
    harness.run();
    harness.run();
    harness.get_by_label("Blocked Harbour Swim Club");
    assert_eq!(harness.query_all_by_role_and_label(Role::Button, "Let in").count(), 1);
    // Let in on the card left: the queue is empty and says so.
    harness.get_by_role_and_label(Role::Button, "Let in").click();
    harness.run();
    harness.run();
    harness.get_by_label("Accepted Fernhill Nursery");
    harness.get_by_label("Nobody is waiting.");
}

#[test]
fn a_held_message_opens_as_a_preview_that_asks_whether_to_let_the_sender_in() {
    let harness = previewing();
    harness.get_by_label("Let Harbour Swim Club in?");
    harness.get_by_label("Preview · nothing is accepted yet");
    assert!(harness.query_by_label("In People").is_none(), "held mail is in no group yet");
    harness.get_by_label("club@example.org  ·  first mail 06:00  ·  1 message waiting");
    harness.get_by_label("No group chosen: InboxCraft sorts their mail itself.");
    harness.get_by_label("Winter swim at the harbour steps");
    assert!(harness.query_by_role_and_label(Role::Button, "Reply").is_none(), "the preview has no Reply");
    assert!(harness.query_by_role_and_label(Role::Button, "Snooze").is_none(), "the preview decides nothing: no acts");
    harness.get_by_role_and_label(Role::Button, "More");
    harness.get_by_label("New sender");
    for name in ["People", "Notifications", "Newsletters", "Accept", "Accept and reply", "Block"] {
        harness.get_by_role_and_label(Role::Button, name);
    }
    assert!(harness.query_by_role_and_label(Role::Button, "Invitations").is_none(), "no Invitations group to let a sender into");
    let app = harness.state();
    assert_eq!(app.mailbox().last_run(), Some("mail.read"), "read as a preview, not opened");
    assert!(!app.mailbox().can_undo(), "the preview decides nothing");
}

#[test]
fn accept_with_a_group_lets_the_sender_in_and_undo_holds_them_again() {
    let mut harness = previewing();
    harness.get_by_role_and_label(Role::Button, "Notifications").click();
    harness.run();
    assert_eq!(harness.get_by_role_and_label(Role::Button, "Notifications").accesskit_node().toggled(), Some(Toggled::True));
    harness.get_by_label("Their mail goes to Notifications.");
    harness.get_by_role_and_label(Role::Button, "Accept into Notifications").click();
    harness.run();
    harness.get_by_label("Accepted Harbour Swim Club into Notifications");
    assert_eq!(harness.state().mailbox().waiting().len(), 1);
    assert!(harness.query_by_label(SWIM).is_none());
    harness.get_by_role_and_label(Role::Button, "undo").click();
    harness.run();
    assert_eq!(harness.state().mailbox().waiting().len(), 2);
    harness.get_by_label(SWIM);
}

#[test]
fn accept_and_reply_lets_the_sender_in_and_opens_a_reply() {
    let mut harness = previewing();
    harness.get_by_role_and_label(Role::Button, "Accept and reply").click();
    harness.run();
    harness.run();
    harness.get_by_label("Accepted Harbour Swim Club");
    let mailbox = harness.state().mailbox();
    assert_eq!(mailbox.place(), Place::View(inboxcraft_model::Role::Inbox));
    let open = mailbox.selected().unwrap();
    assert!(mailbox.messages().iter().any(|row| row.id == open && row.subject == "Winter swim at the harbour steps" && row.seen));
    assert!(harness.query_by_label("Let Harbour Swim Club in?").is_none(), "let in, so no longer held");
    // The reply is the owner's to write: the composer opens over it, nothing is sent.
    assert_eq!(harness.state().composer().map(|composer| composer.title()), Some("Re: Winter swim at the harbour steps".to_owned()));
}

#[test]
fn the_let_in_card_stands_between_the_tags_and_the_subject_with_the_cyan_edge() {
    let harness = previewing();
    let question = harness.get_by_label("Let Harbour Swim Club in?").rect();
    let more = harness.get_by_role_and_label(Role::Button, "More").rect();
    let subject = harness.get_by_label("Winter swim at the harbour steps").rect();
    assert!(more.bottom() < question.top() && question.bottom() < subject.top(), "{more:?} {question:?} {subject:?}");
    let look = crate::theme::Theme::current(&harness.ctx, &crate::theme::Look::of(harness.state().mailbox().appearance()));
    fn walk(shape: &egui::Shape, colour: egui::Color32, found: &mut Vec<egui::Rect>) {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, colour, found)),
            egui::Shape::Rect(rect) if rect.stroke.color == colour && rect.stroke.width > 0.0 => found.push(rect.rect),
            _ => {}
        }
    }
    let mut edges = Vec::new();
    harness.output().shapes.iter().for_each(|clipped| walk(&clipped.shape, look.palette.cyan_edge, &mut edges));
    // 14 above the question and 16 left of it.
    let card = edges.iter().find(|edge| edge.contains_rect(question));
    assert!(
        card.is_some_and(|card| (question.top() - card.top() - 14.0).abs() <= 1.0 && (question.left() - card.left() - 16.0).abs() <= 1.0),
        "{card:?} {question:?}"
    );
}

#[test]
fn block_files_the_sender_away() {
    let mut harness = previewing();
    harness.get_by_role_and_label(Role::Button, "Block").click();
    harness.run();
    harness.get_by_label("Blocked Harbour Swim Club");
    assert_eq!(harness.query_all_by_role_and_label(Role::Button, "Let in").count(), 1, "one sender waits in the queue");
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
    harness.get_by_role_and_label(Role::Button, "undo").click();
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
    app.mailbox_mut().set_inbox_view(InboxView::Grouped);
    app.mailbox_mut().set_grouped(2, false);
    let harness = harness_of(app);
    harness.get_by_label("People, 1 unread");
    harness.get_by_label("Other, 2 unread");
    assert!(harness.query_by_label("Done all").is_none(), "no cards");
    assert_eq!(rows(&harness), 11);
}

fn settings_on_triage() -> Harness<'static, InboxApp> {
    let mut app = app();
    super::tests::at_settings(&mut app, Section::Triage);
    harness_of(app)
}

#[test]
fn the_triage_page_changes_the_screener_and_the_grouping() {
    let mut harness = settings_on_triage();
    harness.get_by_label("Triage, settings page");
    harness.get_by_label("[ D ] Grouped inbox");
    assert_eq!(harness.get_by_role_and_label(Role::RadioButton, "Before the inbox").accesskit_node().toggled(), Some(Toggled::True));
    harness.get_by_role_and_label(Role::RadioButton, "Inside, marked").click();
    harness.run();
    assert_eq!(harness.state().mailbox().screener_mode(), ScreenerMode::Inside);
    // The inbox view is on Appearance (settings_page_tests.rs).
    assert!(harness.query_by_role_and_label(Role::RadioButton, "Grouped list").is_none());
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
    InboxApp::at_mail(Mailbox::new(session, clock()))
}

#[test]
fn the_kept_triage_choices_come_back_and_each_change_is_kept() {
    let keep = Held::default();
    *keep.triage.lock().unwrap() = Some(Triage {
        screener: ScreenerMode::Inside,
        view: InboxView::Grouped,
        keep_pins: false,
        hold: None,
        after: After::Next,
        greet_by: GreetBy::First,
    });
    let app = real_app().with_keep(Box::new(keep.clone()));
    assert_eq!(app.mailbox().screener_mode(), ScreenerMode::Inside);
    assert_eq!(app.mailbox().inbox_view(), InboxView::Grouped);
    let mut harness = harness_of(app);
    assert!(keep.stored.lock().unwrap().is_empty(), "nothing changed yet");
    harness.state_mut().mailbox_mut().set_inbox_view(InboxView::List);
    harness.run();
    assert_eq!(
        *keep.stored.lock().unwrap(),
        [Triage {
            screener: ScreenerMode::Inside,
            view: InboxView::List,
            keep_pins: false,
            hold: Some(5),
            after: After::Next,
            greet_by: GreetBy::First
        }]
    );
    harness.state_mut().mailbox_mut().set_screener_mode(ScreenerMode::Off);
    harness.run();
    assert_eq!(
        keep.stored.lock().unwrap().last(),
        Some(&Triage {
            screener: ScreenerMode::Off,
            view: InboxView::List,
            keep_pins: false,
            hold: Some(5),
            after: After::Next,
            greet_by: GreetBy::First
        })
    );
}

#[test]
fn without_kept_choices_the_window_keeps_its_own_from_the_first_change() {
    let keep = Held::default();
    let mut harness = harness_of(real_app().with_keep(Box::new(keep.clone())));
    harness.run();
    assert!(keep.stored.lock().unwrap().is_empty());
    harness.state_mut().mailbox_mut().set_inbox_view(InboxView::Grouped);
    harness.run();
    assert_eq!(
        *keep.stored.lock().unwrap(),
        [Triage {
            screener: ScreenerMode::Off,
            view: InboxView::Grouped,
            keep_pins: false,
            hold: Some(5),
            after: After::Next,
            greet_by: GreetBy::First
        }]
    );
}

#[test]
fn the_sample_takes_the_kept_view_but_never_keeps_its_screener_mode() {
    let keep = Held::default();
    *keep.triage.lock().unwrap() = Some(Triage {
        screener: ScreenerMode::Off,
        view: InboxView::List,
        keep_pins: false,
        hold: None,
        after: After::Next,
        greet_by: GreetBy::First,
    });
    let app = app().with_keep(Box::new(keep.clone()));
    assert_eq!(app.mailbox().screener_mode(), ScreenerMode::Before, "the sample keeps its own");
    assert_eq!(app.mailbox().inbox_view(), InboxView::List);
    let mut harness = harness_of(app);
    harness.state_mut().mailbox_mut().set_screener_mode(ScreenerMode::Inside);
    harness.run();
    assert!(keep.stored.lock().unwrap().is_empty(), "the owner's mode is not the sample's to change");
    harness.state_mut().mailbox_mut().set_inbox_view(InboxView::Smart);
    harness.run();
    assert_eq!(
        *keep.stored.lock().unwrap(),
        [Triage {
            screener: ScreenerMode::Off,
            view: InboxView::Smart,
            keep_pins: false,
            hold: Some(5),
            after: After::Next,
            greet_by: GreetBy::First
        }]
    );
    let fresh = Held::default();
    let mut harness = harness_of(self::app().with_keep(Box::new(fresh.clone())));
    harness.state_mut().mailbox_mut().set_inbox_view(InboxView::Grouped);
    harness.run();
    assert_eq!(
        *fresh.stored.lock().unwrap(),
        [Triage {
            screener: ScreenerMode::Off,
            view: InboxView::Grouped,
            keep_pins: false,
            hold: Some(5),
            after: After::Next,
            greet_by: GreetBy::First
        }]
    );
}

#[test]
fn a_triage_choice_that_cannot_be_kept_shows_why_once() {
    let keep = Held { fail: true, ..Held::default() };
    let mut harness = harness_of(real_app().with_keep(Box::new(keep.clone())));
    harness.state_mut().mailbox_mut().set_inbox_view(InboxView::Grouped);
    harness.run();
    harness.run();
    assert_eq!(keep.stored.lock().unwrap().len(), 1, "tried once per change");
    assert!(harness.state().mailbox().error().is_some_and(|error| error.contains("the disk is full")));
}

#[test]
fn the_kept_greeting_comes_back_and_a_change_of_it_is_kept() {
    let keep = Held::default();
    let kept =
        Triage { screener: ScreenerMode::Off, view: InboxView::List, keep_pins: false, hold: Some(5), after: After::Next, greet_by: GreetBy::Last };
    *keep.triage.lock().unwrap() = Some(kept);
    let app = real_app().with_keep(Box::new(keep.clone()));
    assert_eq!(app.mailbox().greet_by(), GreetBy::Last, "the kept greeting comes back at the start");
    let mut harness = harness_of(app);
    assert!(keep.stored.lock().unwrap().is_empty(), "what came back is not kept again");
    harness.state_mut().mailbox_mut().set_greet_by(GreetBy::None);
    harness.run();
    assert_eq!(*keep.stored.lock().unwrap(), [Triage { greet_by: GreetBy::None, ..kept }]);
}

#[test]
fn a_greeting_the_window_starts_with_is_not_kept_again() {
    let keep = Held::default();
    let mut app = real_app();
    app.mailbox_mut().set_greet_by(GreetBy::Full);
    let mut harness = harness_of(app.with_keep(Box::new(keep.clone())));
    harness.run();
    assert!(keep.stored.lock().unwrap().is_empty(), "the window's own greeting is what it starts from");
    harness.state_mut().mailbox_mut().set_greet_by(GreetBy::Last);
    harness.run();
    assert_eq!(keep.stored.lock().unwrap().last().map(|triage| triage.greet_by), Some(GreetBy::Last));
}

#[test]
fn new_senders_offers_the_screener_mode_over_the_queue() {
    let mut harness = previewing();
    assert_eq!(harness.get_by_role_and_label(Role::RadioButton, "Before the inbox for the screener").accesskit_node().toggled(), Some(Toggled::True));
    harness.get_by_role_and_label(Role::RadioButton, "Inside, marked for the screener").click();
    harness.run();
    assert_eq!(harness.state().mailbox().screener_mode(), ScreenerMode::Inside);
}

#[test]
fn the_held_header_keeps_to_a_narrow_window() {
    let mut harness = previewing();
    harness.set_size(vec2(420.0, 420.0));
    harness.run();
    harness.get_by_label("Let Harbour Swim Club in?");
    harness.get_by_role_and_label(Role::Button, "Block");
}

/// The tops of the queue's Let in buttons, top down.
fn let_in_tops(harness: &Harness<'static, InboxApp>) -> Vec<f32> {
    let mut tops: Vec<f32> = harness.query_all_by_role_and_label(Role::Button, "Let in").map(|node| node.rect().top()).collect();
    tops.sort_by(f32::total_cmp);
    tops
}

/// The colours of the rectangle edges painted in the last frame.
fn edges(harness: &Harness<'static, InboxApp>) -> Vec<egui::Color32> {
    fn leaves(shape: &egui::Shape, found: &mut Vec<egui::Color32>) {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| leaves(shape, found)),
            egui::Shape::Rect(rect) if rect.stroke.width > 0.0 => found.push(rect.stroke.color),
            _ => {}
        }
    }
    let mut found = Vec::new();
    harness.output().shapes.iter().for_each(|clipped| leaves(&clipped.shape, &mut found));
    found
}

#[test]
fn the_queue_in_a_room_narrower_than_a_cards_padding_draws_and_never_panics() {
    use crate::theme::{Mode, TextScale, Theme};
    // The room a hostile layout leaves the queue: none, a point, less than a card's 40 of padding.
    for width in [0.0, 1.0, 20.0, 39.0, 41.0] {
        let theme = Theme::new(Mode::Light, TextScale::default());
        let keymap = crate::keymap::Keymap::default();
        let mut mailbox = Mailbox::new(sample::session(clock()).unwrap(), clock());
        mailbox.open(Place::Senders);
        let mut harness = Harness::builder().with_size(vec2(440.0, 600.0)).with_context_setup(install_fonts).build_ui_state(
            move |ui, mailbox: &mut Mailbox| {
                crate::motion::set_reduced(ui.ctx(), true);
                ui.set_style(theme.style());
                let room = egui::Rect::from_min_size(ui.max_rect().min, vec2(width, 400.0));
                ui.scope_builder(egui::UiBuilder::new().max_rect(room), |ui| {
                    let _ = super::list::rows(ui, &theme, &keymap, mailbox);
                });
                // A card alone in that room, as a layout that squeezes the queue's column would give it.
                let newest = mailbox.messages().first().cloned();
                if let Some(newest) = newest {
                    ui.scope_builder(egui::UiBuilder::new().max_rect(room), |ui| {
                        let high = super::list::sender_card_of(ui, &theme, &keymap, mailbox, &newest);
                        assert!(high.is_finite() && high > 0.0, "{width}: {high}");
                    });
                }
            },
            mailbox,
        );
        harness.run();
        assert!(harness.query_all_by_role_and_label(Role::Button, "Let in").count() > 0, "{width}: the cards are drawn");
    }
}

/// The sample window with motion, a frame every hundredth of a second, on New senders.
fn queue_moving() -> Harness<'static, InboxApp> {
    let mut app = app();
    app.set_reduce_motion(false);
    let mut harness = Harness::builder()
        .with_size(vec2(1300.0, 900.0))
        .with_context_setup(install_fonts)
        .with_step_dt(1.0 / 100.0)
        .with_max_steps(400)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Review").click();
    harness.run();
    harness
}

#[test]
fn an_answered_card_left_mid_motion_does_not_play_again_when_new_senders_comes_back() {
    use crate::theme::{Mode, TextScale, Theme};
    let minus: Vec<egui::Color32> = [Mode::Light, Mode::Dark].map(|mode| Theme::new(mode, TextScale::default()).palette.minus).to_vec();
    let mut harness = queue_moving();
    harness.query_all_by_role_and_label(Role::Button, "Keep out").next().unwrap().click();
    harness.run_steps(5);
    // Away to the inbox mid-motion, a while there, then back.
    harness.state_mut().mailbox_mut().open(Place::View(inboxcraft_model::Role::Inbox));
    harness.run_steps(100);
    harness.state_mut().mailbox_mut().open(Place::Senders);
    harness.step();
    harness.step();
    assert!(!edges(&harness).iter().any(|colour| minus.contains(colour)), "no answered card fades again");
    assert_eq!(harness.query_all_by_role_and_label(Role::Button, "Let in").count(), 1, "only the sender who waits");
}

#[test]
fn a_second_answer_mid_motion_ends_the_first_cards_movement() {
    let mut harness = queue_moving();
    let tops = let_in_tops(&harness);
    let (first, second) = (tops[0], tops[1]);
    harness.query_all_by_role_and_label(Role::Button, "Keep out").next().unwrap().click();
    // While the first card leaves, the next one waits where it is: answer it too.
    harness.run_steps(3);
    assert_eq!(let_in_tops(&harness).last().copied(), Some(second));
    harness.query_all_by_role_and_label(Role::Button, "Let in").last().unwrap().click();
    harness.step();
    harness.step();
    assert!(harness.state().mailbox().messages().is_empty(), "both answered");
    // The click ends the first card's movement (M11): only the second is drawn, leaving from the first one's slot.
    let left = let_in_tops(&harness);
    assert_eq!(left.len(), 1, "only the second answered card is drawn, leaving: {left:?}");
    assert!(left[0] < second && left[0] <= first, "it leaves from the first card's slot, upwards: {left:?} {first}");
    harness.run_steps(80);
    harness.get_by_label("Nobody is waiting.");
}

#[test]
fn a_leaving_cards_subject_is_plain_text() {
    let mut harness = queue_moving();
    let links = |harness: &Harness<'static, InboxApp>| harness.query_all_by_role(Role::Link).count();
    let before = links(&harness);
    assert_eq!(before, 2, "each waiting card's subject opens its message");
    harness.query_all_by_role_and_label(Role::Button, "Keep out").next().unwrap().click();
    harness.step();
    harness.step();
    assert_eq!(links(&harness), 1, "the answered card, leaving, offers its subject to open no more");
}

#[test]
fn a_row_s_context_menu_keeps_tab_inside_and_esc_gives_the_row_the_keyboard() {
    let mut harness = harness_of(app());
    harness.get_by_label(ODILE).click_secondary();
    harness.run();
    let role = harness.get_by_label(ODILE).accesskit_node().role();
    crate::component::popup::tests::walk_an_open_menu(&mut harness, (role, ODILE));
    // A menu closing never changes the message shown: nothing was open, and nothing is.
    harness.run();
    assert_eq!(harness.state().mailbox().selected(), None, "the row the menu was on is not selected");
    assert_eq!(harness.state().reader().open_id(), None, "nor opened");
}

/// The rectangles painted in the last frame with an edge.
fn edged(harness: &Harness<'static, InboxApp>) -> Vec<egui::Rect> {
    fn leaves(shape: &egui::Shape, found: &mut Vec<egui::Rect>) {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| leaves(shape, found)),
            egui::Shape::Rect(rect) if rect.stroke.width > 0.0 => found.push(rect.rect),
            _ => {}
        }
    }
    let mut found = Vec::new();
    harness.output().shapes.iter().for_each(|clipped| leaves(&clipped.shape, &mut found));
    found
}

#[test]
fn the_new_senders_cards_are_the_design_files() {
    // At 1440: cards 900 × 140 (the current one, with its why-line) and 900 × 104; the avatar at +21,+17, the 1 px
    // edge counted; name, subject and preview on lines of 22, 22 and 18, 4 apart; the why-line 32 high and 620 wide;
    // the cards 166 under the title's box, which is 48 high.
    let mut harness = Harness::builder()
        .with_size(vec2(1440.0, 900.0))
        .with_context_setup(install_fonts)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app());
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Review").click();
    harness.run();
    harness.run();
    let mut cards: Vec<egui::Rect> = edged(&harness).into_iter().filter(|rect| rect.width() == 900.0).collect();
    cards.sort_by(|a, b| a.top().total_cmp(&b.top()));
    let sizes: Vec<_> = cards.iter().map(|rect| rect.size()).collect();
    assert_eq!(sizes, [vec2(900.0, 140.0), vec2(900.0, 104.0)], "{cards:?}");
    let card = cards[0];
    // The queue 56 in from the body, as the frame's 289 = 233 + 56 (#328): here the body starts at 242, 9 in for the
    // window's own frame, the index's 232 and its hairline.
    let index = harness.get_by_role_and_label(Role::Button, "Home").rect();
    assert_eq!(index.left(), 9.0, "the window's frame: {index:?}");
    assert_eq!(card.left(), 242.0 + 56.0, "{card:?}");
    let title = harness.get_by_label("Who gets in?").rect();
    assert_eq!((title.height(), card.top() - title.top()), (48.0, 166.0), "{title:?}");
    let at =
        |name: &str| harness.query_all_by_label(name).map(|node| node.rect()).find(|rect| card.contains_rect(*rect)).map(|rect| rect.min - card.min);
    let line = |name: &str| {
        harness
            .query_all_by_label(name)
            .map(|node| node.rect())
            .find(|rect| card.contains_rect(*rect))
            .map(|rect| (rect.min - card.min, rect.height()))
    };
    assert_eq!(line("Harbour Swim Club"), Some((vec2(77.0, 17.0), 22.0)), "the name");
    assert_eq!(
        line("Join us on Sunday morning for the first winter swim. Towels and hot tea on the quay afterwards.").map(|(at, high)| (at.y, high)),
        Some((69.0, 18.0)),
        "the preview"
    );
    let subject = harness
        .query_all_by_role(Role::Link)
        .map(|node| node.rect())
        .find(|rect| card.contains_rect(*rect))
        .map(|rect| (rect.min - card.min, rect.height()));
    assert_eq!(subject, Some((vec2(77.0, 43.0), 22.0)), "the subject");
    let why = edged(&harness).into_iter().find(|rect| card.contains_rect(*rect) && rect.height() < 40.0 && rect.width() > 300.0);
    assert_eq!(why.map(|rect| (rect.min - card.min, rect.size())), Some((vec2(77.0, 91.0), vec2(620.0, 32.0))), "the why-line");
    assert_eq!(at("Let in").map(|at| at.y), Some(17.0), "the answers on the first line's top");
    let avatar = harness.output().shapes.iter().find_map(|clipped| match &clipped.shape {
        egui::Shape::Rect(rect) if rect.rect.size() == vec2(40.0, 40.0) && card.contains_rect(rect.rect) => Some(rect.rect.min - card.min),
        _ => None,
    });
    assert_eq!(avatar, Some(vec2(21.0, 17.0)), "the avatar");
}

#[test]
fn the_new_senders_queue_says_the_design_files_words() {
    let mut harness = harness_of(app());
    harness.get_by_role_and_label(Role::Button, "Review").click();
    harness.run();
    harness.get_by_label("Nobody new reaches the inbox before you say so. If you keep a sender out, their mail goes to Blocked from then on.");
}

/// What has the keyboard: its name and its rect.
fn focused(harness: &Harness<'static, InboxApp>) -> Option<(String, egui::Rect)> {
    let id = harness.ctx.memory(egui::Memory::focused)?;
    let rect = harness.ctx.read_response(id)?.rect;
    let node = harness.query_all_by_role(Role::Button).chain(harness.query_all_by_role(Role::Link)).find(|node| node.rect() == rect)?;
    Some((node.accesskit_node().label().unwrap_or_default(), rect))
}

#[test]
fn on_a_new_senders_card_tab_reaches_let_in_before_keep_out_as_they_are_drawn() {
    // The owner's ruling: Let in first, read from the left as the design file draws it, and walked so; Shift Tab the
    // mirror. Wide (the answers at the card's right) and narrow (under the text).
    for width in [1440.0, 900.0] {
        let mut harness = Harness::builder()
            .with_size(vec2(width, 900.0))
            .with_context_setup(install_fonts)
            .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app());
        harness.run();
        harness.get_by_role_and_label(Role::Button, "Review").click();
        harness.run();
        harness.run();
        let mut subjects: Vec<egui::Rect> = harness.query_all_by_role(Role::Link).map(|node| node.rect()).collect();
        subjects.sort_by(|a, b| a.top().total_cmp(&b.top()));
        assert_eq!(subjects.len(), 2, "{width}: two cards");
        let answers = |name: &str| {
            let mut found: Vec<egui::Rect> = harness.query_all_by_role_and_label(Role::Button, name).map(|node| node.rect()).collect();
            found.sort_by(|a, b| a.top().total_cmp(&b.top()));
            found
        };
        let (lets, keeps) = (answers("Let in"), answers("Keep out"));
        assert_eq!((lets.len(), keeps.len()), (2, 2), "{width}");
        for (let_in, keep_out) in lets.iter().zip(&keeps) {
            assert!(let_in.right() < keep_out.left() && let_in.top() == keep_out.top(), "{width}: Let in {let_in:?} left of Keep out {keep_out:?}");
        }
        if width > 1000.0 {
            // Wide, the pair still ends at the card's right padding.
            let card = edged(&harness).into_iter().filter(|rect| rect.width() == 900.0).min_by(|a, b| a.top().total_cmp(&b.top())).unwrap();
            assert_eq!(keeps[0].right(), card.right() - 21.0, "{width}: Keep out ends 20 in from the 1 px edge");
        }
        // From the first card's subject: its Let in, its Keep out, the next card's subject, Let in, Keep out.
        let first = harness.query_all_by_role(Role::Link).find(|node| node.rect() == subjects[0]).unwrap();
        first.focus();
        harness.run();
        let wanted = [("Let in", lets[0]), ("Keep out", keeps[0]), ("", subjects[1]), ("Let in", lets[1]), ("Keep out", keeps[1])];
        let mut walked = Vec::new();
        for _ in 0..wanted.len() {
            harness.key_press(egui::Key::Tab);
            harness.run();
            walked.push(focused(&harness).map(|(_, rect)| rect));
        }
        assert_eq!(walked, wanted.map(|(_, rect)| Some(rect)), "{width}: Tab walks {:?}", wanted.map(|(name, _)| name));
        let mut back = Vec::new();
        for _ in 0..wanted.len() {
            harness.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::Tab);
            harness.run();
            back.push(focused(&harness).map(|(_, rect)| rect));
        }
        let mut mirror: Vec<_> = [subjects[0]].into_iter().chain(wanted.iter().map(|(_, rect)| *rect)).map(Some).collect();
        mirror.pop();
        mirror.reverse();
        assert_eq!(back, mirror, "{width}: Shift Tab walks back the same way");
    }
}
