// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A recipient line of the composer, as its sheets draw it (`recipient-chip`, `recipient-field`).
//!
//! A chip is 24 high on `grid` with a 1 px `line` edge: the name in `body`, 8 in, and the cross that removes it, 14
//! in `muted`, 4 from its right end. One that is not an address has a `minus` edge, a warning before its name and the
//! name in `minus`. Under the pointer it is the `stage` with a `muted` edge and an `ink` cross; while its cross has
//! the keyboard its edge is `cyan_edge`; pressed, which the design does not draw, it takes the press of a secondary
//! button (`stage`, an `ink` edge). Disabled: no fill, a dashed `line` edge, the name `muted`, the cross `dim`.
//!
//! The field is a box on `term` 36 high: the line's name ("TO") in the `label` style 12 in, the chips and the text
//! being typed after it, and "Cc Bcc" in `mono_s` at the right end while those lines are hidden. Focused its edge is
//! `cyan_edge` with the caret; a line with a chip that is not an address has a `minus` edge and the reason under the
//! box. Chips that do not fit on one row wrap, 10 apart, and the box grows. Nothing moves: the design gives a chip and
//! the field no transition.
//!
//! In the composer the same line has no box ([`recipient_line`], the composer's frames): its name in a column 64 wide,
//! the chips 12 after it, 4 apart and wrapping 4 apart, 6 above and below them, and "Cc  Bcc" as a small link at the
//! right end; the composer rules a hairline under it. The caret says that it has the keys.

use std::sync::Arc;

use egui::{Color32, Galley, Id, Rect, Response, Role, Sense, Stroke, StrokeKind, Ui, UiBuilder, WidgetInfo, pos2, vec2};

use crate::draw::paint;
use crate::iconset::IconId;
use crate::theme::Theme;
use crate::tr;

use super::field::{self, Field};
use super::{Hover, Shown, cut, galley, lit, scroll, solid, soon};

/// Whether a recipient is an address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecipientState {
    /// An address the mail can go to.
    Valid,
    /// Text that is not an address: it blocks sending.
    Invalid,
}

/// The Phosphor warning glyph before a recipient that is not an address.
const WARNING: char = '\u{e4e0}';

/// The least width of the line's name ("TO"), in points.
const NAME_COLUMN: f32 = 24.0;

/// The widest a chip gets before its name ends with an ellipsis, in points.
const WIDEST: f32 = 176.0;

/// The gap between two rows of chips, in points.
const ROW_GAP: f32 = 10.0;

/// The width of a composer line's name column, in points (the composer's frames).
pub const LINE_NAMES: f32 = 64.0;

/// The narrowest a composer's recipient line gets with its widest chip whole beside its name and "Cc  Bcc".
pub fn line_least(ui: &Ui, theme: &Theme) -> f32 {
    let copies = galley(ui, tr!("Cc  Bcc"), theme.fonts.mono_s, theme.palette.cyan_text);
    LINE_NAMES + theme.spacing.s3 + WIDEST + theme.spacing.s3 + paint::run_box(copies.size().x)
}

/// How a recipient line is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Look {
    /// The sheet's field: a box on `term`.
    Boxed,
    /// A line of the composer: no box, the name in its column.
    Line,
}

/// A recipient's chip (see the module), 24 high: its response is the cross's, clicked when the recipient is to go.
/// A screen reader hears the chip as `name` and the cross as "Remove `name`".
pub fn recipient_chip(ui: &mut Ui, theme: &Theme, name: &str, state: RecipientState) -> Response {
    chip_with(ui, theme, name, name, None, state, None)
}

/// [`recipient_chip`] heard as `spoken`, with `tip` (its address, where it shows a name) under the pointer, drawn in
/// `shown` whatever the pointer and the keyboard do (a board of the gallery).
pub(crate) fn chip_with(
    ui: &mut Ui,
    theme: &Theme,
    name: &str,
    spoken: &str,
    tip: Option<&str>,
    state: RecipientState,
    shown: Option<Shown>,
) -> Response {
    let p = &theme.palette;
    let s = &theme.spacing;
    let enabled = ui.is_enabled() && !soon::is_soon(ui);
    let height = theme.boxes().pill;
    let glyph = theme.boxes().mark.min(height);
    let invalid = state == RecipientState::Invalid;
    let ink = chip_ink(theme, state, enabled);
    // 1 px edge and 8 before the name (the warning and 4 first when it is not an address), 4 and the cross, 4 and the
    // edge after it.
    let lead = 1.0 + s.s2 + if invalid { glyph + s.s1 } else { 0.0 };
    let tail = s.s1 + glyph + s.s1 + 1.0;
    // In a line narrower than a chip's 176, the chip is cut to the line's room ([`chip_width`]).
    let laid = chip_words(ui, theme, name, ink, lead + tail, ui.available_width());
    let (words, width) = (laid.words, (lead + laid.width + tail).ceil());
    let (rect, chip) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    chip.widget_info(|| WidgetInfo::labeled(Role::ListItem, enabled, spoken));
    // A cut chip that shows its address has the whole of it under the pointer.
    let chip = match tip.or_else(|| laid.cut.then_some(name)) {
        Some(tip) => chip.on_hover_text(tip),
        None => chip,
    };
    let cross = Rect::from_min_size(pos2(rect.right() - 1.0 - s.s1 - glyph, rect.center().y - glyph / 2.0), vec2(glyph, glyph));
    let remove = tr!("Remove {recipient}", recipient = name);
    // Two chips may show one name; what a screen reader hears holds the address.
    let id = ui.make_persistent_id(("recipient-chip", spoken));
    let response = ui.interact(cross.expand(s.s1 / 2.0), id, if enabled { Sense::click() } else { Sense::hover() });
    response.widget_info(|| WidgetInfo::labeled(Role::Button, enabled, &remove));
    // A cross the keyboard reaches out of view (a line that scrolls) is brought into it.
    if response.gained_focus() {
        response.scroll_to_me(None);
    }
    let lit = lit(ui, theme, &response, enabled, shown, Hover::AtOnce);
    let hover = enabled && (lit.hover || (shown.is_none() && chip.hovered()) || shown == Some(Shown::Hover));
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let painter = solid(ui);
    let (fill, edge, cross_ink) = if !enabled {
        (None, None, p.dim)
    } else if lit.pressed {
        (Some(p.stage), Some(p.ink), p.ink)
    } else if lit.focus {
        (Some(p.grid), Some(p.cyan_edge), p.muted)
    } else if hover {
        (Some(p.stage), Some(p.muted), p.ink)
    } else {
        (Some(p.grid), Some(if invalid { p.minus } else { p.line }), p.muted)
    };
    if let Some(fill) = fill {
        painter.rect_filled(rect, 0.0, fill);
    }
    match edge {
        Some(edge) => {
            painter.rect_stroke(rect, 0.0, Stroke::new(1.0, edge), StrokeKind::Inside);
        }
        None => paint::dashed(&painter, rect, p.line),
    }
    let mut left = rect.left() + 1.0 + s.s2;
    if invalid {
        let at = Rect::from_min_size(pos2(left, rect.center().y - glyph / 2.0), vec2(glyph, glyph));
        crate::iconset::paint_glyph(&painter, at, WARNING, if enabled { p.minus } else { p.dim });
        left += glyph + s.s1;
    }
    paint::run_at(&painter, pos2(left, rect.center().y - words.size().y / 2.0), words, ink);
    crate::iconset::paint(&painter, cross, IconId::Close, cross_ink);
    response
}

/// A recipient on a line: what its chip shows, what a screen reader hears, and whether it is an address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recipient {
    /// The name, or the address when there is none.
    pub shown: String,
    /// What a screen reader says for it.
    pub spoken: String,
    /// Its tooltip: the address, where it shows a name.
    pub tip: Option<String>,
    /// Whether it is an address.
    pub state: RecipientState,
}

/// What a recipient line shows.
#[derive(Debug, Clone, Copy)]
pub struct Recipients<'a> {
    /// The line's name ("To", "Cc", "Bcc"): in capitals before the chips, and the text field's name.
    pub name: &'a str,
    /// The chips, in order.
    pub chips: &'a [Recipient],
    /// What stands in the empty line ("Name or address").
    pub hint: &'a str,
    /// Whether "Cc Bcc" stands at the right end (the To line while those lines are hidden).
    pub copies: bool,
    /// Why the line cannot go, under the box; its edge is `minus` then, and with a chip that is not an address.
    pub error: Option<&'a str>,
    /// Whether it can be used.
    pub enabled: bool,
    /// Whether the line shows more rows of chips before it scrolls ([`GROWN_ROWS`], not [`MOST_ROWS`]): the composer
    /// grows its lines to bring every recipient into sight before it sends.
    pub grown: bool,
    /// The most height the line takes, in points, whatever its rows: fewer rows show and it scrolls, one row at least.
    /// The composer's lines keep to the room over its foot, so every row they show is in sight.
    pub room: Option<f32>,
    /// The chip, by its place, to scroll into view in this frame.
    pub bring: Option<usize>,
}

/// What a person did to a recipient line in this frame.
#[derive(Debug)]
pub struct RecipientsOutput {
    /// The text field's response: changed, lost focus, its rect.
    pub field: Response,
    /// The chip whose cross was pressed, by its place.
    pub removed: Option<usize>,
    /// "Cc Bcc" was pressed.
    pub copies: bool,
    /// The chips, by their places, drawn whole in sight in this frame (not cut off by a scroll or an edge).
    pub in_sight: Vec<usize>,
}

/// The least room the text being typed keeps beside the last chip before it goes to a row of its own, in points.
const TYPING: f32 = 32.0;

/// A recipient line (see the module), filling the width `ui` offers: the field `id` with `typing` in it.
pub fn recipient_field(ui: &mut Ui, theme: &Theme, id: Id, typing: &mut String, line: &Recipients<'_>) -> RecipientsOutput {
    recipient_field_with(ui, theme, id, typing, line, None)
}

/// [`recipient_field`] drawn in `shown`: a board of the gallery.
pub(crate) fn recipient_field_with(
    ui: &mut Ui,
    theme: &Theme,
    id: Id,
    typing: &mut String,
    line: &Recipients<'_>,
    shown: Option<Shown>,
) -> RecipientsOutput {
    recipients_drawn(ui, theme, id, typing, line, shown, Look::Boxed)
}

/// A recipient line of the composer (see the module), filling the width `ui` offers: the field `id` with `typing` in
/// it, without a box.
pub fn recipient_line(ui: &mut Ui, theme: &Theme, id: Id, typing: &mut String, line: &Recipients<'_>) -> RecipientsOutput {
    recipients_drawn(ui, theme, id, typing, line, None, Look::Line)
}

/// A recipient line in `look`.
#[allow(clippy::too_many_lines)]
fn recipients_drawn(
    ui: &mut Ui,
    theme: &Theme,
    id: Id,
    typing: &mut String,
    line: &Recipients<'_>,
    shown: Option<Shown>,
    look: Look,
) -> RecipientsOutput {
    let p = &theme.palette;
    let s = &theme.spacing;
    let boxed = look == Look::Boxed;
    let least = if boxed { theme.boxes().field * 4.0 } else { LINE_NAMES * 2.0 };
    let width = if ui.available_width().is_finite() { ui.available_width().max(least) } else { least };
    let enabled = line.enabled && ui.is_enabled() && !soon::is_soon(ui);
    let pad = if boxed { s.s3 + 1.0 } else { 0.0 };
    let chip = theme.boxes().pill;
    let field_height = if boxed { theme.boxes().field } else { chip + s.s3 };
    let label = theme.fonts.label;
    let name_width = if boxed { galley(ui, line.name, label, p.muted).size().x.max(NAME_COLUMN) } else { LINE_NAMES };
    let copies_text = if boxed { tr!("Cc Bcc") } else { tr!("Cc  Bcc") };
    // A disabled line offers no Cc Bcc: its board draws none.
    // The composer's is a link, in the accent.
    let copies_ink = if boxed { p.muted } else { p.cyan_text };
    let copies = (line.copies && enabled).then(|| galley(ui, copies_text, theme.fonts.mono_s, copies_ink));
    let gap = if boxed { s.s2 } else { s.s3 };
    let right = pad + copies.as_ref().map_or(0.0, |copies| paint::run_box(copies.size().x) + gap);
    let left = pad + name_width + gap;
    let room = (width - left - right).max(1.0);
    let row_gap = if boxed { ROW_GAP } else { s.s1 };
    let inks: Vec<Color32> = line.chips.iter().map(|recipient| chip_ink(theme, recipient.state, enabled)).collect();
    let widths: Vec<f32> =
        line.chips.iter().zip(&inks).map(|(recipient, ink)| chip_width(ui, theme, recipient, *ink, room - theme.sizes.scrollbar - s.s1)).collect();
    let (mut places, mut typing_at, mut rows) = flow(&widths, room, s.s1);
    let inner = |rows: f32| rows * chip + (rows - 1.0).max(0.0) * row_gap;
    // What the rows of a wrapped line stand in, above and below them.
    let edges = if boxed { (s.s1 + s.s1 / 2.0 + 1.0) * 2.0 } else { field_height - chip };
    // Past its most rows the line scrolls: the bar's room comes off the chips' row. It shows no more rows than its
    // height holds, so every row it shows is in sight.
    let most_rows = if line.grown { GROWN_ROWS } else { MOST_ROWS };
    let most_rows = match line.room {
        None => most_rows,
        Some(most) => (1..=most_rows).rev().find(|shown| inner(*shown as f32) + edges <= most).unwrap_or(1),
    };
    let scrolls = rows > most_rows;
    if scrolls {
        (places, typing_at, rows) = flow(&widths, (room - theme.sizes.scrollbar - s.s1).max(1.0), s.s1);
    }
    let shown_rows = rows.min(most_rows) as f32;
    let wrapped = rows > 1;
    let height = if wrapped { inner(shown_rows) + edges } else { field_height };
    ui.vertical(|ui| {
        let (mut removed, mut copies_pressed, mut in_sight) = (None, false, Vec::new());
        ui.spacing_mut().item_spacing.y = s.s2;
        let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
        let focused = shown == Some(Shown::Focus) || (enabled && ui.memory(|memory| memory.has_focus(id)));
        let invalid =
            line.error.is_some() || field::refusal(ui.ctx(), id).is_some() || line.chips.iter().any(|chip| chip.state == RecipientState::Invalid);
        let painter = solid(ui);
        if boxed && ui.is_rect_visible(rect) {
            if !enabled {
                paint::dashed(&painter, rect, p.line);
            } else {
                let edge = if focused {
                    p.cyan_edge
                } else if invalid {
                    p.minus
                } else {
                    p.line
                };
                painter.rect(rect, 0.0, p.term, Stroke::new(1.0, edge), StrokeKind::Inside);
            }
        }
        // The first row's middle: the name, the chips and "Cc Bcc" line up on it.
        let top = if wrapped { rect.top() + (height - inner(shown_rows)) / 2.0 } else { rect.top() + (height - chip) / 2.0 };
        let first = top + chip / 2.0;
        let name = galley(ui, line.name, label, p.muted);
        paint::run_at(&painter, pos2(rect.left() + pad, first - name.size().y / 2.0), name, p.muted);
        let mut typed = None;
        // The chips and the typing, from `origin`: in the box, or in the area that scrolls in it.
        let mut content = |ui: &mut Ui, origin: egui::Pos2| {
            let at = |(x, row): (f32, usize)| origin + vec2(x, row as f32 * (chip + row_gap));
            for (index, (recipient, place)) in line.chips.iter().zip(places.iter()).enumerate() {
                let place = Rect::from_min_size(at(*place), vec2(widths.get(index).copied().unwrap_or(0.0), chip));
                // Every chip is built, in view or not, so Tab and a screen reader reach it; one out of view paints
                // nothing (`chip_with`). Each chip in an id of its own: two chips may hold one address.
                let mut child = ui.new_child(UiBuilder::new().max_rect(place).id_salt(("chip", index)));
                if !enabled {
                    child.disable();
                }
                // Its row at the top of the line: the rows after it, not yet seen, come into sight with it.
                if line.bring == Some(index) {
                    child.scroll_to_rect(place, Some(egui::Align::TOP));
                }
                // Whole in sight: inside every clip over it (the line's scroll, the composer's edges, the window).
                if child.clip_rect().expand(0.5).contains_rect(place) {
                    in_sight.push(index);
                }
                let chip_shown = if enabled { None } else { Some(Shown::Rest) };
                if chip_with(&mut child, theme, &recipient.shown, &recipient.spoken, recipient.tip.as_deref(), recipient.state, chip_shown).clicked()
                {
                    removed = Some(index);
                }
            }
            let start = at(typing_at);
            let typing_rect = Rect::from_min_max(
                pos2(start.x, start.y + (chip - field_height) / 2.0),
                pos2(origin.x + room, start.y + (chip + field_height) / 2.0),
            );
            let mut child = ui.new_child(UiBuilder::new().max_rect(typing_rect));
            let hint = if line.chips.is_empty() { line.hint } else { "" };
            let field = Field { label: "", hint, error: None, enabled, read_only: false };
            typed = Some(field::bare_line(&mut child, theme, id, typing, &field, shown));
        };
        if scrolls {
            let area = Rect::from_min_max(pos2(rect.left() + left, rect.top() + 1.0), pos2(rect.right() - right, rect.bottom() - 1.0));
            let mut child = ui.new_child(UiBuilder::new().max_rect(area));
            let scroll = egui::ScrollArea::vertical().id_salt(id.with("chips")).max_height(area.height()).auto_shrink([false, false]);
            let pads = top - rect.top() - 1.0;
            scroll::show(&mut child, theme, scroll, |ui| {
                let (whole, _) = ui.allocate_exact_size(vec2(ui.available_width(), inner(rows as f32) + pads * 2.0), Sense::hover());
                content(ui, whole.min + vec2(0.0, pads));
            });
        } else {
            content(ui, pos2(rect.left() + left, top));
        }
        let typed = typed.unwrap_or_else(|| ui.interact(Rect::NOTHING, id, Sense::hover()));
        let spoken = line.name;
        ui.ctx().accesskit_node_builder(typed.id, |node| node.set_label(spoken));
        if let Some(copies) = copies {
            // The design's box: the run's width rounded up, at the right pad, the run at its left.
            let size = vec2(paint::run_box(copies.size().x), copies.size().y);
            let place = Rect::from_min_size(pos2(rect.right() - pad - size.x, first - size.y / 2.0), size);
            let response = ui.interact(place, id.with("copies"), if enabled { Sense::click() } else { Sense::hover() });
            response.widget_info(|| WidgetInfo::labeled(Role::Button, enabled, tr!("Cc  Bcc")));
            let ink = if boxed {
                if enabled && (response.hovered() || response.has_focus()) { p.ink } else { p.muted }
            } else {
                // The composer's is a small link: the accent, underlined under the pointer, on the wash pressed.
                if response.is_pointer_button_down_on() {
                    painter.rect_filled(place, 0.0, p.cyan_muted);
                }
                if response.hovered() {
                    let under = place.bottom() - 1.0;
                    painter.hline(place.x_range(), under + 0.5, Stroke::new(1.0, p.cyan_text));
                }
                p.cyan_text
            };
            paint::run_at(&painter, place.min, copies, ink);
            if response.has_focus() {
                paint::focus_ring(&painter, theme, place, paint::Ring::Outside);
            }
            copies_pressed = response.clicked();
        }
        let refused = field::refusal(ui.ctx(), id);
        let bring = line.error.is_none() && refused.as_ref().is_some_and(|(_, fresh)| *fresh);
        if let Some(error) = line.error.or(refused.as_ref().map(|(sentence, _)| sentence.as_str())) {
            let mut job = egui::text::LayoutJob::single_section(error.to_owned(), theme.fonts.caption.format(p.minus));
            job.wrap.max_width = width.max(1.0);
            let text = ui.painter().layout_job(job);
            let (place, response) = ui.allocate_exact_size(vec2(width, text.size().y), Sense::hover());
            response.widget_info(|| WidgetInfo::labeled(Role::Label, true, error));
            if bring {
                response.scroll_to_me(None);
            }
            paint::rows_at(&painter, place.min, width, text, p.minus);
        }
        RecipientsOutput { field: typed, removed, copies: copies_pressed, in_sight }
    })
    .inner
}

/// The most rows of chips a line shows before it scrolls.
const MOST_ROWS: usize = 4;

/// The most rows of chips a grown line shows before it scrolls, where its room holds them ([`Recipients::grown`]).
const GROWN_ROWS: usize = 8;

/// Where chips `widths` wide go in rows `room` wide, `gap` apart: each chip's `(x, row)`, the typing's, and the rows.
fn flow(widths: &[f32], room: f32, gap: f32) -> (Vec<(f32, usize)>, (f32, usize), usize) {
    let mut places = Vec::with_capacity(widths.len());
    let (mut x, mut row) = (0.0_f32, 0_usize);
    for width in widths {
        if x > 0.0 && x + width > room {
            x = 0.0;
            row = row.saturating_add(1);
        }
        places.push((x, row));
        x += width + gap;
    }
    if x > 0.0 && room - x < TYPING.min(room) {
        x = 0.0;
        row = row.saturating_add(1);
    }
    (places, (x, row), row.saturating_add(1))
}

/// The colour of a chip's name: what [`chip_with`] lays it out in, so the two share one layout.
fn chip_ink(theme: &Theme, state: RecipientState, enabled: bool) -> Color32 {
    let p = &theme.palette;
    if !enabled {
        p.muted
    } else if state == RecipientState::Invalid {
        p.minus
    } else {
        p.ink
    }
}

/// The width a recipient's chip takes, in points, its name laid out in `ink` as the chip lays it out: at most 176, and
/// at most `room` (with a scroll bar beside the chips too) in a line narrower than that.
fn chip_width(ui: &Ui, theme: &Theme, recipient: &Recipient, ink: Color32, room: f32) -> f32 {
    let s = &theme.spacing;
    let glyph = theme.boxes().mark.min(theme.boxes().pill);
    let invalid = recipient.state == RecipientState::Invalid;
    let lead = 1.0 + s.s2 + if invalid { glyph + s.s1 } else { 0.0 };
    let tail = s.s1 + glyph + s.s1 + 1.0;
    (lead + chip_words(ui, theme, &recipient.shown, ink, lead + tail, room).width + tail).ceil()
}

/// A chip's words as [`chip_words`] lays them out.
struct ChipWords {
    /// What the chip shows: the name whole, or cut with an ellipsis.
    words: Arc<Galley>,
    /// The box the words take: their width, or all the room they were cut to, as the design's long chip keeps its 144.
    width: f32,
    /// Whether something of the name does not show.
    cut: bool,
}

/// The words of a chip showing `name`, its edges and cross taking `sides`, in a line `room` wide: cut at the chip's
/// 176 with an ellipsis. An address is cut in its local part, never in its domain (everything after its last at sign,
/// which says where the mail goes); a domain wider than a chip grows the chip to `room`, and one wider than that is
/// cut only from its left, so its registrable end shows.
fn chip_words(ui: &Ui, theme: &Theme, name: &str, ink: Color32, sides: f32, room: f32) -> ChipWords {
    let font = theme.fonts.body;
    let room = if room.is_finite() { room.max(1.0) } else { WIDEST };
    let most = (WIDEST.min(room) - sides).max(1.0);
    let whole = galley(ui, name, font, ink);
    if whole.size().x <= most {
        let width = whole.size().x;
        return ChipWords { words: whole, width, cut: false };
    }
    let Some(domain) = name.rfind('@').and_then(|at| name.get(at..)).filter(|domain| domain.len() > 1) else {
        return ChipWords { words: cut(ui, name, font, ink, most), width: most, cut: true };
    };
    let local = name.get(..name.len().saturating_sub(domain.len())).unwrap_or_default();
    let grown = (room - sides).max(1.0);
    for most in [most, grown] {
        if whole.size().x <= most {
            let width = whole.size().x;
            return ChipWords { words: whole, width, cut: false };
        }
        // The most letters of the local part that fit before the ellipsis and the whole domain.
        let fits = |kept: usize| {
            let end = local.char_indices().nth(kept).map_or(local.len(), |(at, _)| at);
            let words = galley(ui, &format!("{}…{domain}", local.get(..end).unwrap_or_default()), font, ink);
            (words.size().x <= most).then_some(words)
        };
        if let Some(words) = most_that_fit(local.chars().count(), fits) {
            return ChipWords { words, width: most, cut: true };
        }
    }
    // The domain alone is wider than the line: its right end, after an ellipsis.
    let count = domain.chars().count();
    let fits = |kept: usize| {
        let start = domain.char_indices().nth(count.saturating_sub(kept)).map_or(domain.len(), |(at, _)| at);
        let words = galley(ui, &format!("…{}", domain.get(start..).unwrap_or_default()), font, ink);
        (words.size().x <= grown).then_some(words)
    };
    let words = most_that_fit(count, fits).unwrap_or_else(|| cut(ui, name, font, ink, grown));
    ChipWords { words, width: grown, cut: true }
}

/// What `fits` gives for the most of `0..=most` that it gives something for (it gives something for every count up to
/// that one), by halving; `None` when it gives nothing even for none.
fn most_that_fit(most: usize, fits: impl Fn(usize) -> Option<Arc<Galley>>) -> Option<Arc<Galley>> {
    let mut best = fits(0)?;
    let (mut low, mut high) = (0_usize, most);
    while low < high {
        let mid = low + (high - low).div_ceil(2);
        match fits(mid) {
            Some(words) => {
                best = words;
                low = mid;
            }
            None => high = mid.saturating_sub(1),
        }
    }
    Some(best)
}

#[cfg(test)]
#[path = "recipient_tests.rs"]
mod tests;
