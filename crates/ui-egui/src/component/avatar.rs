// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! An avatar: a person's initials on a round field of colour. The colour comes from the theme's avatar palette and
//! is picked from the address alone (case and surrounding spaces aside), so one person always has one colour, on
//! every screen and in every session. The initials are white or the deep ink, whichever reads on the colour.

use egui::{Color32, Painter, Rect, Response, Role, Sense, Ui, WidgetInfo, vec2};

use crate::theme::{Font, Theme, ink_on};

/// A stable 64-bit hash of `text` (FNV-1a), the same on every platform and in every version.
fn hash(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3))
}

/// The avatar colour of `address`.
#[must_use]
pub fn colour(theme: &Theme, address: &str) -> Color32 {
    let colours = &theme.palette.avatars;
    let len = u64::try_from(colours.len()).unwrap_or(1).max(1);
    let index = hash(&address.trim().to_lowercase()) % len;
    usize::try_from(index).ok().and_then(|at| colours.get(at)).copied().unwrap_or(theme.palette.accent)
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
    letters.or_else(|| letter(address.split('@').next().unwrap_or_default())).unwrap_or_else(|| "?".to_owned())
}

/// Paints the avatar of `name` and `address` into the circle inside `rect`.
pub fn paint(painter: &Painter, theme: &Theme, rect: Rect, name: &str, address: &str) {
    let side = rect.width().min(rect.height());
    if !side.is_finite() || side <= 0.0 {
        return;
    }
    let fill = colour(theme, address);
    painter.circle_filled(rect.center(), side / 2.0, fill);
    let ink = ink_on(fill);
    let font = Font { size: side * 0.4, ..theme.fonts.strong };
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
