// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! [`page`]: a [`Document`] written out again as a page of HTML a browser can show, nothing in it that runs or loads.
//!
//! The page is made from the display blocks, never from the message's own HTML: only the elements and the few inline
//! styles written here can be in it. No address is written at all: a picture is a box with its alternative text (its
//! size when its tag gave one), a link is its words with the host or mail address it goes to in a tooltip, and a
//! `Content-Security-Policy` meta element forbids every load and script besides, with no referrer.

use std::fmt::Write as _;

use crate::{Align, Block, Cell, Document, Image, Link, LinkTarget, Look, MAX_SIDE, Marker, Paint, Span, TextKind, Width};

/// The longest title written, in characters.
pub const MAX_TITLE: usize = 200;

/// Most tables written inside one another; the content of deeper ones is written as plain paragraphs.
pub const MAX_NESTING: usize = crate::MAX_TABLE_DEPTH;

/// The page's head: no load of any kind, no referrer, and the look of the boxes that stand for pictures.
const HEAD: &str = "<meta charset=\"utf-8\">\n\
<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'; img-src 'none'; \
form-action 'none'; base-uri 'none'\">\n\
<meta name=\"referrer\" content=\"no-referrer\">\n\
<meta name=\"viewport\" content=\"width=device-width\">\n\
<style>body{font-family:sans-serif;max-width:60em;margin:1em auto;padding:0 1em;line-height:1.5}\
.picture{display:inline-block;box-sizing:border-box;max-width:100%;border:1px dashed #888;color:#666;\
font-size:smaller;padding:4px;overflow:hidden;vertical-align:middle}\
.link{color:#1a5fb4;text-decoration:underline}.quote{border-left:2px solid #999;padding-left:8px}\
table{border-collapse:collapse;max-width:100%}td,th{border:1px solid #ccc;padding:4px;vertical-align:top}</style>\n";

/// `document` as a page of HTML titled `title` (control characters, line breaks and bidi controls dropped, cut to [`MAX_TITLE`]).
/// Linear in the document's size; never panics.
#[must_use]
pub fn page(document: &Document, title: &str) -> String {
    let mut out = String::with_capacity(4096);
    out.push_str("<!DOCTYPE html>\n<html><head>\n");
    out.push_str(HEAD);
    out.push_str("<title>");
    let title: String = title.chars().filter(|c| !c.is_control() && kept_char(*c)).take(MAX_TITLE).collect();
    escape_into(&mut out, &title);
    out.push_str("</title>\n</head><body>\n");
    blocks(&mut out, document, &document.blocks, 0);
    out.push_str("</body></html>\n");
    out
}

/// Whether `c` may stand in the page's text: no control character but the line feed and the tab, no bidi override,
/// embedding or isolate.
fn kept_char(c: char) -> bool {
    !(c.is_control() && c != '\n' && c != '\t') && !matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
}

/// `text` with the five characters HTML gives a meaning escaped, the rest as [`kept_char`] keeps it.
fn escape_into(out: &mut String, text: &str) {
    for c in text.chars().filter(|c| kept_char(*c)) {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
}

/// Writes `list`, inside `nesting` tables.
fn blocks(out: &mut String, document: &Document, list: &[Block], nesting: usize) {
    for block in list {
        one(out, document, block, nesting);
    }
}

/// The `style` attribute of a block standing `quote` quotes in with `look` (none when it needs none).
fn style(quote: u8, look: &Look) -> String {
    let mut style = String::new();
    if quote > 0 {
        let _ = write!(style, "margin-left:{}px;", u32::from(quote).saturating_mul(12));
    }
    match look.align {
        Align::Left => {}
        Align::Centre => style.push_str("text-align:center;"),
        Align::Right => style.push_str("text-align:right;"),
    }
    paint(&mut style, look.background);
    if style.is_empty() {
        return String::new();
    }
    let class = if quote > 0 { " class=\"quote\"" } else { "" };
    format!("{class} style=\"{}\"", style.trim_end_matches(';'))
}

/// A kept background and its ink, as style declarations.
fn paint(style: &mut String, background: Option<Paint>) {
    if let Some(Paint { fill, ink }) = background {
        let _ = write!(style, "background:#{:02x}{:02x}{:02x};color:#{:02x}{:02x}{:02x};", fill.r, fill.g, fill.b, ink.r, ink.g, ink.b);
    }
}

/// A width a table or a cell asks for, as a style declaration.
fn width(width: Option<Width>) -> String {
    match width {
        Some(Width::Pixels(px)) => format!("width:{}px;", px.min(MAX_SIDE)),
        Some(Width::Share(share)) => format!("width:{}%;", share.clamp(1, 100)),
        None => String::new(),
    }
}

/// Writes one block, inside `nesting` tables.
fn one(out: &mut String, document: &Document, block: &Block, nesting: usize) {
    match block {
        Block::Text { kind, quote, look, spans } => {
            let style = style(*quote, look);
            let (open, close) = match kind {
                TextKind::Paragraph => (format!("<p{style}>"), "</p>".to_owned()),
                TextKind::Heading(level) => {
                    let level = (*level).clamp(1, 6);
                    (format!("<h{level}{style}>"), format!("</h{level}>"))
                }
                TextKind::ListItem { depth, marker } => {
                    let indent = u32::from(*depth).saturating_mul(24);
                    let quote_room = u32::from(*quote).saturating_mul(12);
                    let mut open = format!("<p style=\"margin:0 0 0 {}px\">", indent.saturating_add(quote_room));
                    match marker {
                        Marker::Bullet => open.push_str("\u{2022} "),
                        Marker::Number(number) => {
                            let _ = write!(open, "{number}. ");
                        }
                    }
                    (open, "</p>".to_owned())
                }
                TextKind::Preformatted => (format!("<pre{style}>"), "</pre>".to_owned()),
            };
            out.push_str(&open);
            for span in spans {
                self::span(out, document, span, *kind == TextKind::Preformatted);
            }
            out.push_str(&close);
            out.push('\n');
        }
        Block::Rule { .. } => out.push_str("<hr>\n"),
        Block::Image { quote, look, image } => {
            out.push_str(&format!("<p{}>", style(*quote, look)));
            picture(out, image);
            out.push_str("</p>\n");
        }
        Block::Table { quote, look, width: asked, rows } => {
            if nesting >= MAX_NESTING {
                flat(out, document, block);
                return;
            }
            let mut table_style = style(*quote, look);
            let asked = width(*asked);
            if !asked.is_empty() {
                table_style = if table_style.is_empty() {
                    format!(" style=\"{}\"", asked.trim_end_matches(';'))
                } else {
                    table_style.replacen(" style=\"", &format!(" style=\"{asked}"), 1)
                };
            }
            out.push_str(&format!("<table{table_style}>\n"));
            for row in rows {
                out.push_str("<tr>");
                for cell in row {
                    self::cell(out, document, cell, nesting.saturating_add(1));
                }
                out.push_str("</tr>\n");
            }
            out.push_str("</table>\n");
        }
    }
}

/// Writes a table cell, inside `nesting` tables.
fn cell(out: &mut String, document: &Document, cell: &Cell, nesting: usize) {
    let tag = if cell.header { "th" } else { "td" };
    let mut style = width(cell.width);
    paint(&mut style, cell.background);
    if style.is_empty() {
        out.push_str(&format!("<{tag}>"));
    } else {
        out.push_str(&format!("<{tag} style=\"{}\">", style.trim_end_matches(';')));
    }
    blocks(out, document, &cell.blocks, nesting);
    out.push_str(&format!("</{tag}>"));
}

/// Writes what `block` holds, at any depth, as plain paragraphs and pictures, without recursion.
fn flat(out: &mut String, document: &Document, block: &Block) {
    let mut stack: Vec<&Block> = vec![block];
    while let Some(block) = stack.pop() {
        match block {
            Block::Table { rows, .. } => {
                // Pushed in reverse, so they come out in reading order.
                for cell in rows.iter().rev().flat_map(|row| row.iter().rev()) {
                    stack.extend(cell.blocks.iter().rev());
                }
            }
            other => one(out, document, other, MAX_NESTING),
        }
    }
}

/// Writes a run of text: its words escaped, a line break as `<br>` (outside preformatted text), its look as plain
/// elements, a link as its words with where it goes in a tooltip.
fn span(out: &mut String, document: &Document, span: &Span, preformatted: bool) {
    let mut close: Vec<&str> = Vec::new();
    if let Some(link) = span.link.and_then(|index| document.links.get(index)) {
        out.push_str("<span class=\"link\"");
        if let Some(goes) = goes_to(link) {
            out.push_str(" title=\"");
            escape_into(out, &goes);
            out.push('"');
        }
        out.push('>');
        close.push("</span>");
    }
    for (on, open, end) in [
        (span.bold, "<b>", "</b>"),
        (span.italic, "<i>", "</i>"),
        (span.underline, "<u>", "</u>"),
        (span.strike, "<s>", "</s>"),
        (span.code && !preformatted, "<code>", "</code>"),
    ] {
        if on {
            out.push_str(open);
            close.push(end);
        }
    }
    let mut lines = span.text.split('\n');
    if let Some(first) = lines.next() {
        escape_into(out, first);
    }
    for line in lines {
        out.push_str(if preformatted { "\n" } else { "<br>" });
        escape_into(out, line);
    }
    for end in close.into_iter().rev() {
        out.push_str(end);
    }
}

/// Where a link goes, as the tooltip names it: a web link's host, a mail link's address; never an address a browser
/// could follow.
fn goes_to(link: &Link) -> Option<String> {
    match &link.target {
        LinkTarget::Web { host, .. } => Some(host.clone()),
        LinkTarget::Mail { address, .. } => Some(address.clone()),
        LinkTarget::Refused => None,
    }
}

/// A picture as a box with its alternative text, as large as its tag says (at most [`MAX_SIDE`]).
fn picture(out: &mut String, image: &Image) {
    out.push_str("<span class=\"picture\"");
    if let Some((w, h)) = image.size {
        let _ = write!(out, " style=\"width:{}px;height:{}px\"", w.min(MAX_SIDE), h.min(MAX_SIDE));
    }
    out.push('>');
    let alt = image.alt.trim();
    escape_into(out, if alt.is_empty() { "\u{1f5bc}" } else { alt });
    out.push_str("</span>");
}

#[cfg(test)]
#[path = "page_tests.rs"]
mod tests;
