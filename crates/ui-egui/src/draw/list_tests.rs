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

/// The parts of the groups' heads, which the List view leaves out.
const CARDS_ONLY: [&str; 4] = ["done all", "show all", "People, 2 unread", "Notifications, 1 unread"];

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
        mailbox.set_inbox_view(InboxView::List);
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
fn the_list_view_draws_no_group_head_but_the_screener_card() {
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
    app.mailbox_mut().set_inbox_view(InboxView::List);
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

/// The spoken name of the row after Odile's in the simple list.
fn after_odile(harness: &Harness<'static, InboxApp>) -> String {
    let mailbox = harness.state().mailbox();
    let at = mailbox.messages().iter().position(|row| row.subject == "Allotment rota for November").unwrap();
    mailbox.spoken(&mailbox.messages()[at + 1])
}

#[test]
fn a_row_that_leaves_acts_at_once_and_its_gap_closes_as_the_motion_level_says() {
    // Motion off: the act and the list without the row in the frame after the press, the toast at its place at once.
    let mut harness = still();
    let next = after_odile(&harness);
    let below = harness.get_by_label(&next).rect();
    harness.get_by_label(ODILE).hover();
    harness.run();
    let count = harness.state().mailbox().list_count();
    harness.get_by_role_and_label(Role::Button, "Done").click();
    harness.step();
    harness.step();
    assert!(harness.state().mailbox().messages().iter().all(|row| row.subject != "Allotment rota for November"), "done at once");
    assert_ne!(harness.state().mailbox().list_count(), count, "the count changes with the act");
    assert!(harness.query_by_label(ODILE).is_none());
    harness.run_steps(3);
    assert!(harness.state().mailbox().left_row().is_none(), "no gap to close");
    assert_eq!(harness.get_by_label(&next).rect().top(), below.top() - 56.0, "the row under it moved up at once");
    let at_once = harness.get_by_role_and_label(Role::Button, "Undo").rect();
    harness.run();
    assert_eq!(harness.get_by_role_and_label(Role::Button, "Undo").rect(), at_once);
    // Motion on: the act at once too; the gap closes over `row_collapse` (180 ms), the rows under it rising.
    let mut harness = moving();
    let next = after_odile(&harness);
    let below = harness.get_by_label(&next).rect().top();
    harness.get_by_label(ODILE).hover();
    harness.run_steps(2);
    harness.get_by_role_and_label(Role::Button, "Done").click();
    harness.step();
    harness.step();
    assert!(harness.state().mailbox().messages().iter().all(|row| row.subject != "Allotment rota for November"), "done at once");
    assert!(harness.state().mailbox().left_row().is_some(), "its gap still closing");
    let ghost = harness.query_by_label(ODILE).map(|node| node.accesskit_node().is_disabled());
    assert_eq!(ghost, Some(true), "the leaving row is named but takes no pointer or focus");
    let mut tops = vec![harness.get_by_label(&next).rect().top()];
    // The toast where it first shows: it rises from there.
    let mut rising = harness.query_by_role_and_label(Role::Button, "Undo").map(|node| node.rect());
    for _ in 0..14 {
        harness.step();
        tops.push(harness.get_by_label(&next).rect().top());
        rising = rising.or_else(|| harness.query_by_role_and_label(Role::Button, "Undo").map(|node| node.rect()));
    }
    assert!(tops.windows(2).all(|pair| pair[1] <= pair[0]), "the row under it only rises: {tops:?}");
    assert!(tops.first().is_some_and(|first| *first > below - 56.0), "it rises over frames: {tops:?}");
    assert_eq!(tops.last().copied(), Some(below - 56.0), "the gap closed: {tops:?}");
    harness.run_steps(2);
    assert!(harness.state().mailbox().left_row().is_none(), "let go once closed");
    let rising = rising.expect("the toast shows while the row closes");
    harness.run_steps(20);
    let risen = harness.get_by_role_and_label(Role::Button, "Undo").rect();
    assert!(rising.top() > risen.top(), "the toast rose from {rising:?} to {risen:?}");
    assert_eq!(risen.top(), at_once.top(), "to where it stands at once with motion off");
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
fn smart_shows_three_rows_and_show_all() {
    let mut harness = harness(true);
    assert_eq!(harness.state().mailbox().inbox_view(), InboxView::Smart, "Smart is the view to begin with");
    harness.get_by_label("2 new senders");
    harness.get_by_role_and_label(Role::Button, "Review");
    harness.get_by_label("People, 2 unread");
    harness.get_by_label("Notifications, 1 unread");
    // The rows no group holds stand under a head of their own, last.
    harness.get_by_label("Other, 1 unread");
    assert_eq!(harness.query_all_by_role_and_label(Role::Button, "done all").count(), 0, "only on a hovered or keyed head");
    harness.get_by_label("People, 2 unread").hover();
    harness.run();
    assert_eq!(harness.query_all_by_role_and_label(Role::Button, "done all").count(), 1);
    // People holds seven rows and shows three of them, its head says so and offers "show all"; Notifications shows its
    // three and offers nothing; Kasimir's row stays out of the groups.
    assert!(painted(&harness, "3 of 7 ·").is_some(), "how many of the group's rows show");
    assert_eq!(harness.query_all_by_role_and_label(Role::Button, "show all").count(), 1, "one group has more than three rows");
    assert_eq!(harness.query_all_by_role(Role::ListBoxOption).count(), 3 + 3 + 1);
    harness.get_by_label("Kasimir Vell, Studio open day: volunteers?, Yesterday, unread");
    // "show all" lists every row of the group; "show fewer" folds it again.
    harness.get_by_role_and_label(Role::Button, "show all").click();
    harness.run();
    assert_eq!(harness.query_all_by_role(Role::ListBoxOption).count(), 7 + 3 + 1);
    assert!(painted(&harness, "7 of 7 ·").is_some());
    harness.get_by_role_and_label(Role::Button, "show fewer").click();
    harness.run();
    assert_eq!(harness.query_all_by_role(Role::ListBoxOption).count(), 3 + 3 + 1);
    // Grouped: every group open, no "show all".
    harness.state_mut().mailbox_mut().set_inbox_view(InboxView::Grouped);
    harness.run();
    assert_eq!(harness.query_all_by_role(Role::ListBoxOption).count(), 7 + 3 + 1);
    assert!(harness.query_by_role_and_label(Role::Button, "show all").is_none());
    assert!(painted(&harness, "3 of 7 ·").is_none());
    // The heads are the design's group heads, 32 high, across the list's rows.
    let head = harness.get_by_label("People, 2 unread").rect();
    let row = harness.get_by_label(ODILE).rect();
    assert_eq!((head.height(), head.left(), head.width()), (32.0, row.left(), row.width()));
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
fn the_hover_actions_stand_in_the_design_files_order_and_tab_walks_them_so() {
    // The board's group: Done, Snooze, Read, Delete, 43, 43, 43 and 56 wide at +0, +47, +94 and +141.
    let mut harness = still();
    let row = harness.get_by_label(ODILE).rect();
    harness.hover_at(row.center());
    harness.run();
    let boxes: Vec<_> = ["Done", "Snooze", "Mark as read", "Delete"].map(|name| on_row(&harness, row, name)).into();
    let first = boxes[0].left();
    let places: Vec<_> = boxes.iter().map(|rect| (rect.left() - first, rect.width())).collect();
    assert_eq!(places, [(0.0, 43.0), (47.0, 43.0), (94.0, 43.0), (141.0, 56.0)], "{boxes:?}");
    // The keys' walk follows what is drawn: from the row, Tab reaches them left to right.
    harness.ctx.memory_mut(|memory| memory.request_focus(super::list::row_id(id_with(&harness, "Allotment rota for November"))));
    harness.run();
    harness.run();
    let mut walked = Vec::new();
    for _ in 0..4 {
        harness.key_press(Key::Tab);
        harness.run();
        let focused = harness.ctx.memory(egui::Memory::focused).and_then(|id| harness.ctx.read_response(id)).map(|response| response.rect);
        walked.push(focused);
    }
    let row = harness.ctx.read_response(super::list::row_id(id_with(&harness, "Allotment rota for November"))).unwrap().rect;
    let read = if harness.query_by_label(ODILE).is_some() { "Mark as read" } else { "Mark as unread" };
    let now: Vec<_> = ["Done", "Snooze", read, "Delete"].map(|name| Some(on_row(&harness, row, name))).into();
    assert_eq!(walked, now, "Tab walks the actions as they are drawn");
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
    for name in ["Review", "show all"] {
        let before = harness.query_all_by_label(name).count();
        harness.query_all_by_role_and_label(Role::Button, name).next().unwrap().hover();
        harness.run();
        assert_eq!(harness.query_all_by_label(name).count(), before + 1, "{name} has a tooltip");
    }
    // A head's "done all" shows while the head is hovered; hovered itself, it has its tooltip too.
    harness.get_by_label("People, 2 unread").hover();
    harness.run();
    let before = harness.query_all_by_label("done all").count();
    harness.get_by_role_and_label(Role::Button, "done all").hover();
    harness.run();
    assert_eq!(harness.query_all_by_label("done all").count(), before + 1, "done all has a tooltip");
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
fn the_account_stripe_marks_the_rows_of_every_inbox_view() {
    // The design file's rows carry the bar 8 after the sender in Grouped and Smart as in List, two accounts shown.
    assert!(stripes(&harness(false)) > 0, "the simple list of two accounts shows whose each row is");
    for view in [InboxView::Grouped, InboxView::Smart, InboxView::List] {
        let mut harness = harness(true);
        harness.state_mut().mailbox_mut().set_inbox_view(view);
        harness.run();
        assert!(stripes(&harness) > 0, "{view:?} shows whose each row is");
    }
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
fn show_all_is_an_accent_link_in_its_groups_head_before_the_count() {
    let harness = harness(true);
    let (text, colour) = painted(&harness, "show all").expect("show all is drawn");
    let theme = crate::theme::Theme::current(&harness.ctx, &crate::theme::Look::of(harness.state().mailbox().appearance()));
    assert_eq!(colour, theme.palette.cyan_text, "show all is in the accent colour");
    let head = harness.get_by_label("People, 2 unread").rect();
    assert!(head.contains(text.center()), "in the head: {text:?} in {head:?}");
    let (shown, _) = painted(&harness, "3 of 7 ·").expect("how many show");
    // The count stands at the head's right, 16 in, after a gap of 8.
    assert!(shown.right() < text.left() && text.right() < head.right() - 16.0 - 8.0, "how many show, show all, the count: {shown:?} {text:?}");
}

#[test]
fn a_hovered_row_offers_done_snooze_read_and_delete_in_that_order() {
    let mut harness = harness(true);
    harness.get_by_label(ODILE).hover();
    harness.run();
    let row = harness.get_by_label(ODILE).rect();
    let mut lefts = Vec::new();
    for name in ["Done", "Snooze", "Mark as read", "Delete"] {
        let button = harness.query_all_by_role_and_label(Role::Button, name).find(|node| row.contains(node.rect().center())).expect(name);
        assert!(!button.accesskit_node().is_disabled(), "{name}");
        lefts.push(button.rect().left());
    }
    assert!(lefts.windows(2).all(|pair| pair[0] < pair[1]), "in the design's order: {lefts:?}");
}

#[test]
fn the_view_switch_changes_the_view() {
    let mut harness = harness(true);
    for (name, view) in [("List", InboxView::List), ("Grouped", InboxView::Grouped), ("Smart", InboxView::Smart)] {
        harness.get_by_role_and_label(Role::RadioButton, &format!("{name} for the inbox")).click();
        harness.run();
        assert_eq!(harness.state().mailbox().inbox_view(), view);
    }
}

#[test]
fn the_view_switch_reads_grouped_smart_list_as_the_design_file_orders_it() {
    let mut harness = harness(true);
    let left = |harness: &Harness<'static, InboxApp>, name: &str| {
        harness.get_by_role_and_label(Role::RadioButton, &format!("{name} for the inbox")).rect().left()
    };
    let lefts = ["Grouped", "Smart", "List"].map(|name| left(&harness, name));
    assert!(lefts[0] < lefts[1] && lefts[1] < lefts[2], "Grouped, Smart, List: {lefts:?}");
    // The arrows walk the views as they are drawn.
    harness.get_by_role_and_label(Role::RadioButton, "Grouped for the inbox").click();
    harness.run();
    harness.get_by_role_and_label(Role::RadioGroup, "the inbox").focus();
    harness.run();
    let mut walked = Vec::new();
    for _ in 0..2 {
        harness.key_press(Key::ArrowRight);
        harness.run();
        walked.push(harness.state().mailbox().inbox_view());
    }
    assert_eq!(walked, [InboxView::Smart, InboxView::List]);
}

#[test]
fn the_view_switch_stays_beside_an_open_message() {
    let mut harness = harness(true);
    harness.get_by_label(ODILE).click();
    harness.run();
    // The list keeps its 440 beside the reader, and the switch its place with "Show done".
    harness.get_by_role_and_label(Role::RadioButton, "Smart for the inbox");
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
    // The rows and the screener card over them.
    assert!(built > 0 && built <= all + 1, "{built} of {all}");
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
fn the_list_is_440_wide_beside_the_reader() {
    use crate::theme::TextScale;
    // Tall enough that every row fits, and short enough that the list scrolls: the bar lies over the rows, which keep
    // their 440 either way.
    for (percent, high) in [(100, 1400.0), (130, 1400.0), (100, 500.0), (130, 500.0)] {
        let percent_high = format!("{percent} % at {high}");
        let mut app = app(false);
        app.set_text_scale(TextScale::from_percent(percent).unwrap());
        let mut harness = Harness::builder()
            .with_size(vec2(1300.0, high))
            .with_context_setup(install_fonts)
            .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
        harness.run();
        let teodor = harness.state().mailbox().messages().iter().find(|row| row.subject.contains("coast walk")).unwrap().id;
        harness.state_mut().mailbox_mut().select(Some(teodor));
        harness.run();
        assert!(harness.state().reader().open_id().is_some(), "Teodor's message is open");
        let last = *super::list::order(&harness.ctx).last().unwrap();
        assert_eq!(in_sight(&harness, last), high > 1000.0, "{percent_high}: the list scrolls only in the short window");
        let row = harness.get_by_label(ODILE).rect();
        assert_eq!(row.width(), 440.0, "{percent_high}: the rows are the design's 440 at every text size");
        // A pointer press in its middle opens it and acts on nothing.
        press_odile_in_the_middle(&mut harness, &percent_high);
        // No splitter: dragging where the list's edge was leaves it 440.
        let y = row.center().y;
        for x in [0.0, 1.0, 2.0, 3.0] {
            let from = egui::pos2(row.left() + 440.0 + x, y);
            harness.hover_at(from);
            harness.drag_at(from);
            harness.run();
            let to = egui::pos2(row.left() + 200.0, y);
            harness.hover_at(to);
            harness.run();
            harness.drop_at(to);
            harness.run();
        }
        assert_eq!(harness.get_by_label(ODILE_READ).rect().width(), 440.0, "{percent_high}: the edge does not move");
    }
    // A width a person once dragged the list to, kept in egui's memory under the list panel's id, is not read.
    let mut harness = beside_the_reader();
    let id = harness.ctx.data(|data| data.get_temp::<egui::Id>(egui::Id::unique("list-panel-id"))).unwrap();
    let kept = harness.ctx.data_mut(|data| data.get_persisted::<egui::containers::panel::PanelState>(id));
    assert!(kept.is_some(), "the panel keeps its state under that id: the planted width is where a panel reads it");
    harness.ctx.data_mut(|data| {
        data.insert_persisted(
            id,
            egui::containers::panel::PanelState { outer_rect: egui::Rect::from_min_size(egui::pos2(300.0, 0.0), vec2(120.0, 800.0)) },
        )
    });
    harness.run();
    harness.run();
    assert_eq!(harness.get_by_label(ODILE).rect().width(), 440.0, "a kept splitter width is left alone");
}

/// The shapes of the last frame, flattened, in paint order.
fn flat_shapes(harness: &Harness<'_, InboxApp>) -> Vec<egui::Shape> {
    fn leaves(shape: &egui::Shape, found: &mut Vec<egui::Shape>) {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| leaves(shape, found)),
            shape => found.push(shape.clone()),
        }
    }
    let mut flat = Vec::new();
    harness.output().shapes.iter().for_each(|clipped| leaves(&clipped.shape, &mut flat));
    flat
}

#[test]
fn the_lists_scroll_bar_lies_over_its_rows_and_the_focus_edge_over_its_thumb() {
    let mut harness = Harness::builder()
        .with_size(vec2(1300.0, 500.0))
        .with_context_setup(install_fonts)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app(false));
    harness.run();
    let order = super::list::order(&harness.ctx);
    // The first row open and with the focus: at the top of the list, the thumb passes it.
    harness.state_mut().mailbox_mut().select(Some(order[0]));
    harness.run();
    harness.ctx.memory_mut(|memory| memory.request_focus(super::list::row_id(order[0])));
    harness.run();
    harness.run();
    let view = super::list::view_rect(&harness.ctx).unwrap();
    let row = row_rect(&harness, order[0]).unwrap();
    assert_eq!(row.width(), 440.0, "the row keeps its 440 in a list that scrolls");
    assert!((view.right() - row.right()).abs() < 0.01, "the rows reach the view's right edge: {view:?} {row:?}");
    let strip = egui::Rect::from_x_y_ranges(view.right() - 12.0..=view.right(), view.y_range());
    let shapes = flat_shapes(&harness);
    let (thumb, thumb_rect) = shapes
        .iter()
        .enumerate()
        .rev()
        .find_map(|(at, shape)| match shape {
            egui::Shape::Rect(rect)
                if rect.fill.a() > 0 && rect.rect.width() < 12.0 && strip.expand(0.01).contains_rect(rect.rect) && rect.rect.height() >= 32.0 =>
            {
                Some((at, rect.rect))
            }
            _ => None,
        })
        .expect("a thumb over the rows' right edge");
    assert!(thumb_rect.intersects(row), "the thumb passes the focused row: {thumb_rect:?} {row:?}");
    let edge_after = shapes[thumb..].iter().any(|shape| {
        matches!(shape, egui::Shape::Rect(rect) if rect.fill.a() == 0 && rect.stroke.width > 0.0 && rect.rect.intersects(strip) && row.contains_rect(rect.rect.shrink(0.5)))
    });
    assert!(edge_after, "the focus edge is painted over the thumb");
}

#[test]
fn under_980_the_message_and_the_list_swap_and_the_reader_always_has_room() {
    // From 980 up the reader stands beside the 440 list; under it the window holds one pane beside the folders, and the
    // open message and the list swap: a press or Enter opens, Esc goes back. The window's least width is 800.
    for width in [1100.0, 980.0, 979.0, 900.0, 800.0, 600.0, 300.0] {
        let mut harness = Harness::builder()
            .with_size(vec2(width, 1400.0))
            .with_context_setup(install_fonts)
            .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app(false));
        harness.run();
        let odile = harness.get_by_label(ODILE).rect();
        harness.query_all_by_label_contains("Photos from the coast walk").next().unwrap().click();
        harness.run();
        harness.run();
        let reader = super::reader_rect(&harness.ctx);
        if width >= 980.0 {
            assert_eq!(harness.get_by_label(ODILE).rect().width(), 440.0, "{width}: the rows keep their width beside the reader");
            let reader = reader.unwrap();
            assert!(reader.width() >= 200.0, "{width}: the reader beside the list: {reader:?}");
            assert!(reader.left() >= harness.get_by_label(ODILE).rect().right(), "{width}: beside, not over the list");
        } else if width >= 800.0 {
            // The message stands in the list's place, as wide as the list was with nothing open.
            assert_eq!(rows(&harness), 0, "{width}: the list gave its place to the message");
            let reader = reader.unwrap();
            assert!(reader.width() >= 440.0, "{width}: the reader has the one pane, at least the list's room: {reader:?}");
            assert!(reader.left() <= odile.left() && reader.right() >= odile.right() - 1.0, "{width}: in the list's place: {reader:?} {odile:?}");
            // Esc goes back to the list, its keyed row focused; Enter opens the message again.
            harness.key_press(Key::Escape);
            harness.run();
            harness.run();
            assert!(rows(&harness) > 0, "{width}: Esc brings the list back");
            assert_eq!(super::reader_rect(&harness.ctx), None, "{width}: in the message's place");
            assert!(selected_focused(&harness), "{width}: the keyed row has the focus");
            assert!(harness.get_by_label(ODILE).rect().width() > 440.0, "{width}: the list has the whole pane");
            harness.key_press(Key::Enter);
            harness.run();
            harness.run();
            assert_eq!(rows(&harness), 0, "{width}: Enter opens it again");
            assert!(super::reader_rect(&harness.ctx).is_some_and(|reader| reader.width() >= 440.0));
            harness.key_press(Key::Escape);
            harness.run();
            harness.run();
        }
        // Every row stands in the list's column, one under another.
        let rows: Vec<egui::Rect> = harness.query_all_by_role(Role::ListBoxOption).map(|node| node.rect()).collect();
        for pair in rows.windows(2) {
            assert!(pair[0].bottom() <= pair[1].top() + 0.01, "{width}: rows overlap: {pair:?}");
            assert_eq!(pair[0].left(), pair[1].left());
        }
        for key in [Key::ArrowDown, Key::End, Key::Home, Key::J, Key::K, Key::Enter, Key::Escape] {
            harness.key_press(key);
            harness.run();
        }
    }
}

#[test]
fn in_one_pane_the_keys_move_the_keyed_row_without_reading_what_is_not_shown() {
    let mut app = app(false);
    app.set_reduce_motion(true);
    let mut harness =
        Harness::builder().with_size(vec2(900.0, 1400.0)).with_context_setup(install_fonts).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    let order = super::list::order(&harness.ctx);
    let first = *order.first().unwrap();
    harness.state_mut().mailbox_mut().select(Some(first));
    harness.run();
    assert_eq!(rows(&harness), 0, "the open message is in front");
    harness.key_press(Key::Escape);
    harness.run();
    harness.run();
    assert!(selected_focused(&harness), "Esc: the list in front, the keyed row focused");
    let was: Vec<(u64, bool)> = harness.state().mailbox().messages().iter().map(|row| (row.id, row.seen)).collect();
    for _ in 0..4 {
        harness.key_press(Key::J);
        harness.run();
        assert!(rows(&harness) > 0, "the list stays in front");
        assert_eq!(super::reader_rect(&harness.ctx), None);
    }
    let keyed = harness.state().mailbox().selected().unwrap();
    assert_ne!(keyed, first);
    let now: Vec<(u64, bool)> = harness.state().mailbox().messages().iter().map(|row| (row.id, row.seen)).collect();
    assert_eq!(now, was, "J moved over rows whose messages were not shown: none read");
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
fn a_hovered_rows_snooze_shows_its_key_as_the_sheet_does() {
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
    // Its icon and its one-letter chip, as wide as Done with its own.
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
    // A marked row with a sender long enough to reach its marks at 130 %.
    let mut rows = harness.state().mailbox().messages().to_vec();
    let long = rows.iter_mut().find(|row| row.flagged || row.pinned).unwrap();
    long.sender = "Teodor Lisk of the Coastal Walking Society".to_owned();
    harness.state_mut().mailbox_mut().set_rows(rows);
    harness.run();
    let marked: Vec<_> = harness.state().mailbox().messages().iter().filter(|row| row.flagged || row.pinned).cloned().collect();
    assert!(!marked.is_empty(), "the sample has a flagged or pinned message");
    let mut cut = 0;
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
        let sender = texts_in(&harness, rect).into_iter().find(|(_, words)| words.len() > 1 && row.sender.starts_with(words.trim_end_matches('…')));
        let sender = sender.unwrap_or_else(|| panic!("{name}: the sender is drawn"));
        assert!(sender.0.right() <= leftmost - 11.0 + 0.5, "{name}: the sender {:?} ends 11 short of the marks at {leftmost}", sender.0);
        // A sender cut short of them ends with one ellipsis after its last letter kept.
        assert!(sender.1 == row.sender || (sender.1.ends_with('…') && sender.1.matches('…').count() == 1), "{name}: {:?}", sender.1);
        cut += usize::from(sender.1 != row.sender);
        let time = harness.state().mailbox().row_date(&row);
        assert!(texts_in(&harness, rect).iter().all(|(_, words)| *words != time), "{name}: the hovered row gives its time way");
        checked += 1;
    }
    assert!(checked >= 1, "a marked row in sight was hovered");
    assert!(cut >= 1, "a sender reached its marks and was cut");
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
    mailbox.set_inbox_view(InboxView::Grouped);
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
    /// How many rows' words were worked out on this thread (`list::row_text`).
    pub(super) static TEXTS_BUILT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[test]
fn a_rows_words_are_worked_out_once_for_a_list_of_rows() {
    let mut harness = ten_thousand(InboxView::List);
    harness.run();
    TEXTS_BUILT.with(|built| built.set(0));
    for _ in 0..30 {
        harness.ctx.request_repaint();
        harness.step();
    }
    assert_eq!(TEXTS_BUILT.with(std::cell::Cell::get), 0, "the same rows, nothing worked out again");
    // Scrolled on: only the rows newly built are worked out.
    wheel(&mut harness, 2_000.0);
    let built = TEXTS_BUILT.with(std::cell::Cell::get);
    assert!(built > 0 && built <= 2 * MOST_ROWS, "{built}");
    // The rows listed again (a message read): their words are worked out afresh, the new state with them.
    TEXTS_BUILT.with(|built| built.set(0));
    let first = super::list::first_in_sight(&harness.ctx).unwrap();
    let mut rows = harness.state().mailbox().messages().to_vec();
    for row in &mut rows {
        if row.id == first {
            row.seen = !row.seen;
        }
    }
    let spoken = |harness: &Harness<'static, InboxApp>| {
        let mailbox = harness.state().mailbox();
        mailbox.spoken(mailbox.messages().iter().find(|row| row.id == first).unwrap())
    };
    harness.state_mut().mailbox_mut().set_rows(rows);
    harness.run();
    assert!(TEXTS_BUILT.with(std::cell::Cell::get) > 0, "worked out again");
    harness.get_by_label(&spoken(&harness));
    // The time zone changes: the times are said where the owner now is, worked out afresh.
    let time = |harness: &Harness<'static, InboxApp>| {
        let mailbox = harness.state().mailbox();
        mailbox.row_date(mailbox.messages().iter().find(|row| row.id == first).unwrap())
    };
    let before = time(&harness);
    TEXTS_BUILT.with(|built| built.set(0));
    harness.state_mut().mailbox_mut().set_offset(inboxcraft_time::Offset::from_minutes(150).unwrap());
    harness.run();
    assert!(TEXTS_BUILT.with(std::cell::Cell::get) > 0, "worked out again for the new offset");
    assert_ne!(time(&harness), before, "the row's time moved with the offset");
    harness.get_by_label(&spoken(&harness));
}

/// The sample mailbox over a list of 10 000 messages in three groups (People, Notifications, Newsletters, in turn), longer than the engine lists, in `view`.
fn ten_thousand_mailbox(view: InboxView) -> Mailbox {
    use crate::mailbox::GroupKind;
    let mut mailbox = Mailbox::new(sample::session(clock()).unwrap(), clock());
    mailbox.set_inbox_view(view);
    let model = mailbox.messages().first().unwrap().clone();
    let rows = (0..10_000_u64)
        .map(|n| crate::mailbox::MessageRow {
            id: 1_000_000 + n,
            subject: format!("Message {n}"),
            group: GroupKind::SHOWN.get(usize::try_from(n % 3).unwrap()).copied(),
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
    let mut harness = pane(InboxView::Grouped);
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
    let mut harness = pane(InboxView::List);
    pane_wheel(&mut harness, 280_000.0);
    let built = super::list::built(&harness.ctx).unwrap();
    assert!(built.count <= MOST_ROWS && built.first > 4_000, "{built:?} in the simple list");
}

#[test]
fn a_group_opening_or_folding_above_the_viewport_keeps_the_first_message_in_sight() {
    let mut harness = pane(InboxView::Smart);
    // The last group opens; the list scrolls into its middle.
    harness.ctx.data_mut(|data| data.insert_temp(super::list::group_open_id("Newsletters"), true));
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
    let mut harness = pane(InboxView::List);
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
    for view in [InboxView::List, InboxView::Grouped, InboxView::Smart] {
        let mut harness = ten_thousand(view);
        // A row far down, chosen without the keyboard: nothing scrolls to it.
        harness.state_mut().mailbox_mut().select(Some(1_005_000));
        harness.run();
        assert!(row_rect(&harness, 1_005_000).is_none(), "{view:?}: not built");
        if view == InboxView::Smart {
            // Smart shows a group's first rows: 5 000's group (Newsletters) shows all of them.
            harness.ctx.data_mut(|data| data.insert_temp(super::list::group_open_id("Newsletters"), true));
            harness.run();
        }
        // The next row as the list shows it: by time in List, the next of the same group in Grouped and Smart.
        let order = super::list::order(&harness.ctx);
        let next = order.iter().position(|id| *id == 1_005_000).and_then(|at| order.get(at + 1)).copied();
        assert_eq!(next, Some(if view == InboxView::List { 1_005_001 } else { 1_005_003 }), "{view:?}");
        harness.key_press(Key::ArrowDown);
        harness.run();
        assert_eq!(harness.state().mailbox().selected(), next, "{view:?}");
        assert!(selected_in_sight(&harness), "{view:?}: the keyed row is brought into sight");
        let built = super::list::built(&harness.ctx).unwrap();
        assert!(built.count <= MOST_BUILT, "{built:?}");
    }
}

#[test]
fn a_row_the_pointer_hovers_far_down_the_list_is_built_with_its_actions() {
    let mut harness = ten_thousand(InboxView::List);
    wheel(&mut harness, 300_000.0);
    let first = super::list::first_in_sight(&harness.ctx).unwrap();
    let rect = row_rect(&harness, first + 3).unwrap();
    harness.hover_at(rect.center());
    harness.run();
    assert_eq!(buttons_in(&harness, rect, "Done").len(), 1, "the hovered row shows its actions");
}

#[test]
fn a_message_arriving_above_keeps_the_rows_in_sight_in_place_unless_the_list_is_at_its_top() {
    let mut harness = ten_thousand(InboxView::List);
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
    mailbox.set_inbox_view(InboxView::List);
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
    for view in [InboxView::List, InboxView::Grouped, InboxView::Smart] {
        let mut harness = ten_thousand(view);
        harness.state_mut().mailbox_mut().set_rows(Vec::new());
        harness.run();
        harness.key_press(Key::End);
        harness.run();
        // Senders wait in the screener: its card stands over the empty inbox.
        harness.get_by_label("2 new senders");
    }
    for count in [1_u64, 3] {
        for view in [InboxView::List, InboxView::Grouped, InboxView::Smart] {
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
    let harness = ten_thousand(InboxView::Grouped);
    let mailbox = harness.state().mailbox();
    let other = Group { icon: crate::iconset::IconId::Inbox, name: "Other".to_owned(), messages: Vec::new() };
    for percent in [100, 130] {
        let theme = Theme::new(Mode::Light, TextScale::from_percent(percent).unwrap());
        for view in [InboxView::List, InboxView::Grouped, InboxView::Smart] {
            let grouped = mailbox.groups();
            assert_eq!(grouped.len(), 3);
            let listing = super::list::lines(&theme, mailbox, view, &grouped, &other, true, &|_| true, None);
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
            let end = spans.last().unwrap().1;
            assert_eq!(end, listing.height, "{percent} {view:?}: the last line (or its card) ends the content exactly");
            if view == InboxView::List {
                assert_eq!(spans.last().unwrap().1, listing.height, "the last row's bottom is the content's height");
                assert_eq!(
                    listing.height,
                    theme.size(56.0) * 10_000.0 + theme.size(64.0),
                    "{percent}: no point lost over 10 000 rows and the screener card"
                );
            }
        }
    }
}

/// A row given by its message id is keyed: the selection moved there by the keyboard, from the row before it as the
/// list shows them.
fn key(harness: &mut Harness<'static, InboxApp>, id: u64) {
    let order = super::list::order(&harness.ctx);
    let before = order.iter().position(|shown| *shown == id).and_then(|at| at.checked_sub(1)).and_then(|at| order.get(at)).copied();
    harness.state_mut().mailbox_mut().select(before);
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
    let mut harness = ten_thousand(InboxView::Grouped);
    key(&mut harness, 1_005_001);
    wheel(&mut harness, 20_000.0);
    let before = first_place(&harness);
    assert!(!in_sight(&harness, 1_005_001), "wheeled away from the keyed row");
    arrive(&mut harness, 7, GroupKind::Updates);
    assert_eq!(first_place(&harness), before, "the first message in sight stays where it was");
    // Down: the next row of the keyed row's group, as the grouped list shows them.
    harness.key_press(Key::ArrowDown);
    harness.run();
    assert_eq!(harness.state().mailbox().selected(), Some(1_005_004));
    assert!(selected_in_sight(&harness), "the newly keyed row comes into sight");
}

#[test]
fn a_keyed_row_scrolled_away_from_stays_away_when_a_group_opens_or_folds_above() {
    let mut harness = ten_thousand(InboxView::Smart);
    harness.ctx.data_mut(|data| data.insert_temp(super::list::group_open_id("Newsletters"), true));
    harness.run();
    // 5 003 is in Newsletters, the last group, open.
    key(&mut harness, 1_005_003);
    wheel(&mut harness, 20_000.0);
    let before = first_place(&harness);
    assert!(!in_sight(&harness, 1_005_003), "wheeled away from the keyed row");
    for open in [true, false] {
        harness.ctx.data_mut(|data| data.insert_temp(super::list::group_open_id("People"), open));
        harness.run();
        assert_eq!(first_place(&harness), before, "the first message in sight stays where it was, People open: {open}");
    }
    // Down once: the keys skip the rows a folded group hides (5 004 and 5 005 are in folded groups), to 5 006, the next
    // of Newsletters.
    harness.key_press(Key::ArrowDown);
    harness.run();
    assert_eq!(harness.state().mailbox().selected(), Some(1_005_006));
    assert!(selected_in_sight(&harness), "the newly keyed row comes into sight");
}

#[test]
fn tab_walks_down_the_whole_list_building_rows_as_it_goes() {
    let mut harness = ten_thousand(InboxView::List);
    harness.ctx.memory_mut(|memory| memory.request_focus(super::list::row_id(1_000_000)));
    harness.run();
    // Tab onto a row keys it, so its hover actions come after it in the walk: each row takes a Tab and one for each
    // action that runs (Done, mark read, Delete; Snooze is not yet).
    let mut tabs = 0;
    while harness.ctx.memory(egui::Memory::focused) != Some(super::list::row_id(1_000_040)) && tabs < 40 * 6 {
        harness.key_press(Key::Tab);
        harness.run();
        tabs += 1;
        let built = super::list::built(&harness.ctx).unwrap();
        assert!(built.count <= MOST_ROWS, "{built:?}");
        // The row with the focus is the selected one: never two current rows.
        if let Some(focused) = super::list::focused_row(&harness.ctx) {
            assert_eq!(harness.state().mailbox().selected(), Some(focused), "Tab onto a row keys it");
        }
    }
    // The first row takes its actions a frame after it is keyed, so Tab may pass them once.
    assert!((40 * 4 - 3..=40 * 4).contains(&tabs), "a row and its three actions a step each: {tabs}");
    assert!(in_sight(&harness, 1_000_040), "and is wholly in sight");
    // Back up: only the keyed row shows its actions, so Shift+Tab steps from row to row.
    let mut back = 0;
    while harness.ctx.memory(egui::Memory::focused) != Some(super::list::row_id(1_000_000)) && back < tabs {
        harness.key_press_modifiers(Modifiers::SHIFT, Key::Tab);
        harness.run();
        back += 1;
    }
    assert_eq!(harness.ctx.memory(egui::Memory::focused), Some(super::list::row_id(1_000_000)), "Shift+Tab walks back up");
    assert!(back <= 40 + 3, "a step a row: {back}");
    assert!(in_sight(&harness, 1_000_000));
}

/// Whether the message `id` is read, as the list holds it.
fn seen(harness: &Harness<'static, InboxApp>, id: u64) -> bool {
    harness.state().mailbox().messages().iter().find(|row| row.id == id).is_some_and(|row| row.seen)
}

#[test]
fn tab_onto_rows_shows_their_messages_unread_and_enter_or_a_press_reads_them() {
    let mut app = app(false);
    app.set_reduce_motion(true);
    let mut harness = harness_of(app);
    let order = super::list::order(&harness.ctx);
    let first = *order.first().unwrap();
    harness.ctx.memory_mut(|memory| memory.request_focus(super::list::row_id(first)));
    harness.run();
    let was: Vec<(u64, bool)> = harness.state().mailbox().messages().iter().map(|row| (row.id, row.seen)).collect();
    let was_seen = |id: u64| was.iter().find(|(row, _)| *row == id).is_some_and(|(_, seen)| *seen);
    // Tab crosses rows and their actions: every row it reaches opens in the reader as a preview and stays unread.
    let mut crossed_unread = Vec::new();
    for _ in 0..40 {
        harness.key_press(Key::Tab);
        harness.run();
        let Some(focused) = super::list::focused_row(&harness.ctx) else { continue };
        assert_eq!(harness.state().mailbox().selected(), Some(focused), "Tab onto a row keys it");
        assert_eq!(harness.state().reader().open_id(), Some(focused), "and shows its message");
        assert_eq!(seen(&harness, focused), was_seen(focused), "Tab onto {focused} leaves its read state");
        if !was_seen(focused) && !crossed_unread.contains(&focused) {
            crossed_unread.push(focused);
        }
        if crossed_unread.len() == 3 {
            break;
        }
    }
    assert_eq!(crossed_unread.len(), 3, "Tab crossed three unread rows: {crossed_unread:?}");
    for (id, seen_before) in &was {
        assert_eq!(seen(&harness, *id), *seen_before, "the walk left {id} as it was");
    }
    // Enter on the third reads it.
    let third = crossed_unread[2];
    harness.key_press(Key::Enter);
    harness.run();
    assert!(seen(&harness, third), "Enter reads the previewed message");
    assert!(!seen(&harness, crossed_unread[0]) && !seen(&harness, crossed_unread[1]), "and only it");
    // Focus given by assistive technology shows a row unread too, and a press on it reads it.
    let second = crossed_unread[1];
    let label =
        |harness: &Harness<'static, InboxApp>, id: u64| harness.state().mailbox().messages().iter().find(|row| row.id == id).unwrap().subject.clone();
    let subject = label(&harness, second);
    harness.query_all_by_label_contains(&subject).next().unwrap().focus();
    harness.run();
    harness.run();
    assert_eq!(super::list::focused_row(&harness.ctx), Some(second));
    assert_eq!(harness.state().reader().open_id(), Some(second), "the focused row's message shows");
    assert!(!seen(&harness, second), "focus alone shows it unread");
    harness.query_all_by_label_contains(&subject).next().unwrap().click();
    harness.run();
    assert!(seen(&harness, second), "a press reads it");
    // The list's reading keys read as before: K from the row under it onto the first unread row marks it read.
    let first_unread = crossed_unread[0];
    let at = order.iter().position(|id| *id == first_unread).unwrap();
    let under = order[at + 1];
    let subject = label(&harness, under);
    harness.query_all_by_label_contains(&subject).next().unwrap().focus();
    harness.run();
    harness.run();
    assert!(!seen(&harness, first_unread));
    harness.key_press(Key::K);
    harness.run();
    harness.run();
    assert_eq!(harness.state().mailbox().selected(), Some(first_unread));
    assert!(seen(&harness, first_unread), "J reads the row it moves to");
    // And the row a key leaves, which only took the focus, stays unread: the focus never read it.
    let left = order[order.len() - 1];
    harness.state_mut().mailbox_mut().start(left, crate::mailbox::Act::Unread);
    harness.run();
    assert!(!seen(&harness, left), "marked unread again");
    let above = order[order.iter().position(|id| *id == left).unwrap() - 1];
    let subject = label(&harness, left);
    harness.query_all_by_label_contains(&subject).next().unwrap().focus();
    harness.run();
    harness.run();
    assert_eq!(harness.state().reader().open_id(), Some(left));
    assert!(!seen(&harness, left), "focus alone left it unread");
    harness.key_press(Key::K);
    harness.run();
    harness.run();
    assert_eq!(harness.state().mailbox().selected(), Some(above));
    assert!(!seen(&harness, left), "K moved on: the row it left is still unread");
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
    let mut harness = ten_thousand(InboxView::List);
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
    let mut harness = ten_thousand(InboxView::List);
    // A message opened with Enter: the keyboard is the reader's, and the list's keys leave the open message alone.
    harness.state_mut().mailbox_mut().select(Some(1_000_003));
    harness.run();
    harness.key_press(Key::Enter);
    harness.run();
    assert_eq!(harness.ctx.memory(egui::Memory::focused), None);
    for pressed in [Key::End, Key::PageDown, Key::Home, Key::PageUp, Key::ArrowDown, Key::J, Key::K, Key::ArrowUp] {
        harness.key_press(pressed);
        harness.run();
        assert_eq!(harness.state().mailbox().selected(), Some(1_000_003), "{pressed:?} while reading");
    }
    // Esc in the reader gives the keyboard back: the keyed row takes the focus, and the keys move it again.
    harness.key_press(Key::Escape);
    harness.run();
    assert!(selected_focused(&harness), "Esc puts the focus on the keyed row");
    harness.key_press(Key::J);
    harness.run();
    assert_eq!(harness.state().mailbox().selected(), Some(1_000_004));
    assert!(selected_focused(&harness));
    // A press in the reader takes the keyboard too.
    let reader = egui::pos2(1200.0, 600.0);
    harness.hover_at(reader);
    harness.run();
    harness.drag_at(reader);
    harness.run();
    harness.drop_at(reader);
    harness.run();
    harness.key_press(Key::J);
    harness.run();
    assert_eq!(harness.state().mailbox().selected(), Some(1_000_004), "J while reading");
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

#[test]
fn the_longest_sender_and_subject_cut_with_one_ellipsis_at_130() {
    use crate::theme::{Mode, TextScale, Theme};
    let theme = Theme::new(Mode::Light, TextScale::ALL[4]);
    let keymap = crate::keymap::Keymap::default();
    let mut mailbox = Mailbox::new(sample::session(clock()).unwrap(), clock());
    mailbox.set_inbox_view(InboxView::List);
    let model = mailbox.messages().first().unwrap().clone();
    let sender: String = "Annelise Vandenberghe-Okonkwo-Halvorssen ".repeat(5).chars().take(200).collect();
    let subject = "Q".repeat(400);
    let long = crate::mailbox::MessageRow { id: 900_001, sender: sender.clone(), subject: subject.clone(), preview: String::new(), ..model.clone() };
    let short = crate::mailbox::MessageRow { id: 900_002, sender: "Ines".to_owned(), subject: "Hi".to_owned(), preview: String::new(), ..model };
    mailbox.set_rows(vec![long.clone(), short.clone()]);
    let mut harness = Harness::builder().with_size(vec2(440.0, 828.0)).with_context_setup(install_fonts).build_ui_state(
        move |ui, mailbox: &mut Mailbox| {
            crate::motion::set_reduced(ui.ctx(), true);
            ui.set_style(theme.style());
            let _ = super::list::rows(ui, &theme, &keymap, mailbox);
        },
        mailbox,
    );
    harness.run();
    let rect_of = |harness: &Harness<'static, Mailbox>, row: &crate::mailbox::MessageRow| {
        let name = harness.state().spoken(row);
        harness.get_by_label(&name).rect()
    };
    let (long_rect, short_rect) = (rect_of(&harness, &long), rect_of(&harness, &short));
    assert_eq!(long_rect.height(), theme.size(56.0), "the row keeps its height");
    let texts = |rect: egui::Rect| -> Vec<(egui::Rect, String)> {
        harness
            .output()
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                egui::Shape::Text(text) => {
                    let at = egui::Rect::from_min_size(text.pos, text.galley.size());
                    rect.contains(at.center()).then(|| (at, text.galley.job.text.clone()))
                }
                _ => None,
            })
            .collect()
    };
    let (long_texts, short_texts) = (texts(long_rect), texts(short_rect));
    let time = harness.state().row_date(&long);
    let place = |texts: &[(egui::Rect, String)], origin: egui::Pos2| {
        texts.iter().find(|(_, words)| *words == time).map(|(at, _)| at.translate(-origin.to_vec2()))
    };
    assert_eq!(place(&long_texts, long_rect.min), place(&short_texts, short_rect.min), "the time column's rect is untouched");
    assert!(place(&long_texts, long_rect.min).is_some(), "the time is drawn");
    let line_one =
        long_texts.iter().find(|(_, words)| sender.starts_with(words.trim_end_matches('…')) && words.len() > 1).map(|(_, words)| words.clone());
    let line_two = long_texts.iter().find(|(_, words)| words.starts_with('Q')).map(|(_, words)| words.clone());
    for (line, words) in [("sender", line_one), ("subject", line_two)] {
        let words = words.unwrap_or_else(|| panic!("the {line} is drawn: {long_texts:?}"));
        assert!(words.ends_with('…') && words.matches('…').count() == 1, "the {line} ends with exactly one ellipsis: {words:?}");
        assert!(!words.trim_end_matches('…').ends_with(' '), "no space before it: {words:?}");
    }
    assert!(long_texts.iter().all(|(at, _)| at.right() <= long_rect.right() + 0.5), "nothing runs out of the row: {long_texts:?}");
}

#[test]
fn the_last_row_of_a_card_keeps_its_line_six_points_above_the_cards_edge() {
    let harness = harness(true);
    let theme = crate::theme::Theme::current(&harness.ctx, &crate::theme::Look::of(harness.state().mailbox().appearance()));
    let line = theme.palette.panel_line;
    // Whether a hairline in `panel_line` runs along the foot of `rect`.
    let lined = |rect: egui::Rect| {
        harness.output().shapes.iter().any(|clipped| match &clipped.shape {
            egui::Shape::LineSegment { points, stroke } => {
                stroke.color == line && (points[0].y - (rect.bottom() - 0.5)).abs() < 0.6 && points[0].x <= rect.left() + 1.0
            }
            egui::Shape::Rect(shape) => shape.fill == line && (shape.rect.bottom() - rect.bottom()).abs() < 0.6 && shape.rect.height() <= 2.0,
            _ => false,
        })
    };
    // Kasimir's row is alone on the last card; Odile's has a row under it.
    let last = harness.get_by_label_contains("Kasimir Vell").rect();
    let first = harness.get_by_label(ODILE).rect();
    assert!(lined(first), "a row with a row under it has its line");
    // The design lifts the line only where a row ends on its pane's edge; the card's edge stands 6 points lower.
    assert!(lined(last), "the last row of a card keeps its line");
}

/// The showcase in the grouped list, and the messages its "Other" head stands over: those no group holds.
fn grouped_other() -> (Harness<'static, InboxApp>, Vec<u64>, String) {
    let mut app = app(true);
    app.mailbox_mut().set_inbox_view(InboxView::Grouped);
    let groups = app.mailbox().groups();
    let held: std::collections::HashSet<u64> = groups.iter().flat_map(|group| group.messages.iter().copied()).collect();
    let other: Vec<u64> = app.mailbox().messages().iter().map(|row| row.id).filter(|id| !held.contains(id)).collect();
    let unread = app.mailbox().messages().iter().filter(|row| other.contains(&row.id) && !row.seen).count();
    let head = crate::component::group_head::spoken("Other", u64::try_from(unread).unwrap());
    (harness_of(app), other, head)
}

#[test]
fn done_all_on_the_grouped_lists_other_head_marks_its_messages_done_and_undo_brings_them_back() {
    for by_key in [false, true] {
        let (mut harness, other, head) = grouped_other();
        assert!(!other.is_empty(), "the showcase keeps a message out of the groups");
        let before = harness.state().mailbox().messages().len();
        if by_key {
            harness.get_by_label(&head).focus();
            harness.run();
            harness.key_press(Key::Tab);
            harness.run();
            harness.key_press(Key::Enter);
        } else {
            harness.get_by_label(&head).hover();
            harness.run();
            harness.get_by_role_and_label(Role::Button, "done all").click();
        }
        harness.run();
        let mailbox = harness.state().mailbox();
        assert!(mailbox.messages().iter().all(|row| !other.contains(&row.id)), "by key {by_key}: Other's messages are done");
        assert_eq!(mailbox.messages().len(), before - other.len(), "by key {by_key}: those and no others");
        assert!(mailbox.undoable().is_some(), "by key {by_key}: with its toast");
        undo(&mut harness);
        let mailbox = harness.state().mailbox();
        assert!(other.iter().all(|id| mailbox.messages().iter().any(|row| row.id == *id)), "by key {by_key}: Undo brings them back");
    }
}

/// The sample's simple list alone in a pane `high` points high, motion on, a frame every sixtieth of a second, at
/// `scale` and `ppp`.
fn moving_pane(high: f32, scale: crate::theme::TextScale, ppp: f32) -> Harness<'static, Mailbox> {
    use crate::theme::{Mode, Theme};
    let theme = Theme::new(Mode::Light, scale);
    let keymap = crate::keymap::Keymap::default();
    let mut mailbox = Mailbox::new(sample::session(clock()).unwrap(), clock());
    mailbox.set_inbox_view(InboxView::List);
    let mut harness = Harness::builder()
        .with_size(vec2(440.0, high))
        .with_pixels_per_point(ppp)
        .with_context_setup(install_fonts)
        .with_step_dt(1.0 / 60.0)
        .with_max_steps(240)
        .build_ui_state(
            move |ui, mailbox: &mut Mailbox| {
                crate::motion::set_reduced(ui.ctx(), false);
                ui.set_style(theme.style());
                let asked = super::list::rows(ui, &theme, &keymap, mailbox);
                if asked.out {
                    mailbox.left();
                }
            },
            mailbox,
        );
    harness.run_steps(2);
    harness
}

/// The rectangle of the row of the message called `subject` in a pane, when it is built.
fn pane_row(harness: &Harness<'static, Mailbox>, subject: &str) -> Option<egui::Rect> {
    let row = harness.state().messages().iter().find(|row| row.subject == subject)?.clone();
    harness.query_by_label(&harness.state().spoken(&row)).map(|node| node.rect())
}

/// The subjects of the sample's simple list, in order.
fn subjects(mailbox: &Mailbox) -> Vec<String> {
    mailbox.messages().iter().map(|row| row.subject.clone()).collect()
}

#[test]
fn an_act_the_engine_refuses_leaves_no_leaving_row_and_the_row_stays() {
    let mut harness = moving_pane(828.0, crate::theme::TextScale::default(), 1.0);
    let mailbox = harness.state_mut();
    let rota = mailbox.messages().iter().find(|row| row.subject == "Allotment rota for November").map(|row| row.id).unwrap();
    // A folder of another account: the engine refuses the move.
    let elsewhere = mailbox.messages().iter().find(|row| row.account != mailbox.messages()[0].account).map(|row| row.folder).unwrap();
    let account = mailbox.messages().iter().find(|row| row.id == rota).map(|row| row.account).unwrap();
    assert_ne!(mailbox.messages().iter().find(|row| row.folder == elsewhere).map(|row| row.account), Some(account));
    mailbox.start(rota, crate::mailbox::Act::Move(elsewhere));
    assert!(mailbox.error().is_some(), "the engine refused");
    assert!(mailbox.left_row().is_none(), "no leaving row");
    assert!(mailbox.messages().iter().any(|row| row.id == rota), "the row stays");
    let before = pane_row(&harness, "Allotment rota for November");
    harness.run_steps(3);
    assert_eq!(pane_row(&harness, "Allotment rota for November"), before, "in its place");
}

#[test]
fn undo_while_the_gap_closes_puts_the_row_back_whole_and_nothing_moves_after() {
    let mut harness = moving();
    let next = after_odile(&harness);
    let (odile, below) = (harness.get_by_label(ODILE).rect(), harness.get_by_label(&next).rect());
    harness.get_by_label(ODILE).hover();
    harness.run_steps(2);
    harness.get_by_role_and_label(Role::Button, "Done").click();
    harness.run_steps(4);
    assert!(harness.state().mailbox().left_row().is_some(), "still closing");
    assert!(harness.get_by_label(&next).rect().top() < below.top(), "the gap has begun to close");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run_steps(2);
    assert!(harness.state().mailbox().left_row().is_none(), "Undo lets the leaving row go");
    assert_eq!(harness.get_by_label(ODILE).rect(), odile, "back in its place, whole");
    assert!(!harness.get_by_label(ODILE).accesskit_node().is_disabled(), "a row again");
    assert_eq!(harness.get_by_label(&next).rect(), below, "the row under it where it was");
    harness.run_steps(15);
    assert_eq!(harness.get_by_label(ODILE).rect(), odile, "no movement left over");
}

#[test]
fn two_acts_within_the_close_on_neighbouring_rows_both_land() {
    let mut harness = moving();
    let start = subjects(harness.state().mailbox());
    let at = start.iter().position(|subject| subject == "Allotment rota for November").unwrap();
    let second = after_odile(&harness);
    let third = {
        let mailbox = harness.state().mailbox();
        mailbox.spoken(&mailbox.messages()[at + 2])
    };
    let below = harness.get_by_label(&third).rect().top();
    harness.get_by_label(ODILE).hover();
    harness.run_steps(2);
    harness.get_by_role_and_label(Role::Button, "Done").click();
    harness.run_steps(3);
    harness.get_by_label(&second).hover();
    harness.run_steps(1);
    harness.get_by_role_and_label(Role::Button, "Done").click();
    harness.run_steps(2);
    let now = subjects(harness.state().mailbox());
    assert_eq!(now.len(), start.len() - 2, "both acts landed at once: {now:?}");
    harness.run_steps(20);
    assert!(harness.state().mailbox().left_row().is_none());
    assert_eq!(harness.get_by_label(&third).rect().top(), below - 112.0, "both gaps closed, nothing left over");
}

#[test]
fn done_on_the_keyed_row_with_motion_keys_the_next_row_in_sight() {
    let mut harness = moving();
    let start = subjects(harness.state().mailbox());
    let at = start.iter().position(|subject| subject == "Allotment rota for November").unwrap();
    let rota = harness.state().mailbox().messages()[at].id;
    let next = harness.state().mailbox().messages()[at + 1].id;
    harness.state_mut().mailbox_mut().select(Some(rota));
    harness.run_steps(2);
    harness.key_press(Key::D);
    harness.run_steps(2);
    let mailbox = harness.state().mailbox();
    assert!(mailbox.messages().iter().all(|row| row.id != rota), "done");
    assert_eq!(mailbox.selected(), Some(next), "the row under it is the keyed one");
    harness.run_steps(20);
    let rect = row_rect(&harness, next).unwrap();
    let view = super::list::view_rect(&harness.ctx).unwrap();
    assert!(view.contains_rect(rect), "and in sight: {rect:?} in {view:?}");
}

#[test]
fn a_row_leaving_above_the_viewport_keeps_the_first_row_in_sight_where_it_stands() {
    let mut harness = moving_pane(300.0, crate::theme::TextScale::default(), 1.0);
    harness.hover_at(egui::pos2(220.0, 150.0));
    harness.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: vec2(0.0, -200.0),
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Unknown,
        modifiers: Modifiers::NONE,
    });
    harness.run_steps(40);
    let first = super::list::first_in_sight(&harness.ctx).unwrap();
    let top = pane_rect(&harness, first).unwrap().top();
    let above = harness.state().messages().first().map(|row| row.id).unwrap();
    assert_ne!(above, first, "the first message is above the viewport");
    harness.state_mut().start(above, crate::mailbox::Act::Done);
    for step in 0..16 {
        harness.step();
        assert_eq!(pane_rect(&harness, first).map(|rect| rect.top()), Some(top), "step {step}: the first row in sight stays");
    }
    assert!(harness.state().left_row().is_none(), "gone");
}

#[test]
fn a_row_snoozed_above_the_viewport_keeps_the_first_row_in_sight_where_it_stands() {
    let mut harness = moving_pane(300.0, crate::theme::TextScale::default(), 1.0);
    harness.hover_at(egui::pos2(220.0, 150.0));
    harness.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: vec2(0.0, -200.0),
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Unknown,
        modifiers: Modifiers::NONE,
    });
    harness.run_steps(40);
    let first = super::list::first_in_sight(&harness.ctx).unwrap();
    let top = pane_rect(&harness, first).unwrap().top();
    let above = harness.state().messages().first().map(|row| row.id).unwrap();
    assert_ne!(above, first, "the first message is above the viewport");
    // Snoozed until tomorrow: it fades in its place, then its line goes at once.
    let until = Timestamp::from_unix(1_793_869_200 + 86_400, 0).unwrap();
    harness.state_mut().start(above, crate::mailbox::Act::Snooze(until));
    for step in 0..16 {
        harness.step();
        assert_eq!(pane_rect(&harness, first).map(|rect| rect.top()), Some(top), "step {step}: the first row in sight stays");
    }
    assert!(harness.state().left_row().is_none(), "gone");
}

#[test]
fn the_lines_add_up_to_whole_pixels_while_a_row_closes() {
    use crate::theme::{Mode, TextScale, Theme};
    for view in [InboxView::List, InboxView::Grouped, InboxView::Smart] {
        let mut mailbox = Mailbox::new(sample::session(clock()).unwrap(), clock());
        let invented = sample::showcase(&mailbox);
        mailbox.set_showcase(invented);
        mailbox.set_inbox_view(view);
        let rota = mailbox.messages().iter().find(|row| row.subject == "Allotment rota for November").map(|row| row.id).unwrap();
        mailbox.start(rota, crate::mailbox::Act::Done);
        let other = crate::mailbox::Group { icon: crate::iconset::IconId::Inbox, name: "Other".to_owned(), messages: Vec::new() };
        let groups = mailbox.groups();
        assert_eq!(groups.is_empty(), view == InboxView::List, "{view:?}");
        for scale in [TextScale::default(), TextScale::ALL[4]] {
            let theme = Theme::new(Mode::Light, scale);
            let mut last = f32::INFINITY;
            for closed in [0.0, 0.1, 0.33, 0.5, 0.77, 0.99] {
                let gone = super::list::Gone { closed, faded: closed };
                let listing = super::list::lines(&theme, &mailbox, view, &groups, &other, true, &|_| false, Some(gone));
                let ghost = (0..listing.lines.len()).find(|index| matches!(listing.lines[*index], super::list::Line::Leaving(..))).unwrap();
                let (top, bottom) = listing.span(ghost).unwrap();
                let high = bottom - top;
                assert!(high <= last && high <= theme.size(56.0), "{scale:?} {closed}: {high} shrinks");
                last = high;
                let mut sum = 0.0;
                for index in 0..listing.lines.len() {
                    let (top, bottom) = listing.span(index).unwrap();
                    assert_eq!(top, sum, "{scale:?} {closed}: line {index} starts where the one above ends");
                    for ppp in [1.0_f32, 1.5] {
                        assert_eq!((bottom * ppp).fract(), 0.0, "{scale:?} {closed} at {ppp}: line {index} ends on a whole pixel");
                    }
                    sum = bottom;
                }
                assert_eq!(listing.height, sum, "{view:?} {scale:?} {closed}: the content's height is the lines' sum");
            }
        }
    }
}

#[test]
fn a_groups_head_leaves_with_its_last_row() {
    use crate::mailbox::{GroupKind, MessageRow};
    use crate::theme::{Mode, TextScale, Theme};
    let mut mailbox = Mailbox::new(sample::session(clock()).unwrap(), clock());
    mailbox.set_inbox_view(InboxView::Grouped);
    let model = mailbox.messages().first().unwrap().clone();
    let row = |id: u64, group: GroupKind| MessageRow { id, subject: format!("Message {id}"), group: Some(group), ..model.clone() };
    mailbox.set_rows(vec![row(1, GroupKind::People), row(2, GroupKind::Newsletters), row(3, GroupKind::People)]);
    mailbox.start(2, crate::mailbox::Act::Done);
    // The engine lists the sample's own rows again: the test's list without the row that left.
    mailbox.set_rows(vec![row(1, GroupKind::People), row(3, GroupKind::People)]);
    assert_eq!(mailbox.left_row().map(|left| left.row.id), Some(2), "row 2 is leaving");
    let other = crate::mailbox::Group { icon: crate::iconset::IconId::Inbox, name: "Other".to_owned(), messages: Vec::new() };
    let groups = mailbox.groups();
    let theme = Theme::new(Mode::Light, TextScale::default());
    let mut last = f32::INFINITY;
    for closed in [0.0, 0.25, 0.5, 0.75, 0.99] {
        let gone = super::list::Gone { closed, faded: closed };
        let listing = super::list::lines(&theme, &mailbox, InboxView::Grouped, &groups, &other, false, &|_| true, Some(gone));
        // Newsletters' only row is leaving: no bare head stands over nothing, the head closes with the row.
        let heads: Vec<usize> =
            (0..listing.lines.len()).filter(|index| matches!(listing.lines[*index], super::list::Line::HeadLeaving(..))).collect();
        assert_eq!(heads.len(), 1, "{closed}: the leaving group's head leaves");
        let (top, bottom) = listing.span(heads[0]).unwrap();
        assert!(bottom - top <= last && bottom - top <= theme.size(32.0), "{closed}: {} shrinks", bottom - top);
        last = bottom - top;
        assert!(matches!(listing.lines.get(heads[0] + 1), Some(super::list::Line::Leaving(..))), "the row under it");
    }
    // Once the row is gone, the group is gone with its head.
    let listing = super::list::lines(&theme, &mailbox, InboxView::Grouped, &groups, &other, false, &|_| true, None);
    assert!(!listing.lines.iter().any(|line| matches!(line, super::list::Line::HeadLeaving(..))));
    let heads = listing.lines.iter().filter(|line| matches!(line, super::list::Line::Head(..))).count();
    assert_eq!(heads, 1, "only People's head");
}

#[test]
fn nothing_asks_for_a_frame_once_the_gap_has_closed() {
    let mut harness = moving_pane(828.0, crate::theme::TextScale::default(), 1.0);
    let rota = harness.state().messages().iter().find(|row| row.subject == "Allotment rota for November").map(|row| row.id).unwrap();
    harness.state_mut().start(rota, crate::mailbox::Act::Done);
    harness.step();
    assert!(harness.state().left_row().is_some(), "closing");
    harness.run_steps(20);
    assert!(harness.state().left_row().is_none(), "closed");
    assert!(crate::component::tests::rests(&mut harness, 4), "and at rest");
}

#[test]
fn a_snoozed_row_fades_in_its_place_and_is_then_gone() {
    let mut harness = moving_pane(828.0, crate::theme::TextScale::default(), 1.0);
    let at = subjects(harness.state()).iter().position(|subject| subject == "Allotment rota for November").unwrap();
    let (rota, next) = (harness.state().messages()[at].id, harness.state().messages()[at + 1].subject.clone());
    let below = pane_row(&harness, &next).unwrap().top();
    let later = Timestamp::from_unix(1_793_869_200 + 86_400, 0).unwrap();
    harness.state_mut().start(rota, crate::mailbox::Act::Snooze(later));
    // Fading (125 ms), the row keeps its height: the row under it stays.
    for step in 0..5 {
        harness.step();
        assert!(harness.state().left_row().is_some(), "step {step}: fading");
        assert_eq!(pane_row(&harness, &next).map(|rect| rect.top()), Some(below), "step {step}: no gap closes");
    }
    harness.run_steps(6);
    assert!(harness.state().left_row().is_none(), "gone after the fade");
    assert_eq!(pane_row(&harness, &next).map(|rect| rect.top()), Some(below - 56.0), "and its place with it");
}

#[test]
fn a_row_closes_on_its_card_and_in_the_grouped_list_too() {
    for view in [InboxView::Smart, InboxView::Grouped] {
        let mut app = app(true);
        app.mailbox_mut().set_inbox_view(view);
        app.set_reduce_motion(false);
        let mut harness = Harness::builder()
            .with_size(vec2(1300.0, 900.0))
            .with_context_setup(install_fonts)
            .with_step_dt(1.0 / 60.0)
            .with_max_steps(60)
            .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
        harness.run_steps(2);
        let next = after_odile(&harness);
        let below = harness.get_by_label(&next).rect().top();
        harness.get_by_label(ODILE).hover();
        harness.run_steps(2);
        harness.get_by_role_and_label(Role::Button, "Done").click();
        harness.run_steps(4);
        let mid = harness.get_by_label(&next).rect().top();
        assert!(mid < below && mid > below - 56.0, "{view:?}: closing in its group, {mid} between {below} and {}", below - 56.0);
        harness.run_steps(20);
        assert_eq!(harness.get_by_label(&next).rect().top(), below - 56.0, "{view:?}: closed");
    }
}

#[test]
fn while_a_row_closes_no_line_is_doubled_and_the_row_above_keeps_its_own() {
    let mut harness = moving_pane(828.0, crate::theme::TextScale::default(), 1.0);
    // The third row: a row above it and one under it.
    let at = 2;
    let (rota, above) = (harness.state().messages()[at].id, harness.state().messages()[at - 1].subject.clone());
    let next = harness.state().messages()[at + 1].subject.clone();
    let line = crate::theme::Theme::new(crate::theme::Mode::Light, crate::theme::TextScale::default()).palette.panel_line;
    harness.state_mut().start(rota, crate::mailbox::Act::Done);
    for step in 0..14 {
        harness.step();
        let (top, foot) = (pane_row(&harness, &above).unwrap(), pane_row(&harness, &next).unwrap());
        let ys: Vec<f32> = harness
            .output()
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                egui::Shape::Rect(shape) if shape.fill == line && shape.rect.height() <= 2.0 && shape.rect.width() > 100.0 => {
                    let y = shape.rect.center().y;
                    (y > top.top() && y < foot.bottom() && clipped.clip_rect.contains(shape.rect.center())).then_some(y)
                }
                egui::Shape::LineSegment { points, stroke } if stroke.color == line => {
                    let y = points[0].y;
                    (y > top.top() && y < foot.bottom()).then_some(y)
                }
                _ => None,
            })
            .collect();
        assert!(ys.iter().any(|y| (y - (top.bottom() - 0.5)).abs() < 0.6), "step {step}: the row above keeps its line: {ys:?}");
        for (index, a) in ys.iter().enumerate() {
            for b in ys.iter().skip(index + 1) {
                assert!((a - b).abs() >= 2.0, "step {step}: two lines {a} and {b} touch: {ys:?}");
            }
        }
    }
}

#[test]
fn the_hover_actions_of_a_row_are_found_by_their_message() {
    let mut harness = harness(false);
    let rota = harness.state().mailbox().messages().iter().find(|row| row.subject == "Allotment rota for November").map(|row| row.id).unwrap();
    harness.get_by_label(ODILE).hover();
    harness.run();
    let snooze = harness.get_by_role_and_label(Role::Button, "Snooze").rect();
    // The fourth action, Snooze: what a picker anchors to.
    let found = harness.ctx.read_response(super::list::actions_id(rota).with(super::list::SNOOZE_ACTION)).map(|response| response.rect);
    assert_eq!(found, Some(snooze));
}

#[test]
fn an_invitation_is_grouped_by_its_sender() {
    use crate::mailbox::{GroupKind, MessageRow};
    let mut mailbox = Mailbox::new(sample::session(clock()).unwrap(), clock());
    mailbox.set_inbox_view(InboxView::Grouped);
    let model = mailbox.messages().first().unwrap().clone();
    let row = |id: u64, sender: &str, address: &str, group: GroupKind| MessageRow {
        id,
        sender: sender.to_owned(),
        address: address.to_owned(),
        subject: format!("Invitation {id}"),
        group: Some(group),
        ..model.clone()
    };
    mailbox.set_rows(vec![
        row(1, "Marit Solheim", "marit@example.org", GroupKind::Invitations),
        row(2, "Calendar", "no-reply@calendar.example.org", GroupKind::Invitations),
        row(3, "Room booking", "Do.Not.Reply@rooms.example.org", GroupKind::Invitations),
        row(4, "Pavel Hruska", "pavel@example.org", GroupKind::People),
        row(5, "Forgehub", "notices@forge.example.org", GroupKind::Updates),
    ]);
    let groups: Vec<(String, Vec<u64>)> = mailbox.groups().into_iter().map(|group| (group.name, group.messages)).collect();
    // No Invitations group: a person's invitation stands with People, a system's with the system's mail.
    assert_eq!(groups, [("People".to_owned(), vec![1, 4]), ("Notifications".to_owned(), vec![2, 3, 5])]);
    // The list draws them so: two heads, every row under its sender's.
    let mut app = InboxApp::new(mailbox);
    app.set_reduce_motion(true);
    let harness = harness_of(app);
    assert!(harness.query_by_label_contains("Invitations").is_none(), "no Invitations head");
    let head = |name: &str| harness.get_by_label_contains(name).rect().top();
    let row_top = |subject: &str| harness.get_by_label_contains(subject).rect().top();
    let (people, updates) = (head("People, "), head("Notifications, "));
    assert!(people < row_top("Invitation 1") && row_top("Invitation 1") < updates);
    for subject in ["Invitation 2", "Invitation 3"] {
        assert!(row_top(subject) > updates, "{subject} under Notifications");
    }
    // The group menu offers the groups a sender can be put in: no Invitations there either.
    assert_eq!(GroupKind::SHOWN, [GroupKind::People, GroupKind::Updates, GroupKind::Newsletters]);
    // A hostile address cuts nothing and says person.
    assert_eq!(GroupKind::shown_for(GroupKind::Invitations, "@@@"), GroupKind::People);
    assert_eq!(GroupKind::shown_for(GroupKind::Invitations, &"noreply".repeat(10_000)), GroupKind::Updates);
    assert_eq!(GroupKind::shown_for(GroupKind::Newsletters, "no-reply@example.org"), GroupKind::Newsletters);
}

#[test]
fn j_and_k_move_as_the_arrows_and_rest_in_a_text_field() {
    for view in [InboxView::List, InboxView::Grouped, InboxView::Smart] {
        let mut harness = ten_thousand(view);
        let order = super::list::order(&harness.ctx);
        let (first, second, third) = (order[0], order[1], order[2]);
        // Nothing keyed yet: J keys the first row and gives it the focus.
        harness.key_press(Key::J);
        harness.run();
        assert_eq!(harness.state().mailbox().selected(), Some(first), "{view:?}");
        assert!(selected_focused(&harness), "{view:?}: focus and selection move together");
        // J, the arrow down, K and the arrow up move alike, the focus with them.
        for (key, wanted) in [(Key::J, second), (Key::ArrowDown, third), (Key::K, second), (Key::ArrowUp, first), (Key::ArrowDown, second)] {
            harness.key_press(key);
            harness.run();
            assert_eq!(harness.state().mailbox().selected(), Some(wanted), "{view:?} {key:?}");
            assert!(selected_focused(&harness), "{view:?} {key:?}: the focused row is the keyed one");
            assert!(selected_in_sight(&harness));
        }
    }
    // In the search field J and K are letters: the list rests.
    let mut harness = ten_thousand(InboxView::List);
    harness.key_press(Key::J);
    harness.run();
    let keyed = harness.state().mailbox().selected();
    harness.ctx.memory_mut(|memory| memory.request_focus(super::list::search_id()));
    harness.run();
    for key in [Key::J, Key::K, Key::ArrowDown, Key::ArrowUp] {
        harness.key_press(key);
        harness.run();
        assert_eq!(harness.state().mailbox().selected(), keyed, "{key:?} in a text field");
    }
    harness.event(egui::Event::Text("jk".to_owned()));
    harness.run();
    assert_eq!(harness.state().search_text(), "jk", "typed, not run");
    // Rebound: N moves to the next row, J no more.
    let mut harness = ten_thousand(InboxView::List);
    harness.state_mut().keymap_mut().bind(crate::keymap::NEXT, egui::KeyboardShortcut::new(Modifiers::NONE, Key::N));
    harness.key_press(Key::J);
    harness.run();
    assert_eq!(harness.state().mailbox().selected(), None, "J has no action now");
    harness.key_press(Key::N);
    harness.run();
    assert_eq!(harness.state().mailbox().selected(), Some(1_000_000), "its new key");
    // The palette open: its own keys, the list's rest.
    let ctx = harness.ctx.clone();
    harness.state_mut().run_action(&ctx, crate::keymap::PALETTE);
    harness.run();
    harness.key_press(Key::ArrowDown);
    harness.run();
    assert_eq!(harness.state().mailbox().selected(), Some(1_000_000), "the palette has the arrows");
}

#[test]
fn the_group_menu_opens_from_the_keyboard() {
    for key in [(Modifiers::SHIFT, Key::F10), (Modifiers::NONE, Key::Menu)] {
        let mut harness = harness(true);
        // Odile's row keyed with J: her account is grouped, so her row has the group menu.
        let odile = harness.state().mailbox().messages().iter().find(|row| row.sender == "Odile Brannock").unwrap().id;
        harness.state_mut().mailbox_mut().select_keyed(odile);
        harness.ctx.memory_mut(|memory| memory.request_focus(super::list::row_id(odile)));
        harness.run();
        assert!(harness.query_by_label("Let InboxCraft sort it").is_none(), "closed");
        harness.key_press_modifiers(key.0, key.1);
        harness.run();
        harness.get_by_label("Let InboxCraft sort it");
        // Under the row, its first choice focused, so the arrows and Enter work in it.
        let row = harness.get_by_label(ODILE_READ).rect();
        let people = harness.get_by_role_and_label(Role::MenuItemRadio, "People");
        assert!(people.rect().top() >= row.bottom() - 1.0, "under the row: {:?} {row:?}", people.rect());
        assert!(people.accesskit_node().is_focused(), "{key:?}: the first choice has the focus");
        // Esc closes it and the focus goes back to the row.
        harness.key_press(Key::Escape);
        harness.run();
        assert!(harness.query_by_label("Let InboxCraft sort it").is_none(), "Esc closes it");
        assert!(harness.ctx.memory(|memory| memory.has_focus(super::list::row_id(odile))), "{key:?}: back on the row");
    }
    // Rebound, Shift F10 runs nothing; nothing keyed in a text field, neither.
    let mut harness = harness(true);
    harness.state_mut().keymap_mut().unbind(crate::keymap::MENU);
    harness.key_press_modifiers(Modifiers::SHIFT, Key::F10);
    harness.run();
    assert!(harness.query_by_label("Let InboxCraft sort it").is_none());
}

#[test]
fn a_keyed_step_at_the_edge_scrolls_the_least_and_a_partly_shown_row_comes_whole() {
    let mut harness = ten_thousand(InboxView::List);
    harness.key_press(Key::J);
    harness.run();
    let order = super::list::order(&harness.ctx);
    // Down while the next row is wholly in sight: the keyed row is then the last wholly in sight.
    let mut at = 0;
    while order.get(at + 1).is_some_and(|next| in_sight(&harness, *next)) {
        harness.key_press(Key::J);
        harness.run();
        at += 1;
    }
    let edge = order[at];
    assert_eq!(harness.state().mailbox().selected(), Some(edge));
    let before = row_rect(&harness, edge).unwrap();
    // One step past the edge: the next row comes whole with the least scroll, its bottom on the viewport's.
    harness.key_press(Key::J);
    harness.run();
    let next = order[at + 1];
    let view = super::list::view_rect(&harness.ctx).unwrap();
    let shown = row_rect(&harness, next).unwrap();
    assert!(in_sight(&harness, next), "the row past the edge comes into sight");
    assert!((shown.bottom() - view.bottom()).abs() < 0.01, "the least scroll: {shown:?} in {view:?}");
    let moved = before.top() - row_rect(&harness, edge).unwrap().top();
    assert!(moved > 0.0 && moved <= 56.0, "a step scrolls a row at most: {moved}");
    // A row partly above the viewport, keyed: it comes whole, its top on the viewport's top.
    harness.key_press(Key::Home);
    harness.run();
    for _ in 0..3 {
        harness.key_press(Key::J);
        harness.run();
    }
    // Past the screener card's 64 and 20 into the first rows.
    wheel(&mut harness, 84.0);
    let first = super::list::first_in_sight(&harness.ctx).unwrap();
    let partly = row_rect(&harness, first).unwrap();
    let view = super::list::view_rect(&harness.ctx).unwrap();
    assert!(partly.top() < view.top(), "partly above: {partly:?}");
    for _ in 0..4 {
        if harness.state().mailbox().selected() == Some(first) {
            break;
        }
        harness.key_press(Key::K);
        harness.run();
    }
    assert_eq!(harness.state().mailbox().selected(), Some(first));
    let view = super::list::view_rect(&harness.ctx).unwrap();
    let whole = row_rect(&harness, first).unwrap();
    assert!((whole.top() - view.top()).abs() < 0.01, "its top on the viewport's: {whole:?} in {view:?}");
}

#[test]
fn a_keyed_row_scrolls_into_sight_by_this_frames_viewport_and_when_first_shown() {
    // First shown with a row far down keyed: egui scrolls to it once it knows the viewport.
    let mut mailbox = ten_thousand_mailbox(InboxView::List);
    mailbox.select_keyed(1_000_600);
    let mut app = InboxApp::new(mailbox);
    app.set_reduce_motion(true);
    let mut harness =
        Harness::builder().with_size(vec2(1300.0, 900.0)).with_context_setup(install_fonts).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    assert!(in_sight(&harness, 1_000_600), "the keyed row is in sight when the list first shows");
    // A row low in the window keyed; the window shrinks: a step down brings the next row whole into the smaller
    // viewport, measured this frame, not into last frame's.
    harness.key_press(Key::Home);
    harness.run();
    while harness.state().mailbox().selected().and_then(|id| row_rect(&harness, id)).is_some_and(|rect| rect.top() < 600.0) {
        harness.key_press(Key::J);
        harness.run();
    }
    harness.set_size(vec2(1300.0, 500.0));
    harness.run();
    harness.key_press(Key::J);
    harness.run();
    assert!(selected_in_sight(&harness), "in sight in the smaller window");
    let view = super::list::view_rect(&harness.ctx).unwrap();
    assert!(view.height() < 500.0, "{view:?}");
}

#[test]
fn the_planned_parts_of_the_lists_are_drawn_coming_soon() {
    use inboxcraft_model::Role as MailRole;
    let mut harness = harness(true);
    // Several messages at once: the way in is there, disabled, with the mark.
    let select = harness.get_by_role_and_label(Role::Button, "Select");
    assert!(select.accesskit_node().is_disabled());
    assert!(harness.query_all_by_label("coming soon").count() >= 1);
    // Searching in the text of messages.
    harness.state_mut().search("rota");
    harness.run();
    let text = harness.get_by_label("in message text");
    assert!(text.accesskit_node().is_disabled());
    // Drafts: its rows drawn from the mail there, the list coming soon.
    harness.state_mut().search("");
    harness.state_mut().mailbox_mut().open(Place::View(MailRole::Drafts));
    harness.run();
    harness.get_by_label("coming soon");
    let drafts = harness.state().mailbox().messages().len();
    let disabled = harness
        .root()
        .children_recursive()
        .filter(|node| node.accesskit_node().is_disabled() && node.accesskit_node().label().is_some_and(|label| label.contains(", ")))
        .count();
    assert!(drafts == 0 || disabled >= 1, "{drafts} drafts, {disabled} drawn coming soon");
}

#[test]
fn every_scene_of_the_lists_is_walked_by_tab_and_rests() {
    use crate::mailbox::Pile;
    use inboxcraft_model::Role as MailRole;
    type Scene = (&'static str, fn(&mut InboxApp));
    let scenes: [Scene; 8] = [
        ("smart", |_| {}),
        ("grouped", |app| app.mailbox_mut().set_inbox_view(InboxView::Grouped)),
        ("list", |app| app.mailbox_mut().set_inbox_view(InboxView::List)),
        ("later", |app| app.mailbox_mut().open(Place::Later(Pile::Snoozed))),
        ("new senders", |app| app.mailbox_mut().open(Place::Senders)),
        ("empty search", |app| app.search("zeppelin")),
        ("drafts", |app| app.mailbox_mut().open(Place::View(MailRole::Drafts))),
        ("outbox", |app| app.mailbox_mut().open(Place::Outbox)),
    ];
    for (name, set) in scenes {
        let mut app = app(true);
        app.set_reduce_motion(true);
        set(&mut app);
        // A message open, as Tab onto a row opens its message: the reader stands beside the list all the walk long.
        let first = app.mailbox().messages().first().map(|row| row.id);
        app.mailbox_mut().select(first);
        let mut harness = harness_of(app);
        // The list's pane: the walk's scene. Tab onto a row opens its message, so the reader's controls change under
        // the walk: they are its own slice's.
        let pane = super::list::view_rect(&harness.ctx).map(|view| (view.left() - 1.0, view.left() + 441.0));
        let in_list: Vec<String> = harness
            .root()
            .children_recursive()
            .filter(|node| {
                let middle = node.accesskit_node().bounding_box().map(|b| ((b.x0 + b.x1) / 2.0) as f32);
                pane.zip(middle).is_some_and(|((left, right), x)| x > left && x < right)
            })
            .filter_map(|node| node.accesskit_node().label())
            .collect();
        // Tab is a focus key, not a reading one: every row it crosses keeps its read state.
        let read_state = |harness: &Harness<'static, InboxApp>| -> Vec<(u64, bool)> {
            harness.state().mailbox().messages().iter().map(|row| (row.id, row.seen)).collect()
        };
        let before = read_state(&harness);
        let missed: Vec<String> = crate::component::tests::keyboard_walk(&mut harness).into_iter().filter(|label| in_list.contains(label)).collect();
        assert!(missed.is_empty(), "{name}: Tab never reaches {missed:?}");
        assert_eq!(read_state(&harness), before, "{name}: the walk marks nothing read");
        if matches!(name, "smart" | "grouped" | "list") {
            assert!(before.iter().filter(|(_, seen)| !seen).count() >= 3, "{name}: the walk crosses unread rows");
        }
        assert!(crate::component::tests::rests(&mut harness, 4), "{name}: at rest");
    }
}

#[test]
fn a_rows_menu_never_lingers_away_from_its_row() {
    let open = |harness: &Harness<'static, InboxApp>| crate::component::popup::any_open(&harness.ctx);
    // Open by the keys, then the row scrolls out of the rows built: the menu closes, and the list's keys work again.
    let mut harness = ten_thousand(InboxView::List);
    harness.key_press(Key::J);
    harness.run();
    let keyed = harness.state().mailbox().selected().unwrap();
    harness.key_press_modifiers(Modifiers::SHIFT, Key::F10);
    harness.run();
    assert!(open(&harness), "Shift F10 opens the keyed row's menu");
    // The menu is a float that holds the pointer: the wheel does not scroll the list under it.
    wheel(&mut harness, 4000.0);
    harness.run();
    assert!(row_rect(&harness, keyed).is_some() && open(&harness), "the list stays under the open menu");
    // The list moves another way (a row far below brought into sight): the menu closes with its row.
    let far = super::list::order(&harness.ctx).get(40).copied().unwrap();
    super::list::bring(&harness.ctx, far);
    harness.run();
    assert!(row_rect(&harness, keyed).is_none(), "the row is no longer built");
    assert!(!open(&harness), "its menu closed with it");
    // Another row pressed while the first is away, then the first scrolls back: its menu's end gives the focus to
    // nothing, and the reader keeps the row pressed.
    let order = super::list::order(&harness.ctx);
    let other = *order.iter().find(|id| in_sight(&harness, **id)).unwrap();
    harness.get_by_label(&row_label(&harness, other)).click();
    harness.run();
    assert_eq!(harness.state().mailbox().selected(), Some(other));
    wheel(&mut harness, -8000.0);
    harness.run();
    harness.run();
    assert!(row_rect(&harness, keyed).is_some(), "the first row is back");
    assert!(!open(&harness), "and its menu does not come back");
    assert_eq!(harness.state().mailbox().selected(), Some(other), "the menu's end changed nothing the reader shows");
    assert_eq!(harness.state().reader().open_id(), Some(other));
    // Asked for a message gone from the list: nothing opens, then or when it comes back.
    let rows_before: Vec<crate::mailbox::MessageRow> = harness.state().mailbox().messages().to_vec();
    let gone = keyed;
    let rest: Vec<crate::mailbox::MessageRow> = rows_before.iter().filter(|row| row.id != gone).cloned().collect();
    harness.state_mut().mailbox_mut().set_rows(rest);
    harness.run();
    super::list::ask_menu(&harness.ctx, gone);
    harness.run();
    harness.run();
    assert!(!open(&harness), "no row took the ask");
    harness.state_mut().mailbox_mut().set_rows(rows_before);
    harness.run();
    harness.run();
    assert!(row_rect(&harness, gone).is_some(), "its row is back");
    assert!(!open(&harness), "the ask did not linger until the row came back");
    // Open, then the message goes: the menu closes.
    harness.state_mut().mailbox_mut().select_keyed(keyed);
    harness.ctx.memory_mut(|memory| memory.request_focus(super::list::row_id(keyed)));
    harness.run();
    harness.key_press_modifiers(Modifiers::SHIFT, Key::F10);
    harness.run();
    assert!(open(&harness));
    let rest: Vec<crate::mailbox::MessageRow> = harness.state().mailbox().messages().iter().filter(|row| row.id != keyed).cloned().collect();
    harness.state_mut().mailbox_mut().set_rows(rest);
    harness.run();
    harness.run();
    assert!(!open(&harness), "the menu of a message gone closes");
}

/// What a screen reader says for message `id`'s row.
fn row_label(harness: &Harness<'static, InboxApp>, id: u64) -> String {
    let mailbox = harness.state().mailbox();
    mailbox.messages().iter().find(|row| row.id == id).map(|row| mailbox.spoken(row)).unwrap()
}

#[test]
fn shift_f10_on_a_keyed_row_out_of_sight_brings_it_into_sight_and_opens_its_menu() {
    let mut harness = ten_thousand(InboxView::List);
    harness.key_press(Key::J);
    harness.run();
    let keyed = harness.state().mailbox().selected().unwrap();
    wheel(&mut harness, 4000.0);
    harness.run();
    assert!(!in_sight(&harness, keyed), "scrolled away");
    harness.key_press_modifiers(Modifiers::SHIFT, Key::F10);
    harness.run();
    harness.run();
    assert!(in_sight(&harness, keyed), "the row comes into sight");
    assert!(crate::component::popup::any_open(&harness.ctx), "and its menu opens at it");
    let row = row_rect(&harness, keyed).unwrap();
    let first = harness.get_by_role_and_label(Role::MenuItemRadio, "People").rect();
    assert!(first.top() >= row.bottom() - 1.0, "under the row: {first:?} {row:?}");
    assert_eq!(harness.state().mailbox().selected(), Some(keyed), "the keyed row stays the keyed one");
}

#[test]
fn esc_that_closes_a_menu_in_the_reader_only_closes_it() {
    let mut harness = beside_the_reader();
    let open = harness.state().mailbox().selected();
    // The reader's More menu, opened with the pointer: the reader has the keyboard.
    harness.get_by_role_and_label(Role::Button, "More").click();
    harness.run();
    assert!(crate::component::popup::any_open(&harness.ctx), "More is open");
    harness.key_press(Key::Escape);
    harness.run();
    assert!(!crate::component::popup::any_open(&harness.ctx), "Esc closes it");
    assert!(!selected_focused(&harness), "and leaves the focus where it was, not on the list's row");
    harness.key_press(Key::J);
    harness.run();
    assert_eq!(harness.state().mailbox().selected(), open, "the reader keeps the keyboard: J rests");
    // A second Esc gives the keyboard back to the list.
    harness.key_press(Key::Escape);
    harness.run();
    assert!(selected_focused(&harness), "Esc in the reader: back on the list's row");
}

#[test]
fn the_list_keys_rest_under_a_rows_open_menu() {
    let mut harness = harness(true);
    let odile = harness.state().mailbox().messages().iter().find(|row| row.sender == "Odile Brannock").unwrap().id;
    harness.state_mut().mailbox_mut().select_keyed(odile);
    harness.ctx.memory_mut(|memory| memory.request_focus(super::list::row_id(odile)));
    harness.run();
    // The row keeps the focus while its menu, opened by the pointer, is open over it.
    let row = harness.get_by_label(ODILE_READ).rect();
    harness.hover_at(row.center());
    harness.run();
    harness.event(egui::Event::PointerButton {
        pos: row.center(),
        button: egui::PointerButton::Secondary,
        pressed: true,
        modifiers: Modifiers::NONE,
    });
    harness.event(egui::Event::PointerButton {
        pos: row.center(),
        button: egui::PointerButton::Secondary,
        pressed: false,
        modifiers: Modifiers::NONE,
    });
    harness.run();
    assert!(crate::component::popup::any_open(&harness.ctx), "the menu is open");
    // The menu takes the keyboard from the row, so that Tab starts on its first choice; Esc gives it back.
    assert_eq!(harness.ctx.memory(|memory| memory.focused()), None, "the keyboard left the row for its menu");
    for key in [Key::J, Key::ArrowDown, Key::End] {
        harness.key_press(key);
        harness.run();
        assert_eq!(harness.state().mailbox().selected(), Some(odile), "{key:?} under the open menu");
    }
}

#[test]
fn a_step_in_the_pass_the_list_shrinks_scrolls_by_that_passs_viewport() {
    /// Each pass's viewport, the keyed message and its row's rect as the pass built it.
    type Seen = Vec<(egui::Rect, Option<u64>, Option<egui::Rect>)>;
    let mut app = InboxApp::new(ten_thousand_mailbox(InboxView::List));
    app.set_reduce_motion(true);
    let mut harness = Harness::builder().with_size(vec2(1300.0, 900.0)).with_context_setup(install_fonts).with_max_steps(240).build_ui_state(
        |ui, (app, seen, armed): &mut (InboxApp, Seen, u8)| {
            // Armed (1), the pass after the one that read J shows the error banner, which takes room from the list:
            // the list scrolls to the keyed row in the pass after the key, and its viewport there is not the last
            // pass's, as when the window shrinks right after a key.
            if *armed == 2 {
                *armed = 0;
                app.mailbox_mut().fail(crate::UiError::Shot("the banner".to_owned()));
            }
            if *armed == 1 && ui.input(|input| input.key_pressed(Key::J)) {
                *armed = 2;
            }
            app.ui(ui);
            let row = |id: u64| ui.ctx().viewport(|viewport| viewport.this_pass.widgets.get(super::list::row_id(id)).map(|widget| widget.rect));
            let keyed = app.mailbox().selected();
            if let Some(view) = super::list::view_rect(ui.ctx()) {
                seen.push((view, keyed, keyed.and_then(row)));
            }
        },
        (app, Vec::new(), 0),
    );
    harness.run();
    harness.key_press(Key::Home);
    harness.run();
    let view = harness.state().1.last().unwrap().0;
    // Keyed: a row whose next one stands whole in the viewport, less than a banner's height above its bottom.
    let order = super::list::order(&harness.ctx);
    let mut at = 0;
    loop {
        harness.key_press(Key::J);
        harness.run();
        at += 1;
        let keyed = harness.state().1.last().and_then(|(_, _, row)| *row).unwrap();
        let next_bottom = keyed.bottom() + 56.0;
        if next_bottom + 56.0 > view.bottom() {
            assert!(next_bottom <= view.bottom(), "the next row is whole: {next_bottom} in {view:?}");
            break;
        }
    }
    let next = order[at + 1];
    harness.state_mut().1.clear();
    harness.state_mut().2 = 1;
    harness.key_press(Key::J);
    harness.step();
    harness.run();
    let (shrunk, _, row) =
        harness.state().1.iter().find(|(shown, keyed, _)| *keyed == Some(next) && shown.top() > view.top() + 1.0).copied().unwrap();
    assert!(shrunk.height() < view.height(), "the list's room shrank in that pass: {shrunk:?} from {view:?}");
    let row = row.expect("the keyed row is built in the pass it was keyed in");
    assert!(row.top() >= shrunk.top() - 0.01 && row.bottom() <= shrunk.bottom() + 0.01, "whole in that pass's viewport: {row:?} in {shrunk:?}");
}

#[test]
fn the_pills_line_beside_the_reader_never_wraps_and_leaves_out_what_does_not_fit() {
    for (width, open) in [(1440.0, true), (1440.0, false), (1100.0, true), (1300.0, true)] {
        let mut harness = Harness::builder()
            .with_size(vec2(width, 900.0))
            .with_context_setup(install_fonts)
            .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app(true));
        harness.run();
        if open {
            let first = harness.state().mailbox().messages().first().map(|row| row.id);
            harness.state_mut().mailbox_mut().select(first);
            harness.run();
            harness.run();
        }
        let line = harness.get_by_role_and_label(Role::Button, "Show done").rect();
        // Every part of the line stands on it: none wraps under it.
        for node in harness.query_all_by_label_contains("coming soon").chain(harness.query_all_by_role_and_label(Role::Button, "Select")) {
            let rect = node.rect();
            if rect.top() > line.bottom() + 40.0 {
                continue;
            }
            assert!(rect.top() < line.bottom() && rect.bottom() > line.top(), "{width} open {open}: {rect:?} wraps under {line:?}");
        }
        if !open {
            // With the whole width, the planned Select shows with its mark.
            harness.get_by_role_and_label(Role::Button, "Select");
        }
    }
}

#[test]
fn a_frame_at_rest_keeps_the_lists_order_as_it_was() {
    let mut harness = ten_thousand(InboxView::Grouped);
    let before = super::list::order(&harness.ctx);
    harness.ctx.request_repaint();
    harness.run();
    let after = super::list::order(&harness.ctx);
    assert!(std::sync::Arc::ptr_eq(&before, &after), "the same order is kept, not built again");
    // A change of the drawn order is taken.
    harness.state_mut().mailbox_mut().set_inbox_view(InboxView::List);
    harness.run();
    let list = super::list::order(&harness.ctx);
    assert_ne!(*after, *list, "the list by time is drawn in another order than the grouped one");
    assert_eq!(list.first().copied(), Some(1_000_000), "and the new order is the one kept");
}

/// The sample window `width` wide, motion off.
fn wide(width: f32) -> Harness<'static, InboxApp> {
    let mut app = app(false);
    app.set_reduce_motion(true);
    let mut harness =
        Harness::builder().with_size(vec2(width, 900.0)).with_context_setup(install_fonts).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

#[test]
fn in_one_pane_the_reader_starts_with_back_to_the_list() {
    let mut harness = wide(800.0);
    harness.get_by_label(ODILE).click();
    harness.run();
    harness.run();
    assert_eq!(rows(&harness), 0, "the message stands in the list's place");
    let back = harness.get_by_role_and_label(Role::Button, "Back to the list");
    let reader = super::reader_rect(&harness.ctx).unwrap();
    assert!(reader.contains_rect(back.rect()), "in the reader's tools");
    // First in the tools: no other button of the reader stands left of it on its line.
    let tools = harness
        .query_all_by_role(Role::Button)
        .filter(|node| reader.contains_rect(node.rect()) && (node.rect().center().y - back.rect().center().y).abs() < 1.0);
    assert!(tools.into_iter().all(|node| node.rect().left() >= back.rect().left()), "first in the tools");
    harness.get_by_role_and_label(Role::Button, "Back to the list").click();
    harness.run();
    harness.run();
    assert!(rows(&harness) > 0, "the list is back");
    assert_eq!(super::reader_rect(&harness.ctx), None);
    assert!(selected_focused(&harness), "the keyboard on the row it left");
    // An action by id, in the palette and on the Shortcuts page: it runs as the button does.
    assert!(crate::keymap::action(crate::keymap::BACK).is_some());
    // Beside the list (two panes) it is not drawn.
    let mut harness = wide(1300.0);
    harness.get_by_label(ODILE).click();
    harness.run();
    assert!(harness.query_by_label("Back to the list").is_none(), "two panes: the list is beside the reader");
}

/// The read state of every message of the list.
fn read_states(harness: &Harness<'static, InboxApp>) -> Vec<(u64, bool)> {
    harness.state().mailbox().messages().iter().map(|row| (row.id, row.seen)).collect()
}

/// Where the widget with the keyboard's focus stands, as a screen reader is told.
fn focused_rect(harness: &Harness<'static, InboxApp>) -> Option<egui::Rect> {
    harness.root().children_recursive().find(|node| node.accesskit_node().is_focused()).map(|node| node.rect())
}

#[test]
fn in_one_pane_with_nothing_open_tab_walks_the_list_and_it_stays_in_front() {
    let mut harness = wide(900.0);
    let was = read_states(&harness);
    assert!(rows(&harness) > 0 && super::reader_rect(&harness.ctx).is_none(), "nothing open: the list is in front");
    let mut reached = Vec::new();
    for _ in 0..80 {
        harness.key_press(Key::Tab);
        harness.run();
        if let Some(row) = super::list::focused_row(&harness.ctx) {
            if !reached.contains(&row) {
                reached.push(row);
            }
            assert!(rows(&harness) > 0, "Tab onto {row}: the list stays in front");
            assert_eq!(super::reader_rect(&harness.ctx), None, "and the reader is not shown");
            assert_eq!(harness.state().mailbox().selected(), Some(row), "the row is keyed");
        }
        if reached.len() == 3 {
            break;
        }
    }
    assert_eq!(reached.len(), 3, "Tab walks on from row to row: {reached:?}");
    assert_eq!(read_states(&harness), was, "Tab read nothing");
}

#[test]
fn in_one_pane_with_nothing_open_j_moves_the_keyed_row_and_reads_nothing() {
    let mut harness = wide(900.0);
    let was = read_states(&harness);
    harness.key_press(Key::J);
    harness.run();
    let first = harness.state().mailbox().selected();
    assert!(first.is_some(), "J keys a row");
    harness.key_press(Key::J);
    harness.run();
    assert_ne!(harness.state().mailbox().selected(), first, "and moves on");
    assert!(rows(&harness) > 0 && super::reader_rect(&harness.ctx).is_none(), "the list stays in front");
    assert!(selected_focused(&harness), "the keyed row has the focus");
    assert_eq!(read_states(&harness), was, "J read nothing it did not show");
}

#[test]
fn in_one_pane_enter_opens_into_the_reader_and_esc_goes_back_to_the_row_it_left() {
    let mut harness = wide(900.0);
    // A row keyed by J, then the walk: list, Enter, reader, Esc, list.
    for _ in 0..2 {
        harness.key_press(Key::J);
        harness.run();
    }
    let keyed = harness.state().mailbox().selected().unwrap();
    harness.key_press(Key::Enter);
    harness.run();
    harness.run();
    assert_eq!(rows(&harness), 0, "Enter: the message in the list's place");
    let reader = super::reader_rect(&harness.ctx).unwrap();
    assert!(seen(&harness, keyed), "Enter reads it");
    let focus = focused_rect(&harness).expect("the focus goes into the reader, not nowhere");
    assert!(reader.contains_rect(focus), "the focus is in the reader: {focus:?} in {reader:?}");
    harness.key_press(Key::Tab);
    harness.run();
    let focus = focused_rect(&harness).expect("Tab walks on in the reader");
    assert!(reader.contains_rect(focus), "Tab stays in the reader: {focus:?}");
    harness.key_press(Key::Escape);
    harness.run();
    harness.run();
    assert!(rows(&harness) > 0, "Esc: the list is back");
    assert_eq!(harness.state().mailbox().selected(), Some(keyed));
    assert!(selected_focused(&harness), "the keyboard on the row it left");
    // J after Esc walks the list and reads nothing.
    let was = read_states(&harness);
    harness.key_press(Key::J);
    harness.run();
    assert_ne!(harness.state().mailbox().selected(), Some(keyed));
    assert!(rows(&harness) > 0);
    assert_eq!(read_states(&harness), was, "J after Esc reads nothing");
    // A press opens too, into the reader.
    let pressed = harness.state().mailbox().selected().unwrap();
    let label = harness.state().mailbox().messages().iter().find(|row| row.id == pressed).map(|row| harness.state().mailbox().spoken(row)).unwrap();
    harness.get_by_label(&label).click();
    harness.run();
    harness.run();
    assert_eq!(rows(&harness), 0, "a press: the message in the list's place");
    assert!(seen(&harness, pressed), "a press reads it");
    let reader = super::reader_rect(&harness.ctx).unwrap();
    assert!(focused_rect(&harness).is_some_and(|focus| reader.contains_rect(focus)), "the focus in the reader");
}

#[test]
fn in_one_pane_the_docked_composer_swaps_with_the_list_and_keeps_what_is_written() {
    let mut harness = wide(900.0);
    harness.state_mut().compose();
    harness.run();
    harness.ctx.memory_mut(|memory| memory.request_focus(super::compose::text_id()));
    harness.run();
    harness.event(egui::Event::Text("Dear Odile".to_owned()));
    harness.run();
    assert_eq!(harness.state().composer().map(crate::compose::Composer::text), Some("Dear Odile"));
    assert_eq!(rows(&harness), 0, "the composer stands in the list's place");
    // Esc: the list comes back; nothing written is lost.
    harness.key_press(Key::Escape);
    harness.run();
    harness.run();
    assert!(rows(&harness) > 0, "Esc brings the list back");
    assert_eq!(harness.state().composer().map(crate::compose::Composer::text), Some("Dear Odile"), "what was written is kept");
    // New message brings the composer back as it was.
    let ctx = harness.ctx.clone();
    harness.state_mut().run_action(&ctx, crate::keymap::COMPOSE);
    harness.run();
    harness.run();
    assert_eq!(rows(&harness), 0, "the composer is back in the list's place");
    assert_eq!(harness.state().composer().map(crate::compose::Composer::text), Some("Dear Odile"));
}

#[test]
fn in_the_lists_bar_only_the_thumb_takes_a_press() {
    let mut harness = ten_thousand(InboxView::List);
    let view = super::list::view_rect(&harness.ctx).unwrap();
    let order = super::list::order(&harness.ctx);
    // A row well below the thumb (at the top, 32 high): a press in the bar's strip over its right end opens it.
    let row_id = *order.iter().find(|id| row_rect(&harness, **id).is_some_and(|rect| rect.top() > view.top() + 200.0)).unwrap();
    let row = row_rect(&harness, row_id).unwrap();
    let at = egui::pos2(row.right() - 6.0, row.center().y);
    let before = row_rect(&harness, order[0]).map(|rect| rect.top());
    harness.hover_at(at);
    harness.run();
    harness.drag_at(at);
    harness.run();
    harness.drop_at(at);
    harness.run();
    assert_eq!(row_rect(&harness, order[0]).map(|rect| rect.top()), before, "the list did not scroll");
    assert_eq!(harness.state().mailbox().selected(), Some(row_id), "the row under the press took it");
    // The thumb itself still drags the list (beside the reader now).
    let view = super::list::view_rect(&harness.ctx).unwrap();
    let thumb = egui::pos2(view.right() - 6.0, view.top() + 10.0);
    harness.hover_at(thumb);
    harness.run();
    harness.drag_at(thumb);
    harness.run();
    let to = egui::pos2(thumb.x, thumb.y + 200.0);
    harness.hover_at(to);
    harness.run();
    harness.drop_at(to);
    harness.run();
    assert!(row_rect(&harness, order[0]).is_none(), "dragging the thumb scrolled the list");
}

/// The docked composer of `harness` holding a whole message: to Odile, a subject and "Dear Odile".
fn write_to_odile(harness: &mut Harness<'static, InboxApp>) {
    harness.state_mut().compose();
    harness.run();
    let (Some(composer), mailbox) = harness.state_mut().composer_mut() else { panic!("New message opens the composer") };
    if let Some(field) = composer.typing_mut(crate::compose::Line::To) {
        "Odile Brannock <odile@example.org>,".clone_into(field);
    }
    composer.typed(mailbox, crate::compose::Line::To).unwrap();
    "Tuesday".clone_into(composer.subject_mut());
    "Dear Odile".clone_into(composer.text_mut());
    harness.run();
}

/// What the composer holds and whether it floats.
fn written(harness: &Harness<'static, InboxApp>) -> Option<(String, bool)> {
    harness.state().composer().map(|composer| (composer.text().to_owned(), composer.floating()))
}

#[test]
fn in_one_pane_the_list_over_the_docked_composer_has_its_keys_and_sends_nothing() {
    let mut harness = wide(900.0);
    write_to_odile(&mut harness);
    assert_eq!(rows(&harness), 0, "the composer stands in the list's place");
    harness.key_press(Key::Escape);
    harness.run();
    harness.run();
    assert!(rows(&harness) > 0, "Esc brings the list in front of the composer");
    // Behind the list the composer sends by no way in: not by its key, not by the palette.
    assert!(!harness.state().can_run(crate::keymap::SEND), "the palette offers no Send for a composer not shown");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Enter);
    harness.run();
    harness.run();
    assert_ne!(harness.state().mailbox().last_run(), Some("mail.send"), "Ctrl Enter sends nothing the person cannot see");
    assert_eq!(written(&harness), Some(("Dear Odile".to_owned(), false)), "the composer keeps all it holds, behind the list");
    // The list in front has all its keys; walking it reads nothing and leaves the composer where it is.
    let was = read_states(&harness);
    harness.key_press(Key::J);
    harness.run();
    harness.key_press(Key::J);
    harness.run();
    let second = harness.state().mailbox().selected();
    assert!(second.is_some(), "J keys a row");
    harness.key_press(Key::K);
    harness.run();
    assert_ne!(harness.state().mailbox().selected(), second, "K moves back up");
    harness.key_press(Key::ArrowDown);
    harness.run();
    assert_eq!(harness.state().mailbox().selected(), second, "and the arrows move too");
    assert!(selected_focused(&harness), "the keyed row has the focus");
    assert!(rows(&harness) > 0 && super::reader_rect(&harness.ctx).is_none(), "the list stays in front");
    assert_eq!(read_states(&harness), was, "the walk read nothing");
    assert_eq!(written(&harness), Some(("Dear Odile".to_owned(), false)), "the composer stays behind the list as it was");
    harness.key_press_modifiers(Modifiers::SHIFT, Key::F10);
    harness.run();
    assert!(crate::component::popup::any_open(&harness.ctx), "Shift F10 opens the keyed row's menu");
    harness.key_press(Key::Escape);
    harness.run();
    assert!(!crate::component::popup::any_open(&harness.ctx));
    // Enter opens the keyed message in front; the composer floats with its text.
    let keyed = second.unwrap();
    harness.key_press(Key::Enter);
    harness.run();
    harness.run();
    assert_eq!(rows(&harness), 0, "Enter: the message in the list's place");
    assert_eq!(harness.state().reader().open_id(), Some(keyed));
    assert!(seen(&harness, keyed), "Enter reads it");
    assert_eq!(written(&harness), Some(("Dear Odile".to_owned(), true)), "the composer floats, nothing lost");
}

#[test]
fn the_list_over_the_composer_ends_where_the_window_holds_two_panes_and_two_panes_send() {
    let mut harness = wide(900.0);
    write_to_odile(&mut harness);
    harness.key_press(Key::Escape);
    harness.run();
    harness.run();
    assert!(rows(&harness) > 0, "the list in front of the composer");
    harness.set_size(vec2(1300.0, 900.0));
    harness.run();
    harness.run();
    assert!(rows(&harness) > 0 && harness.query_all_by_role(Role::MultilineTextInput).count() > 0, "two panes: both show");
    harness.set_size(vec2(900.0, 900.0));
    harness.run();
    harness.run();
    assert_eq!(rows(&harness), 0, "narrowed again, the composer is in front: two panes ended the list's turn");
    assert_eq!(written(&harness), Some(("Dear Odile".to_owned(), false)));
    // Two panes: Ctrl Enter sends the composer shown, from inside it.
    let mut harness = wide(1300.0);
    write_to_odile(&mut harness);
    harness.ctx.memory_mut(|memory| memory.request_focus(super::compose::text_id()));
    harness.run();
    assert!(harness.state().can_run(crate::keymap::SEND));
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Enter);
    harness.run();
    assert_eq!(harness.state().mailbox().last_run(), Some("mail.send"), "beside the list the composer sends");
}

/// One pane at 900 with the list in front: J keys a row, the row under it is made unread, the keyed row keeps the
/// focus. The keyed row and the one under it.
fn keyed_over_an_unread_row() -> (Harness<'static, InboxApp>, u64, u64) {
    let mut harness = wide(900.0);
    harness.key_press(Key::J);
    harness.run();
    let keyed = harness.state().mailbox().selected().unwrap();
    let order = super::list::order(&harness.ctx);
    let at = order.iter().position(|id| *id == keyed).unwrap();
    let next = order[at + 1];
    harness.state_mut().mailbox_mut().start(next, crate::mailbox::Act::Unread);
    harness.run();
    harness.state_mut().mailbox_mut().select_keyed(keyed);
    harness.ctx.memory_mut(|memory| memory.request_focus(super::list::row_id(keyed)));
    harness.run();
    assert!(!seen(&harness, next), "the row under the keyed one is unread");
    assert!(rows(&harness) > 0 && super::reader_rect(&harness.ctx).is_none(), "the list in front");
    (harness, keyed, next)
}

#[test]
fn in_one_pane_an_act_on_the_keyed_row_keys_the_next_and_reads_nothing_unseen() {
    // D, Delete, and an act from outside (a notification's Done, the control channel), each on the keyed row.
    for way in ["D", "Delete", "outside"] {
        let (mut harness, keyed, next) = keyed_over_an_unread_row();
        match way {
            "D" => harness.key_press(Key::D),
            "Delete" => harness.key_press(Key::Delete),
            _ => harness.state_mut().mailbox_mut().act(keyed, crate::mailbox::Act::Done),
        }
        harness.run();
        harness.run();
        assert_eq!(harness.state().mailbox().selected(), Some(next), "{way}: the next row is the keyed row");
        assert!(harness.state().mailbox().keyed(), "{way}: keyed, as the keys key it");
        assert!(rows(&harness) > 0 && super::reader_rect(&harness.ctx).is_none(), "{way}: the list stays in front");
        assert!(!seen(&harness, next), "{way}: the next message, not shown, is not marked read");
    }
    // Two panes as before: the next message is shown, and read.
    let mut harness = wide(1300.0);
    harness.key_press(Key::J);
    harness.run();
    let keyed = harness.state().mailbox().selected().unwrap();
    let order = super::list::order(&harness.ctx);
    let next = order[order.iter().position(|id| *id == keyed).unwrap() + 1];
    harness.state_mut().mailbox_mut().start(next, crate::mailbox::Act::Unread);
    harness.run();
    harness.state_mut().mailbox_mut().select_keyed(keyed);
    harness.run();
    harness.key_press(Key::D);
    harness.run();
    harness.run();
    assert_eq!(harness.state().reader().open_id(), Some(next), "two panes: the next message shows");
    assert!(seen(&harness, next), "and is read");
}

#[test]
fn in_one_pane_esc_with_the_row_left_out_of_sight_lands_on_the_nearest_row_in_sight() {
    let mut harness = ten_thousand(InboxView::List);
    harness.set_size(vec2(900.0, 900.0));
    harness.run();
    for _ in 0..20 {
        harness.key_press(Key::J);
        harness.run();
    }
    let keyed = harness.state().mailbox().selected().unwrap();
    harness.key_press(Key::Enter);
    harness.run();
    harness.run();
    assert_eq!(rows(&harness), 0, "Enter: the message in front");
    // The window gets short while the message is read: the row left is out of sight once the list is back.
    harness.set_size(vec2(900.0, 300.0));
    harness.run();
    harness.run();
    harness.key_press(Key::Escape);
    harness.run();
    harness.run();
    assert!(rows(&harness) > 0, "Esc: the list is back");
    assert!(!in_sight(&harness, keyed), "the row left is out of sight");
    let landed = super::list::focused_row(&harness.ctx).expect("the keyboard is not lost: a row has the focus");
    assert!(in_sight(&harness, landed), "the focused row is in sight");
    // The nearest: no row in sight stands between it and the row left.
    let order = super::list::order(&harness.ctx);
    let (from, to) = (order.iter().position(|id| *id == landed).unwrap(), order.iter().position(|id| *id == keyed).unwrap());
    let between = if from < to { &order[from + 1..to] } else { &order[to + 1..from] };
    assert!(between.iter().all(|id| !in_sight(&harness, *id)), "the nearest row in sight to the row left");
    assert_eq!(harness.state().mailbox().selected(), Some(landed), "keyed, as a row the focus reaches is");
}

#[test]
fn in_one_pane_when_the_message_in_front_goes_the_keyboard_lands_on_the_row_nearest_it() {
    let mut harness = wide(900.0);
    harness.state_mut().mailbox_mut().set_after(crate::mailbox::After::List);
    for _ in 0..2 {
        harness.key_press(Key::J);
        harness.run();
    }
    let keyed = harness.state().mailbox().selected().unwrap();
    let order = super::list::order(&harness.ctx);
    let below = order[order.iter().position(|id| *id == keyed).unwrap() + 1];
    harness.key_press(Key::Enter);
    harness.run();
    harness.run();
    assert_eq!(rows(&harness), 0, "Enter: the message in front");
    let was = seen(&harness, below);
    // The message leaves from outside (a notification's Done): nothing is selected after it.
    harness.state_mut().mailbox_mut().act(keyed, crate::mailbox::Act::Done);
    for _ in 0..4 {
        harness.run();
    }
    assert!(rows(&harness) > 0, "the list is back in front");
    assert_eq!(super::list::focused_row(&harness.ctx), Some(below), "the keyboard lands on the row that took its place");
    assert_eq!(seen(&harness, below), was, "landing reads nothing");
}

#[test]
fn across_980_the_keyboard_stays_and_a_row_reached_by_tab_stays_behind_the_list() {
    // A press opens a message beside the list; narrowed, it stands in front with the focus in it; widened, the
    // focus goes to its row, not nowhere.
    let mut harness = wide(1300.0);
    harness.get_by_label(ODILE).click();
    harness.run();
    let odile = harness.state().mailbox().selected().unwrap();
    harness.set_size(vec2(900.0, 900.0));
    harness.run();
    harness.run();
    assert_eq!(rows(&harness), 0, "the message pressed stands in front");
    let reader = super::reader_rect(&harness.ctx).unwrap();
    assert!(focused_rect(&harness).is_some_and(|focus| reader.contains_rect(focus)), "narrowed: the focus in the reader");
    harness.set_size(vec2(1300.0, 900.0));
    for _ in 0..3 {
        harness.run();
    }
    assert_eq!(harness.state().reader().open_id(), Some(odile));
    assert_eq!(super::list::focused_row(&harness.ctx), Some(odile), "widened: the focus on its row");
    // A row reached by Tab after a press is a preview: narrowed, it stays behind the list, the keyboard on it.
    let mut harness = wide(1300.0);
    harness.get_by_label(ODILE).click();
    harness.run();
    harness.query_all_by_label_contains("Teodor Lisk").next().unwrap().focus();
    harness.run();
    harness.run();
    let teodor = harness.state().mailbox().selected().unwrap();
    assert_ne!(teodor, odile);
    let was = read_states(&harness);
    harness.set_size(vec2(900.0, 900.0));
    harness.run();
    harness.run();
    assert!(rows(&harness) > 0, "the row reached by Tab does not jump in front");
    assert!(selected_focused(&harness), "the keyboard stays on it");
    assert_eq!(read_states(&harness), was, "narrowing read nothing");
}

#[test]
fn in_one_pane_a_held_messages_head_starts_with_back_to_the_list_and_the_focus_on_it() {
    let mut harness = wide(900.0);
    harness.get_by_role_and_label(Role::Button, "Review").click();
    harness.run();
    harness.get_by_label("Harbour Swim Club, Winter swim at the harbour steps, 06:00, unread, new sender").click();
    harness.run();
    harness.run();
    harness.get_by_label("Let Harbour Swim Club in?");
    assert_eq!(rows(&harness), 0, "the held message stands in the list's place");
    let back = harness.get_by_role_and_label(Role::Button, "Back to the list").rect();
    assert_eq!(focused_rect(&harness), Some(back), "a press put the focus on the head's first tool");
    // It stands on the middle of the head's avatar, the square right of it.
    let avatar = flat_shapes(&harness)
        .into_iter()
        .map(|shape| shape.visual_bounding_rect())
        .find(|rect| rect.left() > back.right() && rect.left() < back.right() + 20.0 && rect.width() >= 30.0 && rect.top() < back.bottom())
        .unwrap();
    assert!((back.center().y - avatar.center().y).abs() < 0.5, "{back:?} on the middle of the avatar {avatar:?}");
    harness.get_by_role_and_label(Role::Button, "Back to the list").click();
    harness.run();
    harness.run();
    assert_eq!(super::reader_rect(&harness.ctx), None, "a pointer gets back to the list");
    harness.get_by_label("Who gets in?");
}

#[test]
fn a_landing_no_list_took_never_lands_later_in_another_place() {
    let mut harness = wide(900.0);
    harness.get_by_role_and_label(Role::Button, "Review").click();
    harness.run();
    harness.get_by_label("Harbour Swim Club, Winter swim at the harbour steps, 06:00, unread, new sender").click();
    harness.run();
    harness.run();
    // Back to New senders' queue, which has no rows to land on.
    harness.get_by_role_and_label(Role::Button, "Back to the list").click();
    for _ in 0..4 {
        harness.run();
    }
    // The inbox's rows, drawn later with nothing focused, take no keyboard asked for in New senders.
    let ctx = harness.ctx.clone();
    harness.state_mut().run_action(&ctx, crate::keymap::MAIL);
    harness.state_mut().mailbox_mut().open(Place::View(inboxcraft_model::Role::Inbox));
    for _ in 0..3 {
        harness.run();
    }
    assert_eq!(super::list::focused_row(&harness.ctx), None, "no row took a stale landing");
    assert_eq!(harness.state().mailbox().selected(), None, "and nothing was keyed by it");
}

#[test]
fn back_to_the_list_stands_8_before_the_next_tool_with_no_divider_and_runs_only_where_it_does_something() {
    use crate::keymap::BACK;
    let mut harness = wide(900.0);
    harness.get_by_label(ODILE).click();
    harness.run();
    harness.run();
    let back = harness.get_by_role_and_label(Role::Button, "Back to the list").rect();
    let next = harness
        .query_all_by_role(Role::Button)
        .map(|node| node.rect())
        .filter(|rect| (rect.center().y - back.center().y).abs() < 1.0 && rect.left() > back.right())
        .min_by(|a, b| a.left().total_cmp(&b.left()))
        .unwrap();
    assert_eq!(next.left() - back.right(), 8.0, "the frame's gap of 8 and nothing between");
    let between = egui::Rect::from_min_max(egui::pos2(back.right(), back.top()), egui::pos2(next.left(), back.bottom()));
    let lines = flat_shapes(&harness).into_iter().filter(|shape| between.contains_rect(shape.visual_bounding_rect())).count();
    assert_eq!(lines, 0, "no divider the frame does not draw");
    assert!(harness.state().can_run(BACK), "one pane, the message in front: Back runs");
    // Where it does nothing the palette does not offer it.
    harness.get_by_role_and_label(Role::Button, "Back to the list").click();
    harness.run();
    harness.run();
    assert!(!harness.state().can_run(BACK), "one pane, the list in front of a keyed message");
    let mut harness = wide(1300.0);
    harness.get_by_label(ODILE).click();
    harness.run();
    assert!(!harness.state().can_run(BACK), "two panes: the list is beside the reader");
    let mut harness = wide(900.0);
    write_to_odile(&mut harness);
    assert!(harness.state().can_run(BACK), "one pane, the composer in front");
    harness.key_press(Key::Escape);
    harness.run();
    harness.run();
    assert!(!harness.state().can_run(BACK), "one pane, the list in front of the composer");
}

#[test]
fn dragging_the_lists_thumb_keeps_the_grab_point_under_the_pointer_past_an_end_and_back() {
    let thumb = |harness: &Harness<'static, InboxApp>| crate::component::scroll::over_thumb(&harness.ctx).unwrap();
    let mut harness = ten_thousand(InboxView::List);
    let view = super::list::view_rect(&harness.ctx).unwrap();
    let start = thumb(&harness);
    // Held 10 below the thumb's top.
    let grab = egui::pos2(start.center().x, start.top() + 10.0);
    harness.hover_at(grab);
    harness.run();
    harness.drag_at(grab);
    harness.step();
    let under = |harness: &Harness<'static, InboxApp>, y: f32| (thumb(harness).top() + 10.0 - y).abs() < 0.5;
    // Each move puts the grab point under the pointer in the frame of the move.
    for y in [grab.y + 60.0, grab.y + 150.0] {
        harness.hover_at(egui::pos2(grab.x, y));
        harness.step();
        assert!(under(&harness, y), "moved to {y}: the thumb {:?} keeps its grab point under the pointer", thumb(&harness));
    }
    // Past the end the thumb stops there; back up, it waits for the pointer to reach its grab point.
    harness.hover_at(egui::pos2(grab.x, view.bottom() + 200.0));
    harness.step();
    assert!((thumb(&harness).bottom() - view.bottom()).abs() < 0.5, "at the end: {:?}", thumb(&harness));
    let y = view.bottom() - start.height() + 10.0 - 100.0;
    harness.hover_at(egui::pos2(grab.x, y));
    harness.step();
    assert!(under(&harness, y), "back 100 above the end's grab point: the thumb {:?} is under the pointer at {y}", thumb(&harness));
    harness.drop_at(egui::pos2(grab.x, y));
    harness.run();
}

/// The rect of the button `name` on the row `row`.
fn on_row(harness: &Harness<'static, InboxApp>, row: egui::Rect, name: &str) -> egui::Rect {
    let found = harness.query_all_by_role_and_label(Role::Button, name).map(|node| node.rect()).find(|rect| row.contains(rect.center()));
    found.unwrap_or_else(|| panic!("no {name} on the row"))
}

/// Opens the time picker from the Snooze of Odile's row with the pointer; the Snooze's rect.
fn snooze_odile(harness: &mut Harness<'static, InboxApp>) -> egui::Rect {
    let row = harness.get_by_label(ODILE).rect();
    harness.hover_at(row.center());
    harness.run();
    let snooze = on_row(harness, row, "Snooze");
    harness.hover_at(snooze.center());
    harness.run();
    harness.query_all_by_role_and_label(Role::Button, "Snooze").find(|node| node.rect() == snooze).unwrap().click();
    harness.run();
    snooze
}

#[test]
fn the_hover_snooze_opens_the_time_picker_at_the_row_and_a_choice_snoozes_that_message() {
    let mut harness = still();
    let snooze = snooze_odile(&mut harness);
    let title = harness.get_by_label("Snooze until").rect();
    assert!(title.top() > snooze.bottom() && title.top() < snooze.bottom() + 40.0, "under the row's Snooze: {title:?} {snooze:?}");
    harness.get_by_label("Tomorrow morning, Fri 08:00").click();
    harness.run_steps(4);
    assert_eq!(harness.state().mailbox().undoable().map(|undo| undo.text.as_str()), Some("Snoozed until Friday 6 November, 08:00"));
    assert!(harness.state().mailbox().messages().iter().all(|row| row.subject != "Allotment rota for November"), "that message left");
    assert_eq!(rows(&harness), 10, "and no other");
    assert!(harness.query_by_label("Snooze until").is_none(), "the picker closed");
}

#[test]
fn the_keyboard_opens_the_hover_snooze_s_picker_and_esc_gives_the_row_back_its_focus() {
    // The row's group shows under the pointer; the keyboard goes to its Snooze (beside the reader the keyed row's
    // group has no room for it, and H opens the reader's own picker).
    let mut harness = still();
    let row = harness.get_by_label(ODILE).rect();
    harness.hover_at(row.center());
    harness.run();
    let snooze = on_row(&harness, row, "Snooze");
    harness.query_all_by_role_and_label(Role::Button, "Snooze").find(|node| node.rect() == snooze).unwrap().focus();
    harness.run();
    harness.key_press(Key::Enter);
    harness.run();
    let title = harness.get_by_label("Snooze until").rect();
    assert!(title.top() > snooze.bottom() && title.top() < snooze.bottom() + 40.0, "under the row's Snooze: {title:?} {snooze:?}");
    harness.key_press(Key::Escape);
    harness.run();
    assert!(harness.query_by_label("Snooze until").is_none(), "Esc closes it");
    assert!(harness.get_by_label(ODILE).is_focused(), "and the row has the keyboard again");
    assert!(harness.state().mailbox().undoable().is_none(), "nothing snoozed");
    // The pointer's opening closes on Esc the same way.
    snooze_odile(&mut harness);
    harness.get_by_label("Snooze until");
    harness.key_press(Key::Escape);
    harness.run();
    assert!(harness.query_by_label("Snooze until").is_none());
    assert!(harness.state().mailbox().undoable().is_none());
}

/// The id of the listed message with `subject`.
fn id_with(harness: &Harness<'static, InboxApp>, subject: &str) -> u64 {
    harness.state().mailbox().messages().iter().find(|row| row.subject == subject).map(|row| row.id).unwrap()
}

#[test]
fn the_hover_snooze_s_picker_closes_for_good_when_its_row_is_not_drawn() {
    let mut harness = still();
    snooze_odile(&mut harness);
    harness.get_by_label("Snooze until");
    // Another list: the row is not drawn, the picker goes, and does not come back with the row.
    harness.state_mut().mailbox_mut().open(Place::View(inboxcraft_model::Role::Archive));
    harness.run();
    assert!(harness.query_by_label("Snooze until").is_none(), "gone with its row");
    harness.state_mut().mailbox_mut().open(Place::View(inboxcraft_model::Role::Inbox));
    harness.run_steps(5);
    harness.get_by_label(ODILE);
    assert!(harness.query_by_label("Snooze until").is_none(), "and stays closed with the row back");
    assert!(harness.state().mailbox().undoable().is_none(), "nothing snoozed");
    // The list itself undrawn (the calendar, then Mail again): the same.
    snooze_odile(&mut harness);
    harness.get_by_label("Snooze until");
    harness.state_mut().show_calendar();
    harness.run();
    assert!(harness.query_by_label("Snooze until").is_none());
    harness.state_mut().run_action(&egui::Context::default(), crate::keymap::MAIL);
    harness.run_steps(5);
    harness.get_by_label(ODILE);
    assert!(harness.query_by_label("Snooze until").is_none(), "no picker comes back with the list");
}

#[test]
fn the_hover_snooze_s_picker_closes_when_its_message_goes_and_stays_closed_when_it_comes_back() {
    let mut harness = still();
    let rota = id_with(&harness, "Allotment rota for November");
    snooze_odile(&mut harness);
    harness.state_mut().mailbox_mut().act(rota, crate::mailbox::Act::Done);
    harness.run_steps(5);
    assert!(harness.query_by_label(ODILE).is_none(), "the message is done");
    assert!(harness.query_by_label("Snooze until").is_none(), "its picker went with it");
    harness.state_mut().mailbox_mut().undo();
    harness.run_steps(5);
    harness.get_by_label(ODILE);
    assert!(harness.query_by_label("Snooze until").is_none(), "and does not come back with it");
}

#[test]
fn the_hover_snooze_s_picker_stays_with_its_message_when_mail_comes_in_above() {
    let mut harness = still();
    let rota = id_with(&harness, "Allotment rota for November");
    harness.state_mut().mailbox_mut().act(rota, crate::mailbox::Act::Done);
    harness.run_steps(3);
    // Teodor's row, its picker open.
    let row = harness.get_by_label(TEODOR).rect();
    harness.hover_at(row.center());
    harness.run();
    let snooze = on_row(&harness, row, "Snooze");
    harness.hover_at(snooze.center());
    harness.run();
    harness.query_all_by_role_and_label(Role::Button, "Snooze").find(|node| node.rect() == snooze).unwrap().click();
    harness.run();
    let before = harness.get_by_label("Snooze until").rect();
    // The rota comes back above it: Teodor's row moves down, and the picker with it.
    harness.state_mut().mailbox_mut().undo();
    harness.run_steps(3);
    let moved = harness.get_by_label(TEODOR).rect().top() - row.top();
    assert!(moved > 0.0, "the row moved down");
    let after = harness.get_by_label("Snooze until").rect();
    assert!((after.top() - before.top() - moved).abs() < 0.5, "the picker followed it: {before:?} {after:?} {moved}");
    harness.get_by_label("Tomorrow morning, Fri 08:00").click();
    harness.run_steps(4);
    assert!(harness.state().mailbox().messages().iter().all(|row| row.subject != "Photos from the coast walk"), "Teodor's message snoozed");
    harness.get_by_label(ODILE);
}

#[test]
fn a_picker_pushed_out_of_sight_by_a_smaller_window_closes_and_leaves_no_keyboard_in_it() {
    let mut harness = still();
    let row = harness.get_by_label(TEODOR).rect();
    harness.hover_at(row.center());
    harness.run();
    let snooze = on_row(&harness, row, "Snooze");
    harness.hover_at(snooze.center());
    harness.run();
    harness.query_all_by_role_and_label(Role::Button, "Snooze").find(|node| node.rect() == snooze).unwrap().click();
    harness.run();
    // The keyboard goes into the picker's rows.
    harness.key_press(Key::Tab);
    harness.run();
    let inside = harness.ctx.memory(|memory| memory.focused());
    assert!(inside.is_some(), "a row of the picker has the keyboard");
    harness.set_size(vec2(1300.0, row.top() - 20.0));
    harness.run_steps(3);
    assert!(harness.query_by_label("Snooze until").is_none(), "closed with its row out of sight");
    let now = harness.ctx.memory(|memory| memory.focused());
    assert!(now.is_none() || now != inside, "the keyboard is not left in the closed picker: {now:?}");
    harness.set_size(vec2(1300.0, 900.0));
    harness.run_steps(3);
    assert!(harness.query_by_label("Snooze until").is_none(), "and stays closed");
}

#[test]
fn the_rows_snooze_picker_hangs_from_the_button_its_one_id_and_place_name() {
    let mut harness = still();
    let odile = id_with(&harness, "Allotment rota for November");
    // Another message open beside the list, so that the list is 440 wide and the picker has room under the button.
    harness.get_by_label(TEODOR).click();
    harness.run();
    let row = harness.get_by_label(ODILE).rect();
    harness.hover_at(row.center());
    harness.run();
    let snooze = on_row(&harness, row, "Snooze");
    // One id, one place: the button they name is the row's Snooze.
    let named = harness.ctx.read_response(super::list::actions_id(odile).with(super::list::SNOOZE_ACTION)).map(|response| response.rect);
    assert_eq!(named, Some(snooze), "actions_id and SNOOZE_ACTION name the row's Snooze");
    harness.hover_at(snooze.center());
    harness.run();
    harness.query_all_by_role_and_label(Role::Button, "Snooze").find(|node| node.rect() == snooze).unwrap().click();
    harness.run();
    // The picker hangs from that button: 4 under it, its left edge on the button's.
    let float = harness.ctx.memory(|memory| memory.area_rect(super::list::snooze_popup(odile))).unwrap();
    assert!((float.left() - snooze.left()).abs() < 0.5 && (float.top() - (snooze.bottom() + 4.0)).abs() < 0.5, "{float:?} under {snooze:?}");
}

/// Opens the group menu of the row `label` with the pointer, then closes it with Esc.
fn menu_then_esc(harness: &mut Harness<'static, InboxApp>, label: &str) {
    harness.get_by_label(label).click_secondary();
    harness.run();
    assert!(crate::component::popup::any_open(&harness.ctx), "the row's menu opens");
    harness.key_press(Key::Escape);
    harness.run();
    harness.run();
    assert!(!crate::component::popup::any_open(&harness.ctx), "Esc closes it");
}

#[test]
fn beside_the_reader_esc_from_a_rows_pointer_menu_gives_it_the_keyboard_and_shows_the_same_message() {
    let mut harness = wide(1300.0);
    harness.get_by_label(TEODOR).click();
    harness.run();
    let teodor = harness.state().reader().open_id().unwrap();
    let odile = id_with(&harness, "Allotment rota for November");
    menu_then_esc(&mut harness, ODILE);
    assert!(harness.ctx.memory(|memory| memory.has_focus(super::list::row_id(odile))), "the row has the keyboard");
    assert_eq!(harness.state().mailbox().selected(), Some(teodor), "the row the menu was on is not selected");
    assert_eq!(harness.state().reader().open_id(), Some(teodor), "the reader shows the same message");
    assert!(!seen(&harness, odile), "and the row's message is not read");
}

#[test]
fn in_one_pane_esc_from_a_rows_pointer_menu_gives_it_the_keyboard_and_opens_nothing() {
    let (mut harness, keyed, next) = keyed_over_an_unread_row();
    let label = row_label(&harness, next);
    menu_then_esc(&mut harness, &label);
    assert!(harness.ctx.memory(|memory| memory.has_focus(super::list::row_id(next))), "the row has the keyboard");
    assert_eq!(harness.state().mailbox().selected(), Some(keyed), "the row the menu was on is not keyed");
    assert!(rows(&harness) > 0 && super::reader_rect(&harness.ctx).is_none(), "the list stays in front");
    assert!(!seen(&harness, next), "and the row's message is not read");
}

#[test]
fn in_one_pane_another_account_or_search_while_reading_keys_no_row_of_the_new_list() {
    // The message in front leaves the list because the person picked another account or searched: the list comes back
    // in front, and no row of the new list is keyed for them (as beside the reader).
    for way in ["account", "search"] {
        let mut harness = wide(900.0);
        harness.get_by_label(ODILE).click();
        harness.run();
        harness.run();
        assert_eq!(rows(&harness), 0, "{way}: the message in front");
        match way {
            "account" => harness.state_mut().mailbox_mut().narrow(Some(2)),
            _ => harness.state_mut().search("Teodor"),
        }
        for _ in 0..5 {
            harness.run();
        }
        assert!(rows(&harness) > 0, "{way}: the new list in front");
        assert_eq!(harness.state().mailbox().selected(), None, "{way}: nothing keyed");
        assert_eq!(super::list::focused_row(&harness.ctx), None, "{way}: the focus on no row");
        assert_eq!(harness.state().reader().open_id(), None, "{way}: nothing opened");
    }
}

#[test]
fn a_rows_menu_the_keyboard_opened_gives_the_row_the_keyboard_however_it_closes() {
    for width in [900.0_f32, 1300.0] {
        for close in ["Esc", "Enter on a choice", "a press outside"] {
            let mut harness = wide(width);
            harness.key_press(Key::J);
            harness.run();
            let keyed = harness.state().mailbox().selected();
            harness.key_press_modifiers(Modifiers::SHIFT, Key::F10);
            harness.run();
            harness.run();
            assert!(crate::component::popup::any_open(&harness.ctx), "{width} {close}: Shift F10 opens the row's menu");
            match close {
                "Esc" => harness.key_press(Key::Escape),
                "Enter on a choice" => harness.key_press(Key::Enter),
                _ => {
                    let at = egui::pos2(width - 10.0, 880.0);
                    harness.hover_at(at);
                    harness.run();
                    for pressed in [true, false] {
                        harness.event(egui::Event::PointerButton {
                            pos: at,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: Modifiers::NONE,
                        });
                        harness.step();
                    }
                }
            }
            harness.run();
            harness.run();
            assert!(!crate::component::popup::any_open(&harness.ctx), "{width} {close}: the menu closed");
            assert_eq!(harness.state().mailbox().selected(), keyed, "{width} {close}: the same row keyed");
            assert_eq!(super::list::focused_row(&harness.ctx), keyed, "{width} {close}: the row has the keyboard again");
        }
    }
}

#[test]
fn one_esc_closes_the_float_over_the_reader_and_nothing_under_it() {
    // Over the full-width reader: More closes, the reader stays full width.
    let mut harness = wide(1300.0);
    harness.get_by_label(TEODOR).click();
    harness.run();
    harness.state_mut().run_action(&egui::Context::default(), crate::keymap::FULL_WIDTH);
    harness.run();
    harness.run();
    assert!(harness.state().reader().full_width(), "full width");
    harness.get_by_role_and_label(Role::Button, "More").click();
    harness.run();
    harness.run();
    assert!(crate::component::popup::any_open(&harness.ctx), "More is open");
    harness.key_press(Key::Escape);
    harness.run();
    harness.run();
    assert!(!crate::component::popup::any_open(&harness.ctx), "Esc closes More");
    assert!(harness.state().reader().full_width(), "and only More: the reader stays full width");
    // Over the reader in one pane: More closes, the message stays in front.
    let mut harness = wide(900.0);
    harness.get_by_label(ODILE).click();
    harness.run();
    harness.run();
    assert_eq!(rows(&harness), 0, "the message in front");
    harness.get_by_role_and_label(Role::Button, "More").click();
    harness.run();
    harness.run();
    assert!(crate::component::popup::any_open(&harness.ctx), "More is open");
    harness.key_press(Key::Escape);
    harness.run();
    harness.run();
    assert!(!crate::component::popup::any_open(&harness.ctx), "Esc closes More");
    assert!(rows(&harness) == 0 && super::reader_rect(&harness.ctx).is_some(), "and only More: the message stays in front");
}

#[test]
fn a_keyboard_menus_give_back_lapses_when_its_opener_is_not_drawn_again_in_time() {
    // One pane: More opened from the keyboard, closed by a press outside, and the reader gone before More was drawn
    // again. Later another message opens: its first tool takes the keyboard, not More.
    let mut harness = wide(900.0);
    harness.get_by_label(ODILE).click();
    harness.run();
    harness.run();
    harness.get_by_role_and_label(Role::Button, "More").click_accesskit();
    harness.run();
    harness.run();
    assert!(crate::component::popup::any_open(&harness.ctx), "More is open");
    let at = egui::pos2(890.0, 890.0);
    harness.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    harness.step();
    harness.state_mut().run_action(&egui::Context::default(), crate::keymap::BACK);
    harness.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    harness.step();
    harness.run();
    assert!(rows(&harness) > 0, "back to the list");
    harness.key_press(Key::J);
    harness.run();
    harness.key_press(Key::Enter);
    harness.run();
    harness.run();
    harness.run();
    let more = harness.get_by_role_and_label(Role::Button, "More");
    assert!(!more.accesskit_node().is_focused(), "the old ask took the keyboard: {:?}", harness.ctx.memory(|memory| memory.focused()));
}

#[test]
fn the_group_menu_is_the_design_files_box() {
    // 256 × 212: the head 28 under the 1 px edge with no line under it, the choices 32 each from +29, the note 54
    // (8, two caption lines of 18, 10).
    let mut harness = still();
    harness.key_press(Key::J);
    harness.run();
    let keyed = harness.state().mailbox().selected().unwrap();
    harness.key_press_modifiers(Modifiers::SHIFT, Key::F10);
    harness.run();
    harness.run();
    let menu = harness.ctx.memory(|memory| memory.area_rect(super::list::group_menu_id(keyed))).unwrap();
    assert_eq!(menu.size(), vec2(256.0, 212.0), "{menu:?}");
    let head = harness.query_all_by_label("Group").map(|node| node.rect()).find(|rect| menu.contains_rect(*rect)).unwrap();
    assert_eq!((head.top() - menu.top(), head.height()), (1.0, 28.0));
    let first = harness.query_all_by_label("People").map(|node| node.rect()).find(|rect| menu.contains_rect(*rect)).unwrap();
    assert_eq!(first.top() - menu.top(), 29.0, "the first choice straight under the head");
    let painted_line = harness.output().shapes.iter().any(|clipped| match &clipped.shape {
        egui::Shape::Rect(rect) => rect.rect.height() == 1.0 && rect.rect.top() == head.bottom() && rect.rect.width() > 200.0,
        _ => false,
    });
    assert!(!painted_line, "no hairline under the head");
}
