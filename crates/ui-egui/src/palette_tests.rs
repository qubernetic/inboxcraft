// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{MAX_CHARS, matching, rows};
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
fn the_settings_nine_pages_follow_the_actions_in_the_settings_order() {
    let all = super::rows("");
    let pages: Vec<&str> = all.iter().filter_map(|row| row.page).collect();
    assert_eq!(pages, inboxcraft_engine::shown::SETTINGS_PAGES, "one row a page, in order");
    // Every action but the command line's bare key, which opens what the palette's key opens (one row for the two).
    let listed = actions().len() - 1;
    assert!(all.iter().all(|row| row.id != keymap::COMMAND_LINE));
    assert_eq!(all.len(), listed + 9);
    assert!(all.iter().skip(listed).all(|row| row.page.is_some() && row.id == keymap::SETTINGS), "after every action");
    assert!(all.iter().take(listed).all(|row| row.page.is_none()));
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

#[test]
fn hostile_text_matches_nothing() {
    for text in ["\0", "x\r\ny", "🦀", &"\u{202e}x".repeat(10_000)] {
        assert!(rows(text).is_empty(), "{text:?}");
    }
}
