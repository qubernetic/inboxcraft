// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A status item: one fact of the status line, as its sheet draws it. 26 high at 100 % text (the scaled size at
//! another), as wide as it holds: 6 in, its mark, 6 after it the text in `mono_s` on one line, and 6 after the text.
//! The text is at most 420 wide and is cut with one `…` where the room it is given is less.
//!
//! - Plain: no mark, the text `muted`.
//! - Ok: a `plus` `●`, the text `muted`.
//! - Waiting: a ring of eight 2 px `warn` dots in a 10 px box, the text `ink`.
//! - Offline: a `warn` `○`, the text `ink`.
//! - Error: a `minus` `●`, the text `minus`.
//! - Accent: no mark, the text `cyan_text`.
//!
//! An item the caller can press (it opens what the fact is about) is a button: the `grid` fill under the pointer with
//! `muted` text turned `ink`, a 1 px `cyan_edge` edge on its box with the keyboard focus, and pressed the secondary
//! button's press (a `stage` fill and an `ink` edge: the sheet draws no pressed state). It takes a Tab stop and Enter
//! and Space press it. An item that is only a fact takes neither the pointer nor the keyboard.
//!
//! The text is outside data: control characters and bidi controls are the caller's to clean, but whatever it is given
//! is laid out at most [`super::MOST`] characters long, drawn as plain words inside the item and never wraps.

use egui::{Color32, Pos2, Rect, Response, Role, Sense, Stroke, StrokeKind, Ui, WidgetInfo, pos2, vec2};

use crate::draw::paint::{run_at, run_box};
use crate::theme::Theme;

use super::{Hover, Shown, lit};

/// The room left and right of the item's parts, and between its mark and its text, in points.
pub const PAD: f32 = 6.0;

/// The widest its text is drawn, in points.
pub const MOST_TEXT: f32 = 420.0;

/// The width of a `●` or `○` mark at 100 % text: the sign's box in mono 11.
const SIGN: f32 = 11.0;

/// The side of the waiting ring's box.
const RING: f32 = 10.0;

/// The side of one dot of the waiting ring.
const DOT: f32 = 2.0;

/// The dots of the waiting ring in its box, clockwise from the top.
const DOTS: [(f32, f32); 8] = [(4.0, 0.0), (7.0, 1.0), (8.0, 4.0), (7.0, 7.0), (4.0, 8.0), (1.0, 7.0), (0.0, 4.0), (1.0, 1.0)];

/// The sheet's `●` and `○` drawn in code, as shares of the text size: Plex Mono holds no such sign, and the design
/// draws it from another face, a disc 0.75 of the size across, its middle 5/11 of the size right of the mark's left
/// and 1/88 of the size under the middle of its text box (as the progress line draws its own).
const DISC: (f32, f32, f32) = (0.75, 5.0 / 11.0, 1.0 / 88.0);

/// The six states of the sheet that say what a fact is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A plain fact: no mark, `muted`.
    Plain,
    /// All is well: a `plus` `●`.
    Ok,
    /// Something waits (the outbox): the `warn` ring, `ink`.
    Waiting,
    /// No connection: a `warn` `○`, `ink`.
    Offline,
    /// Something failed: a `minus` `●`, `minus`.
    Error,
    /// What comes next: `cyan_text`, no mark.
    Accent,
}

/// The mark before an item's text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mark {
    Disc(Color32),
    Circle(Color32),
    Ring(Color32),
}

impl Kind {
    /// Its mark and its text's colour in `theme`.
    fn look(self, theme: &Theme) -> (Option<Mark>, Color32) {
        let p = &theme.palette;
        match self {
            Self::Plain => (None, p.muted),
            Self::Ok => (Some(Mark::Disc(p.plus)), p.muted),
            Self::Waiting => (Some(Mark::Ring(p.warn)), p.ink),
            Self::Offline => (Some(Mark::Circle(p.warn)), p.ink),
            Self::Error => (Some(Mark::Disc(p.minus)), p.minus),
            Self::Accent => (None, p.cyan_text),
        }
    }
}

impl Mark {
    /// The width it takes before the gap to the text in `theme`: a sign as wide as the text's size makes it (11 at
    /// 100 %), the ring its fixed 10.
    fn width(self, theme: &Theme) -> f32 {
        match self {
            Self::Disc(_) | Self::Circle(_) => (SIGN * theme.fonts.mono_s.size / 11.0 - 0.01).ceil(),
            Self::Ring(_) => RING,
        }
    }
}

/// What a status item shows.
#[derive(Debug, Clone, Copy)]
pub struct StatusItem<'a> {
    /// The fact, one line.
    pub text: &'a str,
    /// What kind of fact it is.
    pub kind: Kind,
    /// Whether the caller takes a press of it: a button, or only a fact.
    pub pressable: bool,
    /// The state drawn whatever the pointer and the keyboard do (the gallery's boards).
    pub shown: Option<Shown>,
}

impl<'a> StatusItem<'a> {
    /// A fact that is only information, `text` of `kind`.
    #[must_use]
    pub const fn new(text: &'a str, kind: Kind) -> Self {
        Self { text, kind, pressable: false, shown: None }
    }

    /// The same fact as a button the caller takes the press of.
    #[must_use]
    pub const fn pressable(mut self) -> Self {
        self.pressable = true;
        self
    }

    /// The same fact drawn in `shown`.
    #[must_use]
    pub const fn shown(mut self, shown: Shown) -> Self {
        self.shown = Some(shown);
        self
    }
}

/// `text` from outside as one line in `theme`'s `mono_s` that fits `room`: at most [`super::MOST`] characters of it
/// looked at, drawn as plain words, cut between grapheme clusters with one `…`. However long `text` is, the work is that
/// of a line.
pub(crate) fn one_line(ui: &Ui, theme: &Theme, text: &str, room: f32) -> String {
    let font = theme.fonts.mono_s;
    let (kept, was_capped) = super::capped(text, super::MOST);
    let clean = super::plain(kept);
    let shown = super::fit_capped(ui, &clean, font, room);
    if was_capped && !shown.ends_with('…') {
        // The line held all it was given: the ellipsis says the rest was left out.
        return super::fit_capped(ui, &format!("{}…", clean.trim_end()), font, room);
    }
    shown
}

/// What a screen reader hears for `text` from outside: plain words, at most [`super::MOST`] characters of them.
pub(crate) fn heard(text: &str) -> String {
    super::spoken(super::capped(text, super::MOST).0)
}

/// The width an item of `item` takes when it may be `room` wide at most: hugging its mark and text, the text at most
/// [`MOST_TEXT`].
#[must_use]
pub fn width(ui: &Ui, theme: &Theme, item: &StatusItem<'_>, room: f32) -> f32 {
    layout(ui, theme, item, room).0
}

/// The item's width, its text as drawn and the text's colour.
fn layout(ui: &Ui, theme: &Theme, item: &StatusItem<'_>, room: f32) -> (f32, String, Option<Mark>, Color32) {
    let (mark, ink) = item.kind.look(theme);
    let before = PAD + mark.map_or(0.0, |mark| mark.width(theme) + PAD);
    // A room that is not a number counts as none.
    let text_room = if room.is_nan() { 0.0 } else { (room - before - PAD).clamp(0.0, MOST_TEXT) };
    let text = one_line(ui, theme, item.text, text_room);
    let wide = run_box(super::galley(ui, &text, theme.fonts.mono_s, ink).size().x).min(text_room);
    ((before + wide + PAD).floor(), text, mark, ink)
}

/// The status item `item` in `ui`, at most as wide as `ui` has room for; its response, clicked when it was pressed
/// (by the pointer, or by Enter or Space with the focus on it). An item that is not pressable is never clicked.
pub fn status_item(ui: &mut Ui, theme: &Theme, item: StatusItem<'_>) -> Response {
    let p = &theme.palette;
    let font = theme.fonts.mono_s;
    let high = theme.boxes().status;
    let room = ui.available_width().max(0.0);
    let (wide, text, mark, ink) = layout(ui, theme, &item, room);
    let sense = if item.pressable { Sense::click() } else { Sense::hover() };
    let (rect, response) = ui.allocate_exact_size(vec2(wide, high), sense);
    let role = if item.pressable { Role::Button } else { Role::Label };
    response.widget_info(|| WidgetInfo::labeled(role, ui.is_enabled(), heard(item.text)));
    let response = if item.pressable { response.on_hover_cursor(egui::CursorIcon::PointingHand) } else { response };
    let lit = lit(ui, theme, &response, item.pressable, item.shown, Hover::Fades);
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    // Under the pointer the `grid` fill comes over the `fast` token, and `muted` words turn `ink` with it.
    let (fill, edge, ink) = if lit.pressed {
        (Some(p.stage), Some(p.ink), if ink == p.muted { p.ink } else { ink })
    } else if lit.fade > 0.0 {
        let face = super::between(
            super::Face { fill: None, edge: None, ink },
            super::Face { fill: Some(p.grid), edge: None, ink: if ink == p.muted { p.ink } else { ink } },
            lit.fade,
            lit.hover,
        );
        (face.fill, None, face.ink)
    } else {
        (None, None, ink)
    };
    if let Some(fill) = fill {
        painter.rect_filled(rect, 0.0, fill);
    }
    let focus_edge = if lit.focus && !lit.pressed { Some(p.cyan_edge) } else { edge };
    if let Some(edge) = focus_edge {
        painter.rect_stroke(rect, 0.0, Stroke::new(1.0, edge), StrokeKind::Inside);
    }
    let top = (rect.center().y - font.line / 2.0).round();
    let mut left = rect.left() + PAD;
    if let Some(mark) = mark {
        paint_mark(&painter, theme, mark, pos2(left, top), rect.center().y);
        left += mark.width(theme) + PAD;
    }
    run_at(&painter, pos2(left, top), super::galley(ui, &text, font, ink), ink);
    response
}

/// Paints `mark` with its left at `at.x`, its text box's top at `at.y`; the ring centred on `middle`.
fn paint_mark(painter: &egui::Painter, theme: &Theme, mark: Mark, at: Pos2, middle: f32) {
    let font = theme.fonts.mono_s;
    let (across, right, down) = DISC;
    let centre = pos2(at.x + right * font.size, at.y + font.line / 2.0 + down * font.size);
    match mark {
        Mark::Disc(colour) => {
            painter.circle_filled(centre, across * font.size / 2.0, colour);
        }
        Mark::Circle(colour) => {
            painter.circle_stroke(centre, across * font.size / 2.0 - 0.5, Stroke::new(1.0, colour));
        }
        Mark::Ring(colour) => {
            let ring = pos2(at.x, (middle - RING / 2.0).round());
            for (dx, dy) in DOTS {
                painter.rect_filled(Rect::from_min_size(ring + vec2(dx, dy), vec2(DOT, DOT)), 0.0, colour);
            }
        }
    }
}

#[cfg(test)]
#[path = "status_item_tests.rs"]
mod tests;
