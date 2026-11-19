// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The status message on its own: every kind of its sheet in dark and light at 100 % and 130 % text, its two presses
//! by the pointer and the keyboard and none elsewhere, and a long hostile failure kept to one line inside it.

use egui::{Color32, Rect, Shape, Vec2, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::{Kind, Left, Press, StatusMessage, status_message};
use crate::component::gallery::keymap;
use crate::component::tests::{Look, Paint, builder, keyboard_walk, painted, rests, stage};
use crate::iconset::{self, IconId};
use crate::motion;
use crate::theme::{Mode, Palette, TextScale, Theme};

const KEY: Option<&str> = Some("Ctrl Z");
const LEFT: Option<Left> = Some(Left { seconds: 4, share: 0.8 });

/// The sheet's kinds with their words.
fn kinds() -> [(&'static str, StatusMessage<'static>); 5] {
    [
        ("undo", StatusMessage { kind: Kind::Undo { key: KEY, left: LEFT }, text: "done · Draft logo for the bakery" }),
        ("send-countdown", StatusMessage { kind: Kind::Countdown { key: KEY, left: LEFT }, text: "sending · Re: Thursday workshop" }),
        ("notice", StatusMessage { kind: Kind::Notice, text: "Link copied: https://example.org/open-day" }),
        ("error", StatusMessage { kind: Kind::Error, text: "Studio: sign-in refused, mail is not being fetched" }),
        ("done", StatusMessage { kind: Kind::Done, text: "sent 14:02 · Re: Thursday workshop" }),
    ]
}

/// A page with `message` `width` wide on the status line's `panel`; the presses of the frames are its state.
fn harness(theme: Theme, message: StatusMessage<'static>, width: f32) -> Harness<'static, Vec<Press>> {
    let keymap = keymap();
    let mut harness = builder().with_size(vec2(width + 48.0, 100.0)).build_ui_state(
        move |ui, presses: &mut Vec<Press>| {
            motion::set_reduced(ui.ctx(), true);
            stage(ui, theme.palette.panel, |ui| {
                ui.set_max_width(width);
                if let Some(press) = status_message(ui, &theme, &keymap, &message) {
                    presses.push(press);
                }
            });
        },
        Vec::new(),
    );
    harness.run();
    harness
}

/// The texts of `paint` that are words (not icons): where each stands against `rect`, its words and colour.
fn words(paint: &Paint, rect: Rect) -> Vec<(Vec2, String, Color32)> {
    let icons = crate::component::tests::icons();
    paint
        .shapes
        .iter()
        .filter_map(|shape| match &shape.look {
            Look::Text { text, colour, family, .. } if !text.is_empty() && *family != icons => {
                Some((shape.rect.min - rect.min, text.clone(), *colour))
            }
            _ => None,
        })
        .collect()
}

/// The icon glyphs of `paint` and their colours.
fn icons(paint: &Paint) -> Vec<(String, Color32)> {
    let icons = crate::component::tests::icons();
    paint
        .shapes
        .iter()
        .filter_map(|shape| match &shape.look {
            Look::Text { text, colour, family, .. } if *family == icons => Some((text.clone(), *colour)),
            _ => None,
        })
        .collect()
}

/// The filled rects of `paint` against `rect`: where, how large, what colour.
fn fills(paint: &Paint, rect: Rect) -> Vec<(Rect, Color32)> {
    paint
        .rects()
        .filter_map(|shape| match shape.look {
            Look::Rect { fill, stroke: None, .. } if fill.a() > 0 => Some((shape.rect.translate(-rect.min.to_vec2()), fill)),
            _ => None,
        })
        .collect()
}

/// What each kind draws, by the sheet: its line's colour, its mark (an icon's glyph and colour, or the disc's
/// colour), its text's left and colour, and its parts' words with their lefts and colours.
struct Expected {
    line: Color32,
    icon: Option<(char, Color32)>,
    disc: Option<Color32>,
    text: (f32, Color32),
    parts: Vec<(&'static str, f32, Color32)>,
}

fn expected(state: &str, p: &Palette) -> Expected {
    let undo = |icon| Expected {
        line: p.cyan_edge,
        icon: Some((icon, p.cyan_text)),
        disc: None,
        text: (42.0, p.ink),
        parts: vec![("undo", 345.0, p.cyan_text), ("Ctrl Z", 378.0 + 5.0, p.muted), ("4 s", 528.0, p.muted)],
    };
    match state {
        "undo" => undo(iconset::glyph(IconId::Check)),
        "send-countdown" => undo('\u{e08e}'),
        "notice" => Expected { line: p.muted, icon: Some((iconset::glyph(IconId::Info), p.muted)), disc: None, text: (42.0, p.ink), parts: vec![] },
        "error" => Expected {
            line: p.minus,
            icon: None,
            disc: Some(p.minus),
            text: (39.0, p.minus),
            parts: vec![("open log", 432.0, p.muted), ("dismiss", 501.0, p.cyan_text)],
        },
        _ => Expected { line: p.plus, icon: Some((iconset::glyph(IconId::Check), p.plus)), disc: None, text: (42.0, p.ink), parts: vec![] },
    }
}

#[test]
fn every_kind_looks_as_its_sheet_says_in_dark_and_light() {
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::default());
        let p = theme.palette;
        for (state, message) in kinds() {
            let harness = harness(theme, message, 560.0);
            let rect = harness.get_by_label(message.text).rect();
            let paint = painted(&harness, message.text);
            let at = format!("{mode:?} {state}: {paint:#?}");
            let want = expected(state, p);
            assert_eq!(rect.size(), vec2(560.0, 26.0), "{at}");
            let fills = fills(&paint, rect);
            assert_eq!(fills.first(), Some(&(Rect::from_min_size(pos2(0.0, 0.0), vec2(560.0, 26.0)), p.panel)), "the panel fill: {at}");
            assert!(fills.contains(&(Rect::from_min_size(pos2(0.0, 0.0), vec2(560.0, 2.0)), want.line)), "the 2 px line: {at}");
            let words = words(&paint, rect);
            let (text_at, text, ink) = words.first().cloned().unwrap_or_else(|| panic!("no text: {at}"));
            assert_eq!((text_at, text.as_str(), ink), (vec2(want.text.0, 5.0), message.text, want.text.1), "{at}");
            for (part, left, colour) in &want.parts {
                let found = words.iter().find(|(_, text, _)| text == part).unwrap_or_else(|| panic!("{part}: {at}"));
                assert_eq!((found.0.x, found.2), (*left, *colour), "{part}: {at}");
            }
            assert_eq!(icons(&paint), want.icon.map(|(glyph, colour)| (glyph.to_string(), colour)).into_iter().collect::<Vec<_>>(), "{at}");
            if let Some(colour) = want.disc {
                assert!(
                    paint.rects().any(|shape| matches!(shape.look, Look::Rect { fill, .. } if fill == colour) && shape.rect.width() < 10.0),
                    "the disc: {at}"
                );
            }
            if state == "undo" || state == "send-countdown" {
                assert!(fills.contains(&(Rect::from_min_size(pos2(438.0, 12.0), vec2(80.0, 2.0)), p.stage)), "the bar: {at}");
                assert!(fills.contains(&(Rect::from_min_size(pos2(438.0, 12.0), vec2(64.0, 2.0)), p.cyan_edge)), "4 of 5 s left: {at}");
                assert_eq!(harness.get_by_label("undo").rect().left() - rect.left(), 345.0, "{at}");
            }
        }
    }
}

#[test]
fn at_130_percent_the_message_is_34_high_and_its_paddings_and_bar_stay() {
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::ALL[4]);
        let p = theme.palette;
        for (state, message) in kinds() {
            let harness = harness(theme, message, 760.0);
            let rect = harness.get_by_label(message.text).rect();
            let paint = painted(&harness, message.text);
            let at = format!("{mode:?} {state}");
            assert_eq!(rect.size(), vec2(760.0, 34.0), "{at}");
            let fills = fills(&paint, rect);
            assert!(fills.contains(&(Rect::from_min_size(pos2(0.0, 0.0), vec2(760.0, 2.0)), expected(state, p).line)), "{at}");
            let words = words(&paint, rect);
            // 18 in; the icon as large as a scaled mark (18) or the sign as wide as the larger text (15); 10 after it.
            let left = if state == "error" { 18.0 + 15.0 + 10.0 } else { 18.0 + theme.mark_side() + 10.0 };
            assert_eq!(words[0].0.x, left, "{at}");
            assert_eq!(words[0].0.y, ((34.0 - theme.fonts.mono_s.line) / 2.0).round(), "{at}");
            let measure = |text: &str| {
                crate::draw::paint::run_box(
                    harness.ctx.fonts_mut(|fonts| fonts.layout_no_wrap(text.into(), theme.fonts.mono_s.id(), p.muted).size().x),
                )
            };
            let at_x =
                |part: &str| words.iter().find(|(_, text, _)| text == part).map(|(offset, ..)| offset.x).unwrap_or_else(|| panic!("{part}: {at}"));
            let from = |name: &str| {
                let node = harness.get_by_label(name).rect();
                (node.left() - rect.left(), node.right() - rect.left())
            };
            match state {
                "undo" | "send-countdown" => {
                    // From the right: 12, the seconds, 10, the bar 80 × 2 in the middle, 10, undo, 6, its key's chip.
                    let seconds = at_x("4 s");
                    assert_eq!(seconds + measure("4 s"), 760.0 - 12.0, "12 after the seconds: {at}");
                    let bar = seconds - 10.0 - 80.0;
                    assert!(fills.contains(&(Rect::from_min_size(pos2(bar, 16.0), vec2(80.0, 2.0)), p.stage)), "the bar: {at}");
                    assert!(fills.contains(&(Rect::from_min_size(pos2(bar, 16.0), vec2(64.0, 2.0)), p.cyan_edge)), "4 of 5 s: {at}");
                    let (_, undo_right) = from("undo");
                    assert_eq!(at_x("Ctrl Z"), undo_right + 6.0 + 5.0, "the chip 6 after undo, its key 5 in: {at}");
                    assert!(at_x("Ctrl Z") + measure("Ctrl Z") <= bar - 10.0, "the chip before the bar: {at}");
                }
                "error" => {
                    let (log_left, log_right) = from("open log");
                    let (dismiss_left, dismiss_right) = from("dismiss");
                    assert_eq!(dismiss_right, 760.0 - 12.0, "{at}");
                    assert_eq!(dismiss_left, log_right + 16.0, "{at}");
                    assert!(words[0].0.x + measure(&words[0].1) <= log_left - 10.0, "the text 10 before the links: {at}");
                }
                _ => assert_eq!(words.len(), 1, "the text alone: {at}"),
            }
        }
    }
}

#[test]
fn undo_and_dismiss_are_pressed_by_the_pointer_enter_and_space() {
    let theme = Theme::new(Mode::Light, TextScale::default());
    for (state, message) in kinds() {
        let mut harness = harness(theme, message, 560.0);
        let (name, press) = match state {
            "undo" | "send-countdown" => ("undo", Press::Undo),
            "error" => ("dismiss", Press::Dismiss),
            _ => {
                harness.key_press(egui::Key::Tab);
                harness.run();
                assert!(harness.ctx.memory(|memory| memory.focused()).is_none(), "{state}: nothing to press");
                assert!(rests(&mut harness, 8), "{state}");
                continue;
            }
        };
        harness.get_by_label(name).click();
        harness.run();
        for key in [egui::Key::Enter, egui::Key::Space] {
            harness.get_by_label(name).focus();
            harness.run();
            harness.key_press(key);
            harness.run();
        }
        assert_eq!(*harness.state(), vec![press; 3], "{state}");
        let missed = keyboard_walk(&mut harness);
        assert!(missed.is_empty(), "{state}: {missed:?}");
        assert!(rests(&mut harness, 8), "{state}");
    }
}

#[test]
fn the_log_is_coming_soon_and_takes_no_press() {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let (_, message) = kinds()[3];
    let mut harness = harness(theme, message, 560.0);
    let log = harness.get_by_label("open log");
    assert!(log.accesskit_node().is_disabled());
    log.click();
    harness.run();
    assert!(harness.state().is_empty());
}

#[test]
fn without_a_bar_the_undo_line_has_no_countdown_and_its_text_takes_the_room() {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let message = StatusMessage { kind: Kind::Undo { key: None, left: None }, text: "done · Draft logo for the bakery" };
    let harness = harness(theme, message, 560.0);
    let rect = harness.get_by_label(message.text).rect();
    let paint = painted(&harness, message.text);
    assert!(!fills(&paint, rect).iter().any(|(r, _)| r.width() == 80.0), "no bar");
    assert!(!words(&paint, rect).iter().any(|(_, text, _)| text.ends_with(" s")), "no seconds");
    assert_eq!(harness.get_by_label("undo").rect().right(), rect.right() - 12.0, "undo at the end");
}

/// Every text and filled rect the last frame painted, as far as its clip lets it show.
fn shown(harness: &Harness<'static, Vec<Press>>) -> Vec<(Rect, Option<String>)> {
    fn open(shape: &Shape, clip: Rect, out: &mut Vec<(Rect, Option<String>)>) {
        match shape {
            Shape::Vec(inner) => inner.iter().for_each(|shape| open(shape, clip, out)),
            Shape::Text(text) => out.push((text.visual_bounding_rect().intersect(clip), Some(text.galley.text().to_owned()))),
            Shape::Rect(rect) => out.push((rect.rect.intersect(clip), None)),
            Shape::Circle(circle) => out.push((Rect::from_center_size(circle.center, Vec2::splat(circle.radius * 2.0)).intersect(clip), None)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    harness.output().shapes.iter().for_each(|clipped| open(&clipped.shape, clipped.clip_rect, &mut out));
    out
}

#[test]
fn a_hostile_10000_character_failure_is_one_clean_line_inside_the_message_cleaned_or_not() {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let raw: &'static str = Box::leak(format!("Studio\r\n\0\u{202E}{}", "refused \u{2067}x\u{301}\u{301} ".repeat(800)).into_boxed_str());
    let cleaned: &'static str =
        Box::leak(raw.chars().filter(|c| !c.is_control() && !('\u{202A}'..='\u{2069}').contains(c)).collect::<String>().into_boxed_str());
    assert!(raw.chars().count() > 10_000);
    for text in [raw, cleaned] {
        let harness = harness(theme, StatusMessage { kind: Kind::Error, text }, 560.0);
        let node = harness.query_all_by_role(egui::accesskit::Role::Label).find(|node| node.rect().width() == 560.0).unwrap();
        let rect = node.rect();
        let heard = node.accesskit_node().label().or_else(|| node.accesskit_node().value()).unwrap_or_default();
        assert!(heard.starts_with("Studio"), "{heard:?}");
        assert!(heard.chars().count() <= crate::component::MOST, "{}", heard.chars().count());
        assert!(!heard.contains(['\r', '\n', '\0', '\u{202E}', '\u{2067}']), "{heard:?}");
        let log = harness.get_by_label("open log").rect();
        let lines: Vec<_> = shown(&harness).into_iter().filter(|(_, text)| text.as_ref().is_some_and(|text| text.starts_with("Studio"))).collect();
        assert_eq!(lines.len(), 1);
        let (bounds, line) = &lines[0];
        let line = line.clone().unwrap_or_default();
        assert!(line.ends_with('…') && line.matches('…').count() == 1, "{line:?}");
        assert!(!line.contains(['\r', '\n', '\0', '\u{202E}', '\u{2067}']), "{line:?}");
        assert!(rect.contains_rect(*bounds), "{bounds:?} leaves {rect:?}");
        assert!(bounds.right() <= log.left() - 10.0 + 0.5, "{bounds:?} reaches the links at {log:?}");
    }
}

#[test]
fn in_a_narrow_line_nothing_is_painted_past_the_message() {
    let theme = Theme::new(Mode::Light, TextScale::default());
    for (state, message) in kinds() {
        for width in [120.0, 1.0] {
            let harness = harness(theme, message, width);
            let rect = harness.get_by_label(message.text).rect();
            for (bounds, text) in shown(&harness) {
                // The stage's ground stands under it.
                if (bounds.contains_rect(rect) && bounds != rect) || !bounds.is_positive() {
                    continue;
                }
                assert!(rect.expand(0.5).contains_rect(bounds), "{state} {width}: {text:?} at {bounds:?} leaves {rect:?}");
            }
        }
    }
}

#[test]
fn the_countdown_draws_any_share_and_any_seconds_inside_the_message() {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let p = theme.palette;
    // A share that is not a number or below 0 lights none of the bar, above 1 or infinite all of it; the seconds are
    // written as given, the largest included.
    for seconds in [0, u32::MAX] {
        for (share, lit) in [(f32::NAN, 0.0), (-1.0, 0.0), (f32::NEG_INFINITY, 0.0), (2.0, 80.0), (f32::INFINITY, 80.0)] {
            let message =
                StatusMessage { kind: Kind::Countdown { key: KEY, left: Some(Left { seconds, share }) }, text: "sending · Re: Thursday workshop" };
            let harness = harness(theme, message, 560.0);
            let rect = harness.get_by_label(message.text).rect();
            let paint = painted(&harness, message.text);
            let at = format!("{seconds} {share}");
            let fills = fills(&paint, rect);
            let accent: Vec<_> = fills.iter().filter(|(r, c)| *c == p.cyan_edge && r.height() == 2.0 && r.top() > 0.0).collect();
            assert_eq!(accent.iter().map(|(r, _)| r.width()).sum::<f32>(), lit, "{at}: {accent:?}");
            let track = fills.iter().find(|(r, c)| *c == p.stage && r.size() == vec2(80.0, 2.0)).map(|(r, _)| *r).unwrap_or_else(|| panic!("{at}"));
            for (r, _) in &accent {
                assert!(track.contains_rect(*r), "{at}: the lit part {r:?} inside its track {track:?}");
            }
            let words = words(&paint, rect);
            let written = format!("{seconds} s");
            let (offset, ..) = words.iter().find(|(_, text, _)| *text == written).unwrap_or_else(|| panic!("{at}: {words:?}"));
            assert!(offset.x >= 0.0 && rect.left() + offset.x < rect.right(), "{at}");
            assert!(words[0].1.starts_with("sending"), "the text keeps its room: {at}");
            for shape in &paint.shapes {
                assert!(rect.expand(0.5).contains_rect(shape.rect) || !shape.inside, "{at}: {shape:?}");
                assert!(!shape.overflows, "{at}: {shape:?}");
            }
        }
    }
}
