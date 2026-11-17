// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::{Color32, Rect, vec2};
use egui_kittest::kittest::Queryable as _;

use super::{empty_state, empty_state_with};
use crate::component::button::TextButton;
use crate::component::tests::{Look, Paint, painted, staged};
use crate::keymap::Keymap;
use crate::theme::{Mode, TextScale, Theme};

const LABEL: &str = "Inbox · nothing to read";
const TITLE: &str = "All read.\nNew mail lands here.";
const HELP: &str = "This folder holds no unread messages. “Show done” brings back what you finished.";

const ACTION_LABEL: &str = "No accounts yet";
const ACTION_TITLE: &str = "Add an account to read its mail here.";
const ACTION_HELP: &str = "Nothing leaves this computer before you do.";

/// The texts painted, `(rect, text, colour, family, size)`, in paint order.
fn texts(paint: &Paint) -> Vec<(Rect, String, Color32, String, f32)> {
    paint
        .shapes
        .iter()
        .filter_map(|shape| match &shape.look {
            Look::Text { text, colour, family, size, .. } => Some((shape.rect, text.clone(), *colour, family.clone(), *size)),
            _ => None,
        })
        .collect()
}

#[test]
fn a_bare_empty_state_looks_as_its_sheet_says_its_lines_centred() {
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::default());
        let p = theme.palette;
        let f = theme.fonts;
        // A pane of exactly the block's size: the block fills it.
        let harness = staged(
            theme,
            |p| p.surface,
            vec2(440.0 + 48.0, 216.0 + 48.0),
            |ui, theme| {
                ui.allocate_ui(vec2(440.0, 216.0), |ui| empty_state(ui, theme, LABEL, TITLE, HELP));
            },
        );
        let rect = harness.get_by_label(TITLE).rect();
        let paint = painted(&harness, TITLE);
        let at = format!("{mode:?}: {paint:#?}");
        assert_eq!(rect.size(), vec2(440.0, 216.0), "{at}");
        assert!(paint.rects().next().is_none(), "no fill and no edge: {at}");
        let got = texts(&paint);
        let wanted = [("INBOX · NOTHING TO READ", f.label, 40.0, 16.0), (TITLE, f.title, 68.0, 60.0), (HELP, f.caption, 140.0, 36.0)];
        assert_eq!(got.len(), 3, "{at}");
        for ((text_rect, text, colour, family, size), (words, font, top, high)) in got.iter().zip(wanted) {
            assert_eq!((text.as_str(), *colour, family.as_str(), *size), (words, p.muted, font.id().family.to_string().as_str(), font.size), "{at}");
            assert_eq!(text_rect.top() - rect.top(), top, "{words}: {at}");
            assert_eq!(text_rect.height(), high, "{words}: {at}");
            assert!((text_rect.center().x - rect.center().x).abs() <= 0.5, "{words} centred: {text_rect:?} {at}");
            assert!(text_rect.left() >= rect.left() + 24.0 - 0.01 && text_rect.right() <= rect.right() - 24.0 + 0.01, "{words}: {at}");
        }
    }
}

#[test]
fn an_empty_state_with_an_action_looks_as_its_sheet_says_its_lines_left() {
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::default());
        let p = theme.palette;
        let f = theme.fonts;
        let harness = staged(
            theme,
            |p| p.surface,
            vec2(440.0 + 48.0, 250.0 + 48.0),
            |ui, theme| {
                ui.allocate_ui(vec2(440.0, 250.0), |ui| {
                    let button = TextButton::new("Add account").primary().arrow();
                    empty_state_with(ui, theme, ACTION_LABEL, ACTION_TITLE, ACTION_HELP, button, &Keymap::default());
                });
            },
        );
        let rect = harness.get_by_label(ACTION_TITLE).rect();
        let paint = painted(&harness, ACTION_TITLE);
        let at = format!("{mode:?}: {paint:#?}");
        assert_eq!(rect.size(), vec2(440.0, 250.0), "{at}");
        let got = texts(&paint);
        let wanted = [("NO ACCOUNTS YET", f.label, 40.0, 16.0), (ACTION_TITLE, f.title, 68.0, 60.0), (ACTION_HELP, f.caption, 140.0, 18.0)];
        for ((text_rect, text, colour, family, size), (words, font, top, high)) in got.iter().take(3).zip(wanted) {
            assert_eq!((text.as_str(), *colour, family.as_str(), *size), (words, p.muted, font.id().family.to_string().as_str(), font.size), "{at}");
            assert_eq!(text_rect.min - rect.min, vec2(24.0, top), "{words}: {at}");
            assert_eq!(text_rect.height(), high, "{words}: {at}");
        }
        let button = harness.get_by_label("Add account").rect();
        assert_eq!(button.min - rect.min, vec2(24.0, 178.0), "the action 8 under the help's gap: {at}");
        assert_eq!(button.size(), vec2(126.0, 32.0), "{at}");
    }
}

#[test]
fn a_bare_empty_state_stands_in_the_middle_of_its_pane() {
    let theme = Theme::new(Mode::Light, TextScale::default());
    let harness = staged(
        theme,
        |p| p.surface,
        vec2(1000.0, 700.0),
        |ui, theme| {
            ui.allocate_ui(vec2(900.0, 600.0), |ui| empty_state(ui, theme, LABEL, TITLE, HELP));
        },
    );
    let rect = harness.get_by_label(TITLE).rect();
    let pane = Rect::from_min_size(egui::pos2(24.0, 24.0), vec2(900.0, 600.0));
    assert_eq!(rect.size(), vec2(440.0, 216.0));
    assert!((rect.center() - pane.center()).length() <= 0.5, "{rect:?} in {pane:?}");
}

#[test]
fn the_action_of_an_empty_state_is_pressed_by_click_enter_and_space() {
    for key in [None, Some(egui::Key::Enter), Some(egui::Key::Space)] {
        let theme = Theme::new(Mode::Light, TextScale::default());
        let mut harness = crate::component::tests::builder().with_size(vec2(600.0, 400.0)).build_ui_state(
            move |ui, pressed: &mut usize| {
                let button = TextButton::new("Add account").primary();
                if empty_state_with(ui, &theme, ACTION_LABEL, ACTION_TITLE, ACTION_HELP, button, &Keymap::default()).clicked() {
                    *pressed += 1;
                }
            },
            0,
        );
        harness.run();
        match key {
            None => harness.get_by_label("Add account").click(),
            Some(key) => {
                harness.get_by_label("Add account").focus();
                harness.run();
                harness.key_press(key);
            }
        }
        harness.run();
        assert_eq!(*harness.state(), 1, "{key:?}");
    }
}
