// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::accesskit::{Role, Toggled};
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use egui::vec2;

use super::{pill, pill_with};
use crate::component::Shown;
use crate::component::press_tests::{Part, keys_press_as_the_pointer};
use crate::component::tests::{Own, bring, builder, own, painted, staged, theme};
use crate::theme::{Mode, TextScale, Theme};

#[test]
fn a_pill_is_a_selectable_button_with_its_word() {
    let theme = theme();
    let mut harness = builder().build_ui_state(
        move |ui, chosen: &mut usize| {
            for (index, word) in ["All", "Unread", "Flagged"].into_iter().enumerate() {
                if pill(ui, &theme, word, *chosen == index).clicked() {
                    *chosen = index;
                }
            }
        },
        1,
    );
    harness.run();
    let unread = harness.get_by_label("Unread");
    assert_eq!(unread.accesskit_node().role(), Role::Button);
    assert_eq!(unread.accesskit_node().toggled(), Some(Toggled::True));
    harness.get_by_label("Flagged").click();
    harness.run();
    assert_eq!(*harness.state(), 2);
    assert_eq!(harness.get_by_label("Flagged").accesskit_node().toggled(), Some(Toggled::True));
}

#[test]
fn a_pill_is_as_wide_as_its_word_and_round() {
    let theme = theme();
    let mut harness = builder().build_ui(move |ui| {
        pill(ui, &theme, "A", false);
        pill(ui, &theme, "A much longer word", false);
    });
    harness.run();
    let (short, long) = (harness.get_by_label("A").rect(), harness.get_by_label("A much longer word").rect());
    assert!(long.width() > short.width() * 2.0);
    assert_eq!(short.height(), long.height());
    assert!(short.width() >= short.height() * 0.9, "{short:?}");
}

/// The rects of the last frame: their fill and their stroke colour.
fn rects(output: &egui::FullOutput) -> Vec<(egui::Color32, egui::Color32)> {
    fn walk(shape: &egui::Shape, found: &mut Vec<(egui::Color32, egui::Color32)>) {
        match shape {
            egui::Shape::Rect(rect) => found.push((rect.fill, rect.stroke.color)),
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, found)),
            _ => {}
        }
    }
    let mut found = Vec::new();
    output.shapes.iter().for_each(|clipped| walk(&clipped.shape, &mut found));
    found
}

#[test]
fn a_hovered_pill_fades_its_hover_in_and_a_focused_one_shows_its_ring() {
    let theme = theme();
    let palette = theme.palette;
    let mut harness = builder().build_ui(move |ui| {
        pill(ui, &theme, "Unread", false);
    });
    harness.run();
    let hover_fill = |harness: &egui_kittest::Harness<'_>| rects(harness.output()).iter().any(|(fill, _)| *fill == palette.stage);
    let ring = |harness: &egui_kittest::Harness<'_>| rects(harness.output()).iter().any(|(_, stroke)| *stroke == palette.cyan_edge);
    assert!(!hover_fill(&harness) && !ring(&harness));
    harness.get_by_label("Unread").hover();
    harness.run();
    assert!(hover_fill(&harness), "the hover fill is in at full strength once the fade is over");
    assert!(!ring(&harness));
    harness.get_by_label("Unread").focus();
    harness.run();
    assert!(ring(&harness), "a focused pill shows the focus ring");
}

/// The states of the pill's sheet, in its order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Default,
    Hover,
    Focus,
    Selected,
    Disabled,
}

const STATES: [State; 5] = [State::Default, State::Hover, State::Focus, State::Selected, State::Disabled];

fn pill_in(mode: Mode, scale: TextScale, state: State, shown: Shown) -> (egui::Rect, Own) {
    let theme = Theme::new(mode, scale);
    let mut harness = staged(
        theme,
        |p| p.surface,
        vec2(400.0, 100.0),
        move |ui, theme| {
            if state == State::Disabled {
                ui.disable();
            }
            pill(ui, theme, "New senders", state == State::Selected);
        },
    );
    bring(&mut harness, "New senders", shown);
    let rect = harness.get_by_label("New senders").rect();
    (rect, own(&painted(&harness, "New senders"), rect))
}

#[test]
fn a_pill_looks_as_its_sheet_says_in_every_state() {
    for mode in [Mode::Dark, Mode::Light] {
        let p = Theme::new(mode, TextScale::default()).palette;
        for state in STATES {
            let shown = match state {
                State::Hover => Shown::Hover,
                State::Focus => Shown::Focus,
                _ => Shown::Rest,
            };
            let (rect, look) = pill_in(mode, TextScale::default(), state, shown);
            let at = format!("{mode:?} {state:?}: {look:#?}");
            assert!((rect.width() - 89.0).abs() < 0.01 && (rect.height() - 24.0).abs() < 0.01, "{at} {rect:?}");
            let (fill, edge, text) = match state {
                State::Default | State::Focus => (None, (p.line, false), p.muted),
                State::Hover => (Some(p.stage), (p.muted, false), p.ink),
                State::Selected => (Some(p.cyan_muted), (p.cyan_edge, false), p.cyan_text),
                State::Disabled => (None, (p.line, true), p.muted),
            };
            assert_eq!(look.fill, fill, "{at}");
            assert_eq!(look.edge, Some((1.0, edge.0, edge.1)), "{at}");
            assert_eq!(look.texts, vec![("New senders".to_owned(), text, "Monospace".to_owned(), 11.0)], "{at}");
            let ring = (state == State::Focus).then_some((rect.expand(3.0), 1.0, p.cyan_edge));
            assert_eq!(look.ring, ring, "{at}");
            assert!(look.radius <= 0.0 && !look.overflows, "{at}");
        }
    }
}

#[test]
fn a_pill_at_130_percent_is_the_scaled_height() {
    let theme = Theme::new(Mode::Light, TextScale::ALL[4]);
    let (rect, _) = pill_in(Mode::Light, TextScale::ALL[4], State::Default, Shown::Rest);
    assert!((rect.height() - theme.boxes().pill).abs() < 0.01, "{rect:?}");
}

#[test]
fn a_long_pill_ends_at_240_and_its_box_at_256() {
    let theme = Theme::new(Mode::Light, TextScale::default());
    let mut harness = staged(
        theme,
        |p| p.surface,
        vec2(600.0, 100.0),
        |ui, theme| {
            pill(ui, theme, "Every sender who wrote to you for the first time this month or the month before", false);
        },
    );
    harness.run();
    let rect = harness.get_by_label("Every sender who wrote to you for the first time this month or the month before").rect();
    assert!((rect.width() - 256.0).abs() < 0.01, "{rect:?}");
}

#[test]
fn a_pressed_pill_is_the_secondary_press_or_the_accent_wash_with_an_ink_word_when_selected() {
    for mode in [Mode::Dark, Mode::Light] {
        let p = Theme::new(mode, TextScale::default()).palette;
        for (state, fill, edge, text) in [(State::Default, p.stage, p.ink, p.ink), (State::Selected, p.cyan_muted, p.cyan_edge, p.ink)] {
            let (_, look) = pill_in(mode, TextScale::default(), state, Shown::Pressed);
            assert_eq!((look.fill, look.edge), (Some(fill), Some((1.0, edge, false))), "{mode:?} {state:?} {look:#?}");
            assert_eq!(look.texts[0].1, text, "{mode:?} {state:?}");
        }
    }
}

#[test]
fn enter_and_space_press_a_focused_pill() {
    let whole: Part = |rect, _| rect;
    let ring: Part = |rect, _| rect.expand(3.0);
    keys_press_as_the_pointer("New senders", vec2(400.0, 100.0), whole, ring, |ui, theme| drop(pill(ui, theme, "New senders", false)));
    keys_press_as_the_pointer("New senders", vec2(400.0, 100.0), whole, ring, |ui, theme| drop(pill(ui, theme, "New senders", true)));
}

#[test]
fn the_gallery_forces_a_hovered_pill() {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let harness = staged(
        theme,
        |p| p.surface,
        vec2(400.0, 100.0),
        |ui, theme| {
            pill_with(ui, theme, "New senders", false, Some(Shown::Hover));
        },
    );
    let rect = harness.get_by_label("New senders").rect();
    assert_eq!(own(&painted(&harness, "New senders"), rect).fill, Some(theme.palette.stage));
}
