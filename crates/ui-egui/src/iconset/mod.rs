// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The icon painter: views ask for an [`IconId`] in a [`Style`]; the [`IconSet`] the theme names maps it to glyphs of
//! its font and paints them into a square.
//!
//! Each set maps every id in an exhaustive `match`, so a new id without a glyph fails the build, and keeps its own
//! [`Metrics`] (how large the glyph is drawn and how far it is moved to sit in the middle). The styles carry meaning:
//! [`Style::Regular`] in toolbars and rows, [`Style::Fill`] for active and selected states, [`Style::Duotone`] for
//! empty states. Phosphor has a font for each style; Lucide has lines only: its Fill is the line glyph, and Duotone
//! draws it over a faint filled disc ([`Glyph::disc`]). The app icon is not from a set; it is drawn in code in [`crate::icons`].

mod lucide;
mod phosphor;

use egui::{Color32, Painter, Rect, Vec2, pos2, vec2};
use inboxcraft_engine::appearance::Icons;

use crate::theme::{IconFont, Theme};

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
}

impl IconId {
    /// Every id, in the order of [`IconId::index`].
    pub const ALL: [Self; 38] = [
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
        }
    }
}

/// How an icon is drawn, by what it says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Style {
    /// Lines: toolbars and rows.
    Regular,
    /// Solid: active and selected states (a pinned pin, a flagged message).
    Fill,
    /// A faint filled back under the lines: empty states.
    Duotone,
}

/// The opacity of a duotone icon's back.
pub const BACK_OPACITY: f32 = 0.2;

/// What a set draws for an id in a style: glyphs of one font, painted one over the other, each at an opacity, over a
/// filled disc where the set has no solid glyphs.
#[derive(Debug, Clone, PartialEq)]
pub struct Glyph {
    /// The font the glyphs are in.
    pub face: IconFont,
    /// The glyphs from back to front, each with its opacity (0 to 1).
    pub layers: Vec<(char, f32)>,
    /// The opacity of a filled disc behind the glyphs (0 to 1); 0 draws none.
    pub disc: f32,
}

/// The diameter of a [`Glyph::disc`] as a share of the square's side.
pub const DISC: f32 = 0.9;

/// How a set's glyphs sit in the square they are painted into.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Metrics {
    /// The font size as a share of the square's side.
    pub size: f32,
    /// How far the glyph is moved down, as a share of the square's side, so its middle is the square's middle.
    pub drop: f32,
}

/// An icon set: a font and the mapping of every [`IconId`] onto it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum IconSet {
    /// Phosphor (MIT), the default.
    #[default]
    Phosphor,
    /// Lucide (ISC).
    Lucide,
}

impl IconSet {
    /// Every set.
    pub const ALL: [Self; 2] = [Self::Phosphor, Self::Lucide];

    /// The set the owner chose.
    #[must_use]
    pub const fn of(choice: Icons) -> Self {
        match choice {
            Icons::Phosphor => Self::Phosphor,
            Icons::Lucide => Self::Lucide,
        }
    }

    /// The glyphs of `id` in `style`.
    #[must_use]
    pub fn glyph(self, id: IconId, style: Style) -> Glyph {
        match self {
            Self::Phosphor => phosphor::glyph(id, style),
            Self::Lucide => lucide::glyph(id, style),
        }
    }

    /// How this set's glyphs sit in their square.
    #[must_use]
    pub const fn metrics(self) -> Metrics {
        match self {
            Self::Phosphor => phosphor::METRICS,
            Self::Lucide => lucide::METRICS,
        }
    }

    /// Paints `id` in `style` and `colour` into the middle of `rect`, as large as its shorter side allows. An empty or
    /// unreal `rect` paints nothing.
    pub fn paint(self, painter: &Painter, rect: Rect, id: IconId, style: Style, colour: Color32) {
        let side = rect.width().min(rect.height());
        if !side.is_finite() || side <= 0.0 || !rect.center().x.is_finite() || !rect.center().y.is_finite() {
            return;
        }
        let metrics = self.metrics();
        let glyph = self.glyph(id, style);
        if glyph.disc > 0.0 {
            painter.circle_filled(rect.center(), side * DISC / 2.0, colour.gamma_multiply(glyph.disc));
        }
        let font = glyph.face.id(side * metrics.size);
        for (layer, opacity) in glyph.layers {
            let colour = colour.gamma_multiply(opacity);
            let galley = painter.layout_no_wrap(layer.to_string(), font.clone(), colour);
            let at = rect.center() + vec2(0.0, side * metrics.drop) - galley.size() / 2.0;
            painter.galley(at, galley, colour);
        }
    }
}

/// The side of an icon on the [`sheet`], in points.
const SHEET_ICON: f32 = 24.0;

/// The side of a cell of the [`sheet`] and its margin, in points.
const SHEET_CELL: f32 = 40.0;

/// The rows of the [`sheet`]: a style and the colour token it is shown in.
const SHEET_ROWS: [Style; 3] = [Style::Regular, Style::Fill, Style::Duotone];

/// The size of the [`sheet`] in points.
#[must_use]
pub fn sheet_size() -> Vec2 {
    let columns = IconId::ALL.len() as f32;
    let rows = SHEET_ROWS.len() as f32 + 1.0;
    vec2(columns * SHEET_CELL + SHEET_CELL, rows * SHEET_CELL + SHEET_CELL)
}

/// Every icon of `theme`'s set on its panel colour, for `ui_shot`: a row a style (Regular in the text colour, Fill
/// and Duotone in the accent) and Regular at the size of a line of body text in the muted colour.
pub fn sheet(painter: &Painter, theme: &Theme) {
    let p = &theme.palette;
    let size = sheet_size();
    painter.rect_filled(Rect::from_min_size(pos2(0.0, 0.0), size), 0.0, p.panel);
    let margin = SHEET_CELL / 2.0;
    let rows = SHEET_ROWS.iter().map(|style| (*style, SHEET_ICON, if *style == Style::Regular { p.text } else { p.accent }));
    let small = std::iter::once((Style::Regular, theme.fonts.body.size, p.muted));
    for (row, (style, side, colour)) in rows.chain(small).enumerate() {
        for (column, id) in IconId::ALL.into_iter().enumerate() {
            let centre = pos2(margin + (column as f32 + 0.5) * SHEET_CELL, margin + (row as f32 + 0.5) * SHEET_CELL);
            theme.icons.paint(painter, Rect::from_center_size(centre, vec2(side, side)), id, style, colour);
        }
    }
}

#[cfg(test)]
mod tests;
