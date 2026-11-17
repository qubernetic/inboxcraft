// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The banner over a message or a page: a label in capitals in the kind's colour, the words in `caption` and `ink`
//! beside it, and under them the small links that answer it, on the `panel` fill in a 1 px edge of the kind's colour.
//! 8 above and below, 12 left and right and between the label and the words, 16 between the links. Its text is data:
//! cut only between characters, drawn as plain words (every control character a space), wrapped onto at most three
//! lines, the last ending with one ellipsis. A link gives back the id the caller gave it, never words from the text.
//! Nothing in it moves.

use egui::{Align, Layout, Rect, Role, Sense, Stroke, StrokeKind, Ui, UiBuilder, WidgetInfo, pos2, vec2};

use crate::keymap::Keymap;
use crate::theme::Theme;
use crate::tr;

use super::button::{LinkSize, link_sized};

/// What a banner says, and so its label and colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BannerKind {
    /// Remote pictures are held back: PICTURES in `warn`, a `line` edge.
    RemotePictures,
    /// A message came back undelivered: NOT DELIVERED in `minus`, a `minus` edge.
    Bounce,
    /// An act failed: ERROR in `minus`, a `minus` edge.
    Error,
    /// An account keeps failing: ERROR in `minus`, a `minus` edge.
    Failing,
    /// A plain note: NOTE in `muted`, a `panel_line` edge.
    Notice,
    /// A warning about the message (a link that goes elsewhere than it says): CAREFUL in `minus`, a `minus` edge.
    Careful,
}

/// A link of a banner: its words and the id [`banner`] gives back when it is pressed, from the caller's own list.
#[derive(Debug, Clone, Copy)]
pub struct BannerLink<'a> {
    /// What it says.
    pub name: &'a str,
    /// What pressing it gives back.
    pub id: &'a str,
}

/// The room above and below the banner's content, in points.
const PAD_Y: f32 = 8.0;

/// The room left and right of the banner's content, and between its label and its words, in points.
const PAD_X: f32 = 12.0;

/// The room between the words and the links under them, in points.
const UNDER: f32 = 4.0;

/// The room between two links, in points.
const LINK_GAP: f32 = 16.0;

/// The most lines the words take.
const ROWS: usize = 3;

/// A banner of `kind` across `ui`'s width saying `text`, with `links` under it; the id of the link pressed.
pub fn banner(ui: &mut Ui, theme: &Theme, keymap: &Keymap, kind: BannerKind, text: &str, links: &[BannerLink<'_>]) -> Option<String> {
    let p = &theme.palette;
    let f = &theme.fonts;
    let (word, edge, ink) = match kind {
        BannerKind::RemotePictures => (tr!("Pictures"), p.line, p.warn),
        BannerKind::Bounce => (tr!("Not delivered"), p.minus, p.minus),
        BannerKind::Error | BannerKind::Failing => (tr!("Error"), p.minus, p.minus),
        BannerKind::Notice => (tr!("Note"), p.panel_line, p.muted),
        BannerKind::Careful => (tr!("Careful"), p.minus, p.minus),
    };
    let width = ui.available_width().max(1.0);
    let label = super::galley(ui, word, f.label, ink);
    let label_width = label.size().x.ceil();
    let left = PAD_X + label_width + PAD_X;
    let room = (width - left - PAD_X).max(1.0);
    let words = super::wrapped(ui, &super::plain(text), f.caption, p.ink, room, ROWS, Align::LEFT);
    let words_high = words.size().y.max(f.caption.line);
    let link_high = theme.boxes().row_link;
    let column = if links.is_empty() { words_high } else { words_high + UNDER + link_high };
    // The label's box is a caption line high, so a banner of one line is as high as its words; on whole points, so its
    // edge lies on whole pixels at every text scale.
    let high = (PAD_Y + column.max(f.caption.line) + PAD_Y).ceil();
    let (_, rect) = ui.allocate_space(vec2(width, high));
    let response = ui.interact(rect, ui.scope_id().with("banner").with(text), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, text));
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        painter.rect(rect, 0.0, p.panel, Stroke::new(1.0, edge), StrokeKind::Inside);
        let label_top = (rect.top() + PAD_Y + (f.caption.line - label.size().y) / 2.0).round();
        painter.galley(pos2(rect.left() + PAD_X, label_top), label, ink);
        painter.galley(pos2(rect.left() + left, rect.top() + PAD_Y), words, p.ink);
    }
    if links.is_empty() {
        return None;
    }
    let row = Rect::from_min_size(pos2(rect.left() + left, rect.top() + PAD_Y + words_high + UNDER), vec2(room, link_high));
    let mut child = ui.new_child(UiBuilder::new().max_rect(row).layout(Layout::left_to_right(Align::Center)));
    child.spacing_mut().item_spacing.x = LINK_GAP;
    let mut pressed = None;
    for link in links {
        if link_sized(&mut child, theme, keymap, link.name, LinkSize::Small).clicked() {
            pressed = Some(link.id.to_owned());
        }
    }
    pressed
}

#[cfg(test)]
#[path = "banner_tests.rs"]
mod tests;
