// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{DateError, MAX_COMMENT_DEPTH, MAX_LEN, MailDate, Zone};
use crate::{CivilDateTime, Offset, Timestamp};

fn offset(minutes: i16) -> Zone {
    Zone::Fixed(Offset::from_minutes(minutes).unwrap())
}

fn date(year: i32, month: u8, day: u8, hour: u8, minute: u8, second: u8, zone: Zone) -> MailDate {
    let local = CivilDateTime::new(year, month, day, hour, minute, second).unwrap();
    MailDate::from_local(local, zone).unwrap()
}

fn parse(text: &str) -> MailDate {
    MailDate::parse(text).unwrap_or_else(|err| panic!("{text:?}: {err}"))
}

fn syntax_at(text: &str) -> usize {
    match MailDate::parse(text) {
        Err(DateError::Syntax { at, .. }) => at,
        other => panic!("{text:?}: {other:?}"),
    }
}

// ---- Formatting

#[test]
fn a_date_formats_as_rfc_5322_with_the_weekday() {
    assert_eq!(date(2026, 11, 4, 13, 5, 9, offset(60)).to_string(), "Wed, 4 Nov 2026 13:05:09 +0100");
    assert_eq!(date(1997, 11, 21, 9, 55, 6, offset(-360)).to_string(), "Fri, 21 Nov 1997 09:55:06 -0600");
    assert_eq!(date(2003, 7, 1, 10, 52, 37, offset(0)).to_string(), "Tue, 1 Jul 2003 10:52:37 +0000");
    assert_eq!(date(2026, 1, 5, 0, 0, 0, Zone::Unknown).to_string(), "Mon, 5 Jan 2026 00:00:00 -0000");
    assert_eq!(date(1, 2, 3, 4, 5, 6, offset(5999)).to_string(), "Sat, 3 Feb 0001 04:05:06 +9959");
    assert_eq!(date(2026, 12, 31, 23, 59, 59, offset(-5999)).to_string(), "Thu, 31 Dec 2026 23:59:59 -9959");
}

#[test]
fn every_month_and_weekday_has_its_name() {
    let months: Vec<String> = (1..=12).map(|m| date(2026, m, 1, 0, 0, 0, offset(0)).to_string()).collect();
    let names: Vec<&str> = months.iter().filter_map(|s| s.split(' ').nth(2)).collect();
    assert_eq!(names, ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"]);
    let days: Vec<String> = (2..=8).map(|d| date(2026, 11, d, 0, 0, 0, offset(0)).to_string()).collect();
    let names: Vec<&str> = days.iter().filter_map(|s| s.get(..3)).collect();
    assert_eq!(names, ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]);
}

#[test]
fn a_date_keeps_its_moment_zone_and_local_time() {
    let d = date(2026, 11, 4, 13, 0, 0, offset(60));
    assert_eq!(d.timestamp().unix_seconds(), 1_793_793_600);
    assert_eq!(d.zone(), offset(60));
    assert_eq!(d.local(), CivilDateTime::new(2026, 11, 4, 13, 0, 0).unwrap());
    let unknown = date(2026, 11, 4, 12, 0, 0, Zone::Unknown);
    assert_eq!(unknown.timestamp().unix_seconds(), 1_793_793_600);
    assert_eq!(unknown.zone().offset(), Offset::UTC);
}

#[test]
fn a_date_drops_the_nanoseconds() {
    let ts = Timestamp::from_unix(1_793_793_600, 999).unwrap();
    let d = MailDate::new(ts, offset(0)).unwrap();
    assert_eq!(d.timestamp(), Timestamp::from_unix(1_793_793_600, 0).unwrap());
}

#[test]
fn a_date_needs_a_local_year_from_1_to_9999() {
    let first = CivilDateTime::new(1, 1, 1, 0, 0, 0).unwrap().to_timestamp(Offset::UTC).unwrap();
    assert!(MailDate::new(first, offset(0)).is_some());
    assert!(MailDate::new(first, offset(-1)).is_none());
    let last = CivilDateTime::new(9999, 12, 31, 23, 59, 59).unwrap().to_timestamp(Offset::UTC).unwrap();
    assert!(MailDate::new(last, Zone::Unknown).is_some());
    assert!(MailDate::new(last, offset(1)).is_none());
    assert!(MailDate::new(Timestamp::MIN, offset(0)).is_none());
    assert!(MailDate::new(Timestamp::MAX, offset(0)).is_none());
    assert!(MailDate::from_local(CivilDateTime::new(0, 12, 31, 0, 0, 0).unwrap(), offset(0)).is_none());
    assert!(MailDate::from_local(CivilDateTime::new(10_000, 1, 1, 0, 0, 0).unwrap(), offset(0)).is_none());
}

// ---- Parsing the current form

#[test]
fn the_current_form_parses() {
    assert_eq!(parse("Wed, 4 Nov 2026 13:05:09 +0100"), date(2026, 11, 4, 13, 5, 9, offset(60)));
    assert_eq!(parse("Fri, 21 Nov 1997 09:55:06 -0600"), date(1997, 11, 21, 9, 55, 6, offset(-360)));
    assert_eq!(parse("21 Nov 1997 09:55:06 -0600"), date(1997, 11, 21, 9, 55, 6, offset(-360)));
    assert_eq!(parse("Wed, 04 Nov 2026 13:05 +0100"), date(2026, 11, 4, 13, 5, 0, offset(60)));
    assert_eq!(parse("Thu, 1 Jan 1970 00:00:00 +0000").timestamp(), Timestamp::UNIX_EPOCH);
    assert_eq!(parse("Thu, 13 Feb 1969 23:32:54 -0330"), date(1969, 2, 13, 23, 32, 54, offset(-210)));
}

#[test]
fn minus_zero_means_the_zone_is_unknown() {
    assert_eq!(parse("Wed, 4 Nov 2026 13:05:09 -0000").zone(), Zone::Unknown);
    assert_eq!(parse("Wed, 4 Nov 2026 13:05:09 +0000").zone(), offset(0));
}

#[test]
fn names_and_white_space_are_case_and_space_insensitive() {
    let want = date(2026, 11, 4, 13, 5, 9, offset(60));
    assert_eq!(parse("wed, 4 nov 2026 13:05:09 +0100"), want);
    assert_eq!(parse("WED,4 NOV 2026 13:05:09 +0100"), want);
    assert_eq!(parse("  Wed ,  4\r\n Nov\t2026  13 : 05 : 09   +0100  "), want);
    assert_eq!(parse("Wednesday, 4 November 2026 13:05:09 +0100"), want);
    assert_eq!(parse("Wed, 4 Nov 2026 9:5:9 +0100").local().hour(), 9);
}

#[test]
fn comments_are_skipped_wherever_white_space_may_be() {
    let want = date(2026, 11, 4, 13, 5, 9, offset(60));
    assert_eq!(parse("Wed, 4 Nov 2026 13:05:09 +0100 (CET)"), want);
    assert_eq!(parse("(day) Wed (x), (y) 4 (m) Nov (y) 2026 (t) 13:05:09 (z) +0100 (a (nested) one)"), want);
    assert_eq!(parse("Wed, 4 Nov 2026 13:05:09 +0100 (escaped \\) and \\( paren)"), want);
    assert_eq!(parse("Wed, 4 Nov 2026 13:05:09 +0100 (Mitteleurop\u{e4}ische Zeit)"), want);
}

#[test]
fn the_weekday_is_not_checked_against_the_date() {
    // RFC 5322 says they must agree; a reader shows the date anyway.
    assert_eq!(parse("Mon, 4 Nov 2026 13:05:09 +0100"), date(2026, 11, 4, 13, 5, 9, offset(60)));
}

#[test]
fn second_sixty_is_read_as_fifty_nine() {
    assert_eq!(parse("Sat, 31 Dec 2016 23:59:60 +0000"), date(2016, 12, 31, 23, 59, 59, offset(0)));
}

#[test]
fn every_month_name_parses() {
    let names = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    for (index, name) in names.iter().enumerate() {
        let d = parse(&format!("1 {name} 2026 00:00 +0000"));
        assert_eq!(usize::from(d.local().month()), index + 1);
    }
    let full = ["January", "February", "March", "April", "June", "July", "August", "September", "October"];
    for name in full {
        assert!(MailDate::parse(&format!("1 {name} 2026 00:00 +0000")).is_ok(), "{name}");
    }
    for day in ["Mon", "Tue", "Thu", "Fri", "Sat", "Sun", "Monday", "Tuesday", "Thursday", "Friday", "Saturday", "Sunday"] {
        assert!(MailDate::parse(&format!("{day}, 1 Nov 2026 00:00 +0000")).is_ok(), "{day}");
    }
}

// ---- Parsing the obsolete forms (RFC 5322 section 4.3)

#[test]
fn two_and_three_digit_years_are_read_as_rfc_5322_says() {
    assert_eq!(parse("4 Nov 26 13:05 +0100").local().year(), 2026);
    assert_eq!(parse("4 Nov 49 13:05 +0100").local().year(), 2049);
    assert_eq!(parse("4 Nov 50 13:05 +0100").local().year(), 1950);
    assert_eq!(parse("4 Nov 97 13:05 +0100").local().year(), 1997);
    assert_eq!(parse("4 Nov 126 13:05 +0100").local().year(), 2026);
    assert_eq!(parse("4 Nov 00 13:05 +0100").local().year(), 2000);
    assert_eq!(parse("4 Nov 02026 13:05 +0100").local().year(), 2026);
}

#[test]
fn the_obsolete_zone_names_have_their_offsets() {
    let cases = [
        ("UT", 0),
        ("GMT", 0),
        ("UTC", 0),
        ("EST", -300),
        ("EDT", -240),
        ("CST", -360),
        ("CDT", -300),
        ("MST", -420),
        ("MDT", -360),
        ("PST", -480),
        ("PDT", -420),
        ("gmt", 0),
        ("pdt", -420),
    ];
    for (name, minutes) in cases {
        assert_eq!(parse(&format!("4 Nov 2026 13:05 {name}")).zone(), offset(minutes), "{name}");
    }
}

#[test]
fn military_and_other_zone_names_mean_an_unknown_zone() {
    for name in ["Z", "A", "M", "N", "Y", "z", "CET", "CEST", "MESZ", "Europe"] {
        assert_eq!(parse(&format!("4 Nov 2026 13:05 {name}")).zone(), Zone::Unknown, "{name}");
    }
}

#[test]
fn a_missing_zone_means_an_unknown_zone() {
    assert_eq!(parse("Wed, 4 Nov 2026 13:05:09"), date(2026, 11, 4, 13, 5, 9, Zone::Unknown));
    assert_eq!(parse("Wed, 4 Nov 2026 13:05:09 (no zone)").zone(), Zone::Unknown);
}

// ---- Refusals

#[test]
fn impossible_dates_are_refused() {
    assert_eq!(MailDate::parse("30 Feb 2026 00:00 +0000"), Err(DateError::NoSuchDate));
    assert_eq!(MailDate::parse("29 Feb 2026 00:00 +0000"), Err(DateError::NoSuchDate));
    assert_eq!(MailDate::parse("0 Nov 2026 00:00 +0000"), Err(DateError::NoSuchDate));
    assert_eq!(MailDate::parse("4 Nov 2026 24:00 +0000"), Err(DateError::NoSuchDate));
    assert_eq!(MailDate::parse("4 Nov 2026 23:60 +0000"), Err(DateError::NoSuchDate));
    assert_eq!(MailDate::parse("4 Nov 2026 23:59:61 +0000"), Err(DateError::NoSuchDate));
    assert_eq!(MailDate::parse("4 Nov 2026 00:00 +0060"), Err(DateError::NoSuchDate));
    assert!(MailDate::parse("29 Feb 2024 00:00 +0000").is_ok());
}

#[test]
fn years_outside_1_to_9999_are_refused() {
    assert_eq!(MailDate::parse("4 Nov 0000 00:00 +0000"), Err(DateError::YearOutOfRange));
    assert_eq!(MailDate::parse("4 Nov 10000 00:00 +0000"), Err(DateError::YearOutOfRange));
    assert_eq!(MailDate::parse("4 Nov 99999999999999999999999 00:00 +0000"), Err(DateError::YearOutOfRange));
    assert_eq!(MailDate::parse("1 Jan 0001 00:00 +0100").map(|d| d.local().year()), Ok(1));
    assert_eq!(MailDate::parse("31 Dec 9999 23:59 -9959").map(|d| d.local().year()), Ok(9999));
}

#[test]
fn malformed_dates_say_where_they_went_wrong() {
    assert_eq!(MailDate::parse(""), Err(DateError::Syntax { expected: "the day of the month", at: 0 }));
    assert_eq!(syntax_at("Wed 4 Nov 2026 13:05 +0100"), 4);
    assert_eq!(syntax_at("Wed, Nov 2026 13:05 +0100"), 5);
    assert_eq!(syntax_at("Wed, 4 2026 13:05 +0100"), 7);
    assert_eq!(syntax_at("Wed, 4 Nob 2026 13:05 +0100"), 7);
    assert_eq!(syntax_at("Wed, 4 Nov 13:05 +0100"), 13);
    assert_eq!(syntax_at("Wed, 4 Nov 7 13:05 +0100"), 11);
    assert_eq!(syntax_at("Wed, 4 Nov 2026 +0100"), 16);
    assert_eq!(syntax_at("Wed, 4 Nov 2026 13 +0100"), 19);
    assert_eq!(syntax_at("Wed, 4 Nov 2026 13: +0100"), 20);
    assert_eq!(syntax_at("Wed, 4 Nov 2026 13:05: +0100"), 23);
    assert_eq!(syntax_at("Wed, 4 Nov 2026 13:05 +01"), 22);
    assert_eq!(syntax_at("Wed, 4 Nov 2026 13:05 +01000"), 22);
    assert_eq!(syntax_at("Wed, 4 Nov 2026 13:05 + 0100"), 22);
    assert_eq!(syntax_at("Wed, 4 Nov 2026 13:05 +0100 extra"), 28);
    assert_eq!(syntax_at("Wed, 4 Nov 2026 13:05 CET CET"), 26);
    assert_eq!(syntax_at("Wed, 123 Nov 2026 13:05 +0100"), 5);
    assert_eq!(syntax_at("Wed, 4 Nov 2026 123:05 +0100"), 16);
    assert_eq!(syntax_at("Wed, 4 Nov 2026 13:5x +0100"), 19);
    assert_eq!(syntax_at("Wed, 4 Nov 2026 13:05 +0100 \u{e9}"), 28);
    assert_eq!(syntax_at("Wed, 4 Nov 2026 13:05 +0100 ,"), 28);
    assert_eq!(syntax_at("Wed, 4 Nov 2026 13:05 +0100 :"), 28);
    assert_eq!(syntax_at("Wed, 4 Nov 2026 13:05 +0100 -"), 28);
    assert_eq!(syntax_at("Wed, 4 Nov 2026 13:05 +0100 1"), 28);
    assert_eq!(syntax_at("Wed, 4 Nov 2026 13:05 1"), 22);
    assert_eq!(syntax_at("Wed, 4 Nov 2026 13:05 ,"), 22);
    assert_eq!(syntax_at("Wednes, 4 Nov 2026 13:05 +0100"), 0);
    assert_eq!(syntax_at(", 4 Nov 2026 13:05 +0100"), 0);
    assert_eq!(syntax_at("4, Nov 2026 13:05 +0100"), 1);
}

#[test]
fn unclosed_and_deep_comments_are_refused() {
    assert_eq!(MailDate::parse("4 Nov 2026 13:05 +0100 (open"), Err(DateError::UnclosedComment));
    assert_eq!(MailDate::parse("4 Nov 2026 13:05 +0100 (open \\"), Err(DateError::UnclosedComment));
    let deep = format!("4 Nov 2026 13:05 +0100 {}{}", "(".repeat(MAX_COMMENT_DEPTH), ")".repeat(MAX_COMMENT_DEPTH));
    assert!(MailDate::parse(&deep).is_ok());
    let deeper = format!("4 Nov 2026 13:05 +0100 {}", "(".repeat(MAX_COMMENT_DEPTH + 1));
    assert_eq!(MailDate::parse(&deeper), Err(DateError::CommentTooDeep));
    assert_eq!(syntax_at("4 Nov 2026 13:05 +0100 )"), 23);
}

#[test]
fn long_input_is_refused_before_it_is_read() {
    let padded = format!("4 Nov 2026 13:05 +0100{}", " ".repeat(MAX_LEN - 22));
    assert_eq!(padded.len(), MAX_LEN);
    assert!(MailDate::parse(&padded).is_ok());
    assert_eq!(MailDate::parse(&format!("{padded} ")), Err(DateError::TooLong));
}

#[test]
fn errors_explain_themselves() {
    let texts = [
        DateError::TooLong.to_string(),
        DateError::UnclosedComment.to_string(),
        DateError::CommentTooDeep.to_string(),
        DateError::NoSuchDate.to_string(),
        DateError::YearOutOfRange.to_string(),
        DateError::Syntax { expected: "the month", at: 7 }.to_string(),
    ];
    assert_eq!(texts[5], "expected the month at byte 7");
    for text in &texts {
        assert!(!text.is_empty());
    }
    let err: Box<dyn std::error::Error> = Box::new(DateError::TooLong);
    assert!(err.source().is_none());
}

#[test]
fn from_str_parses_too() {
    let d: MailDate = "Wed, 4 Nov 2026 13:05:09 +0100".parse().unwrap();
    assert_eq!(d, date(2026, 11, 4, 13, 5, 9, offset(60)));
}
