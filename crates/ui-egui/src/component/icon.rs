// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! An icon on its own: one of the 45 glyphs of Phosphor Regular ([`crate::iconset`]), 16 px at 100 % text, in
//! `muted`, never in a tile. A control that holds an icon paints it in the control's own colour.

use egui::{Response, Role, Sense, Ui, WidgetInfo, vec2};

use crate::iconset::{self, IconId};
use crate::theme::Theme;

/// The icon `id` alone, 16 px square at 100 % text, in `muted`; a screen reader reads `name`.
pub fn icon(ui: &mut Ui, theme: &Theme, id: IconId, name: &str) -> Response {
    let side = theme.icon_side();
    let (rect, response) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Image, true, name));
    if ui.is_rect_visible(rect) {
        iconset::paint(ui.painter(), rect, id, theme.palette.muted);
    }
    response
}

/// The name the design gives `id` (the icon sheet's state), a word or two joined by hyphens: `reply-all`.
#[must_use]
pub const fn name(id: IconId) -> &'static str {
    match id {
        IconId::Inbox => "inbox",
        IconId::Done => "done",
        IconId::Archive => "archive",
        IconId::Delete => "delete",
        IconId::Snooze => "snooze",
        IconId::Pin => "pin",
        IconId::Move => "move",
        IconId::Reply => "reply",
        IconId::ReplyAll => "reply-all",
        IconId::Forward => "forward",
        IconId::Send => "send",
        IconId::Search => "search",
        IconId::Compose => "compose",
        IconId::Calendar => "calendar",
        IconId::Attach => "attach",
        IconId::Settings => "settings",
        IconId::Notify => "notify",
        IconId::People => "people",
        IconId::Flag => "flag",
        IconId::More => "more",
        IconId::File => "file",
        IconId::Failed => "failed",
        IconId::Waiting => "waiting",
        IconId::Skipped => "skipped",
        IconId::Home => "home",
        IconId::Mail => "mail",
        IconId::Add => "add",
        IconId::Junk => "junk",
        IconId::MarkRead => "mark-read",
        IconId::Lock => "lock",
        IconId::Palette => "palette",
        IconId::At => "at",
        IconId::Funnel => "funnel",
        IconId::Keyboard => "keyboard",
        IconId::Shield => "shield",
        IconId::Info => "info",
        IconId::Close => "close",
        IconId::Back => "back",
        IconId::Caret => "caret",
        IconId::Newspaper => "newspaper",
        IconId::Previous => "previous",
        IconId::Next => "next",
        IconId::Globe => "globe",
        IconId::Changed => "changed",
        IconId::Older => "older",
        IconId::Dock => "dock",
    }
}

#[cfg(test)]
#[path = "icon_tests.rs"]
mod tests;
