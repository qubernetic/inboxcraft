// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::accesskit::Role;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::{BadgeKind, badge, count, group, group_ink};
use crate::component::tests::{Own, builder, drew_icon, own, painted, staged, theme};
use crate::iconset::IconId;
use crate::theme::{Mode, TextScale, Theme, contrast};

#[test]
fn a_count_badge_ends_at_its_right_edge_and_grows_with_its_number() {
    let theme = theme();
    let mut harness = builder().build_ui_state(
        move |ui, lefts: &mut Vec<f32>| {
            *lefts = vec![count(ui, &theme, 3, 200.0, 20.0), count(ui, &theme, 12_345, 200.0, 60.0)];
        },
        Vec::new(),
    );
    harness.run();
    let lefts = harness.state();
    assert!(lefts[0] < 200.0 && lefts[1] < lefts[0], "{lefts:?}");
}

#[test]
fn a_group_badge_is_a_named_picture_with_its_icon() {
    let theme = theme();
    let mut harness = builder().build_ui(move |ui| {
        group(ui, &theme, IconId::People, "People", 32.0);
        group(ui, &theme, IconId::Notify, "Updates", f32::NAN);
    });
    harness.run();
    let people = harness.get_by_label("People");
    assert_eq!(people.accesskit_node().role(), Role::Image);
    assert!((people.rect().width() - 32.0).abs() < 0.5);
    assert!(drew_icon(harness.output(), IconId::People));
}

#[test]
fn the_group_badge_s_icon_is_the_muted_colour_and_reads_on_the_card() {
    {
        for mode in [Mode::Light, Mode::Dark] {
            let theme = Theme::new(mode, TextScale::default());
            let ink = group_ink(&theme);
            assert_eq!(ink, theme.palette.muted, "{mode:?}");
            assert!(contrast(ink, theme.palette.surface) >= 3.0, "{mode:?}");
        }
    }
}

/// The states of the badge's sheet, in its order: the count, its kind, the text it shows and its width.
const STATES: [(u64, BadgeKind, &str, f32); 4] = [
    (7, BadgeKind::Count, "7", 16.0),
    (12_345, BadgeKind::Count, "9\u{202f}999+", 48.0),
    (12, BadgeKind::Muted, "12", 22.0),
    (3, BadgeKind::Accent, "3", 16.0),
];

/// The badge for `n` of `kind` on a `surface` stage: its rect and its own look.
fn badge_in(mode: Mode, scale: TextScale, n: u64, kind: BadgeKind, text: &str) -> (egui::Rect, Own, Role) {
    let theme = Theme::new(mode, scale);
    let harness = staged(
        theme,
        |p| p.surface,
        egui::vec2(200.0, 80.0),
        move |ui, theme| {
            badge(ui, theme, n, kind);
        },
    );
    let node = harness.get_by_label(text);
    let rect = node.rect();
    (rect, own(&painted(&harness, text), rect), node.accesskit_node().role())
}

#[test]
fn a_badge_looks_as_its_sheet_says_in_every_state() {
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::default());
        let p = theme.palette;
        let font = theme.fonts.mono_s;
        for (n, kind, text, width) in STATES {
            let (rect, look, role) = badge_in(mode, TextScale::default(), n, kind, text);
            let at = format!("{mode:?} {text}: {look:#?}");
            assert!((rect.width() - width).abs() <= 1.0 && (rect.height() - 16.0).abs() < 0.01, "{at} {rect:?}");
            assert_eq!(role, Role::Label, "a count is read, never pressed: {at}");
            let (fill, ink) = match kind {
                BadgeKind::Count => (Some(p.cyan_muted), p.cyan_text),
                BadgeKind::Muted => (None, p.muted),
                BadgeKind::Accent => (Some(p.cyan), p.on_cyan),
            };
            assert_eq!(look.fill, fill, "{at}");
            assert_eq!(look.edge, None, "{at}");
            assert_eq!(look.ring, None, "{at}");
            assert_eq!(look.texts, vec![(text.to_owned(), ink, font.id().family.to_string(), font.size)], "{at}");
            assert!(look.radius <= 0.0, "square: {at}");
            assert!(!look.overflows, "{at}");
        }
    }
}

#[test]
fn a_badge_at_130_percent_is_the_scaled_height_and_a_count_of_0_is_not_shown() {
    let theme = Theme::new(Mode::Light, TextScale::ALL[4]);
    for (n, kind, text, _) in STATES {
        let (rect, ..) = badge_in(Mode::Light, TextScale::ALL[4], n, kind, text);
        assert!((rect.height() - theme.size(16.0)).abs() < 0.01, "{text} {rect:?}");
        assert!(rect.width() >= rect.height() - 0.01, "at least as wide as high: {text} {rect:?}");
    }
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let mut harness = builder().build_ui(move |ui| {
        let response = badge(ui, &theme, 0, BadgeKind::Count);
        assert_eq!(response.rect.width(), 0.0, "nothing for 0");
    });
    harness.run();
}

#[test]
fn the_count_beside_a_row_is_the_sheets_count_badge() {
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::default());
        let mut harness = builder().build_ui_state(
            move |ui, lefts: &mut Vec<f32>| {
                *lefts = vec![count(ui, &theme, 7, 200.0, 20.0), count(ui, &theme, 12_345, 200.0, 60.0), count(ui, &theme, 0, 200.0, 90.0)];
            },
            Vec::new(),
        );
        harness.run();
        let lefts = harness.state();
        assert!((lefts[0] - 184.0).abs() <= 1.0, "{mode:?} 16 wide: {lefts:?}");
        assert!((lefts[1] - 152.0).abs() <= 1.0, "{mode:?} 48 wide: {lefts:?}");
        assert_eq!(lefts[2], 200.0, "{mode:?} a count of 0 is not shown: {lefts:?}");
    }
}
