// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Lucide (https://github.com/lucide-icons/lucide, release 1.70.0, ISC; the icons derived from Feather MIT): the code
//! points of its font in `assets/icons/lucide/`. Lucide has lines only, so the styles that Phosphor draws solid get a
//! filled disc behind the lines instead: faint for Duotone, stronger for Fill. Callers draw active states in the
//! accent colour as well.

#![deny(clippy::wildcard_enum_match_arm)]

use super::{BACK_OPACITY, Glyph, IconId, Metrics, Style};
use crate::theme::IconFont;

/// The opacity of the disc behind an icon in [`Style::Fill`].
pub(super) const FILL_DISC: f32 = 0.38;

/// A glyph fills the font's em square to its edge and sits a little above the line's middle: drawn a little smaller
/// and moved down, it has Phosphor's margin and sits in the middle (measured in `iconset/tests.rs`).
pub(super) const METRICS: Metrics = Metrics { size: 0.92, drop: 0.045 };

/// The Lucide name of each id and its code point.
const fn point(id: IconId) -> char {
    match id {
        IconId::Inbox => '\u{e0f7}',    // inbox
        IconId::Done => '\u{e226}',     // circle-check
        IconId::Archive => '\u{e041}',  // archive
        IconId::Delete => '\u{e18e}',   // trash-2
        IconId::Snooze => '\u{e087}',   // clock
        IconId::Pin => '\u{e259}',      // pin
        IconId::Move => '\u{e0d7}',     // folder
        IconId::Reply => '\u{e22a}',    // reply
        IconId::ReplyAll => '\u{e22b}', // reply-all
        IconId::Forward => '\u{e229}',  // forward
        IconId::Send => '\u{e152}',     // send
        IconId::Search => '\u{e151}',   // search
        IconId::Compose => '\u{e4f0}',  // pencil-line
        IconId::Calendar => '\u{e063}', // calendar
        IconId::Attach => '\u{e12d}',   // paperclip
        IconId::Settings => '\u{e154}', // settings
        IconId::Notify => '\u{e059}',   // bell
        IconId::People => '\u{e1a4}',   // users
        IconId::Flag => '\u{e0d1}',     // flag
        IconId::More => '\u{e0b6}',     // ellipsis
        IconId::File => '\u{e0c0}',     // file
        IconId::Failed => '\u{e084}',   // circle-x
        IconId::Waiting => '\u{e076}',  // circle
        IconId::Skipped => '\u{e07e}',  // circle-minus
        IconId::Home => '\u{e0f5}',     // home
        IconId::Mail => '\u{e10f}',     // mail
        IconId::Add => '\u{e13d}',      // plus
        IconId::Junk => '\u{e077}',     // alert-circle
        IconId::MarkRead => '\u{e363}', // mail-open
        IconId::Lock => '\u{e10b}',     // lock
        IconId::Palette => '\u{e1dd}',  // palette
        IconId::At => '\u{e04e}',       // at-sign
        IconId::Funnel => '\u{e0dc}',   // filter
        IconId::Keyboard => '\u{e284}', // keyboard
        IconId::Shield => '\u{e1ff}',   // shield-check
        IconId::Info => '\u{e0f9}',     // info
        IconId::Close => '\u{e1b2}',    // x
    }
}

/// The glyphs of `id` in `style`.
pub(super) fn glyph(id: IconId, style: Style) -> Glyph {
    let disc = match style {
        Style::Regular => 0.0,
        Style::Fill => FILL_DISC,
        Style::Duotone => BACK_OPACITY,
    };
    Glyph { face: IconFont::Lucide, layers: vec![(point(id), 1.0)], disc }
}
