// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The facts of a message in the reader: FROM, TO and DATE, a label in capitals in a 72 wide column and the value 12
//! right of it, 22 high, 6 apart; who wrote and to whom in `body` and `ink`, the date in mono and `muted`. The small
//! link `details ▾` at the right end of the FROM line opens the rest: CC, REPLY TO, CHECKED (what the sender's domain
//! proved) and the message's ID, each shown only when the message has it; `hide details ▴` closes them. With more
//! recipients than the line shows, the TO line ends with how many there are. Every value is data: drawn as plain words
//! on one line, cut with one ellipsis. Nothing moves: the rest opens and closes at once.

use egui::{Align, Layout, Rect, Response, Role, Sense, Ui, UiBuilder, WidgetInfo, pos2, vec2};

use crate::keymap::Keymap;
use crate::theme::{Font, Theme};
use crate::tr;

use super::button::{Caret, link_caret, link_caret_width};

/// What the facts show of a message.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Facts<'a> {
    /// Who wrote it: the name and the address.
    pub from: &'a str,
    /// To whom.
    pub to: &'a str,
    /// When, as the app writes dates.
    pub date: &'a str,
    /// Copied to; empty when no one was.
    pub cc: &'a str,
    /// Where answers go when that is not the sender; empty otherwise.
    pub reply_to: &'a str,
    /// What the sender's domain proved (SPF, DKIM); empty when unknown.
    pub checked: &'a str,
    /// The message's ID; empty when it has none.
    pub id: &'a str,
    /// How many recipients there are, shown at the end of the TO line when it is more than the line holds.
    pub recipients: Option<usize>,
}

/// The width of the labels' column, in points.
const LABEL: f32 = 72.0;

/// The room between the labels' column and the values, and between a value and what ends its line, in points.
const GAP: f32 = 12.0;

/// The room between two lines, in points.
const LINE_GAP: f32 = 6.0;

/// FROM, TO and DATE of a message, `open` or not; the response of the link that opens and closes the rest.
pub fn facts(ui: &mut Ui, theme: &Theme, keymap: &Keymap, from: &str, to: &str, date: &str, open: bool) -> Response {
    facts_of(ui, theme, keymap, &Facts { from, to, date, ..Facts::default() }, open)
}

/// What a screen reader hears for the facts.
pub(crate) fn spoken(facts: &Facts<'_>) -> String {
    tr!("From {from}, to {to}, {date}", from = super::plain(facts.from), to = super::plain(facts.to), date = super::plain(facts.date))
}

/// [`facts`] of all that `shown` holds; the response of the link that opens and closes the rest.
pub fn facts_of(ui: &mut Ui, theme: &Theme, keymap: &Keymap, shown: &Facts<'_>, open: bool) -> Response {
    let p = &theme.palette;
    let f = &theme.fonts;
    let line = f.body.line;
    let mut rows: Vec<(&str, &str, Font)> = vec![(tr!("From"), shown.from, f.body), (tr!("To"), shown.to, f.body)];
    if open {
        for (label, value, font) in [
            (tr!("Cc"), shown.cc, f.body),
            (tr!("Reply to"), shown.reply_to, f.body),
            (tr!("Checked"), shown.checked, f.mono),
            (tr!("Id"), shown.id, f.mono),
        ] {
            if !value.is_empty() {
                rows.push((label, value, font));
            }
        }
    }
    rows.push((tr!("Date"), shown.date, f.mono));
    let width = ui.available_width().max(LABEL + GAP + 1.0);
    let count = rows.len() as f32;
    let high = count * line + (count - 1.0) * LINE_GAP;
    let (_, rect) = ui.allocate_space(vec2(width, high));
    let response = ui.interact(rect, ui.scope_id().with("facts"), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Group, true, spoken(shown)));
    let (link_words, caret) = if open { (tr!("hide details"), Caret::Up) } else { (tr!("details"), Caret::Down) };
    let link_width = link_caret_width(ui, theme, link_words);
    let recipients = shown.recipients.filter(|count| *count > 1).map(|count| tr!("{count} in all", count = count));
    let count_galley = recipients.as_deref().map(|count| super::galley(ui, count, f.mono_s, p.muted));
    let value_left = rect.left() + LABEL + GAP;
    let small_top = (line - f.mono_s.line) / 2.0;
    for (index, (label, value, font)) in rows.iter().enumerate() {
        let top = rect.top() + index as f32 * (line + LINE_GAP);
        let row = Rect::from_min_size(pos2(rect.left(), top), vec2(width, line));
        if !ui.is_rect_visible(row) {
            continue;
        }
        // What ends the line: the link on FROM, the count on TO.
        let end = match index {
            0 => link_width + GAP,
            1 => count_galley.as_ref().map_or(0.0, |count| count.size().x.ceil() + GAP),
            _ => 0.0,
        };
        let ink = if *font == f.body { p.ink } else { p.muted };
        let label = super::galley(ui, label, f.label, p.muted);
        let value = super::fitted(ui, value, *font, ink, (row.right() - end - value_left).max(1.0));
        let painter = ui.painter();
        painter.galley(pos2(row.left(), (top + small_top).round()), label, p.muted);
        painter.galley(pos2(value_left, top), value, ink);
        if index == 1
            && let Some(count) = count_galley.clone()
        {
            let left = row.right() - count.size().x.ceil();
            painter.galley(pos2(left, (top + small_top).round()), count, p.muted);
        }
    }
    let link_rect = Rect::from_min_size(pos2(rect.right() - link_width, (rect.top() + small_top).round()), vec2(link_width, theme.boxes().row_link));
    let mut child = ui.new_child(UiBuilder::new().max_rect(link_rect).layout(Layout::left_to_right(Align::Center)));
    link_caret(&mut child, theme, keymap, link_words, caret)
}

#[cfg(test)]
#[path = "facts_tests.rs"]
mod tests;
