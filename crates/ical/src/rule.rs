// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Recurrence rules (RFC 5545 section 3.3.10): reading, writing and expanding them on the wall clock.

use std::collections::VecDeque;
use std::fmt;

use inboxcraft_time::{CivilDateTime, Weekday, days_in_month, is_leap_year};

use crate::value::{DAY_SECS, civil_from_seconds, civil_seconds};
use crate::{IcalError, Property, When};

/// The most values one `BY...` part of a rule may list.
pub const MAX_RULE_VALUES: usize = 1000;

/// The most periods (years, months, weeks, days, hours, minutes or seconds of the rule's frequency) a
/// [`Recurrence`] looks at for one occurrence before it gives up.
pub const MAX_PERIODS: u32 = 100_000;

/// The most candidates a [`Recurrence`] makes of one period before it gives up on the rest of the period.
pub const MAX_PER_PERIOD: usize = 100_000;

/// How often a rule repeats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Frequency {
    /// Every second.
    Secondly,
    /// Every minute.
    Minutely,
    /// Every hour.
    Hourly,
    /// Every day.
    Daily,
    /// Every week.
    Weekly,
    /// Every month.
    Monthly,
    /// Every year.
    Yearly,
}

impl Frequency {
    const ALL: [(Self, &'static str); 7] = [
        (Self::Secondly, "SECONDLY"),
        (Self::Minutely, "MINUTELY"),
        (Self::Hourly, "HOURLY"),
        (Self::Daily, "DAILY"),
        (Self::Weekly, "WEEKLY"),
        (Self::Monthly, "MONTHLY"),
        (Self::Yearly, "YEARLY"),
    ];

    fn code(self) -> &'static str {
        Self::ALL.iter().find(|(frequency, _)| *frequency == self).map_or("YEARLY", |(_, code)| code)
    }

    /// The seconds of one step below a day.
    fn unit_secs(self) -> i64 {
        match self {
            Self::Secondly => 1,
            Self::Minutely => 60,
            _ => 3600,
        }
    }
}

const WEEKDAYS: [(Weekday, &str); 7] = [
    (Weekday::Monday, "MO"),
    (Weekday::Tuesday, "TU"),
    (Weekday::Wednesday, "WE"),
    (Weekday::Thursday, "TH"),
    (Weekday::Friday, "FR"),
    (Weekday::Saturday, "SA"),
    (Weekday::Sunday, "SU"),
];

/// Monday 0 to Sunday 6.
fn weekday_index(weekday: Weekday) -> i64 {
    WEEKDAYS.iter().position(|(day, _)| *day == weekday).map_or(0, |index| index as i64)
}

fn weekday_code(weekday: Weekday) -> &'static str {
    WEEKDAYS.iter().find(|(day, _)| *day == weekday).map_or("MO", |(_, code)| code)
}

fn read_weekday(code: &str) -> Option<Weekday> {
    WEEKDAYS.iter().find(|(_, name)| name.eq_ignore_ascii_case(code)).map(|(day, _)| *day)
}

/// A `BYDAY` entry: a weekday, every one of them in the period (`ordinal` 0) or the n-th (negative: from the end).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WeekdayNum {
    /// 0 for every such weekday, 1 to 53 for the n-th, -1 to -53 for the n-th from the end.
    pub ordinal: i8,
    /// The weekday.
    pub weekday: Weekday,
}

/// How a rule ends.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum End {
    /// It does not.
    Never,
    /// After this many occurrences.
    Count(u32),
    /// With the last occurrence on or before this time.
    Until(When),
}

/// A recurrence rule (`RRULE`, or the `RRULE` of a time zone's observance).
///
/// The fields are public to build a rule; [`Rule::parse`] checks the ranges RFC 5545 gives, and expanding a rule
/// built by hand skips values out of range.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Rule {
    /// How often.
    pub frequency: Frequency,
    /// Every how many periods, from 1.
    pub interval: u32,
    /// How it ends.
    pub end: End,
    /// Seconds, 0 to 60.
    pub by_second: Vec<u8>,
    /// Minutes, 0 to 59.
    pub by_minute: Vec<u8>,
    /// Hours, 0 to 23.
    pub by_hour: Vec<u8>,
    /// Weekdays.
    pub by_day: Vec<WeekdayNum>,
    /// Days of the month, 1 to 31 or -31 to -1 from the end.
    pub by_month_day: Vec<i8>,
    /// Days of the year, 1 to 366 or -366 to -1 from the end.
    pub by_year_day: Vec<i16>,
    /// Weeks of the year, 1 to 53 or -53 to -1 from the end.
    pub by_week_no: Vec<i8>,
    /// Months, 1 to 12.
    pub by_month: Vec<u8>,
    /// Positions in the period's set, 1 to 366 or -366 to -1 from the end.
    pub by_set_pos: Vec<i16>,
    /// The day a week starts on.
    pub week_start: Weekday,
}

impl Rule {
    /// Every `frequency`, for ever, nothing else set; the week starts on Monday.
    #[must_use]
    pub fn new(frequency: Frequency) -> Self {
        Self {
            frequency,
            interval: 1,
            end: End::Never,
            by_second: Vec::new(),
            by_minute: Vec::new(),
            by_hour: Vec::new(),
            by_day: Vec::new(),
            by_month_day: Vec::new(),
            by_year_day: Vec::new(),
            by_week_no: Vec::new(),
            by_month: Vec::new(),
            by_set_pos: Vec::new(),
            week_start: Weekday::Monday,
        }
    }

    /// Reads a rule's value, `FREQ=WEEKLY;BYDAY=MO,WE` and the like. Parts are read in any case; parts it does not
    /// know are skipped.
    ///
    /// # Errors
    ///
    /// [`IcalError::BadValue`] (named `RRULE`) without `FREQ`, with both `COUNT` and `UNTIL`, with an `INTERVAL` of
    /// 0, a value out of its range or a list over [`MAX_RULE_VALUES`] values.
    pub fn parse(text: &str) -> Result<Self, IcalError> {
        read_rule(text.trim()).ok_or_else(|| IcalError::bad_value("RRULE"))
    }

    /// The `RRULE` of `property`.
    ///
    /// # Errors
    ///
    /// As [`Rule::parse`], named after the property.
    pub fn from_property(property: &Property) -> Result<Self, IcalError> {
        read_rule(property.value().trim()).ok_or_else(|| IcalError::bad_value(property.name()))
    }

    /// The occurrences on the wall clock from `start` (the `DTSTART`, which counts only when the rule makes it), in
    /// order, the `COUNT` kept; `UNTIL` is the caller's (see [`Recurrence::stop_after`]).
    #[must_use]
    pub fn iter(&self, start: CivilDateTime) -> Recurrence<'_> {
        Recurrence::new(self, start)
    }
}

fn read_list<T: std::str::FromStr>(value: &str, valid: impl Fn(&T) -> bool) -> Option<Vec<T>> {
    let values: Vec<T> = value.split(',').map(|part| part.trim().parse().ok().filter(&valid)).collect::<Option<_>>()?;
    (values.len() <= MAX_RULE_VALUES).then_some(values)
}

fn read_weekday_num(text: &str) -> Option<WeekdayNum> {
    let split = text.len().checked_sub(2)?;
    let weekday = read_weekday(text.get(split..)?)?;
    let number = text.get(..split)?;
    let ordinal = if number.is_empty() { 0 } else { number.parse::<i8>().ok().filter(|n| *n != 0 && n.unsigned_abs() <= 53)? };
    Some(WeekdayNum { ordinal, weekday })
}

fn read_rule(text: &str) -> Option<Rule> {
    let mut frequency = None;
    let mut rule = Rule::new(Frequency::Yearly);
    let (mut count, mut until) = (None, None);
    for part in text.split(';').filter(|part| !part.is_empty()) {
        let (name, value) = part.split_once('=')?;
        match name.trim().to_ascii_uppercase().as_str() {
            "FREQ" => frequency = Some(Frequency::ALL.iter().find(|(_, code)| code.eq_ignore_ascii_case(value))?.0),
            "INTERVAL" => rule.interval = value.parse().ok().filter(|interval| *interval > 0)?,
            "COUNT" => count = Some(value.parse().ok()?),
            "UNTIL" => until = Some(When::from_property(&Property::new("UNTIL", value)).ok()?),
            "BYSECOND" => rule.by_second = read_list(value, |n| *n <= 60)?,
            "BYMINUTE" => rule.by_minute = read_list(value, |n| *n <= 59)?,
            "BYHOUR" => rule.by_hour = read_list(value, |n| *n <= 23)?,
            "BYDAY" => {
                rule.by_day = value.split(',').map(|day| read_weekday_num(day.trim())).collect::<Option<_>>()?;
                if rule.by_day.len() > MAX_RULE_VALUES {
                    return None;
                }
            }
            "BYMONTHDAY" => rule.by_month_day = read_list(value, |n: &i8| *n != 0 && n.unsigned_abs() <= 31)?,
            "BYYEARDAY" => rule.by_year_day = read_list(value, |n: &i16| *n != 0 && n.unsigned_abs() <= 366)?,
            "BYWEEKNO" => rule.by_week_no = read_list(value, |n: &i8| *n != 0 && n.unsigned_abs() <= 53)?,
            "BYMONTH" => rule.by_month = read_list(value, |n| (1..=12).contains(n))?,
            "BYSETPOS" => rule.by_set_pos = read_list(value, |n: &i16| *n != 0 && n.unsigned_abs() <= 366)?,
            "WKST" => rule.week_start = read_weekday(value.trim())?,
            _ => {}
        }
    }
    rule.frequency = frequency?;
    rule.end = match (count, until) {
        (Some(_), Some(_)) => return None,
        (Some(count), None) => End::Count(count),
        (None, Some(until)) => End::Until(until),
        (None, None) => End::Never,
    };
    Some(rule)
}

fn write_list<T: fmt::Display>(f: &mut fmt::Formatter<'_>, name: &str, values: &[T]) -> fmt::Result {
    if values.is_empty() {
        return Ok(());
    }
    write!(f, ";{name}=")?;
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            f.write_str(",")?;
        }
        write!(f, "{value}")?;
    }
    Ok(())
}

impl fmt::Display for WeekdayNum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.ordinal != 0 {
            write!(f, "{}", self.ordinal)?;
        }
        f.write_str(weekday_code(self.weekday))
    }
}

/// The rule as a value, `UNTIL` in the form it has (a zoned one written as on the wall clock).
impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FREQ={}", self.frequency.code())?;
        match &self.end {
            End::Never => {}
            End::Count(count) => write!(f, ";COUNT={count}")?,
            End::Until(until) => write!(f, ";UNTIL={}", until.to_property("UNTIL").value())?,
        }
        if self.interval != 1 {
            write!(f, ";INTERVAL={}", self.interval)?;
        }
        write_list(f, "BYSECOND", &self.by_second)?;
        write_list(f, "BYMINUTE", &self.by_minute)?;
        write_list(f, "BYHOUR", &self.by_hour)?;
        write_list(f, "BYDAY", &self.by_day)?;
        write_list(f, "BYMONTHDAY", &self.by_month_day)?;
        write_list(f, "BYYEARDAY", &self.by_year_day)?;
        write_list(f, "BYWEEKNO", &self.by_week_no)?;
        write_list(f, "BYMONTH", &self.by_month)?;
        write_list(f, "BYSETPOS", &self.by_set_pos)?;
        if self.week_start != Weekday::Monday {
            write!(f, ";WKST={}", weekday_code(self.week_start))?;
        }
        Ok(())
    }
}

/// Days since 1970-01-01 of a date, or `None` when it does not exist.
fn day_number(year: i32, month: u8, day: u8) -> Option<i64> {
    Some(civil_seconds(CivilDateTime::new(year, month, day, 0, 0, 0)?).div_euclid(DAY_SECS))
}

fn date_of(day: i64) -> Option<CivilDateTime> {
    civil_from_seconds(day.checked_mul(DAY_SECS)?)
}

/// Monday 0 to Sunday 6 of a day number; 1970-01-01 was a Thursday.
fn weekday_of(day: i64) -> i64 {
    day.saturating_add(3).rem_euclid(7)
}

/// The first day of the week that holds `day`.
fn week_start_of(day: i64, week_start: Weekday) -> i64 {
    day.saturating_sub((weekday_of(day) - weekday_index(week_start)).rem_euclid(7))
}

/// The first day of week 1 of `year`: the first week with at least four days in the year.
fn week_one(year: i32, week_start: Weekday) -> Option<i64> {
    let first = day_number(year, 1, 1)?;
    let start = week_start_of(first, week_start);
    Some(if first - start <= 3 { start } else { start.saturating_add(7) })
}

fn year_length(year: i32) -> i64 {
    if is_leap_year(year) { 366 } else { 365 }
}

/// Whether a 1-based `position` in a run of `length` is named by `value` (negative counts from the end).
fn names(value: i64, position: i64, length: i64) -> bool {
    if value > 0 { value == position } else { length.saturating_add(1).saturating_add(value) == position }
}

/// The lists a rule expands with, the defaults from `DTSTART` filled in (RFC 5545 section 3.3.10).
#[derive(Debug, Clone)]
struct Plan {
    months: Vec<u8>,
    month_days: Vec<i8>,
    days: Vec<WeekdayNum>,
    hours: Vec<u8>,
    minutes: Vec<u8>,
    seconds: Vec<u8>,
}

impl Plan {
    fn new(rule: &Rule, start: CivilDateTime) -> Self {
        let mut plan = Self {
            months: rule.by_month.clone(),
            month_days: rule.by_month_day.clone(),
            days: rule.by_day.clone(),
            hours: rule.by_hour.clone(),
            minutes: rule.by_minute.clone(),
            seconds: rule.by_second.clone(),
        };
        let no_days = rule.by_week_no.is_empty() && rule.by_year_day.is_empty() && plan.month_days.is_empty() && plan.days.is_empty();
        let day = i8::try_from(start.day()).unwrap_or(1);
        let weekday = start.weekday();
        match rule.frequency {
            Frequency::Yearly if no_days => {
                if plan.months.is_empty() {
                    plan.months.push(start.month());
                }
                plan.month_days.push(day);
            }
            Frequency::Monthly if no_days => plan.month_days.push(day),
            Frequency::Weekly if no_days => plan.days.push(WeekdayNum { ordinal: 0, weekday }),
            _ => {}
        }
        if rule.frequency > Frequency::Hourly && plan.hours.is_empty() {
            plan.hours.push(start.hour());
        }
        if rule.frequency > Frequency::Minutely && plan.minutes.is_empty() {
            plan.minutes.push(start.minute());
        }
        if rule.frequency > Frequency::Secondly && plan.seconds.is_empty() {
            plan.seconds.push(start.second());
        }
        for list in [&mut plan.months, &mut plan.hours, &mut plan.minutes, &mut plan.seconds] {
            list.sort_unstable();
            list.dedup();
        }
        plan
    }
}

/// The occurrences of a [`Rule`] on the wall clock, in order (see [`Rule::iter`]).
///
/// It ends with the rule's `COUNT`, after [`Recurrence::stop_after`], past year 9999, when [`MAX_PERIODS`] periods
/// in a row bring nothing, or when its work passes a [`Recurrence::with_budget`]; [`Recurrence::truncated`] tells the
/// last two apart from a rule that has ended, and also says when a period had more than [`MAX_PER_PERIOD`]
/// candidates.
#[derive(Debug, Clone)]
pub struct Recurrence<'a> {
    rule: &'a Rule,
    plan: Plan,
    start: CivilDateTime,
    /// The index of the next period to expand.
    period: i64,
    pending: VecDeque<CivilDateTime>,
    made: u32,
    stop: Option<CivilDateTime>,
    done: bool,
    truncated: bool,
    work: usize,
    budget: usize,
}

impl<'a> Recurrence<'a> {
    fn new(rule: &'a Rule, start: CivilDateTime) -> Self {
        let plan = Plan::new(rule, start);
        Self {
            rule,
            plan,
            start,
            period: 0,
            pending: VecDeque::new(),
            made: 0,
            stop: None,
            done: false,
            truncated: false,
            work: 0,
            budget: usize::MAX,
        }
    }

    /// The occurrences up to and including `last` only (a rule's `UNTIL` on this wall clock, or the end of a view).
    #[must_use]
    pub fn stop_after(mut self, last: CivilDateTime) -> Self {
        self.stop = Some(last);
        self
    }

    /// Ends it, truncated, before a period once its [`Recurrence::work`] has come to `budget`.
    #[must_use]
    pub fn with_budget(mut self, budget: usize) -> Self {
        self.budget = budget;
        self
    }

    /// The work it has done: one for each period it looked at, and one for each day of a period and each candidate
    /// it made. A period holds at most 371 days and [`MAX_PER_PERIOD`] candidates.
    #[must_use]
    pub fn work(&self) -> usize {
        self.work
    }

    /// Whether it ended early: a limit, not the rule, ended it or left candidates out.
    #[must_use]
    pub fn truncated(&self) -> bool {
        self.truncated
    }

    /// Jumps to the period that holds `at`, so occurrences before it need not be made; it does nothing for a rule
    /// with a `COUNT`, which has to count them. Occurrences of that period before `at` can still come.
    pub fn skip_to(&mut self, at: CivilDateTime) {
        if matches!(self.rule.end, End::Count(_)) {
            return;
        }
        if let Some(period) = self.period_of(at).filter(|period| *period > self.period) {
            self.period = period;
            self.pending.clear();
        }
    }

    fn interval(&self) -> i64 {
        i64::from(self.rule.interval.max(1))
    }

    fn step_secs(&self) -> i64 {
        self.rule.frequency.unit_secs().saturating_mul(self.interval())
    }

    /// The wall-clock second the first sub-daily period starts on.
    fn base_secs(&self) -> i64 {
        let secs = civil_seconds(self.start);
        secs - secs.rem_euclid(self.rule.frequency.unit_secs())
    }

    fn start_day(&self) -> i64 {
        civil_seconds(self.start).div_euclid(DAY_SECS)
    }

    /// The index of the period that holds `at`.
    fn period_of(&self, at: CivilDateTime) -> Option<i64> {
        let day = civil_seconds(at).div_euclid(DAY_SECS);
        let units = match self.rule.frequency {
            Frequency::Yearly => i64::from(at.year()) - i64::from(self.start.year()),
            Frequency::Monthly => (i64::from(at.year()) - i64::from(self.start.year())) * 12 + i64::from(at.month()) - i64::from(self.start.month()),
            Frequency::Weekly => (week_start_of(day, self.rule.week_start) - week_start_of(self.start_day(), self.rule.week_start)).div_euclid(7),
            Frequency::Daily => day - self.start_day(),
            _ => return Some((civil_seconds(at) - self.base_secs()).div_euclid(self.step_secs())),
        };
        Some(units.div_euclid(self.interval()))
    }

    /// The days of period `index` (day numbers, ascending), its year and, below a day, its first second; `None` past
    /// year 9999.
    fn period_days(&self, index: i64) -> Option<(Vec<i64>, i32, Option<CivilDateTime>)> {
        let steps = index.checked_mul(self.interval())?;
        match self.rule.frequency {
            Frequency::Yearly => {
                let year = i32::try_from(i64::from(self.start.year()).checked_add(steps)?).ok()?;
                Some((self.year_days(year)?, year, None))
            }
            Frequency::Monthly => {
                let months = i64::from(self.start.year()).checked_mul(12)?.checked_add(i64::from(self.start.month()) - 1)?.checked_add(steps)?;
                let year = i32::try_from(months.div_euclid(12)).ok()?;
                let month = u8::try_from(months.rem_euclid(12) + 1).ok()?;
                let first = day_number(year, month, 1)?;
                Some(((first..first + i64::from(days_in_month(year, month))).collect(), year, None))
            }
            Frequency::Weekly => {
                let first = week_start_of(self.start_day(), self.rule.week_start).checked_add(steps.checked_mul(7)?)?;
                let year = date_of(first)?.year();
                Some(((first..first + 7).collect(), year, None))
            }
            Frequency::Daily => {
                let day = self.start_day().checked_add(steps)?;
                Some((vec![day], date_of(day)?.year(), None))
            }
            _ => {
                let secs = self.base_secs().checked_add(index.checked_mul(self.step_secs())?)?;
                let day = secs.div_euclid(DAY_SECS);
                let at = civil_from_seconds(secs)?;
                Some((vec![day], at.year(), Some(at)))
            }
        }
    }

    /// The days of a yearly period: the plan's months, or the weeks of the year for `BYWEEKNO`, or the whole year.
    fn year_days(&self, year: i32) -> Option<Vec<i64>> {
        if !self.rule.by_week_no.is_empty() {
            let first = week_one(year, self.rule.week_start)?;
            let next = week_one(year.checked_add(1)?, self.rule.week_start).unwrap_or(first + 371);
            return Some((first..next).collect());
        }
        if self.plan.months.is_empty() {
            let first = day_number(year, 1, 1)?;
            return Some((first..first + year_length(year)).collect());
        }
        let mut days = Vec::new();
        for month in self.plan.months.iter().copied().filter(|month| (1..=12).contains(month)) {
            let first = day_number(year, month, 1)?;
            days.extend(first..first + i64::from(days_in_month(year, month)));
        }
        Some(days)
    }

    /// Whether `day` passes the rule's day filters in a period of `year`.
    fn day_matches(&self, day: i64, year: i32) -> bool {
        let Some(date) = date_of(day) else { return false };
        let rule = self.rule;
        let in_month = i64::from(date.day());
        let month_length = i64::from(days_in_month(date.year(), date.month()));
        let year_day = day - day_number(date.year(), 1, 1).unwrap_or(day) + 1;
        let year_len = year_length(date.year());
        if !self.plan.months.is_empty() && !self.plan.months.contains(&date.month()) {
            return false;
        }
        if rule.frequency == Frequency::Yearly && !rule.by_week_no.is_empty() && !self.week_matches(day, year) {
            return false;
        }
        if !rule.by_year_day.is_empty() && !rule.by_year_day.iter().any(|value| names(i64::from(*value), year_day, year_len)) {
            return false;
        }
        if !self.plan.month_days.is_empty() && !self.plan.month_days.iter().any(|value| names(i64::from(*value), in_month, month_length)) {
            return false;
        }
        if self.plan.days.is_empty() {
            return true;
        }
        let (scope_day, scope_len) = match rule.frequency {
            Frequency::Monthly => (in_month, month_length),
            Frequency::Yearly if !rule.by_month.is_empty() => (in_month, month_length),
            Frequency::Yearly => (year_day, year_len),
            _ => (0, 0),
        };
        let weekday = weekday_of(day);
        self.plan.days.iter().any(|entry| {
            let nth = scope_len > 0 && entry.ordinal != 0;
            weekday_index(entry.weekday) == weekday
                && (!nth || names(i64::from(entry.ordinal), (scope_day - 1) / 7 + 1, (scope_len - scope_day) / 7 + (scope_day - 1) / 7 + 1))
        })
    }

    fn week_matches(&self, day: i64, year: i32) -> bool {
        let (Some(first), Some(next)) =
            (week_one(year, self.rule.week_start), year.checked_add(1).and_then(|next| week_one(next, self.rule.week_start)))
        else {
            return false;
        };
        let week = (day - first).div_euclid(7) + 1;
        let weeks = (next - first) / 7;
        self.rule.by_week_no.iter().any(|value| names(i64::from(*value), week, weeks))
    }

    /// The times of day of a period: the plan's, or below a day the period's own when it passes the filters.
    fn times(&self, period_start: Option<CivilDateTime>) -> Vec<(u8, u8, u8)> {
        let Some(at) = period_start else {
            return self
                .plan
                .hours
                .iter()
                .flat_map(|h| self.plan.minutes.iter().flat_map(move |m| self.plan.seconds.iter().map(move |s| (*h, *m, *s))))
                .collect();
        };
        let passes = |list: &[u8], value: u8| list.is_empty() || list.contains(&value);
        if !passes(&self.rule.by_hour, at.hour()) {
            return Vec::new();
        }
        let minutes = if self.rule.frequency == Frequency::Hourly { self.plan.minutes.clone() } else { vec![at.minute()] };
        if !passes(&self.rule.by_minute, at.minute()) && self.rule.frequency < Frequency::Hourly {
            return Vec::new();
        }
        let seconds = if self.rule.frequency == Frequency::Secondly { vec![at.second()] } else { self.plan.seconds.clone() };
        if !passes(&self.rule.by_second, at.second()) && self.rule.frequency == Frequency::Secondly {
            return Vec::new();
        }
        minutes.iter().flat_map(|m| seconds.iter().map(move |s| (at.hour(), *m, *s))).collect()
    }

    /// Below a day: the next second worth looking at when the period at `secs` cannot hold an occurrence (its day,
    /// hour or minute is filtered out), or `None` when it can.
    fn skip_from(&self, secs: i64) -> Option<i64> {
        let year = civil_from_seconds(secs)?.year();
        let day = secs.div_euclid(DAY_SECS);
        if !self.day_matches(day, year) {
            return day.checked_add(1)?.checked_mul(DAY_SECS);
        }
        let at = civil_from_seconds(secs)?;
        if !self.rule.by_hour.is_empty() && !self.rule.by_hour.contains(&at.hour()) {
            return Some(secs - secs.rem_euclid(3600) + 3600);
        }
        if self.rule.frequency == Frequency::Secondly && !self.rule.by_minute.is_empty() && !self.rule.by_minute.contains(&at.minute()) {
            return Some(secs - secs.rem_euclid(60) + 60);
        }
        None
    }

    /// The candidates of period `index`, `BYSETPOS` applied, in order; `None` past year 9999.
    fn candidates(&mut self, index: i64) -> Option<Vec<CivilDateTime>> {
        let (days, year, period_start) = self.period_days(index)?;
        let prune = self.rule.by_set_pos.is_empty();
        let first_day = self.start_day();
        let last_day = self.stop.map(|stop| civil_seconds(stop).div_euclid(DAY_SECS));
        if prune && days.first().is_some_and(|first| last_day.is_some_and(|last| *first > last)) {
            return Some(Vec::new());
        }
        let times = self.times(period_start);
        self.work = self.work.saturating_add(days.len());
        let mut found = Vec::new();
        for day in days {
            if prune && (day < first_day || last_day.is_some_and(|last| day > last)) || !self.day_matches(day, year) {
                continue;
            }
            let Some(date) = date_of(day) else { continue };
            for (hour, minute, second) in &times {
                if found.len() >= MAX_PER_PERIOD {
                    self.truncated = true;
                    break;
                }
                found.extend(CivilDateTime::new(date.year(), date.month(), date.day(), *hour, *minute, *second));
            }
        }
        self.work = self.work.saturating_add(found.len());
        if prune {
            return Some(found);
        }
        let count = i64::try_from(found.len()).unwrap_or(i64::MAX);
        let mut chosen: Vec<CivilDateTime> = self
            .rule
            .by_set_pos
            .iter()
            .filter_map(|position| {
                let position = i64::from(*position);
                let index = if position > 0 { position - 1 } else { count + position };
                usize::try_from(index).ok().and_then(|index| found.get(index).copied())
            })
            .collect();
        chosen.sort_unstable();
        chosen.dedup();
        Some(chosen)
    }

    /// Fills `pending` from the next periods; false when the recurrence has ended.
    fn refill(&mut self) -> bool {
        for _ in 0..MAX_PERIODS {
            if self.work >= self.budget {
                self.truncated = true;
                return false;
            }
            self.work = self.work.saturating_add(1);
            let index = self.period;
            if self.rule.frequency < Frequency::Daily {
                let secs = self.base_secs().saturating_add(index.saturating_mul(self.step_secs()));
                if let Some(target) = self.skip_from(secs) {
                    let steps = (target - self.base_secs() + self.step_secs() - 1).div_euclid(self.step_secs());
                    self.period = steps.max(index + 1);
                    if self.past_stop(target) {
                        return false;
                    }
                    continue;
                }
            }
            self.period = index.saturating_add(1);
            let Some(candidates) = self.candidates(index) else { return false };
            let start = self.start;
            self.pending.extend(candidates.into_iter().filter(|at| *at >= start));
            if !self.pending.is_empty() {
                return true;
            }
            if self.period_days(index).and_then(|(days, ..)| days.first().copied()).is_some_and(|day| self.past_stop(day.saturating_mul(DAY_SECS))) {
                return false;
            }
        }
        self.truncated = true;
        false
    }

    fn past_stop(&self, secs: i64) -> bool {
        self.stop.is_some_and(|stop| secs > civil_seconds(stop))
    }
}

impl Iterator for Recurrence<'_> {
    type Item = CivilDateTime;

    fn next(&mut self) -> Option<CivilDateTime> {
        if self.done {
            return None;
        }
        if let End::Count(count) = self.rule.end
            && self.made >= count
        {
            self.done = true;
            return None;
        }
        if self.pending.is_empty() && !self.refill() {
            self.done = true;
            return None;
        }
        let next = self.pending.pop_front()?;
        if self.stop.is_some_and(|stop| next > stop) {
            self.done = true;
            return None;
        }
        self.made = self.made.saturating_add(1);
        Some(next)
    }
}

#[cfg(test)]
#[path = "rule_tests.rs"]
mod tests;
