// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A picture of a message that is not shown, as a box in its place on the `panel` fill: as large as the picture, by the
//! size its tag gives (scaled down to the room as the picture would be), so the text does not move when it comes;
//! without a size, across the room and as high as what it says. Its parts stand in the middle, 6 apart, 16 in from its
//! sides:
//!
//! - not loaded (a remote picture the owner has not asked for): a dashed `muted` edge, the picture icon in `muted`,
//!   "Remote picture not loaded" in `mono_s` and `muted`, the picture's text in `caption` and `ink`, and its size;
//! - loading: a `line` edge, the progress line "loading picture" and the picture's text in `muted`;
//! - not shown (a kind the app does not draw, or one here whose data could not be drawn, in words of its own): a `line`
//!   edge, the icon, why, and the picture's text;
//! - failed: a `minus` edge, the broken-picture icon and why in `minus`, the picture's text, and `try again` where it
//!   fits whole in the box.
//!
//! Every part is fitted to the box, not to the room around it. The picture's text is the sender's data: plain words
//! on one line, cut between grapheme clusters with one ellipsis, heard whole. A box smaller than what it says (a
//! one-pixel picture) shows only its edge: nothing paints past it. The
//! box asks for nothing to load: only `try again` does, on the owner's press. Nothing moves but the progress line's
//! mark.

use egui::{Rect, Response, Role, Sense, Stroke, StrokeKind, Ui, UiBuilder, Vec2, WidgetInfo, pos2, vec2};

use crate::draw::paint;
use crate::iconset;
use crate::keymap::Keymap;
use crate::theme::Theme;
use crate::tr;

use super::button::{LinkSize, link_sized};
use super::progress::{ProgressState, progress_with};

/// Why a picture is not shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PictureLook<'a> {
    /// A remote picture the owner has not asked to load.
    NotLoaded,
    /// Asked for, not here yet.
    Loading,
    /// A kind of picture the app does not draw.
    NotShown,
    /// Here, but its data could not be made into a picture: the look of [`PictureLook::NotShown`], its own words.
    Undrawable,
    /// It could not be shown, for `why`.
    Failed(&'a str),
}

/// What was asked of a picture's box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PictureAsked {
    /// Try loading it again.
    Retry,
}

/// The room left and right of the box's parts, in points.
const PAD: f32 = 16.0;

/// The room between the box's parts, in points.
const GAP: f32 = 6.0;

/// The room above and below the parts of a box with no size of its own, in points.
const BARE: f32 = 24.0;

/// The side of the picture icon, in points.
const ICON: f32 = 18.0;

/// The tallest a picture stands, in points.
pub const MOST_HIGH: f32 = 480.0;

/// The picture icon (Phosphor `image`) and the broken one (`image-broken`).
const PICTURE: char = '\u{e2ca}';
const BROKEN: char = '\u{e7a8}';

/// The words that say why the picture is not shown.
fn why(look: PictureLook<'_>) -> String {
    match look {
        PictureLook::NotLoaded => tr!("Remote picture not loaded").to_owned(),
        PictureLook::Loading => tr!("loading picture").to_owned(),
        PictureLook::NotShown => tr!("Picture not shown: InboxCraft does not draw this kind of picture").to_owned(),
        PictureLook::Undrawable => tr!("Picture not shown: its data could not be drawn").to_owned(),
        PictureLook::Failed(why) => tr!("Picture could not be shown: {why}", why = super::spoken(why)),
    }
}

/// The size a picture of `size` pixels stands at in `room` points: its own, scaled down to the room and to
/// [`MOST_HIGH`], keeping its shape; on whole points, at least one.
#[must_use]
pub fn shown_size(size: (u32, u32), room: f32) -> Vec2 {
    let (w, h) = (size.0.max(1) as f32, size.1.max(1) as f32);
    let room = if room.is_finite() { room.max(1.0) } else { 1.0 };
    let scale = (room / w).min(MOST_HIGH / h).min(1.0);
    vec2((w * scale).floor().max(1.0), (h * scale).floor().max(1.0))
}

/// What a screen reader hears for the box.
pub(crate) fn spoken(look: PictureLook<'_>, alt: &str, size: Option<(u32, u32)>) -> String {
    let alt = super::spoken(alt);
    let mut said = why(look);
    if !alt.is_empty() {
        said = tr!("{why}: {alt}", why = said, alt = alt);
    }
    if let Some((w, h)) = size {
        said = tr!("{said}, {w} × {h}", said = said, w = w, h = h);
    }
    said
}

/// The box of a picture in `look` with the sender's text `alt`, `size` pixels by its tag, across what it needs of `ui`;
/// its response and what was asked.
pub fn picture_box(
    ui: &mut Ui,
    theme: &Theme,
    keymap: &Keymap,
    look: PictureLook<'_>,
    alt: &str,
    size: Option<(u32, u32)>,
) -> (Response, Option<PictureAsked>) {
    picture_box_with(ui, theme, keymap, look, alt, size, false)
}

/// [`picture_box`]; `still` draws the loading mark whole and standing (the gallery's board).
pub(crate) fn picture_box_with(
    ui: &mut Ui,
    theme: &Theme,
    keymap: &Keymap,
    look: PictureLook<'_>,
    alt: &str,
    size: Option<(u32, u32)>,
    still: bool,
) -> (Response, Option<PictureAsked>) {
    let p = &theme.palette;
    let f = &theme.fonts;
    let room = ui.available_width().max(1.0).floor();
    // Its parts are fitted to the box, which may be narrower than the room: nothing paints past its sides.
    let width = size.map_or(room, |size| shown_size(size, room).x);
    let inner = (width - PAD * 2.0).max(1.0);
    let alt_ink = if look == PictureLook::Loading { p.muted } else { p.ink };
    let alt_galley = (!alt.trim().is_empty()).then(|| super::fitted(ui, alt, f.caption, alt_ink, inner));
    let (edge, mark, why_ink) = match look {
        PictureLook::NotLoaded => (p.muted, Some(PICTURE), p.muted),
        PictureLook::Loading => (p.line, None, p.muted),
        PictureLook::NotShown | PictureLook::Undrawable => (p.line, Some(PICTURE), p.muted),
        PictureLook::Failed(_) => (p.minus, Some(BROKEN), p.minus),
    };
    let why_galley = (look != PictureLook::Loading).then(|| super::fitted(ui, &why(look), f.mono_s, why_ink, inner));
    let size_galley = match (look, size) {
        (PictureLook::NotLoaded, Some((w, h))) => Some(super::galley(ui, &tr!("{w} × {h}", w = w, h = h), f.mono_s, p.muted)),
        _ => None,
    };
    let link_high = theme.boxes().row_link;
    // `try again` stands only where it fits whole: a link that cannot be seen is not offered.
    let retry = matches!(look, PictureLook::Failed(_)) && super::galley(ui, tr!("try again"), f.mono_s, p.cyan_text).size().x.ceil() <= inner;
    // What the box says, top to bottom: each part's height.
    let mut parts: Vec<f32> = Vec::new();
    if mark.is_some() {
        parts.push(ICON);
    }
    if look == PictureLook::Loading {
        parts.push(theme.boxes().strip);
    }
    if why_galley.is_some() {
        parts.push(f.mono_s.line);
    }
    if alt_galley.is_some() {
        parts.push(f.caption.line);
    }
    if size_galley.is_some() {
        parts.push(f.mono_s.line);
    }
    if retry {
        parts.push(link_high);
    }
    let content = parts.iter().sum::<f32>() + GAP * parts.len().saturating_sub(1) as f32;
    let wanted = match size {
        Some(size) => shown_size(size, room),
        None => vec2(room, (content + BARE * 2.0).ceil()),
    };
    let (rect, response) = ui.allocate_exact_size(wanted, Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Image, true, spoken(look, alt, size)));
    // A box smaller than what it says shows only its edge.
    let fits = content + 2.0 <= rect.height() && PAD * 2.0 + 1.0 <= rect.width();
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    if ui.is_rect_visible(rect) {
        painter.rect_filled(rect, 0.0, p.panel);
        if look == PictureLook::NotLoaded {
            paint::dashed(&painter, rect, edge);
        } else {
            painter.rect_stroke(rect, 0.0, Stroke::new(1.0, edge), StrokeKind::Inside);
        }
    }
    if !fits {
        return (response, None);
    }
    let middle = rect.center().x;
    let mut top = (rect.center().y - content / 2.0).round();
    let centred = |galley: &std::sync::Arc<egui::Galley>, top: f32| pos2((middle - paint::run_box(galley.size().x) / 2.0).round(), top);
    if let Some(glyph) = mark {
        let at = Rect::from_min_size(pos2((middle - ICON / 2.0).round(), top), vec2(ICON, ICON));
        iconset::paint_glyph(&painter, at, glyph, why_ink);
        top += ICON + GAP;
    }
    if look == PictureLook::Loading {
        let line = theme.boxes().strip;
        let spot = Rect::from_min_size(pos2(rect.left() + PAD, top), vec2(inner, line));
        let mut child = ui.new_child(UiBuilder::new().max_rect(spot).layout(egui::Layout::top_down(egui::Align::Center)));
        child.set_clip_rect(rect.intersect(ui.clip_rect()));
        progress_with(&mut child, theme, keymap, tr!("loading picture"), ProgressState::Running(0.0), "", still);
        top += line + GAP;
    }
    let lines = [(why_galley, f.mono_s.line, why_ink), (alt_galley, f.caption.line, alt_ink), (size_galley, f.mono_s.line, p.muted)];
    for (galley, line, ink) in lines {
        let Some(galley) = galley else { continue };
        paint::run_at(&painter, centred(&galley, top), galley.clone(), ink);
        top += line + GAP;
    }
    let mut asked = None;
    if retry {
        let spot = Rect::from_min_size(pos2(rect.left() + PAD, top), vec2(inner, link_high));
        let mut child = ui.new_child(UiBuilder::new().max_rect(spot).layout(egui::Layout::top_down(egui::Align::Center)));
        child.set_clip_rect(rect.intersect(ui.clip_rect()));
        if link_sized(&mut child, theme, keymap, tr!("try again"), LinkSize::Small).clicked() {
            asked = Some(PictureAsked::Retry);
        }
    }
    (response, asked)
}

#[cfg(test)]
#[path = "picture_box_tests.rs"]
mod tests;
