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
    palette: false,
    capture: false,
    sheet: false,
    editor: false,
    composer: false,
    full_width: false,
    one_pane_reader: false,
    mail: true,
    list_hidden: false,
};

/// The calendar.
const CALENDAR: Facts = Facts { mail: false, ..MAIL };

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
        ("a float over the sheet", Facts { float: true, sheet: true, ..MAIL }, Surface::Float),
        ("the sheet over a list", Facts { sheet: true, ..MAIL }, Surface::Sheet),
        ("the sheet over the composer", Facts { sheet: true, composer: true, ..MAIL }, Surface::Sheet),
        ("the sheet over the event editor", Facts { sheet: true, editor: true, ..CALENDAR }, Surface::Sheet),
        ("a capture on the sheet", Facts { capture: true, sheet: true, ..MAIL }, Surface::Capture),
        ("the palette over the editor", Facts { palette: true, editor: true, ..CALENDAR }, Surface::Palette),
        ("the palette over the composer", Facts { palette: true, composer: true, ..MAIL }, Surface::Palette),
        ("a float over the full-width reader", Facts { float: true, full_width: true, ..MAIL }, Surface::Float),
        ("the full-width reader", Facts { full_width: true, ..MAIL }, Surface::FullWidth),
        ("the full width beside a docked composer", Facts { full_width: true, composer: true, ..MAIL }, Surface::FullWidth),
        ("a float over the composer", Facts { float: true, composer: true, ..MAIL }, Surface::Float),
        ("the reader in one pane over the list", Facts { one_pane_reader: true, ..MAIL }, Surface::OnePaneReader),
        ("a float over the reader in one pane", Facts { float: true, one_pane_reader: true, ..MAIL }, Surface::Float),
        ("the composer in one pane", Facts { composer: true, list_hidden: true, ..MAIL }, Surface::Composer),
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
}

#[test]
fn every_surface_at_once_is_held_in_eight_and_the_place_stays_last() {
    let all = Facts {
        float: true,
        palette: true,
        capture: true,
        sheet: true,
        editor: true,
        composer: true,
        full_width: true,
        one_pane_reader: true,
        mail: true,
        list_hidden: true,
    };
    let surfaces = of(all);
    assert_eq!(surfaces.all().len(), 8);
    assert_eq!(surfaces.top(), Surface::Float);
    assert_eq!(surfaces.all().last(), Some(&Surface::Place));
    assert!(!surfaces.list_has_keyboard());
}

/// The window's key filter as it stood before it asked [`Surfaces`] (`InboxApp::keys` and `pressed`, run with each
/// action bound to F20, without a modifier and with Command, nothing toasted, no field focused, the list keyed), in each
/// stacking: the actions it let run. Send is left out: its key needs the keyboard in the composer, which no surface
/// is. Generated once from those functions (the test `draw::surface_old_tests`, removed with them), checked against
/// `Surfaces::allows` there.
const WINDOW: [&str; 22] = [
    "ui.palette.open",
    "ui.search.focus",
    "ui.mail.show",
    "ui.sent.show",
    "ui.later.show",
    "ui.compose.new",
    "ui.compose.fold",
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
    "ui.motion.toggle",
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

/// What runs under the sheet, with a modifier only.
const SHEET: [&str; 13] = [
    "ui.palette.open",
    "ui.search.focus",
    "ui.mail.show",
    "ui.sent.show",
    "ui.later.show",
    "ui.calendar.show",
    "ui.reader.full_width",
    "ui.settings.open",
    "ui.shortcuts.open",
    "ui.mode.light",
    "ui.mode.dark",
    "ui.mode.system",
    "ui.motion.toggle",
];

/// What runs with a modifier beside the composer in front: the window's keys but Undo, the composer's text's.
const COMPOSER: [&str; 21] = [
    "ui.palette.open",
    "ui.search.focus",
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
    "ui.motion.toggle",
    "ui.view.grouped",
    "ui.view.smart",
    "ui.view.list",
];

/// What ran: `(stacking, its facts, without a modifier, with Command)`, each a set of lists.
type Row = (&'static str, Facts, &'static [&'static [&'static str]], &'static [&'static [&'static str]]);

fn today() -> Vec<(&'static str, Facts, Vec<&'static str>, Vec<&'static str>)> {
    let rows: [Row; 18] = [
        ("place", MAIL, &[&WINDOW, &LIST], &[&WINDOW, &LIST]),
        ("float", Facts { float: true, ..MAIL }, &[&WINDOW], &[&WINDOW]),
        ("palette", Facts { palette: true, ..MAIL }, &[], &[]),
        ("capture", Facts { capture: true, sheet: true, ..MAIL }, &[], &[]),
        ("sheet", Facts { sheet: true, ..MAIL }, &[], &[&SHEET]),
        ("editor", Facts { editor: true, ..CALENDAR }, &[&WINDOW], &[&WINDOW]),
        ("composer", Facts { composer: true, ..MAIL }, &[], &[&COMPOSER]),
        ("full_width", Facts { full_width: true, ..MAIL }, &[&WINDOW, &LIST], &[&WINDOW, &LIST]),
        ("one_pane_reader", Facts { one_pane_reader: true, ..MAIL }, &[&WINDOW, &LIST], &[&WINDOW, &LIST]),
        ("calendar", CALENDAR, &[&WINDOW], &[&WINDOW]),
        ("float_over_sheet", Facts { float: true, sheet: true, ..MAIL }, &[], &[&SHEET]),
        ("sheet_over_composer", Facts { sheet: true, composer: true, ..MAIL }, &[], &[&SHEET]),
        ("palette_over_editor", Facts { palette: true, editor: true, ..CALENDAR }, &[], &[]),
        ("float_over_full_width", Facts { float: true, full_width: true, ..MAIL }, &[&WINDOW], &[&WINDOW]),
        ("float_over_composer", Facts { float: true, composer: true, ..MAIL }, &[], &[&COMPOSER]),
        ("composer_one_pane_list_hidden", Facts { composer: true, list_hidden: true, ..MAIL }, &[], &[&COMPOSER]),
        ("composer_beside_list_hidden", Facts { composer: true, list_hidden: true, ..MAIL }, &[], &[&COMPOSER]),
        ("palette_over_composer", Facts { palette: true, composer: true, ..MAIL }, &[], &[]),
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
            for (modifiers, ran) in [(Modifiers::NONE, &bare), (Modifiers::COMMAND, &command)] {
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
        ("the palette", Facts { palette: true, ..MAIL }),
        ("a capture", Facts { capture: true, sheet: true, ..MAIL }),
        ("the sheet", Facts { sheet: true, ..MAIL }),
        ("the composer in front", Facts { composer: true, ..MAIL }),
        ("hidden beside the composer", Facts { list_hidden: true, ..MAIL }),
    ] {
        assert!(!of(facts).list_has_keyboard(), "{name}");
    }
}
