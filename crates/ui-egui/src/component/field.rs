// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Text fields, as their sheets draw them (`text-field`, `multiline-field`, `password-field`): the label over the box
//! in `label` (Plex Mono 11/16 in capitals, `muted`), 8 above it; the box on `term` with a 1 px `line` edge inside,
//! 36 high for one line; the text 13 in, in `body` (Plex Sans 14/22); the hint `muted` while it is empty. Hovered the
//! edge is `muted`, focused `cyan_edge` with the caret, a 7 × 16 block in `cyan_edge` 2 after the text that blinks
//! once a second ([`crate::motion::blink`]); an error's edge is `minus` with its message under the box in `caption`
//! and `minus` (D14). Disabled: no fill, a dashed `line` edge, the text `muted`, never faded. Read-only: no fill, the
//! `line` edge and the lock at the right end. Nothing else moves: the design gives a field's edge no transition.
//!
//! Also the search field (a square field with a magnifier before the text), the margin of a form field, and the plain
//! line and text the composer writes in until its own restyle.

use std::sync::Arc;

use egui::text::{LayoutJob, TextWrapping};
use egui::text_edit::{TextEditOutput, TextEditState};
use egui::{
    Align2, Color32, Galley, Id, Margin, Painter, Rect, Response, ScrollArea, Sense, Stroke, StrokeKind, TextBuffer, TextEdit, Ui, UiBuilder, Vec2,
    pos2, vec2,
};

use crate::draw::paint;
use crate::iconset::IconId;
use crate::motion;
use crate::theme::{Font, Theme};
use crate::tr;

use super::{Shown, galley, scroll, soon};

/// The margin inside a form field: the field is as tall as a button with a little more room.
#[must_use]
pub fn margin(theme: &Theme) -> Margin {
    let s = &theme.spacing;
    // A few points each; the clamp keeps the casts inside i8.
    Margin::symmetric(s.s2.clamp(0.0, 127.0) as i8, (s.s1 + 2.0).clamp(0.0, 127.0) as i8)
}

/// The search pill `width` points wide: the field colour with its border (the focus ring while it has the focus), a
/// magnifier and the text, `hint` while it is empty. Its response is the text field's.
pub fn search(ui: &mut Ui, theme: &Theme, id: Id, text: &mut String, hint: &str, width: f32) -> Response {
    let p = &theme.palette;
    let s = &theme.spacing;
    let height = theme.boxes().field;
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let round = 0.0;
    let focused = ui.memory(|memory| memory.has_focus(id));
    let (edge, ink) = if focused { (Stroke::new(2.0, p.cyan_edge), p.ink) } else { (Stroke::new(1.0, p.line), p.muted) };
    // Coming soon, the field is disabled with its `Ui`, and its edge is dashed.
    let soon = soon::is_soon(ui);
    if ui.is_rect_visible(rect) {
        ui.painter().rect_filled(rect, round, p.term);
        if soon {
            paint::dashed(ui.painter(), rect, p.muted);
        } else {
            ui.painter().rect_stroke(rect, round, edge, StrokeKind::Inside);
        }
    }
    let icon = theme.fonts.body.size * 1.15;
    let at = Rect::from_center_size(pos2(rect.left() + s.s3 + icon / 2.0, rect.center().y), vec2(icon, icon));
    crate::iconset::paint(ui.painter(), at, IconId::Search, ink);
    let inner = Rect::from_min_max(pos2(at.right() + s.s2, rect.top()), pos2(rect.right() - s.s3, rect.bottom()));
    let edit = TextEdit::singleline(text)
        .id(id)
        .hint_text(hint)
        .frame(egui::Frame::NONE)
        .margin(Margin::ZERO)
        .align(Align2::LEFT_CENTER)
        .desired_width(inner.width());
    ui.put(inner, edit)
}

/// What a field says and how it may be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Field<'a> {
    /// Its name, over the box in capitals and for a screen reader as written; empty for none.
    pub label: &'a str,
    /// What stands in it while it is empty.
    pub hint: &'a str,
    /// What is wrong with what is in it, under the box (D14).
    pub error: Option<&'a str>,
    /// Whether it can be used; a field in a disabled `Ui` or coming soon cannot either.
    pub enabled: bool,
    /// Whether its text can be read and copied but not changed.
    pub read_only: bool,
}

impl<'a> Field<'a> {
    /// A usable field named `label` with `hint` in it while it is empty.
    #[must_use]
    pub const fn new(label: &'a str, hint: &'a str) -> Self {
        Self { label, hint, error: None, enabled: true, read_only: false }
    }
}

/// What a password field holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Secret {
    /// What the person types: a dot for every character.
    Typed,
    /// A password kept in the system's credential store: never shown, not even as dots; the field takes a new one,
    /// and its hint and the sentence under it say that leaving it empty keeps the stored one.
    Stored,
}

/// The longest password a field takes, in characters.
pub const MAX_SECRET: usize = 256;

/// The Phosphor glyphs the field sheets name that are not icons of the set: the lock of a read-only or stored field
/// and the password mark.
const LOCK: char = '\u{e2fa}';
const PASSWORD: char = '\u{e752}';

/// The caret's width at 100 %, in points.
const CARET_WIDTH: f32 = 7.0;

/// The caret's block, 7 × 16 at 100 %, in points at the text scale.
fn caret_size(theme: &Theme) -> Vec2 {
    vec2((CARET_WIDTH * theme.text_scale.factor()).round(), theme.boxes().caret)
}

/// The room between the box's outer edge and its text, left and right: the 12 the sheet pads and the 1 px edge.
fn inset(theme: &Theme) -> f32 {
    theme.spacing.s3 + 1.0
}

/// The kind of a one-line field.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Line {
    Text,
    /// A line the app reads, in `mono` (the typed time).
    Mono,
    Password(Secret),
}

/// A one-line text field filling the width `ui` offers: its label, a box 36 high and the error under it (see the
/// module). Its response is the text's: `changed`, `lost_focus` and the focus are the field's.
pub fn text_field(ui: &mut Ui, theme: &Theme, id: Id, text: &mut String, field: &Field<'_>) -> Response {
    one_line(ui, theme, id, text, field, Line::Text, usize::MAX, None)
}

/// [`text_field`] that takes at most `limit` characters.
pub fn text_field_limited(ui: &mut Ui, theme: &Theme, id: Id, text: &mut String, field: &Field<'_>, limit: usize) -> Response {
    one_line(ui, theme, id, text, field, Line::Text, limit, None)
}

/// [`text_field`] drawn in `shown` whatever the pointer and the keyboard do: a board of the gallery. A forced focus
/// shows a still caret after the text.
pub(crate) fn text_field_with(ui: &mut Ui, theme: &Theme, id: Id, text: &mut String, field: &Field<'_>, shown: Option<Shown>) -> Response {
    one_line(ui, theme, id, text, field, Line::Text, usize::MAX, shown)
}

/// A one-line field whose text is `mono` (Plex Mono 13/20), its hint too, at most `limit` characters, drawn in
/// `shown` for a board: a line the app reads, as the typed time.
pub(crate) fn mono_field(ui: &mut Ui, theme: &Theme, id: Id, text: &mut String, field: &Field<'_>, limit: usize, shown: Option<Shown>) -> Response {
    one_line(ui, theme, id, text, field, Line::Mono, limit, shown)
}

/// A password field: [`text_field`] whose text is never shown, a dot (Plex Mono 13/20) for every character, the
/// password mark at the right end; at most [`MAX_SECRET`] characters. Nothing it holds reaches the undo history,
/// the clipboard or a screen reader. With [`Secret::Stored`] it shows no dots while it is empty, its hint (the
/// sheet's "empty keeps the stored one") in it, the lock at the right end and under it the sentence that the stored
/// password is kept in the credential store and never shown. Disabled it is an empty box.
pub fn password_field(ui: &mut Ui, theme: &Theme, id: Id, text: &mut String, field: &Field<'_>, secret: Secret) -> Response {
    one_line(ui, theme, id, text, field, Line::Password(secret), MAX_SECRET, None)
}

/// [`password_field`] drawn in `shown`: a board of the gallery.
pub(crate) fn password_field_with(ui: &mut Ui, theme: &Theme, id: Id, text: &mut String, field: &Field<'_>, secret: Secret, shown: Option<Shown>) -> Response {
    one_line(ui, theme, id, text, field, Line::Password(secret), MAX_SECRET, shown)
}

/// What a field shows of the pointer and the keyboard.
#[derive(Clone, Copy)]
struct Seen {
    enabled: bool,
    hover: bool,
    focus: bool,
    /// The focus is forced by a board: the caret stands still.
    forced: bool,
}

/// How the field `id` at `rect` is seen this frame (the focus and the pointer as the last frame left them).
fn seen(ui: &Ui, id: Id, rect: Rect, field: &Field<'_>, shown: Option<Shown>) -> Seen {
    let enabled = field.enabled && ui.is_enabled() && !soon::is_soon(ui);
    let forced = shown == Some(Shown::Focus);
    let focus = enabled && (forced || ui.memory(|memory| memory.has_focus(id)));
    let hover = enabled && !field.read_only && (shown == Some(Shown::Hover) || (shown.is_none() && ui.rect_contains_pointer(rect)));
    Seen { enabled, hover, focus, forced }
}

/// The box's fill and edge `(colour, dashed)` for `field` as `seen`.
fn box_look(theme: &Theme, field: &Field<'_>, seen: Seen) -> (Option<Color32>, Color32, bool) {
    let p = &theme.palette;
    if !seen.enabled {
        return (None, p.line, true);
    }
    let edge = if seen.focus {
        p.cyan_edge
    } else if field.error.is_some() {
        p.minus
    } else if seen.hover {
        p.muted
    } else {
        p.line
    };
    ((!field.read_only).then_some(p.term), edge, false)
}

/// Paints the box at `rect`: its fill and its 1 px edge inside, dashed when `dashed`.
fn paint_box(painter: &Painter, rect: Rect, fill: Option<Color32>, edge: Color32, dashed: bool) {
    if let Some(fill) = fill {
        painter.rect_filled(rect, 0.0, fill);
    }
    if dashed {
        paint::dashed(painter, rect, edge);
    } else {
        painter.rect_stroke(rect, 0.0, Stroke::new(1.0, edge), StrokeKind::Inside);
    }
}

/// The label over a field, `width` wide: its height (0 with no label).
fn label_row(ui: &mut Ui, theme: &Theme, label: &str, width: f32) {
    if label.is_empty() {
        return;
    }
    let text = galley(ui, label, theme.fonts.label, theme.palette.muted);
    let (rect, _) = ui.allocate_exact_size(vec2(width, theme.fonts.label.line), Sense::hover());
    if ui.is_rect_visible(rect) {
        super::solid(ui).galley(rect.min, text, theme.palette.muted);
    }
}

/// The sentence under a field: its error in `minus`, else `note` in `muted`, wrapped at `width`.
fn under(ui: &mut Ui, theme: &Theme, error: Option<&str>, note: Option<&str>, width: f32) {
    let p = &theme.palette;
    let Some((text, colour)) = error.map(|error| (error, p.minus)).or(note.map(|note| (note, p.muted))) else { return };
    let mut job = LayoutJob::single_section(theme.fonts.caption.text(text), theme.fonts.caption.format(colour));
    job.wrap = TextWrapping { max_width: width.max(1.0), ..TextWrapping::default() };
    let text = ui.painter().layout_job(job);
    let (rect, _) = ui.allocate_exact_size(vec2(width, text.size().y), Sense::hover());
    if ui.is_rect_visible(rect) {
        super::solid(ui).galley(rect.min, text, colour);
    }
}

/// `text` laid out in `font` and `colour` for a field: wrapped at `wrap` when it is `Some`, on one line otherwise,
/// trailing spaces kept; a password's characters each a dot.
fn layout(ui: &Ui, text: &str, font: Font, colour: Color32, wrap: Option<f32>, password: bool) -> Arc<Galley> {
    let shown = if password { std::iter::repeat_n(egui::epaint::text::PASSWORD_REPLACEMENT_CHAR, text.chars().count()).collect() } else { text.to_owned() };
    let mut job = LayoutJob::single_section(shown, font.format(colour));
    job.keep_trailing_whitespace = true;
    match wrap {
        Some(width) => job.wrap.max_width = width.max(1.0),
        None => {
            job.wrap.max_width = f32::INFINITY;
            job.break_on_newline = false;
        }
    }
    ui.fonts_mut(|fonts| fonts.layout_job(job))
}

/// The scope a field's text is edited in: disabled when it cannot be used, never faded, and egui's own caret hidden
/// (the field paints the design's block).
fn edit_scope(ui: &mut Ui, enabled: bool) {
    if !enabled {
        ui.disable();
    }
    ui.set_opacity(1.0);
    let cursor = &mut ui.visuals_mut().text_cursor;
    cursor.stroke = Stroke::NONE;
    cursor.blink = false;
    cursor.preview = false;
}

/// Paints the caret of the edit in `output` (the field `id`), clipped to `clip`: a block 2 after a character before
/// it on its row, at the row's start otherwise, 3 down in a 22 row (centred); blinking unless `still`.
fn paint_caret(ui: &Ui, theme: &Theme, id: Id, output: &TextEditOutput, clip: Rect, still: bool) {
    let cursor = output.cursor_range.map_or_else(|| output.galley.end(), |range| range.primary);
    let at = output.galley.pos_from_cursor(cursor);
    let column = output.galley.layout_from_cursor(cursor).column;
    let size = caret_size(theme);
    let gap = if column > egui::text::CharIndex(0) { theme.spacing.s1 / 2.0 } else { 0.0 };
    let min = output.galley_pos + vec2(at.left() + gap, at.top() + (at.height() - size.y) / 2.0);
    let ppp = ui.ctx().pixels_per_point();
    let snap = |value: f32| (value * ppp).round() / ppp;
    let block = Rect::from_min_size(pos2(snap(min.x), snap(min.y)), size);
    // The blink starts again whenever the caret moves or the field takes the focus.
    let now = ui.input(|input| input.time);
    let key = id.with("caret-since");
    let index = cursor.index.0;
    let pass_now = ui.ctx().cumulative_pass_nr();
    let since = ui.data_mut(|data| {
        let since = match data.get_temp::<(f64, usize, u64)>(key) {
            Some((since, last, pass)) if last == index && pass.saturating_add(1) >= pass_now => since,
            _ => now,
        };
        data.insert_temp(key, (since, index, pass_now));
        since
    });
    if motion::blink(ui.ctx(), since, still) {
        ui.painter().with_clip_rect(clip).rect_filled(block, 0.0, theme.palette.cyan_edge);
    }
}

/// Paints `hint` in `muted` at `at` (the text's place), clipped to `clip`; after the caret's block and the box's gap
/// when the field has the focus.
fn paint_hint(ui: &Ui, theme: &Theme, hint: &str, font: Font, at: egui::Pos2, clip: Rect, focus: bool) {
    if hint.is_empty() {
        return;
    }
    let shift = if focus { caret_size(theme).x + theme.spacing.s2 } else { 0.0 };
    let text = layout(ui, hint, font, theme.palette.muted, None, false);
    super::solid(ui).with_clip_rect(clip).galley(at + vec2(shift, 0.0), text, theme.palette.muted);
}

/// The label and screen-reader name of a field.
fn name<'a>(field: &Field<'a>) -> &'a str {
    if field.label.is_empty() { field.hint } else { field.label }
}

/// The one-line field of every kind.
#[allow(clippy::too_many_arguments)]
fn one_line(ui: &mut Ui, theme: &Theme, id: Id, text: &mut String, field: &Field<'_>, kind: Line, limit: usize, shown: Option<Shown>) -> Response {
    let p = &theme.palette;
    let width = if ui.available_width().is_finite() { ui.available_width().max(theme.boxes().field) } else { theme.boxes().field };
    let height = theme.boxes().field;
    let stored = kind == Line::Password(Secret::Stored);
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = theme.spacing.s2;
        label_row(ui, theme, field.label, width);
        let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
        let seen = seen(ui, id, rect, field, shown);
        let (fill, edge, dashed) = box_look(theme, field, seen);
        let painter = super::solid(ui);
        if ui.is_rect_visible(rect) {
            paint_box(&painter, rect, fill, edge, dashed);
        }
        let password = matches!(kind, Line::Password(_));
        let font = if password || kind == Line::Mono { theme.fonts.mono } else { theme.fonts.body };
        // The mark at the right end: the lock of a read-only or stored field, the password mark.
        let mark = match kind {
            _ if field.read_only || stored => Some(LOCK),
            Line::Password(Secret::Typed) => Some(PASSWORD),
            Line::Text | Line::Mono => None,
            Line::Password(Secret::Stored) => Some(LOCK),
        };
        let pad = inset(theme);
        let side = theme.boxes().icon;
        let right = if mark.is_some() { pad + side + theme.spacing.s2 } else { pad };
        let colour = if seen.enabled { p.ink } else { p.muted };
        let vertical = ((height - font.line) / 2.0).max(0.0);
        let inner = Rect::from_min_max(pos2(rect.left() + pad, rect.top() + vertical), pos2(rect.right() - right, rect.bottom() - vertical));
        let response = ui
            .scope_builder(UiBuilder::new().max_rect(rect), |ui| {
                edit_scope(ui, seen.enabled);
                // A disabled password field is an empty box.
                let mut nothing = String::new();
                let mut fixed: &str = text.as_str();
                let buffer: &mut dyn TextBuffer = if password && !seen.enabled {
                    &mut nothing
                } else if field.read_only {
                    &mut fixed
                } else {
                    text
                };
                let empty = buffer.as_str().is_empty();
                let mut layouter = |ui: &Ui, text: &dyn TextBuffer, _: f32| layout(ui, text.as_str(), font, colour, None, password);
                let edit = TextEdit::singleline(buffer)
                    .id(id)
                    .frame(egui::Frame::NONE.inner_margin(margin_of(rect, inner)))
                    .desired_width(f32::INFINITY)
                    .min_size(vec2(0.0, height))
                    .char_limit(limit)
                    .password(password)
                    .layouter(&mut layouter);
                let output = edit.show(ui);
                if password && let Some(mut state) = TextEditState::load(ui.ctx(), id) {
                    state.clear_undoer();
                    state.store(ui.ctx(), id);
                }
                let clip = inner.expand2(vec2(caret_size(theme).x + theme.spacing.s1, vertical));
                if empty {
                    // A stored password's hint is words, in `body`, centred as words are.
                    let hint_font = if stored { theme.fonts.body } else { font };
                    let at = pos2(inner.left(), rect.top() + ((height - hint_font.line) / 2.0).max(0.0));
                    paint_hint(ui, theme, field.hint, hint_font, at, Rect::from_x_y_ranges(inner.x_range(), rect.shrink(1.0).y_range()), seen.focus);
                }
                if seen.focus && !field.read_only {
                    paint_caret(ui, theme, id, &output, clip.intersect(rect.shrink(1.0)), seen.forced);
                }
                output.response.response
            })
            .inner;
        let spoken = name(field);
        ui.ctx().accesskit_node_builder(response.id, |node| node.set_label(spoken));
        if let Some(glyph) = mark {
            let at = Rect::from_min_size(pos2(rect.right() - pad - side, rect.center().y - side / 2.0), vec2(side, side));
            let ink = if !seen.enabled && password { p.dim } else { p.muted };
            crate::iconset::paint_glyph(&painter, at, glyph, ink);
        }
        let note = stored.then_some(tr!("The stored password is in your system's credential store and is never shown."));
        under(ui, theme, field.error, note, width);
        response
    })
    .inner
}

/// The margin that puts an edit's text at `inner` inside `outer`.
fn margin_of(outer: Rect, inner: Rect) -> Margin {
    // A few dozen points at most; the clamp keeps the casts inside i8.
    let points = |value: f32| value.round().clamp(0.0, 127.0) as i8;
    Margin {
        left: points(inner.left() - outer.left()),
        right: points(outer.right() - inner.right()),
        top: points(inner.top() - outer.top()),
        bottom: points(outer.bottom() - inner.bottom()),
    }
}

/// A field of several lines filling the width `ui` offers: its label, a box that grows with its text from 96 to 240
/// high and then scrolls, its scroll bar 12 wide inside the right edge, and the error under it; otherwise as
/// [`text_field`]. Enter starts a new line.
pub fn multiline_field(ui: &mut Ui, theme: &Theme, id: Id, text: &mut String, field: &Field<'_>) -> Response {
    multiline_with(ui, theme, id, text, field, None)
}

/// [`multiline_field`] drawn in `shown`: a board of the gallery.
pub(crate) fn multiline_with(ui: &mut Ui, theme: &Theme, id: Id, text: &mut String, field: &Field<'_>, shown: Option<Shown>) -> Response {
    let p = &theme.palette;
    let s = &theme.spacing;
    let width = if ui.available_width().is_finite() { ui.available_width().max(theme.boxes().field * 2.0) } else { theme.boxes().field * 2.0 };
    let font = theme.fonts.body;
    let [least, most] = theme.boxes().multiline;
    let pad = inset(theme);
    let lock = field.read_only;
    let side = theme.boxes().icon;
    // The room right of the text: the pad, or the lock's column of a read-only field.
    let right = if lock { 1.0 + s.s3 + side + s.s2 } else { pad };
    let wrap = width - pad - right;
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = s.s2;
        label_row(ui, theme, field.label, width);
        // Every row is a line high, the last one too.
        let rows = layout(ui, text, font, p.ink, Some(wrap), false).rows.len().max(1) as f32 * font.line;
        let whole = rows + s.s2 * 2.0 + 2.0;
        let scrolls = whole > most;
        let height = whole.clamp(least, most);
        let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
        let seen = seen(ui, id, rect, field, shown);
        let (fill, edge, dashed) = box_look(theme, field, seen);
        let painter = super::solid(ui);
        if ui.is_rect_visible(rect) {
            paint_box(&painter, rect, fill, edge, dashed);
        }
        let colour = if seen.enabled { p.ink } else { p.muted };
        let inside = rect.shrink(1.0);
        // While it scrolls, the bar takes 12 at the right and the text keeps 4 from it.
        let right_pad = if scrolls { s.s1 } else { right - 1.0 };
        let bar = if scrolls { theme.sizes.scrollbar } else { 0.0 };
        let view = Rect::from_min_max(pos2(rect.left() + pad, inside.top() + s.s2), pos2(inside.right() - bar - right_pad, inside.bottom() - s.s2));
        let margin = margin_of(Rect::from_min_max(inside.min, pos2(inside.right() - bar, inside.bottom())), view);
        // A child that leaves the cursor where the box put it.
        let mut child = ui.new_child(UiBuilder::new().max_rect(inside));
        let response = {
            let ui = &mut child;
            {
                edit_scope(ui, seen.enabled);
                let mut fixed: &str = text.as_str();
                let buffer: &mut dyn TextBuffer = if field.read_only { &mut fixed } else { text };
                let empty = buffer.as_str().is_empty();
                let mut layouter = |ui: &Ui, text: &dyn TextBuffer, wrap: f32| layout(ui, text.as_str(), font, colour, Some(wrap), false);
                let mut edit = |ui: &mut Ui| {
                    let output = TextEdit::multiline(buffer)
                        .id(id)
                        .frame(egui::Frame::NONE.inner_margin(margin))
                        .desired_width(f32::INFINITY)
                        .desired_rows(1)
                        .min_size(vec2(0.0, if scrolls { 0.0 } else { inside.height() }))
                        .layouter(&mut layouter)
                        .show(ui);
                    if empty {
                        paint_hint(ui, theme, field.hint, font, view.min, view, seen.focus);
                    }
                    if seen.focus && !field.read_only {
                        let clip = ui.clip_rect().intersect(view.expand2(vec2(caret_size(theme).x + s.s1, 0.0)));
                        paint_caret(ui, theme, id, &output, clip, seen.forced);
                        if scrolls && (output.response.response.changed() || output.response.response.gained_focus()) {
                            // Keep the caret clear of the pads above and below the text.
                            let cursor = output.cursor_range.map_or_else(|| output.galley.end(), |range| range.primary);
                            let at = output.galley.pos_from_cursor(cursor).translate(output.galley_pos.to_vec2());
                            ui.scroll_to_rect(at.expand2(vec2(0.0, s.s2)), None);
                        }
                    }
                    output.response.response
                };
                if scrolls {
                    let area = ScrollArea::vertical().id_salt(id.with("scroll")).max_height(inside.height()).auto_shrink([false, false]);
                    scroll::show(ui, theme, area, |ui| {
                        ui.set_clip_rect(ui.clip_rect().intersect(Rect::from_x_y_ranges(inside.x_range(), view.y_range())));
                        edit(ui)
                    })
                    .inner
                } else {
                    edit(ui)
                }
            }
        };
        let spoken = name(field);
        ui.ctx().accesskit_node_builder(response.id, |node| node.set_label(spoken));
        if lock {
            let at = Rect::from_min_size(pos2(inside.right() - s.s3 - side, inside.top() + s.s2), vec2(side, side));
            crate::iconset::paint_glyph(&painter, at, LOCK, p.muted);
        }
        under(ui, theme, field.error, None, width);
        response
    })
    .inner
}

/// A plain one-line edit without a box, filling the width `ui` offers: the composer's subject until the composer is
/// restyled. Its text is `body` in `ink`; `name` is its screen-reader name.
pub fn plain_line(ui: &mut Ui, theme: &Theme, id: Id, text: &mut String, name: &str) -> Response {
    let edit = TextEdit::singleline(text).id(id).font(theme.fonts.body.id()).frame(egui::Frame::NONE).desired_width(ui.available_width());
    let response = ui.add(edit);
    ui.ctx().accesskit_node_builder(response.id, |node| node.set_label(name));
    response
}

/// A plain edit of several lines without a box, `rows` rows high at least and laid out by `layouter`: the composer's
/// message text until the composer is restyled. `name` is its screen-reader name.
pub fn plain_text(
    ui: &mut Ui,
    id: Id,
    text: &mut String,
    rows: usize,
    layouter: &mut dyn FnMut(&Ui, &dyn TextBuffer, f32) -> Arc<Galley>,
    name: &str,
) -> Response {
    let edit = TextEdit::multiline(text).layouter(layouter).id(id).frame(egui::Frame::NONE).desired_width(ui.available_width()).desired_rows(rows);
    let response = ui.add(edit);
    ui.ctx().accesskit_node_builder(response.id, |node| node.set_label(name));
    response
}

#[cfg(test)]
#[path = "field_tests.rs"]
pub(crate) mod tests;
