// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The command line's field, as its sheet draws it: a box 28 high on `term` with a 1 px edge inside (`line` at rest,
//! `muted` hovered, `cyan_edge` with the focus), 12 in from its edge on each side. In it, left to right, 8 apart:
//!
//! - the prompt `>` in `mono`, `cyan_text`;
//! - the scope's mark, when the caller narrowed it: a 16 high `cyan` block with the scope in `key`, `on_cyan` (`>`
//!   commands, `@` people, `#` mailboxes, or the name of the mailbox searched, 4 in on each side);
//! - the text in `mono`, `ink`, with the caret, a 7 × 16 `cyan_edge` block 2 after it while the field has the focus;
//!   empty, the hint in `muted` (after the caret and 8 more with the focus);
//! - at the right end the scope's name in `label`, `muted`, and 8 after it the key chip: the key that opens the field
//!   at rest, `Esc` once it has the focus, a text or a scope.
//!
//! The field takes its text and its scope from the caller and returns what the keyboard asked: a change, Enter, Esc,
//! Backspace in an empty field. It runs nothing, opens nothing and keeps nothing but what egui keeps for a text edit.
//! It holds at most [`MOST`] characters: a longer text the caller gives is cut to them at once, a paste past them is
//! cut where they end. Without the focus a text longer than its room is cut with one `…`; with it, it scrolls to the
//! caret. The caller cleans control and bidi characters from what it gives; the field lays out at most [`MOST`].

use egui::{Color32, Id, Key, Rect, Response, Sense, Stroke, StrokeKind, TextBuffer, TextEdit, Ui, pos2, vec2};

use super::Shown;
use super::key::{Chip, ChipState};
use crate::draw::paint;
use crate::theme::{Font, Theme};
use crate::tr;

/// The most characters the field holds.
pub const MOST: usize = 1000;

/// The box's height at 100 %, in points.
const HIGH: f32 = 28.0;

/// The scope mark's height at 100 %, in points.
const MARK: f32 = 16.0;

/// The room inside the box's edge to its text, left and right: the sheet's 12 and the 1 px edge.
const INSET: f32 = 13.0;

/// The gap between the parts, in points.
const GAP: f32 = 8.0;

/// The room left and right of a mailbox's name in its mark, in points.
const MARK_PAD: f32 = 4.0;

/// The caret's width at 100 %, in points.
const CARET: f32 = 7.0;

/// The gap between the text and the caret after it, in points.
const CARET_GAP: f32 = 2.0;

/// What the field is narrowed to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope<'a> {
    /// Nothing yet: no mark, no name.
    Open,
    /// A search of all mail: no mark, "all mail".
    AllMail,
    /// Commands: the mark `>`, "commands".
    Commands,
    /// People: the mark `@`, "people".
    People,
    /// Mailboxes: the mark `#`, "mailboxes".
    Mailboxes,
    /// A search of one mailbox: its name as the mark, "mail". The name is the caller's, cleaned.
    Mailbox(&'a str),
}

impl Scope<'_> {
    /// The mark's text, if it has a mark.
    fn mark(self) -> Option<&'static str> {
        match self {
            Self::Commands => Some(">"),
            Self::People => Some("@"),
            Self::Mailboxes => Some("#"),
            Self::Open | Self::AllMail | Self::Mailbox(_) => None,
        }
    }

    /// The scope's name at the right end, if it has one.
    fn name(self) -> Option<&'static str> {
        match self {
            Self::Open => None,
            Self::AllMail => Some(tr!("all mail")),
            Self::Commands => Some(tr!("commands")),
            Self::People => Some(tr!("people")),
            Self::Mailboxes => Some(tr!("mailboxes")),
            Self::Mailbox(_) => Some(tr!("mail")),
        }
    }

    /// The hint in an empty field (the sheet draws none in an empty field of commands, people or mailboxes).
    fn hint(self) -> Option<&'static str> {
        match self {
            Self::Open | Self::AllMail => Some(tr!("search mail or run a command")),
            Self::Mailbox(_) => Some(tr!("search this mailbox")),
            Self::Commands | Self::People | Self::Mailboxes => None,
        }
    }
}

/// What the field was asked this frame.
#[derive(Debug, Clone)]
pub struct Typed {
    /// The text edit's response: its focus and its change.
    pub response: Response,
    /// The box, in points.
    pub rect: Rect,
    /// Enter was pressed with the focus on the field.
    pub enter: bool,
    /// Esc was pressed with the focus on the field (egui takes the focus away; the caller decides where it goes).
    pub esc: bool,
    /// Backspace was pressed with the focus on the field and nothing in it: the caller may drop the scope.
    pub back: bool,
}

/// What a screen reader hears of the field.
#[must_use]
pub fn spoken() -> &'static str {
    tr!("Command line")
}

/// The box's height at `theme`'s text size: 28 at 100 %, 36 at 130 %.
#[must_use]
pub fn height(theme: &Theme) -> f32 {
    Theme::size(theme, HIGH)
}

/// The field `id` over the width `ui` has, holding `text` within `scope`; `open_key` the words of the key that opens
/// it ([`super::key::key_words`]); `forced` draws it hovered or focused whatever the pointer and the keyboard do (a
/// board: its caret stands still).
pub fn command_field(ui: &mut Ui, theme: &Theme, id: Id, text: &mut String, scope: Scope<'_>, open_key: &str, forced: Option<Shown>) -> Typed {
    let p = &theme.palette;
    let mono = theme.fonts.mono;
    // egui cuts what is typed, pasted, composed or set by a screen reader at the limit (`char_limit`); a longer text
    // from the caller is cut here, between graphemes.
    if text.len() > MOST {
        let kept = super::capped(text, MOST).0.len();
        text.truncate(kept);
    }
    let width = if ui.available_width().is_finite() { ui.available_width().max(HIGH) } else { 560.0 };
    let high = height(theme);
    let (rect, area) = ui.allocate_exact_size(vec2(width, high), Sense::hover());
    let had_focus = ui.memory(|memory| memory.has_focus(id));
    let (enter, esc, backspace) =
        ui.input(|input| (input.key_pressed(Key::Enter), input.key_pressed(Key::Escape), input.key_pressed(Key::Backspace)));
    let back = had_focus && backspace && text.is_empty();
    let live_focus = had_focus && forced.is_none();
    let focus = live_focus || forced == Some(Shown::Focus);
    let hover = forced == Some(Shown::Hover) || (forced.is_none() && area.hovered());
    // egui takes the focus from the field before the frame in which Esc is pressed: the frame before says it had it.
    let focus_kept = id.with("had-focus");
    let had_focus_before = ui.data(|data| data.get_temp::<bool>(focus_kept)).unwrap_or(false);
    let clicked_box = forced.is_none() && area.hovered() && !had_focus && ui.input(|input| input.pointer.primary_clicked());
    let painter = super::solid(ui);
    let edge = if focus {
        p.cyan_edge
    } else if hover {
        p.muted
    } else {
        p.line
    };
    painter.rect(rect, 0.0, p.term, Stroke::new(1.0, edge), StrokeKind::Inside);
    let inner_clip = rect.shrink(1.0).intersect(painter.clip_rect());
    let painter = painter.with_clip_rect(inner_clip);
    let text_top = (rect.center().y - mono.line / 2.0).round();
    // The prompt.
    let prompt = super::galley(ui, ">", mono, p.cyan_text);
    let mut left = rect.left() + INSET;
    let prompt_wide = paint::run_box(prompt.size().x);
    painter.galley(pos2(left, text_top), prompt, p.cyan_text);
    left += prompt_wide + GAP;
    // The chip and the scope's name, from the right.
    let narrowed = scope != Scope::Open;
    let chip_key = if focus || !text.is_empty() || narrowed { "Esc" } else { open_key };
    let chip = Chip::new(ui, theme, chip_key, ChipState::Default);
    let chip_at = pos2(rect.right() - INSET - chip.size().x, (rect.center().y - chip.size().y / 2.0).round());
    let mut right = chip_at.x - GAP;
    chip.paint(&painter, chip_at);
    if let Some(name) = scope.name() {
        let label = theme.fonts.label;
        let galley = super::galley(ui, name, label, p.muted);
        let wide = paint::run_box(galley.size().x);
        let top = (rect.center().y - label.line / 2.0).round();
        painter.galley(pos2(right - wide, top), galley, p.muted);
        right -= wide + GAP;
    }
    // The scope's mark.
    let mark_high = Theme::size(theme, MARK);
    let key = theme.fonts.key;
    let mark = match scope {
        Scope::Mailbox(name) => {
            // A mailbox's name is the server's: at most a third of the field, cut with one `…`.
            let most = (width / 3.0 - MARK_PAD * 2.0).max(0.0).floor();
            let fitted = super::fit_capped(ui, &super::plain(super::capped(name, super::MOST).0), key, most);
            Some((super::galley(ui, &fitted, key, p.on_cyan), true))
        }
        other => other.mark().map(|mark| (super::galley(ui, mark, key, p.on_cyan), false)),
    };
    if let Some((galley, padded)) = mark {
        let wide = if padded { paint::run_box(galley.size().x) + MARK_PAD * 2.0 } else { mark_high };
        let block = Rect::from_min_size(pos2(left, (rect.center().y - mark_high / 2.0).round()), vec2(wide, mark_high));
        painter.rect_filled(block, 0.0, p.cyan);
        paint::run_centred(&painter, block, galley, p.on_cyan);
        left = block.right() + GAP;
    }
    // The text, its caret and its hint.
    let caret = vec2((CARET * theme.text_scale.factor()).round(), theme.boxes().caret);
    let room = (right - caret.x - GAP - left).max(1.0);
    let place = Rect::from_min_size(pos2(left, text_top), vec2(room, mono.line));
    let whole_wide = paint::run_box(layout(ui, text.as_str(), mono, p.ink).size().x);
    let cut = !live_focus && !text.is_empty() && whole_wide > room;
    let laid = if cut { Color32::TRANSPARENT } else { p.ink };
    let mut layouter = |ui: &Ui, buffer: &dyn TextBuffer, _: f32| layout(ui, buffer.as_str(), mono, laid);
    let output = ui
        .scope(|ui| {
            let cursor = &mut ui.visuals_mut().text_cursor;
            cursor.stroke = Stroke::NONE;
            cursor.blink = false;
            cursor.preview = false;
            let edit = TextEdit::singleline(text)
                .id(id)
                .frame(egui::Frame::NONE)
                .margin(egui::Margin::ZERO)
                .desired_width(room)
                .min_size(vec2(room, mono.line))
                .char_limit(MOST)
                .return_key(None)
                .layouter(&mut layouter);
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(place));
            edit.show(&mut child)
        })
        .inner;
    let response = output.response.response.clone();
    ui.ctx().accesskit_node_builder(response.id, |node| node.set_label(spoken()));
    let text_clip = Rect::from_min_max(pos2(left, rect.top()), pos2(right, rect.bottom())).intersect(inner_clip);
    let caret_left = if text.is_empty() {
        left
    } else if cut {
        let fitted = super::fit_capped(ui, text.as_str(), mono, room);
        super::solid(ui).with_clip_rect(text_clip).galley(place.min, super::galley(ui, &fitted, mono, p.ink), p.ink);
        right - caret.x
    } else {
        let cursor = output.cursor_range.map_or_else(|| output.galley.end(), |range| range.primary);
        let at = output.galley.pos_from_cursor(cursor);
        let column = output.galley.layout_from_cursor(cursor).column;
        let row_end = output.galley.rows.get(output.galley.layout_from_cursor(cursor).row).map(|row| row.char_count_excluding_newline());
        let at_end = !live_focus || row_end.is_some_and(|end| column >= end);
        let x = if at_end && !live_focus {
            paint::run_box(output.galley.size().x)
        } else if at_end {
            paint::run_box(at.left())
        } else {
            at.left()
        };
        output.galley_pos.x + x + CARET_GAP
    };
    if text.is_empty()
        && let Some(hint) = scope.hint()
    {
        let shift = if focus { caret.x + GAP } else { 0.0 };
        let galley = super::galley(ui, hint, mono, p.muted);
        super::solid(ui).with_clip_rect(text_clip).galley(pos2(left + shift, text_top), galley, p.muted);
    }
    if focus {
        let block = Rect::from_min_size(pos2(caret_left.round(), (text_top + (mono.line - caret.y) / 2.0).round()), caret);
        let since = caret_since(ui, id, output.cursor_range.map_or(0, |range| range.primary.index.0));
        if crate::motion::blink(ui.ctx(), since, !live_focus) {
            super::solid(ui).with_clip_rect(text_clip).rect_filled(block, 0.0, p.cyan_edge);
        }
    }
    if clicked_box {
        // After the edit, which gives the focus up to a press outside its own place.
        ui.memory_mut(|memory| memory.request_focus(id));
    }
    let now_focus = ui.memory(|memory| memory.has_focus(id));
    ui.data_mut(|data| data.insert_temp(focus_kept, now_focus));
    let enter = had_focus && enter;
    let esc = (had_focus || had_focus_before) && esc;
    Typed { response, rect, enter, esc, back }
}

/// When the caret of the field `id`, now at `index`, last moved: its blink starts again whenever it moves.
fn caret_since(ui: &Ui, id: Id, index: usize) -> f64 {
    let now = ui.input(|input| input.time);
    let key = id.with("caret-since");
    let pass = ui.ctx().cumulative_pass_nr();
    ui.data_mut(|data| {
        let since = match data.get_temp::<(f64, usize, u64)>(key) {
            Some((since, last, seen)) if last == index && seen.saturating_add(1) >= pass => since,
            _ => now,
        };
        data.insert_temp(key, (since, index, pass));
        since
    })
}

/// `text` on one line in `font` and `colour`, its trailing spaces kept: the field's text as egui edits it.
fn layout(ui: &Ui, text: &str, font: Font, colour: Color32) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::single_section(text.to_owned(), font.format(colour));
    job.keep_trailing_whitespace = true;
    job.wrap.max_width = f32::INFINITY;
    job.break_on_newline = false;
    ui.fonts_mut(|fonts| fonts.layout_job(job))
}

#[cfg(test)]
#[path = "command_field_tests.rs"]
mod tests;
