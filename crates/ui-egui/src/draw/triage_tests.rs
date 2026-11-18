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
    harness.get_by_label("New senders");
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
    harness.get_by_label("club@example.org · first mail 06:00 · 1 message waiting");
    harness.get_by_label("Winter swim at the harbour steps");
    assert!(harness.query_by_role_and_label(Role::Button, "Reply").is_none(), "the preview has no Reply");
    assert!(harness.query_by_role_and_label(Role::Button, "Snooze").is_none(), "the sender stands in place of the toolbar");
    harness.get_by_role_and_label(Role::Button, "Details");
    for name in ["People", "Notifications", "Newsletters", "Accept", "Accept and open", "Block"] {
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
    harness.get_by_role_and_label(Role::Button, "Accept").click();
    harness.run();
    harness.get_by_label("Accepted Harbour Swim Club into Notifications");
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
    app.settings_mut().open_at(Section::Triage);
    harness_of(app)
}

#[test]
fn the_triage_page_changes_the_screener_and_the_grouping() {
    let mut harness = settings_on_triage();
    harness.get_by_label("Triage, settings page");
    harness.get_by_label("[ C ] Grouped inbox");
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
    InboxApp::new(Mailbox::new(session, clock()))
}

#[test]
fn the_kept_triage_choices_come_back_and_each_change_is_kept() {
    let keep = Held::default();
    *keep.triage.lock().unwrap() = Some(Triage { screener: ScreenerMode::Inside, view: InboxView::Grouped, keep_pins: false, hold: None });
    let app = real_app().with_keep(Box::new(keep.clone()));
    assert_eq!(app.mailbox().screener_mode(), ScreenerMode::Inside);
    assert_eq!(app.mailbox().inbox_view(), InboxView::Grouped);
    let mut harness = harness_of(app);
    assert!(keep.stored.lock().unwrap().is_empty(), "nothing changed yet");
    harness.state_mut().mailbox_mut().set_inbox_view(InboxView::List);
    harness.run();
    assert_eq!(*keep.stored.lock().unwrap(), [Triage { screener: ScreenerMode::Inside, view: InboxView::List, keep_pins: false, hold: Some(5) }]);
    harness.state_mut().mailbox_mut().set_screener_mode(ScreenerMode::Off);
    harness.run();
    assert_eq!(
        keep.stored.lock().unwrap().last(),
        Some(&Triage { screener: ScreenerMode::Off, view: InboxView::List, keep_pins: false, hold: Some(5) })
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
    assert_eq!(*keep.stored.lock().unwrap(), [Triage { screener: ScreenerMode::Off, view: InboxView::Grouped, keep_pins: false, hold: Some(5) }]);
}

#[test]
fn the_sample_takes_the_kept_view_but_never_keeps_its_screener_mode() {
    let keep = Held::default();
    *keep.triage.lock().unwrap() = Some(Triage { screener: ScreenerMode::Off, view: InboxView::List, keep_pins: false, hold: None });
    let app = app().with_keep(Box::new(keep.clone()));
    assert_eq!(app.mailbox().screener_mode(), ScreenerMode::Before, "the sample keeps its own");
    assert_eq!(app.mailbox().inbox_view(), InboxView::List);
    let mut harness = harness_of(app);
    harness.state_mut().mailbox_mut().set_screener_mode(ScreenerMode::Inside);
    harness.run();
    assert!(keep.stored.lock().unwrap().is_empty(), "the owner's mode is not the sample's to change");
    harness.state_mut().mailbox_mut().set_inbox_view(InboxView::Smart);
    harness.run();
    assert_eq!(*keep.stored.lock().unwrap(), [Triage { screener: ScreenerMode::Off, view: InboxView::Smart, keep_pins: false, hold: Some(5) }]);
    let fresh = Held::default();
    let mut harness = harness_of(self::app().with_keep(Box::new(fresh.clone())));
    harness.state_mut().mailbox_mut().set_inbox_view(InboxView::Grouped);
    harness.run();
    assert_eq!(*fresh.stored.lock().unwrap(), [Triage { screener: ScreenerMode::Off, view: InboxView::Grouped, keep_pins: false, hold: Some(5) }]);
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
fn an_answered_card_takes_its_answers_colour_fades_and_the_next_steps_up() {
    use crate::theme::{Mode, TextScale, Theme};
    let minus: Vec<egui::Color32> = [Mode::Light, Mode::Dark].map(|mode| Theme::new(mode, TextScale::default()).palette.minus).to_vec();
    for motion in [true, false] {
        let mut app = app();
        app.set_reduce_motion(!motion);
        let mut harness = Harness::builder()
            .with_size(vec2(1300.0, 900.0))
            .with_context_setup(install_fonts)
            .with_step_dt(1.0 / 100.0)
            .with_max_steps(200)
            .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
        harness.run();
        harness.get_by_role_and_label(Role::Button, "Review").click();
        harness.run();
        let tops = let_in_tops(&harness);
        let (first, second) = (tops[0], tops[1]);
        harness.query_all_by_role_and_label(Role::Button, "Keep out").next().unwrap().click();
        harness.step();
        harness.step();
        assert!(harness.state().mailbox().can_undo(), "the answer acts at once");
        if !motion {
            // Off: the next card is there at once.
            harness.run();
            assert_eq!(let_in_tops(&harness), vec![first], "motion off: the next card stands in the answered one's place");
            continue;
        }
        // 0 to 125 ms the edge eases to the answer's colour, as the presentation's border does: not there at once.
        assert!(!edges(&harness).iter().any(|colour| minus.contains(colour)), "the edge eases in, it does not snap");
        // Up to 160 ms: the card stays, its edge in the answer's colour (Keep out: `minus`), the next card where it was.
        harness.run_steps(13);
        assert!(edges(&harness).iter().any(|colour| minus.contains(colour)), "the edge takes Keep out's colour");
        assert_eq!(let_in_tops(&harness).last().copied(), Some(second), "the next card waits while the answer shows");
        // 160 to 285 ms: it fades at an even pace.
        harness.run_steps(10);
        let faded_minus = |colour: &egui::Color32| {
            let alpha = f32::from(colour.a()) / 255.0;
            colour.a() > 0
                && colour.a() < 255
                && minus.iter().any(|m| {
                    [(colour.r(), m.r()), (colour.g(), m.g()), (colour.b(), m.b())]
                        .iter()
                        .all(|(seen, full)| (f32::from(*seen) - f32::from(*full) * alpha).abs() <= 2.0)
                })
        };
        assert!(edges(&harness).iter().any(faded_minus), "half faded: {:?}", edges(&harness));
        assert_eq!(let_in_tops(&harness).last().copied(), Some(second));
        // 285 to 410 ms: its place closes over 125 ms on the design's curve, as the presentation's reduced path does,
        // and the next card steps up.
        harness.run_steps(7);
        let mid = let_in_tops(&harness).last().copied().unwrap();
        assert!(mid < second && mid > first, "the next card is stepping up: {first} < {mid} < {second}");
        assert!(!edges(&harness).iter().any(|colour| minus.contains(colour)), "the answered card is no longer drawn");
        harness.run_steps(12);
        assert_eq!(let_in_tops(&harness), vec![first], "by 440 ms the next card stands in the answered one's place");
        // The card that stepped up under the pointer takes its hover in the next two frames; then it rests.
        harness.run_steps(2);
        assert!(crate::component::tests::rests(&mut harness, 4), "and it rests");
    }
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
fn a_second_answer_mid_motion_leaves_the_first_card_where_it_is() {
    let mut harness = queue_moving();
    let second = let_in_tops(&harness)[1];
    harness.query_all_by_role_and_label(Role::Button, "Keep out").next().unwrap().click();
    // While the first card shows its answer, the next one waits where it is: answer it too.
    harness.run_steps(3);
    assert_eq!(let_in_tops(&harness).last().copied(), Some(second));
    harness.query_all_by_role_and_label(Role::Button, "Let in").last().unwrap().click();
    harness.step();
    harness.step();
    assert!(harness.state().mailbox().messages().is_empty(), "both answered");
    // The first card is still there, so the second, leaving too, has not jumped into its place.
    assert_eq!(let_in_tops(&harness).last().copied(), Some(second), "the second card stays below the first");
    assert_eq!(let_in_tops(&harness).len(), 2, "both answered cards are still drawn, leaving");
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
