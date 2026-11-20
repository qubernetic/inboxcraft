// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The buttons of the design, each drawn as its sheet says in every state, square and at full strength:
//!
//! - [`IconButton`]: a 16 px glyph in a 32 × 32 square, no box. `muted` at rest; hovered the `stage` fill and an `ink`
//!   glyph; pressed a `muted` edge as well; selected (pinned, flagged) the accent wash and the accent glyph; disabled
//!   or unavailable only the glyph in `dim`, the tooltip saying why. On the navigation rail it is drawn in one ink.
//! - [`TextButton`]: words in mono 13, 32 high, 12 beside the text, in one of four [`Tone`]s.
//! - [`link`] and [`link_sized`]: the accent as text, underlined under the pointer.
//!
//! Disabled is never a fade: a box gets a dashed `line` edge and `muted` text, a glyph `dim`.

use egui::{Color32, Response, Role, Sense, Stroke, Ui, WidgetInfo, vec2};

use crate::draw::paint::{self, Ring};
use crate::iconset::IconId;
use crate::keymap::Keymap;
use crate::theme::Theme;

use super::key::{Chip, ChipState};
use super::{Face, Hover, Shown, between, control_side, lit, soon, tooltip};

/// An icon button.
#[derive(Debug, Clone, Copy)]
pub struct IconButton<'a> {
    icon: IconId,
    name: &'a str,
    action: Option<&'a str>,
    active: bool,
    enabled: bool,
    note: Option<&'a str>,
    ink: Option<Color32>,
    side: Option<f32>,
    shown: Option<Shown>,
}

impl<'a> IconButton<'a> {
    /// A button showing `icon`, called `name` (its accessible name and the text of its tooltip).
    #[must_use]
    pub fn new(icon: IconId, name: &'a str) -> Self {
        Self { icon, name, action: None, active: false, enabled: true, note: None, ink: None, side: None, shown: None }
    }

    /// The action it runs, whose key its tooltip shows.
    #[must_use]
    pub fn action(mut self, action: &'a str) -> Self {
        self.action = Some(action);
        self
    }

    /// Shown as selected (pinned, flagged): the accent wash and the accent glyph.
    #[must_use]
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// Whether it can be pressed.
    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Disabled, its tooltip saying why under its name: `note`.
    #[must_use]
    pub fn unavailable(mut self, note: &'a str) -> Self {
        self.enabled = false;
        self.note = Some(note);
        self
    }

    /// Its icon drawn in `ink` (the navigation rail's).
    #[must_use]
    pub fn ink(mut self, ink: Color32) -> Self {
        self.ink = Some(ink);
        self
    }

    /// A square of `side` points (the rail's larger tiles) instead of a control's side, the icon half as wide.
    #[must_use]
    pub fn side(mut self, side: f32) -> Self {
        self.side = Some(side);
        self
    }

    /// Drawn in `shown` whatever the pointer and the keyboard do: a board of the gallery.
    #[must_use]
    pub fn shown(mut self, shown: Shown) -> Self {
        self.shown = Some(shown);
        self
    }

    /// Draws it into `ui`; its response is clicked when it was pressed.
    pub fn show(self, ui: &mut Ui, theme: &Theme, keymap: &Keymap) -> Response {
        let p = &theme.palette;
        let side = self.side.filter(|side| side.is_finite() && *side >= 1.0).unwrap_or_else(|| control_side(theme)).min(512.0);
        // Coming soon, it is drawn and explains itself like a disabled one.
        let soon = soon::is_soon(ui);
        let enabled = self.enabled && !soon && ui.is_enabled();
        if let Some(action) = self.action {
            soon::offer(ui, action);
        }
        let sense = if enabled { Sense::click() } else { Sense::hover() };
        let (rect, response) = ui.allocate_exact_size(vec2(side, side), sense);
        response.widget_info(|| WidgetInfo::selected(Role::Button, enabled, self.active, self.name));
        let lit = lit(ui, theme, &response, enabled, self.shown, Hover::AtOnce);
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            // The rail's buttons (drawn in its ink) stand on the `panel`, whose hover is `grid`; every other on a
            // `surface`, whose hover is `stage`.
            let hover = if self.ink.is_some() { p.grid } else { p.stage };
            let colour = if !enabled {
                p.dim
            } else if self.active {
                // Selected and pressed: the wash keeps and takes the accent edge, as a pressed primary does (the
                // sheet has no pressed state for a selected button).
                painter.rect_filled(rect, 0.0, p.cyan_muted);
                if lit.pressed {
                    paint::edge(painter, rect, 0.0, 1.0, p.cyan_edge);
                }
                self.ink.unwrap_or(p.cyan_text)
            } else if lit.pressed {
                painter.rect_filled(rect, 0.0, hover);
                paint::edge(painter, rect, 0.0, 1.0, p.muted);
                self.ink.unwrap_or(p.ink)
            } else {
                let rest = Face { fill: None, edge: None, ink: self.ink.unwrap_or(p.muted) };
                let face = between(rest, Face { fill: Some(hover), edge: None, ink: self.ink.unwrap_or(p.ink) }, lit.fade, lit.hover);
                if let Some(fill) = face.fill {
                    painter.rect_filled(rect, 0.0, fill);
                }
                face.ink
            };
            let icon = if self.side.is_some() { side * 0.5 } else { theme.icon_side() };
            crate::iconset::paint(painter, egui::Rect::from_center_size(rect.center(), vec2(icon, icon)), self.icon, colour);
            if lit.focus {
                paint::focus_ring(painter, theme, rect, Ring::Outside);
            }
        }
        // Coming soon, its key would do nothing: the tooltip shows none.
        tooltip::attach_note(response, theme, keymap, self.name, self.action.filter(|_| !soon), self.note)
    }
}

/// How a text button stands out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// `on_cyan` on the `cyan` fill: the one thing to do on a surface.
    Primary,
    /// A 1 px `line` edge (Reply all, Cancel).
    Secondary,
    /// A 1 px `minus` edge and `minus` text: a hard step, such as deleting or blocking.
    Danger,
    /// Text only (Done all, Clear, Discard).
    Plain,
}

/// The widest a text button's label gets before it ends with an ellipsis, in points.
const LABEL_WIDEST: f32 = 240.0;

/// The room left and right inside a text button, in points.
const PAD: f32 = 12.0;

/// The room between a text button's parts (icon, label, arrow, key), in points.
const GAP: f32 = 8.0;

/// What a text button lays out before it is drawn.
struct Laid {
    label: std::sync::Arc<egui::Galley>,
    arrow: Option<std::sync::Arc<egui::Galley>>,
    chip: Option<Chip>,
    label_width: f32,
    width: f32,
}

/// A text button: its name, an optional icon before it, a trailing arrow, its key, and its tone.
#[derive(Debug, Clone, Copy)]
pub struct TextButton<'a> {
    name: &'a str,
    icon: Option<IconId>,
    tone: Tone,
    action: Option<&'a str>,
    note: Option<&'a str>,
    enabled: bool,
    arrow: bool,
    key: bool,
    shown: Option<Shown>,
    /// The widest its label gets ([`Self::widest`]).
    label_widest: f32,
    /// The widest the whole button gets ([`Self::within`]).
    widest: Option<f32>,
    /// As wide as its place ([`Self::fill`]).
    fill: Option<f32>,
    /// The icon alone in a square ([`Self::icon_only`]).
    icon_only: bool,
}

impl<'a> TextButton<'a> {
    /// A plain button called `name`.
    #[must_use]
    pub fn new(name: &'a str) -> Self {
        Self {
            name,
            icon: None,
            tone: Tone::Plain,
            action: None,
            note: None,
            enabled: true,
            arrow: false,
            key: false,
            shown: None,
            label_widest: LABEL_WIDEST,
            widest: None,
            fill: None,
            icon_only: false,
        }
    }

    /// The primary button: on the cyan fill.
    #[must_use]
    pub fn primary(mut self) -> Self {
        self.tone = Tone::Primary;
        self
    }

    /// A secondary button: a 1 px edge.
    #[must_use]
    pub fn secondary(mut self) -> Self {
        self.tone = Tone::Secondary;
        self
    }

    /// The button of a hard step: the `minus` edge and text, filled under the pointer.
    #[must_use]
    pub fn danger(mut self) -> Self {
        self.tone = Tone::Danger;
        self
    }

    /// A plain button: text only.
    #[must_use]
    pub fn plain(mut self) -> Self {
        self.tone = Tone::Plain;
        self
    }

    /// The button in `tone`.
    #[must_use]
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }

    /// `icon` before the name, in the Regular style.
    #[must_use]
    pub fn icon(mut self, icon: IconId) -> Self {
        self.icon = Some(icon);
        self
    }

    /// The trailing `→` of an act that leads to another place or a next step (Read mail, Continue, New event); an
    /// act done in place (Save, Send, Done) has none.
    #[must_use]
    pub fn arrow(mut self) -> Self {
        self.arrow = true;
        self
    }

    /// Whether the key of its action is `shown` as a chip in the button, after the name. An action without a key
    /// shows none.
    #[must_use]
    pub fn key(mut self, shown: bool) -> Self {
        self.key = shown;
        self
    }

    /// The widest its label gets before it ends with an ellipsis, in points: at most the 240 of every text button, at
    /// least 1 (a value that is not a number keeps the 240).
    #[must_use]
    pub fn widest(mut self, points: f32) -> Self {
        if points.is_finite() {
            self.label_widest = points.clamp(1.0, LABEL_WIDEST);
        }
        self
    }

    /// The action it runs, whose key its tooltip shows.
    #[must_use]
    pub fn action(mut self, action: &'a str) -> Self {
        self.action = Some(action);
        self
    }

    /// Whether it can be pressed: one that cannot has `muted` text and a dashed edge.
    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Cannot be pressed yet: drawn disabled, its tooltip saying why (`note`).
    #[must_use]
    pub fn unavailable(mut self, note: &'a str) -> Self {
        self.note = Some(note);
        self
    }

    /// Drawn in `shown` whatever the pointer and the keyboard do: a board of the gallery.
    #[must_use]
    pub fn shown(mut self, shown: Shown) -> Self {
        self.shown = Some(shown);
        self
    }

    /// At most `widest` points wide, its label cut with one `…` where the whole would not fit (its name is still heard
    /// whole): a button in a room narrower than its words never runs past the room.
    #[must_use]
    pub fn within(mut self, widest: f32) -> Self {
        self.widest = Some(widest);
        self
    }

    /// As wide as its place, `width` points (the index's New message): the icon and the label at the left, the key's
    /// chip at the right edge, the label taking the room between them. A width that is not a number is not taken; one
    /// too narrow for the parts leaves the label 1 point.
    #[must_use]
    pub fn fill(mut self, width: f32) -> Self {
        self.fill = width.is_finite().then_some(width.min(4096.0));
        self
    }

    /// Its icon alone in a square of a control's side (the narrow index's New message); its name is heard and its
    /// tooltip says it, with its key. Without an icon it shows its name as usual.
    #[must_use]
    pub fn icon_only(mut self) -> Self {
        self.icon_only = true;
        self
    }

    /// Whether only its icon is drawn ([`Self::icon_only`]).
    fn bare(&self) -> bool {
        self.icon_only && self.icon.is_some()
    }

    /// How wide it is drawn in `ui`, in points: what a row of buttons measures before it places them.
    #[must_use]
    pub fn width(&self, ui: &Ui, theme: &Theme, keymap: &Keymap) -> f32 {
        let soon = soon::is_soon(ui);
        let enabled = self.enabled && self.note.is_none() && !soon && ui.is_enabled();
        self.lay_out(ui, theme, keymap, enabled, soon).width
    }

    /// Its label, arrow and key chip laid out, and the width of its box.
    fn lay_out(&self, ui: &Ui, theme: &Theme, keymap: &Keymap, enabled: bool, soon: bool) -> Laid {
        let action = self.action.filter(|_| !soon);
        let key = action.filter(|_| self.key).and_then(|action| keymap.chip(ui.ctx(), action));
        let font = theme.fonts.button;
        // Laid out before the state is known: the colour is the painter's, given when it is painted.
        let arrow = self.arrow.then(|| super::galley(ui, "→", font, Color32::PLACEHOLDER));
        let chip_state = match (enabled, self.tone) {
            (false, _) => ChipState::Disabled,
            (true, Tone::Primary) => ChipState::OnCyan,
            (true, _) => ChipState::Default,
        };
        let chip = key.map(|key| Chip::new(ui, theme, &key, chip_state));
        // The label's room: its widest ([`Self::widest`]), or less, what `within` leaves it beside the other parts and
        // the pads.
        let others =
            [self.icon.map(|_| theme.icon_side()), arrow.as_ref().map(|arrow| arrow.size().x.ceil()), chip.as_ref().map(|chip| chip.size().x)];
        let beside = others.iter().flatten().map(|wide| wide + GAP).sum::<f32>();
        // As wide as its place, the label takes all the room the other parts leave.
        let filled = self.fill.map(|fill| (fill - beside - PAD * 2.0).floor().max(1.0));
        let room = filled
            .unwrap_or_else(|| {
                self.widest.filter(|widest| widest.is_finite()).map_or(self.label_widest, |widest| self.label_widest.min(widest - beside - PAD * 2.0))
            })
            .floor()
            .max(1.0);
        let label = super::cut(ui, self.name, font, Color32::PLACEHOLDER, room);
        // A label cut at its widest takes all of it, as the design's does; so does a filled one.
        let label_width = if label.elided || filled.is_some() { room } else { label.size().x.ceil() };
        if self.bare() {
            return Laid { label, arrow: None, chip: None, label_width: 0.0, width: control_side(theme) };
        }
        let parts = [
            self.icon.map(|_| theme.icon_side()),
            Some(label_width),
            arrow.as_ref().map(|arrow| arrow.size().x.ceil()),
            chip.as_ref().map(|chip| chip.size().x),
        ];
        let inner = parts.iter().flatten().sum::<f32>() + GAP * (parts.iter().flatten().count().saturating_sub(1) as f32);
        Laid { label, arrow, chip, label_width, width: inner + PAD * 2.0 }
    }

    /// Draws it into `ui`; its response is clicked when it was pressed.
    pub fn show(self, ui: &mut Ui, theme: &Theme, keymap: &Keymap) -> Response {
        let p = &theme.palette;
        let soon = soon::is_soon(ui);
        let enabled = self.enabled && self.note.is_none() && !soon && ui.is_enabled();
        if let Some(action) = self.action {
            soon::offer(ui, action);
        }
        // Coming soon, its key would do nothing: neither the chip nor the tooltip shows it.
        let action = self.action.filter(|_| !soon);
        let Laid { label, arrow, chip, label_width, width } = self.lay_out(ui, theme, keymap, enabled, soon);
        let icon_side = theme.icon_side();
        let sense = if enabled { Sense::click() } else { Sense::hover() };
        let (rect, response) = ui.allocate_exact_size(vec2(width, control_side(theme)), sense);
        response.widget_info(|| WidgetInfo::labeled(Role::Button, enabled, self.name));
        let lit = lit(ui, theme, &response, enabled, self.shown, Hover::Fades);
        // M3: at full the label resolves from glyphs when the pointer comes on; the characters that read so far.
        let bare = self.bare();
        let resolved = crate::scramble::count(&label)
            .filter(|_| !bare)
            .and_then(|count| crate::scramble::hover(ui.ctx(), response.id, enabled && self.shown.is_none() && response.hovered(), count));
        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            // The tone's colours at rest, under the pointer and pressed; hover cross-fades from rest, pressed is at once.
            let face = |fill, edge, ink| Face { fill, edge, ink };
            let (rest, hovered, held) = match self.tone {
                Tone::Primary => {
                    (face(Some(p.cyan), None, p.on_cyan), face(Some(p.ink), None, p.bg), face(Some(p.cyan_muted), Some(p.cyan_edge), p.cyan_text))
                }
                Tone::Secondary => {
                    (face(None, Some(p.line), p.ink), face(Some(p.stage), Some(p.muted), p.ink), face(Some(p.stage), Some(p.ink), p.ink))
                }
                Tone::Danger => (face(None, Some(p.minus), p.minus), face(Some(p.minus), None, p.bg), face(Some(p.stage), Some(p.minus), p.minus)),
                Tone::Plain => (face(None, None, p.ink), face(Some(p.stage), None, p.ink), face(Some(p.stage), Some(p.muted), p.ink)),
            };
            let Face { fill, edge, ink } = if !enabled {
                face(None, None, p.muted)
            } else if lit.pressed {
                held
            } else {
                // A fill or edge that is not there at one end switches at once; the rest cross-fades.
                between(rest, hovered, lit.fade, lit.hover)
            };
            if let Some(fill) = fill {
                painter.rect_filled(paint::on_pixels(painter, rect), 0.0, fill);
            }
            if let Some(edge) = edge {
                paint::edge(painter, rect, 0.0, 1.0, edge);
            }
            // Disabled is a dashed `line` edge round a box, never a fade; a plain button has no box to dash.
            if !enabled && self.tone != Tone::Plain {
                paint::dashed(painter, rect, p.line);
            }
            if bare {
                if let Some(icon) = self.icon {
                    crate::iconset::paint(painter, egui::Rect::from_center_size(rect.center(), vec2(icon_side, icon_side)), icon, ink);
                }
                if lit.focus {
                    paint::focus_ring(painter, theme, rect, Ring::Outside);
                }
                return tooltip::attach_note(response, theme, keymap, self.name, action, self.note);
            }
            let mut x = rect.left() + PAD;
            if let Some(icon) = self.icon {
                let at = egui::Rect::from_min_size(egui::pos2(x, rect.center().y - icon_side / 2.0), vec2(icon_side, icon_side));
                crate::iconset::paint(painter, at, icon, ink);
                x += icon_side + GAP;
            }
            let at = egui::pos2(x, rect.center().y - label.size().y / 2.0);
            if let Some(done) = resolved {
                // The glyphs stand in the label's own box: a wider one never pushes past it.
                let boxed = egui::Rect::from_min_max(egui::pos2(x, rect.top()), egui::pos2(x + label_width, rect.bottom()));
                let clipped = painter.with_clip_rect(painter.clip_rect().intersect(boxed));
                let seed = label.job.text.len() as u64;
                clipped.galley(at, crate::scramble::galley(painter, &label, &(0..done), seed), ink);
            } else {
                painter.galley(at, label, ink);
            }
            x += label_width + GAP;
            if let Some(arrow) = arrow {
                let width = arrow.size().x.ceil();
                painter.galley(egui::pos2(x, rect.center().y - arrow.size().y / 2.0), arrow, ink);
                x += width + GAP;
            }
            if let Some(chip) = chip {
                let top = rect.center().y - chip.size().y / 2.0;
                chip.paint(painter, egui::pos2(x, top.round()));
            }
            if lit.focus {
                paint::focus_ring(painter, theme, rect, Ring::Outside);
            }
        }
        tooltip::attach_note(response, theme, keymap, self.name, action, self.note)
    }
}

/// A text button called `name` that runs `action`, whose key its tooltip shows: a plain one, or the `primary` one on
/// the cyan fill.
pub fn text(ui: &mut Ui, theme: &Theme, keymap: &Keymap, name: &str, action: Option<&str>, primary: bool) -> Response {
    let button = TextButton::new(name);
    let button = if primary { button.primary() } else { button };
    match action {
        Some(action) => button.action(action),
        None => button,
    }
    .show(ui, theme, keymap)
}

/// The width of a [`text`] button called `name`, with no icon, arrow or key: its words (cut at 240) and its 12 at each
/// side.
pub fn text_width(ui: &Ui, theme: &Theme, name: &str) -> f32 {
    let label = super::cut(ui, name, theme.fonts.button, Color32::PLACEHOLDER, LABEL_WIDEST);
    let words = if label.elided { LABEL_WIDEST } else { label.size().x.ceil() };
    words + PAD * 2.0
}

/// A text button like [`text`] that cannot be pressed yet: drawn disabled, and its tooltip says why (`note`).
pub fn unavailable(ui: &mut Ui, theme: &Theme, keymap: &Keymap, name: &str, primary: bool, note: &str) -> Response {
    let button = TextButton::new(name).unavailable(note);
    if primary { button.primary() } else { button }.show(ui, theme, keymap)
}

#[cfg(test)]
#[path = "button_tests.rs"]
mod tests;

/// The room a [`link`] of a line leaves left and right of its text, in points.
pub const LINK_PAD: f32 = 4.0;

/// The two sizes of a link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkSize {
    /// A link of its own: mono 13, 24 high, [`LINK_PAD`] beside the text.
    Line,
    /// The small link of a row (open, save, show all, undo): mono 11, 16 high, no room beside the text, so it stands
    /// on the row's text line.
    Small,
}

/// A link called `name` of a line: [`link_sized`] in [`LinkSize::Line`].
pub fn link(ui: &mut Ui, theme: &Theme, keymap: &Keymap, name: &str) -> Response {
    link_sized(ui, theme, keymap, name, LinkSize::Line)
}

/// A link called `name` in `size`: the accent as text on nothing, underlined under the pointer, underlined on the
/// accent wash while pressed, the focus ring 2 px clear of it; `muted` when it cannot be used (coming soon).
pub fn link_sized(ui: &mut Ui, theme: &Theme, keymap: &Keymap, name: &str, size: LinkSize) -> Response {
    link_with(ui, theme, keymap, name, size, true, None)
}

/// [`link_sized`], `enabled` or not, drawn in `shown` when it is given (the gallery's boards).
pub(crate) fn link_with(ui: &mut Ui, theme: &Theme, keymap: &Keymap, name: &str, size: LinkSize, enabled: bool, shown: Option<Shown>) -> Response {
    link_drawn(ui, theme, keymap, name, None, size, enabled, shown)
}

/// The caret after a link's words that opens (`▾`) or closes (`▴`) what follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Caret {
    /// `▾`: opens.
    Down,
    /// `▴`: closes.
    Up,
    /// `▸`: folded (a thread's message, a quote).
    Right,
}

impl Caret {
    /// The sign the caret stands for, as a screen reader hears it.
    const fn sign(self) -> char {
        match self {
            Self::Down => '▾',
            Self::Up => '▴',
            Self::Right => '▸',
        }
    }
}

/// The sheet's caret drawn in code, as shares of the text size: Plex Mono holds no such sign and the design draws it
/// from another face, a triangle half the size wide and high (measured on the design's board): how far the next text
/// would stand, its left and right, and its top and foot over the baseline.
const CARET: (f32, f32, f32, f32, f32) = (0.502, 0.003, 0.499, 0.509, 0.011);

/// The width a caret takes in `font`: as far as the next text would stand.
pub(crate) fn caret_width(font: crate::theme::Font) -> f32 {
    CARET.0 * font.size
}

/// The design's baseline of a line of `font` whose box is centred on `centre_y`: the box of the line height in the
/// middle, half the leading over the ascent (not the hinted one egui puts the letters on, a fraction of a pixel lower).
pub(crate) fn baseline(font: crate::theme::Font, centre_y: f32) -> f32 {
    let (ascent, descent) = font.face.extent();
    centre_y - font.line / 2.0 + (font.line - (ascent + descent) * font.size) / 2.0 + ascent * font.size
}

/// Paints `caret` in `font`'s size as the design draws it, a triangle standing `after` the text before it, its foot
/// on `baseline`, in `colour`.
pub(crate) fn paint_caret(painter: &egui::Painter, font: crate::theme::Font, after: f32, baseline: f32, caret: Caret, colour: Color32) {
    let (_, left, right, high, low) = CARET;
    let s = font.size;
    let (top, foot) = (baseline - high * s, baseline - low * s);
    let (left, right) = (after + left * s, after + right * s);
    let middle = (left + right) / 2.0;
    let points = match caret {
        Caret::Down => vec![egui::pos2(left, top), egui::pos2(right, top), egui::pos2(middle, foot)],
        Caret::Up => vec![egui::pos2(left, foot), egui::pos2(middle, top), egui::pos2(right, foot)],
        Caret::Right => vec![egui::pos2(left, top), egui::pos2(right, (top + foot) / 2.0), egui::pos2(left, foot)],
    };
    painter.add(egui::Shape::convex_polygon(points, colour, Stroke::NONE));
}

/// A small link of `words` followed by `caret`, drawn as the design draws it: the words, a space, and the caret's
/// triangle; a screen reader hears the words and the caret's sign.
pub(crate) fn link_caret(ui: &mut Ui, theme: &Theme, keymap: &Keymap, words: &str, caret: Caret) -> Response {
    let name = format!("{words} {}", caret.sign());
    link_drawn(ui, theme, keymap, &name, Some((words, caret)), LinkSize::Small, true, None)
}

/// The width of a [`link_caret`] of `words`: its text box hugging the words, the space and the caret.
pub(crate) fn link_caret_width(ui: &Ui, theme: &Theme, words: &str) -> f32 {
    let font = theme.fonts.mono_s;
    paint::run_box(super::galley(ui, &format!("{words} "), font, theme.palette.cyan_text).size().x + CARET.0 * font.size)
}

/// A link called `name`; with `caret`, its words and the caret drawn in place of `name`.
#[expect(clippy::too_many_arguments, reason = "a link's arguments, its caret and the state a board forces")]
fn link_drawn(
    ui: &mut Ui,
    theme: &Theme,
    keymap: &Keymap,
    name: &str,
    caret: Option<(&str, Caret)>,
    size: LinkSize,
    enabled: bool,
    shown: Option<Shown>,
) -> Response {
    let p = &theme.palette;
    let enabled = enabled && !soon::is_soon(ui) && ui.is_enabled();
    let (font, pad, height) = match size {
        LinkSize::Line => (theme.fonts.button, LINK_PAD, theme.boxes().link),
        LinkSize::Small => (theme.fonts.mono_s, 0.0, theme.boxes().row_link),
    };
    let ink = if enabled { p.cyan_text } else { p.muted };
    let (galley, width) = match caret {
        Some((words, _)) => {
            let galley = super::galley(ui, &format!("{words} "), font, ink);
            let width = paint::run_box(galley.size().x + CARET.0 * font.size);
            (galley, width)
        }
        None => {
            let galley = super::galley(ui, name, font, ink);
            let width = galley.size().x.ceil();
            (galley, width)
        }
    };
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, response) = ui.allocate_exact_size(vec2(width + pad * 2.0, height), sense);
    response.widget_info(|| WidgetInfo::labeled(Role::Button, enabled, name));
    let lit = lit(ui, theme, &response, enabled, shown, Hover::AtOnce);
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        if lit.pressed {
            painter.rect_filled(rect, 0.0, p.cyan_muted);
        }
        let top = (rect.center().y - galley.size().y / 2.0).round();
        // The baseline is the design's text box's (the box of the line height in the middle, half the leading over the
        // ascent), not the hinted one egui puts the letters on, which lies a fraction of a pixel lower.
        let baseline = baseline(font, rect.center().y);
        // The underline, under the pointer and while pressed: the font's own, as the sheet's text decoration draws it,
        // its thickness and its place under the baseline from the face, so its edges may fall between pixels.
        if lit.pressed || lit.hover {
            let (below, thick) = font.face.underline();
            let middle = baseline + below * font.size;
            let band = (middle - thick * font.size / 2.0, middle + thick * font.size / 2.0);
            paint::sampled_band(painter, rect.left() + pad..=rect.left() + pad + width, band, p.cyan_text);
        }
        let text_left = rect.left() + pad;
        let after = text_left + galley.size().x;
        painter.galley(egui::pos2(text_left, top), galley, ink);
        if let Some((_, caret)) = caret {
            paint_caret(painter, font, after, baseline, caret, ink);
        }
        if lit.focus {
            paint::focus_ring(painter, theme, rect, Ring::Outside);
        }
    }
    tooltip::attach(response, theme, keymap, name, None)
}

/// Whether `response` was the owner's own press: clicked, and not by a held Enter or Space repeating while it has the
/// keyboard. egui counts a held key's repeats as presses, so a key held on from an earlier press (one that asked, or
/// one that ran the row before this one took its place) would run an owner's command again and again. Only a fresh
/// press, of the pointer or of a key, counts. The settings' controls that destroy or toggle ask this, not `clicked`:
/// the danger buttons (cancel a scheduled entry, delete a template, remove an account) and the row buttons that ask
/// for them, "ask again", the templates' Save and the switches that run a command. A Save whose write is the same on
/// a repeat (the signature's, the account form's) does not.
#[must_use]
pub fn owner_pressed(ui: &Ui, response: &Response) -> bool {
    response.clicked() && !(response.has_focus() && key_repeated(ui))
}

/// Whether Enter or Space came this frame as a held key's repeat.
fn key_repeated(ui: &Ui) -> bool {
    ui.input(|input| {
        input
            .events
            .iter()
            .any(|event| matches!(event, egui::Event::Key { key: egui::Key::Enter | egui::Key::Space, pressed: true, repeat: true, .. }))
    })
}
