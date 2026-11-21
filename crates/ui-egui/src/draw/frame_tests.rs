// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The app's own window frame, the command line as the title bar: drawn only in a native window that lets the app draw
//! it, never in pictures. The four `window-frame/*` frames are state tests here.

use egui::accesskit::Role;
use egui::{Event, Key, Modifiers, PointerButton, Pos2, Rect, ResizeDirection, ViewportCommand, ViewportId, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_engine::appearance::WindowFrame;
use inboxcraft_time::{FakeClock, Timestamp};

use super::InboxApp;
use crate::keymap;
use crate::mailbox::Mailbox;
use crate::sample;
use crate::theme::install_fonts;

/// The three window controls, by what a screen reader hears.
const CONTROLS: [&str; 3] = ["Minimise", "Maximise", "Close the window"];

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

/// The sample at 1100 × 700 in a native window that may draw its frame (`own`), the frame `frame` chosen; the app in
/// the whole window, as eframe gives it to it.
fn harness(own: bool, frame: WindowFrame) -> Harness<'static, InboxApp> {
    harness_sized(own, frame, vec2(1100.0, 700.0), |_| {})
}

fn harness_sized(own: bool, frame: WindowFrame, size: egui::Vec2, setup: impl FnOnce(&mut InboxApp)) -> Harness<'static, InboxApp> {
    harness_with(own, frame, size, None, setup)
}

/// [`harness_sized`], each step `dt` seconds when given.
fn harness_with(own: bool, frame: WindowFrame, size: egui::Vec2, dt: Option<f32>, setup: impl FnOnce(&mut InboxApp)) -> Harness<'static, InboxApp> {
    let mut app = InboxApp::at_mail(Mailbox::new(sample::sending(clock()).unwrap(), clock()));
    app.set_own_frame(own);
    app.set_frame(frame);
    setup(&mut app);
    let mut builder = Harness::builder().with_size(size).with_context_setup(install_fonts);
    if let Some(dt) = dt {
        builder = builder.with_step_dt(dt);
    }
    let mut harness = builder.build_ui_state(
        |ui, app: &mut InboxApp| {
            let whole = ui.ctx().content_rect();
            ui.scope_builder(egui::UiBuilder::new().max_rect(whole), |ui| app.ui(ui));
        },
        app,
    );
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

/// Presses and lets go at `at`; every command on the way.
fn click_at(harness: &mut Harness<'static, InboxApp>, at: Pos2) -> Vec<ViewportCommand> {
    let mut seen = press_at(harness, at);
    harness.event(Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    harness.step();
    seen.extend(commands(harness));
    seen
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

/// The command line's region.
fn line(harness: &Harness<'_, InboxApp>) -> Rect {
    super::shell::laid(&harness.ctx).unwrap().command
}

/// A point of the command line that only the title bar takes: the mark's column, over the mark (which takes no press).
fn free(harness: &Harness<'_, InboxApp>) -> Pos2 {
    let line = line(harness);
    // The middle of the mark's column, left of the field (the index's width: 232, or 56 under 1280).
    pos2(((line.left() + field(harness).left()) / 2.0).round(), line.center().y)
}

/// The command field's box, as the command line keeps it for the tests.
fn field(harness: &Harness<'_, InboxApp>) -> Rect {
    harness.ctx.data(|data| data.get_temp::<Rect>(crate::command::field_id().with("box"))).unwrap()
}

/// The rect of each window control.
fn controls(harness: &Harness<'_, InboxApp>) -> Vec<Rect> {
    CONTROLS.iter().map(|name| harness.get_by_label(name).rect()).collect()
}

/// Whether a picture (a mesh with a texture of its own) is painted inside `within`.
fn picture_in(harness: &Harness<'_, InboxApp>, within: Rect) -> bool {
    harness.output().shapes.iter().any(|clipped| match &clipped.shape {
        egui::Shape::Mesh(mesh) => mesh.texture_id != egui::TextureId::default() && within.contains_rect(mesh.calc_bounds()),
        _ => false,
    })
}

/// The rect of the picture painted in the command line's left 60 points: the mark.
fn mark_rect(harness: &Harness<'_, InboxApp>) -> Option<Rect> {
    let corner = Rect::from_min_size(pos2(0.0, 0.0), vec2(60.0, 46.0));
    harness.output().shapes.iter().find_map(|clipped| match &clipped.shape {
        egui::Shape::Mesh(mesh) if mesh.texture_id != egui::TextureId::default() && corner.contains_rect(mesh.calc_bounds()) => {
            Some(mesh.calc_bounds())
        }
        _ => None,
    })
}

/// The brand's compact mark, 22 points a side: 16 in from the command line's left beside the full index (the frames
/// at 1440), centred in the narrow index's column beside the narrow one (`size-1100/*`): at 1 + (57 − 22) / 2 = 18.5,
/// so its picture starts at pixel 18 and takes 23 pixels, the half pixel drawn into the picture. In either frame.
#[test]
fn the_mark_stands_16_in_and_centred_in_the_narrow_index_s_column() {
    for own in [false, true] {
        let wide = harness_sized(own, WindowFrame::Own, vec2(1440.0, 900.0), |_| {});
        assert_eq!(mark_rect(&wide), Some(Rect::from_min_size(pos2(17.0, 12.0), vec2(22.0, 22.0))), "{own}: 16 in, at 1440");
        let narrow = harness(own, WindowFrame::Own);
        assert_eq!(mark_rect(&narrow), Some(Rect::from_min_size(pos2(18.0, 12.0), vec2(23.0, 22.0))), "{own}: centred, at 1100");
    }
}

/// At 110 %, 120 %, 125 % and 150 % the mark stands 16 in beside the full index (at 1440) and centred beside the
/// narrow one (at 1100), painted from the whole pixel its box starts in and as wide as its box and that part of a
/// pixel: the full index, rounded to whole pixels, never reads as the narrow one.
#[test]
fn the_mark_stands_on_whole_pixels_at_fractional_scales() {
    for ppp in [1.1_f32, 1.2, 1.25, 1.5] {
        for (width, left) in [(1440.0, 16.0), (1100.0, 17.5)] {
            let app = InboxApp::at_mail(Mailbox::new(sample::sending(clock()).unwrap(), clock()));
            let mut harness =
                Harness::builder().with_size(vec2(width, 900.0)).with_pixels_per_point(ppp).with_context_setup(install_fonts).build_ui_state(
                    |ui, app: &mut InboxApp| {
                        let whole = ui.ctx().content_rect();
                        ui.scope_builder(egui::UiBuilder::new().max_rect(whole), |ui| app.ui(ui));
                    },
                    app,
                );
            harness.run();
            let line = line(&harness);
            let (pixel, shift) = crate::brand::placed(pos2(line.left() + left, line.center().y - 11.0), ppp);
            let wide = (22.0 * ppp + shift.x).ceil();
            let rect = mark_rect(&harness).unwrap();
            let what = format!("{width} at {ppp}");
            assert!((rect.left() * ppp - pixel.x).abs() < 1e-3 && (rect.top() * ppp - pixel.y).abs() < 1e-3, "{what}: {rect:?} from {pixel:?}");
            assert!((rect.width() * ppp - wide).abs() < 1e-3, "{what}: {rect:?} {wide} pixels wide");
        }
    }
}

/// Sets what the window says of itself: maximised, in front.
fn window_state(harness: &mut Harness<'static, InboxApp>, maximised: bool, focused: bool) {
    let viewport = harness.input_mut().viewports.entry(ViewportId::ROOT).or_default();
    viewport.maximized = Some(maximised);
    viewport.focused = Some(focused);
    harness.run();
}

/// `window-frame/own` at 1440 × 900: the mark at 16, 11 of the command line (22 a side), the field from the index's
/// column to 12 before the controls, the three controls 108 × 32 at 4 from the line's right end, centred in its height.
#[test]
fn window_frame_own_stands_where_the_frame_draws_it() {
    let harness = harness_sized(true, WindowFrame::Own, vec2(1440.0, 900.0), |_| {});
    assert!(harness.state().draws_frame());
    let line = line(&harness);
    assert_eq!(line, Rect::from_min_size(pos2(1.0, 1.0), vec2(1438.0, 44.0)), "the frame's command line");
    let rects = controls(&harness);
    let whole = rects.iter().fold(Rect::NOTHING, |all, rect| all.union(*rect));
    assert_eq!(whole, Rect::from_min_size(pos2(1327.0, 7.0), vec2(108.0, 32.0)), "the controls at 1326, 6 of the line");
    assert_eq!(field(&harness), Rect::from_min_size(pos2(234.0, 9.0), vec2(1081.0, 28.0)), "the field at 233, 8 of the line");
    assert!(picture_in(&harness, Rect::from_min_size(pos2(17.0, 12.0), vec2(22.0, 22.0)).expand(0.5)), "the mark at 16, 11");
    // New message is the index's (#339): none in the command line.
    assert!(harness.query_all_by_label("New message").all(|node| node.rect().top() >= line.bottom()), "the command line draws no New message");
    assert!(harness.query_by_label("Restore").is_none(), "a window not maximised offers Maximise");
}

/// `window-frame/own-maximised`: the middle control restores, and pressed it asks the window to restore.
#[test]
fn window_frame_own_maximised_offers_restore() {
    let mut harness = harness_sized(true, WindowFrame::Own, vec2(1440.0, 900.0), |_| {});
    window_state(&mut harness, true, true);
    assert!(harness.query_by_label("Maximise").is_none(), "maximised, the middle control is Restore");
    let restore = harness.get_by_label("Restore").rect();
    assert_eq!(restore.size(), vec2(36.0, 32.0));
    let seen = click_at(&mut harness, restore.center());
    assert!(seen.contains(&ViewportCommand::Maximized(false)), "{seen:?}");
    assert_eq!(harness.state().window_corner(&harness.ctx), 0.0, "a maximised window has square corners");
}

/// `window-frame/own-unfocused`: with another window in front the controls are still drawn, named and pressable.
#[test]
fn window_frame_own_unfocused_keeps_the_controls_working() {
    let mut harness = harness_sized(true, WindowFrame::Own, vec2(1440.0, 900.0), |_| {});
    let focused = controls(&harness);
    window_state(&mut harness, false, false);
    assert_eq!(controls(&harness), focused, "the controls stay where they are");
    let close = harness.get_by_label("Close the window").rect().center();
    assert!(click_at(&mut harness, close).contains(&ViewportCommand::Close));
}

/// `window-frame/system`: the desktop's title bar; the command line holds the mark and the field, which runs to 12
/// before the line's right end, and no window controls.
#[test]
fn window_frame_system_leaves_the_controls_to_the_desktop() {
    for own in [true, false] {
        let harness = harness_sized(own, if own { WindowFrame::System } else { WindowFrame::Own }, vec2(1440.0, 900.0), |_| {});
        assert!(!harness.state().draws_frame(), "{own}");
        for name in CONTROLS {
            assert!(harness.query_by_label(name).is_none(), "{own}: {name}");
        }
        assert_eq!(field(&harness), Rect::from_min_size(pos2(234.0, 9.0), vec2(1193.0, 28.0)), "{own}: the field at 233, 8, 1193 wide");
        assert!(picture_in(&harness, Rect::from_min_size(pos2(17.0, 12.0), vec2(22.0, 22.0)).expand(0.5)), "{own}: the mark");
        assert!(harness.ctx.read_response(super::frame::title_bar_id()).is_none(), "{own}: no title bar of the app's");
    }
}

#[test]
fn the_frame_is_one_of_the_appearance_choices() {
    let mut app = InboxApp::at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    assert_eq!(app.mailbox().appearance().frame, WindowFrame::Own, "the own frame at first");
    app.set_frame(WindowFrame::System);
    assert_eq!(app.mailbox().appearance().frame, WindowFrame::System);
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
        let mut harness = harness(true, WindowFrame::Own);
        let seen = press_at(&mut harness, at);
        assert!(seen.contains(&ViewportCommand::BeginResize(direction)), "{at:?}: {seen:?}");
    }
}

#[test]
fn a_drag_on_the_command_line_where_no_control_stands_moves_the_window() {
    let mut harness = harness(true, WindowFrame::Own);
    let (field, first) = (field(&harness), controls(&harness)[0]);
    // The mark's column, and the gap between the field and the controls.
    for at in [free(&harness), pos2((field.right() + first.left()) / 2.0, line(&harness).center().y)] {
        let mut harness = self::harness(true, WindowFrame::Own);
        let seen = drag_from(&mut harness, at);
        assert!(seen.contains(&ViewportCommand::StartDrag), "{at:?}: {seen:?}");
    }
    // A press in the field types there: it moves nothing.
    let seen = drag_from(&mut harness, field.center());
    assert!(!seen.contains(&ViewportCommand::StartDrag), "the field moved the window: {seen:?}");
}

/// The rects of the window controls and of every other control or field.
fn others(harness: &Harness<'_, InboxApp>) -> Vec<(String, Rect)> {
    let mut others = Vec::new();
    for role in [Role::Button, Role::TextInput, Role::SearchInput, Role::ComboBox] {
        for node in harness.query_all_by_role(role) {
            let name = node.accesskit_node().label().unwrap_or_default();
            if !CONTROLS.contains(&name.as_str()) {
                others.push((name, node.rect()));
            }
        }
    }
    others
}

#[test]
fn the_window_controls_lie_over_no_other_control() {
    for settings in [false, true] {
        let harness = harness_sized(true, WindowFrame::Own, vec2(1100.0, 700.0), |app| {
            if settings {
                app.open_settings();
            }
        });
        for control in controls(&harness) {
            for (name, rect) in others(&harness) {
                assert!(!control.intersects(rect), "settings {settings}: a window control {control:?} lies over {name:?} {rect:?}");
            }
        }
    }
}

#[test]
fn the_window_s_corners_are_round_only_while_the_app_draws_the_frame() {
    assert!(harness(true, WindowFrame::Own).state().window_corner(&egui::Context::default()) > 0.0);
    assert_eq!(harness(true, WindowFrame::System).state().window_corner(&egui::Context::default()), 0.0);
    assert_eq!(harness(false, WindowFrame::Own).state().window_corner(&egui::Context::default()), 0.0);
}

#[test]
fn the_window_moves_again_after_a_move_whose_release_the_desktop_kept() {
    // While the desktop moves the window it holds the pointer: the app never hears the button come up.
    let mut harness = harness(true, WindowFrame::Own);
    let at = free(&harness);
    assert!(drag_from(&mut harness, at).contains(&ViewportCommand::StartDrag), "the first move");
    let seen = drag_from(&mut harness, at + vec2(40.0, 0.0));
    assert!(seen.contains(&ViewportCommand::StartDrag), "the second move {seen:?}");
}

#[test]
fn after_the_desktop_moved_or_resized_the_window_the_button_counts_as_up_again() {
    // The desktop keeps the release; until the app hears the button come up, egui shows no hover anywhere.
    let start = harness(true, WindowFrame::Own);
    for (at, grab) in [(free(&start), ViewportCommand::StartDrag), (pos2(1098.0, 400.0), ViewportCommand::BeginResize(ResizeDirection::East))] {
        let mut harness = harness(true, WindowFrame::Own);
        assert!(drag_from(&mut harness, at).contains(&grab), "{grab:?}");
        assert!(harness.ctx.input(|input| input.pointer.any_down()), "{grab:?}: the release never came");
        let ctx = harness.ctx.clone();
        InboxApp::release_after_window_grab(&ctx, harness.input_mut());
        harness.step();
        assert!(!harness.ctx.input(|input| input.pointer.any_down()), "{grab:?}: the button still counts as down");
        assert!(harness.ctx.dragged_id().is_none(), "{grab:?}: a drag still goes on");
    }
}

#[test]
fn without_a_grab_by_the_desktop_no_release_is_added() {
    let mut harness = harness(true, WindowFrame::Own);
    let ctx = harness.ctx.clone();
    InboxApp::release_after_window_grab(&ctx, harness.input_mut());
    assert!(harness.input_mut().events.is_empty());
}

#[test]
fn the_index_starts_under_the_command_line_in_every_frame() {
    for frame in [WindowFrame::Own, WindowFrame::System] {
        let harness = harness(true, frame);
        let laid = super::shell::laid(&harness.ctx).unwrap();
        assert!(laid.index.top() > laid.command.bottom(), "{frame:?}: {laid:?}");
        // New message 12 down, HOME under its 40 high row (#339).
        let new = harness.get_by_role_and_label(Role::Button, "New message").rect();
        assert!((new.top() - (laid.index.top() + 12.0)).abs() <= 0.5, "{frame:?}: New message at {}, the index at {}", new.top(), laid.index.top());
        let home = harness.get_by_role_and_label(Role::Button, "Home").rect();
        assert!((home.top() - (laid.index.top() + 52.0)).abs() <= 0.5, "{frame:?}: HOME at {}, the index at {}", home.top(), laid.index.top());
    }
}

#[test]
fn on_the_settings_the_window_controls_stand_clear_and_do_what_they_do() {
    let mut harness = harness_sized(true, WindowFrame::Own, vec2(1100.0, 700.0), InboxApp::open_settings);
    let sheet = harness.get_by_role_and_label(Role::Pane, "Settings").rect();
    for (name, control) in CONTROLS.iter().zip(controls(&harness)) {
        assert!(!control.intersects(sheet), "{name} {control:?} lies over the settings {sheet:?}");
    }
    // A press on Minimise minimises the window and leaves the settings shown.
    let at = harness.get_by_label("Minimise").rect().center();
    let seen = click_at(&mut harness, at);
    harness.run();
    assert!(seen.contains(&ViewportCommand::Minimized(true)), "{seen:?}");
    assert!(harness.state().settings().is_open(), "the press on a window control left the settings");
}

/// The label of the control with the keyboard.
fn focused(harness: &Harness<'_, InboxApp>) -> Option<String> {
    harness.root().children_recursive().find(|node| node.accesskit_node().is_focused()).and_then(|node| node.accesskit_node().label())
}

#[test]
fn tab_walks_the_command_line_then_the_window_controls_then_the_index() {
    let mut harness = harness(true, WindowFrame::Own);
    let mut walked = Vec::new();
    for _ in 0..5 {
        harness.key_press(Key::Tab);
        harness.run();
        walked.push(focused(&harness).unwrap_or_default());
    }
    assert_eq!(walked, ["Command line", "Minimise", "Maximise", "Close the window", "New message"], "the drawn order");
    harness.key_press_modifiers(Modifiers::SHIFT, Key::Tab);
    harness.run();
    assert_eq!(focused(&harness).as_deref(), Some("Close the window"), "Shift Tab walks it back");
    // Enter and Space on a control press it (the frame of the key going down, whose output the harness keeps alone).
    for key in [Key::Enter, Key::Space] {
        harness.event(Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE });
        harness.step();
        let seen = commands(&harness);
        harness.event(Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers: Modifiers::NONE });
        harness.step();
        assert!(seen.contains(&ViewportCommand::Close), "{key:?} on Close closes: {seen:?}");
    }
}

#[test]
fn on_the_settings_tab_walks_the_window_s_controls_and_the_settings_alike() {
    let mut harness = harness_sized(true, WindowFrame::Own, vec2(1100.0, 700.0), InboxApp::open_settings);
    let settings = harness.get_by_role_and_label(Role::Pane, "Settings").rect();
    let (mut controls, mut on_settings) = (false, 0);
    for _ in 0..200 {
        harness.key_press(Key::Tab);
        harness.run();
        let Some(node) = harness.root().children_recursive().find(|node| node.accesskit_node().is_focused()) else { continue };
        let label = node.accesskit_node().label().unwrap_or_default();
        controls |= label == "Minimise";
        on_settings += usize::from(settings.contains_rect(node.rect()));
    }
    assert!(controls, "the settings are a place: Tab reaches Minimise beside them");
    assert!(on_settings > 20, "Tab reaches the settings' controls ({on_settings})");
    assert!(harness.state().settings().is_open(), "Tab leaves no place");
}

/// The title bar moves the window by the pointer only: a Tab walk round the whole window never lands on it, an unnamed
/// stop that does nothing.
#[test]
fn the_title_bar_is_no_tab_stop() {
    let mut harness = harness(true, WindowFrame::Own);
    let handle = super::frame::title_bar_id();
    assert!(harness.ctx.read_response(handle).is_some(), "the handle is drawn");
    for step in 0..200 {
        harness.key_press(Key::Tab);
        harness.step();
        harness.step();
        assert_ne!(harness.ctx.memory(|memory| memory.focused()), Some(handle), "Tab {step} lands on the handle");
    }
}

#[test]
fn a_double_press_on_the_command_line_maximises_the_window_and_restores_it() {
    for maximised in [false, true] {
        let mut harness = harness_with(true, WindowFrame::Own, vec2(1100.0, 700.0), Some(1.0 / 60.0), |_| {});
        if maximised {
            window_state(&mut harness, true, true);
        }
        let at = free(&harness);
        harness.hover_at(at);
        harness.step();
        let mut seen = Vec::new();
        // Two presses a sixtieth of a second apart: well inside the double press's time.
        for _ in 0..2 {
            for pressed in [true, false] {
                harness.event(Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
            }
            harness.step();
            seen.extend(commands(&harness));
        }
        assert!(seen.contains(&ViewportCommand::Maximized(!maximised)), "maximised {maximised}: {seen:?}");
    }
}

/// Close closes whatever stands in front: the command line's results, a menu, the event editor. Never under a gate
/// that would keep the owner in the window, by the pointer or by a screen reader's activate.
#[test]
fn close_closes_over_the_command_line_s_results_a_menu_and_the_event_editor() {
    for surface in ["results", "menu", "editor"] {
        for activate in [false, true] {
            let mut harness = harness(true, WindowFrame::Own);
            let ctx = harness.ctx.clone();
            match surface {
                "results" => {
                    harness.state_mut().run_action(&ctx, keymap::PALETTE);
                    harness.run();
                    harness.state_mut().command_mut().set_text(">zz");
                    harness.run();
                    assert!(harness.state().command().results(), "the results show");
                }
                "menu" => {
                    let picker = harness
                        .query_all_by_role(Role::Button)
                        .find(|node| node.accesskit_node().label().is_some_and(|name| name.starts_with("Show mail of")))
                        .unwrap();
                    picker.click();
                    harness.run();
                    assert!(harness.query_by_label("All accounts").is_some(), "the menu shows");
                }
                _ => {
                    harness.state_mut().run_action(&ctx, keymap::CALENDAR);
                    harness.run();
                    harness.get_by_role_and_label(Role::Button, "New event").click();
                    harness.run();
                    assert!(harness.state().calendar().editor().is_some(), "the editor shows");
                }
            }
            let seen = if activate {
                harness.get_by_label("Close the window").click_accesskit();
                harness.step();
                commands(&harness)
            } else {
                let at = harness.get_by_label("Close the window").rect().center();
                click_at(&mut harness, at)
            };
            assert!(seen.contains(&ViewportCommand::Close), "{surface}, activate {activate}: {seen:?}");
        }
    }
}

/// Opens the command line's results (commands, none found).
fn open_results(harness: &mut Harness<'static, InboxApp>) {
    let ctx = harness.ctx.clone();
    harness.state_mut().run_action(&ctx, keymap::PALETTE);
    harness.run();
    harness.state_mut().command_mut().set_text(">zz");
    harness.run();
    assert!(harness.state().command().results(), "the results show");
}

/// The title bar's free points: the mark's column at the line's middle and under the field, and the gap before the
/// controls.
fn free_points(harness: &Harness<'_, InboxApp>) -> [Pos2; 3] {
    let (line, field, first) = (line(harness), field(harness), controls(harness)[0]);
    let mark = free(harness);
    [mark, pos2(mark.x, ((field.bottom() + line.bottom()) / 2.0).round()), pos2((field.right() + first.left()) / 2.0, line.center().y)]
}

#[test]
fn under_the_command_line_s_results_the_title_bar_stays_the_title_bar() {
    // #412 (#305 m4): the frames' scrim under the results starts at the command line's foot, so the whole title bar
    // stays one, the mark's column under the field too: a drag there moves the window and the results stay.
    let at = free_points(&self::harness(true, WindowFrame::Own));
    for at in at {
        let mut harness = self::harness(true, WindowFrame::Own);
        open_results(&mut harness);
        let seen = drag_from(&mut harness, at);
        assert!(seen.contains(&ViewportCommand::StartDrag), "{at:?}: {seen:?}");
        assert!(harness.state().command().results(), "{at:?}: the results stay");
    }
    // The scrim (the command line's dim ground) starts at the line's foot, as the frames draw it (the window's y 45),
    // not at the field's (37): a press on the title bar under the field is the title bar's, and the results stay.
    let mut harness = self::harness(true, WindowFrame::Own);
    open_results(&mut harness);
    let dim = harness.ctx.memory(|memory| memory.area_rect(egui::Id::unique("command-line-dim"))).unwrap();
    assert_eq!(dim.top(), line(&harness).bottom(), "the scrim {dim:?} under the line");
    let [_, under_field, _] = free_points(&harness);
    click_at(&mut harness, under_field);
    harness.run();
    assert!(harness.state().command().results(), "a press on the title bar under the field keeps the results");
    // Under the line a press lands on the scrim: it closes the results and moves nothing.
    let under = pos2(free(&harness).x, line(&harness).bottom() + 20.0);
    let seen = click_at(&mut harness, under);
    assert!(!seen.contains(&ViewportCommand::StartDrag), "{seen:?}");
    assert!(!harness.state().command().results(), "a press under the line closes the results");
}
