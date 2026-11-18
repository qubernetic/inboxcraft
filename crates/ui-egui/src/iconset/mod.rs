// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The icon painter: views ask for an [`IconId`]; [`glyph`] maps it to a glyph of Phosphor Regular, the one icon
//! family and weight ([`crate::theme::ICON`]), and [`paint`] paints it into a square: 16 px in controls, 14 px for a
//! mark in a row.
//!
//! Phosphor maps every id in an exhaustive `match`, so a new id without a glyph fails the build, and keeps its
//! [`Metrics`] (how large the glyph is drawn and how far it is moved to sit in the middle). The app icon is not from
//! Phosphor; it is the brand's mark, see [`crate::icons`].

mod phosphor;

use egui::{Color32, Painter, Rect, Vec2, pos2, vec2};

use crate::theme::{self, Theme};

/// What an icon means. Views name these, never a glyph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IconId {
    /// The inbox.
    Inbox,
    /// Done: a message dealt with, a step that passed.
    Done,
    /// Archive.
    Archive,
    /// Delete.
    Delete,
    /// Snooze.
    Snooze,
    /// Pin.
    Pin,
    /// Move to a folder.
    Move,
    /// Reply.
    Reply,
    /// Reply to all.
    ReplyAll,
    /// Forward.
    Forward,
    /// Send.
    Send,
    /// Search.
    Search,
    /// Write a new message.
    Compose,
    /// The calendar.
    Calendar,
    /// Attach a file.
    Attach,
    /// The settings.
    Settings,
    /// Notifications.
    Notify,
    /// People.
    People,
    /// A flag.
    Flag,
    /// More actions.
    More,
    /// A file: an attachment.
    File,
    /// Something that failed.
    Failed,
    /// Something still waiting.
    Waiting,
    /// Something skipped or not done.
    Skipped,
    /// The start view (home).
    Home,
    /// Mail: the app's mark on the rail.
    Mail,
    /// Add (an account).
    Add,
    /// Junk.
    Junk,
    /// Mark as read: an open envelope.
    MarkRead,
    /// Kept safe: a lock.
    Lock,
    /// The look: a palette.
    Palette,
    /// Accounts: an at sign.
    At,
    /// Triage: a funnel.
    Funnel,
    /// Shortcuts: a keyboard.
    Keyboard,
    /// Privacy: a shield with a tick.
    Shield,
    /// About: an i in a circle.
    Info,
    /// Close: a cross.
    Close,
    /// Back: an arrow to the left, before the name of the page it leads back to.
    Back,
    /// Caret: a chevron pointing down, after a choice that opens a menu.
    Caret,
    /// Newspaper: the Newsletters group of the grouped inbox.
    Newspaper,
    /// Previous: a chevron pointing left, a step back (the calendar's days, the mini month).
    Previous,
    /// Next: a chevron pointing right, a step on.
    Next,
    /// Globe: a time zone, the event's own beside the computer's.
    Globe,
    /// Changed: arrows round in a circle, what a newer invitation changed.
    Changed,
    /// Older: a clock turning back, an invitation a later one replaced.
    Older,
}

impl IconId {
    /// Every id, in the order of [`IconId::index`].
    pub const ALL: [Self; 45] = [
        Self::Inbox,
        Self::Done,
        Self::Archive,
        Self::Delete,
        Self::Snooze,
        Self::Pin,
        Self::Move,
        Self::Reply,
        Self::ReplyAll,
        Self::Forward,
        Self::Send,
        Self::Search,
        Self::Compose,
        Self::Calendar,
        Self::Attach,
        Self::Settings,
        Self::Notify,
        Self::People,
        Self::Flag,
        Self::More,
        Self::File,
        Self::Failed,
        Self::Waiting,
        Self::Skipped,
        Self::Home,
        Self::Mail,
        Self::Add,
        Self::Junk,
        Self::MarkRead,
        Self::Lock,
        Self::Palette,
        Self::At,
        Self::Funnel,
        Self::Keyboard,
        Self::Shield,
        Self::Info,
        Self::Close,
        Self::Back,
        Self::Caret,
        Self::Newspaper,
        Self::Previous,
        Self::Next,
        Self::Globe,
        Self::Changed,
        Self::Older,
    ];

    /// Its place in [`IconId::ALL`]; a test checks the two agree, so a new id must join the list.
    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Inbox => 0,
            Self::Done => 1,
            Self::Archive => 2,
            Self::Delete => 3,
            Self::Snooze => 4,
            Self::Pin => 5,
            Self::Move => 6,
            Self::Reply => 7,
            Self::ReplyAll => 8,
            Self::Forward => 9,
            Self::Send => 10,
            Self::Search => 11,
            Self::Compose => 12,
            Self::Calendar => 13,
            Self::Attach => 14,
            Self::Settings => 15,
            Self::Notify => 16,
            Self::People => 17,
            Self::Flag => 18,
            Self::More => 19,
            Self::File => 20,
            Self::Failed => 21,
            Self::Waiting => 22,
            Self::Skipped => 23,
            Self::Home => 24,
            Self::Mail => 25,
            Self::Add => 26,
            Self::Junk => 27,
            Self::MarkRead => 28,
            Self::Lock => 29,
            Self::Palette => 30,
            Self::At => 31,
            Self::Funnel => 32,
            Self::Keyboard => 33,
            Self::Shield => 34,
            Self::Info => 35,
            Self::Close => 36,
            Self::Back => 37,
            Self::Caret => 38,
            Self::Newspaper => 39,
            Self::Previous => 40,
            Self::Next => 41,
            Self::Globe => 42,
            Self::Changed => 43,
            Self::Older => 44,
        }
    }
}

/// How a glyph sits in the square it is painted into.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Metrics {
    /// The font size as a share of the square's side.
    pub size: f32,
    /// How far the glyph is moved down, as a share of the square's side, so its middle is the square's middle.
    pub drop: f32,
}

/// The glyph of `id` in Phosphor Regular.
#[must_use]
pub fn glyph(id: IconId) -> char {
    phosphor::glyph(id)
}

/// Paints `id` in `colour` into the middle of `rect`, as large as its shorter side allows. An empty or unreal `rect`
/// paints nothing.
pub fn paint(painter: &Painter, rect: Rect, id: IconId, colour: Color32) {
    paint_glyph(painter, rect, glyph(id), colour);
}

/// Paints the Phosphor Regular `glyph` as [`paint`] paints an icon: a glyph a component's sheet names that is not one
/// of the [`IconId`]s (a field's lock, the password mark, a warning).
pub(crate) fn paint_glyph(painter: &Painter, rect: Rect, glyph: char, colour: Color32) {
    let side = rect.width().min(rect.height());
    if !side.is_finite() || side <= 0.0 || !rect.center().x.is_finite() || !rect.center().y.is_finite() {
        return;
    }
    let metrics = phosphor::METRICS;
    let galley = painter.layout_no_wrap(glyph.to_string(), theme::icon_font(side * metrics.size), colour);
    let at = rect.center() + vec2(0.0, side * metrics.drop) - galley.size() / 2.0;
    painter.galley(at, galley, colour);
}

/// The side of an icon on the [`sheet`], in points.
const SHEET_ICON: f32 = 24.0;

/// The side of a cell of the [`sheet`] and its margin, in points.
const SHEET_CELL: f32 = 40.0;

/// The rows of the [`sheet`]: what an icon is painted in.
#[derive(Clone, Copy)]
enum Ink {
    /// The text colour.
    Text,
    /// The accent.
    Accent,
    /// The muted colour.
    Muted,
}

/// The rows of the [`sheet`]: the side an icon is painted at and its ink (24 px in the text colour, 16 px as in a
/// control in the accent, 14 px as a mark in a row in the muted colour).
const SHEET_ROWS: [(f32, Ink); 3] = [(SHEET_ICON, Ink::Text), (16.0, Ink::Accent), (14.0, Ink::Muted)];

/// The size of the [`sheet`] in points.
#[must_use]
pub fn sheet_size() -> Vec2 {
    let columns = IconId::ALL.len() as f32;
    let rows = SHEET_ROWS.len() as f32;
    vec2(columns * SHEET_CELL + SHEET_CELL, rows * SHEET_CELL + SHEET_CELL)
}

/// Every icon on `theme`'s panel colour, for `ui_shot`: a row a size and ink.
pub fn sheet(painter: &Painter, theme: &Theme) {
    let p = &theme.palette;
    let size = sheet_size();
    painter.rect_filled(Rect::from_min_size(pos2(0.0, 0.0), size), 0.0, p.panel);
    let margin = SHEET_CELL / 2.0;
    for (row, (side, ink)) in SHEET_ROWS.into_iter().enumerate() {
        let colour = match ink {
            Ink::Text => p.ink,
            Ink::Accent => p.cyan_text,
            Ink::Muted => p.muted,
        };
        for (column, id) in IconId::ALL.into_iter().enumerate() {
            let centre = pos2(margin + (column as f32 + 0.5) * SHEET_CELL, margin + (row as f32 + 0.5) * SHEET_CELL);
            paint(painter, Rect::from_center_size(centre, vec2(side, side)), id, colour);
        }
    }
}

#[cfg(test)]
mod tests;
