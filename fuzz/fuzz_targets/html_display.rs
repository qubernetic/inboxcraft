// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Safe HTML display on any text, with and without consent for remote pictures: it never panics, stays within its
//! caps, keeps every link index in range, shows no remote picture without consent, and its text holds no control
//! character but the line break and tab and no invisible format character but the joiners. A link text that reads as
//! `bank.example` once invisible characters are dropped and full-stop look-alikes are read as dots is flagged when
//! the link goes to another host, and not flagged when it goes to `bank.example`.

#![no_main]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use inboxcraft_html::{Block, Document, ImageSource, MAX_BLOCKS, MAX_LINKS, Options, RemoteContent, display};
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

fn check(blocks: &[Block], doc: &Document, consent: bool) {
    for block in blocks {
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
            }
            Block::Table { rows, .. } => rows.iter().flatten().for_each(|cell| check(&cell.blocks, doc, consent)),
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
    }
    let text = disguised(data);
    for (href, mismatch) in [("https://evil.example/", true), ("https://bank.example/", false)] {
        let doc = display(&format!("<a href=\"{href}\">{text}</a>"), &Options::default());
        assert!(doc.links.first().is_some_and(|l| l.mismatch == mismatch), "{text} -> {href}");
    }
});
