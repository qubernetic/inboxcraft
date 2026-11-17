// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::accesskit::Role;
use egui::{Color32, Key, Rect, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::{text_size, text_size_with};
use crate::component::Shown;
use crate::component::press_tests::{Part, keys_press_as_the_pointer};
use crate::component::tests::{Look, Paint, bring, builder, keyboard_walk, own, painted, stage, staged};
use crate::theme::{Mode, TextScale, Theme};

/// The states of the slider's sheet, in its order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Default,
    Hover,
    Focus,
    Dragging,
    Disabled,
    At90,
    At100,
    At130,
}

const STATES: [State; 8] = [State::Default, State::Hover, State::Focus, State::Dragging, State::Disabled, State::At90, State::At100, State::At130];

/// The size each state rests on.
fn resting(state: State) -> TextScale {
    let at = match state {
        State::At90 => 0,
        State::At100 => 1,
        State::At130 => 4,
        _ => 2,
    };
    TextScale::ALL[at]
}

/// The filled rects of `paint` inside `rect`: the track, the ticks and the handle, left to right.
fn blocks(paint: &Paint, rect: Rect) -> Vec<(Rect, Color32)> {
    let mut found: Vec<(Rect, Color32)> = paint
        .rects()
        .filter_map(|shape| match shape.look {
            Look::Rect { fill, .. } if fill.a() > 0 && rect.contains_rect(shape.rect) && shape.rect != rect => Some((shape.rect, fill)),
            _ => None,
        })
        .collect();
    found.sort_by(|a, b| a.0.left().total_cmp(&b.0.left()).then(a.0.width().total_cmp(&b.0.width())));
    found
}

/// The texts of `paint`: their words and colours, left to right.
fn words(paint: &Paint) -> Vec<(String, Color32, Rect)> {
    let mut found: Vec<(String, Color32, Rect)> = paint
        .shapes
        .iter()
        .filter_map(|shape| match &shape.look {
            Look::Text { text, colour, .. } => Some((text.clone(), *colour, shape.rect)),
            _ => None,
        })
        .collect();
    found.sort_by(|a, b| a.2.left().total_cmp(&b.2.left()));
    found
}

fn slider_in(mode: Mode, state: State) -> (Rect, Paint, Harness<'static>) {
    let theme = Theme::new(mode, TextScale::default());
    let mut harness = staged(
        theme,
        |p| p.surface,
        vec2(400.0, 120.0),
        move |ui, theme| {
            if state == State::Disabled {
                ui.disable();
            }
            text_size(ui, theme, resting(state));
        },
    );
    match state {
        State::Hover => {
            // The pointer over the handle.
            let rect = harness.get_by_label("Text size").rect();
            harness.hover_at(rect.min + vec2(120.5, 12.0));
            harness.run();
        }
        State::Focus => bring(&mut harness, "Text size", Shown::Focus),
        State::Dragging => {
            let rect = harness.get_by_label("Text size").rect();
            harness.hover_at(rect.min + vec2(120.5, 12.0));
            harness.drag_at(rect.min + vec2(120.5, 12.0));
            harness.run();
            harness.hover_at(rect.min + vec2(168.0, 12.0));
            harness.run();
        }
        _ => {}
    }
    let rect = harness.get_by_label("Text size").rect();
    let paint = painted(&harness, "Text size");
    (rect, paint, harness)
}

#[test]
fn a_slider_looks_as_its_sheet_says_in_every_state() {
    for mode in [Mode::Dark, Mode::Light] {
        let p = Theme::new(mode, TextScale::default()).palette;
        for state in STATES {
            let (rect, paint, _) = slider_in(mode, state);
            let at = format!("{mode:?} {state:?}: {paint:#?}");
            assert!((rect.width() - 240.0).abs() < 0.01 && (rect.height() - 44.0).abs() < 0.01, "{at} {rect:?}");
            let disabled = state == State::Disabled;
            let tick = if disabled { p.dim } else { p.muted };
            let at_index = TextScale::ALL.iter().position(|scale| *scale == resting(state)).unwrap_or(0);
            let (place, high, top) = if state == State::Dragging { (3, 25.0, 0.0) } else { (at_index, 17.0, 4.0) };
            let handle = match state {
                State::Disabled => p.dim,
                State::Hover | State::Dragging => p.cyan_edge,
                _ => p.ink,
            };
            let x = |index: usize| 20.0 + 50.0 * index as f32;
            let mut want = vec![(Rect::from_min_size(rect.min + vec2(20.0, 12.0), vec2(201.0, 1.0)), p.line)];
            for index in 0..5 {
                want.push((Rect::from_min_size(rect.min + vec2(x(index), 8.0), vec2(1.0, 9.0)), tick));
            }
            want.push((Rect::from_min_size(rect.min + vec2(x(place) - 4.0, top), vec2(9.0, high)), handle));
            want.sort_by(|a, b| a.0.left().total_cmp(&b.0.left()).then(a.0.width().total_cmp(&b.0.width())));
            assert_eq!(blocks(&paint, rect), want, "{at}");
            let texts = words(&paint);
            let names = ["90 %", "100 %", "110 %", "120 %", "130 %"];
            assert_eq!(texts.iter().map(|(text, ..)| text.as_str()).collect::<Vec<_>>(), names, "{at}");
            for (index, (_, colour, shape)) in texts.iter().enumerate() {
                let want = match (disabled, state == State::Dragging, index == at_index, index == place) {
                    (true, ..) => p.muted,
                    (false, true, _, true) => p.cyan_text,
                    (false, true, _, false) => p.muted,
                    (false, false, true, _) => p.ink,
                    (false, false, false, _) => p.muted,
                };
                assert_eq!(*colour, want, "{at} {index}");
                assert!((shape.center().x - (rect.left() + x(index) + 0.5)).abs() < 0.6 && shape.top() >= rect.top() + 28.0 - 0.01, "{at} {index}");
            }
            let look = own(&paint, rect);
            // The ring holds the handle, not the whole slider: read from the strokes, its outer bound.
            let rings: Vec<(Rect, f32, Color32)> = paint
                .rects()
                .filter_map(|shape| match shape.look {
                    Look::Rect { stroke: Some((width, colour)), stroke_kind, .. } => {
                        let outer = match stroke_kind {
                            egui::StrokeKind::Inside => shape.rect,
                            egui::StrokeKind::Middle => shape.rect.expand(width / 2.0),
                            egui::StrokeKind::Outside => shape.rect.expand(width),
                        };
                        Some((outer, width, colour))
                    }
                    _ => None,
                })
                .collect();
            let ring =
                (state == State::Focus).then_some((Rect::from_min_size(rect.min + vec2(x(place) - 7.0, 1.0), vec2(15.0, 23.0)), 1.0, p.cyan_edge));
            assert_eq!(rings, ring.into_iter().collect::<Vec<_>>(), "{at}");
            assert!(look.radius <= 0.0 && look.fill.is_none() && look.edge.is_none(), "{at}");
        }
    }
}

#[test]
fn a_slider_is_named_tells_its_size_and_takes_one_stop_of_tab() {
    let theme = Theme::new(Mode::Light, TextScale::default());
    let mut harness = builder().build_ui_state(
        move |ui, now: &mut TextScale| {
            if let Some(scale) = text_size(ui, &theme, *now) {
                *now = scale;
            }
        },
        TextScale::default(),
    );
    harness.run();
    let node = harness.get_by_role_and_label(Role::Slider, "Text size");
    assert_eq!(node.accesskit_node().numeric_value(), Some(100.0));
    assert!(keyboard_walk(&mut harness).is_empty());
}

#[test]
fn a_press_sets_the_size_under_it_and_the_arrows_move_one_stop_keeping_the_focus() {
    let theme = Theme::new(Mode::Light, TextScale::default());
    let mut harness = builder().build_ui_state(
        move |ui, now: &mut TextScale| {
            ui.label("before");
            if let Some(scale) = text_size(ui, &theme, *now) {
                *now = scale;
            }
        },
        TextScale::default(),
    );
    harness.run();
    let rect = harness.get_by_label("Text size").rect();
    harness.hover_at(rect.min + vec2(218.0, 12.0));
    harness.drag_at(rect.min + vec2(218.0, 12.0));
    harness.run();
    assert_eq!(*harness.state(), TextScale::default(), "held down, nothing is set yet");
    harness.drop_at(rect.min + vec2(218.0, 12.0));
    harness.run();
    assert_eq!(*harness.state(), TextScale::ALL[4]);
    harness.get_by_label("Text size").focus();
    harness.run();
    harness.key_press(Key::ArrowLeft);
    harness.run();
    assert_eq!(*harness.state(), TextScale::ALL[3]);
    harness.key_press(Key::ArrowRight);
    harness.run();
    harness.key_press(Key::ArrowRight);
    harness.run();
    assert_eq!(*harness.state(), TextScale::ALL[4], "the last stays the last");
    assert!(harness.get_by_label("Text size").is_focused());
}

#[test]
fn a_drag_names_the_size_it_will_take_and_sets_it_when_let_go() {
    let theme = Theme::new(Mode::Light, TextScale::default());
    let mut harness = builder().build_ui_state(
        move |ui, now: &mut TextScale| {
            if let Some(scale) = text_size(ui, &theme, *now) {
                *now = scale;
            }
        },
        TextScale::default(),
    );
    harness.run();
    let rect = harness.get_by_label("Text size").rect();
    harness.hover_at(rect.min + vec2(70.0, 12.0));
    harness.drag_at(rect.min + vec2(70.0, 12.0));
    harness.run();
    harness.hover_at(rect.min + vec2(175.0, 12.0));
    harness.run();
    assert_eq!(*harness.state(), TextScale::default());
    harness.drop_at(rect.min + vec2(175.0, 12.0));
    harness.run();
    assert_eq!(*harness.state(), TextScale::ALL[3]);
}

#[test]
fn enter_and_space_hold_the_handle_as_a_drag_does() {
    // At 110 % the track's middle is the handle's tick: the pointer and the keys press the same handle.
    let handle: Part = |rect, _| Rect::from_min_size(rect.min + vec2(116.0, 0.0), vec2(9.0, 25.0));
    let ring: Part = |rect, _| Rect::from_min_size(rect.min + vec2(113.0, -3.0), vec2(15.0, 31.0));
    keys_press_as_the_pointer("Text size", vec2(400.0, 120.0), handle, ring, |ui, theme| {
        text_size(ui, theme, TextScale::ALL[2]);
    });
}

#[test]
fn the_gallery_forces_a_drag_towards_a_size() {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let harness = staged(
        theme,
        |p| p.surface,
        vec2(400.0, 120.0),
        |ui, theme| {
            text_size_with(ui, theme, TextScale::ALL[2], Some(Shown::Pressed), Some(TextScale::ALL[3]));
        },
    );
    let rect = harness.get_by_label("Text size").rect();
    let paint = painted(&harness, "Text size");
    let tall = blocks(&paint, rect).into_iter().find(|(block, _)| block.height() > 20.0);
    assert_eq!(tall, Some((Rect::from_min_size(pos2(rect.left() + 166.0, rect.top()), vec2(9.0, 25.0)), theme.palette.cyan_edge)));
}

#[test]
fn a_press_takes_the_focus_for_the_arrow_keys() {
    let theme = Theme::new(Mode::Light, TextScale::default());
    let mut harness = builder().build_ui_state(
        move |ui, now: &mut TextScale| {
            if let Some(scale) = text_size(ui, &theme, *now) {
                *now = scale;
            }
        },
        TextScale::default(),
    );
    harness.run();
    harness.get_by_label("Text size").click();
    harness.run();
    assert_eq!(*harness.state(), TextScale::ALL[2]);
    harness.key_press(Key::ArrowRight);
    harness.run();
    assert_eq!(*harness.state(), TextScale::ALL[3]);
}

/// The slider at `scale` on a `surface` stage: its rect and its paint.
fn slider_at(scale: TextScale, current: TextScale) -> (Rect, Paint) {
    let theme = Theme::new(Mode::Light, scale);
    let mut harness = staged(
        theme,
        |p| p.surface,
        vec2(400.0, 140.0),
        move |ui, theme| {
            text_size(ui, theme, current);
        },
    );
    harness.run();
    let rect = harness.get_by_label("Text size").rect();
    (rect, painted(&harness, "Text size"))
}

#[test]
fn every_name_is_whole_apart_from_its_neighbours_and_inside_at_every_text_size() {
    for scale in TextScale::ALL {
        for current in [TextScale::ALL[0], TextScale::ALL[4]] {
            let (rect, paint) = slider_at(scale, current);
            let texts = words(&paint);
            let names = ["90 %", "100 %", "110 %", "120 %", "130 %"];
            assert_eq!(texts.iter().map(|(text, ..)| text.as_str()).collect::<Vec<_>>(), names, "{scale:?}");
            let size = Theme::new(Mode::Light, scale).fonts.mono_s.size;
            for (index, (text, _, shape)) in texts.iter().enumerate() {
                // Plex Mono's advance is 0.6 em: a whole name is as wide as its characters, a cut one narrower.
                let whole = text.chars().count() as f32 * 0.6 * size;
                assert!((shape.width() - whole).abs() < 1.0, "{scale:?} {text}: {} wide, {whole} whole", shape.width());
                assert!(rect.contains_rect(*shape), "{scale:?} {text} inside {rect:?}: {shape:?}");
                if let Some((_, _, next)) = texts.get(index + 1) {
                    assert!(shape.right() < next.left(), "{scale:?} {text} runs into its neighbour");
                }
            }
            let track = blocks(&paint, rect).into_iter().find(|(block, _)| block.width() > 100.0).map(|(block, _)| block);
            let handle = blocks(&paint, rect).into_iter().find(|(block, _)| block.width() > 2.0 && block.height() > 10.0).map(|(block, _)| block);
            let (Some(track), Some(handle)) = (track, handle) else { panic!("{scale:?}: no track or handle") };
            assert!(track.left() <= handle.center().x && handle.center().x <= track.right(), "{scale:?} the handle on the track");
        }
    }
}

#[test]
fn ticks_track_and_handle_stand_on_whole_pixels_from_any_origin() {
    for ppp in [1.0_f32, 1.25, 1.5, 2.0] {
        let theme = Theme::new(Mode::Light, TextScale::default());
        let mut harness = builder().with_pixels_per_point(ppp).with_size(vec2(400.0, 160.0)).build_ui(move |ui| {
            stage(ui, theme.palette.surface, |ui| {
                // An origin off the pixel grid: x = 10.3 inside the stage, y a fraction too.
                let place = Rect::from_min_size(ui.cursor().min + vec2(10.3, 0.7), vec2(300.0, 80.0));
                ui.scope_builder(egui::UiBuilder::new().max_rect(place), |ui| {
                    text_size(ui, &theme, TextScale::ALL[2]);
                });
            });
        });
        harness.run();
        let rect = harness.get_by_label("Text size").rect();
        let paint = painted(&harness, "Text size");
        let found = blocks(&paint, rect);
        assert_eq!(found.len(), 7, "{ppp}: {found:?}");
        let whole = |value: f32| ((value * ppp) - (value * ppp).round()).abs() < 0.01;
        for (block, _) in found {
            assert!(whole(block.left()) && whole(block.right()) && whole(block.top()) && whole(block.bottom()), "{ppp}: {block:?}");
        }
    }
}
