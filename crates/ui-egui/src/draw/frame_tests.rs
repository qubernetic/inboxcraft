// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The app's own window frame: drawn only in a native window that lets the app draw it, never in pictures.

use egui::accesskit::Role;
use egui::{Event, Modifiers, PointerButton, Pos2, Rect, ResizeDirection, ViewportCommand, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
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

fn harness_with(frame: WindowFrame, setup: impl FnOnce(&mut InboxApp)) -> Harness<'static, InboxApp> {
    let mut app = InboxApp::new(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    app.set_own_frame(true);
    app.set_frame(frame);
    setup(&mut app);
    let mut harness =
        Harness::builder().with_size(vec2(1100.0, 700.0)).with_context_setup(install_fonts).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

/// The viewport commands of the last frame.
fn commands(harness: &Harness<'_, InboxApp>) -> Vec<ViewportCommand> {
    harness.output().viewport_output.values().flat_map(|viewport| viewport.commands.iter().cloned()).collect()
}

/// Moves the pointer to `at`, presses the primary button there and returns the commands of that frame.
fn press_at(harness: &mut Harness<'static, InboxApp>, at: Pos2) -> Vec<ViewportCommand> {
    harness.hover_at(at);
    harness.step();
    harness.event(Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    harness.step();
    commands(harness)
}

/// Presses at `at`, drags 20 points down and returns every command on the way.
fn drag_from(harness: &mut Harness<'static, InboxApp>, at: Pos2) -> Vec<ViewportCommand> {
    let mut seen = press_at(harness, at);
    for step in 1..=4 {
        harness.event(Event::PointerMoved(at + vec2(0.0, step as f32 * 5.0)));
        harness.step();
        seen.extend(commands(harness));
    }
    seen
}

#[test]
fn a_press_at_an_edge_resizes_the_window_even_where_the_desktop_keeps_the_window_s_place_to_itself() {
    // Wayland tells no window where it is, so egui knows the window's size but not its rect; the test harness neither.
    for (at, direction) in [
        (pos2(1098.0, 400.0), ResizeDirection::East),
        (pos2(1.0, 400.0), ResizeDirection::West),
        (pos2(600.0, 698.0), ResizeDirection::South),
        (pos2(1098.0, 698.0), ResizeDirection::SouthEast),
    ] {
        for frame in [WindowFrame::Strip, WindowFrame::TopRow] {
            let mut harness = harness(true, frame);
            let seen = press_at(&mut harness, at);
            assert!(seen.contains(&ViewportCommand::BeginResize(direction)), "{frame:?} {at:?}: {seen:?}");
        }
    }
}

#[test]
fn dragging_the_top_row_beside_the_panes_moves_the_window() {
    let mut harness = harness(true, WindowFrame::TopRow);
    // Above the panes, below the resizing edge.
    let seen = drag_from(&mut harness, pos2(600.0, 8.0));
    assert!(seen.contains(&ViewportCommand::StartDrag), "{seen:?}");
}

/// The rects of the window controls and of every other control or field.
fn controls_and_others(harness: &Harness<'_, InboxApp>) -> (Vec<Rect>, Vec<(String, Rect)>) {
    let ours = ["Minimise", "Maximise", "Close the window"];
    let window = ours.iter().map(|name| harness.get_by_label(name).rect()).collect();
    let mut others = Vec::new();
    for role in [Role::Button, Role::TextInput, Role::SearchInput, Role::ComboBox] {
        for node in harness.query_all_by_role(role) {
            let name = node.accesskit_node().label().unwrap_or_default();
            if !ours.contains(&name.as_str()) {
                others.push((name, node.rect()));
            }
        }
    }
    (window, others)
}

#[test]
fn the_window_controls_in_the_top_row_lie_over_no_other_control() {
    for settings in [false, true] {
        let scene = if settings { "settings" } else { "mail" };
        let harness = harness_with(WindowFrame::TopRow, |app| {
            if settings {
                app.open_settings();
            }
        });
        let (window, others) = controls_and_others(&harness);
        for control in &window {
            for (name, rect) in &others {
                assert!(!control.intersects(*rect), "{scene}: a window control {control:?} lies over {name:?} {rect:?}");
            }
        }
    }
}

#[test]
fn the_window_s_corners_are_round_only_while_the_app_draws_the_frame() {
    assert!(harness(true, WindowFrame::Strip).state().window_corner(&egui::Context::default()) > 0.0);
    assert!(harness(true, WindowFrame::TopRow).state().window_corner(&egui::Context::default()) > 0.0);
    assert_eq!(harness(true, WindowFrame::System).state().window_corner(&egui::Context::default()), 0.0);
    assert_eq!(harness(false, WindowFrame::Strip).state().window_corner(&egui::Context::default()), 0.0);
}

#[test]
fn the_strip_s_window_controls_sit_in_its_middle() {
    let mut app = InboxApp::new(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    app.set_own_frame(true);
    app.set_frame(WindowFrame::Strip);
    // The whole window, as eframe gives it; the harness's own `ui` has a margin.
    let mut harness = Harness::builder().with_size(vec2(1100.0, 700.0)).with_context_setup(install_fonts).build_ui_state(
        |ui, app: &mut InboxApp| {
            let whole = ui.ctx().content_rect();
            ui.scope_builder(egui::UiBuilder::new().max_rect(whole), |ui| app.ui(ui));
        },
        app,
    );
    harness.run();
    for name in ["Minimise", "Maximise", "Close the window"] {
        let rect = harness.get_by_label(name).rect();
        let (above, below) = (rect.top(), super::frame::STRIP - rect.bottom());
        assert!((above - below).abs() <= 1.0, "{name}: {above} above, {below} below in {rect:?}");
    }
}

#[test]
fn the_window_moves_again_after_a_move_whose_release_the_desktop_kept() {
    // While the desktop moves the window it holds the pointer: the app never hears the button come up.
    for (frame, at) in [(WindowFrame::Strip, pos2(500.0, 17.0)), (WindowFrame::TopRow, pos2(600.0, 8.0))] {
        let mut harness = harness(true, frame);
        assert!(drag_from(&mut harness, at).contains(&ViewportCommand::StartDrag), "{frame:?}: the first move");
        let seen = drag_from(&mut harness, at + vec2(40.0, 0.0));
        assert!(seen.contains(&ViewportCommand::StartDrag), "{frame:?}: the second move {seen:?}");
    }
}
