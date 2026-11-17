// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::accesskit::Role;
use egui::epaint::TextShape;
use egui::{Color32, Rect, Shape, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::{Field, Secret, margin, multiline_field, password_field, search, text_field};
use crate::component::Shown;
use crate::component::tests::{Look, Own, bring, builder, drew_icon, own, painted, painted_in, staged, theme};
use crate::iconset::IconId;
use crate::theme::{Mode, TextScale, Theme};

#[test]
fn the_search_pill_shows_a_magnifier_and_takes_typing() {
    let theme = theme();
    let mut harness = builder().build_ui_state(
        move |ui, text: &mut String| {
            search(ui, &theme, egui::Id::unique("search"), text, "Search", 200.0);
        },
        String::new(),
    );
    harness.run();
    assert!(drew_icon(harness.output(), IconId::Search));
    harness.get_by_role(Role::TextInput).focus();
    harness.run();
    harness.get_by_role(Role::TextInput).type_text("odile");
    harness.run();
    assert_eq!(harness.state(), "odile");
}

#[test]
fn a_form_field_has_room_around_its_text() {
    let m = margin(&theme());
    assert!(m.top > 2 && m.left >= m.top, "{m:?}");
}

/// One text the last frame painted: its words, rect, colour, family, size and first row's height.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Drawn {
    pub text: String,
    pub rect: Rect,
    pub colour: Color32,
    pub family: String,
    pub size: f32,
    pub row: f32,
}

/// Every text the last frame of `harness` painted, glyphs included.
pub(crate) fn drawn<State>(harness: &Harness<'_, State>) -> Vec<Drawn> {
    fn walk(shape: &Shape, found: &mut Vec<Drawn>) {
        match shape {
            Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, found)),
            Shape::Text(text) => found.push(read(text)),
            _ => {}
        }
    }
    fn read(text: &TextShape) -> Drawn {
        let section = text.galley.job.sections.first();
        let colour = section.map(|section| section.format.color).filter(|colour| *colour != Color32::PLACEHOLDER);
        let font = section.map(|section| section.format.font_id.clone()).unwrap_or_default();
        Drawn {
            text: text.galley.job.text.clone(),
            rect: text.galley.rect.translate(text.pos.to_vec2()),
            colour: text.override_text_color.or(colour).unwrap_or(text.fallback_color),
            family: font.family.to_string(),
            size: font.size,
            row: text.galley.rows.first().map_or(0.0, |row| row.height()),
        }
    }
    let mut found = Vec::new();
    harness.output().shapes.iter().for_each(|clipped| walk(&clipped.shape, &mut found));
    found
}

/// The one text `text` painted in the last frame.
pub(crate) fn one(harness: &Harness<'_, impl Sized>, text: &str) -> Drawn {
    let all: Vec<Drawn> = drawn(harness).into_iter().filter(|drawn| drawn.text == text).collect();
    assert_eq!(all.len(), 1, "{text}: {all:#?}");
    all.into_iter().next().unwrap_or_else(|| panic!("{text}"))
}

/// The caret a field painted: a filled 7 × 16 block in `cyan_edge`.
pub(crate) fn caret(own_paint: &crate::component::tests::Paint, colour: Color32) -> Option<Rect> {
    own_paint.rects().find_map(|shape| match shape.look {
        Look::Rect { fill, stroke: None, .. } if fill == colour && (shape.rect.width() - 7.0).abs() < 0.01 => Some(shape.rect),
        _ => None,
    })
}

/// The states of the text field's sheet, in its order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Empty,
    Filled,
    Hover,
    Focus,
    Error,
    Disabled,
    ReadOnly,
    Long,
}

const STATES: [State; 8] = [State::Empty, State::Filled, State::Hover, State::Focus, State::Error, State::Disabled, State::ReadOnly, State::Long];

const LONG: &str = "https://dav.example.org/calendars/odile/shared-with-the-studio/";

/// The field's label, label as painted, and value in `state`.
fn words(state: State) -> (&'static str, &'static str, &'static str) {
    match state {
        State::Empty => ("Name on sent mail", "NAME ON SENT MAIL", ""),
        State::Error => ("Port", "PORT", "99999"),
        State::ReadOnly => ("Address", "ADDRESS", "odile@example.org"),
        State::Long => ("CalDAV address, for calendars", "CALDAV ADDRESS, FOR CALENDARS", LONG),
        _ => ("Name on sent mail", "NAME ON SENT MAIL", "Odile Brannock"),
    }
}

/// The text field of the sheet, 320 wide, in `state`: its box, its look, its paint and the harness.
fn text_field_in(mode: Mode, state: State) -> (Rect, Own, crate::component::tests::Paint, Harness<'static>) {
    let theme = Theme::new(mode, TextScale::default());
    let (label, _, value) = words(state);
    let mut harness = staged(
        theme,
        |p| p.bg,
        vec2(420.0, 200.0),
        move |ui, theme| {
            let mut text = value.to_owned();
            let field = Field {
                label,
                hint: "optional",
                error: (state == State::Error).then_some("A port is a number from 1 to 65535."),
                enabled: state != State::Disabled,
                read_only: state == State::ReadOnly,
            };
            ui.vertical(|ui| {
                ui.set_width(320.0);
                text_field(ui, theme, egui::Id::unique("field"), &mut text, &field);
            });
        },
    );
    match state {
        State::Hover => bring(&mut harness, label, Shown::Hover),
        State::Focus => bring(&mut harness, label, Shown::Focus),
        _ => {}
    }
    let rect = harness.get_by_role_and_label(Role::TextInput, label).rect();
    let paint = painted(&harness, label);
    (rect, own(&paint, rect), paint, harness)
}

#[test]
fn a_text_field_looks_as_its_sheet_says_in_every_state() {
    for mode in [Mode::Dark, Mode::Light] {
        let p = Theme::new(mode, TextScale::default()).palette;
        for state in STATES {
            let (rect, look, paint, harness) = text_field_in(mode, state);
            let at = format!("{mode:?} {state:?}: {look:#?}");
            assert!((rect.width() - 320.0).abs() < 0.01 && (rect.height() - 36.0).abs() < 0.01, "{at} {rect:?}");
            let (fill, edge) = match state {
                State::Empty | State::Filled | State::Long => (Some(p.term), (p.line, false)),
                State::Hover => (Some(p.term), (p.muted, false)),
                State::Focus => (Some(p.term), (p.cyan_edge, false)),
                State::Error => (Some(p.term), (p.minus, false)),
                State::Disabled => (None, (p.line, true)),
                State::ReadOnly => (None, (p.line, false)),
            };
            assert_eq!(look.fill, fill, "{at}");
            assert_eq!(look.edge, Some((1.0, edge.0, edge.1)), "{at}");
            assert!(look.ring.is_none() && look.radius <= 0.0, "{at}");
            // The label 24 over the box, in `label` (Plex Mono 11/16, capitals) and `muted`.
            let (_, painted_label, value) = words(state);
            let label = one(&harness, painted_label);
            assert_eq!(label.rect.min, rect.min - vec2(0.0, 24.0), "{at} {label:?}");
            assert_eq!((label.colour, label.family.as_str(), label.size, label.row), (p.muted, "Monospace", 11.0, 16.0), "{at}");
            // The value (or the hint) 13 in and 7 down, Plex Sans 14/22.
            let (shown, colour) = match state {
                State::Empty => ("optional", p.muted),
                State::Disabled => (value, p.muted),
                _ => (value, p.ink),
            };
            let text = one(&harness, shown);
            assert_eq!(text.rect.min, rect.min + vec2(13.0, 7.0), "{at} {text:?}");
            assert_eq!((text.colour, text.family.as_str(), text.size, text.row), (colour, "Proportional", 14.0, 22.0), "{at}");
            // The focus: the caret 2 after the text, 3 down in its row.
            let block = caret(&paint, p.cyan_edge);
            if state == State::Focus {
                let left = (text.rect.right() + 2.0).round();
                assert_eq!(block, Some(Rect::from_min_size(pos2(left, rect.top() + 10.0), vec2(7.0, 16.0))), "{at}");
            } else {
                assert_eq!(block, None, "{at}");
            }
            // Read-only: the lock, 16 at 291, 10, `muted`.
            let lock: Vec<_> = drawn(&harness).into_iter().filter(|drawn| drawn.text == "\u{e2fa}").collect();
            if state == State::ReadOnly {
                assert_eq!(lock.len(), 1, "{at}");
                let centre = rect.min + vec2(299.0, 18.0);
                assert!((lock[0].rect.center() - centre).length() < 1.5 && lock[0].colour == p.muted, "{at} {lock:?}");
            } else {
                assert!(lock.is_empty(), "{at}");
            }
            // The error's message 8 under the box, in `caption` and `minus`.
            if state == State::Error {
                let message = one(&harness, "A port is a number from 1 to 65535.");
                assert_eq!(message.rect.min, rect.left_bottom() + vec2(0.0, 8.0), "{at}");
                assert_eq!((message.colour, message.family.as_str(), message.size, message.row), (p.minus, "Proportional", 12.0, 18.0));
            }
        }
    }
}

#[test]
fn a_text_field_takes_typing_and_a_disabled_one_does_not() {
    for enabled in [true, false] {
        let theme = theme();
        let mut harness = builder().build_ui_state(
            move |ui, text: &mut String| {
                let field = Field { enabled, ..Field::new("Name", "") };
                text_field(ui, &theme, egui::Id::unique("name"), text, &field);
            },
            String::new(),
        );
        harness.run();
        let node = harness.get_by_role_and_label(Role::TextInput, "Name");
        assert_eq!(node.accesskit_node().is_disabled(), !enabled);
        node.focus();
        harness.run();
        harness.get_by_role_and_label(Role::TextInput, "Name").type_text("odile");
        harness.run();
        assert_eq!(harness.state(), if enabled { "odile" } else { "" });
    }
}

#[test]
fn a_focused_caret_blinks_once_a_second_and_asks_for_no_more_frames() {
    let theme = theme();
    let mut harness = builder().with_size(vec2(400.0, 120.0)).build_ui(move |ui| {
        crate::motion::set_reduced(ui.ctx(), true);
        crate::component::tests::stage(ui, theme.palette.bg, |ui| {
            let mut text = "Odile".to_owned();
            text_field(ui, &theme, egui::Id::unique("name"), &mut text, &Field::new("Name", ""));
        });
    });
    harness.run();
    harness.get_by_role_and_label(Role::TextInput, "Name").focus();
    harness.run();
    let shown = |harness: &Harness<'static>| caret(&painted(harness, "Name"), theme.palette.cyan_edge).is_some();
    assert!(shown(&harness), "on at first");
    let delay = harness.output().viewport_output.values().map(|viewport| viewport.repaint_delay).min().unwrap_or_default();
    assert!(delay > std::time::Duration::from_millis(100) && delay <= std::time::Duration::from_millis(500), "{delay:?}");
    // Half a turn later it is off.
    for _ in 0..32 {
        harness.step();
    }
    assert!(!shown(&harness), "off in the second half of its turn");
}

#[test]
fn hostile_text_in_a_field_never_panics() {
    let mib = "x".repeat(1 << 20);
    let marks = format!("a{}", "\u{0301}".repeat(10_000));
    for text in [mib, marks, "\u{0}\r\n".to_owned()] {
        for kind in 0..3 {
            let theme = theme();
            let mut harness = builder().with_size(vec2(400.0, 400.0)).build_ui_state(
                move |ui, text: &mut String| {
                    let field = Field::new("Hostile", "hint");
                    match kind {
                        0 => text_field(ui, &theme, egui::Id::unique("one"), text, &field),
                        1 => multiline_field(ui, &theme, egui::Id::unique("many"), text, &field),
                        _ => password_field(ui, &theme, egui::Id::unique("secret"), text, &field, Secret::Typed),
                    };
                },
                text.clone(),
            );
            harness.run();
            harness.get_by_label("Hostile").focus();
            harness.run();
            harness.key_press(egui::Key::End);
            harness.run();
            harness.key_press(egui::Key::Backspace);
            harness.run();
        }
    }
}

/// The kinds of field a person types into, for the hostile tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Text,
    Many,
    Password,
    Typed,
    Recipient,
    PlainLine,
    PlainText,
}

const KINDS: [Kind; 7] = [Kind::Text, Kind::Many, Kind::Password, Kind::Typed, Kind::Recipient, Kind::PlainLine, Kind::PlainText];

/// The name a screen reader hears for the field of `kind`.
fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Typed => "Type a time",
        Kind::Recipient => "To",
        Kind::PlainLine => "Subject",
        Kind::PlainText => "Message text",
        _ => "Hostile",
    }
}

/// A field of `kind` holding the harness's text, focused.
fn field_of(kind: Kind, text: &str) -> Harness<'static, String> {
    let theme = theme();
    let mut harness = builder().with_size(vec2(600.0, 500.0)).build_ui_state(
        move |ui, text: &mut String| {
            let field = Field::new("Hostile", "hint");
            match kind {
                Kind::Text => {
                    text_field(ui, &theme, egui::Id::unique("one"), text, &field);
                }
                Kind::Many => {
                    multiline_field(ui, &theme, egui::Id::unique("many"), text, &field);
                }
                Kind::Password => {
                    password_field(ui, &theme, egui::Id::unique("secret"), text, &field, Secret::Typed);
                }
                Kind::Typed => {
                    let now = crate::component::gallery::now();
                    crate::component::typed_time::typed_time_field(ui, &theme, egui::Id::unique("typed"), "Type a time", text, now, inboxcraft_time::Offset::UTC);
                }
                Kind::Recipient => {
                    let line = crate::component::recipient::Recipients { name: "To", chips: &[], hint: "Name or address", copies: false, error: None, enabled: true };
                    crate::component::recipient::recipient_field(ui, &theme, egui::Id::unique("to"), text, &line);
                }
                Kind::PlainLine => {
                    super::plain_line(ui, &theme, egui::Id::unique("subject"), text, "Subject");
                }
                Kind::PlainText => {
                    let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, width: f32| {
                        let mut job = egui::text::LayoutJob::single_section(text.as_str().to_owned(), theme.fonts.body.format(theme.palette.ink));
                        job.wrap.max_width = width;
                        ui.fonts_mut(|fonts| fonts.layout_job(job))
                    };
                    super::plain_text(ui, egui::Id::unique("body"), text, 3, &mut layouter, "Message text", &theme);
                }
            }
        },
        text.to_owned(),
    );
    harness.run_ok();
    harness.get_by_label(kind_name(kind)).focus();
    harness.run_ok();
    harness
}

/// The most a field of `kind` holds, and whether that counts bytes (else characters).
fn cap_of(kind: Kind) -> (usize, bool) {
    match kind {
        Kind::Many | Kind::PlainText => (super::MAX_TEXT, true),
        Kind::Password => (super::MAX_SECRET, false),
        Kind::Typed => (crate::component::typed_time::MAX_TYPED, false),
        _ => (super::MAX_LINE, false),
    }
}

/// Whether the last frame says, somewhere, that something was refused as too long.
fn says_too_long(harness: &Harness<'static, String>) -> bool {
    drawn(harness).iter().any(|text| text.text.starts_with("Too long"))
}

#[test]
fn a_paste_of_10_mib_is_refused_whole_in_every_field() {
    let big = "x".repeat(10 << 20);
    for kind in KINDS {
        let mut harness = field_of(kind, "kept");
        let started = std::time::Instant::now();
        harness.event(egui::Event::Paste(big.clone()));
        harness.run_ok();
        assert!(started.elapsed() < std::time::Duration::from_secs(5), "{kind:?}: the frame returns");
        assert_eq!(harness.state(), "kept", "{kind:?}: the text stays exactly as it was");
        assert!(says_too_long(&harness), "{kind:?}: the field says it is too long");
        // The next change that is taken clears the sentence.
        harness.event(egui::Event::Text("!".to_owned()));
        harness.run_ok();
        assert!(harness.state().len() == 5 && !says_too_long(&harness), "{kind:?}: {:?}", harness.state().len());
    }
}

#[test]
fn a_paste_onto_the_cap_is_taken_and_one_past_it_is_refused() {
    for kind in KINDS {
        let (most, _) = cap_of(kind);
        let mut harness = field_of(kind, "");
        harness.event(egui::Event::Paste("y".repeat(most)));
        harness.run_ok();
        assert_eq!(harness.state().len(), most, "{kind:?}: exactly the cap goes in");
        harness.event(egui::Event::Paste("z".to_owned()));
        harness.run_ok();
        assert_eq!(harness.state().len(), most, "{kind:?}: one past the cap is refused");
        assert!(says_too_long(&harness), "{kind:?}");
        // With the whole text selected, a paste as long as the cap replaces it.
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        harness.run_ok();
        harness.event(egui::Event::Paste("w".repeat(most)));
        harness.run_ok();
        assert!(harness.state().starts_with('w') && harness.state().len() == most, "{kind:?}: a selection gives its room");
    }
}

#[test]
fn bidi_controls_ime_and_a_stale_selection_never_panic() {
    for kind in KINDS {
        let mut harness = field_of(kind, "Hello world");
        harness.event(egui::Event::Paste("\u{202e}abc\u{2066}def\u{2069}\u{200f}ghi\u{202c}".to_owned()));
        harness.run_ok();
        // An IME composition in progress, then a commit far past the cap, then one that fits.
        harness.event(egui::Event::Ime(egui::ImeEvent::Preedit { text: "ëñ".to_owned(), active_range_chars: Some(0..2) }));
        harness.run_ok();
        harness.event(egui::Event::Ime(egui::ImeEvent::Commit("x".repeat(10 << 20))));
        harness.run_ok();
        assert!(harness.state().len() < 1 << 20 || matches!(kind, Kind::Many | Kind::PlainText), "{kind:?}");
        harness.event(egui::Event::Ime(egui::ImeEvent::Commit("ñë".to_owned())));
        harness.run_ok();
        // A selection made, then the text replaced from outside: the selection points past its end.
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        harness.run_ok();
        *harness.state_mut() = "x".to_owned();
        harness.run_ok();
        harness.event(egui::Event::Paste("y".repeat(10)));
        harness.event(egui::Event::Text("z".to_owned()));
        harness.key_press(egui::Key::Backspace);
        harness.run_ok();
        assert!(harness.state().chars().count() <= 12, "{kind:?}: {:?}", harness.state());
    }
}

#[test]
fn a_draft_of_1_mib_at_rest_is_laid_out_once() {
    let text: String = (0..(1 << 20) / 32).map(|n| format!("Line {n:>8} of a long draft..\n")).collect();
    assert!(text.len() <= super::MAX_TEXT);
    let mut harness = field_of(Kind::Many, &text);
    harness.step();
    let galley = |harness: &Harness<'static, String>| {
        fn walk(shape: &egui::Shape, found: &mut Option<std::sync::Arc<egui::Galley>>) {
            match shape {
                egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, found)),
                egui::Shape::Text(text) if text.galley.job.text.len() > 1000 => *found = Some(text.galley.clone()),
                _ => {}
            }
        }
        let mut found = None;
        harness.output().shapes.iter().for_each(|clipped| walk(&clipped.shape, &mut found));
        found
    };
    let first = galley(&harness).unwrap_or_else(|| panic!("the draft is painted"));
    let started = std::time::Instant::now();
    for _ in 0..5 {
        harness.step();
        let now = galley(&harness).unwrap_or_else(|| panic!("the draft is painted"));
        assert!(std::sync::Arc::ptr_eq(&first, &now), "a frame at rest laid the draft out again");
    }
    eprintln!("1 MiB draft at rest: {:?} a frame", started.elapsed() / 5);
}

/// The states of the multiline field's sheet, in its order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Many {
    Empty,
    Filled,
    Focus,
    Error,
    Disabled,
    ReadOnly,
    Grown,
    AtLimit,
    Scrolling,
    ScrollingFocus,
}

const MANY: [Many; 10] = [
    Many::Empty,
    Many::Filled,
    Many::Focus,
    Many::Error,
    Many::Disabled,
    Many::ReadOnly,
    Many::Grown,
    Many::AtLimit,
    Many::Scrolling,
    Many::ScrollingFocus,
];

/// `count` short lines of a note.
fn lines(count: usize) -> String {
    (1..=count).map(|n| format!("Line {n} of the note.")).collect::<Vec<_>>().join("\n")
}

/// The value of the multiline field in `state`.
fn many_value(state: Many) -> String {
    match state {
        Many::Empty => String::new(),
        Many::Filled | Many::Disabled | Many::ReadOnly => "Bring the printed proofs.\nThe side door is open from 14:00.".to_owned(),
        Many::Focus => "Bring the printed proofs.\nThe side door is open from 14:00.\n".to_owned(),
        Many::Error => lines(5),
        Many::Grown => lines(6),
        Many::AtLimit => lines(10),
        Many::Scrolling | Many::ScrollingFocus => lines(16),
    }
}

const NOTES: &str = "Notes for the guests";

/// The multiline field of the sheet, 320 wide, in `state`.
fn multiline_in(mode: Mode, state: Many) -> (Rect, Own, crate::component::tests::Paint, Harness<'static>) {
    let theme = Theme::new(mode, TextScale::default());
    let mut harness = staged(
        theme,
        |p| p.bg,
        vec2(420.0, 400.0),
        move |ui, theme| {
            let id = egui::Id::unique("notes");
            let mut text = ui.data_mut(|data| data.get_temp_mut_or_insert_with(id.with("text"), || many_value(state)).clone());
            let field = Field {
                label: NOTES,
                hint: "Anything they should know",
                error: (state == Many::Error).then_some("Too long for a note: 200 characters at most, this has 236."),
                enabled: state != Many::Disabled,
                read_only: state == Many::ReadOnly,
            };
            ui.spacing_mut().scroll = crate::component::scroll::style(theme);
            ui.vertical(|ui| {
                ui.set_width(320.0);
                multiline_field(ui, theme, id, &mut text, &field);
            });
            ui.data_mut(|data| data.insert_temp(id.with("text"), text));
        },
    );
    if matches!(state, Many::Focus | Many::ScrollingFocus) {
        bring(&mut harness, NOTES, Shown::Focus);
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::End);
        harness.run();
    }
    let rect = box_of(&harness, NOTES);
    let paint = painted_in(&harness, NOTES, rect);
    (rect, own(&paint, rect), paint, harness)
}

/// The box of the field `label`: its text node's rect, or the scroll area's around it while it scrolls.
fn box_of(harness: &Harness<'static>, label: &str) -> Rect {
    let node = harness.get_by_label(label).rect();
    // The edit lies inside the box's 1 px edge; a scrolled one is taller than the box and starts above it.
    let edges: Vec<Rect> =
        drawn_rects(harness).into_iter().filter(|rect| (rect.width() - 320.0).abs() < 0.01 && rect.left() == node.left() - 1.0).collect();
    edges.first().copied().unwrap_or(node.expand(1.0))
}

/// The rects of the 1 px edges the last frame painted.
fn drawn_rects<State>(harness: &Harness<'_, State>) -> Vec<Rect> {
    fn walk(shape: &Shape, found: &mut Vec<Rect>) {
        match shape {
            Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, found)),
            Shape::Rect(rect) if rect.stroke.width > 0.0 => found.push(rect.rect),
            _ => {}
        }
    }
    let mut found = Vec::new();
    harness.output().shapes.iter().for_each(|clipped| walk(&clipped.shape, &mut found));
    found
}

#[test]
fn a_multiline_field_looks_as_its_sheet_says_in_every_state() {
    for mode in [Mode::Dark, Mode::Light] {
        let p = Theme::new(mode, TextScale::default()).palette;
        for state in MANY {
            let (rect, look, paint, harness) = multiline_in(mode, state);
            let at = format!("{mode:?} {state:?}: {look:#?} {rect:?}");
            let height = match state {
                Many::Error => 128.0,
                Many::Grown => 150.0,
                Many::AtLimit => 238.0,
                Many::Scrolling | Many::ScrollingFocus => 240.0,
                _ => 96.0,
            };
            assert!((rect.width() - 320.0).abs() < 0.01 && (rect.height() - height).abs() < 0.01, "{at}");
            let (fill, edge) = match state {
                Many::Focus | Many::ScrollingFocus => (Some(p.term), (p.cyan_edge, false)),
                Many::Error => (Some(p.term), (p.minus, false)),
                Many::Disabled => (None, (p.line, true)),
                Many::ReadOnly => (None, (p.line, false)),
                _ => (Some(p.term), (p.line, false)),
            };
            assert_eq!(look.fill, fill, "{at}");
            assert_eq!(look.edge, Some((1.0, edge.0, edge.1)), "{at}");
            assert!(look.ring.is_none() && look.radius <= 0.0, "{at}");
            let label = one(&harness, "NOTES FOR THE GUESTS");
            assert_eq!(label.rect.min, rect.min - vec2(0.0, 24.0), "{at}");
            assert_eq!((label.colour, label.family.as_str(), label.size), (p.muted, "Monospace", 11.0), "{at}");
            // The text 13 in and 9 down (the 1 px edge and the 8 above), Plex Sans 14/22.
            let value = many_value(state);
            let (shown, colour) = match state {
                Many::Empty => ("Anything they should know".to_owned(), p.muted),
                Many::Disabled => (value, p.muted),
                _ => (value, p.ink),
            };
            let text = one(&harness, &shown);
            if !matches!(state, Many::ScrollingFocus) {
                assert_eq!(text.rect.min, rect.min + vec2(13.0, 9.0), "{at} {text:?}");
            }
            assert_eq!((text.colour, text.family.as_str(), text.size, text.row), (colour, "Proportional", 14.0, 22.0), "{at}");
            let wide = match state {
                Many::ReadOnly => 270.0,
                Many::Scrolling | Many::ScrollingFocus => 290.0,
                _ => 294.0,
            };
            assert!(text.rect.width() <= wide + 0.01, "{at} {text:?} wider than {wide}");
            marks_of_a_multiline_field(mode, state, rect, &paint, &harness);
        }
    }
}

/// The caret, the scroll thumb, the lock and the message of the multiline field in `state`.
fn marks_of_a_multiline_field(mode: Mode, state: Many, rect: Rect, paint: &crate::component::tests::Paint, harness: &Harness<'static>) {
    let p = Theme::new(mode, TextScale::default()).palette;
    let at = format!("{mode:?} {state:?}");
    // The caret: at the start of the third line, or inside the view while it scrolls.
    let block = caret(paint, p.cyan_edge);
    match state {
        Many::Focus => assert_eq!(block, Some(Rect::from_min_size(rect.min + vec2(13.0, 56.0), vec2(7.0, 16.0))), "{at}"),
        Many::ScrollingFocus => {
            let view = Rect::from_min_max(rect.min + vec2(13.0, 9.0), rect.max - vec2(17.0, 9.0));
            assert!(block.is_some_and(|block| view.expand(1.0).contains_rect(block)), "{at} {block:?}");
        }
        _ => assert_eq!(block, None, "{at}"),
    }
    // While it scrolls, the bar's thumb at rest stands 4 wide in the 12 strip inside the right edge.
    let thumb =
        paint.rects().find(|shape| matches!(shape.look, Look::Rect { fill, .. } if fill == p.muted) && (shape.rect.width() - 4.0).abs() < 0.01);
    if matches!(state, Many::Scrolling | Many::ScrollingFocus) {
        let thumb = thumb.map(|shape| shape.rect);
        assert!(
            thumb
                .is_some_and(|thumb| thumb.left() == rect.left() + 311.0 && thumb.top() >= rect.top() + 1.0 && thumb.bottom() <= rect.bottom() - 1.0),
            "{at} {thumb:?}"
        );
    } else {
        assert!(thumb.is_none(), "{at}");
    }
    let lock: Vec<_> = drawn(harness).into_iter().filter(|drawn| drawn.text == "\u{e2fa}").collect();
    if state == Many::ReadOnly {
        assert!(lock.len() == 1 && (lock[0].rect.center() - (rect.min + vec2(299.0, 17.0))).length() < 1.5, "{at} {lock:?}");
    } else {
        assert!(lock.is_empty(), "{at}");
    }
    if state == Many::Error {
        let message = one(harness, "Too long for a note: 200 characters at most, this has 236.");
        assert_eq!((message.rect.min, message.colour), (rect.left_bottom() + vec2(0.0, 8.0), p.minus), "{at}");
    }
}

#[test]
fn a_multiline_field_grows_to_240_then_scrolls() {
    for (count, height, scrolls) in [(3, 96.0, false), (10, 238.0, false), (16, 240.0, true)] {
        let theme = Theme::new(Mode::Dark, TextScale::default());
        let harness = staged(
            theme,
            |p| p.bg,
            vec2(420.0, 400.0),
            move |ui, theme| {
                let mut text = lines(count);
                ui.spacing_mut().scroll = crate::component::scroll::style(theme);
                ui.vertical(|ui| {
                    ui.set_width(320.0);
                    multiline_field(ui, theme, egui::Id::unique("notes"), &mut text, &Field::new(NOTES, ""));
                });
            },
        );
        let rect = box_of(&harness, NOTES);
        assert!((rect.height() - height).abs() < 0.01, "{count} lines: {rect:?}");
        let paint = painted_in(&harness, NOTES, rect);
        let thumb = paint.rects().find(|shape| matches!(shape.look, Look::Rect { fill, .. } if fill == theme.palette.muted)).map(|shape| shape.rect);
        if scrolls {
            // The bar's strip is 12 wide between the box's right edge and the text, inside the edge.
            let strip = Rect::from_min_max(pos2(rect.right() - 13.0, rect.top() + 1.0), pos2(rect.right() - 1.0, rect.bottom() - 1.0));
            assert!(
                thumb.is_some_and(|thumb| strip.contains_rect(thumb) && (thumb.center().x - strip.center().x).abs() < 0.01),
                "{thumb:?} in {strip:?}"
            );
        } else {
            assert_eq!(thumb, None, "{count} lines");
        }
    }
}

/// The states of the password field's sheet, in its order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pass {
    Empty,
    Filled,
    Focus,
    Error,
    Disabled,
    Stored,
}

const PASSES: [Pass; 6] = [Pass::Empty, Pass::Filled, Pass::Focus, Pass::Error, Pass::Disabled, Pass::Stored];

/// Not a real password: twelve characters for the twelve dots of the sheet.
const TYPED: &str = "kettle-on-12";

/// The password field of the sheet, 320 wide, in `state`.
fn password_in(mode: Mode, state: Pass) -> (Rect, Own, crate::component::tests::Paint, Harness<'static>) {
    let theme = Theme::new(mode, TextScale::default());
    let label = if state == Pass::Stored { "New password" } else { "Password" };
    let mut harness = staged(
        theme,
        |p| p.bg,
        vec2(420.0, 200.0),
        move |ui, theme| {
            let mut text = if matches!(state, Pass::Empty | Pass::Stored) { String::new() } else { TYPED.to_owned() };
            let field = Field {
                label,
                hint: if state == Pass::Stored { "empty keeps the stored one" } else { "" },
                error: (state == Pass::Error).then_some("The server refused this password."),
                enabled: state != Pass::Disabled,
                read_only: false,
            };
            let secret = if state == Pass::Stored { Secret::Stored } else { Secret::Typed };
            ui.vertical(|ui| {
                ui.set_width(320.0);
                password_field(ui, theme, egui::Id::unique("password"), &mut text, &field, secret);
            });
        },
    );
    if state == Pass::Focus {
        bring(&mut harness, label, Shown::Focus);
        harness.key_press(egui::Key::End);
        harness.run();
    }
    let rect = harness.get_by_label(label).rect();
    let paint = painted(&harness, label);
    (rect, own(&paint, rect), paint, harness)
}

#[test]
fn a_password_field_looks_as_its_sheet_says_in_every_state() {
    let dots: String = std::iter::repeat_n(egui::epaint::text::PASSWORD_REPLACEMENT_CHAR, TYPED.chars().count()).collect();
    for mode in [Mode::Dark, Mode::Light] {
        let p = Theme::new(mode, TextScale::default()).palette;
        for state in PASSES {
            let (rect, look, paint, harness) = password_in(mode, state);
            let at = format!("{mode:?} {state:?}: {look:#?}");
            assert!((rect.width() - 320.0).abs() < 0.01 && (rect.height() - 36.0).abs() < 0.01, "{at} {rect:?}");
            let (fill, edge) = match state {
                Pass::Focus => (Some(p.term), (p.cyan_edge, false)),
                Pass::Error => (Some(p.term), (p.minus, false)),
                Pass::Disabled => (None, (p.line, true)),
                _ => (Some(p.term), (p.line, false)),
            };
            assert_eq!(look.fill, fill, "{at}");
            assert_eq!(look.edge, Some((1.0, edge.0, edge.1)), "{at}");
            let texts = drawn(&harness);
            assert!(texts.iter().all(|text| !text.text.contains(TYPED)), "{at}: the password in clear");
            // The dots 13 in and 8 down, Plex Mono 13/20 in `ink`; none while empty, disabled or stored.
            let shown: Vec<_> = texts.iter().filter(|text| text.text.contains(egui::epaint::text::PASSWORD_REPLACEMENT_CHAR)).collect();
            if matches!(state, Pass::Filled | Pass::Focus | Pass::Error) {
                assert_eq!(shown.len(), 1, "{at}");
                assert_eq!(shown[0].text, dots, "{at}");
                assert_eq!(shown[0].rect.min, rect.min + vec2(13.0, 8.0), "{at}");
                assert_eq!((shown[0].colour, shown[0].family.as_str(), shown[0].size, shown[0].row), (p.ink, "Monospace", 13.0, 20.0), "{at}");
            } else {
                assert!(shown.is_empty(), "{at}: {shown:?}");
            }
            // The mark at 291, 10: the password mark (`dim` when disabled), the lock when stored.
            let (glyph, ink) = match state {
                Pass::Stored => ("\u{e2fa}", p.muted),
                Pass::Disabled => ("\u{e752}", p.dim),
                _ => ("\u{e752}", p.muted),
            };
            let mark = one(&harness, glyph);
            assert!((mark.rect.center() - (rect.min + vec2(299.0, 18.0))).length() < 1.5 && mark.colour == ink, "{at} {mark:?}");
            let block = caret(&paint, p.cyan_edge);
            if state == Pass::Focus {
                let left = (shown[0].rect.right() + 2.0).round();
                assert_eq!(block, Some(Rect::from_min_size(pos2(left, rect.top() + 10.0), vec2(7.0, 16.0))), "{at}");
            } else {
                assert_eq!(block, None, "{at}");
            }
            match state {
                Pass::Error => {
                    let message = one(&harness, "The server refused this password.");
                    assert_eq!((message.rect.min, message.colour), (rect.left_bottom() + vec2(0.0, 8.0), p.minus), "{at}");
                }
                Pass::Stored => {
                    let hint = one(&harness, "empty keeps the stored one");
                    assert_eq!(hint.rect.min, rect.min + vec2(13.0, 7.0), "{at}");
                    assert_eq!((hint.colour, hint.family.as_str(), hint.size), (p.muted, "Proportional", 14.0), "{at}");
                    let note = one(&harness, "The stored password is in your system's credential store and is never shown.");
                    assert_eq!((note.rect.min, note.colour, note.size), (rect.left_bottom() + vec2(0.0, 8.0), p.muted, 12.0), "{at}");
                    assert!((note.rect.height() - 36.0).abs() < 0.01, "{at}: two lines in 320 {note:?}");
                }
                _ => {}
            }
        }
    }
}

#[test]
fn a_stored_password_shows_no_dots() {
    let (rect, _, _, harness) = password_in(Mode::Light, Pass::Stored);
    assert!(drawn(&harness).iter().all(|text| !text.text.contains(egui::epaint::text::PASSWORD_REPLACEMENT_CHAR)), "{rect:?}");
    assert_eq!(harness.get_by_role(Role::PasswordInput).accesskit_node().value().as_deref().unwrap_or_default(), "");
}

#[test]
fn a_password_never_reaches_a_screen_reader_nor_the_undo_history() {
    let theme = theme();
    let mut harness = builder().build_ui_state(
        move |ui, text: &mut String| {
            password_field(ui, &theme, egui::Id::unique("password"), text, &Field::new("Password", ""), Secret::Typed);
        },
        String::new(),
    );
    harness.run();
    harness.get_by_role(Role::PasswordInput).focus();
    harness.run();
    harness.get_by_role(Role::PasswordInput).type_text(TYPED);
    harness.run();
    assert_eq!(harness.state(), TYPED);
    let node = harness.get_by_role(Role::PasswordInput);
    assert!(!node.accesskit_node().value().unwrap_or_default().contains(TYPED));
    assert!(harness.root().children_recursive().all(|node| !format!("{:?}", node.accesskit_node()).contains(TYPED)), "a node holds the password");
    // Undo brings nothing back.
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    harness.key_press(egui::Key::Backspace);
    harness.run();
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    harness.run();
    assert_eq!(harness.state(), "");
}

#[test]
fn a_disabled_stored_password_is_an_empty_box() {
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::default());
        let harness = staged(theme, |p| p.bg, vec2(420.0, 200.0), |ui, theme| {
            let mut text = String::new();
            let field = Field { enabled: false, ..Field::new("New password", "empty keeps the stored one") };
            ui.vertical(|ui| {
                ui.set_width(320.0);
                password_field(ui, theme, egui::Id::unique("stored"), &mut text, &field, Secret::Stored);
            });
        });
        let texts = drawn(&harness);
        assert!(texts.iter().all(|text| text.text != "empty keeps the stored one"), "{mode:?}: no hint");
        assert!(texts.iter().all(|text| text.text != "\u{e2fa}"), "{mode:?}: no lock");
        assert!(texts.iter().all(|text| !text.text.starts_with("The stored password")), "{mode:?}: no sentence");
        let mark = one(&harness, "\u{e752}");
        assert_eq!(mark.colour, theme.palette.dim, "{mode:?}: the password mark in `dim`");
    }
}

#[test]
fn copy_and_cut_in_a_password_field_put_nothing_on_the_clipboard() {
    for event in [egui::Event::Copy, egui::Event::Cut] {
        let theme = theme();
        let mut harness = builder().build_ui_state(
            move |ui, text: &mut String| {
                password_field(ui, &theme, egui::Id::unique("password"), text, &Field::new("Password", ""), Secret::Typed);
            },
            String::new(),
        );
        harness.run();
        harness.get_by_role(Role::PasswordInput).focus();
        harness.run();
        harness.get_by_role(Role::PasswordInput).type_text(TYPED);
        harness.run();
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        harness.run();
        harness.event(event.clone());
        harness.step();
        let copied: Vec<String> = harness
            .output()
            .platform_output
            .commands
            .iter()
            .filter_map(|command| match command {
                egui::OutputCommand::CopyText(text) => Some(text.clone()),
                _ => None,
            })
            .collect();
        assert!(copied.iter().all(|text| text.is_empty() || !text.contains('k')), "{event:?}: {copied:?}");
        assert!(copied.is_empty(), "{event:?}: nothing goes to the clipboard: {copied:?}");
    }
}

#[test]
fn a_caret_blinks_at_full_motion_too_and_comes_back_on() {
    for reduced in [false, true] {
        let theme = theme();
        let mut harness = builder().with_size(vec2(400.0, 120.0)).build_ui(move |ui| {
            crate::motion::set_reduced(ui.ctx(), reduced);
            crate::component::tests::stage(ui, theme.palette.bg, |ui| {
                let mut text = "Odile".to_owned();
                text_field(ui, &theme, egui::Id::unique("name"), &mut text, &Field::new("Name", ""));
            });
        });
        harness.run_ok();
        harness.get_by_role_and_label(Role::TextInput, "Name").focus();
        harness.run_ok();
        let shown = |harness: &Harness<'static>| caret(&painted(harness, "Name"), theme.palette.cyan_edge).is_some();
        let mut seen = Vec::new();
        // Frames of 1/60 s over a second and a half: on, off, on again.
        for _ in 0..90 {
            harness.step();
            seen.push(shown(&harness));
        }
        let changes = seen.windows(2).filter(|pair| pair[0] != pair[1]).count();
        assert!(seen.first() == Some(&true) && changes >= 2, "reduced {reduced}: on, then off and on again: {seen:?}");
        assert!(seen.iter().filter(|on| **on).count() > 30 && seen.iter().filter(|on| !**on).count() > 20, "reduced {reduced}: {seen:?}");
        let off = seen.iter().position(|on| !on).unwrap_or(0);
        assert!(seen.iter().skip(off).any(|on| *on), "reduced {reduced}: it comes back on");
    }
}
