// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::accesskit::Role;
use egui::{Rect, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::{Recipient, RecipientState, Recipients, recipient_chip, recipient_field, recipient_line};
use crate::component::Shown;
use crate::component::field::tests::{caret, drawn, one};
use crate::component::tests::{Own, bring, own, painted, staged};
use crate::iconset::IconId;
use crate::theme::{Mode, TextScale, Theme};

/// The states of the recipient chip's sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Chip {
    Valid,
    Invalid,
    Hover,
    Focus,
    Long,
    Disabled,
}

const CHIPS: [Chip; 6] = [Chip::Valid, Chip::Invalid, Chip::Hover, Chip::Focus, Chip::Long, Chip::Disabled];

fn chip_name(state: Chip) -> &'static str {
    match state {
        Chip::Invalid => "teodor@",
        Chip::Long => "Bakery on the corner, orders and deliveries",
        _ => "Teodor Vasquez",
    }
}

/// The chip of the sheet in `state` on a `term` stage.
fn chip_in(mode: Mode, state: Chip, shown: Shown) -> (Rect, Own, Harness<'static>) {
    let theme = Theme::new(mode, TextScale::default());
    let name = chip_name(state);
    let mut harness = staged(
        theme,
        |p| p.term,
        vec2(400.0, 100.0),
        move |ui, theme| {
            if state == Chip::Disabled {
                ui.disable();
            }
            let kind = if state == Chip::Invalid { RecipientState::Invalid } else { RecipientState::Valid };
            recipient_chip(ui, theme, name, kind);
        },
    );
    match state {
        Chip::Hover => bring(&mut harness, name, Shown::Hover),
        Chip::Focus => bring(&mut harness, &format!("Remove {name}"), Shown::Focus),
        _ => bring(&mut harness, &format!("Remove {name}"), shown),
    }
    let rect = harness.get_by_role_and_label(Role::ListItem, name).rect();
    (rect, own(&painted(&harness, name), rect), harness)
}

#[test]
fn a_recipient_chip_looks_as_its_sheet_says_in_every_state() {
    for mode in [Mode::Dark, Mode::Light] {
        let p = Theme::new(mode, TextScale::default()).palette;
        for state in CHIPS {
            let (rect, look, harness) = chip_in(mode, state, Shown::Rest);
            let at = format!("{mode:?} {state:?}: {look:#?}");
            let (fill, edge, ink, cross) = match state {
                Chip::Valid | Chip::Long => (Some(p.grid), (p.line, false), p.ink, p.muted),
                Chip::Invalid => (Some(p.grid), (p.minus, false), p.minus, p.muted),
                Chip::Hover => (Some(p.stage), (p.muted, false), p.ink, p.ink),
                Chip::Focus => (Some(p.grid), (p.cyan_edge, false), p.ink, p.muted),
                Chip::Disabled => (None, (p.line, true), p.muted, p.dim),
            };
            assert_eq!(look.fill, fill, "{at}");
            assert_eq!(look.edge, Some((1.0, edge.0, edge.1)), "{at}");
            assert!(look.ring.is_none() && look.radius <= 0.0, "{at}");
            let name = one(&harness, chip_name(state));
            let lead = if state == Chip::Invalid { 27.0 } else { 9.0 };
            assert_eq!((name.rect.min, name.colour, name.size, name.row), (rect.min + vec2(lead, 1.0), ink, 14.0, 22.0), "{at} {name:?}");
            let width = if state == Chip::Long { 176.0 } else { (lead + name.rect.width() + 23.0).ceil() };
            assert_eq!(rect.size(), vec2(width, 24.0), "{at}");
            let x = one(&harness, &crate::iconset::glyph(IconId::Close).to_string());
            assert!((x.rect.center() - (rect.right_top() + vec2(-12.0, 12.0))).length() < 1.0 && x.colour == cross, "{at} {x:?}");
            let warning: Vec<_> = drawn(&harness).into_iter().filter(|text| text.text == "\u{e4e0}").collect();
            if state == Chip::Invalid {
                assert!(
                    warning.len() == 1 && (warning[0].rect.center() - (rect.min + vec2(16.0, 12.0))).length() < 1.0 && warning[0].colour == p.minus,
                    "{at}"
                );
            } else {
                assert!(warning.is_empty(), "{at}");
            }
            if state == Chip::Long {
                assert!(name.rect.right() <= rect.left() + 153.0 + 0.5, "{at}: cut 4 before the cross");
            }
        }
    }
}

#[test]
fn a_recipient_chip_pressed_by_the_pointer_enter_or_space_is_the_secondary_press() {
    for mode in [Mode::Dark, Mode::Light] {
        let p = Theme::new(mode, TextScale::default()).palette;
        let (_, look, _) = chip_in(mode, Chip::Valid, Shown::Pressed);
        assert_eq!((look.fill, look.edge), (Some(p.stage), Some((1.0, p.ink, false))), "{mode:?}");
        for key in [egui::Key::Enter, egui::Key::Space] {
            let theme = Theme::new(mode, TextScale::default());
            let mut harness = staged(
                theme,
                |p| p.term,
                vec2(400.0, 100.0),
                |ui, theme| {
                    recipient_chip(ui, theme, "Teodor Vasquez", RecipientState::Valid);
                },
            );
            bring(&mut harness, "Remove Teodor Vasquez", Shown::Focus);
            harness.key_down(key);
            harness.run();
            let rect = harness.get_by_role_and_label(Role::ListItem, "Teodor Vasquez").rect();
            let held = own(&painted(&harness, "Teodor Vasquez"), rect);
            assert_eq!((held.fill, held.edge), (look.fill, look.edge), "{mode:?} {key:?}");
        }
    }
}

/// The states of the recipient field's sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Line {
    Empty,
    Focus,
    Typing,
    WithChips,
    Invalid,
    Disabled,
    Wrapped,
}

const LINES: [Line; 7] = [Line::Empty, Line::Focus, Line::Typing, Line::WithChips, Line::Invalid, Line::Disabled, Line::Wrapped];

fn people(names: &[&str]) -> Vec<Recipient> {
    names
        .iter()
        .map(|name| {
            let state = if name.ends_with('@') { RecipientState::Invalid } else { RecipientState::Valid };
            Recipient { shown: (*name).to_owned(), spoken: (*name).to_owned(), state }
        })
        .collect()
}

/// The recipient field of the sheet, 560 wide, in `state`: its box, look, paint and harness.
fn line_in(mode: Mode, state: Line) -> (Rect, Own, crate::component::tests::Paint, Harness<'static>) {
    let theme = Theme::new(mode, TextScale::default());
    let names: &[&str] = match state {
        Line::WithChips => &["Teodor Vasquez", "Mirela Anton"],
        Line::Invalid => &["Mirela Anton", "teodor@"],
        Line::Disabled => &["Teodor Vasquez"],
        Line::Wrapped => &["Teodor Vasquez", "Mirela Anton", "Pavel Hruska", "Bakery on the corner", "Ines Okafor", "Studio, everyone"],
        _ => &[],
    };
    let chips = people(names);
    let mut harness = staged(
        theme,
        |p| p.bg,
        vec2(640.0, 200.0),
        move |ui, theme| {
            let id = egui::Id::unique("to");
            let mut typing = ui.data_mut(|data| {
                data.get_temp_mut_or_insert_with(id.with("t"), || if state == Line::Typing { "teo".to_owned() } else { String::new() }).clone()
            });
            let line = Recipients {
                name: "To",
                chips: &chips,
                hint: "Name or address",
                copies: true,
                error: (state == Line::Invalid).then_some("teodor@ is not an address"),
                enabled: state != Line::Disabled,
            };
            ui.vertical(|ui| {
                ui.set_width(560.0);
                recipient_field(ui, theme, id, &mut typing, &line);
            });
            ui.data_mut(|data| data.insert_temp(id.with("t"), typing));
        },
    );
    if matches!(state, Line::Focus | Line::Typing | Line::Wrapped) {
        bring(&mut harness, "To", Shown::Focus);
        harness.key_press(egui::Key::End);
        harness.run();
    }
    let node = harness.get_by_role_and_label(Role::TextInput, "To").rect();
    let height = if state == Line::Wrapped { 72.0 } else { 36.0 };
    // The stage's 16 and the harness's own 8: the box's top left corner, 45 left of where the chips start.
    let rect = Rect::from_min_size(pos2(24.0, 24.0), vec2(560.0, height));
    assert!(node.left() >= rect.left() + 45.0, "{node:?}");
    let paint = crate::component::tests::painted_in(&harness, "To", rect);
    (rect, own(&paint, rect), paint, harness)
}

#[test]
fn a_recipient_field_looks_as_its_sheet_says_in_every_state() {
    for mode in [Mode::Dark, Mode::Light] {
        let p = Theme::new(mode, TextScale::default()).palette;
        for state in LINES {
            let (rect, look, paint, harness) = line_in(mode, state);
            let at = format!("{mode:?} {state:?}: {look:#?}");
            let (fill, edge) = match state {
                Line::Empty | Line::WithChips => (Some(p.term), (p.line, false)),
                Line::Focus | Line::Typing | Line::Wrapped => (Some(p.term), (p.cyan_edge, false)),
                Line::Invalid => (Some(p.term), (p.minus, false)),
                Line::Disabled => (None, (p.line, true)),
            };
            assert_eq!(look.fill, fill, "{at}");
            assert_eq!(look.edge, Some((1.0, edge.0, edge.1)), "{at}");
            // The name 12 in, the first row's middle 18 down; "Cc Bcc" at the right end.
            let name = one(&harness, "TO");
            let first = if state == Line::Wrapped { 19.0 } else { 18.0 };
            assert!(name.rect.left() == rect.left() + 13.0 && (name.rect.center().y - (rect.top() + first)).abs() < 0.01, "{at} {name:?}");
            assert_eq!((name.colour, name.size), (p.muted, 11.0), "{at}");
            // "Cc Bcc" at the right end; a disabled line offers none, as its board.
            if state == Line::Disabled {
                assert!(drawn(&harness).iter().all(|text| text.text != "Cc Bcc"), "{at}");
            } else {
                let copies = one(&harness, "Cc Bcc");
                assert!(
                    (copies.rect.right() - (rect.right() - 13.0)).abs() < 0.5 && copies.colour == p.muted && copies.size == 11.0,
                    "{at} {copies:?}"
                );
            }
            let block = caret(&paint, p.cyan_edge);
            match state {
                Line::Empty | Line::Focus => {
                    let hint = one(&harness, "Name or address");
                    let left = if state == Line::Focus { 56.0 } else { 45.0 };
                    assert_eq!((hint.rect.min, hint.colour), (rect.min + vec2(left, 7.0), p.muted), "{at}");
                    let expected = (state == Line::Focus).then_some(Rect::from_min_size(rect.min + vec2(45.0, 10.0), vec2(7.0, 16.0)));
                    assert_eq!(block, expected, "{at}");
                }
                Line::Typing => {
                    let typed = one(&harness, "teo");
                    assert_eq!((typed.rect.min, typed.colour), (rect.min + vec2(45.0, 7.0), p.ink), "{at}");
                    assert_eq!(
                        block,
                        Some(Rect::from_min_size(pos2(typed.rect.left() + typed.rect.width().ceil() + 2.0, rect.top() + 10.0), vec2(7.0, 16.0))),
                        "{at}"
                    );
                }
                _ => {}
            }
            let chips: Vec<_> =
                harness.query_all_by_role(Role::ListItem).map(|node| (node.accesskit_node().label().unwrap_or_default(), node.rect())).collect();
            match state {
                Line::WithChips | Line::Invalid | Line::Disabled => {
                    assert_eq!(chips.first().map(|(_, rect)| rect.min), Some(rect.min + vec2(45.0, 6.0)), "{at} {chips:?}");
                    if let [(_, a), (_, b), ..] = chips.as_slice() {
                        assert_eq!(b.left(), a.right() + 4.0, "{at}: 4 between chips");
                    }
                }
                Line::Wrapped => {
                    assert_eq!(chips.len(), 6, "{at}");
                    assert_eq!(chips.first().map(|(_, rect)| rect.min), Some(rect.min + vec2(45.0, 7.0)), "{at}");
                    let second = chips.iter().filter(|(_, chip)| chip.top() == rect.top() + 41.0).count();
                    assert!(second >= 1, "{at}: a second row 34 under the first {chips:?}");
                    assert_eq!(rect.height(), 72.0, "{at}");
                }
                _ => assert!(chips.is_empty(), "{at}"),
            }
            if state == Line::Invalid {
                let message = one(&harness, "teodor@ is not an address");
                assert_eq!((message.rect.min, message.colour), (rect.left_bottom() + vec2(0.0, 8.0), p.minus), "{at}");
            }
        }
    }
}

#[test]
fn a_recipient_line_takes_typing_removes_a_chip_and_opens_cc_bcc() {
    let theme = Theme::new(Mode::Light, TextScale::default());
    let mut harness = crate::component::tests::builder().with_size(vec2(640.0, 200.0)).build_ui_state(
        move |ui, (typing, removed, copies): &mut (String, Option<usize>, bool)| {
            let chips = people(&["Teodor Vasquez"]);
            let line = Recipients { name: "To", chips: &chips, hint: "Name or address", copies: true, error: None, enabled: true };
            let out = recipient_field(ui, &theme, egui::Id::unique("to"), typing, &line);
            *removed = removed.or(out.removed);
            *copies |= out.copies;
        },
        (String::new(), None, false),
    );
    harness.run();
    harness.get_by_role_and_label(Role::TextInput, "To").focus();
    harness.run();
    harness.get_by_role_and_label(Role::TextInput, "To").type_text("odile");
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Remove Teodor Vasquez").click();
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Cc  Bcc").click();
    harness.run();
    assert_eq!(*harness.state(), ("odile".to_owned(), Some(0), true));
}

/// A recipient line of `chips` on a stage, 560 wide; its box's height read from the drawn `term` fill.
fn line_of(chips: Vec<Recipient>) -> Harness<'static> {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    staged(
        theme,
        |p| p.bg,
        vec2(640.0, 400.0),
        move |ui, theme| {
            ui.spacing_mut().scroll = crate::component::scroll::style(theme);
            let mut typing = String::new();
            let line = Recipients { name: "To", chips: &chips, hint: "Name or address", copies: true, error: None, enabled: true };
            ui.vertical(|ui| {
                ui.set_width(560.0);
                recipient_field(ui, theme, egui::Id::unique("to"), &mut typing, &line);
            });
        },
    )
}

#[test]
fn two_chips_of_one_address_are_two_controls() {
    let same =
        Recipient { shown: "Teodor Vasquez".to_owned(), spoken: "Teodor Vasquez, teodor@example.org".to_owned(), state: RecipientState::Valid };
    let harness = line_of(vec![same.clone(), same]);
    let crosses: Vec<_> = harness.query_all_by_label("Remove Teodor Vasquez").map(|node| node.accesskit_node().locate().0).collect();
    assert_eq!(crosses.len(), 2);
    assert_ne!(crosses[0], crosses[1], "one id each");
}

#[test]
fn five_hundred_recipients_stay_in_a_box_of_four_rows_that_scrolls() {
    let many: Vec<Recipient> = (0..500)
        .map(|n| Recipient { shown: format!("person{n}@example.org"), spoken: format!("person{n}@example.org"), state: RecipientState::Valid })
        .collect();
    let clock = inboxcraft_time::SystemClock::new();
    let started = inboxcraft_time::Clock::monotonic(&clock);
    let harness = line_of(many);
    assert!(inboxcraft_time::Clock::monotonic(&clock).saturating_sub(started) < std::time::Duration::from_secs(5));
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let boxes: Vec<Rect> = {
        fn walk(shape: &egui::Shape, found: &mut Vec<Rect>, fill: egui::Color32) {
            match shape {
                egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, found, fill)),
                egui::Shape::Rect(rect) if rect.fill == fill && rect.rect.width() > 500.0 => found.push(rect.rect),
                _ => {}
            }
        }
        let mut found = Vec::new();
        harness.output().shapes.iter().for_each(|clipped| walk(&clipped.shape, &mut found, theme.palette.term));
        found
    };
    assert_eq!(boxes.len(), 1, "{boxes:?}");
    // Four rows of 24, 10 apart, and 7 above and below.
    assert_eq!(boxes[0].height(), 4.0 * 24.0 + 3.0 * 10.0 + 14.0, "{boxes:?}");
    let thumb =
        drawn_fills(&harness).into_iter().any(|(rect, fill)| fill == theme.palette.muted && rect.width() == 4.0 && boxes[0].contains_rect(rect));
    assert!(thumb, "the slice's scroll bar");
}

/// The filled rects the last frame painted.
fn drawn_fills(harness: &Harness<'static>) -> Vec<(Rect, egui::Color32)> {
    fn walk(shape: &egui::Shape, found: &mut Vec<(Rect, egui::Color32)>) {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, found)),
            egui::Shape::Rect(rect) if rect.fill.a() > 0 => found.push((rect.rect, rect.fill)),
            _ => {}
        }
    }
    let mut found = Vec::new();
    harness.output().shapes.iter().for_each(|clipped| walk(&clipped.shape, &mut found));
    found
}

/// `n` recipients `person<k>@example.org`, all addresses.
fn persons(n: usize) -> Vec<Recipient> {
    (0..n)
        .map(|k| Recipient { shown: format!("person{k}@example.org"), spoken: format!("person{k}@example.org"), state: RecipientState::Valid })
        .collect()
}

/// How many shapes the last frame painted, groups opened.
fn shape_count(harness: &Harness<'static>) -> usize {
    fn count(shape: &egui::Shape) -> usize {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().map(count).sum(),
            _ => 1,
        }
    }
    harness.output().shapes.iter().map(|clipped| count(&clipped.shape)).sum()
}

#[test]
fn every_chip_of_a_long_line_is_reached_by_tab_and_heard_though_scrolled_out() {
    let mut harness = line_of(persons(40));
    // A screen reader hears every chip and every cross, in view or not.
    assert_eq!(harness.query_all_by_role(Role::ListItem).count(), 40);
    for k in 0..40 {
        assert_eq!(harness.query_all_by_label(&format!("Remove person{k}@example.org")).count(), 1, "cross {k}");
    }
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let field = drawn_fills(&harness).into_iter().find(|(rect, fill)| *fill == theme.palette.term && rect.width() > 500.0).map(|(rect, _)| rect);
    let Some(field) = field else { panic!("the line's box") };
    // Tab goes through every cross in order, and each one it reaches is scrolled into the box.
    for k in 0..40 {
        harness.key_press(egui::Key::Tab);
        harness.run();
        let label = format!("Remove person{k}@example.org");
        let cross = harness.get_by_label(&label);
        assert!(cross.accesskit_node().is_focused(), "{label} has the keyboard");
        let rect = cross.rect();
        assert!(field.contains_rect(rect), "{label} at {rect:?} is in the box {field:?}");
    }
}

#[test]
fn a_long_line_paints_only_the_rows_in_view() {
    let few = shape_count(&line_of(persons(40)));
    let many = shape_count(&line_of(persons(400)));
    assert_eq!(few, many, "the shapes a frame paints do not grow with the chips out of view");
}

/// A composer's recipient line of `chips` on the composer's `surface`, 684 wide (the docked frame's lines).
fn composer_line(chips: Vec<Recipient>, focus: bool) -> Harness<'static> {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let mut harness = staged(
        theme,
        |p| p.surface,
        vec2(760.0, 400.0),
        move |ui, theme| {
            ui.spacing_mut().scroll = crate::component::scroll::style(theme);
            let mut typing = String::new();
            let line = Recipients { name: "To", chips: &chips, hint: "Name or address", copies: true, error: None, enabled: true };
            ui.vertical(|ui| {
                ui.set_width(684.0);
                recipient_line(ui, theme, egui::Id::unique("to"), &mut typing, &line);
            });
        },
    );
    if focus {
        harness.ctx.memory_mut(|memory| memory.request_focus(egui::Id::unique("to")));
        harness.run();
    }
    harness
}

#[test]
fn a_composer_line_has_no_box_its_name_in_a_column_of_64_and_cc_bcc_at_its_end() {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    for focus in [false, true] {
        let harness = composer_line(people(&["Cosima Reuter"]), focus);
        assert!(!drawn_fills(&harness).iter().any(|(rect, fill)| *fill == theme.palette.term && rect.width() > 500.0), "no box on `term`");
        let chip = harness.get_by_role_and_label(Role::ListItem, "Cosima Reuter").rect();
        let copies = harness.get_by_role_and_label(Role::Button, "Cc  Bcc").rect();
        let typing = harness.get_by_role_and_label(Role::TextInput, "To").rect();
        // The chips 76 in (64 of names and 12), 6 under the line's top; Cc Bcc at its right end.
        let left = copies.right() - 684.0;
        assert!((chip.left() - left - 76.0).abs() < 0.5, "{chip:?} 76 in from {left}");
        assert!(typing.left() > chip.right(), "the typing after the chip");
        assert_eq!(typing.height(), 36.0, "a line of 36: {typing:?}");
        assert!((chip.top() - typing.top() - 6.0).abs() < 0.5, "{chip:?} in {typing:?}");
    }
}

#[test]
fn a_composer_line_wraps_its_chips_four_apart_and_grows() {
    let names: Vec<String> = (0..12).map(|n| format!("Person number {n}")).collect();
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let harness = composer_line(people(&names), false);
    let rows: std::collections::BTreeSet<i32> = harness.query_all_by_role(Role::ListItem).map(|chip| chip.rect().top().round() as i32).collect();
    let rows: Vec<i32> = rows.into_iter().collect();
    assert!(rows.len() > 1, "wrapped: {rows:?}");
    assert_eq!(rows[1] - rows[0], 28, "24 high, 4 apart: {rows:?}");
}
