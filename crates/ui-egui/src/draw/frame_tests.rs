// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The app's own window frame: drawn only in a native window that lets the app draw it, never in pictures.

use egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use inboxcraft_engine::appearance::WindowFrame;
use inboxcraft_time::{FakeClock, Timestamp};

use super::InboxApp;
use crate::mailbox::Mailbox;
use crate::sample;
use crate::theme::install_fonts;

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

fn harness(own: bool, frame: WindowFrame) -> Harness<'static, InboxApp> {
    let mut app = InboxApp::new(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    app.set_own_frame(own);
    app.set_frame(frame);
    let mut harness =
        Harness::builder().with_size(vec2(1100.0, 700.0)).with_context_setup(install_fonts).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

#[test]
fn the_strip_shows_the_title_and_the_window_controls() {
    let harness = harness(true, WindowFrame::Strip);
    assert!(harness.state().draws_frame());
    for name in ["Minimise", "Maximise", "Close the window"] {
        harness.get_by_label(name);
    }
}

#[test]
fn the_top_row_shows_the_window_controls_without_a_strip() {
    let harness = harness(true, WindowFrame::TopRow);
    harness.get_by_label("Close the window");
}

#[test]
fn no_own_frame_with_the_system_s_title_bar_or_outside_a_native_window() {
    for (own, frame) in [(true, WindowFrame::System), (false, WindowFrame::Strip), (false, WindowFrame::TopRow)] {
        let harness = harness(own, frame);
        assert!(!harness.state().draws_frame(), "{own} {frame:?}");
        assert!(harness.query_by_label("Close the window").is_none(), "{own} {frame:?}");
    }
}

#[test]
fn the_frame_is_one_of_the_appearance_choices() {
    let mut app = InboxApp::new(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    app.set_frame(WindowFrame::TopRow);
    assert_eq!(app.mailbox().appearance().frame, WindowFrame::TopRow);
}
