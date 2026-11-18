// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! An HTML message as a text-only program shows it, for the reader's `plain text` view of a message that has no text
//! part of its own: its blocks as lines, a quote's lines after `> `, a list's items after their marks, a table's rows
//! as lines of cells, a link's address after its words. The words are data: nothing in them is followed. The text
//! stops at [`MAX_TEXT`] bytes, where the reader cuts a text part too.

use inboxcraft_html::{Block, Document, LinkTarget, Marker, TextKind};

use crate::quote::MAX_TEXT;

/// How deep tables inside tables are followed; deeper ones are left out.
const DEEPEST: usize = 16;

/// `document` as plain text, at most [`MAX_TEXT`] bytes and a line more.
pub(crate) fn plain(document: &Document) -> String {
    let mut out = String::new();
    blocks(document, &document.blocks, 0, &mut out);
    out.truncate(out.trim_end().len());
    out.push('\n');
    out
}

/// Appends `list` to `out`, tables `depth` deep.
fn blocks(document: &Document, list: &[Block], depth: usize, out: &mut String) {
    let mut in_list = false;
    for block in list {
        if out.len() > MAX_TEXT {
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
        let mut text = String::new();
        match block {
            Block::Text { kind, spans, .. } => {
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
                    text.push_str(crate::quote::cut(&span.text, MAX_TEXT.saturating_sub(text.len())));
                    if text.len() >= MAX_TEXT {
                        break;
                    }
                }
                address(document, link, &mut text);
            }
            Block::Rule { .. } => text.push_str("----------"),
            Block::Image { image, .. } if !image.alt.trim().is_empty() => {
                text.push('[');
                text.push_str(image.alt.trim());
                text.push(']');
            }
            Block::Image { .. } => continue,
            Block::Table { rows, .. } if depth < DEEPEST => {
                let lines: Vec<String> = rows
                    .iter()
                    .map(|row| {
                        let cells: Vec<String> = row
                            .iter()
                            .map(|cell| {
                                let mut inner = String::new();
                                blocks(document, &cell.blocks, depth.saturating_add(1), &mut inner);
                                inner.split_whitespace().collect::<Vec<_>>().join(" ")
                            })
                            .collect();
                        cells.join("  ")
                    })
                    .collect();
                text.push_str(&lines.join("\n"));
            }
            Block::Table { .. } => continue,
        }
        let mark = "> ".repeat(usize::from(quote));
        for line in text.split('\n') {
            out.push_str(&mark);
            out.push_str(line);
            out.push('\n');
        }
        if !listed {
            out.push_str(mark.trim_end());
            out.push('\n');
        }
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
