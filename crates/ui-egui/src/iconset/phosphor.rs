// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Phosphor (https://github.com/phosphor-icons/web, release 2.1.1, MIT): the code points of its Regular font,
//! `assets/icons/phosphor/Phosphor.ttf`.

#![deny(clippy::wildcard_enum_match_arm)]

use super::{IconId, Metrics};

/// A glyph fills the font's em square with a margin of its own, centred on the line's middle.
pub(super) const METRICS: Metrics = Metrics { size: 1.0, drop: 0.0 };

/// The Phosphor name of each id and its code point.
pub(super) const fn glyph(id: IconId) -> char {
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
        IconId::Previous => '\u{e138}',  // caret-left
        IconId::Next => '\u{e13a}',      // caret-right
        IconId::Globe => '\u{e288}',     // globe
        IconId::Changed => '\u{e094}',   // arrows-clockwise
        IconId::Older => '\u{e1a0}',     // clock-counter-clockwise
    }
}
