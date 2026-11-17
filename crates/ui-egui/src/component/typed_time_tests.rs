// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use inboxcraft_time::{CivilDateTime, Offset, Timestamp};

use super::{MAX_TYPED, long, parse, short};

/// A UTC time.
fn at(year: i32, month: u8, day: u8, hour: u8, minute: u8) -> Timestamp {
    CivilDateTime::new(year, month, day, hour, minute, 0).and_then(|civil| civil.to_timestamp(Offset::UTC)).unwrap_or(Timestamp::MIN)
}

/// Friday 6 November 2026, 14:30 UTC.
fn now() -> Timestamp {
    at(2026, 11, 6, 14, 30)
}

fn utc(text: &str) -> Option<Timestamp> {
    parse(text, now(), Offset::UTC)
}

#[test]
fn a_weekday_and_an_hour_is_the_next_such_day_at_that_hour() {
    assert_eq!(utc("thu 9"), Some(at(2026, 11, 12, 9, 0)));
    assert_eq!(utc("Thursday 9:30"), Some(at(2026, 11, 12, 9, 30)));
    assert_eq!(utc("  THU   9  "), Some(at(2026, 11, 12, 9, 0)));
    assert_eq!(utc("9 thurs"), Some(at(2026, 11, 12, 9, 0)));
}

#[test]
fn todays_weekday_is_today_while_its_time_is_ahead_and_next_week_after() {
    assert_eq!(utc("fri 16"), Some(at(2026, 11, 6, 16, 0)));
    assert_eq!(utc("fri 9"), Some(at(2026, 11, 13, 9, 0)));
    assert_eq!(utc("fri"), Some(at(2026, 11, 13, 8, 0)), "a day without a time is its morning, 08:00");
}

#[test]
fn a_time_alone_is_today_while_it_is_ahead_and_tomorrow_after() {
    assert_eq!(utc("17:30"), Some(at(2026, 11, 6, 17, 30)));
    assert_eq!(utc("17.30"), Some(at(2026, 11, 6, 17, 30)));
    assert_eq!(utc("9"), Some(at(2026, 11, 7, 9, 0)));
    assert_eq!(utc("14:30"), Some(at(2026, 11, 7, 14, 30)), "now itself is not ahead");
    assert_eq!(utc("5pm"), Some(at(2026, 11, 6, 17, 0)));
    assert_eq!(utc("5 PM"), Some(at(2026, 11, 6, 17, 0)));
    assert_eq!(utc("5:15am"), Some(at(2026, 11, 7, 5, 15)));
    assert_eq!(utc("12am"), Some(at(2026, 11, 7, 0, 0)));
    assert_eq!(utc("12pm"), Some(at(2026, 11, 7, 12, 0)));
    assert_eq!(utc("noon"), Some(at(2026, 11, 7, 12, 0)));
    assert_eq!(utc("midnight"), Some(at(2026, 11, 7, 0, 0)));
}

#[test]
fn today_and_tomorrow_name_their_day() {
    assert_eq!(utc("tomorrow"), Some(at(2026, 11, 7, 8, 0)));
    assert_eq!(utc("tomorrow 9:30"), Some(at(2026, 11, 7, 9, 30)));
    assert_eq!(utc("tmrw 6pm"), Some(at(2026, 11, 7, 18, 0)));
    assert_eq!(utc("today 18"), Some(at(2026, 11, 6, 18, 0)));
    assert_eq!(utc("today"), None, "this morning is behind");
    assert_eq!(utc("today 9"), None);
}

#[test]
fn a_day_and_a_month_is_the_next_such_date() {
    assert_eq!(utc("12 nov"), Some(at(2026, 11, 12, 8, 0)));
    assert_eq!(utc("nov 12 9"), Some(at(2026, 11, 12, 9, 0)));
    assert_eq!(utc("12 November 9:30"), Some(at(2026, 11, 12, 9, 30)));
    assert_eq!(utc("1 jan"), Some(at(2027, 1, 1, 8, 0)));
    assert_eq!(utc("6 nov 10"), Some(at(2027, 11, 6, 10, 0)), "this year's is behind");
    assert_eq!(utc("6 nov 16"), Some(at(2026, 11, 6, 16, 0)));
    assert_eq!(utc("29 feb"), Some(at(2028, 2, 29, 8, 0)), "the next year that has one");
    assert_eq!(utc("31 feb"), None);
    assert_eq!(utc("0 nov"), None);
}

#[test]
fn an_iso_date_is_that_date() {
    assert_eq!(utc("2026-11-20"), Some(at(2026, 11, 20, 8, 0)));
    assert_eq!(utc("2026-11-20 9:15"), Some(at(2026, 11, 20, 9, 15)));
    assert_eq!(utc("2025-01-01"), None, "behind");
    assert_eq!(utc("2026-02-30"), None);
    assert_eq!(utc("2026-1-x"), None);
}

#[test]
fn in_a_number_of_units_counts_from_now_to_the_minute() {
    assert_eq!(utc("in 3 days"), Some(at(2026, 11, 9, 14, 30)));
    assert_eq!(utc("in 1 day"), Some(at(2026, 11, 7, 14, 30)));
    assert_eq!(utc("in 2 hours"), Some(at(2026, 11, 6, 16, 30)));
    assert_eq!(utc("in 45 min"), Some(at(2026, 11, 6, 15, 15)));
    assert_eq!(utc("in 1 week"), Some(at(2026, 11, 13, 14, 30)));
    assert_eq!(utc("3d"), Some(at(2026, 11, 9, 14, 30)));
    assert_eq!(utc("2h"), Some(at(2026, 11, 6, 16, 30)));
    assert_eq!(utc("45m"), Some(at(2026, 11, 6, 15, 15)));
    assert_eq!(utc("2w"), Some(at(2026, 11, 20, 14, 30)));
    let seconds = now().checked_add(std::time::Duration::from_secs(25)).unwrap_or(Timestamp::MIN);
    assert_eq!(parse("in 1 hour", seconds, Offset::UTC), Some(at(2026, 11, 6, 15, 30)), "the seconds are dropped");
}

#[test]
fn what_is_not_a_time_is_nothing() {
    for text in [
        "",
        "   ",
        "hello",
        "25",
        "9:60",
        "24:00",
        "13pm",
        "0am",
        "thu fri",
        "thu 9 10",
        "12 nov dec",
        "in",
        "in 3",
        "in days",
        "in 0 days",
        "in 3 fortnights",
        "in 99999999999 days",
        "in 999999 weeks",
        "3x",
        "9::30",
        ":30",
        "9:3",
        "9:300",
        "pm",
        "th",
        "2026-11-20-1",
        "thu 9 extra",
    ] {
        assert_eq!(utc(text), None, "{text:?}");
    }
}

#[test]
fn hostile_text_is_nothing_and_never_panics() {
    let long_text = "9 ".repeat(MAX_TYPED);
    assert_eq!(utc(&long_text), None);
    for text in ["\u{0}", "thu\u{0}9", "ﬁ 9", "９", "thü 9", "12 nov\u{202E}", "-9", "+9", "in -3 days", "\u{1F600}"] {
        assert_eq!(utc(text), None, "{text:?}");
    }
    assert_eq!(parse("in 10 days", Timestamp::MAX, Offset::UTC), None);
    assert_eq!(parse("tomorrow", Timestamp::MAX, Offset::UTC), None);
    assert_eq!(parse("1 jan", Timestamp::MAX, Offset::UTC), None);
    assert_eq!(parse("thu 9", Timestamp::MIN, Offset::UTC).map(|t| t > Timestamp::MIN), Some(true));
}

#[test]
fn a_parsed_time_is_always_ahead_of_now() {
    for text in ["9", "23:59", "0:00", "fri", "sat", "today 23", "6 nov", "6 nov 23:59", "in 1 min", "1m"] {
        if let Some(time) = utc(text) {
            assert!(time > now(), "{text:?}");
        }
    }
}

#[test]
fn the_wall_clock_is_the_given_offsets() {
    let budapest = Offset::from_minutes(60).unwrap_or(Offset::UTC);
    assert_eq!(parse("thu 9", now(), budapest), Some(at(2026, 11, 12, 8, 0)));
    // 23:30 UTC on Friday is already Saturday in Budapest: "sat 9" is the next day, not a week on.
    let late = at(2026, 11, 6, 23, 30);
    assert_eq!(parse("sat 9", late, budapest), Some(at(2026, 11, 7, 8, 0)));
    assert_eq!(parse("tomorrow 9", late, budapest), Some(at(2026, 11, 8, 8, 0)));
}

#[test]
fn long_names_the_day_date_and_time() {
    assert_eq!(long(at(2026, 11, 12, 9, 0), now(), Offset::UTC), "Thursday 12 November, 09:00");
    assert_eq!(long(at(2027, 1, 1, 18, 5), now(), Offset::UTC), "Friday 1 January 2027, 18:05", "another year names it");
    let budapest = Offset::from_minutes(60).unwrap_or(Offset::UTC);
    assert_eq!(long(at(2026, 11, 12, 9, 0), now(), budapest), "Thursday 12 November, 10:00");
}

#[test]
fn short_is_the_time_today_the_weekday_this_week_and_the_date_after() {
    let utc = Offset::UTC;
    assert_eq!(short(at(2026, 11, 6, 17, 30), now(), utc), "17:30");
    assert_eq!(short(at(2026, 11, 7, 8, 0), now(), utc), "Sat 08:00");
    assert_eq!(short(at(2026, 11, 12, 9, 0), now(), utc), "Thu 09:00");
    assert_eq!(short(at(2026, 11, 13, 8, 0), now(), utc), "Fri 13 Nov");
    assert_eq!(short(at(2027, 1, 1, 8, 0), now(), utc), "Fri 1 Jan 2027");
}

#[test]
fn a_time_out_of_range_formats_as_nothing() {
    let west = Offset::from_minutes(-5999).unwrap_or(Offset::UTC);
    assert_eq!(long(Timestamp::MIN, now(), west), "");
    assert_eq!(short(Timestamp::MIN, now(), west), "");
    assert_eq!(parse("9", Timestamp::MIN, west), None);
}

#[test]
fn every_month_and_weekday_has_its_name() {
    let months: Vec<String> = (1..=12).map(|month| long(at(2026, month, 1, 9, 0), now(), Offset::UTC)).collect();
    for (shown, name) in
        months.iter().zip(["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"])
    {
        assert!(shown.contains(name), "{shown}");
    }
    let days: Vec<String> = (7..=13).map(|day| short(at(2026, 11, day, 9, 0), at(2026, 11, 7, 0, 0), Offset::UTC)).collect();
    assert_eq!(days, ["09:00", "Sun 09:00", "Mon 09:00", "Tue 09:00", "Wed 09:00", "Thu 09:00", "Fri 09:00"]);
}

/// The states of the typed time's sheet, with the past one the time picker shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Typed {
    Empty,
    Focus,
    Valid,
    Invalid,
    Past,
}

/// The field of the sheet, 296 wide, in `state`, focused where the sheet focuses it.
fn typed_in(
    mode: crate::theme::Mode,
    state: Typed,
) -> (egui::Rect, crate::component::tests::Own, crate::component::tests::Paint, egui_kittest::Harness<'static>) {
    use egui_kittest::kittest::Queryable as _;
    let theme = crate::theme::Theme::new(mode, crate::theme::TextScale::default());
    let value = match state {
        Typed::Empty | Typed::Focus => "",
        Typed::Valid => "thu 9",
        Typed::Invalid => "thursdy",
        Typed::Past => "2026-11-02 09:00",
    };
    let mut harness = crate::component::tests::staged(
        theme,
        |p| p.bg,
        egui::vec2(400.0, 160.0),
        move |ui, theme| {
            let id = egui::Id::unique("typed");
            let mut text = ui.data_mut(|data| data.get_temp_mut_or_insert_with(id.with("text"), || value.to_owned()).clone());
            ui.vertical(|ui| {
                ui.set_width(296.0);
                super::typed_time_field(ui, theme, id, "Type a time", &mut text, now(), Offset::UTC);
            });
            ui.data_mut(|data| data.insert_temp(id.with("text"), text));
        },
    );
    if state != Typed::Empty {
        crate::component::tests::bring(&mut harness, "Type a time", crate::component::Shown::Focus);
        harness.key_press(egui::Key::End);
        harness.run();
    }
    let rect = harness.get_by_label("Type a time").rect();
    let paint = crate::component::tests::painted(&harness, "Type a time");
    (rect, crate::component::tests::own(&paint, rect), paint, harness)
}

#[test]
fn a_typed_time_looks_as_its_sheet_says_in_every_state() {
    use crate::component::field::tests::{caret, drawn, one};
    for mode in [crate::theme::Mode::Dark, crate::theme::Mode::Light] {
        let p = crate::theme::Theme::new(mode, crate::theme::TextScale::default()).palette;
        for state in [Typed::Empty, Typed::Focus, Typed::Valid, Typed::Invalid, Typed::Past] {
            let (rect, look, paint, harness) = typed_in(mode, state);
            let at = format!("{mode:?} {state:?}: {look:#?}");
            assert_eq!(rect.size(), egui::vec2(296.0, 36.0), "{at}");
            assert_eq!(look.fill, Some(p.term), "{at}");
            let edge = if state == Typed::Empty { p.line } else { p.cyan_edge };
            assert_eq!(look.edge, Some((1.0, edge, false)), "{at}");
            let hint = "thu 9, 12 nov 17:00, in 3 days";
            let block = caret(&paint, p.cyan_edge);
            match state {
                Typed::Empty | Typed::Focus => {
                    let shown = one(&harness, hint);
                    let left = if state == Typed::Focus { 28.0 } else { 13.0 };
                    assert_eq!(
                        (shown.rect.min, shown.colour, shown.family.as_str(), shown.size, shown.row),
                        (rect.min + egui::vec2(left, 8.0), p.muted, "Monospace", 13.0, 20.0),
                        "{at}"
                    );
                    let expected =
                        (state == Typed::Focus).then_some(egui::Rect::from_min_size(rect.min + egui::vec2(13.0, 10.0), egui::vec2(7.0, 16.0)));
                    assert_eq!(block, expected, "{at}");
                }
                _ => {
                    let value = match state {
                        Typed::Valid => "thu 9",
                        Typed::Invalid => "thursdy",
                        _ => "2026-11-02 09:00",
                    };
                    let shown = one(&harness, value);
                    assert_eq!(
                        (shown.rect.min, shown.colour, shown.family.as_str(), shown.size),
                        (rect.min + egui::vec2(13.0, 8.0), p.ink, "Monospace", 13.0),
                        "{at}"
                    );
                    assert_eq!(
                        block,
                        Some(egui::Rect::from_min_size(egui::pos2((shown.rect.right() + 2.0).round(), rect.top() + 10.0), egui::vec2(7.0, 16.0))),
                        "{at}"
                    );
                    assert!(drawn(&harness).iter().all(|text| text.text != hint), "{at}: no hint over typed text");
                }
            }
            // What it reads as, 8 under the box in `caption`.
            let reading = match state {
                Typed::Valid => Some(("Thursday 12 November, 09:00", p.cyan_text, 18.0)),
                Typed::Invalid => Some(("Not a time yet: try thu 9, 12 nov or in 3 days", p.muted, 0.0)),
                Typed::Past => Some(("That time has passed: type one still ahead", p.muted, 0.0)),
                _ => None,
            };
            let tick: Vec<_> = drawn(&harness).into_iter().filter(|text| text.text == crate::component::popup::CHECK.to_string()).collect();
            match reading {
                Some((words, colour, left)) => {
                    let line = one(&harness, words);
                    assert_eq!(
                        (line.rect.min, line.colour, line.size, line.row),
                        (rect.left_bottom() + egui::vec2(left, 8.0), colour, 12.0, 18.0),
                        "{at}"
                    );
                    if state == Typed::Valid {
                        assert!(
                            tick.len() == 1
                                && (tick[0].rect.center() - (rect.left_bottom() + egui::vec2(7.0, 17.0))).length() < 1.0
                                && tick[0].colour == p.cyan_text,
                            "{at} {tick:?}"
                        );
                    } else {
                        assert!(tick.is_empty(), "{at}");
                    }
                }
                None => assert!(tick.is_empty(), "{at}"),
            }
        }
    }
}

#[test]
fn a_date_behind_now_has_passed_and_a_word_never_has() {
    assert!(super::passed("2026-11-02", now(), Offset::UTC));
    assert!(!super::passed("2026-11-20", now(), Offset::UTC));
    assert!(!super::passed("thu 9", now(), Offset::UTC));
    assert!(!super::passed("thursdy", now(), Offset::UTC));
}
