// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The components every view is built from (`docs/design.md`), each drawn only from the [`crate::theme`] tokens,
//! moving only through [`crate::motion`] and showing keys only from the [`crate::keymap`]:
//!
//! - [`card`]: a rounded card with a header that opens and closes it (grouped inbox groups, the screener, messages).
//! - [`button`]: an icon button with its tooltip, active or disabled.
//! - [`field`]: the search pill and the margin of a form field.
//! - [`pill`]: a square filter chip, on or off.
//! - [`segmented`]: a few choices side by side in one track, one of them on the selection.
//! - [`select`]: a closed field with the choice in it and a popup of the choices.
//! - [`checkbox`]: a filled check box and a radio button.
//! - [`tooltip`]: the bubble with the action's name and its key chip.
//! - [`key`]: the chip of a key, its words read from a shortcut.
//! - [`tag`]: a short mark in a 1 px box, and the coming-soon mark.
//! - [`label`]: the bracketed section label and the written form of a count.
//! - [`avatar`]: a person's initials, `muted` on a `stage` square.
//! - [`badge`]: an unread count, or a group's outline icon.
//! - [`icon`]: one of the 45 icons on its own, `muted`.
//! - [`scroll`]: the scroll bar, 12 wide beside what scrolls, and the style egui scrolls with.
//! - [`hover`]: the action group that fades in over a hovered or selected row.
//! - [`toast`]: a message that rises from the bottom edge, with Undo and its key chip.
//! - [`popup`]: a float with menu rows, over sharp text or content that fades.
//! - [`recipient`]: a recipient's chip and the recipient line of the composer.
//! - [`switch`]: a square box with a square knob, on or off, waiting or unavailable.
//! - [`slider`]: the text size on five ticks, 90 % to 130 %.
//! - [`account_bar`]: the 3 × 12 px mark in an account's colour.
//! - [`soon`]: what is designed and not built yet, drawn unavailable, and its "coming soon" tag.
//! - [`time_picker`]: presets and a typed time for snooze, send later and reminders ([`typed_time`] reads the text).
//!
//! [`gallery`] draws every one of them on one page for `ui_shot`, [`gallery_fields`] the fields, pickers and menus on a
//! second.

pub mod avatar;
pub mod badge;
pub mod banner;
pub mod button;
pub mod card;
pub mod check_step;
pub mod checkbox;
pub mod empty;
pub mod facts;
pub mod field;
pub mod gallery;
pub mod gallery_blocks;
pub mod gallery_fields;
pub mod hover;
pub mod icon;
pub mod key;
pub mod label;
pub mod pill;
pub mod popup;
pub mod progress;
pub mod recipient;
pub mod row;
pub mod scroll;
pub mod segmented;
pub mod select;
pub mod settings_row;
pub mod slider;
pub mod soon;
pub mod switch;
pub mod tag;
pub mod time_picker;
pub mod toast;
pub mod tooltip;
pub mod typed_time;

use std::sync::Arc;

use egui::{CornerRadius, Galley, Key, Margin, Response, Ui};

use crate::theme::{Font, Theme};

/// The side of a control (an icon button, a text button, a hover action): the design's fixed button height at the
/// text scale; its text is centred in it.
#[must_use]
pub fn control_side(theme: &Theme) -> f32 {
    theme.boxes().button
}

/// The state a pressable control is drawn in, as its sheet names it: what the pointer and the keyboard do to it, or
/// what a board of the gallery asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shown {
    /// Nothing on it.
    Rest,
    /// The pointer over it.
    Hover,
    /// The keyboard focus on it.
    Focus,
    /// Held down by the pointer, or by Enter or Space while it has the focus.
    Pressed,
}

/// What a pressable control shows of the pointer and the keyboard: the hover fill, the pressed look, the focus ring.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct Lit {
    /// The pointer is over it (and it can be used).
    pub hover: bool,
    /// How far its colours have faded from rest (0) to hover (1): over the `fast` token at full motion, at once
    /// with reduced motion.
    pub fade: f32,
    pub pressed: bool,
    pub focus: bool,
}

/// How a control's hover comes: as the presentation gives it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Hover {
    /// A cross-fade over the `fast` token at full motion (`.btn`: its edge and background); at once with reduced
    /// motion.
    Fades,
    /// At once at every level (the presentation gives it no transition).
    AtOnce,
}

/// What `response` shows, `enabled` or not, in `theme`; `forced` draws a state whatever the pointer does (the
/// gallery's boards). Hover comes as `hover` says, through [`crate::motion::fade`] when it fades; pressed and focus
/// are at once at every level. Pressed is the pointer held down on it, or Enter or Space held down while it has the
/// focus.
pub(crate) fn lit(ui: &Ui, theme: &crate::theme::Theme, response: &Response, enabled: bool, forced: Option<Shown>, hover_comes: Hover) -> Lit {
    if let Some(shown) = forced {
        let hover = shown == Shown::Hover;
        return Lit { hover, fade: if hover { 1.0 } else { 0.0 }, pressed: shown == Shown::Pressed && enabled, focus: shown == Shown::Focus };
    }
    let hover = enabled && response.hovered();
    let fade = match hover_comes {
        Hover::Fades => crate::motion::fade(ui.ctx(), response.id.with("hover"), hover, theme.motion.fast),
        Hover::AtOnce => f32::from(u8::from(hover)),
    };
    let focus = response.has_focus();
    let keyed = focus && ui.input(|input| input.key_down(Key::Enter) || input.key_down(Key::Space));
    let pressed = enabled && (response.is_pointer_button_down_on() || keyed);
    Lit { hover, fade: if enabled { fade } else { 0.0 }, pressed, focus }
}

/// A control's colours in one state: its fill and edge (`None`: not painted) and its text.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Face {
    pub fill: Option<egui::Color32>,
    pub edge: Option<egui::Color32>,
    pub ink: egui::Color32,
}

/// The colours `t` of the way from `rest` to `hover` (0 to 1), moving `toward_hover` or back: each colour opaque at
/// both ends is mixed between them, never faded by alpha. A fill or edge that one end does not paint has no colour to
/// mix with (the ground under a control is not known), so it switches at once to the end it moves toward; the text
/// goes with its fill.
pub(crate) fn between(rest: Face, hover: Face, t: f32, toward_hover: bool) -> Face {
    let t = t.clamp(0.0, 1.0);
    if t <= 0.0 {
        return rest;
    }
    if t >= 1.0 {
        return hover;
    }
    let pick = |a: Option<egui::Color32>, b: Option<egui::Color32>| match (a, b) {
        (Some(a), Some(b)) => Some(a.lerp_to_gamma(b, t)),
        _ if toward_hover => b,
        _ => a,
    };
    // The text follows its fill: it fades when the fill fades and switches with it when the fill switches, so it
    // never stands for a moment in the colour of the fill under it.
    let ink = match (rest.fill, hover.fill) {
        (Some(_), None) | (None, Some(_)) => {
            if toward_hover {
                hover.ink
            } else {
                rest.ink
            }
        }
        _ => rest.ink.lerp_to_gamma(hover.ink, t),
    };
    Face { fill: pick(rest.fill, hover.fill), edge: pick(rest.edge, hover.edge), ink }
}

/// `text` in `font` and `colour`, on one line as high as the style's line height, with its tracking and, if it asks for
/// them, in capitals.
pub(crate) fn galley(ui: &Ui, text: &str, font: Font, colour: egui::Color32) -> Arc<Galley> {
    let mut job = egui::text::LayoutJob::single_section(font.text(text), font.format(colour));
    job.wrap.max_width = f32::INFINITY;
    job.break_on_newline = false;
    crate::draw::paint::on_baseline(ui.painter().layout_job(job))
}

/// [`galley`], cut with an ellipsis at `width`. As the design cuts a line, the ellipsis follows the last letter kept:
/// spaces the cut leaves before it are dropped.
pub(crate) fn cut(ui: &Ui, text: &str, font: Font, colour: egui::Color32, width: f32) -> Arc<Galley> {
    let mut job = egui::text::LayoutJob::single_section(font.text(text), font.format(colour));
    job.wrap = egui::text::TextWrapping { max_width: width.max(1.0), max_rows: 1, break_anywhere: true, overflow_character: Some('…') };
    job.break_on_newline = false;
    crate::draw::paint::on_baseline(crate::draw::paint::cut_within(ui.painter(), job, width.max(1.0)))
}

/// `text` in `font` and `colour`, wrapped at `width` onto at most `rows` lines (at least one), the last cut with one
/// ellipsis; each line placed as `align` says (centred lines centre on the galley's origin). Text from outside (a
/// mail's words, a server's answer) goes through [`plain`] first.
pub(crate) fn wrapped(ui: &Ui, text: &str, font: Font, colour: egui::Color32, width: f32, rows: usize, align: egui::Align) -> Arc<Galley> {
    let mut job = egui::text::LayoutJob::single_section(font.text(text), font.format(colour));
    job.wrap = egui::text::TextWrapping { max_width: width.max(1.0), max_rows: rows.max(1), break_anywhere: false, overflow_character: Some('…') };
    job.halign = align;
    crate::draw::paint::on_baseline(ui.painter().layout_job(job))
}

/// `text` with every control character (a NUL, a line break, a tab) put as a space and every explicit bidi embedding,
/// override and isolate left out: text from outside drawn as plain words, never as layout. egui applies the bidi
/// algorithm, so an override in a mail's words would turn round what follows it. The direction marks (U+200E, U+200F,
/// U+061C) stay: they only settle where brackets and digits of a right-to-left name fall, and the words' own direction
/// (Hebrew, Arabic) still shows.
pub(crate) fn plain(text: &str) -> std::borrow::Cow<'_, str> {
    if text.chars().any(|c| c.is_control() || bidi_override(c)) {
        std::borrow::Cow::Owned(text.chars().filter(|c| !bidi_override(*c)).map(|c| if c.is_control() { ' ' } else { c }).collect())
    } else {
        std::borrow::Cow::Borrowed(text)
    }
}

/// `text` cut after at most `most` characters, between grapheme clusters (a letter with its marks, an emoji sequence
/// is never split), and whether it was: a bound on what one line lays out, so a mail's 10 000 combining marks (which
/// take no width and so are never cut by the line) cost no more than a line. One cluster longer than `most` is left out
/// whole.
pub(crate) fn capped(text: &str, most: usize) -> (&str, bool) {
    use unicode_segmentation::UnicodeSegmentation as _;
    let mut chars = 0_usize;
    for (at, grapheme) in text.grapheme_indices(true) {
        chars = chars.saturating_add(grapheme.chars().count());
        if chars > most {
            return (text.get(..at).unwrap_or_default(), true);
        }
    }
    (text, false)
}

/// `text` as it fits on one line of `room` points in `font`: whole, or its longest beginning in the written order that
/// ends at a grapheme boundary, with no space before one closing `…`; empty when not even the `…` fits. A right-to-left
/// name is cut at its logical end and laid out once, so it is never mirrored.
pub(crate) fn fit(ui: &Ui, text: &str, font: Font, room: f32) -> String {
    use unicode_segmentation::UnicodeSegmentation as _;
    let width = |text: &str| galley(ui, text, font, egui::Color32::PLACEHOLDER).size().x;
    if width(text) <= room {
        return text.to_owned();
    }
    let ends: Vec<usize> = text.grapheme_indices(true).map(|(at, grapheme)| at + grapheme.len()).collect();
    let cut = |count: usize| {
        let end = if count == 0 { 0 } else { ends.get(count - 1).copied().unwrap_or(0) };
        format!("{}…", text.get(..end).unwrap_or_default().trim_end())
    };
    // The most clusters whose cut fits: a binary search, each step one layout (egui keeps the galleys it laid out).
    let (mut low, mut high) = (0_usize, ends.len());
    if width(&cut(0)) > room {
        return String::new();
    }
    while low < high {
        let middle = low + (high - low).div_ceil(2);
        if width(&cut(middle)) <= room {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    cut(low)
}

/// The most characters of one line of words from outside that are laid out at all: far more than a line shows.
pub(crate) const MOST: usize = 400;

/// `text` as it fits on one line of `room` points in `font` ([`fit`]), of which at most [`MOST`] characters are laid out
/// at all: a value as long as a header costs no more than a line. A cut at the cap ends with the ellipsis too.
pub(crate) fn fit_capped(ui: &Ui, text: &str, font: Font, room: f32) -> String {
    let (kept, capped) = capped(text, MOST);
    let shown = fit(ui, kept, font, room);
    if capped && !shown.ends_with('…') {
        // The line held all it was given: the ellipsis says the rest was left out.
        return fit(ui, &format!("{}…", kept.trim_end()), font, room);
    }
    shown
}

/// `text` from outside on one line of `room` points in `font` and `colour`: [`plain`], capped and cut between grapheme
/// clusters with one ellipsis ([`fit_capped`]).
pub(crate) fn fitted(ui: &Ui, text: &str, font: Font, colour: egui::Color32, room: f32) -> Arc<Galley> {
    galley(ui, &fit_capped(ui, &plain(text), font, room), font, colour)
}

/// Whether `c` is an explicit bidi embedding, override or isolate (U+202A to U+202E, U+2066 to U+2069): what could turn
/// round the words after it.
fn bidi_override(c: char) -> bool {
    matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
}

/// `text` from outside as a name a screen reader hears or a link carries: [`plain`], so no name carries an override
/// that could turn round what follows it.
pub(crate) fn spoken(text: &str) -> String {
    plain(text).into_owned()
}

/// `text` as a label in `font` and `colour`. A style in capitals paints capitals, but a screen reader hears `text` as
/// written.
pub(crate) fn label(ui: &mut Ui, font: Font, text: &str, colour: egui::Color32) -> egui::Response {
    let response = ui.label(font.rich(text).color(colour));
    if font.caps {
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, ui.is_enabled(), text));
    }
    response
}

/// A margin of `x` points left and right and `y` above and below.
pub(crate) fn margin(x: f32, y: f32) -> Margin {
    // Distances are a few points; the clamp keeps the casts inside i8.
    let points = |value: f32| value.clamp(0.0, 100.0) as i8;
    Margin::symmetric(points(x), points(y))
}

/// The size of the account bar in points: a fixed mark, as large at every text scale.
const ACCOUNT_BAR: egui::Vec2 = egui::vec2(3.0, 12.0);

/// The account bar: 3 × 12 px, square, in the colour of the account counted `account` from 0, the six colours
/// repeating from the seventh (`accounts[account % 6]`). It is a mark beside a row's sender; the row says the account.
pub fn account_bar(ui: &mut Ui, theme: &Theme, account: u64) -> Response {
    let (rect, response) = ui.allocate_exact_size(ACCOUNT_BAR, egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        ui.painter().rect_filled(rect, 0.0, theme.account_colour(account));
    }
    response
}

/// A corner radius of `points`.
pub(crate) fn corner(points: f32) -> CornerRadius {
    // A few points; the clamp keeps the cast inside u8.
    CornerRadius::same(points.clamp(0.0, 255.0) as u8)
}

/// `ui`'s painter at full strength: egui fades a disabled `Ui`'s painter, and disabled is never a fade in this look
/// (`muted` text and a dashed edge instead). A control that draws its own disabled state paints with this.
pub(crate) fn solid(ui: &Ui) -> egui::Painter {
    let mut painter = ui.painter().clone();
    if !ui.is_enabled() {
        painter.set_opacity(1.0);
    }
    painter
}

#[cfg(test)]
pub(crate) mod tests;

#[cfg(test)]
#[path = "walk_tests.rs"]
mod walk_tests;

#[cfg(test)]
#[path = "press_tests.rs"]
pub(crate) mod press_tests;

#[cfg(test)]
#[path = "account_tests.rs"]
mod account_tests;
