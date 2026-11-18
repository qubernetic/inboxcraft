// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A table of a fitted message as hairlines: a 1 px `line` edge round it; the head on the `panel` fill, 28 high, its
//! labels in `label` and `muted`; then a row for each line, 32 high, a `panel_line` between two, the first column in
//! `body` filling what the others leave and wrapping onto more lines (the row then 6 above and below its text), the
//! others in `mono`, right-aligned in their widths; a `line` over the total, 36 high, in `strong`. 12 in from the sides
//! and between the columns. Every cell and head is data: plain words, capped before they are laid out; a cell of a
//! narrow column stays on one line, cut between grapheme clusters with one ellipsis; a screen reader hears the table's
//! text. A table is bounded before it is laid out: 16 columns, 200 rows, column widths from 1 to 480; a line under it
//! says what was left out. Only the rows in sight lay out more than their first cell. Nothing paints past the table's
//! box, and a table wider than its room scrolls sideways inside itself. Nothing in it moves.

use std::sync::Arc;

use egui::{Align, Color32, Galley, Rect, Response, Role, ScrollArea, Sense, Stroke, StrokeKind, Ui, WidgetInfo, pos2, vec2};

use crate::draw::paint;
use crate::theme::{Font, Theme};
use crate::tr;

/// The room left and right of a row's cells and between two cells, in points.
const PAD: f32 = 12.0;

/// The room above and below a row whose first cell wraps, in points.
const PAD_Y: f32 = 6.0;

/// The most lines the first cell of a row wraps onto.
const MOST_LINES: usize = 12;

/// The most rows of a table drawn.
const MOST_ROWS: usize = 200;

/// The most columns of a table drawn; the rest are said to be left out.
const MOST_COLUMNS: usize = 16;

/// The widest a column after the first is drawn, in points; a width that is not a number takes [`USUAL_WIDTH`].
const WIDEST: f32 = 480.0;

/// The width of a column after the first whose width is missing or not a number, in points.
const USUAL_WIDTH: f32 = 72.0;

/// The least room the first column keeps before the table scrolls sideways inside itself, in points.
const FIRST_LEAST: f32 = 160.0;

/// A hairline table: its columns' heads and the widths of all but the first (which fills), its rows and its total.
#[derive(Debug, Clone, Copy)]
pub struct Table<'a> {
    /// The head of each column.
    pub heads: &'a [&'a str],
    /// The width of each column after the first, in points.
    pub widths: &'a [f32],
    /// The rows, each a cell for each column (a missing cell is empty).
    pub rows: &'a [&'a [&'a str]],
    /// The total row, if any.
    pub total: Option<&'a [&'a str]>,
}

impl Table<'_> {
    /// How many columns the table has: its heads, or its longest row shown.
    fn columns(&self) -> usize {
        self.rows.iter().take(MOST_ROWS).map(|row| row.len()).chain(self.total.map(<[&str]>::len)).fold(self.heads.len(), usize::max)
    }

    /// How many columns are drawn.
    fn shown_columns(&self) -> usize {
        self.columns().clamp(1, MOST_COLUMNS)
    }

    /// The width of each column drawn after the first: a number from 1 to [`WIDEST`].
    fn widths(&self) -> Vec<f32> {
        (1..self.shown_columns())
            .map(|index| match self.widths.get(index - 1).copied() {
                Some(width) if width.is_finite() => width.clamp(1.0, WIDEST),
                _ => USUAL_WIDTH,
            })
            .collect()
    }
}

/// What a screen reader hears for `table`: its head and its rows drawn, cell by cell, each cell capped.
pub(crate) fn spoken(table: &Table<'_>) -> String {
    let columns = table.shown_columns();
    let line =
        |cells: &[&str]| (0..columns).map(|index| super::spoken(super::capped(cell(cells, index), super::MOST).0)).collect::<Vec<_>>().join(", ");
    let mut lines = vec![line(table.heads)];
    lines.extend(table.rows.iter().take(MOST_ROWS).map(|row| line(row)));
    lines.extend(table.total.map(line));
    let all = lines.join("; ");
    super::capped(&all, super::MOST * 8).0.to_owned()
}

/// The cell `index` of `cells`, or nothing.
fn cell<'a>(cells: &[&'a str], index: usize) -> &'a str {
    cells.get(index).copied().unwrap_or_default()
}

/// The hairline table across `ui`'s width; its response, a group named by [`spoken`]. A table wider than the room
/// (its first column narrower than 160) scrolls sideways inside itself. At most 16 columns and 200 rows are drawn, the
/// widths bounded; a line under the table says what was left out.
pub fn hairline_table(ui: &mut Ui, theme: &Theme, table: &Table<'_>) -> Response {
    let room = ui.available_width().max(1.0).floor();
    let widths = table.widths();
    let natural = PAD * 2.0 + FIRST_LEAST + widths.iter().map(|width| width + PAD).sum::<f32>();
    let response = if natural > room {
        let area = ScrollArea::horizontal().id_salt(ui.next_auto_id().with("table")).auto_shrink([false, true]);
        super::scroll::show(ui, theme, area, |ui| drawn(ui, theme, table, &widths, natural)).inner
    } else {
        drawn(ui, theme, table, &widths, room)
    };
    let (columns, rows) = (table.columns().saturating_sub(MOST_COLUMNS), table.rows.len().saturating_sub(MOST_ROWS));
    let note = match (columns, rows) {
        (0, 0) => None,
        (columns, 0) => Some(tr!("{columns} more columns are not shown", columns = columns)),
        (0, rows) => Some(tr!("{rows} more rows are not shown", rows = rows)),
        (columns, rows) => Some(tr!("{columns} more columns and {rows} more rows are not shown", columns = columns, rows = rows)),
    };
    if let Some(note) = note {
        super::label(ui, theme.fonts.caption, &note, theme.palette.muted);
    }
    response
}

/// The table `width` wide: what [`hairline_table`] draws in its room or inside its sideways scroll.
fn drawn(ui: &mut Ui, theme: &Theme, table: &Table<'_>, widths: &[f32], width: f32) -> Response {
    let p = &theme.palette;
    let f = &theme.fonts;
    let [head_high, row_high, total_high] = theme.boxes().table;
    let columns = table.shown_columns();
    let fixed: f32 = widths.iter().map(|width| width + PAD).sum();
    let first = (width - PAD * 2.0 - fixed).max(1.0);
    let room_of = |index: usize| if index == 0 { first } else { widths.get(index - 1).copied().unwrap_or(USUAL_WIDTH) };
    // A row's first cell laid out first: it wraps, and its lines set the row's height.
    let lead = |cells: &[&str], font: Font| {
        let (words, _) = super::capped(cell(cells, 0), super::MOST);
        super::wrapped(ui, &super::plain(words), font, p.ink, first, MOST_LINES, Align::LEFT)
    };
    // The others stay on one line: laid out only for a row in sight.
    let rest = |ui: &Ui, cells: &[&str], mono: Font| {
        (1..columns).map(|index| super::fitted(ui, cell(cells, index), mono, p.ink, room_of(index))).collect::<Vec<_>>()
    };
    let leads: Vec<_> = table.rows.iter().take(MOST_ROWS).map(|row| lead(row, f.body)).collect();
    let total = table.total.map(|row| lead(row, f.strong));
    let high_of = |lead: &Arc<Galley>, least: f32| if lead.rows.len() > 1 { (PAD_Y + lead.size().y + PAD_Y).ceil().max(least) } else { least };
    let rows_high: f32 = leads.iter().map(|lead| high_of(lead, row_high) + 1.0).sum();
    let total_part = total.as_ref().map_or(0.0, |lead| high_of(lead, total_high) + 1.0);
    let high = head_high + rows_high + total_part;
    let (rect, response) = ui.allocate_exact_size(vec2(width, high), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Group, true, spoken(table)));
    if !ui.is_rect_visible(rect) {
        return response;
    }
    // Nothing paints past the table's box.
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    let head = Rect::from_min_size(rect.min, vec2(width, head_high));
    painter.rect_filled(head, 0.0, p.panel);
    // The cells of one row between `top` and `bottom`, the first at the left, the rest right-aligned in their widths.
    let place = |top: f32, bottom: f32, lead: Arc<Galley>, rest: Vec<Arc<Galley>>, ink: Color32| {
        let single = lead.rows.len() <= 1;
        let at = |high: f32| if single { ((top + bottom) / 2.0 - high / 2.0).round() } else { top + PAD_Y };
        paint::rows_at(&painter, pos2(rect.left() + PAD, at(lead.size().y)), first, lead, ink);
        let mut right = rect.left() + PAD + first;
        for (index, galley) in rest.into_iter().enumerate() {
            right += PAD + widths.get(index).copied().unwrap_or(USUAL_WIDTH);
            paint::run_right(&painter, right, at(galley.size().y), galley, ink);
        }
    };
    // The heads are data too: plain words, capped and cut to their columns.
    let mut right = rect.left() + PAD + first;
    for index in 0..columns {
        let galley = super::fitted(ui, cell(table.heads, index), f.label, p.muted, room_of(index));
        let top = (head.center().y - galley.size().y / 2.0).round();
        if index == 0 {
            paint::run_at(&painter, pos2(rect.left() + PAD, top), galley, p.muted);
        } else {
            right += PAD + room_of(index);
            paint::run_right(&painter, right, top, galley, p.muted);
        }
    }
    let mut top = head.bottom();
    for (row, lead) in table.rows.iter().zip(leads) {
        paint::hairline(&painter, pos2(rect.left(), top), pos2(rect.right() - 1.0, top), p.panel_line);
        let bottom = top + 1.0 + high_of(&lead, row_high);
        if ui.is_rect_visible(Rect::from_min_max(pos2(rect.left(), top), pos2(rect.right(), bottom))) {
            place(top + 1.0, bottom, lead, rest(ui, row, f.mono), p.ink);
        }
        top = bottom;
    }
    if let (Some(lead), Some(row)) = (total, table.total) {
        paint::hairline(&painter, pos2(rect.left(), top), pos2(rect.right() - 1.0, top), p.line);
        let bottom = top + 1.0 + high_of(&lead, total_high);
        place(top + 1.0, bottom, lead, rest(ui, row, f.strong), p.ink);
    }
    painter.rect_stroke(rect, 0.0, Stroke::new(1.0, p.line), StrokeKind::Inside);
    response
}

#[cfg(test)]
#[path = "hairline_table_tests.rs"]
mod tests;
