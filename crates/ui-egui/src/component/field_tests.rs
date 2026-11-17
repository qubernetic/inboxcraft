// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::accesskit::Role;
use egui::epaint::TextShape;
use egui::{Color32, Rect, Shape, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::{Field, Secret, margin, multiline_field, password_field, search, text_field};
use crate::component::Shown;
use crate::component::tests::{Look, Own, bring, builder, drew_icon, own, painted, staged, theme};
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
pub(super) struct Drawn {
    pub text: String,
    pub rect: Rect,
    pub colour: Color32,
    pub family: String,
    pub size: f32,
    pub row: f32,
}

/// Every text the last frame of `harness` painted, glyphs included.
pub(super) fn drawn<State>(harness: &Harness<'_, State>) -> Vec<Drawn> {
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
pub(super) fn one(harness: &Harness<'_, impl Sized>, text: &str) -> Drawn {
    let all: Vec<Drawn> = drawn(harness).into_iter().filter(|drawn| drawn.text == text).collect();
    assert_eq!(all.len(), 1, "{text}: {all:#?}");
    all.into_iter().next().unwrap_or_else(|| panic!("{text}"))
}

/// The caret a field painted: a filled 7 × 16 block in `cyan_edge`.
pub(super) fn caret(own_paint: &crate::component::tests::Paint, colour: Color32) -> Option<Rect> {
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

const STATES: [State; 8] =
    [State::Empty, State::Filled, State::Hover, State::Focus, State::Error, State::Disabled, State::ReadOnly, State::Long];

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
