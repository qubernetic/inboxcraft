// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The week rule as a setting: kept as JSON and read back leniently, and `week.settings` / `week.set`.

use inboxcraft_time::{FirstWeek, Offset, WeekRule, Weekday};
use serde_json::{Value, json};

use super::{DAYS, FIRST_WEEKS, day_from_name, day_name, first_week_from_name, first_week_name, from_value, to_value};
use crate::Caller;
use crate::test_support::session;

#[test]
fn every_day_and_first_week_has_a_name_that_reads_back() {
    assert_eq!(Weekday::ALL.map(day_name), DAYS);
    for day in Weekday::ALL {
        assert_eq!(day_from_name(day_name(day)), Some(day));
    }
    for first in [FirstWeek::FirstFourDayWeek, FirstWeek::WeekOf1January] {
        assert_eq!(first_week_from_name(first_week_name(first)), Some(first));
    }
    assert_eq!([FirstWeek::FirstFourDayWeek, FirstWeek::WeekOf1January].map(first_week_name), FIRST_WEEKS);
    for junk in ["", "Monday", "mon", "sunday ", "first_four_day_week\0", "week_of_1_January"] {
        assert_eq!(day_from_name(junk), None, "{junk:?}");
        assert_eq!(first_week_from_name(junk), None, "{junk:?}");
    }
}

#[test]
fn the_rule_is_kept_as_json_and_read_back() {
    for first_day in Weekday::ALL {
        for first_week in [FirstWeek::FirstFourDayWeek, FirstWeek::WeekOf1January] {
            let rule = WeekRule { first_day, first_week };
            assert_eq!(from_value(&to_value(rule)), rule);
        }
    }
    assert_eq!(
        to_value(WeekRule { first_day: Weekday::Sunday, first_week: FirstWeek::WeekOf1January }),
        json!({ "first_day": "sunday", "first_week": "week_of_1_january" })
    );
}

#[test]
fn a_kept_value_that_is_junk_falls_back_to_the_default_part_by_part() {
    let default = WeekRule::default();
    for junk in [
        Value::Null,
        json!([]),
        json!("sunday"),
        json!({}),
        json!({ "first_day": 6, "first_week": true }),
        json!({ "first_day": "Sunday", "first_week": "iso" }),
        json!({ "first_day": { "day": "sunday" }, "first_week": ["week_of_1_january"] }),
    ] {
        assert_eq!(from_value(&junk), default, "{junk}");
    }
    assert_eq!(
        from_value(&json!({ "first_day": "saturday", "first_week": "loud" })),
        WeekRule { first_day: Weekday::Saturday, first_week: FirstWeek::FirstFourDayWeek }
    );
    assert_eq!(
        from_value(&json!({ "first_day": "someday", "first_week": "week_of_1_january" })),
        WeekRule { first_day: Weekday::Monday, first_week: FirstWeek::WeekOf1January }
    );
}

#[test]
fn the_settings_show_the_rule_and_the_week_of_today() {
    // NOW is Thursday 5 November 2026, 09:00 UTC: ISO week 45, from Monday 2 November.
    let mut session = session();
    let answer = session.run_as(Caller::Agent, "week.settings", &json!({})).unwrap();
    assert_eq!(
        answer,
        json!({
            "first_day": "monday",
            "first_week": "first_four_day_week",
            "days": DAYS,
            "first_weeks": FIRST_WEEKS,
            "this_week": { "first_day": "2026-11-02", "number": 45, "year": 2026 },
        })
    );
    assert_eq!(session.week_rule(), WeekRule::default());
}

#[test]
fn setting_the_first_day_changes_the_week_of_today() {
    let mut session = session();
    let answer = session.run("week.set", &json!({ "first_day": "sunday" })).unwrap();
    assert_eq!(answer["first_day"], json!("sunday"));
    assert_eq!(answer["first_week"], json!("first_four_day_week"));
    // Week 1 of a Sunday week with four days in 2026 starts Sunday 4 January.
    assert_eq!(answer["this_week"], json!({ "first_day": "2026-11-01", "number": 44, "year": 2026 }));
    assert_eq!(session.week_rule(), WeekRule { first_day: Weekday::Sunday, first_week: FirstWeek::FirstFourDayWeek });
    let answer = session.run("week.set", &json!({ "first_week": "week_of_1_january" })).unwrap();
    // 1 January 2026 is a Thursday: week 1 of a Sunday week holding it starts 28 December 2025.
    assert_eq!(answer["this_week"], json!({ "first_day": "2026-11-01", "number": 45, "year": 2026 }));
    assert_eq!(session.week_rule(), WeekRule { first_day: Weekday::Sunday, first_week: FirstWeek::WeekOf1January });
    assert_eq!(session.run("week.settings", &json!({})).unwrap(), answer);
    let answer = session.run("week.set", &json!({ "first_day": "saturday", "first_week": "first_four_day_week" })).unwrap();
    assert_eq!(answer["this_week"], json!({ "first_day": "2026-10-31", "number": 44, "year": 2026 }));
}

#[test]
fn the_week_of_today_is_on_the_owner_s_wall_clock() {
    // 09:00 UTC on Thursday is already Friday at +15:00 and still Thursday at -10:00.
    let mut session = session();
    session.set_offsets(Box::new(Offset::from_minutes(15 * 60).unwrap()));
    session.run("week.set", &json!({ "first_day": "friday" })).unwrap();
    let answer = session.run("week.settings", &json!({})).unwrap();
    assert_eq!(answer["this_week"]["first_day"], json!("2026-11-06"));
    session.set_offsets(Box::new(Offset::from_minutes(-10 * 60).unwrap()));
    let answer = session.run("week.settings", &json!({})).unwrap();
    assert_eq!(answer["this_week"]["first_day"], json!("2026-10-30"));
}

#[test]
fn changing_the_week_is_the_owner_s() {
    let mut session = session();
    let refused = session.run_as(Caller::Agent, "week.set", &json!({ "first_day": "sunday" })).unwrap_err();
    assert_eq!(refused.code(), "not_allowed");
    assert_eq!(session.week_rule(), WeekRule::default());
}

#[test]
fn bad_week_settings_are_refused_and_change_nothing() {
    let mut session = session();
    for params in [
        json!({}),
        json!(null),
        json!({ "first_day": "Sunday" }),
        json!({ "first_day": 0 }),
        json!({ "first_week": "iso" }),
        json!({ "first_day": "sunday", "first_week": "iso" }),
        json!({ "first_day": "sunday", "volume": 3 }),
    ] {
        assert_eq!(session.run("week.set", &params).unwrap_err().code(), "bad_params", "{params}");
    }
    assert_eq!(session.run("week.settings", &json!({ "first_day": "sunday" })).unwrap_err().code(), "bad_params");
    assert_eq!(session.week_rule(), WeekRule::default());
}

#[test]
fn a_kept_rule_handed_back_is_the_one_used() {
    let mut session = session();
    session.set_week_rule(from_value(&json!({ "first_day": "sunday" })));
    assert_eq!(session.run("week.settings", &json!({})).unwrap()["first_day"], json!("sunday"));
}
