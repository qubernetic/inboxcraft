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
use crate::theme::{Mode, TextScale, Theme};

/// What the harness keeps: whether the dialog is open, what was answered, and its words.
struct State {
    open: bool,
    answers: Vec<Answer>,
    question: String,
    lines: Vec<(String, Tone)>,
    /// What the dialog asks about, where it stands.
    near: Option<Rect>,
}

/// A 1440 × 900 window with a button under the dialog and the templates frame's dialog open over it.
fn harness_with(question: &str, lines: &[(&str, Tone)]) -> Harness<'static, State> {
    harness_at(vec2(1440.0, 900.0), theme(), None, question, lines)
}

/// A window `size` drawn in `theme` with a button under the dialog and a dialog asking about `near` open over it.
fn harness_at(size: egui::Vec2, theme: Theme, near: Option<Rect>, question: &str, lines: &[(&str, Tone)]) -> Harness<'static, State> {
    let keymap = Keymap::default();
    let state = State {
        open: true,
        answers: Vec::new(),
        question: question.to_owned(),
        lines: lines.iter().map(|(text, tone)| ((*text).to_owned(), *tone)).collect(),
        near,
    };
    let mut harness = builder().with_size(size).build_ui_state(
        move |ui, state: &mut State| {
            crate::motion::set_reduced(ui.ctx(), true);
            if ui.button("Under").clicked() {
                state.answers.clear();
                state.open = false;
            }
            if state.open {
                let lines: Vec<(&str, Tone)> = state.lines.iter().map(|(text, tone)| (text.as_str(), *tone)).collect();
                let dialog = Dialog {
                    id: Id::unique("test-dialog"),
                    question: &state.question,
                    lines: &lines,
                    keep: "Cancel",
                    act: "Delete template",
                    near: state.near,
                };
                if let Some(answer) = show(ui, &theme, &keymap, &dialog) {
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

const QUESTION: &str = "Delete the template “Away from the studio”?";

const LINES: [(&str, Tone); 2] =
    [("It goes from Studio.", Tone::Ink), ("Drafts and sent messages that were written with it keep their text.", Tone::Muted)];

fn harness() -> Harness<'static, State> {
    harness_with(QUESTION, &LINES)
}

/// The templates frame's dialog asking about `near`.
fn near(near: Rect) -> Harness<'static, State> {
    harness_at(vec2(1440.0, 900.0), theme(), Some(near), QUESTION, &LINES)
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
    assert_eq!(card.top(), 322.0, "asking about nothing, in the middle of the dimmed part, floored: (873 − 228) / 2 = 322.5");
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
    let question = format!("Delete the template “a\r\nb\0c\u{202E}\u{200B}\u{FEFF}\u{2028}d{long}”?");
    let line = format!("It goes from x\r\ny\u{2067}z{long}.");
    let harness = harness_with(&question, &[(&line, Tone::Ink)]);
    let heading = harness.get_by_role(Role::Heading);
    let said = heading.accesskit_node().label().unwrap_or_default();
    assert!(said.starts_with("Delete the template “a  b cdNNN"), "{:?}", said.get(..40));
    assert!(!said.chars().any(|c| c.is_control() || super::super::hidden(c)), "plain");
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

#[test]
fn it_stands_twelve_under_what_it_asks_about() {
    // The frame `settings-templates/delete-confirm`: the row "Away from the studio" ends at 411, the card is at 423.
    let harness = near(Rect::from_min_max(pos2(273.0, 363.0), pos2(993.0, 411.0)));
    assert_eq!(card(&harness).min, pos2(480.0, 423.0));
}

#[test]
fn without_room_under_it_stands_twelve_over_what_it_asks_about() {
    // 634 + 12 + 228 is past 873 − 12: over it, 602 − 12 − 228.
    let harness = near(Rect::from_min_max(pos2(900.0, 602.0), pos2(1040.0, 634.0)));
    assert_eq!(card(&harness).min, pos2(480.0, 362.0));
    // Room for it on neither side: 12 from the top.
    let harness = self::near(Rect::from_min_max(pos2(0.0, 100.0), pos2(1440.0, 800.0)));
    assert_eq!(card(&harness).min, pos2(480.0, 12.0));
}

#[test]
fn what_it_asks_about_out_of_the_dimmed_part_leaves_it_in_the_middle() {
    let harness = near(Rect::from_min_max(pos2(0.0, 2000.0), pos2(1440.0, 2048.0)));
    assert_eq!(card(&harness).min, pos2(480.0, 322.0));
    let harness = near(Rect::from_min_max(pos2(0.0, f32::NAN), pos2(1440.0, 2048.0)));
    assert_eq!(card(&harness).min, pos2(480.0, 322.0));
}

#[test]
fn its_first_frame_stands_where_the_next_ones_do() {
    // In the middle of the dimmed part, where its place follows its height.
    let mut harness = harness();
    let settled = card(&harness);
    key(&mut harness, Key::Escape, Modifiers::NONE);
    assert!(!harness.state().open);
    for _ in 0..3 {
        harness.step();
    }
    harness.state_mut().open = true;
    harness.step();
    assert_eq!(card(&harness), settled, "the first frame of a reopen");
    harness.run();
    assert_eq!(card(&harness), settled);
}

#[test]
fn esc_in_its_opening_frame_keeps() {
    let mut harness = harness();
    key(&mut harness, Key::Escape, Modifiers::NONE);
    harness.state_mut().answers.clear();
    for _ in 0..3 {
        harness.step();
    }
    harness.state_mut().open = true;
    harness.key_press(Key::Escape);
    harness.step();
    assert_eq!(harness.state().answers, vec![Answer::Keep], "the Esc of the frame it opens in");
    assert!(!harness.state().open);
}

#[test]
fn in_the_least_window_at_130_its_buttons_stay_in_the_dimmed_part_and_its_lines_scroll() {
    // The window's least size, 130 % text, the account's question with a hostile name and address at their caps.
    let theme = Theme::new(Mode::Light, TextScale::ALL[4]);
    let name = "Studio ".repeat(80);
    let address = format!("{}@example.org", "a".repeat(400));
    let question = format!("Remove {name} from this computer?");
    let goes = format!(
        "From this computer go: the mail and folders of {name} kept here, its calendars, its drafts, and its password in your system's credential store."
    );
    let writing = format!("The message you are writing from {name} is discarded with it.");
    let stays = format!("Everything on the server stays: {address} keeps its mail and its calendars, and the account can be added again.");
    let lines = [("This cannot be undone.", Tone::Ink), (goes.as_str(), Tone::Ink), (writing.as_str(), Tone::Ink), (stays.as_str(), Tone::Muted)];
    let asked = Rect::from_min_max(pos2(500.0, 400.0), pos2(640.0, 432.0));
    let mut harness = harness_at(vec2(800.0, 480.0), Theme::new(Mode::Light, TextScale::ALL[4]), Some(asked), &question, &lines);
    let cover = cover(&harness.ctx, &theme);
    let found = filled(&harness, theme.palette.surface);
    let card = *found.iter().find(|rect| (rect.width() - 480.0).abs() < 0.5).unwrap_or_else(|| panic!("no card in {found:?}"));
    assert!(card.top() >= cover.top() + 12.0 && card.bottom() <= cover.bottom() - 12.0, "{card:?} in {cover:?}");
    assert!((card.height() - (cover.height() - 24.0)).abs() <= 1.0, "its lines need more: the card is bound to the dimmed part, {card:?}");
    for label in ["Cancel", "Delete template"] {
        let button = harness.get_by_label(label).rect();
        assert!(cover.contains_rect(button) && card.contains_rect(button), "{label}: {button:?} in {card:?}");
    }
    harness.get_by_label("Delete template").click();
    harness.run();
    assert_eq!(harness.state().answers, vec![Answer::Act], "the danger button takes the owner's press");
}
