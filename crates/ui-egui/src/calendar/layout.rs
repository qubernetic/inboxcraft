// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Where the events go on the screen, apart from the drawing: the timed events of a day side by side where they
//! overlap ([`pieces`]), and the blocks over whole days in lanes across a row of days ([`bars`]).

use super::{DAY_MINUTES, Day, EventRow};

/// The shortest an event is laid out, in minutes, so that an event without length still takes room.
pub const SHORTEST: u16 = 15;

/// A timed event on one day: its minutes there, and its column among the events it overlaps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Piece {
    /// The event's place in the list laid out.
    pub event: usize,
    /// From this minute of the day.
    pub from: u16,
    /// To this minute of the day.
    pub to: u16,
    /// Its column, from 0.
    pub column: usize,
    /// How many columns the events it overlaps (directly or through others) take.
    pub columns: usize,
}

/// The timed events of `events` on `day` (blocks left out), side by side where they overlap: each takes the first
/// column free at its start, and every event of a group that overlaps shares the group's number of columns.
#[must_use]
pub fn pieces(events: &[&EventRow], day: Day) -> Vec<Piece> {
    let mut placed: Vec<Piece> = events
        .iter()
        .enumerate()
        .filter(|(_, event)| !event.is_block() && event.covers(day))
        .map(|(index, event)| {
            let from = if event.start.day < day { 0 } else { event.start.minute.min(DAY_MINUTES - SHORTEST) };
            let to = if event.end.day > day { DAY_MINUTES } else { event.end.minute };
            Piece { event: index, from, to: to.max(from.saturating_add(SHORTEST)).min(DAY_MINUTES), column: 0, columns: 1 }
        })
        .collect();
    placed.sort_by_key(|piece| (piece.from, std::cmp::Reverse(piece.to), piece.event));
    let mut group_start = 0;
    let mut group_end = 0_u16;
    // The minute each column is free again, in the group under way.
    let mut free: Vec<u16> = Vec::new();
    for at in 0..placed.len() {
        let Some(piece) = placed.get(at).copied() else { continue };
        if piece.from >= group_end {
            close_group(&mut placed, group_start, at, free.len());
            group_start = at;
            free.clear();
        }
        let column = free.iter().position(|until| *until <= piece.from).unwrap_or(free.len());
        match free.get_mut(column) {
            Some(until) => *until = piece.to,
            None => free.push(piece.to),
        }
        if let Some(slot) = placed.get_mut(at) {
            slot.column = column;
        }
        group_end = if at == group_start { piece.to } else { group_end.max(piece.to) };
    }
    let end = placed.len();
    close_group(&mut placed, group_start, end, free.len());
    placed
}

fn close_group(placed: &mut [Piece], from: usize, to: usize, columns: usize) {
    for piece in placed.get_mut(from..to).unwrap_or_default() {
        piece.columns = columns.max(1);
    }
}

/// A block over whole days in a row of days: the columns it covers there and its lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bar {
    /// The event's place in the list laid out.
    pub event: usize,
    /// The first column it covers in the row.
    pub from: usize,
    /// The last column it covers in the row.
    pub to: usize,
    /// Its lane, from 0 at the top.
    pub lane: usize,
    /// Whether it starts in this row (otherwise it goes on from the row before).
    pub starts: bool,
    /// Whether it ends in this row (otherwise it goes on in the next).
    pub ends: bool,
}

/// The blocks of `events` (all-day, or over more than one day) in the `days` days from `first`, each in the first
/// lane free over all its days, the earlier and longer ones first.
#[must_use]
pub fn bars(events: &[&EventRow], first: Day, days: usize) -> Vec<Bar> {
    let last = first.plus(i64::try_from(days).unwrap_or(0).saturating_sub(1));
    let column = |day: Day| usize::try_from(day.since(first)).unwrap_or(0).min(days.saturating_sub(1));
    let mut placed: Vec<Bar> = events
        .iter()
        .enumerate()
        .filter(|(_, event)| days > 0 && event.is_block() && event.last >= first && event.first <= last)
        .map(|(index, event)| Bar {
            event: index,
            from: column(event.first.max(first)),
            to: column(event.last.min(last)),
            lane: 0,
            starts: event.first >= first,
            ends: event.last <= last,
        })
        .collect();
    placed.sort_by_key(|bar| (bar.from, std::cmp::Reverse(bar.to), bar.event));
    // The last column taken in each lane.
    let mut lanes: Vec<usize> = Vec::new();
    for bar in &mut placed {
        let lane = lanes.iter().position(|taken| *taken < bar.from).unwrap_or(lanes.len());
        match lanes.get_mut(lane) {
            Some(taken) => *taken = bar.to,
            None => lanes.push(bar.to),
        }
        bar.lane = lane;
    }
    placed
}

/// How many lanes `bars` take.
#[must_use]
pub fn lanes(bars: &[Bar]) -> usize {
    bars.iter().map(|bar| bar.lane.saturating_add(1)).max().unwrap_or(0)
}

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;
