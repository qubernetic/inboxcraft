// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The character field and the emblem (`docs/design.md`, "The character field", "Home" and the rows M1 and M2 of
//! "Motion"): the ground of Home, the first start and the cover.
//!
//! The field is a grid of 8 × 15 point cells from its rect's top left corner; about 5 in 100 hold a `·` in `faint`.
//! Which ones is a small hash of the cell's column and row with the field's seed, so the same rect, seed and text
//! scale give the same cells on every frame, launch and machine; the pattern never moves by itself. At a larger text
//! the cell's width stays and its height follows the text (15 × the scale, to the whole point: 20 at 130 %); the dot
//! keeps its size.
//!
//! The emblem is a grid of character cells the caller hands in ([`Emblem`]), drawn at its one size (12 point mono
//! on 7.2 × 12.5 point cells) centred in the area the caller names, and only if it fits there whole: never cut,
//! never smaller. The field's cells it covers stay empty.
//!
//! Under the pointer (M1) the field's cells within 60 points flip to code glyphs, one in four tinted `cyan_text`, the
//! rest `dim`; at the reduced level only Home's field answers, within 30 points; at off the field is still dots. A
//! cell flips at once, holds its glyph while the pointer stays near it, and cools back to what it was within 200 ms
//! of the pointer leaving it (each cell at a time of its own, so the trail thins out). The emblem's cells (M2) flip
//! and settle back the same way within 60 points, at full and at reduced alike; at off it is still. Which cells flip,
//! how long each cools and the glyph each flips to are hashes too, chosen once per flip: a flipped cell never
//! changes its glyph until it has cooled.
//!
//! The cells are built once, at the first [`show`], for as many whole cells as the rect holds, the seed, the text
//! scale, the pixels per point and where the emblem stands on them, and again only when one of these changes: a rect
//! that moves moves the cells and builds nothing, and the cells warm under the pointer keep their glyphs (when the
//! cells are built again, the warm ones still there keep theirs); the row texts are built once for the theme's mode.
//! A frame at rest paints them (one text a row, each found again in egui's galley cache) and builds nothing; while
//! cells are warm, only the rows that hold one are built again, and a repaint is asked for only while a cell cools. The field
//! takes no click, no key and no focus, and shows nothing to a screen reader: it only paints.

use std::collections::BTreeMap;
use std::sync::Arc;

use egui::epaint::TextShape;
use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, Galley, Pos2, Rect, Ui, Vec2, pos2, vec2};

use crate::theme::{Font, Mode, TextScale, Theme};

/// A cell's width, in points, at every text scale.
pub const CELL_WIDTH: f32 = 8.0;

/// A cell's height at 100 % text, in points.
pub const CELL_HEIGHT: f32 = 15.0;

/// How many cells in 100 hold a dot.
pub const DOTS_IN_100: u32 = 5;

/// How far from the pointer the field's cells flip at the full level, in points.
pub const REACH_FULL: f32 = 60.0;

/// How far from the pointer Home's field's cells flip at the reduced level, in points.
pub const REACH_REDUCED: f32 = 30.0;

/// How far from the pointer the emblem's cells flip at the full and the reduced level, in points.
pub const EMBLEM_REACH: f32 = 60.0;

/// The `field_trail` token: the longest a cell takes to cool down once the pointer has left it, in seconds.
pub const TRAIL: f64 = 0.2;

/// The code glyphs a cell flips to.
pub const CODE: &[u8] = b"01<>/\\[]{}()#$%&*+=-_:;!?|~^";

/// The emblem's ramp: a cell of density `d` (1 to 9) is drawn as the `d`th character.
pub const RAMP: &[u8] = b" .:-=+*#%@";

/// One of the emblem's cells, in points.
pub const EMBLEM_CELL: Vec2 = vec2(7.2, 12.5);

/// How strongly the field shows on the empty area of a pane: 20 %.
pub const PANE_STRENGTH: f32 = 0.2;

/// The most columns a field has: 8 192 points at 8 a cell, wider than an 8K screen at one pixel a point. A wider
/// rect is covered from its left edge up to this.
pub const MAX_COLUMNS: usize = 1024;

/// The most rows a field has: 7 680 points at 15 a row. A taller rect is covered from its top edge up to this.
pub const MAX_ROWS: usize = 512;

/// The most pixels per point a field is built at; more are taken as not usable, and nothing is built.
pub const MAX_PPP: f32 = 1024.0;

/// The farthest a field's corner or its emblem's area reaches from 0, in points, either way: a rect reaching farther is
/// clamped to it.
pub const MAX_COORDINATE: f32 = 1.0e7;

/// The most cells warm at once; past it no further cell flips until some have cooled. The pointer's reach holds
/// about a hundred at a time.
pub const MAX_WARM: usize = 4096;

/// The most columns of an emblem; an emblem handed in wider is cut to it.
pub const MAX_EMBLEM_COLUMNS: usize = 160;

/// The most rows of an emblem; an emblem handed in taller is cut to it.
pub const MAX_EMBLEM_ROWS: usize = 80;

/// How much the window moves (`docs/design.md`, "Motion"): passed in by the caller, never read here.
///
/// The level belongs with the rest of the motion levels (`crate::motion`) once they are there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// Every movement.
    Full,
    /// The calm ones: the field answers only on Home, and nearer; the emblem as at full.
    Reduced,
    /// None: still dots and a still emblem.
    Off,
}

/// Where a field lies and what stands on it, as [`show`] is told each frame.
#[derive(Debug, Clone, Copy, Default)]
pub struct Opts {
    /// Whether this is Home's field: at the reduced level only Home's field answers the pointer.
    pub on_home: bool,
    /// Whether the field lies on the empty area of a pane, where it shows at [`PANE_STRENGTH`]; otherwise (Home, the
    /// first start, the cover) it is the ground at full strength.
    pub on_pane: bool,
    /// The area the emblem stands in, centred; it is drawn only if it fits there whole. `None`: no emblem.
    pub emblem: Option<Rect>,
}

/// An emblem: a grid of character cells, each off, or on with a density of 1 to 9 in ink or in cyan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Emblem {
    columns: usize,
    rows: usize,
    /// Row by row: 0 off, 1 to 9 an ink cell of that density, 11 to 19 a cyan cell of density 1 to 9.
    cells: Vec<u8>,
}

/// What a cell of [`Emblem::cells`] holds above this is cyan.
const CYAN: u8 = 10;

impl Emblem {
    /// The emblem drawn by `rows`, one string a row: a space (or anything else not listed here) is an empty cell, a
    /// character of the ramp `.:-=+*#%@` an ink cell of its density, a digit `1` to `9` a cyan cell of that density.
    /// Rows and columns past [`MAX_EMBLEM_ROWS`] and [`MAX_EMBLEM_COLUMNS`] are left out.
    #[must_use]
    pub fn from_rows(rows: &[&str]) -> Self {
        let rows = rows.get(..rows.len().min(MAX_EMBLEM_ROWS)).unwrap_or_default();
        let columns = rows.iter().map(|row| row.chars().take(MAX_EMBLEM_COLUMNS).count()).max().unwrap_or(0);
        let mut cells = vec![0; columns.saturating_mul(rows.len())];
        for (r, row) in rows.iter().enumerate() {
            for (c, ch) in row.chars().take(columns).enumerate() {
                let held = match ch {
                    '1'..='9' => u8::try_from(ch).map_or(0, |digit| digit.saturating_sub(b'0').saturating_add(CYAN)),
                    _ => RAMP.iter().skip(1).position(|ramp| char::from(*ramp) == ch).and_then(|at| u8::try_from(at + 1).ok()).unwrap_or(0),
                };
                if let Some(cell) = cells.get_mut(r.saturating_mul(columns).saturating_add(c)) {
                    *cell = held;
                }
            }
        }
        Self { columns, rows: rows.len(), cells }
    }

    /// Its columns and rows.
    #[must_use]
    pub const fn grid(&self) -> (usize, usize) {
        (self.columns, self.rows)
    }

    /// Its one size, in whole points: the columns and rows of [`EMBLEM_CELL`].
    #[must_use]
    pub fn size(&self) -> Vec2 {
        // At most 160 columns and 80 rows: the casts lose nothing.
        vec2((self.columns as f32 * EMBLEM_CELL.x).round(), (self.rows as f32 * EMBLEM_CELL.y).round())
    }

    /// The cell at `column` and `row`: its density (0 off) and whether it is cyan.
    fn cell(&self, column: usize, row: usize) -> (u8, bool) {
        if column >= self.columns {
            return (0, false);
        }
        let held = self.cells.get(row.saturating_mul(self.columns).saturating_add(column)).copied().unwrap_or(0);
        if held > CYAN { (held - CYAN, true) } else { (held, false) }
    }
}

/// A warm cell's place: one of the field's or one of the emblem's, by its index row by row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Spot {
    Field(u32),
    Emblem(u32),
}

/// A cell flipped under the pointer.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Warm {
    /// The code glyph it flipped to, an index into [`CODE`].
    glyph: u8,
    /// Whether it took the other colour: `cyan_text` on the field, the emblem's other colour on the emblem.
    tint: bool,
    /// When the pointer left it, an input time in seconds; `None` while the pointer is near.
    left: Option<f64>,
    /// How long it takes to cool once left, in seconds: up to [`TRAIL`].
    cool: f64,
    /// The update it was last within reach in.
    seen: u64,
}

/// What a field's cell holds.
const EMPTY: u8 = 0;
const DOT: u8 = 1;
/// Under one of the emblem's cells: stays empty, never flips.
const UNDER: u8 = 2;

/// The cells of a field as they were built, and where they stand this frame.
#[derive(Debug, Clone)]
struct Cells {
    /// The rect asked for this frame.
    rect: Rect,
    scale: TextScale,
    /// The pixels per point, sanitised: 1 for a value that is not a usable one.
    ppp: f32,
    /// The top left corner of the first cell, on a whole pixel.
    origin: Pos2,
    /// A cell's height, in points.
    height: f32,
    columns: usize,
    rows: usize,
    /// Row by row: [`EMPTY`], [`DOT`] or [`UNDER`].
    held: Vec<u8>,
    /// Where the emblem's top left corner stands, if it is drawn.
    emblem_at: Option<Pos2>,
}

impl Cells {
    /// Where the emblem stands from the first cell, in whole pixels: which cells it covers depends on this, not on
    /// where the two stand.
    fn emblem_offset(&self) -> Option<[f32; 2]> {
        self.emblem_at.map(|corner| [((corner.x - self.origin.x) * self.ppp).round(), ((corner.y - self.origin.y) * self.ppp).round()])
    }

    /// Whether `other` holds the same cells: as many columns and rows, of the same height, at the same pixels per
    /// point, with the emblem at the same place on them. Where they stand does not count.
    fn same_cells(&self, other: &Self) -> bool {
        self.columns == other.columns
            && self.rows == other.rows
            && self.scale == other.scale
            && self.ppp.to_bits() == other.ppp.to_bits()
            && self.emblem_offset() == other.emblem_offset()
    }
}

/// The row texts built for one mode and one build, as jobs: egui's galley cache turns a job it laid out last frame
/// into its galley again without laying it out. A galley is never kept across frames, since egui may drop its glyph
/// atlas at the start of any frame and a kept galley would point into the old one.
#[derive(Debug, Clone)]
struct Look {
    mode: Mode,
    build: u32,
    rows: Vec<Option<LayoutJob>>,
    emblem_rows: Vec<Option<LayoutJob>>,
}

/// A character field: its cells, the emblem on it and the cells warm under the pointer. Make it once and keep it;
/// [`show`] paints it each frame.
#[derive(Debug, Clone)]
pub struct Field {
    seed: u32,
    emblem: Option<Emblem>,
    cells: Option<Cells>,
    look: Option<Look>,
    warm: BTreeMap<Spot, Warm>,
    /// Updates so far.
    frame: u64,
    /// The input time of the last update, in seconds: a cell the pointer was near then and is not now was left then.
    last: Option<f64>,
    /// Flips so far: a flip's glyph is a hash of its cell and this.
    flips: u32,
    /// How many times the cells were built.
    builds: u32,
    /// How many row texts were built.
    layouts: u64,
}

impl Field {
    /// A field with the pattern of `seed`. Its cells are built at its first [`show`], for the rect and the text scale
    /// that [`show`] is given.
    #[must_use]
    pub fn new(seed: u32) -> Self {
        Self { seed, emblem: None, cells: None, look: None, warm: BTreeMap::new(), frame: 0, last: None, flips: 0, builds: 0, layouts: 0 }
    }

    /// The same field with `emblem` on it, where [`Opts::emblem`] puts it.
    #[must_use]
    pub fn with_emblem(mut self, emblem: Emblem) -> Self {
        self.emblem = Some(emblem);
        self.cells = None;
        self.look = None;
        self
    }

    /// Builds the cells again if their columns or rows, the text scale, the pixels per point or where the emblem
    /// stands on them changed. A rect that only moved moves the cells, and the warm ones keep their glyph and their
    /// time; when the cells are built again, the warm ones that are still there keep theirs.
    fn build(&mut self, rect: Rect, scale: TextScale, area: Option<Rect>, ppp: f32) {
        let mut placed = place_cells(rect, scale, area, ppp, self.emblem.as_ref());
        if let Some(cells) = &mut self.cells
            && cells.same_cells(&placed)
        {
            cells.rect = placed.rect;
            cells.origin = placed.origin;
            cells.emblem_at = placed.emblem_at;
            return;
        }
        fill(&mut placed, self.seed, self.emblem.as_ref());
        let before = self.cells.as_ref().map_or(0, |cells| cells.columns);
        let warm = std::mem::take(&mut self.warm);
        self.warm = warm.into_iter().filter_map(|(spot, warm)| Some((still_there(spot, before, &placed)?, warm))).collect();
        self.cells = Some(placed);
        self.look = None;
        self.builds = self.builds.saturating_add(1);
    }

    /// Builds the row texts again if they were built for another build or mode (new pixels per point build the cells
    /// again).
    fn lay_out(&mut self, theme: &Theme) {
        let Some(cells) = &self.cells else { return };
        let fresh = self.look.as_ref().is_some_and(|look| look.mode == theme.mode && look.build == self.builds);
        if fresh {
            return;
        }
        let mut layouts = 0_u64;
        let rows = (0..cells.rows)
            .map(|row| {
                let job = field_row(cells, row, &[], theme)?;
                layouts = layouts.saturating_add(1);
                Some(job)
            })
            .collect();
        let emblem_rows = match (&self.emblem, cells.emblem_at) {
            (Some(emblem), Some(_)) => (0..emblem.rows)
                .map(|row| {
                    let job = emblem_row(emblem, row, &[], theme)?;
                    layouts = layouts.saturating_add(1);
                    Some(job)
                })
                .collect(),
            _ => Vec::new(),
        };
        self.layouts = self.layouts.saturating_add(layouts);
        self.look = Some(Look { mode: theme.mode, build: self.builds, rows, emblem_rows });
    }

    /// Flips the cells within the level's reach of `pointer` and cools those it has left, at input time `now`.
    fn update(&mut self, now: f64, pointer: Option<Pos2>, level: Level, on_home: bool) {
        self.frame = self.frame.wrapping_add(1);
        let field_reach = match level {
            Level::Full => REACH_FULL,
            Level::Reduced if on_home => REACH_REDUCED,
            Level::Reduced | Level::Off => 0.0,
        };
        let emblem_reach = if level == Level::Off { 0.0 } else { EMBLEM_REACH };
        // A level without motion changes everything at once.
        if field_reach <= 0.0 {
            self.warm.retain(|spot, _| matches!(spot, Spot::Emblem(_)));
        }
        if emblem_reach <= 0.0 {
            self.warm.retain(|spot, _| matches!(spot, Spot::Field(_)));
        }
        let pointer = pointer.filter(|at| at.x.is_finite() && at.y.is_finite());
        if let (Some(at), Some(cells)) = (pointer, &self.cells) {
            let mut near = Vec::new();
            if field_reach > 0.0 {
                field_near(cells, at, field_reach, self.seed, &mut near);
            }
            if let (Some(emblem), Some(corner)) = (&self.emblem, cells.emblem_at)
                && emblem_reach > 0.0
            {
                emblem_near(emblem, corner, at, emblem_reach, self.seed, &mut near);
            }
            for spot in near {
                self.touch(spot);
            }
        }
        // A cell the pointer has left was left after the last update: if the field was not drawn for a while, the
        // cells that have cooled since are cold at once.
        let (frame, left) = (self.frame, self.last.unwrap_or(now).min(now));
        for warm in self.warm.values_mut() {
            if warm.seen != frame && warm.left.is_none() {
                warm.left = Some(left);
            }
        }
        self.warm.retain(|_, warm| warm.left.is_none_or(|left| now - left < warm.cool));
        self.last = Some(now);
    }

    /// `spot` is within reach: it stays warm, or flips if it was not.
    fn touch(&mut self, spot: Spot) {
        if let Some(warm) = self.warm.get_mut(&spot) {
            warm.seen = self.frame;
            warm.left = None;
            return;
        }
        if self.warm.len() >= MAX_WARM {
            return;
        }
        self.flips = self.flips.wrapping_add(1);
        let (index, salt) = match spot {
            Spot::Field(index) => (index, 0x5EED),
            Spot::Emblem(index) => (index, 0xE3B1),
        };
        let hash = mix(index, self.flips, self.seed, salt);
        // The remainders are below 28, 4 and 1 001: the casts lose nothing.
        let glyph = (hash % CODE.len() as u32) as u8;
        let tint = (hash >> 8).is_multiple_of(4);
        let cool = TRAIL * f64::from((hash >> 16) % 1000 + 1) / 1000.0;
        self.warm.insert(spot, Warm { glyph, tint, left: None, cool, seen: self.frame });
    }

    /// Whether a cell is cooling down, so the next frame changes something.
    fn cooling(&self) -> bool {
        self.warm.values().any(|warm| warm.left.is_some())
    }

    /// Paints the cells: the row texts built before, and the rows that hold a warm cell built again.
    fn paint(&mut self, ui: &Ui, theme: &Theme, opts: Opts) {
        let (Some(cells), Some(look)) = (&self.cells, &self.look) else { return };
        let clip = cells.rect.intersect(ui.clip_rect());
        if !clip.is_positive() {
            return;
        }
        let painter = ui.painter().with_clip_rect(clip);
        let strength = if opts.on_pane { PANE_STRENGTH } else { 1.0 };
        let mut layouts = 0_u64;
        let text = |at: Pos2, galley: Arc<Galley>| {
            let mut shape = TextShape::new(at, galley, theme.palette.faint);
            shape.opacity_factor = strength;
            painter.add(shape);
        };
        let mut warm = Vec::new();
        for row in 0..cells.rows {
            warm.clear();
            // The cells of a row are row × columns to the row's end; at most 512 × 1 024, far inside u32.
            let first = (row * cells.columns) as u32;
            let end = ((row + 1) * cells.columns) as u32;
            warm.extend(self.warm.range(Spot::Field(first)..Spot::Field(end)).map(|(spot, warm)| (spot_index(*spot) - first, *warm)));
            let at = pos2(cells.origin.x, row_top(cells, row));
            if warm.is_empty() {
                if let Some(Some(job)) = look.rows.get(row) {
                    text(at, painter.layout_job(job.clone()));
                }
            } else if let Some(job) = field_row(cells, row, &warm, theme) {
                layouts = layouts.saturating_add(1);
                text(at, painter.layout_job(job));
            }
        }
        if let (Some(emblem), Some(corner)) = (&self.emblem, cells.emblem_at) {
            for row in 0..emblem.rows {
                warm.clear();
                let first = (row * emblem.columns) as u32;
                let end = ((row + 1) * emblem.columns) as u32;
                warm.extend(self.warm.range(Spot::Emblem(first)..Spot::Emblem(end)).map(|(spot, warm)| (spot_index(*spot) - first, *warm)));
                let at = pos2(corner.x, floor_px(corner.y + row as f32 * EMBLEM_CELL.y, cells.ppp));
                if warm.is_empty() {
                    if let Some(Some(job)) = look.emblem_rows.get(row) {
                        text(at, painter.layout_job(job.clone()));
                    }
                } else if let Some(job) = emblem_row(emblem, row, &warm, theme) {
                    layouts = layouts.saturating_add(1);
                    text(at, painter.layout_job(job));
                }
            }
        }
        self.layouts = self.layouts.saturating_add(layouts);
    }
}

/// Paints `field` over `rect` in `theme` at `level`, as `opts` says: builds its cells if they are not built for this
/// rect, text scale and emblem area yet, flips and cools the cells under the pointer, and asks for a repaint while a
/// cell cools. It takes no input and adds nothing a screen reader or the keyboard lands on.
pub fn show(ui: &mut Ui, theme: &Theme, field: &mut Field, rect: Rect, level: Level, opts: Opts) {
    let ctx = ui.ctx().clone();
    let ppp = ctx.pixels_per_point();
    field.build(rect, theme.text_scale, opts.emblem, ppp);
    field.lay_out(theme);
    let (now, pointer) = ui.input(|input| (input.time, input.pointer.hover_pos()));
    field.update(now, pointer, level, opts.on_home);
    if field.cooling() {
        ctx.request_repaint();
    }
    field.paint(ui, theme, opts);
}

/// A cell's height at `scale`: 15 points × the scale, to the whole point.
fn cell_height(scale: TextScale) -> f32 {
    (CELL_HEIGHT * scale.factor()).round().max(1.0)
}

/// `points` down to a whole pixel at `ppp` pixels per point.
fn floor_px(points: f32, ppp: f32) -> f32 {
    (points * ppp).floor() / ppp
}

/// `points` up to a whole pixel at `ppp` pixels per point.
fn ceil_px(points: f32, ppp: f32) -> f32 {
    (points * ppp).ceil() / ppp
}

/// The top of row `row`, on a whole pixel and never above where it should be.
fn row_top(cells: &Cells, row: usize) -> f32 {
    // At most 512 rows: the cast loses nothing.
    floor_px(cells.origin.y + row as f32 * cells.height, cells.ppp)
}

/// The index of a spot.
const fn spot_index(spot: Spot) -> u32 {
    match spot {
        Spot::Field(index) | Spot::Emblem(index) => index,
    }
}

/// Where the cells of a field over `rect` at `scale` stand, the emblem centred in `area` if it fits there whole, at
/// `ppp`: everything but what they hold, at no cost.
fn place_cells(rect: Rect, scale: TextScale, area: Option<Rect>, ppp: f32, emblem: Option<&Emblem>) -> Cells {
    let height = cell_height(scale);
    let ppp_ok = ppp.is_finite() && ppp > 0.0 && ppp <= MAX_PPP;
    let ppp = if ppp_ok { ppp } else { 1.0 };
    let mut placed = Cells { rect, scale, ppp, origin: rect.min, height, columns: 0, rows: 0, held: Vec::new(), emblem_at: None };
    if !(ppp_ok && rect.is_finite() && rect.width() > 0.0 && rect.height() > 0.0) {
        return placed;
    }
    // Clamped before any arithmetic, so no corner or origin overflows to an infinity.
    let rect = within_reach(rect);
    let area = area.map(within_reach);
    placed.rect = rect;
    let origin = pos2(ceil_px(rect.min.x, ppp), ceil_px(rect.min.y, ppp));
    // Clamped to the caps first, so the casts lose nothing.
    placed.columns = ((rect.max.x - origin.x) / CELL_WIDTH).floor().clamp(0.0, MAX_COLUMNS as f32) as usize;
    placed.rows = ((rect.max.y - origin.y) / height).floor().clamp(0.0, MAX_ROWS as f32) as usize;
    placed.origin = origin;
    placed.emblem_at = emblem.zip(area).and_then(|(emblem, area)| place(emblem, area, rect, ppp));
    placed
}

/// `rect` with its corners clamped to [`MAX_COORDINATE`] either way: far off any screen, and far from overflowing
/// once multiplied by up to [`MAX_PPP`] pixels per point.
fn within_reach(rect: Rect) -> Rect {
    let clamp = |at: Pos2| pos2(at.x.clamp(-MAX_COORDINATE, MAX_COORDINATE), at.y.clamp(-MAX_COORDINATE, MAX_COORDINATE));
    Rect::from_min_max(clamp(rect.min), clamp(rect.max))
}

/// What the `built` cells hold in the pattern of `seed`, the field's cells under the emblem empty.
fn fill(built: &mut Cells, seed: u32, emblem: Option<&Emblem>) {
    let (columns, rows) = (built.columns, built.rows);
    built.held = vec![EMPTY; columns * rows];
    for row in 0..rows {
        for column in 0..columns {
            // At most 1 024 columns and 512 rows: the casts lose nothing.
            if dot(column as u32, row as u32, seed)
                && let Some(cell) = built.held.get_mut(row * columns + column)
            {
                *cell = DOT;
            }
        }
    }
    if let (Some(emblem), Some(corner)) = (emblem, built.emblem_at) {
        cover(built, emblem, corner);
    }
}

/// Where the warm `spot`, of cells `columns` wide before, is among the `built` cells: `None` if it is not there any
/// more.
fn still_there(spot: Spot, columns: usize, built: &Cells) -> Option<Spot> {
    match spot {
        Spot::Field(index) => {
            let index = usize::try_from(index).ok()?;
            let (row, column) = (index.checked_div(columns)?, index.checked_rem(columns)?);
            if column >= built.columns || row >= built.rows {
                return None;
            }
            let at = row * built.columns + column;
            built.held.get(at).filter(|held| **held != UNDER)?;
            u32::try_from(at).ok().map(Spot::Field)
        }
        Spot::Emblem(_) => built.emblem_at.map(|_| spot),
    }
}

/// Where `emblem`'s top left corner stands centred in `area`, on a whole pixel, if it fits there whole and inside the
/// field's `rect`.
fn place(emblem: &Emblem, area: Rect, rect: Rect, ppp: f32) -> Option<Pos2> {
    let size = emblem.size();
    if !area.is_finite() || size.x <= 0.0 || size.y <= 0.0 {
        return None;
    }
    let corner = pos2(ceil_px(area.min.x + (area.width() - size.x) / 2.0, ppp), ceil_px(area.min.y + (area.height() - size.y) / 2.0, ppp));
    let placed = Rect::from_min_size(corner, size);
    // An area narrower or lower than the emblem puts its corner outside the area. Within a thousandth of a point:
    // the corner was rounded up by less than a pixel inside the room left over.
    (area.expand(1e-3).contains_rect(placed) && rect.expand(1e-3).contains_rect(placed)).then_some(corner)
}

/// Empties the field's cells that lie under one of `emblem`'s cells standing at `corner`.
fn cover(cells: &mut Cells, emblem: &Emblem, corner: Pos2) {
    let size = emblem.size();
    let (columns, rows) = (cells.columns, cells.rows);
    for row in 0..rows {
        let top = row_top(cells, row);
        let bottom = top + cells.height;
        if bottom <= corner.y || top >= corner.y + size.y {
            continue;
        }
        for column in 0..columns {
            // At most 1 024 columns: the cast loses nothing.
            let left = cells.origin.x + column as f32 * CELL_WIDTH;
            let right = left + CELL_WIDTH;
            if right <= corner.x || left >= corner.x + size.x {
                continue;
            }
            let (first_column, last_column) = span(left - corner.x, right - corner.x, EMBLEM_CELL.x, emblem.columns);
            let (first_row, last_row) = span(top - corner.y, bottom - corner.y, EMBLEM_CELL.y, emblem.rows);
            let covered = (first_row..last_row).any(|r| (first_column..last_column).any(|c| emblem.cell(c, r).0 > 0));
            if covered && let Some(cell) = cells.held.get_mut(row * columns + column) {
                *cell = UNDER;
            }
        }
    }
}

/// The cells of `step` points, `count` of them from 0, that the span `from` to `to` overlaps: first and one past
/// the last.
fn span(from: f32, to: f32, step: f32, count: usize) -> (usize, usize) {
    // Clamped to 0 and the count, at most 160, before the casts.
    let first = (from / step).floor().clamp(0.0, count as f32) as usize;
    let end = (to / step).ceil().clamp(0.0, count as f32) as usize;
    (first, end)
}

/// Whether the cell at `column` and `row` holds a dot in the pattern of `seed`: about 5 in 100 do. With seed 0 it is
/// the pattern of the design file's character field.
fn dot(column: u32, row: u32, seed: u32) -> bool {
    (column.wrapping_mul(73_856_093) ^ row.wrapping_mul(19_349_663) ^ seed) % 100 < DOTS_IN_100
}

/// A small hash of four numbers (splitmix64's finish).
fn mix(a: u32, b: u32, c: u32, d: u32) -> u32 {
    let mut x = (u64::from(a) | (u64::from(b) << 32)) ^ u64::from(c).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ u64::from(d).rotate_left(41);
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    // The high half: the cast keeps what it is meant to.
    (x >> 32) as u32
}

/// A share of 0 to 1 from a hash.
fn share(hash: u32) -> f32 {
    // The top 24 bits fit an f32 whole.
    (hash >> 8) as f32 / (1_u32 << 24) as f32
}

/// The field's cells near `at` that flip within `reach`: each cell has a reach of its own, so the cells nearest the
/// pointer flip most often and about 45 in 100 of those within `reach` flip.
fn field_near(cells: &Cells, at: Pos2, reach: f32, seed: u32, near: &mut Vec<Spot>) {
    let (first_column, end_column) = span(at.x - reach - cells.origin.x, at.x + reach - cells.origin.x, CELL_WIDTH, cells.columns);
    let (first_row, end_row) = span(at.y - reach - cells.origin.y, at.y + reach - cells.origin.y, cells.height, cells.rows);
    for row in first_row..end_row {
        let top = row_top(cells, row);
        for column in first_column..end_column {
            let index = row * cells.columns + column;
            if cells.held.get(index).is_none_or(|held| *held == UNDER) {
                continue;
            }
            // At most 1 024 columns: the casts lose nothing.
            let centre = pos2(cells.origin.x + (column as f32 + 0.5) * CELL_WIDTH, top + cells.height / 2.0);
            let own = share(mix(column as u32, row as u32, seed, 0xF1E1));
            let distance = centre.distance(at) / reach;
            // Within reach while the share is under 0.9 × (1 - distance²).
            if distance <= 1.0 && own < 0.9 * (1.0 - distance * distance) {
                near.push(Spot::Field(index as u32));
            }
        }
    }
}

/// The emblem's cells near `at` that flip within `reach`: about half of those within it, the nearest most often.
fn emblem_near(emblem: &Emblem, corner: Pos2, at: Pos2, reach: f32, seed: u32, near: &mut Vec<Spot>) {
    let (first_column, end_column) = span(at.x - reach - corner.x, at.x + reach - corner.x, EMBLEM_CELL.x, emblem.columns);
    let (first_row, end_row) = span(at.y - reach - corner.y, at.y + reach - corner.y, EMBLEM_CELL.y, emblem.rows);
    for row in first_row..end_row {
        for column in first_column..end_column {
            if emblem.cell(column, row).0 == 0 {
                continue;
            }
            // At most 160 columns and 80 rows: the casts lose nothing.
            let centre = corner + vec2((column as f32 + 0.5) * EMBLEM_CELL.x, (row as f32 + 0.5) * EMBLEM_CELL.y);
            let own = share(mix(column as u32, row as u32, seed, 0xE1B1));
            let distance = centre.distance(at) / reach;
            if distance <= 1.0 && own < 0.25 + 0.75 * (1.0 - distance) {
                near.push(Spot::Emblem((row * emblem.columns + column) as u32));
            }
        }
    }
}

/// The text of the field's row `row`, `warm` cells (by column) flipped: `None` for a row with nothing to show.
fn field_row(cells: &Cells, row: usize, warm: &[(u32, Warm)], theme: &Theme) -> Option<LayoutJob> {
    let held = cells.held.get(row * cells.columns..(row + 1) * cells.columns)?;
    let p = &theme.palette;
    let mut runs = Runs::new(theme.fonts.field(), cells.height, CELL_WIDTH);
    let mut warm = warm.iter().peekable();
    for (column, cell) in held.iter().enumerate() {
        // At most 1 024 columns: the cast loses nothing.
        let flipped = warm.next_if(|(at, _)| *at == column as u32).map(|(_, warm)| *warm);
        match flipped {
            Some(warm) => runs.push(glyph(warm.glyph), if warm.tint { p.cyan_text } else { p.dim }),
            None if *cell == DOT => runs.push('·', p.faint),
            None => runs.push(' ', p.faint),
        }
    }
    runs.finish()
}

/// The text of the emblem's row `row`, `warm` cells (by column) flipped: `None` for an empty row.
fn emblem_row(emblem: &Emblem, row: usize, warm: &[(u32, Warm)], theme: &Theme) -> Option<LayoutJob> {
    let p = &theme.palette;
    let mut runs = Runs::new(theme.fonts.emblem(), EMBLEM_CELL.y, EMBLEM_CELL.x);
    let mut warm = warm.iter().peekable();
    for column in 0..emblem.columns {
        let (density, cyan) = emblem.cell(column, row);
        // At most 160 columns: the cast loses nothing.
        let flipped = warm.next_if(|(at, _)| *at == column as u32).map(|(_, warm)| *warm);
        let own = if cyan { p.cyan_text } else { p.ink };
        let other = if cyan { p.ink } else { p.cyan_text };
        match flipped {
            Some(warm) => runs.push(glyph(warm.glyph), if warm.tint { other } else { own }),
            None => runs.push(RAMP.get(usize::from(density)).map_or(' ', |ramp| char::from(*ramp)), own),
        }
    }
    runs.finish()
}

/// The code glyph `index`.
fn glyph(index: u8) -> char {
    CODE.get(usize::from(index)).map_or('·', |code| char::from(*code))
}

/// A row's text gathered in runs of one colour. A long gap starts a new run that stands its gap off as leading space
/// instead of spaces, so a row of the field is about as long as its dots, not its columns; the spaces at the row's
/// end are left out.
struct Runs {
    job: LayoutJob,
    font: Font,
    line: f32,
    /// A cell's width, in points: a gap of `n` cells is `n` of these.
    cell: f32,
    text: String,
    colour: Color32,
    /// The leading space of the run being gathered, in points.
    lead: f32,
    /// Empty cells since the last one that shows.
    gap: usize,
    shows: bool,
}

/// The longest gap kept as spaces inside a run of one colour.
const SPACES_KEPT: usize = 3;

impl Runs {
    fn new(font: Font, line: f32, cell: f32) -> Self {
        Self { job: LayoutJob::default(), font, line, cell, text: String::new(), colour: Color32::TRANSPARENT, lead: 0.0, gap: 0, shows: false }
    }

    fn push(&mut self, ch: char, colour: Color32) {
        if ch == ' ' {
            self.gap = self.gap.saturating_add(1);
            return;
        }
        let gap = std::mem::take(&mut self.gap);
        if !self.shows || colour != self.colour || gap > SPACES_KEPT {
            self.flush();
            self.colour = colour;
            // A row has at most 1 024 cells: the cast loses nothing.
            self.lead = gap as f32 * self.cell;
        } else {
            self.text.extend(std::iter::repeat_n(' ', gap));
        }
        self.text.push(ch);
        self.shows = true;
    }

    fn flush(&mut self) {
        if !self.text.is_empty() {
            let format = TextFormat { line_height: Some(self.line), ..self.font.format(self.colour) };
            self.job.append(&std::mem::take(&mut self.text), self.lead, format);
        }
    }

    fn finish(mut self) -> Option<LayoutJob> {
        self.flush();
        self.shows.then_some(self.job)
    }
}

#[cfg(test)]
#[path = "field_tests.rs"]
mod tests;
