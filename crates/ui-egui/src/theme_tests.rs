// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::Color32;

use super::{Mode, Theme, contrast};

fn both() -> [Theme; 2] {
    [Theme::light(), Theme::dark()]
}

#[test]
fn contrast_of_black_on_white_is_21_and_of_a_colour_on_itself_is_1() {
    assert!((contrast(Color32::BLACK, Color32::WHITE) - 21.0).abs() < 0.01);
    assert!((contrast(Color32::WHITE, Color32::BLACK) - 21.0).abs() < 0.01);
    let grey = Color32::from_rgb(120, 120, 120);
    assert!((contrast(grey, grey) - 1.0).abs() < 0.001);
}

#[test]
fn every_text_colour_meets_wcag_aa_on_the_surface_it_sits_on() {
    for theme in both() {
        let p = theme.palette;
        for (name, text, background) in [
            ("text on background", p.text, p.background),
            ("text on panel", p.text, p.panel),
            ("text on surface", p.text, p.surface),
            ("text on selection", p.text, p.selection),
            ("muted on surface", p.muted, p.surface),
            ("muted on panel", p.muted, p.panel),
            ("muted on selection", p.muted, p.selection),
            ("accent text on accent", p.on_accent, p.accent),
            ("badge text on badge", p.on_badge, p.badge),
            ("error text on error", p.on_error, p.error),
        ] {
            let ratio = contrast(text, background);
            assert!(ratio >= 4.5, "{:?} {name}: {ratio:.2}", theme.mode);
        }
    }
}

#[test]
fn controls_and_markers_meet_three_to_one() {
    for theme in both() {
        let p = theme.palette;
        for (name, colour, background) in [
            ("accent on surface", p.accent, p.surface),
            ("unread marker on surface", p.unread, p.surface),
            ("focus ring on surface", p.focus, p.surface),
            ("focus ring on panel", p.focus, p.panel),
            ("field border on surface", p.field_border, p.surface),
        ] {
            let ratio = contrast(colour, background);
            assert!(ratio >= 3.0, "{:?} {name}: {ratio:.2}", theme.mode);
        }
    }
}

#[test]
fn account_colours_are_distinct_and_visible_in_both_modes() {
    for theme in both() {
        let colours = theme.palette.accounts;
        for (index, colour) in colours.iter().enumerate() {
            assert!(contrast(*colour, theme.palette.surface) >= 3.0, "{:?} account colour {index}", theme.mode);
            assert!(colours.iter().skip(index + 1).all(|other| other != colour), "{:?} account colour {index} repeats", theme.mode);
        }
        assert_eq!(theme.account_colour(0), colours[0]);
        assert_eq!(theme.account_colour(colours.len() as u64), colours[0], "wraps around");
        assert_eq!(theme.account_colour(u64::MAX), colours[(u64::MAX % colours.len() as u64) as usize]);
    }
}

#[test]
fn the_type_scale_grows_and_the_spacing_is_positive() {
    for theme in both() {
        let t = theme.type_scale;
        assert!(0.0 < t.small && t.small < t.body && t.body < t.heading && t.heading < t.title, "{t:?}");
        let s = theme.spacing;
        assert!(0.0 < s.xs && s.xs < s.s && s.s < s.m && s.m < s.l && s.l < s.xl, "{s:?}");
        assert!(s.row_height > t.body * 2.0, "a row holds two lines of text");
        assert!(s.folder_pane > 0.0 && s.corner > 0.0);
    }
}

#[test]
fn for_mode_picks_the_matching_theme_and_the_visuals_follow() {
    assert_eq!(Theme::for_mode(Mode::Light), Theme::light());
    assert_eq!(Theme::for_mode(Mode::Dark), Theme::dark());
    assert_ne!(Theme::light().palette, Theme::dark().palette);
    for theme in both() {
        let visuals = theme.visuals();
        assert_eq!(visuals.dark_mode, theme.mode == Mode::Dark);
        assert_eq!(visuals.panel_fill, theme.palette.panel);
        assert_eq!(visuals.extreme_bg_color, theme.palette.field);
        assert_eq!(visuals.selection.bg_fill, theme.palette.selection);
        assert_eq!(visuals.hyperlink_color, theme.palette.accent);
        assert_eq!(visuals.override_text_color, Some(theme.palette.text));
        let style = theme.style();
        assert_eq!(style.spacing.item_spacing.y, theme.spacing.xs);
        let body = style.text_styles.get(&egui::TextStyle::Body).map(|font| font.size);
        assert_eq!(body, Some(theme.type_scale.body));
        let heading = style.text_styles.get(&egui::TextStyle::Heading).map(|font| font.size);
        assert_eq!(heading, Some(theme.type_scale.heading));
    }
}

#[test]
fn mode_of_reads_the_context_and_apply_installs_both_styles() {
    let ctx = egui::Context::default();
    Theme::apply(&ctx);
    assert_eq!(ctx.style_of(egui::Theme::Light).visuals.panel_fill, Theme::light().palette.panel);
    assert_eq!(ctx.style_of(egui::Theme::Dark).visuals.panel_fill, Theme::dark().palette.panel);
    ctx.set_theme(egui::Theme::Dark);
    assert_eq!(Mode::of(&ctx), Mode::Dark);
    assert_eq!(Theme::current(&ctx), Theme::dark());
    ctx.set_theme(egui::Theme::Light);
    assert_eq!(Mode::of(&ctx), Mode::Light);
    assert_eq!(Theme::current(&ctx), Theme::light());
}
