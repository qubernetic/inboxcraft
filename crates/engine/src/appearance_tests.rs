// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde_json::json;

use super::*;

#[test]
fn every_choice_has_a_stable_id_it_is_found_by() {
    for mode in ThemeMode::ALL {
        assert_eq!(ThemeMode::named(mode.id()), Some(mode));
    }
    for frame in WindowFrame::ALL {
        assert_eq!(WindowFrame::named(frame.id()), Some(frame));
    }
    for motion in Motion::ALL {
        assert_eq!(Motion::named(motion.id()), Some(motion));
    }
    assert_eq!(Motion::named("on"), None, "the stored ids are not the command's words");
    assert_eq!([Motion::Reduce, Motion::Full, Motion::System].map(Motion::id), ["reduce", "full", "system"]);
    assert_eq!(TEXT_SCALES, [90, 100, 110, 120, 130]);
}

#[test]
fn an_appearance_comes_back_whole_from_its_json() {
    let appearance =
        Appearance { theme: ThemeMode::Dark, text_scale: 130, motion: Motion::Reduce, frame: WindowFrame::TopRow, own_frame: WindowFrame::TopRow };
    let value = appearance.to_value();
    assert_eq!(value, json!({ "theme": "dark", "text_scale": 130, "motion": "reduce", "frame": "top_row", "own_frame": "top_row" }));
    assert_eq!(Appearance::from_value(&value), appearance);
}

#[test]
fn what_cannot_be_read_from_kept_json_falls_back_to_the_default_choice_by_choice() {
    assert_eq!(Appearance::from_value(&json!(null)), Appearance::default());
    assert_eq!(Appearance::from_value(&json!([1, 2])), Appearance::default());
    let kept = json!({
        "theme": "light",
        "text_scale": 95,
        "motion": "sideways",
        "frame": 3,
        "unknown": true,
    });
    assert_eq!(Appearance::from_value(&kept), Appearance { theme: ThemeMode::Light, ..Appearance::default() });
}

#[test]
fn an_appearance_kept_by_the_old_look_loads() {
    let kept = json!({
        "theme": "dark",
        "accent": { "custom": ["#112233", "#445566", "#778899", "#aabbcc"] },
        "font": "inter",
        "icons": "lucide",
        "text_scale": 120,
        "motion": "reduce",
        "frame": "strip",
    });
    let appearance = Appearance::from_value(&kept);
    assert_eq!(
        appearance,
        Appearance { theme: ThemeMode::Dark, text_scale: 120, motion: Motion::Reduce, frame: WindowFrame::Strip, own_frame: WindowFrame::Strip }
    );
    let value = appearance.to_value();
    let mut keys: Vec<&str> = value.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, ["frame", "motion", "own_frame", "text_scale", "theme"]);
}

#[test]
fn the_own_frame_last_on_is_kept_and_never_the_desktop_s() {
    let kept = Appearance::from_value(&json!({ "frame": "system", "own_frame": "top_row" }));
    assert_eq!((kept.frame, kept.own_frame), (WindowFrame::System, WindowFrame::TopRow));
    assert_eq!(Appearance::from_value(&kept.to_value()), kept);
    // An own frame on is the one kept, whatever the file says; the desktop's is never the own one.
    assert_eq!(Appearance::from_value(&json!({ "frame": "strip", "own_frame": "top_row" })).own_frame, WindowFrame::Strip);
    assert_eq!(Appearance::from_value(&json!({ "frame": "top_row" })).own_frame, WindowFrame::TopRow);
    for junk in [json!({ "frame": "system", "own_frame": "system" }), json!({ "frame": "system", "own_frame": 3 }), json!({ "frame": "system" })] {
        assert_eq!(Appearance::from_value(&junk).own_frame, WindowFrame::Strip, "{junk}");
    }
}
