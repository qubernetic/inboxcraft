// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::accesskit::{Role, Toggled};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::{IconButton, TextButton};
use crate::component::Shown;
use crate::component::control_side;
use crate::component::tests::{Own, bring, builder, drew_icon, icons, own, painted, staged, theme};
use crate::iconset::IconId;
use crate::keymap::Keymap;
use crate::theme::{Mode, TextScale, Theme};

/// How often each of the three buttons was pressed.
type Presses = [u32; 3];

fn harness() -> Harness<'static, Presses> {
    let theme = theme();
    let keymap = Keymap::default();
    let mut harness = builder().build_ui_state(
        move |ui, presses: &mut Presses| {
            let buttons = [
                IconButton::new(IconId::Archive, "Archive"),
                IconButton::new(IconId::Pin, "Unpin").active(true),
                IconButton::new(IconId::Reply, "Reply").enabled(false),
            ];
            for (button, count) in buttons.into_iter().zip(presses.iter_mut()) {
                if button.show(ui, &theme, &keymap).clicked() {
                    *count += 1;
                }
            }
        },
        [0; 3],
    );
    harness.run();
    harness
}

#[test]
fn an_icon_button_is_a_named_button_that_can_be_pressed() {
    let mut harness = harness();
    let archive = harness.get_by_label("Archive");
    assert_eq!(archive.accesskit_node().role(), Role::Button);
    archive.click();
    harness.run();
    assert_eq!(harness.state(), &[1, 0, 0]);
}

#[test]
fn a_disabled_button_takes_no_press_and_says_so() {
    let mut harness = harness();
    let reply = harness.get_by_label("Reply");
    assert!(reply.accesskit_node().is_disabled());
    reply.click();
    harness.run();
    assert_eq!(harness.state(), &[0, 0, 0]);
}

#[test]
fn an_active_button_is_selected_and_shows_its_icon() {
    let harness = harness();
    assert_eq!(harness.get_by_label("Unpin").accesskit_node().toggled(), Some(Toggled::True));
    assert_eq!(harness.get_by_label("Archive").accesskit_node().toggled(), Some(Toggled::False));
    assert!(drew_icon(harness.output(), IconId::Pin));
    assert!(drew_icon(harness.output(), IconId::Archive));
}

#[test]
fn the_button_grows_with_the_text_scale() {
    let sides: Vec<f32> = [TextScale::ALL[0], TextScale::ALL[4]]
        .into_iter()
        .map(|scale| {
            let theme = theme().with_text_scale(scale);
            let mut harness = builder().build_ui_state(
                move |ui, side: &mut f32| {
                    crate::theme::install_fonts(ui.ctx());
                    *side = control_side(&theme);
                    IconButton::new(IconId::Done, "Done").show(ui, &theme, &Keymap::default());
                },
                0.0,
            );
            harness.run();
            let side = *harness.state();
            let rect = harness.get_by_label("Done").rect();
            assert!((rect.width() - side).abs() < 0.5 && (rect.height() - side).abs() < 0.5, "{rect:?} {side}");
            side
        })
        .collect();
    assert!(sides[1] > sides[0] * 1.2, "{sides:?}");
}

/// Every text shape of the last frame of `output`: its text and its first glyph colour.
fn text_colours(output: &egui::FullOutput) -> Vec<(String, egui::Color32)> {
    fn open(shape: &egui::Shape, out: &mut Vec<(String, egui::Color32)>) {
        match shape {
            egui::Shape::Vec(inner) => inner.iter().for_each(|shape| open(shape, out)),
            egui::Shape::Text(text) => {
                let colour = text.galley.rows.iter().flat_map(|row| row.visuals.mesh.vertices.iter()).map(|vertex| vertex.color).next();
                out.push((text.galley.text().to_owned(), colour.unwrap_or(text.fallback_color)));
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    output.shapes.iter().for_each(|clipped| open(&clipped.shape, &mut out));
    out
}

#[test]
fn a_button_on_a_gradient_draws_its_icon_in_the_ink_it_is_given() {
    let theme = theme();
    let ink = egui::Color32::WHITE;
    for active in [false, true] {
        let mut harness = builder().build_ui(move |ui| {
            IconButton::new(IconId::Settings, "Settings").ink(ink).active(active).show(ui, &theme, &Keymap::default());
        });
        harness.run();
        assert!(drew_icon(harness.output(), IconId::Settings));
        assert!(text_colours(harness.output()).iter().all(|(_, colour)| *colour == ink), "{active}");
        assert_eq!(harness.get_by_label("Settings").accesskit_node().toggled(), Some(if active { Toggled::True } else { Toggled::False }));
    }
}

#[test]
fn a_text_button_is_a_named_button_with_a_tooltip_and_the_primary_one_is_on_cyan() {
    let theme = theme();
    let mut harness = builder().build_ui_state(
        move |ui, presses: &mut [u32; 2]| {
            if super::text(ui, &theme, &Keymap::default(), "Done all", None, false).clicked() {
                presses[0] += 1;
            }
            if super::text(ui, &theme, &Keymap::default(), "Review", Some(crate::keymap::FIND), true).clicked() {
                presses[1] += 1;
            }
        },
        [0; 2],
    );
    harness.run();
    let colours = text_colours(harness.output());
    let colour = |name: &str| colours.iter().find(|(text, _)| text == name).map(|(_, colour)| *colour);
    assert_eq!(colour("Done all"), Some(theme.palette.ink));
    assert_eq!(colour("Review"), Some(theme.palette.on_cyan));
    harness.get_by_role_and_label(Role::Button, "Review").click();
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Done all").click();
    harness.run();
    assert_eq!(harness.state(), &[1, 1]);
    crate::component::tests::quick_tooltips(&harness.ctx);
    harness.get_by_role_and_label(Role::Button, "Review").hover();
    harness.run();
    harness.get_by_label("Ctrl F");
}

#[test]
fn an_unavailable_button_takes_no_press_and_says_why() {
    let theme = theme();
    let mut harness = builder().build_ui_state(
        move |ui, presses: &mut u32| {
            let keymap = Keymap::default();
            if IconButton::new(IconId::Snooze, "Snooze").unavailable("Not available yet").show(ui, &theme, &keymap).clicked() {
                *presses += 1;
            }
            if super::unavailable(ui, &theme, &keymap, "Reply", true, "Not yet either").clicked() {
                *presses += 1;
            }
        },
        0,
    );
    crate::component::tests::quick_tooltips(&harness.ctx);
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Snooze").hover();
    harness.run();
    harness.get_by_label("Not available yet");
    harness.get_by_role_and_label(Role::Button, "Reply").hover();
    harness.run();
    harness.get_by_label("Not yet either");
    for name in ["Snooze", "Reply"] {
        let button = harness.get_by_role_and_label(Role::Button, name);
        assert!(button.accesskit_node().is_disabled(), "{name}");
        button.click();
        harness.run();
    }
    assert_eq!(*harness.state(), 0);
}

/// Primary with an icon, secondary, and a primary that is unavailable; how often each was pressed.
fn text_harness() -> Harness<'static, Presses> {
    let theme = theme();
    let keymap = Keymap::default();
    let mut harness = builder().build_ui_state(
        move |ui, presses: &mut Presses| {
            let buttons = [
                TextButton::new("Reply").primary().icon(IconId::Reply),
                TextButton::new("Forward").secondary().icon(IconId::Forward),
                TextButton::new("Check again").primary().unavailable("Not yet"),
            ];
            for (button, count) in buttons.into_iter().zip(presses.iter_mut()) {
                if button.show(ui, &theme, &keymap).clicked() {
                    *count += 1;
                }
            }
        },
        [0; 3],
    );
    harness.run();
    harness
}

#[test]
fn text_buttons_of_every_tone_are_named_buttons_with_their_icons() {
    let mut harness = text_harness();
    for name in ["Reply", "Forward"] {
        let button = harness.get_by_label(name);
        assert_eq!(button.accesskit_node().role(), Role::Button);
        button.click();
    }
    harness.run();
    assert_eq!(harness.state(), &[1, 1, 0]);
    assert!(drew_icon(harness.output(), IconId::Reply));
    assert!(drew_icon(harness.output(), IconId::Forward));
}

#[test]
fn an_unavailable_text_button_takes_no_press() {
    let mut harness = text_harness();
    let again = harness.get_by_label("Check again");
    assert!(again.accesskit_node().is_disabled());
    again.click();
    harness.run();
    assert_eq!(harness.state(), &[0, 0, 0]);
}

#[test]
fn a_danger_button_is_filled_with_the_error_colour_and_reads_on_it() {
    let theme = theme();
    let mut harness = builder().build_ui(move |ui| {
        TextButton::new("Block").danger().show(ui, &theme, &Keymap::default());
    });
    harness.run();
    let colours = text_colours(harness.output());
    assert_eq!(colours.iter().find(|(text, _)| text == "Block").map(|(_, colour)| *colour), Some(theme.palette.bg));
    harness.get_by_role_and_label(Role::Button, "Block");
}

/// The states of the icon button's sheet, in its order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IconState {
    Default,
    Hover,
    Focus,
    Pressed,
    Selected,
    Disabled,
    Unavailable,
}

const ICON_STATES: [IconState; 7] =
    [IconState::Default, IconState::Hover, IconState::Focus, IconState::Pressed, IconState::Selected, IconState::Disabled, IconState::Unavailable];

/// The icon button "Pin" in `state` on a `surface` stage, at `scale`, in `mode`: its rect and its own look.
fn icon_button_in(mode: Mode, scale: TextScale, state: IconState) -> (egui::Rect, Own) {
    let theme = Theme::new(mode, scale);
    let mut harness = staged(
        theme,
        |p| p.surface,
        egui::vec2(200.0, 200.0),
        move |ui, theme| {
            let button = IconButton::new(IconId::Pin, "Pin");
            let button = match state {
                IconState::Selected => button.active(true),
                IconState::Disabled => button.enabled(false),
                IconState::Unavailable => button.unavailable("Not yet: choose a message first"),
                _ => button,
            };
            button.show(ui, theme, &Keymap::default());
        },
    );
    let shown = match state {
        IconState::Hover => Shown::Hover,
        IconState::Focus => Shown::Focus,
        IconState::Pressed => Shown::Pressed,
        _ => Shown::Rest,
    };
    bring(&mut harness, "Pin", shown);
    let rect = harness.get_by_label("Pin").rect();
    (rect, own(&painted(&harness, "Pin"), rect))
}

#[test]
fn an_icon_button_looks_as_its_sheet_says_in_every_state() {
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::default());
        let p = theme.palette;
        for state in ICON_STATES {
            let (rect, look) = icon_button_in(mode, TextScale::default(), state);
            let at = format!("{mode:?} {state:?}: {look:#?}");
            assert!((rect.width() - 32.0).abs() < 0.01 && (rect.height() - 32.0).abs() < 0.01, "{at} {rect:?}");
            let (fill, edge, glyph) = match state {
                IconState::Default | IconState::Focus => (None, None, p.muted),
                IconState::Hover => (Some(p.stage), None, p.ink),
                IconState::Pressed => (Some(p.stage), Some((1.0, p.muted, false)), p.ink),
                IconState::Selected => (Some(p.cyan_muted), None, p.cyan_text),
                IconState::Disabled | IconState::Unavailable => (None, None, p.dim),
            };
            assert_eq!(look.fill, fill, "{at}");
            assert_eq!(look.edge, edge, "{at}");
            let ring = (state == IconState::Focus).then_some((rect.expand(3.0), 1.0, p.cyan_edge));
            assert_eq!(look.ring, ring, "{at}");
            assert_eq!(look.texts.len(), 1, "{at}");
            let (_, colour, family, size) = &look.texts[0];
            assert_eq!((*colour, family.clone(), *size), (glyph, icons(), 16.0), "{at}");
            assert!(!look.overflows, "{at}");
            assert!(look.radius <= 0.0, "{at}");
        }
    }
}

#[test]
fn an_icon_button_at_130_percent_is_the_scaled_side() {
    let theme = Theme::new(Mode::Dark, TextScale::ALL[4]);
    let side = theme.boxes().button;
    for state in ICON_STATES {
        let (rect, _) = icon_button_in(Mode::Dark, TextScale::ALL[4], state);
        assert!((rect.width() - side).abs() < 0.01 && (rect.height() - side).abs() < 0.01, "{state:?} {rect:?}");
    }
}

#[test]
fn an_icon_button_held_down_by_enter_is_pressed() {
    let theme = Theme::new(Mode::Light, TextScale::default());
    let mut harness = staged(
        theme,
        |p| p.surface,
        egui::vec2(200.0, 200.0),
        |ui, theme| {
            IconButton::new(IconId::Pin, "Pin").show(ui, theme, &Keymap::default());
        },
    );
    bring(&mut harness, "Pin", Shown::Focus);
    harness.key_down(egui::Key::Enter);
    harness.run();
    let rect = harness.get_by_label("Pin").rect();
    let look = own(&painted(&harness, "Pin"), rect);
    assert_eq!(look.edge, Some((1.0, theme.palette.muted, false)), "{look:#?}");
    assert_eq!(look.fill, Some(theme.palette.stage), "{look:#?}");
}
