// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::{Key, KeyboardShortcut, Modifiers};
use egui_kittest::kittest::Queryable as _;

use super::{ChipState, key_chip, key_words};
use crate::component::tests::{own, painted, staged};
use crate::theme::{Mode, TextScale, Theme};

#[test]
fn keys_are_written_as_words() {
    let words = |modifiers, key| key_words(&KeyboardShortcut::new(modifiers, key));
    assert_eq!(words(Modifiers::CTRL, Key::Enter), "Ctrl Enter");
    assert_eq!(words(Modifiers::COMMAND, Key::Enter), "Ctrl Enter");
    assert_eq!(words(Modifiers::NONE, Key::Delete), "Del");
    assert_eq!(words(Modifiers::NONE, Key::ArrowDown), "↓");
    assert_eq!(words(Modifiers::CTRL, Key::Comma), "Ctrl Comma");
    assert_eq!(words(Modifiers::NONE, Key::Escape), "Esc");
    assert_eq!(words(Modifiers { alt: true, shift: true, ..Modifiers::CTRL }, Key::K), "Ctrl Alt Shift K");
    assert_eq!(words(Modifiers::NONE, Key::A), "A");
}

/// The states of the key chip's sheet, in its order, with the key each shows there.
const STATES: [(&str, ChipState, &str); 5] = [
    ("default", ChipState::Default, "A"),
    ("on-cyan", ChipState::OnCyan, "Ctrl Enter"),
    ("capture", ChipState::Capture, "press the keys"),
    ("none", ChipState::Disabled, "none"),
    ("disabled", ChipState::Disabled, "H"),
];

#[test]
fn a_key_chip_looks_as_its_sheet_says_in_every_state() {
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::default());
        let p = theme.palette;
        for (name, state, key) in STATES {
            // The on-cyan chip stands on the primary button's fill.
            let ground: fn(&crate::theme::Palette) -> egui::Color32 = if state == ChipState::OnCyan { |p| p.cyan } else { |p| p.surface };
            let harness = staged(theme, ground, egui::vec2(300.0, 100.0), move |ui, theme| {
                key_chip(ui, theme, key, state);
            });
            let rect = harness.get_by_label(key).rect();
            let look = own(&painted(&harness, key), rect);
            let at = format!("{mode:?} {name}: {look:#?}");
            let width = match key {
                "A" | "H" => 17.0,
                "Ctrl Enter" => 76.0,
                "press the keys" => 103.0,
                _ => 37.0,
            };
            assert!((rect.width() - width).abs() <= 1.0 && (rect.height() - 20.0).abs() < 0.01, "{at} {rect:?}");
            let (fill, edge, ink) = match state {
                ChipState::Default => (None, (1.0, p.line, false), p.muted),
                ChipState::OnCyan => (Some(p.cyan), (1.0, p.on_cyan, false), p.on_cyan),
                ChipState::Capture => (None, (1.0, p.cyan_edge, false), p.cyan_text),
                ChipState::Disabled => (None, (1.0, p.line, true), p.muted),
            };
            assert_eq!(look.fill, fill, "{at}");
            assert_eq!(look.edge, Some(edge), "{at}");
            assert_eq!(look.ring, None, "{at}");
            let font = theme.fonts.key;
            assert_eq!(look.texts, vec![(key.to_owned(), ink, font.id().family.to_string(), font.size)], "{at}");
            assert!(!look.overflows, "{at}");
            // Every chip has the sheet's radius 2, the dashed ones too.
            assert_eq!(look.radius, 2.0, "{at}");
        }
    }
}

#[test]
fn a_key_chip_at_130_percent_is_the_scaled_chip() {
    let theme = Theme::new(Mode::Light, TextScale::ALL[4]);
    for (_, state, key) in STATES {
        let harness = staged(
            theme,
            |p| p.surface,
            egui::vec2(300.0, 100.0),
            move |ui, theme| {
                key_chip(ui, theme, key, state);
            },
        );
        assert!((harness.get_by_label(key).rect().height() - theme.boxes().chip).abs() < 0.01, "{key}");
    }
}
