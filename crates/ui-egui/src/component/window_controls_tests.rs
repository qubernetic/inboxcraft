// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::cell::RefCell;
use std::rc::Rc;

use egui::{Color32, Id, Key, Rect, vec2};
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::{CELL, Control, Window, height, spoken, window_controls};
use crate::component::Shown;
use crate::component::tests::{Look, builder, keyboard_walk, painted, rests, stage, staged};
use crate::theme::{Mode, Palette, TextScale, Theme};

/// A state of the sheet: its name, the window, the control forced to hover (if any).
const STATES: [(&str, Window, Option<Control>); 6] = [
    ("default", Window { maximised: false, focused: true }, None),
    ("hover-minimise", Window { maximised: false, focused: true }, Some(Control::Minimise)),
    ("hover-maximise", Window { maximised: false, focused: true }, Some(Control::Maximise)),
    ("hover-close", Window { maximised: false, focused: true }, Some(Control::Close)),
    ("maximised", Window { maximised: true, focused: true }, None),
    ("unfocused", Window { maximised: false, focused: false }, None),
];

/// What one cell painted: its glyph, the glyph's colour and rect, and its fills.
fn cell(harness: &egui_kittest::Harness<'_>, label: &str) -> (Rect, String, Color32, Rect, Vec<Color32>) {
    let rect = harness.get_by_label(label).rect();
    let paint = painted(harness, label);
    let mut glyph = (String::new(), Color32::TRANSPARENT, Rect::NOTHING);
    let mut fills = Vec::new();
    for shape in &paint.shapes {
        match &shape.look {
            Look::Text { text, colour, .. } => glyph = (text.clone(), *colour, shape.rect),
            Look::Rect { fill, .. } if fill.a() > 0 && shape.rect == rect => fills.push(*fill),
            _ => {}
        }
    }
    (rect, glyph.0, glyph.1, glyph.2, fills)
}

#[test]
fn the_window_controls_look_as_their_sheet_says_in_every_state() {
    for scale in [TextScale::default(), TextScale::ALL[4]] {
        for mode in [Mode::Dark, Mode::Light] {
            let theme = Theme::new(mode, scale);
            let p: &Palette = theme.palette;
            for (state, window, hovered) in STATES {
                let forced = Some(hovered.map_or((Control::Minimise, Shown::Rest), |control| (control, Shown::Hover)));
                let harness = staged(
                    theme,
                    |p| p.panel,
                    vec2(200.0, 100.0),
                    move |ui, theme| {
                        window_controls(ui, theme, Id::unique("controls"), window, forced);
                    },
                );
                let high = if scale == TextScale::default() { 32.0 } else { 42.0 };
                assert_eq!(height(&theme), high);
                let mut left = None;
                for (at, control) in (0_u8..).zip(Control::ALL) {
                    let label = spoken(control, window.maximised);
                    let (rect, glyph, ink, glyph_rect, fills) = cell(&harness, label);
                    let what = format!("{mode:?} {}% {state} {label}", scale.percent());
                    let first = *left.get_or_insert(rect.left());
                    assert_eq!(rect, Rect::from_min_size(egui::pos2(first + CELL * f32::from(at), rect.top()), vec2(36.0, high)), "{what}");
                    assert_eq!(rect.min, rect.min.round(), "{what}");
                    let want_glyph = match control {
                        Control::Minimise => "\u{e32a}",
                        Control::Maximise if window.maximised => "\u{e1ca}",
                        Control::Maximise => "\u{e45e}",
                        Control::Close => "\u{e4f6}",
                    };
                    assert_eq!(glyph, want_glyph, "{what}");
                    let hover = hovered == Some(control);
                    let (want_fill, want_ink) = match (hover, control) {
                        (true, Control::Close) => (vec![p.minus], p.bg),
                        (true, _) => (vec![p.grid], p.ink),
                        (false, _) => (vec![], if window.focused { p.ink } else { p.muted }),
                    };
                    assert_eq!(fills, want_fill, "{what}");
                    assert_eq!(ink, want_ink, "{what}");
                    // The glyph stands in the middle of its cell.
                    assert!((glyph_rect.center() - rect.center()).length() <= 2.0, "{what}: {glyph_rect:?} in {rect:?}");
                }
            }
        }
    }
}

/// A harness with the controls live, keeping what they returned.
fn live(window: Window) -> (egui_kittest::Harness<'static>, Rc<RefCell<Vec<Control>>>) {
    let theme = Theme::new(Mode::Light, TextScale::default());
    let pressed = Rc::new(RefCell::new(Vec::new()));
    let kept = Rc::clone(&pressed);
    let mut harness = builder().with_size(vec2(200.0, 100.0)).build_ui(move |ui| {
        crate::motion::set_reduced(ui.ctx(), true);
        stage(ui, theme.palette.panel, |ui| {
            if let Some(control) = window_controls(ui, &theme, Id::unique("controls"), window, None) {
                kept.borrow_mut().push(control);
            }
        });
    });
    harness.run();
    (harness, pressed)
}

#[test]
fn tab_walks_the_controls_in_order_and_enter_and_space_press_them() {
    for focused in [true, false] {
        let window = Window { maximised: true, focused };
        let (mut harness, pressed) = live(window);
        assert!(keyboard_walk(&mut harness).is_empty());
        // From nothing: Tab to the first, then on, and Shift Tab back.
        let (mut harness, pressed2) = live(window);
        let mut order = Vec::new();
        for _ in 0..3 {
            harness.key_press(Key::Tab);
            harness.run();
            let on = Control::ALL.into_iter().find(|control| harness.get_by_label(spoken(*control, true)).accesskit_node().is_focused());
            order.push(on);
        }
        assert_eq!(order, Control::ALL.map(Some).to_vec(), "focused {focused}");
        harness.key_press_modifiers(egui::Modifiers::SHIFT, Key::Tab);
        harness.run();
        assert!(harness.get_by_label(spoken(Control::Maximise, true)).accesskit_node().is_focused());
        harness.key_press(Key::Enter);
        harness.run();
        harness.key_press(Key::Tab);
        harness.run();
        harness.key_press(Key::Space);
        harness.run();
        assert_eq!(*pressed2.borrow(), vec![Control::Maximise, Control::Close], "focused {focused}: a window behind still works");
        assert!(pressed.borrow().is_empty(), "the walk pressed nothing");
    }
}

#[test]
fn a_click_returns_its_control_and_the_focus_ring_stands_inside() {
    let (mut harness, pressed) = live(Window { maximised: false, focused: true });
    harness.get_by_label(spoken(Control::Close, false)).click();
    harness.run();
    harness.get_by_label(spoken(Control::Minimise, false)).click();
    harness.run();
    assert_eq!(*pressed.borrow(), vec![Control::Close, Control::Minimise]);
    harness.key_press(Key::Tab);
    harness.run();
    let label = spoken(Control::Minimise, false);
    let rect = harness.get_by_label(label).rect();
    let ring = painted(&harness, label).rects().find(|shape| matches!(shape.look, Look::Rect { stroke: Some((_, colour)), .. } if colour == Theme::new(Mode::Light, TextScale::default()).palette.cyan_edge)).map(|shape| shape.rect);
    let Some(ring) = ring else { panic!("no focus ring") };
    assert!(rect.contains_rect(ring), "{ring:?} inside {rect:?}");
}

#[test]
fn the_window_controls_rest() {
    let (mut harness, _) = live(Window { maximised: false, focused: true });
    harness.get_by_label(spoken(Control::Close, false)).hover();
    assert!(rests(&mut harness, 8));
}

#[test]
fn the_controls_say_what_the_title_bar_says() {
    assert_eq!(spoken(Control::Minimise, false), "Minimise");
    assert_eq!(spoken(Control::Maximise, false), "Maximise");
    assert_eq!(spoken(Control::Maximise, true), "Restore");
    assert_eq!(spoken(Control::Close, false), "Close the window", "one name for closing the window, not another Close");
}
