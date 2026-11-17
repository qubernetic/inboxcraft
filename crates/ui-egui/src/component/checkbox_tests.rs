// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::accesskit::{Role, Toggled};
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use egui::{Rect, pos2, vec2};

use super::{checkbox, checkbox_with, radio, radio_with};
use crate::component::Shown;
use crate::component::press_tests::keys_press_as_the_pointer;
use crate::component::tests::{Look, Own, bring, builder, own, painted, staged, theme};
use crate::motion;
use crate::theme::{Mode, TextScale, Theme};

#[test]
fn a_check_box_is_named_says_whether_it_is_ticked_and_flips_when_pressed() {
    let theme = theme();
    let mut harness = builder().build_ui_state(
        move |ui, on: &mut bool| {
            motion::set_reduced(ui.ctx(), true);
            if checkbox(ui, &theme, "Private", *on).clicked() {
                *on = !*on;
            }
        },
        false,
    );
    harness.run();
    let node = harness.get_by_role_and_label(Role::CheckBox, "Private");
    assert_eq!(node.accesskit_node().toggled(), Some(Toggled::False));
    node.click();
    harness.run();
    assert!(*harness.state());
    assert_eq!(harness.get_by_role_and_label(Role::CheckBox, "Private").accesskit_node().toggled(), Some(Toggled::True));
}

#[test]
fn a_radio_button_is_named_and_says_whether_it_is_chosen() {
    let theme = theme();
    let mut harness = builder().build_ui_state(
        move |ui, choice: &mut bool| {
            motion::set_reduced(ui.ctx(), true);
            if radio(ui, &theme, "Don't tell them", !*choice).clicked() {
                *choice = false;
            }
            if radio(ui, &theme, "Send them the update", *choice).clicked() {
                *choice = true;
            }
        },
        false,
    );
    harness.run();
    let quiet = harness.get_by_role_and_label(Role::RadioButton, "Don't tell them");
    assert_eq!(quiet.accesskit_node().toggled(), Some(Toggled::True));
    harness.get_by_role_and_label(Role::RadioButton, "Send them the update").click();
    harness.run();
    assert!(*harness.state());
}

#[test]
fn the_name_is_cut_in_too_little_room_and_the_box_keeps_its_size() {
    let theme = theme();
    let mut harness = builder().build_ui(move |ui| {
        ui.set_max_width(60.0);
        checkbox(ui, &theme, "A calendar with a very long name indeed", true);
        checkbox(ui, &theme, "Short", false);
    });
    harness.run();
    let long = harness.get_by_role_and_label(Role::CheckBox, "A calendar with a very long name indeed").rect();
    assert!(long.width() <= 61.0 && long.height() >= 18.0, "{long:?}");
}

/// The states of the check box's sheet, in its order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Check {
    Off,
    On,
    OffHover,
    OnHover,
    Focus,
    Disabled,
}

const CHECKS: [Check; 6] = [Check::Off, Check::On, Check::OffHover, Check::OnHover, Check::Focus, Check::Disabled];

/// The box of a check box or radio button whose row is `row`: at its left, centred on the row's height.
fn boxed(row: Rect, theme: &Theme) -> Rect {
    let side = theme.size(16.0);
    Rect::from_min_size(pos2(row.left(), row.center().y - side / 2.0), vec2(side, side))
}

/// The ring round [`boxed`]: 2 clear, its 1 px line outside.
fn ring(row: Rect, theme: &Theme) -> Rect {
    boxed(row, theme).expand(3.0)
}

/// `label` drawn by `draw` (on or off, disabled or not) on a `surface` stage in `mode` at `scale`, brought into
/// `shown`: its row, its box and the box's look (the texts are the whole row's).
fn toggle_in(mode: Mode, scale: TextScale, label: &'static str, shown: Shown, draw: impl Fn(&mut egui::Ui, &Theme) + 'static) -> (Rect, Rect, Own) {
    let theme = Theme::new(mode, scale);
    let mut harness = staged(theme, |p| p.surface, vec2(320.0, 120.0), draw);
    bring(&mut harness, label, shown);
    let row = harness.get_by_label(label).rect();
    let mark = boxed(row, &theme);
    (row, mark, own(&painted(&harness, label), mark))
}

fn check_in(mode: Mode, scale: TextScale, state: Check) -> (Rect, Rect, Own) {
    let on = matches!(state, Check::On | Check::OnHover | Check::Focus);
    let shown = match state {
        Check::OffHover | Check::OnHover => Shown::Hover,
        Check::Focus => Shown::Focus,
        _ => Shown::Rest,
    };
    toggle_in(mode, scale, "Keep a copy", shown, move |ui, theme| {
        if state == Check::Disabled {
            ui.disable();
        }
        checkbox(ui, theme, "Keep a copy", on);
    })
}

/// The name in mono 13, in `colour`.
fn name(text: &str, colour: egui::Color32, scale: f32) -> (String, egui::Color32, String, f32) {
    (text.to_owned(), colour, "Monospace".to_owned(), 13.0 * scale)
}

#[test]
fn a_check_box_looks_as_its_sheet_says_in_every_state() {
    for mode in [Mode::Dark, Mode::Light] {
        let p = Theme::new(mode, TextScale::default()).palette;
        for state in CHECKS {
            let (row, mark, look) = check_in(mode, TextScale::default(), state);
            let at = format!("{mode:?} {state:?}: {look:#?}");
            assert!((row.width() - 114.0).abs() < 0.01 && (row.height() - 20.0).abs() < 0.01, "{at} {row:?}");
            assert!((mark.width() - 16.0).abs() < 0.01, "{at}");
            let (fill, edge, cross) = match state {
                Check::Off => (None, Some((1.0, p.muted, false)), None),
                Check::On | Check::Focus => (Some(p.cyan), Some((1.0, p.cyan_edge, false)), Some(p.on_cyan)),
                Check::OffHover => (Some(p.stage), Some((1.0, p.ink, false)), None),
                Check::OnHover => (Some(p.ink), None, Some(p.bg)),
                Check::Disabled => (None, Some((1.0, p.muted, true)), None),
            };
            assert_eq!(look.fill, fill, "{at}");
            assert_eq!(look.edge, edge, "{at}");
            let ink = if state == Check::Disabled { p.muted } else { p.ink };
            let mut texts = vec![name("Keep a copy", ink, 1.0)];
            if let Some(colour) = cross {
                texts.push(("×".to_owned(), colour, "IBMPlexMono-SemiBold".to_owned(), 14.0));
            }
            assert_eq!(look.texts, texts, "{at}");
            let ring = (state == Check::Focus).then_some((mark.expand(3.0), 1.0, p.cyan_edge));
            assert_eq!(look.ring, ring, "{at}");
            assert!(look.radius <= 0.0 && !look.overflows, "{at}");
        }
    }
}

#[test]
fn a_check_box_at_130_percent_is_the_scaled_box_and_row() {
    for state in CHECKS {
        let (row, mark, look) = check_in(Mode::Dark, TextScale::ALL[4], state);
        assert!((mark.width() - 20.0).abs() < 0.01 && (row.height() - 26.0).abs() < 0.01, "{state:?} {row:?}");
        assert!(look.texts.iter().any(|(text, ..)| text == "Keep a copy"), "{state:?}");
    }
}

#[test]
fn a_ticked_and_disabled_box_keeps_its_cross_in_muted() {
    let theme = Theme::new(Mode::Light, TextScale::default());
    let (_, _, look) = toggle_in(Mode::Light, TextScale::default(), "Keep a copy", Shown::Rest, |ui, theme| {
        ui.disable();
        checkbox(ui, theme, "Keep a copy", true);
    });
    assert_eq!(look.edge, Some((1.0, theme.palette.muted, true)), "{look:#?}");
    assert_eq!(look.fill, None, "{look:#?}");
    assert!(look.texts.iter().any(|(text, colour, ..)| text == "×" && *colour == theme.palette.muted), "{look:#?}");
}

#[test]
fn a_pressed_check_box_is_the_accent_wash_on_or_off() {
    for mode in [Mode::Dark, Mode::Light] {
        let p = Theme::new(mode, TextScale::default()).palette;
        for on in [false, true] {
            let draw: fn(&mut egui::Ui, &Theme) = if on {
                |ui, theme| drop(checkbox(ui, theme, "Keep a copy", true))
            } else {
                |ui, theme| drop(checkbox(ui, theme, "Keep a copy", false))
            };
            let (_, _, look) = toggle_in(mode, TextScale::default(), "Keep a copy", Shown::Pressed, draw);
            assert_eq!((look.fill, look.edge), (Some(p.cyan_muted), Some((1.0, p.cyan_edge, false))), "{mode:?} {on}: {look:#?}");
            let cross = look.texts.iter().find(|(text, ..)| text == "×").map(|(_, colour, ..)| *colour);
            assert_eq!(cross, on.then_some(p.cyan_text), "{mode:?} {on}");
        }
    }
}

#[test]
fn enter_and_space_press_a_focused_check_box_or_radio_button() {
    let size = vec2(320.0, 120.0);
    keys_press_as_the_pointer("Keep a copy", size, boxed, ring, |ui, theme| drop(checkbox(ui, theme, "Keep a copy", false)));
    keys_press_as_the_pointer("Keep a copy", size, boxed, ring, |ui, theme| drop(checkbox(ui, theme, "Keep a copy", true)));
    keys_press_as_the_pointer("Tell the guests", size, boxed, ring, |ui, theme| drop(radio(ui, theme, "Tell the guests", false)));
    keys_press_as_the_pointer("Tell the guests", size, boxed, ring, |ui, theme| drop(radio(ui, theme, "Tell the guests", true)));
}

/// The states of the radio button's sheet, in its order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Choice {
    Off,
    On,
    Hover,
    Focus,
    Disabled,
}

const CHOICES: [Choice; 5] = [Choice::Off, Choice::On, Choice::Hover, Choice::Focus, Choice::Disabled];

fn choice_in(mode: Mode, scale: TextScale, state: Choice) -> (Rect, Rect, Own, Vec<(Rect, egui::Color32)>) {
    let on = matches!(state, Choice::On | Choice::Focus);
    let shown = match state {
        Choice::Hover => Shown::Hover,
        Choice::Focus => Shown::Focus,
        _ => Shown::Rest,
    };
    let theme = Theme::new(mode, scale);
    let mut harness = staged(
        theme,
        |p| p.surface,
        vec2(320.0, 120.0),
        move |ui, theme| {
            if state == Choice::Disabled {
                ui.disable();
            }
            radio(ui, theme, "Tell the guests", on);
        },
    );
    bring(&mut harness, "Tell the guests", shown);
    let row = harness.get_by_label("Tell the guests").rect();
    let mark = boxed(row, &theme);
    let paint = painted(&harness, "Tell the guests");
    // The filled squares inside the box that are not the box.
    let dots = paint
        .rects()
        .filter_map(|shape| match shape.look {
            Look::Rect { fill, .. } if fill.a() > 0 && mark.contains_rect(shape.rect) && shape.rect != mark => Some((shape.rect, fill)),
            _ => None,
        })
        .collect();
    (row, mark, own(&paint, mark), dots)
}

#[test]
fn a_radio_button_looks_as_its_sheet_says_in_every_state() {
    for mode in [Mode::Dark, Mode::Light] {
        let p = Theme::new(mode, TextScale::default()).palette;
        for state in CHOICES {
            let (row, mark, look, dots) = choice_in(mode, TextScale::default(), state);
            let at = format!("{mode:?} {state:?}: {look:#?} {dots:?}");
            assert!((row.width() - 145.0).abs() < 0.01 && (row.height() - 20.0).abs() < 0.01, "{at} {row:?}");
            let (fill, edge) = match state {
                Choice::Off => (None, Some((1.0, p.muted, false))),
                Choice::On | Choice::Focus => (None, Some((1.0, p.cyan_edge, false))),
                Choice::Hover => (Some(p.stage), Some((1.0, p.ink, false))),
                Choice::Disabled => (None, Some((1.0, p.muted, true))),
            };
            assert_eq!(look.fill, fill, "{at}");
            assert_eq!(look.edge, edge, "{at}");
            let dot = matches!(state, Choice::On | Choice::Focus).then(|| (mark.shrink(4.0), p.cyan_edge));
            assert_eq!(dots, dot.into_iter().collect::<Vec<_>>(), "{at}");
            let ink = if state == Choice::Disabled { p.muted } else { p.ink };
            assert_eq!(look.texts, vec![name("Tell the guests", ink, 1.0)], "{at}");
            let ring = (state == Choice::Focus).then_some((mark.expand(3.0), 1.0, p.cyan_edge));
            assert_eq!(look.ring, ring, "{at}");
            assert!(look.radius <= 0.0 && !look.overflows, "{at}");
        }
    }
}

#[test]
fn a_radio_button_at_130_percent_is_the_scaled_box_with_the_dot_4_inside() {
    let (row, mark, _, dots) = choice_in(Mode::Light, TextScale::ALL[4], Choice::On);
    assert!((mark.width() - 20.0).abs() < 0.01 && (row.height() - 26.0).abs() < 0.01, "{row:?}");
    assert_eq!(dots.first().map(|(rect, _)| *rect), Some(mark.shrink(4.0)));
}

#[test]
fn a_pressed_radio_button_is_the_accent_wash_and_keeps_its_dot() {
    let p = Theme::new(Mode::Dark, TextScale::default()).palette;
    let (_, _, look) = toggle_in(Mode::Dark, TextScale::default(), "Tell the guests", Shown::Pressed, |ui, theme| {
        radio(ui, theme, "Tell the guests", true);
    });
    assert_eq!((look.fill, look.edge), (Some(p.cyan_muted), Some((1.0, p.cyan_edge, false))), "{look:#?}");
}

#[test]
fn the_gallery_draws_forced_states_as_a_person_brings_them() {
    // A board's hovered box is the hovered box.
    let forced = |shown| {
        toggle_in(Mode::Light, TextScale::default(), "Keep a copy", Shown::Rest, move |ui, theme| {
            checkbox_with(ui, theme, "Keep a copy", false, Some(shown));
        })
        .2
    };
    let (_, _, hovered) = check_in(Mode::Light, TextScale::default(), Check::OffHover);
    assert_eq!(forced(Shown::Hover), hovered);
    let radio_forced = toggle_in(Mode::Light, TextScale::default(), "Tell the guests", Shown::Rest, |ui, theme| {
        radio_with(ui, theme, "Tell the guests", false, Some(Shown::Hover));
    })
    .2;
    assert_eq!(radio_forced.fill, hovered.fill);
}

#[test]
fn a_chosen_radio_button_hovered_turns_to_ink_as_the_ticked_check_box_does() {
    for mode in [Mode::Dark, Mode::Light] {
        let p = Theme::new(mode, TextScale::default()).palette;
        let (_, mark, look) = toggle_in(mode, TextScale::default(), "Tell the guests", Shown::Hover, |ui, theme| {
            radio(ui, theme, "Tell the guests", true);
        });
        assert_eq!((look.fill, look.edge), (Some(p.ink), None), "{mode:?} {look:#?} {mark:?}");
    }
}
