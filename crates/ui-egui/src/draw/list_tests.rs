// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The message list on the design system: the list card, the grouped inbox and screener cards shown by the showcase
//! `ui_shot` fills, and the hover actions, which act on a real account as on the sample.

use egui::accesskit::Role;
use egui::{Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_engine::Session;
use inboxcraft_time::{FakeClock, Timestamp};

use super::{InboxApp, TOAST_SECONDS};
use crate::mailbox::{InboxView, Mailbox, Place};
use crate::sample;
use crate::theme::install_fonts;

/// The parts of the group cards, which the simple list leaves out.
const CARDS_ONLY: [&str; 4] = ["Done all", "Show all (7)", "People", "Updates"];

const ODILE: &str = "Odile Brannock, Allotment rota for November, 08:35, unread";
const ODILE_READ: &str = "Odile Brannock, Allotment rota for November, 08:35";
const TEODOR: &str = "Teodor Lisk, Photos from the coast walk, 04:00, flagged";
const TEODOR_UNREAD: &str = "Teodor Lisk, Photos from the coast walk, 04:00, unread, flagged";

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

/// The sample window: the simple list, or with `showcase` the invented cards.
fn app(showcase: bool) -> InboxApp {
    let mut mailbox = Mailbox::new(sample::session(clock()).unwrap(), clock());
    if showcase {
        let invented = sample::showcase(&mailbox);
        mailbox.set_showcase(invented);
    } else {
        mailbox.set_inbox_view(InboxView::Simple);
    }
    InboxApp::new(mailbox)
}

fn harness_of(app: InboxApp) -> Harness<'static, InboxApp> {
    let mut harness =
        Harness::builder().with_size(vec2(1300.0, 900.0)).with_context_setup(install_fonts).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

fn harness(showcase: bool) -> Harness<'static, InboxApp> {
    harness_of(app(showcase))
}

/// The sample window with reduced motion, so every act lands in the frame after it.
fn still() -> Harness<'static, InboxApp> {
    let mut app = app(false);
    app.set_reduce_motion(true);
    harness_of(app)
}

/// The sample window with motion, a frame every sixtieth of a second.
fn moving() -> Harness<'static, InboxApp> {
    let mut app = app(false);
    app.set_reduce_motion(false);
    let mut harness = Harness::builder()
        .with_size(vec2(1300.0, 900.0))
        .with_context_setup(install_fonts)
        .with_step_dt(1.0 / 60.0)
        .with_max_steps(60)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run_steps(2);
    harness
}

fn rows(harness: &Harness<'static, InboxApp>) -> usize {
    harness.query_all_by_role(Role::ListBoxOption).count()
}

/// Presses the hover action `name` of the row `row`.
fn press(harness: &mut Harness<'static, InboxApp>, row: &str, name: &str) {
    harness.get_by_label(row).hover();
    harness.run();
    harness.get_by_role_and_label(Role::Button, name).click();
    harness.run();
}

fn undo(harness: &mut Harness<'static, InboxApp>) {
    harness.get_by_role_and_label(Role::Button, "Undo").click();
    harness.run();
}

fn quick_tooltips(harness: &Harness<'static, InboxApp>) {
    harness.ctx.all_styles_mut(|style| {
        style.interaction.tooltip_delay = 0.0;
        style.interaction.show_tooltips_only_when_still = false;
    });
}

#[test]
fn the_simple_list_draws_no_group_card_but_the_screener_card() {
    let harness = harness(false);
    for name in CARDS_ONLY {
        assert!(harness.query_by_label(name).is_none(), "{name} shows in the simple list");
    }
    harness.get_by_label("2 new senders");
    assert_eq!(rows(&harness), 11);
}

#[test]
fn on_a_real_account_the_hover_actions_and_their_keys_act_too() {
    // The same mail in a session over a store, as an account's would be: not the sample.
    let session = Session::new(Box::new(sample::mailbox(Timestamp::from_unix(1_793_869_200, 0).unwrap()).unwrap()), clock()).unwrap();
    let mut app = InboxApp::new(Mailbox::new(session, clock()));
    app.set_reduce_motion(true);
    app.mailbox_mut().set_inbox_view(InboxView::Simple);
    let mut harness = harness_of(app);
    press(&mut harness, ODILE, "Mark as read");
    harness.get_by_label(ODILE_READ);
    harness.get_by_label("Marked as read");
    undo(&mut harness);
    harness.get_by_label(ODILE);
    press(&mut harness, TEODOR, "Delete");
    assert!(harness.query_by_label(TEODOR).is_none());
    assert_eq!(rows(&harness), 10);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run();
    harness.get_by_label(TEODOR);
}

#[test]
fn read_and_unread_and_their_undo_act_on_the_sample() {
    let mut harness = still();
    press(&mut harness, ODILE, "Mark as read");
    harness.get_by_label(ODILE_READ);
    harness.get_by_label("Marked as read");
    undo(&mut harness);
    harness.get_by_label(ODILE);
    assert!(harness.query_by_label("Undo").is_none(), "the toast goes with its act");
    press(&mut harness, TEODOR, "Mark as unread");
    harness.get_by_label(TEODOR_UNREAD);
    harness.get_by_label("Marked as unread");
    undo(&mut harness);
    harness.get_by_label(TEODOR);
}

#[test]
fn done_and_delete_and_their_undo_act_on_the_sample() {
    let mut harness = still();
    press(&mut harness, ODILE, "Done");
    assert!(harness.query_by_label(ODILE).is_none(), "the row has left the inbox");
    assert_eq!(rows(&harness), 10);
    harness.get_by_label("Marked as done");
    undo(&mut harness);
    harness.get_by_label(ODILE);
    assert_eq!(rows(&harness), 11);
    press(&mut harness, TEODOR, "Delete");
    assert!(harness.query_by_label(TEODOR).is_none());
    harness.get_by_label("Moved to Trash");
    harness.state_mut().mailbox_mut().open(Place::View(inboxcraft_model::Role::Trash));
    harness.run();
    assert!(harness.query_all_by_label("Teodor Lisk, Photos from the coast walk, 04:00, flagged").count() == 1, "in the Trash");
    undo(&mut harness);
    assert_eq!(rows(&harness), 2, "only the two newsletters are left in the Trash");
}

#[test]
fn the_keys_act_on_the_keyboard_selected_row_and_the_undo_key_undoes() {
    let mut harness = still();
    harness.key_press(Key::ArrowDown);
    harness.run();
    harness.get_by_label(ODILE_READ);
    harness.key_press(Key::R);
    harness.run();
    harness.get_by_label(ODILE);
    harness.key_press(Key::R);
    harness.run();
    harness.get_by_label(ODILE_READ);
    harness.key_press(Key::S);
    harness.run();
    harness.get_by_label("Odile Brannock, Allotment rota for November, 08:35, flagged");
    harness.key_press(Key::S);
    harness.run();
    harness.get_by_label(ODILE_READ);
    harness.key_press(Key::D);
    harness.run();
    assert!(harness.query_by_label(ODILE_READ).is_none());
    harness.get_by_label("Ctrl Z");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run();
    harness.get_by_label(ODILE_READ);
    let rota = harness.state().mailbox().messages()[0].id;
    assert_eq!(harness.state().mailbox().selected(), Some(rota), "selected again");
    harness.key_press(Key::A);
    harness.run();
    assert!(harness.query_by_label(ODILE_READ).is_none());
    harness.get_by_label("Moved to Archive");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run();
    harness.key_press(Key::Delete);
    harness.run();
    assert!(harness.query_by_label(ODILE_READ).is_none());
    harness.get_by_label("Moved to Trash");
}

#[test]
fn keys_typed_into_the_search_field_do_not_act() {
    let mut harness = still();
    harness.key_press(Key::ArrowDown);
    harness.run();
    harness.key_press(Key::D);
    harness.run();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::F);
    harness.run();
    assert!(harness.query_by_label("Ctrl Z").is_none(), "Undo's key belongs to the field while it has the focus");
    harness.key_press(Key::R);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run();
    assert!(harness.query_by_label(ODILE).is_none(), "Ctrl+Z in the field undid nothing");
    harness.get_by_label("Marked as done");
}

#[test]
fn the_slide_out_and_the_toast_obey_the_reduced_motion_flag() {
    // Reduced motion: the row is out in the frame after the press, the toast stands at its place at once.
    let mut harness = still();
    harness.get_by_label(ODILE).hover();
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Done").click();
    // The press, the slide (done at once), the list without the row and the toast's measuring frame, the toast.
    harness.run_steps(4);
    assert!(harness.query_by_label(ODILE).is_none());
    let at_once = harness.get_by_role_and_label(Role::Button, "Undo").rect();
    harness.run();
    assert_eq!(harness.get_by_role_and_label(Role::Button, "Undo").rect(), at_once);
    // Motion: the row slides out over the row slide token, the toast rises into its place after.
    let mut harness = moving();
    harness.get_by_label(ODILE).hover();
    harness.run_steps(2);
    harness.get_by_role_and_label(Role::Button, "Done").click();
    harness.run_steps(3);
    assert!(harness.query_by_label(ODILE).is_some(), "still sliding");
    assert!(harness.query_by_label("Undo").is_none());
    harness.run_steps(12);
    assert!(harness.query_by_label(ODILE).is_none(), "out after the slide");
    let rising = harness.get_by_role_and_label(Role::Button, "Undo").rect();
    harness.run_steps(20);
    let risen = harness.get_by_role_and_label(Role::Button, "Undo").rect();
    assert!(rising.top() > risen.top(), "the toast rose from {rising:?} to {risen:?}");
    assert_eq!(risen.top(), at_once.top(), "to where it stands at once with reduced motion");
}

#[test]
fn the_toast_goes_after_its_time_and_its_act_can_no_longer_be_undone() {
    let mut harness = still();
    press(&mut harness, ODILE, "Mark as read");
    harness.get_by_label("Marked as read");
    // A frame every quarter of a second.
    let steps = (TOAST_SECONDS * 4.0) as usize;
    harness.run_steps(steps / 2);
    harness.get_by_label("Marked as read");
    harness.run_steps(steps);
    assert!(harness.query_by_label("Marked as read").is_none());
    assert_eq!(harness.state().mailbox().undoable(), None);
}

#[test]
fn the_showcase_shows_the_screener_and_the_group_cards_with_their_rows() {
    let harness = harness(true);
    harness.get_by_label("2 new senders");
    harness.get_by_role_and_label(Role::Button, "Review");
    harness.get_by_label("People, 2 unread");
    harness.get_by_label("Updates, 1 unread");
    assert_eq!(harness.query_all_by_role_and_label(Role::Button, "Done all").count(), 2);
    // People holds seven rows and shows three of them; Updates its three; Kasimir's row stays out of the groups.
    harness.get_by_role_and_label(Role::Button, "Show all (7)");
    assert!(harness.query_by_label("Show all (3)").is_none());
    assert_eq!(harness.query_all_by_role(Role::ListBoxOption).count(), 3 + 3 + 1);
    harness.get_by_label("Kasimir Vell, Studio open day: volunteers?, Yesterday, unread");
}

#[test]
fn show_all_lists_every_row_of_a_long_group() {
    let mut harness = harness(true);
    harness.get_by_role_and_label(Role::Button, "Show all (7)").click();
    harness.run();
    assert_eq!(harness.query_all_by_role(Role::ListBoxOption).count(), 7 + 3 + 1);
    harness.get_by_role_and_label(Role::Button, "Show fewer").click();
    harness.run();
    assert_eq!(harness.query_all_by_role(Role::ListBoxOption).count(), 3 + 3 + 1);
}

#[test]
fn a_hovered_row_shows_its_actions_at_its_right_end() {
    let mut harness = harness(true);
    assert!(harness.query_by_label("Done").is_none());
    harness.get_by_label(ODILE).hover();
    harness.run();
    let row = harness.get_by_label(ODILE).rect();
    for name in ["Done", "Mark as read", "Delete"] {
        let button = harness.get_by_role_and_label(Role::Button, name).rect();
        assert!(row.contains(button.center()) && button.left() > row.center().x, "{name} {button:?} in {row:?}");
    }
}

#[test]
fn keyboard_selection_shows_the_same_actions_on_the_selected_row() {
    let mut harness = harness(true);
    harness.key_press(Key::ArrowDown);
    harness.run();
    let selected = harness.state().mailbox().selected().unwrap();
    let row = harness.state().mailbox().messages().iter().find(|row| row.id == selected).unwrap().clone();
    let spoken = harness.state().mailbox().spoken(&row);
    let rect = harness.get_by_label(&spoken).rect();
    let done: Vec<_> = harness.query_all_by_role_and_label(Role::Button, "Done").map(|node| node.rect()).collect();
    assert!(done.iter().any(|done| rect.contains(done.center())), "{done:?} in {rect:?}");
    // A read row offers to mark it unread.
    harness.get_by_role_and_label(Role::Button, if row.seen { "Mark as unread" } else { "Mark as read" });
}

#[test]
fn the_hover_actions_stand_clear_of_the_row_text_and_the_time_gives_way() {
    // The design's group has no float of its own: the row cuts its sender short of it and hides its time.
    let mut harness = harness(true);
    harness.get_by_label(ODILE).hover();
    harness.run();
    let row = harness.get_by_label(ODILE).rect();
    let time = date_of(&harness, "Allotment rota for November");
    let buttons: Vec<egui::Rect> = ["Done", "Mark as read", "Delete", "Snooze"]
        .iter()
        .flat_map(|name| harness.query_all_by_role_and_label(Role::Button, name).map(|node| node.rect()).collect::<Vec<_>>())
        .filter(|rect| row.contains(rect.center()))
        .collect();
    assert_eq!(buttons.len(), 4, "the row's group");
    let mut seen = 0;
    for clipped in &harness.output().shapes {
        let egui::Shape::Text(text) = &clipped.shape else { continue };
        let rect = text.visual_bounding_rect();
        if !row.contains(rect.left_center()) {
            continue;
        }
        let words = &text.galley.job.text;
        assert!(*words != time, "the time is hidden while the actions show");
        if words.starts_with("Odile") || words.starts_with("Allotment") {
            seen += 1;
            for button in &buttons {
                assert!(!rect.intersects(*button), "{words:?} at {rect:?} runs under an action at {button:?}");
            }
        }
    }
    assert!(seen >= 2, "the sender and the subject were drawn");
}

#[test]
fn every_button_of_the_list_and_the_rail_has_a_tooltip_with_its_key_chip() {
    let mut harness = harness(true);
    quick_tooltips(&harness);
    harness.get_by_label(ODILE).hover();
    harness.run();
    for (name, chip) in [("Done", "D"), ("Mark as read", "R"), ("Mail", "Ctrl 1"), ("Appearance settings", "Ctrl Comma")] {
        harness.get_by_role_and_label(Role::Button, name).hover();
        harness.run();
        assert!(harness.query_all_by_label(chip).count() >= 1, "{name} shows {chip}");
    }
    for name in ["Done all", "Review", "Show all (7)"] {
        let before = harness.query_all_by_label(name).count();
        harness.query_all_by_role_and_label(Role::Button, name).next().unwrap().hover();
        harness.run();
        assert_eq!(harness.query_all_by_label(name).count(), before + 1, "{name} has a tooltip");
    }
}

#[test]
fn ctrl_comma_opens_the_settings_and_the_mail_button_or_ctrl_1_leaves_them() {
    let mut harness = harness(false);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Comma);
    harness.run();
    assert!(harness.state().settings().is_open());
    harness.get_by_role_and_label(Role::Button, "Mail").click();
    harness.run();
    assert!(!harness.state().settings().is_open());
    harness.get_by_label("Appearance settings").click();
    harness.run();
    assert!(harness.state().settings().is_open());
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Num1);
    harness.run();
    assert!(!harness.state().settings().is_open());
}

/// A keep that holds reduced motion and counts what it is asked to keep.
struct Counting(std::sync::Arc<std::sync::atomic::AtomicUsize>);

impl crate::keep::Keep for Counting {
    fn load(&self) -> Option<inboxcraft_engine::appearance::Appearance> {
        use inboxcraft_engine::appearance::{Appearance, Motion};
        Some(Appearance { motion: Motion::Reduce, ..Appearance::default() })
    }

    fn store(&mut self, _: &inboxcraft_engine::appearance::Appearance) -> Result<(), String> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}

#[test]
fn acting_on_the_sample_keeps_nothing() {
    let stores = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut harness = harness_of(app(false).with_keep(Box::new(Counting(stores.clone()))));
    for (row, name) in [(ODILE, "Mark as read"), (TEODOR, "Mark as unread"), (ODILE, "Done"), (TEODOR, "Delete")] {
        press(&mut harness, row, name);
        undo(&mut harness);
    }
    assert_eq!(stores.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert_eq!(rows(&harness), 11);
}

/// The account stripes of the last frame: the thin upright bars at the left of the rows.
fn stripes(harness: &Harness<'_, InboxApp>) -> usize {
    fn count(shape: &egui::Shape) -> usize {
        match shape {
            egui::Shape::Vec(inner) => inner.iter().map(count).sum(),
            egui::Shape::Rect(rect) if (rect.rect.width() - 3.0).abs() < 0.01 && rect.rect.height() > 10.0 => 1,
            _ => 0,
        }
    }
    harness.output().shapes.iter().map(|clipped| count(&clipped.shape)).sum()
}

#[test]
fn the_account_stripe_marks_the_rows_of_the_simple_list_only() {
    assert!(stripes(&harness(false)) > 0, "the simple list of two accounts shows whose each row is");
    assert_eq!(stripes(&harness(true)), 0, "the grouped inbox's cards show no stripes");
}

/// The text `wanted` of the last frame: where it is drawn and its colour.
fn painted(harness: &Harness<'_, InboxApp>, wanted: &str) -> Option<(egui::Rect, egui::Color32)> {
    fn find(shape: &egui::Shape, wanted: &str) -> Option<(egui::Rect, egui::Color32)> {
        match shape {
            egui::Shape::Vec(inner) => inner.iter().find_map(|shape| find(shape, wanted)),
            egui::Shape::Text(text) if text.galley.text() == wanted => {
                let colour = text.galley.job.sections.first().map_or(text.fallback_color, |section| section.format.color);
                Some((shape.visual_bounding_rect(), colour))
            }
            _ => None,
        }
    }
    harness.output().shapes.iter().find_map(|clipped| find(&clipped.shape, wanted))
}

#[test]
fn show_all_is_an_accent_link_a_little_left_of_the_rows_text() {
    let harness = harness(true);
    let (text, colour) = painted(&harness, "Show all (7)").expect("Show all is drawn");
    let theme = crate::theme::Theme::current(&harness.ctx, &crate::theme::Look::of(harness.state().mailbox().appearance()));
    assert_eq!(colour, theme.palette.cyan_text, "Show all is in the accent colour");
    let row = harness.get_by_label(ODILE).rect();
    let sender = painted(&harness, "Odile Brannock").expect("the sender is drawn").0;
    assert!(sender.left() > row.left() + 20.0, "the sender stands right of the avatar");
    let wanted = sender.left() - theme.spacing.s4;
    assert!((text.left() - wanted).abs() <= 1.5, "Show all starts at {}, a step left of the rows' text at {}", text.left(), sender.left());
}

#[test]
fn a_hovered_row_offers_done_read_delete_and_snooze_in_that_order_snooze_not_yet() {
    let mut harness = harness(true);
    harness.get_by_label(ODILE).hover();
    harness.run();
    let row = harness.get_by_label(ODILE).rect();
    let mut lefts = Vec::new();
    for name in ["Done", "Mark as read", "Delete", "Snooze"] {
        let button = harness.query_all_by_role_and_label(Role::Button, name).find(|node| row.contains(node.rect().center())).expect(name);
        assert_eq!(button.accesskit_node().is_disabled(), name == "Snooze", "{name}");
        lefts.push(button.rect().left());
    }
    assert!(lefts.windows(2).all(|pair| pair[0] < pair[1]), "in the design's order: {lefts:?}");
}

#[test]
fn the_view_switch_shows_over_the_wide_inbox_and_changes_the_view() {
    let mut harness = harness(true);
    harness.get_by_role_and_label(Role::Button, "Simple list").click();
    harness.run();
    assert_eq!(harness.state().mailbox().inbox_view(), InboxView::Simple);
    harness.get_by_role_and_label(Role::Button, "Grouped").click();
    harness.run();
    assert_eq!(harness.state().mailbox().inbox_view(), InboxView::List, "Grouped is the one list under headings");
    harness.get_by_role_and_label(Role::Button, "Cards").click();
    harness.run();
    assert_eq!(harness.state().mailbox().inbox_view(), InboxView::Cards);
}

#[test]
fn the_view_switch_leaves_the_narrow_list_beside_an_open_message() {
    let mut harness = harness(true);
    harness.get_by_role_and_label(Role::Button, "Cards");
    harness.get_by_label(ODILE).click();
    harness.run();
    assert!(harness.query_by_role_and_label(Role::Button, "Cards").is_none());
    harness.get_by_role_and_label(Role::Button, "Show done");
}

#[test]
fn the_lists_keep_working_when_they_overflow_a_small_window() {
    for showcase in [true, false] {
        let mut harness = Harness::builder()
            .with_size(vec2(760.0, 260.0))
            .with_context_setup(install_fonts)
            .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app(showcase));
        harness.run();
        harness.run();
        harness.get_by_label(ODILE);
    }
}

#[test]
fn a_row_without_a_preview_has_no_dot_after_its_subject() {
    assert_eq!(super::list::after_subject(""), "");
    assert_eq!(super::list::after_subject("  \t"), "");
    assert_eq!(super::list::after_subject("Hi all"), " — Hi all");
}

#[test]
fn the_list_counts_the_rows_it_built_for_the_bench() {
    let harness = harness(false);
    let built = crate::rows_built(&harness.ctx).unwrap();
    let all = harness.state().mailbox().messages().len();
    assert!(built > 0 && built <= all, "{built} of {all}");
    assert_eq!(crate::rows_built(&harness.ctx), None, "read once");
}

/// Archive is no example here: the reader's toolbar offers it whenever a message is open.
#[test]
fn the_key_of_an_action_only_a_coming_soon_control_offers_runs_nothing() {
    for normal in [false, true] {
        let mut harness = still();
        crate::component::tests::offer_soon(&harness.ctx, crate::keymap::PALETTE, normal);
        harness.run();
        assert_eq!(crate::component::soon::blocked(&harness.ctx, crate::keymap::PALETTE), !normal);
        harness.key_press_modifiers(Modifiers::COMMAND, Key::K);
        harness.run();
        assert_eq!(harness.state().palette().is_open(), normal, "its key opens the palette, a usable control offering it too: {normal}");
    }
}

/// Opens Teodor's message with a pointer press on its row, so the list stands beside the reader at its default width.
fn beside_the_reader() -> Harness<'static, InboxApp> {
    let mut harness = harness(false);
    harness.query_all_by_label_contains("Photos from the coast walk").next().unwrap().click();
    harness.run();
    assert!(harness.state().reader().open_id().is_some(), "Teodor's message is open");
    harness
}

/// Presses Odile's row in its middle with the pointer, the way a person does: the pointer comes, the row shows its
/// hover actions, then the press. The row opens and nothing is acted on.
fn press_odile_in_the_middle(harness: &mut Harness<'static, InboxApp>, width: &str) {
    let before = harness.state().reader().open_id();
    let middle = harness.get_by_label(ODILE).rect().center();
    harness.hover_at(middle);
    harness.run();
    for name in ["Done", "Mark as read", "Delete", "Snooze"] {
        for button in harness.query_all_by_role_and_label(Role::Button, name) {
            assert!(!button.rect().contains(middle), "{width}: {name} at {:?} lies under the row's middle {middle:?}", button.rect());
        }
    }
    harness.get_by_label(ODILE).click();
    harness.run();
    assert!(harness.query_by_label("Marked as done").is_none(), "{width}: nothing was done");
    assert_ne!(harness.state().reader().open_id(), before, "{width}: Odile's message opened");
}

#[test]
fn a_pointer_press_on_a_rows_middle_opens_it_at_the_default_and_the_narrowest_list() {
    let mut harness = beside_the_reader();
    let row = harness.get_by_label(ODILE).rect();
    assert!((row.width() - (380.0 - 36.0)).abs() <= 12.0, "the default list: {row:?}");
    press_odile_in_the_middle(&mut harness, "380");
    // The list's edge dragged as far left as it goes.
    let mut harness = beside_the_reader();
    let row = harness.get_by_label(ODILE).rect();
    let y = row.center().y;
    let mut narrowest = row;
    for x in 1..40 {
        let from = egui::pos2(row.right() + x as f32, y);
        harness.hover_at(from);
        harness.drag_at(from);
        harness.run();
        harness.hover_at(egui::pos2(10.0, y));
        harness.run();
        harness.drop_at(egui::pos2(10.0, y));
        harness.run();
        narrowest = harness.get_by_label(ODILE).rect();
        if narrowest.width() < row.width() - 100.0 {
            break;
        }
    }
    assert!(narrowest.width() < 120.0, "the list is at its narrowest: {narrowest:?}");
    press_odile_in_the_middle(&mut harness, "narrowest");
}

#[test]
fn a_pointer_press_on_done_beside_the_reader_marks_the_message_done() {
    let mut harness = beside_the_reader();
    let row = harness.get_by_label(ODILE).rect();
    harness.hover_at(row.center());
    harness.run();
    // The row's own Done, not the reader's.
    let done = harness.query_all_by_role_and_label(Role::Button, "Done").map(|node| node.rect()).find(|rect| row.contains(rect.center()));
    let done = done.unwrap_or_else(|| panic!("no Done on the hovered row"));
    harness.hover_at(done.center());
    harness.run();
    harness.query_all_by_role_and_label(Role::Button, "Done").find(|node| node.rect() == done).unwrap().click();
    // The row slides out over a few frames.
    harness.run_steps(12);
    harness.get_by_label("Marked as done");
    assert!(harness.query_by_label(ODILE).is_none(), "the row has left the inbox");
}

#[test]
fn a_hovered_rows_snooze_shows_its_key_as_the_sheet_does_though_it_cannot_run_yet() {
    let mut harness = harness(true);
    harness.get_by_label(ODILE).hover();
    harness.run();
    let row = harness.get_by_label(ODILE).rect();
    let width = |harness: &Harness<'static, InboxApp>, name: &str| {
        harness
            .query_all_by_role_and_label(Role::Button, name)
            .map(|node| node.rect())
            .find(|rect| row.contains(rect.center()))
            .map(|rect| rect.width())
    };
    // Its icon and its dashed one-letter chip, as wide as Done with its own.
    assert_eq!(width(&harness, "Snooze"), width(&harness, "Done"));
}

/// The date the list writes for the message with `subject`.
fn date_of(harness: &Harness<'static, InboxApp>, subject: &str) -> String {
    let mailbox = harness.state().mailbox();
    let row = mailbox.messages().iter().find(|row| row.subject == subject).cloned();
    row.map(|row| mailbox.row_date(&row)).unwrap_or_default()
}

/// The texts the last frame drew whose left middle lies in `rect`: `(rect, words)`.
fn texts_in(harness: &Harness<'static, InboxApp>, rect: egui::Rect) -> Vec<(egui::Rect, String)> {
    harness
        .output()
        .shapes
        .iter()
        .filter_map(|clipped| match &clipped.shape {
            egui::Shape::Text(text) => Some((text.visual_bounding_rect(), text.galley.job.text.clone())),
            _ => None,
        })
        .filter(|(at, _)| rect.contains(at.left_center()))
        .collect()
}

/// The rects of the buttons called `name` in `rect`.
fn buttons_in(harness: &Harness<'static, InboxApp>, rect: egui::Rect, name: &str) -> Vec<egui::Rect> {
    harness.query_all_by_role_and_label(Role::Button, name).map(|node| node.rect()).filter(|at| rect.contains(at.center())).collect()
}

#[test]
fn the_keyed_row_keeps_its_time_and_its_actions_stand_left_of_it() {
    let mut harness = harness(true);
    harness.hover_at(egui::pos2(1.0, 1.0));
    harness.key_press(Key::ArrowDown);
    harness.run();
    let selected = harness.state().mailbox().selected().unwrap();
    let row = harness.state().mailbox().messages().iter().find(|row| row.id == selected).unwrap().clone();
    let time = harness.state().mailbox().row_date(&row);
    let rect = harness.get_by_label(&harness.state().mailbox().spoken(&row)).rect();
    let shown: Vec<_> = texts_in(&harness, rect).into_iter().filter(|(_, words)| *words == time).collect();
    assert_eq!(shown.len(), 1, "the keyed row shows its time {time:?}, as the mail row's focus state");
    // Done gives way last: whatever else fits left of the time, Done shows.
    let done = buttons_in(&harness, rect, "Done");
    assert_eq!(done.len(), 1, "the keyed row shows its actions");
    for name in ["Done", "Mark as read", "Mark as unread", "Delete", "Snooze"] {
        for button in buttons_in(&harness, rect, name) {
            assert!(button.right() < shown[0].0.left(), "{name} stands left of the time: {button:?} {shown:?}");
        }
    }
}

/// The icon glyphs `id` the last frame drew in `rect`.
fn marks_in(harness: &Harness<'static, InboxApp>, rect: egui::Rect, id: crate::iconset::IconId) -> Vec<egui::Rect> {
    let glyph = crate::iconset::glyph(id).to_string();
    texts_in(harness, rect).into_iter().filter(|(_, words)| *words == glyph).map(|(at, _)| at).collect()
}

#[test]
fn a_hovered_rows_flag_and_pin_stay_left_of_its_actions() {
    use crate::iconset::IconId;
    // Beside the reader at 130 % the list is narrow and the words large: a marked row's sender reaches its marks.
    let mut harness = beside_the_reader();
    harness.state_mut().set_text_scale(crate::theme::TextScale::ALL[4]);
    harness.run();
    let marked: Vec<_> = harness.state().mailbox().messages().iter().filter(|row| row.flagged || row.pinned).cloned().collect();
    assert!(!marked.is_empty(), "the sample has a flagged or pinned message");
    let mut checked = 0;
    for row in marked {
        let name = harness.state().mailbox().spoken(&row);
        let Some(node) = harness.query_by_label(&name) else { continue };
        let rect = node.rect();
        harness.hover_at(egui::pos2(rect.left() + 40.0, rect.center().y));
        harness.run();
        let done = buttons_in(&harness, rect, "Done");
        assert_eq!(done.len(), 1, "{name}: hovered, it shows its actions");
        let mut leftmost = f32::INFINITY;
        for (shown, id) in [(row.flagged, IconId::Flag), (row.pinned, IconId::Pin)] {
            let marks = marks_in(&harness, rect, id);
            if shown {
                assert_eq!(marks.len(), 1, "{name}: its {id:?} stays while the actions show");
                assert!(marks[0].right() <= done[0].left() - 8.0 + 0.5, "{name}: {id:?} {marks:?} left of the group {done:?}");
                leftmost = leftmost.min(marks[0].left());
            }
        }
        // The sender is cut short of the marks: the sheet's spacer and a gap.
        let sender = texts_in(&harness, rect)
            .into_iter()
            .find(|(_, words)| words.len() > 1 && row.sender.starts_with(words.trim_end_matches('…')));
        let sender = sender.unwrap_or_else(|| panic!("{name}: the sender is drawn"));
        assert!(sender.0.right() <= leftmost - 11.0 + 0.5, "{name}: the sender {:?} ends 11 short of the marks at {leftmost}", sender.0);
        // A sender cut short of them ends with one ellipsis after its last letter kept.
        assert!(sender.1 == row.sender || (sender.1.ends_with('…') && sender.1.matches('…').count() == 1), "{name}: {:?}", sender.1);
        let time = harness.state().mailbox().row_date(&row);
        assert!(texts_in(&harness, rect).iter().all(|(_, words)| *words != time), "{name}: the hovered row gives its time way");
        checked += 1;
    }
    assert!(checked >= 1, "a marked row in sight was hovered");
}

#[test]
fn two_touching_rows_never_both_take_the_pointer_on_their_shared_edge() {
    let mut harness = harness(true);
    let upper = harness.get_by_label(ODILE).rect();
    let rows: Vec<egui::Rect> = harness
        .state()
        .mailbox()
        .messages()
        .iter()
        .filter_map(|row| harness.query_by_label(&harness.state().mailbox().spoken(row)).map(|node| node.rect()))
        .collect();
    let lower = rows.into_iter().find(|rect| (rect.top() - upper.bottom()).abs() < 0.01 && (rect.left() - upper.left()).abs() < 0.01);
    let lower = lower.expect("a row right under Odile's");
    harness.hover_at(egui::pos2(upper.left() + 40.0, upper.bottom()));
    harness.run();
    let groups = [upper, lower].iter().filter(|rect| !buttons_in(&harness, **rect, "Done").is_empty()).count();
    assert_eq!(groups, 1, "one row owns the shared edge");
    // The one that owns it is the lower: a row's top edge is its own, its bottom edge the next row's.
    assert_eq!(buttons_in(&harness, lower, "Done").len(), 1);
}

#[test]
fn a_frame_of_a_thousand_rows_none_hovered_or_keyed_lays_out_no_hover_action() {
    let clock = FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap());
    let store = sample::grown(Timestamp::from_unix(1_793_869_200, 0).unwrap(), 1_200).unwrap();
    let session = sample::session_over(store, Box::new(clock.clone())).unwrap();
    let mut mailbox = Mailbox::new(session, Box::new(clock));
    mailbox.set_inbox_view(InboxView::List);
    let mut harness = harness_of(InboxApp::new(mailbox));
    harness.hover_at(egui::pos2(1.0, 1.0));
    harness.run();
    crate::component::hover::tests::reset_laid();
    ACTIONS_BUILT.with(|built| built.set(0));
    harness.step();
    // Of the thousand rows listed, the frame builds those in sight (and two above and below).
    let built = crate::rows_built(&harness.ctx).unwrap_or(0);
    assert!((1..=MOST_BUILT).contains(&built), "{built} rows laid out");
    assert_eq!(harness.state().mailbox().messages().len(), 1_000, "the engine lists a thousand");
    assert_eq!(crate::component::hover::tests::laid(), 0, "no row's actions were laid out");
    assert_eq!(ACTIONS_BUILT.with(std::cell::Cell::get), 0, "no row's list of actions was built");
}

thread_local! {
    /// How many rows' lists of hover actions were built on this thread (`list::actions`).
    pub(super) static ACTIONS_BUILT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The sample mailbox over a list of 10 000 messages in four groups (People, Updates, Newsletters, Invitations, in
/// turn), longer than the engine lists, in `view`.
fn ten_thousand_mailbox(view: InboxView) -> Mailbox {
    use crate::mailbox::GroupKind;
    let mut mailbox = Mailbox::new(sample::session(clock()).unwrap(), clock());
    mailbox.set_inbox_view(view);
    let model = mailbox.messages().first().unwrap().clone();
    let rows = (0..10_000_u64)
        .map(|n| crate::mailbox::MessageRow {
            id: 1_000_000 + n,
            subject: format!("Message {n}"),
            group: GroupKind::ALL.get(usize::try_from(n % 4).unwrap()).copied(),
            ..model.clone()
        })
        .collect();
    mailbox.set_rows(rows);
    mailbox
}

/// The sample window over [`ten_thousand_mailbox`], reduced motion on.
fn ten_thousand(view: InboxView) -> Harness<'static, InboxApp> {
    let mut app = InboxApp::new(ten_thousand_mailbox(view));
    app.set_reduce_motion(true);
    let mut harness = Harness::builder()
        .with_size(vec2(1300.0, 900.0))
        .with_context_setup(install_fonts)
        .with_max_steps(240)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

/// The list alone, in a pane 440 × 828 (the brief's), over [`ten_thousand_mailbox`].
fn pane(view: InboxView) -> Harness<'static, Mailbox> {
    use crate::theme::{Mode, TextScale, Theme};
    let theme = Theme::new(Mode::Light, TextScale::default());
    let keymap = crate::keymap::Keymap::default();
    let mut harness = Harness::builder().with_size(vec2(440.0, 828.0)).with_context_setup(install_fonts).with_max_steps(240).build_ui_state(
        move |ui, mailbox: &mut Mailbox| {
            crate::motion::set_reduced(ui.ctx(), true);
            ui.set_style(theme.style());
            let _ = super::list::rows(ui, &theme, &keymap, mailbox);
        },
        ten_thousand_mailbox(view),
    );
    harness.run();
    harness
}

/// The rectangle of message `id`'s row in the pane's AccessKit tree, when it was built.
fn pane_rect(harness: &Harness<'static, Mailbox>, id: u64) -> Option<egui::Rect> {
    let mailbox = harness.state();
    let row = mailbox.messages().iter().find(|row| row.id == id)?;
    harness.query_by_label(&mailbox.spoken(row)).map(|node| node.rect())
}

/// Turns the wheel over the pane's list by `points` (down when positive) and lets the scrolling settle.
fn pane_wheel(harness: &mut Harness<'static, Mailbox>, points: f32) {
    harness.hover_at(egui::pos2(220.0, 400.0));
    harness.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: vec2(0.0, -points),
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Unknown,
        modifiers: Modifiers::NONE,
    });
    harness.run();
}

/// Turns the wheel over the list by `points` (down when positive) and lets the scrolling settle.
fn wheel(harness: &mut Harness<'static, InboxApp>, points: f32) {
    harness.hover_at(egui::pos2(400.0, 500.0));
    harness.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: vec2(0.0, -points),
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Unknown,
        modifiers: Modifiers::NONE,
    });
    harness.run();
}

/// The most lines a pane 828 high can build, the brief's bound: those of 32 (the least line) that meet it, and two
/// above and below: `ceil(828 / 32) + 4`.
const MOST_BUILT: usize = 828_usize.div_ceil(32) + 4;

/// The most rows of 56 a pane 828 high builds: those that meet it, and two above and below.
const MOST_ROWS: usize = 828_usize.div_ceil(56) + 1 + 4;

#[test]
fn ten_thousand_messages_build_only_the_rows_in_sight() {
    let mut harness = pane(InboxView::List);
    let view = super::list::view_rect(&harness.ctx).unwrap();
    assert!(view.height() > 780.0 && view.height() <= 828.0, "the list's viewport fills the pane: {view:?}");
    let built = super::list::built(&harness.ctx).unwrap();
    assert!(built.count <= MOST_BUILT && built.count >= 8, "{built:?} at the top");
    assert_eq!(built.first, 0, "the top of the list");
    pane_wheel(&mut harness, 280_000.0);
    let built = super::list::built(&harness.ctx).unwrap();
    assert!(built.count <= MOST_BUILT && built.count >= 8, "{built:?} in the middle");
    assert!(built.first > 4_000, "{built:?}: the middle of the list");
    // The first message in sight is built and in AccessKit's tree; the list's first is not.
    let first = super::list::first_in_sight(&harness.ctx).unwrap();
    assert!(pane_rect(&harness, first).is_some(), "the first in sight is built");
    assert!(pane_rect(&harness, 1_000_000).is_none(), "the first of the list is not");
    // The simple list, rows only, builds no more than the rows of 56 that meet the pane and four.
    let mut harness = pane(InboxView::Simple);
    pane_wheel(&mut harness, 280_000.0);
    let built = super::list::built(&harness.ctx).unwrap();
    assert!(built.count <= MOST_ROWS && built.first > 4_000, "{built:?} in the simple list");
}

#[test]
fn a_group_opening_or_folding_above_the_viewport_keeps_the_first_message_in_sight() {
    let mut harness = pane(InboxView::Cards);
    // The last group opens; the list scrolls into its middle.
    harness.ctx.data_mut(|data| data.insert_temp(super::list::group_open_id("Invitations"), true));
    harness.run();
    pane_wheel(&mut harness, 70_000.0);
    let first = super::list::first_in_sight(&harness.ctx).unwrap();
    let place = pane_rect(&harness, first).unwrap();
    let built = super::list::built(&harness.ctx).unwrap();
    assert!(built.count <= MOST_BUILT, "{built:?}");
    for open in [true, false] {
        // The first group, far above, opens (2 500 rows more above the viewport), then folds again.
        harness.ctx.data_mut(|data| data.insert_temp(super::list::group_open_id("People"), open));
        harness.run();
        assert_eq!(super::list::first_in_sight(&harness.ctx), Some(first), "the first message in sight stays, People open: {open}");
        assert_eq!(pane_rect(&harness, first), Some(place), "on the same place on the screen, People open: {open}");
        let built = super::list::built(&harness.ctx).unwrap();
        assert!(built.count <= MOST_BUILT, "{built:?}");
    }
}

#[test]
fn each_row_tells_a_screen_reader_its_place_in_the_whole_list_and_the_list_its_size() {
    let mut harness = pane(InboxView::Simple);
    pane_wheel(&mut harness, 280_000.0);
    let list = harness.get_by_role_and_label(Role::ListBox, "Messages");
    assert_eq!(list.accesskit_node().size_of_set(), Some(10_000), "the list's size, built or not");
    let first = super::list::first_in_sight(&harness.ctx).unwrap();
    let index = usize::try_from(first - 1_000_000).unwrap();
    let mailbox = harness.state();
    let row = mailbox.messages().iter().find(|row| row.id == first).unwrap();
    let spoken = mailbox.spoken(row);
    let node = harness.get_by_label(&spoken);
    assert_eq!(node.accesskit_node().position_in_set(), Some(index), "its place, from 0");
    assert_eq!(node.accesskit_node().size_of_set(), Some(10_000));
    let rows = harness.query_all_by_role(Role::ListBoxOption).count();
    assert!(rows <= MOST_ROWS, "only the rows built are items: {rows}");
}

/// The rectangle of message `id`'s row in AccessKit's tree, when it was built.
fn row_rect(harness: &Harness<'static, InboxApp>, id: u64) -> Option<egui::Rect> {
    let mailbox = harness.state().mailbox();
    let row = mailbox.messages().iter().find(|row| row.id == id)?;
    harness.query_by_label(&mailbox.spoken(row)).map(|node| node.rect())
}

/// Whether the selected row is built and wholly inside the window's list.
fn selected_in_sight(harness: &Harness<'static, InboxApp>) -> bool {
    let Some(id) = harness.state().mailbox().selected() else { return false };
    in_sight(harness, id)
}

/// Whether message `id`'s row is built and wholly inside the list's viewport.
fn in_sight(harness: &Harness<'static, InboxApp>, id: u64) -> bool {
    let Some(view) = super::list::view_rect(&harness.ctx) else { return false };
    row_rect(harness, id).is_some_and(|rect| view.expand(0.01).contains_rect(rect))
}

/// The place of the selected row in the list.
fn selected_at(harness: &Harness<'static, InboxApp>) -> Option<usize> {
    let mailbox = harness.state().mailbox();
    mailbox.messages().iter().position(|row| Some(row.id) == mailbox.selected())
}

#[test]
fn the_keyed_row_is_scrolled_into_sight_though_it_was_not_built() {
    for view in [InboxView::Simple, InboxView::List, InboxView::Cards] {
        let mut harness = ten_thousand(view);
        // A row far down, chosen without the keyboard: nothing scrolls to it.
        harness.state_mut().mailbox_mut().select(Some(1_005_000));
        harness.run();
        assert!(row_rect(&harness, 1_005_000).is_none(), "{view:?}: not built");
        if view == InboxView::Cards {
            // On cards a group shows its first rows: the next row's group (5 001 is in Updates) shows all of them.
            harness.ctx.data_mut(|data| data.insert_temp(super::list::group_open_id("Updates"), true));
            harness.run();
        }
        harness.key_press(Key::ArrowDown);
        harness.run();
        assert_eq!(selected_at(&harness), Some(5_001), "{view:?}");
        assert!(selected_in_sight(&harness), "{view:?}: the keyed row is brought into sight");
        let built = super::list::built(&harness.ctx).unwrap();
        assert!(built.count <= MOST_BUILT, "{built:?}");
    }
}

#[test]
fn a_row_the_pointer_hovers_far_down_the_list_is_built_with_its_actions() {
    let mut harness = ten_thousand(InboxView::Simple);
    wheel(&mut harness, 300_000.0);
    let first = super::list::first_in_sight(&harness.ctx).unwrap();
    let rect = row_rect(&harness, first + 3).unwrap();
    harness.hover_at(rect.center());
    harness.run();
    assert_eq!(buttons_in(&harness, rect, "Done").len(), 1, "the hovered row shows its actions");
}

#[test]
fn a_message_arriving_above_keeps_the_rows_in_sight_in_place_unless_the_list_is_at_its_top() {
    let mut harness = ten_thousand(InboxView::Simple);
    let arrive = |harness: &mut Harness<'static, InboxApp>, id: u64| {
        let mut rows = harness.state().mailbox().messages().to_vec();
        let mut new = rows[0].clone();
        new.id = id;
        new.subject = format!("Arrived {id}");
        rows.insert(0, new);
        harness.state_mut().mailbox_mut().set_rows(rows);
        harness.run();
    };
    // At the top, the new message shows at the top.
    arrive(&mut harness, 1);
    assert_eq!(super::list::first_in_sight(&harness.ctx), Some(1));
    wheel(&mut harness, 100_000.0);
    let first = super::list::first_in_sight(&harness.ctx).unwrap();
    let before = row_rect(&harness, first).unwrap();
    arrive(&mut harness, 2);
    assert_eq!(super::list::first_in_sight(&harness.ctx), Some(first), "the same row first in sight");
    assert_eq!(row_rect(&harness, first), Some(before), "in the same place");
}

#[test]
fn done_on_the_keyed_row_keeps_the_next_keyed_and_in_sight() {
    let store = sample::grown(Timestamp::from_unix(1_793_869_200, 0).unwrap(), 1_200).unwrap();
    let clock = FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap());
    let session = sample::session_over(store, Box::new(clock.clone())).unwrap();
    let mut mailbox = Mailbox::new(session, Box::new(clock));
    mailbox.set_inbox_view(InboxView::Simple);
    let deep = mailbox.messages()[600].id;
    let mut app = InboxApp::new(mailbox);
    app.set_reduce_motion(true);
    let mut harness = harness_of(app);
    harness.state_mut().mailbox_mut().select(Some(deep));
    harness.run();
    harness.key_press(Key::ArrowDown);
    harness.run();
    assert!(selected_in_sight(&harness), "the keyed row in sight");
    let keyed = harness.state().mailbox().selected();
    harness.key_press(Key::D);
    harness.run();
    harness.run();
    let mailbox = harness.state().mailbox();
    assert!(mailbox.messages().iter().all(|row| Some(row.id) != keyed), "done, out of the list");
    assert!(mailbox.selected().is_some() && mailbox.keyed(), "the next row keyed");
    assert!(selected_in_sight(&harness), "and in sight");
}

#[test]
fn an_empty_list_a_list_of_one_and_a_list_shorter_than_its_pane_draw() {
    for view in [InboxView::Simple, InboxView::List, InboxView::Cards] {
        let mut harness = ten_thousand(view);
        harness.state_mut().mailbox_mut().set_rows(Vec::new());
        harness.run();
        harness.key_press(Key::End);
        harness.run();
        harness.get_by_label("Nothing here");
    }
    for count in [1_u64, 3] {
        for view in [InboxView::Simple, InboxView::List, InboxView::Cards] {
            let mut harness = ten_thousand(view);
            let rows: Vec<_> = harness.state().mailbox().messages().iter().take(usize::try_from(count).unwrap()).cloned().collect();
            harness.state_mut().mailbox_mut().set_rows(rows.clone());
            harness.run();
            harness.key_press(Key::End);
            harness.run();
            harness.key_press(Key::PageDown);
            harness.run();
            harness.key_press(Key::Home);
            harness.run();
            harness.key_press(Key::PageUp);
            harness.run();
            let built = super::list::built(&harness.ctx).unwrap();
            assert_eq!(built.first, 0, "{view:?}, {count}");
            for row in &rows {
                assert!(row_rect(&harness, row.id).is_some(), "{view:?}: every row of {count} is built");
            }
        }
    }
}

#[test]
fn the_lines_of_ten_thousand_messages_add_up_to_whole_pixels_at_every_text_scale() {
    use crate::mailbox::Group;
    use crate::theme::{Mode, TextScale, Theme};
    let harness = ten_thousand(InboxView::List);
    let mailbox = harness.state().mailbox();
    let other = Group { icon: crate::iconset::IconId::Inbox, name: "Other".to_owned(), messages: Vec::new() };
    for percent in [100, 130] {
        let theme = Theme::new(Mode::Light, TextScale::from_percent(percent).unwrap());
        for view in [InboxView::Simple, InboxView::List, InboxView::Cards] {
            let grouped = mailbox.groups();
            assert_eq!(grouped.len(), 4);
            let listing = super::list::lines(&theme, mailbox, view, &grouped, &other, true, &|_| true);
            let rows = listing.lines.iter().filter(|line| matches!(line, super::list::Line::Mail(_))).count();
            assert_eq!(rows, 10_000, "{percent} {view:?}: every message a line");
            let spans: Vec<(f32, f32)> = (0..listing.lines.len()).map(|index| listing.span(index).unwrap()).collect();
            for (index, pair) in spans.windows(2).enumerate() {
                let ((_, bottom), (top, _)) = (pair[0], pair[1]);
                assert!(top >= bottom, "{percent} {view:?}: line {index} overlaps the next");
                let both_rows =
                    matches!(listing.lines[index], super::list::Line::Mail(_)) && matches!(listing.lines[index + 1], super::list::Line::Mail(_));
                if both_rows {
                    assert_eq!(top, bottom, "{percent} {view:?}: rows {index} and {} touch, no gap, no doubled edge", index + 1);
                }
            }
            for ((top, bottom), line) in spans.iter().zip(&listing.lines) {
                if let super::list::Line::Mail(_) = line {
                    assert_eq!(bottom - top, theme.size(56.0), "{percent} {view:?}: the mail row's fixed height");
                }
                for ppp in [1.0_f32, 1.5] {
                    for at in [top, bottom] {
                        assert_eq!((at * ppp).fract(), 0.0, "{percent} {view:?} at {ppp}: {at} on a whole pixel");
                    }
                }
            }
            let end = listing.sheets.last().map_or(spans.last().unwrap().1, |sheet| sheet.1);
            assert_eq!(end, listing.height, "{percent} {view:?}: the last line (or its card) ends the content exactly");
            if view == InboxView::Simple {
                assert_eq!(spans.last().unwrap().1, listing.height, "the last row's bottom is the content's height");
                assert_eq!(listing.height, theme.size(56.0) * 10_000.0, "{percent}: no point lost over 10 000 rows");
            }
        }
    }
}

/// A row given by its message id is keyed: the selection moved there by the keyboard, from the row before it.
fn key(harness: &mut Harness<'static, InboxApp>, id: u64) {
    harness.state_mut().mailbox_mut().select(Some(id - 1));
    harness.run();
    harness.key_press(Key::ArrowDown);
    harness.run();
    assert_eq!(harness.state().mailbox().selected(), Some(id));
    assert!(selected_in_sight(harness), "keyed {id}, in sight");
}

/// Puts message `n` of [`ten_thousand`]'s list, in `group`, at the top of the list.
fn arrive(harness: &mut Harness<'static, InboxApp>, id: u64, group: crate::mailbox::GroupKind) {
    let mut rows = harness.state().mailbox().messages().to_vec();
    let mut new = rows[0].clone();
    new.id = id;
    new.subject = format!("Arrived {id}");
    new.group = Some(group);
    rows.insert(0, new);
    harness.state_mut().mailbox_mut().set_rows(rows);
    harness.run();
}

/// The first message in sight and its row's rectangle.
fn first_place(harness: &Harness<'static, InboxApp>) -> (u64, egui::Rect) {
    let first = super::list::first_in_sight(&harness.ctx).unwrap();
    (first, row_rect(harness, first).unwrap())
}

#[test]
fn a_keyed_row_scrolled_away_from_stays_away_when_mail_arrives_above() {
    use crate::mailbox::GroupKind;
    let mut harness = ten_thousand(InboxView::List);
    key(&mut harness, 1_005_001);
    wheel(&mut harness, 20_000.0);
    let before = first_place(&harness);
    assert!(!in_sight(&harness, 1_005_001), "wheeled away from the keyed row");
    arrive(&mut harness, 7, GroupKind::Updates);
    assert_eq!(first_place(&harness), before, "the first message in sight stays where it was");
    harness.key_press(Key::ArrowDown);
    harness.run();
    assert_eq!(harness.state().mailbox().selected(), Some(1_005_002));
    assert!(selected_in_sight(&harness), "the newly keyed row comes into sight");
}

#[test]
fn a_keyed_row_scrolled_away_from_stays_away_when_a_group_opens_or_folds_above() {
    let mut harness = ten_thousand(InboxView::Cards);
    harness.ctx.data_mut(|data| data.insert_temp(super::list::group_open_id("Invitations"), true));
    harness.run();
    // 5 003 is in Invitations, the last group, open.
    key(&mut harness, 1_005_003);
    wheel(&mut harness, 20_000.0);
    let before = first_place(&harness);
    assert!(!in_sight(&harness, 1_005_003), "wheeled away from the keyed row");
    for open in [true, false] {
        harness.ctx.data_mut(|data| data.insert_temp(super::list::group_open_id("People"), open));
        harness.run();
        assert_eq!(first_place(&harness), before, "the first message in sight stays where it was, People open: {open}");
    }
    // Down four times: 5 004 to 5 006 are in folded groups, 5 007 in Invitations.
    for _ in 0..4 {
        harness.key_press(Key::ArrowDown);
        harness.run();
    }
    assert_eq!(harness.state().mailbox().selected(), Some(1_005_007));
    assert!(selected_in_sight(&harness), "the newly keyed row comes into sight");
}

#[test]
fn tab_walks_down_the_whole_list_building_rows_as_it_goes() {
    let mut harness = ten_thousand(InboxView::Simple);
    harness.ctx.memory_mut(|memory| memory.request_focus(super::list::row_id(1_000_000)));
    harness.run();
    for _ in 0..40 {
        harness.key_press(Key::Tab);
        harness.run();
    }
    let focused = harness.ctx.memory(egui::Memory::focused);
    assert_eq!(focused, Some(super::list::row_id(1_000_040)), "the 41st row has the focus");
    assert!(in_sight(&harness, 1_000_040), "and is wholly in sight");
    let built = super::list::built(&harness.ctx).unwrap();
    assert!(built.count <= MOST_ROWS, "{built:?}");
    for _ in 0..40 {
        harness.key_press_modifiers(Modifiers::SHIFT, Key::Tab);
        harness.run();
    }
    assert_eq!(harness.ctx.memory(egui::Memory::focused), Some(super::list::row_id(1_000_000)), "Shift+Tab walks back up");
    assert!(in_sight(&harness, 1_000_000));
}

/// The row of the selected message has the keyboard's focus.
fn focus_selected(harness: &mut Harness<'static, InboxApp>) {
    let id = harness.state().mailbox().selected().unwrap();
    harness.ctx.memory_mut(|memory| memory.request_focus(super::list::row_id(id)));
    harness.run();
}

/// Whether the selected message's row has the keyboard's focus.
fn selected_focused(harness: &Harness<'static, InboxApp>) -> bool {
    let id = harness.state().mailbox().selected();
    id.is_some_and(|id| harness.ctx.memory(|memory| memory.has_focus(super::list::row_id(id))))
}

#[test]
fn page_down_page_up_end_and_home_move_the_focused_row_as_keymap_actions() {
    use crate::keymap::{self, Keymap};
    let defaults = Keymap::default();
    for (action, key) in [(keymap::PAGE_DOWN, Key::PageDown), (keymap::PAGE_UP, Key::PageUp), (keymap::FIRST, Key::Home), (keymap::LAST, Key::End)] {
        assert_eq!(defaults.shortcut(action), Some(egui::KeyboardShortcut::new(Modifiers::NONE, key)), "{action}");
        assert!(keymap::action(action).is_some(), "{action} is an action, on the Shortcuts page");
    }
    let mut harness = ten_thousand(InboxView::Simple);
    key(&mut harness, 1_005_001);
    focus_selected(&mut harness);
    harness.key_press(Key::PageDown);
    harness.run();
    let paged = selected_at(&harness).unwrap();
    assert!(paged > 5_005 && paged < 5_030, "a page down: {paged}");
    assert!(selected_in_sight(&harness) && selected_focused(&harness), "after Page Down the row is in sight and focused");
    harness.key_press(Key::PageUp);
    harness.run();
    assert_eq!(selected_at(&harness), Some(5_001), "a page up again");
    assert!(selected_in_sight(&harness) && selected_focused(&harness), "after Page Up");
    harness.key_press(Key::End);
    harness.run();
    assert_eq!(selected_at(&harness), Some(9_999), "End");
    assert!(selected_in_sight(&harness) && selected_focused(&harness), "after End");
    harness.key_press(Key::Home);
    harness.run();
    assert_eq!(selected_at(&harness), Some(0), "Home");
    assert!(selected_in_sight(&harness) && selected_focused(&harness), "after Home");
    // Rebound: N goes to the last row, End no more.
    harness.state_mut().keymap_mut().bind(keymap::LAST, egui::KeyboardShortcut::new(Modifiers::NONE, Key::N));
    harness.key_press(Key::End);
    harness.run();
    assert_eq!(selected_at(&harness), Some(0), "End has no action now");
    harness.key_press(Key::N);
    harness.run();
    assert_eq!(selected_at(&harness), Some(9_999), "its new key");
}

#[test]
fn the_list_keys_rest_while_the_list_does_not_have_the_keyboard() {
    let mut harness = ten_thousand(InboxView::Simple);
    // A message open, nothing focused: reading, End and Page Down leave the open message alone.
    harness.state_mut().mailbox_mut().select(Some(1_000_003));
    harness.run();
    assert_eq!(harness.ctx.memory(egui::Memory::focused), None);
    for pressed in [Key::End, Key::PageDown, Key::Home, Key::PageUp] {
        harness.key_press(pressed);
        harness.run();
        assert_eq!(harness.state().mailbox().selected(), Some(1_000_003), "{pressed:?} with nothing focused");
    }
    // The search field keeps its own Home and End.
    harness.ctx.memory_mut(|memory| memory.request_focus(super::list::search_id()));
    harness.run();
    harness.event(egui::Event::Text("odile".to_owned()));
    harness.run();
    harness.key_press(Key::Home);
    harness.run();
    harness.event(egui::Event::Text("x".to_owned()));
    harness.run();
    harness.key_press(Key::End);
    harness.run();
    harness.event(egui::Event::Text("y".to_owned()));
    harness.run();
    assert_eq!(harness.state().search_text(), "xodiley", "Home and End move the field's caret");
}
