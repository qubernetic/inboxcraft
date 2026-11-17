// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::accesskit::Role;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::{count, section_label_with};
use crate::component::Shown;
use crate::component::tests::{Look, bring, own, painted, staged};
use crate::theme::{Mode, TextScale, Theme};

#[test]
fn counts_group_and_cap() {
    assert_eq!(count(0), "");
    assert_eq!(count(7), "7");
    assert_eq!(count(1204), "1\u{202f}204");
    assert_eq!(count(9999), "9\u{202f}999");
    assert_eq!(count(10_000), "9\u{202f}999+");
    assert_eq!(count(u64::MAX), "9\u{202f}999+");
}

/// The label "[ 01 ] Mailboxes", `active` or not, folding (and so hoverable) or not, brought into `shown`.
fn label_in(mode: Mode, scale: TextScale, active: bool, folds: bool, shown: Shown) -> (egui::Rect, crate::component::tests::Own, Role) {
    let theme = Theme::new(mode, scale);
    let mut harness = staged(
        theme,
        |p| p.panel,
        egui::vec2(300.0, 100.0),
        move |ui, theme| {
            section_label_with(ui, theme, "[ 01 ]", "Mailboxes", active, folds, None);
        },
    );
    let name = "[ 01 ] Mailboxes";
    bring(&mut harness, name, shown);
    let node = harness.get_by_label(name);
    let rect = node.rect();
    (rect, own(&painted(&harness, name), rect), node.accesskit_node().role())
}

#[test]
fn a_section_label_looks_as_its_sheet_says_in_every_state() {
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::default());
        let p = theme.palette;
        let font = theme.fonts.label;
        let family = font.id().family.to_string();
        // default, active, hover (a label that folds its section).
        for (state, active, folds, shown) in
            [("default", false, false, Shown::Rest), ("active", true, false, Shown::Rest), ("hover", false, true, Shown::Hover)]
        {
            let (rect, look, role) = label_in(mode, TextScale::default(), active, folds, shown);
            let at = format!("{mode:?} {state}: {look:#?}");
            assert!((rect.width() - 119.0).abs() <= 1.0 && (rect.height() - 16.0).abs() < 0.01, "{at} {rect:?}");
            assert_eq!(role, if folds { Role::Button } else { Role::Label }, "{at}");
            let word = if state == "default" { p.muted } else { p.ink };
            assert_eq!(
                look.texts,
                vec![("[ 01 ]".to_owned(), p.cyan_text, family.clone(), font.size), ("MAILBOXES".to_owned(), word, family.clone(), font.size)],
                "{at}"
            );
            assert_eq!(look.edge, None, "{at}");
            assert_eq!(look.ring, None, "{at}");
            assert!(!look.overflows, "{at}");
            assert!(look.radius <= 0.0, "{at}");
        }
    }
}

#[test]
fn a_hovered_section_label_stands_on_the_grid_fill_4_and_2_round_it() {
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::default());
        let mut harness = staged(
            theme,
            |p| p.panel,
            egui::vec2(300.0, 100.0),
            move |ui, theme| {
                section_label_with(ui, theme, "[ 01 ]", "Mailboxes", false, true, None);
            },
        );
        let name = "[ 01 ] Mailboxes";
        bring(&mut harness, name, Shown::Hover);
        let rect = harness.get_by_label(name).rect();
        let paint = painted(&harness, name);
        let fill = paint.rects().find(|shape| matches!(shape.look, crate::component::tests::Look::Rect { fill, .. } if fill == theme.palette.grid));
        assert_eq!(fill.map(|shape| shape.rect), Some(rect.expand2(egui::vec2(4.0, 2.0))), "{mode:?} {paint:#?}");
    }
}

#[test]
fn a_section_label_at_130_percent_is_one_scaled_line() {
    let theme = Theme::new(Mode::Light, TextScale::ALL[4]);
    let (rect, ..) = label_in(Mode::Light, TextScale::ALL[4], false, false, Shown::Rest);
    assert!((rect.height() - theme.fonts.label.line.ceil()).abs() < 0.01, "{rect:?}");
}

#[test]
fn a_pressed_folding_section_label_takes_a_muted_edge_round_the_hover_fill() {
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::default());
        let p = theme.palette;
        let name = "[ 01 ] Mailboxes";
        let looks: Vec<_> = [Shown::Rest, Shown::Hover, Shown::Pressed]
            .into_iter()
            .map(|shown| {
                let mut harness = staged(
                    theme,
                    |p| p.panel,
                    egui::vec2(300.0, 100.0),
                    move |ui, theme| {
                        section_label_with(ui, theme, "[ 01 ]", "Mailboxes", false, true, None);
                    },
                );
                bring(&mut harness, name, shown);
                let rect = harness.get_by_label(name).rect();
                let paint = painted(&harness, name);
                let reach = rect.expand2(egui::vec2(4.0, 2.0));
                let fill = paint.rects().find(|shape| matches!(shape.look, Look::Rect { fill, .. } if fill == p.grid)).map(|shape| shape.rect);
                let edge = paint.rects().find_map(|shape| match shape.look {
                    Look::Rect { stroke: Some((width, colour)), stroke_kind: egui::StrokeKind::Inside, dashed: false, .. } if shape.rect == reach => {
                        Some((width, colour))
                    }
                    _ => None,
                });
                let word = own(&paint, rect).texts.iter().find(|(text, ..)| text == "MAILBOXES").map(|(_, colour, ..)| *colour);
                (fill, edge, word, reach)
            })
            .collect();
        let (rest, hovered, pressed) = (&looks[0], &looks[1], &looks[2]);
        assert_eq!(pressed.0, Some(pressed.3), "{mode:?} the grid fill");
        assert_eq!(pressed.1, Some((1.0, p.muted)), "{mode:?} the muted edge");
        assert_eq!(pressed.2, Some(p.ink), "{mode:?}");
        assert_ne!(pressed, hovered, "{mode:?}");
        assert_ne!(pressed, rest, "{mode:?}");
    }
}

#[test]
fn enter_and_space_press_a_focused_folding_label_and_its_ring_stands_round_the_reach() {
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::default());
        let p = theme.palette;
        let name = "[ 01 ] Mailboxes";
        for key in [None, Some(egui::Key::Enter), Some(egui::Key::Space)] {
            let mut harness = staged(
                theme,
                |p| p.panel,
                egui::vec2(300.0, 100.0),
                move |ui, theme| {
                    section_label_with(ui, theme, "[ 01 ]", "Mailboxes", false, true, None);
                },
            );
            bring(&mut harness, name, Shown::Focus);
            if let Some(key) = key {
                harness.key_down(key);
                harness.run();
            }
            let rect = harness.get_by_label(name).rect();
            let reach = rect.expand2(egui::vec2(4.0, 2.0));
            let paint = painted(&harness, name);
            // The ring stands past the 4 points `painted` looks round the label: read it from the whole frame.
            let mut strokes = Vec::new();
            for clipped in &harness.output().shapes {
                let mut flat = vec![&clipped.shape];
                while let Some(shape) = flat.pop() {
                    match shape {
                        egui::Shape::Vec(shapes) => flat.extend(shapes.iter()),
                        egui::Shape::Rect(shape) if shape.stroke.width > 0.0 && reach.expand(4.0).contains_rect(shape.rect) => {
                            strokes.push((shape.rect, shape.stroke.width, shape.stroke.color, shape.stroke_kind));
                        }
                        _ => {}
                    }
                }
            }
            let at = format!("{mode:?} {key:?}: {strokes:?} {rect:?}");
            assert!(strokes.contains(&(reach.expand(2.0), 1.0, p.cyan_edge, egui::StrokeKind::Outside)), "the ring, 2 px clear of the reach: {at}");
            let edge = strokes.contains(&(reach, 1.0, p.muted, egui::StrokeKind::Inside));
            let fill = paint.rects().any(|shape| shape.rect == reach && matches!(shape.look, Look::Rect { fill, .. } if fill == p.grid));
            assert_eq!((edge, fill), (key.is_some(), key.is_some()), "pressed only while the key is down: {at}");
        }
    }
}
