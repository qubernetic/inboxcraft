// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The repeat presets and the wall-clock texts.

use inboxcraft_ical::{End, Event, Frequency, Rule, When};
use inboxcraft_time::{Offset, Timestamp};
use serde_json::json;

use super::{Repeat, wall};

#[test]
fn every_preset_is_found_by_its_name_and_by_its_rule() {
    for repeat in Repeat::ALL {
        assert_eq!(Repeat::named(repeat.name()), Some(repeat));
        let mut event = Event::new("a@example.org", When::Utc(Timestamp::UNIX_EPOCH));
        event.rules.extend(repeat.rule());
        assert_eq!(Repeat::of(&event), Some(repeat));
    }
    assert_eq!(Repeat::named("fortnightly"), None);
}

#[test]
fn a_preset_that_ends_is_still_the_preset_and_anything_else_is_custom() {
    let mut event = Event::new("a@example.org", When::Utc(Timestamp::UNIX_EPOCH));
    event.rules.push(Rule { end: End::Count(3), ..Rule::new(Frequency::Daily) });
    assert_eq!(Repeat::of(&event), Some(Repeat::Daily));
    event.rules[0].interval = 2;
    assert_eq!(Repeat::of(&event), None);
    event.rules.clear();
    event.dates.push(When::Utc(Timestamp::UNIX_EPOCH));
    assert_eq!(Repeat::of(&event), None);
}

#[test]
fn wall_clock_text_carries_the_offset_west_of_utc_too() {
    let at = Timestamp::from_unix(1_793_869_200, 0).unwrap();
    assert_eq!(wall(at, &Offset::from_minutes(-330).unwrap()), json!("2026-11-05T03:30:00-05:30"));
    assert_eq!(wall(at, &Offset::UTC), json!("2026-11-05T09:00:00+00:00"));
}
