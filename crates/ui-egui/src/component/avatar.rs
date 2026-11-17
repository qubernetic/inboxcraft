// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! An avatar: a person's initials in `muted` on a `stage` square with corners of 2 px, 24, 32 or 40 px
//! ([`AvatarSize`]); an unknown person is a `?` in a dashed box. Every person's avatar has the same colours: colour
//! belongs to accounts, never to people.

use egui::{Painter, Rect, Response, Role, Sense, Ui, WidgetInfo, pos2, vec2};

use crate::draw::paint::dashed_round;
use crate::theme::Theme;
use crate::tr;

/// The corner radius of an avatar square: one of the two things that are not square.
pub const CORNER: f32 = 2.0;

/// The three sizes of an avatar on its sheet: small, middle and large.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AvatarSize {
    /// 24 px.
    S,
    /// 32 px, the default.
    M,
    /// 40 px.
    L,
}

impl AvatarSize {
    /// The side of the square at the text scale of `theme`.
    #[must_use]
    pub fn side(self, theme: &Theme) -> f32 {
        let [s, m, l] = theme.boxes().avatar;
        match self {
            Self::S => s,
            Self::M => m,
            Self::L => l,
        }
    }
}

/// The first letter or digit of `word`, upper case.
fn letter(word: &str) -> Option<String> {
    word.chars().find(|c| c.is_alphanumeric()).map(|c| c.to_uppercase().collect())
}

/// The initials of `name`: the first letters of its first and last word, or of its only word; the first letter of
/// the address when the name has none; `?` when neither has one.
#[must_use]
pub fn initials(name: &str, address: &str) -> String {
    let words: Vec<&str> = name.split_whitespace().filter(|word| word.chars().any(char::is_alphanumeric)).collect();
    let letters = match words.as_slice() {
        [] => None,
        [only] => letter(only),
        [first, .., last] => letter(first).zip(letter(last)).map(|(a, b)| a + &b),
    };
    letters.or_else(|| letter(address.split('@').next().unwrap_or_default())).unwrap_or_else(|| UNKNOWN.to_owned())
}

/// Paints the avatar of `name` and `address` into the square inside `rect`: the initials in `muted` on a `stage`
/// square with corners of 2, in the key style (Plex Mono SemiBold 11/14), and 13/16 from the large side up. A person
/// with neither a name nor a usable address is a `?` in a dashed `line` box with the same corners.
pub fn paint(painter: &Painter, theme: &Theme, rect: Rect, name: &str, address: &str) {
    let side = rect.width().min(rect.height());
    if !side.is_finite() || side <= 0.0 {
        return;
    }
    let p = theme.palette;
    // The square on whole pixels first, and the initials placed from it, so the two never round apart.
    let ppp = painter.pixels_per_point();
    let ppp = if ppp.is_finite() && ppp > 0.0 { ppp } else { 1.0 };
    let snap = |at: f32| (at * ppp).round() / ppp;
    let square = Rect::from_min_size(pos2(snap(rect.center().x - side / 2.0), snap(rect.center().y - side / 2.0)), vec2(side, side));
    let letters = initials(name, address);
    if letters == UNKNOWN {
        dashed_round(painter, square, p.line, CORNER);
    } else {
        painter.rect_filled(square, CORNER, p.stage);
    }
    let large = side >= AvatarSize::L.side(theme) - 0.5;
    let font = if large { theme.fonts.button.semibold() } else { theme.fonts.key };
    let ink = p.muted;
    let galley = painter.layout_job(egui::text::LayoutJob::single_section(letters, font.format(ink)));
    crate::draw::paint::run_centred(painter, square, galley, ink);
}

/// What an avatar shows for a person it knows nothing of.
const UNKNOWN: &str = "?";

/// An avatar of `side` points for `name` and `address` ([`AvatarSize::side`] gives the sheet's three); a screen
/// reader reads the name, the address when there is none, and "Unknown sender" when there is neither.
pub fn avatar(ui: &mut Ui, theme: &Theme, name: &str, address: &str, side: f32) -> Response {
    let side = if side.is_finite() { side.clamp(1.0, 512.0) } else { 1.0 };
    let (rect, response) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
    let spoken = if !name.trim().is_empty() {
        name
    } else if !address.trim().is_empty() {
        address
    } else {
        tr!("Unknown sender")
    };
    response.widget_info(|| WidgetInfo::labeled(Role::Image, true, spoken));
    if ui.is_rect_visible(rect) {
        paint(ui.painter(), theme, rect, name, address);
    }
    response
}

#[cfg(test)]
#[path = "avatar_tests.rs"]
mod tests;
