// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Safe HTML display on any text, with and without consent for remote pictures: it never panics, stays within its
//! caps, keeps every link index in range, shows no remote picture without consent, and its text holds no control
//! character but the line break and tab and no bidi override.

#![no_main]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use inboxcraft_html::{Block, Document, ImageSource, MAX_BLOCKS, MAX_LINKS, Options, RemoteContent, display};
use libfuzzer_sys::fuzz_target;

/// A character display text must not hold: a control character other than the line break and the tab, or a bidi
/// override or isolate.
fn is_hidden(c: char) -> bool {
    (c.is_control() && c != '\n' && c != '\t') || matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
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
});
