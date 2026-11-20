// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::{Event, Key, KeyboardShortcut, Modifiers, RawInput};

use super::{Facts, Surface, Surfaces};
use crate::keymap::{self, Keymap};

/// The surfaces of `facts`.
fn of(facts: Facts) -> Surfaces {
    Surfaces::new(facts)
}

/// Mail, the list shown.
const MAIL: Facts = Facts {
    float: false,
    dialog: false,
    command: false,
    capture: false,
    editor: false,
    composer: false,
    full_width: false,
    one_pane_reader: false,
    mail: true,
    settings: false,
    home: false,
    composer_drawn: false,
};

/// The calendar.
const CALENDAR: Facts = Facts { mail: false, ..MAIL };

/// The settings, shown from the mail: a place, not a surface in front.
const SETTINGS: Facts = Facts { settings: true, ..MAIL };

/// Home, a place without mail.
const HOME: Facts = Facts { mail: false, home: true, ..MAIL };

/// Whether `ctx`, in a pass with `events`, gives `escape` its answer; the pass is ended.
fn in_a_pass<R>(events: Vec<Event>, run: impl FnOnce(&egui::Context) -> R) -> R {
    let ctx = egui::Context::default();
    ctx.begin_pass(RawInput { events, ..RawInput::default() });
    let answer = run(&ctx);
    let mut output = ctx.end_pass();
    output.textures_delta.clear();
    answer
}

fn esc() -> Vec<Event> {
    vec![Event::Key { key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }]
}

/// Every stacking the window can reach, from the screens' Esc tests (`rests_tests`, `cover_tests`, `list_tests`,
/// `reader_tests`), with the surface one Esc closes: the top one.
fn stackings() -> Vec<(&'static str, Facts, Surface)> {
    vec![
        ("a float over the list", Facts { float: true, ..MAIL }, Surface::Float),
        ("a float over a settings page", Facts { float: true, ..SETTINGS }, Surface::Float),
        ("a capture on the settings", Facts { capture: true, ..SETTINGS }, Surface::Capture),
        ("a confirmation on the settings", Facts { dialog: true, ..SETTINGS }, Surface::Dialog),
        ("a float over a capture on the settings", Facts { float: true, capture: true, ..SETTINGS }, Surface::Float),
        ("the command results over the settings", Facts { command: true, ..SETTINGS }, Surface::Command),
        ("a float over the command results over the settings", Facts { float: true, command: true, ..SETTINGS }, Surface::Float),
        ("the command results over the editor", Facts { command: true, editor: true, ..CALENDAR }, Surface::Command),
        ("the command results over the composer", Facts { command: true, composer: true, ..MAIL }, Surface::Command),
        ("a float over the full-width reader", Facts { float: true, full_width: true, ..MAIL }, Surface::Float),
        ("the full-width reader", Facts { full_width: true, ..MAIL }, Surface::FullWidth),
        ("the full width beside a docked composer", Facts { full_width: true, composer: true, ..MAIL }, Surface::FullWidth),
        ("a float over the composer", Facts { float: true, composer: true, ..MAIL }, Surface::Float),
        ("the reader in one pane over the list", Facts { one_pane_reader: true, ..MAIL }, Surface::OnePaneReader),
        ("a float over the reader in one pane", Facts { float: true, one_pane_reader: true, ..MAIL }, Surface::Float),
        ("the composer in one pane", Facts { composer: true, ..MAIL }, Surface::Composer),
        ("the event editor", Facts { editor: true, ..CALENDAR }, Surface::Editor),
        ("a float over the event editor", Facts { float: true, editor: true, ..CALENDAR }, Surface::Float),
    ]
}

#[test]
fn one_esc_closes_one_surface_in_every_stacking() {
    for (name, facts, top) in stackings() {
        let surfaces = of(facts);
        assert_eq!(surfaces.top(), top, "{name}: on top");
        assert_eq!(in_a_pass(esc(), |ctx| surfaces.escape(ctx)), Some(top), "{name}: one Esc closes the top one");
        assert_eq!(in_a_pass(Vec::new(), |ctx| surfaces.escape(ctx)), None, "{name}: no Esc, nothing closes");
        // Only it: what stood under it is still there after the Esc's answer.
        assert_eq!(surfaces.all().iter().filter(|surface| **surface == top).count(), 1, "{name}");
        assert_eq!(surfaces.all().last(), Some(&Surface::Place), "{name}: the place is last");
    }
    // Nothing in front: the Esc is the place's (the reader gives the list the keyboard back).
    assert_eq!(in_a_pass(esc(), |ctx| of(MAIL).escape(ctx)), None);
    assert_eq!(of(MAIL).all(), &[Surface::Place]);
    // The settings are a place: an Esc with nothing in front closes nothing of them.
    assert_eq!(in_a_pass(esc(), |ctx| of(SETTINGS).escape(ctx)), None);
    assert_eq!(of(SETTINGS).all(), &[Surface::Place]);
    // The full width and the reader in one pane are the mail's: the settings stand in their place.
    assert_eq!(of(Facts { full_width: true, ..SETTINGS }).all(), &[Surface::Place]);
}

#[test]
fn every_surface_at_once_is_held_in_nine_and_the_place_stays_last() {
    let all = Facts {
        float: true,
        dialog: true,
        command: true,
        capture: true,
        editor: true,
        composer: true,
        full_width: true,
        one_pane_reader: true,
        mail: true,
        settings: false,
        home: false,
        composer_drawn: true,
    };
    let surfaces = of(all);
    assert_eq!(surfaces.all().len(), 9);
    assert_eq!(surfaces.top(), Surface::Float);
    assert_eq!(surfaces.all().last(), Some(&Surface::Place));
    assert!(!surfaces.list_has_keyboard());
}

/// The window's key filter as it stood before it asked [`Surfaces`] (`InboxApp::keys` and `pressed`, run with each
/// action bound to F20, without a modifier and with Command, nothing toasted, no field focused, the list keyed), in each
/// stacking: the actions it let run. Send is left out: its key needs the keyboard in the composer, which no surface
/// is. Generated once from those functions (the test `draw::surface_old_tests`, removed with them), checked against
/// `Surfaces::allows` there.
const WINDOW: [&str; 26] = [
    "ui.palette.open",
    "ui.command.open",
    "ui.search.focus",
    "ui.home.open",
    "ui.mail.show",
    "ui.sent.show",
    "ui.later.show",
    "ui.compose.new",
    "ui.calendar.show",
    "ui.reader.open_held",
    "ui.message.snooze",
    "ui.undo",
    "ui.reader.full_width",
    "ui.reader.back",
    "ui.settings.open",
    "ui.shortcuts.open",
    "ui.mode.light",
    "ui.mode.dark",
    "ui.mode.system",
    "ui.text_size.larger",
    "ui.text_size.larger_equals",
    "ui.text_size.smaller",
    "ui.text_size.reset",
    "ui.view.grouped",
    "ui.view.smart",
    "ui.view.list",
];

/// The list's keys.
const LIST: [&str; 11] = [
    "ui.list.next",
    "ui.list.previous",
    "ui.list.down",
    "ui.list.up",
    "ui.list.open",
    "ui.list.menu",
    "ui.list.menu_key",
    "ui.list.page_down",
    "ui.list.page_up",
    "ui.list.first",
    "ui.list.last",
];

/// What runs on the settings, with a modifier only: the window's keys but those of the mail (Undo, the views, Back,
/// the held-back message, Snooze, the composer's fold). Under the sheet New message did not run; on the settings it
/// shows the place the owner came from and opens the composer there.
const ON_SETTINGS: [&str; 19] = [
    "ui.palette.open",
    "ui.command.open",
    "ui.search.focus",
    "ui.home.open",
    "ui.mail.show",
    "ui.sent.show",
    "ui.later.show",
    "ui.compose.new",
    "ui.calendar.show",
    "ui.reader.full_width",
    "ui.settings.open",
    "ui.shortcuts.open",
    "ui.mode.light",
    "ui.mode.dark",
    "ui.mode.system",
    "ui.text_size.larger",
    "ui.text_size.larger_equals",
    "ui.text_size.smaller",
    "ui.text_size.reset",
];

/// What runs on Home, with a modifier or without (Home has no field the letters are for): the window's keys but those
/// of the mail, the same rule as on the settings.
const ON_HOME: [&str; 19] = ON_SETTINGS;

/// What runs with a modifier beside the composer in front: the window's keys but Undo, the composer's text's.
const COMPOSER: [&str; 26] = [
    "ui.palette.open",
    "ui.command.open",
    "ui.search.focus",
    "ui.home.open",
    "ui.mail.show",
    "ui.sent.show",
    "ui.later.show",
    "ui.compose.new",
    "ui.compose.fold",
    "ui.calendar.show",
    "ui.reader.open_held",
    "ui.message.snooze",
    "ui.reader.full_width",
    "ui.reader.back",
    "ui.settings.open",
    "ui.shortcuts.open",
    "ui.mode.light",
    "ui.mode.dark",
    "ui.mode.system",
    "ui.text_size.larger",
    "ui.text_size.larger_equals",
    "ui.text_size.smaller",
    "ui.text_size.reset",
    "ui.view.grouped",
    "ui.view.smart",
    "ui.view.list",
];

/// The composer's fold: it runs wherever a composer is drawn, on any place, and rests where none is. Before #303's
/// fix it ran on the mail and the calendar with or without one, and rested on Home though Home draws the floating one.
const FOLD: [&str; 1] = ["ui.compose.fold"];

/// What runs under the event editor: what comes over it and the look (the mode and the text size).
const EDITOR: [&str; 10] = [
    "ui.palette.open",
    "ui.settings.open",
    "ui.shortcuts.open",
    "ui.mode.light",
    "ui.mode.dark",
    "ui.mode.system",
    "ui.text_size.larger",
    "ui.text_size.larger_equals",
    "ui.text_size.smaller",
    "ui.text_size.reset",
];

/// What ran: `(stacking, its facts, without a modifier, with Command)`, each a set of lists.
type Row = (&'static str, Facts, &'static [&'static [&'static str]], &'static [&'static [&'static str]]);

fn today() -> Vec<(&'static str, Facts, Vec<&'static str>, Vec<&'static str>)> {
    let rows: [Row; 24] = [
        ("place", MAIL, &[&WINDOW, &LIST], &[&WINDOW, &LIST]),
        ("float", Facts { float: true, ..MAIL }, &[&WINDOW], &[&WINDOW]),
        ("palette", Facts { command: true, ..MAIL }, &[], &[]),
        ("capture", Facts { capture: true, ..SETTINGS }, &[], &[]),
        ("settings", SETTINGS, &[], &[&ON_SETTINGS]),
        ("settings_with_the_full_width_kept", Facts { full_width: true, one_pane_reader: true, ..SETTINGS }, &[], &[&ON_SETTINGS]),
        // Before #254's fix every window key ran under the editor (`WINDOW`, also with a modifier): New message
        // opened under it. Now only what comes over it.
        ("editor", Facts { editor: true, ..CALENDAR }, &[&EDITOR], &[&EDITOR]),
        ("composer", Facts { composer: true, composer_drawn: true, ..MAIL }, &[], &[&COMPOSER]),
        ("full_width", Facts { full_width: true, ..MAIL }, &[&WINDOW, &LIST], &[&WINDOW, &LIST]),
        ("one_pane_reader", Facts { one_pane_reader: true, ..MAIL }, &[&WINDOW, &LIST], &[&WINDOW, &LIST]),
        ("calendar", CALENDAR, &[&WINDOW], &[&WINDOW]),
        ("float_over_settings", Facts { float: true, ..SETTINGS }, &[], &[&ON_SETTINGS]),
        ("palette_over_editor", Facts { command: true, editor: true, ..CALENDAR }, &[], &[]),
        ("float_over_full_width", Facts { float: true, full_width: true, ..MAIL }, &[&WINDOW], &[&WINDOW]),
        ("float_over_composer", Facts { float: true, composer: true, composer_drawn: true, ..MAIL }, &[], &[&COMPOSER]),
        ("palette_over_composer", Facts { command: true, composer: true, composer_drawn: true, ..MAIL }, &[], &[]),
        ("home", HOME, &[&ON_HOME], &[&ON_HOME]),
        ("float_over_home", Facts { float: true, ..HOME }, &[&ON_HOME], &[&ON_HOME]),
        ("palette_over_home", Facts { command: true, ..HOME }, &[], &[]),
        // The floating composer, on each place that draws it: its fold runs there.
        ("floating_composer", Facts { composer_drawn: true, ..MAIL }, &[&WINDOW, &LIST, &FOLD], &[&WINDOW, &LIST, &FOLD]),
        ("floating_composer_on_the_calendar", Facts { composer_drawn: true, ..CALENDAR }, &[&WINDOW, &FOLD], &[&WINDOW, &FOLD]),
        ("floating_composer_on_home", Facts { composer_drawn: true, ..HOME }, &[&ON_HOME, &FOLD], &[&ON_HOME, &FOLD]),
        ("float_over_the_floating_composer_on_home", Facts { float: true, composer_drawn: true, ..HOME }, &[&ON_HOME, &FOLD], &[&ON_HOME, &FOLD]),
        ("palette_over_the_floating_composer_on_home", Facts { command: true, composer_drawn: true, ..HOME }, &[], &[]),
    ];
    rows.into_iter()
        .map(|(name, facts, bare, command)| {
            let flat = |lists: &[&[&'static str]]| lists.iter().flat_map(|list| list.iter().copied()).collect::<Vec<_>>();
            (name, facts, flat(bare), flat(command))
        })
        .collect()
}

#[test]
fn allows_matches_today() {
    for (name, facts, bare, command) in today() {
        let surfaces = of(facts);
        for action in keymap::actions().into_iter().map(|action| action.id).filter(|id| *id != keymap::SEND) {
            // Alt is a modifier as Command is; Shift alone is not (a bare key).
            let columns = [(Modifiers::NONE, &bare), (Modifiers::COMMAND, &command), (Modifiers::ALT, &command), (Modifiers::SHIFT, &bare)];
            for (modifiers, ran) in columns {
                let mut keys = Keymap::empty();
                keys.bind(action, KeyboardShortcut::new(modifiers, Key::F20));
                assert_eq!(surfaces.allows(action, &keys), ran.contains(&action), "{name}: {action} with {modifiers:?}");
            }
        }
    }
}

#[test]
fn the_list_has_the_keys_only_on_the_mail_with_nothing_over_it_and_shown() {
    assert!(of(MAIL).list_has_keyboard());
    assert!(of(Facts { full_width: true, ..MAIL }).list_has_keyboard(), "the reader's full width takes no keys from it");
    assert!(of(Facts { one_pane_reader: true, ..MAIL }).list_has_keyboard(), "the reader in one pane: the focus decides");
    for (name, facts) in [
        ("the calendar", CALENDAR),
        ("a float", Facts { float: true, ..MAIL }),
        ("the palette", Facts { command: true, ..MAIL }),
        ("a capture", Facts { capture: true, ..SETTINGS }),
        ("the settings", SETTINGS),
        ("the composer in front", Facts { composer: true, ..MAIL }),
    ] {
        assert!(!of(facts).list_has_keyboard(), "{name}");
    }
}

// The window under the palette and the event editor: a key with a modifier runs nothing that opens or changes what
// lies under them.

fn clock() -> Box<inboxcraft_time::FakeClock> {
    Box::new(inboxcraft_time::FakeClock::new(inboxcraft_time::Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

/// The sample window, `setup` done first, New message bound to Ctrl N.
fn window(setup: impl FnOnce(&mut crate::InboxApp)) -> egui_kittest::Harness<'static, crate::InboxApp> {
    let mut app = crate::InboxApp::at_mail(crate::mailbox::Mailbox::new(crate::sample::session(clock()).unwrap(), clock()));
    app.keymap_mut().bind(keymap::COMPOSE, KeyboardShortcut::new(Modifiers::COMMAND, Key::N));
    setup(&mut app);
    let mut harness = egui_kittest::Harness::builder()
        .with_size(egui::vec2(1440.0, 900.0))
        .with_max_steps(12)
        .build_ui_state(|ui, app: &mut crate::InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

#[test]
fn under_the_palette_a_key_with_a_modifier_runs_nothing() {
    let mut harness = window(|_| {});
    harness.key_press_modifiers(Modifiers::COMMAND, Key::K);
    harness.run();
    assert!(harness.state().command().results());
    for key in [Key::N, Key::Comma, Key::F] {
        harness.key_press_modifiers(Modifiers::COMMAND, key);
        harness.run();
        assert!(harness.state().composer().is_none(), "{key:?}: no new message under the palette");
        assert!(!harness.state().settings().is_open(), "{key:?}: no settings under the palette");
        assert!(harness.state().command().results(), "{key:?}: the palette stays");
    }
}

#[test]
fn under_the_event_editor_a_key_with_a_modifier_runs_only_what_comes_over_it() {
    let mut harness = window(|app| {
        app.show_calendar();
        app.calendar_mut().0.new_event();
    });
    assert!(harness.state().calendar().editor().is_some(), "the editor is open");
    // New message and Find would open or show something under it.
    for key in [Key::N, Key::F] {
        harness.key_press_modifiers(Modifiers::COMMAND, key);
        harness.run();
        assert!(harness.state().composer().is_none(), "{key:?}: no new message under the editor");
        assert_eq!(harness.state().screen(), crate::Screen::Calendar, "{key:?}: the calendar stays");
        assert!(harness.state().calendar().editor().is_some(), "{key:?}: the editor stays");
    }
    // The settings are shown in its place; the editor waits, as it was, for the calendar's return.
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Comma);
    harness.run();
    assert!(harness.state().settings().is_open(), "the settings are shown");
    assert!(harness.state().calendar().editor().is_some(), "the editor is kept");
}

#[test]
fn send_runs_wherever_a_composer_is_drawn_and_rests_where_none_is() {
    let keys = Keymap::default();
    let rows = [
        ("in the reader's place", Facts { composer: true, composer_drawn: true, ..MAIL }, true),
        ("floating over the mail", Facts { composer_drawn: true, ..MAIL }, true),
        ("floating over the calendar", Facts { composer_drawn: true, ..CALENDAR }, true),
        ("floating over Home", Facts { composer_drawn: true, ..HOME }, true),
        ("Home without one", HOME, false),
        ("the settings, which draw none", SETTINGS, false),
        ("the mail without one", MAIL, false),
    ];
    for (name, facts, runs) in rows {
        assert_eq!(of(facts).allows(keymap::SEND, &keys), runs, "{name}: its key");
        assert_eq!(of(facts).allows_press(keymap::SEND), runs, "{name}: a press");
    }
}

#[test]
fn a_confirmation_dialog_is_modal_holds_the_keys_and_lets_no_key_of_the_window_run() {
    let surfaces = of(Facts { dialog: true, ..SETTINGS });
    assert_eq!(surfaces.top(), Surface::Dialog);
    assert!(surfaces.modal(), "nothing of the place runs under it");
    assert!(surfaces.held(), "its keys are its own");
    let keymap = Keymap::default();
    for action in WINDOW.iter().chain(LIST.iter()) {
        assert!(!surfaces.allows(action, &keymap), "{action} rests under the dialog");
        assert!(!surfaces.allows_press(action), "{action}'s control rests under the dialog");
    }
    assert!(!of(SETTINGS).modal(), "the settings alone are no modal surface");
}
