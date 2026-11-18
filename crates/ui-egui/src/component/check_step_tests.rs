// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::{Color32, Rect, vec2};
use egui_kittest::kittest::Queryable as _;

use super::{StepState, check_step, spoken};
use crate::component::tests::{Look, Paint, painted, rests, staged, staged_motion};
use crate::theme::{Mode, Palette, TextScale, Theme};

/// A state of the sheet: the state, its mark, step, detail, note and the colours of mark, step and detail.
type State = (StepState, &'static str, &'static str, &'static str, &'static str, fn(&Palette) -> [Color32; 3]);

const STATES: [State; 8] = [
    (StepState::Waiting, "[    ]", "SMTP", "smtp.example.org:465 · TLS", "waiting", |p| [p.dim, p.muted, p.muted]),
    (StepState::Running, "[ .. ]", "SMTP", "smtp.example.org:465 · TLS", "0.2 s …", |p| [p.warn, p.ink, p.ink]),
    (StepState::Passed, "[ ok ]", "IMAP", "sign-in, 6 folders, IDLE CONDSTORE QRESYNC MOVE", "0.6 s", |p| [p.plus, p.ink, p.ink]),
    (StepState::Failed, "[fail]", "IMAP", "sign-in", "1.2 s", |p| [p.minus, p.ink, p.ink]),
    (StepState::Skipped, "[skip]", "SMTP", "smtp.example.org:465 · TLS", "skipped", |p| [p.muted, p.muted, p.muted]),
    (StepState::NotRun, "[ -- ]", "Clock", "compared with the server's", "not run", |p| [p.muted, p.muted, p.muted]),
    (StepState::NoServer, "[none]", "CalDAV", "calendars", "no server set", |p| [p.muted, p.muted, p.muted]),
    (StepState::NotBuilt, "[ n/a]", "Push", "new mail without asking", "not built yet", |p| [p.muted, p.muted, p.muted]),
];

const MESSAGE: &str = "refused: [AUTHENTICATIONFAILED] Invalid credentials.";

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

#[test]
fn a_check_step_looks_as_its_sheet_says_in_every_state() {
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::default());
        let p = theme.palette;
        let f = theme.fonts;
        for (state, mark, step, detail, note, colours) in STATES {
            let failed = state == StepState::Failed;
            let harness = staged(
                theme,
                |p| p.term,
                vec2(560.0 + 48.0, 120.0),
                move |ui, theme| {
                    ui.set_width(560.0);
                    check_step(ui, theme, step, detail, note, state, failed.then_some(MESSAGE));
                },
            );
            let name = spoken(step, state);
            let rect = harness.get_by_label(&name).rect();
            let paint = painted(&harness, &name);
            let at = format!("{mode:?} {state:?}: {paint:#?}");
            assert_eq!(rect.size(), vec2(560.0, if failed { 44.0 } else { 24.0 }), "{at}");
            assert!(paint.rects().next().is_none(), "no fill, no edge: {at}");
            let [mark_ink, step_ink, detail_ink] = colours(p);
            let got: Vec<_> = texts(&paint).into_iter().map(|(r, t, c, s)| (r.min - rect.min, t, c, s)).collect();
            let note_width = got.iter().find(|text| text.1 == note).map_or(0.0, |text| text.0.x);
            let mut want = vec![
                (vec2(0.0, 2.0), mark.to_owned(), mark_ink, f.mono.size),
                (vec2(57.0, 2.0), step.to_owned(), step_ink, f.mono.size),
                (vec2(131.0, 2.0), detail.to_owned(), detail_ink, f.mono.size),
                (vec2(note_width, 4.0), note.to_owned(), p.muted, f.mono_s.size),
            ];
            if failed {
                want.push((vec2(131.0, 24.0), MESSAGE.to_owned(), p.minus, f.mono_s.size));
            }
            assert_eq!(got, want, "{at}");
            // The note ends at the line's right end.
            let note_rect = texts(&paint).into_iter().find(|text| text.1 == note).map_or(Rect::NOTHING, |text| text.0);
            assert!((note_rect.right() - rect.right()).abs() <= 1.0, "{at}");
        }
    }
}

#[test]
fn a_long_detail_and_message_end_with_an_ellipsis_inside_the_line() {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let long = "mail.a-very-long-host-name-that-goes-on-and-on.example.org:993 · TLS · ".repeat(20);
    let detail: &'static str = Box::leak(long.into_boxed_str());
    let harness = staged(
        theme,
        |p| p.term,
        vec2(560.0 + 48.0, 120.0),
        move |ui, theme| {
            ui.set_width(560.0);
            check_step(ui, theme, "A step name too long for its column", detail, "1.2 s", StepState::Failed, Some(detail));
        },
    );
    let name = spoken("A step name too long for its column", StepState::Failed);
    let rect = harness.get_by_label(&name).rect();
    let paint = painted(&harness, &name);
    for (text_rect, ..) in texts(&paint) {
        assert!(rect.contains_rect(text_rect), "{text_rect:?} leaves {rect:?}");
    }
    assert!(!paint.shapes.iter().any(|shape| shape.overflows));
}

#[test]
fn a_running_step_stands_still_and_rests() {
    let theme = Theme::new(Mode::Light, TextScale::default());
    let mut harness = staged_motion(
        theme,
        false,
        |p| p.term,
        vec2(600.0, 100.0),
        |ui, theme| {
            check_step(ui, theme, "SMTP", "smtp.example.org:465 · TLS", "0.2 s …", StepState::Running, None);
        },
    );
    assert!(rests(&mut harness, 8));
}

#[test]
fn a_check_step_is_the_scaled_strip_at_130_percent() {
    let theme = Theme::new(Mode::Light, TextScale::ALL[4]);
    let harness = staged(
        theme,
        |p| p.term,
        vec2(700.0, 120.0),
        |ui, theme| {
            check_step(ui, theme, "SMTP", "smtp.example.org", "waiting", StepState::Waiting, None);
        },
    );
    let rect = harness.get_by_label(&spoken("SMTP", StepState::Waiting)).rect();
    assert_eq!(rect.height(), theme.boxes().strip);
}

/// Whether `shown`, without its closing ellipsis, is a beginning of `full` that ends between grapheme clusters.
fn whole_prefix(full: &str, shown: &str) -> bool {
    use unicode_segmentation::UnicodeSegmentation as _;
    let kept = shown.strip_suffix('…').unwrap_or(shown).trim_end();
    full.starts_with(kept) && (kept.is_empty() || full.grapheme_indices(true).any(|(at, grapheme)| at + grapheme.len() == kept.len()))
}

#[test]
fn a_cut_step_detail_and_message_keep_their_graphemes_whole_and_are_capped() {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let marks: &'static str = Box::leak("e\u{301}\u{323}".repeat(300).into_boxed_str());
    let family: &'static str = Box::leak("\u{1F469}\u{200D}\u{1F469}\u{200D}\u{1F467} ".repeat(200).into_boxed_str());
    let long: &'static str = Box::leak("v".repeat(100_000).into_boxed_str());
    for value in [marks, family, long] {
        let mut harness = crate::component::tests::builder().allow_missing_glyphs().with_size(vec2(560.0 + 48.0, 120.0)).build_ui(move |ui| {
            crate::component::tests::stage(ui, theme.palette.term, |ui| {
                ui.set_width(560.0);
                check_step(ui, &theme, value, value, "1.2 s", StepState::Failed, Some(value));
            });
        });
        harness.run();
        let paint = painted(&harness, &spoken(value, StepState::Failed));
        let drawn: Vec<String> = texts(&paint).into_iter().map(|text| text.1).filter(|text| text.chars().next() == value.chars().next()).collect();
        assert_eq!(drawn.len(), 3, "step, detail and message");
        for drawn in drawn {
            assert!(drawn.chars().count() <= crate::component::MOST + 1, "capped: {} characters", drawn.chars().count());
            assert!(drawn.ends_with('…') && whole_prefix(value, &drawn), "a whole-grapheme beginning: {drawn:?}");
        }
    }
}
