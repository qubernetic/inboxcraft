// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What the views lay out of the events, kept from one frame to the next: the hours' cells and bands, a month week's
//! bands and chips, the agenda's rows. Each view keeps one layout, for the events' [`Calendar::serial`] and the days in
//! sight; a new read, a calendar shown or hidden, or other days lay it out again. Every list is bounded where it is
//! made (the columns, the lanes, the chips and rows a day shows): what is past a bound is only counted.

use std::sync::Arc;

use egui::{Context, Id};

use crate::calendar::layout::{self, Bar, Piece};
use crate::calendar::{Calendar, Day, EventRow};

/// The hours of `days` days: per day its cells within the columns and how many it leaves out; the bands within the
/// lanes, and per day how many it leaves out.
#[derive(Debug, Default)]
pub(super) struct Hours {
    pub pieces: Vec<(Vec<Piece>, usize)>,
    pub bars: Vec<Bar>,
    pub bands_left: Vec<usize>,
}

/// A week of the month: its bands within the lanes and per day how many it leaves out; per day its first timed events
/// (at most [`MOST_CHIPS`]) and how many it holds.
#[derive(Debug, Default)]
pub(super) struct MonthWeek {
    pub bars: Vec<Bar>,
    pub bands_left: Vec<usize>,
    pub timed: Vec<(Vec<usize>, usize)>,
}

/// The most chips of a day kept for the month: more than a cell shows.
pub(super) const MOST_CHIPS: usize = 64;

/// The agenda's days: per day its first events (at most `most`) and how many it holds.
#[derive(Debug, Default)]
pub(super) struct Agenda {
    pub days: Vec<(Vec<usize>, usize)>,
}

/// What a layout was made for: the events, the first day, the days, and the bound.
type Key = (u64, Day, usize, usize);

/// The layout kept under `slot` when it was made for `key`, else `make`'s, kept in its place.
fn kept<T: Send + Sync + 'static>(ctx: &Context, slot: Id, key: Key, make: impl FnOnce() -> T) -> Arc<T> {
    if let Some((held, laid)) = ctx.data(|data| data.get_temp::<(Key, Arc<T>)>(slot))
        && held == key
    {
        return laid;
    }
    let laid = Arc::new(make());
    ctx.data_mut(|data| data.insert_temp(slot, (key, Arc::clone(&laid))));
    laid
}

/// The hours of the `days` days from `first`, at most `columns` side by side and `lanes` lanes of bands.
pub(super) fn hours(
    ctx: &Context,
    calendar: &Calendar,
    events: &[&EventRow],
    (first, days): (Day, usize),
    (columns, lanes): (usize, usize),
) -> Arc<Hours> {
    let key = (calendar.serial(), first, days, columns.saturating_mul(16).saturating_add(lanes));
    kept(ctx, Id::unique("calendar-laid-hours"), key, || {
        let pieces = (0..days).map(|index| layout::pieces(events, first.plus(i64::try_from(index).unwrap_or(0)), columns)).collect();
        let (bars, bands_left) = layout::bars(events, first, days, lanes);
        Hours { pieces, bars, bands_left }
    })
}

/// Week `week` of the month from `start`, at most `lanes` lanes of bands.
pub(super) fn month_week(ctx: &Context, calendar: &Calendar, events: &[&EventRow], (week, start): (usize, Day), lanes: usize) -> Arc<MonthWeek> {
    let key = (calendar.serial(), start, 7, lanes);
    kept(ctx, Id::unique(("calendar-laid-month", week)), key, || {
        let (bars, bands_left) = layout::bars(events, start, 7, lanes);
        let timed = (0..7_i64)
            .map(|index| {
                let day = start.plus(index);
                let mut held = events.iter().enumerate().filter(|(_, event)| !event.is_block() && event.covers(day));
                let first: Vec<usize> = held.by_ref().take(MOST_CHIPS).map(|(at, _)| at).collect();
                let count = first.len().saturating_add(held.count());
                (first, count)
            })
            .collect();
        MonthWeek { bars, bands_left, timed }
    })
}

/// The `days` days of the agenda from `first`, at most `most` rows a day.
pub(super) fn agenda(ctx: &Context, calendar: &Calendar, events: &[&EventRow], (first, days): (Day, usize), most: usize) -> Arc<Agenda> {
    let key = (calendar.serial(), first, days, most);
    kept(ctx, Id::unique("calendar-laid-agenda"), key, || {
        let days = (0..days)
            .map(|index| {
                let day = first.plus(i64::try_from(index).unwrap_or(0));
                let mut held = events.iter().enumerate().filter(|(_, event)| event.covers(day));
                let rows: Vec<usize> = held.by_ref().take(most).map(|(at, _)| at).collect();
                let count = rows.len().saturating_add(held.count());
                (rows, count)
            })
            .collect();
        Agenda { days }
    })
}
