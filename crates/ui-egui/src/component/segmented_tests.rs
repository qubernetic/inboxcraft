// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::accesskit::{Role, Toggled};
use egui::{Align, Color32, Layout, Rect, vec2};
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::{segmented, segmented_for, segmented_with, width};
use crate::component::Shown;
use crate::component::press_tests::{Part, keys_press_as_the_pointer};
use crate::component::tests::{Look, Own, bring, builder, keyboard_walk, own, painted, staged, theme};
use crate::motion;
use crate::theme::{Font, Mode, TextScale, Theme};

fn options(font: Font) -> [(u8, &'static str, Font); 3] {
    [(0, "Day", font), (1, "Week", font), (2, "Month", font)]
}

#[test]
fn each_option_is_a_named_button_and_the_pressed_one_comes_back() {
    let theme = theme();
    let mut harness = builder().build_ui_state(
        move |ui, current: &mut u8| {
            motion::set_reduced(ui.ctx(), true);
            if let Some(picked) = segmented(ui, &theme, &options(theme.fonts.button), *current) {
                *current = picked;
            }
        },
        1,
    );
    harness.run();
    assert_eq!(harness.get_by_role_and_label(Role::Button, "Week").accesskit_node().toggled(), Some(Toggled::True));
    assert_eq!(harness.get_by_role_and_label(Role::Button, "Day").accesskit_node().toggled(), Some(Toggled::False));
    harness.get_by_role_and_label(Role::Button, "Month").click();
    harness.run();
    assert_eq!(*harness.state(), 2);
    assert_eq!(harness.get_by_role_and_label(Role::Button, "Month").accesskit_node().toggled(), Some(Toggled::True));
}

#[test]
fn several_rows_with_the_same_options_say_what_each_is_for() {
    let theme = theme();
    let mut harness = builder().build_ui(move |ui| {
        segmented_for(ui, &theme, &options(theme.fonts.button), 0, Some("Personal"));
    });
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Week for Personal");
}

#[test]
fn the_options_stay_in_order_in_a_row_laid_out_from_the_right() {
    let theme = theme();
    let mut harness = builder().build_ui(move |ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            segmented(ui, &theme, &options(theme.fonts.button), 0);
        });
    });
    harness.run();
    let day = harness.get_by_role_and_label(Role::Button, "Day").rect();
    let week = harness.get_by_role_and_label(Role::Button, "Week").rect();
    let month = harness.get_by_role_and_label(Role::Button, "Month").rect();
    assert!(day.right() <= week.left() && week.right() <= month.left(), "{day:?} {week:?} {month:?}");
}

#[test]
fn the_track_is_as_wide_as_width_says_and_an_option_keeps_its_width_when_chosen() {
    let theme = theme();
    let measure = |current: u8| {
        let mut harness = builder().build_ui(move |ui| {
            let full = width(ui, &theme, &options(theme.fonts.button));
            let before = ui.next_widget_position().x;
            segmented(ui, &theme, &options(theme.fonts.button), current);
            assert!((ui.min_rect().width() - full).abs() <= 1.0, "{} wide, {full} expected from {before}", ui.min_rect().width());
        });
        harness.run();
        harness.get_by_role_and_label(Role::Button, "Week").rect().width()
    };
    assert!((measure(0) - measure(1)).abs() < 1.0);
}

#[test]
fn in_too_little_room_the_options_share_it_and_stay_inside() {
    let theme = theme();
    for room in [160.0_f32, 24.0, 1.0] {
        let mut harness = builder().build_ui(move |ui| {
            ui.scope(|ui| {
                ui.set_max_width(room);
                segmented(ui, &theme, &options(theme.fonts.button), 1);
            });
        });
        harness.run();
        let day = harness.get_by_role_and_label(Role::Button, "Day").rect();
        let month = harness.get_by_role_and_label(Role::Button, "Month").rect();
        assert!(month.right() - day.left() <= room.max(20.0) + 4.0, "{day:?} {month:?} in {room}");
    }
}

#[test]
fn no_options_draw_nothing() {
    let theme = theme();
    let mut harness = builder().build_ui(move |ui| {
        assert_eq!(segmented::<u8>(ui, &theme, &[], 0), None);
    });
    harness.run();
}

#[test]
fn a_long_name_takes_its_room() {
    let theme = theme();
    let mut harness = builder().with_size(vec2(600.0, 80.0)).build_ui(move |ui| {
        let font = theme.fonts.button;
        segmented(ui, &theme, &[(0, "A name that is a good deal longer than any view", font), (1, "Short", font)], 1);
    });
    harness.run();
    let long = harness.get_by_role_and_label(Role::Button, "A name that is a good deal longer than any view").rect();
    let short = harness.get_by_role_and_label(Role::Button, "Short").rect();
    assert!(long.width() > short.width());
}

/// The states of the segmented control's sheet, in its order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Default,
    Hover,
    Focus,
    Pressed,
    Disabled,
    LongLabel,
}

const STATES: [State; 6] = [State::Default, State::Hover, State::Focus, State::Pressed, State::Disabled, State::LongLabel];

const VIEWS: [&str; 3] = ["Grouped", "Smart", "List"];

const LONG: [&str; 3] = ["Gruppiert", "Zusammenfassung des Posteingangs", "Liste"];

/// What the test reads of a segmented control: the track's rect and look, each option's rect and look, the filled
/// shapes inside the track that are neither the track nor an option (hairlines and bars).
struct Seen {
    track: Rect,
    look: Own,
    options: Vec<(Rect, Own)>,
    marks: Vec<(Rect, Color32)>,
}

fn seen_in(mode: Mode, scale: TextScale, state: State) -> Seen {
    let theme = Theme::new(mode, scale);
    let mut harness = staged(
        theme,
        |p| p.surface,
        vec2(600.0, 120.0),
        move |ui, theme| {
            if state == State::Disabled {
                ui.disable();
            }
            let names = if state == State::LongLabel { LONG } else { VIEWS };
            let font = theme.fonts.button;
            segmented(ui, theme, &[(0, names[0], font), (1, names[1], font), (2, names[2], font)], 0);
        },
    );
    let names = if state == State::LongLabel { LONG } else { VIEWS };
    let group = names.join(" / ");
    match state {
        State::Hover => bring(&mut harness, "Smart", Shown::Hover),
        State::Pressed => bring(&mut harness, "Smart", Shown::Pressed),
        State::Focus => bring(&mut harness, &group, Shown::Focus),
        _ => {}
    }
    let track = harness.get_by_label(&group).rect();
    let paint = painted(&harness, &group);
    let options: Vec<(Rect, Own)> = names
        .iter()
        .map(|name| {
            let rect = harness.get_by_label(name).rect();
            let mut look = own(&paint, rect);
            // `own` reads every text of the control: keep this option's.
            let inside: Vec<String> = paint
                .shapes
                .iter()
                .filter_map(|shape| match &shape.look {
                    Look::Text { text, .. } if rect.contains(shape.rect.center()) => Some(text.clone()),
                    _ => None,
                })
                .collect();
            look.texts.retain(|(text, ..)| inside.contains(text));
            (rect, look)
        })
        .collect();
    let marks = paint
        .rects()
        .filter_map(|shape| match shape.look {
            Look::Rect { fill, .. }
                if fill.a() > 0 && track.contains_rect(shape.rect) && shape.rect != track && options.iter().all(|(rect, _)| *rect != shape.rect) =>
            {
                Some((shape.rect, fill))
            }
            _ => None,
        })
        .collect();
    Seen { track, look: own(&paint, track), options, marks }
}

/// The colour and family-less text of `look`.
fn texts(look: &Own) -> Vec<(String, Color32)> {
    look.texts.iter().map(|(text, colour, ..)| (text.clone(), *colour)).collect()
}

#[test]
fn a_segmented_control_looks_as_its_sheet_says_in_every_state() {
    for mode in [Mode::Dark, Mode::Light] {
        let p = Theme::new(mode, TextScale::default()).palette;
        for state in STATES {
            let seen = seen_in(mode, TextScale::default(), state);
            let at = format!("{mode:?} {state:?}: {:#?} {:#?} {:?}", seen.look, seen.options, seen.marks);
            let t = seen.track;
            let widths: &[f32] = if state == State::LongLabel { &[95.0, 160.0, 63.0] } else { &[79.0, 63.0, 56.0] };
            let total = if state == State::LongLabel { 322.0 } else { 202.0 };
            assert!((t.width() - total).abs() < 0.01 && (t.height() - 32.0).abs() < 0.01, "{at} {t:?}");
            let disabled = state == State::Disabled;
            assert_eq!(seen.look.fill, (!disabled).then_some(p.term), "{at}");
            assert_eq!(seen.look.edge, Some((1.0, p.line, disabled)), "{at}");
            let ring = (state == State::Focus).then_some((t.expand(3.0), 1.0, p.cyan_edge));
            assert_eq!(seen.look.ring, ring, "{at}");
            // The options side by side, a hairline between two.
            let mut left = t.left() + 1.0;
            let mut want_marks = Vec::new();
            for (index, ((rect, look), wide)) in seen.options.iter().zip(widths).enumerate() {
                let want = Rect::from_min_size(egui::pos2(left, t.top() + 1.0), vec2(*wide, 30.0));
                assert_eq!(*rect, want, "{at} option {index}");
                left += wide;
                if index < 2 {
                    want_marks.push((Rect::from_min_size(egui::pos2(left, t.top() + 1.0), vec2(1.0, 30.0)), p.line));
                    left += 1.0;
                }
                let chosen = index == 0;
                let pressed = state == State::Pressed && index == 1;
                let fill = match (chosen, pressed) {
                    (true, _) => Some(p.stage),
                    (false, true) => Some(p.cyan_muted),
                    (false, false) => None,
                };
                assert_eq!(look.fill, fill, "{at} option {index}");
                let colour = match (disabled, chosen, pressed, state == State::Hover && index == 1) {
                    (true, ..) => p.muted,
                    (false, true, ..) => p.ink,
                    (false, false, true, _) => p.cyan_text,
                    (false, false, false, true) => p.ink,
                    (false, false, false, false) => p.muted,
                };
                let shown = texts(look);
                assert_eq!(shown.len(), 1, "{at} option {index}");
                assert_eq!(shown[0].1, colour, "{at} option {index}");
                assert_eq!((look.texts[0].2.as_str(), look.texts[0].3), ("Monospace", 13.0), "{at} option {index}");
                if chosen {
                    let bar = if disabled { p.muted } else { p.cyan_edge };
                    want_marks.push((Rect::from_min_max(egui::pos2(rect.left(), rect.bottom() - 2.0), rect.max), bar));
                }
            }
            let mut marks = seen.marks.clone();
            let order = |a: &(Rect, Color32), b: &(Rect, Color32)| a.0.left().total_cmp(&b.0.left()).then(a.0.top().total_cmp(&b.0.top()));
            marks.sort_by(order);
            want_marks.sort_by(order);
            assert_eq!(marks, want_marks, "{at}");
            assert!(seen.look.radius <= 0.0 && !seen.look.overflows, "{at}");
        }
    }
}

#[test]
fn a_long_option_is_cut_at_136() {
    let theme = Theme::new(Mode::Light, TextScale::default());
    let mut harness = staged(
        theme,
        |p| p.surface,
        vec2(600.0, 120.0),
        |ui, theme| {
            let font = theme.fonts.button;
            segmented(ui, theme, &[(0, LONG[0], font), (1, LONG[1], font), (2, LONG[2], font)], 0);
        },
    );
    harness.run();
    let paint = painted(&harness, &LONG.join(" / "));
    let long = paint.shapes.iter().find(|shape| matches!(&shape.look, Look::Text { text, .. } if text == LONG[1]));
    let width = long.map(|shape| shape.rect.width());
    assert!(width.is_some_and(|width| width <= 136.5 && width > 120.0), "{width:?}");
}

#[test]
fn a_segmented_control_at_130_percent_is_the_scaled_height() {
    let theme = Theme::new(Mode::Dark, TextScale::ALL[4]);
    let seen = seen_in(Mode::Dark, TextScale::ALL[4], State::Default);
    assert!((seen.track.height() - theme.boxes().button).abs() < 0.01, "{:?}", seen.track);
}

#[test]
fn the_control_is_one_stop_for_tab_and_the_arrows_move_the_choice() {
    let theme = theme();
    let mut harness = builder().build_ui_state(
        move |ui, current: &mut u8| {
            motion::set_reduced(ui.ctx(), true);
            ui.label("before");
            if let Some(picked) = segmented(ui, &theme, &options(theme.fonts.button), *current) {
                *current = picked;
            }
        },
        1,
    );
    harness.run();
    assert!(keyboard_walk(&mut harness).is_empty());
    harness.get_by_label("Day / Week / Month").focus();
    harness.run();
    harness.key_press(egui::Key::ArrowRight);
    harness.run();
    assert_eq!(*harness.state(), 2);
    harness.key_press(egui::Key::ArrowRight);
    harness.run();
    assert_eq!(*harness.state(), 2, "the last stays the last");
    harness.key_press(egui::Key::ArrowLeft);
    harness.run();
    harness.key_press(egui::Key::ArrowLeft);
    harness.run();
    assert_eq!(*harness.state(), 0);
    assert!(harness.get_by_label("Day / Week / Month").is_focused(), "the arrows keep the focus on the control");
}

#[test]
fn enter_and_space_press_the_chosen_option() {
    // Day 48, Week 56, Month 63 wide: the track's middle is on Week, the choice.
    let week: Part = |track, _| Rect::from_min_size(track.min + vec2(50.0, 1.0), vec2(56.0, 30.0));
    let ring: Part = |track, _| track.expand(3.0);
    keys_press_as_the_pointer("Day / Week / Month", vec2(400.0, 100.0), week, ring, |ui, theme| {
        segmented(ui, theme, &options(theme.fonts.button), 1);
    });
}

#[test]
fn the_gallery_forces_a_hovered_option() {
    let p = Theme::new(Mode::Light, TextScale::default()).palette;
    let theme = Theme::new(Mode::Light, TextScale::default());
    let harness = staged(
        theme,
        |p| p.surface,
        vec2(400.0, 100.0),
        |ui, theme| {
            segmented_with(ui, theme, &options(theme.fonts.button), 0, None, Some((Shown::Pressed, 1)));
        },
    );
    let rect = harness.get_by_label("Week").rect();
    assert_eq!(own(&painted(&harness, "Day / Week / Month"), rect).fill, Some(p.cyan_muted));
}
