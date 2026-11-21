// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A status message: a stretch of the status line that says what just happened, as its sheet draws it. No box: the
//! `panel` fill under a 2 px line along its top in its kind's colour, 26 high at 100 % text (the scaled size at
//! another), as wide as the caller gives it. 18 in, its mark (a 14 px icon, or the error's `●`); 10 after it the text in
//! `mono_s` on one line, cut with one `…`; then its parts, 10 apart, and 12 before its right end.
//!
//! - Undo: the `check` icon in `cyan_text`, the text `ink`, the small link "undo" and the chip of its key, the
//!   countdown (an 80 × 2 `stage` bar whose `cyan_edge` part is the share of the time left) and the seconds left in
//!   `muted`. The line is `cyan_edge`.
//! - Send countdown: the same with the `arrow-up` icon, while a send can still be taken back.
//! - Notice: the `info` icon in `muted`, the text `ink`, the line `muted`.
//! - Error: the `minus` `●` and the text in `minus`, then the small links "open log" (no log to open yet: drawn coming
//!   soon) and "dismiss", 16 apart. The line is `minus`.
//! - Done: the `check` icon in `plus`, the text `ink`, the line `plus`.
//!
//! The message keeps no clock: the caller gives the seconds and the share left, and leaves the bar out where it shows
//! none. Undo and dismiss are its only presses ([`Press`]), each a Tab stop pressed by Enter and Space; the rest is
//! information. Its text is outside data: whatever it is given is laid out at most [`super::MOST`] characters long,
//! drawn as plain words inside the message, never past its right end.

use egui::{Align, Color32, Layout, Rect, Response, Role, Sense, Ui, UiBuilder, WidgetInfo, pos2, vec2};

use crate::draw::paint::{run_at, run_box};
use crate::iconset::{self, IconId};
use crate::keymap::Keymap;
use crate::theme::Theme;
use crate::tr;

use super::button::{LinkSize, link_sized};
use super::key::{Chip, ChipState};
use super::soon::soon;
use super::status_item::{heard, one_line};

/// The room before the mark, in points.
const LEFT: f32 = 18.0;

/// The room after the last part, in points.
const RIGHT: f32 = 12.0;

/// The gap between the message's parts, in points.
const GAP: f32 = 10.0;

/// The gap between the link and its key chip, in points.
const KEY_GAP: f32 = 6.0;

/// The gap between the error's two links, in points.
const LINK_GAP: f32 = 16.0;

/// The countdown bar's width, in points.
pub const BAR: f32 = 80.0;

/// The countdown bar's height and the top line's, in points: a hairline's weight, the same at every text scale.
const LINE: f32 = 2.0;

/// The width of the error's `●` at 100 % text: the sign's box in mono 11.
const SIGN: f32 = 11.0;

/// Phosphor Regular's `arrow-up`, the send countdown's icon: not one of the [`IconId`]s.
const ARROW_UP: char = '\u{e08e}';

/// The sheet's `●` drawn in code, as the status item draws it.
const DISC: (f32, f32, f32) = (0.75, 5.0 / 11.0, 1.0 / 88.0);

/// The time a message can still be taken back: what its countdown shows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Left {
    /// The whole seconds left, as the caller counts them.
    pub seconds: u32,
    /// The share of the time left, 0 to 1: how much of the bar is the accent. A share that is not a number is none.
    pub share: f32,
    /// Whether the bar shows: not at motion off, where the line keeps only its seconds (M5).
    pub bar: bool,
}

/// The kinds of the sheet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind<'a> {
    /// Something done that can be taken back: undo, its key and the time left (`None`: no bar, no seconds).
    Undo { key: Option<&'a str>, left: Option<Left> },
    /// A send that has not gone yet: undo, its key and the time left.
    Countdown { key: Option<&'a str>, left: Option<Left> },
    /// A notice: it says something and goes.
    Notice,
    /// A failure: it stays until dismissed.
    Error,
    /// Something done that cannot be taken back.
    Done,
}

/// What a status message shows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StatusMessage<'a> {
    /// Its kind.
    pub kind: Kind<'a>,
    /// What it says, one line.
    pub text: &'a str,
}

/// What was pressed in a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// Undo (an undo or a send countdown).
    Undo,
    /// Dismiss (an error).
    Dismiss,
}

/// The mark before the text.
enum Mark {
    Icon(char, Color32),
    Disc(Color32),
}

impl Kind<'_> {
    /// Its top line's colour, its mark and its text's colour in `theme`.
    fn look(self, theme: &Theme) -> (Color32, Mark, Color32) {
        let p = &theme.palette;
        match self {
            Self::Undo { .. } => (p.cyan_edge, Mark::Icon(iconset::glyph(IconId::Check), p.cyan_text), p.ink),
            Self::Countdown { .. } => (p.cyan_edge, Mark::Icon(ARROW_UP, p.cyan_text), p.ink),
            Self::Notice => (p.muted, Mark::Icon(iconset::glyph(IconId::Info), p.muted), p.ink),
            Self::Error => (p.minus, Mark::Disc(p.minus), p.minus),
            Self::Done => (p.plus, Mark::Icon(iconset::glyph(IconId::Check), p.plus), p.ink),
        }
    }
}

/// The seconds left as the message writes them.
fn seconds(left: Left) -> String {
    tr!("{seconds} s", seconds = left.seconds)
}

/// The width of a small link of `words`: its text box.
fn link_width(ui: &Ui, theme: &Theme, words: &str) -> f32 {
    super::galley(ui, words, theme.fonts.mono_s, theme.palette.cyan_text).size().x.ceil()
}

/// Draws a small link of `words` in `ui` at `place`, coming soon or not; its response.
fn link_at(ui: &mut Ui, theme: &Theme, keymap: &Keymap, place: Rect, clip: Rect, words: &str, coming: bool) -> Response {
    let mut child =
        ui.new_child(UiBuilder::new().id_salt(("status-message-link", words)).max_rect(place).layout(Layout::left_to_right(Align::Center)));
    child.set_clip_rect(clip);
    if coming {
        soon(&mut child, theme, |ui| link_sized(ui, theme, keymap, words, LinkSize::Small))
    } else {
        link_sized(&mut child, theme, keymap, words, LinkSize::Small)
    }
}

/// The status message `message` across `ui`'s width; what was pressed in it this frame.
pub fn status_message(ui: &mut Ui, theme: &Theme, keymap: &Keymap, message: &StatusMessage<'_>) -> Option<Press> {
    let p = &theme.palette;
    let font = theme.fonts.mono_s;
    let width = ui.available_width().max(1.0).floor();
    let high = theme.boxes().status;
    let (rect, response) = ui.allocate_exact_size(vec2(width, high), Sense::hover());
    // A send counting down is heard as what is sent and that it can be undone, once: its seconds left stay out of the
    // label, which a live region would say again each second.
    let said = match message.kind {
        Kind::Countdown { .. } => tr!("{text}, can be undone", text = heard(message.text)),
        _ => heard(message.text),
    };
    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, &said));
    // A live region: a screen reader says the message when it comes, an error at once, the rest when it is free.
    let live = if message.kind == Kind::Error { egui::accesskit::Live::Assertive } else { egui::accesskit::Live::Polite };
    ui.ctx().accesskit_node_builder(response.id, |node| node.set_live(live));
    let clip = rect.intersect(ui.clip_rect());
    let (edge, mark, ink) = message.kind.look(theme);
    let middle = rect.center().y;
    let top = (middle - font.line / 2.0).round();
    // The parts from the right end: each its left edge and width.
    let mut right = rect.right() - RIGHT;
    let mut place = |wide: f32| {
        let left = right - wide;
        right = left - GAP;
        left
    };
    let mut press = None;
    let mut parts: Vec<(f32, String, Color32)> = Vec::new();
    let mut bar = None;
    let mut undo = None;
    let mut error_links = None;
    match message.kind {
        Kind::Undo { key, left } | Kind::Countdown { key, left } => {
            if let Some(left) = left {
                let words = seconds(left);
                let wide = run_box(super::galley(ui, &words, font, p.muted).size().x);
                parts.push((place(wide), words, p.muted));
                if left.bar {
                    bar = Some((place(BAR), left.share));
                }
            }
            let link = link_width(ui, theme, tr!("undo"));
            let chip = key.map(|key| Chip::new(ui, theme, key, ChipState::Default));
            let chip_wide = chip.as_ref().map_or(0.0, |chip| KEY_GAP + chip.size().x);
            let at = place(link + chip_wide);
            undo = Some((at, link, chip, key));
        }
        Kind::Error => {
            let (log, dismiss) = (link_width(ui, theme, tr!("open log")), link_width(ui, theme, tr!("dismiss")));
            let at = place(log + LINK_GAP + dismiss);
            error_links = Some((at, log, dismiss));
        }
        Kind::Notice | Kind::Done => {}
    }
    let mark_wide = match mark {
        Mark::Icon(..) => theme.mark_side(),
        Mark::Disc(_) => (SIGN * font.size / 11.0 - 0.01).ceil(),
    };
    let text_left = rect.left() + LEFT + mark_wide + GAP;
    let room = (right - text_left).max(0.0);
    let text = one_line(ui, theme, message.text, room);
    if ui.is_rect_visible(rect) {
        let painter = ui.painter().with_clip_rect(clip);
        painter.rect_filled(rect, 0.0, p.panel);
        painter.rect_filled(Rect::from_min_size(rect.min, vec2(rect.width(), LINE)), 0.0, edge);
        match mark {
            Mark::Icon(glyph, colour) => {
                let side = theme.mark_side();
                let at = Rect::from_min_size(pos2(rect.left() + LEFT, (middle - side / 2.0).round()), vec2(side, side));
                iconset::paint_glyph(&painter, at, glyph, colour);
            }
            Mark::Disc(colour) => {
                let (across, right, down) = DISC;
                let centre = pos2(rect.left() + LEFT + right * font.size, top + font.line / 2.0 + down * font.size);
                painter.circle_filled(centre, across * font.size / 2.0, colour);
            }
        }
        run_at(&painter, pos2(text_left, top), super::galley(ui, &text, font, ink), ink);
        for (left, words, colour) in parts {
            run_at(&painter, pos2(left, top), super::galley(ui, &words, font, colour), colour);
        }
        if let Some((left, share)) = bar {
            let track = Rect::from_min_size(pos2(left, (middle - LINE / 2.0).round()), vec2(BAR, LINE));
            painter.rect_filled(track, 0.0, p.stage);
            let share = if share.is_nan() { 0.0 } else { share.clamp(0.0, 1.0) };
            let lit = (BAR * share).round();
            if lit > 0.0 {
                painter.rect_filled(Rect::from_min_size(track.min, vec2(lit, LINE)), 0.0, p.cyan_edge);
            }
        }
    }
    let line = |left: f32, wide: f32| {
        Rect::from_min_size(pos2(left, (middle - theme.boxes().row_link / 2.0).round()), vec2(wide.max(1.0), theme.boxes().row_link))
    };
    if let Some((left, link, chip, key)) = undo {
        let response = link_at(ui, theme, keymap, line(left, link), clip, tr!("undo"), false);
        // Its key, while it is offered, is the link's for a screen reader too.
        if let Some(key) = key {
            ui.ctx().accesskit_node_builder(response.id, |node| node.set_keyboard_shortcut(key));
        }
        if response.clicked() {
            press = Some(Press::Undo);
        }
        if let Some(chip) = chip {
            let size = chip.size();
            let at = pos2(left + link + KEY_GAP, (middle - size.y / 2.0).round());
            chip.paint(&ui.painter().with_clip_rect(clip), at);
        }
    }
    if let Some((left, log, dismiss)) = error_links {
        link_at(ui, theme, keymap, line(left, log), clip, tr!("open log"), true);
        if link_at(ui, theme, keymap, line(left + log + LINK_GAP, dismiss), clip, tr!("dismiss"), false).clicked() {
            press = Some(Press::Dismiss);
        }
    }
    press
}

#[cfg(test)]
#[path = "status_message_tests.rs"]
mod tests;
