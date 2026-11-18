// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The invitation card over a message's text, on the `panel` fill in a 1 px edge: `cyan_edge` while it asks for an
//! answer (an invitation, a changed one, one that clashes), `minus` when the event is off, `line` otherwise. At the
//! left a column 92 wide holds the day: the weekday in `label` and `cyan_text`, the day in `display`, the month in
//! `label` and `muted`. A `line` hairline parts it from the rest, 14 above and below and 16 at the sides, 10 between:
//! the title in `heading` (struck through and `muted` when cancelled), the facts WHEN, WHERE, WHO and CAL (labels in a
//! 56 wide column, the values 12 right of it, 22 high, 6 apart), then by state: Going (primary, but on an older card of a thread), Maybe and No with to
//! whom the answer goes; what changed or what clashes in a `warn` box; the answer given and Change answer; that it is
//! cancelled in a `minus` box and Remove from calendar; the guests' answers; or why there is nothing to answer. A
//! version a later one replaced is one line instead: the older-version clock and what it says, on the `panel` fill in a
//! `panel_line` edge. Every word from the event is data: plain words on one line, cut between grapheme clusters with
//! one ellipsis; a screen reader hears them whole, each line a label of its own. The owner changing an answer also gets
//! Keep my answer after the three. A control focused inside the card shows its own ring. Nothing moves.

use egui::{Align, Color32, Layout, Rect, Response, Role, Sense, Stroke, StrokeKind, Ui, UiBuilder, WidgetInfo, pos2, vec2};

use crate::draw::paint;
use crate::iconset::{self, IconId};
use crate::keymap::Keymap;
use crate::theme::{Font, Theme};
use crate::tr;

use super::button::TextButton;
use super::button_row::{ButtonLines, lines_of, show_lines};

/// The event a card is about, as the card writes it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CardEvent<'a> {
    /// The weekday, short (`MON`).
    pub weekday: &'a str,
    /// The day of the month (`16`).
    pub day: &'a str,
    /// The month, short (`NOV`).
    pub month: &'a str,
    /// The event's title.
    pub title: &'a str,
    /// When (`15:00 – 16:00  ·  today`).
    pub when: &'a str,
    /// Where; empty when the event says nothing.
    pub place: &'a str,
    /// Who is invited.
    pub who: &'a str,
    /// The calendar it goes into.
    pub cal: &'a str,
}

/// A guest's answer, on a card of replies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Said {
    /// Going: a `plus` check.
    Going,
    /// Maybe: a `warn` question mark.
    Maybe,
    /// Not going: a `minus` cross.
    No,
    /// No answer yet: a `muted` dash.
    Nothing,
}

/// What a card shows under the facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardState<'a> {
    /// It asks for an answer; the answer goes to `goes_to`.
    Request { goes_to: &'a str },
    /// It asks again after a change, which `changed` says.
    Changed { changed: &'a str, goes_to: &'a str },
    /// A later version replaced it: only `line`.
    OutOfDate { line: &'a str },
    /// It was answered: `answer` says how.
    Answered { answer: &'a str },
    /// The event is off: `line` says who called it off; Remove from calendar while it is in the calendar.
    Cancelled { line: &'a str, remove: bool },
    /// The guests' answers.
    Replies { replies: &'a [(Said, &'a str)] },
    /// It asks for no answer.
    NoAnswerAsked,
    /// It cannot be answered, for `why`.
    CannotAnswer { why: &'a str },
    /// It asks for an answer and clashes with `clash`.
    Clash { clash: &'a str, goes_to: &'a str },
}

/// What was pressed on a card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardAsked {
    /// Going, Maybe or No: the index in that order.
    Answer(usize),
    /// Change answer.
    Change,
    /// Remove from calendar.
    Remove,
    /// Keep my answer: the owner changing an answer keeps the one given.
    Keep,
}

/// The width of the day's column, in points.
const DAY: f32 = 92.0;

/// The room above and below the card's content, in points.
const PAD_Y: f32 = 14.0;

/// The room left and right of the card's content, in points.
const PAD_X: f32 = 16.0;

/// The room between the parts of the card, in points.
const GAP: f32 = 10.0;

/// The width of the facts' labels' column, and the room after it, in points.
const LABEL: f32 = 56.0;
const LABEL_GAP: f32 = 12.0;

/// The room between two facts, in points.
const FACT_GAP: f32 = 6.0;

/// The room inside a warning box, above and below and at the sides, and between its label and its words, in points.
const BOX_Y: f32 = 6.0;
const BOX_X: f32 = 10.0;
const BOX_GAP: f32 = 8.0;

/// The room between two answers, in points.
const BUTTON_GAP: f32 = 8.0;

/// The room between two guests' answers, and between an answer's mark and its words, in points.
const REPLY_GAP: f32 = 2.0;
const MARK_GAP: f32 = 10.0;

/// The room inside the line of a replaced version, above and below and at the sides, in points.
const LINE_Y: f32 = 10.0;
const LINE_X: f32 = 12.0;

/// The glyphs of the guests' answers (Phosphor `check`, `question`, `x`, `minus`).
const CHECK: char = '\u{e182}';
const QUESTION: char = '\u{e3e8}';
const CROSS: char = '\u{e4f6}';
const DASH: char = '\u{e32a}';

/// What a screen reader hears for the card of `event`.
pub(crate) fn spoken(event: &CardEvent<'_>) -> String {
    tr!("Invitation: {title}, {when}", title = super::spoken(event.title), when = super::spoken(event.when))
}

/// The edge of a card in `state`.
fn edge(theme: &Theme, state: &CardState<'_>) -> Color32 {
    let p = &theme.palette;
    match state {
        CardState::Request { .. } | CardState::Changed { .. } | CardState::Clash { .. } => p.cyan_edge,
        CardState::Cancelled { .. } => p.minus,
        CardState::OutOfDate { .. } => p.panel_line,
        CardState::Answered { .. } | CardState::Replies { .. } | CardState::NoAnswerAsked | CardState::CannotAnswer { .. } => p.line,
    }
}

/// The card of `event` in `state` across `ui`'s width; what was pressed on it.
pub fn invite_card(ui: &mut Ui, theme: &Theme, keymap: &Keymap, event: &CardEvent<'_>, state: &CardState<'_>) -> Option<CardAsked> {
    invite_card_with(ui, theme, keymap, event, state, false, true)
}

/// [`invite_card`]; `keep` adds Keep my answer after the answers of a card that asks (the owner changing an answer), and
/// `primary` says whether its Going is the pane's one primary button (on an older card of the thread it is secondary).
pub fn invite_card_with(
    ui: &mut Ui,
    theme: &Theme,
    keymap: &Keymap,
    event: &CardEvent<'_>,
    state: &CardState<'_>,
    keep: bool,
    primary: bool,
) -> Option<CardAsked> {
    if let CardState::OutOfDate { line } = state {
        replaced(ui, theme, event, line);
        return None;
    }
    let p = &theme.palette;
    let f = &theme.fonts;
    let width = ui.available_width().max(DAY + PAD_X * 2.0 + LABEL + LABEL_GAP + 1.0).floor();
    let button = super::control_side(theme);
    let room = width - DAY - PAD_X * 2.0;
    let fact_line = f.body.line;
    let facts_high = fact_line * 4.0 + FACT_GAP * 3.0;
    let boxed = box_high(theme);
    let goes_high = f.caption.line;
    let why = if let CardState::CannotAnswer { why } = state {
        Some(super::wrapped(
            ui,
            &super::plain(&tr!("This invitation cannot be answered: {why}", why = why)),
            f.caption,
            p.muted,
            room,
            2,
            Align::LEFT,
        ))
    } else {
        None
    };
    // The buttons, measured once: on the info column's lines, or across the card under the day where the widest is
    // wider than the column (a narrow reader), so none is cut.
    let buttons = card_buttons(state, keep, primary);
    let widths: Vec<f32> = buttons.iter().map(|button| button.width(ui, theme, keymap)).collect();
    let widest = widths.iter().copied().fold(0.0, f32::max);
    let spread = widest > room;
    let lower_room = if spread { width - PAD_X * 2.0 } else { room };
    let placed = lines_of(theme, &widths, lower_room, BUTTON_GAP);
    let answered = if let CardState::Answered { answer } = state { Some(answered_row(ui, theme, keymap, answer, lower_room)) } else { None };
    let part = match state {
        CardState::Request { .. } => placed.height + GAP + goes_high,
        CardState::Changed { .. } | CardState::Clash { .. } => boxed + GAP + placed.height + GAP + goes_high,
        CardState::Answered { .. } => answered.as_ref().map_or(button, |row| if row.split { button * 2.0 + GAP } else { button }),
        CardState::Cancelled { remove, .. } => boxed + if *remove { GAP + placed.height } else { 0.0 },
        CardState::Replies { replies } => {
            let count = replies.len() as f32;
            count * fact_line + (count - 1.0).max(0.0) * REPLY_GAP
        }
        CardState::NoAnswerAsked => f.caption.line,
        CardState::CannotAnswer { .. } => why.as_ref().map_or(0.0, |why| why.size().y),
        CardState::OutOfDate { .. } => 0.0,
    };
    let high = (PAD_Y + f.heading.line + GAP + facts_high + GAP + part + PAD_Y).ceil();
    let (rect, response) = ui.allocate_exact_size(vec2(width, high), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Group, true, spoken(event)));
    let info_left = rect.left() + DAY + PAD_X;
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        painter.rect_filled(rect, 0.0, p.panel);
        // The day's hairline; it ends over the buttons where they span the card.
        let foot = if spread { rect.top() + PAD_Y + f.heading.line + GAP + facts_high + GAP / 2.0 } else { rect.bottom() } - 1.0;
        paint::hairline(painter, pos2(rect.left() + DAY, rect.top()), pos2(rect.left() + DAY, foot.round()), p.line);
        // The day: weekday, day and month centred in the column.
        let column = Rect::from_min_size(rect.min, vec2(DAY, high));
        let mut top = rect.top() + PAD_Y;
        for (words, font, ink) in [(event.weekday, f.label, p.cyan_text), (event.day, f.display, p.ink), (event.month, f.label, p.muted)] {
            let galley = super::fitted(ui, words, font, ink, DAY);
            let line = Rect::from_min_size(pos2(column.left(), top), vec2(DAY, font.line));
            paint::run_centred(painter, line, galley, ink);
            top += font.line;
        }
        let cancelled = matches!(state, CardState::Cancelled { .. });
        let mut top = rect.top() + PAD_Y;
        let title_ink = if cancelled { p.muted } else { p.ink };
        let title = super::fitted(ui, event.title, f.heading, title_ink, room);
        let title_width = title.size().x;
        paint::run_at(painter, pos2(info_left, top + (f.heading.line - title.size().y) / 2.0), title, title_ink);
        if cancelled {
            // The strike: one pixel row through the middle of the line, as long as the title.
            let middle = (top + f.heading.line / 2.0).round();
            painter.rect_filled(Rect::from_min_size(pos2(info_left, middle), vec2(title_width.ceil(), 1.0)), 0.0, p.muted);
        }
        top += f.heading.line + GAP;
        let value_left = info_left + LABEL + LABEL_GAP;
        let value_room = (rect.right() - PAD_X - value_left).max(1.0);
        let facts: [(&str, &str, Font, Color32); 4] = [
            (tr!("When"), event.when, f.mono, p.ink),
            (tr!("Where"), event.place, f.body, p.ink),
            (tr!("Who"), event.who, f.body, p.ink),
            (tr!("Cal"), event.cal, f.body, p.muted),
        ];
        for (label, value, font, ink) in facts {
            let label = super::galley(ui, label, f.label, p.muted);
            paint::run_at(painter, pos2(info_left, (top + (fact_line - label.size().y) / 2.0).round()), label, p.muted);
            let value = super::fitted(ui, value, font, ink, value_room);
            paint::run_at(painter, pos2(value_left, top), value, ink);
            top += fact_line + FACT_GAP;
        }
    }
    let part_top = rect.top() + PAD_Y + f.heading.line + GAP + facts_high + GAP;
    let part_left = if spread { rect.left() + PAD_X } else { info_left };
    let part_rect = Rect::from_min_max(pos2(part_left, part_top), pos2(rect.right() - PAD_X, rect.bottom() - PAD_Y));
    let lower_part = Lower { rect: part_rect, buttons, placed, answered, why, keep };
    let asked = lower(ui, theme, keymap, state, lower_part);
    // The edge last, over the fills and the hairline.
    if ui.is_rect_visible(rect) {
        ui.painter().rect_stroke(rect, 0.0, Stroke::new(1.0, edge(theme, state)), StrokeKind::Inside);
    }
    asked
}

/// The height of a warning box.
fn box_high(theme: &Theme) -> f32 {
    let f = &theme.fonts;
    (BOX_Y + f.mono_s.line.max(f.label.line) + BOX_Y).ceil()
}

/// The buttons of a card in `state`, in order: Going (primary while `primary`), Maybe, No and, while the owner changes
/// an answer, Keep my answer; Change answer; or Remove from calendar.
fn card_buttons(state: &CardState<'_>, keep: bool, primary: bool) -> Vec<TextButton<'static>> {
    match state {
        CardState::Request { .. } | CardState::Changed { .. } | CardState::Clash { .. } => {
            let going = TextButton::new(tr!("Going"));
            let going = if primary { going.primary() } else { going.secondary() };
            let mut buttons = vec![going, TextButton::new(tr!("Maybe")).secondary(), TextButton::new(tr!("No")).secondary()];
            if keep {
                buttons.push(TextButton::new(tr!("Keep my answer")).secondary());
            }
            buttons
        }
        CardState::Answered { .. } => vec![TextButton::new(tr!("Change answer")).secondary()],
        CardState::Cancelled { remove: true, .. } => vec![TextButton::new(tr!("Remove from calendar")).secondary()],
        _ => Vec::new(),
    }
}

/// How the answer given stands: its words cut to the room, and whether Change answer goes on a line of its own.
struct AnsweredRow {
    words: std::sync::Arc<egui::Galley>,
    split: bool,
}

/// The row of the answer given in `room`: the check, the words and Change answer, 12 apart, on one line where they fit.
fn answered_row(ui: &Ui, theme: &Theme, keymap: &Keymap, answer: &str, room: f32) -> AnsweredRow {
    let side = theme.mark_side();
    let words = super::fitted(ui, answer, theme.fonts.body, theme.palette.ink, (room - side - LABEL_GAP).max(1.0));
    let change = TextButton::new(tr!("Change answer")).secondary().width(ui, theme, keymap);
    let split = side + LABEL_GAP + paint::run_box(words.size().x) + LABEL_GAP + change > room;
    AnsweredRow { words, split }
}

/// The part of a card under its facts: where it stands, its buttons measured, the answer given and why it cannot be
/// answered, and whether Keep my answer stands.
struct Lower {
    rect: Rect,
    buttons: Vec<TextButton<'static>>,
    placed: ButtonLines,
    answered: Option<AnsweredRow>,
    why: Option<std::sync::Arc<egui::Galley>>,
    keep: bool,
}

/// The part of a card under its facts; what was pressed.
fn lower(ui: &mut Ui, theme: &Theme, keymap: &Keymap, state: &CardState<'_>, part: Lower) -> Option<CardAsked> {
    let p = &theme.palette;
    let f = &theme.fonts;
    let Lower { rect, buttons, placed, answered, why, keep } = part;
    let button = super::control_side(theme);
    let mut top = rect.top();
    let mut asked = None;
    let goes = |ui: &Ui, top: f32, to: &str| {
        let whole = tr!("Your answer goes to {address} at once.", address = to);
        let words = super::fitted(ui, &whole, f.caption, p.muted, rect.width());
        said(ui, Rect::from_min_size(pos2(rect.left(), top), vec2(paint::run_box(words.size().x), f.caption.line)), &whole, ("goes", 0));
        paint::run_at(ui.painter(), pos2(rect.left(), top), words, p.muted);
    };
    let row = |ui: &mut Ui, top: f32| {
        ui.new_child(
            UiBuilder::new()
                .max_rect(Rect::from_min_size(pos2(rect.left(), top), vec2(rect.width(), button)))
                .layout(Layout::left_to_right(Align::Center)),
        )
    };
    let lines_at = |top: f32| Rect::from_min_size(pos2(rect.left(), top), vec2(rect.width(), placed.height));
    match state {
        CardState::Request { goes_to } | CardState::Changed { goes_to, .. } | CardState::Clash { goes_to, .. } => {
            if let CardState::Changed { changed: words, .. } | CardState::Clash { clash: words, .. } = state {
                let label = if matches!(state, CardState::Changed { .. }) { tr!("Changed") } else { tr!("Clash") };
                top += warning(ui, theme, rect, top, label, words, p.warn) + GAP;
            }
            asked = match show_lines(ui, theme, keymap, buttons, &placed, lines_at(top), BUTTON_GAP) {
                Some(3) if keep => Some(CardAsked::Keep),
                Some(index) => Some(CardAsked::Answer(index)),
                None => None,
            };
            goes(ui, top + placed.height + GAP, goes_to);
        }
        CardState::Answered { answer } => {
            let mut child = row(ui, top);
            child.spacing_mut().item_spacing.x = LABEL_GAP;
            let side = theme.mark_side();
            let (mark, _) = child.allocate_exact_size(vec2(side, side), Sense::hover());
            iconset::paint_glyph(child.painter(), mark, CHECK, p.plus);
            if let Some(row) = &answered {
                let words = row.words.clone();
                let (spot, said) = child.allocate_exact_size(vec2(paint::run_box(words.size().x), f.body.line), Sense::hover());
                said.widget_info(|| WidgetInfo::labeled(Role::Label, true, super::spoken(answer)));
                paint::run_at(child.painter(), spot.min, words, p.ink);
            }
            let split = answered.as_ref().is_some_and(|row| row.split);
            let mut place = if split { row(ui, top + button + GAP) } else { child };
            if TextButton::new(tr!("Change answer")).secondary().show(&mut place, theme, keymap).clicked() {
                asked = Some(CardAsked::Change);
            }
        }
        CardState::Cancelled { line, remove } => {
            top += warning(ui, theme, rect, top, tr!("Cancelled"), line, p.minus);
            if *remove && show_lines(ui, theme, keymap, buttons, &placed, lines_at(top + GAP), BUTTON_GAP).is_some() {
                asked = Some(CardAsked::Remove);
            }
        }
        CardState::Replies { replies } => {
            let side = theme.mark_side();
            for (index, (answer, words)) in replies.iter().enumerate() {
                let (glyph, ink) = match answer {
                    Said::Going => (CHECK, p.plus),
                    Said::Maybe => (QUESTION, p.warn),
                    Said::No => (CROSS, p.minus),
                    Said::Nothing => (DASH, p.muted),
                };
                let line = f.body.line;
                said(ui, Rect::from_min_size(pos2(rect.left(), top), vec2(rect.width(), line)), words, ("reply", index));
                if ui.is_rect_visible(rect) {
                    let mark = Rect::from_min_size(pos2(rect.left(), (top + (line - side) / 2.0).round()), vec2(side, side));
                    iconset::paint_glyph(ui.painter(), mark, glyph, ink);
                    let words = super::fitted(ui, words, f.body, p.ink, rect.width() - side - MARK_GAP);
                    paint::run_at(ui.painter(), pos2(rect.left() + side + MARK_GAP, top), words, p.ink);
                }
                top += line + REPLY_GAP;
            }
        }
        CardState::NoAnswerAsked => {
            let whole = tr!("This calendar message asks for no answer.");
            let words = super::galley(ui, whole, f.caption, p.muted);
            said(
                ui,
                Rect::from_min_size(pos2(rect.left(), top), vec2(paint::run_box(words.size().x).min(rect.width()), f.caption.line)),
                whole,
                ("none", 0),
            );
            paint::run_at(ui.painter(), pos2(rect.left(), top), words, p.muted);
        }
        CardState::CannotAnswer { why: words } => {
            if let Some(why) = why {
                let whole = tr!("This invitation cannot be answered: {why}", why = words);
                said(ui, Rect::from_min_size(pos2(rect.left(), top), vec2(rect.width(), why.size().y)), &whole, ("why", 0));
                paint::rows_at(ui.painter(), pos2(rect.left(), top), rect.width(), why, p.muted);
            }
        }
        CardState::OutOfDate { .. } => {}
    }
    asked
}

/// Tells a screen reader that `words` stand in `rect`, whole (the text drawn may be cut): a label of their own.
fn said(ui: &Ui, rect: Rect, words: &str, salt: (&str, usize)) {
    let response = ui.interact(rect, ui.next_auto_id().with(("invite-said", salt)), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, super::spoken(words)));
}

/// A box of `colour` at `top` of `rect`: `label` in `label` and the colour, `words` in `mono_s` and `ink`; its height.
fn warning(ui: &Ui, theme: &Theme, rect: Rect, top: f32, label: &str, words: &str, colour: Color32) -> f32 {
    let p = &theme.palette;
    let f = &theme.fonts;
    let high = box_high(theme);
    let at = Rect::from_min_size(pos2(rect.left(), top), vec2(rect.width(), high));
    said(ui, at, &tr!("{label}: {words}", label = label, words = words), ("warning", 0));
    if ui.is_rect_visible(at) {
        let painter = ui.painter();
        painter.rect_stroke(at, 0.0, Stroke::new(1.0, colour), StrokeKind::Inside);
        let label = super::galley(ui, label, f.label, colour);
        let label_width = paint::run_box(label.size().x);
        let middle = at.center().y;
        paint::run_at(painter, pos2(at.left() + BOX_X, (middle - label.size().y / 2.0).round()), label, colour);
        let left = at.left() + BOX_X + label_width + BOX_GAP;
        let words = super::fitted(ui, words, f.mono_s, p.ink, (at.right() - BOX_X - left).max(1.0));
        paint::run_at(painter, pos2(left, (middle - words.size().y / 2.0).round()), words, p.ink);
    }
    high
}

/// The line of a version a later one replaced: the older-version clock and `line` in `caption` and `muted`.
fn replaced(ui: &mut Ui, theme: &Theme, event: &CardEvent<'_>, line: &str) -> Response {
    let p = &theme.palette;
    let f = &theme.fonts;
    let width = ui.available_width().max(1.0).floor();
    let high = (LINE_Y + f.caption.line + LINE_Y).ceil();
    let (rect, response) = ui.allocate_exact_size(vec2(width, high), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Group, true, tr!("{card}: {line}", card = spoken(event), line = super::spoken(line))));
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        painter.rect(rect, 0.0, p.panel, Stroke::new(1.0, p.panel_line), StrokeKind::Inside);
        let side = theme.icon_side();
        let middle = rect.center().y;
        let icon = Rect::from_min_size(pos2(rect.left() + LINE_X, (middle - side / 2.0).round()), vec2(side, side));
        iconset::paint(painter, icon, IconId::Older, p.muted);
        let left = icon.right() + GAP;
        let words = super::fitted(ui, line, f.caption, p.muted, (rect.right() - LINE_X - left).max(1.0));
        paint::run_at(painter, pos2(left, (middle - words.size().y / 2.0).round()), words, p.muted);
    }
    response
}

#[cfg(test)]
#[path = "invite_card_tests.rs"]
mod tests;
