// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde_json::{Value, json};

use crate::appearance::{Appearance, Motion, MotionLevel, ThemeMode, WindowFrame};
use crate::test_support::session;
use crate::{AgentClass, Caller, EngineError};

const COMMANDS: [&str; 4] = ["ui.theme.set", "ui.text_scale.set", "ui.motion.set", "ui.frame.set"];

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

/// The level and whether it was chosen after `ui.motion.set` with `params`, and the answer's level.
fn motion_after(session: &mut crate::Session, params: &Value) -> (MotionLevel, bool) {
    let answer = session.run_as(Caller::Agent, "ui.motion.set", params).unwrap();
    let appearance = *session.appearance();
    assert_eq!(answer["appearance"], appearance.to_value(), "{params}");
    assert_eq!(answer["appearance"]["motion_level"], json!(appearance.motion_level().id()), "{params}");
    (appearance.motion_level(), appearance.motion_chosen())
}

#[test]
fn motion_set_takes_a_level_the_owner_chose() {
    let mut session = session();
    for (level, expected) in [("off", MotionLevel::Off), ("full", MotionLevel::Full), ("reduced", MotionLevel::Reduced)] {
        assert_eq!(motion_after(&mut session, &json!({ "level": level })), (expected, true), "{level}");
    }
}

#[test]
fn motion_set_still_takes_the_old_reduce_for_one_release() {
    let mut session = session();
    assert_eq!(motion_after(&mut session, &json!({ "reduce": "on" })), (MotionLevel::Off, true));
    assert_eq!(motion_after(&mut session, &json!({ "reduce": "off" })), (MotionLevel::Full, true));
    assert_eq!(motion_after(&mut session, &json!({ "reduce": "system" })), (MotionLevel::Reduced, false), "the choice goes back to the computer");
    let answer = session.run("ui.motion.set", &json!({ "reduce": "on" })).unwrap();
    assert_eq!(answer["appearance"]["motion"], json!("reduce"), "an old client reads the old field");
    assert_eq!(session.appearance().motion, Motion::Reduce);
}

#[test]
fn motion_set_with_both_takes_the_level_and_refuses_junk_in_either() {
    let mut session = session();
    assert_eq!(motion_after(&mut session, &json!({ "level": "reduced", "reduce": "on" })), (MotionLevel::Reduced, true));
    assert_eq!(motion_after(&mut session, &json!({ "level": "full", "reduce": "system" })), (MotionLevel::Full, true));
    let before = *session.appearance();
    for junk in [
        json!({}),
        json!({ "level": null }),
        json!({ "level": "sideways", "reduce": "on" }),
        json!({ "level": "off", "reduce": "yes" }),
        json!({ "level": "Off" }),
        json!({ "level": 1 }),
        json!({ "level": "x".repeat(10_000) }),
        json!({ "reduce": true }),
        json!({ "reduce": "yes" }),
        json!({ "level": "off", "speed": 2 }),
    ] {
        assert!(bad(session.run("ui.motion.set", &junk)), "{junk}");
    }
    assert_eq!(*session.appearance(), before, "a refused command changes nothing");
}

#[test]
fn one_command_changes_only_its_own_choice() {
    let mut session = session();
    session.run("ui.theme.set", &json!({ "mode": "dark" })).unwrap();
    session.run("ui.motion.set", &json!({ "reduce": "on" })).unwrap();
    session.run("ui.text_scale.set", &json!({ "percent": 120 })).unwrap();
    let appearance = *session.appearance();
    assert_eq!(appearance.theme, ThemeMode::Dark);
    assert_eq!(appearance.motion, Motion::Reduce);
    assert_eq!(appearance.text_scale, 120);
    assert_eq!(appearance.frame, WindowFrame::Strip);
}

#[test]
fn a_restored_appearance_is_what_the_commands_start_from() {
    let mut session = session();
    let restored = Appearance { theme: ThemeMode::Light, frame: WindowFrame::TopRow, ..Appearance::default() };
    session.set_appearance(restored);
    session.run("ui.motion.set", &json!({ "reduce": "on" })).unwrap();
    assert_eq!(*session.appearance(), Appearance { motion: Motion::Reduce, ..restored });
}

#[test]
fn a_restored_appearance_whose_motion_fields_disagree_comes_in_as_one_level() {
    let mut session = session();
    let disagreeing = Appearance { motion: Motion::Full, motion_reduced: true, theme: ThemeMode::Dark, ..Appearance::default() };
    session.set_appearance(disagreeing);
    let restored = *session.appearance();
    assert_eq!(restored, Appearance { theme: ThemeMode::Dark, ..Appearance::default() }.with_motion(MotionLevel::Reduced, true));
    assert_eq!((restored.motion, restored.motion_reduced), (Motion::Reduce, true), "the old field agrees with the level");
    assert_eq!(restored, Appearance::from_value(&disagreeing.to_value()), "what a kept file of it gives");
    // The old field alone, flag at its default, keeps the meaning it always had.
    session.set_appearance(Appearance { motion: Motion::Reduce, ..Appearance::default() });
    assert_eq!((session.appearance().motion_level(), session.appearance().motion_chosen()), (MotionLevel::Off, true));
}

#[test]
fn the_removed_appearance_commands_are_unknown() {
    let mut session = session();
    for (id, params) in [
        ("ui.accent.set", json!({ "preset": "aurora" })),
        ("ui.font.set", json!({ "font": "inter" })),
        ("ui.icons.set", json!({ "icons": "lucide" })),
    ] {
        assert_eq!(session.run(id, &params).unwrap_err(), EngineError::UnknownCommand(id.into()), "{id}");
        assert!(session.registry().get(id).is_none(), "{id}");
    }
}

#[test]
fn the_desktop_s_frame_keeps_the_own_frame_last_on_to_come_back_to() {
    let mut session = session();
    assert_eq!(session.appearance().own_frame, WindowFrame::Strip, "the strip until another own frame is on");
    for own in [WindowFrame::Strip, WindowFrame::TopRow] {
        session.run("ui.frame.set", &json!({ "frame": own.id() })).unwrap();
        let answer = session.run("ui.frame.set", &json!({ "frame": "system" })).unwrap();
        assert_eq!(session.appearance().frame, WindowFrame::System);
        assert_eq!(session.appearance().own_frame, own, "the own frame left for the desktop's");
        assert_eq!(answer["appearance"]["own_frame"], own.id());
    }
}
