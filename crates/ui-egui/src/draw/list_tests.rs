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
    harness.get_by_label("Ctrl+Z");
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
    assert!(harness.query_by_label("Ctrl+Z").is_none(), "Undo's key belongs to the field while it has the focus");
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
fn the_hover_actions_cover_the_row_text_on_a_solid_float() {
    /// Whether `shape` holds an opaque rectangle under `point`.
    fn solid_under(shape: &egui::Shape, point: egui::Pos2) -> bool {
        match shape {
            egui::Shape::Rect(rect) => rect.fill.a() == 255 && rect.rect.contains(point),
            egui::Shape::Vec(shapes) => shapes.iter().any(|shape| solid_under(shape, point)),
            _ => false,
        }
    }
    let mut harness = harness(true);
    harness.get_by_label(ODILE).hover();
    harness.run();
    let done = harness.get_by_role_and_label(Role::Button, "Done").rect();
    let row = harness.get_by_label(ODILE).rect();
    let shapes = &harness.output().shapes;
    let float = shapes.iter().rposition(|clipped| solid_under(&clipped.shape, done.center()));
    let float = float.expect("the float under the Done button");
    let mut seen = 0;
    for (index, clipped) in shapes.iter().enumerate() {
        let egui::Shape::Text(text) = &clipped.shape else { continue };
        let rect = text.visual_bounding_rect();
        if row.contains(rect.left_center()) && (text.galley.job.text.starts_with("Odile") || text.galley.job.text.starts_with("Allotment")) {
            seen += 1;
            assert!(index < float, "{:?} is drawn over the float", text.galley.job.text);
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
    for (name, chip) in [("Done", "D"), ("Mark as read", "R"), ("Mail", "Ctrl+1"), ("Appearance settings", "Ctrl+Comma")] {
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
    assert_eq!(colour, theme.palette.accent, "Show all is in the accent colour");
    let row = harness.get_by_label(ODILE).rect();
    let sender = painted(&harness, "Odile Brannock").expect("the sender is drawn").0;
    assert!(sender.left() > row.left() + 20.0, "the sender stands right of the avatar");
    let wanted = sender.left() - theme.spacing.l;
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
    assert_eq!(super::list::after_subject("Hi all"), "  · Hi all");
}
