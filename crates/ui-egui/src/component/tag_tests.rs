// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::accesskit::Role;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::{TagKind, tag};
use crate::component::soon::soon_tag;
use crate::component::tests::{own, painted, staged};
use crate::theme::{Mode, TextScale, Theme};

/// The states of the tag's sheet the app draws, in its order: the label each shows there, its kind (`None`: coming
/// soon) and its width.
const STATES: [(&str, Option<TagKind>, f32); 6] = [
    ("Draft", Some(TagKind::Default), 45.0),
    ("New sender", Some(TagKind::Accent), 78.0),
    ("Returned", Some(TagKind::Warn), 65.0),
    ("Not delivered", Some(TagKind::Minus), 98.0),
    ("Let in", Some(TagKind::Plus), 52.0),
    ("coming soon", None, 85.0),
];

fn tag_in(mode: Mode, scale: TextScale, label: &'static str, kind: Option<TagKind>) -> (egui::Rect, crate::component::tests::Own, Role) {
    let theme = Theme::new(mode, scale);
    let harness = staged(
        theme,
        |p| p.surface,
        egui::vec2(300.0, 100.0),
        move |ui, theme| match kind {
            Some(kind) => {
                tag(ui, theme, label, kind);
            }
            None => {
                soon_tag(ui, theme);
            }
        },
    );
    let node = harness.get_by_label(label);
    let rect = node.rect();
    (rect, own(&painted(&harness, label), rect), node.accesskit_node().role())
}

#[test]
fn a_tag_looks_as_its_sheet_says_in_every_state() {
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::default());
        let p = theme.palette;
        let font = theme.fonts.mono_s;
        for (label, kind, width) in STATES {
            let (rect, look, role) = tag_in(mode, TextScale::default(), label, kind);
            let at = format!("{mode:?} {label}: {look:#?}");
            assert!((rect.width() - width).abs() <= 1.0 && (rect.height() - 20.0).abs() < 0.01, "{at} {rect:?}");
            assert_eq!(role, Role::Label, "never a button: {at}");
            let (edge, ink, dashed) = match kind {
                Some(TagKind::Default) => (p.line, p.muted, false),
                Some(TagKind::Accent) => (p.cyan_edge, p.cyan_text, false),
                Some(TagKind::Warn) => (p.warn, p.warn, false),
                Some(TagKind::Minus) => (p.minus, p.minus, false),
                Some(TagKind::Plus) => (p.plus, p.plus, false),
                None => (p.muted, p.muted, true),
            };
            assert_eq!(look.fill, None, "the fill stays empty: {at}");
            assert_eq!(look.edge, Some((1.0, edge, dashed)), "{at}");
            assert_eq!(look.ring, None, "{at}");
            assert_eq!(look.texts, vec![(label.to_owned(), ink, font.id().family.to_string(), font.size)], "{at}");
            assert!(!look.overflows, "{at}");
            assert!(look.radius <= 0.0, "{at}");
        }
    }
}

#[test]
fn a_tag_at_130_percent_is_the_scaled_chip_and_a_long_one_ends_at_240() {
    let theme = Theme::new(Mode::Light, TextScale::ALL[4]);
    for (label, kind, _) in STATES {
        let (rect, ..) = tag_in(Mode::Light, TextScale::ALL[4], label, kind);
        assert!((rect.height() - theme.boxes().chip).abs() < 0.01, "{label} {rect:?}");
    }
    let long = "A tag whose words run on much further than any tag ever should";
    let (rect, look, _) = tag_in(Mode::Light, TextScale::default(), long, Some(TagKind::Default));
    assert!(rect.width() <= 240.0 + 12.0 + 0.01, "{rect:?}");
    assert!(!look.overflows, "{look:#?}");
}
