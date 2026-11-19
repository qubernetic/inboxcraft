// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{MAX_CHARS, Palette, matching};
use crate::keymap::{self, actions};

fn ids(query: &str) -> Vec<&'static str> {
    matching(query).into_iter().map(|action| action.id).collect()
}

#[test]
fn an_empty_text_lists_every_action_and_words_narrow_it() {
    assert_eq!(matching("").len(), actions().len());
    assert_eq!(matching("   ").len(), actions().len());
    assert_eq!(ids("show the calendar"), vec![keymap::CALENDAR]);
    assert_eq!(ids("SHOW CAL"), vec![keymap::CALENDAR], "case and order of words left aside");
    assert_eq!(ids("palette.open"), vec![keymap::PALETTE], "the id counts too");
    assert!(ids("zzz nothing").is_empty());
    // Names that start with the text come first.
    let mode = ids("mode");
    assert_eq!(mode.first(), Some(&keymap::SYSTEM_MODE));
    assert!(mode.contains(&keymap::LIGHT) && mode.contains(&keymap::DARK));
}

#[test]
fn the_pick_moves_round_and_starts_again_when_the_text_changes() {
    let mut palette = Palette::default();
    assert!(!palette.is_open());
    palette.open();
    assert!(palette.is_open());
    assert_eq!(palette.chosen().map(|action| action.id), Some(keymap::PALETTE));
    let count = palette.matches().len();
    palette.step(false, count);
    assert_eq!(palette.picked(), count - 1, "up from the first is the last");
    palette.step(true, count);
    assert_eq!(palette.picked(), 0, "down from the last is the first");
    palette.step(true, count);
    assert_eq!(palette.picked(), 1);
    palette.step(true, 0);
    assert_eq!(palette.picked(), 0, "no rows, no pick");
    palette.step(true, count);
    palette.set_query("dark");
    assert_eq!(palette.picked(), 0);
    assert_eq!(palette.chosen().map(|action| action.id), Some(keymap::DARK));
    palette.query_mut().push_str(" nothing");
    palette.typed();
    assert_eq!(palette.chosen(), None);
    palette.close();
    assert!(!palette.is_open());
    assert_eq!(palette.query(), "");
}

#[test]
fn hostile_text_is_cut_and_matches_nothing() {
    let mut palette = Palette::default();
    palette.open();
    palette.set_query(&"\u{202e}x".repeat(10_000));
    assert_eq!(palette.query().chars().count(), MAX_CHARS);
    assert_eq!(palette.chosen(), None);
    palette.step(false, 3);
    assert_eq!(palette.picked(), 2);
    for text in ["\0", "x\r\ny", "🦀"] {
        palette.set_query(text);
        assert!(palette.matches().is_empty(), "{text:?}");
    }
}

#[test]
fn a_screen_s_own_actions_are_listed_on_that_screen_alone() {
    let mut palette = super::Palette::default();
    let calendar_keys = |palette: &super::Palette| palette.matches().iter().filter(|action| keymap::CALENDAR_KEYS.contains(&action.id)).count();
    palette.set_screen(keymap::Scope::Mail);
    palette.set_query("calendar");
    let ids = |palette: &super::Palette| palette.matches().iter().map(|row| row.page.unwrap_or(row.id)).collect::<Vec<_>>();
    assert_eq!(ids(&palette), vec![keymap::CALENDAR, "calendar"], "on the mail: the action and the settings' page");
    palette.set_screen(keymap::Scope::Calendar);
    assert_eq!(calendar_keys(&palette), keymap::CALENDAR_KEYS.len(), "on the calendar its six keys are listed");
    assert!(palette.matches().iter().any(|action| action.id == keymap::CALENDAR));
}

#[test]
fn the_settings_nine_pages_follow_the_actions_in_the_settings_order() {
    let all = super::rows("");
    let pages: Vec<&str> = all.iter().filter_map(|row| row.page).collect();
    assert_eq!(pages, inboxcraft_engine::shown::SETTINGS_PAGES, "one row a page, in order");
    assert_eq!(all.len(), actions().len() + 9);
    assert!(all.iter().skip(actions().len()).all(|row| row.page.is_some() && row.id == keymap::SETTINGS), "after every action");
    assert!(all.iter().take(actions().len()).all(|row| row.page.is_none()));
    assert_eq!(super::rows(""), all, "the same order every time");
    let found = |query: &str| super::rows(query).into_iter().map(|row| (row.page, row.name.into_owned())).collect::<Vec<_>>();
    assert_eq!(found("accounts"), vec![(Some("accounts"), "Settings: Accounts".to_owned())], "by the section's name");
    assert_eq!(found("SETTINGS: TRIAGE"), vec![(Some("triage"), "Settings: Triage".to_owned())]);
    let settings = found("settings");
    assert_eq!(settings.first().map(|row| row.1.as_str()), Some("Settings"), "the action first, its name starting with the text");
    assert_eq!(settings.iter().filter(|row| row.0.is_some()).count(), 9, "\"settings\" finds every page");
}

#[test]
fn a_long_hostile_text_is_matched_on_its_first_characters_only() {
    let head = format!("settings{}", " ".repeat(MAX_CHARS));
    let hostile = format!("{head}zzz{}", "\u{202e}x".repeat(10_000));
    assert_eq!(super::rows(&hostile), super::rows(&head), "nothing past the first {MAX_CHARS} characters is read");
    assert_eq!(super::rows(&hostile).iter().filter(|row| row.page.is_some()).count(), 9);
}
