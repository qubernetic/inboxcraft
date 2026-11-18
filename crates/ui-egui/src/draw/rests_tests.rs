// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The mail list's keys from the lists slice beside the screens' surfaces: under the settings sheet and while the list
//! is hidden they rest (by key and from the palette), the arrow steps the list once, and one Esc closes one surface.

use egui::accesskit::Role;
use egui::{Key, KeyboardShortcut, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_time::{FakeClock, Timestamp};

use crate::component::popup;
use crate::keymap;
use crate::mailbox::{InboxView, Mailbox};
use crate::{InboxApp, sample};

/// 2026-11-05T09:00:00Z.
const NOW: i64 = 1_793_869_200;

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(NOW, 0).unwrap()))
}

/// The sample mail in a window `width` wide.
fn window(width: f32) -> Harness<'static, InboxApp> {
    let mut app = InboxApp::new(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    app.set_reduce_motion(true);
    let mut harness = Harness::builder()
        .with_size(vec2(width, 900.0))
        .with_max_steps(40)
        .with_context_setup(crate::theme::install_fonts)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

/// The ids of the lists slice's keys: the list's, Back and the inbox views.
fn epic_keys() -> Vec<&'static str> {
    keymap::LIST_KEYS.into_iter().chain([keymap::BACK, keymap::VIEW_GROUPED, keymap::VIEW_SMART, keymap::VIEW_LIST]).collect()
}

/// Gives Back and the inbox views keys of their own (they have none by default), with Ctrl and without.
fn bind_views_and_back(harness: &mut Harness<'static, InboxApp>) {
    let keymap = harness.state_mut().keymap_mut();
    keymap.bind(keymap::BACK, KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, Key::B));
    keymap.bind(keymap::VIEW_LIST, KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, Key::L));
    keymap.bind(keymap::VIEW_GROUPED, KeyboardShortcut::new(Modifiers::NONE, Key::G));
}

/// Every key of the list and the views, and Back's when `back`, pressed once each.
fn press_every_epic_key(harness: &mut Harness<'static, InboxApp>, back: bool) {
    let bare = [Key::J, Key::K, Key::ArrowDown, Key::ArrowUp, Key::PageDown, Key::PageUp, Key::Home, Key::End, Key::Enter, Key::Menu, Key::G];
    for key in bare {
        harness.key_press(key);
        harness.run();
    }
    harness.key_press_modifiers(Modifiers::SHIFT, Key::F10);
    if back {
        harness.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::B);
    }
    harness.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::L);
    harness.run();
}

/// The list has the keyboard on its first row: keyed, the row focused.
fn keyed_first_row(harness: &mut Harness<'static, InboxApp>) -> u64 {
    harness.key_press(Key::J);
    harness.run();
    let keyed = harness.state().mailbox().selected().unwrap();
    assert!(super::list::focused_row(&harness.ctx).is_some(), "the list has the keyboard");
    keyed
}

/// What the keys could change: the selection, the inbox view, a float open, whether the reader is drawn.
fn what_keys_change(harness: &Harness<'static, InboxApp>) -> (Option<u64>, InboxView, bool, bool) {
    let state = harness.state();
    (state.mailbox().selected(), state.mailbox().inbox_view(), popup::any_open(&harness.ctx), super::reader_rect(&harness.ctx).is_some())
}

#[test]
fn under_the_settings_sheet_no_key_of_the_list_back_or_the_views_runs() {
    // Two panes, the list keyed; and one pane with a message in front, where Back would run.
    for width in [1440.0, 900.0] {
        let mut harness = window(width);
        bind_views_and_back(&mut harness);
        let keyed = keyed_first_row(&mut harness);
        if width < 980.0 {
            harness.key_press(Key::Enter);
            harness.run();
            assert!(super::reader_rect(&harness.ctx).is_some(), "{width}: the message in front");
        }
        let before = what_keys_change(&harness);
        harness.key_press_modifiers(Modifiers::COMMAND, Key::Comma);
        harness.run();
        assert!(harness.state().settings().is_open());
        for id in epic_keys().into_iter().filter(|id| *id != keymap::VIEW_SMART) {
            assert!(harness.state().keymap().shortcut(id).is_some(), "{id} has a key");
        }
        press_every_epic_key(&mut harness, true);
        assert!(harness.state().settings().is_open(), "{width}: the sheet stays");
        // Esc on the sheet closes only the sheet: the window under it is as it was.
        harness.key_press(Key::Escape);
        harness.run();
        assert!(!harness.state().settings().is_open());
        assert_eq!(what_keys_change(&harness), before, "{width}: nothing under the sheet moved");
        assert_eq!(harness.state().mailbox().selected(), Some(keyed));
        if width < 980.0 {
            assert!(super::reader_rect(&harness.ctx).is_some(), "{width}: Back did not run under the sheet");
        }
    }
}

/// A row picked and flagged, so its acts and Undo could run, then New message: the window `width` wide hides the list
/// (one pane at 720, beside the composer at 1000).
fn hidden_list(width: f32) -> Harness<'static, InboxApp> {
    let mut harness = window(width);
    bind_views_and_back(&mut harness);
    harness.query_all_by_label_contains("Draft logo for the bakery").next().unwrap().click();
    harness.run();
    if width < 980.0 {
        // One pane: back to the list, the row picked.
        harness.key_press(Key::Escape);
        harness.run();
    }
    let ctx = harness.ctx.clone();
    harness.state_mut().run_action(&ctx, keymap::FLAG);
    harness.run();
    for id in [keymap::NEXT, keymap::DOWN, keymap::VIEW_LIST, keymap::DONE, keymap::UNDO] {
        assert!(harness.state().can_run(id), "{width}: {id} before");
    }
    harness.get_by_role_and_label(Role::Button, "New message").click();
    harness.run();
    harness.run();
    assert!(harness.state().list_hidden, "{width}: the list hidden");
    harness
}

#[test]
fn with_the_list_hidden_its_keys_views_and_row_commands_rest_and_the_palette_shows_them_unavailable() {
    for width in [720.0, 1000.0] {
        let mut harness = hidden_list(width);
        let rows = [keymap::DONE, keymap::ARCHIVE, keymap::READ, keymap::FLAG, keymap::DELETE, keymap::UNDO];
        // In one pane Back is the way the list comes back in front of the composer (the lists slice's own rule); in two
        // panes it has nothing to do.
        let one_pane = width < 980.0;
        for id in epic_keys().into_iter().filter(|id| !one_pane || *id != keymap::BACK).chain(rows) {
            assert!(!harness.state().can_run(id), "{width}: {id} with the list hidden");
        }
        // By the keys, from outside the composer's fields: nothing moves.
        harness.ctx.memory_mut(|memory| memory.stop_text_input());
        harness.run();
        let before = what_keys_change(&harness);
        press_every_epic_key(&mut harness, !one_pane);
        assert_eq!(what_keys_change(&harness), before, "{width}: no key of the hidden list ran");
        assert!(harness.state().composer().is_some_and(|composer| !composer.floating()), "{width}: the composer stays");
        // The palette lists them, unavailable.
        harness.key_press_modifiers(Modifiers::COMMAND, Key::K);
        harness.run();
        assert!(harness.state().palette().is_open());
        for (typed, name) in [
            ("Inbox view", "Inbox view: List"),
            ("Inbox view", "Inbox view: Grouped"),
            ("next message", "The next message of the list"),
            ("group menu", "The group menu of the list's message"),
            ("Done", "Done"),
        ] {
            harness.state_mut().palette_mut().set_query(typed);
            harness.run();
            let item = harness.get_by_role_and_label(Role::MenuItem, name);
            assert!(item.accesskit_node().is_disabled(), "{width}: {name} is offered as unavailable");
        }
    }
}

#[test]
fn the_arrow_moves_the_list_one_row_once() {
    let mut harness = window(1440.0);
    let keyed = keyed_first_row(&mut harness);
    let order = super::list::order(&harness.ctx);
    let at = order.iter().position(|id| *id == keyed).unwrap();
    for (press, step) in [(1, 1), (2, 2), (3, 3)] {
        harness.key_press(Key::ArrowDown);
        harness.run();
        let now = harness.state().mailbox().selected().unwrap();
        assert_eq!(order.get(at + step).copied(), Some(now), "press {press}: one row on, not two");
    }
    harness.key_press(Key::ArrowUp);
    harness.run();
    assert_eq!(harness.state().mailbox().selected(), order.get(at + 2).copied(), "up: one row back");
    // A row pressed, the keyboard the list's with no row focused: one row on too.
    let mut harness = window(1440.0);
    harness.query_all_by_label_contains("Draft logo for the bakery").next().unwrap().click();
    harness.run();
    let pressed = harness.state().mailbox().selected().unwrap();
    let order = super::list::order(&harness.ctx);
    let at = order.iter().position(|id| *id == pressed).unwrap();
    harness.key_press(Key::ArrowDown);
    harness.run();
    assert_eq!(harness.state().mailbox().selected(), order.get(at + 1).copied(), "from a pressed row: one row on, not two");
}

/// The sheet open on the add-account form with its IMAP security list open: a list float over the sheet.
fn float_over_the_sheet() -> Harness<'static, InboxApp> {
    let mut harness = window(1440.0);
    harness.state_mut().onboarding_mut().0.open();
    harness.run();
    assert!(harness.state().settings().is_open());
    harness.get_by_label("IMAP security").scroll_to_me();
    harness.run();
    harness.get_by_label("IMAP security").click();
    harness.run();
    assert!(harness.query_by_role_and_label(Role::MenuItemRadio, "STARTTLS").is_some(), "the security list is open");
    harness
}

#[test]
fn one_esc_closes_one_surface_a_list_over_the_sheet_the_sheet_over_a_list_and_the_palette_over_both() {
    // A list float over the sheet: the first Esc closes the list, the second the sheet.
    let mut harness = float_over_the_sheet();
    harness.key_press(Key::Escape);
    harness.run();
    assert!(harness.query_by_role_and_label(Role::MenuItemRadio, "STARTTLS").is_none(), "the list closed");
    assert!(harness.state().settings().is_open(), "the sheet stays under it");
    harness.key_press(Key::Escape);
    harness.run();
    assert!(!harness.state().settings().is_open(), "the second Esc closes the sheet");

    // The sheet over a list float (a row's group menu): the sheet closed the menu as it came; one Esc closes the
    // sheet, and the menu does not come back.
    let mut harness = window(1440.0);
    let keyed = keyed_first_row(&mut harness);
    harness.key_press_modifiers(Modifiers::SHIFT, Key::F10);
    harness.run();
    let menu = super::list::group_menu_id(keyed);
    assert!(popup::is_open(&harness.ctx, menu), "the row's group menu is open");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Comma);
    harness.run();
    assert!(harness.state().settings().is_open() && !popup::is_open(&harness.ctx, menu), "the sheet over it, the menu closed");
    harness.key_press(Key::Escape);
    harness.run();
    assert!(!harness.state().settings().is_open(), "one Esc: the sheet");
    assert!(!popup::is_open(&harness.ctx, menu) && !popup::any_open(&harness.ctx), "the menu does not come back");
    assert_eq!(harness.state().mailbox().selected(), Some(keyed));

    // The palette over both: it comes over the list and the sheet and closes them; one Esc closes the palette alone.
    let mut harness = float_over_the_sheet();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::K);
    harness.run();
    assert!(harness.state().palette().is_open());
    harness.key_press(Key::Escape);
    harness.run();
    assert!(!harness.state().palette().is_open(), "one Esc: the palette");
    assert!(harness.query_by_role_and_label(Role::MenuItemRadio, "STARTTLS").is_none() && !popup::any_open(&harness.ctx), "no list");
    assert!(!harness.state().settings().is_open(), "the palette closed the sheet as it came, as before");
}

/// Drags the thumb of the bar over the content by `down` points, from its middle.
fn drag_thumb(harness: &mut Harness<'static, InboxApp>, down: f32) {
    let from = crate::component::scroll::over_thumb(&harness.ctx).unwrap().center();
    harness.hover_at(from);
    harness.run();
    harness.drag_at(from);
    harness.step();
    for step in 1..=4 {
        harness.hover_at(from + vec2(0.0, down * step as f32 / 4.0));
        harness.step();
    }
    harness.drop_at(from + vec2(0.0, down));
    harness.run();
}

/// A press, down and up, at `at`.
fn press_at(harness: &mut Harness<'static, InboxApp>, at: egui::Pos2) {
    harness.event(egui::Event::PointerMoved(at));
    harness.step();
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
        harness.step();
    }
    harness.run();
}

/// Where in the bar's strip (from `top` down) a press lands on `content` (a row, an event), off the thumb: the strip's
/// middle, at the content's middle, which the thumb does not cover.
fn beside_the_thumb(harness: &Harness<'static, InboxApp>, top: f32, content: &[egui::Rect]) -> Option<(egui::Rect, egui::Pos2)> {
    let thumb = crate::component::scroll::over_thumb(&harness.ctx)?;
    let bottom = harness.ctx.content_rect().bottom();
    content
        .iter()
        .copied()
        .filter(|rect| rect.right() >= thumb.right() - 0.5 && !thumb.y_range().contains(rect.center().y) && rect.height() > 8.0)
        .find(|rect| rect.center().y > top + 4.0 && rect.center().y < bottom - 4.0)
        .map(|rect| (rect, egui::pos2(thumb.center().x, rect.center().y)))
}

#[test]
fn the_one_bar_over_the_content_drags_by_its_thumb_and_gives_a_press_beside_it_to_the_content() {
    // A mail list, short enough to scroll.
    let mut app = InboxApp::new(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    app.set_reduce_motion(true);
    let mut harness = Harness::builder()
        .with_size(vec2(1440.0, 420.0))
        .with_max_steps(40)
        .with_context_setup(crate::theme::install_fonts)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    let rows = |harness: &Harness<'static, InboxApp>| -> Vec<(egui::Rect, String)> {
        let view = super::list::view_rect(&harness.ctx).unwrap();
        let rows = harness.query_all_by_role(Role::ListBoxOption).map(|node| (node.rect(), node.accesskit_node().label().unwrap_or_default()));
        rows.filter(|(rect, _)| view.contains(rect.center())).collect()
    };
    let (top, first) = rows(&harness).first().map(|(rect, label)| (rect.top(), label.clone())).unwrap();
    // The thumb at the top of its strip: where the strip begins.
    let strip = crate::component::scroll::over_thumb(&harness.ctx).unwrap().top();
    drag_thumb(&mut harness, 60.0);
    let moved = harness.query_all_by_role(Role::ListBoxOption).find(|node| node.accesskit_node().label().as_deref() == Some(first.as_str()));
    assert!(moved.is_some_and(|row| row.rect().top() < top - 40.0), "the list: the thumb's drag scrolls it");
    let rects: Vec<egui::Rect> = rows(&harness).into_iter().map(|(rect, _)| rect).collect();
    let (row, at) = beside_the_thumb(&harness, strip, &rects).unwrap();
    let label = rows(&harness).into_iter().find(|(rect, _)| *rect == row).map(|(_, label)| label);
    let selected = harness.state().mailbox().selected();
    press_at(&mut harness, at);
    assert_ne!(harness.state().mailbox().selected(), selected, "the list: a press in the strip beside the thumb picks {label:?}");

    // The calendar's agenda.
    let mut app = InboxApp::new(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    app.set_reduce_motion(true);
    app.show_calendar();
    let (calendar, mailbox) = app.calendar_mut();
    calendar.set_view(mailbox, crate::calendar::View::Agenda);
    let mut harness = Harness::builder()
        .with_size(vec2(1440.0, 600.0))
        .with_max_steps(40)
        .with_context_setup(crate::theme::install_fonts)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    let events = |harness: &Harness<'static, InboxApp>| -> Vec<egui::Rect> {
        let thumb = crate::component::scroll::over_thumb(&harness.ctx).unwrap();
        let named: Vec<String> = harness.state().calendar().events().map(|event| event.summary.clone()).collect();
        let event = |label: Option<String>| label.is_some_and(|label| named.iter().any(|name| label.contains(name.as_str())));
        let rows = harness.query_all_by_role(Role::Button).filter(|node| event(node.accesskit_node().label())).map(|node| node.rect());
        rows.filter(|rect| rect.right() >= thumb.right() - 0.5 && rect.left() < thumb.left() - 200.0).collect()
    };
    let before = events(&harness);
    let strip = crate::component::scroll::over_thumb(&harness.ctx).unwrap().top();
    drag_thumb(&mut harness, 60.0);
    assert_ne!(events(&harness).first().map(|rect| rect.top()), before.first().map(|rect| rect.top()), "the agenda: the thumb's drag scrolls it");
    let (_, at) = beside_the_thumb(&harness, strip, &events(&harness)).unwrap();
    assert!(harness.state().calendar().picked().is_none());
    press_at(&mut harness, at);
    assert!(harness.state().calendar().picked().is_some(), "the agenda: a press in the strip beside the thumb picks the event");
}
