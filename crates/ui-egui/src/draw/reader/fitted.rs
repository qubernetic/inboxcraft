// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! An HTML message fitted to the app: its blocks set in the app's type and colours, 12 apart. Paragraphs and list
//! items in `read` (a list's marks in a column of 24); a heading in Plex Sans SemiBold 22/29, a deeper one in `strong`
//! (a heading is the writer's words, and people's words are in Plex Sans); preformatted text in `mono_s` on `term` in a
//! 1 px `line` box, 8 × 12 inside; a rule as a `panel_line` hairline. A link is `cyan_text`, underlined, a Tab stop
//! with the focus ring round its words; Enter or a click copies its address and the status float says where it goes;
//! nothing opens. A paragraph is no Tab stop: a screen reader hears it as a label.
//!
//! A quote (`blockquote`) that follows the message's own text is folded behind a quote fold ("▸ 14 quoted lines");
//! open, its blocks stand under the fold in `muted`, the fold's 2 px edge running down beside them and each deeper
//! level with a 1 px `line` rule at its left, 12 in per level, to depth 4. A quote before any text of the message's own
//! stands open with a rule for each level.
//!
//! A table is a hairline table as its sheet draws one: a 1 px `line` edge round it, a 1 px `panel_line` between two
//! rows and none between the columns, the words 12 in from the sides and the columns 12 apart, a row of one line 32
//! high (a row of head cells 28, on `panel`, its words in `label` and `muted`) and a taller one 6 above and below its
//! words. Its columns get their widths from [`column_widths`], each what its words ask while all fit (the cleaner
//! keeps no widths of the mail's own), cut at whole pixels; a column keeps at least 36, and a table that cannot keep
//! that in its room scrolls sideways inside itself. Tables nest 4 deep; a deeper one is set as its cells' blocks one
//! after another. At most 64 columns and 1 000 rows are drawn; a line under the table says what was left out.
//!
//! Everything in a message is data and bounded: the blocks are laid out once for a message, a width and the state of
//! its quotes and pictures, and kept while they are drawn (again only when egui builds its font atlas afresh); each
//! frame only what is in sight is painted, and only what is within a pane's height of it takes the pointer and the
//! keyboard, with the nearest link before and after that (so Tab always reaches the next one) and the link that has
//! the focus, wherever it is.

use std::hash::{Hash as _, Hasher as _};
use std::sync::{Arc, Mutex};

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
const PRE_PAD: Vec2 = vec2(12.0, 8.0);

/// The room left and right of a table's cells and between two of its columns, in points.
const CELL_GAP: f32 = 12.0;

/// The room above and below the words of a table's row that holds more than a line, in points.
const CELL_PAD_Y: f32 = 6.0;

/// The least width of a table's column's words, in points (48 with the room after it).
const LEAST_COLUMN: f32 = 36.0;

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

/// The most letters the kept layouts hold together: about two messages as large as the cleaner lets one be. Only the
/// layouts drawn in the frame are kept at all (a thread may show more than one message); a message that does not fit
/// beside them is shown in the plain view instead.
const KEPT_GLYPHS: usize = 4 * 1024 * 1024;

/// A message's blocks laid out: what stands where, relative to the top left of the message.
pub(super) struct Laid {
    items: Vec<Item>,
    /// The quotes' rules and fold edges; once laid out, in columns (one for each left edge), each from the top down.
    rules: Vec<Rect>,
    /// The columns of `rules`: where each begins and ends in it.
    columns: Vec<std::ops::Range<usize>>,
    /// For each rule, the lowest foot of the rules of its column down to it, so the first in sight is found by halving.
    reach: Vec<f32>,
    /// The items that hold a Tab stop (a link, a fold, a table with a link), in order.
    stops: Vec<usize>,
    height: f32,
}

impl Laid {
    /// Nothing laid out.
    fn empty() -> Self {
        Self { items: Vec::new(), rules: Vec::new(), columns: Vec::new(), reach: Vec::new(), stops: Vec::new(), height: 0.0 }
    }

    /// Notes which items hold a Tab stop, once all are placed.
    fn find_stops(&mut self, document: &Document) {
        self.stops = self.items.iter().enumerate().filter(|(_, item)| item.kind.stops(document)).map(|(index, _)| index).collect();
    }

    /// Puts the rules in their columns, once all are made.
    fn order_rules(&mut self) {
        self.rules.sort_by(|a, b| a.left().total_cmp(&b.left()).then(a.top().total_cmp(&b.top())));
        self.columns.clear();
        self.reach.clear();
        let mut start = 0;
        while let Some(first) = self.rules.get(start) {
            let left = first.left();
            let length = self.rules.get(start..).map_or(0, |rest| rest.iter().take_while(|rule| rule.left() == left).count()).max(1);
            let end = start.saturating_add(length);
            let mut lowest = f32::NEG_INFINITY;
            for rule in self.rules.get(start..end).unwrap_or_default() {
                lowest = lowest.max(rule.bottom());
                self.reach.push(lowest);
            }
            self.columns.push(start..end);
            start = end;
        }
    }
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

impl Kind {
    /// Whether it holds a Tab stop.
    fn stops(&self, document: &Document) -> bool {
        let linked = |link: Option<usize>| link.is_some_and(|index| document.links.get(index).is_some());
        match self {
            Self::Text { links, .. } => !links.is_empty(),
            Self::Picture { image, .. } => linked(image.link),
            Self::Missing { link, .. } => linked(*link),
            Self::Fold { .. } => true,
            Self::Table(grid) => !grid.stops.is_empty(),
            Self::Pre { .. } | Self::Rule => false,
        }
    }
}

/// The words of one link in a text, as it wrapped: a box for each of its rows and the band of its underline there,
/// relative to the galley.
struct Piece {
    link: usize,
    rects: Vec<Rect>,
    bands: Vec<Rect>,
}

/// A run of one look in a text: its characters `start..end`, its link, and its underline's place under the baseline
/// and its thickness, in points.
struct Section {
    start: usize,
    end: usize,
    link: Option<usize>,
    under: (f32, f32),
}

/// A table laid out: its columns' lefts and widths, its rows, how wide and high it is, and what was left out.
struct Grid {
    lefts: Vec<f32>,
    rows: Vec<GridRow>,
    /// The rows that hold a Tab stop, in order.
    stops: Vec<usize>,
    size: Vec2,
    note: Option<Arc<Galley>>,
}

/// A row of a table: its top and height in the table, whether it is a row of head cells, its cells laid out.
struct GridRow {
    top: f32,
    high: f32,
    head: bool,
    cells: Vec<Laid>,
    /// How far under the row's top each cell's words stand.
    tops: Vec<f32>,
    /// The cells that hold a Tab stop, in order.
    stops: Vec<usize>,
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
    document: &'a Document,
    id: u64,
    quotes: bool,
    ppp: f32,
}

/// Draws `list`, blocks of message `id`'s `document` (its `generation`, which changes whenever the reader lays the
/// message's content out again), tables `depth` deep, across `ui`'s width; whether it did. It draws nothing where the
/// layout does not fit beside the ones kept for the other messages in sight (the caller shows the plain view).
#[expect(clippy::too_many_arguments, reason = "the drawing context of a message's blocks")]
pub(super) fn blocks(
    ui: &mut Ui,
    theme: &Theme,
    reader: &Reader,
    id: u64,
    generation: u64,
    document: &Document,
    list: &[Block],
    depth: usize,
    textures: &mut Textures,
    drawn: &mut Drawn,
) -> bool {
    let width = ui.available_width().max(1.0).floor();
    let quotes = reader.shows_quotes(id);
    let ppp = ui.ctx().pixels_per_point();
    // What the layout was made for: the width, the quotes, the pictures and the look; whose it is: the message, the
    // generation of its content and the depth (never where its blocks lie, a place the next list may take).
    let key = {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        (list.len(), width.to_bits(), ppp.to_bits(), quotes, reader.pictures_state()).hash(&mut hasher);
        (theme.palette.ink, theme.palette.cyan_text, theme.fonts.read.size.to_bits()).hash(&mut hasher);
        hasher.finish()
    };
    let who = {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        (id, generation, depth).hash(&mut hasher);
        hasher.finish()
    };
    let probe = ui.painter().layout_job(LayoutJob::single_section("fitted".to_owned(), theme.fonts.read.format(theme.palette.ink)));
    let Some(laid) = kept(ui, probe, who, key, || {
        let context = Context { ui, theme, reader, document, id, quotes, ppp };
        Arc::new(lay(&context, list, width, Style { text: theme.fonts.read, head: false }, depth))
    }) else {
        return false;
    };
    let (_, rect) = ui.allocate_space(vec2(width, laid.height));
    let base = ui.scope_id().with(("fitted", id, depth));
    let mut show = Show { theme, reader, id, document, textures, drawn };
    // What lies within a pane's height of the sight takes the pointer and the keyboard, so a screen reader finds the
    // words round what is shown; Tab reaches the links beyond through the nearest stop before and after.
    let sight = ui.clip_rect();
    show.laid(ui, &laid, rect.min, base, Reach::Near(sight.expand2(vec2(0.0, sight.height().max(1.0)))));
    true
}

#[cfg(test)]
thread_local! {
    /// The cap a test sets in place of [`KEPT_GLYPHS`] (egui_kittest draws on the test's own thread).
    static CAP: std::cell::Cell<usize> = const { std::cell::Cell::new(KEPT_GLYPHS) };
}

/// The most letters the kept layouts hold together: [`KEPT_GLYPHS`], or in a test the cap it set.
fn cap() -> usize {
    #[cfg(test)]
    return CAP.with(std::cell::Cell::get);
    #[cfg(not(test))]
    KEPT_GLYPHS
}

/// Runs `run` with the kept layouts capped at `cap` letters, then sets the cap back.
#[cfg(test)]
pub(crate) fn with_cap<T>(cap: usize, run: impl FnOnce() -> T) -> T {
    let before = CAP.with(|kept| kept.replace(cap));
    let out = run();
    CAP.with(|kept| kept.set(before));
    out
}

/// The layouts kept, the ones refused, the galley that shows whether egui built its font atlas afresh since they were
/// made, and how many layouts were made.
#[derive(Default)]
struct Store {
    probe: Option<Arc<Galley>>,
    keeps: Vec<Keep>,
    refused: Vec<Refused>,
    made: usize,
}

/// A layout made and not kept, as it did not fit beside the others: whose it is, what for, the pass that last asked for
/// it and how many letters it holds. Its message is shown plain while it would not fit.
struct Refused {
    who: u64,
    key: u64,
    pass: u64,
    glyphs: usize,
}

/// A layout kept: whose it is, what it was laid out for, the pass that last drew it and how many letters it holds.
struct Keep {
    who: u64,
    key: u64,
    pass: u64,
    glyphs: usize,
    laid: Arc<Laid>,
}

/// The kept layouts, in egui's memory for the window.
type Kept = Arc<Mutex<Store>>;

/// Where the kept layouts are.
fn kept_slot() -> Id {
    Id::unique("fitted-kept")
}

/// The layout kept for `who` under `key`, or the one `make` lays out, kept in its place. Only what is drawn is kept:
/// before a layout is made, every layout this pass has not drawn is dropped (a message left, an old width). The letters
/// kept together never pass [`KEPT_GLYPHS`]: a layout that does not fit beside the others is not kept, and its message
/// gets none (`None`, shown plain) until it fits, without being laid out again each frame; so a thread of huge messages
/// lays out nothing at rest, and a folder of them opened one after another holds one.
///
/// A kept layout's letters point into egui's font atlas as it was, and egui builds the atlas afresh when it fills or
/// the text options change, dropping every galley it keeps. `probe`, a word laid out through egui each frame, is the
/// same galley until then: when it is not, every kept layout is dropped and laid out again. Nothing else is laid out,
/// copied or hashed again per frame.
fn kept(ui: &Ui, probe: Arc<Galley>, who: u64, key: u64, make: impl FnOnce() -> Arc<Laid>) -> Option<Arc<Laid>> {
    let pass = ui.ctx().cumulative_pass_nr();
    let store: Kept = ui.data_mut(|data| data.get_temp_mut_or_default::<Kept>(kept_slot()).clone());
    let mut store = store.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if !store.probe.as_ref().is_some_and(|kept| Arc::ptr_eq(kept, &probe)) {
        store.keeps.clear();
        store.probe = Some(probe);
    }
    let Store { keeps, refused, made, .. } = &mut *store;
    if let Some(keep) = keeps.iter_mut().find(|keep| keep.who == who && keep.key == key) {
        keep.pass = pass;
        return Some(keep.laid.clone());
    }
    let total = keeps.iter().map(|keep| keep.glyphs).fold(0, usize::saturating_add);
    // Refused before: still plain while it would not fit beside what is kept, and not laid out again to see.
    if let Some(refusal) = refused.iter_mut().find(|refusal| refusal.who == who && refusal.key == key) {
        refusal.pass = pass;
        if refusal.glyphs.saturating_add(total) > cap() {
            return None;
        }
    }
    // Not kept: what this pass has not drawn or refused goes before the new layout is made.
    keeps.retain(|keep| keep.pass == pass && keep.who != who);
    refused.retain(|refusal| refusal.pass == pass && refusal.who != who);
    let laid = make();
    *made = made.saturating_add(1);
    let count = glyphs(&laid);
    let total = keeps.iter().map(|keep| keep.glyphs).fold(count, usize::saturating_add);
    if total > cap() {
        refused.push(Refused { who, key, pass, glyphs: count });
        return None;
    }
    keeps.push(Keep { who, key, pass, glyphs: count, laid: laid.clone() });
    Some(laid)
}

/// How many letters `laid` holds laid out: what a kept layout costs, about.
fn glyphs(laid: &Laid) -> usize {
    let of = |galley: &Galley| galley.rows.iter().map(|row| row.glyphs.len()).fold(0_usize, usize::saturating_add);
    laid.items
        .iter()
        .map(|item| match &item.kind {
            Kind::Text { galley, mark, .. } => of(galley).saturating_add(mark.as_ref().map_or(0, |mark| of(mark))),
            Kind::Pre { galley, .. } | Kind::Missing { galley, .. } => of(galley),
            Kind::Table(grid) => grid
                .rows
                .iter()
                .flat_map(|row| row.cells.iter())
                .map(glyphs)
                .fold(grid.note.as_ref().map_or(0, |note| of(note)), usize::saturating_add),
            Kind::Rule | Kind::Picture { .. } | Kind::Fold { .. } => 0,
        })
        .fold(0, usize::saturating_add)
}

/// How many layouts were made since the window began.
#[cfg(test)]
fn made_now(ctx: &egui::Context) -> usize {
    let store = ctx.data(|data| data.get_temp::<Kept>(kept_slot()));
    store.map_or(0, |store| store.lock().unwrap_or_else(std::sync::PoisonError::into_inner).made)
}

/// How many layouts are kept, and how many letters they hold together.
#[cfg(test)]
fn kept_now(ctx: &egui::Context) -> (usize, usize) {
    let store = ctx.data(|data| data.get_temp::<Kept>(kept_slot()));
    let kept = store.as_ref().map(|store| store.lock().unwrap_or_else(std::sync::PoisonError::into_inner));
    kept.map_or((0, 0), |kept| (kept.keeps.len(), kept.keeps.iter().map(|keep| keep.glyphs).fold(0, usize::saturating_add)))
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
    let mut laid = Laid::empty();
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
    laid.order_rules();
    laid.find_stops(context.document);
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
        TextKind::Preformatted => f.mono_s,
        _ if style.head => f.label,
        // A heading in a message is its writer's words: in Plex Sans SemiBold, 22/29 at the first level, `strong` below.
        TextKind::Heading(1) => f.written_title(),
        TextKind::Heading(_) => f.strong,
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
    let wrap = if pre { room - PRE_PAD.x * 2.0 } else { room - left };
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap.max(1.0);
    job.wrap.break_anywhere = pre;
    let mut sections: Vec<Section> = Vec::new();
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
        // A link's underline is painted as the design draws it (the face's own place and weight, see `pieces`).
        if span.underline && !linked {
            format.underline = Stroke::new(1.0, colour);
        }
        if span.strike {
            format.strikethrough = Stroke::new(1.0, colour);
        }
        let shown = font.text(&crate::component::plain_lines(&span.text));
        let count = shown.chars().count();
        let (below, thick) = face.underline();
        sections.push(Section { start: chars, end: chars.saturating_add(count), link: span.link, under: (below * font.size, thick * font.size) });
        chars = chars.saturating_add(count);
        words.push_str(crate::quote::cut(&shown, (MOST_SPOKEN * 4).saturating_sub(words.len())));
        job.append(&shown, 0.0, format);
    }
    let galley = paint::on_baseline(context.ui.painter().layout_job(job));
    // A link the message does not have is no link.
    let mut links = pieces(&galley, &sections);
    links.retain(|piece| context.document.links.get(piece.link).is_some());
    if pre {
        let size = vec2(room, (galley.size().y + PRE_PAD.y * 2.0).ceil());
        let spoken = crate::component::spoken(crate::component::capped(&words, MOST_SPOKEN).0);
        return Some((Kind::Pre { galley, spoken }, size));
    }
    let only_links = sections.iter().all(|section| section.link.is_some() || section.start == section.end);
    let spoken = (!only_links).then(|| crate::component::spoken(crate::component::capped(&words, MOST_SPOKEN).0));
    let size = vec2(room, galley.size().y.ceil().max(font.line));
    Some((Kind::Text { galley, left, mark, links, spoken }, size))
}

/// The boxes of the links of `galley`, whose `sections` say which characters belong to which link.
fn pieces(galley: &Galley, sections: &[Section]) -> Vec<Piece> {
    let mut found: Vec<Piece> = Vec::new();
    let mut start = 0_usize;
    let mut section = 0_usize;
    // The link of the character before, so a link that runs on keeps one piece.
    let mut running: Option<usize> = None;
    for row in &galley.rows {
        let rect = row.rect();
        for (offset, glyph) in row.glyphs.iter().enumerate() {
            let at = start.saturating_add(offset);
            while sections.get(section).is_some_and(|run| run.end <= at) {
                section += 1;
            }
            let Some((link, (below, thick))) =
                sections.get(section).filter(|run| run.start <= at).and_then(|run| run.link.map(|link| (link, run.under)))
            else {
                running = None;
                continue;
            };
            let left = rect.left() + glyph.pos.x;
            let right = left + glyph.advance_width;
            let boxed = Rect::from_min_max(pos2(left, rect.top()), pos2(right, rect.bottom()));
            let middle = rect.top() + glyph.pos.y + below;
            let band = Rect::from_min_max(pos2(left, middle - thick / 2.0), pos2(right, middle + thick / 2.0));
            match found.last_mut() {
                Some(piece) if running == Some(link) && piece.link == link => match (piece.rects.last_mut(), piece.bands.last_mut()) {
                    (Some(last), Some(under)) if (last.top() - boxed.top()).abs() < 0.5 => {
                        *last = last.union(boxed);
                        *under = under.union(band);
                    }
                    _ => {
                        piece.rects.push(boxed);
                        piece.bands.push(band);
                    }
                },
                _ => found.push(Piece { link, rects: vec![boxed], bands: vec![band] }),
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
            columns.iter().map(|column| column + CELL_GAP).sum::<f32>() + CELL_GAP
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

/// A table laid out in `room` as the hairline table's sheet draws one: its columns' words 12 in from the sides and 12
/// apart, their widths from [`column_widths`] (each what its widest words ask while all fit, the table no wider) cut at
/// whole pixels and at least [`LEAST_COLUMN`] (wider than the room, it scrolls); a row of one line 32 high and a head
/// 28, its cells' words in the middle; a row of more lines 6 above and below its tallest cell, its cells at the top; a
/// 1 px rule between two rows.
fn grid(context: &Context<'_>, rows: &[Vec<Cell>], room: f32, depth: usize) -> Grid {
    let theme = context.theme;
    let wanted = wanted_columns(context, rows, depth);
    let count = wanted.len();
    let gaps = CELL_GAP * (count as f32 + 1.0);
    let least = LEAST_COLUMN * count as f32;
    let widths = column_widths(&wanted, (room - gaps).max(least), 0.0);
    let ppp = if context.ppp.is_finite() && context.ppp > 0.0 { context.ppp } else { 1.0 };
    let widths: Vec<f32> = widths.into_iter().map(|width| ((width * ppp).floor() / ppp).max(1.0)).collect();
    let mut lefts = Vec::with_capacity(count);
    let mut x = CELL_GAP;
    for width in &widths {
        lefts.push(x);
        x += width + CELL_GAP;
    }
    let [head_high, row_high, _] = theme.boxes().table;
    let mut laid_rows = Vec::new();
    let mut top = 0.0_f32;
    for row in rows.iter().take(MOST_ROWS) {
        if !laid_rows.is_empty() {
            top += 1.0;
        }
        let head = !row.is_empty() && row.iter().all(|cell| cell.header);
        let style = Style { text: theme.fonts.body, head };
        let cells: Vec<Laid> = widths
            .iter()
            .enumerate()
            .map(|(index, width)| match row.get(index) {
                Some(cell) => lay(context, &cell.blocks, *width, style, depth.saturating_add(1)),
                None => Laid::empty(),
            })
            .collect();
        // A row of one line (of a head's labels, or of the cells' words) stands as high as the sheet's; a taller one
        // takes its tallest cell and 6 above and below.
        let (line, least) = if head { (theme.fonts.label.line, head_high) } else { (theme.fonts.body.line, row_high) };
        let tallest = cells.iter().map(|cell| cell.height).fold(0.0, f32::max);
        let single = tallest <= line;
        let high = if single { least } else { (tallest + CELL_PAD_Y * 2.0).ceil().max(least) };
        let tops = cells.iter().map(|cell| if single { ((high - cell.height) / 2.0).round().max(0.0) } else { CELL_PAD_Y }).collect();
        let stops = cells.iter().enumerate().filter(|(_, cell)| !cell.stops.is_empty()).map(|(index, _)| index).collect();
        laid_rows.push(GridRow { top, high, head, cells, tops, stops });
        top += high;
    }
    let shown_columns = rows.iter().take(MOST_ROWS).map(Vec::len).max().unwrap_or(0);
    let note = crate::component::hairline_table::left_out(shown_columns.saturating_sub(count), rows.len().saturating_sub(MOST_ROWS))
        .map(|note| crate::component::wrapped(context.ui, &note, theme.fonts.caption, theme.palette.muted, room.max(1.0), 2, egui::Align::LEFT));
    let stops = laid_rows.iter().enumerate().filter(|(_, row)| !row.stops.is_empty()).map(|(index, _)| index).collect();
    Grid { lefts, rows: laid_rows, stops, size: vec2(x, top.max(2.0)), note }
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

/// What of a laid out part takes the pointer and the keyboard this frame.
#[derive(Clone, Copy)]
enum Reach {
    /// What meets the rect (sight and a pane's height round it), with the nearest stop before it and the nearest after:
    /// Tab and Shift Tab always have the next link to go to, however far, and the focus brings it into sight.
    Near(Rect),
    /// Only its first stop.
    First,
    /// Only its last stop.
    Last,
    /// Only the stop that has the focus.
    Held,
}

/// Which of the parts of something to draw for `reach`, in their order: those in `near` (the parts meeting the reach's
/// rect) with the nearest stop `before` and `after` them, or the `first` or `last` stop, and the part `focused` (with
/// the focus), each with what of it to draw.
fn picks(
    reach: Reach,
    near: std::ops::Range<usize>,
    (before, after): (Option<usize>, Option<usize>),
    (first, last): (Option<usize>, Option<usize>),
    focused: Option<usize>,
) -> Vec<(usize, Reach)> {
    let mut picks = Vec::new();
    match reach {
        Reach::Near(_) => {
            picks.extend(before.map(|index| (index, Reach::Last)));
            picks.extend(near.map(|index| (index, reach)));
            picks.extend(after.map(|index| (index, Reach::First)));
        }
        Reach::First => picks.extend(first.map(|index| (index, Reach::First))),
        Reach::Last => picks.extend(last.map(|index| (index, Reach::Last))),
        Reach::Held => {}
    }
    if let Some(index) = focused
        && !picks.iter().any(|(picked, _)| *picked == index)
    {
        let place = picks.partition_point(|(picked, _)| *picked < index);
        picks.insert(place, (index, Reach::Held));
    }
    picks
}

/// The nearest of the ordered `stops` before index `start` and the nearest at or after `end`.
fn around(stops: &[usize], start: usize, end: usize) -> (Option<usize>, Option<usize>) {
    let before = stops.partition_point(|stop| *stop < start).checked_sub(1).and_then(|at| stops.get(at)).copied();
    let after = stops.get(stops.partition_point(|stop| *stop < end)).copied();
    (before, after)
}

/// Remembers under `slot` which part has the focus (`holds`), or forgets it.
fn remember<T: Clone + Send + Sync + 'static>(ui: &Ui, slot: Id, holds: Option<T>) {
    ui.data_mut(|data| match holds {
        Some(part) => {
            data.insert_temp(slot, part);
        }
        None => data.remove::<T>(slot),
    });
}

impl Show<'_> {
    /// Draws what of `laid` `reach` asks, with its top left at `origin`, its widgets' ids from `base`; whether one of
    /// its links has the focus.
    fn laid(&mut self, ui: &mut Ui, laid: &Laid, origin: Pos2, base: Id, reach: Reach) -> bool {
        let sight = ui.clip_rect();
        let p = &self.theme.palette;
        if let Reach::Near(_) = reach {
            // The rules in sight, column by column: the first found by halving, the rest until one starts below sight.
            for column in &laid.columns {
                let (Some(rules), Some(reach)) = (laid.rules.get(column.clone()), laid.reach.get(column.clone())) else { continue };
                let first = reach.partition_point(|foot| origin.y + foot < sight.top());
                for rule in rules.iter().skip(first) {
                    let rule = rule.translate(origin.to_vec2());
                    if rule.top() > sight.bottom() {
                        break;
                    }
                    if rule.intersects(sight) {
                        ui.painter().rect_filled(rule, 0.0, p.line);
                    }
                }
            }
        }
        let focus_slot = base.with("focus");
        let focused = ui.data(|data| data.get_temp::<usize>(focus_slot));
        let near = match reach {
            Reach::Near(near) => {
                let start = laid.items.partition_point(|item| origin.y + item.rect.bottom() < near.top());
                let length =
                    laid.items.get(start..).map_or(0, |rest| rest.iter().take_while(|item| origin.y + item.rect.top() <= near.bottom()).count());
                start..start.saturating_add(length)
            }
            Reach::First | Reach::Last | Reach::Held => 0..0,
        };
        let edges = (laid.stops.first().copied(), laid.stops.last().copied());
        let mut holds = None;
        for (index, part) in picks(reach, near.clone(), around(&laid.stops, near.start, near.end), edges, focused) {
            let Some(item) = laid.items.get(index) else { continue };
            if self.item(ui, item, item.rect.translate(origin.to_vec2()), base.with(index), sight, part) {
                holds = Some(index);
            }
        }
        remember(ui, focus_slot, holds);
        holds.is_some()
    }

    /// Draws `item` at `rect`, painting it only where it meets `sight` and giving the pointer and the keyboard what
    /// `reach` asks; whether a link in it has the focus.
    fn item(&mut self, ui: &mut Ui, item: &Item, rect: Rect, id: Id, sight: Rect, reach: Reach) -> bool {
        let theme = self.theme;
        let p = &theme.palette;
        let shows = rect.intersects(sight);
        match &item.kind {
            Kind::Text { galley, left, mark, links, spoken } => {
                if let (Some(spoken), Reach::Near(_)) = (spoken, reach) {
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
                // Only the links near sight take the pointer and the keyboard (found by halving: a paragraph may hold
                // thousands), with the nearest before and after them and the one with the focus.
                let slot = id.with("focus");
                let focused = ui.data(|data| data.get_temp::<usize>(slot));
                let near = match reach {
                    Reach::Near(near) => {
                        let start = links.partition_point(|piece| piece.rects.last().is_some_and(|last| at.y + last.bottom() < near.top()));
                        let length = links.get(start..).map_or(0, |rest| {
                            rest.iter().take_while(|piece| piece.rects.first().is_some_and(|top| at.y + top.top() <= near.bottom())).count()
                        });
                        start..start.saturating_add(length)
                    }
                    Reach::First | Reach::Last | Reach::Held => 0..0,
                };
                let around = (near.start.checked_sub(1), (near.end < links.len()).then_some(near.end));
                let edges = ((!links.is_empty()).then_some(0), links.len().checked_sub(1));
                let mut holds = None;
                for (index, _) in picks(reach, near, around, edges, focused) {
                    if let Some(piece) = links.get(index)
                        && self.link_piece(ui, piece, at, id.with(("link", index)))
                    {
                        holds = Some(index);
                    }
                }
                remember(ui, slot, holds);
                holds.is_some()
            }
            Kind::Pre { galley, spoken } => {
                if let Reach::Near(_) = reach {
                    let response = ui.interact(rect, id.with("words"), Sense::hover());
                    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, spoken));
                }
                if shows {
                    let painter = ui.painter();
                    painter.rect(rect, 0.0, p.term, Stroke::new(1.0, p.line), StrokeKind::Inside);
                    paint::rows_at(painter, rect.min + PRE_PAD, (rect.width() - PRE_PAD.x * 2.0).max(1.0), galley.clone(), p.ink);
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
                        holds = self.grid(ui, grid, at.min, id, reach);
                    });
                } else {
                    let (_, at) = child.allocate_space(grid.size);
                    holds = self.grid(&mut child, grid, at.min, id, reach);
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

    /// Draws `grid` with its top left at `origin`, giving the pointer and the keyboard what `reach` asks; whether a
    /// link in it has the focus.
    fn grid(&mut self, ui: &mut Ui, grid: &Grid, origin: Pos2, id: Id, reach: Reach) -> bool {
        let p = &self.theme.palette;
        let sight = ui.clip_rect();
        let whole = Rect::from_min_size(origin, grid.size);
        let shows = whole.intersects(sight);
        if shows {
            let painter = ui.painter();
            let first = grid.rows.partition_point(|row| origin.y + row.top + row.high < sight.top());
            for row in grid.rows.iter().skip(first).take_while(|row| origin.y + row.top <= sight.bottom()) {
                if row.head {
                    painter.rect_filled(Rect::from_min_size(pos2(origin.x, origin.y + row.top), vec2(grid.size.x, row.high)), 0.0, p.panel);
                }
                if row.top > 0.0 {
                    let y = origin.y + row.top - 1.0;
                    paint::hairline(painter, pos2(origin.x, y), pos2(origin.x + grid.size.x - 1.0, y), p.panel_line);
                }
            }
        }
        let near = match reach {
            Reach::Near(near) => {
                let start = grid.rows.partition_point(|row| origin.y + row.top + row.high < near.top());
                let length = grid.rows.get(start..).map_or(0, |rest| rest.iter().take_while(|row| origin.y + row.top <= near.bottom()).count());
                start..start.saturating_add(length)
            }
            Reach::First | Reach::Last | Reach::Held => 0..0,
        };
        let edges = (grid.stops.first().copied(), grid.stops.last().copied());
        // The cell with the focus keeps its row drawn, as a paragraph's link does, wherever it is.
        let slot = id.with("focus");
        let focused = ui.data(|data| data.get_temp::<(usize, usize)>(slot));
        let mut holds = None;
        for (row_index, part) in picks(reach, near.clone(), around(&grid.stops, near.start, near.end), edges, focused.map(|(row, _)| row)) {
            let Some(row) = grid.rows.get(row_index) else { continue };
            // A row in reach draws every cell; a row reached for its first or last stop draws that cell only.
            let mut cells: Vec<(usize, Reach)> = match part {
                Reach::Near(_) => (0..row.cells.len()).map(|column| (column, part)).collect(),
                Reach::First => row.stops.first().map(|column| (*column, part)).into_iter().collect(),
                Reach::Last => row.stops.last().map(|column| (*column, part)).into_iter().collect(),
                Reach::Held => Vec::new(),
            };
            if let Some((_, column)) = focused.filter(|(row, _)| *row == row_index)
                && !cells.iter().any(|(picked, _)| *picked == column)
            {
                let place = cells.partition_point(|(picked, _)| *picked < column);
                cells.insert(place, (column, Reach::Held));
            }
            for (column, part) in cells {
                let (Some(cell), Some(left)) = (row.cells.get(column), grid.lefts.get(column)) else { continue };
                let at = pos2(origin.x + left, origin.y + row.top + row.tops.get(column).copied().unwrap_or(CELL_PAD_Y));
                if self.laid(ui, cell, at, id.with(("cell", row_index, column)), part) {
                    holds = Some((row_index, column));
                }
            }
        }
        remember(ui, slot, holds);
        let holds = holds.is_some();
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
        if ui.is_rect_visible(first) || rest.iter().any(|rect| ui.is_rect_visible(*rect)) {
            for band in &piece.bands {
                let band = band.translate(at.to_vec2());
                paint::sampled_band(ui.painter(), band.left()..=band.right(), (band.top(), band.bottom()), self.theme.palette.cyan_text);
            }
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

/// The words of the gallery's fitted message: a heading, a paragraph with a link, a list, a table with its head,
/// preformatted text and a quote two levels deep. Invented, as all sample mail is.
const BOARD: &str = "<h1>Allotment rota</h1><p>Here is the plan for November. The <b>water butts</b> need emptying before \
    the frost; <a href=\"https://allotments.example.org/rota\">the full rota</a> is on the site.</p>\
    <ul><li>Week one: Odile</li><li>Week two: Mira</li></ul>\
    <table><tr><th>Week</th><th>Who</th><th>Plot</th></tr><tr><td>1</td><td>Odile Brannock</td><td>14</td></tr>\
    <tr><td>2</td><td>Mira Holloway, with the shed key and the long rake</td><td>9</td></tr></table>\
    <pre>gate code: 4417</pre>\
    <blockquote><p>On Mon 16 Nov, Teodor wrote:</p><p>Could you take the second week?</p>\
    <blockquote><p>The shed key is under the pot.</p></blockquote></blockquote>";

/// A fitted message on the reader's gallery page across `ui`'s width, its quote folded or `open`.
pub(crate) fn board(ui: &mut Ui, theme: &Theme, open: bool) {
    let document = inboxcraft_html::display(BOARD, &inboxcraft_html::Options::default());
    let mut reader = Reader::new(Box::<crate::fetch::Offline>::default(), Box::new(crate::save::NoSave));
    let id = if open { 2 } else { 1 };
    if open {
        reader.toggle_quotes(id);
    }
    let mut textures = Textures::default();
    let mut drawn = Drawn::default();
    blocks(ui, theme, &reader, id, 0, &document, &document.blocks, 0, &mut textures, &mut drawn);
}

#[cfg(test)]
#[path = "fitted_tests.rs"]
mod tests;
