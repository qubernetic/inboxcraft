// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use inboxcraft_time::{CivilDateTime, Timestamp, Weekday};
use proptest::prelude::*;

use super::{End, Frequency, MAX_RULE_VALUES, Rule, WeekdayNum};
use crate::{Date, IcalError, Property, When};

fn at(text: &str) -> CivilDateTime {
    match When::from_property(&Property::new("DTSTART", text)).unwrap() {
        When::Floating(civil) => civil,
        When::Date(date) => date.midnight(),
        other => panic!("{other:?}"),
    }
}

fn show(civil: CivilDateTime) -> String {
    format!("{}T{:02}{:02}{:02}", Date::of(civil), civil.hour(), civil.minute(), civil.second())
}

/// The first `take` occurrences of `rule` from `start`, as `YYYYMMDDTHHMMSS`.
fn expand(rule: &str, start: &str, take: usize) -> Vec<String> {
    Rule::parse(rule).unwrap().iter(at(start)).take(take).map(show).collect()
}

fn days(dates: &[&str]) -> Vec<String> {
    dates.iter().map(|date| format!("{date}T090000")).collect()
}

#[test]
fn daily_rules() {
    assert_eq!(
        expand("FREQ=DAILY;COUNT=10", "19970902T090000", 100),
        days(&["19970902", "19970903", "19970904", "19970905", "19970906", "19970907", "19970908", "19970909", "19970910", "19970911"])
    );
    assert_eq!(expand("FREQ=DAILY;INTERVAL=10;COUNT=5", "19970902T090000", 100), days(&["19970902", "19970912", "19970922", "19971002", "19971012"]));
    let january = Rule::parse("FREQ=DAILY;BYMONTH=1").unwrap();
    let all: Vec<_> = january.iter(at("19980101T090000")).stop_after(at("20000131T140000")).collect();
    assert_eq!(all.len(), 93);
    let yearly = Rule::parse("FREQ=YEARLY;BYMONTH=1;BYDAY=SU,MO,TU,WE,TH,FR,SA").unwrap();
    let same: Vec<_> = yearly.iter(at("19980101T090000")).stop_after(at("20000131T140000")).collect();
    assert_eq!(all, same);
}

#[test]
fn weekly_rules() {
    assert_eq!(expand("FREQ=WEEKLY;COUNT=3", "19970902T090000", 100), days(&["19970902", "19970909", "19970916"]));
    let every_other = Rule::parse("FREQ=WEEKLY;INTERVAL=2;WKST=SU;BYDAY=MO,WE,FR").unwrap();
    let got: Vec<_> = every_other.iter(at("19970901T090000")).stop_after(at("19971224T000000")).map(show).collect();
    assert_eq!(
        got,
        days(&[
            "19970901", "19970903", "19970905", "19970915", "19970917", "19970919", "19970929", "19971001", "19971003", "19971013", "19971015",
            "19971017", "19971027", "19971029", "19971031", "19971110", "19971112", "19971114", "19971124", "19971126", "19971128", "19971208",
            "19971210", "19971212", "19971222"
        ])
    );
    // The week start changes which days share a period.
    assert_eq!(
        expand("FREQ=WEEKLY;INTERVAL=2;COUNT=4;BYDAY=TU,SU;WKST=MO", "19970805T090000", 9),
        days(&["19970805", "19970810", "19970819", "19970824"])
    );
    assert_eq!(
        expand("FREQ=WEEKLY;INTERVAL=2;COUNT=4;BYDAY=TU,SU;WKST=SU", "19970805T090000", 9),
        days(&["19970805", "19970817", "19970819", "19970831"])
    );
    // An ordinal means nothing in a week.
    assert_eq!(expand("FREQ=WEEKLY;BYDAY=2TU", "19970902T090000", 2), days(&["19970902", "19970909"]));
}

#[test]
fn monthly_rules() {
    assert_eq!(
        expand("FREQ=MONTHLY;COUNT=10;BYDAY=1FR", "19970905T090000", 100),
        days(&["19970905", "19971003", "19971107", "19971205", "19980102", "19980206", "19980306", "19980403", "19980501", "19980605"])
    );
    assert_eq!(
        expand("FREQ=MONTHLY;COUNT=6;BYDAY=-2MO", "19970922T090000", 100),
        days(&["19970922", "19971020", "19971117", "19971222", "19980119", "19980216"])
    );
    assert_eq!(
        expand("FREQ=MONTHLY;BYMONTHDAY=-3", "19970928T090000", 6),
        days(&["19970928", "19971029", "19971128", "19971229", "19980129", "19980226"])
    );
    assert_eq!(
        expand("FREQ=MONTHLY;BYDAY=FR;BYMONTHDAY=13", "19970902T090000", 5),
        days(&["19980213", "19980313", "19981113", "19990813", "20001013"])
    );
    assert_eq!(
        expand("FREQ=MONTHLY;BYDAY=SA;BYMONTHDAY=7,8,9,10,11,12,13", "19970913T090000", 6),
        days(&["19970913", "19971011", "19971108", "19971213", "19980110", "19980207"])
    );
    // A day the month does not have is skipped.
    assert_eq!(
        expand("FREQ=MONTHLY;BYMONTHDAY=15,30;COUNT=5", "20070115T090000", 9),
        days(&["20070115", "20070130", "20070215", "20070315", "20070330"])
    );
    assert_eq!(expand("FREQ=MONTHLY", "20070131T090000", 3), days(&["20070131", "20070331", "20070531"]));
}

#[test]
fn set_positions() {
    assert_eq!(expand("FREQ=MONTHLY;COUNT=3;BYDAY=TU,WE,TH;BYSETPOS=3", "19970904T090000", 9), days(&["19970904", "19971007", "19971106"]));
    assert_eq!(
        expand("FREQ=MONTHLY;BYDAY=MO,TU,WE,TH,FR;BYSETPOS=-2", "19970929T090000", 7),
        days(&["19970929", "19971030", "19971127", "19971230", "19980129", "19980226", "19980330"])
    );
    // Positions past the end of the set name nothing; the same position twice comes once.
    assert_eq!(expand("FREQ=MONTHLY;BYMONTHDAY=1,2;BYSETPOS=1,1,5,-9", "19970901T090000", 2), days(&["19970901", "19971001"]));
}

#[test]
fn yearly_rules() {
    assert_eq!(expand("FREQ=YEARLY;COUNT=4;BYMONTH=6,7", "19970610T090000", 9), days(&["19970610", "19970710", "19980610", "19980710"]));
    assert_eq!(expand("FREQ=YEARLY;BYDAY=20MO", "19970519T090000", 3), days(&["19970519", "19980518", "19990517"]));
    assert_eq!(expand("FREQ=YEARLY;BYDAY=-1SU", "19971228T090000", 2), days(&["19971228", "19981227"]));
    assert_eq!(expand("FREQ=YEARLY;BYWEEKNO=20;BYDAY=MO", "19970512T090000", 3), days(&["19970512", "19980511", "19990517"]));
    assert_eq!(expand("FREQ=YEARLY;BYWEEKNO=-1;BYDAY=MO", "19971222T090000", 2), days(&["19971222", "19981228"]));
    // Week 1 of 1998 starts on 29 December 1997.
    assert_eq!(expand("FREQ=YEARLY;BYWEEKNO=1;BYDAY=MO", "19970101T090000", 2), days(&["19971229", "19990104"]));
    assert_eq!(
        expand("FREQ=YEARLY;BYMONTH=3;BYDAY=TH", "19970313T090000", 7),
        days(&["19970313", "19970320", "19970327", "19980305", "19980312", "19980319", "19980326"])
    );
    assert_eq!(
        expand("FREQ=YEARLY;INTERVAL=4;BYMONTH=11;BYDAY=TU;BYMONTHDAY=2,3,4,5,6,7,8", "19961105T090000", 3),
        days(&["19961105", "20001107", "20041102"])
    );
    assert_eq!(
        expand("FREQ=YEARLY;COUNT=6;INTERVAL=3;BYYEARDAY=1,100,200", "19970101T090000", 9),
        days(&["19970101", "19970410", "19970719", "20000101", "20000409", "20000718"])
    );
    assert_eq!(expand("FREQ=YEARLY;BYYEARDAY=-1", "19971231T090000", 2), days(&["19971231", "19981231"]));
    assert_eq!(expand("FREQ=YEARLY;BYMONTH=3;BYDAY=-1SU", "20260329T020000", 2), ["20260329T020000", "20270328T020000"]);
    assert_eq!(expand("FREQ=YEARLY", "20000229T090000", 3), days(&["20000229", "20040229", "20080229"]));
}

#[test]
fn rules_below_a_day() {
    let hours: Vec<_> =
        Rule::parse("FREQ=HOURLY;INTERVAL=3").unwrap().iter(at("19970902T090000")).stop_after(at("19970902T170000")).map(show).collect();
    assert_eq!(hours, ["19970902T090000", "19970902T120000", "19970902T150000"]);
    assert_eq!(
        expand("FREQ=MINUTELY;INTERVAL=15;COUNT=6", "19970902T090000", 9),
        ["19970902T090000", "19970902T091500", "19970902T093000", "19970902T094500", "19970902T100000", "19970902T101500"]
    );
    assert_eq!(expand("FREQ=SECONDLY;BYSECOND=0,30;COUNT=3", "19970902T090000", 9), ["19970902T090000", "19970902T090030", "19970902T090100"]);
    assert_eq!(expand("FREQ=SECONDLY;BYMINUTE=5;COUNT=2", "19970902T090000", 9), ["19970902T090500", "19970902T090501"]);
    assert_eq!(expand("FREQ=SECONDLY;BYHOUR=10", "19970902T090000", 1), ["19970902T100000"]);
    assert_eq!(expand("FREQ=HOURLY;BYMONTH=1", "19970902T090000", 2), ["19980101T000000", "19980101T010000"]);
    assert_eq!(expand("FREQ=HOURLY;BYMINUTE=0,30", "19970902T091500", 2), ["19970902T093000", "19970902T100000"]);
    assert_eq!(
        expand("FREQ=MINUTELY;BYHOUR=9;BYMINUTE=59;BYSECOND=1,2", "19970902T090000", 3),
        ["19970902T095901", "19970902T095902", "19970903T095901"]
    );
    // Every 20 minutes from 9:00 to 16:40, two ways.
    let daily = Rule::parse("FREQ=DAILY;BYHOUR=9,10,11,12,13,14,15,16;BYMINUTE=0,20,40").unwrap();
    let minutely = Rule::parse("FREQ=MINUTELY;INTERVAL=20;BYHOUR=9,10,11,12,13,14,15,16").unwrap();
    let start = at("19970902T090000");
    let one: Vec<_> = daily.iter(start).take(60).collect();
    let other: Vec<_> = minutely.iter(start).take(60).collect();
    assert_eq!(one, other);
    assert_eq!(show(one[23]), "19970902T164000");
    assert_eq!(show(one[24]), "19970903T090000");
}

#[test]
fn a_rule_that_never_matches_ends_and_says_so() {
    let rule = Rule::parse("FREQ=DAILY;BYMONTH=2;BYMONTHDAY=30").unwrap();
    let mut recurrence = rule.iter(at("20260101T090000"));
    assert_eq!(recurrence.next(), None);
    assert!(recurrence.truncated());
    assert_eq!(recurrence.next(), None);
    // Past year 9999 a rule simply ends.
    let rule = Rule::parse("FREQ=YEARLY;BYMONTH=2;BYMONTHDAY=30").unwrap();
    let mut recurrence = rule.iter(at("20260101T090000"));
    assert_eq!(recurrence.next(), None);
    assert!(!recurrence.truncated());
    assert_eq!(expand("FREQ=YEARLY;INTERVAL=4000", "20260101T090000", 9), days(&["20260101", "60260101"]));
    assert_eq!(expand("FREQ=SECONDLY;INTERVAL=4000000000", "99991231T235959", 9), ["99991231T235959"]);
    assert_eq!(expand("FREQ=MONTHLY;INTERVAL=4000000000", "99991231T235959", 9), ["99991231T235959"]);
    assert_eq!(expand("FREQ=WEEKLY;INTERVAL=4000000000", "99991231T235959", 9), ["99991231T235959"]);
    assert_eq!(expand("FREQ=DAILY;INTERVAL=4000000000", "99991231T235959", 9), ["99991231T235959"]);
    let last_year = CivilDateTime::new(10_000, 1, 1, 9, 0, 0).unwrap();
    assert_eq!(Rule::parse("FREQ=YEARLY;BYWEEKNO=1").unwrap().iter(last_year).next(), None);
}

#[test]
fn a_rule_with_too_many_candidates_in_a_period_says_so() {
    let rule = Rule::parse("FREQ=YEARLY;BYMONTH=1,2,3,4,5,6,7,8,9,10,11,12;BYHOUR=0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23;BYMINUTE=0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32,33,34,35,36,37,38,39,40,41,42,43,44,45,46,47,48,49,50,51,52,53,54,55,56,57,58,59;BYMONTHDAY=1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31").unwrap();
    let mut recurrence = rule.iter(at("20260101T000000"));
    assert_eq!(recurrence.next().map(show).as_deref(), Some("20260101T000000"));
    assert!(recurrence.truncated());
}

#[test]
fn a_budget_ends_a_recurrence_after_that_much_work_and_says_so() {
    // Without a budget this rule walks 100 000 empty days before it gives up.
    let rule = Rule::parse("FREQ=DAILY;BYMONTH=2;BYMONTHDAY=30").unwrap();
    let mut recurrence = rule.iter(at("20260101T090000")).with_budget(1000);
    assert_eq!(recurrence.next(), None);
    assert!(recurrence.truncated());
    assert!((1000..1010).contains(&recurrence.work()), "{}", recurrence.work());
    // Each period counts, with the days it looks at and the candidates it makes.
    let rule = Rule::parse("FREQ=YEARLY;BYMONTH=3;BYDAY=-1SU").unwrap();
    let mut recurrence = rule.iter(at("20260329T020000"));
    assert_eq!(recurrence.next().map(show).as_deref(), Some("20260329T020000"));
    assert_eq!(recurrence.work(), 1 + 31 + 1);
    let hourly = Rule::parse("FREQ=HOURLY;BYMONTH=2").unwrap();
    let mut sub_daily = hourly.iter(at("20260101T000000")).with_budget(10);
    assert_eq!(sub_daily.next(), None);
    assert!(sub_daily.truncated());
    let twice = Rule::parse("FREQ=DAILY;COUNT=2").unwrap();
    let mut enough = twice.iter(at("20260101T090000")).with_budget(100);
    assert_eq!(enough.by_ref().count(), 2);
    assert!(!enough.truncated());
}

#[test]
fn a_count_is_kept_and_skip_to_does_nothing_with_one() {
    let rule = Rule::parse("FREQ=DAILY;COUNT=3").unwrap();
    let mut recurrence = rule.iter(at("20260101T090000"));
    recurrence.skip_to(at("20270101T000000"));
    assert_eq!(recurrence.map(show).collect::<Vec<_>>(), days(&["20260101", "20260102", "20260103"]));
    let none = Rule::parse("FREQ=DAILY;COUNT=0").unwrap();
    assert_eq!(none.iter(at("20260101T090000")).next(), None);
}

#[test]
fn skip_to_jumps_ahead() {
    for (rule, start, to, first) in [
        ("FREQ=DAILY", "20000101T090000", "20260315T120000", "20260316T090000"),
        ("FREQ=WEEKLY;BYDAY=MO", "20000103T090000", "20260315T120000", "20260316T090000"),
        ("FREQ=MONTHLY;BYMONTHDAY=16", "20000116T090000", "20260315T120000", "20260316T090000"),
        ("FREQ=YEARLY;BYMONTH=3;BYMONTHDAY=16", "20000316T090000", "20260315T120000", "20260316T090000"),
        ("FREQ=HOURLY;INTERVAL=7", "20000101T000000", "20260315T120000", "20260315T180000"),
    ] {
        let rule = Rule::parse(rule).unwrap();
        let mut recurrence = rule.iter(at(start));
        recurrence.skip_to(at(to));
        let next = recurrence.find(|occurrence| *occurrence >= at(to));
        assert_eq!(next.map(show).as_deref(), Some(first), "{rule}");
    }
    // Skipping backwards does nothing.
    let rule = Rule::parse("FREQ=DAILY").unwrap();
    let mut recurrence = rule.iter(at("20260101T090000"));
    recurrence.skip_to(at("20000101T000000"));
    assert_eq!(recurrence.next().map(show).as_deref(), Some("20260101T090000"));
}

#[test]
fn reads_and_writes_rules() {
    let rule = Rule::parse(" freq=monthly;interval=2;until=20261231;byday=1MO,-1fr,su;bymonthday=1,-1;wkst=su;x-name=1;; ").unwrap();
    assert_eq!(rule.frequency, Frequency::Monthly);
    assert_eq!(rule.interval, 2);
    assert_eq!(rule.end, End::Until(When::Date(Date::new(2026, 12, 31).unwrap())));
    assert_eq!(
        rule.by_day,
        [
            WeekdayNum { ordinal: 1, weekday: Weekday::Monday },
            WeekdayNum { ordinal: -1, weekday: Weekday::Friday },
            WeekdayNum { ordinal: 0, weekday: Weekday::Sunday }
        ]
    );
    assert_eq!(rule.week_start, Weekday::Sunday);
    assert_eq!(rule.to_string(), "FREQ=MONTHLY;UNTIL=20261231;INTERVAL=2;BYDAY=1MO,-1FR,SU;BYMONTHDAY=1,-1;WKST=SU");
    let full = "FREQ=SECONDLY;COUNT=4;BYSECOND=1;BYMINUTE=2;BYHOUR=3;BYYEARDAY=-366;BYWEEKNO=53;BYMONTH=12;BYSETPOS=1";
    assert_eq!(Rule::parse(full).unwrap().to_string(), full);
    let until = Rule::parse("FREQ=DAILY;UNTIL=20261231T120000Z").unwrap();
    assert_eq!(until.end, End::Until(When::Utc(Timestamp::from_unix(1_798_718_400, 0).unwrap())));
    let zoned = Rule { end: End::Until(When::Zoned(at("20261231T120000"), "A".into())), ..Rule::new(Frequency::Daily) };
    assert_eq!(zoned.to_string(), "FREQ=DAILY;UNTIL=20261231T120000");
    let property = Property::new("RRULE", "FREQ=DAILY;COUNT=2");
    assert_eq!(Rule::from_property(&property).unwrap().end, End::Count(2));
    assert_eq!(Rule::from_property(&Property::new("RRULE", "COUNT=2")), Err(IcalError::bad_value("RRULE")));
}

#[test]
fn refuses_rules_out_of_range() {
    let too_many = format!("FREQ=DAILY;BYHOUR={}", vec!["1"; MAX_RULE_VALUES + 1].join(","));
    let too_many_days = format!("FREQ=DAILY;BYDAY={}", vec!["MO"; MAX_RULE_VALUES + 1].join(","));
    for text in [
        "",
        "COUNT=1",
        "FREQ=FORTNIGHTLY",
        "FREQ=DAILY;COUNT=1;UNTIL=20260101",
        "FREQ=DAILY;INTERVAL=0",
        "FREQ=DAILY;INTERVAL=x",
        "FREQ=DAILY;COUNT=-1",
        "FREQ=DAILY;UNTIL=x",
        "FREQ=DAILY;BYSECOND=61",
        "FREQ=DAILY;BYMINUTE=60",
        "FREQ=DAILY;BYHOUR=24",
        "FREQ=DAILY;BYDAY=XX",
        "FREQ=DAILY;BYDAY=0MO",
        "FREQ=DAILY;BYDAY=54MO",
        "FREQ=DAILY;BYDAY=+",
        "FREQ=DAILY;BYDAY=M",
        "FREQ=DAILY;BYMONTHDAY=0",
        "FREQ=DAILY;BYMONTHDAY=32",
        "FREQ=DAILY;BYYEARDAY=367",
        "FREQ=DAILY;BYWEEKNO=-54",
        "FREQ=DAILY;BYMONTH=13",
        "FREQ=DAILY;BYSETPOS=0",
        "FREQ=DAILY;WKST=XX",
        "FREQ=DAILY;NOEQUALS",
        &too_many,
        &too_many_days,
    ] {
        assert_eq!(Rule::parse(text), Err(IcalError::bad_value("RRULE")), "{text}");
    }
}

#[test]
fn a_rule_built_by_hand_skips_values_out_of_range() {
    let rule = Rule { by_month: vec![0, 13, 2], by_second: vec![60, 0], ..Rule::new(Frequency::Yearly) };
    assert_eq!(rule.iter(at("20260210T090000")).take(2).map(show).collect::<Vec<_>>(), days(&["20260210", "20270210"]));
    let zero = Rule { interval: 0, ..Rule::new(Frequency::Daily) };
    assert_eq!(zero.iter(at("20260210T090000")).take(2).map(show).collect::<Vec<_>>(), days(&["20260210", "20260211"]));
}

fn frequency() -> impl Strategy<Value = Frequency> {
    prop_oneof![Just(Frequency::Hourly), Just(Frequency::Daily), Just(Frequency::Weekly), Just(Frequency::Monthly), Just(Frequency::Yearly)]
}

fn weekday() -> impl Strategy<Value = Weekday> {
    prop::sample::select(vec![
        Weekday::Monday,
        Weekday::Tuesday,
        Weekday::Wednesday,
        Weekday::Thursday,
        Weekday::Friday,
        Weekday::Saturday,
        Weekday::Sunday,
    ])
}

fn rule() -> impl Strategy<Value = Rule> {
    let ordinal = prop_oneof![Just(0i8), -5i8..=-1, 1i8..=5];
    let by_day = prop::collection::vec((ordinal, weekday()).prop_map(|(ordinal, weekday)| WeekdayNum { ordinal, weekday }), 0..3);
    let month_day = prop_oneof![-31i8..=-1, 1i8..=31];
    let lists = (
        prop::collection::vec(1u8..=12, 0..3),
        prop::collection::vec(month_day, 0..3),
        by_day,
        prop::collection::vec(0u8..=23, 0..3),
        prop::collection::vec(prop_oneof![-3i16..=-1, 1i16..=3], 0..2),
    );
    (frequency(), 1u32..=4, lists, weekday()).prop_map(|(frequency, interval, (months, month_days, days, hours, set_pos), week_start)| Rule {
        interval,
        by_month: months,
        by_month_day: month_days,
        by_day: days,
        by_hour: if frequency == Frequency::Hourly { Vec::new() } else { hours },
        by_set_pos: set_pos,
        week_start,
        ..Rule::new(frequency)
    })
}

fn start() -> impl Strategy<Value = CivilDateTime> {
    (1990i32..2030, 1u8..=12, 1u8..=28, 0u8..24, 0u8..60).prop_map(|(y, m, d, h, min)| CivilDateTime::new(y, m, d, h, min, 0).unwrap())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    #[test]
    fn occurrences_rise_start_at_dtstart_and_pass_the_filters(rule in rule(), start in start()) {
        let occurrences: Vec<_> = rule.iter(start).take(40).collect();
        prop_assert!(occurrences.windows(2).all(|pair| pair[0] < pair[1]));
        prop_assert!(occurrences.iter().all(|occurrence| *occurrence >= start));
        for occurrence in &occurrences {
            prop_assert!(rule.by_month.is_empty() || rule.by_month.contains(&occurrence.month()));
            prop_assert!(rule.by_hour.is_empty() || rule.by_hour.contains(&occurrence.hour()));
            prop_assert!(rule.by_day.is_empty() || rule.by_day.iter().any(|day| day.weekday == occurrence.weekday()));
        }
    }

    #[test]
    fn a_count_gives_at_most_that_many(rule in rule(), start in start(), count in 0u32..20) {
        let counted = Rule { end: End::Count(count), ..rule.clone() };
        let with_count: Vec<_> = counted.iter(start).collect();
        let without: Vec<_> = rule.iter(start).take(with_count.len()).collect();
        prop_assert!(with_count.len() <= count as usize);
        prop_assert_eq!(with_count, without);
    }

    #[test]
    fn skipping_ahead_loses_nothing_after_the_point(rule in rule(), start in start(), ahead in 0i64..2000) {
        let point = crate::value::civil_from_seconds(crate::value::civil_seconds(start) + ahead * 86_400).unwrap();
        let full: Vec<_> = rule.iter(start).filter(|occurrence| *occurrence >= point).take(10).collect();
        let mut skipping = rule.iter(start);
        skipping.skip_to(point);
        let skipped: Vec<_> = skipping.filter(|occurrence| *occurrence >= point).take(10).collect();
        prop_assert_eq!(skipped, full);
    }

    #[test]
    fn stopping_is_taking_while_before_the_stop(rule in rule(), start in start(), ahead in 0i64..400) {
        let stop = crate::value::civil_from_seconds(crate::value::civil_seconds(start) + ahead * 86_400).unwrap();
        let stopped: Vec<_> = rule.iter(start).stop_after(stop).take(50).collect();
        let taken: Vec<_> = rule.iter(start).take_while(|occurrence| *occurrence <= stop).take(50).collect();
        prop_assert_eq!(stopped, taken);
    }

    #[test]
    fn a_daily_interval_is_kept(interval in 1u32..30, start in start()) {
        let rule = Rule { interval, ..Rule::new(Frequency::Daily) };
        let occurrences: Vec<_> = rule.iter(start).take(20).collect();
        let step = i64::from(interval) * 86_400;
        prop_assert!(occurrences.windows(2).all(|pair| crate::value::civil_seconds(pair[1]) - crate::value::civil_seconds(pair[0]) == step));
    }

    #[test]
    fn written_rules_read_back(rule in rule(), count in prop::option::of(0u32..100)) {
        let rule = Rule { end: count.map_or(End::Never, End::Count), ..rule };
        prop_assert_eq!(Rule::parse(&rule.to_string()), Ok(rule));
    }
}
