// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::Key;
use egui::accesskit::{Role, Toggled};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_time::{CivilDateTime, Offset, Timestamp};

use super::{Ask, Picked, Preset, TimePicker, show};
use crate::component::tests::{builder, theme};
use crate::motion;

/// A UTC time.
fn at(year: i32, month: u8, day: u8, hour: u8, minute: u8) -> Timestamp {
    CivilDateTime::new(year, month, day, hour, minute, 0).and_then(|civil| civil.to_timestamp(Offset::UTC)).unwrap_or(Timestamp::MIN)
}

/// Friday 6 November 2026, 14:30 UTC.
fn friday() -> Timestamp {
    at(2026, 11, 6, 14, 30)
}

const PRESETS: [(&str, [u8; 3]); 4] =
    [("Later today", [6, 17, 30]), ("This morning", [6, 8, 0]), ("Tomorrow morning", [7, 8, 0]), ("Next week", [9, 9, 0])];

/// What the harness keeps: the picker, the time it is, whether it asks about an alert, and what was picked.
struct Probe {
    picker: TimePicker,
    now: Timestamp,
    alert: bool,
    picked: Vec<Picked>,
}

fn harness(alert: bool) -> Harness<'static, Probe> {
    let theme = theme();
    let mut harness = builder().with_size(egui::vec2(360.0, 420.0)).build_ui_state(
        move |ui, probe: &mut Probe| {
            motion::set_reduced(ui.ctx(), true);
            let presets: Vec<Preset<'_>> =
                PRESETS.iter().map(|(name, [day, hour, minute])| Preset { name, at: at(2026, 11, *day, *hour, *minute) }).collect();
            let ask = Ask {
                title: "Snooze until",
                confirm: "Snooze",
                alert: probe.alert.then_some("Notify me when it comes back"),
                presets: &presets,
                extra: None,
                note: None,
            };
            match show(ui, &theme, &mut probe.picker, &ask, probe.now, Offset::UTC) {
                Picked::Open => {}
                picked => probe.picked.push(picked),
            }
        },
        Probe { picker: TimePicker::default(), now: friday(), alert, picked: Vec::new() },
    );
    harness.run();
    harness
}

fn typed(harness: &mut Harness<'static, Probe>, text: &str) {
    let field = harness.get_by_role_and_label(Role::TextInput, "Or type a time");
    field.focus();
    harness.run();
    harness.get_by_role_and_label(Role::TextInput, "Or type a time").type_text(text);
    harness.run();
}

#[test]
fn the_presets_ahead_show_with_their_time_and_past_ones_are_hidden() {
    let harness = harness(true);
    assert!(harness.query_by_label("Snooze until").is_some());
    let later = harness.get_by_label("Later today, 17:30");
    assert_eq!(later.accesskit_node().role(), Role::MenuItem);
    assert!(harness.query_by_label("Tomorrow morning, Sat 08:00").is_some());
    assert!(harness.query_by_label("Next week, Mon 09:00").is_some());
    assert!(harness.query_by_label_contains("This morning").is_none(), "08:00 today is behind");
}

#[test]
fn a_preset_is_chosen_with_one_click_and_its_time_is_the_one_shown() {
    let mut harness = harness(false);
    harness.get_by_label("Tomorrow morning, Sat 08:00").click();
    harness.run();
    assert_eq!(harness.state().picked, vec![Picked::At { at: at(2026, 11, 7, 8, 0), alert: false }]);
}

#[test]
fn a_typed_time_is_used_without_enter() {
    let mut harness = harness(true);
    typed(&mut harness, "thu 9");
    assert!(harness.query_by_label("Thursday 12 November, 09:00").is_some(), "the parsed time shows as it is typed");
    assert!(harness.state().picked.is_empty(), "typing alone picks nothing");
    harness.get_by_role_and_label(Role::Button, "Snooze").click();
    harness.run();
    assert_eq!(harness.state().picked, vec![Picked::At { at: at(2026, 11, 12, 9, 0), alert: false }]);
}

#[test]
fn enter_takes_the_typed_time_too() {
    let mut harness = harness(false);
    typed(&mut harness, "tomorrow 9:30");
    harness.key_press(Key::Enter);
    harness.run();
    assert_eq!(harness.state().picked, vec![Picked::At { at: at(2026, 11, 7, 9, 30), alert: false }]);
}

#[test]
fn the_shown_time_follows_the_clock_and_is_the_one_used() {
    let mut harness = harness(false);
    typed(&mut harness, "17:30");
    assert!(harness.query_by_label("Friday 6 November, 17:30").is_some());
    harness.state_mut().now = at(2026, 11, 6, 17, 45);
    harness.run();
    assert!(harness.query_by_label("Friday 6 November, 17:30").is_none(), "a time behind is never shown");
    assert!(harness.query_by_label("Saturday 7 November, 17:30").is_some());
    harness.get_by_role_and_label(Role::Button, "Snooze").click();
    harness.run();
    assert_eq!(harness.state().picked, vec![Picked::At { at: at(2026, 11, 7, 17, 30), alert: false }]);
}

#[test]
fn text_that_is_no_time_says_so_and_cannot_be_confirmed() {
    let mut harness = harness(false);
    typed(&mut harness, "someday");
    assert!(harness.query_by_label("Not a time yet: try thu 9, 12 nov or in 3 days").is_some());
    harness.get_by_role_and_label(Role::Button, "Snooze").click();
    harness.run();
    harness.key_press(Key::Enter);
    harness.run();
    assert!(harness.state().picked.is_empty());
    assert_eq!(harness.state().picker.chosen(friday(), Offset::UTC), None);
}

#[test]
fn nothing_typed_cannot_be_confirmed_either() {
    let mut harness = harness(false);
    harness.get_by_role_and_label(Role::Button, "Snooze").click();
    harness.run();
    assert!(harness.state().picked.is_empty());
}

#[test]
fn cancel_and_escape_cancel() {
    let mut harness = harness(false);
    harness.get_by_role_and_label(Role::Button, "Cancel").click();
    harness.run();
    harness.key_press(Key::Escape);
    harness.run();
    assert_eq!(harness.state().picked, vec![Picked::Cancelled, Picked::Cancelled]);
}

#[test]
fn the_alert_switch_goes_with_the_time_and_remembers_its_last_value() {
    let mut harness = harness(true);
    let switch = harness.get_by_role_and_label(Role::Switch, "Notify me when it comes back");
    assert_eq!(switch.accesskit_node().toggled(), Some(Toggled::False));
    switch.click();
    harness.run();
    harness.get_by_label("Later today, 17:30").click();
    harness.run();
    assert_eq!(harness.state().picked, vec![Picked::At { at: at(2026, 11, 6, 17, 30), alert: true }]);
    // The next opening starts with nothing typed and the switch as it was left.
    typed(&mut harness, "thu 9");
    harness.state_mut().picker.open();
    harness.run();
    assert_eq!(harness.state().picker.typed(), "");
    assert!(harness.state().picker.alert());
    let switch = harness.get_by_role_and_label(Role::Switch, "Notify me when it comes back");
    assert_eq!(switch.accesskit_node().toggled(), Some(Toggled::True));
}

#[test]
fn without_an_alert_question_there_is_no_switch_and_no_alert() {
    let mut harness = harness(false);
    harness.state_mut().picker.set_alert(true);
    harness.run();
    assert!(harness.query_by_role(Role::Switch).is_none());
    harness.get_by_label("Later today, 17:30").click();
    harness.run();
    assert_eq!(harness.state().picked, vec![Picked::At { at: at(2026, 11, 6, 17, 30), alert: false }]);
}

#[test]
fn typed_text_is_capped() {
    let mut harness = harness(false);
    typed(&mut harness, &"9".repeat(super::MAX_TYPED * 2));
    assert!(harness.state().picker.typed().len() <= super::MAX_TYPED);
}

#[test]
fn a_picker_made_with_an_alert_value_starts_with_it() {
    let picker = TimePicker::with_alert(true);
    assert!(picker.alert());
    assert_eq!(picker.typed(), "");
}

#[test]
fn set_typed_keeps_at_most_max_typed_bytes_at_a_character_boundary() {
    let mut picker = TimePicker::default();
    picker.set_typed("thu 9");
    assert_eq!(picker.typed(), "thu 9");
    picker.set_typed(&"é".repeat(super::MAX_TYPED));
    assert!(picker.typed().len() <= super::MAX_TYPED && picker.typed().chars().all(|c| c == 'é'), "{}", picker.typed());
    assert_eq!(picker.typed().chars().count(), super::MAX_TYPED / 2);
    picker.set_typed(&format!("a{}", "é".repeat(super::MAX_TYPED)));
    assert_eq!(picker.typed().len(), super::MAX_TYPED - 1, "the cut steps back to the last whole character");
}

/// The states of the time picker's sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sheet {
    Presets,
    Hover,
    TypedValid,
    TypedInvalid,
    WithAlert,
    Past,
    EmptyHint,
}

const SHEET: [Sheet; 7] = [Sheet::Presets, Sheet::Hover, Sheet::TypedValid, Sheet::TypedInvalid, Sheet::WithAlert, Sheet::Past, Sheet::EmptyHint];

/// The sheet's presets: later today, tomorrow morning, this weekend and next week from Monday 16 November 2026.
const SHEET_PRESETS: [(&str, [u8; 3]); 4] =
    [("Later today", [16, 17, 0]), ("Tomorrow morning", [17, 8, 0]), ("This weekend", [21, 9, 0]), ("Next week", [23, 9, 0])];

/// The picker of the sheet in `state`, in a float's panel 320 wide at (16, 16) on `bg`.
fn sheet_in(mode: crate::theme::Mode, state: Sheet) -> Harness<'static> {
    let theme = crate::theme::Theme::new(mode, crate::theme::TextScale::default());
    // Monday 16 November 2026: 10:00, or 18:00 when the first preset has passed.
    let now = at(2026, 11, 16, if state == Sheet::Past { 18 } else { 10 }, 0);
    let typed = match state {
        Sheet::TypedValid | Sheet::WithAlert => "thu 9",
        Sheet::TypedInvalid => "thursdy",
        Sheet::Past => "2026-11-02 09:00",
        _ => "",
    };
    let mut harness = builder().with_size(egui::vec2(420.0, 480.0)).build_ui(move |ui| {
        motion::set_reduced(ui.ctx(), true);
        ui.painter().rect_filled(ui.max_rect(), 0.0, theme.palette.bg);
        let id = egui::Id::unique("sheet-picker");
        let mut picker = ui.data_mut(|data| {
            data.get_temp_mut_or_insert_with(id, || {
                let mut picker = TimePicker::with_alert(true);
                picker.set_typed(typed);
                picker
            })
            .clone()
        });
        let presets: Vec<Preset<'_>> =
            SHEET_PRESETS.iter().map(|(name, [day, hour, minute])| Preset { name, at: at(2026, 11, *day, *hour, *minute) }).collect();
        let alert = (state == Sheet::WithAlert).then_some("Notify me when it comes back");
        let ask = Ask { title: "Snooze until", confirm: "Snooze", alert, presets: &presets, extra: None, note: None };
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.add_space(8.0);
            crate::component::popup::panel(ui, &theme, super::WIDTH, |ui| show(ui, &theme, &mut picker, &ask, now, Offset::UTC));
        });
        ui.data_mut(|data| data.insert_temp(id, picker));
    });
    harness.run();
    match state {
        Sheet::Hover => {
            harness.get_by_label("Tomorrow morning, Tue 08:00").hover();
            harness.run();
        }
        Sheet::TypedValid | Sheet::TypedInvalid | Sheet::WithAlert | Sheet::Past | Sheet::EmptyHint => {
            harness.get_by_label("Or type a time").focus();
            harness.run();
            harness.key_press(Key::End);
            harness.run();
        }
        Sheet::Presets => {}
    }
    harness
}

/// The rects the last frame painted with their fill.
fn fills(harness: &Harness<'static>) -> Vec<(egui::Rect, egui::Color32)> {
    fn walk(shape: &egui::Shape, found: &mut Vec<(egui::Rect, egui::Color32)>) {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, found)),
            egui::Shape::Rect(rect) if rect.blur_width == 0.0 && rect.fill.a() > 0 => found.push((rect.rect, rect.fill)),
            _ => {}
        }
    }
    let mut found = Vec::new();
    harness.output().shapes.iter().for_each(|clipped| walk(&clipped.shape, &mut found));
    found
}

#[test]
fn a_time_picker_looks_as_its_sheet_says_in_every_state() {
    use crate::component::field::tests::one;
    use egui::{pos2, vec2};
    for mode in [crate::theme::Mode::Dark, crate::theme::Mode::Light] {
        let p = crate::theme::Theme::new(mode, crate::theme::TextScale::default()).palette;
        for state in SHEET {
            let harness = sheet_in(mode, state);
            let at = format!("{mode:?} {state:?}");
            let found = fills(&harness);
            let origin = pos2(16.0, 16.0);
            let float = found.iter().find(|(_, fill)| *fill == p.surface).map(|(rect, _)| *rect);
            let height = match state {
                Sheet::Presets | Sheet::Hover => 304.0,
                Sheet::TypedValid | Sheet::TypedInvalid => 330.0,
                Sheet::WithAlert => 367.0,
                Sheet::Past => 297.0,
                Sheet::EmptyHint => 348.0,
            };
            assert_eq!(float, Some(egui::Rect::from_min_size(origin, vec2(320.0, height))), "{at}");
            // The title in the `label` style 12 in and 10 down, the Esc chip at the right end.
            let title = one(&harness, "SNOOZE UNTIL");
            assert_eq!((title.rect.min, title.colour, title.size), (origin + vec2(13.0, 11.0), p.muted, 11.0), "{at}");
            let esc = one(&harness, "Esc");
            assert!((esc.rect.center().x - (origin.x + 1.0 + 276.0 + 15.0)).abs() < 1.0, "{at} {esc:?}");
            let line = |y: f32, colour| {
                found.iter().any(|(rect, fill)| *rect == egui::Rect::from_min_size(origin + vec2(1.0, y), vec2(318.0, 1.0)) && *fill == colour)
            };
            assert!(line(37.0, p.line), "{at}: the title's hairline");
            // The presets still ahead, 32 high, `panel_line` between them.
            let names: Vec<&str> = if state == Sheet::Past {
                vec!["Tomorrow morning, Tue 08:00", "This weekend, Sat 09:00", "Next week, Mon 23 Nov"]
            } else {
                vec!["Later today, 17:00", "Tomorrow morning, Tue 08:00", "This weekend, Sat 09:00", "Next week, Mon 23 Nov"]
            };
            for (index, name) in names.iter().enumerate() {
                let top = 38.0 + 33.0 * index as f32;
                assert_eq!(harness.get_by_label(name).rect(), egui::Rect::from_min_size(origin + vec2(1.0, top), vec2(318.0, 32.0)), "{at} {name}");
                if index > 0 {
                    assert!(line(top - 1.0, p.panel_line), "{at}: the line over {name}");
                }
            }
            let typed_top = 38.0 + 33.0 * names.len() as f32 - 1.0;
            assert!(line(typed_top, p.line), "{at}: the line over the typed time");
            let label = one(&harness, "OR TYPE A TIME");
            assert_eq!(label.rect.min, origin + vec2(13.0, typed_top + 13.0), "{at}");
            let field = harness.get_by_label("Or type a time").rect();
            assert_eq!(field, egui::Rect::from_min_size(origin + vec2(13.0, typed_top + 37.0), vec2(294.0, 36.0)), "{at}");
            let hovered = found.iter().any(|(rect, fill)| *fill == p.stage && rect.height() == 32.0);
            assert_eq!(hovered, state == Sheet::Hover, "{at}");
            if state == Sheet::EmptyHint {
                let hint = crate::component::field::tests::rows(&harness, "It counts as soon as it reads as a time: no Enter needed.");
                assert_eq!((hint.rect.min, hint.colour, hint.rect.height()), (field.left_bottom() + vec2(0.0, 8.0), p.muted, 36.0), "{at}");
            }
            if state == Sheet::WithAlert {
                let words = one(&harness, "Notify me when it comes back");
                assert_eq!((words.rect.min, words.colour), (origin + vec2(13.0, 288.0), p.ink), "{at}");
                let switch = harness.get_by_role(Role::Switch).rect();
                assert_eq!(switch, egui::Rect::from_min_size(origin + vec2(279.0, 291.0), vec2(28.0, 16.0)), "{at}");
            }
            // Cancel and the confirm at the right of the last row, 8 down, 71 × 32.
            let row = height - 49.0;
            let cancel = harness.get_by_role_and_label(Role::Button, "Cancel").rect();
            let confirm = harness.get_by_role_and_label(Role::Button, "Snooze").rect();
            assert_eq!(confirm, egui::Rect::from_min_size(origin + vec2(236.0, row + 8.0), vec2(71.0, 32.0)), "{at}");
            assert_eq!(cancel, egui::Rect::from_min_size(origin + vec2(157.0, row + 8.0), vec2(71.0, 32.0)), "{at}");
            let ready = matches!(state, Sheet::TypedValid | Sheet::WithAlert);
            assert_eq!(harness.get_by_role_and_label(Role::Button, "Snooze").accesskit_node().is_disabled(), !ready, "{at}");
            assert!(line(row - 1.0, p.line), "{at}: the line over the buttons");
        }
    }
}

/// The picker with the engine's five presets of Thursday 5 November 2026 09:00 (Snooze's and Send later's list), an
/// extra row, the alert and a note, in a float hanging from a control at (40, 120), in a window `size` at `percent`
/// text, after the wheel turned `wheel` points over the float (down when negative): the float's rect, the window's, and
/// the rects of the five rows and of the confirm button.
fn five_in_a_float(size: egui::Vec2, percent: u16, wheel: f32) -> (egui::Rect, egui::Rect, Vec<egui::Rect>, egui::Rect) {
    let theme = crate::theme::Theme::new(crate::theme::Mode::Light, crate::theme::TextScale::from_percent(percent).unwrap());
    let now = at(2026, 11, 5, 9, 0);
    let five = [
        ("Later today", [5, 12, 0]),
        ("This evening", [5, 20, 0]),
        ("Tomorrow morning", [6, 8, 0]),
        ("This weekend", [7, 9, 0]),
        ("Next week", [9, 9, 0]),
    ];
    let surface = theme.palette.surface;
    let mut harness = builder().with_size(size).build_ui(move |ui| {
        motion::set_reduced(ui.ctx(), true);
        let presets: Vec<Preset<'_>> =
            five.iter().map(|(name, [day, hour, minute])| Preset { name, at: at(2026, 11, *day, *hour, *minute) }).collect();
        let ask = Ask {
            title: "Snooze until",
            confirm: "Snooze",
            alert: Some("Notify me when it comes back"),
            presets: &presets,
            extra: Some("Send now instead"),
            note: Some("It comes back to the top of the inbox."),
        };
        let float = crate::component::popup::Float {
            id: egui::Id::unique("five-presets"),
            anchor: egui::Rect::from_min_size(egui::pos2(40.0, 120.0), egui::vec2(80.0, 28.0)),
            width: super::WIDTH,
            returns_to: egui::Id::unique("five-presets-opener"),
        };
        let mut picker = TimePicker::default();
        crate::component::popup::float(ui.ctx(), &theme, &float, |ui| show(ui, &theme, &mut picker, &ask, now, Offset::UTC));
    });
    for _ in 0..4 {
        harness.run();
    }
    fn walk(shape: &egui::Shape, found: &mut Vec<(egui::Rect, egui::Color32)>) {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, found)),
            egui::Shape::Rect(rect) if rect.blur_width == 0.0 && rect.fill.a() > 0 => found.push((rect.rect, rect.fill)),
            _ => {}
        }
    }
    let float_of = |harness: &Harness<'_>| {
        let mut found = Vec::new();
        harness.output().shapes.iter().for_each(|clipped| walk(&clipped.shape, &mut found));
        found.iter().filter(|(_, fill)| *fill == surface).map(|(rect, _)| *rect).max_by(|a, b| a.height().total_cmp(&b.height())).unwrap()
    };
    if wheel != 0.0 {
        harness.hover_at(float_of(&harness).center());
        harness.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, wheel),
            modifiers: egui::Modifiers::NONE,
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Wheel,
        });
        for _ in 0..30 {
            harness.run();
        }
    }
    let float = float_of(&harness);
    let rows = ["Later today, 12:00", "This evening, 20:00", "Tomorrow morning, Fri 08:00", "This weekend, Sat 09:00", "Next week, Mon 09:00"]
        .map(|name| harness.get_by_label(name).rect())
        .to_vec();
    let confirm = harness.get_by_role_and_label(Role::Button, "Snooze").rect();
    (float, egui::Rect::from_min_size(egui::Pos2::ZERO, size), rows, confirm)
}

#[test]
fn the_five_presets_fit_their_float_at_100_and_130_percent_and_scroll_in_the_least_window() {
    for percent in [100, 130] {
        let (float, window, rows, confirm) = five_in_a_float(egui::vec2(1280.0, 800.0), percent, 0.0);
        assert!(window.contains_rect(float), "{percent} %: {float:?}");
        for row in rows.iter().chain([&confirm]) {
            assert!(float.contains_rect(*row), "{percent} %: {row:?} inside {float:?}");
        }
        assert!(rows.windows(2).all(|pair| pair[0].bottom() <= pair[1].top()), "{percent} %: the engine's order, one under the other");
    }
    // The least window, 800 × 480, at 130 %: the float stays inside it and its rows scroll. Opened, the last preset is
    // whole in it and the confirm button below the fold; the wheel brings the confirm button whole into it.
    let (float, window, rows, confirm) = five_in_a_float(egui::vec2(800.0, 480.0), 130, 0.0);
    assert!(window.contains_rect(float), "{float:?}");
    assert!(float.height() <= 480.0 - 6.0 + 0.5, "{float:?}");
    assert!(rows.last().is_some_and(|next_week| float.contains_rect(*next_week)), "Next week whole: {rows:?} in {float:?}");
    assert!(!float.contains_rect(confirm), "the confirm button is out of sight before the rows scroll: {confirm:?}");
    let (float, window, _, confirm) = five_in_a_float(egui::vec2(800.0, 480.0), 130, -2000.0);
    assert!(window.contains_rect(float), "{float:?}");
    assert!(float.contains_rect(confirm), "scrolled, the confirm button is whole in the float: {confirm:?} in {float:?}");
}
