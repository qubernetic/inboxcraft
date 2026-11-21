// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The all-day alert's choices: their names, their triggers, and the kept value read as outside data.

use serde_json::json;

use super::{AllDayAlert, KEY, from_value, to_value};

#[test]
fn the_default_is_nine_the_day_before() {
    assert_eq!(AllDayAlert::default(), AllDayAlert::DayBefore9);
}

#[test]
fn each_choice_is_its_hour_from_the_midnight_the_day_starts() {
    let spans: Vec<(i64, i64)> = AllDayAlert::ALL.iter().map(|choice| choice.span().map(|span| (span.days(), span.seconds())).unwrap()).collect();
    assert_eq!(spans, [(0, -15 * 3600), (0, -12 * 3600), (0, -6 * 3600), (0, 9 * 3600)]);
}

#[test]
fn every_choice_is_kept_and_read_back_by_its_name() {
    for choice in AllDayAlert::ALL {
        assert_eq!(AllDayAlert::from_name(choice.name()), Some(choice));
        assert_eq!(from_value(&to_value(choice)), choice);
    }
    let names: Vec<&str> = AllDayAlert::ALL.iter().map(|choice| choice.name()).collect();
    assert_eq!(names, ["day_before_9", "day_before_12", "day_before_18", "on_the_day_9"]);
}

#[test]
fn a_kept_file_without_the_choice_or_with_junk_reads_as_the_default() {
    let week_only = json!({ "first_day": "sunday", "first_week": "week_of_1_january" });
    assert_eq!(from_value(&week_only), AllDayAlert::DayBefore9, "a file kept before the setting came");
    for junk in
        [json!(null), json!([]), json!("day_before_18"), json!({ KEY: 18 }), json!({ KEY: "DAY_BEFORE_18" }), json!({ KEY: "day_before_18 " })]
    {
        assert_eq!(from_value(&junk), AllDayAlert::DayBefore9, "{junk}");
    }
    assert_eq!(from_value(&json!({ "first_day": "sunday", KEY: "day_before_18" })), AllDayAlert::DayBefore18);
}
