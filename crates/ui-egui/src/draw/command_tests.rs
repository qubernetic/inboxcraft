// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The command line driven through its AccessKit tree: `Ctrl K` and `/` open it empty, `Ctrl F` narrowed to the
//! mailbox; `>` lists what the palette listed, `@` people, `#` mailboxes; the arrows and Enter or a press run one; one
//! `Esc` closes the results, the next gives the keyboard back; hostile and awkward typing runs nothing.

use egui::accesskit::Role;
use egui::{Event, Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_engine::appearance::ThemeMode;
use inboxcraft_time::{FakeClock, Timestamp};

use super::{InboxApp, Screen};
use crate::command::{Mode, field_id};
use crate::component::tests::{keyboard_walk, rests};
use crate::keymap;
use crate::mailbox::{Mailbox, Place};
use crate::sample;

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

fn sized(theme: egui::Theme, size: egui::Vec2) -> Harness<'static, InboxApp> {
    let app = InboxApp::at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    let mut harness =
        Harness::builder().with_size(size).with_theme(theme).with_max_steps(12).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

fn harness(theme: egui::Theme) -> Harness<'static, InboxApp> {
    sized(theme, vec2(1200.0, 820.0))
}

/// Opened by `Ctrl K`, `prefix` typed.
fn opened(theme: egui::Theme, prefix: &str) -> Harness<'static, InboxApp> {
    let mut harness = harness(theme);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::K);
    harness.run();
    assert!(harness.state().command().results());
    if !prefix.is_empty() {
        type_in(&mut harness, prefix);
    }
    harness
}

fn type_in(harness: &mut Harness<'static, InboxApp>, text: &str) {
    harness.get_by_role_and_label(Role::TextInput, "Command line").type_text(text);
    harness.run();
}

/// Whether the last frame painted a text that reads `wanted`.
fn painted_text(harness: &Harness<'static, InboxApp>, wanted: &str) -> bool {
    fn find(shape: &egui::Shape, wanted: &str) -> bool {
        match shape {
            egui::Shape::Vec(inner) => inner.iter().any(|shape| find(shape, wanted)),
            egui::Shape::Text(text) => text.galley.text() == wanted,
            _ => false,
        }
    }
    harness.output().shapes.iter().any(|clipped| find(&clipped.shape, wanted))
}

/// The field's box as last drawn.
fn field_box(harness: &Harness<'static, InboxApp>) -> Option<egui::Rect> {
    harness.ctx.data(|data| data.get_temp::<egui::Rect>(field_id().with("box")))
}

fn field_focused(harness: &Harness<'static, InboxApp>) -> bool {
    harness.ctx.memory(|memory| memory.has_focus(field_id()))
}

/// The rows of the results: each menu row's name and whether it is enabled.
fn rows(harness: &Harness<'static, InboxApp>) -> Vec<(String, bool)> {
    harness
        .query_all_by_role(Role::MenuItem)
        .map(|node| (node.accesskit_node().label().unwrap_or_default(), !node.accesskit_node().is_disabled()))
        .collect()
}

/// Presses `/` as a keyboard does: the key, then the text it types.
fn slash(harness: &mut Harness<'static, InboxApp>) {
    harness.event(Event::Key { key: Key::Slash, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE });
    harness.event(Event::Text("/".to_owned()));
    harness.event(Event::Key { key: Key::Slash, physical_key: None, pressed: false, repeat: false, modifiers: Modifiers::NONE });
    harness.run();
}

#[test]
fn ctrl_k_and_slash_open_it_empty_and_ctrl_f_narrowed_to_the_mailbox() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let harness = opened(theme, "");
        assert!(field_focused(&harness), "Ctrl K: the field has the keyboard");
        assert_eq!((harness.state().command().mode(), harness.state().command().text()), (Mode::Search, ""));
        harness.check_accessibility();

        let mut harness = self::harness(theme);
        slash(&mut harness);
        assert!(field_focused(&harness) && harness.state().command().results(), "/: the field has the keyboard");
        assert_eq!(harness.state().command().text(), "", "the slash that opened it is not typed");

        let mut harness = self::harness(theme);
        harness.key_press_modifiers(Modifiers::COMMAND, Key::F);
        harness.run();
        assert!(field_focused(&harness) && harness.state().command().narrowed());
        assert!(painted_text(&harness, "search this mailbox"), "narrowed to the mailbox");
    }
}

/// The ids and names the palette listed (`palette::rows` on the screen shown), each with whether it could run.
fn palette_list(harness: &Harness<'static, InboxApp>, screen: keymap::Scope) -> Vec<(String, bool)> {
    let app = harness.state();
    crate::palette::rows("")
        .into_iter()
        .filter(|row| keymap::scope(row.id).meets(screen))
        .map(|row| {
            let runs = app.can_run(row.id) && !crate::component::soon::blocked(&harness.ctx, row.id);
            (row.name.into_owned(), runs)
        })
        .collect()
}

#[test]
fn the_command_list_is_the_palettes_on_every_place_with_and_without_a_composer() {
    for composer in [false, true] {
        for place in ["mail", "home", "calendar", "settings"] {
            let mut harness = harness(egui::Theme::Light);
            if composer {
                harness.state_mut().compose();
                harness.run();
            }
            let ctx = harness.ctx.clone();
            match place {
                "home" => harness.state_mut().show_home(),
                "calendar" => harness.state_mut().show_calendar(),
                "settings" => harness.state_mut().run_action(&ctx, keymap::SETTINGS),
                _ => {}
            }
            harness.run();
            harness.state_mut().run_action(&ctx, keymap::PALETTE);
            harness.run();
            type_in(&mut harness, ">");
            let screen = if place == "calendar" { keymap::Scope::Calendar } else { keymap::Scope::Mail };
            let mut wanted = palette_list(&harness, screen);
            // The plugin's command, coming soon, ends the list.
            wanted.push(("A plugin's command".to_owned(), false));
            assert_eq!(rows(&harness), wanted, "{place}, composer {composer}");
        }
    }
}

#[test]
fn typing_narrows_the_commands_and_enter_runs_the_first() {
    let mut harness = opened(egui::Theme::Light, ">calendar");
    let names: Vec<String> = rows(&harness).into_iter().map(|(name, _)| name).collect();
    assert_eq!(names.first().map(String::as_str), Some("Show the calendar"), "{names:?}");
    harness.key_press(Key::Enter);
    harness.run();
    assert!(!harness.state().command().results(), "running a command closes the results");
    assert_eq!(harness.state().screen(), Screen::Calendar);
    assert_eq!(harness.state().command().text(), "", "a command typed goes with it");
}

/// Which row of `role` the AccessKit tree says is picked: the one row said selected, which the field names as its
/// active descendant. People are told from the mail's rows (also list box options) by their address.
fn said_picked(harness: &Harness<'static, InboxApp>, role: Role) -> Option<usize> {
    let person = |row: &egui_kittest::Node<'_>| {
        role != Role::ListBoxOption || row.accesskit_node().label().is_some_and(|label| label.contains("@example.org"))
    };
    let rows: Vec<_> = harness.query_all_by_role(role).filter(person).collect();
    let selected: Vec<usize> =
        rows.iter().enumerate().filter(|(_, row)| row.accesskit_node().is_selected() == Some(true)).map(|(at, _)| at).collect();
    let field = harness.get_by_role_and_label(Role::TextInput, "Command line");
    let active = field.accesskit_node().active_descendant().map(|node| node.id());
    match selected.as_slice() {
        [at] if active == rows.get(*at).map(|row| row.accesskit_node().id()) => Some(*at),
        _ => None,
    }
}

#[test]
fn the_people_and_the_mailboxes_say_the_row_picked() {
    let mut harness = opened(egui::Theme::Light, "#a");
    assert!(rows(&harness).len() > 1, "more than one mailbox");
    harness.key_press(Key::ArrowDown);
    harness.run();
    assert_eq!(said_picked(&harness, Role::MenuItem), Some(1), "#");
    let mut harness = opened(egui::Theme::Light, "@e");
    let people =
        harness.query_all_by_role(Role::ListBoxOption).filter(|row| row.accesskit_node().label().is_some_and(|label| label.contains("@example.org")));
    assert!(people.count() > 1, "more than one person");
    harness.key_press(Key::ArrowDown);
    harness.run();
    assert_eq!(said_picked(&harness, Role::ListBoxOption), Some(1), "@");
}

#[test]
fn the_arrows_move_the_pick_and_enter_runs_the_picked_command() {
    let mut harness = opened(egui::Theme::Light, ">mode");
    let selected = harness.state().mailbox().selected();
    assert_eq!(said_picked(&harness, Role::MenuItem), Some(0), "the first row is said picked");
    harness.key_press(Key::ArrowDown);
    harness.run();
    assert_eq!(harness.state().command().picked(), 1);
    assert_eq!(said_picked(&harness, Role::MenuItem), Some(1), "the second row is said picked, the field's active descendant");
    assert!(field_focused(&harness), "the keyboard stays in the field");
    assert_eq!(harness.state().mailbox().selected(), selected, "the arrows are the command line's, not the list's");
    harness.key_press(Key::Enter);
    harness.run();
    assert_ne!(harness.state().appearance().theme, ThemeMode::System, "the second mode row ran");
    assert_eq!(harness.state().mailbox().last_run(), Some("ui.theme.set"), "through the engine's command");
}

#[test]
fn a_text_that_matches_no_command_says_so_and_enter_runs_nothing() {
    let mut harness = opened(egui::Theme::Light, ">zzz");
    harness.get_by_label("Nothing matches “zzz”.");
    let before = harness.state().mailbox().last_run();
    harness.key_press(Key::Enter);
    harness.run();
    assert!(harness.state().command().results(), "Enter with nothing picked runs nothing");
    assert_eq!(harness.state().mailbox().last_run(), before);
}

#[test]
fn a_refused_command_says_why_and_runs_nothing() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let mut harness = opened(theme, ">snooze");
        assert!(harness.state().reader().open_id().is_none(), "no message is open");
        let snooze = keymap::action(keymap::SNOOZE).unwrap().name;
        assert!(harness.get_by_role_and_label(Role::MenuItem, snooze).accesskit_node().is_disabled());
        assert!(painted_text(&harness, "no message is open"), "the reason shows in the row");
        let before = harness.state().mailbox().last_run();
        harness.key_press(Key::Enter);
        harness.run();
        assert!(harness.state().command().results(), "the refused command stays listed");
        assert_eq!(harness.state().mailbox().last_run(), before, "nothing ran");
    }
}

#[test]
fn a_press_on_a_row_runs_it() {
    let mut harness = opened(egui::Theme::Light, ">dark");
    harness.get_by_role_and_label(Role::MenuItem, "Dark mode").click();
    harness.run();
    assert_eq!(harness.state().appearance().theme, ThemeMode::Dark);
    assert!(!harness.state().command().results());
}

#[test]
fn a_page_row_shows_the_settings_on_its_page() {
    let mut harness = opened(egui::Theme::Light, ">settings: sending");
    harness.key_press(Key::Enter);
    harness.run();
    assert!(harness.state().settings().is_open());
    assert_eq!(harness.state().settings().section(), crate::settings::Section::Sending);
}

#[test]
fn at_lists_the_people_of_the_mail_and_enter_shows_their_mail() {
    let mut harness = opened(egui::Theme::Light, "@teodor");
    harness.query_all_by_label_contains("teodor@example.org").next().expect("Teodor is listed");
    harness.key_press(Key::Enter);
    harness.run();
    assert_eq!(harness.state().command().mode(), Mode::Search);
    assert_eq!(harness.state().search_text(), "teodor@example.org", "their mail is searched");
}

#[test]
fn hash_lists_the_mailboxes_and_enter_goes_there() {
    let mut harness = opened(egui::Theme::Light, "#bakery");
    assert_eq!(rows(&harness), vec![("Clients/Harbour Bakery".to_owned(), true)]);
    harness.key_press(Key::Enter);
    harness.run();
    assert_eq!(harness.state().mailbox().place(), Place::Folder(15));
    assert!(!harness.state().command().results());
}

#[test]
fn plain_text_searches_the_mail_as_it_is_typed() {
    let harness = opened(egui::Theme::Light, "bakery");
    assert_eq!(harness.state().search_text(), "bakery");
    assert_eq!(harness.state().mailbox().query(), "bakery");
}

#[test]
fn typing_on_home_or_the_calendar_changes_no_place_until_enter() {
    for place in [Screen::Home, Screen::Calendar] {
        let mut harness = harness(egui::Theme::Light);
        match place {
            Screen::Home => harness.state_mut().show_home(),
            _ => harness.state_mut().show_calendar(),
        }
        harness.run();
        harness.key_press_modifiers(Modifiers::COMMAND, Key::K);
        harness.run();
        type_in(&mut harness, "bake");
        assert_eq!(harness.state().screen(), place, "{place:?}: typing keeps the place");
        assert_eq!(harness.state().search_text(), "", "{place:?}: and searches nothing under it");
        harness.key_press(Key::Escape);
        harness.run();
        assert_eq!(harness.state().screen(), place, "{place:?}: Esc keeps it");
        type_in(&mut harness, "ry");
        assert_eq!(harness.state().screen(), place, "{place:?}: typed again");
        harness.key_press(Key::Enter);
        harness.run();
        assert_eq!((harness.state().screen(), harness.state().search_text()), (Screen::Mail, "bakery"), "{place:?}: Enter shows the mail searched");
        assert_eq!(harness.state().mailbox().query(), "bakery");
        assert!(!harness.state().command().results());
    }
}

// Review focus 4.

#[test]
fn a_long_paste_is_capped() {
    // Put in past the field (whose own limit would cut it first), as the pictures and the prefixes' hints put text in:
    // the command line's state cuts it.
    let mut harness = opened(egui::Theme::Light, "");
    let before = harness.state().mailbox().last_run();
    harness.state_mut().command_mut().open_with(&"x".repeat(10_000));
    harness.run();
    assert_eq!(harness.state().command().text().chars().count(), crate::command::MOST);
    assert!(field_focused(&harness), "the keyboard stays in the field");
    let mut harness = opened(egui::Theme::Light, "");
    harness.state_mut().command_mut().open_with(&format!(">{}", "y".repeat(10_000)));
    harness.run();
    assert_eq!(harness.state().command().mode(), Mode::Command);
    assert_eq!(harness.state().command().text().chars().count(), crate::command::MOST - 1, "the mark and the text, capped");
    harness.key_press(Key::Enter);
    harness.run();
    assert_eq!(harness.state().mailbox().last_run(), before, "nothing ran");
}

#[test]
fn a_bare_prefix_lists_and_runs_nothing() {
    for (prefix, mode) in [(">", Mode::Command), ("@", Mode::Person), ("#", Mode::Mailbox)] {
        let mut harness = opened(egui::Theme::Light, prefix);
        assert_eq!((harness.state().command().mode(), harness.state().command().text()), (mode, ""), "{prefix}");
        let (screen, place, before) = (harness.state().screen(), harness.state().mailbox().place(), harness.state().mailbox().last_run());
        if mode == Mode::Command {
            assert!(rows(&harness).len() > 9, "> lists every command");
        }
        harness.key_press(Key::Enter);
        harness.run();
        let app = harness.state();
        assert_eq!((app.screen(), app.mailbox().place(), app.mailbox().last_run()), (screen, place, before), "{prefix} ran nothing");
    }
}

#[test]
fn slash_in_a_text_field_types_a_slash() {
    // The floating composer is no quiet surface: only the window's gate for a focused field keeps the bare `/` from
    // opening the command line.
    let mut harness = harness(egui::Theme::Light);
    harness.state_mut().compose();
    if let (Some(composer), _) = harness.state_mut().composer_mut() {
        composer.set_floating(true);
    }
    harness.run();
    let ctx = harness.ctx.clone();
    ctx.memory_mut(|memory| memory.request_focus(super::compose::subject_id()));
    harness.run();
    slash(&mut harness);
    assert!(!harness.state().command().results(), "the command line did not open");
    assert!(!field_focused(&harness));
    let subject = harness.state().composer().map(|composer| composer.subject().to_owned());
    assert_eq!(subject.as_deref(), Some("/"), "the slash is typed in the field");
}

#[test]
fn tab_and_shift_tab_keep_the_keyboard_in_the_field_while_the_results_show() {
    let mut harness = opened(egui::Theme::Light, ">");
    for (shift, times) in [(false, 6), (true, 3)] {
        for _ in 0..times {
            let modifiers = if shift { Modifiers::SHIFT } else { Modifiers::NONE };
            harness.key_press_modifiers(modifiers, Key::Tab);
            harness.run();
            assert!(field_focused(&harness), "shift {shift}: the field keeps the keyboard");
            assert!(harness.state().command().results(), "shift {shift}: the results stay");
        }
    }
}

#[test]
fn esc_closes_results_then_returns_the_focus() {
    let mut harness = harness(egui::Theme::Light);
    harness.state_mut().compose();
    harness.run();
    let subject = super::compose::subject_id();
    harness.ctx.memory_mut(|memory| memory.request_focus(subject));
    harness.run();
    let from = harness.ctx.memory(egui::Memory::focused);
    assert_eq!(from, Some(subject), "the subject has the keyboard");
    harness.key_press_modifiers(Modifiers::COMMAND, Key::K);
    harness.run();
    type_in(&mut harness, ">dark");
    let open = harness.state().reader().open_id();
    harness.key_press(Key::Escape);
    harness.run();
    assert!(!harness.state().command().results(), "the first Esc closes the results");
    assert!(field_focused(&harness), "and the field keeps the keyboard");
    assert_eq!(harness.state().reader().open_id(), open, "nothing under it closed");
    harness.key_press(Key::Escape);
    harness.run();
    assert!(!field_focused(&harness), "the second gives it back");
    assert_eq!(harness.ctx.memory(egui::Memory::focused), from, "to where it came from");
    assert_eq!(harness.state().command().text(), "", "a command typed goes");
    assert_eq!(harness.state().appearance().theme, ThemeMode::System, "nothing ran");
}

#[test]
fn a_press_beside_the_results_closes_them_and_reaches_nothing() {
    let mut harness = opened(egui::Theme::Light, ">");
    let selected = harness.state().mailbox().selected();
    let beside = egui::pos2(600.0, 800.0);
    harness.hover_at(beside);
    harness.run();
    for pressed in [true, false] {
        harness.event(Event::PointerButton { pos: beside, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
        harness.run();
    }
    assert!(!harness.state().command().results());
    assert_eq!(harness.state().mailbox().selected(), selected);
}

/// The eight frames of the command line at 1440 × 900, each in dark and light: the field 1081 × 28 at 234,9 (the
/// command line's 233,8 inside the window's 1 px edge), the results the field's width straight under it.
#[test]
fn the_eight_frames_stand_where_the_design_draws_them() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        // rest: the hint and the key that opens it.
        let mut harness = sized(theme, vec2(1440.0, 900.0));
        // The harness lays the window 8 in: the frame's numbers are from the window's corner.
        let laid = super::shell::laid(&harness.ctx).unwrap();
        let corner = laid.command.min - egui::vec2(1.0, 1.0);
        let field = egui::Rect::from_min_size(corner + vec2(234.0, 9.0), vec2(laid.command.width() + 2.0 - 234.0 - 125.0, 28.0));
        assert_eq!(laid.command.width() + 2.0, 1424.0, "the window, 8 in from each side of the harness");
        assert_eq!(field_box(&harness), Some(field), "rest");
        assert!(painted_text(&harness, "search mail or run a command"), "rest: the hint");
        // focus, typing-search, commands, people, mailboxes, no-result, refused.
        for (typed, head) in [
            ("", None),
            ("bakery", None),
            (">", Some("Commands")),
            ("@teodor", Some("People")),
            ("#", Some("Mailboxes")),
            (">zzz", Some("Commands")),
            (">snooze", Some("Commands")),
        ] {
            harness.key_press_modifiers(Modifiers::COMMAND, Key::K);
            harness.run();
            if !typed.is_empty() {
                type_in(&mut harness, typed);
            }
            assert!(field_focused(&harness), "{typed:?}");
            for word in ["commands", "people", "mailboxes"] {
                harness.get_by_label(&crate::component::key_hint::spoken(
                    match word {
                        "commands" => ">",
                        "people" => "@",
                        _ => "#",
                    },
                    word,
                ));
            }
            if let Some(head) = head {
                let shown = harness.get_by_label(head).rect();
                assert!((shown.left() - field.left()).abs() <= 1.0, "{typed:?}: the head across the results, at the field's left");
                assert!(shown.top() >= field.bottom(), "{typed:?}: under the field");
            }
            harness.key_press(Key::Escape);
            harness.key_press(Key::Escape);
            harness.run();
        }
    }
}

#[test]
fn every_control_is_reached_by_tab_and_the_open_line_rests_but_for_the_caret() {
    let harness = opened(egui::Theme::Light, ">");
    // With the results shown the keyboard stays in the field (the arrows pick a row); closed, Tab reaches the field.
    assert!(field_focused(&harness));
    let mut harness = harness_resting();
    let unreached = keyboard_walk(&mut harness);
    assert!(!unreached.iter().any(|label| label == "Command line"), "{unreached:?}");
    assert!(rests(&mut harness, 8), "the closed command line asks for no frame");
}

fn harness_resting() -> Harness<'static, InboxApp> {
    harness(egui::Theme::Light)
}

#[test]
fn every_action_runs_through_run_action_and_an_unknown_one_does_nothing() {
    let mut harness = harness(egui::Theme::Light);
    let ctx = harness.ctx.clone();
    let app = harness.state_mut();
    app.run_action(&ctx, "mail.nothing");
    assert!(!app.can_run("mail.nothing"));
    app.run_action(&ctx, keymap::SHORTCUTS);
    assert_eq!(app.settings().section(), crate::settings::Section::Shortcuts);
    app.run_action(&ctx, keymap::SETTINGS);
    assert!(!app.settings().is_open(), "Settings again shows the place the owner came from");
    app.run_action(&ctx, keymap::SETTINGS);
    assert!(app.settings().is_open(), "and once more the settings");
    assert_eq!(app.settings().section(), crate::settings::Section::Shortcuts, "on the page last shown");
    app.run_action(&ctx, keymap::SENT);
    assert!(!app.settings().is_open());
    assert_eq!(app.mailbox().place(), crate::mailbox::Place::View(inboxcraft_model::Role::Sent));
    app.run_action(&ctx, keymap::MAIL);
    assert_eq!(app.mailbox().place(), crate::mailbox::Place::View(inboxcraft_model::Role::Inbox));
    app.run_action(&ctx, keymap::DARK);
    assert_eq!(app.appearance().theme, ThemeMode::Dark);
    app.run_action(&ctx, keymap::LIGHT);
    assert_eq!(app.appearance().theme, ThemeMode::Light);
    app.run_action(&ctx, keymap::SYSTEM_MODE);
    assert_eq!(app.appearance().theme, ThemeMode::System);
    let reduce = app.reduce_motion();
    app.run_action(&ctx, keymap::MOTION);
    assert_eq!(app.reduce_motion(), !reduce);
    assert!(!app.can_run(keymap::FULL_WIDTH), "no message is open");
    app.run_action(&ctx, keymap::CALENDAR);
    assert_eq!(app.screen(), Screen::Calendar);
    app.run_action(&ctx, keymap::FIND);
    assert_eq!(app.screen(), Screen::Mail, "the search shows the mail");
    harness.run();
    assert!(field_focused(&harness), "the command line has the keyboard");
    harness.key_press(Key::Escape);
    harness.run();
    harness.query_all_by_label_contains("Draft logo for the bakery").next().unwrap().click();
    harness.run();
    let ctx = harness.ctx.clone();
    let app = harness.state_mut();
    assert!(app.can_run(keymap::FULL_WIDTH));
    app.run_action(&ctx, keymap::FULL_WIDTH);
    assert!(app.reader().full_width());
    app.run_action(&ctx, keymap::READ);
    assert!(app.mailbox().undoable().is_some(), "read or unread acts on the open message");
    app.run_action(&ctx, keymap::UNDO);
    assert!(app.mailbox().undoable().is_none(), "undone");
    app.run_action(&ctx, keymap::DELETE);
    assert!(app.mailbox().leaving().is_some(), "the deleted row slides out");
}

#[test]
fn the_command_line_offers_no_action_only_a_coming_soon_control_offers() {
    for normal in [false, true] {
        let mut harness = harness(egui::Theme::Light);
        crate::component::tests::offer_soon(&harness.ctx, keymap::SHORTCUTS, normal);
        harness.run();
        assert!(harness.state().can_run(keymap::SHORTCUTS));
        let ctx = harness.ctx.clone();
        harness.state_mut().run_action(&ctx, keymap::PALETTE);
        harness.run();
        type_in(&mut harness, ">");
        let shortcuts = harness.get_by_role_and_label(Role::MenuItem, "Shortcuts").accesskit_node().is_disabled();
        assert_eq!(shortcuts, !normal, "Shortcuts is runnable in the command line only when a usable control offers it too");
    }
}

/// How many texts the last frame painted with their middle in `rect`: a row's icon, its name and its key's words.
fn texts_in(harness: &Harness<'static, InboxApp>, rect: egui::Rect) -> usize {
    fn open(shape: &egui::Shape, rect: egui::Rect, count: &mut usize) {
        match shape {
            egui::Shape::Vec(inner) => inner.iter().for_each(|shape| open(shape, rect, count)),
            egui::Shape::Text(text) if !text.galley.text().trim().is_empty() && rect.contains(text.visual_bounding_rect().center()) => *count += 1,
            _ => {}
        }
    }
    let mut count = 0;
    harness.output().shapes.iter().for_each(|clipped| open(&clipped.shape, rect, &mut count));
    count
}

#[test]
fn a_page_row_has_no_key_where_settings_shows_its_own() {
    let names = ["Settings: Appearance", "Settings: Accounts", "Settings: Notifications"];
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let mut harness = opened(theme, ">settings");
        assert!(harness.state().keymap().chip(&egui::Context::default(), keymap::SETTINGS).is_some(), "Settings has a key");
        let bound: Vec<usize> = names.iter().map(|name| texts_in(&harness, harness.get_by_role_and_label(Role::MenuItem, name).rect())).collect();
        // Settings' key taken away: a page row draws what it drew, as it never showed the key.
        harness.state_mut().keymap_mut().unbind(keymap::SETTINGS);
        harness.run();
        let unbound: Vec<usize> = names.iter().map(|name| texts_in(&harness, harness.get_by_role_and_label(Role::MenuItem, name).rect())).collect();
        assert_eq!(bound, unbound, "no key column on a page row");
        assert!(bound.iter().all(|texts| *texts >= 2), "its icon and its name: {bound:?}");
    }
}
