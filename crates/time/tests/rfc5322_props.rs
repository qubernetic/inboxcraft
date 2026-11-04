// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Property tests of the RFC 5322 dates: formatting then parsing gives the date back, the obsolete spellings of a
//! date read as the date, and no text makes the parser panic.

use inboxcraft_time::{CivilDateTime, MailDate, Offset, Timestamp, Zone};
use proptest::prelude::*;

const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

fn zone() -> impl Strategy<Value = Zone> {
    prop_oneof![Just(Zone::Unknown), (-5999_i16..=5999).prop_map(|minutes| Offset::from_minutes(minutes).map_or(Zone::Unknown, Zone::Fixed)),]
}

fn timestamp() -> impl Strategy<Value = Timestamp> {
    (Timestamp::MIN.unix_seconds()..=Timestamp::MAX.unix_seconds(), 0_u32..1_000_000_000)
        .prop_filter_map("in range", |(secs, nanos)| Timestamp::from_unix(secs, nanos))
}

/// A local date in the years an obsolete two-digit year can name.
fn local() -> impl Strategy<Value = CivilDateTime> {
    (1950_i32..=2049, 1_u8..=12, 1_u8..=28, 0_u8..24, 0_u8..60, 0_u8..60)
        .prop_filter_map("a real date", |(y, mo, d, h, mi, s)| CivilDateTime::new(y, mo, d, h, mi, s))
}

/// Comments and folding white space, as a sender may put them between the parts of a date.
fn cfws() -> impl Strategy<Value = String> {
    prop_oneof![
        Just(" ".to_owned()),
        Just("  ".to_owned()),
        Just("\r\n ".to_owned()),
        Just("\t".to_owned()),
        Just(" (comment) ".to_owned()),
        Just(" (a (nested) \\) one) ".to_owned()),
    ]
}

fn spelled_month(month: u8, full: bool, upper: bool) -> String {
    let name = MONTHS.get(usize::from(month) - 1).copied().unwrap_or_default();
    let name = if full { name } else { &name[..3] };
    if upper { name.to_ascii_uppercase() } else { name.to_ascii_lowercase() }
}

proptest! {
    #[test]
    fn formatting_then_parsing_gives_the_date_back(ts in timestamp(), zone in zone()) {
        if let Some(date) = MailDate::new(ts, zone) {
            let text = date.to_string();
            prop_assert_eq!(MailDate::parse(&text), Ok(date), "{}", text);
        }
    }

    #[test]
    fn obsolete_spellings_read_as_the_same_date(
        local in local(),
        zone in zone(),
        gaps in proptest::collection::vec(cfws(), 8),
        two_digit_year in any::<bool>(),
        full_month in any::<bool>(),
        upper in any::<bool>(),
        weekday in any::<bool>(),
    ) {
        let date = MailDate::from_local(local, zone).unwrap();
        let canonical = date.to_string();
        let zone_text = canonical.rsplit(' ').next().unwrap_or_default();
        let year = if two_digit_year { format!("{:02}", local.year() % 100) } else { local.year().to_string() };
        let weekday = if weekday { format!("{}{},", canonical.get(..3).unwrap_or_default(), gaps[0]) } else { String::new() };
        let seconds = if local.second() == 0 && gaps[1].len() % 2 == 0 {
            String::new()
        } else {
            format!("{}:{}{}", gaps[1], gaps[2], local.second())
        };
        let month = spelled_month(local.month(), full_month, upper);
        let (day, hour, minute) = (local.day(), local.hour(), local.minute());
        let text = format!(
            "{weekday}{}{day}{}{month}{}{year}{}{hour}:{minute}{seconds}{}{zone_text}{}",
            gaps[3], gaps[4], gaps[5], gaps[6], gaps[7], gaps[0],
        );
        prop_assert_eq!(MailDate::parse(&text), Ok(date), "{:?}", text);
    }

    #[test]
    fn no_text_makes_the_parser_panic(text in "\\PC{0,64}") {
        let _ = MailDate::parse(&text);
    }

    #[test]
    fn date_shaped_noise_parses_or_errs_and_what_parses_formats_stably(
        text in "[A-Za-z]{0,4},? ?[0-9]{0,3} [A-Za-z]{0,4} [0-9]{0,5} [0-9]{1,2}:[0-9]{1,2}(:[0-9]{1,2})? ?[-+A-Z0-9()]{0,6}"
    ) {
        if let Ok(date) = MailDate::parse(&text) {
            prop_assert_eq!(MailDate::parse(&date.to_string()), Ok(date));
        }
    }
}
