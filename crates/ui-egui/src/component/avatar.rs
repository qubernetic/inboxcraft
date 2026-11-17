// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! An avatar: a person's initials in `muted` on a `stage` square with corners of 2 px. Every person's avatar has the
//! same colours: colour belongs to accounts, never to people.

use egui::{Painter, Rect, Response, Role, Sense, Ui, WidgetInfo, vec2};

use crate::theme::Theme;

/// The corner radius of an avatar square: one of the two things that are not square.
pub const CORNER: f32 = 2.0;

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
    letters.or_else(|| letter(address.split('@').next().unwrap_or_default())).unwrap_or_else(|| "?".to_owned())
}

/// Paints the avatar of `name` and `address` into the square inside `rect`.
pub fn paint(painter: &Painter, theme: &Theme, rect: Rect, name: &str, address: &str) {
    let side = rect.width().min(rect.height());
    if !side.is_finite() || side <= 0.0 {
        return;
    }
    let p = theme.palette;
    painter.rect_filled(Rect::from_center_size(rect.center(), vec2(side, side)), CORNER, p.stage);
    let ink = p.muted;
    let font = theme.fonts.strong.sized(side * 0.4);
    let galley = painter.layout_no_wrap(initials(name, address), font.id(), ink);
    painter.galley(rect.center() - galley.size() / 2.0, galley, ink);
}

/// An avatar of `side` points for `name` and `address`; a screen reader reads the name, or the address when there is
/// none.
pub fn avatar(ui: &mut Ui, theme: &Theme, name: &str, address: &str, side: f32) -> Response {
    let side = if side.is_finite() { side.clamp(1.0, 512.0) } else { 1.0 };
    let (rect, response) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
    let spoken = if name.trim().is_empty() { address } else { name };
    response.widget_info(|| WidgetInfo::labeled(Role::Image, true, spoken));
    if ui.is_rect_visible(rect) {
        paint(ui.painter(), theme, rect, name, address);
    }
    response
}

#[cfg(test)]
#[path = "avatar_tests.rs"]
mod tests;
