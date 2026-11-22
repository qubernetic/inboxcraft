// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The group head of the inbox (`group-head`): 440 × 32 on the `panel`, a [`super::row`] on the panel ground, its line
//! under it. 16 in from the left the group's index (`[ B ]`) in `label` and `cyan_text`, 8 after it its name in
//! `label` (capitals) and `muted`, `ink` while hovered; at the right, 16 in, the unread count in `mono_s` and `muted`,
//! or "nothing to read" when none is unread. Before the count, 8 apart, what a state adds: how many of the group's rows
//! show ("3 of 6 ·", `mono_s`, `muted`) and a small link ("show all", "show fewer"), or, hovered or with the keyboard
//! on it, "done all" (the sheet's focus state draws none; a keyed head shows it as a keyed mail row shows its hover
//! actions, so `Tab` reaches it). The name is cut with one `…` short of them. The focus edge is the row's, 2 px inside.

use egui::{Id, Rect, Role, Ui, UiBuilder, WidgetInfo, pos2, vec2};

use crate::draw::paint;
use crate::keymap::Keymap;
use crate::theme::Theme;

use super::Shown;
use super::button::{LinkSize, link_with};
use super::row::{RowGround, row_with};

/// The head's height at 100 % text, in points.
pub const HEIGHT: f32 = 32.0;

/// The room left and right of the head's content, and between its parts, in points.
const PAD: f32 = 16.0;
const GAP: f32 = 8.0;

/// What a group head shows.
#[derive(Debug, Clone, Copy)]
pub struct GroupHead<'a> {
    /// Its id: its focus and its link's.
    pub id: Id,
    /// The group's index, `[ A ]`.
    pub index: &'a str,
    /// The group's name, as written (drawn in capitals).
    pub name: &'a str,
    /// Its unread messages.
    pub unread: u64,
    /// The messages it holds, counted at its right instead of the unread ones where a place counts what it holds
    /// (Later's heads, `later/snoozed`).
    pub held: Option<u64>,
    /// How many of its rows show, "3 of 6 ·", when it shows some of them.
    pub shown: Option<&'a str>,
    /// The small link that stays ("show all", "show fewer").
    pub link: Option<&'a str>,
    /// The link that shows while it is hovered or keyed ("done all").
    pub hover_link: Option<&'a str>,
}

/// What a head did this frame.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Pressed {
    /// The link that stays was pressed.
    pub link: bool,
    /// The hover link was pressed.
    pub hover_link: bool,
}

/// What a screen reader hears for the head of `name` with `unread` messages.
#[must_use]
pub fn spoken(name: &str, unread: u64) -> String {
    let name = super::spoken(name);
    if unread == 0 { crate::tr!("{name}, nothing to read", name = name) } else { crate::tr!("{name}, {count} unread", name = name, count = unread) }
}

/// The head `head` across `ui`'s width; `shown` draws it hovered or focused whatever the pointer does (the gallery's
/// board). What was pressed.
pub fn group_head(ui: &mut Ui, theme: &Theme, keymap: &Keymap, head: &GroupHead<'_>, shown: Option<Shown>) -> Pressed {
    let p = &theme.palette;
    let f = &theme.fonts;
    // The panel the head stands on, under the row's hover fill.
    let room = ui.available_rect_before_wrap();
    let ground = Rect::from_min_size(room.min, vec2(room.width().max(0.0), Theme::size(theme, HEIGHT)));
    if ui.is_rect_visible(ground) {
        ui.painter().rect_filled(ground, 0.0, p.panel);
    }
    // Taken before the row asks for the focus: in the frame `Tab` leaves the head it has given the focus away, and its
    // link must be there to take it.
    let had_focus = ui.memory(|memory| memory.has_focus(head.id));
    let row = row_with(ui, theme, head.id, HEIGHT, RowGround::Panel, false, false, shown);
    let spoken = match head.held {
        Some(1) => crate::tr!("{name}, 1 message", name = super::spoken(head.name)),
        Some(held) => crate::tr!("{name}, {count} messages", name = super::spoken(head.name), count = held),
        None => spoken(head.name, head.unread),
    };
    row.response.widget_info(|| WidgetInfo::labeled(Role::Heading, true, &spoken));
    let rect = row.rect;
    if !ui.is_rect_visible(rect) {
        return Pressed::default();
    }
    // Where the hover link's own id is kept: while it has the focus, it stays.
    let kept = head.id.with("hover-link-id");
    let link_focused = ui.data(|data| data.get_temp::<Id>(kept)).is_some_and(|id| ui.memory(|memory| memory.has_focus(id)));
    let lit = row.hovered || row.focused || had_focus || link_focused;
    let line = f.label.line;
    let top = rect.top() + ((rect.height() - line) / 2.0).round();
    let mut right = rect.right() - PAD;
    // The count, or "nothing to read".
    let count = count_words(head);
    let galley = super::galley(ui, &count, f.mono_s, p.muted);
    right = paint::run_right(ui.painter(), right, top + ((line - f.mono_s.line) / 2.0).round(), galley, p.muted);
    let mut pressed = Pressed::default();
    let links = [(head.link, false), (head.hover_link.filter(|_| lit), true)];
    for (words, hover) in links {
        let Some(words) = words else { continue };
        let wide = super::galley(ui, words, f.mono_s, p.cyan_text).size().x.ceil();
        right -= GAP + wide;
        let high = theme.boxes().row_link;
        let at = Rect::from_min_size(pos2(right, rect.top() + ((rect.height() - high) / 2.0).round()), vec2(wide, high));
        let clicked = ui
            .scope_builder(UiBuilder::new().max_rect(at).id_salt(if hover { "hover-link" } else { "link" }), |ui| {
                let response = link_with(ui, theme, keymap, words, LinkSize::Small, true, None);
                if hover {
                    ui.data_mut(|data| data.insert_temp(kept, response.id));
                    if response.gained_focus() {
                        response.scroll_to_me(None);
                    }
                }
                response.clicked()
            })
            .inner;
        if hover {
            pressed.hover_link = clicked;
        } else {
            pressed.link = clicked;
        }
    }
    if let Some(words) = head.shown {
        let galley = super::galley(ui, words, f.mono_s, p.muted);
        right -= GAP + paint::run_box(galley.size().x);
        paint::run_at(ui.painter(), pos2(right, top + ((line - f.mono_s.line) / 2.0).round()), galley, p.muted);
    }
    let left = rect.left() + PAD;
    let index = super::galley(ui, head.index, f.label, p.cyan_text);
    let index_wide = paint::run_box(index.size().x);
    paint::run_at(ui.painter(), pos2(left, top), index, p.cyan_text);
    let name_left = left + index_wide + GAP;
    let ink = if row.hovered { p.ink } else { p.muted };
    let name = super::fitted(ui, head.name, f.label, ink, (right - GAP - name_left).max(0.0));
    paint::run_at(ui.painter(), pos2(name_left, top), name, ink);
    row.finish(ui, theme);
    pressed
}

/// What `head` counts at its right: the messages it holds where it says, else the unread ones or "nothing to read".
fn count_words(head: &GroupHead<'_>) -> String {
    match head.held {
        Some(held) => held.to_string(),
        None if head.unread == 0 => crate::tr!("nothing to read").to_owned(),
        None => head.unread.to_string(),
    }
}

/// The narrowest `head` stands whole in, its hover link shown: its pads, its index and name, how many show, its links
/// and its count with the gaps between them, as [`group_head`] lays them out.
#[must_use]
pub fn least_width(ui: &Ui, theme: &Theme, head: &GroupHead<'_>) -> f32 {
    let p = &theme.palette;
    let f = &theme.fonts;
    let wide = |text: &str, font| super::galley(ui, text, font, p.muted).size().x;
    let count = count_words(head);
    let parts = [head.link, head.hover_link].into_iter().flatten().map(|words| GAP + wide(words, f.mono_s).ceil()).sum::<f32>()
        + head.shown.map_or(0.0, |words| GAP + paint::run_box(wide(words, f.mono_s)));
    (PAD * 2.0
        + paint::run_box(wide(head.index, f.label))
        + GAP
        + paint::measured(&super::galley(ui, head.name, f.label, p.muted)).max(wide(head.name, f.label))
        + GAP
        + parts
        + paint::run_box(wide(&count, f.mono_s)))
    .ceil()
}

#[cfg(test)]
#[path = "group_head_tests.rs"]
mod tests;
