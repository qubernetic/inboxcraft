// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The calendar follows the owner's week rule (`week.set`): the week view's days, the month grid's first cell, the
//! week's number, today and the arrows, with Monday, Sunday and Saturday as the first day and both first weeks, at the
//! turns of 2026/2027 and 2032/2033. The expected values are worked out by hand from the calendar, not by the code.

use inboxcraft_time::{FakeClock, FirstWeek, Timestamp, WeekRule, Weekday};
use serde_json::json;

use super::{Calendar, Day, View, Way};
use crate::mailbox::Mailbox;
use crate::sample;

/// Thursday 5 November 2026, 09:00 UTC.
const NOW: i64 = 1_793_869_200;

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(NOW, 0).unwrap()))
}

fn day(year: i32, month: u8, date: u8) -> Day {
    Day::new(year, month, date).unwrap()
}

const MON: Weekday = Weekday::Monday;
const SUN: Weekday = Weekday::Sunday;
const SAT: Weekday = Weekday::Saturday;
const FOUR: FirstWeek = FirstWeek::FirstFourDayWeek;
const JAN1: FirstWeek = FirstWeek::WeekOf1January;

/// The names `week.set` takes.
fn names(first_day: Weekday, first_week: FirstWeek) -> serde_json::Value {
    let day = match first_day {
        Weekday::Sunday => "sunday",
        Weekday::Saturday => "saturday",
        _ => "monday",
    };
    let week = if first_week == FOUR { "first_four_day_week" } else { "week_of_1_january" };
    json!({ "first_day": day, "first_week": week })
}

/// The sample with the week rule set through the session, as `week.set` does it, and the calendar read.
fn with_rule(first_day: Weekday, first_week: FirstWeek) -> (Mailbox, Calendar) {
    let mut mailbox = Mailbox::new(sample::session(clock()).unwrap(), clock());
    mailbox.run("week.set", &names(first_day, first_week)).unwrap();
    let mut calendar = Calendar::default();
    calendar.refresh(&mut mailbox);
    (mailbox, calendar)
}

/// A day, a rule, then the week's first day, the week's number and the month grid's first cell.
type Row = ((i32, u8, u8), Weekday, FirstWeek, (i32, u8, u8), u8, (i32, u8, u8));

const TURNS: [Row; 24] = [
    // Friday 1 January 2027.
    ((2027, 1, 1), MON, FOUR, (2026, 12, 28), 53, (2026, 12, 28)),
    ((2027, 1, 1), MON, JAN1, (2026, 12, 28), 1, (2026, 12, 28)),
    ((2027, 1, 1), SUN, FOUR, (2026, 12, 27), 52, (2026, 12, 27)),
    ((2027, 1, 1), SUN, JAN1, (2026, 12, 27), 1, (2026, 12, 27)),
    ((2027, 1, 1), SAT, FOUR, (2026, 12, 26), 52, (2026, 12, 26)),
    ((2027, 1, 1), SAT, JAN1, (2026, 12, 26), 1, (2026, 12, 26)),
    // Sunday 3 January 2027: a week of its own from Sunday or Saturday.
    ((2027, 1, 3), MON, FOUR, (2026, 12, 28), 53, (2026, 12, 28)),
    ((2027, 1, 3), MON, JAN1, (2026, 12, 28), 1, (2026, 12, 28)),
    ((2027, 1, 3), SUN, FOUR, (2027, 1, 3), 1, (2026, 12, 27)),
    ((2027, 1, 3), SUN, JAN1, (2027, 1, 3), 2, (2026, 12, 27)),
    ((2027, 1, 3), SAT, FOUR, (2027, 1, 2), 1, (2026, 12, 26)),
    ((2027, 1, 3), SAT, JAN1, (2027, 1, 2), 2, (2026, 12, 26)),
    // Friday 31 December 2032.
    ((2032, 12, 31), MON, FOUR, (2032, 12, 27), 53, (2032, 11, 29)),
    ((2032, 12, 31), MON, JAN1, (2032, 12, 27), 1, (2032, 11, 29)),
    ((2032, 12, 31), SUN, FOUR, (2032, 12, 26), 52, (2032, 11, 28)),
    ((2032, 12, 31), SUN, JAN1, (2032, 12, 26), 1, (2032, 11, 28)),
    ((2032, 12, 31), SAT, FOUR, (2032, 12, 25), 52, (2032, 11, 27)),
    ((2032, 12, 31), SAT, JAN1, (2032, 12, 25), 53, (2032, 11, 27)),
    // Saturday 1 January 2033.
    ((2033, 1, 1), MON, FOUR, (2032, 12, 27), 53, (2032, 12, 27)),
    ((2033, 1, 1), MON, JAN1, (2032, 12, 27), 1, (2032, 12, 27)),
    ((2033, 1, 1), SUN, FOUR, (2032, 12, 26), 52, (2032, 12, 26)),
    ((2033, 1, 1), SUN, JAN1, (2032, 12, 26), 1, (2032, 12, 26)),
    ((2033, 1, 1), SAT, FOUR, (2033, 1, 1), 1, (2033, 1, 1)),
    ((2033, 1, 1), SAT, JAN1, (2033, 1, 1), 1, (2033, 1, 1)),
];

#[test]
fn the_week_the_month_grid_and_the_week_number_follow_the_rule_at_the_year_s_turn() {
    for ((y, m, d), first_day, first_week, (wy, wm, wd), number, (gy, gm, gd)) in TURNS {
        let at = format!("{y}-{m}-{d} from {first_day:?}, {first_week:?}");
        let (mut mailbox, mut calendar) = with_rule(first_day, first_week);
        assert_eq!(calendar.week_rule(), WeekRule { first_day, first_week }, "{at}");
        calendar.go_to(&mut mailbox, day(y, m, d));
        calendar.set_view(&mut mailbox, View::Week);
        let (first, last) = calendar.span();
        assert_eq!((first, last), (day(wy, wm, wd), day(wy, wm, wd).plus(6)), "{at}: the week's days");
        let order: Vec<Weekday> = (0..7).map(|index| first.plus(index).weekday()).collect();
        let from = Weekday::ALL.iter().position(|weekday| *weekday == first_day).unwrap();
        let expected: Vec<Weekday> = (0..7).map(|index| Weekday::ALL[(from + index) % 7]).collect();
        assert_eq!(order, expected, "{at}: the days in order from the first day");
        assert_eq!(calendar.week_number(), Some(number), "{at}: the week's number");
        calendar.set_view(&mut mailbox, View::Month);
        assert_eq!(calendar.span().0, day(gy, gm, gd), "{at}: the month grid's first cell");
        assert_eq!(calendar.span().0.weekday(), first_day, "{at}");
        assert_eq!(calendar.span().1, day(gy, gm, gd).plus(41), "{at}: six weeks");
    }
}

#[test]
fn today_stays_in_the_week_whichever_day_it_starts_on() {
    // Thursday 5 November 2026: the week from Monday 2, Sunday 1 or Saturday 31 October, numbered by the rule.
    for (first_day, first_week, first, column, number) in [
        (MON, FOUR, day(2026, 11, 2), 3, 45),
        (MON, JAN1, day(2026, 11, 2), 3, 45),
        (SUN, FOUR, day(2026, 11, 1), 4, 44),
        (SUN, JAN1, day(2026, 11, 1), 4, 45),
        (SAT, FOUR, day(2026, 10, 31), 5, 44),
        (SAT, JAN1, day(2026, 10, 31), 5, 45),
    ] {
        let (_, calendar) = with_rule(first_day, first_week);
        let today = calendar.today();
        assert_eq!(today, day(2026, 11, 5));
        assert_eq!(calendar.focus(), today, "the first read puts the view on today");
        assert_eq!(calendar.span(), (first, first.plus(6)), "{first_day:?}");
        assert_eq!(today.since(first), column, "today's column");
        assert_eq!(calendar.week_number(), Some(number), "{first_day:?} {first_week:?}");
    }
}

#[test]
fn the_arrows_walk_the_month_in_the_drawn_order_with_sunday_first() {
    let (mut mailbox, mut calendar) = with_rule(SUN, FOUR);
    calendar.set_view(&mut mailbox, View::Month);
    // November 2026 starts on a Sunday: the grid's first cell, its first column.
    let first = calendar.span().0;
    assert_eq!(first, day(2026, 11, 1));
    let column = |day: Day| day.since(first).rem_euclid(7);
    calendar.pick_day(Some(day(2026, 11, 7)));
    assert_eq!(column(day(2026, 11, 7)), 6, "Saturday is the last column");
    calendar.move_pick(&mut mailbox, Way::Right, false);
    assert_eq!(calendar.picked_day(), Some(day(2026, 11, 8)));
    assert_eq!(column(day(2026, 11, 8)), 0, "right from the last column is the next row's first");
    calendar.move_pick(&mut mailbox, Way::Left, false);
    assert_eq!(calendar.picked_day(), Some(day(2026, 11, 7)), "and left goes back up to the row before's last");
    calendar.move_pick(&mut mailbox, Way::Down, false);
    assert_eq!(calendar.picked_day(), Some(day(2026, 11, 14)));
    assert_eq!(column(day(2026, 11, 14)), 6, "down keeps the column");
    assert_eq!(calendar.span().0, first, "the grid stays on its month");
    // The mini month walks the same way.
    calendar.pick_day(Some(day(2026, 11, 7)));
    calendar.move_pick(&mut mailbox, Way::Right, true);
    assert_eq!(calendar.picked_day(), Some(day(2026, 11, 8)));
}

#[test]
fn a_change_of_the_rule_shows_at_the_next_frame_and_keeps_the_view_and_the_pick() {
    let (mut mailbox, mut calendar) = with_rule(MON, FOUR);
    calendar.set_view(&mut mailbox, View::Week);
    assert_eq!(calendar.span().0, day(2026, 11, 2));
    // An event in both weeks, from Monday 2 and from Sunday 1 November.
    let ferry = calendar.events().map(super::EventRow::key).find(|key| (day(2026, 11, 2)..=day(2026, 11, 7)).contains(&key.start.day));
    assert!(ferry.is_some());
    calendar.pick(ferry);
    calendar.pick_day(Some(day(2026, 11, 6)));
    let serial = calendar.serial();
    calendar.follow_week(&mut mailbox);
    assert_eq!(calendar.serial(), serial, "an unchanged rule reads nothing again");
    mailbox.run("week.set", &names(SUN, JAN1)).unwrap();
    calendar.follow_week(&mut mailbox);
    assert_eq!(calendar.week_rule(), WeekRule { first_day: SUN, first_week: JAN1 });
    assert_eq!(calendar.span().0, day(2026, 11, 1), "the week from Sunday");
    assert_ne!(calendar.serial(), serial, "the days shown were read again for the new week");
    assert_eq!(calendar.focus(), day(2026, 11, 5), "the view stays on its day");
    assert_eq!(calendar.view(), View::Week);
    assert_eq!(calendar.picked().map(super::EventRow::key), ferry, "the picked event stays picked");
    assert_eq!(calendar.picked_day(), Some(day(2026, 11, 6)), "the picked day stays picked");
    let serial = calendar.serial();
    for _ in 0..3 {
        calendar.follow_week(&mut mailbox);
    }
    assert_eq!(calendar.serial(), serial, "once read, the next frames read nothing again");
}

#[test]
fn a_calendar_never_read_follows_no_rule_until_its_first_read() {
    let mut mailbox = Mailbox::new(sample::session(clock()).unwrap(), clock());
    mailbox.run("week.set", &names(SUN, JAN1)).unwrap();
    let mut calendar = Calendar::default();
    let serial = calendar.serial();
    calendar.follow_week(&mut mailbox);
    assert_eq!(calendar.serial(), serial, "a calendar not read yet reads nothing");
    assert_eq!(calendar.week_rule(), WeekRule::default());
    calendar.refresh(&mut mailbox);
    assert_eq!(calendar.week_rule(), WeekRule { first_day: SUN, first_week: JAN1 }, "the first read takes the rule");
}
