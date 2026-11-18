// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::{Color32, Rect, StrokeKind, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

use super::{ThreadLine, spoken, thread_row_with};
use crate::component::Shown;
use crate::component::tests::{Look, Paint, painted, staged};
use crate::keymap::Keymap;
use crate::theme::{Mode, TextScale, Theme};

const FOLDED: ThreadLine<'static> = ThreadLine {
    sender: "Marit Solheim",
    address: "marit@example.org",
    preview: "Room 3 is free from ten. I can bring the projector, and Ines has the keys to the store room.",
    date: "13:40",
    unread: false,
    open: false,
};

const OPEN: ThreadLine<'static> = ThreadLine { preview: "marit@example.org  ·  to you and Teodor", open: true, ..FOLDED };

/// The sheet's states: name, line, and the state forced.
fn states() -> [(&'static str, ThreadLine<'static>, Shown); 5] {
    [
        ("folded", FOLDED, Shown::Rest),
        ("open", OPEN, Shown::Rest),
        ("unread", ThreadLine { unread: true, ..FOLDED }, Shown::Rest),
        ("hover", FOLDED, Shown::Hover),
        ("focus", FOLDED, Shown::Focus),
    ]
}

/// The row 640 wide on the reader's `surface`.
fn harness(theme: Theme, line: ThreadLine<'static>, shown: Shown) -> Harness<'static> {
    staged(
        theme,
        |p| p.surface,
        vec2(640.0 + 48.0, 100.0),
        move |ui, theme| {
            ui.set_width(640.0);
            thread_row_with(ui, theme, &Keymap::default(), &line, Some(shown));
        },
    )
}

/// The text painted that starts with `start`: its rect, colour and size.
fn text(paint: &Paint, start: &str) -> Option<(Rect, Color32, f32)> {
    paint.shapes.iter().find_map(|shape| match &shape.look {
        Look::Text { text, colour, size, .. } if text.starts_with(start) => Some((shape.rect, *colour, *size)),
        _ => None,
    })
}

#[test]
fn the_thread_row_looks_as_its_sheet_says_in_every_state() {
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::default());
        let p = theme.palette;
        let f = theme.fonts;
        for (state, line, shown) in states() {
            let harness = harness(theme, line, shown);
            let name = spoken(&line);
            let rect = harness.get_by_label(&name).rect();
            let paint = painted(&harness, &name);
            let at = format!("{mode:?} {state}: {paint:#?}");
            assert_eq!(rect.size(), vec2(640.0, 40.0), "{at}");
            let fill =
                |colour: Color32| paint.rects().any(|shape| shape.rect == rect && matches!(shape.look, Look::Rect { fill, .. } if fill == colour));
            assert_eq!(fill(p.stage), shown == Shown::Hover, "the stage fill under the pointer only: {at}");
            // The 1 px panel_line at its foot.
            assert!(
                paint.shapes.iter().any(|shape| matches!(shape.look, Look::Rect { stroke: Some((1.0, c)), .. } if c == p.panel_line)
                    && (shape.rect.center().y - (rect.bottom() - 0.5)).abs() < 0.01),
                "{at}"
            );
            // The unread square, 6 × 6 at 28, cyan_edge, only when unread.
            let square = Rect::from_min_size(pos2(rect.left() + 28.0, rect.top() + 17.0), vec2(6.0, 6.0));
            let squared = paint.rects().any(|shape| shape.rect == square && matches!(shape.look, Look::Rect { fill, .. } if fill == p.cyan_edge));
            assert_eq!(squared, line.unread, "{at}");
            // The avatar: 24 at 44, stage, surface under the pointer.
            let face = Rect::from_min_size(pos2(rect.left() + 44.0, rect.top() + 8.0), vec2(24.0, 24.0));
            let ground = if shown == Shown::Hover { p.surface } else { p.stage };
            assert!(
                paint
                    .rects()
                    .any(|shape| shape.rect == face && matches!(shape.look, Look::Rect { fill, radius, .. } if fill == ground && radius == 2.0)),
                "{at}"
            );
            let strong = line.open || line.unread;
            let (sender_rect, sender_ink, sender_size) = text(&paint, "Marit Solheim").unwrap_or_else(|| panic!("sender: {at}"));
            assert_eq!((sender_rect.left() - rect.left(), sender_ink, sender_size), (78.0, p.ink, f.body.size), "{at}");
            let family = paint.shapes.iter().find_map(|shape| match &shape.look {
                Look::Text { text, family, .. } if text == "Marit Solheim" => Some(family.clone()),
                _ => None,
            });
            let expected = if strong { f.strong } else { f.body }.id().family.to_string();
            assert_eq!(family, Some(expected), "{at}");
            let start = if line.open { "marit@" } else { "Room 3" };
            let (preview_rect, preview_ink, preview_size) = text(&paint, start).unwrap_or_else(|| panic!("preview: {at}"));
            let (want_ink, want_size) = match (line.open, line.unread) {
                (true, _) => (p.muted, f.mono_s.size),
                (false, true) => (p.ink, f.caption.size),
                (false, false) => (p.muted, f.caption.size),
            };
            assert_eq!((preview_rect.left() - rect.left(), preview_ink, preview_size), (240.0, want_ink, want_size), "{at}");
            let (date_rect, date_ink, _) = text(&paint, "13:40").unwrap_or_else(|| panic!("date: {at}"));
            assert!((date_rect.right() - (rect.right() - 12.0)).abs() <= 1.0, "the date ends 12 in: {at}");
            assert_eq!(date_ink, if line.unread { p.cyan_text } else { p.muted }, "{at}");
            assert!(preview_rect.right() <= date_rect.left() - 10.0, "{at}");
            // The caret, drawn in code in muted, 12 in.
            assert!(
                paint.shapes.iter().any(|shape| matches!(shape.look, Look::Rect { fill, .. } if fill == p.muted)
                    && (shape.rect.left() - (rect.left() + 12.0)).abs() < 0.5
                    && shape.rect.width() < 7.0),
                "the caret: {at}"
            );
            let ring = paint
                .rects()
                .find(|shape| matches!(shape.look, Look::Rect { stroke: Some((1.0, c)), stroke_kind: StrokeKind::Inside, .. } if c == p.cyan_edge));
            if shown == Shown::Focus {
                let ring = ring.unwrap_or_else(|| panic!("the ring: {at}"));
                assert_eq!(ring.rect, Rect::from_min_size(rect.min + vec2(2.0, 2.0), vec2(636.0, 35.0)), "{at}");
            } else {
                assert!(ring.is_none(), "{at}");
            }
            if line.open {
                let link = harness.get_by_label("details ▾").rect();
                assert_eq!((link.right(), link.height()), (date_rect.left().floor() - 10.0, 16.0), "{at}");
            } else {
                assert_eq!(harness.query_all_by_label("details ▾").count(), 0, "{at}");
            }
            assert!(paint.rects().all(|shape| !matches!(shape.look, Look::Rect { radius, .. } if radius > 2.0)), "{at}");
        }
    }
}

#[test]
fn the_thread_row_keeps_its_scaled_height_and_its_columns_at_130_percent_with_hostile_words() {
    let theme = Theme::new(Mode::Dark, TextScale::ALL[4]);
    let sender: &'static str = Box::leak(format!("\u{202E}{}", "W".repeat(200)).into_boxed_str());
    let preview: &'static str = Box::leak("x".repeat(400).into_boxed_str());
    let line = ThreadLine { sender, preview, ..FOLDED };
    let harness = harness(theme, line, Shown::Rest);
    let name = spoken(&line);
    assert!(!name.contains('\u{202E}'));
    let rect = harness.get_by_label(&name).rect();
    assert_eq!(rect.height(), theme.size(40.0));
    let paint = painted(&harness, &name);
    let (sender_rect, ..) = text(&paint, "WWW").unwrap_or_else(|| panic!("{paint:#?}"));
    let (preview_rect, ..) = text(&paint, "xxx").unwrap_or_else(|| panic!("{paint:#?}"));
    let (date_rect, ..) = text(&paint, "13:40").unwrap_or_else(|| panic!("{paint:#?}"));
    assert!(sender_rect.right() <= preview_rect.left() - 10.0 + 0.5, "the sender stays in its column");
    assert!(preview_rect.right() <= date_rect.left() - 10.0 + 0.5, "nothing overlaps the date");
    for start in ["WWW", "xxx"] {
        let ended = paint.shapes.iter().any(|shape| matches!(&shape.look, Look::Text { text, .. } if text.starts_with(start) && text.ends_with('…')));
        assert!(ended, "{start} cut with one ellipsis");
    }
}

/// The row `room` wide on the reader's `surface`.
fn narrow(theme: Theme, line: ThreadLine<'static>, room: f32) -> Harness<'static> {
    staged(
        theme,
        |p| p.surface,
        vec2(room + 48.0, 100.0),
        move |ui, theme| {
            ui.set_width(room);
            thread_row_with(ui, theme, &Keymap::default(), &line, Some(Shown::Rest));
        },
    )
}

#[test]
fn in_a_narrow_row_nothing_paints_outside_its_box_and_the_date_stays_whole() {
    let long = ThreadLine { sender: "Odile Brannock-Harbourmaster", ..FOLDED };
    let open = ThreadLine { sender: "Odile Brannock-Harbourmaster", ..OPEN };
    for scale in [TextScale::default(), TextScale::ALL[4]] {
        let theme = Theme::new(Mode::Dark, scale);
        for room in [640.0, 420.0, 340.0, 280.0, 240.0, 200.0] {
            for (state, line) in [("folded", long), ("open", open), ("unread", ThreadLine { unread: true, ..long })] {
                let harness = narrow(theme, line, room);
                let name = spoken(&line);
                let rect = harness.get_by_label(&name).rect();
                let paint = painted(&harness, &name);
                let at = format!("{scale:?} {room} {state}");
                let texts: Vec<(Rect, String)> = paint
                    .shapes
                    .iter()
                    .filter_map(|shape| match &shape.look {
                        Look::Text { text, .. } => Some((shape.rect, text.clone())),
                        _ => None,
                    })
                    .collect();
                for (shape, text) in &texts {
                    // Cut by the one rule, never clipped at the row's edge.
                    assert!(rect.expand(0.01).contains_rect(*shape), "{text:?} at {shape:?} paints past the row {rect:?}: {at}");
                }
                for (index, (one, text)) in texts.iter().enumerate() {
                    for (other, word) in texts.iter().skip(index + 1) {
                        assert!(one.intersect(*other).area() <= 0.5 || one.width() < 1.0, "{text:?} runs over {word:?}: {at}");
                    }
                }
                // The date never gives way.
                assert!(texts.iter().any(|(_, text)| text == "13:40"), "the date whole: {texts:?} {at}");
                // `details ▾`, where it stands, keeps inside the row.
                if let Some(link) = harness.query_by_label("details ▾") {
                    assert!(rect.expand(0.01).contains_rect(link.rect()), "details at {:?} in {rect:?}: {at}", link.rect());
                }
            }
        }
    }
}

/// The row `room` wide on the reader's `surface` at `ppp` pixels per point.
fn narrow_at(theme: Theme, line: ThreadLine<'static>, room: f32, ppp: f32) -> Harness<'static> {
    let mut harness = crate::component::tests::builder().with_size(vec2(room + 48.0, 100.0)).with_pixels_per_point(ppp).build_ui(move |ui| {
        crate::motion::set_reduced(ui.ctx(), true);
        crate::component::tests::stage(ui, theme.palette.surface, |ui| {
            ui.set_width(room);
            thread_row_with(ui, &theme, &Keymap::default(), &line, Some(Shown::Rest));
        });
    });
    harness.run();
    harness
}

#[test]
fn every_room_from_the_least_to_400_cuts_the_words_then_the_sender_and_leaves_details_out_last() {
    const LONG: &str = "Odile Brannock-Harbourmaster";
    let lines = [("folded", ThreadLine { sender: LONG, ..FOLDED }), ("open", ThreadLine { sender: LONG, ..OPEN }), ("open, short sender", OPEN)];
    for scale in [TextScale::default(), TextScale::ALL[4]] {
        let theme = Theme::new(Mode::Dark, scale);
        for ppp in [1.0_f32, 1.5] {
            for (state, line) in lines {
                let first = |text: &str, of: &str| text.chars().next().is_some() && text.chars().next() == of.chars().next();
                // What stood in the room before, from the widest down: once a part has given way, it never comes back
                // in a narrower room. The sender as it stands in its whole column, at 400.
                let (mut had_details, mut had_words, mut had_sender) = (true, true, true);
                let mut column: Option<String> = None;
                // Down to below the row's least (the avatar and the date: 125 at 100 %, 143 at 130 %).
                let least = if scale == TextScale::default() { 125.0 } else { 143.0 };
                for room in (120..=400).rev().step_by(2) {
                    let room = room as f32;
                    let harness = narrow_at(theme, line, room, ppp);
                    let name = spoken(&line);
                    let rect = harness.get_by_label(&name).rect();
                    let paint = painted(&harness, &name);
                    let at = format!("{scale:?} {ppp}x {state} room {room}");
                    let texts: Vec<(Rect, String)> = paint
                        .shapes
                        .iter()
                        .filter_map(|shape| match &shape.look {
                            Look::Text { text, .. } => Some((shape.rect, text.clone())),
                            _ => None,
                        })
                        .collect();
                    for (shape, text) in &texts {
                        assert!(rect.expand(0.01).contains_rect(*shape), "{text:?} at {shape:?} paints past the row {rect:?}: {at}");
                        assert!(text.trim() != "…", "a lone ellipsis: {at}");
                    }
                    for (index, (one, text)) in texts.iter().enumerate() {
                        for (other, word) in texts.iter().skip(index + 1) {
                            assert!(one.intersect(*other).area() <= 0.5 || one.width() < 1.0, "{text:?} runs over {word:?}: {at}");
                        }
                    }
                    // The date is whole while the row holds the avatar and the date.
                    assert!(room < least || texts.iter().any(|(_, text)| text == line.date), "the date whole: {texts:?} {at}");
                    let sender = texts.iter().find(|(_, text)| first(text, line.sender) && text.chars().count() > 2).map(|(_, text)| text.clone());
                    let words = texts.iter().any(|(_, text)| first(text, line.preview));
                    let details = harness.query_by_label("details ▾").is_some();
                    if let Some(link) = harness.query_by_label("details ▾") {
                        assert!(rect.expand(0.01).contains_rect(link.rect()), "details at {:?} in {rect:?}: {at}", link.rect());
                    }
                    // The words give way first: they stand only beside the whole sender, and on an open line only with
                    // `details ▾`.
                    if column.is_none() {
                        column.clone_from(&sender);
                    }
                    if words {
                        assert!(sender == column, "words beside a sender cut below its column {sender:?}: {at}");
                        assert!(!line.open || details, "the words stand without details: {at}");
                    }
                    // `details ▾` leaves last of all: an open line without it holds no sender and no words.
                    if line.open && !details {
                        assert!(sender.is_none() && !words, "details left before the sender {sender:?}: {at}");
                    }
                    if room == 400.0 {
                        // At 130 % the words already have no room left at 400.
                        let words_fit = scale == TextScale::default();
                        assert!(sender.is_some() && (words || !words_fit) && (details || !line.open), "every part stands at 400: {texts:?} {at}");
                    }
                    assert!(had_details || !details, "details came back: {at}");
                    assert!(had_words || !words, "the words came back: {at}");
                    assert!(had_sender || sender.is_none(), "the sender came back: {at}");
                    (had_details, had_words, had_sender) = (details || !line.open, words, sender.is_some());
                }
            }
        }
    }
}
