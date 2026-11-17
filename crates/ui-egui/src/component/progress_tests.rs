// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::{Color32, Rect, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

use super::{ProgressState, bar, progress, progress_with};
use crate::component::tests::{Look, Paint, builder, painted, stage, staged};
use crate::keymap::Keymap;
use crate::motion;
use crate::theme::{Mode, TextScale, Theme};

/// The sheet's places of the eight dots of the running mark in its 10 × 10 box.
const DOTS: [(f32, f32); 8] = [(4.0, 0.0), (7.0, 1.0), (8.0, 4.0), (7.0, 7.0), (4.0, 8.0), (1.0, 7.0), (0.0, 4.0), (1.0, 1.0)];

/// The texts painted, `(rect, text, colour, size)`.
fn texts(paint: &Paint) -> Vec<(Rect, String, Color32, f32)> {
    paint
        .shapes
        .iter()
        .filter_map(|shape| match &shape.look {
            Look::Text { text, colour, size, .. } => Some((shape.rect, text.clone(), *colour, *size)),
            _ => None,
        })
        .collect()
}

/// The 2 × 2 dots painted, by their place against `at`.
fn dots(paint: &Paint, at: egui::Pos2, colour: Color32) -> Vec<(f32, f32)> {
    paint
        .rects()
        .filter_map(|shape| match shape.look {
            Look::Rect { fill, stroke: None, .. } if fill == colour && shape.rect.size() == vec2(2.0, 2.0) => {
                Some((shape.rect.left() - at.x, shape.rect.top() - at.y))
            }
            _ => None,
        })
        .collect()
}

/// Where the dot of a finished line stands against `rect`: in the middle of the sheet's 11 points.
fn dot_left(paint: &Paint, rect: Rect) -> f32 {
    let dot = texts(paint).into_iter().find(|text| text.1 == "●").map_or(Rect::NOTHING, |text| text.0);
    assert!((dot.center().x - rect.left() - 5.5).abs() <= 0.5, "the dot in the middle of 11: {dot:?} {rect:?}");
    dot.left() - rect.left()
}

/// One progress line in `state`, drawn still (the sheet's frame) or moving, on the status line's `surface`.
fn line(theme: Theme, word: &'static str, state: ProgressState, note: &'static str, still: bool) -> Harness<'static> {
    staged(
        theme,
        |p| p.surface,
        vec2(400.0, 80.0),
        move |ui, theme| {
            progress_with(ui, theme, &Keymap::default(), word, state, note, still);
        },
    )
}

#[test]
fn a_progress_line_looks_as_its_sheet_says_in_every_state() {
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::default());
        let p = theme.palette;
        let f = theme.fonts;
        // Running, the sheet's still frame: the ring of eight `warn` dots, the word and bar in `ink`, the note `muted`.
        let harness = line(theme, "syncing", ProgressState::Running(0.54), "54 %", true);
        let rect = harness.get_by_label("syncing").rect();
        let paint = painted(&harness, "syncing");
        let at = format!("{mode:?} running: {paint:#?}");
        assert_eq!(rect.size(), vec2(188.0, 24.0), "{at}");
        assert_eq!(dots(&paint, rect.min + vec2(0.0, 7.0), p.warn), DOTS.to_vec(), "{at}");
        let got = texts(&paint);
        let want = [("syncing", p.ink, 18.0), ("[=====>    ]", p.ink, 73.0), ("54 %", p.muted, 161.0)];
        assert_eq!(got.len(), 3, "{at}");
        for ((text_rect, text, colour, size), (words, ink, x)) in got.iter().zip(want) {
            assert_eq!((text.as_str(), *colour, *size), (words, ink, f.mono_s.size), "{at}");
            assert_eq!(text_rect.min - rect.min, vec2(x, 4.0), "{words}: {at}");
        }
        // Done: a `plus` dot, the word, a full `muted` bar and the note.
        let harness = line(theme, "synced", ProgressState::Done, "14:02", true);
        let rect = harness.get_by_label("synced").rect();
        let paint = painted(&harness, "synced");
        let at = format!("{mode:?} done: {paint:#?}");
        assert_eq!(rect.size(), vec2(188.0, 24.0), "{at}");
        let got: Vec<_> = texts(&paint).into_iter().map(|(r, t, c, _)| (r.min - rect.min, t, c)).collect();
        let want = vec![
            (vec2(dot_left(&paint, rect), 4.0), "●".to_owned(), p.plus),
            (vec2(19.0, 4.0), "synced".to_owned(), p.ink),
            (vec2(67.0, 4.0), "[==========]".to_owned(), p.muted),
            (vec2(155.0, 4.0), "14:02".to_owned(), p.muted),
        ];
        assert_eq!(got, want, "{at}");
        // Failed: everything `minus`, the bar broken where it stopped, the note the link to the log.
        let harness = line(theme, "sync failed", ProgressState::Failed(0.54), "open log", true);
        let rect = harness.get_by_label("sync failed").rect();
        let paint = painted(&harness, "sync failed");
        let at = format!("{mode:?} failed: {paint:#?}");
        assert_eq!(rect.size(), vec2(241.0, 24.0), "{at}");
        let got: Vec<_> = texts(&paint).into_iter().map(|(r, t, c, _)| (r.min - rect.min, t, c)).collect();
        let want = vec![
            (vec2(dot_left(&paint, rect), 4.0), "●".to_owned(), p.minus),
            (vec2(19.0, 4.0), "sync failed".to_owned(), p.minus),
            (vec2(100.0, 4.0), "[=====×    ]".to_owned(), p.minus),
            (vec2(188.0, 4.0), "open log".to_owned(), p.cyan_text),
        ];
        assert_eq!(got, want, "{at}");
        harness.get_by_label("open log");
        assert!(paint.rects().all(|shape| matches!(shape.look, Look::Rect { radius, .. } if radius <= 0.0)), "{at}");
    }
}

#[test]
fn the_bar_has_ten_cells_and_an_arrow_at_the_work_done() {
    assert_eq!(bar(ProgressState::Running(0.0)), "[>         ]");
    assert_eq!(bar(ProgressState::Running(0.54)), "[=====>    ]");
    assert_eq!(bar(ProgressState::Running(0.99)), "[=========>]");
    assert_eq!(bar(ProgressState::Running(1.0)), "[==========]");
    assert_eq!(bar(ProgressState::Done), "[==========]");
    assert_eq!(bar(ProgressState::Failed(0.54)), "[=====×    ]");
    assert_eq!(bar(ProgressState::Failed(1.0)), "[=========×]");
}

#[test]
fn a_running_mark_goes_round_one_dot_each_125_ms() {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let p = theme.palette;
    for reduced in [false, true] {
        let mut harness = builder().with_size(vec2(400.0, 80.0)).build_ui(move |ui| {
            motion::set_reduced(ui.ctx(), reduced);
            stage(ui, theme.palette.surface, |ui| progress(ui, &theme, &Keymap::default(), "syncing", ProgressState::Running(0.5), "50 %"));
        });
        harness.step();
        let rect = harness.get_by_label("syncing").rect();
        let box_at = rect.min + vec2(0.0, 7.0);
        let mut gaps = Vec::new();
        for _ in 0..24 {
            harness.step();
            let shown = dots(&painted(&harness, "syncing"), box_at, p.warn);
            assert_eq!(shown.len(), 7, "reduced {reduced}: seven dots and one gap: {shown:?}");
            assert!(shown.iter().all(|dot| DOTS.contains(dot)), "{shown:?}");
            let gap = DOTS.iter().position(|dot| !shown.contains(dot));
            gaps.push(gap);
        }
        let mut moved = gaps.windows(2).filter(|pair| pair[0] != pair[1]).count();
        // 24 frames of 1/60 s are 400 ms: the gap moves three times.
        assert!((2..=4).contains(&moved), "reduced {reduced}: {gaps:?}");
        moved = gaps.windows(2).filter(|pair| matches!(pair, [Some(a), Some(b)] if *b != (*a + 1) % 8 && a != b)).count();
        assert_eq!(moved, 0, "clockwise one dot at a time: {gaps:?}");
    }
}

#[test]
fn the_link_to_the_log_is_pressed_by_click_enter_and_space() {
    let theme = Theme::new(Mode::Light, TextScale::default());
    for key in [None, Some(egui::Key::Enter), Some(egui::Key::Space)] {
        let mut harness = builder().with_size(vec2(400.0, 80.0)).build_ui_state(
            move |ui, pressed: &mut usize| {
                let link = progress(ui, &theme, &Keymap::default(), "sync failed", ProgressState::Failed(0.4), "open log");
                *pressed += usize::from(link.is_some_and(|link| link.clicked()));
            },
            0,
        );
        harness.run();
        match key {
            None => harness.get_by_label("open log").click(),
            Some(key) => {
                harness.get_by_label("open log").focus();
                harness.run();
                harness.key_press(key);
            }
        }
        harness.run();
        assert_eq!(*harness.state(), 1, "{key:?}");
    }
    let _ = pos2(0.0, 0.0);
}

#[test]
fn a_progress_line_is_the_scaled_strip_at_130_percent() {
    let theme = Theme::new(Mode::Light, TextScale::ALL[4]);
    let harness = line(theme, "syncing", ProgressState::Running(0.5), "50 %", true);
    assert_eq!(harness.get_by_label("syncing").rect().height(), theme.boxes().strip);
}
