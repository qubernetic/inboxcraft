// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Safe HTML display on any text, with and without consent for remote pictures: it never panics, stays within its
//! caps, keeps every link index in range, shows no remote picture without consent, gives a picture's size only when
//! both sides are above nought and within `MAX_SIDE`, and its text holds no control character but the line break and
//! tab and no invisible format character but the joiners. Every background it keeps carries an ink that reaches
//! 4.5:1 on it, every width is a share of 1 to 100 per cent or at most `MAX_SIDE` pixels, and a `javascript:` button
//! is a link that goes nowhere. A link text that reads as `bank.example` once invisible
//! characters are dropped and full-stop look-alikes are read as dots is flagged when the link goes to another host,
//! and not flagged when it goes to `bank.example`.

#![no_main]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use inboxcraft_html::{
    Block, Document, ImageSource, LEAST_CONTRAST, LinkTarget, Look, MAX_BLOCKS, MAX_LINKS, MAX_SIDE, Options, Paint, RemoteContent, Width, display,
};
use libfuzzer_sys::fuzz_target;

/// A character display text must not hold: a control character other than the line break and the tab, a bidi
/// override or isolate, or an invisible format character other than the joiners.
fn is_hidden(c: char) -> bool {
    (c.is_control() && c != '\n' && c != '\t')
        || matches!(
            c,
            '\u{ad}' | '\u{61c}' | '\u{180e}' | '\u{200b}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{206f}' | '\u{feff}'
        )
}

/// Characters a reader does not see in a link text.
const INVISIBLE: [&str; 12] =
    ["&shy;", "&#x61C;", "&#x180E;", "&#x200B;", "&#x200C;", "&#x200D;", "&#x200E;", "&#x200F;", "&#x2060;", "&#x2064;", "&#xFEFF;", "\u{200b}"];

/// Full stops a reader takes for a dot.
const DOTS: [&str; 5] = [".", "&#x3002;", "&#xFF0E;", "&#xFF61;", "\u{3002}"];

/// `bank.example` with the invisible characters and the full stop the input picks.
fn disguised(data: &[u8]) -> String {
    let mut text = String::new();
    for (i, c) in "bank.example".chars().enumerate() {
        let pick = usize::from(data.get(i).copied().unwrap_or(0));
        if pick % 2 == 1
            && let Some(s) = INVISIBLE.get(pick / 2 % INVISIBLE.len())
        {
            text.push_str(s);
        }
        match DOTS.get(pick % DOTS.len()) {
            Some(dot) if c == '.' => text.push_str(dot),
            _ => text.push(c),
        }
    }
    text
}

/// `text` as HTML's characters, not its markup: no tag and no way out of an attribute's quotes.
fn escaped(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// A kept background is readable: its ink reaches the contrast the app asks of text.
fn readable(paint: Option<Paint>) {
    assert!(paint.is_none_or(|paint| paint.fill.contrast(paint.ink) >= LEAST_CONTRAST), "{paint:?}");
}

/// A width within its caps.
fn bounded(width: Option<Width>) {
    assert!(
        width.is_none_or(|width| match width {
            Width::Pixels(pixels) => pixels > 0 && pixels <= MAX_SIDE,
            Width::Share(share) => share > 0 && share <= 100,
        }),
        "{width:?}"
    );
}

fn check(blocks: &[Block], doc: &Document, consent: bool) {
    for block in blocks {
        if let Block::Text { look: Look { background, .. }, .. } | Block::Image { look: Look { background, .. }, .. } | Block::Table { look: Look { background, .. }, .. } = block {
            readable(*background);
        }
        match block {
            Block::Text { spans, .. } => {
                for span in spans {
                    assert!(span.link.is_none_or(|i| i < doc.links.len()));
                    assert!(!span.text.chars().any(is_hidden), "{:?}", span.text);
                }
            }
            Block::Image { image, .. } => {
                assert!(image.link.is_none_or(|i| i < doc.links.len()));
                assert!(consent || !matches!(image.source, ImageSource::Remote(_)));
                assert!(image.size.is_none_or(|(w, h)| w > 0 && h > 0 && w <= MAX_SIDE && h <= MAX_SIDE), "{:?}", image.size);
            }
            Block::Table { rows, width, .. } => {
                bounded(*width);
                for cell in rows.iter().flatten() {
                    readable(cell.background);
                    bounded(cell.width);
                    check(&cell.blocks, doc, consent);
                }
            }
            Block::Rule { .. } => {}
        }
    }
}

fuzz_target!(|data: &[u8]| {
    let html = String::from_utf8_lossy(data);
    for consent in [false, true] {
        let remote_content = if consent { RemoteContent::Allowed } else { RemoteContent::Blocked };
        let doc = display(&html, &Options { remote_content });
        assert!(doc.blocks.len() <= MAX_BLOCKS && doc.links.len() <= MAX_LINKS);
        check(&doc.blocks, &doc, consent);
        doc.links.iter().for_each(|link| readable(link.fill));
    }
    // The input, escaped, as a button's action, script and words: the link the button itself makes (the first) goes
    // nowhere. A link inside the input may be a button that goes somewhere; escaped, it cannot be made here.
    let escaped = escaped(&html);
    let doc = display(&format!("<button formaction=\"javascript:{escaped}\" onclick=\"{escaped}\">{escaped}</button>"), &Options::default());
    assert!(doc.links.first().is_some_and(|link| link.button && link.target == LinkTarget::Refused), "{:?}", doc.links);
    let text = disguised(data);
    for (href, mismatch) in [("https://evil.example/", true), ("https://bank.example/", false)] {
        let doc = display(&format!("<a href=\"{href}\">{text}</a>"), &Options::default());
        assert!(doc.links.first().is_some_and(|l| l.mismatch == mismatch), "{text} -> {href}");
    }
});
