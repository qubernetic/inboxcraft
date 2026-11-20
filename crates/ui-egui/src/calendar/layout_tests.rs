// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{Bar, Piece, bars, lanes, pieces};
use crate::calendar::{Day, EventRow, Moment};

fn day(date: u8) -> Day {
    Day::new(2026, 11, date).unwrap()
}

/// A timed event on 5 November from `from` to `to` (`"09:30"`).
fn timed(from: &str, to: &str) -> EventRow {
    let at = |time: &str| Moment::parse(&format!("2026-11-05T{time}:00Z")).unwrap();
    event(at(from), at(to), day(5), day(5), false)
}

/// An all-day event from `first` to `last` November.
fn block(first: u8, last: u8) -> EventRow {
    let midnight = |date: u8| Moment { day: day(date), minute: 0 };
    event(midnight(first), midnight(last.saturating_add(1)), day(first), day(last), true)
}

fn event(start: Moment, end: Moment, first: Day, last: Day, all_day: bool) -> EventRow {
    EventRow {
        id: 1,
        calendar: 1,
        summary: "Invented".to_owned(),
        location: String::new(),
        description: String::new(),
        start,
        end,
        first,
        last,
        all_day,
        busy: true,
        private: false,
        repeat: None,
        zone: None,
        guests: Vec::new(),
        alert: crate::calendar::Alert::Default,
        untitled: false,
    }
}

fn piece(event: usize, from: u16, to: u16, column: usize, columns: usize) -> Piece {
    Piece { event, from, to, column, columns }
}

#[test]
fn overlapping_events_sit_side_by_side_and_the_rest_take_the_full_width() {
    let events = [timed("09:30", "10:30"), timed("10:00", "11:30"), timed("13:00", "15:00"), timed("10:45", "11:15"), timed("08:00", "09:00")];
    let refs: Vec<&EventRow> = events.iter().collect();
    let laid = pieces(&refs, day(5), 4).0;
    assert_eq!(
        laid,
        vec![piece(4, 480, 540, 0, 1), piece(0, 570, 630, 0, 2), piece(1, 600, 690, 1, 2), piece(3, 645, 675, 0, 2), piece(2, 780, 900, 0, 1),],
        "the call ends before the third one starts, so the third takes its column"
    );
    assert!(pieces(&refs, day(6), 4).0.is_empty(), "nothing on another day");
}

#[test]
fn an_event_without_length_still_takes_a_quarter_hour_and_one_over_midnight_is_cut_at_the_day() {
    let events = [timed("23:55", "23:55"), timed("12:00", "12:00")];
    let refs: Vec<&EventRow> = events.iter().collect();
    assert_eq!(pieces(&refs, day(5), 4).0, vec![piece(1, 720, 735, 0, 1), piece(0, 1425, 1440, 0, 1)]);
    let mut late = timed("22:00", "23:00");
    late.end = Moment { day: day(6), minute: 0 };
    let refs = [&late];
    assert_eq!(pieces(&refs, day(5), 4).0, vec![piece(0, 1320, 1440, 0, 1)], "an end at midnight ends the day");
    let mut night = timed("09:00", "10:00");
    night.start = Moment { day: day(4), minute: 1320 };
    night.first = day(4);
    night.all_day = false;
    // Starting the day before makes it a block over two days; the hours never see it.
    let refs = [&night];
    assert!(pieces(&refs, day(5), 4).0.is_empty());
}

#[test]
fn blocks_take_the_first_free_lane_and_are_cut_at_the_row() {
    let events = [block(16, 18), block(17, 17), block(18, 22), block(19, 19), timed("09:00", "10:00")];
    let refs: Vec<&EventRow> = events.iter().collect();
    let laid = bars(&refs, day(16), 7, 8).0;
    let bar = |event, from, to, lane, starts, ends| Bar { event, from, to, lane, starts, ends };
    assert_eq!(laid, vec![bar(0, 0, 2, 0, true, true), bar(1, 1, 1, 1, true, true), bar(2, 2, 6, 1, true, true), bar(3, 3, 3, 0, true, true),]);
    assert_eq!(lanes(&laid), 2);
    let next = bars(&refs, day(20), 7, 8).0;
    assert_eq!(next, vec![bar(2, 0, 2, 0, false, true)], "the long one goes on from the week before");
    let short = bars(&refs, day(16), 2, 8).0;
    assert_eq!(short.iter().find(|bar| bar.event == 0).map(|bar| (bar.to, bar.ends)), Some((1, false)));
    assert!(bars(&refs, day(16), 0, 8).0.is_empty());
    assert_eq!(lanes(&[]), 0);
}

#[test]
fn past_the_columns_and_lanes_events_are_counted_not_laid_out() {
    // 500 at one hour: four cells, the rest counted.
    let crowd: Vec<EventRow> = (0..500).map(|_| timed("09:00", "10:00")).collect();
    let refs: Vec<&EventRow> = crowd.iter().collect();
    let (laid, left_out) = pieces(&refs, day(5), 4);
    assert_eq!(laid.len(), 4);
    assert_eq!(left_out, 496);
    assert!(laid.iter().all(|piece| piece.columns == 4 && piece.column < 4));
    assert_eq!(laid.iter().map(|piece| piece.column).collect::<Vec<_>>(), vec![0, 1, 2, 3]);
    // Within the bound the layout is the one without it.
    let events = [timed("09:30", "10:30"), timed("10:00", "11:30"), timed("13:00", "15:00"), timed("10:45", "11:15")];
    let refs: Vec<&EventRow> = events.iter().collect();
    assert_eq!(pieces(&refs, day(5), 2).0, pieces(&refs, day(5), 64).0);
    let (one, out) = pieces(&refs, day(5), 1);
    assert_eq!((one.len(), out), (3, 1), "one column: the dentist overlapping the call is counted");
    // 300 blocks over the same days: three lanes, the rest counted in each of their columns.
    let blocks: Vec<EventRow> = (0..300).map(|_| block(17, 18)).collect();
    let refs: Vec<&EventRow> = blocks.iter().collect();
    let (laid, left_out) = bars(&refs, day(16), 7, 3);
    assert_eq!(laid.len(), 3);
    assert_eq!(lanes(&laid), 3);
    assert_eq!(left_out, vec![0, 297, 297, 0, 0, 0, 0]);
}
