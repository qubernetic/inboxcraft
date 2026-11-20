// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! New message at the top of the index, in the window: on every place its press runs `compose.new` as its key `C` does
//! (the composer opens), and nothing of it runs under the palette, whatever pressed it.

use egui::accesskit::Role;
use egui::{Rect, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_time::{FakeClock, Timestamp};

use super::InboxApp;
use crate::keymap;
use crate::mailbox::Mailbox;
use crate::sample;

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

/// The sample in the frames' window, 1440 × 900, motion reduced.
fn window() -> Harness<'static, InboxApp> {
    let mut app = InboxApp::at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    app.set_reduce_motion(true);
    let mut harness =
        Harness::builder().with_size(vec2(1440.0, 900.0)).with_theme(egui::Theme::Dark).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

/// The index's New message, its rect.
fn new_message(harness: &Harness<'static, InboxApp>) -> Rect {
    let index = super::shell::laid(&harness.ctx).unwrap().index;
    harness
        .query_all_by_role(Role::Button)
        .filter(|node| node.accesskit_node().label().as_deref() == Some("New message"))
        .map(|node| node.rect())
        .find(|rect| index.contains_rect(*rect))
        .unwrap()
}

/// Presses the index's New message with the pointer.
fn press(harness: &mut Harness<'static, InboxApp>) {
    let at = new_message(harness).center();
    harness.hover_at(at);
    harness.run();
    harness.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
    harness.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
    harness.run();
}

#[test]
fn new_message_writes_a_new_message_from_every_place() {
    for place in [keymap::HOME, keymap::MAIL, keymap::CALENDAR, keymap::LATER, keymap::SETTINGS] {
        let mut harness = window();
        let ctx = harness.ctx.clone();
        harness.state_mut().run_action(&ctx, place);
        harness.run();
        assert!(harness.state().composer.is_none(), "{place}: nothing written yet");
        let drafts = harness.state().mailbox.drafts_total();
        press(&mut harness);
        assert!(harness.state().composer.is_some(), "{place}: New message opens the composer, as C does");
        assert!(!harness.state().settings.is_open(), "{place}: the settings draw no composer");
        assert_eq!(harness.state().mailbox.drafts_total(), drafts + 1, "{place}: one new draft (compose.new)");
    }
}

#[test]
fn nothing_of_new_message_runs_under_the_palette() {
    let mut harness = window();
    let ctx = harness.ctx.clone();
    harness.state_mut().run_action(&ctx, keymap::PALETTE);
    harness.run();
    // A screen reader's activate reaches under egui's modal layers: the gate stops it.
    let index = super::shell::laid(&harness.ctx).unwrap().index;
    harness
        .query_all_by_role(Role::Button)
        .find(|node| node.accesskit_node().label().as_deref() == Some("New message") && index.contains_rect(node.rect()))
        .unwrap()
        .click_accesskit();
    harness.run();
    assert!(harness.state().composer.is_none(), "no composer under the palette");
}
