// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde_json::{Value, json};

use crate::appearance::{Appearance, ComposerHost, EventEditorHost, Layout, Motion, MotionLevel, ReaderHost, ThemeMode, WindowFrame};
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
    assert_eq!(Appearance::default().frame, WindowFrame::Own);
}

#[test]
fn frame_set_takes_system_or_own_reads_the_old_own_frames_as_own_and_refuses_anything_else() {
    let mut session = session();
    for (name, expected) in [
        ("system", WindowFrame::System),
        ("own", WindowFrame::Own),
        ("system", WindowFrame::System),
        ("top_row", WindowFrame::Own),
        ("system", WindowFrame::System),
        ("strip", WindowFrame::Own),
    ] {
        let answer = session.run_as(Caller::Agent, "ui.frame.set", &json!({ "frame": name })).unwrap();
        assert_eq!(answer["appearance"]["frame"], expected.id(), "{name}: the answer says the frame mapped");
        assert_eq!(session.appearance().frame, expected, "{name}");
    }
    for junk in [json!({}), json!({ "frame": "title" }), json!({ "frame": 1 }), json!({ "frame": "strip", "more": true }), json!({ "frame": "Own" })]
    {
        assert!(bad(session.run("ui.frame.set", &junk)), "{junk}");
    }
    assert_eq!(session.appearance().frame, WindowFrame::Own, "a refused set changes nothing");
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
    assert_eq!(appearance.frame, WindowFrame::Own);
}

#[test]
fn a_restored_appearance_is_what_the_commands_start_from() {
    let mut session = session();
    let restored = Appearance { theme: ThemeMode::Light, frame: WindowFrame::System, ..Appearance::default() };
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
fn settings_open_is_safe_and_asks_the_window_for_the_page_named() {
    use crate::shown::{Asked, SETTINGS_PAGES, SettingsAsked};
    let mut session = session();
    assert_eq!(session.registry().get("ui.settings.open").unwrap().class, Some(AgentClass::Safe));
    for page in SETTINGS_PAGES {
        let answer = session.run_as(Caller::Agent, "ui.settings.open", &json!({ "page": page })).unwrap();
        assert_eq!(answer, json!({ "page": page, "window": false }));
        assert_eq!(session.take_asked(), vec![Asked::Settings(SettingsAsked { page: Some(page), bare: false, by_agent: true })], "{page}");
    }
    for bare in [json!({}), json!(null), json!({ "page": null })] {
        assert_eq!(session.run_as(Caller::Agent, "ui.settings.open", &bare).unwrap(), json!({ "page": null, "window": false }), "{bare}");
        assert_eq!(session.take_asked(), vec![Asked::Settings(SettingsAsked { page: None, bare: true, by_agent: true })], "{bare}: as the key");
    }
    session.run_as(Caller::Owner, "ui.settings.open", &json!({ "page": "about" })).unwrap();
    assert_eq!(session.take_asked(), vec![Asked::Settings(SettingsAsked { page: Some("about"), bare: false, by_agent: false })], "the owner's own");
    assert!(session.take_asked().is_empty(), "taken once");
}

#[test]
fn settings_open_refuses_other_parameters_in_its_own_words() {
    let mut session = session();
    let hostile = "\u{202e}\r\n\u{0}evil";
    for params in [json!({ hostile: 1 }), json!({ "page": "accounts", hostile: true }), json!([hostile]), json!(hostile), json!(1)] {
        let error = session.run_as(Caller::Agent, "ui.settings.open", &params).unwrap_err();
        assert!(matches!(error, EngineError::BadParams { .. }), "{params}: {error:?}");
        assert!(!error.to_string().contains("evil"), "the junk is not echoed: {error}");
    }
    assert!(session.take_asked().is_empty(), "a refused call asks for nothing");
}

#[test]
fn the_window_s_asks_are_kept_up_to_a_few_while_nobody_takes_them() {
    use crate::shown::MAX_ASKED;
    let mut session = session();
    for _ in 0..MAX_ASKED * 3 {
        session.run("ui.settings.open", &json!({ "page": "about" })).unwrap();
    }
    session.run("ui.settings.open", &json!({ "page": "accounts" })).unwrap();
    let asked = session.take_asked();
    assert_eq!(asked.len(), MAX_ASKED);
    let page = |asked: &crate::shown::Asked| match asked {
        crate::shown::Asked::Settings(asked) => asked.page,
        crate::shown::Asked::Home(_) => None,
    };
    assert_eq!(asked.last().and_then(page), Some("accounts"), "the newest kept");
}

#[test]
fn settings_open_says_whether_a_window_takes_the_ask() {
    let mut session = session();
    let headless = session.run_as(Caller::Agent, "ui.settings.open", &json!({ "page": "accounts" })).unwrap();
    assert_eq!(headless["window"], json!(false), "on a store nothing is shown, and the answer says so");
    session.set_window(true);
    let shown = session.run_as(Caller::Agent, "ui.settings.open", &json!({ "page": "accounts" })).unwrap();
    assert_eq!(shown, json!({ "page": "accounts", "window": true }));
}

#[test]
fn home_open_is_safe_and_asks_the_window_for_home_by_whoever_ran_it() {
    use crate::shown::{Asked, HomeAsked};
    let mut session = session();
    assert_eq!(session.registry().get("ui.home.open").unwrap().class, Some(AgentClass::Safe));
    for params in [json!({}), json!(null)] {
        assert_eq!(session.run_as(Caller::Agent, "ui.home.open", &params).unwrap(), json!({ "window": false }), "{params}");
        assert_eq!(session.take_asked(), vec![Asked::Home(HomeAsked { by_agent: true })], "{params}");
    }
    session.run_as(Caller::Owner, "ui.home.open", &json!({})).unwrap();
    assert_eq!(session.take_asked(), vec![Asked::Home(HomeAsked { by_agent: false })], "the owner's own");
    assert!(session.take_asked().is_empty(), "taken once");
}

#[test]
fn home_open_says_whether_a_window_takes_the_ask() {
    let mut session = session();
    assert_eq!(session.run_as(Caller::Agent, "ui.home.open", &json!({})).unwrap(), json!({ "window": false }), "on a store nothing is shown");
    session.set_window(true);
    assert_eq!(session.run_as(Caller::Agent, "ui.home.open", &json!({})).unwrap(), json!({ "window": true }));
}

#[test]
fn home_open_refuses_every_parameter_in_its_own_words() {
    let mut session = session();
    let hostile = "\u{202e}\r\n\u{0}evil";
    for params in [json!({ hostile: 1 }), json!({ "page": "accounts" }), json!([hostile]), json!(hostile), json!(1)] {
        let error = session.run_as(Caller::Agent, "ui.home.open", &params).unwrap_err();
        assert!(matches!(error, EngineError::BadParams { .. }), "{params}: {error:?}");
        assert!(!error.to_string().contains("evil"), "the junk is not echoed: {error}");
    }
    assert!(session.take_asked().is_empty(), "a refused call asks for nothing");
}

#[test]
fn the_window_s_asks_keep_the_order_the_commands_ran_in() {
    use crate::shown::{Asked, HomeAsked, SettingsAsked};
    let mut session = session();
    session.run_as(Caller::Agent, "ui.home.open", &json!({})).unwrap();
    session.run_as(Caller::Agent, "ui.settings.open", &json!({ "page": "about" })).unwrap();
    session.run_as(Caller::Owner, "ui.home.open", &json!({})).unwrap();
    let settings = Asked::Settings(SettingsAsked { page: Some("about"), bare: false, by_agent: true });
    assert_eq!(session.take_asked(), vec![Asked::Home(HomeAsked { by_agent: true }), settings, Asked::Home(HomeAsked { by_agent: false })]);
}

#[test]
fn layout_set_is_the_owners_and_an_agent_is_refused_without_a_change() {
    let mut session = session();
    assert_eq!(session.registry().get("ui.layout.set").unwrap().class, Some(AgentClass::Owner));
    let refused = session.run_as(Caller::Agent, "ui.layout.set", &json!({ "composer": "window" })).unwrap_err();
    assert_eq!(refused.code(), "not_allowed");
    assert_eq!(session.appearance().layout, Layout::default(), "a refused set changes nothing");
}

#[test]
fn layout_set_changes_the_part_named_and_answers_the_whole_appearance() {
    let mut session = session();
    let answer = session.run("ui.layout.set", &json!({ "composer": "floating" })).unwrap();
    assert_eq!(answer["appearance"]["layout"], json!({ "reader": "docked", "composer": "floating", "event_editor": "full" }));
    assert_eq!(answer["appearance"]["theme"], "system", "the answer is the whole appearance");
    let answer = session.run("ui.layout.set", &json!({ "reader": "window" })).unwrap();
    assert_eq!(answer["appearance"]["layout"], json!({ "reader": "window", "composer": "floating", "event_editor": "full" }), "the composer stays");
    assert_eq!(
        session.appearance().layout,
        Layout { reader: ReaderHost::Window, composer: ComposerHost::Floating, event_editor: EventEditorHost::Full }
    );
    session.run("ui.layout.set", &json!({ "reader": "docked", "composer": "window" })).unwrap();
    assert_eq!(
        session.appearance().layout,
        Layout { reader: ReaderHost::Docked, composer: ComposerHost::Window, event_editor: EventEditorHost::Full }
    );
    assert_eq!(session.appearance().theme, ThemeMode::System, "the other choices stay");
}

#[test]
fn layout_set_opens_the_event_editor_small_full_or_in_a_window_and_leaves_the_rest() {
    let mut session = session();
    assert_eq!(session.appearance().layout.event_editor, EventEditorHost::Full, "a new event opens full by default");
    session.run("ui.layout.set", &json!({ "composer": "floating" })).unwrap();
    for (id, host) in [("small", EventEditorHost::Small), ("window", EventEditorHost::Window), ("full", EventEditorHost::Full)] {
        let answer = session.run("ui.layout.set", &json!({ "event_editor": id })).unwrap();
        assert_eq!(answer["appearance"]["layout"]["event_editor"], id);
        assert_eq!(session.appearance().layout, Layout { reader: ReaderHost::Docked, composer: ComposerHost::Floating, event_editor: host });
    }
    let refused = session.run_as(Caller::Agent, "ui.layout.set", &json!({ "event_editor": "window" })).unwrap_err();
    assert_eq!(refused.code(), "not_allowed");
    assert_eq!(session.appearance().layout.event_editor, EventEditorHost::Full, "an agent's set changes nothing");
}

#[test]
fn layout_set_refuses_anything_but_a_host_of_its_part() {
    let mut session = session();
    session.run("ui.layout.set", &json!({ "reader": "window", "composer": "floating" })).unwrap();
    let before = *session.appearance();
    for junk in [
        json!(null),
        json!({}),
        json!({ "reader": null }),
        json!({ "reader": "floating" }),
        json!({ "reader": "Window" }),
        json!({ "composer": "tab" }),
        json!({ "composer": 1 }),
        json!({ "composer": "window", "reader": "tab" }),
        json!({ "composer": "window", "more": true }),
        json!({ "event_editor": "float" }),
        json!({ "event_editor": "Full" }),
        json!({ "event_editor": 2 }),
        json!({ "event_editor": null }),
        json!({ "event_editor": "small", "reader": "tab" }),
        json!({ "layout": { "composer": "window" } }),
        json!("window"),
    ] {
        assert!(bad(session.run("ui.layout.set", &junk)), "{junk}");
        assert_eq!(*session.appearance(), before, "{junk}: a refused set changes nothing");
    }
}
