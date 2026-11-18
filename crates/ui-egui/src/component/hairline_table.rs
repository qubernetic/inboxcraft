// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A table of a fitted message as hairlines: a 1 px `line` edge round it; the head on the `panel` fill, 28 high, its
//! labels in `label` and `muted`; then a row for each line, 32 high, a `panel_line` between two, the first column in
//! `body` filling what the others leave and wrapping onto more lines (the row then 6 above and below its text), the
//! others in `mono`, right-aligned in their widths; a `line` over the total, 36 high, in `strong`. 12 in from the sides
//! and between the columns. Every cell is data: plain words; a cell of a narrow column stays on one line, cut between
//! grapheme clusters with one ellipsis; a screen reader hears the table's text whole. Nothing in it moves.

use egui::{Align, Color32, Rect, Response, Role, Sense, Stroke, StrokeKind, Ui, WidgetInfo, pos2, vec2};

use crate::draw::paint;
use crate::theme::{Font, Theme};

/// The room left and right of a row's cells and between two cells, in points.
const PAD: f32 = 12.0;

/// The room above and below a row whose first cell wraps, in points.
const PAD_Y: f32 = 6.0;

/// The most lines the first cell of a row wraps onto.
const MOST_LINES: usize = 12;

/// The most rows of a table drawn.
const MOST_ROWS: usize = 200;

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

/// What a screen reader hears for `table`: its head and its rows, cell by cell.
pub(crate) fn spoken(table: &Table<'_>) -> String {
    let line = |cells: &[&str]| cells.iter().map(|cell| super::spoken(cell)).collect::<Vec<_>>().join(", ");
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

/// The hairline table across `ui`'s width; its response, a group named by [`spoken`].
pub fn hairline_table(ui: &mut Ui, theme: &Theme, table: &Table<'_>) -> Response {
    let p = &theme.palette;
    let f = &theme.fonts;
    let width = ui.available_width().max(1.0).floor();
    let [head_high, row_high, total_high] = theme.boxes().table;
    let fixed: f32 = table.widths.iter().map(|width| width + PAD).sum();
    let first = (width - PAD * 2.0 - fixed).max(1.0);
    let columns = table.heads.len().max(1);
    // Each row laid out first: the first cell wraps, the others stay on one line.
    let lay = |cells: &[&str], font: Font, mono: Font| {
        let (words, _) = super::capped(cell(cells, 0), super::MOST);
        let lead = super::wrapped(ui, &super::plain(words), font, p.ink, first, MOST_LINES, Align::LEFT);
        let rest: Vec<_> = (1..columns)
            .map(|index| super::fitted(ui, cell(cells, index), mono, p.ink, table.widths.get(index - 1).copied().unwrap_or(1.0)))
            .collect();
        (lead, rest)
    };
    let rows: Vec<_> = table.rows.iter().take(MOST_ROWS).map(|row| lay(row, f.body, f.mono)).collect();
    let total = table.total.map(|row| lay(row, f.strong, f.strong));
    let high_of =
        |lead: &std::sync::Arc<egui::Galley>, least: f32| if lead.rows.len() > 1 { (PAD_Y + lead.size().y + PAD_Y).ceil().max(least) } else { least };
    let rows_high: f32 = rows.iter().map(|(lead, _)| high_of(lead, row_high) + 1.0).sum();
    let total_part = total.as_ref().map_or(0.0, |(lead, _)| high_of(lead, total_high) + 1.0);
    let high = head_high + rows_high + total_part;
    let (rect, response) = ui.allocate_exact_size(vec2(width, high), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Group, true, spoken(table)));
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let painter = ui.painter();
    let head = Rect::from_min_size(rect.min, vec2(width, head_high));
    painter.rect_filled(head, 0.0, p.panel);
    // The cells of one row between `top` and `bottom`, the first at the left, the rest right-aligned in their widths.
    let place = |top: f32, bottom: f32, lead: std::sync::Arc<egui::Galley>, rest: Vec<std::sync::Arc<egui::Galley>>, ink: Color32| {
        let single = lead.rows.len() <= 1;
        let at = |high: f32| if single { ((top + bottom) / 2.0 - high / 2.0).round() } else { top + PAD_Y };
        paint::rows_at(painter, pos2(rect.left() + PAD, at(lead.size().y)), first, lead, ink);
        let mut right = rect.left() + PAD + first;
        for (index, galley) in rest.into_iter().enumerate() {
            right += PAD + table.widths.get(index).copied().unwrap_or(0.0);
            paint::run_right(painter, right, at(galley.size().y), galley, ink);
        }
    };
    let heads: Vec<_> = (0..columns).map(|index| super::galley(ui, cell(table.heads, index), f.label, p.muted)).collect();
    let mut right = rect.left() + PAD + first;
    for (index, galley) in heads.into_iter().enumerate() {
        let top = (head.center().y - galley.size().y / 2.0).round();
        if index == 0 {
            paint::run_at(painter, pos2(rect.left() + PAD, top), galley, p.muted);
        } else {
            right += PAD + table.widths.get(index - 1).copied().unwrap_or(0.0);
            paint::run_right(painter, right, top, galley, p.muted);
        }
    }
    let mut top = head.bottom();
    for (lead, rest) in rows {
        paint::hairline(painter, pos2(rect.left(), top), pos2(rect.right() - 1.0, top), p.panel_line);
        let bottom = top + 1.0 + high_of(&lead, row_high);
        place(top + 1.0, bottom, lead, rest, p.ink);
        top = bottom;
    }
    if let Some((lead, rest)) = total {
        paint::hairline(painter, pos2(rect.left(), top), pos2(rect.right() - 1.0, top), p.line);
        let bottom = top + 1.0 + high_of(&lead, total_high);
        place(top + 1.0, bottom, lead, rest, p.ink);
    }
    painter.rect_stroke(rect, 0.0, Stroke::new(1.0, p.line), StrokeKind::Inside);
    response
}

#[cfg(test)]
#[path = "hairline_table_tests.rs"]
mod tests;
