// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The Shortcuts page of the settings sheet: every action with its key, a new key from the next press, the keys
//! kept between runs; and the sheet's one open state, forgotten by every way out.

use egui::accesskit::Role;
use egui::{Key, KeyboardShortcut, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_model::Role as FolderRole;
use inboxcraft_time::{FakeClock, Timestamp};

use super::InboxApp;
use crate::keep::{FILE, Keep, KeepFile};
use crate::keymap::{self, Keymap};
use crate::mailbox::{Mailbox, Place};
use crate::sample;
use crate::settings::Section;

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

fn build(app: InboxApp, theme: egui::Theme) -> Harness<'static, InboxApp> {
    let mut harness = Harness::builder()
        .with_size(vec2(1200.0, 820.0))
        .with_theme(theme)
        .with_max_steps(12)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

fn app() -> InboxApp {
    InboxApp::new(Mailbox::new(sample::session(clock()).unwrap(), clock()))
}

/// The sheet open on Shortcuts, through its section in the nav.
fn on_shortcuts(theme: egui::Theme) -> Harness<'static, InboxApp> {
    let mut harness = build(app(), theme);
    harness.get_by_label("Appearance settings").click();
    harness.run();
    harness.get_by_label("Shortcuts, settings page").click();
    harness.run();
    assert_eq!(harness.state().settings().section(), Section::Shortcuts);
    harness
}

/// Presses the key chip of the action `name` as a screen reader does; it may lie below the fold.
fn press_chip(harness: &mut Harness<'static, InboxApp>, starts: &str) {
    let chip = harness.query_all_by_role(Role::Button).find(|node| node.accesskit_node().label().is_some_and(|label| label.starts_with(starts)));
    chip.unwrap_or_else(|| panic!("no chip starting {starts:?}")).click_accesskit();
    harness.run();
}

fn press_beside_the_sheet(harness: &mut Harness<'static, InboxApp>) {
    let sheet = harness.get_by_role_and_label(Role::Dialog, "Settings").rect();
    let at = egui::pos2(sheet.left() / 2.0, sheet.center().y);
    harness.hover_at(at);
    harness.run();
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
        harness.run();
    }
}

#[test]
fn the_shortcuts_page_lists_every_action_with_its_key() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let harness = on_shortcuts(theme);
        assert!(!harness.get_by_label("Shortcuts, settings page").accesskit_node().is_disabled());
        harness.get_by_label("Key of Command palette: Ctrl K");
        harness.get_by_label("Key of Search mail: Ctrl F");
        harness.get_by_label("Show sent mail has no key");
        // The list's keys, each with its own: J and K, the arrows, the page keys, Enter, Shift F10 and the Menu key.
        let keymap = crate::keymap::Keymap::default();
        for id in crate::keymap::LIST_KEYS {
            let action = crate::keymap::action(id).unwrap();
            let words = crate::component::key::key_words(&keymap.shortcut(id).unwrap());
            harness.get_by_label(&format!("Key of {}: {words}", action.name));
        }
        // The inbox's three views, actions without a key of their own.
        for name in ["Inbox view: Grouped", "Inbox view: Smart", "Inbox view: List", "Back to the list"] {
            harness.get_by_label(&format!("{name} has no key"));
        }
        harness.get_by_label("Key of The next message of the list: J");
        harness.get_by_label("Key of The message before in the list: K");
        harness.get_by_role_and_label(Role::TextInput, "Find a command");
        harness.get_by_role_and_label(Role::Button, "Reset all keys");
        harness.get_by_role_and_label(Role::Button, "Remove the key of Command palette");
        harness.check_accessibility();
    }
}

#[test]
fn a_press_on_a_key_waits_for_the_next_key_which_becomes_the_action_s() {
    let mut harness = on_shortcuts(egui::Theme::Light);
    press_chip(&mut harness, "Show sent mail has no key");
    assert_eq!(harness.state().settings().capturing(), Some(keymap::SENT));
    harness.get_by_label("Show sent mail: press a key, or Escape to keep the old one");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Num3);
    harness.run();
    assert_eq!(harness.state().settings().capturing(), None);
    assert_eq!(harness.state().keymap().shortcut(keymap::SENT), Some(KeyboardShortcut::new(Modifiers::COMMAND, Key::Num3)));
    harness.get_by_label("Key of Show sent mail: Ctrl 3");
    assert!(harness.state().settings().is_open(), "the press only gave the key");
    // The new key works at once.
    harness.state_mut().close_settings();
    harness.run();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Num3);
    harness.run();
    assert_eq!(harness.state().mailbox().place(), Place::View(FolderRole::Sent));
}

#[test]
fn a_key_another_action_had_moves_and_the_page_says_so() {
    let mut harness = on_shortcuts(egui::Theme::Light);
    press_chip(&mut harness, "Show sent mail has no key");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::K);
    harness.run();
    assert_eq!(harness.state().keymap().shortcut(keymap::SENT), Some(KeyboardShortcut::new(Modifiers::COMMAND, Key::K)));
    assert_eq!(harness.state().keymap().shortcut(keymap::PALETTE), None, "one key runs one action");
    assert!(!harness.state().palette().is_open(), "the press gave a key and opened nothing");
    harness.get_by_label("Ctrl K was the key of Command palette; it has none now.");
    harness.get_by_label("Command palette has no key");
    // Giving an action its own key again says nothing.
    press_chip(&mut harness, "Key of Show sent mail");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::K);
    harness.run();
    assert_eq!(harness.state().settings().keys_note(), None);
}

#[test]
fn escape_while_waiting_keeps_the_old_key_and_the_sheet() {
    let mut harness = on_shortcuts(egui::Theme::Light);
    press_chip(&mut harness, "Key of Command palette");
    harness.key_press(Key::Escape);
    harness.run();
    assert_eq!(harness.state().settings().capturing(), None);
    assert!(harness.state().settings().is_open(), "Escape stops the wait only");
    assert_eq!(harness.state().keymap(), &Keymap::default());
}

#[test]
fn remove_takes_a_key_away_and_reset_gives_every_action_its_first_key() {
    let mut harness = on_shortcuts(egui::Theme::Light);
    harness.get_by_role_and_label(Role::Button, "Remove the key of Command palette").click();
    harness.run();
    assert_eq!(harness.state().keymap().shortcut(keymap::PALETTE), None);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::K);
    harness.run();
    assert!(!harness.state().palette().is_open(), "a key taken away runs nothing");
    harness.get_by_role_and_label(Role::Button, "Reset all keys").click();
    harness.run();
    assert_eq!(harness.state().keymap(), &Keymap::default());
}

#[test]
fn the_field_narrows_the_list() {
    let mut harness = on_shortcuts(egui::Theme::Light);
    harness.get_by_role_and_label(Role::TextInput, "Find a command").click();
    harness.run();
    harness.get_by_role_and_label(Role::TextInput, "Find a command").type_text("sent");
    harness.run();
    assert_eq!(harness.state().settings().find(), "sent");
    harness.get_by_label("Show sent mail has no key");
    assert!(harness.query_by_label("Key of Command palette: Ctrl K").is_none());
    harness.get_by_role_and_label(Role::TextInput, "Find a command").type_text(" zzz");
    harness.run();
    harness.get_by_label("No command matches “sent zzz”.");
}

#[test]
fn a_key_without_ctrl_or_alt_types_into_a_field_instead_of_running() {
    let mut harness = on_shortcuts(egui::Theme::Light);
    press_chip(&mut harness, "Key of Show the calendar");
    harness.key_press(Key::G);
    harness.run();
    assert_eq!(harness.state().keymap().shortcut(keymap::CALENDAR), Some(KeyboardShortcut::new(Modifiers::NONE, Key::G)));
    harness.key_press(Key::G);
    harness.run();
    assert!(harness.state().settings().is_open(), "G runs nothing while the sheet is open");
    harness.state_mut().close_settings();
    harness.run();
    harness.state_mut().search("");
    harness.ctx.memory_mut(|memory| memory.request_focus(super::list::search_id()));
    harness.run();
    harness.key_press(Key::G);
    harness.run();
    assert_eq!(harness.state().screen(), super::Screen::Mail, "G in the search field is typed");
    harness.ctx.memory_mut(|memory| memory.surrender_focus(super::list::search_id()));
    harness.run();
    harness.key_press(Key::G);
    harness.run();
    assert_eq!(harness.state().screen(), super::Screen::Calendar, "G shows the calendar where nothing is typed");
}

#[test]
fn the_keys_are_kept_between_runs() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(FILE);
    let mut harness = build(app().with_keep(Box::new(KeepFile::new(path.clone()))), egui::Theme::Light);
    harness.state_mut().open_settings();
    harness.run();
    harness.get_by_label("Shortcuts, settings page").click();
    harness.run();
    press_chip(&mut harness, "Show sent mail has no key");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Num3);
    harness.run();
    let kept = KeepFile::new(path.clone()).load_keys().unwrap();
    assert_eq!(kept.shortcut(keymap::SENT), Some(KeyboardShortcut::new(Modifiers::COMMAND, Key::Num3)));
    let again = app().with_keep(Box::new(KeepFile::new(path)));
    assert_eq!(again.keymap(), &kept, "the next run starts with them");
}

#[test]
fn keys_that_cannot_be_kept_show_in_the_error_banner() {
    let dir = tempfile::tempdir().unwrap();
    let blocker = dir.path().join("blocker");
    std::fs::write(&blocker, b"a file where the folder should be").unwrap();
    let mut harness = build(app().with_keep(Box::new(KeepFile::new(blocker.join(FILE)))), egui::Theme::Light);
    harness.state_mut().keymap_mut().unbind(keymap::PALETTE);
    harness.run();
    assert!(harness.state().mailbox().error().is_some_and(|error| error.contains("blocker")), "{:?}", harness.state().mailbox().error());
}

/// The sheet on Shortcuts in the state an open sheet can hold: a text in the field and an action waiting for a key.
fn busy_sheet() -> Harness<'static, InboxApp> {
    let mut harness = on_shortcuts(egui::Theme::Light);
    harness.state_mut().settings_mut().find_mut().push_str("show");
    harness.run();
    press_chip(&mut harness, "Key of Show the calendar");
    assert_eq!(harness.state().settings().capturing(), Some(keymap::CALENDAR));
    harness
}

fn assert_closed_clean(harness: &mut Harness<'static, InboxApp>) {
    let settings = harness.state().settings();
    assert!(!settings.is_open());
    assert_eq!((settings.capturing(), settings.find(), settings.keys_note()), (None, "", None), "the open state is forgotten");
    assert_eq!(harness.state().keymap(), &Keymap::default(), "no key was given on the way out");
    assert!(harness.query_by_role_and_label(Role::Dialog, "Settings").is_none());
    // The next opening is clean.
    harness.state_mut().open_settings();
    harness.run();
    assert_eq!(harness.state().settings().section(), Section::Appearance);
    assert_eq!(harness.state().settings().capturing(), None);
}

#[test]
fn the_close_button_forgets_the_open_state() {
    let mut harness = busy_sheet();
    harness.get_by_role_and_label(Role::Button, "Close").click();
    harness.run();
    assert_closed_clean(&mut harness);
}

#[test]
fn escape_forgets_the_open_state() {
    let mut harness = busy_sheet();
    // The first Escape stops the wait, the second leaves the sheet.
    harness.key_press(Key::Escape);
    harness.run();
    assert!(harness.state().settings().is_open());
    harness.key_press(Key::Escape);
    harness.run();
    assert_closed_clean(&mut harness);
}

#[test]
fn a_press_beside_the_sheet_forgets_the_open_state() {
    let mut harness = busy_sheet();
    press_beside_the_sheet(&mut harness);
    assert_closed_clean(&mut harness);
}

#[test]
fn the_rail_and_the_keys_that_show_another_place_forget_the_open_state() {
    for leave in ["Mail", "Calendar", "Sent mail", "Search mail", "Appearance settings"] {
        let mut harness = busy_sheet();
        harness.get_by_role_and_label(Role::Button, leave).click();
        harness.run();
        assert_closed_clean(&mut harness);
    }
    let mut harness = busy_sheet();
    harness.key_press(Key::Escape);
    harness.run();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::K);
    harness.run();
    assert!(harness.state().palette().is_open());
    harness.state_mut().palette_mut().close();
    harness.run();
    assert_closed_clean(&mut harness);
}

#[test]
fn a_waiting_key_chip_has_a_1_px_edge_and_the_focus_ring_stands_2_px_clear_of_it() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let mut harness = on_shortcuts(theme);
        press_chip(&mut harness, "Show sent mail has no key");
        let waiting = "Show sent mail: press a key, or Escape to keep the old one";
        harness.get_by_label(waiting).focus();
        harness.run();
        let rect = harness.get_by_label(waiting).rect();
        let mode = if theme == egui::Theme::Dark { crate::theme::Mode::Dark } else { crate::theme::Mode::Light };
        let cyan = crate::theme::Theme::new(mode, crate::theme::TextScale::default()).palette.cyan_edge;
        let mut strokes = super::tests::strokes_round(harness.output(), cyan, rect);
        strokes.sort_by(|a, b| a.0.width().total_cmp(&b.0.width()));
        let near = |a: egui::Rect, b: egui::Rect| (a.min - b.min).length() < 0.51 && (a.max - b.max).length() < 0.51;
        assert_eq!(strokes.len(), 2, "{theme:?} the edge and the ring: {strokes:?}");
        let ((edge, edge_width, _), (ring, ring_width, whole)) = (strokes[0], strokes[1]);
        assert!(near(edge, rect) && edge_width == 1.0, "{theme:?} the edge is 1 px: {strokes:?} {rect:?}");
        assert!(near(ring, rect.expand(3.0)) && ring_width == 1.0, "{theme:?} the ring is outside, 2 px clear: {strokes:?} {rect:?}");
        assert!(whole, "{theme:?} the ring has room: {strokes:?}");
    }
}

/// The page under the key scopes: it says a key runs one action per screen, lists Fold and the calendar's keys, takes
/// a key only from an action whose scope meets the one given it, and names that action.
#[test]
fn the_page_lists_fold_and_the_calendar_s_keys_and_moves_a_key_only_where_scopes_meet() {
    let mut harness = on_shortcuts(egui::Theme::Dark);
    harness.get_by_label("Every action can have a key. One key runs one action per screen: giving a key takes it from its holder there.");
    harness.get_by_label("Fold or open the floating draft has no key");
    for name in ["left", "right"] {
        let row = format!("Key of Calendar: pick to the {name}");
        assert!(
            harness.query_all_by_role(Role::Button).any(|node| node.accesskit_node().label().is_some_and(|label| label.starts_with(&row))),
            "{row}"
        );
    }
    // Fold given the calendar's Enter: the window's and the calendar's scopes meet, so Open loses it, and the page says so.
    let enter = KeyboardShortcut::new(Modifiers::NONE, Key::Enter);
    press_chip(&mut harness, "Fold or open the floating draft has no key");
    harness.key_press(Key::Enter);
    harness.run();
    assert_eq!(harness.state().keymap().shortcut(keymap::FOLD), Some(enter));
    assert_eq!(harness.state().keymap().shortcut(keymap::CALENDAR_OPEN), None, "one key runs one action where scopes meet");
    assert!(harness.state().settings().keys_note().is_some_and(|note| note.contains("Calendar: open what is picked")));
    // A key held only by the mail list's action stays the list's when a calendar key takes it: the two screens never
    // show together, so nothing is taken and nothing is said.
    let j = KeyboardShortcut::new(Modifiers::NONE, Key::J);
    harness.state_mut().keymap_mut().bind("ui.list.down", j);
    press_chip(&mut harness, "Key of Calendar: pick to the left");
    harness.key_press(Key::J);
    harness.run();
    assert_eq!(harness.state().keymap().shortcut(keymap::CALENDAR_LEFT), Some(j));
    assert_eq!(harness.state().keymap().shortcut("ui.list.down"), Some(j), "the list keeps its key");
    assert_eq!(harness.state().settings().keys_note(), None);
    // Within the calendar's scope one key still runs one action: Right given J takes it from Left.
    press_chip(&mut harness, "Key of Calendar: pick to the right");
    harness.key_press(Key::J);
    harness.run();
    assert_eq!(harness.state().keymap().shortcut(keymap::CALENDAR_LEFT), None);
    assert_eq!(harness.state().keymap().shortcut("ui.list.down"), Some(j));
    assert!(harness.state().settings().keys_note().is_some_and(|note| note.contains("Calendar: pick to the left")));
}

/// The `colour` filled rects painted this frame inside `within`.
fn fills_in(output: &egui::FullOutput, colour: egui::Color32, within: egui::Rect) -> Vec<egui::Rect> {
    fn walk(shape: &egui::Shape, colour: egui::Color32, within: egui::Rect, found: &mut Vec<egui::Rect>) {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, colour, within, found)),
            egui::Shape::Rect(rect) if rect.fill == colour && within.contains_rect(rect.rect) => found.push(rect.rect),
            _ => {}
        }
    }
    let mut found = Vec::new();
    for clipped in &output.shapes {
        walk(&clipped.shape, colour, within, &mut found);
    }
    found
}

#[test]
fn the_row_waiting_for_a_key_is_a_box_720_by_36_with_a_1_px_cyan_edge() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let mut harness = on_shortcuts(theme);
        press_chip(&mut harness, "Show sent mail has no key");
        let chip = harness.get_by_label("Show sent mail: press a key, or Escape to keep the old one").rect();
        let sheet = harness.get_by_role_and_label(Role::Dialog, "Settings").rect();
        // `settings-shortcuts/capture`: the whole row, the column's 720 by its 36, the chip 8 under its top and 16 in
        // from its right end.
        let left = sheet.left() + 1.0 + 232.0 + 1.0 + 40.0;
        let row = egui::Rect::from_min_size(egui::pos2(left, chip.top() - 8.0), vec2(720.0, 36.0));
        assert_eq!(chip.right(), row.right() - 16.0, "{theme:?}: {chip:?} in {row:?}");
        let mode = if theme == egui::Theme::Dark { crate::theme::Mode::Dark } else { crate::theme::Mode::Light };
        let cyan = crate::theme::Theme::new(mode, crate::theme::TextScale::default()).palette.cyan_edge;
        let bands: Vec<_> = fills_in(harness.output(), cyan, row).into_iter().filter(|band| band.width() == 1.0 || band.height() == 1.0).collect();
        assert_eq!(bands.len(), 4, "{theme:?}: four sides: {bands:?}");
        let around = bands.iter().fold(egui::Rect::NOTHING, |all, band| all.union(*band));
        assert_eq!(around, row, "{theme:?}: the edge round the whole row");
    }
}

#[test]
fn reset_all_keys_is_a_secondary_button_134_by_32() {
    let harness = on_shortcuts(egui::Theme::Dark);
    let reset = harness.get_by_role_and_label(Role::Button, "Reset all keys").rect();
    // `settings-shortcuts/default`: a secondary text button, its 1 px `line` edge round 134 × 32.
    assert_eq!((reset.width(), reset.height()), (134.0, 32.0), "{reset:?}");
    let line = crate::theme::Theme::new(crate::theme::Mode::Dark, crate::theme::TextScale::default()).palette.line;
    assert!(!super::tests::strokes_round(harness.output(), line, reset).is_empty(), "its edge");
}

#[test]
fn the_inbox_views_are_actions_the_palette_runs_and_a_key_can_take() {
    use crate::mailbox::InboxView;
    let mut harness = build(app(), egui::Theme::Light);
    for (id, view) in [(keymap::VIEW_GROUPED, InboxView::Grouped), (keymap::VIEW_LIST, InboxView::List), (keymap::VIEW_SMART, InboxView::Smart)] {
        assert!(harness.state().can_run(id), "{id}");
        let ctx = harness.ctx.clone();
        harness.state_mut().run_action(&ctx, id);
        harness.run();
        assert_eq!(harness.state().mailbox().inbox_view(), view, "{id}");
    }
    // Rebindable: a key given to one runs it.
    harness.state_mut().keymap_mut().bind(keymap::VIEW_LIST, KeyboardShortcut::new(Modifiers::COMMAND | Modifiers::SHIFT, Key::L));
    harness.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::L);
    harness.run();
    assert_eq!(harness.state().mailbox().inbox_view(), InboxView::List);
}
