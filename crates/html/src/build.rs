// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Tokens to display blocks.
//!
//! One pass over the tokens with a stack of open elements (at most [`MAX_DEPTH`]) and a stack of
//! frames the blocks go into: the body, table cells, captions and tables (at most
//! [`MAX_TABLE_DEPTH`] tables). Each open element remembers what it changed (bold, a link, a list,
//! a frame) and undoes exactly that when it closes, so broken markup can never leave the counters
//! or the frames inconsistent. An end tag closes the nearest open element of its name, never across
//! a table cell or caption (except `</table>`), and is ignored when there is none.

use std::collections::HashSet;

use crate::entities::unescape;
use crate::tokenizer::{Tag, Token, Tokenizer};
use crate::{
    Block, Cell, Document, Image, ImageSource, Link, MAX_BLOCKS, MAX_DEPTH, MAX_IMAGES, MAX_INPUT, MAX_LINKS, MAX_TABLE_COLUMNS, MAX_TABLE_DEPTH,
    MAX_TABLE_ROWS, Marker, Options, Span, TextKind, link, truncate,
};

/// Most characters of a link's text kept for comparing with its target.
const MAX_LINK_TEXT: usize = 1_024;

/// Most characters of a picture's alternative text kept.
const MAX_ALT: usize = 1_024;

/// Elements that end the paragraph before them and after them.
const BLOCK: [&str; 46] = [
    "address",
    "article",
    "aside",
    "blockquote",
    "body",
    "caption",
    "center",
    "dd",
    "details",
    "dialog",
    "dir",
    "div",
    "dl",
    "dt",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "form",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "header",
    "hgroup",
    "html",
    "legend",
    "li",
    "main",
    "menu",
    "nav",
    "ol",
    "p",
    "pre",
    "section",
    "summary",
    "table",
    "tbody",
    "td",
    "tfoot",
    "th",
    "thead",
    "tr",
    "ul",
];

/// Elements that never have content or an end tag.
const VOID: [&str; 17] = [
    "area", "base", "basefont", "br", "col", "embed", "frame", "hr", "img", "input", "isindex", "keygen", "link", "meta", "param", "source", "track",
];

/// Elements whose whole content is dropped: what embeds, draws, plays or belongs to the head.
const DROPPED: [&str; 11] = ["applet", "audio", "canvas", "datalist", "head", "map", "math", "object", "svg", "template", "video"];

/// The look of a run of text.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Style {
    bold: bool,
    italic: bool,
    underline: bool,
    strike: bool,
    code: bool,
    link: Option<usize>,
}

/// What an open element changed, undone when it closes.
#[derive(Debug, Default)]
struct Elem {
    name: String,
    bold: bool,
    italic: bool,
    underline: bool,
    strike: bool,
    code: bool,
    pre: bool,
    quote: bool,
    hidden: bool,
    list: bool,
    item: bool,
    row: bool,
    table: bool,
    flat: bool,
    /// The link that was current before it, when it started one.
    link: Option<Option<usize>>,
    /// The heading level that was current before it, when it is a heading.
    heading: Option<u8>,
    /// How many frames there were before it pushed one.
    frame: Option<usize>,
}

impl Elem {
    fn new(name: &str) -> Elem {
        Elem { name: name.to_owned(), ..Elem::default() }
    }
}

/// An open table.
#[derive(Debug, Default)]
struct TableFrame {
    quote: u8,
    rows: Vec<Vec<Cell>>,
    caption: Vec<Block>,
    /// What did not fit: the content of rows and cells past the caps.
    after: Vec<Block>,
    /// The row past [`MAX_TABLE_ROWS`] is open: its cells go after the table.
    full_row: bool,
}

impl TableFrame {
    fn can_add_cell(&self) -> bool {
        !self.full_row && self.rows.last().map_or(self.rows.len() < MAX_TABLE_ROWS, |r| r.len() < MAX_TABLE_COLUMNS)
    }

    fn add_cell(&mut self, cell: Cell) {
        if self.rows.is_empty() {
            self.rows.push(Vec::new());
        }
        if let Some(row) = self.rows.last_mut() {
            row.push(cell);
        }
    }

    fn start_row(&mut self) {
        self.full_row = self.rows.len() >= MAX_TABLE_ROWS;
        if !self.full_row {
            self.rows.push(Vec::new());
        }
    }

    /// The caption, then the table (or, when it has one column, its cells' blocks), then what did
    /// not fit. The second value says whether the table itself is a new block.
    fn into_blocks(self) -> (Vec<Block>, Option<Block>, Vec<Block>) {
        let TableFrame { quote, rows, mut caption, after, .. } = self;
        if rows.iter().all(|r| r.len() <= 1) {
            caption.extend(rows.into_iter().flatten().flat_map(|c| c.blocks));
            return (caption, None, after);
        }
        (caption, Some(Block::Table { quote, rows }), after)
    }
}

#[derive(Debug)]
enum Frame {
    Body(Vec<Block>),
    Cell { header: bool, implicit: bool, blocks: Vec<Block> },
    Caption(Vec<Block>),
    Table(TableFrame),
}

/// Where the open tables' rows and cells start in the element stack.
#[derive(Debug)]
struct TableAt {
    elem: usize,
    row: Option<usize>,
}

#[derive(Debug, Clone, Copy)]
struct List {
    ordered: bool,
    next: u32,
}

struct Builder<'o> {
    options: &'o Options,
    elems: Vec<Elem>,
    frames: Vec<Frame>,
    tables: Vec<TableAt>,
    /// Tables past [`MAX_TABLE_DEPTH`] open now; while any is, table structure is plain paragraphs.
    flat: usize,
    lists: Vec<List>,
    // Counters of open elements.
    bold: u32,
    italic: u32,
    underline: u32,
    strike: u32,
    code: u32,
    pre: u32,
    quote: u32,
    hidden: u32,
    heading: u8,
    link: Option<usize>,
    /// The paragraph being written.
    line: Vec<Span>,
    /// White space seen since the last character, with the look it was seen in.
    space: Option<Style>,
    /// A `pre` just opened: a newline right after it is not content.
    pre_start: bool,
    /// The marker of a list item whose first block has not been written yet.
    marker: Option<(u8, Marker)>,
    links: Vec<Link>,
    link_chars: Vec<usize>,
    images: usize,
    inline_images: Vec<String>,
    seen_cids: HashSet<String>,
    remote_blocked: usize,
    blocks: usize,
    truncated: bool,
    /// Where blocks would go should the body ever be closed early; never read.
    spare: Vec<Block>,
}

pub(crate) fn build(html: &str, options: &Options) -> Document {
    let input = truncate(html, MAX_INPUT);
    let mut b = Builder::new(options);
    b.truncated = input.len() < html.len();
    for token in Tokenizer::new(input) {
        match token {
            Token::Text(text) => b.text(text),
            Token::Open(tag) => b.open(&tag),
            Token::Close(name) => b.close(&name),
        }
    }
    b.finish()
}

impl<'o> Builder<'o> {
    fn new(options: &'o Options) -> Builder<'o> {
        Builder {
            options,
            elems: Vec::new(),
            frames: vec![Frame::Body(Vec::new())],
            tables: Vec::new(),
            flat: 0,
            lists: Vec::new(),
            bold: 0,
            italic: 0,
            underline: 0,
            strike: 0,
            code: 0,
            pre: 0,
            quote: 0,
            hidden: 0,
            heading: 0,
            link: None,
            line: Vec::new(),
            space: None,
            pre_start: false,
            marker: None,
            links: Vec::new(),
            link_chars: Vec::new(),
            images: 0,
            inline_images: Vec::new(),
            seen_cids: HashSet::new(),
            remote_blocked: 0,
            blocks: 0,
            truncated: false,
            spare: Vec::new(),
        }
    }

    fn finish(mut self) -> Document {
        self.flush();
        self.close_to(0);
        while let Some(Frame::Cell { .. } | Frame::Caption(_) | Frame::Table(_)) = self.frames.last() {
            self.finish_frame();
        }
        let blocks = match self.frames.pop() {
            Some(Frame::Body(blocks)) => blocks,
            _ => Vec::new(),
        };
        let mut links = self.links;
        for l in &mut links {
            l.mismatch = link::mismatch(&l.text, &l.target);
        }
        Document { blocks, links, inline_images: self.inline_images, remote_blocked: self.remote_blocked, truncated: self.truncated }
    }

    fn style(&self) -> Style {
        Style {
            bold: self.bold > 0,
            italic: self.italic > 0,
            underline: self.underline > 0,
            strike: self.strike > 0,
            code: self.code > 0 || self.pre > 0,
            link: self.link,
        }
    }

    fn quote_depth(&self) -> u8 {
        u8::try_from(self.quote).unwrap_or(u8::MAX)
    }

    // ---- text ----

    fn text(&mut self, raw: &str) {
        if self.hidden > 0 {
            return;
        }
        for c in unescape(raw).chars() {
            // A form feed is white space, and in preformatted text a plain space.
            let c = if c == '\x0c' { ' ' } else { c };
            if removed(c) {
                continue;
            }
            if self.pre > 0 {
                let leading = std::mem::take(&mut self.pre_start) && c == '\n';
                if !leading && c != '\r' {
                    self.push(c, self.style());
                }
            } else if c.is_whitespace() && c != '\u{a0}' {
                self.white_space();
            } else {
                if let Some(style) = self.space.take() {
                    self.push(' ', style);
                }
                self.push(c, self.style());
            }
        }
    }

    fn white_space(&mut self) {
        let ends_line = self.line.last().is_none_or(|s| s.text.ends_with('\n'));
        if !ends_line && self.space.is_none() {
            self.space = Some(self.style());
        }
    }

    fn push(&mut self, c: char, style: Style) {
        if let Some(i) = style.link
            && let (Some(link), Some(count)) = (self.links.get_mut(i), self.link_chars.get_mut(i))
            && *count < MAX_LINK_TEXT
        {
            link.text.push(c);
            *count = count.saturating_add(1);
        }
        match self.line.last_mut() {
            Some(last) if span_style(last) == style => last.text.push(c),
            _ => {
                let Style { bold, italic, underline, strike, code, link } = style;
                self.line.push(Span { text: c.to_string(), bold, italic, underline, strike, code, link });
            }
        }
    }

    fn line_break(&mut self) {
        self.space = None;
        if self.hidden == 0 && !self.line.is_empty() {
            self.push('\n', self.style());
        }
    }

    /// End the paragraph being written and put it where blocks go now.
    fn flush(&mut self) {
        self.space = None;
        let mut spans = std::mem::take(&mut self.line);
        while let Some(last) = spans.last_mut() {
            let kept = last.text.trim_end_matches([' ', '\n', '\t', '\r']).len();
            last.text.truncate(kept);
            if !last.text.is_empty() {
                break;
            }
            spans.pop();
        }
        if spans.is_empty() {
            return;
        }
        let kind = if let Some((depth, marker)) = self.marker.take() {
            TextKind::ListItem { depth, marker }
        } else if self.heading > 0 {
            TextKind::Heading(self.heading)
        } else if self.pre > 0 {
            TextKind::Preformatted
        } else {
            TextKind::Paragraph
        };
        let quote = self.quote_depth();
        self.emit(Block::Text { kind, quote, spans });
    }

    // ---- blocks and frames ----

    /// Put a new block where blocks go now, within [`MAX_BLOCKS`].
    fn emit(&mut self, block: Block) {
        if self.blocks >= MAX_BLOCKS {
            self.truncated = true;
            return;
        }
        self.blocks = self.blocks.saturating_add(1);
        self.target().push(block);
    }

    /// Where blocks go now: the innermost body, cell or caption; inside a table but outside a cell,
    /// a new cell when the table has room, else after the table.
    fn target(&mut self) -> &mut Vec<Block> {
        if let Some(Frame::Table(t)) = self.frames.last()
            && t.can_add_cell()
        {
            self.frames.push(Frame::Cell { header: false, implicit: true, blocks: Vec::new() });
        }
        let (frames, spare) = (&mut self.frames, &mut self.spare);
        match frames.last_mut() {
            Some(Frame::Body(blocks) | Frame::Cell { blocks, .. } | Frame::Caption(blocks)) => blocks,
            Some(Frame::Table(t)) => &mut t.after,
            // The body is never closed before the end; should it be, nothing more is kept.
            None => spare,
        }
    }

    /// Close the innermost frame and hand its content to the one around it.
    fn finish_frame(&mut self) {
        match self.frames.pop() {
            Some(Frame::Cell { header, blocks, .. }) => {
                if let Some(Frame::Table(t)) = self.frames.last_mut() {
                    t.add_cell(Cell { header, blocks });
                }
            }
            Some(Frame::Caption(blocks)) => {
                if let Some(Frame::Table(t)) = self.frames.last_mut() {
                    t.caption.extend(blocks);
                }
            }
            Some(Frame::Table(t)) => {
                let (before, table, after) = t.into_blocks();
                self.target().extend(before);
                if let Some(table) = table {
                    self.emit(table);
                }
                self.target().extend(after);
            }
            Some(body @ Frame::Body(_)) => self.frames.push(body),
            None => {}
        }
    }

    fn finish_implicit_cell(&mut self) {
        if let Some(Frame::Cell { implicit: true, .. }) = self.frames.last() {
            self.finish_frame();
        }
    }

    fn push_frame(&mut self, elem: &mut Elem, frame: Frame) {
        elem.frame = Some(self.frames.len());
        self.frames.push(frame);
    }

    // ---- elements ----

    fn open(&mut self, tag: &Tag<'_>) {
        let name = tag.name.as_str();
        if name.is_empty() {
            return;
        }
        if VOID.contains(&name) {
            self.void(tag);
            return;
        }
        if self.elems.len() >= MAX_DEPTH {
            return;
        }
        let mut elem = Elem::new(name);
        if self.hidden > 0 {
            self.elems.push(elem);
            return;
        }
        if name == "body" {
            self.close_named("head");
        }
        if BLOCK.contains(&name) {
            self.flush();
        }
        if DROPPED.contains(&name) || is_hidden(tag) {
            elem.hidden = true;
            self.hidden = self.hidden.saturating_add(1);
        } else if self.in_table() && matches!(name, "table" | "tr" | "td" | "th" | "caption") {
            self.table_part(tag, &mut elem);
        } else if name == "table" {
            self.table(&mut elem);
        } else {
            self.element(tag, &mut elem);
        }
        self.elems.push(elem);
    }

    /// Whether the innermost table structure is a real table (not one flattened past the cap).
    fn in_table(&self) -> bool {
        !self.tables.is_empty() && self.flat == 0
    }

    fn element(&mut self, tag: &Tag<'_>, elem: &mut Elem) {
        let name = elem.name.clone();
        let name = name.as_str();
        match name {
            "b" | "strong" | "th" => (elem.bold, self.bold) = (true, self.bold.saturating_add(1)),
            "i" | "em" | "cite" | "dfn" | "var" => (elem.italic, self.italic) = (true, self.italic.saturating_add(1)),
            "u" | "ins" => (elem.underline, self.underline) = (true, self.underline.saturating_add(1)),
            "s" | "strike" | "del" => (elem.strike, self.strike) = (true, self.strike.saturating_add(1)),
            "code" | "tt" | "kbd" | "samp" => (elem.code, self.code) = (true, self.code.saturating_add(1)),
            "pre" | "xmp" | "listing" | "plaintext" => {
                (elem.pre, self.pre, self.pre_start) = (true, self.pre.saturating_add(1), true);
            }
            "blockquote" => (elem.quote, self.quote) = (true, self.quote.saturating_add(1)),
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                elem.heading = Some(self.heading);
                self.heading = name.get(1..).and_then(|l| l.parse().ok()).unwrap_or(1);
            }
            "ul" | "ol" | "menu" | "dir" => {
                let next = if name == "ol" { tag.attr("start").and_then(|s| s.trim().parse().ok()).unwrap_or(1) } else { 1 };
                self.lists.push(List { ordered: name == "ol", next });
                elem.list = true;
            }
            "li" => {
                let depth = u8::try_from(self.lists.len().max(1)).unwrap_or(u8::MAX);
                let marker = match self.lists.last_mut() {
                    Some(list) if list.ordered => {
                        let n = list.next;
                        list.next = n.saturating_add(1);
                        Marker::Number(n)
                    }
                    _ => Marker::Bullet,
                };
                self.marker = Some((depth, marker));
                elem.item = true;
            }
            "a" => self.anchor(tag, elem),
            _ => {}
        }
    }

    fn anchor(&mut self, tag: &Tag<'_>, elem: &mut Elem) {
        let Some(href) = tag.attr("href") else { return };
        if self.links.len() >= MAX_LINKS {
            self.truncated = true;
            return;
        }
        self.links.push(Link { target: link::target(&href), text: String::new(), mismatch: false });
        self.link_chars.push(0);
        elem.link = Some(self.link);
        self.link = Some(self.links.len().saturating_sub(1));
    }

    fn table(&mut self, elem: &mut Elem) {
        if self.tables.len() >= MAX_TABLE_DEPTH || self.flat > 0 {
            elem.flat = true;
            self.flat = self.flat.saturating_add(1);
            return;
        }
        let quote = self.quote_depth();
        self.push_frame(elem, Frame::Table(TableFrame { quote, ..TableFrame::default() }));
        elem.table = true;
        self.tables.push(TableAt { elem: self.elems.len().saturating_add(1), row: None });
    }

    /// A `table`, `tr`, `td`, `th` or `caption` start tag inside a real table.
    fn table_part(&mut self, tag: &Tag<'_>, elem: &mut Elem) {
        let Some(at) = self.tables.last().map(|t| (t.elem, t.row)) else { return };
        let name = elem.name.clone();
        match name.as_str() {
            "table" => self.table(elem),
            "tr" => {
                self.close_to(at.0);
                self.finish_implicit_cell();
                if let Some(Frame::Table(t)) = self.frames.last_mut() {
                    t.start_row();
                }
                elem.row = true;
                if let Some(t) = self.tables.last_mut() {
                    t.row = Some(self.elems.len().saturating_add(1));
                }
            }
            "caption" => {
                self.close_to(at.0);
                self.finish_implicit_cell();
                self.push_frame(elem, Frame::Caption(Vec::new()));
            }
            _ => {
                self.close_to(at.1.unwrap_or(at.0));
                self.finish_implicit_cell();
                let room = matches!(self.frames.last(), Some(Frame::Table(t)) if t.can_add_cell());
                let header = tag.name == "th";
                if room {
                    self.push_frame(elem, Frame::Cell { header, implicit: false, blocks: Vec::new() });
                }
                if header {
                    (elem.bold, self.bold) = (true, self.bold.saturating_add(1));
                }
            }
        }
    }

    fn void(&mut self, tag: &Tag<'_>) {
        if self.hidden > 0 {
            return;
        }
        match tag.name.as_str() {
            "br" => self.line_break(),
            "hr" => {
                self.flush();
                let quote = self.quote_depth();
                self.emit(Block::Rule { quote });
            }
            "img" => self.image(tag),
            _ => {}
        }
    }

    fn image(&mut self, tag: &Tag<'_>) {
        let alt = tag.attr("alt").map(|a| clean_text(&a, MAX_ALT)).unwrap_or_default();
        let src = tag.attr("src");
        if src.is_none() && alt.is_empty() {
            return;
        }
        if self.images >= MAX_IMAGES {
            self.truncated = true;
            return;
        }
        self.images = self.images.saturating_add(1);
        let source = src.map_or(ImageSource::Unsupported, |s| link::image_source(&s, self.options));
        match &source {
            ImageSource::Blocked => self.remote_blocked = self.remote_blocked.saturating_add(1),
            ImageSource::Inline(id) if self.seen_cids.insert(id.clone()) => self.inline_images.push(id.clone()),
            _ => {}
        }
        self.flush();
        let quote = self.quote_depth();
        self.emit(Block::Image { quote, image: Image { alt, source, link: self.link } });
    }

    fn close(&mut self, name: &str) {
        if name == "br" {
            self.line_break();
            return;
        }
        if BLOCK.contains(&name) && self.hidden == 0 {
            self.flush();
        }
        self.close_named(name);
    }

    /// Close the nearest open element called `name`, and everything inside it; nothing when there
    /// is none, or when a table cell or caption is in the way (only `</table>` goes past those).
    fn close_named(&mut self, name: &str) {
        for i in (0..self.elems.len()).rev() {
            let Some(e) = self.elems.get(i) else { return };
            if e.name == name {
                self.close_to(i);
                return;
            }
            if e.frame.is_some() && name != "table" {
                return;
            }
        }
    }

    /// Close open elements until `len` are left.
    fn close_to(&mut self, len: usize) {
        while self.elems.len() > len {
            if self.elems.last().is_some_and(|e| e.pre) {
                self.flush();
            }
            self.pop();
        }
    }

    fn pop(&mut self) {
        let Some(e) = self.elems.pop() else { return };
        let dec = |n: &mut u32, on: bool| {
            if on {
                *n = n.saturating_sub(1);
            }
        };
        dec(&mut self.bold, e.bold);
        dec(&mut self.italic, e.italic);
        dec(&mut self.underline, e.underline);
        dec(&mut self.strike, e.strike);
        dec(&mut self.code, e.code);
        dec(&mut self.pre, e.pre);
        dec(&mut self.quote, e.quote);
        dec(&mut self.hidden, e.hidden);
        if e.flat {
            self.flat = self.flat.saturating_sub(1);
        }
        if let Some(previous) = e.link {
            self.link = previous;
        }
        if let Some(previous) = e.heading {
            self.flush();
            self.heading = previous;
        }
        if e.list {
            self.lists.pop();
            self.marker = None;
        }
        if e.item {
            self.flush();
            self.marker = None;
        }
        if e.row
            && let Some(t) = self.tables.last_mut()
        {
            t.row = None;
        }
        if e.table {
            self.tables.pop();
        }
        if let Some(at) = e.frame {
            self.flush();
            while self.frames.len() > at {
                self.finish_frame();
            }
        }
    }
}

fn span_style(s: &Span) -> Style {
    Style { bold: s.bold, italic: s.italic, underline: s.underline, strike: s.strike, code: s.code, link: s.link }
}

/// Characters never shown: controls other than white space, and invisible characters (among them
/// the bidi overrides and isolates that can make text read differently from what it is) other than
/// the joiners emoji and some scripts need.
fn removed(c: char) -> bool {
    (c.is_control() && !matches!(c, '\t' | '\n' | '\r')) || (link::invisible(c) && !link::joins(c))
}

/// Attribute text as shown: removed characters dropped, white space collapsed, at most `max`
/// characters.
fn clean_text(s: &str, max: usize) -> String {
    let words = s.split(|c: char| c.is_whitespace() && c != '\u{a0}').filter(|w| !w.is_empty());
    let joined = words.collect::<Vec<_>>().join(" ");
    joined.chars().filter(|c| !removed(*c)).take(max).collect()
}

/// Whether an element is hidden: the `hidden` attribute, or `display: none` or
/// `visibility: hidden` in its inline style. Nothing else of the style is read.
fn is_hidden(tag: &Tag<'_>) -> bool {
    if tag.has("hidden") {
        return true;
    }
    let Some(style) = tag.attr("style") else { return false };
    let style: String = style.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_ascii_lowercase();
    style.split(';').any(|d| {
        let d = d.trim_end_matches("!important");
        d == "display:none" || d == "visibility:hidden"
    })
}
