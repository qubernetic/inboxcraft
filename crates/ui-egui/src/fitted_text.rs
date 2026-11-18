// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! An HTML message as a text-only program shows it, for the reader's `plain text` view of a message that has no text
//! part of its own: its blocks as lines, a quote's lines after `> `, a list's items after their marks, a table's rows
//! as lines of cells, a link's address after its words. The words are data: nothing in them is followed. The text
//! stops at [`MAX_TEXT`] bytes, where the reader cuts a text part too, and is never built far past it; a quote deeper
//! than four reads after four marks.

use inboxcraft_html::{Block, Document, LinkTarget, Marker, TextKind};

use crate::quote::MAX_TEXT;

/// How deep tables inside tables are followed; deeper ones are left out.
const DEEPEST: usize = 16;

/// The most quote marks before a line: a deeper quote reads as deep as the fitted view draws it.
const MOST_MARKS: u8 = 4;

/// `document` as plain text, at most [`MAX_TEXT`] bytes.
pub(crate) fn plain(document: &Document) -> String {
    let mut out = String::new();
    blocks(document, &document.blocks, 0, &mut out, MAX_TEXT);
    let kept = crate::quote::cut(out.trim_end(), MAX_TEXT.saturating_sub(1)).len();
    out.truncate(kept);
    out.push('\n');
    out
}

/// Appends `list` to `out`, tables `depth` deep, until `out` holds `most` bytes: the cap is checked line by line and
/// cell by cell, so nothing is built far past it.
fn blocks(document: &Document, list: &[Block], depth: usize, out: &mut String, most: usize) {
    let mut in_list = false;
    for block in list {
        if out.len() >= most {
            return;
        }
        let quote = match block {
            Block::Text { quote, .. } | Block::Rule { quote } | Block::Image { quote, .. } | Block::Table { quote, .. } => *quote,
        };
        let listed = matches!(block, Block::Text { kind: TextKind::ListItem { .. }, .. });
        if in_list && !listed {
            out.push('\n');
        }
        in_list = listed;
        let mark = "> ".repeat(usize::from(quote.min(MOST_MARKS)));
        match block {
            Block::Text { kind, spans, .. } => {
                let room = most.saturating_sub(out.len());
                let mut text = String::new();
                if let TextKind::ListItem { depth, marker } = kind {
                    text.push_str(&"  ".repeat(usize::from((*depth).clamp(1, 6)).saturating_sub(1)));
                    match marker {
                        Marker::Bullet => text.push_str("• "),
                        Marker::Number(n) => text.push_str(&format!("{n}. ")),
                    }
                }
                let mut link = None;
                for span in spans {
                    if span.link != link {
                        address(document, link, &mut text);
                        link = span.link;
                    }
                    text.push_str(crate::quote::cut(&span.text, room.saturating_sub(text.len())));
                    if text.len() >= room {
                        break;
                    }
                }
                address(document, link, &mut text);
                lines(out, &mark, &text, most);
            }
            Block::Rule { .. } => lines(out, &mark, "----------", most),
            Block::Image { image, .. } if !image.alt.trim().is_empty() => {
                lines(out, &mark, &format!("[{}]", crate::quote::cut(image.alt.trim(), most.saturating_sub(out.len()))), most);
            }
            Block::Image { .. } => continue,
            Block::Table { rows, .. } if depth < DEEPEST => {
                'rows: for row in rows {
                    if out.len() >= most {
                        break;
                    }
                    let mut line = String::new();
                    for (index, cell) in row.iter().enumerate() {
                        let room = most.saturating_sub(out.len().saturating_add(mark.len()).saturating_add(line.len()));
                        if room == 0 {
                            lines(out, &mark, &line, most);
                            break 'rows;
                        }
                        let mut inner = String::new();
                        blocks(document, &cell.blocks, depth.saturating_add(1), &mut inner, room);
                        if index > 0 {
                            line.push_str("  ");
                        }
                        for (at, word) in inner.split_whitespace().enumerate() {
                            if at > 0 {
                                line.push(' ');
                            }
                            line.push_str(word);
                        }
                    }
                    lines(out, &mark, &line, most);
                }
            }
            Block::Table { .. } => continue,
        }
        if !listed && out.len() < most {
            out.push_str(mark.trim_end());
            out.push('\n');
        }
    }
}

/// Appends each line of `text` after `mark` to `out`, until `out` holds `most` bytes.
fn lines(out: &mut String, mark: &str, text: &str, most: usize) {
    for line in text.split('\n') {
        if out.len() >= most {
            return;
        }
        out.push_str(mark);
        out.push_str(crate::quote::cut(line, most.saturating_sub(out.len())));
        out.push('\n');
    }
}

/// Appends the address of link `link` of `document` after its words, unless the words are the address.
fn address(document: &Document, link: Option<usize>, text: &mut String) {
    let Some(link) = link.and_then(|index| document.links.get(index)) else { return };
    let url = match &link.target {
        LinkTarget::Web { url, .. } | LinkTarget::Mail { url, .. } => url,
        LinkTarget::Refused => return,
    };
    if !text.ends_with(url.as_str()) {
        text.push_str(" <");
        text.push_str(url);
        text.push('>');
    }
}

#[cfg(test)]
#[path = "fitted_text_tests.rs"]
mod tests;
