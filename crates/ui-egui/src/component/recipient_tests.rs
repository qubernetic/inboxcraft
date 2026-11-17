// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::accesskit::Role;
use egui::{Rect, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

use super::{RecipientState, recipient_chip};
use crate::component::Shown;
use crate::component::field::tests::{drawn, one};
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
    let mut harness = staged(theme, |p| p.term, vec2(400.0, 100.0), move |ui, theme| {
        if state == Chip::Disabled {
            ui.disable();
        }
        let kind = if state == Chip::Invalid { RecipientState::Invalid } else { RecipientState::Valid };
        recipient_chip(ui, theme, name, kind);
    });
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
                assert!(warning.len() == 1 && (warning[0].rect.center() - (rect.min + vec2(16.0, 12.0))).length() < 1.0 && warning[0].colour == p.minus, "{at}");
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
            let mut harness = staged(theme, |p| p.term, vec2(400.0, 100.0), |ui, theme| {
                recipient_chip(ui, theme, "Teodor Vasquez", RecipientState::Valid);
            });
            bring(&mut harness, "Remove Teodor Vasquez", Shown::Focus);
            harness.key_down(key);
            harness.run();
            let rect = harness.get_by_role_and_label(Role::ListItem, "Teodor Vasquez").rect();
            let held = own(&painted(&harness, "Teodor Vasquez"), rect);
            assert_eq!((held.fill, held.edge), (look.fill, look.edge), "{mode:?} {key:?}");
        }
    }
}
