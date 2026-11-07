// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Phosphor (https://github.com/phosphor-icons/web, release 2.1.1, MIT): the code points of its Regular, Fill and
//! Duotone fonts in `assets/icons/phosphor/`. Regular and Fill share a code point per icon; Duotone has two, the
//! faint back and the lines.

#![deny(clippy::wildcard_enum_match_arm)]

use super::{BACK_OPACITY, Glyph, IconId, Metrics, Style};
use crate::theme::IconFont;

/// A glyph fills the font's em square with a margin of its own, centred on the line's middle.
pub(super) const METRICS: Metrics = Metrics { size: 1.0, drop: 0.0 };

/// The Phosphor name of each id and its Regular (and Fill) code point; Duotone draws it as the back and the next
/// code point as the lines.
const fn point(id: IconId) -> char {
    match id {
        IconId::Inbox => '\u{e4aa}',     // tray
        IconId::Done => '\u{e184}',      // check-circle
        IconId::Archive => '\u{e00c}',   // archive
        IconId::Delete => '\u{e4a6}',    // trash
        IconId::Snooze => '\u{e19a}',    // clock
        IconId::Pin => '\u{e3e2}',       // push-pin
        IconId::Move => '\u{e25a}',      // folder-simple
        IconId::Reply => '\u{e024}',     // arrow-bend-up-left
        IconId::ReplyAll => '\u{e03a}',  // arrow-bend-double-up-left
        IconId::Forward => '\u{e026}',   // arrow-bend-up-right
        IconId::Send => '\u{e398}',      // paper-plane-tilt
        IconId::Search => '\u{e30c}',    // magnifying-glass
        IconId::Compose => '\u{ebc6}',   // pencil-simple-line
        IconId::Calendar => '\u{e10a}',  // calendar-blank
        IconId::Attach => '\u{e39a}',    // paperclip
        IconId::Settings => '\u{e272}',  // gear-six
        IconId::Notify => '\u{e0ce}',    // bell
        IconId::People => '\u{e4d6}',    // users
        IconId::Flag => '\u{e244}',      // flag
        IconId::More => '\u{e1fe}',      // dots-three
        IconId::File => '\u{e230}',      // file
        IconId::Failed => '\u{e4f8}',    // x-circle
        IconId::Waiting => '\u{e18a}',   // circle
        IconId::Skipped => '\u{e32c}',   // minus-circle
        IconId::Home => '\u{e2c2}',      // house
        IconId::Mail => '\u{e218}',      // envelope-simple
        IconId::Add => '\u{e3d4}',       // plus
        IconId::Junk => '\u{e4e2}',      // warning-circle
        IconId::MarkRead => '\u{e21a}',  // envelope-simple-open
        IconId::Lock => '\u{e308}',      // lock-simple
        IconId::Palette => '\u{e6c8}',   // palette
        IconId::At => '\u{e0ac}',        // at
        IconId::Funnel => '\u{e266}',    // funnel
        IconId::Keyboard => '\u{e2d8}',  // keyboard
        IconId::Shield => '\u{e40c}',    // shield-check
        IconId::Info => '\u{e2ce}',      // info
        IconId::Close => '\u{e4f6}',     // x
        IconId::Back => '\u{e058}',      // arrow-left
        IconId::Caret => '\u{e136}',     // caret-down
        IconId::Newspaper => '\u{e344}', // newspaper
    }
}

/// The glyphs of `id` in `style`.
pub(super) fn glyph(id: IconId, style: Style) -> Glyph {
    let point = point(id);
    match style {
        Style::Regular => Glyph { face: IconFont::PhosphorRegular, layers: vec![(point, 1.0)], disc: 0.0 },
        Style::Fill => Glyph { face: IconFont::PhosphorFill, layers: vec![(point, 1.0)], disc: 0.0 },
        Style::Duotone => {
            let lines = char::from_u32(u32::from(point).saturating_add(1)).unwrap_or(point);
            Glyph { face: IconFont::PhosphorDuotone, layers: vec![(point, BACK_OPACITY), (lines, 1.0)], disc: 0.0 }
        }
    }
}
