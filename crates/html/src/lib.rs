// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Safe HTML: message HTML turned into something that can be shown without running anything.
//!
//! Layer 1. Mail HTML is hostile: it comes from strangers, it is often broken, and it may try to run
//! scripts, submit forms, load trackers or hide where a link goes. This crate never renders it.
//! [`display`] turns it into display [`Block`]s (paragraphs, headings, list items, quotes, pictures,
//! tables) that a front end draws itself:
//!
//! - nothing runs: scripts, style sheets, frames, objects, SVG and event attributes are dropped; the
//!   only attributes read are a link's `href`, a picture's `src` and `alt`, an ordered list's
//!   `start`, and `hidden` and `style` for whether an element is hidden (`display: none`,
//!   `visibility: hidden`), which is all of the style that is read;
//! - forms keep their visible text and lose everything that could submit;
//! - remote pictures are [`ImageSource::Blocked`] until the owner gives consent
//!   ([`Options::remote_content`]); `cid:` pictures point at the message's own parts;
//! - every link's target is exposed ([`Link`]), only `http`, `https` and `mailto` targets are kept,
//!   and a link whose text names another place than its target is flagged ([`Link::mismatch`]),
//!   invisible characters and full-stop look-alikes in the text making no difference;
//! - the work is bounded: input over [`MAX_INPUT`] bytes is cut, nesting over [`MAX_DEPTH`] is
//!   flattened, tables are capped ([`MAX_TABLE_DEPTH`], [`MAX_TABLE_ROWS`], [`MAX_TABLE_COLUMNS`]),
//!   and so are blocks, links and pictures.
//!
//! The tokenizer and the character references are adapted from UCS-Care-Group/mailcraft
//! (`crates/html`, MIT OR Apache-2.0; see `PROVENANCE.md`).
//!
//! ```
//! use inboxcraft_html::{display, Block, ImageSource, Options};
//!
//! let doc = display("<p>Hi <script>steal()</script><img src=https://t.example/p.gif>", &Options::default());
//! assert_eq!(doc.blocks[0].text(), "Hi");
//! assert!(matches!(&doc.blocks[1], Block::Image { image, .. } if image.source == ImageSource::Blocked));
//! assert_eq!(doc.remote_blocked, 1);
//! ```

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod build;
mod document;
mod entities;
mod link;
mod tokenizer;

pub use document::{Block, Cell, Document, Image, ImageSource, Link, LinkTarget, Marker, Options, RemoteContent, Span, TextKind};

/// Longest input [`display`] reads, in bytes; the rest is cut at a character boundary and
/// [`Document::truncated`] is set.
pub const MAX_INPUT: usize = 2 * 1024 * 1024;

/// Most elements open at once; deeper start tags are ignored (their text stays).
pub const MAX_DEPTH: usize = 256;

/// Most blocks one document gives, counting those inside table cells; the rest is dropped and
/// [`Document::truncated`] is set.
pub const MAX_BLOCKS: usize = 20_000;

/// Most links one document keeps; further links show as plain text.
pub const MAX_LINKS: usize = 10_000;

/// Most pictures one document keeps; further pictures are dropped.
pub const MAX_IMAGES: usize = 5_000;

/// Most tables inside one another; a deeper table's cells become plain paragraphs of the outer cell.
pub const MAX_TABLE_DEPTH: usize = 8;

/// Most rows one table keeps; the content of further rows goes after the table.
pub const MAX_TABLE_ROWS: usize = 1_000;

/// Most cells one row keeps; the content of further cells goes after the table.
pub const MAX_TABLE_COLUMNS: usize = 100;

/// Longest link target kept, in bytes; a longer one is refused.
pub const MAX_URL: usize = 8 * 1024;

/// Turn message HTML into display blocks. Takes any string, never panics, and is linear in the
/// input.
#[must_use]
pub fn display(html: &str, options: &Options) -> Document {
    build::build(html, options)
}

/// Cut `s` to at most `max` bytes, at a character boundary.
pub(crate) fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    s.get(..end).unwrap_or("")
}

#[cfg(test)]
mod tests;
