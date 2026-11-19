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
//! cut where they end. What enters it is one line of plain words: control characters (NUL, CR, LF, tab, the rest of
//! C0 and C1) and the bidi embeddings, overrides and isolates are dropped from what is typed, pasted, composed or set
//! by a screen reader before egui puts it in (so the caret stands after what was kept), and from the caller's text
//! before it is laid out. Without the focus a text longer than its room is cut with one `…`; with it, it scrolls to
//! the caret. The field lays out at most [`MOST`] characters.

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

/// The width the sheet draws the field at, in points: its width when the room it is given has no end.
const SHEET_WIDTH: f32 = 560.0;

/// The caret's width at 100 %, in points.
const CARET: f32 = 7.0;

/// The gap between the text and the caret after it, in points (the sheet's `typing` frame: no spacing token).
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
    let s = &theme.spacing;
    // The sheet's 12 inside the 1 px edge, the 8 between the parts, the 4 beside a mailbox's name in its mark.
    let (inset, gap, mark_pad) = (s.s3 + 1.0, s.s2, s.s1);
    let mono = theme.fonts.mono;
    // egui cuts what is typed, pasted, composed or set by a screen reader at the limit (`char_limit`); a longer text
    // from the caller is cut here, between graphemes, and cleaned, before anything is laid out.
    if text.len() > MOST {
        let kept = super::capped(text, MOST).0.len();
        text.truncate(kept);
    }
    if let Some(clean) = cleaned(text) {
        *text = clean;
    }
    let width = if ui.available_width().is_finite() { ui.available_width().max(HIGH) } else { SHEET_WIDTH };
    let high = height(theme);
    let (rect, area) = ui.allocate_exact_size(vec2(width, high), Sense::hover());
    let had_focus = ui.memory(|memory| memory.has_focus(id));
    clean_input(ui, id, had_focus);
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
    let mut left = rect.left() + inset;
    let prompt_wide = paint::run_box(prompt.size().x);
    painter.galley(pos2(left, text_top), prompt, p.cyan_text);
    left += prompt_wide + gap;
    // The chip and the scope's name, from the right.
    let narrowed = scope != Scope::Open;
    let chip_key = if focus || !text.is_empty() || narrowed { "Esc" } else { open_key };
    let chip = Chip::new(ui, theme, chip_key, ChipState::Default);
    let chip_at = pos2(rect.right() - inset - chip.size().x, (rect.center().y - chip.size().y / 2.0).round());
    let mut right = chip_at.x - gap;
    chip.paint(&painter, chip_at);
    if let Some(name) = scope.name() {
        let label = theme.fonts.label;
        let galley = super::galley(ui, name, label, p.muted);
        let wide = paint::run_box(galley.size().x);
        let top = (rect.center().y - label.line / 2.0).round();
        painter.galley(pos2(right - wide, top), galley, p.muted);
        right -= wide + gap;
    }
    // The scope's mark.
    let mark_high = Theme::size(theme, MARK);
    let key = theme.fonts.key;
    let mark = match scope {
        Scope::Mailbox(name) => {
            // A mailbox's name is the server's: at most a third of the field, cut with one `…`.
            let most = (width / 3.0 - mark_pad * 2.0).max(0.0).floor();
            let fitted = super::fit_capped(ui, &super::plain(super::capped(name, super::MOST).0), key, most);
            Some((super::galley(ui, &fitted, key, p.on_cyan), true))
        }
        other => other.mark().map(|mark| (super::galley(ui, mark, key, p.on_cyan), false)),
    };
    if let Some((galley, padded)) = mark {
        let wide = if padded { paint::run_box(galley.size().x) + mark_pad * 2.0 } else { mark_high };
        let block = Rect::from_min_size(pos2(left, (rect.center().y - mark_high / 2.0).round()), vec2(wide, mark_high));
        painter.rect_filled(block, 0.0, p.cyan);
        paint::run_centred(&painter, block, galley, p.on_cyan);
        left = block.right() + gap;
    }
    // The text, its caret and its hint.
    let caret = vec2((CARET * theme.text_scale.factor()).round(), theme.boxes().caret);
    let room = (right - caret.x - gap - left).max(1.0);
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
        let shift = if focus { caret.x + gap } else { 0.0 };
        let galley = super::galley(ui, hint, mono, p.muted);
        super::solid(ui).with_clip_rect(text_clip).galley(pos2(left + shift, text_top), galley, p.muted);
    }
    if focus {
        let block = Rect::from_min_size(pos2(caret_left.round(), (text_top + (mono.line - caret.y) / 2.0).round()), caret);
        let since = caret_since(ui, id, output.cursor_range.map_or(0, |range| range.primary.index.0));
        // The caret blinks at full and at reduced; at off it is still (M8).
        let still = !live_focus || crate::motion::level(ui.ctx()) == crate::motion::Level::Off;
        if crate::motion::blink(ui.ctx(), since, still) {
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

/// Whether `c` never enters the field: a control character (C0, DEL, C1) or a bidi embedding, override or isolate.
/// The direction marks stay, as [`super::plain`] keeps them.
fn unwanted(c: char) -> bool {
    c.is_control() || super::bidi_override(c)
}

/// `text` without what never enters the field, or `None` when it holds none of it.
fn cleaned(text: &str) -> Option<String> {
    text.chars().any(unwanted).then(|| text.chars().filter(|c| !unwanted(*c)).collect())
}

/// Cleans, in this frame's input, what would enter the field `id` (`focused` or not): typed and pasted text and an
/// IME's composition while it has the focus, and a screen reader's value or replacement aimed at it. egui then puts in
/// what was kept, and its caret after it.
fn clean_input(ui: &Ui, id: Id, focused: bool) {
    use egui::{Event, ImeEvent};
    let target = id.accesskit_id();
    let dirty = |text: &str| text.chars().any(unwanted);
    let any = ui.input(|input| {
        input.events.iter().any(|event| match event {
            Event::Text(text) | Event::Paste(text) | Event::Ime(ImeEvent::Commit(text) | ImeEvent::Preedit { text, .. }) => focused && dirty(text),
            Event::AccessKitActionRequest(request) => {
                request.target_node == target && matches!(&request.data, Some(egui::accesskit::ActionData::Value(value)) if dirty(value))
            }
            _ => false,
        })
    });
    if !any {
        return;
    }
    let clean = |text: &mut String| {
        if let Some(kept) = cleaned(text) {
            *text = kept;
        }
    };
    ui.ctx().input_mut(|input| {
        for event in &mut input.events {
            match event {
                Event::Text(text) | Event::Paste(text) | Event::Ime(ImeEvent::Commit(text) | ImeEvent::Preedit { text, .. }) if focused => {
                    clean(text);
                }
                Event::AccessKitActionRequest(request) if request.target_node == target => {
                    if let Some(egui::accesskit::ActionData::Value(value)) = &mut request.data
                        && let Some(kept) = cleaned(value)
                    {
                        *value = kept.into_boxed_str();
                    }
                }
                _ => {}
            }
        }
    });
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
    #[cfg(test)]
    LAID_OUT.with(|most| most.set(most.get().max(text.chars().count())));
    let mut job = egui::text::LayoutJob::single_section(text.to_owned(), font.format(colour));
    job.keep_trailing_whitespace = true;
    job.wrap.max_width = f32::INFINITY;
    job.break_on_newline = false;
    ui.fonts_mut(|fonts| fonts.layout_job(job))
}

#[cfg(test)]
thread_local! {
    /// The longest text, in characters, handed to the field's layout on this thread: the work a frame does.
    static LAID_OUT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
#[path = "command_field_tests.rs"]
mod tests;
