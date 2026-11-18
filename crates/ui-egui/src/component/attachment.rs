// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! An attachment: a box as wide as what it holds, 44 high, in a 1 px `line` edge (`minus` when saving failed), 12 in
//! from its sides and between its parts: the file icon in `muted`, the name in `mono` and `ink` (at most 240 wide), the
//! size in `mono_s` and `muted`, 12 of room, then what can be done: `open` and `save`; while saving, how far it is;
//! saved, a `plus` check and where it went, and `open folder`; failed, why in `minus`, and `try again`. Under the
//! pointer the box takes the `stage` fill and its icon `ink`; the focus ring stands 2 clear outside it. The box is one
//! stop for Tab: Enter saves it (tries again after a failure); the pointer saves only through `save` or `try again`. In
//! a box narrower than what it holds the name gives way first, then the state's words, then the links are left out
//! whole from the last, then the size; nothing paints past the box. The name is data and decides nothing but its label:
//! drawn as plain words on one line, cut between grapheme clusters with one ellipsis, heard whole; the icon is the same
//! for every file. Opening a file or its folder is not built yet: those links stand unavailable (the caller shows the
//! `coming soon` tag once). Nothing moves.

use egui::{Align, Layout, Rect, Response, Role, Sense, Stroke, StrokeKind, Ui, UiBuilder, WidgetInfo, pos2, vec2};

use crate::draw::paint::{self, Ring};
use crate::iconset::{self, IconId};
use crate::keymap::Keymap;
use crate::theme::Theme;
use crate::tr;

use super::button::{LinkSize, link_sized};
use super::soon::soon;
use super::{Hover, Shown, lit};

/// Where saving an attachment stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileState<'a> {
    /// Nothing asked yet.
    Ready,
    /// Being saved: how far, in words (`saving [=====>  ]`).
    Saving(&'a str),
    /// Saved: where it went (`saved to Downloads`).
    Saved(&'a str),
    /// Not saved: why (`not saved: the disk is full`).
    Failed(&'a str),
}

/// What was asked of an attachment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileAsked {
    /// Save it (again, after a failure).
    Save,
}

/// The room inside the box and between its parts, in points.
const PAD: f32 = 12.0;

/// The widest a name gets before it is cut, in points.
const NAME: f32 = 240.0;

/// The room between two links, in points.
const LINK_GAP: f32 = 16.0;

/// The room between the saved check and its words, in points.
const CHECK_GAP: f32 = 6.0;

/// The check of a saved file (Phosphor `check`).
const CHECK_GLYPH: char = '\u{e182}';

/// What a screen reader hears for an attachment named `name` of `size`.
pub(crate) fn spoken(name: &str, size: &str, state: FileState<'_>) -> String {
    let (name, size) = (super::spoken(name), super::spoken(size));
    match state {
        FileState::Ready => tr!("{name}, {size}", name = name, size = size),
        FileState::Saving(how) | FileState::Saved(how) | FileState::Failed(how) => {
            tr!("{name}, {size}, {state}", name = name, size = size, state = super::spoken(how))
        }
    }
}

/// An attachment `name` of `size` in `state`; its response and what was asked.
pub fn attachment(ui: &mut Ui, theme: &Theme, keymap: &Keymap, name: &str, size: &str, state: FileState<'_>) -> (Response, Option<FileAsked>) {
    attachment_with(ui, theme, keymap, name, size, state, None)
}

/// [`attachment`] drawn in `shown` whatever the pointer and the keyboard do (the gallery's boards).
pub(crate) fn attachment_with(
    ui: &mut Ui,
    theme: &Theme,
    keymap: &Keymap,
    name: &str,
    size: &str,
    state: FileState<'_>,
    shown: Option<Shown>,
) -> (Response, Option<FileAsked>) {
    let p = &theme.palette;
    let f = &theme.fonts;
    let [.., high, _] = theme.boxes().reader;
    let icon = theme.icon_side();
    let name_galley = super::fitted(ui, name, f.mono, p.ink, NAME);
    let size_galley = super::fitted(ui, size, f.mono_s, p.muted, NAME);
    let small = |text: &str| super::galley(ui, text, f.mono_s, p.muted).size().x.ceil();
    let (state_words, state_ink, check) = match state {
        FileState::Ready => (None, p.muted, false),
        FileState::Saving(how) => (Some(how), p.muted, false),
        FileState::Saved(how) => (Some(how), p.plus, true),
        FileState::Failed(how) => (Some(how), p.minus, false),
    };
    let plain = super::spoken(name);
    // Each link's words, what a screen reader hears, and whether it is built.
    let links: Vec<(&str, String, bool)> = match state {
        FileState::Ready => vec![(tr!("open"), tr!("Open {name}", name = plain), false), (tr!("save"), tr!("Save {name}", name = plain), true)],
        FileState::Saving(_) => Vec::new(),
        FileState::Saved(_) => vec![(tr!("open folder"), tr!("Open the folder of {name}", name = plain), false)],
        FileState::Failed(_) => vec![(tr!("try again"), tr!("Save {name} again", name = plain), true)],
    };
    let check_side = theme.mark_side();
    let words = state_words.map(|how| super::fitted(ui, how, f.mono_s, state_ink, NAME));
    let mark = if check { check_side + CHECK_GAP } else { 0.0 };
    let words_width = words.as_ref().map_or(0.0, |words| paint::run_box(words.size().x) + mark);
    let link_widths: Vec<f32> = links.iter().map(|(word, _, _)| small(word)).collect();
    let links_width = |count: usize| link_widths.iter().take(count).sum::<f32>() + LINK_GAP * count.saturating_sub(1) as f32;
    let tail = |words: f32, count: usize| match (words > 0.0 || state_words.is_some(), count) {
        (true, 0) => words,
        (true, _) => words + PAD + links_width(count),
        (false, _) => links_width(count),
    };
    let name_width = paint::run_box(name_galley.size().x);
    let size_width = paint::run_box(size_galley.size().x);
    let around = PAD + icon + PAD + PAD + PAD + PAD + PAD + PAD;
    let wanted = around + name_width + size_width + tail(words_width, links.len());
    let width = wanted.min(ui.available_width().max(1.0)).ceil();
    // A box narrower than what it holds: the name gives way first, then what the state says, then the links are left
    // out whole from the last, then the size; what still does not fit is clipped at the box's edge.
    let mut short = (wanted - width).max(0.0);
    let give = |had: f32, short: &mut f32| {
        let kept = (had - *short).max(0.0);
        *short -= had - kept;
        kept
    };
    let name_room = give(name_width, &mut short);
    let words_room = if words.is_some() { give((words_width - mark).max(0.0), &mut short) } else { 0.0 };
    let mut shown_links = links.len();
    while short > 0.0 && shown_links > 0 {
        let before = tail(words_room, shown_links);
        shown_links -= 1;
        short -= before - tail(words_room, shown_links);
    }
    let size_room = give(size_width, &mut short);
    let name_galley = if name_room < name_width { super::fitted(ui, name, f.mono, p.ink, name_room) } else { name_galley };
    let size_galley = if size_room < size_width { super::fitted(ui, size, f.mono_s, p.muted, size_room) } else { size_galley };
    let words = match (words, state_words) {
        (Some(_), Some(how)) if words_room + mark < words_width => Some(super::fitted(ui, how, f.mono_s, state_ink, words_room)),
        (words, _) => words,
    };
    let (rect, response) = ui.allocate_exact_size(vec2(width, high), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(Role::Button, true, spoken(name, size, state)));
    let lit = lit(ui, theme, &response, true, shown, Hover::AtOnce);
    let middle = rect.center().y;
    let top = |line: f32| (middle - line / 2.0).round();
    let icon_left = rect.left() + PAD;
    let name_left = icon_left + icon + PAD;
    let size_left = name_left + name_room.min(name_width) + PAD;
    let tail_left = size_left + size_room.min(size_width) + PAD + PAD + PAD;
    if ui.is_rect_visible(rect) {
        let edge = if matches!(state, FileState::Failed(_)) { p.minus } else { p.line };
        let fill = if lit.hover { p.stage } else { egui::Color32::TRANSPARENT };
        ui.painter().rect(rect, 0.0, fill, Stroke::new(1.0, edge), StrokeKind::Inside);
        if lit.focus {
            paint::focus_ring(ui.painter(), theme, rect, Ring::Outside);
        }
        // What it holds never paints past its edge.
        let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
        let icon_rect = Rect::from_min_size(pos2(icon_left, top(icon)), vec2(icon, icon));
        iconset::paint(&painter, icon_rect, IconId::File, if lit.hover { p.ink } else { p.muted });
        paint::run_at(&painter, pos2(name_left, top(name_galley.size().y)), name_galley, p.ink);
        paint::run_at(&painter, pos2(size_left, top(size_galley.size().y)), size_galley, p.muted);
        if let Some(words) = words.clone() {
            let mut left = tail_left;
            if check {
                let mark = Rect::from_min_size(pos2(left, top(check_side)), vec2(check_side, check_side));
                iconset::paint_glyph(&painter, mark, CHECK_GLYPH, p.plus);
                left += check_side + CHECK_GAP;
            }
            paint::run_at(&painter, pos2(left, top(words.size().y)), words, state_ink);
        }
    }
    let mut asked = None;
    // Enter on the box saves it (tries again after a failure); a press of the pointer saves only through its link.
    if response.clicked() && !response.clicked_by(egui::PointerButton::Primary) && matches!(state, FileState::Ready | FileState::Failed(_)) {
        asked = Some(FileAsked::Save);
    }
    let links_left = if words.is_some() { tail_left + words_room + mark + PAD } else { tail_left };
    let links_right = links_left + links_width(shown_links);
    if shown_links > 0 && links_right <= rect.right() - PAD + 0.5 {
        let row = Rect::from_min_max(pos2(links_left, rect.top()), pos2(links_right.max(links_left), rect.bottom()));
        let mut child = ui.new_child(UiBuilder::new().max_rect(row).layout(Layout::left_to_right(Align::Center)));
        child.set_clip_rect(rect.intersect(ui.clip_rect()));
        child.spacing_mut().item_spacing.x = LINK_GAP;
        for (word, whole, built) in links.into_iter().take(shown_links) {
            let pressed = if built {
                link_sized(&mut child, theme, keymap, word, LinkSize::Small)
            } else {
                soon(&mut child, theme, |ui| link_sized(ui, theme, keymap, word, LinkSize::Small))
            };
            pressed.widget_info(|| WidgetInfo::labeled(Role::Button, pressed.enabled(), &whole));
            if pressed.clicked() {
                asked = Some(FileAsked::Save);
            }
        }
    }
    (response, asked)
}

#[cfg(test)]
#[path = "attachment_tests.rs"]
mod tests;
