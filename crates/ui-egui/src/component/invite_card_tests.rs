// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::{Color32, Rect, StrokeKind, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::{CardAsked, CardEvent, CardState, Said, invite_card, spoken};
use crate::component::tests::{Look, Paint, own, painted, staged};
use crate::keymap::Keymap;
use crate::theme::{Mode, TextScale, Theme};

const EVENT: CardEvent<'static> = CardEvent {
    weekday: "MON",
    day: "16",
    month: "NOV",
    title: "Design review",
    when: "15:00 – 16:00  ·  today",
    place: "Room 3",
    who: "Marit Solheim (organiser), Teodor Vasquez, Ines Okafor and you",
    cal: "Studio",
    not_from_organiser: false,
};

const GOES: &str = "marit@example.org";

const REPLIES: [(Said, &str); 4] = [
    (Said::Going, "Teodor Vasquez is going."),
    (Said::Maybe, "Ines Okafor might go."),
    (Said::No, "Pavel Hruska is not going."),
    (Said::Nothing, "Dmitri Vale has not answered."),
];

/// The sheet's nine states with their heights.
fn states() -> [(&'static str, CardState<'static>, f32); 9] {
    [
        ("request", CardState::Request { goes_to: GOES }, 236.0),
        ("changed", CardState::Changed { changed: "time 14:00 → 15:00", goes_to: GOES }, 274.0),
        ("out-of-date", CardState::OutOfDate { line: "Out of date: a later invitation replaces this one. You answered Going." }, 38.0),
        ("answered", CardState::Answered { answer: "You answered Going." }, 208.0),
        ("cancelled", CardState::Cancelled { line: "Marit Solheim cancelled this event.", remove: true }, 246.0),
        ("reply", CardState::Replies { replies: &REPLIES }, 270.0),
        ("no-answer-asked", CardState::NoAnswerAsked, 194.0),
        (
            "cannot-answer",
            CardState::CannotAnswer {
                why: "it was sent to lists@example.org, which is not one of your addresses, and the organiser asked only that address.",
            },
            212.0,
        ),
        ("clash", CardState::Clash { clash: "Call Ines 15:30 – 15:45 is in the same hour", goes_to: GOES }, 274.0),
    ]
}

/// The card 640 wide on the reader's `surface`.
fn harness(theme: Theme, state: CardState<'static>) -> Harness<'static> {
    staged(
        theme,
        |p| p.surface,
        vec2(640.0 + 48.0, 340.0),
        move |ui, theme| {
            ui.set_width(640.0);
            invite_card(ui, theme, &Keymap::default(), &EVENT, &state);
        },
    )
}

/// The text painted that starts with `start`: its rect and colour.
fn text(paint: &Paint, start: &str) -> Option<(Rect, Color32, f32)> {
    paint.shapes.iter().find_map(|shape| match &shape.look {
        Look::Text { text, colour, size, .. } if text.starts_with(start) => Some((shape.rect, *colour, *size)),
        _ => None,
    })
}

#[test]
fn the_invitation_card_looks_as_its_sheet_says_in_every_state() {
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::default());
        let p = theme.palette;
        let f = theme.fonts;
        for (name, state, high) in states() {
            let harness = harness(theme, state);
            let label = match state {
                CardState::OutOfDate { line } => format!("{}: {line}", spoken(&EVENT)),
                _ => spoken(&EVENT),
            };
            let rect = harness.get_by_label(&label).rect();
            let paint = painted(&harness, &label);
            let at = format!("{mode:?} {name}: {paint:#?}");
            assert_eq!(rect.size(), vec2(640.0, high), "{at}");
            let look = own(&paint, rect);
            let edge = match state {
                CardState::Request { .. } | CardState::Changed { .. } | CardState::Clash { .. } => p.cyan_edge,
                CardState::Cancelled { .. } => p.minus,
                CardState::OutOfDate { .. } => p.panel_line,
                _ => p.line,
            };
            assert_eq!((look.fill, look.edge, look.radius), (Some(p.panel), Some((1.0, edge, false)), 0.0), "{at}");
            if let CardState::OutOfDate { line } = state {
                let (words, ink, _) = text(&paint, &line[..12]).unwrap_or_else(|| panic!("{at}"));
                assert_eq!((words.left() - rect.left(), ink), (38.0, p.muted), "{at}");
                assert!(look.texts.iter().any(|(text, colour, ..)| text == "\u{e1a0}" && *colour == p.muted), "the older clock: {at}");
                continue;
            }
            // The day's column.
            let (weekday, weekday_ink, _) = text(&paint, "MON").unwrap_or_else(|| panic!("{at}"));
            assert_eq!((weekday.top() - rect.top(), weekday_ink), (14.0, p.cyan_text), "{at}");
            let (day, day_ink, day_size) = text(&paint, "16").unwrap_or_else(|| panic!("{at}"));
            assert_eq!((day.top() - rect.top(), day_ink, day_size), (30.0, p.ink, f.display.size), "{at}");
            assert!((day.center().x - (rect.left() + 46.0)).abs() <= 1.0, "the day in the column's middle: {at}");
            // The hairline between the column and the rest.
            assert!(
                paint.shapes.iter().any(|shape| matches!(shape.look, Look::Rect { stroke: Some((1.0, c)), .. } if c == p.line)
                    && shape.rect.width() == 0.0
                    && (shape.rect.left() - (rect.left() + 92.5)).abs() < 0.01),
                "{at}"
            );
            let cancelled = matches!(state, CardState::Cancelled { .. });
            let (title, title_ink, title_size) = text(&paint, "Design review").unwrap_or_else(|| panic!("{at}"));
            assert_eq!(
                (title.left() - rect.left(), title.top() - rect.top(), title_ink, title_size),
                (108.0, 14.0, if cancelled { p.muted } else { p.ink }, f.heading.size),
                "{at}"
            );
            for (label, top) in [("WHEN", 49.0), ("WHERE", 77.0), ("WHO", 105.0), ("CAL", 133.0)] {
                let (found, ink, _) = text(&paint, label).unwrap_or_else(|| panic!("{label}: {at}"));
                assert_eq!((found.left() - rect.left(), found.top() - rect.top(), ink), (108.0, top, p.muted), "{label}: {at}");
            }
            let (when, when_ink, when_size) = text(&paint, "15:00").unwrap_or_else(|| panic!("{at}"));
            assert_eq!((when.left() - rect.left(), when_ink, when_size), (176.0, p.ink, f.mono.size), "{at}");
            let (cal, cal_ink, _) = text(&paint, "Studio").unwrap_or_else(|| panic!("{at}"));
            assert_eq!((cal.top() - rect.top(), cal_ink), (130.0, p.muted), "{at}");
            lower_part(&harness, &paint, &theme, rect, title, state, &at);
        }
    }
}

/// The part of the card under its facts in `state`, as the sheet draws it.
fn lower_part(harness: &Harness<'static>, paint: &Paint, theme: &Theme, rect: Rect, title: Rect, state: CardState<'static>, at: &str) {
    let p = theme.palette;
    let look = own(paint, rect);
    match state {
        CardState::Request { .. } | CardState::Changed { .. } | CardState::Clash { .. } => {
            for word in ["Going", "Maybe", "No"] {
                assert!(!harness.get_by_label(word).accesskit_node().is_disabled(), "{word}: {at}");
            }
            assert!(text(paint, "Your answer goes to marit@example.org at once.").is_some(), "{at}");
            if let CardState::Changed { .. } | CardState::Clash { .. } = state {
                let word = if matches!(state, CardState::Changed { .. }) { "CHANGED" } else { "CLASH" };
                assert_eq!(text(paint, word).map(|found| found.1), Some(p.warn), "{at}");
            }
        }
        CardState::Answered { answer } => {
            assert!(look.texts.iter().any(|(text, colour, ..)| text == "\u{e182}" && *colour == p.plus), "{at}");
            assert_eq!(text(paint, answer).map(|found| found.1), Some(p.ink), "{at}");
            harness.get_by_label("Change answer");
        }
        CardState::Cancelled { .. } => {
            assert_eq!(text(paint, "CANCELLED").map(|found| found.1), Some(p.minus), "{at}");
            harness.get_by_label("Remove from calendar");
            // The title struck through in muted.
            assert!(
                paint.rects().any(|shape| matches!(shape.look, Look::Rect { fill, .. } if fill == p.muted)
                    && shape.rect.height() == 1.0
                    && shape.rect.left() == title.left()
                    && shape.rect.width() >= title.width()),
                "{at}"
            );
        }
        CardState::Replies { replies } => {
            for ((said, words), glyph) in replies.iter().zip(["\u{e182}", "\u{e3e8}", "\u{e4f6}", "\u{e32a}"]) {
                let colour = match said {
                    Said::Going => p.plus,
                    Said::Maybe => p.warn,
                    Said::No => p.minus,
                    Said::Nothing => p.muted,
                };
                assert!(look.texts.iter().any(|(text, c, ..)| text == glyph && *c == colour), "{words}: {at}");
                assert_eq!(text(paint, words).map(|found| found.0.left() - rect.left()), Some(132.0), "{words}: {at}");
            }
        }
        CardState::NoAnswerAsked => assert!(text(paint, "This calendar message asks for no answer.").is_some(), "{at}"),
        CardState::CannotAnswer { .. } => assert!(text(paint, "This invitation cannot be answered").is_some(), "{at}"),
        CardState::OutOfDate { .. } => {}
    }
}

#[test]
fn going_answers_and_a_focused_button_in_an_asking_card_keeps_its_own_ring() {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let asked = std::rc::Rc::new(std::cell::Cell::new(None));
    let kept = asked.clone();
    let mut harness = staged(
        theme,
        |p| p.surface,
        vec2(700.0, 340.0),
        move |ui, theme| {
            if let Some(got) = invite_card(ui, theme, &Keymap::default(), &EVENT, &CardState::Request { goes_to: GOES }) {
                kept.set(Some(got));
            }
        },
    );
    harness.get_by_label("Maybe").focus();
    harness.run();
    let ring = painted(&harness, "Maybe");
    let maybe = harness.get_by_label("Maybe").rect();
    let found = ring.rects().any(|shape| {
        matches!(shape.look, Look::Rect { stroke: Some((1.0, c)), stroke_kind: StrokeKind::Outside, .. } if c == theme.palette.cyan_edge)
            && shape.rect.contains_rect(maybe)
    });
    assert!(found, "{ring:#?}");
    harness.get_by_label("Going").click();
    harness.run();
    assert_eq!(asked.get(), Some(CardAsked::Answer(0)));
}

#[test]
fn hostile_event_words_stay_on_their_lines() {
    let theme = Theme::new(Mode::Dark, TextScale::ALL[4]);
    let title: &'static str = Box::leak(format!("\u{202E}{}", "T".repeat(400)).into_boxed_str());
    let event = CardEvent { title, ..EVENT };
    let harness = staged(
        theme,
        |p| p.surface,
        vec2(700.0, 420.0),
        move |ui, theme| {
            ui.set_width(640.0);
            invite_card(ui, theme, &Keymap::default(), &event, &CardState::NoAnswerAsked);
        },
    );
    let label = spoken(&event);
    assert!(!label.contains('\u{202E}'));
    let rect = harness.get_by_label(&label).rect();
    let paint = painted(&harness, &label);
    let (cut, ..) = text(&paint, "TTT").unwrap_or_else(|| panic!("{paint:#?}"));
    assert!(cut.right() <= rect.right() - 16.0 + 1.0, "{cut:?}");
}

#[test]
fn an_organiser_who_is_not_the_sender_is_named_as_whom_the_answer_goes_to_on_one_line() {
    // At 130 %: the card of an organiser who did not send the message is the same height, its line says so, and an
    // address too long for the line is cut with one ellipsis inside the card.
    let theme = Theme::new(Mode::Light, TextScale::ALL[4]);
    let long: &'static str = Box::leak(format!("{}@example.org", "o".repeat(600)).into_boxed_str());
    let card = |event: CardEvent<'static>, goes_to: &'static str| {
        let harness = staged(
            theme,
            |p| p.surface,
            vec2(640.0 + 48.0, 420.0),
            move |ui, theme| {
                ui.set_width(640.0);
                invite_card(ui, theme, &Keymap::default(), &event, &CardState::Request { goes_to });
            },
        );
        let label = spoken(&event);
        let rect = harness.get_by_label(&label).rect();
        (painted(&harness, &label), rect)
    };
    let (same, same_rect) = card(EVENT, GOES);
    assert!(text(&same, "Your answer goes to marit@example.org at once.").is_some(), "as today");
    let other = CardEvent { not_from_organiser: true, ..EVENT };
    let (paint, rect) = card(other, GOES);
    assert_eq!(rect.height(), same_rect.height(), "one line either way");
    assert!(text(&paint, "Your answer goes at once to the organiser, not to the sender: marit").is_some(), "{paint:#?}");
    assert!(text(&paint, "Your answer goes to marit").is_none(), "the line of an organiser who sent it is not drawn");
    let (paint, rect) = card(other, long);
    let (cut, ..) = text(&paint, "Your answer goes at once to the organiser").unwrap_or_else(|| panic!("{paint:#?}"));
    assert!(cut.right() <= rect.right() - 16.0 + 1.0, "{cut:?} inside {rect:?}");
    let words = paint.shapes.iter().find_map(|shape| match &shape.look {
        Look::Text { text, .. } if text.starts_with("Your answer goes at once") => Some(text.clone()),
        _ => None,
    });
    assert_eq!(words.map(|words| words.matches('…').count()), Some(1), "cut with one ellipsis");
}
