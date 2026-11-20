// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::{Path, PathBuf};

use egui::Color32;

use super::{
    FALLBACK_PROBES, FLOAT_SHADOW, Face, Fallback, Font, Fonts, ICON, Look, Mode, ModePreference, Palette, SHIPPED_FALLBACKS, TextScale, Theme,
    contrast, font_definitions, font_definitions_with, install_fallbacks, install_fonts, missing_probes, mix, over, preference,
};
use crate::motion::MOTION;

/// Both modes.
fn all() -> Vec<Theme> {
    [Mode::Light, Mode::Dark].map(|mode| Theme::new(mode, TextScale::default())).to_vec()
}

fn hex(colour: Color32) -> String {
    let [r, g, b, a] = colour.to_srgba_unmultiplied();
    format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
}

/// The token named `name` (`acct1` to `acct6` for the account colours), as the design file names it.
fn token(p: &Palette, name: &str) -> Option<Color32> {
    Some(match name {
        "bg" => p.bg,
        "surface" => p.surface,
        "panel" => p.panel,
        "term" => p.term,
        "line" => p.line,
        "panel_line" => p.panel_line,
        "grid" => p.grid,
        "stage" => p.stage,
        "ink" => p.ink,
        "muted" => p.muted,
        "dim" => p.dim,
        "faint" => p.faint,
        "cyan" => p.cyan,
        "on_cyan" => p.on_cyan,
        "cyan_text" => p.cyan_text,
        "cyan_edge" => p.cyan_edge,
        "cyan_muted" => p.cyan_muted,
        "minus" => p.minus,
        "plus" => p.plus,
        "warn" => p.warn,
        "scrim" => p.scrim,
        _ => {
            let n: usize = name.strip_prefix("acct")?.parse().ok()?;
            *p.accounts.get(n.checked_sub(1)?)?
        }
    })
}

/// `#rrggbbaa` as a colour.
fn rgba(value: u32) -> Color32 {
    let [r, g, b, a] = value.to_be_bytes();
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

/// The design file's table, "Colour": token, dark, light, as `#rrggbbaa`.
const TOKENS: [(&str, u32, u32); 27] = [
    ("bg", 0x0E0F10FF, 0xF5F5F2FF),
    ("surface", 0x151617FF, 0xFFFFFFFF),
    ("panel", 0x101112FF, 0xF5F5F2FF),
    ("term", 0x0A0B0CFF, 0xFFFFFFFF),
    ("line", 0x2A2D2FFF, 0xD6D7D3FF),
    ("panel_line", 0x202224FF, 0xE2E3DFFF),
    ("grid", 0x17191AFF, 0xECECE8FF),
    ("stage", 0x212427FF, 0xDEDFDBFF),
    ("ink", 0xD4D4D4FF, 0x191919FF),
    ("muted", 0x8B9196FF, 0x5A6065FF),
    ("dim", 0x4A4F53FF, 0x9EA3A6FF),
    ("faint", 0x2B2E30FF, 0xCFD1CDFF),
    ("cyan", 0x22A8E0FF, 0x22A8E0FF),
    ("on_cyan", 0x0E0F10FF, 0x0E0F10FF),
    ("cyan_text", 0x22A8E0FF, 0x075E88FF),
    ("cyan_edge", 0x22A8E0FF, 0x0B79ADFF),
    ("cyan_muted", 0x00354CFF, 0xD3EEF9FF),
    ("minus", 0xC98080FF, 0x9A3A36FF),
    ("plus", 0x6AB26BFF, 0x246628FF),
    ("warn", 0xEEC448FF, 0x6E5400FF),
    ("scrim", 0x00000099, 0x19191955),
    ("acct1", 0x9DAF19FF, 0x778224FF),
    ("acct2", 0xA359B9FF, 0xB54ED5FF),
    ("acct3", 0xC05928FF, 0xD84F05FF),
    ("acct4", 0x547FFAFF, 0x163E8BFF),
    ("acct5", 0xF55BB8FF, 0x7C1457FF),
    ("acct6", 0x64DABFFF, 0x368977FF),
];

#[test]
fn the_tokens_are_the_design_file_s_values() {
    for (name, dark, light) in TOKENS {
        for (mode, want) in [(Mode::Dark, dark), (Mode::Light, light)] {
            let got = token(Palette::of(mode), name);
            assert_eq!(got, Some(rgba(want)), "{mode:?} {name}: {:?} for #{want:08x}", got.map(hex));
        }
    }
    assert_eq!(Theme::new(Mode::Dark, TextScale::default()).palette, Palette::of(Mode::Dark));
    assert_eq!(Theme::new(Mode::Light, TextScale::default()).palette, Palette::of(Mode::Light));
    assert_eq!(token(Palette::of(Mode::Dark), "acct7"), None);
}

/// The pairs the design file checks in each mode: text on a ground at 4.5:1, an edge or mark that carries meaning at
/// 3:1. Exempt and not listed: `line`, `panel_line` and `faint` (structure and decoration) and `dim` (decoration).
const PAIRS: [(&str, &str, f32); 71] = [
    ("ink", "bg", 4.5),
    ("ink", "surface", 4.5),
    ("ink", "panel", 4.5),
    ("ink", "term", 4.5),
    ("ink", "grid", 4.5),
    ("ink", "stage", 4.5),
    ("muted", "bg", 4.5),
    ("muted", "surface", 4.5),
    ("muted", "panel", 4.5),
    ("muted", "term", 4.5),
    ("muted", "grid", 4.5),
    ("muted", "stage", 4.5),
    ("cyan_text", "bg", 4.5),
    ("cyan_text", "surface", 4.5),
    ("cyan_text", "panel", 4.5),
    ("cyan_text", "term", 4.5),
    ("cyan_text", "grid", 4.5),
    ("cyan_text", "stage", 4.5),
    ("minus", "bg", 4.5),
    ("minus", "surface", 4.5),
    ("minus", "panel", 4.5),
    ("minus", "term", 4.5),
    ("minus", "grid", 4.5),
    ("minus", "stage", 4.5),
    ("plus", "bg", 4.5),
    ("plus", "surface", 4.5),
    ("plus", "panel", 4.5),
    ("plus", "term", 4.5),
    ("plus", "grid", 4.5),
    ("plus", "stage", 4.5),
    ("warn", "bg", 4.5),
    ("warn", "surface", 4.5),
    ("warn", "panel", 4.5),
    ("warn", "term", 4.5),
    ("warn", "grid", 4.5),
    ("warn", "stage", 4.5),
    ("on_cyan", "cyan", 4.5),
    ("cyan_edge", "bg", 3.0),
    ("cyan_edge", "surface", 3.0),
    ("cyan_edge", "panel", 3.0),
    ("cyan_edge", "term", 3.0),
    ("cyan_edge", "stage", 3.0),
    ("acct1", "bg", 3.0),
    ("acct1", "surface", 3.0),
    ("acct1", "panel", 3.0),
    ("acct1", "stage", 3.0),
    ("acct2", "bg", 3.0),
    ("acct2", "surface", 3.0),
    ("acct2", "panel", 3.0),
    ("acct2", "stage", 3.0),
    ("acct3", "bg", 3.0),
    ("acct3", "surface", 3.0),
    ("acct3", "panel", 3.0),
    ("acct3", "stage", 3.0),
    ("acct4", "bg", 3.0),
    ("acct4", "surface", 3.0),
    ("acct4", "panel", 3.0),
    ("acct4", "stage", 3.0),
    ("acct5", "bg", 3.0),
    ("acct5", "surface", 3.0),
    ("acct5", "panel", 3.0),
    ("acct5", "stage", 3.0),
    ("acct6", "bg", 3.0),
    ("acct6", "surface", 3.0),
    ("acct6", "panel", 3.0),
    ("acct6", "stage", 3.0),
    // The "coming soon" mark: its words and its dashed edge, checked as a pair of its own.
    ("muted", "bg", 4.5),
    ("muted", "surface", 4.5),
    ("muted", "panel", 4.5),
    ("muted", "term", 4.5),
    ("muted", "stage", 4.5),
];

#[test]
fn every_checked_pair_meets_its_ratio() {
    let modes = [Mode::Dark, Mode::Light];
    assert_eq!(PAIRS.len() * modes.len(), 142, "the design file checks 142 pairs over the two modes");
    for mode in modes {
        let p = Palette::of(mode);
        for (text, ground, need) in PAIRS {
            let (Some(a), Some(b)) = (token(p, text), token(p, ground)) else { panic!("{text} on {ground}: no such token") };
            let ratio = contrast(a, b);
            assert!(ratio >= need, "{mode:?} {text} on {ground}: {ratio:.2} < {need}");
        }
    }
}

#[test]
fn the_exempt_tokens_are_never_a_text_colour() {
    for exempt in ["line", "panel_line", "faint", "dim"] {
        assert!(PAIRS.iter().all(|(text, _, _)| *text != exempt), "{exempt} is decoration, never checked as text");
    }
}

#[test]
fn the_accent_is_no_account_colour() {
    for mode in [Mode::Dark, Mode::Light] {
        let p = Palette::of(mode);
        for (index, account) in p.accounts.iter().enumerate() {
            for accent in [p.cyan, p.cyan_text, p.cyan_edge] {
                assert_ne!(*account, accent, "{mode:?} account {index}");
            }
        }
    }
}

#[test]
fn sizes_scale_with_text_and_distances_do_not() {
    let large = TextScale::from_percent(130).unwrap();
    let theme = Theme::new(Mode::Dark, large);
    assert_eq!(theme.size(32.0), 42.0);
    assert_eq!(theme.size(36.0), 46.0);
    assert_eq!(theme.size(26.0), 34.0);
    assert_eq!(theme.size(34.0), 44.0);
    assert_eq!(theme.size(16.0), 20.0);
    assert_eq!(theme.spacing.s4, 16.0);
    let s = theme.spacing;
    assert_eq!([s.s1, s.s2, s.s3, s.s4, s.s6, s.s10], [4.0, 8.0, 12.0, 16.0, 24.0, 40.0]);
    assert_eq!(theme.spacing, Theme::new(Mode::Dark, TextScale::default()).spacing);
    let z = theme.sizes;
    assert_eq!(
        [z.button, z.field, z.menu_row, z.mail_row, z.group_head, z.chip, z.list, z.scrollbar],
        [32.0, 36.0, 32.0, 56.0, 32.0, 20.0, 440.0, 12.0]
    );
    assert_eq!(z.avatar, [24.0, 32.0, 40.0]);
    assert_eq!([z.icon, z.mark], [16.0, 14.0]);
    assert_eq!(theme.icon_side(), 20.0);
    assert_eq!(Theme::new(Mode::Light, TextScale::default()).size(32.0), 32.0);
}

#[test]
fn the_float_shadow_is_the_one_shadow() {
    assert_eq!(FLOAT_SHADOW.offset, [0, 12]);
    assert_eq!(FLOAT_SHADOW.blur, 32);
    assert_eq!(FLOAT_SHADOW.spread, 0);
    assert_eq!(FLOAT_SHADOW.color, Color32::from_black_alpha(102), "black at 40 %");
}

#[test]
fn account_colours_repeat_from_the_seventh() {
    for theme in all() {
        let colours = theme.palette.accounts;
        assert_eq!(theme.account_colour(0), colours[0]);
        assert_eq!(theme.account_colour(5), colours[5]);
        assert_eq!(theme.account_colour(6), colours[0], "wraps around");
        assert_eq!(theme.account_colour(u64::MAX), colours[(u64::MAX % 6) as usize]);
    }
}

#[test]
fn the_visuals_follow_the_tokens_flat_and_square() {
    for theme in all() {
        let p = theme.palette;
        let visuals = theme.visuals();
        assert_eq!(visuals.dark_mode, theme.mode == Mode::Dark);
        assert_eq!(visuals.panel_fill, p.bg);
        assert_eq!(visuals.window_fill, p.surface);
        assert_eq!(visuals.extreme_bg_color, p.term);
        assert_eq!(visuals.selection.bg_fill, p.cyan_muted);
        assert_eq!(visuals.hyperlink_color, p.cyan_text);
        assert_eq!(visuals.override_text_color, Some(p.ink));
        // Glyph coverage is alpha as it is, in both modes, as the design draws it: no boost in the dark.
        assert_eq!(visuals.text_options.color_transfer_function, egui::epaint::FontColorTransferFunction::Off, "{:?}", theme.mode);
        assert_eq!(visuals.window_shadow, egui::epaint::Shadow::NONE);
        assert_eq!(visuals.popup_shadow, egui::epaint::Shadow::NONE);
        assert_eq!(visuals.window_corner_radius, egui::CornerRadius::ZERO);
        assert_eq!(visuals.menu_corner_radius, egui::CornerRadius::ZERO);
        let w = &visuals.widgets;
        for state in [w.noninteractive, w.inactive, w.hovered, w.active, w.open] {
            assert_eq!(state.corner_radius, egui::CornerRadius::ZERO);
            assert_eq!(state.bg_stroke.color, p.line);
        }
        let style = theme.style();
        assert_eq!(style.spacing.item_spacing.y, theme.spacing.s1);
        assert_eq!(style.spacing.scroll.fade.strength, 0.0, "no fade of egui's own over text");
        // The thumb grows under the pointer inside a strip of one width: the scroll bar component paints it.
        assert_eq!(style.spacing.scroll, crate::component::scroll::style(&theme), "the scroll bar's own style");
        for (style_of, font) in [
            (egui::TextStyle::Small, theme.fonts.caption),
            (egui::TextStyle::Body, theme.fonts.body),
            (egui::TextStyle::Button, theme.fonts.button),
            (egui::TextStyle::Heading, theme.fonts.heading),
            (egui::TextStyle::Monospace, theme.fonts.mono),
        ] {
            assert_eq!(style.text_styles.get(&style_of), Some(&font.id()), "{style_of:?}");
        }
    }
}

#[test]
fn a_look_takes_the_owner_s_choices() {
    use inboxcraft_engine::appearance::Appearance;
    let appearance = Appearance { text_scale: 110, ..Appearance::default() };
    let look = Look::of(&appearance);
    assert_eq!(look.text_scale.percent(), 110);
    let theme = Theme::of(Mode::Dark, &look);
    assert_eq!(theme, Theme::new(Mode::Dark, look.text_scale));
    assert_eq!(theme.palette, Palette::of(Mode::Dark));
    assert_eq!(Look::of(&Appearance::default()), Look::default());
    assert_eq!(Look::of(&Appearance { text_scale: 95, ..Appearance::default() }).text_scale, TextScale::default());
}

#[test]
fn mode_of_reads_the_context_and_apply_installs_both_modes() {
    let ctx = egui::Context::default();
    let large = TextScale::from_percent(130).unwrap();
    let look = Look { text_scale: large };
    Theme::apply(&ctx, &look);
    assert_eq!(ctx.style_of(egui::Theme::Light).visuals.panel_fill, Palette::of(Mode::Light).bg);
    assert_eq!(ctx.style_of(egui::Theme::Dark).visuals.panel_fill, Palette::of(Mode::Dark).bg);
    let body = ctx.style_of(egui::Theme::Dark).text_styles.get(&egui::TextStyle::Body).map(|font| font.size);
    assert_eq!(body, Some(Theme::new(Mode::Dark, large).fonts.body.size));
    ctx.set_theme(egui::Theme::Dark);
    assert_eq!(Mode::of(&ctx), Mode::Dark);
    assert_eq!(Theme::current(&ctx, &look), Theme::new(Mode::Dark, large));
    ctx.set_theme(egui::Theme::Light);
    assert_eq!(Mode::of(&ctx), Mode::Light);
    assert_eq!(Theme::current(&ctx, &Look::default()), Theme::new(Mode::Light, TextScale::default()));
}

/// Whether `line` paints a gradient, a blur or a translucent fill of its own: a mesh, texture coordinates, a blurred
/// rect, a gradient, or a colour made translucent in code.
fn paints_a_gradient_or_a_blur(line: &str) -> bool {
    ["Mesh", "new_uv", "with_blur_width", "Gradient", "gamma_multiply(", "linear_multiply(", "from_black_alpha(", "from_white_alpha("]
        .iter()
        .any(|word| line.contains(word))
}

#[test]
fn the_gradient_check_finds_meshes_blurs_and_washes() {
    assert!(paints_a_gradient_or_a_blur("let mut mesh = Mesh::default();"));
    assert!(paints_a_gradient_or_a_blur("shape.with_blur_width(8.0)"));
    assert!(paints_a_gradient_or_a_blur("let fill = Gradient::Linear { .. };"));
    assert!(paints_a_gradient_or_a_blur("painter.rect_filled(rect, 0.0, p.stage.gamma_multiply(hover));"));
    assert!(!paints_a_gradient_or_a_blur("painter.rect_filled(rect, 0.0, p.stage);"));
}

#[test]
fn no_code_paints_a_gradient_or_a_blur() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    for dir in ["src", "examples"] {
        if root.join(dir).is_dir() {
            sources(&root.join(dir), &mut files);
        }
    }
    // The tokens (`scrim` and `FLOAT_SHADOW`), the float painter and the picture rasteriser are allowed.
    let src = root.join("src");
    let allowed = [src.join("theme.rs"), src.join("draw").join("paint.rs"), src.join("shot.rs")];
    let mut found = Vec::new();
    for file in files.iter().filter(|file| !allowed.contains(file)) {
        let text = std::fs::read_to_string(file).unwrap();
        for (index, line) in text.lines().enumerate() {
            let code = line.split("//").next().unwrap_or_default();
            if paints_a_gradient_or_a_blur(code) {
                found.push(format!("{}:{}: {}", file.strip_prefix(root).unwrap().display(), index + 1, line.trim()));
            }
        }
    }
    assert!(found.is_empty(), "flat fills only:\n{}", found.join("\n"));
}

#[test]
fn contrast_of_black_on_white_is_21_and_of_a_colour_on_itself_is_1() {
    assert!((contrast(Color32::BLACK, Color32::WHITE) - 21.0).abs() < 0.01);
    assert!((contrast(Color32::WHITE, Color32::BLACK) - 21.0).abs() < 0.01);
    let grey = Color32::from_rgb(120, 120, 120);
    assert!((contrast(grey, grey) - 1.0).abs() < 0.001);
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
fn mix_runs_from_one_colour_to_the_other() {
    let (a, b) = (Color32::from_rgb(0, 100, 200), Color32::from_rgb(200, 0, 100));
    assert_eq!(mix(a, b, 0.0), a);
    assert_eq!(mix(a, b, 1.0), b);
    assert_eq!(mix(a, b, 0.5), Color32::from_rgb(100, 50, 150));
    assert_eq!(mix(a, b, -1.0), a);
    assert_eq!(mix(a, b, f32::NAN), a);
}

/// A style as the design file's table gives it: face, size, line, tracking and capitals.
fn style(face: Face, size: f32, line: f32, tracking: f32, caps: bool) -> Font {
    Font { face, size, line, tracking, caps }
}

#[test]
fn the_twelve_styles_are_the_design_file_s() {
    let f = Fonts::of(TextScale::default());
    let expected = [
        ("display", f.display, style(Face::Mono, 40.0, 48.0, -0.01, false)),
        ("title", f.title, style(Face::Mono, 22.0, 30.0, 0.0, false)),
        ("heading", f.heading, style(Face::MonoSemiBold, 15.0, 22.0, 0.0, false)),
        ("label", f.label, style(Face::Mono, 11.0, 16.0, 0.08, true)),
        ("mono", f.mono, style(Face::Mono, 13.0, 20.0, 0.0, false)),
        ("mono_s", f.mono_s, style(Face::Mono, 11.0, 16.0, 0.0, false)),
        ("key", f.key, style(Face::MonoSemiBold, 11.0, 14.0, 0.0, false)),
        ("button", f.button, style(Face::Mono, 13.0, 16.0, 0.0, false)),
        ("body", f.body, style(Face::Sans, 14.0, 22.0, 0.0, false)),
        ("strong", f.strong, style(Face::SansSemiBold, 14.0, 22.0, 0.0, false)),
        ("caption", f.caption, style(Face::Sans, 12.0, 18.0, 0.0, false)),
        ("read", f.read, style(Face::Sans, 16.0, 26.0, 0.0, false)),
        ("brand", f.brand, style(Face::Brand, 22.0, 30.0, 0.0, false)),
    ];
    for (name, got, want) in expected {
        assert_eq!(got, want, "{name}");
    }
    assert_eq!(f.all().len(), expected.len(), "every style is listed");
    // Type is not rounded (boxes are): at 130 % the body is 18.2 on 28.6, its tracking untouched.
    let large = Fonts::of(TextScale::from_percent(130).unwrap());
    assert!((large.body.size - 18.2).abs() < 1e-4 && (large.body.line - 28.6).abs() < 1e-4, "{:?}", large.body);
    assert_eq!(large.label.tracking, 0.08);
    assert!(large.label.caps);
}

#[test]
fn a_style_s_format_carries_its_line_height_and_tracking_in_points() {
    let f = Fonts::of(TextScale::default());
    let format = f.label.format(Color32::RED);
    assert_eq!(format.font_id, f.label.id());
    assert_eq!(format.line_height, Some(16.0));
    assert!((format.extra_letter_spacing - 0.88).abs() < 1e-5, "{}", format.extra_letter_spacing);
    assert_eq!(format.color, Color32::RED);
    assert_eq!(f.label.text("Mail"), "MAIL");
    assert_eq!(f.body.text("Mail"), "Mail");
    let brand = f.brand.sized(44.0);
    assert_eq!((brand.size, brand.line), (44.0, 60.0));
    for wrong in [0.0, -3.0, f32::NAN, f32::INFINITY] {
        assert_eq!(f.brand.sized(wrong), f.brand, "{wrong}");
    }
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
    let base = Theme::new(Mode::Dark, TextScale::default());
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
            assert_eq!((scaled.face, scaled.tracking, scaled.caps), (plain.face, plain.tracking, plain.caps));
            assert!((scaled.size - plain.size * scale.factor()).abs() < 1e-4, "{scale:?}: {scaled:?} from {plain:?}");
            assert!((scaled.line - plain.line * scale.factor()).abs() < 1e-4, "{scale:?}: {scaled:?} from {plain:?}");
        }
        let body = theme.style().text_styles.get(&egui::TextStyle::Body).map(|font| font.size);
        assert_eq!(body, Some(theme.fonts.body.size));
    }
    // Scaling twice starts from the 100 % sizes again.
    let twice = base.with_text_scale(TextScale::ALL[4]).with_text_scale(TextScale::ALL[0]);
    assert_eq!(twice, base.with_text_scale(TextScale::ALL[0]));
}

#[test]
fn every_style_names_a_family_whose_first_font_is_its_face() {
    let definitions = font_definitions();
    for font in Fonts::of(TextScale::default()).all() {
        let id = font.id();
        assert_eq!(id.size, font.size);
        let family = definitions.families.get(&id.family).unwrap_or_else(|| panic!("{:?} is defined", id.family));
        assert_eq!(family.first().map(String::as_str), Some(font.face.name()), "{font:?}");
        assert!(family.len() > 1, "{:?} falls back to egui's faces", font.face);
        assert!(family.iter().all(|name| definitions.font_data.contains_key(name)), "{family:?}");
    }
}

#[test]
fn plex_replaces_the_bundled_faces() {
    let bundled = egui::FontDefinitions::default();
    let definitions = font_definitions();
    for (family, first) in [(egui::FontFamily::Proportional, "IBMPlexSans-Regular"), (egui::FontFamily::Monospace, "IBMPlexMono-Regular")] {
        let fonts = definitions.families.get(&family).unwrap();
        assert_eq!(fonts.first().map(String::as_str), Some(first), "{family:?}");
        assert!(!["Ubuntu-Light", "Hack"].contains(&fonts[0].as_str()));
        // egui's own faces stay behind: the fallbacks and the icon fonts, Hack behind Plex Sans too.
        for font in bundled.families.get(&family).unwrap().iter().chain(std::iter::once(&"Hack".to_owned())) {
            assert!(fonts[1..].contains(font), "{family:?}: {font} in {fonts:?}");
        }
    }
    for face in Face::ALL {
        assert!(definitions.font_data.contains_key(face.name()), "{face:?}");
    }
}

#[test]
fn install_fonts_puts_plex_into_the_context() {
    let ctx = egui::Context::default();
    install_fonts(&ctx);
    ctx.run_ui(egui::RawInput::default(), |_| {}).textures_delta.clear();
    let families = ctx.fonts(|fonts| fonts.definitions().families.clone());
    assert_eq!(families.get(&egui::FontFamily::Proportional).and_then(|fonts| fonts.first()).map(String::as_str), Some("IBMPlexSans-Regular"));
    let f = Fonts::of(TextScale::default());
    let width = |font: Font| ctx.fonts_mut(|fonts| fonts.glyph_width(&font.id(), 'm'));
    assert!(width(f.strong) > width(f.body), "SemiBold is its own face");
    assert_eq!(width(f.heading), width(f.title.sized(15.0)), "Plex Mono keeps one advance in both weights");
}

/// The shipped Plex Sans as a fallback under another name.
fn plex_as(name: &str) -> Fallback {
    Fallback { name: name.to_owned(), data: include_bytes!("../../../assets/fonts/plex-sans/IBMPlexSans-Regular.ttf").to_vec(), index: 0 }
}

#[test]
fn fallbacks_stand_behind_plex_in_every_text_family() {
    let definitions = font_definitions_with(&[plex_as("system: Fake")]);
    let icon = egui::FontFamily::Name(ICON.into());
    let mut text_families = 0;
    for (family, fonts) in &definitions.families {
        if *family == icon {
            assert!(!fonts.iter().any(|name| name == "system: Fake"), "the icon family holds no fallback");
        } else {
            text_families += 1;
            assert_eq!(fonts.last().map(String::as_str), Some("system: Fake"), "{family:?}: {fonts:?}");
        }
    }
    assert_eq!(text_families, 5, "the five faces' families, Proportional and Monospace among them");
    let proportional = definitions.families.get(&egui::FontFamily::Proportional).unwrap();
    assert_eq!(proportional.first().map(String::as_str), Some("IBMPlexSans-Regular"));
    assert!(definitions.font_data.contains_key("system: Fake"));
}

#[test]
fn a_fallback_that_is_not_a_font_is_skipped() {
    let junk = Fallback { name: "junk".to_owned(), data: b"not a font".to_vec(), index: 0 };
    let far = Fallback { index: 99, ..plex_as("far") };
    let empty = Fallback { name: "empty".to_owned(), data: Vec::new(), index: 0 };
    let fallbacks = [junk, far, empty];
    let definitions = font_definitions_with(&fallbacks);
    for name in ["junk", "far", "empty"] {
        assert!(!definitions.font_data.contains_key(name), "{name}");
        assert!(definitions.families.values().all(|fonts| !fonts.iter().any(|font| font == name)), "{name}");
    }
    let ctx = egui::Context::default();
    let _ = install_fallbacks(&ctx, fallbacks.to_vec());
    ctx.run_ui(egui::RawInput::default(), |ui| {
        ui.label("日本語 🙂");
    })
    .textures_delta
    .clear();
}

#[test]
fn a_fallback_never_replaces_one_of_the_app_s_faces() {
    let definitions = font_definitions_with(&[plex_as("IBMPlexMono-Regular"), plex_as(ICON)]);
    let mono = definitions.families.get(&egui::FontFamily::Monospace).unwrap();
    assert_eq!(mono.iter().filter(|name| *name == "IBMPlexMono-Regular").count(), 1);
    assert_eq!(definitions.families.get(&egui::FontFamily::Name(ICON.into())).map(Vec::len), Some(1));
}

#[test]
fn no_fallbacks_is_today_s_definitions() {
    let (with, without) = (font_definitions_with(&[]), font_definitions());
    assert_eq!(with.families, without.families);
    assert_eq!(with.font_data.keys().collect::<Vec<_>>(), without.font_data.keys().collect::<Vec<_>>());
    for (name, data) in &with.font_data {
        let other = &without.font_data[name];
        assert_eq!((data.bytes(), data.index), (other.bytes(), other.index), "{name}");
    }
}

#[test]
fn install_fallbacks_puts_them_into_the_context_and_asks_for_one_repaint() {
    let ctx = fonts_ctx();
    let fallback = plex_as("system: Fake");
    let bytes = fallback.data.as_ptr();
    let definitions = install_fallbacks(&ctx, vec![fallback]);
    assert!(ctx.has_requested_repaint(), "the new fonts are drawn without waiting for input");
    assert_eq!(definitions.font_data["system: Fake"].bytes().as_ptr(), bytes, "the bytes move in, not copied");
    ctx.run_ui(egui::RawInput::default(), |_| {}).textures_delta.clear();
    let families = ctx.fonts(|fonts| fonts.definitions().families.clone());
    assert_eq!(families.get(&egui::FontFamily::Proportional).and_then(|fonts| fonts.last()).map(String::as_str), Some("system: Fake"));
    assert_eq!(ctx.fonts(|fonts| fonts.definitions().font_data["system: Fake"].bytes().as_ptr()), bytes, "nor in the context");
}

#[test]
fn the_computer_is_searched_for_every_script_the_app_s_fonts_lack() {
    // The shipped Noto faces hold Arabic, Hebrew, Thai, Devanagari and emoji; Plex, Audiowide and egui's bundled faces
    // (Ubuntu Light, Hack, the egui icons; egui's emoji faces are not compiled in) hold none of the other probes, so
    // the start searches for those, and only those.
    let shipped = ["arabic", "hebrew", "thai", "devanagari", "emoji"];
    let others: Vec<(&str, char)> = FALLBACK_PROBES.iter().copied().filter(|(script, _)| !shipped.contains(script)).collect();
    assert_eq!(missing_probes(&FALLBACK_PROBES), others);
    assert_eq!(others.len(), 21, "Chinese, Japanese, Korean and the other scripts mail carries");
    assert_eq!(missing_probes(&[("latin", 'A'), ("arrows", '⇧'), ("han", '日')]), vec![("han", '日')], "Plex and Hack hold these");
    let mut scripts: Vec<&str> = FALLBACK_PROBES.iter().map(|(script, _)| *script).collect();
    scripts.sort_unstable();
    scripts.dedup();
    assert_eq!(scripts.len(), FALLBACK_PROBES.len(), "one probe per script");
    assert!(FALLBACK_PROBES.len() <= 32, "a found face's probes are bits of a u32");
}

/// One string of each script a shipped Noto face holds: `(script, text)`.
const SHIPPED_SCRIPTS: [(&str, &str); 5] = [
    ("hebrew", "שלום עולם, מה שלומך?"),
    ("arabic", "مرحبا بالعالم، كيف حالك؟"),
    ("devanagari", "नमस्ते दुनिया, क्षत्रिय"),
    ("thai", "สวัสดีชาวโลก ภาษาไทย"),
    ("emoji", "🙂 👍 🎉 ❤ ✌ 😀"),
];

#[test]
fn the_shipped_scripts_lay_out_with_no_box_in_every_text_style() {
    // The test harness panics on a sign no font holds, where the app draws a box: laying the strings out in it is the
    // check that every glyph of every galley comes from a font.
    let mut harness = egui_kittest::Harness::builder().with_context_setup(install_fonts).build_ui(|ui| {
        for font in Fonts::of(TextScale::default()).all() {
            for (_, text) in SHIPPED_SCRIPTS {
                let galley = crate::component::galley(ui, text, font, Color32::BLACK);
                assert!(galley.size().x > 0.0, "{font:?} {text}");
                ui.label(egui::RichText::new(text).font(font.id()));
            }
        }
    });
    harness.run();
    let ctx = fonts_ctx();
    for font in Fonts::of(TextScale::default()).all() {
        for (script, text) in SHIPPED_SCRIPTS {
            let galley = laid_out(&ctx, text, font);
            // A shaped run keeps a glyph per character, the ones a cluster took in with no picture of their own.
            let glyphs: Vec<&egui::epaint::text::Glyph> = galley.rows.iter().flat_map(|row| row.glyphs.iter()).collect();
            let drawn = glyphs.iter().filter(|glyph| !glyph.uv_rect.is_nothing()).count();
            assert!(drawn >= text.split_whitespace().count() && galley.size().x > 0.0, "{script} in {font:?}: {drawn} drawn");
            for glyph in &glyphs {
                assert!(text.contains(glyph.chr), "{script} in {font:?}: {:?} is not the text's", glyph.chr);
            }
            for sign in text.chars().filter(|sign| !sign.is_whitespace()) {
                assert!(ctx.fonts_mut(|fonts| fonts.has_glyph(&font.id(), sign)), "{script} in {font:?}: {sign:?} is a box");
            }
        }
    }
}

#[test]
fn plex_then_the_shipped_noto_faces_then_the_computer_s_in_every_text_family() {
    let definitions = font_definitions_with(&[plex_as("system: Fake")]);
    let icon = egui::FontFamily::Name(ICON.into());
    let [hebrew, arabic, devanagari, thai, emoji] = SHIPPED_FALLBACKS;
    for (family, fonts) in &definitions.families {
        if *family == icon {
            assert_eq!(fonts, &vec![ICON.to_owned()], "the icon family is Phosphor alone");
            continue;
        }
        let first = fonts.first().map(String::as_str).unwrap_or_default();
        assert!(Face::ALL.iter().any(|face| face.name() == first), "{family:?}: the app's face first, {fonts:?}");
        assert_eq!(&fonts[1..5], &[hebrew, arabic, devanagari, thai], "{family:?}: the script faces right behind it");
        let n = fonts.len();
        assert_eq!(&fonts[n - 2..], &[emoji, "system: Fake"], "{family:?}: the emoji face, then the computer's: {fonts:?}");
        assert!(fonts[5..n - 2].iter().all(|font| egui::FontDefinitions::builtin_font_names().contains(&font.as_str())), "{fonts:?}");
        for name in SHIPPED_FALLBACKS {
            assert_eq!(fonts.iter().filter(|font| *font == name).count(), 1, "{family:?}: {name} once");
        }
    }
    for name in SHIPPED_FALLBACKS {
        assert!(definitions.font_data.contains_key(name), "{name}");
    }
}

/// `text` laid out in `font` the way the components do it, in a frame of `ctx`.
fn laid_out(ctx: &egui::Context, text: &str, font: Font) -> std::sync::Arc<egui::Galley> {
    let mut galley = None;
    ctx.run_ui(egui::RawInput::default(), |ui| galley = Some(crate::component::galley(ui, text, font, Color32::BLACK))).textures_delta.clear();
    galley.unwrap()
}

fn fonts_ctx() -> egui::Context {
    let ctx = egui::Context::default();
    install_fonts(&ctx);
    ctx.run_ui(egui::RawInput::default(), |_| {}).textures_delta.clear();
    ctx
}

#[test]
fn text_the_fonts_do_not_hold_falls_back() {
    let ctx = fonts_ctx();
    let f = Fonts::of(TextScale::default());
    // Right-to-left and shaped text is no longer one glyph per character in order now that faces hold it
    // (`the_shipped_scripts_lay_out_with_no_box_in_every_text_style` lays it out).
    let text = "日本語 🙂";
    for font in [f.body, f.mono] {
        let galley = laid_out(&ctx, text, font);
        let chars: String = galley.rows.iter().flat_map(|row| row.glyphs.iter().map(|glyph| glyph.chr)).collect();
        assert_eq!(chars, text, "every character is laid out: {font:?}");
        assert!(galley.size().x > 0.0 && galley.size().y > 0.0);
        // Signs Plex does not hold come from the faces behind it: emoji from the shipped Noto Emoji, the
        // rest from egui's (Hack, Ubuntu Light). No shipped face holds Japanese: it is boxes (`docs/gaps.md`).
        for sign in ['●', '▶', '⇧', 'ǅ'] {
            assert!(ctx.fonts_mut(|fonts| fonts.has_glyph(&font.id(), sign)), "{font:?} {sign}");
        }
    }
}

#[test]
fn a_label_is_capitals_with_tracking() {
    let ctx = fonts_ctx();
    let label = Fonts::of(TextScale::default()).label;
    let tracked = laid_out(&ctx, "Mail", label);
    assert_eq!(tracked.job.text, "MAIL");
    let plain = laid_out(&ctx, "Mail", Font { tracking: 0.0, ..label });
    let extra = tracked.size().x - plain.size().x;
    assert!((extra - 3.0 * 0.08 * 11.0).abs() <= 0.5, "{extra}");
    assert_eq!(tracked.size().y, 16.0, "the line height is the style's");
}

#[test]
fn the_mono_face_has_one_advance() {
    let ctx = fonts_ctx();
    let mono = Fonts::of(TextScale::default()).mono;
    let narrow = laid_out(&ctx, "iiii", mono).size().x;
    let wide = laid_out(&ctx, "MMMM", mono).size().x;
    assert!((narrow - wide).abs() < 0.01, "{narrow} {wide}");
}

/// Whether `line` names a font of its own instead of a font token: a family, a font id, an egui text style, a face,
/// or a size set by hand (`.size(` or a token `.sized(`).
fn names_a_font(line: &str) -> bool {
    let size_by_hand = line.split(".size(").skip(1).any(|rest| !rest.starts_with(')'));
    let styled = [".heading()", ".small()", ".strong()", ".monospace()", ".text_style(", "TextStyle::", "Face::", ".sized("]
        .iter()
        .any(|call| line.contains(call));
    line.contains("FontFamily") || line.contains("FontId") || size_by_hand || styled
}

#[test]
fn the_font_check_finds_families_sizes_and_styles() {
    assert!(names_a_font("TextFormat::simple(FontId::new(size, FontFamily::Proportional), c)"));
    assert!(names_a_font("RichText::new(text).size(theme.type_scale.small)"));
    assert!(names_a_font("RichText::new(text).strong()"));
    assert!(names_a_font("ui.label(RichText::new(x).heading())"));
    assert!(names_a_font("style.text_styles.insert(TextStyle::Body, x)"));
    assert!(names_a_font("Font { face: Face::SansSemiBold, ..theme.fonts.caption }"));
    assert!(names_a_font("theme.fonts.title.sized(30.0)"));
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

#[test]
fn the_theme_carries_the_motion_tokens_and_egui_s_animation_follows_them() {
    for theme in all() {
        assert_eq!(theme.motion, MOTION);
        assert_eq!(theme.style().animation_time, MOTION.tooltip);
    }
}

/// The font atlas `ctx` builds in `mode` for a few words in the mono and the sans: every texel's alpha.
fn atlas_in(mode: egui::Theme) -> Vec<u8> {
    let ctx = egui::Context::default();
    install_fonts(&ctx);
    Theme::apply(&ctx, &Look::default());
    ctx.set_theme(mode);
    let mut atlas = Vec::new();
    for _ in 0..2 {
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let theme = Theme::current(ui.ctx(), &Look::default());
            ui.label(theme.fonts.mono_s.rich("Archive H 01"));
            ui.label(theme.fonts.body.rich("Marked as done"));
        });
        for (_, patches) in output.textures_delta.set.drain() {
            for patch in patches {
                let egui::ImageData::Color(image) = &patch.image;
                if patch.pos.is_none() {
                    atlas = image.pixels.iter().map(|texel| texel.a()).collect();
                }
            }
        }
    }
    atlas
}

#[test]
fn glyphs_have_the_same_linear_coverage_in_dark_and_light() {
    let (dark, light) = (atlas_in(egui::Theme::Dark), atlas_in(egui::Theme::Light));
    assert!(dark.iter().any(|alpha| *alpha > 0 && *alpha < 255), "the atlas holds edge texels");
    let differ = dark.iter().zip(&light).filter(|(d, l)| d != l).count();
    assert_eq!(differ, 0, "{differ} texels of the dark atlas are not the light one's");
}

/// `text` as the compiler reads it, for a scan: comments and the insides of string and character literals dropped,
/// and white space dropped wherever it does not stand between two parts of words, so that a call split over lines or
/// spaced out reads as one (`ui . button (` is `ui.button(`). The second value is the line (from 1) of each character
/// kept.
fn squeezed(text: &str) -> (String, Vec<usize>) {
    let chars: Vec<char> = text.chars().collect();
    let word = |c: char| c.is_alphanumeric() || c == '_';
    let (mut out, mut lines) = (String::new(), Vec::new());
    let (mut line, mut at, mut gap) = (1, 0, false);
    // Skips to the end of a quoted run that started before `at`, counting its lines.
    let skip_quoted = |at: &mut usize, line: &mut usize, close: &str| {
        let close: Vec<char> = close.chars().collect();
        while *at < chars.len() {
            if chars[*at] == '\n' {
                *line += 1;
            }
            if close.len() == 1 && chars[*at] == '\\' {
                *at += 2;
                continue;
            }
            if chars[*at..].starts_with(&close) {
                *at += close.len();
                return;
            }
            *at += 1;
        }
    };
    while at < chars.len() {
        let c = chars[at];
        let next = chars.get(at + 1).copied();
        let before = out.chars().last();
        if c == '\n' {
            line += 1;
            gap = true;
            at += 1;
        } else if c.is_whitespace() {
            gap = true;
            at += 1;
        } else if c == '/' && next == Some('/') {
            while at < chars.len() && chars[at] != '\n' {
                at += 1;
            }
        } else if c == '/' && next == Some('*') {
            let mut depth = 0;
            while at < chars.len() {
                if chars[at..].starts_with(&['/', '*']) {
                    depth += 1;
                    at += 2;
                } else if chars[at..].starts_with(&['*', '/']) {
                    depth -= 1;
                    at += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    if chars[at] == '\n' {
                        line += 1;
                    }
                    at += 1;
                }
            }
            gap = true;
        } else if (c == 'r' || (c == 'b' && next == Some('r'))) && !(before.is_some_and(word) && !gap) && {
            let start = if c == 'b' { at + 2 } else { at + 1 };
            let hashes = chars[start.min(chars.len())..].iter().take_while(|c| **c == '#').count();
            chars.get(start + hashes) == Some(&'"')
        } {
            let start = if c == 'b' { at + 2 } else { at + 1 };
            let hashes = chars[start..].iter().take_while(|c| **c == '#').count();
            at = start + hashes + 1;
            skip_quoted(&mut at, &mut line, &format!("\"{}", "#".repeat(hashes)));
            out.push_str("\"\"");
            lines.extend([line, line]);
            gap = false;
        } else if c == '"' {
            at += 1;
            skip_quoted(&mut at, &mut line, "\"");
            out.push_str("\"\"");
            lines.extend([line, line]);
            gap = false;
        } else if c == '\'' && (next == Some('\\') || chars.get(at + 2) == Some(&'\'')) {
            at += 1;
            skip_quoted(&mut at, &mut line, "'");
            out.push_str("''");
            lines.extend([line, line]);
            gap = false;
        } else {
            if gap && before.is_some_and(word) && word(c) {
                out.push(' ');
                lines.push(line);
            }
            gap = false;
            out.push(c);
            lines.push(line);
            at += 1;
        }
    }
    (out, lines)
}

/// Whether `name` stands in `code` at `at` as a word of its own.
fn word_at(code: &str, at: usize, name: &str) -> bool {
    let word = |c: char| c.is_alphanumeric() || c == '_';
    code[at..].starts_with(name) && !code[..at].chars().last().is_some_and(word) && !code[at + name.len()..].chars().next().is_some_and(word)
}

/// Where in `code` the word `name` stands, as a word of its own.
fn words(code: &str, name: &str) -> Vec<usize> {
    code.match_indices(name).map(|(at, _)| at).filter(|at| word_at(code, *at, name)).collect()
}

/// The raw egui widgets a squeezed view's `code` uses, each with where it starts: a method of `Ui` that adds one, or
/// one of their types (an accessibility role or a text style of the same name, `Role::ComboBox`, `TextStyle::Button`,
/// is not one).
fn raw_widgets(code: &str) -> Vec<(usize, String)> {
    const METHODS: [&str; 15] = [
        "button",
        "small_button",
        "checkbox",
        "radio",
        "radio_value",
        "selectable_label",
        "selectable_value",
        "toggle_value",
        "separator",
        "text_edit_singleline",
        "text_edit_multiline",
        "link",
        "hyperlink",
        "menu_button",
        "collapsing",
    ];
    const TYPES: [&str; 13] = [
        "ComboBox",
        "Button",
        "Checkbox",
        "SelectableLabel",
        "Separator",
        "RadioButton",
        "TextEdit",
        "Popup",
        "Hyperlink",
        "ImageButton",
        "Slider",
        "DragValue",
        "CollapsingHeader",
    ];
    let mut found = Vec::new();
    // egui's popups (the float is the one way to a menu): its free function as well as its type.
    for call in ["popup_below_widget(", "popup_above_or_below_widget("] {
        found.extend(code.match_indices(call).filter(|(at, _)| word_at(code, *at, call.trim_end_matches('('))).map(|(at, _)| (at, call.to_owned())));
    }
    for method in METHODS {
        for call in [format!(".{method}("), format!("Ui::{method}(")] {
            found.extend(code.match_indices(call.as_str()).map(|(at, _)| (at, call.clone())));
        }
    }
    for name in TYPES {
        found.extend(
            words(code, name)
                .into_iter()
                .filter(|at| !code[..*at].ends_with("Role::") && !code[..*at].ends_with("TextStyle::"))
                .map(|at| (at, name.to_owned())),
        );
    }
    found.sort();
    found
}

/// The end of the balanced brackets that open at `at` in `code`, or `at` when none opens there.
fn closing(code: &str, at: usize) -> usize {
    let bytes = code.as_bytes();
    let Some(open @ (b'(' | b'<')) = bytes.get(at).copied() else { return at };
    let close = if open == b'(' { b')' } else { b'>' };
    let mut depth = 0;
    for (index, byte) in bytes.iter().enumerate().skip(at) {
        if *byte == open {
            depth += 1;
        } else if *byte == close {
            depth -= 1;
            if depth == 0 {
                return index + 1;
            }
        }
    }
    bytes.len()
}

/// The methods called one after the other from `at` in `code`: `.a(…).b::<…>(…)` gives `a` and `b`.
fn chain(code: &str, mut at: usize) -> Vec<String> {
    let word = |c: char| c.is_alphanumeric() || c == '_';
    let mut called = Vec::new();
    at = closing(code, at);
    while code[at..].starts_with('.') {
        let name: String = code[at + 1..].chars().take_while(|c| word(*c)).collect();
        if name.is_empty() {
            break;
        }
        at += 1 + name.len();
        if code[at..].starts_with("::") {
            at = closing(code, at + 2);
        }
        at = closing(code, at);
        called.push(name);
    }
    called
}

/// The scroll areas a squeezed file's `code` shows itself instead of through `component::scroll`, each with where it
/// starts: one built and shown in one chain, one kept in a binding or a parameter and shown from there, and a
/// renamed import, which the scan could not follow.
fn bare_scroll_areas(code: &str) -> Vec<(usize, String)> {
    const SHOWN: [&str; 3] = ["show", "show_rows", "show_viewport"];
    let word = |c: char| c.is_alphanumeric() || c == '_';
    let mut found = Vec::new();
    for at in words(code, "ScrollArea") {
        let rest = &code[at + "ScrollArea".len()..];
        if rest.starts_with(" as ") {
            found.push((at, "ScrollArea as".to_owned()));
        }
        if let Some(built) = rest.strip_prefix("::") {
            let name: String = built.chars().take_while(|c| word(*c)).collect();
            let after = at + "ScrollArea::".len() + name.len();
            if chain(code, after).iter().any(|called| SHOWN.contains(&called.as_str())) {
                found.push((at, format!("ScrollArea::{name}(…).show")));
            }
        }
        // `let area = ScrollArea::…`, `area: ScrollArea`, `area: egui::ScrollArea`: the name it is kept under.
        let mut lead = &code[..at];
        while let Some(path) = lead.strip_suffix("::") {
            lead = path.trim_end_matches(word);
        }
        let kept = if lead.ends_with('=') && !lead.ends_with("==") {
            Some(lead.trim_end_matches('='))
        } else if lead.ends_with(':') && !lead.ends_with("::") {
            Some(lead.trim_end_matches(':'))
        } else {
            None
        };
        let Some(kept) = kept else { continue };
        let name: String = kept.chars().rev().take_while(|c| word(*c)).collect::<Vec<_>>().into_iter().rev().collect();
        if name.is_empty() {
            continue;
        }
        for shown in SHOWN {
            let call = format!("{name}.{shown}(");
            found.extend(code.match_indices(call.as_str()).filter(|(at, _)| word_at(code, *at, &name)).map(|(at, _)| (at, call.clone())));
        }
    }
    found.sort();
    found
}

#[test]
fn the_squeeze_drops_comments_literals_and_the_room_between_words() {
    let (code, lines) =
        squeezed("ui\n  . button (\"Dismiss // not a comment\") // ui.checkbox(\nlet mut a = 'x'; /* ComboBox */ r#\"ui.separator()\"#;");
    assert_eq!(code, "ui.button(\"\")let mut a='';\"\";");
    assert_eq!(lines.first(), Some(&1));
    assert_eq!(lines.get(code.find(".button").unwrap()), Some(&2));
    assert_eq!(squeezed("fn f<'a>(x: &'a str) {}").0, "fn f<'a>(x:&'a str){}");
}

#[test]
fn the_widget_scan_sees_calls_and_types_however_they_are_written() {
    let scan = |text: &str| raw_widgets(&squeezed(text).0).into_iter().map(|(_, name)| name).collect::<Vec<_>>();
    assert_eq!(scan("if ui.button(tr!(\"Dismiss\")).clicked() {}"), [".button("]);
    assert_eq!(scan("ui\n    .checkbox(\n        &mut on,\n        name,\n    );"), [".checkbox("]);
    assert_eq!(scan("row . selectable_label ( on , text )"), [".selectable_label("]);
    assert_eq!(scan("egui::ComboBox::from_id_salt(\"x\")"), ["ComboBox"]);
    assert_eq!(scan("ui.add(egui::Button::new(\"x\")); Ui::separator(ui);"), ["Button", "Ui::separator("]);
    assert!(scan("TextButton::new(x).show(ui, theme, keymap); IconButton::new(i, n)").is_empty());
    assert!(scan("WidgetInfo::labeled(Role::ComboBox, true, name); egui::PointerButton::Primary").is_empty());
    assert!(scan("(TextStyle::Button, f.button.id())").is_empty());
    assert!(scan("theme.fonts.button.rich(text); // ui.button(\"x\")\nlet s = \"ui.separator()\";").is_empty());
    // Text: views write in the fields of `component::field`.
    assert_eq!(scan("ui.add(egui::TextEdit::singleline(&mut text).hint_text(hint));"), ["TextEdit"]);
    assert_eq!(
        scan("ui.text_edit_singleline(&mut text); ui . text_edit_multiline ( &mut more );"),
        [".text_edit_singleline(", ".text_edit_multiline("]
    );
    assert!(scan("if let Some(state) = egui::text_edit::TextEditState::load(ctx, id) {}").is_empty());
    // Menus: views open them through `component::popup`.
    assert_eq!(scan("egui::Popup::menu(&response).show(|ui| {});"), ["Popup"]);
    assert_eq!(scan("Popup::context_menu(&row).show(add); egui::popup_below_widget(ui, id, &r, close, add);"), ["Popup", "popup_below_widget("]);
    assert!(scan("popup::menu(ctx, theme, &response, id, width, add); PopupCloseBehavior::CloseOnClick;").is_empty());
}

#[test]
fn the_widget_scan_sees_links_menus_folds_and_value_widgets() {
    let scan = |text: &str| raw_widgets(&squeezed(text).0).into_iter().map(|(_, name)| name).collect::<Vec<_>>();
    assert_eq!(scan("ui.link(\"x\"); ui.hyperlink(url);"), [".link(", ".hyperlink("]);
    assert_eq!(scan("ui.menu_button(\"More\", |ui| {}); ui.collapsing(\"Log\", |ui| {});"), [".menu_button(", ".collapsing("]);
    assert_eq!(scan("ui.add(egui::Hyperlink::new(url)); ui.add(egui::ImageButton::new(image));"), ["Hyperlink", "ImageButton"]);
    assert_eq!(scan("ui.add(egui::Slider::new(&mut v, 0.0..=1.0)); ui.add(DragValue::new(&mut v));"), ["Slider", "DragValue"]);
    assert_eq!(scan("egui::CollapsingHeader::new(\"Server\").show(ui, add);"), ["CollapsingHeader"]);
    assert_eq!(scan("ui.radio(on, name); ui.toggle_value(&mut on, name);"), [".radio(", ".toggle_value("]);
    assert!(scan("button::link(ui, theme, keymap, name); WidgetInfo::slider(true, 1.0, name); Role::Slider").is_empty());
}

#[test]
fn the_scroll_scan_sees_an_area_shown_without_the_component() {
    let scan = |text: &str| bare_scroll_areas(&squeezed(text).0).into_iter().map(|(_, name)| name).collect::<Vec<_>>();
    assert_eq!(
        scan("ScrollArea::vertical().id_salt(\"a\")\n    .auto_shrink([false; 2])\n    .show(ui, |ui| {});"),
        ["ScrollArea::vertical(…).show"]
    );
    assert_eq!(scan("egui::ScrollArea::both().show_rows(ui, 20.0, 10, |ui, rows| {});"), ["ScrollArea::both(…).show"]);
    assert_eq!(scan("let mut area = ScrollArea::vertical();\narea = area.max_height(4.0);\narea . show(ui, add);"), ["area.show("]);
    assert_eq!(scan("fn f(ui: &mut Ui, area: egui::ScrollArea) { area.show_viewport(ui, add); }"), ["area.show_viewport("]);
    assert_eq!(scan("use egui::ScrollArea as Area;"), ["ScrollArea as"]);
    assert!(scan("let area = ScrollArea::vertical().max_height(4.0);\nscroll::show(ui, theme, area, |ui| {});").is_empty());
    assert!(scan("crate::component::scroll::show(ui, theme, ScrollArea::vertical().auto_shrink([false, false]), |ui| {});").is_empty());
}

#[test]
fn no_view_uses_a_raw_egui_widget() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    for dir in ["src", "examples"] {
        if root.join(dir).is_dir() {
            sources(&root.join(dir), &mut files);
        }
    }
    assert!(files.len() > 10, "the walk found the sources");
    let components = root.join("src").join("component");
    let scroll = components.join("scroll.rs");
    let mut found = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(file).unwrap();
        let (code, lines) = squeezed(&text);
        // Only the scroll bar's own code shows a scroll area; components may use egui's widgets, views may not.
        let mut sites = if *file == scroll { Vec::new() } else { bare_scroll_areas(&code) };
        if !file.starts_with(&components) {
            sites.extend(raw_widgets(&code));
        }
        for (at, what) in sites {
            let line = code[..at].chars().count();
            let line = lines.get(line).copied().unwrap_or(0);
            found.push((file.strip_prefix(root).unwrap().display().to_string(), line, what));
        }
    }
    found.sort();
    let found: Vec<String> = found.into_iter().map(|(file, line, what)| format!("{file}:{line}: {what}")).collect();
    assert!(found.is_empty(), "views use the components, and only component/scroll.rs shows a scroll area:\n{}", found.join("\n"));
}
