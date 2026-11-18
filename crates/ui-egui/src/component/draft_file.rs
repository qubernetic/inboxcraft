// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A file on a draft, as its sheet draws it (`draft-file`): 44 high, a 1 px `line` edge, 12 in: the file's icon (16, in
//! `muted`, `ink` under the pointer), its name in `mono` cut with an ellipsis, its size in `mono_s` and `muted`, and
//! "remove" as a small link. Under the pointer it stands on the `stage`; with the keys its focus edge is 2 px clear of
//! it. A file too large to send has a `minus` edge and its size in `minus`, and says so in words: a screen reader hears
//! "too large to send", and the pointer over it shows it in a tooltip. Attaching files is not built yet: the
//! composer draws only its coming-soon button, and this is the board's.

use egui::{Rect, Response, Role, Sense, Stroke, StrokeKind, Ui, UiBuilder, WidgetInfo, pos2, vec2};

use crate::draw::paint;
use crate::iconset::IconId;
use crate::keymap::Keymap;
use crate::theme::Theme;
use crate::tr;

use super::button::{LinkSize, link_with};
use super::{Hover, Shown, cut, galley, lit, solid, soon};

/// A file on a draft: what its row shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DraftFile<'a> {
    /// The file's name.
    pub name: &'a str,
    /// Its size in words ("184 KB").
    pub size: &'a str,
    /// Whether it is too large to send.
    pub too_large: bool,
}

/// What a person did to a file's row.
#[derive(Debug)]
pub struct DraftFileOutput {
    /// The row's response: its focus and its rect.
    pub row: Response,
    /// "remove" was pressed.
    pub removed: bool,
}

/// The row of `file`, `width` wide.
pub fn draft_file(ui: &mut Ui, theme: &Theme, keymap: &Keymap, file: &DraftFile<'_>, width: f32) -> DraftFileOutput {
    draft_file_with(ui, theme, keymap, file, width, None)
}

/// [`draft_file`] drawn in `shown`: a board of the gallery.
pub(crate) fn draft_file_with(
    ui: &mut Ui,
    theme: &Theme,
    keymap: &Keymap,
    file: &DraftFile<'_>,
    width: f32,
    shown: Option<Shown>,
) -> DraftFileOutput {
    let p = &theme.palette;
    let s = &theme.spacing;
    let enabled = ui.is_enabled() && !soon::is_soon(ui);
    let height = Theme::size(theme, ROW);
    let (rect, row) = ui.allocate_exact_size(vec2(width.max(height), height), if enabled { Sense::click() } else { Sense::hover() });
    // Too large is said in words as well as in `minus`: heard, and in the tooltip under the pointer.
    let spoken = if file.too_large {
        tr!("{name}, {size}, too large to send", name = file.name, size = file.size)
    } else {
        tr!("{name}, {size}", name = file.name, size = file.size)
    };
    row.widget_info(|| WidgetInfo::labeled(Role::ListItem, enabled, &spoken));
    let lit = lit(ui, theme, &row, enabled, shown, Hover::AtOnce);
    let painter = solid(ui);
    if lit.hover {
        painter.rect_filled(rect, 0.0, p.stage);
    }
    if enabled {
        painter.rect_stroke(rect, 0.0, Stroke::new(1.0, if file.too_large { p.minus } else { p.line }), StrokeKind::Inside);
    } else {
        paint::dashed(&painter, rect, p.line);
    }
    if lit.focus {
        paint::focus_ring(&painter, theme, rect, paint::Ring::Outside);
    }
    let mid = rect.center().y;
    let side = theme.boxes().icon;
    let mut left = rect.left() + s.s3;
    let icon_ink = if !enabled {
        p.dim
    } else if lit.hover {
        p.ink
    } else {
        p.muted
    };
    crate::iconset::paint(&painter, Rect::from_min_size(pos2(left, mid - side / 2.0), vec2(side, side)), IconId::File, icon_ink);
    left += side + s.s3;
    let remove = tr!("remove");
    let link_width = galley(ui, remove, theme.fonts.mono_s, p.cyan_text).size().x.ceil();
    let link =
        Rect::from_min_size(pos2(rect.right() - s.s3 - link_width, mid - theme.boxes().row_link / 2.0), vec2(link_width, theme.boxes().row_link));
    let size_ink = if file.too_large && enabled { p.minus } else { p.muted };
    let size = galley(ui, file.size, theme.fonts.mono_s, size_ink);
    let size_left = link.left() - s.s3 - paint::run_box(size.size().x);
    paint::run_at(&painter, pos2(size_left, mid - size.size().y / 2.0), size, size_ink);
    let name_ink = if enabled { p.ink } else { p.muted };
    let name = cut(ui, file.name, theme.fonts.mono, name_ink, (size_left - s.s3 - left).max(1.0));
    paint::run_at(&painter, pos2(left, mid - name.size().y / 2.0), name, name_ink);
    let removed = ui
        .scope_builder(UiBuilder::new().max_rect(link).id_salt(("draft-file-remove", file.name)), |ui| {
            let response = link_with(ui, theme, keymap, remove, LinkSize::Small, enabled, None);
            let spoken = tr!("Remove {name}", name = file.name);
            ui.ctx().accesskit_node_builder(response.id, |node| node.set_label(spoken.as_str()));
            response.clicked()
        })
        .inner;
    let row = if file.too_large { super::tooltip::attach(row, theme, keymap, tr!("Too large to send"), None) } else { row };
    DraftFileOutput { row, removed }
}

/// A file's row is 44 high at 100 %, in points.
const ROW: f32 = 44.0;

#[cfg(test)]
#[path = "draft_file_tests.rs"]
mod tests;
