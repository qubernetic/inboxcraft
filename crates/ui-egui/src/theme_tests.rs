// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::{Path, PathBuf};

use egui::Color32;

use super::{
    AVATARS, BLACK, DARK, Face, Font, GHOST, Look, Mode, ModePreference, PRESETS, Palette, Preset, TextScale, Theme, Typeface, WHITE, Weight,
    contrast, font_definitions, icon_ink_over, ink_on, ink_over, install_fonts, mix, over, preference, readable, rgb,
};
use crate::iconset::IconSet;
use crate::motion::MOTION;

/// Every preset in both modes.
fn all() -> Vec<Theme> {
    PRESETS.iter().flat_map(|preset| [Theme::new(Mode::Light, preset), Theme::new(Mode::Dark, preset)]).collect()
}

fn hex(colour: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", colour.r(), colour.g(), colour.b())
}

#[test]
fn contrast_of_black_on_white_is_21_and_of_a_colour_on_itself_is_1() {
    assert!((contrast(Color32::BLACK, Color32::WHITE) - 21.0).abs() < 0.01);
    assert!((contrast(Color32::WHITE, Color32::BLACK) - 21.0).abs() < 0.01);
    let grey = Color32::from_rgb(120, 120, 120);
    assert!((contrast(grey, grey) - 1.0).abs() < 0.001);
}

#[test]
fn the_three_presets_are_one_table_entry_each_with_the_owners_colours() {
    let ids: Vec<&str> = PRESETS.iter().map(|preset| preset.id).collect();
    assert_eq!(ids, ["electric", "aurora", "citrus"]);
    let electric = Preset::named("electric").unwrap();
    assert_eq!(electric.gradient.map(hex), ["#2f6bff", "#8a3ffc", "#ff2d95"]);
    assert_eq!(hex(electric.accent), "#ffd60a");
    let aurora = Preset::named("aurora").unwrap();
    assert_eq!(aurora.gradient.map(hex), ["#9b5cff", "#ff2bd6", "#ff8a00"]);
    assert_eq!(hex(aurora.accent), "#00e5ff");
    let citrus = Preset::named("citrus").unwrap();
    assert_eq!(citrus.gradient.map(hex), ["#ff2e63", "#ff8f00", "#ffd000"]);
    assert_eq!(hex(citrus.accent), "#3d5afe");
    assert_eq!(Preset::default(), *electric, "Electric is the default");
    assert_eq!(Preset::named("teal"), None);
    assert_eq!(Preset::named("Electric"), None, "ids are lower case");
}

#[test]
fn the_mode_preference_is_system_by_default_and_maps_onto_egui() {
    assert_eq!(ModePreference::default(), ModePreference::System);
    assert_eq!(preference(ModePreference::System), egui::ThemePreference::System);
    assert_eq!(preference(ModePreference::Light), egui::ThemePreference::Light);
    assert_eq!(preference(ModePreference::Dark), egui::ThemePreference::Dark);
    for (id, preference) in [("system", ModePreference::System), ("light", ModePreference::Light), ("dark", ModePreference::Dark)] {
        assert_eq!(ModePreference::named(id), Some(preference));
        assert_eq!(preference.id(), id);
    }
    assert_eq!(ModePreference::named("sepia"), None);
}

#[test]
fn every_text_colour_meets_wcag_aa_on_the_surface_it_sits_on_for_every_preset_in_both_modes() {
    for theme in all() {
        let p = theme.palette;
        for (name, text, background) in [
            ("text on background", p.text, p.background),
            ("text on panel", p.text, p.panel),
            ("text on surface", p.text, p.surface),
            ("text on field", p.text, p.field),
            ("text on selection", p.text, p.selection),
            ("text on the selection's end", p.text, p.selection_end),
            ("muted on background", p.muted, p.background),
            ("muted on surface", p.muted, p.surface),
            ("muted on panel", p.muted, p.panel),
            ("muted on selection", p.muted, p.selection),
            ("accent text on background", p.accent, p.background),
            ("accent text on surface", p.accent, p.surface),
            ("accent text on panel", p.accent, p.panel),
            ("accent text on selection", p.accent, p.selection),
            ("text on accent", p.on_accent, p.accent),
            ("badge text on badge", p.on_badge, p.badge),
            ("error text on error", p.on_error, p.error),
            ("error text on surface", p.error, p.surface),
            ("error text on panel", p.error, p.panel),
            ("flag text on surface", p.flag, p.surface),
        ] {
            let ratio = contrast(text, background);
            assert!(ratio >= 4.5, "{} {:?} {name}: {ratio:.2}", theme.preset.id, theme.mode);
        }
    }
}

#[test]
fn controls_and_markers_meet_three_to_one_for_every_preset_in_both_modes() {
    for theme in all() {
        let p = theme.palette;
        for (name, colour, background) in [
            ("unread marker on surface", p.unread, p.surface),
            ("unread marker on selection", p.unread, p.selection),
            ("flag on surface", p.flag, p.surface),
            ("flag on selection", p.flag, p.selection),
            ("focus ring on surface", p.focus, p.surface),
            ("focus ring on panel", p.focus, p.panel),
            ("focus ring on background", p.focus, p.background),
            ("field border on surface", p.field_border, p.surface),
            ("field border on field", p.field_border, p.field),
        ] {
            let ratio = contrast(colour, background);
            assert!(ratio >= 3.0, "{} {:?} {name}: {ratio:.2}", theme.preset.id, theme.mode);
        }
    }
}

#[test]
fn text_on_a_gradient_picks_white_or_dark_ink_by_contrast() {
    assert_eq!(ink_on(Color32::from_rgb(0x20, 0x20, 0x60)), Color32::WHITE);
    assert_ne!(ink_on(Color32::from_rgb(0xff, 0xd0, 0x00)), Color32::WHITE, "dark ink on yellow");
    for theme in all() {
        for step in 0..=32 {
            let at = step as f32 / 32.0;
            let behind = theme.gradient_at(at);
            let ink = ink_on(behind);
            let other = if ink == Color32::WHITE { theme.palette.ink } else { Color32::WHITE };
            assert!(contrast(ink, behind) >= contrast(other, behind), "{} at {at}: the better ink", theme.preset.id);
            assert!(contrast(ink, behind) >= 4.5, "{} {:?} ink on the gradient at {at}: {:.2}", theme.preset.id, theme.mode, contrast(ink, behind));
        }
    }
    // Citrus's yellow end: white would not do.
    let citrus = Theme::new(Mode::Light, Preset::named("citrus").unwrap());
    assert!(contrast(Color32::WHITE, citrus.gradient_at(1.0)) < 4.5);
    assert_ne!(ink_on(citrus.gradient_at(1.0)), Color32::WHITE);
}

#[test]
fn the_gradient_runs_through_the_three_stops() {
    for theme in all() {
        let [first, middle, last] = theme.preset.gradient;
        assert_eq!(theme.gradient_at(0.0), first);
        assert_eq!(theme.gradient_at(0.5), middle);
        assert_eq!(theme.gradient_at(1.0), last);
        assert_eq!(theme.gradient_at(-3.0), first, "clamped");
        assert_eq!(theme.gradient_at(f32::NAN), first, "nonsense starts at the start");
        assert_eq!(theme.gradient_at(7.0), last, "clamped");
        assert_eq!(theme.palette.gradient, theme.preset.gradient, "the gradient token is the preset at full strength");
    }
}

#[test]
fn light_mode_accent_text_is_the_middle_stop_darkened_and_dark_mode_the_middle_stop_lightened() {
    for preset in &PRESETS {
        let light = Theme::new(Mode::Light, preset).palette;
        let middle = preset.gradient[1];
        assert!(light.accent != middle && light.accent != preset.accent, "{}: darkened, not the raw colour", preset.id);
        let channels = |c: Color32| u16::from(c.r()) + u16::from(c.g()) + u16::from(c.b());
        assert!(channels(light.accent) < channels(middle), "{}: darker than the middle stop", preset.id);
        let dark = Theme::new(Mode::Dark, preset).palette;
        // Dark mode takes the middle stop mixed 30 % with white (lightened further where it would not read), never
        // the preset's separate accent, which stays an account colour.
        assert_ne!(dark.accent, preset.accent, "{}: not the separate accent", preset.id);
        assert!(channels(dark.accent) >= channels(mix(middle, WHITE, 0.3)), "{}: at least the lightened stop", preset.id);
        assert!(channels(dark.accent) > channels(middle), "{}: lighter than the middle stop", preset.id);
    }
}

#[test]
fn readable_moves_a_colour_only_as_far_as_it_must() {
    let white = Color32::WHITE;
    let already = Color32::from_rgb(0x20, 0x20, 0x20);
    assert_eq!(readable(already, &[white], 4.5, Mode::Light), already);
    let yellow = Color32::from_rgb(0xff, 0xd0, 0x00);
    let darkened = readable(yellow, &[white], 4.5, Mode::Light);
    assert!(contrast(darkened, white) >= 4.5);
    assert!(contrast(darkened, white) < 6.0, "not darker than needed: {:.2}", contrast(darkened, white));
    let lightened = readable(Color32::from_rgb(0x3d, 0x5a, 0xfe), &[Color32::BLACK], 7.0, Mode::Dark);
    assert!(contrast(lightened, Color32::BLACK) >= 7.0);
    assert_eq!(readable(yellow, &[], 4.5, Mode::Light), yellow, "nothing to sit on");
}

#[test]
fn mix_runs_from_one_colour_to_the_other() {
    let (a, b) = (Color32::from_rgb(0, 100, 200), Color32::from_rgb(200, 0, 100));
    assert_eq!(mix(a, b, 0.0), a);
    assert_eq!(mix(a, b, 1.0), b);
    assert_eq!(mix(a, b, 0.5), Color32::from_rgb(100, 50, 150));
    assert_eq!(mix(a, b, -1.0), a);
    assert_eq!(mix(a, b, f32::NAN), a);
}

/// Teal: blue-green with more green than blue (the old `#1d6a64` and `#6cc4ba`); the presets' cyans have more blue.
/// Coral: a soft orange-pink (the old `#f0a06a`).
fn teal_or_coral(colour: Color32) -> bool {
    let (r, g, b) = (i32::from(colour.r()), i32::from(colour.g()), i32::from(colour.b()));
    let teal = g > r + 40 && b > r + 20 && g >= b;
    let coral = r > 200 && r - g > 50 && (100..190).contains(&g) && (70..160).contains(&b);
    teal || coral
}

#[test]
fn no_teal_or_coral_is_left() {
    for theme in all() {
        let p = theme.palette;
        let mut tokens = vec![p.accent, p.unread, p.flag, p.focus, p.badge, p.selection, p.selection_end];
        tokens.extend(p.accounts);
        tokens.extend(p.avatars);
        for colour in tokens {
            assert!(!teal_or_coral(colour), "{} {:?}: {} is teal or coral", theme.preset.id, theme.mode, hex(colour));
        }
    }
}

#[test]
fn glass_float_and_colour_fields_are_translucent_and_the_float_about_four_fifths() {
    for theme in all() {
        let p = theme.palette;
        for (name, colour) in [
            ("glass", p.glass),
            ("glass edge", p.glass_edge),
            ("glass highlight", p.glass_highlight),
            ("shadow", p.shadow),
            ("line", p.line),
            ("hover", p.hover),
        ] {
            assert!(colour.a() < 255 && colour.a() > 0, "{} {:?} {name}: alpha {}", theme.preset.id, theme.mode, colour.a());
        }
        assert!((195..=215).contains(&p.float.a()), "{} {:?} float alpha {}", theme.preset.id, theme.mode, p.float.a());
        for (index, field) in p.colour_fields.iter().enumerate() {
            assert!(field.a() > 0 && field.a() < 160, "{} {:?} colour field {index}", theme.preset.id, theme.mode);
        }
    }
}

#[test]
fn account_colours_are_distinct_and_visible_for_every_preset_in_both_modes() {
    for theme in all() {
        let colours = theme.palette.accounts;
        for (index, colour) in colours.iter().enumerate() {
            assert!(contrast(*colour, theme.palette.surface) >= 3.0, "{} {:?} account colour {index}", theme.preset.id, theme.mode);
            assert!(
                colours.iter().skip(index + 1).all(|other| other != colour),
                "{} {:?} account colour {index} repeats",
                theme.preset.id,
                theme.mode
            );
        }
        assert_eq!(theme.account_colour(0), colours[0]);
        assert_eq!(theme.account_colour(colours.len() as u64), colours[0], "wraps around");
        assert_eq!(theme.account_colour(u64::MAX), colours[(u64::MAX % colours.len() as u64) as usize]);
    }
}

#[test]
fn every_preset_gives_its_own_colours_and_the_modes_differ() {
    for mode in [Mode::Light, Mode::Dark] {
        let palettes: Vec<_> = PRESETS.iter().map(|preset| Theme::new(mode, preset).palette).collect();
        for (index, palette) in palettes.iter().enumerate() {
            for other in palettes.iter().skip(index + 1) {
                assert_ne!(palette.accent, other.accent, "{mode:?}");
                assert_ne!(palette.selection, other.selection, "{mode:?}");
                assert_ne!(palette.background, other.background, "{mode:?}: the surfaces carry a trace of the preset");
            }
        }
    }
    for preset in &PRESETS {
        assert_ne!(Theme::new(Mode::Light, preset).palette, Theme::new(Mode::Dark, preset).palette);
    }
}

#[test]
fn the_type_grows_and_the_spacing_is_positive() {
    for theme in all() {
        let f = theme.fonts;
        assert!(0.0 < f.caption.size && f.caption.size < f.body.size && f.body.size < f.heading.size && f.heading.size < f.title.size, "{f:?}");
        let s = theme.spacing;
        assert!(0.0 < s.xs && s.xs < s.s && s.s < s.m && s.m < s.l && s.l < s.xl, "{s:?}");
        assert!(s.folder_pane > 0.0 && s.corner > 0.0);
    }
}

#[test]
fn sender_and_subject_are_strong_the_preview_plain_and_unread_heavier() {
    let f = Theme::new(Mode::Light, &Preset::default()).fonts;
    assert_eq!(f.preview.face, Face::Sans(Weight::Regular));
    assert_eq!(f.body.face, Face::Sans(Weight::Regular));
    for (read, unread) in [(f.sender, f.sender_unread), (f.subject, f.subject_unread)] {
        let (Face::Sans(read), Face::Sans(unread)) = (read.face, unread.face) else { panic!("sans") };
        assert!(read < unread, "unread is heavier: {read:?} {unread:?}");
    }
    let Face::Sans(sender) = f.sender.face else { panic!("sans") };
    assert!(sender > Weight::Regular, "the sender is stronger than the preview");
    assert!(f.title.face == Face::Sans(Weight::Bold));
    assert_eq!(f.mono.face, Face::Mono);
}

#[test]
fn the_text_scale_runs_from_90_to_130_percent_in_steps_of_10() {
    let percents: Vec<u16> = TextScale::ALL.iter().map(|scale| scale.percent()).collect();
    assert_eq!(percents, [90, 100, 110, 120, 130]);
    assert_eq!(TextScale::default().percent(), 100);
    assert_eq!(TextScale::from_percent(120).map(TextScale::percent), Some(120));
    for wrong in [0, 80, 95, 105, 140, u16::MAX] {
        assert_eq!(TextScale::from_percent(wrong), None, "{wrong}");
    }
    assert_eq!(TextScale::from_percent(130).map(TextScale::factor), Some(1.3));
}

#[test]
fn the_text_scale_applies_to_every_font_token_and_to_egui_s_text_styles() {
    let base = Theme::new(Mode::Dark, &Preset::default());
    assert_eq!(base.text_scale, TextScale::default());
    for scale in TextScale::ALL {
        let theme = base.with_text_scale(scale);
        assert_eq!(theme.text_scale, scale);
        assert_eq!(theme.palette, base.palette);
        assert_eq!(theme.spacing, base.spacing);
        let (scaled, plain) = (theme.fonts.all(), base.fonts.all());
        assert_eq!(scaled.len(), plain.len());
        assert!(scaled.len() >= 12, "every token is listed");
        for (scaled, plain) in scaled.iter().zip(&plain) {
            assert_eq!(scaled.face, plain.face);
            assert!((scaled.size - plain.size * scale.factor()).abs() < 1e-4, "{scale:?}: {scaled:?} from {plain:?}");
        }
        let body = theme.style().text_styles.get(&egui::TextStyle::Body).map(|font| font.size);
        assert_eq!(body, Some(theme.fonts.body.size));
    }
    // Scaling twice starts from the 100 % sizes again.
    let twice = base.with_text_scale(TextScale::ALL[4]).with_text_scale(TextScale::ALL[0]);
    assert_eq!(twice, base.with_text_scale(TextScale::ALL[0]));
}

#[test]
fn every_font_token_names_a_family_its_typeface_defines() {
    let definitions = font_definitions();
    for (typeface, prefix) in [(Typeface::Figtree, "Figtree-"), (Typeface::Inter, "Inter-")] {
        let theme = Theme::new(Mode::Light, &Preset::default()).with_typeface(typeface);
        assert_eq!(theme.typeface, typeface);
        for font in theme.fonts.all() {
            let id = font.id();
            assert_eq!(id.size, font.size);
            let family = definitions.families.get(&id.family).unwrap_or_else(|| panic!("{:?} is defined", id.family));
            let first = family.first().unwrap();
            assert!(definitions.font_data.contains_key(first), "{first}");
            match font.face {
                Face::Mono => assert_eq!(first, "Hack"),
                Face::Sans(_) => assert!(first.starts_with(prefix), "{first}"),
            }
        }
    }
}

#[test]
fn the_typeface_and_the_text_scale_change_independently() {
    let base = Theme::new(Mode::Dark, &Preset::default());
    let large = TextScale::from_percent(130).unwrap();
    let both = base.with_text_scale(large).with_typeface(Typeface::Inter);
    assert_eq!(both, base.with_typeface(Typeface::Inter).with_text_scale(large));
    assert_eq!(both.fonts.body.size, base.with_text_scale(large).fonts.body.size);
    assert!(both.fonts.all().iter().all(|font| font.typeface == Typeface::Inter));
    assert_eq!(both.with_typeface(Typeface::Figtree), base.with_text_scale(large));
}

#[test]
fn inter_draws_with_its_own_shapes() {
    let ctx = egui::Context::default();
    install_fonts(&ctx);
    ctx.run_ui(egui::RawInput::default(), |_| {}).textures_delta.clear();
    let width = |typeface: Typeface| {
        let font = Font { size: 14.0, face: Face::Sans(Weight::Regular), typeface };
        ctx.fonts_mut(|fonts| "Budget for 2026".chars().map(|c| fonts.glyph_width(&font.id(), c)).sum::<f32>())
    };
    assert!((width(Typeface::Inter) - width(Typeface::Figtree)).abs() > 0.5, "Inter is a face of its own");
}

#[test]
fn a_look_takes_the_owner_s_choices() {
    use inboxcraft_engine::appearance::{Accent, Appearance, CustomAccent, Icons, Rgb};
    let custom = CustomAccent { gradient: [Rgb(1, 2, 3), Rgb(4, 5, 6), Rgb(7, 8, 9)], accent: Rgb(0xff, 0xd6, 0x0a) };
    let appearance =
        Appearance { accent: Accent::Custom(custom), font: Typeface::Inter, icons: Icons::Lucide, text_scale: 110, ..Appearance::default() };
    let look = Look::of(&appearance);
    assert_eq!(look.preset.id, "custom");
    assert_eq!(look.preset.gradient, [rgb(0x010203), rgb(0x040506), rgb(0x070809)]);
    assert_eq!(look.preset.accent, rgb(0xffd60a));
    assert_eq!(look.typeface, Typeface::Inter);
    assert_eq!(look.icons, IconSet::Lucide);
    assert_eq!(look.text_scale.percent(), 110);
    let theme = Theme::of(Mode::Dark, &look);
    assert_eq!((theme.typeface, theme.icons, theme.text_scale), (Typeface::Inter, IconSet::Lucide, look.text_scale));
    assert_eq!(Look::of(&Appearance::default()), Look::default());
    assert_eq!(Preset::of(&Accent::Preset("aurora")), *Preset::named("aurora").unwrap());
    assert_eq!(Preset::of(&Accent::Preset("gone")), Preset::default(), "an id this front end does not ship is Electric");
    assert_eq!(Look::of(&Appearance { text_scale: 95, ..Appearance::default() }).text_scale, TextScale::default());
}

#[test]
fn the_engine_s_dark_surface_is_the_lightest_dark_neutral_and_every_preset_id_is_shipped() {
    use inboxcraft_engine::appearance::{DARK_SURFACE, PRESETS as PRESET_IDS, Rgb};
    let Rgb(r, g, b) = DARK_SURFACE;
    assert_eq!(Color32::from_rgb(r, g, b), DARK.float);
    for neutral in [DARK.background, DARK.panel, DARK.surface, DARK.field] {
        assert!(contrast(neutral, BLACK) <= contrast(DARK.float, BLACK), "{}", hex(neutral));
    }
    assert_eq!(PRESET_IDS.to_vec(), PRESETS.iter().map(|preset| preset.id).collect::<Vec<_>>());
}

#[test]
fn a_custom_accent_the_engine_nudged_reads_in_dark_mode_for_any_gradient() {
    use inboxcraft_engine::appearance::{Accent, CustomAccent, Rgb};
    for gradient in [[Rgb(0, 0, 0); 3], [Rgb(255, 255, 255); 3], [Rgb(0xff, 0xd0, 0), Rgb(0, 0xe5, 0xff), Rgb(0x3d, 0x5a, 0xfe)]] {
        let custom = CustomAccent { gradient, accent: Rgb(0x30, 0x10, 0x60) }.readable();
        let preset = Preset::of(&Accent::Custom(custom));
        for mode in [Mode::Light, Mode::Dark] {
            let p = Theme::new(mode, &preset).palette;
            for behind in [p.background, p.panel, p.surface] {
                assert!(contrast(p.accent, behind) >= 4.5, "{mode:?} {}", hex(p.accent));
                assert!(contrast(p.text, behind) >= 4.5, "{mode:?}");
            }
        }
    }
}

#[test]
fn figtree_replaces_ubuntu_light_everywhere() {
    let definitions = font_definitions();
    assert!(!definitions.font_data.contains_key("Ubuntu-Light"));
    for (family, fonts) in &definitions.families {
        assert!(!fonts.iter().any(|font| font.contains("Ubuntu")), "{family:?}: {fonts:?}");
        assert!(fonts.iter().all(|font| definitions.font_data.contains_key(font)), "{family:?}: {fonts:?}");
    }
    let proportional = definitions.families.get(&egui::FontFamily::Proportional).unwrap();
    assert_eq!(proportional.first().map(String::as_str), Some("Figtree-Regular"));
    let monospace = definitions.families.get(&egui::FontFamily::Monospace).unwrap();
    assert!(monospace.contains(&"Figtree-Regular".to_owned()), "Figtree stands in for Hack's missing signs: {monospace:?}");
    for weight in [Weight::Regular, Weight::Medium, Weight::SemiBold, Weight::Bold] {
        let family = Font { size: 14.0, face: Face::Sans(weight), typeface: Typeface::Inter }.id().family;
        let fonts = definitions.families.get(&family).unwrap();
        assert!(fonts.len() > 1, "{weight:?} falls back to the emoji and icon fonts");
    }
}

#[test]
fn install_fonts_puts_figtree_into_the_context() {
    let ctx = egui::Context::default();
    install_fonts(&ctx);
    ctx.run_ui(egui::RawInput::default(), |_| {}).textures_delta.clear();
    let families = ctx.fonts(|fonts| fonts.definitions().families.clone());
    assert_eq!(families.get(&egui::FontFamily::Proportional).and_then(|fonts| fonts.first()).map(String::as_str), Some("Figtree-Regular"));
    let bold = Font { size: 14.0, face: Face::Sans(Weight::Bold), typeface: Typeface::Figtree };
    let regular = Font { size: 14.0, face: Face::Sans(Weight::Regular), typeface: Typeface::Figtree };
    let width = |font: Font| ctx.fonts_mut(|fonts| fonts.glyph_width(&font.id(), 'm'));
    assert!(width(bold) > width(regular), "the bold weight is its own face");
}

#[test]
fn the_visuals_follow_the_tokens() {
    for theme in all() {
        let visuals = theme.visuals();
        assert_eq!(visuals.dark_mode, theme.mode == Mode::Dark);
        assert_eq!(visuals.panel_fill, theme.palette.panel);
        assert_eq!(visuals.extreme_bg_color, theme.palette.field);
        assert_eq!(visuals.selection.bg_fill, theme.palette.selection);
        assert_eq!(visuals.hyperlink_color, theme.palette.accent);
        assert_eq!(visuals.override_text_color, Some(theme.palette.text));
        assert_eq!(visuals.widgets.hovered.bg_stroke.color, theme.palette.focus);
        let style = theme.style();
        assert_eq!(style.spacing.item_spacing.y, theme.spacing.xs);
        for (style_of, token) in [
            (egui::TextStyle::Small, theme.fonts.caption),
            (egui::TextStyle::Body, theme.fonts.body),
            (egui::TextStyle::Button, theme.fonts.button),
            (egui::TextStyle::Heading, theme.fonts.heading),
            (egui::TextStyle::Monospace, theme.fonts.mono),
        ] {
            assert_eq!(style.text_styles.get(&style_of), Some(&token.id()), "{style_of:?}");
        }
    }
}

#[test]
fn mode_of_reads_the_context_and_apply_installs_both_modes_of_the_preset() {
    let ctx = egui::Context::default();
    let aurora = Preset::named("aurora").unwrap();
    let large = TextScale::from_percent(130).unwrap();
    let look = Look { preset: *aurora, text_scale: large, ..Look::default() };
    Theme::apply(&ctx, &look);
    assert_eq!(ctx.style_of(egui::Theme::Light).visuals.panel_fill, Theme::new(Mode::Light, aurora).palette.panel);
    assert_eq!(ctx.style_of(egui::Theme::Dark).visuals.panel_fill, Theme::new(Mode::Dark, aurora).palette.panel);
    let body = ctx.style_of(egui::Theme::Dark).text_styles.get(&egui::TextStyle::Body).map(|font| font.size);
    assert_eq!(body, Some(Theme::new(Mode::Dark, aurora).with_text_scale(large).fonts.body.size));
    ctx.set_theme(egui::Theme::Dark);
    assert_eq!(Mode::of(&ctx), Mode::Dark);
    assert_eq!(Theme::current(&ctx, &look), Theme::new(Mode::Dark, aurora).with_text_scale(large));
    ctx.set_theme(egui::Theme::Light);
    assert_eq!(Mode::of(&ctx), Mode::Light);
    assert_eq!(Theme::current(&ctx, &Look { preset: *aurora, ..Look::default() }), Theme::new(Mode::Light, aurora));
}

/// Whether `line` names a font of its own instead of a font token: a family, a font id, an egui text style or a
/// size set by hand.
fn names_a_font(line: &str) -> bool {
    let size_by_hand = line.split(".size(").skip(1).any(|rest| !rest.starts_with(')'));
    let styled = [".heading()", ".small()", ".strong()", ".monospace()", ".text_style(", "TextStyle::"].iter().any(|call| line.contains(call));
    line.contains("FontFamily") || line.contains("FontId") || size_by_hand || styled
}

#[test]
fn the_font_check_finds_families_sizes_and_styles() {
    assert!(names_a_font("TextFormat::simple(FontId::new(size, FontFamily::Proportional), c)"));
    assert!(names_a_font("RichText::new(text).size(theme.type_scale.small)"));
    assert!(names_a_font("RichText::new(text).strong()"));
    assert!(names_a_font("ui.label(RichText::new(x).heading())"));
    assert!(names_a_font("style.text_styles.insert(TextStyle::Body, x)"));
    assert!(!names_a_font("let width = galley.size().x;"));
    assert!(!names_a_font("RichText::new(text).font(theme.fonts.caption.id())"));
}

#[test]
fn no_view_names_a_font() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    for dir in ["src", "examples"] {
        if root.join(dir).is_dir() {
            sources(&root.join(dir), &mut files);
        }
    }
    let token_code = root.join("src").join("theme.rs");
    let mut found = Vec::new();
    for file in files.iter().filter(|file| **file != token_code) {
        let text = std::fs::read_to_string(file).unwrap();
        for (index, line) in text.lines().enumerate() {
            let code = line.split("//").next().unwrap_or_default();
            if names_a_font(code) {
                found.push(format!("{}:{}: {}", file.strip_prefix(root).unwrap().display(), index + 1, line.trim()));
            }
        }
    }
    assert!(found.is_empty(), "fonts are tokens of theme.rs:\n{}", found.join("\n"));
}

/// The `.rs` files under `dir`, test files left out.
fn sources(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().and_then(|name| name.to_str()).unwrap_or_default().to_owned();
        if path.is_dir() {
            sources(&path, found);
        } else if name.ends_with(".rs") && name != "tests.rs" && !name.ends_with("_tests.rs") {
            found.push(path);
        }
    }
}

/// Whether `line` names a colour of its own: an RGB or hex literal, a grey, an HSVA colour built from numbers, or a
/// named egui colour other than "none" and the placeholder.
fn names_a_colour(line: &str) -> bool {
    let literal = ["from_rgb(", "from_rgba_unmultiplied(", "from_rgba_premultiplied(", "from_gray(", "from_black_alpha(", "from_white_alpha("]
        .iter()
        .any(|call| line.split(call).skip(1).any(|rest| rest.trim_start().starts_with(|c: char| c.is_ascii_digit())));
    let named = line.split("Color32::").skip(1).any(|rest| {
        let constant: String = rest.chars().take_while(|c| c.is_ascii_uppercase() || *c == '_').collect();
        !constant.is_empty() && !["TRANSPARENT", "PLACEHOLDER"].contains(&constant.as_str())
    });
    let hex = line.contains("rgb(0x") || line.contains("Rgba::from_") || line.contains("from_hex(") || line.contains("Hsva::new(");
    literal || named || hex
}

#[test]
fn the_colour_check_finds_literals_and_named_colours() {
    assert!(names_a_colour("let x = Color32::from_rgb(0x1D, 0x6A, 0x64);"));
    assert!(names_a_colour("fill(Color32::WHITE)"));
    assert!(names_a_colour("Color32::from_gray(40)"));
    assert!(names_a_colour("const X: Color32 = rgb(0xffffff);"));
    assert!(names_a_colour("Color32::from_hex(\"#2f6bff\")"));
    assert!(names_a_colour("let h = Hsva::new(0.5, 1.0, 1.0, 1.0);"));
    assert!(!names_a_colour("Color32::from_rgba_premultiplied(*px.first()?, 1, 2, 3)"));
    assert!(!names_a_colour("painter.galley(pos, galley, Color32::PLACEHOLDER)"));
    assert!(!names_a_colour("let fill = p.surface;"));
}

#[test]
fn the_old_teal_and_coral_would_be_caught() {
    for old in [0x1D6A64, 0x6CC4BA, 0xF0A06A] {
        let colour = Color32::from_rgb((old >> 16) as u8, (old >> 8) as u8, old as u8);
        assert!(teal_or_coral(colour), "{}", hex(colour));
    }
}

#[test]
fn no_colour_constant_outside_the_token_code() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    for dir in ["src", "examples"] {
        if root.join(dir).is_dir() {
            sources(&root.join(dir), &mut files);
        }
    }
    assert!(files.len() > 10, "the walk found the sources");
    let token_code = root.join("src").join("theme.rs");
    let mut found = Vec::new();
    for file in files.iter().filter(|file| **file != token_code) {
        let text = std::fs::read_to_string(file).unwrap();
        for (index, line) in text.lines().enumerate() {
            let code = line.split("//").next().unwrap_or_default();
            if names_a_colour(code) {
                found.push(format!("{}:{}: {}", file.strip_prefix(root).unwrap().display(), index + 1, line.trim()));
            }
        }
    }
    assert!(found.is_empty(), "colours belong in theme.rs:\n{}", found.join("\n"));
}

#[test]
fn over_lays_a_translucent_colour_on_an_opaque_one() {
    let half = Color32::from_rgba_unmultiplied(200, 100, 0, 128);
    let under = Color32::from_rgb(0, 0, 200);
    let laid = over(half, under);
    assert_eq!(laid.a(), 255);
    assert!((i32::from(laid.r()) - 100).abs() <= 1 && (i32::from(laid.g()) - 50).abs() <= 1 && (i32::from(laid.b()) - 100).abs() <= 1, "{laid:?}");
    assert_eq!(over(Color32::TRANSPARENT, under), under);
    assert_eq!(over(Color32::from_rgb(1, 2, 3), under), Color32::from_rgb(1, 2, 3));
}

/// What the window shows behind a layer: the base, and each colour field at its centre.
fn backdrops(p: &Palette) -> Vec<Color32> {
    let mut found = vec![p.background];
    found.extend(p.colour_fields.map(|field| over(field, p.background)));
    found
}

#[test]
fn text_reads_on_glass_over_the_colour_fields_for_every_preset_in_both_modes() {
    for theme in all() {
        let p = theme.palette;
        for behind in backdrops(&p) {
            for glass in [over(p.glass, behind), over(p.hover, over(p.glass, behind))] {
                for (name, ink) in [("text", p.text), ("muted", p.muted), ("accent", p.accent), ("error", p.error)] {
                    let ratio = contrast(ink, glass);
                    assert!(ratio >= 4.5, "{} {:?} {name} on glass over {}: {ratio:.2}", theme.preset.id, theme.mode, hex(behind));
                }
            }
        }
    }
}

#[test]
fn text_reads_on_a_float_over_anything_for_every_preset_in_both_modes() {
    for theme in all() {
        let p = theme.palette;
        let mut behind = backdrops(&p);
        behind.extend(p.gradient);
        behind.extend([p.panel, p.surface]);
        // The plain float sits only where the content under it fades; the other one over sharp text too.
        let mut sharp = behind.clone();
        sharp.extend([p.text, p.muted, p.accent]);
        let floats = behind.into_iter().map(|under| (p.float, under)).chain(sharp.into_iter().map(|under| (p.float_over_text, under)));
        for (float, under) in floats {
            {
                for (name, ink) in [("text", p.text), ("muted", p.muted), ("accent", p.accent)] {
                    let ratio = contrast(ink, over(float, under));
                    assert!(ratio >= 4.5, "{} {:?} {name} on a float over {}: {ratio:.2}", theme.preset.id, theme.mode, hex(under));
                }
            }
        }
    }
}

#[test]
fn a_float_over_sharp_text_goes_more_opaque_until_the_text_under_it_is_quiet() {
    for theme in all() {
        let p = theme.palette;
        assert!(p.float_over_text.a() > p.float.a(), "{} {:?}: more opaque than {}", theme.preset.id, theme.mode, p.float.a());
        let mut surfaces = vec![p.background, p.panel, p.surface];
        surfaces.extend(backdrops(&p).into_iter().map(|behind| over(p.glass, behind)));
        for surface in surfaces {
            for ink in [p.text, p.muted, p.accent] {
                let ghost = contrast(over(p.float_over_text, ink), over(p.float_over_text, surface));
                assert!(ghost <= GHOST, "{} {:?}: text under the float shows at {ghost:.2}", theme.preset.id, theme.mode);
                let plain = contrast(over(p.float, ink), over(p.float, surface));
                assert!(plain >= ghost, "the plain float hides less");
            }
        }
    }
}

#[test]
fn menus_and_tooltips_float_over_sharp_text_and_nothing_casts_an_egui_shadow() {
    for theme in all() {
        let visuals = theme.visuals();
        assert_eq!(visuals.window_fill, theme.palette.float_over_text);
        assert_eq!(visuals.window_shadow, egui::epaint::Shadow::NONE);
        assert_eq!(visuals.popup_shadow, egui::epaint::Shadow::NONE);
        assert_eq!(theme.style().spacing.scroll.fade.strength, 0.0, "no fade of egui's own over text");
        let s = theme.spacing;
        assert!(s.card > s.corner, "glass layers are rounder than fields");
        assert!(s.shadow > 0.0 && s.gap > 0.0);
    }
}

/// Hue in degrees, saturation and value (HSV) of `colour`, each 0 to 1 but the hue.
fn hsv(colour: Color32) -> (f32, f32, f32) {
    let [r, g, b] = [colour.r(), colour.g(), colour.b()].map(|c| f32::from(c) / 255.0);
    let (high, low) = (r.max(g).max(b), r.min(g).min(b));
    let span = high - low;
    let hue = if span == 0.0 {
        0.0
    } else if high == r {
        60.0 * ((g - b) / span).rem_euclid(6.0)
    } else if high == g {
        60.0 * ((b - r) / span + 2.0)
    } else {
        60.0 * ((r - g) / span + 4.0)
    };
    (hue, if high == 0.0 { 0.0 } else { span / high }, high)
}

#[test]
fn the_avatar_palette_is_vivid_distinct_and_free_of_teal_and_coral() {
    assert!(AVATARS.len() >= 8);
    for colour in AVATARS {
        let (hue, saturation, value) = hsv(colour);
        assert!(saturation >= 0.7 && value >= 0.6, "{} is not vivid: {saturation} {value}", hex(colour));
        assert!(!(150.0..=200.0).contains(&hue), "{} is teal: {hue}", hex(colour));
        assert!(!teal_or_coral(colour), "{}", hex(colour));
    }
    for (index, one) in AVATARS.iter().enumerate() {
        for other in AVATARS.iter().skip(index + 1) {
            let distance: i32 =
                [(one.r(), other.r()), (one.g(), other.g()), (one.b(), other.b())].iter().map(|(a, b)| (i32::from(*a) - i32::from(*b)).abs()).sum();
            assert!(distance >= 90, "{} and {} are too alike", hex(*one), hex(*other));
        }
    }
    for theme in all() {
        assert_eq!(theme.palette.avatars, AVATARS, "{} {:?}", theme.preset.id, theme.mode);
    }
}

#[test]
fn every_avatar_colour_passes_wcag_aa_with_its_initials_in_both_modes() {
    for theme in all() {
        for colour in theme.palette.avatars {
            let ratio = contrast(ink_on(colour), colour);
            assert!(ratio >= 4.5, "{} {:?}: initials on {} at {ratio}", theme.preset.id, theme.mode, hex(colour));
        }
    }
}

#[test]
fn ink_over_several_colours_reads_on_every_one_of_them() {
    for theme in all() {
        let stops = theme.palette.gradient;
        let ink = ink_over(&stops);
        let worst = stops.iter().map(|stop| contrast(ink, *stop)).fold(f32::INFINITY, f32::min);
        assert!(worst >= 3.0, "{}: an icon on the gradient at {worst}", theme.preset.id);
        assert_eq!(ink_over(&[stops[0]]), ink_on(stops[0]));
    }
    assert_eq!(ink_over(&[]), ink_on(Color32::TRANSPARENT));
}

#[test]
fn icon_ink_over_prefers_white_wherever_it_reaches_3_to_1() {
    let white = Color32::WHITE;
    assert_eq!(icon_ink_over(&[rgb(0x2F6BFF), rgb(0x8A3FFC), rgb(0xFF2D95)]), white);
    assert_eq!(icon_ink_over(&[rgb(0xFF8F00), rgb(0xFFD000)]), ink_over(&[rgb(0xFF8F00), rgb(0xFFD000)]));
    assert_ne!(icon_ink_over(&[rgb(0xFFD000)]), white);
    assert_eq!(icon_ink_over(&[]), ink_over(&[]));
    for theme in all() {
        let stops = theme.palette.gradient;
        let ink = icon_ink_over(&stops);
        let worst = stops.iter().map(|stop| contrast(ink, *stop)).fold(f32::INFINITY, f32::min);
        assert!(worst >= 3.0, "{} {:?}: a solid icon on the gradient at {worst}", theme.preset.id, theme.mode);
    }
}

#[test]
fn the_theme_carries_the_motion_tokens_and_egui_s_animation_follows_them() {
    for theme in all() {
        assert_eq!(theme.motion, MOTION);
        assert_eq!(theme.style().animation_time, MOTION.tooltip);
    }
}
