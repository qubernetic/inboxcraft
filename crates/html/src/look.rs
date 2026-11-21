// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The little of a message's look the fitted view keeps: a coloured background where text can be read on it, the
//! alignment of its text, the widths of its tables, and whether a link is drawn as a button.
//!
//! Only an element's own attributes (`align`, `bgcolor`, `width`, `role`) and its inline `style` are read, one
//! declaration after another; a `<style>` block, classes and selectors never are. Every value is parsed in one pass
//! over its text and kept within its caps.

use crate::document::{Align, Paint, Rgb, Width};
use crate::tokenizer::Tag;

/// The contrast text must reach on a kept background (WCAG 2, the app's own rule for text on every ground).
pub const LEAST_CONTRAST: f32 = 4.5;

/// The dark ink set on a light background the message keeps.
pub const DARK_INK: Rgb = Rgb { r: 0x19, g: 0x19, b: 0x19 };

/// The light ink set on a dark background the message keeps.
pub const LIGHT_INK: Rgb = Rgb { r: 0xFF, g: 0xFF, b: 0xFF };

/// How far apart a background's strongest and weakest channel must be for it to count as coloured: a white, grey
/// or black page is the sender's ground, not a part's colour, and the fitted view sets the message on its own.
const LEAST_CHROMA: u8 = 24;

/// What an element says of a property: nothing, that it is transparent (or a value not understood), or a value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Said<T> {
    /// The element does not set it: what is around it holds.
    Nothing,
    /// The element sets it to a value the fitted view does not keep: nothing of it is drawn.
    Dropped,
    /// The element sets it.
    Kept(T),
}

/// The background `colour` as the fitted view keeps it: a coloured one with the ink that reaches
/// [`LEAST_CONTRAST`] on it; `None` for a white, grey or black one, or one no ink can be read on.
#[must_use]
pub fn kept(colour: Rgb) -> Option<Paint> {
    let (high, low) = (colour.r.max(colour.g).max(colour.b), colour.r.min(colour.g).min(colour.b));
    if high.saturating_sub(low) < LEAST_CHROMA {
        return None;
    }
    let (dark, light) = (colour.contrast(DARK_INK), colour.contrast(LIGHT_INK));
    let (ink, best) = if dark >= light { (DARK_INK, dark) } else { (LIGHT_INK, light) };
    (best >= LEAST_CONTRAST).then_some(Paint { fill: colour, ink })
}

/// The declarations of an inline style, in order: each property in lower case, its value trimmed and without
/// `!important`.
fn declarations(style: &str) -> impl DoubleEndedIterator<Item = (String, &str)> {
    style.split(';').filter_map(|declaration| {
        let (property, value) = declaration.split_once(':')?;
        let value = value.trim();
        let value = value.strip_suffix("!important").unwrap_or(value).trim();
        Some((property.trim().to_ascii_lowercase(), value))
    })
}

/// The last declaration of `property` in `tag`'s inline style.
fn declared(tag: &Tag<'_>, property: &str) -> Option<String> {
    let style = tag.attr("style")?;
    declarations(&style).rev().find(|(name, _)| name == property).map(|(_, value)| value.to_owned())
}

/// The alignment of an element's text: its `text-align` declaration, else its `align` attribute.
pub(crate) fn align(tag: &Tag<'_>) -> Said<Align> {
    let value = declared(tag, "text-align").or_else(|| tag.attr("align"));
    let Some(value) = value else { return Said::Nothing };
    match value.trim().to_ascii_lowercase().as_str() {
        "left" | "start" | "justify" | "-webkit-left" | "-moz-left" => Said::Kept(Align::Left),
        "center" | "centre" | "middle" | "-webkit-center" | "-moz-center" => Said::Kept(Align::Centre),
        "right" | "end" | "-webkit-right" | "-moz-right" => Said::Kept(Align::Right),
        _ => Said::Nothing,
    }
}

/// Whether a table stands centred: `align="center"` or `margin: 0 auto` (and the like).
pub(crate) fn table_align(tag: &Tag<'_>) -> Said<Align> {
    if let Some(margin) = declared(tag, "margin") {
        let parts: Vec<&str> = margin.split_whitespace().take(5).collect();
        if matches!(parts.as_slice(), [_, "auto"] | [_, "auto", _] | [_, "auto", _, "auto"] | ["auto"]) {
            return Said::Kept(Align::Centre);
        }
    }
    match tag.attr("align").map(|value| value.trim().to_ascii_lowercase()).as_deref() {
        Some("center" | "middle") => Said::Kept(Align::Centre),
        Some("right") => Said::Kept(Align::Right),
        Some("left") => Said::Kept(Align::Left),
        _ => Said::Nothing,
    }
}

/// The background an element sets: its `background-color` or `background` declaration (the last one), else its
/// `bgcolor` attribute; kept by [`kept`] or dropped. Transparent says nothing.
pub(crate) fn background(tag: &Tag<'_>) -> Said<Paint> {
    let mut said = None;
    if let Some(style) = tag.attr("style") {
        for (property, value) in declarations(&style) {
            match property.as_str() {
                "background-color" => said = Some(colour(value)),
                "background" => said = Some(value.split_whitespace().map(colour).find(|found| *found != Said::Dropped).unwrap_or(Said::Dropped)),
                _ => {}
            }
        }
    }
    let said = said.or_else(|| tag.attr("bgcolor").map(|value| colour(&value))).unwrap_or(Said::Nothing);
    match said {
        Said::Kept(rgb) => kept(rgb).map_or(Said::Dropped, Said::Kept),
        Said::Nothing => Said::Nothing,
        Said::Dropped => Said::Dropped,
    }
}

/// A colour as CSS or `bgcolor` writes it: `#rgb`, `#rrggbb` (with or without the `#`), `rgb(r, g, b)` with whole
/// numbers, or one of the basic colour names. Transparent says nothing; anything else is dropped.
pub(crate) fn colour(value: &str) -> Said<Rgb> {
    let value = value.trim().to_ascii_lowercase();
    if matches!(value.as_str(), "transparent" | "inherit" | "initial" | "unset" | "none" | "") {
        return Said::Nothing;
    }
    let hex = value.strip_prefix('#').unwrap_or(&value);
    if let Some(rgb) = from_hex(hex) {
        return Said::Kept(rgb);
    }
    if let Some(inside) = value.strip_prefix("rgb(").and_then(|rest| rest.strip_suffix(')')) {
        return from_numbers(inside).map_or(Said::Dropped, Said::Kept);
    }
    named(&value).map_or(Said::Dropped, Said::Kept)
}

/// Three or six hexadecimal digits.
fn from_hex(hex: &str) -> Option<Rgb> {
    if !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let digit = |at: usize| hex.get(at..at.saturating_add(1)).and_then(|d| u8::from_str_radix(d, 16).ok());
    match hex.len() {
        3 => Some(Rgb { r: digit(0)?.saturating_mul(17), g: digit(1)?.saturating_mul(17), b: digit(2)?.saturating_mul(17) }),
        6 => {
            let pair = |at: usize| hex.get(at..at.saturating_add(2)).and_then(|d| u8::from_str_radix(d, 16).ok());
            Some(Rgb { r: pair(0)?, g: pair(2)?, b: pair(4)? })
        }
        _ => None,
    }
}

/// `r, g, b` (commas or spaces between), each a whole number from 0 to 255.
fn from_numbers(inside: &str) -> Option<Rgb> {
    let mut parts = inside.split(|c: char| c == ',' || c.is_whitespace()).filter(|part| !part.is_empty());
    let mut next = || -> Option<u8> {
        let part = parts.next()?;
        if part.len() > 3 {
            return None;
        }
        part.parse::<u8>().ok()
    };
    let rgb = Rgb { r: next()?, g: next()?, b: next()? };
    parts.next().is_none().then_some(rgb)
}

/// The basic colour names of CSS.
fn named(name: &str) -> Option<Rgb> {
    let (r, g, b) = match name {
        "black" => (0, 0, 0),
        "white" => (255, 255, 255),
        "gray" | "grey" => (128, 128, 128),
        "silver" => (192, 192, 192),
        "red" => (255, 0, 0),
        "maroon" => (128, 0, 0),
        "orange" => (255, 165, 0),
        "yellow" => (255, 255, 0),
        "olive" => (128, 128, 0),
        "lime" => (0, 255, 0),
        "green" => (0, 128, 0),
        "aqua" | "cyan" => (0, 255, 255),
        "teal" => (0, 128, 128),
        "blue" => (0, 0, 255),
        "navy" => (0, 0, 128),
        "fuchsia" | "magenta" => (255, 0, 255),
        "purple" => (128, 0, 128),
        _ => return None,
    };
    Some(Rgb { r, g, b })
}

/// The width an element asks for: its `width` declaration, else its `width` attribute; whole pixels up to
/// [`crate::MAX_SIDE`] or a share of 1 to 100 per cent.
pub(crate) fn width(tag: &Tag<'_>) -> Option<Width> {
    let value = declared(tag, "width").or_else(|| tag.attr("width"))?;
    let value = value.trim();
    if let Some(share) = value.strip_suffix('%') {
        let share = share.trim();
        let whole = share.split_once('.').map_or(share, |(whole, _)| whole);
        if whole.is_empty() || whole.len() > 3 || !whole.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        return whole.parse::<u8>().ok().filter(|share| *share > 0).map(|share| Width::Share(share.min(100)));
    }
    let digits = value.strip_suffix("px").unwrap_or(value).trim();
    if digits.is_empty() || digits.len() > 9 || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    digits.parse::<u32>().ok().filter(|pixels| *pixels > 0).map(|pixels| Width::Pixels(pixels.min(crate::MAX_SIDE)))
}

/// Whether a link is styled as a button: `role="button"`, or a background it sets with padding or as a box of its own
/// (`display: inline-block` or `block`).
pub(crate) fn is_button(tag: &Tag<'_>, background: Said<Paint>) -> bool {
    if tag.attr("role").is_some_and(|role| role.trim().eq_ignore_ascii_case("button")) {
        return true;
    }
    if background == Said::Nothing {
        return false;
    }
    let Some(style) = tag.attr("style") else { return false };
    declarations(&style).any(|(property, value)| match property.as_str() {
        "padding" | "padding-top" | "padding-left" | "padding-right" | "padding-bottom" => value
            .split_whitespace()
            .any(|part| part.trim_end_matches(|c: char| c.is_ascii_alphabetic() || c == '%').parse::<f32>().is_ok_and(|n| n > 0.0)),
        "display" => matches!(value.to_ascii_lowercase().as_str(), "inline-block" | "block"),
        _ => false,
    })
}

#[cfg(test)]
#[path = "look_tests.rs"]
mod tests;
