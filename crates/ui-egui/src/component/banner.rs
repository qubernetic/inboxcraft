// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The banner over a message or a page: a label in capitals in the kind's colour, the words in `caption` and `ink`
//! beside it, and under them the small links that answer it, on the `panel` fill in a 1 px edge of the kind's colour.
//! 8 above and below, 12 left and right and between the label and the words, 16 between the links. Its text is data:
//! drawn as plain words (every control character a space, no bidi override), on one line, cut between grapheme clusters
//! with one ellipsis; a link's words are cut so too, and a screen reader hears them whole. A link gives back the id the
//! caller gave it, never words from the text.
//! Nothing in it moves.

use egui::{Align, Layout, Rect, Role, Sense, Stroke, StrokeKind, Ui, UiBuilder, WidgetInfo, pos2, vec2};

use crate::draw::paint;
use crate::keymap::Keymap;
use crate::theme::Theme;
use crate::tr;

use super::MOST;
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

/// A banner of `kind` across `ui`'s width saying `text`, with `links` under it; the id of the link pressed.
pub fn banner(ui: &mut Ui, theme: &Theme, keymap: &Keymap, kind: BannerKind, text: &str, links: &[BannerLink<'_>]) -> Option<String> {
    banner_in(ui, theme, keymap, kind, text, links, Ground::Boxed).0
}

/// What a banner paints under its words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ground {
    /// Its `panel` fill in a 1 px edge of its kind's colour: a banner on its own.
    Boxed,
    /// Nothing: the caller paints the ground and the edges (the window's error banner, whose edges are the window's
    /// lines and whose corner is the window's).
    Bare,
}

/// The label of `kind`, as the sheet writes it (in capitals when drawn), and its edge's and label's colours.
fn look(theme: &Theme, kind: BannerKind) -> (&'static str, egui::Color32, egui::Color32) {
    let p = &theme.palette;
    match kind {
        BannerKind::RemotePictures => (tr!("Pictures"), p.line, p.warn),
        BannerKind::Bounce => (tr!("Not delivered"), p.minus, p.minus),
        BannerKind::Error | BannerKind::Failing => (tr!("Error"), p.minus, p.minus),
        BannerKind::Notice => (tr!("Note"), p.panel_line, p.muted),
        BannerKind::Careful => (tr!("Careful"), p.minus, p.minus),
    }
}

/// The name a screen reader hears for a banner of `kind` saying `text`: its label, then its words made plain.
pub(crate) fn spoken(theme: &Theme, kind: BannerKind, text: &str) -> String {
    format!("{}: {}", look(theme, kind).0, super::spoken(text))
}

/// [`banner`] on `ground`: the id of the link pressed and the banner's rect. The banner is a group named by
/// [`spoken`]; its words are a label of their own, named by the words.
pub(crate) fn banner_in(
    ui: &mut Ui,
    theme: &Theme,
    keymap: &Keymap,
    kind: BannerKind,
    text: &str,
    links: &[BannerLink<'_>],
    ground: Ground,
) -> (Option<String>, Rect) {
    let p = &theme.palette;
    let f = &theme.fonts;
    let (word, edge, ink) = look(theme, kind);
    let width = ui.available_width().max(1.0);
    let label = super::galley(ui, word, f.label, ink);
    let label_width = paint::run_box(label.size().x);
    let left = PAD_X + label_width + PAD_X;
    let room = (width - left - PAD_X).max(1.0);
    let words = words(ui, theme, &super::plain(text), room);
    let words_high = words.size().y.max(f.caption.line);
    let link_high = theme.boxes().row_link;
    let column = if links.is_empty() { words_high } else { words_high + UNDER + link_high };
    // The label's box is a caption line high, so a banner of one line is as high as its words; on whole points, so its
    // edge lies on whole pixels at every text scale.
    let high = (PAD_Y + column.max(f.caption.line) + PAD_Y).ceil();
    let (_, rect) = ui.allocate_space(vec2(width, high));
    let id = ui.scope_id().with("banner").with(text);
    let response = ui.interact(rect, id, Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Group, true, spoken(theme, kind, text)));
    let words_at = pos2(rect.left() + left, rect.top() + PAD_Y);
    let words_rect = Rect::from_min_size(words_at, vec2(paint::run_box(words.size().x), words_high));
    let said = ui.interact(words_rect, id.with("words"), Sense::hover());
    said.widget_info(|| WidgetInfo::labeled(Role::Label, true, super::spoken(text)));
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        if ground == Ground::Boxed {
            painter.rect(rect, 0.0, p.panel, Stroke::new(1.0, edge), StrokeKind::Inside);
        }
        let label_top = (rect.top() + PAD_Y + (f.caption.line - label.size().y) / 2.0).round();
        paint::run_at(painter, pos2(rect.left() + PAD_X, label_top), label, ink);
        paint::run_at(painter, words_at, words, p.ink);
    }
    if links.is_empty() {
        return (None, rect);
    }
    let row = Rect::from_min_size(pos2(rect.left() + left, rect.top() + PAD_Y + words_high + UNDER), vec2(room, link_high));
    let mut child = ui.new_child(UiBuilder::new().max_rect(row).layout(Layout::left_to_right(Align::Center)));
    child.spacing_mut().item_spacing.x = LINK_GAP;
    let mut pressed = None;
    let mut left = room;
    for link in links {
        // A link's words are data too (an address): without bidi controls, cut with one ellipsis to the room left.
        let whole = super::spoken(link.name);
        let name = cut_name(ui, theme, &whole, left);
        if name.is_empty() {
            break;
        }
        let response = link_sized(&mut child, theme, keymap, &name, LinkSize::Small);
        // A screen reader hears the whole name, not the cut one.
        response.widget_info(|| WidgetInfo::labeled(Role::Button, response.enabled(), &whole));
        left -= response.rect.width() + LINK_GAP;
        if response.clicked() {
            pressed = Some(link.id.to_owned());
        }
    }
    (pressed, rect)
}

/// The banner's words `text` laid out in `room` points: one line, cut between grapheme clusters with one ellipsis (at
/// most [`MOST`] characters are laid out at all).
pub(crate) fn words(ui: &Ui, theme: &Theme, text: &str, room: f32) -> std::sync::Arc<egui::Galley> {
    let f = theme.fonts.caption;
    super::galley(ui, &super::fit_capped(ui, text, f, room), f, theme.palette.ink)
}

/// A link's `name` as it fits in `room` points in the small link's face: whole, or cut between grapheme clusters in its
/// written order with one ellipsis; empty when not even the ellipsis fits. At most [`MOST`] characters of it are laid
/// out at all, so a name as long as a header costs no more than a line.
pub(crate) fn cut_name(ui: &Ui, theme: &Theme, name: &str, room: f32) -> String {
    if room < 1.0 {
        return String::new();
    }
    super::fit_capped(ui, name, theme.fonts.mono_s, room)
}

#[cfg(test)]
#[path = "banner_tests.rs"]
mod tests;
