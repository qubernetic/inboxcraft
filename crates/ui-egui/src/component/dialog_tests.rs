// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The confirmation dialog: its card and scrim where the frames put them, the keyboard kept inside and opening on the
//! safe button, Esc and a press on the dimmed window as the safe answer, the danger answer on the owner's press only,
//! and hostile words drawn as plain bounded text.

use egui::accesskit::Role;
use egui::{Color32, Id, Key, Modifiers, Rect, Shape, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::{Answer, Dialog, LINE_ROWS, MOST_CHARS, QUESTION_ROWS, Tone, cover, show};
use crate::component::tests::{builder, theme};
use crate::keymap::Keymap;

/// What the harness keeps: whether the dialog is open, what was answered, and its words.
struct State {
    open: bool,
    answers: Vec<Answer>,
    question: String,
    lines: Vec<(String, Tone)>,
}

/// A 1440 × 900 window with a button under the dialog and the templates frame's dialog open over it.
fn harness_with(question: &str, lines: &[(&str, Tone)]) -> Harness<'static, State> {
    let theme = theme();
    let keymap = Keymap::default();
    let state = State {
        open: true,
        answers: Vec::new(),
        question: question.to_owned(),
        lines: lines.iter().map(|(text, tone)| ((*text).to_owned(), *tone)).collect(),
    };
    let mut harness = builder().with_size(vec2(1440.0, 900.0)).build_ui_state(
        move |ui, state: &mut State| {
            crate::motion::set_reduced(ui.ctx(), true);
            if ui.button("Under").clicked() {
                state.answers.clear();
                state.open = false;
            }
            if state.open {
                let lines: Vec<(&str, Tone)> = state.lines.iter().map(|(text, tone)| (text.as_str(), *tone)).collect();
                let dialog =
                    Dialog { id: Id::unique("test-dialog"), question: &state.question, lines: &lines, keep: "Cancel", act: "Delete template" };
                if let Some(answer) = show(ui.ctx(), &theme, &keymap, &dialog) {
                    state.answers.push(answer);
                    state.open = false;
                }
            }
        },
        state,
    );
    harness.run();
    harness
}

fn harness() -> Harness<'static, State> {
    harness_with(
        "Delete the template “Away from the studio”?",
        &[("It goes from Studio.", Tone::Ink), ("Drafts and sent messages that were written with it keep their text.", Tone::Muted)],
    )
}

fn focused(harness: &Harness<'_, State>) -> Option<String> {
    harness.root().children_recursive().find(|node| node.accesskit_node().is_focused()).map(|node| node.accesskit_node().label().unwrap_or_default())
}

fn key(harness: &mut Harness<'_, State>, key: Key, modifiers: Modifiers) {
    harness.key_press_modifiers(modifiers, key);
    harness.run();
}

/// Every filled rect painted in the last frame in `colour`.
fn filled(harness: &Harness<'_, State>, colour: Color32) -> Vec<Rect> {
    fn find(shape: &Shape, colour: Color32, out: &mut Vec<Rect>) {
        match shape {
            Shape::Vec(inner) => inner.iter().for_each(|shape| find(shape, colour, out)),
            Shape::Rect(rect) if rect.fill == colour => out.push(rect.rect),
            _ => {}
        }
    }
    let mut out = Vec::new();
    harness.output().shapes.iter().for_each(|clipped| find(&clipped.shape, colour, &mut out));
    out
}

/// Where the text starting with `wanted` was painted in the last frame.
fn painted(harness: &Harness<'_, State>, wanted: &str) -> Option<Rect> {
    fn find(shape: &Shape, wanted: &str) -> Option<Rect> {
        match shape {
            Shape::Vec(inner) => inner.iter().find_map(|shape| find(shape, wanted)),
            Shape::Text(text) if text.galley.text().starts_with(wanted) => Some(Rect::from_min_size(text.pos, text.galley.size())),
            _ => None,
        }
    }
    harness.output().shapes.iter().find_map(|clipped| find(&clipped.shape, wanted))
}

/// The card: the surface-filled rect 480 wide.
fn card(harness: &Harness<'_, State>) -> Rect {
    let found = filled(harness, theme().palette.surface);
    *found.iter().find(|rect| (rect.width() - 480.0).abs() < 0.5).unwrap_or_else(|| panic!("no card in {found:?}"))
}

#[test]
fn the_card_and_the_scrim_stand_as_the_frame_draws_them() {
    // The frame: the scrim 1440 × 873 from the window's corner; the card 480 × 228 at x 480; CONFIRM at 25, 25; the
    // question at 53; the lines at 87 and 121; the buttons at 28 (the row's 3 in), 171, 12 apart.
    let harness = harness();
    let scrim = filled(&harness, theme().palette.scrim);
    assert_eq!(scrim.len(), 1, "one scrim: {scrim:?}");
    assert_eq!(scrim[0], Rect::from_min_max(pos2(0.0, 0.0), pos2(1440.0, 873.0)), "over all but the status line");
    assert_eq!(scrim[0], cover(&harness.ctx, &theme()));
    let card = card(&harness);
    assert_eq!(card.size(), vec2(480.0, 228.0), "{card:?}");
    assert_eq!(card.left(), 480.0, "in the middle");
    assert_eq!(card.top(), ((873.0 - 228.0) / 2.0_f32).round(), "in the middle of the dimmed part");
    let at = |wanted: &str| painted(&harness, wanted).unwrap_or_else(|| panic!("{wanted} is drawn")).min - card.min;
    assert_eq!(at("CONFIRM"), vec2(25.0, 25.0));
    assert_eq!(at("Delete the template"), vec2(25.0, 53.0));
    assert_eq!(at("It goes from"), vec2(25.0, 87.0));
    assert_eq!(at("Drafts and sent"), vec2(25.0, 121.0));
    let cancel = harness.get_by_label("Cancel").rect();
    let delete = harness.get_by_label("Delete template").rect();
    assert_eq!(cancel.min - card.min, vec2(28.0, 171.0), "{cancel:?}");
    assert_eq!(cancel.height(), 32.0);
    assert_eq!(delete.left() - cancel.right(), 12.0);
    assert_eq!(delete.top(), cancel.top());
    assert!(painted(&harness, "It goes from").is_some_and(|line| line.width() <= 430.0));
}

#[test]
fn it_opens_on_the_safe_button_and_the_keyboard_stays_inside() {
    let mut harness = harness();
    assert_eq!(focused(&harness).as_deref(), Some("Cancel"));
    harness.get_by_role_and_label(Role::Heading, "Delete the template “Away from the studio”?");
    for (modifiers, want) in [
        (Modifiers::NONE, "Delete template"),
        (Modifiers::NONE, "Cancel"),
        (Modifiers::NONE, "Delete template"),
        (Modifiers::SHIFT, "Cancel"),
        (Modifiers::SHIFT, "Delete template"),
    ] {
        key(&mut harness, Key::Tab, modifiers);
        assert_eq!(focused(&harness).as_deref(), Some(want), "Tab goes round the two buttons only");
    }
    assert!(harness.state().answers.is_empty(), "a focus answers nothing");
}

#[test]
fn esc_is_the_safe_answer() {
    let mut harness = harness();
    key(&mut harness, Key::Tab, Modifiers::NONE);
    assert_eq!(focused(&harness).as_deref(), Some("Delete template"));
    key(&mut harness, Key::Escape, Modifiers::NONE);
    assert_eq!(harness.state().answers, vec![Answer::Keep], "Esc keeps, also from the danger button");
}

#[test]
fn a_press_on_the_dimmed_window_is_the_safe_answer_and_reaches_nothing_under_it() {
    let mut harness = harness();
    let under = harness.get_by_label("Under").rect().center();
    harness.get_by_label("Under").hover();
    harness.run();
    harness.event(egui::Event::PointerButton { pos: under, button: egui::PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    harness.run();
    harness.event(egui::Event::PointerButton { pos: under, button: egui::PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    harness.run();
    assert_eq!(harness.state().answers, vec![Answer::Keep], "the press on the scrim keeps, and the button under it is not pressed");
}

#[test]
fn a_press_on_the_card_answers_nothing() {
    let mut harness = harness();
    let inside = painted(&harness, "It goes from").unwrap().center();
    harness.hover_at(inside);
    harness.run();
    harness.event(egui::Event::PointerButton { pos: inside, button: egui::PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    harness.event(egui::Event::PointerButton { pos: inside, button: egui::PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    harness.run();
    assert!(harness.state().open && harness.state().answers.is_empty());
}

#[test]
fn enter_presses_the_focused_button() {
    let mut harness = harness();
    key(&mut harness, Key::Enter, Modifiers::NONE);
    assert_eq!(harness.state().answers, vec![Answer::Keep], "Enter on Cancel keeps");
    let mut harness = self::harness();
    key(&mut harness, Key::Tab, Modifiers::NONE);
    key(&mut harness, Key::Enter, Modifiers::NONE);
    assert_eq!(harness.state().answers, vec![Answer::Act], "Enter on the danger button acts, once");
}

#[test]
fn a_hover_on_the_danger_button_does_nothing_and_its_press_acts_once() {
    let mut harness = harness();
    harness.get_by_label("Delete template").hover();
    harness.run();
    assert!(harness.state().answers.is_empty());
    harness.get_by_label("Delete template").click();
    harness.run();
    assert_eq!(harness.state().answers, vec![Answer::Act]);
}

#[test]
fn hostile_words_are_plain_and_bounded() {
    let long = "N".repeat(10_000);
    let question = format!("Delete the template “a\r\nb\0c\u{202E}d{long}”?");
    let line = format!("It goes from x\r\ny\u{2067}z{long}.");
    let harness = harness_with(&question, &[(&line, Tone::Ink)]);
    let heading = harness.get_by_role(Role::Heading);
    let said = heading.accesskit_node().label().unwrap_or_default();
    assert!(said.starts_with("Delete the template “a  b cdNNN"), "{:?}", said.get(..40));
    assert!(!said.chars().any(|c| c.is_control() || c == '\u{202E}'), "plain");
    assert!(said.chars().count() <= MOST_CHARS);
    let card = card(&harness);
    assert_eq!(card.width(), 480.0, "the card keeps its width");
    let heading = painted(&harness, "Delete the template").unwrap();
    let words = painted(&harness, "It goes from").unwrap();
    assert!(heading.width() <= 430.5 && words.width() <= 430.5, "{heading:?} {words:?}");
    let style = theme().fonts;
    assert!(heading.height() <= style.heading.line * QUESTION_ROWS as f32 + 0.5, "{heading:?}");
    assert!(words.height() <= style.body.line * LINE_ROWS as f32 + 0.5, "{words:?}");
    assert!(card.bottom() <= 873.0 && card.top() >= 0.0, "{card:?}");
}
