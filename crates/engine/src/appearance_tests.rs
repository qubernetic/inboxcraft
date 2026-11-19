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
        Appearance { theme: ThemeMode::Dark, text_scale: 130, motion: Motion::Reduce, motion_reduced: false, frame: WindowFrame::System };
    let value = appearance.to_value();
    assert_eq!(
        value,
        json!({
            "theme": "dark",
            "text_scale": 130,
            "motion": "reduce",
            "motion_level": "off",
            "motion_chosen": true,
            "frame": "system",
        })
    );
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
        Appearance { theme: ThemeMode::Dark, text_scale: 120, motion: Motion::Reduce, motion_reduced: false, frame: WindowFrame::Own }
    );
    let value = appearance.to_value();
    let mut keys: Vec<&str> = value.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, ["frame", "motion", "motion_chosen", "motion_level", "text_scale", "theme"]);
}

#[test]
fn a_kept_frame_of_an_older_version_reads_as_the_own_frame_and_junk_as_the_default() {
    assert_eq!(WindowFrame::ALL.map(WindowFrame::id), ["system", "own"]);
    assert_eq!(WindowFrame::default(), WindowFrame::Own);
    for (kept, frame) in [("top_row", WindowFrame::Own), ("strip", WindowFrame::Own), ("own", WindowFrame::Own), ("system", WindowFrame::System)] {
        let appearance = Appearance::from_value(&json!({ "theme": "light", "text_scale": 110, "frame": kept, "own_frame": "top_row" }));
        assert_eq!((appearance.theme, appearance.text_scale, appearance.frame), (ThemeMode::Light, 110, frame), "{kept}");
        // Written back, it says the frame by this version's name, and the old field is gone.
        assert_eq!(appearance.to_value()["frame"], frame.id(), "{kept}");
        assert!(appearance.to_value().get("own_frame").is_none(), "{kept}");
    }
    for junk in [json!(3), json!(null), json!("Strip"), json!("top_row "), json!("x".repeat(10_000)), json!(["own"])] {
        let appearance = Appearance::from_value(&json!({ "theme": "light", "frame": junk }));
        assert_eq!((appearance.theme, appearance.frame), (ThemeMode::Light, WindowFrame::Own), "{junk}");
    }
}

/// The level and whether the owner chose it, as a kept `appearance.json` with `motion` = `motion` gives them.
fn kept_motion(motion: &serde_json::Value) -> (MotionLevel, bool) {
    let appearance = Appearance::from_value(&json!({ "theme": "dark", "text_scale": 120, "motion": motion, "frame": "top_row" }));
    assert_eq!((appearance.theme, appearance.text_scale, appearance.frame), (ThemeMode::Dark, 120, WindowFrame::Own), "{motion}");
    (appearance.motion_level(), appearance.motion_chosen())
}

#[test]
fn the_motion_levels_have_stable_ids_and_reduced_is_the_default() {
    assert_eq!(MotionLevel::ALL.map(MotionLevel::id), ["full", "reduced", "off"]);
    for level in MotionLevel::ALL {
        assert_eq!(MotionLevel::named(level.id()), Some(level));
    }
    assert_eq!(MotionLevel::default(), MotionLevel::Reduced);
    let fresh = Appearance::default();
    assert_eq!((fresh.motion_level(), fresh.motion_chosen()), (MotionLevel::Reduced, false));
}

#[test]
fn a_kept_motion_from_before_the_levels_maps_to_a_level() {
    assert_eq!(kept_motion(&json!("system")), (MotionLevel::Reduced, false), "the system's: the default, not chosen");
    assert_eq!(kept_motion(&json!("reduce")), (MotionLevel::Off, true));
    assert_eq!(kept_motion(&json!("full")), (MotionLevel::Full, true));
}

#[test]
fn a_kept_motion_named_by_its_level_reads_as_itself() {
    assert_eq!(kept_motion(&json!("reduced")), (MotionLevel::Reduced, true));
    assert_eq!(kept_motion(&json!("off")), (MotionLevel::Off, true));
}

#[test]
fn a_kept_motion_that_cannot_be_read_is_the_default_and_the_rest_is_kept() {
    for junk in [json!(3), json!(null), json!("x".repeat(10_000)), json!(["reduce"]), json!({ "level": "off" }), json!("Reduce")] {
        assert_eq!(kept_motion(&junk), (MotionLevel::Reduced, false), "{junk}");
    }
    let missing = Appearance::from_value(&json!({ "theme": "dark", "text_scale": 120, "frame": "top_row" }));
    assert_eq!((missing.theme, missing.text_scale, missing.frame), (ThemeMode::Dark, 120, WindowFrame::Own));
    assert_eq!((missing.motion_level(), missing.motion_chosen()), (MotionLevel::Reduced, false));
}

#[test]
fn every_motion_this_version_keeps_comes_back_the_same() {
    for (level, chosen) in [(MotionLevel::Full, true), (MotionLevel::Reduced, true), (MotionLevel::Off, true), (MotionLevel::Reduced, false)] {
        let appearance = Appearance { theme: ThemeMode::Light, text_scale: 110, ..Appearance::default() }.with_motion(level, chosen);
        assert_eq!((appearance.motion_level(), appearance.motion_chosen()), (level, chosen));
        let value = appearance.to_value();
        assert_eq!(value["motion_level"], json!(level.id()), "{value}");
        assert_eq!(value["motion_chosen"], json!(chosen), "{value}");
        assert_eq!(Appearance::from_value(&value), appearance, "{value}");
    }
}

#[test]
fn the_kept_level_wins_over_the_old_field_and_a_damaged_one_falls_back_to_it() {
    let both = json!({ "motion": "reduce", "motion_level": "reduced", "motion_chosen": true });
    assert_eq!(Appearance::from_value(&both).motion_level(), MotionLevel::Reduced);
    let damaged = json!({ "motion": "full", "motion_level": 7, "motion_chosen": true });
    assert_eq!(Appearance::from_value(&damaged).motion_level(), MotionLevel::Full);
    // A level without its flag, or with a flag that is not one, was chosen; a level not chosen is the default.
    for chosen in [json!(null), json!("no"), json!(0)] {
        let kept = Appearance::from_value(&json!({ "motion_level": "off", "motion_chosen": chosen }));
        assert_eq!((kept.motion_level(), kept.motion_chosen()), (MotionLevel::Off, true), "{chosen}");
    }
    let unchosen = Appearance::from_value(&json!({ "motion_level": "full", "motion_chosen": false }));
    assert_eq!((unchosen.motion_level(), unchosen.motion_chosen()), (MotionLevel::Reduced, false));
}

#[test]
fn the_old_motion_field_answers_as_before_for_each_level() {
    let old = |level, chosen| Appearance::default().with_motion(level, chosen).motion;
    assert_eq!(old(MotionLevel::Full, true), Motion::Full);
    assert_eq!(old(MotionLevel::Reduced, true), Motion::Reduce);
    assert_eq!(old(MotionLevel::Off, true), Motion::Reduce);
    assert_eq!(old(MotionLevel::Reduced, false), Motion::System);
    // A front end that still sets the old field alone gets the level the kept file maps it to.
    let reduce = Appearance { motion: Motion::Reduce, ..Appearance::default() };
    assert_eq!((reduce.motion_level(), reduce.motion_chosen()), (MotionLevel::Off, true));
}

#[test]
fn a_pair_that_disagrees_is_the_level_the_new_field_says_once_normalised() {
    // The old field decides only while the new one is at its default; a set flag is what `with_motion` wrote for a
    // chosen Reduced, and wins.
    let reduced = Appearance::default().with_motion(MotionLevel::Reduced, true);
    for motion in Motion::ALL {
        let literal = Appearance { motion, motion_reduced: true, ..Appearance::default() };
        assert_eq!((literal.motion_level(), literal.motion_chosen()), (MotionLevel::Reduced, true), "{motion:?}");
        assert_eq!(literal.normalised(), reduced, "{motion:?}");
        assert_eq!(Appearance::from_value(&literal.to_value()), reduced, "{motion:?}: the kept file says the same");
        assert_eq!(literal.to_value()["motion"], json!("reduce"), "{motion:?}: an old client reads the same level");
    }
    for motion in Motion::ALL {
        let literal = Appearance { motion, ..Appearance::default() };
        assert_eq!(literal.normalised(), literal, "{motion:?}: a pair with the flag at its default is already one");
    }
}

#[test]
fn the_system_s_wish_decides_only_while_the_owner_has_not_chosen() {
    use MotionLevel::{Full, Off, Reduced};
    for (level, chosen, system_reduces, expected) in [
        (Full, true, false, Full),
        (Full, true, true, Full),
        (Reduced, true, false, Reduced),
        (Reduced, true, true, Reduced),
        (Off, true, false, Off),
        (Off, true, true, Off),
        (Reduced, false, false, Reduced),
        (Reduced, false, true, Off),
        (Full, false, false, Full),
        (Full, false, true, Off),
        (Off, false, false, Off),
        (Off, false, true, Off),
    ] {
        assert_eq!(effective(level, chosen, system_reduces), expected, "{level:?} chosen {chosen} system reduces {system_reduces}");
    }
}
