// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use inboxcraft_time::{CivilDateTime, Offset, Offsets, Timestamp};

use super::{TimeZone, Zones};
use crate::{Date, IcalError, When, parse};

pub(crate) const BUDAPEST: &str = "BEGIN:VTIMEZONE\r\n\
    TZID:Europe/Budapest\r\n\
    BEGIN:DAYLIGHT\r\n\
    TZOFFSETFROM:+0100\r\n\
    TZOFFSETTO:+0200\r\n\
    TZNAME:CEST\r\n\
    DTSTART:19700329T020000\r\n\
    RRULE:FREQ=YEARLY;BYMONTH=3;BYDAY=-1SU\r\n\
    END:DAYLIGHT\r\n\
    BEGIN:STANDARD\r\n\
    TZOFFSETFROM:+0200\r\n\
    TZOFFSETTO:+0100\r\n\
    TZNAME:CET\r\n\
    DTSTART:19701025T030000\r\n\
    RRULE:FREQ=YEARLY;BYMONTH=10;BYDAY=-1SU\r\n\
    END:STANDARD\r\n\
    END:VTIMEZONE\r\n";

const NEW_YORK: &str = "BEGIN:VTIMEZONE\r\n\
    TZID:America/New_York\r\n\
    BEGIN:DAYLIGHT\r\n\
    TZOFFSETFROM:-0500\r\n\
    TZOFFSETTO:-0400\r\n\
    DTSTART:19870405T020000\r\n\
    RRULE:FREQ=YEARLY;BYMONTH=4;BYDAY=1SU;UNTIL=20060402T070000Z\r\n\
    END:DAYLIGHT\r\n\
    BEGIN:DAYLIGHT\r\n\
    TZOFFSETFROM:-0500\r\n\
    TZOFFSETTO:-0400\r\n\
    DTSTART:20070311T020000\r\n\
    RRULE:FREQ=YEARLY;BYMONTH=3;BYDAY=2SU\r\n\
    END:DAYLIGHT\r\n\
    BEGIN:STANDARD\r\n\
    TZOFFSETFROM:-0400\r\n\
    TZOFFSETTO:-0500\r\n\
    DTSTART:19671029T020000\r\n\
    RRULE:FREQ=YEARLY;BYMONTH=10;BYDAY=-1SU;UNTIL=20061029T060000Z\r\n\
    END:STANDARD\r\n\
    BEGIN:STANDARD\r\n\
    TZOFFSETFROM:-0400\r\n\
    TZOFFSETTO:-0500\r\n\
    DTSTART:20071104T020000\r\n\
    RRULE:FREQ=YEARLY;BYMONTH=11;BYDAY=1SU\r\n\
    END:STANDARD\r\n\
    END:VTIMEZONE\r\n";

pub(crate) fn zone(text: &str) -> TimeZone {
    TimeZone::from_component(&parse(text.as_bytes()).unwrap()).unwrap()
}

fn civil(year: i32, month: u8, day: u8, hour: u8, minute: u8) -> CivilDateTime {
    CivilDateTime::new(year, month, day, hour, minute, 0).unwrap()
}

fn utc(year: i32, month: u8, day: u8, hour: u8, minute: u8) -> Timestamp {
    civil(year, month, day, hour, minute).to_timestamp(Offset::UTC).unwrap()
}

fn minutes(zone: &TimeZone, at: Timestamp) -> i16 {
    zone.offset_at(at).minutes()
}

#[test]
fn a_zone_with_summer_time_changes_at_its_onsets() {
    let budapest = zone(BUDAPEST);
    assert_eq!(budapest.tzid(), "Europe/Budapest");
    assert_eq!(minutes(&budapest, utc(2026, 3, 29, 0, 59)), 60);
    assert_eq!(minutes(&budapest, utc(2026, 3, 29, 1, 0)), 120);
    assert_eq!(minutes(&budapest, utc(2026, 10, 25, 0, 59)), 120);
    assert_eq!(minutes(&budapest, utc(2026, 10, 25, 1, 0)), 60);
    assert_eq!(minutes(&budapest, utc(2027, 1, 15, 12, 0)), 60);
    // Before its first onset a zone keeps the offset the first onset changes from.
    assert_eq!(minutes(&budapest, utc(1960, 7, 1, 0, 0)), 60);
    assert_eq!(budapest.component().name(), "VTIMEZONE");
}

#[test]
fn a_wall_clock_time_in_a_gap_or_twice_resolves_as_the_time_crate_says() {
    let budapest = zone(BUDAPEST);
    assert_eq!(budapest.resolve(civil(2026, 3, 29, 2, 30)), Some(utc(2026, 3, 29, 1, 30)));
    assert_eq!(budapest.resolve(civil(2026, 10, 25, 2, 30)), Some(utc(2026, 10, 25, 0, 30)));
    assert_eq!(budapest.resolve(civil(2026, 7, 1, 9, 0)), Some(utc(2026, 7, 1, 7, 0)));
}

#[test]
fn a_zone_whose_rules_changed_follows_each_in_its_years() {
    let new_york = zone(NEW_YORK);
    assert_eq!(minutes(&new_york, utc(2006, 3, 20, 12, 0)), -300);
    assert_eq!(minutes(&new_york, utc(2006, 4, 15, 12, 0)), -240);
    assert_eq!(minutes(&new_york, utc(2006, 11, 1, 12, 0)), -300);
    assert_eq!(minutes(&new_york, utc(2007, 3, 5, 12, 0)), -300);
    assert_eq!(minutes(&new_york, utc(2007, 3, 20, 12, 0)), -240);
    assert_eq!(minutes(&new_york, utc(2007, 11, 1, 12, 0)), -240);
    assert_eq!(minutes(&new_york, utc(2007, 11, 5, 12, 0)), -300);
    assert_eq!(minutes(&new_york, utc(2026, 7, 1, 12, 0)), -240);
}

#[test]
fn onsets_come_from_dates_and_rules_that_end_on_a_day_or_a_wall_clock_time() {
    let text = "BEGIN:VTIMEZONE\r\n\
        TZID:Test/Onsets\r\n\
        BEGIN:STANDARD\r\n\
        DTSTART:20000101T000000\r\n\
        TZOFFSETFROM:+0000\r\n\
        TZOFFSETTO:+0100\r\n\
        RRULE:FREQ=YEARLY;UNTIL=20030101\r\n\
        END:STANDARD\r\n\
        BEGIN:DAYLIGHT\r\n\
        DTSTART:20000701T000000\r\n\
        TZOFFSETFROM:+0100\r\n\
        TZOFFSETTO:+0200\r\n\
        RRULE:FREQ=YEARLY;UNTIL=20040701T000000\r\n\
        END:DAYLIGHT\r\n\
        BEGIN:STANDARD\r\n\
        DTSTART:19990101T000000\r\n\
        TZOFFSETFROM:+0200\r\n\
        TZOFFSETTO:+0300\r\n\
        RDATE:20050101T000000Z,20090101T000000\r\n\
        RDATE;VALUE=DATE:20070101\r\n\
        END:STANDARD\r\n\
        BEGIN:X-OTHER\r\n\
        END:X-OTHER\r\n\
        END:VTIMEZONE\r\n";
    let zone = zone(text);
    assert_eq!(minutes(&zone, utc(1999, 6, 1, 0, 0)), 180);
    assert_eq!(minutes(&zone, utc(2002, 3, 1, 0, 0)), 60);
    assert_eq!(minutes(&zone, utc(2004, 3, 1, 0, 0)), 120);
    assert_eq!(minutes(&zone, utc(2005, 3, 1, 0, 0)), 180);
    assert_eq!(minutes(&zone, utc(2005, 8, 1, 0, 0)), 180);
    assert_eq!(minutes(&zone, utc(2008, 8, 1, 0, 0)), 180);
}

#[test]
fn a_rule_with_a_count_is_walked_from_its_start() {
    let text = "BEGIN:VTIMEZONE\r\nTZID:Test/Count\r\n\
        BEGIN:DAYLIGHT\r\nDTSTART:20000401T000000\r\nTZOFFSETFROM:+0000\r\nTZOFFSETTO:+0100\r\nRRULE:FREQ=YEARLY;COUNT=3\r\nEND:DAYLIGHT\r\n\
        BEGIN:STANDARD\r\nDTSTART:20001001T000000\r\nTZOFFSETFROM:+0100\r\nTZOFFSETTO:+0000\r\nRRULE:FREQ=YEARLY\r\nEND:STANDARD\r\n\
        END:VTIMEZONE\r\n";
    let zone = zone(text);
    assert_eq!(minutes(&zone, utc(2002, 5, 1, 0, 0)), 60);
    assert_eq!(minutes(&zone, utc(2003, 5, 1, 0, 0)), 0);
}

#[test]
fn refuses_zones_it_cannot_read() {
    let read = |text: &str| TimeZone::from_component(&parse(text.as_bytes()).unwrap());
    let missing = |property| Err(IcalError::Missing { component: "VTIMEZONE", property });
    assert_eq!(read("BEGIN:VTIMEZONE\r\nBEGIN:STANDARD\r\nEND:STANDARD\r\nEND:VTIMEZONE\r\n"), missing("TZID"));
    assert_eq!(read("BEGIN:VTIMEZONE\r\nTZID:A\r\nEND:VTIMEZONE\r\n"), missing("STANDARD"));
    let part = |body: &str| format!("BEGIN:VTIMEZONE\r\nTZID:A\r\nBEGIN:STANDARD\r\n{body}END:STANDARD\r\nEND:VTIMEZONE\r\n");
    assert_eq!(read(&part("TZOFFSETFROM:+0100\r\nTZOFFSETTO:+0100\r\n")), missing("DTSTART"));
    assert_eq!(read(&part("DTSTART:20000101T000000\r\nTZOFFSETTO:+0100\r\n")), missing("TZOFFSETFROM"));
    assert_eq!(read(&part("DTSTART:20000101T000000\r\nTZOFFSETFROM:+0100\r\n")), missing("TZOFFSETTO"));
    let good = "DTSTART:20000101T000000\r\nTZOFFSETFROM:+0100\r\nTZOFFSETTO:+0100\r\n";
    assert_eq!(read(&part(&format!("{good}RRULE:FREQ=NEVER\r\n"))), Err(IcalError::bad_value("RRULE")));
    assert_eq!(read(&part(&format!("{good}RDATE:x\r\n"))), Err(IcalError::bad_value("RDATE")));
    assert_eq!(read(&part("DTSTART:x\r\nTZOFFSETFROM:+0100\r\nTZOFFSETTO:+0100\r\n")), Err(IcalError::bad_value("DTSTART")));
    assert!(read(&part(good)).is_ok());
}

#[test]
fn zones_turn_values_into_moments() {
    let mut zones = Zones::new();
    zones.push(zone(BUDAPEST));
    zones.push(zone(NEW_YORK));
    zones.push(zone(BUDAPEST));
    assert_eq!(zones.all().len(), 2);
    assert_eq!(zones.get("America/New_York").map(TimeZone::tzid), Some("America/New_York"));
    assert!(zones.get("Asia/Tokyo").is_none());

    let local = Offset::from_minutes(-180).unwrap();
    let nine = civil(2026, 7, 1, 9, 0);
    let zoned = |tzid: &str| When::Zoned(nine, tzid.into());
    assert_eq!(zones.resolve(&zoned("Europe/Budapest"), &local), Some(utc(2026, 7, 1, 7, 0)));
    assert_eq!(zones.resolve(&zoned("America/New_York"), &local), Some(utc(2026, 7, 1, 13, 0)));
    assert_eq!(zones.resolve(&zoned("etc/utc"), &local), Some(utc(2026, 7, 1, 9, 0)));
    // A zone no VTIMEZONE describes falls back to the person's own.
    assert_eq!(zones.resolve(&zoned("Asia/Tokyo"), &local), Some(utc(2026, 7, 1, 12, 0)));
    assert_eq!(zones.resolve(&When::Floating(nine), &local), Some(utc(2026, 7, 1, 12, 0)));
    assert_eq!(zones.resolve(&When::Date(Date::new(2026, 7, 1).unwrap()), &local), Some(utc(2026, 7, 1, 3, 0)));
    let moment = utc(2026, 7, 1, 9, 0);
    assert_eq!(zones.resolve(&When::Utc(moment), &local), Some(moment));
    assert_eq!(zones.offsets(&When::Utc(moment), &local).offset_at(moment), Offset::UTC);
}
