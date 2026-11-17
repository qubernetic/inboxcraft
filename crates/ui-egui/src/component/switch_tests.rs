// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::accesskit::{Role, Toggled};
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use egui::{Rect, vec2};

use super::{SwitchState, switch, switch_in, switch_noted, switch_with};
use crate::component::Shown;
use crate::component::press_tests::{Part, keys_press_as_the_pointer};
use crate::component::tests::{Look, Own, Paint, bring, builder, keyboard_walk, own, painted, rests, stage, staged, theme};
use crate::motion;
use crate::theme::{Mode, TextScale, Theme};

#[test]
fn a_switch_is_named_says_whether_it_is_on_and_flips_when_pressed() {
    let theme = theme();
    let mut harness = builder().build_ui_state(
        move |ui, on: &mut bool| {
            motion::set_reduced(ui.ctx(), true);
            if switch(ui, &theme, "Notify me", *on).clicked() {
                *on = !*on;
            }
        },
        false,
    );
    harness.run();
    let node = harness.get_by_role_and_label(Role::Switch, "Notify me");
    assert_eq!(node.accesskit_node().toggled(), Some(Toggled::False));
    node.click();
    harness.run();
    assert!(*harness.state());
    assert_eq!(harness.get_by_role_and_label(Role::Switch, "Notify me").accesskit_node().toggled(), Some(Toggled::True));
}

#[test]
fn a_switch_is_wider_than_tall_and_shows_its_focus() {
    let theme = theme();
    let mut harness = builder().build_ui(move |ui| {
        let response = switch(ui, &theme, "Alert", true);
        response.request_focus();
    });
    harness.run();
    let rect = harness.get_by_role_and_label(Role::Switch, "Alert").rect();
    assert!(rect.width() > rect.height() * 1.5 && rect.height() > 10.0, "{rect:?}");
}

/// The states of the switch's sheet, in its order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Off,
    On,
    Hover,
    Focus,
    Disabled,
    DisabledOn,
    Waiting,
    Unavailable,
}

const STATES: [State; 8] =
    [State::Off, State::On, State::Hover, State::Focus, State::Disabled, State::DisabledOn, State::Waiting, State::Unavailable];

/// The 12 px squares inside `rect`: the knob, where it stands and its colour.
fn knobs(paint: &Paint, rect: Rect) -> Vec<(Rect, egui::Color32)> {
    paint
        .rects()
        .filter_map(|shape| match shape.look {
            Look::Rect { fill, .. } if fill.a() > 0 && rect.contains_rect(shape.rect) && shape.rect != rect => Some((shape.rect, fill)),
            _ => None,
        })
        .collect()
}

/// The switch "Notify me" in `state` on a `surface` stage in `mode` at `scale`: its rect, its look and its knobs.
fn switch_state(mode: Mode, scale: TextScale, state: State) -> (Rect, Own, Vec<(Rect, egui::Color32)>) {
    let theme = Theme::new(mode, scale);
    let mut harness = staged(
        theme,
        |p| p.surface,
        vec2(200.0, 100.0),
        move |ui, theme| {
            if matches!(state, State::Disabled | State::DisabledOn) {
                ui.disable();
            }
            let shown = match state {
                State::Off | State::Hover | State::Disabled => SwitchState::Off,
                State::On | State::Focus | State::DisabledOn => SwitchState::On,
                State::Waiting => SwitchState::Waiting,
                State::Unavailable => SwitchState::Unavailable,
            };
            switch_in(ui, theme, "Notify me", shown);
        },
    );
    let shown = match state {
        State::Hover => Shown::Hover,
        State::Focus => Shown::Focus,
        _ => Shown::Rest,
    };
    bring(&mut harness, "Notify me", shown);
    let rect = harness.get_by_label("Notify me").rect();
    let paint = painted(&harness, "Notify me");
    (rect, own(&paint, rect), knobs(&paint, rect))
}

#[test]
fn a_switch_looks_as_its_sheet_says_in_every_state() {
    for mode in [Mode::Dark, Mode::Light] {
        let p = Theme::new(mode, TextScale::default()).palette;
        for state in STATES {
            let (rect, look, knob) = switch_state(mode, TextScale::default(), state);
            let at = format!("{mode:?} {state:?}: {look:#?} {knob:?}");
            assert!((rect.width() - 28.0).abs() < 0.01 && (rect.height() - 16.0).abs() < 0.01, "{at} {rect:?}");
            let (fill, edge, at_x, colour) = match state {
                State::Off => (None, (p.muted, false), Some(2.0), p.muted),
                State::On | State::Focus => (Some(p.cyan), (p.cyan_edge, false), Some(14.0), p.on_cyan),
                State::Hover => (Some(p.stage), (p.ink, false), Some(2.0), p.ink),
                State::Disabled => (None, (p.muted, true), Some(2.0), p.dim),
                State::DisabledOn => (None, (p.muted, true), Some(14.0), p.dim),
                State::Waiting => (None, (p.warn, false), Some(8.0), p.warn),
                State::Unavailable => (None, (p.muted, true), None, p.muted),
            };
            assert_eq!(look.fill, fill, "{at}");
            assert_eq!(look.edge, Some((1.0, edge.0, edge.1)), "{at}");
            let want: Vec<_> = at_x.map(|x| (Rect::from_min_size(rect.min + vec2(x, 2.0), vec2(12.0, 12.0)), colour)).into_iter().collect();
            assert_eq!(knob, want, "{at}");
            let ring = (state == State::Focus).then_some((rect.expand(3.0), 1.0, p.cyan_edge));
            assert_eq!(look.ring, ring, "{at}");
            assert!(look.radius <= 0.0 && look.texts.is_empty(), "{at}");
        }
    }
}

#[test]
fn a_switch_keeps_its_size_at_130_percent() {
    for state in STATES {
        let (rect, ..) = switch_state(Mode::Dark, TextScale::ALL[4], state);
        assert!((rect.width() - 28.0).abs() < 0.01 && (rect.height() - 16.0).abs() < 0.01, "{state:?} {rect:?}");
    }
}

#[test]
fn a_pressed_switch_is_the_accent_wash_with_the_knob_at_its_place() {
    for mode in [Mode::Dark, Mode::Light] {
        let p = Theme::new(mode, TextScale::default()).palette;
        for on in [false, true] {
            let theme = Theme::new(mode, TextScale::default());
            let draw: fn(&mut egui::Ui, &Theme) =
                if on { |ui, theme| drop(switch(ui, theme, "Notify me", true)) } else { |ui, theme| drop(switch(ui, theme, "Notify me", false)) };
            let mut harness = staged(theme, |p| p.surface, vec2(200.0, 100.0), draw);
            bring(&mut harness, "Notify me", Shown::Pressed);
            let rect = harness.get_by_label("Notify me").rect();
            let paint = painted(&harness, "Notify me");
            let look = own(&paint, rect);
            assert_eq!((look.fill, look.edge), (Some(p.cyan_muted), Some((1.0, p.cyan_edge, false))), "{mode:?} {on} {look:#?}");
            let x = if on { 14.0 } else { 2.0 };
            assert_eq!(knobs(&paint, rect), vec![(Rect::from_min_size(rect.min + vec2(x, 2.0), vec2(12.0, 12.0)), p.cyan_edge)]);
        }
    }
}

#[test]
fn enter_and_space_press_a_focused_switch() {
    let whole: Part = |rect, _| rect;
    let ring: Part = |rect, _| rect.expand(3.0);
    keys_press_as_the_pointer("Notify me", vec2(200.0, 100.0), whole, ring, |ui, theme| drop(switch(ui, theme, "Notify me", false)));
    keys_press_as_the_pointer("Notify me", vec2(200.0, 100.0), whole, ring, |ui, theme| drop(switch(ui, theme, "Notify me", true)));
}

#[test]
fn a_waiting_or_unavailable_switch_takes_no_press_and_neither_takes_the_focus() {
    for state in [SwitchState::Waiting, SwitchState::Unavailable] {
        let theme = theme();
        let mut harness = builder().build_ui_state(
            move |ui, pressed: &mut bool| {
                if switch_in(ui, &theme, "Notify me", state).clicked() {
                    *pressed = true;
                }
            },
            false,
        );
        harness.run();
        harness.get_by_label("Notify me").click();
        harness.run();
        assert!(!*harness.state(), "{state:?}");
        assert!(keyboard_walk(&mut harness).is_empty(), "{state:?}");
    }
}

#[test]
fn the_knob_lands_at_once_with_motion_full_and_the_switch_then_rests() {
    let theme = theme();
    let mut harness = builder().build_ui_state(
        move |ui, on: &mut bool| {
            motion::set_reduced(ui.ctx(), false);
            stage(ui, theme.palette.surface, |ui| {
                if switch(ui, &theme, "Notify me", *on).clicked() {
                    *on = !*on;
                }
            });
        },
        false,
    );
    harness.run();
    harness.get_by_label("Notify me").click();
    harness.step();
    harness.step();
    let rect = harness.get_by_label("Notify me").rect();
    let paint = painted(&harness, "Notify me");
    assert_eq!(knobs(&paint, rect).first().map(|(knob, _)| knob.left() - rect.left()), Some(14.0), "the presentation moves the knob at once");
    harness.hover_at(egui::pos2(500.0, 500.0));
    assert!(rests(&mut harness, 12));
}

#[test]
fn the_gallery_forces_a_hovered_switch() {
    let p = Theme::new(Mode::Light, TextScale::default()).palette;
    let theme = Theme::new(Mode::Light, TextScale::default());
    let harness = staged(
        theme,
        |p| p.surface,
        vec2(200.0, 100.0),
        |ui, theme| {
            switch_with(ui, theme, "Notify me", SwitchState::Off, Some(Shown::Hover));
        },
    );
    let rect = harness.get_by_label("Notify me").rect();
    assert_eq!(own(&painted(&harness, "Notify me"), rect).fill, Some(p.stage));
}

#[test]
fn an_on_switch_hovered_turns_to_ink_as_the_ticked_check_box_does() {
    for mode in [Mode::Dark, Mode::Light] {
        let p = Theme::new(mode, TextScale::default()).palette;
        let theme = Theme::new(mode, TextScale::default());
        let mut harness = staged(
            theme,
            |p| p.surface,
            vec2(200.0, 100.0),
            |ui, theme| {
                switch(ui, theme, "Notify me", true);
            },
        );
        bring(&mut harness, "Notify me", Shown::Hover);
        let rect = harness.get_by_label("Notify me").rect();
        let paint = painted(&harness, "Notify me");
        let look = own(&paint, rect);
        assert_eq!((look.fill, look.edge), (Some(p.ink), None), "{mode:?} {look:#?}");
        assert_eq!(knobs(&paint, rect), vec![(Rect::from_min_size(rect.min + vec2(14.0, 2.0), vec2(12.0, 12.0)), p.bg)], "{mode:?}");
    }
}

#[test]
fn an_unavailable_switch_says_why_in_its_tooltip() {
    let theme = Theme::new(Mode::Light, TextScale::default());
    let mut harness = staged(
        theme,
        |p| p.surface,
        vec2(400.0, 200.0),
        |ui, theme| {
            switch_noted(ui, theme, "Start with the computer", SwitchState::Unavailable, Some("Not there in the Flatpak yet"));
        },
    );
    assert!(harness.query_by_label_contains("Not there in the Flatpak yet").is_none(), "no reason before the pointer comes");
    bring(&mut harness, "Start with the computer", Shown::Hover);
    for _ in 0..90 {
        harness.step();
    }
    let reason = harness.get_by_label_contains("Not there in the Flatpak yet").rect();
    let switch = harness.get_by_role_and_label(Role::Switch, "Start with the computer").rect();
    assert!(reason.top() >= switch.bottom(), "the reason under the switch: {reason:?} {switch:?}");
    assert!(harness.get_by_role_and_label(Role::Switch, "Start with the computer").accesskit_node().is_disabled());
}

#[test]
fn a_switch_without_a_note_shows_no_tooltip() {
    let theme = Theme::new(Mode::Light, TextScale::default());
    let mut harness = staged(
        theme,
        |p| p.surface,
        vec2(400.0, 200.0),
        |ui, theme| {
            switch_noted(ui, theme, "Start with the computer", SwitchState::On, None);
        },
    );
    bring(&mut harness, "Start with the computer", Shown::Hover);
    for _ in 0..90 {
        harness.step();
    }
    assert_eq!(harness.query_all_by_label("Start with the computer").count(), 1, "only the switch");
}
