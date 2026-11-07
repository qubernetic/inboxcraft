// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde_json::{Value, json};

use crate::appearance::{Accent, Appearance, Icons, Motion, Rgb, ThemeMode, Typeface, WindowFrame, contrast};
use crate::test_support::session;
use crate::{AgentClass, Caller, EngineError};

const COMMANDS: [&str; 7] = ["ui.theme.set", "ui.accent.set", "ui.font.set", "ui.icons.set", "ui.text_scale.set", "ui.motion.set", "ui.frame.set"];

fn bad(result: Result<Value, EngineError>) -> bool {
    matches!(result, Err(EngineError::BadParams { .. }))
}

#[test]
fn every_appearance_command_is_safe_so_an_agent_may_run_it() {
    let session = session();
    for id in COMMANDS {
        let spec = session.registry().get(id).unwrap();
        assert_eq!(spec.class, Some(AgentClass::Safe), "{id}");
    }
}

#[test]
fn a_new_session_has_the_default_appearance() {
    let session = session();
    assert_eq!(*session.appearance(), Appearance::default());
    assert_eq!(Appearance::default().theme, ThemeMode::System);
    assert_eq!(Appearance::default().accent, Accent::Preset("electric"));
    assert_eq!(Appearance::default().font, Typeface::Figtree);
    assert_eq!(Appearance::default().icons, Icons::Phosphor);
    assert_eq!(Appearance::default().text_scale, 100);
    assert_eq!(Appearance::default().motion, Motion::System);
    assert_eq!(Appearance::default().frame, WindowFrame::Strip);
}

#[test]
fn frame_set_takes_system_top_row_or_strip_and_refuses_anything_else() {
    let mut session = session();
    for (name, expected) in [("system", WindowFrame::System), ("top_row", WindowFrame::TopRow), ("strip", WindowFrame::Strip)] {
        let answer = session.run_as(Caller::Agent, "ui.frame.set", &json!({ "frame": name })).unwrap();
        assert_eq!(answer["appearance"]["frame"], name);
        assert_eq!(session.appearance().frame, expected);
    }
    for junk in [json!({}), json!({ "frame": "title" }), json!({ "frame": 1 }), json!({ "frame": "strip", "more": true })] {
        assert!(bad(session.run("ui.frame.set", &junk)), "{junk}");
    }
    let kept = Appearance::from_value(&json!({ "frame": "top_row" }));
    assert_eq!(kept.frame, WindowFrame::TopRow);
    assert_eq!(Appearance::from_value(&json!({ "frame": "window" })).frame, WindowFrame::Strip);
}

#[test]
fn theme_set_takes_system_light_or_dark_and_answers_the_whole_appearance() {
    let mut session = session();
    for (mode, expected) in [("dark", ThemeMode::Dark), ("light", ThemeMode::Light), ("system", ThemeMode::System)] {
        let answer = session.run_as(Caller::Agent, "ui.theme.set", &json!({ "mode": mode })).unwrap();
        assert_eq!(session.appearance().theme, expected);
        assert_eq!(answer["appearance"], session.appearance().to_value());
        assert_eq!(answer["appearance"]["theme"], json!(mode));
    }
    for junk in [json!({ "mode": "Dark" }), json!({ "mode": "sepia" }), json!({}), json!({ "mode": 1 }), json!({ "mode": "dark", "x": 1 })] {
        assert!(bad(session.run("ui.theme.set", &junk)), "{junk}");
    }
    assert_eq!(session.appearance().theme, ThemeMode::System, "a refused command changes nothing");
}

#[test]
fn accent_set_takes_a_shipped_preset() {
    let mut session = session();
    let answer = session.run("ui.accent.set", &json!({ "preset": "citrus" })).unwrap();
    assert_eq!(session.appearance().accent, Accent::Preset("citrus"));
    assert_eq!(answer["appearance"]["accent"], json!({ "preset": "citrus" }));
    assert_eq!(answer["nudged"], json!(false));
    for junk in [json!({ "preset": "sunset" }), json!({ "preset": "Citrus" }), json!({}), json!({ "preset": "aurora", "custom": ["#000000"] })] {
        assert!(bad(session.run("ui.accent.set", &junk)), "{junk}");
    }
    assert_eq!(session.appearance().accent, Accent::Preset("citrus"));
}

#[test]
fn accent_set_takes_four_custom_colours_that_already_read() {
    let mut session = session();
    let custom = json!(["#2F6BFF", "#8a3ffc", "#ff2d95", "#ffd60a"]);
    let answer = session.run("ui.accent.set", &json!({ "custom": custom })).unwrap();
    let Accent::Custom(chosen) = session.appearance().accent else { panic!("custom") };
    assert_eq!(chosen.gradient, [Rgb(0x2f, 0x6b, 0xff), Rgb(0x8a, 0x3f, 0xfc), Rgb(0xff, 0x2d, 0x95)]);
    assert_eq!(chosen.accent, Rgb(0xff, 0xd6, 0x0a));
    assert_eq!(answer["appearance"]["accent"], json!({ "custom": ["#2f6bff", "#8a3ffc", "#ff2d95", "#ffd60a"] }));
    assert_eq!(answer["nudged"], json!(false));
    assert!(answer["contrast"].as_f64().unwrap() >= 4.5);
}

#[test]
fn a_custom_accent_that_misses_wcag_aa_is_nudged_in_lightness_and_the_answer_says_so() {
    let mut session = session();
    let answer = session.run("ui.accent.set", &json!({ "custom": ["#2f6bff", "#8a3ffc", "#ff2d95", "#3a1d8c"] })).unwrap();
    let Accent::Custom(chosen) = session.appearance().accent else { panic!("custom") };
    assert_ne!(chosen.accent, Rgb(0x3a, 0x1d, 0x8c));
    assert!(contrast(chosen.accent, crate::appearance::DARK_SURFACE) >= 4.5);
    assert_eq!(answer["nudged"], json!(true));
    assert_eq!(answer["requested"], json!("#3a1d8c"));
    assert_eq!(answer["appearance"]["accent"]["custom"][3], json!(chosen.accent.hex()));
    assert!(answer["contrast"].as_f64().unwrap() >= 4.5);
    assert!(answer["reason"].as_str().unwrap().contains("WCAG AA"));
    // Only lighter: the hue stays violet, so blue is still the strongest channel and green the weakest.
    let Rgb(r, g, b) = chosen.accent;
    assert!(b > r && r > g, "{r} {g} {b}");
    // The gradient is kept as it was.
    assert_eq!(chosen.gradient[0], Rgb(0x2f, 0x6b, 0xff));
}

#[test]
fn a_black_accent_is_nudged_to_a_grey_that_reads() {
    let mut session = session();
    let answer = session.run("ui.accent.set", &json!({ "custom": ["#000000", "#000000", "#000000", "#000000"] })).unwrap();
    assert_eq!(answer["nudged"], json!(true));
    let Accent::Custom(chosen) = session.appearance().accent else { panic!("custom") };
    assert!(contrast(chosen.accent, crate::appearance::DARK_SURFACE) >= 4.5);
}

#[test]
fn accent_set_refuses_custom_colours_that_are_not_four_hex_colours() {
    let mut session = session();
    for junk in [
        json!({ "custom": [] }),
        json!({ "custom": ["#000000", "#000000", "#000000"] }),
        json!({ "custom": ["#000000", "#000000", "#000000", "#000000", "#000000"] }),
        json!({ "custom": ["#000000", "#000000", "#000000", "000000"] }),
        json!({ "custom": ["#000000", "#000000", "#000000", "#00000g"] }),
        json!({ "custom": ["#000000", "#000000", "#000000", "#000"] }),
        json!({ "custom": ["#000000", "#000000", "#000000", "#ééé"] }),
        json!({ "custom": ["#000000", "#000000", "#000000", 0] }),
        json!({ "custom": "#000000" }),
    ] {
        assert!(bad(session.run("ui.accent.set", &junk)), "{junk}");
    }
    assert_eq!(session.appearance().accent, Accent::Preset("electric"));
}

#[test]
fn font_set_takes_figtree_or_inter() {
    let mut session = session();
    let answer = session.run_as(Caller::Agent, "ui.font.set", &json!({ "font": "inter" })).unwrap();
    assert_eq!(session.appearance().font, Typeface::Inter);
    assert_eq!(answer["appearance"]["font"], json!("inter"));
    session.run("ui.font.set", &json!({ "font": "figtree" })).unwrap();
    assert_eq!(session.appearance().font, Typeface::Figtree);
    for junk in [json!({ "font": "Comic Sans" }), json!({ "font": "" }), json!({})] {
        assert!(bad(session.run("ui.font.set", &junk)), "{junk}");
    }
}

#[test]
fn icons_set_takes_phosphor_or_lucide() {
    let mut session = session();
    let answer = session.run_as(Caller::Agent, "ui.icons.set", &json!({ "icons": "lucide" })).unwrap();
    assert_eq!(session.appearance().icons, Icons::Lucide);
    assert_eq!(answer["appearance"]["icons"], json!("lucide"));
    session.run("ui.icons.set", &json!({ "icons": "phosphor" })).unwrap();
    assert_eq!(session.appearance().icons, Icons::Phosphor);
    for junk in [json!({ "icons": "tabler" }), json!({ "icons": null }), json!({})] {
        assert!(bad(session.run("ui.icons.set", &junk)), "{junk}");
    }
}

#[test]
fn text_scale_set_takes_90_to_130_per_cent_in_steps_of_ten() {
    let mut session = session();
    for percent in [90, 110, 120, 130, 100] {
        let answer = session.run_as(Caller::Agent, "ui.text_scale.set", &json!({ "percent": percent })).unwrap();
        assert_eq!(session.appearance().text_scale, percent);
        assert_eq!(answer["appearance"]["text_scale"], json!(percent));
    }
    for junk in [
        json!({ "percent": 95 }),
        json!({ "percent": 80 }),
        json!({ "percent": 140 }),
        json!({ "percent": -100 }),
        json!({ "percent": 1.1 }),
        json!({ "percent": "110" }),
    ] {
        assert!(bad(session.run("ui.text_scale.set", &junk)), "{junk}");
    }
    assert_eq!(session.appearance().text_scale, 100);
}

#[test]
fn motion_set_reduces_motion_lets_it_run_or_follows_the_system() {
    let mut session = session();
    for (reduce, expected) in [("on", Motion::Reduce), ("off", Motion::Full), ("system", Motion::System)] {
        let answer = session.run_as(Caller::Agent, "ui.motion.set", &json!({ "reduce": reduce })).unwrap();
        assert_eq!(session.appearance().motion, expected);
        assert_eq!(answer["appearance"]["motion"], json!(expected.id()));
    }
    for junk in [json!({ "reduce": true }), json!({ "reduce": "yes" }), json!({})] {
        assert!(bad(session.run("ui.motion.set", &junk)), "{junk}");
    }
}

#[test]
fn one_command_changes_only_its_own_choice() {
    let mut session = session();
    session.run("ui.theme.set", &json!({ "mode": "dark" })).unwrap();
    session.run("ui.font.set", &json!({ "font": "inter" })).unwrap();
    session.run("ui.text_scale.set", &json!({ "percent": 120 })).unwrap();
    let appearance = *session.appearance();
    assert_eq!(appearance.theme, ThemeMode::Dark);
    assert_eq!(appearance.font, Typeface::Inter);
    assert_eq!(appearance.text_scale, 120);
    assert_eq!(appearance.icons, Icons::Phosphor);
}

#[test]
fn a_restored_appearance_is_what_the_commands_start_from() {
    let mut session = session();
    let restored = Appearance { theme: ThemeMode::Light, icons: Icons::Lucide, ..Appearance::default() };
    session.set_appearance(restored);
    session.run("ui.font.set", &json!({ "font": "inter" })).unwrap();
    assert_eq!(*session.appearance(), Appearance { font: Typeface::Inter, ..restored });
}
