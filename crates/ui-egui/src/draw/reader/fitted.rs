// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! An HTML message fitted to the app: its blocks set in the app's type and colours, 12 apart. Paragraphs and list
//! items in `read` (a list's marks in a column of 24), a heading in the title's or the heading's size in Plex Sans
//! SemiBold (a heading is the writer's words, and people's words are in Plex Sans), preformatted text in `mono` on
//! `term` in a 1 px `line` box, a rule as a `panel_line` hairline. A link is `cyan_text`, underlined, a Tab stop with
//! the focus ring round its words; Enter or a click copies its address and the status float says where it goes;
//! nothing opens. A paragraph is no Tab stop: a screen reader hears it as a label.
//!
//! A quote (`blockquote`) that follows the message's own text is folded behind a quote fold ("▸ 14 quoted lines");
//! open, its blocks stand under the fold in `muted`, the fold's 2 px edge running down beside them and each deeper
//! level with a 1 px `line` rule at its left, 12 in per level, to depth 4. A quote before any text of the message's own
//! stands open with a rule for each level.
//!
//! A table is a hairline table: a 1 px `line` edge round it, 1 px `panel_line` between its rows and its columns, cells
//! padded 8 × 12, a row of head cells on `panel` with its words in `label` and `muted`. Its columns get their widths
//! from [`column_widths`], cut at whole pixels; a column keeps at least 48, and a table that cannot keep that in its
//! room scrolls sideways inside itself. Tables nest 4 deep; a deeper one is set as its cells' blocks one after another.
//! At most 64 columns and 1 000 rows are drawn; a line under the table says what was left out.
//!
//! Everything in a message is data and bounded: the blocks are laid out once for a message, a width and the state of
//! its quotes and pictures, and kept; each frame only what is in sight is painted, and only what is within a pane's
//! height of it takes the pointer and the keyboard (and the link that has the focus, wherever it is).

use std::hash::{Hash as _, Hasher as _};
use std::sync::Arc;

use egui::text::{LayoutJob, TextFormat};
use egui::{
    ColorImage, Galley, Id, Pos2, Rect, Role, ScrollArea, Sense, Stroke, StrokeKind, TextureOptions, Ui, UiBuilder, Vec2, WidgetInfo, pos2, vec2,
};
use inboxcraft_html::{Block, Cell, Document, Image, ImageSource, LinkTarget, Marker, Span, TextKind};

use crate::component::quote_fold::{FOLD_EDGE, fold_line};
use crate::draw::paint::{self, Ring};
use crate::picture::Decoded;
use crate::reader::{PictureState, Reader, inline_key, link_status};
use crate::theme::{Font, Theme};
use crate::tr;

use super::body::column_widths;
use super::{Asked, Drawn, Textures};

/// The room between two blocks, in points.
const GAP: f32 = 12.0;

/// The room between two items of one list, in points.
const LIST_GAP: f32 = 4.0;

/// The column a list item's mark stands in, in points.
const MARK: f32 = 24.0;

/// How far in each level of a quote stands, in points.
const LEVEL: f32 = 12.0;

/// The deepest quote drawn as its own level.
const MOST_LEVELS: u8 = 4;

/// The room inside preformatted text's box, left and right and above and below, in points.
const PRE_PAD: Vec2 = vec2(12.0, 6.0);

/// The room inside a table's cell, left and right and above and below, in points.
const CELL_PAD: Vec2 = vec2(12.0, 8.0);

/// The least width of a table's column, its padding included, in points.
const LEAST_COLUMN: f32 = 48.0;

/// The most columns of a table drawn.
const MOST_COLUMNS: usize = 64;

/// The most rows of a table drawn.
const MOST_ROWS: usize = 1_000;

/// How deep tables inside tables are drawn as tables.
const MAX_DEPTH: usize = 4;

/// How deep blocks inside tables are followed at all: deeper ones are left out.
const DEEPEST: usize = 16;

/// The tallest picture shown, in points.
const MAX_PICTURE_HEIGHT: f32 = 480.0;

/// How wide a picture's column asks to be while the picture is not there to measure.
const WANTED_PICTURE: f32 = 160.0;

/// About how wide a letter of running text is, as a part of the font's size: enough to size a column from the number
/// of letters without laying the text out twice.
const LETTER: f32 = 0.55;

/// The most letters of one block a screen reader hears.
const MOST_SPOKEN: usize = 4_096;

/// How many messages' layouts are kept.
const KEPT: usize = 8;

/// A message's blocks laid out: what stands where, relative to the top left of the message.
pub(super) struct Laid {
    items: Vec<Item>,
    /// The quotes' rules and fold edges.
    rules: Vec<Rect>,
    height: f32,
}

/// One block, placed.
struct Item {
    rect: Rect,
    kind: Kind,
}

/// What a placed block is.
enum Kind {
    /// Words: the galley at `left` in the item, a list item's mark before it, the links in it, what a screen reader
    /// hears (none where the words are only links, which say themselves).
    Text { galley: Arc<Galley>, left: f32, mark: Option<Arc<Galley>>, links: Vec<Piece>, spoken: Option<String> },
    /// Preformatted text in its box.
    Pre { galley: Arc<Galley>, spoken: String },
    /// A horizontal rule.
    Rule,
    /// A picture that is here, at its size.
    Picture { image: Image, key: String },
    /// A picture that is not, and why, in its box.
    Missing { galley: Arc<Galley>, alt: String, link: Option<usize> },
    /// A quote's fold line.
    Fold { lines: usize, open: bool },
    /// A table.
    Table(Box<Grid>),
}

/// The words of one link in a text, as it wrapped: a box for each of its rows, relative to the galley.
struct Piece {
    link: usize,
    rects: Vec<Rect>,
}

/// A table laid out: its columns' lefts and widths, its rows, how wide and high it is, and what was left out.
struct Grid {
    lefts: Vec<f32>,
    rows: Vec<GridRow>,
    size: Vec2,
    note: Option<Arc<Galley>>,
}

/// A row of a table: its top and height in the table, whether it is a row of head cells, its cells laid out.
struct GridRow {
    top: f32,
    high: f32,
    head: bool,
    cells: Vec<Laid>,
}

/// How the text of a block is set: the style of its paragraphs, and whether it is a table's head.
#[derive(Clone, Copy)]
struct Style {
    text: Font,
    head: bool,
}

/// What laying out needs to know.
struct Context<'a> {
    ui: &'a Ui,
    theme: &'a Theme,
    reader: &'a Reader,
    id: u64,
    quotes: bool,
    ppp: f32,
}

/// Draws `list`, blocks of message `id`'s `document`, tables `depth` deep, across `ui`'s width.
#[expect(clippy::too_many_arguments, reason = "the drawing context of a message's blocks")]
pub(super) fn blocks(
    ui: &mut Ui,
    theme: &Theme,
    reader: &Reader,
    id: u64,
    document: &Document,
    list: &[Block],
    depth: usize,
    textures: &mut Textures,
    drawn: &mut Drawn,
) {
    let width = ui.available_width().max(1.0).floor();
    let quotes = reader.shows_quotes(id);
    let ppp = ui.ctx().pixels_per_point();
    let key = {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        (list.as_ptr() as usize, list.len(), depth, width.to_bits(), ppp.to_bits(), quotes, reader.pictures_state()).hash(&mut hasher);
        (theme.palette.ink, theme.palette.cyan_text, theme.fonts.read.size.to_bits()).hash(&mut hasher);
        hasher.finish()
    };
    let who = {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        (id, list.as_ptr() as usize).hash(&mut hasher);
        hasher.finish()
    };
    let laid = kept(ui, who, key, || {
        let context = Context { ui, theme, reader, id, quotes, ppp };
        Arc::new(lay(&context, list, width, Style { text: theme.fonts.read, head: false }, depth))
    });
    let (_, rect) = ui.allocate_space(vec2(width, laid.height));
    let base = ui.scope_id().with(("fitted", id, depth));
    let mut show = Show { theme, reader, id, document, textures, drawn };
    show.laid(ui, &laid, rect.min, base);
}

/// The layout kept for `who` under `key`, or the one `make` lays out, kept in its place.
fn kept(ui: &Ui, who: u64, key: u64, make: impl FnOnce() -> Arc<Laid>) -> Arc<Laid> {
    type Kept = Vec<(u64, u64, Arc<Laid>)>;
    let slot = Id::unique("fitted-kept");
    if let Some(laid) = ui
        .data(|data| data.get_temp::<Arc<Kept>>(slot))
        .and_then(|kept| kept.iter().find(|(owner, kept_key, _)| *owner == who && *kept_key == key).map(|(_, _, laid)| laid.clone()))
    {
        return laid;
    }
    let laid = make();
    ui.data_mut(|data| {
        let mut kept: Kept =
            data.get_temp::<Arc<Kept>>(slot).map(|kept| kept.iter().filter(|(owner, ..)| *owner != who).cloned().collect()).unwrap_or_default();
        kept.insert(0, (who, key, laid.clone()));
        kept.truncate(KEPT);
        data.insert_temp(slot, Arc::new(kept));
    });
    laid
}

/// The quote depth of `block`.
fn quote_of(block: &Block) -> u8 {
    match block {
        Block::Text { quote, .. } | Block::Rule { quote } | Block::Image { quote, .. } | Block::Table { quote, .. } => *quote,
    }
}

/// How many lines a quote of `run` holds, as its fold says.
fn lines_of(run: &[Block]) -> usize {
    run.iter()
        .map(|block| match block {
            Block::Text { spans, .. } => spans.iter().map(|span| span.text.matches('\n').count()).fold(1_usize, usize::saturating_add),
            _ => 1,
        })
        .fold(0, usize::saturating_add)
}

/// The rules of a quote's levels while they run: for each level, where its rule began and where it ends so far.
#[derive(Default)]
struct Levels {
    open: [Option<(f32, f32)>; MOST_LEVELS as usize],
}

impl Levels {
    /// A block from `top` to `bottom` at `level`, its rules from level `from` on: the levels it stands in run on to its
    /// foot, the deeper ones end.
    fn stand(&mut self, rules: &mut Vec<Rect>, from: u8, level: u8, top: f32, bottom: f32) {
        for (index, slot) in self.open.iter_mut().enumerate() {
            let at = u8::try_from(index).unwrap_or(MOST_LEVELS).saturating_add(1);
            if at >= from && at <= level {
                let begun = slot.map_or(top, |(begun, _)| begun);
                *slot = Some((begun, bottom));
            } else if let Some((begun, ended)) = slot.take() {
                rules.push(rule_at(at, begun, ended));
            }
        }
    }

    /// Ends every level.
    fn end(&mut self, rules: &mut Vec<Rect>) {
        for (index, slot) in self.open.iter_mut().enumerate() {
            if let Some((begun, ended)) = slot.take() {
                rules.push(rule_at(u8::try_from(index).unwrap_or(MOST_LEVELS).saturating_add(1), begun, ended));
            }
        }
    }
}

/// The 1 px rule of quote level `level` from `top` to `bottom`.
fn rule_at(level: u8, top: f32, bottom: f32) -> Rect {
    let x = LEVEL * f32::from(level.saturating_sub(1));
    Rect::from_min_max(pos2(x, top), pos2(x + 1.0, bottom.max(top)))
}

/// Lays out `list` `width` wide in `style`, tables `depth` deep.
fn lay(context: &Context<'_>, list: &[Block], width: f32, style: Style, depth: usize) -> Laid {
    let mut laid = Laid { items: Vec::new(), rules: Vec::new(), height: 0.0 };
    if depth > DEEPEST {
        return laid;
    }
    let mut y = 0.0_f32;
    let mut own = false;
    let mut previous: Option<&Block> = None;
    let mut index = 0;
    while let Some(block) = list.get(index) {
        let quote = quote_of(block);
        if quote == 0 {
            own = true;
            place(context, &mut laid, &mut y, &mut previous, block, width, style, depth, 0);
            index += 1;
            continue;
        }
        let run_length = list.get(index..).map_or(0, |rest| rest.iter().take_while(|block| quote_of(block) > 0).count()).max(1);
        let run = list.get(index..index.saturating_add(run_length)).unwrap_or_default();
        index = index.saturating_add(run_length);
        let mut levels = Levels::default();
        // A quote after the message's own text folds; the fold's edge stands for its first level, the rules for the
        // deeper ones. A quote before any text of the message's own stands open, a rule for each level.
        let mut from = 1;
        let mut edge = None;
        if own {
            if previous.is_some() {
                y += GAP;
            }
            let [.., high] = context.theme.boxes().reader;
            let fold = Kind::Fold { lines: lines_of(run), open: context.quotes };
            laid.items.push(Item { rect: Rect::from_min_size(pos2(0.0, y), vec2(width, high)), kind: fold });
            edge = Some(y);
            y += high;
            previous = run.first();
            from = 2;
            if !context.quotes {
                continue;
            }
        }
        for block in run {
            let level = quote_of(block).min(MOST_LEVELS);
            let before = laid.items.len();
            place(context, &mut laid, &mut y, &mut previous, block, width, style, depth, level);
            let start = laid.items.get(before).map_or(y, |item| item.rect.top());
            levels.stand(&mut laid.rules, from, level, start, y);
        }
        if let Some(top) = edge {
            laid.rules.push(Rect::from_min_max(pos2(0.0, top), pos2(FOLD_EDGE, y)));
        }
        levels.end(&mut laid.rules);
    }
    laid.height = y.ceil();
    laid
}

/// Places `block` under what stands at `y`, `level` levels into a quote, and moves `y` to its foot.
#[expect(clippy::too_many_arguments, reason = "the state of a layout in progress")]
fn place<'b>(
    context: &Context<'_>,
    laid: &mut Laid,
    y: &mut f32,
    previous: &mut Option<&'b Block>,
    block: &'b Block,
    width: f32,
    style: Style,
    depth: usize,
    level: u8,
) {
    let listed = |block: &Block| matches!(block, Block::Text { kind: TextKind::ListItem { .. }, .. });
    if let Some(before) = previous {
        *y += if listed(before) && listed(block) { LIST_GAP } else { GAP };
    }
    let indent = LEVEL * f32::from(level);
    let room = (width - indent).max(1.0);
    let at = pos2(indent, *y);
    let item = match block {
        Block::Text { kind, spans, .. } => {
            text(context, *kind, spans, level > 0, style, room).map(|(kind, size)| Item { rect: Rect::from_min_size(at, size), kind })
        }
        Block::Rule { .. } => Some(Item { rect: Rect::from_min_size(at, vec2(room, 1.0)), kind: Kind::Rule }),
        Block::Image { image, .. } => picture(context, image, room).map(|(kind, size)| Item { rect: Rect::from_min_size(at, size), kind }),
        Block::Table { rows, .. } if depth < MAX_DEPTH => {
            let grid = grid(context, rows, room, depth);
            let size = vec2(grid.size.x.min(room), grid.size.y + grid.note.as_ref().map_or(0.0, |note| LIST_GAP + note.size().y));
            Some(Item { rect: Rect::from_min_size(at, size), kind: Kind::Table(Box::new(grid)) })
        }
        Block::Table { rows, .. } => {
            // Too deep to draw as a table: its cells' blocks one after another.
            let mut flat = lay(context, &[], room, style, depth.saturating_add(1));
            let mut top = 0.0_f32;
            for cell in rows.iter().take(MOST_ROWS).flat_map(|row| row.iter().take(MOST_COLUMNS)) {
                let inner = lay(context, &cell.blocks, room, style, depth.saturating_add(1));
                if inner.items.is_empty() {
                    continue;
                }
                if top > 0.0 {
                    top += GAP;
                }
                let shift = vec2(indent, *y + top);
                flat.items.extend(inner.items.into_iter().map(|item| Item { rect: item.rect.translate(shift), kind: item.kind }));
                flat.rules.extend(inner.rules.into_iter().map(|rule| rule.translate(shift)));
                top += inner.height;
            }
            if top > 0.0 {
                laid.items.append(&mut flat.items);
                laid.rules.append(&mut flat.rules);
                *y += top;
                *previous = Some(block);
            } else if previous.is_some() {
                *y -= if previous.is_some_and(listed) && listed(block) { LIST_GAP } else { GAP };
            }
            return;
        }
    };
    match item {
        Some(item) => {
            *y = item.rect.bottom();
            laid.items.push(item);
            *previous = Some(block);
        }
        None => {
            // Nothing to draw: the room before it goes too.
            if let Some(before) = previous {
                *y -= if listed(before) && listed(block) { LIST_GAP } else { GAP };
            }
        }
    }
}

/// The style of a text block of `kind` in `style`.
fn font_of(theme: &Theme, kind: TextKind, style: Style) -> Font {
    let f = &theme.fonts;
    match kind {
        TextKind::Preformatted => f.mono,
        _ if style.head => f.label,
        // A heading in a message is its writer's words: in Plex Sans SemiBold, in the title's or the heading's size.
        TextKind::Heading(1) => f.written_title(),
        TextKind::Heading(_) => f.written_heading(),
        TextKind::Paragraph | TextKind::ListItem { .. } => style.text,
    }
}

/// A text block laid out `room` wide: its kind and size; nothing for a block of no words.
fn text(context: &Context<'_>, kind: TextKind, spans: &[Span], quoted: bool, style: Style, room: f32) -> Option<(Kind, Vec2)> {
    let theme = context.theme;
    let p = &theme.palette;
    let f = &theme.fonts;
    if spans.iter().all(|span| span.text.trim().is_empty()) {
        return None;
    }
    let font = font_of(theme, kind, style);
    let colour = if quoted || style.head { p.muted } else { p.ink };
    let (mark, left) = match kind {
        TextKind::ListItem { depth, marker } => {
            let mark = match marker {
                Marker::Bullet => "•".to_owned(),
                Marker::Number(n) => format!("{n}."),
            };
            let left = MARK * f32::from(depth.clamp(1, 6).saturating_sub(1));
            (Some(crate::component::galley(context.ui, &mark, font, colour)), left + MARK)
        }
        _ => (None, 0.0),
    };
    let pre = kind == TextKind::Preformatted;
    let wrap = if pre { room - PRE_PAD.x * 2.0 - 2.0 } else { room - left };
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap.max(1.0);
    job.wrap.break_anywhere = pre;
    // Per section: its characters (start, end) and its link.
    let mut sections: Vec<(usize, usize, Option<usize>)> = Vec::new();
    let mut chars = 0_usize;
    let mut words = String::new();
    for span in spans {
        // Code is monospaced, bold text a heavier weight, both in the size of the block.
        let face = if span.code {
            f.mono.face
        } else if span.bold && matches!(kind, TextKind::Paragraph | TextKind::ListItem { .. }) && !style.head {
            f.strong.face
        } else {
            font.face
        };
        let linked = span.link.is_some();
        let mut format: TextFormat = Font { face, ..font }.format(if linked { p.cyan_text } else { colour });
        format.italics = span.italic;
        if span.underline || linked {
            format.underline = Stroke::new(1.0, if linked { p.cyan_text } else { colour });
        }
        if span.strike {
            format.strikethrough = Stroke::new(1.0, colour);
        }
        let shown = font.text(&crate::component::plain_lines(&span.text));
        let count = shown.chars().count();
        sections.push((chars, chars.saturating_add(count), span.link));
        chars = chars.saturating_add(count);
        words.push_str(crate::quote::cut(&shown, (MOST_SPOKEN * 4).saturating_sub(words.len())));
        job.append(&shown, 0.0, format);
    }
    let galley = paint::on_baseline(context.ui.painter().layout_job(job));
    let links = pieces(&galley, &sections);
    if pre {
        let size = vec2(room, (galley.size().y + PRE_PAD.y * 2.0).ceil());
        let spoken = crate::component::spoken(crate::component::capped(&words, MOST_SPOKEN).0);
        return Some((Kind::Pre { galley, spoken }, size));
    }
    let only_links = sections.iter().all(|(start, end, link)| link.is_some() || start == end);
    let spoken = (!only_links).then(|| crate::component::spoken(crate::component::capped(&words, MOST_SPOKEN).0));
    let size = vec2(room, galley.size().y.ceil().max(font.line));
    Some((Kind::Text { galley, left, mark, links, spoken }, size))
}

/// The boxes of the links of `galley`, whose `sections` say which characters belong to which link.
fn pieces(galley: &Galley, sections: &[(usize, usize, Option<usize>)]) -> Vec<Piece> {
    let mut found: Vec<Piece> = Vec::new();
    let mut start = 0_usize;
    let mut section = 0_usize;
    // The link of the character before, so a link that runs on keeps one piece.
    let mut running: Option<usize> = None;
    for row in &galley.rows {
        let rect = row.rect();
        for (offset, glyph) in row.glyphs.iter().enumerate() {
            let at = start.saturating_add(offset);
            while sections.get(section).is_some_and(|(_, end, _)| *end <= at) {
                section += 1;
            }
            let link = sections.get(section).filter(|(begin, ..)| *begin <= at).and_then(|(.., link)| *link);
            let Some(link) = link else {
                running = None;
                continue;
            };
            let left = rect.left() + glyph.pos.x;
            let right = left + glyph.advance_width;
            let boxed = Rect::from_min_max(pos2(left, rect.top()), pos2(right, rect.bottom()));
            match found.last_mut() {
                Some(piece) if running == Some(link) && piece.link == link => match piece.rects.last_mut() {
                    Some(last) if (last.top() - boxed.top()).abs() < 0.5 => *last = last.union(boxed),
                    _ => piece.rects.push(boxed),
                },
                _ => found.push(Piece { link, rects: vec![boxed] }),
            }
            running = Some(link);
        }
        let chars: usize = row.char_count_including_newline().into();
        start = start.saturating_add(chars);
        if row.ends_with_newline {
            running = None;
        }
    }
    found
}

/// The key a picture of `image` is kept under, if it can be had at all.
fn key_of(id: u64, image: &Image) -> Option<String> {
    match &image.source {
        ImageSource::Inline(content_id) => Some(inline_key(id, content_id)),
        ImageSource::Remote(url) => Some(url.clone()),
        ImageSource::Blocked | ImageSource::Unsupported => None,
    }
}

/// A picture laid out `room` wide: at its size when it is here, else a box that says why not; nothing for a picture
/// that cannot be shown and says nothing.
fn picture(context: &Context<'_>, image: &Image, room: f32) -> Option<(Kind, Vec2)> {
    let theme = context.theme;
    let key = key_of(context.id, image);
    let state = key.as_deref().and_then(|key| context.reader.picture(key));
    if let (Some(key), Some(PictureState::Ready(decoded))) = (key.as_ref(), state) {
        let (w, h) = (decoded.width as f32, decoded.height as f32);
        if w > 0.0 && h > 0.0 {
            let scale = (room / w).min(MAX_PICTURE_HEIGHT / h).min(1.0);
            return Some((
                Kind::Picture { image: image.clone(), key: key.clone() },
                vec2((w * scale).floor().max(1.0), (h * scale).floor().max(1.0)),
            ));
        }
    }
    let why = match (&image.source, state) {
        (ImageSource::Unsupported, _) if image.alt.is_empty() => return None,
        (ImageSource::Unsupported, _) => tr!("Picture not shown").to_owned(),
        (ImageSource::Blocked, _) => tr!("Remote picture not loaded").to_owned(),
        (_, Some(PictureState::Failed(reason))) => tr!("Picture could not be shown: {reason}", reason = reason),
        (_, Some(PictureState::Waiting) | None) => tr!("Loading picture…").to_owned(),
        (_, Some(PictureState::Ready(_))) => tr!("Picture could not be shown").to_owned(),
    };
    let alt = crate::component::spoken(crate::component::capped(&image.alt, MOST_SPOKEN).0);
    let words = if alt.is_empty() { why } else { format!("{why}: {alt}") };
    let inner = (room - 18.0).max(1.0);
    let galley = crate::component::wrapped(
        context.ui,
        &crate::component::plain(&words),
        theme.fonts.caption,
        theme.palette.muted,
        inner,
        4,
        egui::Align::LEFT,
    );
    let size = vec2((galley.size().x.ceil() + 18.0).min(room), galley.size().y.ceil() + 18.0);
    Some((Kind::Missing { galley, alt: words, link: image.link }, size))
}

/// How wide `block` asks to be on one line.
fn wanted_width(context: &Context<'_>, block: &Block, depth: usize) -> f32 {
    match block {
        Block::Text { spans, .. } => {
            let letters = spans.iter().map(|span| span.text.chars().count()).fold(0_usize, usize::saturating_add);
            letters as f32 * context.theme.fonts.body.size * LETTER
        }
        Block::Rule { .. } => 0.0,
        Block::Image { image, .. } => match key_of(context.id, image).as_deref().and_then(|key| context.reader.picture(key)) {
            Some(PictureState::Ready(decoded)) => decoded.width as f32,
            _ => WANTED_PICTURE,
        },
        Block::Table { rows, .. } if depth < MAX_DEPTH => {
            let columns = wanted_columns(context, rows, depth.saturating_add(1));
            columns.iter().map(|column| column + CELL_PAD.x * 2.0 + 1.0).sum::<f32>() + 1.0
        }
        Block::Table { .. } => 0.0,
    }
}

/// How wide each column of a table asks to be, its padding left out: its widest cell, each cell its widest block.
fn wanted_columns(context: &Context<'_>, rows: &[Vec<Cell>], depth: usize) -> Vec<f32> {
    let count = rows.iter().take(MOST_ROWS).map(Vec::len).max().unwrap_or(0).min(MOST_COLUMNS);
    let mut wanted = vec![0.0_f32; count];
    for row in rows.iter().take(MOST_ROWS) {
        for (cell, column) in row.iter().zip(wanted.iter_mut()) {
            let widest = cell.blocks.iter().map(|block| wanted_width(context, block, depth)).fold(0.0, f32::max);
            *column = column.max(widest);
        }
    }
    wanted
}

/// A table laid out in `room`: its columns from [`column_widths`] cut at whole pixels, each at least
/// [`LEAST_COLUMN`] (wider than the room, it scrolls), its cells laid out, its rows as high as their highest cell.
fn grid(context: &Context<'_>, rows: &[Vec<Cell>], room: f32, depth: usize) -> Grid {
    let theme = context.theme;
    let wanted: Vec<f32> = wanted_columns(context, rows, depth).into_iter().map(|width| width + CELL_PAD.x * 2.0).collect();
    let count = wanted.len();
    let rules = count as f32 + 1.0;
    let least = LEAST_COLUMN * count as f32;
    let widths = column_widths(&wanted, (room - rules).max(least), 0.0);
    let ppp = if context.ppp.is_finite() && context.ppp > 0.0 { context.ppp } else { 1.0 };
    let widths: Vec<f32> = widths.into_iter().map(|width| ((width * ppp).floor() / ppp).max(1.0)).collect();
    let mut lefts = Vec::with_capacity(count);
    let mut x = 1.0_f32;
    for width in &widths {
        lefts.push(x);
        x += width + 1.0;
    }
    let mut laid_rows = Vec::new();
    let mut top = 1.0_f32;
    for row in rows.iter().take(MOST_ROWS) {
        let head = !row.is_empty() && row.iter().all(|cell| cell.header);
        let style = Style { text: theme.fonts.body, head };
        let cells: Vec<Laid> = widths
            .iter()
            .enumerate()
            .map(|(index, width)| match row.get(index) {
                Some(cell) => lay(context, &cell.blocks, (width - CELL_PAD.x * 2.0).max(1.0), style, depth.saturating_add(1)),
                None => Laid { items: Vec::new(), rules: Vec::new(), height: 0.0 },
            })
            .collect();
        let tallest = cells.iter().map(|cell| cell.height).fold(theme.fonts.body.line, f32::max);
        let high = (tallest + CELL_PAD.y * 2.0).ceil();
        laid_rows.push(GridRow { top, high, head, cells });
        top += high + 1.0;
    }
    let shown_columns = rows.iter().take(MOST_ROWS).map(Vec::len).max().unwrap_or(0);
    let note = crate::component::hairline_table::left_out(shown_columns.saturating_sub(count), rows.len().saturating_sub(MOST_ROWS))
        .map(|note| crate::component::wrapped(context.ui, &note, theme.fonts.caption, theme.palette.muted, room.max(1.0), 2, egui::Align::LEFT));
    Grid { lefts, rows: laid_rows, size: vec2(x, top.max(2.0)), note }
}

/// The drawing of a laid out message: what it needs and what it gives back.
struct Show<'a> {
    theme: &'a Theme,
    reader: &'a Reader,
    id: u64,
    document: &'a Document,
    textures: &'a mut Textures,
    drawn: &'a mut Drawn,
}

impl Show<'_> {
    /// Draws `laid` with its top left at `origin`, its widgets' ids from `base`; whether one of its links has the focus.
    fn laid(&mut self, ui: &mut Ui, laid: &Laid, origin: Pos2, base: Id) -> bool {
        let sight = ui.clip_rect();
        // What lies within a pane's height of the sight takes the pointer and the keyboard, so Tab reaches the next
        // link before it is in sight (the focus brings it in).
        let near = sight.expand2(vec2(0.0, sight.height().max(1.0)));
        let p = &self.theme.palette;
        for rule in &laid.rules {
            let rule = rule.translate(origin.to_vec2());
            if rule.intersects(sight) {
                ui.painter().rect_filled(rule, 0.0, p.line);
            }
        }
        let focus_slot = base.with("focus");
        let focused = ui.data(|data| data.get_temp::<usize>(focus_slot));
        let first = laid.items.partition_point(|item| origin.y + item.rect.bottom() < near.top());
        let mut holds = None;
        let mut seen_focused = false;
        for (index, item) in laid.items.iter().enumerate().skip(first) {
            let rect = item.rect.translate(origin.to_vec2());
            if rect.top() > near.bottom() {
                break;
            }
            seen_focused |= focused == Some(index);
            if self.item(ui, item, rect, base.with(index), sight) {
                holds = Some(index);
            }
        }
        // The link with the focus keeps it while it is far from sight.
        if let (Some(index), false) = (focused, seen_focused)
            && let Some(item) = laid.items.get(index)
            && self.item(ui, item, item.rect.translate(origin.to_vec2()), base.with(index), sight)
        {
            holds = Some(index);
        }
        ui.data_mut(|data| match holds {
            Some(index) => {
                data.insert_temp(focus_slot, index);
            }
            None => data.remove::<usize>(focus_slot),
        });
        holds.is_some()
    }

    /// Draws `item` at `rect`, painting it only where it meets `sight`; whether a link in it has the focus.
    fn item(&mut self, ui: &mut Ui, item: &Item, rect: Rect, id: Id, sight: Rect) -> bool {
        let theme = self.theme;
        let p = &theme.palette;
        let shows = rect.intersects(sight);
        match &item.kind {
            Kind::Text { galley, left, mark, links, spoken } => {
                if let Some(spoken) = spoken {
                    let response = ui.interact(rect, id.with("words"), Sense::hover());
                    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, spoken));
                }
                let at = pos2(rect.left() + left, rect.top());
                if shows {
                    if let Some(mark) = mark {
                        paint::run_at(ui.painter(), pos2(at.x - MARK, at.y), mark.clone(), p.ink);
                    }
                    paint::rows_at(ui.painter(), at, (rect.width() - left).max(1.0), galley.clone(), p.ink);
                }
                let mut holds = false;
                for (index, piece) in links.iter().enumerate() {
                    holds |= self.link_piece(ui, piece, at, id.with(("link", index)));
                }
                holds
            }
            Kind::Pre { galley, spoken } => {
                let response = ui.interact(rect, id.with("words"), Sense::hover());
                response.widget_info(|| WidgetInfo::labeled(Role::Label, true, spoken));
                if shows {
                    let painter = ui.painter();
                    painter.rect(rect, 0.0, p.term, Stroke::new(1.0, p.line), StrokeKind::Inside);
                    paint::rows_at(
                        painter,
                        rect.min + PRE_PAD + vec2(1.0, 0.0),
                        (rect.width() - PRE_PAD.x * 2.0 - 2.0).max(1.0),
                        galley.clone(),
                        p.ink,
                    );
                }
                false
            }
            Kind::Rule => {
                if shows {
                    paint::hairline(ui.painter(), rect.left_top(), pos2(rect.right() - 1.0, rect.top()), p.panel_line);
                }
                false
            }
            Kind::Picture { image, key } => {
                let response = self.picture_response(ui, rect, id, &image.alt, image.link);
                if shows
                    && let Some(PictureState::Ready(decoded)) = self.reader.picture(key)
                    && let Some(handle) = texture(ui, self.textures, key, decoded)
                {
                    ui.painter().image(handle.id(), rect, Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)), crate::theme::NO_TINT);
                }
                self.picture_link(ui, &response, rect, image.link)
            }
            Kind::Missing { galley, alt, link } => {
                let response = self.picture_response(ui, rect, id, alt, *link);
                if shows {
                    ui.painter().rect_stroke(rect, 0.0, Stroke::new(1.0, p.muted), StrokeKind::Inside);
                    paint::rows_at(ui.painter(), rect.min + vec2(9.0, 9.0), (rect.width() - 18.0).max(1.0), galley.clone(), p.muted);
                }
                self.picture_link(ui, &response, rect, *link)
            }
            Kind::Fold { lines, open } => {
                if fold_line(ui, theme, rect, rect.bottom(), id.with("fold"), *lines, *open, None).clicked() {
                    self.drawn.asked.push(Asked::Quotes(self.id));
                }
                false
            }
            Kind::Table(grid) => {
                let mut child = ui.new_child(UiBuilder::new().max_rect(rect).id_salt(id));
                let room = rect.width();
                let mut holds = false;
                if grid.size.x > room + 0.5 {
                    let area = ScrollArea::horizontal().id_salt(id.with("sideways")).auto_shrink([false, true]);
                    crate::component::scroll::show(&mut child, theme, area, |ui| {
                        let (_, at) = ui.allocate_space(grid.size);
                        holds = self.grid(ui, grid, at.min, id);
                    });
                } else {
                    let (_, at) = child.allocate_space(grid.size);
                    holds = self.grid(&mut child, grid, at.min, id);
                }
                if let Some(note) = &grid.note {
                    let at = Rect::from_min_size(pos2(rect.left(), rect.top() + grid.size.y + LIST_GAP), note.size());
                    let said = ui.interact(at, id.with("left out"), Sense::hover());
                    said.widget_info(|| WidgetInfo::labeled(Role::Label, true, note.text()));
                    if shows {
                        paint::rows_at(ui.painter(), at.min, rect.width(), note.clone(), p.muted);
                    }
                }
                holds
            }
        }
    }

    /// Draws `grid` with its top left at `origin`; whether a link in it has the focus.
    fn grid(&mut self, ui: &mut Ui, grid: &Grid, origin: Pos2, id: Id) -> bool {
        let p = &self.theme.palette;
        let sight = ui.clip_rect();
        let near = sight.expand2(vec2(0.0, sight.height().max(1.0)));
        let whole = Rect::from_min_size(origin, grid.size);
        let shows = whole.intersects(sight);
        let first = grid.rows.partition_point(|row| origin.y + row.top + row.high < near.top());
        if shows {
            let painter = ui.painter();
            for row in grid.rows.iter().skip(first).take_while(|row| origin.y + row.top <= sight.bottom()) {
                if row.head {
                    painter.rect_filled(
                        Rect::from_min_size(pos2(origin.x + 1.0, origin.y + row.top), vec2(grid.size.x - 2.0, row.high)),
                        0.0,
                        p.panel,
                    );
                }
                if row.top > 1.0 {
                    let y = origin.y + row.top - 1.0;
                    paint::hairline(painter, pos2(origin.x, y), pos2(origin.x + grid.size.x - 1.0, y), p.panel_line);
                }
            }
            for left in grid.lefts.iter().skip(1) {
                let x = origin.x + left - 1.0;
                let (top, bottom) = ((origin.y + 1.0).max(sight.top() - 1.0), (origin.y + grid.size.y - 1.0).min(sight.bottom() + 1.0));
                if top < bottom {
                    paint::hairline(painter, pos2(x, top), pos2(x, bottom - 1.0), p.panel_line);
                }
            }
        }
        let mut holds = false;
        for (row_index, row) in grid.rows.iter().enumerate().skip(first) {
            if origin.y + row.top > near.bottom() {
                break;
            }
            for (column, (cell, left)) in row.cells.iter().zip(&grid.lefts).enumerate() {
                let at = pos2(origin.x + left + CELL_PAD.x, origin.y + row.top + CELL_PAD.y);
                holds |= self.laid(ui, cell, at, id.with(("cell", row_index, column)));
            }
        }
        if shows {
            ui.painter().rect_stroke(whole, 0.0, Stroke::new(1.0, p.line), StrokeKind::Inside);
        }
        holds
    }

    /// The widget of a picture: a link (a Tab stop) when it is one, else what a screen reader hears of it.
    fn picture_response(&self, ui: &mut Ui, rect: Rect, id: Id, alt: &str, link: Option<usize>) -> egui::Response {
        let linked = link.and_then(|index| self.document.links.get(index));
        let sense = if linked.is_some() { Sense::click() } else { Sense::hover() };
        let response = ui.interact(rect, id.with("picture"), sense);
        let name = if alt.is_empty() { tr!("Picture").to_owned() } else { crate::component::spoken(alt) };
        let role = if linked.is_some() { Role::Link } else { Role::Image };
        response.widget_info(|| WidgetInfo::labeled(role, true, &name));
        response
    }

    /// What a picture's `response` asks as link `link`; whether it has the focus.
    fn picture_link(&mut self, ui: &Ui, response: &egui::Response, rect: Rect, link: Option<usize>) -> bool {
        let Some(link) = link.and_then(|index| self.document.links.get(index)) else { return false };
        let focus = response.has_focus();
        if response.hovered() || focus {
            self.drawn.hover = Some(link_status(link));
        }
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if response.gained_focus() {
            response.scroll_to_me(None);
        }
        if response.clicked() {
            copy(self.drawn, &link.target);
        }
        if focus {
            paint::focus_ring(ui.painter(), self.theme, rect, Ring::Outside);
        }
        focus
    }

    /// The link of `piece` in a text at `at`: a Tab stop on its first box, the pointer on all of them; whether it has
    /// the focus.
    fn link_piece(&mut self, ui: &mut Ui, piece: &Piece, at: Pos2, id: Id) -> bool {
        let Some(link) = self.document.links.get(piece.link) else { return false };
        let mut boxes = piece.rects.iter().map(|rect| rect.translate(at.to_vec2()));
        let Some(first) = boxes.next() else { return false };
        let response = ui.interact(first, id, Sense::click());
        let name = if link.text.trim().is_empty() { link_status(link) } else { crate::component::spoken(&link.text) };
        response.widget_info(|| WidgetInfo::labeled(Role::Link, true, &name));
        let rest: Vec<Rect> = boxes.collect();
        // The boxes after the first (a link that wraps) take the pointer too, but are no stop of their own.
        let pointer = ui.ctx().pointer_hover_pos();
        let over_rest = pointer.is_some_and(|pos| rest.iter().any(|rect| rect.contains(pos))) && ui.ui_contains_pointer();
        let hovered = response.hovered() || over_rest;
        let focus = response.has_focus();
        if hovered || focus {
            self.drawn.hover = Some(link_status(link));
        }
        if hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if response.gained_focus() {
            response.scroll_to_me(None);
        }
        if response.clicked() || (over_rest && ui.input(|input| input.pointer.primary_clicked())) {
            copy(self.drawn, &link.target);
        }
        if focus {
            for rect in std::iter::once(first).chain(rest) {
                paint::focus_ring(ui.painter(), self.theme, rect, Ring::Outside);
            }
        }
        focus
    }
}

/// Asks for the address of a link to `target` to be copied; a link that goes nowhere a front end follows copies
/// nothing.
fn copy(drawn: &mut Drawn, target: &LinkTarget) {
    match target {
        LinkTarget::Web { url, .. } | LinkTarget::Mail { url, .. } => drawn.asked.push(Asked::Copy(url.clone())),
        LinkTarget::Refused => {}
    }
}

/// The texture of `picture` kept under `key`, made the first time it is drawn.
fn texture<'t>(ui: &Ui, textures: &'t mut Textures, key: &str, picture: &Decoded) -> Option<&'t egui::TextureHandle> {
    if picture.rgba.len() != picture.width.saturating_mul(picture.height).saturating_mul(4) {
        return None;
    }
    if !textures.contains_key(key) {
        let image = ColorImage::from_rgba_unmultiplied([picture.width, picture.height], &picture.rgba);
        textures.insert(key.to_owned(), ui.ctx().load_texture(key, image, TextureOptions::LINEAR));
    }
    textures.get(key)
}
