// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The message list on the design system: the list card, the grouped inbox and screener cards and the hover actions,
//! hidden in the app and shown by the showcase `ui_shot` fills.

use egui::accesskit::Role;
use egui::{Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use inboxcraft_time::{FakeClock, Timestamp};

use super::InboxApp;
use crate::mailbox::Mailbox;
use crate::sample;
use crate::theme::install_fonts;

/// The hover actions and the parts of the cards that have no command yet.
const SHOWCASE_ONLY: [&str; 6] = ["Done", "Mark as read", "Delete", "Done all", "Review", "Show all (7)"];

const ODILE: &str = "Odile Brannock, Allotment rota for November, 08:35, unread";

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

fn app(showcase: bool) -> InboxApp {
    let mut mailbox = Mailbox::new(sample::session(clock()).unwrap(), clock());
    if showcase {
        let invented = sample::showcase(&mailbox);
        mailbox.set_showcase(invented);
    }
    InboxApp::new(mailbox)
}

fn harness(showcase: bool) -> Harness<'static, InboxApp> {
    let mut harness = Harness::builder()
        .with_size(vec2(1300.0, 900.0))
        .with_context_setup(install_fonts)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app(showcase));
    harness.run();
    harness
}

fn quick_tooltips(harness: &Harness<'static, InboxApp>) {
    harness.ctx.all_styles_mut(|style| {
        style.interaction.tooltip_delay = 0.0;
        style.interaction.show_tooltips_only_when_still = false;
    });
}

#[test]
fn the_app_draws_no_group_or_screener_card_and_no_hover_action() {
    let mut harness = harness(false);
    harness.get_by_label(ODILE).hover();
    harness.run();
    harness.key_press(Key::ArrowDown);
    harness.run();
    assert!(harness.state().mailbox().keyed());
    for name in SHOWCASE_ONLY.into_iter().chain(["People", "Updates", "2 new senders"]) {
        assert!(harness.query_by_label(name).is_none(), "{name} shows in the app");
    }
    assert_eq!(harness.query_all_by_role(Role::ListBoxOption).count(), 11);
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
    let done = harness.get_by_role_and_label(Role::Button, "Done").rect();
    assert!(rect.contains(done.center()), "{done:?} in {rect:?}");
    // A read row offers to mark it unread.
    harness.get_by_role_and_label(Role::Button, if row.seen { "Mark as unread" } else { "Mark as read" });
}

#[test]
fn the_row_text_fades_out_before_the_hover_actions() {
    let mut harness = harness(true);
    harness.get_by_label(ODILE).hover();
    harness.run();
    let done = harness.get_by_role_and_label(Role::Button, "Done").rect();
    let row = harness.get_by_label(ODILE).rect();
    let mut seen = 0;
    for clipped in &harness.output().shapes {
        let egui::Shape::Text(text) = &clipped.shape else { continue };
        let rect = text.visual_bounding_rect();
        if !(row.contains(rect.center()) || row.contains(rect.left_center())) || text.galley.job.text.chars().count() < 3 {
            continue;
        }
        if text.galley.job.text.starts_with("Odile") || text.galley.job.text.starts_with("Allotment") {
            seen += 1;
            assert!(clipped.clip_rect.right() < done.left(), "{:?} reaches {} past {}", text.galley.job.text, clipped.clip_rect.right(), done.left());
        }
    }
    assert!(seen >= 2, "the sender and the subject were drawn faded");
}

#[test]
fn every_button_of_the_list_and_the_rail_has_a_tooltip_with_its_key_chip() {
    let mut harness = harness(true);
    *harness.state_mut().keymap_mut() = crate::component::gallery::keymap();
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
