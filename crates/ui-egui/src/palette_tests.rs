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
    assert_eq!(ids("calendar"), vec![keymap::CALENDAR]);
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
