// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde_json::json;

use super::*;

#[test]
fn every_choice_has_a_stable_id_it_is_found_by() {
    for mode in ThemeMode::ALL {
        assert_eq!(ThemeMode::named(mode.id()), Some(mode));
    }
    for font in Typeface::ALL {
        assert_eq!(Typeface::named(font.id()), Some(font));
    }
    for icons in Icons::ALL {
        assert_eq!(Icons::named(icons.id()), Some(icons));
    }
    for motion in Motion::ALL {
        assert_eq!(Motion::named(motion.id()), Some(motion));
    }
    assert_eq!(Motion::named("on"), None, "the stored ids are not the command's words");
    assert_eq!([Motion::Reduce, Motion::Full, Motion::System].map(Motion::id), ["reduce", "full", "system"]);
    assert_eq!(PRESETS, ["electric", "aurora", "citrus"]);
    assert_eq!(TEXT_SCALES, [90, 100, 110, 120, 130]);
}

#[test]
fn an_appearance_comes_back_whole_from_its_json() {
    let custom = CustomAccent { gradient: [Rgb(1, 2, 3), Rgb(4, 5, 6), Rgb(7, 8, 9)], accent: Rgb(0xff, 0xd6, 0x0a) };
    let appearance = Appearance {
        theme: ThemeMode::Dark,
        accent: Accent::Custom(custom),
        font: Typeface::Inter,
        icons: Icons::Lucide,
        text_scale: 130,
        motion: Motion::Reduce,
        frame: WindowFrame::TopRow,
    };
    let value = appearance.to_value();
    assert_eq!(
        value,
        json!({
            "theme": "dark",
            "accent": { "custom": ["#010203", "#040506", "#070809", "#ffd60a"] },
            "font": "inter",
            "icons": "lucide",
            "text_scale": 130,
            "motion": "reduce",
            "frame": "top_row",
        })
    );
    assert_eq!(Appearance::from_value(&value), appearance);
    let preset = Appearance { accent: Accent::Preset("aurora"), ..Appearance::default() };
    assert_eq!(Appearance::from_value(&preset.to_value()), preset);
}

#[test]
fn what_cannot_be_read_from_kept_json_falls_back_to_the_default_choice_by_choice() {
    assert_eq!(Appearance::from_value(&json!(null)), Appearance::default());
    assert_eq!(Appearance::from_value(&json!([1, 2])), Appearance::default());
    let kept = json!({
        "theme": "light",
        "accent": { "preset": "nope" },
        "font": 7,
        "icons": "lucide",
        "text_scale": 95,
        "motion": "sideways",
        "unknown": true,
    });
    assert_eq!(Appearance::from_value(&kept), Appearance { theme: ThemeMode::Light, icons: Icons::Lucide, ..Appearance::default() });
    // A kept custom accent that does not read is nudged on the way in too.
    let dim = json!({ "accent": { "custom": ["#000000", "#000000", "#000000", "#000000"] } });
    let Accent::Custom(custom) = Appearance::from_value(&dim).accent else { panic!("custom") };
    assert!(contrast(custom.accent, DARK_SURFACE) >= AA);
    assert_eq!(Appearance::from_value(&json!({ "accent": { "custom": ["#000000"] } })).accent, Accent::default());
    assert_eq!(Appearance::from_value(&json!({ "accent": "electric" })).accent, Accent::default());
}

#[test]
fn hex_colours_are_read_strictly_and_written_in_lower_case() {
    assert_eq!(Rgb::from_hex("#A1b2C3"), Some(Rgb(0xa1, 0xb2, 0xc3)));
    assert_eq!(Rgb(0xa1, 0xb2, 0xc3).hex(), "#a1b2c3");
    for junk in ["", "#", "a1b2c3", "#a1b2c", "#a1b2c3d", "#a1b2cg", "#+1b2c3", "#ééé", "##a1b2c"] {
        assert_eq!(Rgb::from_hex(junk), None, "{junk}");
    }
}

#[test]
fn contrast_is_the_wcag_ratio() {
    assert!((contrast(Rgb(0, 0, 0), Rgb(255, 255, 255)) - 21.0).abs() < 0.01);
    assert!((contrast(Rgb(9, 9, 9), Rgb(9, 9, 9)) - 1.0).abs() < 0.001);
    assert!((contrast(Rgb(0x77, 0x77, 0x77), Rgb(255, 255, 255)) - 4.48).abs() < 0.01);
}

#[test]
fn readable_keeps_an_accent_that_reads_and_only_lightens_one_that_does_not() {
    let yellow = Rgb(0xff, 0xd6, 0x0a);
    assert_eq!(readable(yellow), yellow);
    for colour in [Rgb(0, 0, 0), Rgb(0x3a, 0x1d, 0x8c), Rgb(0x80, 0, 0), Rgb(0, 0x30, 0), Rgb(0x1e, 0x23, 0x30)] {
        let nudged = readable(colour);
        assert!(contrast(nudged, DARK_SURFACE) >= AA, "{colour:?} -> {nudged:?}");
        assert!(luminance(nudged) > luminance(colour));
        // The least step that reads: one step darker does not.
        let (h, s, l) = hsl(nudged);
        assert!(contrast(from_hsl(h, s, l - 0.02), DARK_SURFACE) < AA, "{colour:?} -> {nudged:?} is not the least step");
    }
}

#[test]
fn hsl_goes_there_and_back() {
    for colour in [Rgb(0, 0, 0), Rgb(255, 255, 255), Rgb(0x3a, 0x1d, 0x8c), Rgb(0xff, 0x2d, 0x95), Rgb(0x12, 0xc0, 0x40), Rgb(0x80, 0x80, 0x20)] {
        let (h, s, l) = hsl(colour);
        assert_eq!(from_hsl(h, s, l), colour);
    }
    assert_eq!(from_hsl(f32::NAN, f32::INFINITY, -3.0), Rgb(0, 0, 0));
}
