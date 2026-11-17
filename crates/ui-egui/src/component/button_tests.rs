// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::accesskit::{Role, Toggled};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::{IconButton, LinkSize, TextButton, Tone};
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
                // A galley laid out in the placeholder colour takes the painter's colour.
                let colour = colour.filter(|colour| *colour != egui::Color32::PLACEHOLDER);
                out.push((text.galley.text().to_owned(), text.override_text_color.or(colour).unwrap_or(text.fallback_color)));
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
fn a_danger_button_has_the_minus_edge_and_text() {
    let theme = theme();
    let mut harness = builder().build_ui(move |ui| {
        TextButton::new("Block").danger().show(ui, &theme, &Keymap::default());
    });
    harness.run();
    let colours = text_colours(harness.output());
    assert_eq!(colours.iter().find(|(text, _)| text == "Block").map(|(_, colour)| *colour), Some(theme.palette.minus));
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

/// The states of the text button's sheet, in its order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextState {
    Default,
    Hover,
    Focus,
    Pressed,
    Disabled,
    Unavailable,
}

const TEXT_STATES: [TextState; 6] =
    [TextState::Default, TextState::Hover, TextState::Focus, TextState::Pressed, TextState::Disabled, TextState::Unavailable];

/// The four tones with the sheet's label of each and its width there.
const TONES: [(Tone, &str, f32); 4] =
    [(Tone::Primary, "Send", 56.0), (Tone::Secondary, "Reply all", 95.0), (Tone::Danger, "Delete", 71.0), (Tone::Plain, "Discard", 79.0)];

/// A keymap with Ctrl Enter for sending.
fn send_keys() -> Keymap {
    let mut keymap = Keymap::default();
    keymap.bind("mail.send", egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Enter));
    keymap
}

/// `button` (labelled `label`) on a `surface` stage in `mode` at `scale`, brought into `shown`: its rect and its look.
fn text_button_in(
    mode: Mode,
    scale: TextScale,
    label: &'static str,
    shown: Shown,
    button: impl Fn() -> TextButton<'static> + 'static,
) -> (egui::Rect, Own) {
    let theme = Theme::new(mode, scale);
    let mut harness = staged(
        theme,
        |p| p.surface,
        egui::vec2(400.0, 120.0),
        move |ui, theme| {
            button().show(ui, theme, &send_keys());
        },
    );
    bring(&mut harness, label, shown);
    let rect = harness.get_by_label(label).rect();
    (rect, own(&painted(&harness, label), rect))
}

/// The text button `label` in `tone` and `state`.
fn toned(label: &'static str, tone: Tone, state: TextState) -> impl Fn() -> TextButton<'static> + 'static {
    move || {
        let button = TextButton::new(label);
        let button = match tone {
            Tone::Primary => button.primary(),
            Tone::Secondary => button.secondary(),
            Tone::Danger => button.danger(),
            Tone::Plain => button.plain(),
        };
        match state {
            TextState::Disabled => button.enabled(false),
            TextState::Unavailable => button.unavailable("Not yet: add a recipient first"),
            _ => button,
        }
    }
}

fn shown_of(state: TextState) -> Shown {
    match state {
        TextState::Hover => Shown::Hover,
        TextState::Focus => Shown::Focus,
        TextState::Pressed => Shown::Pressed,
        _ => Shown::Rest,
    }
}

/// The label's text in `look`: its colour, family and size.
fn label_of(look: &Own, label: &str) -> Option<(egui::Color32, String, f32)> {
    look.texts.iter().find(|(text, ..)| text == label).map(|(_, colour, family, size)| (*colour, family.clone(), *size))
}

#[test]
fn a_text_button_looks_as_its_sheet_says_in_every_tone_and_state() {
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::default());
        let p = theme.palette;
        let font = theme.fonts.button;
        for (tone, label, width) in TONES {
            for state in TEXT_STATES {
                let (rect, look) = text_button_in(mode, TextScale::default(), label, shown_of(state), toned(label, tone, state));
                let at = format!("{mode:?} {tone:?} {state:?}: {look:#?}");
                assert!((rect.width() - width).abs() <= 1.0 && (rect.height() - 32.0).abs() < 0.01, "{at} {rect:?}");
                let dashed = (1.0, p.line, true);
                let (fill, edge, ink) = match (tone, state) {
                    (Tone::Plain, TextState::Disabled | TextState::Unavailable) => (None, None, p.muted),
                    (_, TextState::Disabled | TextState::Unavailable) => (None, Some(dashed), p.muted),
                    (Tone::Primary, TextState::Default | TextState::Focus) => (Some(p.cyan), None, p.on_cyan),
                    (Tone::Primary, TextState::Hover) => (Some(p.ink), None, p.bg),
                    (Tone::Primary, TextState::Pressed) => (Some(p.cyan_muted), Some((1.0, p.cyan_edge, false)), p.cyan_text),
                    (Tone::Secondary, TextState::Default | TextState::Focus) => (None, Some((1.0, p.line, false)), p.ink),
                    (Tone::Secondary, TextState::Hover) => (Some(p.stage), Some((1.0, p.muted, false)), p.ink),
                    (Tone::Secondary, TextState::Pressed) => (Some(p.stage), Some((1.0, p.ink, false)), p.ink),
                    (Tone::Danger, TextState::Default | TextState::Focus) => (None, Some((1.0, p.minus, false)), p.minus),
                    (Tone::Danger, TextState::Hover) => (Some(p.minus), None, p.bg),
                    (Tone::Danger, TextState::Pressed) => (Some(p.stage), Some((1.0, p.minus, false)), p.minus),
                    (Tone::Plain, TextState::Default | TextState::Focus) => (None, None, p.ink),
                    (Tone::Plain, TextState::Hover) => (Some(p.stage), None, p.ink),
                    (Tone::Plain, TextState::Pressed) => (Some(p.stage), Some((1.0, p.muted, false)), p.ink),
                };
                assert_eq!(look.fill, fill, "{at}");
                assert_eq!(look.edge, edge, "{at}");
                let ring = (state == TextState::Focus).then_some((rect.expand(3.0), 1.0, p.cyan_edge));
                assert_eq!(look.ring, ring, "{at}");
                assert_eq!(label_of(&look, label), Some((ink, font.id().family.to_string(), font.size)), "{at}");
                assert_eq!(look.texts.len(), 1, "{at}");
                assert!(!look.overflows, "{at}");
                assert!(look.radius <= 0.0, "{at}");
            }
        }
    }
}

#[test]
fn a_text_button_with_an_arrow_a_key_an_icon_or_a_long_label_looks_as_its_sheet_says() {
    const LONG: &str = "Ask for a receipt when this message is read by every recipient";
    for mode in [Mode::Dark, Mode::Light] {
        let p = Theme::new(mode, TextScale::default()).palette;
        let rest = |label, button: fn() -> TextButton<'static>| text_button_in(mode, TextScale::default(), label, Shown::Rest, button);
        let (rect, look) = rest("Continue", || TextButton::new("Continue").primary().arrow());
        assert!((rect.width() - 103.0).abs() <= 1.0, "{rect:?}");
        assert_eq!(
            look.texts.iter().map(|(text, colour, ..)| (text.as_str(), *colour)).collect::<Vec<_>>(),
            [("Continue", p.on_cyan), ("→", p.on_cyan)]
        );
        let (rect, look) = rest("Send", || TextButton::new("Send").primary().action("mail.send").key(true));
        assert!((rect.width() - 140.0).abs() <= 1.0, "{rect:?}");
        assert_eq!(look.fill, Some(p.cyan), "{look:#?}");
        assert!(look.texts.iter().any(|(text, colour, ..)| text == "Ctrl Enter" && *colour == p.on_cyan), "{look:#?}");
        assert!(look.radius <= 2.0, "only the chip is rounded: {look:#?}");
        let (rect, look) = rest("Attach", || TextButton::new("Attach").secondary().icon(IconId::Attach));
        assert!((rect.width() - 95.0).abs() <= 1.0, "{rect:?}");
        assert!(look.texts.iter().any(|(_, colour, family, size)| *colour == p.ink && *family == icons() && *size == 16.0), "{look:#?}");
        for button in [(|| TextButton::new(LONG).primary()) as fn() -> TextButton<'static>, || TextButton::new(LONG).secondary()] {
            let (rect, look) = rest(LONG, button);
            assert!((rect.width() - 264.0).abs() <= 1.0, "{rect:?}");
            assert!(!look.overflows, "{look:#?}");
        }
    }
}

#[test]
fn a_text_button_at_130_percent_is_the_scaled_height() {
    let theme = Theme::new(Mode::Light, TextScale::ALL[4]);
    for (tone, label, _) in TONES {
        for state in TEXT_STATES {
            let (rect, _) = text_button_in(Mode::Light, TextScale::ALL[4], label, shown_of(state), toned(label, tone, state));
            assert!((rect.height() - theme.boxes().button).abs() < 0.01, "{tone:?} {state:?} {rect:?}");
        }
    }
}

#[test]
fn a_pressed_button_differs_from_a_hovered_one() {
    for mode in [Mode::Dark, Mode::Light] {
        for (tone, label, _) in TONES {
            let (_, hovered) = text_button_in(mode, TextScale::default(), label, Shown::Hover, toned(label, tone, TextState::Hover));
            let (_, pressed) = text_button_in(mode, TextScale::default(), label, Shown::Pressed, toned(label, tone, TextState::Pressed));
            assert_ne!(hovered, pressed, "{mode:?} {tone:?}");
        }
        let (_, hovered) = icon_button_in(mode, TextScale::default(), IconState::Hover);
        let (_, pressed) = icon_button_in(mode, TextScale::default(), IconState::Pressed);
        assert_ne!(hovered, pressed, "{mode:?} icon button");
    }
}

/// The states of the link's sheet, in its order: the line size has all five, the small one no pressed.
const LINK_STATES: [(Shown, bool); 5] =
    [(Shown::Rest, true), (Shown::Hover, true), (Shown::Focus, true), (Shown::Pressed, true), (Shown::Rest, false)];

/// The link `label` in `size`, `enabled` or not, on a `surface` stage, brought into `shown`: its rect and its look.
fn link_in(mode: Mode, scale: TextScale, label: &'static str, size: LinkSize, enabled: bool, shown: Shown) -> (egui::Rect, Own) {
    let theme = Theme::new(mode, scale);
    let mut harness = staged(
        theme,
        |p| p.surface,
        egui::vec2(300.0, 100.0),
        move |ui, theme| {
            super::link_with(ui, theme, &Keymap::default(), label, size, enabled, None);
        },
    );
    if enabled {
        bring(&mut harness, label, shown);
    }
    let rect = harness.get_by_label(label).rect();
    (rect, own(&painted(&harness, label), rect))
}

#[test]
fn a_link_looks_as_its_sheet_says_in_every_size_and_state() {
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::default());
        let p = theme.palette;
        for (size, label, width, height, font) in
            [(LinkSize::Line, "Show details", 102.0, 24.0, theme.fonts.button), (LinkSize::Small, "open log", 53.0, 16.0, theme.fonts.mono_s)]
        {
            for (shown, enabled) in LINK_STATES {
                if size == LinkSize::Small && shown == Shown::Pressed {
                    continue;
                }
                let (rect, look) = link_in(mode, TextScale::default(), label, size, enabled, shown);
                let at = format!("{mode:?} {size:?} {shown:?} {enabled}: {look:#?}");
                assert!((rect.width() - width).abs() <= 1.0 && (rect.height() - height).abs() < 0.01, "{at} {rect:?}");
                let ink = if enabled { p.cyan_text } else { p.muted };
                assert_eq!(look.texts, vec![(label.to_owned(), ink, font.id().family.to_string(), font.size)], "{at}");
                let underlined = enabled && matches!(shown, Shown::Hover | Shown::Pressed);
                assert_eq!(look.lines, if underlined { vec![p.cyan_text] } else { Vec::new() }, "{at}");
                assert_eq!(look.fill, (enabled && shown == Shown::Pressed).then_some(p.cyan_muted), "{at}");
                assert_eq!(look.edge, None, "{at}");
                let ring = (enabled && shown == Shown::Focus).then_some((rect.expand(3.0), 1.0, p.cyan_edge));
                assert_eq!(look.ring, ring, "{at}");
                assert!(!look.overflows, "{at}");
                assert!(look.radius <= 0.0, "{at}");
            }
        }
    }
}

#[test]
fn a_link_at_130_percent_is_the_scaled_height() {
    let theme = Theme::new(Mode::Dark, TextScale::ALL[4]);
    for (size, height) in [(LinkSize::Line, theme.boxes().link), (LinkSize::Small, theme.boxes().row_link)] {
        let (rect, _) = link_in(Mode::Dark, TextScale::ALL[4], "open", size, true, Shown::Rest);
        assert!((rect.height() - height).abs() < 0.01, "{size:?} {rect:?}");
    }
}

/// The icon button "Pin", selected, on a `surface` stage, in its rail ink when `ink` is given, brought into `shown`.
fn selected_in(mode: Mode, ink: Option<egui::Color32>, shown: Shown) -> (egui::Rect, Own) {
    let theme = Theme::new(mode, TextScale::default());
    let mut harness = staged(
        theme,
        |p| p.surface,
        egui::vec2(200.0, 200.0),
        move |ui, theme| {
            let button = IconButton::new(IconId::Pin, "Pin").active(true);
            let button = match ink {
                Some(ink) => button.ink(ink),
                None => button,
            };
            button.show(ui, theme, &Keymap::default());
        },
    );
    bring(&mut harness, "Pin", shown);
    let rect = harness.get_by_label("Pin").rect();
    (rect, own(&painted(&harness, "Pin"), rect))
}

#[test]
fn a_selected_icon_button_pressed_keeps_the_wash_and_takes_the_accent_edge() {
    for mode in [Mode::Dark, Mode::Light] {
        let p = Theme::new(mode, TextScale::default()).palette;
        for ink in [None, Some(p.ink)] {
            let (_, rest) = selected_in(mode, ink, Shown::Rest);
            let (_, hovered) = selected_in(mode, ink, Shown::Hover);
            let (_, pressed) = selected_in(mode, ink, Shown::Pressed);
            let at = format!("{mode:?} {ink:?}: {pressed:#?}");
            assert_eq!(pressed.fill, Some(p.cyan_muted), "{at}");
            assert_eq!(pressed.edge, Some((1.0, p.cyan_edge, false)), "{at}");
            assert_eq!(pressed.texts[0].1, ink.unwrap_or(p.cyan_text), "{at}");
            assert_ne!(pressed, rest, "{at}");
            assert_ne!(pressed, hovered, "{at}");
        }
    }
}

/// The widget `label` that `add` draws on a `surface` stage in `mode`, held down by the pointer (`key` is `None`) or by
/// `key` while it has the focus: its rect and its look.
fn held_by(mode: Mode, label: &'static str, key: Option<egui::Key>, add: impl Fn(&mut egui::Ui, &Theme) + 'static) -> (egui::Rect, Own) {
    let theme = Theme::new(mode, TextScale::default());
    let mut harness = staged(theme, |p| p.surface, egui::vec2(400.0, 120.0), add);
    match key {
        None => bring(&mut harness, label, Shown::Pressed),
        Some(key) => {
            bring(&mut harness, label, Shown::Focus);
            harness.key_down(key);
            harness.run();
        }
    }
    let rect = harness.get_by_label(label).rect();
    (rect, own(&painted(&harness, label), rect))
}

/// How a test draws one control.
type Draw = Box<dyn Fn(&mut egui::Ui, &Theme)>;

/// Every pressable control of these sets but the section label (its own test): its label and how to draw it.
fn pressables() -> Vec<(&'static str, Draw)> {
    let mut all: Vec<(&'static str, Draw)> = vec![
        ("Pin", Box::new(|ui, theme| drop(IconButton::new(IconId::Pin, "Pin").show(ui, theme, &Keymap::default())))),
        ("Pinned", Box::new(|ui, theme| drop(IconButton::new(IconId::Pin, "Pinned").active(true).show(ui, theme, &Keymap::default())))),
        ("Show details", Box::new(|ui, theme| drop(super::link_sized(ui, theme, &Keymap::default(), "Show details", LinkSize::Line)))),
        ("open log", Box::new(|ui, theme| drop(super::link_sized(ui, theme, &Keymap::default(), "open log", LinkSize::Small)))),
    ];
    for (tone, label, _) in TONES {
        all.push((label, Box::new(move |ui, theme| drop(TextButton::new(label).tone(tone).show(ui, theme, &Keymap::default())))));
    }
    all
}

#[test]
fn enter_and_space_each_give_the_pressed_look_to_the_focused_control() {
    for mode in [Mode::Dark, Mode::Light] {
        let p = Theme::new(mode, TextScale::default()).palette;
        for (index, (label, _)) in pressables().into_iter().enumerate() {
            let draw = move |ui: &mut egui::Ui, theme: &Theme| (pressables().swap_remove(index).1)(ui, theme);
            let (_, by_pointer) = held_by(mode, label, None, draw);
            let (_, at_rest) = held_by(mode, label, Some(egui::Key::A), draw);
            for key in [egui::Key::Enter, egui::Key::Space] {
                let (rect, look) = held_by(mode, label, Some(key), draw);
                let at = format!("{mode:?} {label} {key:?}: {look:#?} pointer {by_pointer:#?}");
                assert_eq!(
                    (look.fill, look.edge, &look.texts, &look.lines),
                    (by_pointer.fill, by_pointer.edge, &by_pointer.texts, &by_pointer.lines),
                    "{at}"
                );
                assert_ne!((look.fill, look.edge, &look.lines), (at_rest.fill, at_rest.edge, &at_rest.lines), "pressed differs from focus: {at}");
                assert_eq!(look.ring, Some((rect.expand(3.0), 1.0, p.cyan_edge)), "{at}");
            }
        }
    }
}

/// A control of the hover fade test: its label, how to draw it, its ground, and the one colour the fade moves, read
/// from its look (`None`: not painted, so the ground shows).
type Faded = (&'static str, Draw, fn(&crate::theme::Palette) -> egui::Color32, fn(&Own) -> Option<egui::Color32>);

fn faded() -> Vec<Faded> {
    vec![
        (
            "Pin",
            Box::new(|ui, theme| drop(IconButton::new(IconId::Pin, "Pin").show(ui, theme, &Keymap::default()))),
            |p| p.surface,
            |o| o.texts.first().map(|t| t.1),
        ),
        ("Send", Box::new(|ui, theme| drop(TextButton::new("Send").primary().show(ui, theme, &Keymap::default()))), |p| p.surface, |o| o.fill),
        (
            "Reply all",
            Box::new(|ui, theme| drop(TextButton::new("Reply all").secondary().show(ui, theme, &Keymap::default()))),
            |p| p.surface,
            |o| o.edge.map(|e| e.1),
        ),
        (
            "Delete",
            Box::new(|ui, theme| drop(TextButton::new("Delete").danger().show(ui, theme, &Keymap::default()))),
            |p| p.surface,
            |o| o.texts.first().map(|t| t.1),
        ),
        ("Discard", Box::new(|ui, theme| drop(TextButton::new("Discard").plain().show(ui, theme, &Keymap::default()))), |p| p.surface, |o| o.fill),
        (
            "Show details",
            Box::new(|ui, theme| drop(super::link(ui, theme, &Keymap::default(), "Show details"))),
            |p| p.surface,
            |o| o.lines.first().copied(),
        ),
    ]
}

/// The fade frames of the control at `index` of [`faded`] with motion `reduced` or full: (rest, hover, on, off).
fn fade_of(mode: Mode, index: usize, reduced: bool) -> (Own, Own, Vec<Own>, Vec<Own>) {
    let theme = Theme::new(mode, TextScale::default());
    let label = faded()[index].0;
    let draw = move |ui: &mut egui::Ui, theme: &Theme| (faded().swap_remove(index).1)(ui, theme);
    let still = |shown| {
        let mut harness = staged(theme, faded()[index].2, egui::vec2(400.0, 120.0), draw);
        bring(&mut harness, label, shown);
        let rect = harness.get_by_label(label).rect();
        own(&painted(&harness, label), rect)
    };
    let mut harness = crate::component::tests::staged_motion(theme, reduced, faded()[index].2, egui::vec2(400.0, 120.0), draw);
    let (on, off) = crate::component::tests::hover_frames(&mut harness, label, 10);
    assert!(crate::component::tests::rests(&mut harness, 2), "{mode:?} {label} asks for no repaint once the fade has ended");
    (still(Shown::Rest), still(Shown::Hover), on, off)
}

#[test]
fn at_full_motion_hover_cross_fades_over_125_ms_both_ways() {
    for mode in [Mode::Dark, Mode::Light] {
        let p = Theme::new(mode, TextScale::default()).palette;
        // Only the text button fades: the presentation's `.btn` transition.
        for index in (0..faded().len()).filter(|index| !INSTANT.contains(&faded()[*index].0)) {
            let (label, _, ground, colour) = faded().swap_remove(index);
            let p_ref = &p;
            let (rest, hover, on, off) = fade_of(mode, index, false);
            let (from, to) = (colour(&rest).unwrap_or(ground(p_ref)), colour(&hover).unwrap_or(ground(p_ref)));
            // Frame 4 is 4/60 s, half-way; frame 8 is past 125 ms.
            for (frames, a, b, end) in [(&on, from, to, &hover), (&off, to, from, &rest)] {
                let mid = colour(&frames[3]).unwrap_or(ground(p_ref));
                assert!(crate::component::tests::strictly_between(mid, a, b), "{mode:?} {label}: {mid:?} not between {a:?} and {b:?}");
                assert_eq!(&frames[8], end, "{mode:?} {label}: the fade ends at the value itself");
            }
        }
    }
}

/// The controls the presentation gives no transition: their hover is at once at every level of motion.
const INSTANT: [&str; 2] = ["Pin", "Show details"];

#[test]
fn at_full_motion_an_icon_button_and_a_links_underline_change_at_once() {
    for mode in [Mode::Dark, Mode::Light] {
        for index in (0..faded().len()).filter(|index| INSTANT.contains(&faded()[*index].0)) {
            let label = faded()[index].0;
            let (rest, hover, on, off) = fade_of(mode, index, false);
            assert_eq!(on[0], hover, "{mode:?} {label}: the first hovered frame");
            assert_eq!(off[0], rest, "{mode:?} {label}: the first frame after leaving");
        }
    }
}

#[test]
fn at_reduced_motion_hover_follows_the_pointer_in_the_same_frame() {
    for mode in [Mode::Dark, Mode::Light] {
        for index in 0..faded().len() {
            let label = faded()[index].0;
            let (rest, hover, on, off) = fade_of(mode, index, true);
            assert_eq!(on[0], hover, "{mode:?} {label}: the first hovered frame");
            assert_eq!(off[0], rest, "{mode:?} {label}: the first frame after leaving");
        }
    }
}
