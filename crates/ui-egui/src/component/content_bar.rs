// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The reader's content bar: what kind of message it is (TEXT, HTML, LIST, INVITE) in an accent tag, for a message that
//! is not plain text the choice between `fitted` and `plain text`, what was held back in `mono_s` (`muted`, or `warn`
//! while pictures wait), the small links that answer it, and `open in browser ↗` at the right end; on the `panel` fill
//! in a 1 px `panel_line` edge, 12 left and right and between its parts, 40 high. For plain text the words follow the
//! tag; otherwise they stand at the right, before the links. The words and the links' words are data where they name
//! an address: plain words, cut between grapheme clusters with one ellipsis; a screen reader hears them whole. A link
//! gives back the id the caller gave it. The browser's cleaned copy is not built yet, and the choice of view waits for
//! the fitted view: what is not built stands unavailable, with the `coming soon` tag once, 12 after the choice of view,
//! where the words leave it room. The words keep the places the sheet gives them: the browser's link gives way first in
//! a narrow bar, then the tag; the words are cut last. Nothing in it moves.

use egui::{Align, Layout, Rect, Role, Sense, Stroke, StrokeKind, Ui, UiBuilder, WidgetInfo, pos2, vec2};

use crate::draw::paint;
use crate::keymap::Keymap;
use crate::theme::Theme;
use crate::tr;

use super::banner::{BannerLink, cut_name};
use super::button::{LinkSize, link_sized};
use super::segmented::segmented;
use super::soon::{soon, soon_tag};
use super::tag::{TagKind, tag};

/// What kind of message the bar names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarKind {
    /// Plain text, shown as sent: no choice of view, no browser.
    Text,
    /// An HTML message.
    Html,
    /// A message of a mailing list.
    List,
    /// An invitation, read into its card.
    Invite,
}

/// How an HTML message is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// Set in the app's type and colours.
    Fitted,
    /// As a text-only program shows it.
    Plain,
}

/// What a content bar shows.
#[derive(Debug, Clone, Copy)]
pub struct ContentBar<'a> {
    /// The kind of message.
    pub kind: BarKind,
    /// The view chosen; not drawn for [`BarKind::Text`].
    pub view: View,
    /// Whether the choice of view works; otherwise it stands unavailable.
    pub views: bool,
    /// What was held back, or what the message is.
    pub held: &'a str,
    /// Whether what was held back waits for the owner (`warn`).
    pub waiting: bool,
    /// The links that answer it, in order.
    pub links: &'a [BannerLink<'a>],
}

/// What was pressed on a content bar.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BarAsked {
    /// The view chosen.
    pub view: Option<View>,
    /// The id of the link pressed.
    pub link: Option<String>,
}

/// The room left and right of the bar's parts and between them, in points.
const PAD: f32 = 12.0;

/// The room between two links, in points.
const LINK_GAP: f32 = 10.0;

/// The least room the words keep before the browser's link gives way, in points.
const LEAST_WORDS: f32 = 48.0;

/// The tag's word for `kind`, as the sheet writes it.
fn word(kind: BarKind) -> &'static str {
    match kind {
        BarKind::Text => tr!("TEXT"),
        BarKind::Html => "HTML",
        BarKind::List => tr!("LIST"),
        BarKind::Invite => tr!("INVITE"),
    }
}

/// What a screen reader hears for a content bar of `kind` saying `held`.
pub(crate) fn spoken(kind: BarKind, held: &str) -> String {
    tr!("Content: {kind}, {held}", kind = word(kind), held = super::spoken(held))
}

/// The content bar across `ui`'s width; what was pressed on it.
pub fn content_bar(ui: &mut Ui, theme: &Theme, keymap: &Keymap, bar: &ContentBar<'_>) -> BarAsked {
    let p = &theme.palette;
    let f = &theme.fonts;
    let width = ui.available_width().max(1.0).floor();
    let [_, high, ..] = theme.boxes().reader;
    let (_, rect) = ui.allocate_space(vec2(width, high));
    let id = ui.scope_id().with("content-bar");
    let response = ui.interact(rect, id, Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Group, true, spoken(bar.kind, bar.held)));
    if ui.is_rect_visible(rect) {
        ui.painter().rect(rect, 0.0, p.panel, Stroke::new(1.0, p.panel_line), StrokeKind::Inside);
    }
    let inner = Rect::from_min_max(pos2(rect.left() + PAD, rect.top()), pos2(rect.right() - PAD, rect.bottom()));
    let mut asked = BarAsked::default();
    let mut child = ui.new_child(UiBuilder::new().max_rect(inner).layout(Layout::left_to_right(Align::Center)));
    child.spacing_mut().item_spacing.x = PAD;
    tag(&mut child, theme, word(bar.kind), TagKind::Accent);
    let plain = bar.kind == BarKind::Text;
    if !plain {
        let options = [(View::Fitted, tr!("fitted"), f.button), (View::Plain, tr!("plain text"), f.button)];
        let chosen = if bar.views {
            segmented(&mut child, theme, &options, bar.view)
        } else {
            soon(&mut child, theme, |ui| segmented(ui, theme, &options, bar.view))
        };
        asked.view = chosen.filter(|view| *view != bar.view);
    }
    // The right end, measured first: the browser's link; then the links before it. The words take what is left,
    // right-aligned before them, where the sheet puts them.
    let small = |text: &str| super::galley(ui, text, f.mono_s, p.muted).size().x.ceil();
    let browser = tr!("open in browser ↗");
    let links: Vec<(String, &BannerLink<'_>)> = bar.links.iter().map(|link| (super::spoken(link.name), link)).collect();
    let links_width = links.iter().map(|(name, _)| small(name)).sum::<f32>() + LINK_GAP * links.len().saturating_sub(1) as f32;
    let links_room = if links.is_empty() { 0.0 } else { links_width + PAD };
    let words_left = child.cursor().left();
    // The browser's link (not built) gives way first in a narrow bar, so nothing lies over another.
    let wanted = PAD + small(browser);
    let browsing = !plain && inner.right() - wanted - links_room >= words_left + LEAST_WORDS;
    let end = if browsing { wanted } else { 0.0 };
    let words_right = (inner.right() - end - links_room).max(words_left + 1.0);
    let room = words_right - words_left;
    let ink = if bar.waiting { p.warn } else { p.muted };
    let words = super::fitted(ui, bar.held, f.mono_s, ink, room);
    let top = (rect.center().y - words.size().y / 2.0).round();
    if ui.is_rect_visible(rect) {
        if plain {
            paint::run_at(ui.painter(), pos2(words_left, top), words.clone(), ink);
        } else {
            paint::run_right(ui.painter(), words_right, top, words.clone(), ink);
        }
    }
    // The coming-soon tag once for what is not built (the choice of view, the browser's copy): 12 after the choice,
    // in the room the right-aligned words leave; where they leave none it gives way, never the words.
    let tag_width = super::tag::width(ui, theme, tr!("coming soon"));
    let words_start = words_right - paint::run_box(words.size().x);
    if !plain && words_left + tag_width + PAD <= words_start {
        let place = Rect::from_min_max(pos2(words_left, rect.top()), pos2(words_left + tag_width, rect.bottom()));
        let mut tag_ui = ui.new_child(UiBuilder::new().max_rect(place).layout(Layout::left_to_right(Align::Center)));
        soon(&mut tag_ui, theme, |ui| soon_tag(ui, theme));
    }
    let after = Rect::from_min_max(pos2(words_right + PAD, rect.top()), pos2(inner.right().max(words_right + PAD), rect.bottom()));
    let mut right = ui.new_child(UiBuilder::new().max_rect(after).layout(Layout::left_to_right(Align::Center)));
    right.spacing_mut().item_spacing.x = LINK_GAP;
    let mut left = (inner.right() - end - words_right - PAD).max(0.0);
    for (whole, link) in &links {
        let name = cut_name(ui, theme, whole, left);
        if name.is_empty() {
            break;
        }
        let pressed = link_sized(&mut right, theme, keymap, &name, LinkSize::Small);
        pressed.widget_info(|| WidgetInfo::labeled(Role::Button, pressed.enabled(), whole));
        left -= pressed.rect.width() + LINK_GAP;
        if pressed.clicked() {
            asked.link = Some(link.id.to_owned());
        }
    }
    if browsing {
        let end_rect = Rect::from_min_max(pos2(inner.right() - end + PAD, rect.top()), pos2(inner.right(), rect.bottom()));
        let mut tail = ui.new_child(UiBuilder::new().max_rect(end_rect).layout(Layout::left_to_right(Align::Center)));
        soon(&mut tail, theme, |ui| link_sized(ui, theme, keymap, browser, LinkSize::Small));
    }
    asked
}

#[cfg(test)]
#[path = "content_bar_tests.rs"]
mod tests;
