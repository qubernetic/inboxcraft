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
//! rest `dim`; at the reduced level only Home's field answers, and as at full; at off the field is still dots. As in
//! the motion source, the cells within reach roll again and again while the pointer stays, moving or not: a cell flips
//! with a chance of 0.8 × f² × q, f = 1 at the pointer and 0 at the edge of its reach; one that flipped may flip again
//! 50 to 190 ms later ([`REFLIP`]), one that did not tries again after 60 ms ([`RETRY`]). A flipped cell is back to
//! what it was 80 to 200 ms after its flip ([`HOT`]), the pointer there or not; out of reach it takes a new glyph every
//! 70 to 230 ms ([`FLICKER`]) until then, so the trail thins out. Under a still pointer away from text about 22 in 100
//! of the cells within reach are flipped at a time (`field_tests`). Near the quiet blocks its caller names (Home's
//! text, [`Field::set_quiet`]) the field calms, as the source does near text: q is 0.2 inside a block and comes back
//! to 1 over 80 points, eased ([`quiet_at`]), and a flipped cell shows at q of its opacity (its colour mixed into
//! `bg` by q, a flat colour); the dots at rest do not change. The emblem's cells (M2) flip with a chance of 0.25 + 0.75 × f and settle back the same way within 60 points, at full and at reduced
//! alike; at off it is still. While the field is live (full, or reduced on Home) the emblem also twinkles by itself:
//! in each slot of 1/60 s of input time, with a chance of 1 in 8, one of its cells that is not warm flips for 250 to
//! 750 ms and flickers as it settles, half of the ink ones in cyan and the cyan ones in cyan; about 7.5 a second at
//! any frame rate. It starts a quarter second after the field is drawn ([`TWINKLE_SETTLE`]), so the few frames of a
//! picture are at rest; the source starts at once. Which cells flip, how long each stays, the glyph each takes and
//! which cell twinkles are hashes of the cell, the seed and a count or a slot: the same input times give the same
//! frames on every machine.
//!
//! The cells are built once, at the first [`show`], for as many whole cells as the rect holds, the seed, the text
//! scale, the pixels per point and where the emblem stands on them, and again only when one of these changes: a rect
//! that moves moves the cells and builds nothing, and the cells warm under the pointer keep their glyphs (when the
//! cells are built again, the warm ones still there keep theirs); the row texts are built once for the theme's mode.
//! A frame at rest paints them (one text a row, each found again in egui's galley cache) and builds nothing; while
//! cells are warm, only the rows that hold one are built again. A repaint is asked for at the time of the next change
//! (a roll, a glyph, a cell back, a twinkle), never sooner: none at rest; measured in a 60 Hz window that draws only
//! when asked, about 30 a second while the emblem twinkles with the pointer away (295 in 10 s, the twinkles' new
//! glyphs as they settle) and about 60 under a still pointer (597 in 10 s); none while the field is not drawn. The field takes no click, no key and no focus,
//! and shows nothing to a screen reader: it only paints.

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

/// How far from the pointer the emblem's cells flip at the full and the reduced level, in points.
pub const EMBLEM_REACH: f32 = 60.0;

/// The share of a field cell's chance to flip, and of a flipped cell's opacity, left inside a quiet block (one of
/// Home's text blocks, [`Field::set_quiet`]).
pub const QUIET_IN: f32 = 0.2;

/// How far from a quiet block the share comes back to 1, eased, in points.
pub const FADE: f32 = 80.0;

/// The most quiet blocks a field keeps; those handed in past it are left out.
pub const MAX_QUIET: usize = 32;

/// The `field_trail` token: the longest a cell stays flipped, in seconds.
pub const TRAIL: f64 = 0.2;

/// The shortest and the longest a cell flipped under the pointer stays flipped, in seconds: 0.4 to 1 × [`TRAIL`].
/// It is back to what it was then, the pointer there or not.
pub const HOT: (f64, f64) = (TRAIL * 0.4, TRAIL);

/// How long a cell within reach that did not flip waits before it may flip again, in seconds.
pub const RETRY: f64 = 0.06;

/// The shortest and the longest wait, in seconds, before a cell within reach that flipped may flip again.
pub const REFLIP: (f64, f64) = (0.05, 0.19);

/// The shortest and the longest wait, in seconds, before a flipped cell out of reach takes a new glyph.
pub const FLICKER: (f64, f64) = (0.07, 0.23);

/// How long a slot of the emblem's twinkle lasts: a frame at 60 Hz. In each slot one of its cells starts to twinkle
/// with a chance of 1 in 8: about 7.5 a second, whatever the frame rate.
pub const TWINKLE_SLOT: f64 = 1.0 / 60.0;

/// The shortest and the longest a cell twinkles, in seconds.
pub const TWINKLE_LIFE: (f64, f64) = (0.25, 0.75);

/// How long after a twinkle starts its cell takes its next glyph, in seconds.
pub const TWINKLE_NEXT: f64 = 0.09;

/// How long after the field is drawn (first, or again after it was not drawn) the emblem starts to twinkle, in
/// seconds: unseen, and the few frames of a picture stay at rest.
pub const TWINKLE_SETTLE: f64 = 0.25;

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

/// How much the window moves: passed in by the caller, never read here.
pub use crate::motion::Level;

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
    /// The indexes of the cells that are on, the ones that twinkle.
    marks: Vec<u32>,
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
        // At most 160 × 80 cells: the indexes fit a u32.
        let marks = cells.iter().enumerate().filter(|(_, held)| **held != 0).filter_map(|(index, _)| u32::try_from(index).ok()).collect();
        Self { columns, rows: rows.len(), cells, marks }
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

/// A cell flipped under the pointer, or twinkling.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Warm {
    /// The code glyph it flipped to, an index into [`CODE`].
    glyph: u8,
    /// Whether it took the other colour: `cyan_text` on the field, the emblem's other colour on the emblem.
    tint: bool,
    /// When it flipped, an input time in seconds.
    at: f64,
    /// When it is back to what it was, an input time in seconds: [`HOT`] after its flip, a twinkle's
    /// [`TWINKLE_LIFE`].
    until: f64,
    /// The update it was last within reach in.
    seen: u64,
    /// When it may change next, an input time in seconds: within reach it may flip again, out of reach it takes a new
    /// glyph.
    next: f64,
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
    /// The quiet blocks, as [`Field::set_quiet`] was last told: at most [`MAX_QUIET`].
    quiet: Vec<Rect>,
    /// The cells within reach that did not flip, when each may try again and the update it was last within reach in:
    /// at most [`MAX_WARM`].
    waiting: BTreeMap<Spot, (f64, u64)>,
    /// The last of the emblem's twinkle slots done with; `None` while it does not twinkle.
    twinkled: Option<u64>,
    /// Updates so far.
    frame: u64,
    /// egui's pass number of the last update: if the field missed passes since (it was not drawn), the emblem settles
    /// again before it twinkles.
    last: Option<u64>,
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
        Self {
            seed,
            emblem: None,
            cells: None,
            look: None,
            warm: BTreeMap::new(),
            quiet: Vec::new(),
            waiting: BTreeMap::new(),
            twinkled: None,
            frame: 0,
            last: None,
            flips: 0,
            builds: 0,
            layouts: 0,
        }
    }

    /// The same field with `emblem` on it, where [`Opts::emblem`] puts it.
    #[must_use]
    pub fn with_emblem(mut self, emblem: Emblem) -> Self {
        self.emblem = Some(emblem);
        self.cells = None;
        self.look = None;
        self
    }

    /// The emblem on it.
    #[must_use]
    pub const fn emblem(&self) -> Option<&Emblem> {
        self.emblem.as_ref()
    }

    /// Puts `emblem` on it (`None` takes it off), where [`Opts::emblem`] puts it; the cells are built again at the next
    /// [`show`].
    pub fn set_emblem(&mut self, emblem: Option<&Emblem>) {
        self.emblem = emblem.cloned();
        self.cells = None;
        self.look = None;
    }

    /// The blocks the field keeps quiet near from now on (Home's text, as it was drawn last frame): within one a field
    /// cell flips with [`QUIET_IN`] of its chance and a flipped one shows at that opacity, coming back to the whole
    /// over [`FADE`] points eased ([`quiet_at`]). The emblem's cells and the dots at rest are not quietened. Blocks that
    /// are empty or not a number are left out, and those past [`MAX_QUIET`]; the list is kept, so this builds nothing
    /// once it has grown.
    pub fn set_quiet(&mut self, blocks: &[Rect]) {
        self.quiet.clear();
        let usable = blocks.iter().filter(|block| block.is_finite() && block.is_positive()).map(|block| within_reach(*block));
        self.quiet.extend(usable.take(MAX_QUIET));
    }

    /// Where the emblem's top left corner was drawn at the last build, if it was.
    #[cfg(test)]
    pub(crate) fn emblem_at(&self) -> Option<Pos2> {
        self.cells.as_ref().and_then(|cells| cells.emblem_at)
    }

    /// The rect the cells were last placed over.
    #[cfg(test)]
    pub(crate) fn covers(&self) -> Option<Rect> {
        self.cells.as_ref().map(|cells| cells.rect)
    }

    /// The quiet blocks it keeps.
    #[cfg(test)]
    pub(crate) fn quiet(&self) -> &[Rect] {
        &self.quiet
    }

    /// The centres of the field's warm cells (not the emblem's).
    #[cfg(test)]
    pub(crate) fn warm_centres(&self) -> Vec<Pos2> {
        let Some(cells) = &self.cells else { return Vec::new() };
        let centre = |index: usize| {
            let (row, column) = (index / cells.columns, index % cells.columns);
            pos2(cells.origin.x + (column as f32 + 0.5) * CELL_WIDTH, row_top(cells, row) + cells.height / 2.0)
        };
        self.warm.keys().filter_map(|spot| if let Spot::Field(index) = spot { Some(centre(*index as usize)) } else { None }).collect()
    }

    /// How many times the cells were built (what the page tests of a screen with a field count).
    #[cfg(test)]
    pub(crate) const fn builds(&self) -> u32 {
        self.builds
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
        let waiting = std::mem::take(&mut self.waiting);
        self.waiting = waiting.into_iter().filter_map(|(spot, next)| Some((still_there(spot, before, &placed)?, next))).collect();
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
                let job = field_row(cells, row, &[], &[], theme)?;
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

    /// Rolls the cells within the level's reach of `pointer` (again and again while it stays there), lets the flipped
    /// ones out of reach flicker, settles those whose time is up and twinkles the emblem, at input time `now` in egui's
    /// pass `pass`.
    fn update(&mut self, now: f64, pass: u64, pointer: Option<Pos2>, level: Level, on_home: bool) {
        // A time no window reaches is ignored: the field's sums stay whole.
        if !now.is_finite() || now.abs() > LATEST {
            return;
        }
        self.frame = self.frame.wrapping_add(1);
        let (field_reach, strength) = match level {
            // Home's field answers at the reduced level as at full; every other field is still there.
            Level::Full => (REACH_FULL, 1.0),
            Level::Reduced if on_home => (REACH_FULL, 1.0),
            Level::Reduced | Level::Off => (0.0, 0.0),
        };
        let emblem_reach = if level == Level::Off { 0.0 } else { EMBLEM_REACH };
        // A level without motion changes everything at once.
        if field_reach <= 0.0 {
            self.warm.retain(|spot, _| matches!(spot, Spot::Emblem(_)));
            self.waiting.retain(|spot, _| matches!(spot, Spot::Emblem(_)));
        }
        if emblem_reach <= 0.0 {
            self.warm.retain(|spot, _| matches!(spot, Spot::Field(_)));
            self.waiting.retain(|spot, _| matches!(spot, Spot::Field(_)));
        }
        // A cell waits until its time comes, or the clock went back.
        self.waiting.retain(|_, (next, _)| *next > now && *next - now <= RETRY);
        let pointer = pointer.filter(|at| at.x.is_finite() && at.y.is_finite());
        if let (Some(at), Some(cells)) = (pointer, &self.cells) {
            // Each update rolls afresh; the first rolls with the cells' own hashes.
            // The low half of the count: the cast keeps what it is meant to.
            let key = self.frame.wrapping_sub(1) as u32;
            let mut near = Vec::new();
            if field_reach > 0.0 {
                field_near(cells, at, (field_reach, strength), &self.quiet, self.seed, key, &mut near);
            }
            if let (Some(emblem), Some(corner)) = (&self.emblem, cells.emblem_at)
                && emblem_reach > 0.0
            {
                emblem_near(emblem, corner, at, emblem_reach, self.seed, key, &mut near);
            }
            for (spot, hit) in near {
                self.roll(spot, hit, now);
            }
        }
        let frame = self.frame;
        // Only the cells within reach wait.
        self.waiting.retain(|_, (_, seen)| *seen == frame);
        // The emblem's twinkles first: one that started between two frames far apart takes its next glyph now.
        let missed = self.last.is_some_and(|last| pass > last.saturating_add(1));
        let live = level == Level::Full || (level == Level::Reduced && on_home);
        self.twinkle(now, live, missed);
        // A flipped cell out of reach takes a new glyph from time to time until its time is up.
        for (spot, warm) in &mut self.warm {
            if warm.seen != frame && due(now, warm.next, LONGEST_NEXT) {
                self.flips = self.flips.wrapping_add(1);
                let (index, salt) = salted(*spot);
                warm.glyph = code(mix(index, self.flips, self.seed, salt));
                warm.next = now + between(FLICKER, mix(index, self.flips, self.seed, salt ^ 0xF11C));
            }
        }
        // A cell whose time is up is back to what it was; one flipped in the future (the clock went back) too.
        self.warm.retain(|_, warm| (warm.at..warm.until).contains(&now));
        self.last = Some(pass);
    }

    /// `spot` is within reach and, if it is due, flips when `hit`: a flipped cell that flips again takes a new glyph
    /// and a new time, one that does not waits [`RETRY`] and keeps its time; a cell that flips turns warm, one that
    /// does not waits [`RETRY`].
    fn roll(&mut self, spot: Spot, hit: bool, now: f64) {
        let (index, salt) = salted(spot);
        if let Some(warm) = self.warm.get_mut(&spot) {
            warm.seen = self.frame;
            if !due(now, warm.next, LONGEST_NEXT) {
                return;
            }
            if hit {
                self.flips = self.flips.wrapping_add(1);
                *warm = flipped(index, self.flips, self.seed, salt, now, self.frame);
            } else {
                warm.next = now + RETRY;
            }
            return;
        }
        if let Some((_, seen)) = self.waiting.get_mut(&spot) {
            *seen = self.frame;
            return;
        }
        if !hit {
            if self.waiting.len() < MAX_WARM {
                self.waiting.insert(spot, (now + RETRY, self.frame));
            }
            return;
        }
        if self.warm.len() >= MAX_WARM {
            return;
        }
        self.flips = self.flips.wrapping_add(1);
        self.warm.insert(spot, flipped(index, self.flips, self.seed, salt, now, self.frame));
    }

    /// Starts the emblem's twinkles of the slots up to `now`, if the field is `live` and the emblem drawn: in each slot
    /// one of its cells that is not warm, with a chance of 1 in 8. After the field `missed` passes it starts again
    /// [`TWINKLE_SETTLE`] on.
    fn twinkle(&mut self, now: f64, live: bool, missed: bool) {
        let drawn = self.cells.as_ref().is_some_and(|cells| cells.emblem_at.is_some());
        let Some(emblem) = self.emblem.as_ref().filter(|emblem| live && drawn && !emblem.marks.is_empty()) else {
            self.twinkled = None;
            return;
        };
        let current = slot_of(now);
        let settled = slot_of(now + TWINKLE_SETTLE);
        let done = match self.twinkled {
            // Later than the settling allows: the clock went back.
            Some(done) if !missed && done <= settled => done,
            _ => settled,
        };
        self.twinkled = Some(done.max(current));
        // A twinkle that would have settled by now is not started.
        let first = done.max(slot_of(now - TWINKLE_LIFE.1)).saturating_add(1);
        let last = current.min(first.saturating_add(MOST_SLOTS - 1));
        for slot in first..=last {
            let Some(index) = twinkle_in(&emblem.marks, self.seed, slot) else { continue };
            let spot = Spot::Emblem(index);
            if self.warm.contains_key(&spot) || self.warm.len() >= MAX_WARM {
                continue;
            }
            // Never after now, whatever the rounding.
            let at = slot_time(slot).min(now);
            self.flips = self.flips.wrapping_add(1);
            let hash = mix(index, self.flips, self.seed, 0x7B1E);
            let cyan = usize::try_from(index).ok().and_then(|index| emblem.cells.get(index)).is_some_and(|held| *held > CYAN);
            // An ink cell takes cyan half the time; a cyan one keeps its own.
            let tint = !cyan && (hash >> 8).is_multiple_of(2);
            let until = at + between(TWINKLE_LIFE, mix(index, self.flips, self.seed, 0x7C1E));
            // Not within reach in this update: it flickers like a cell the pointer left.
            let seen = self.frame.wrapping_sub(1);
            self.warm.insert(spot, Warm { glyph: code(hash), tint, at, until, seen, next: at + TWINKLE_NEXT });
        }
    }

    /// When the field next changes by itself, an input time in seconds: `None` while it rests.
    pub(crate) fn next_change(&self, now: f64) -> Option<f64> {
        let warm = self.warm.values().map(|warm| warm.until.min(warm.next));
        let twinkle = self.twinkled.zip(self.emblem.as_ref()).and_then(|(done, emblem)| {
            let first = done.max(slot_of(now)).saturating_add(1);
            (first..first.saturating_add(LOOK_AHEAD)).find(|slot| twinkle_in(&emblem.marks, self.seed, *slot).is_some()).map(slot_time)
        });
        warm.chain(self.waiting.values().map(|(next, _)| *next)).chain(twinkle).reduce(f64::min)
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
            } else if let Some(job) = field_row(cells, row, &warm, &self.quiet, theme) {
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
/// rect, text scale and emblem area yet, flips, cools and twinkles the cells, and asks for a repaint at the time of
/// the next change, never sooner. It takes no input and adds nothing a screen reader or the keyboard lands on.
pub fn show(ui: &mut Ui, theme: &Theme, field: &mut Field, rect: Rect, level: Level, opts: Opts) {
    let ctx = ui.ctx().clone();
    let ppp = ctx.pixels_per_point();
    field.build(rect, theme.text_scale, opts.emblem, ppp);
    field.lay_out(theme);
    let (now, pointer) = ui.input(|input| (input.time, input.pointer.hover_pos()));
    field.update(now, ctx.cumulative_pass_nr(), pointer, level, opts.on_home);
    // NaN clamps to NaN, which no wait is made of. A change due within a millisecond waits that long: a wait of
    // nothing would ask egui for this frame and the next one too.
    if let Some(wait) = field.next_change(now).map(|next| (next - now).clamp(SHORTEST_WAIT, LONGEST_WAIT))
        && let Ok(wait) = std::time::Duration::try_from_secs_f64(wait)
    {
        ctx.request_repaint_after(wait);
    }
    field.paint(ui, theme, opts);
}

/// The longest wait a field asks for before its next frame, in seconds.
const LONGEST_WAIT: f64 = 4.0;

/// The shortest wait a field asks for before its next frame, in seconds.
const SHORTEST_WAIT: f64 = 0.001;

/// The most twinkle slots an update goes through: those of the longest twinkle, and one.
const MOST_SLOTS: u64 = 47;

/// The longest a cell waits for its next change, in seconds: the longest of [`REFLIP`], [`FLICKER`], [`RETRY`] and
/// [`TWINKLE_NEXT`]. A change planned further ahead counts as due: the clock went back.
const LONGEST_NEXT: f64 = FLICKER.1;

/// The latest input time the field answers, in seconds (about 31 years): a later one is ignored.
const LATEST: f64 = 1.0e9;

/// How many twinkle slots ahead the next twinkle is looked for: four seconds.
const LOOK_AHEAD: u64 = 240;

/// Whether a change planned for `next` is due at `now`: it is, or it lies more than `longest` ahead (the clock went
/// back).
fn due(now: f64, next: f64, longest: f64) -> bool {
    now >= next || next - now > longest
}

/// A spot's index and the salt of its flips.
const fn salted(spot: Spot) -> (u32, u32) {
    match spot {
        Spot::Field(index) => (index, 0x5EED),
        Spot::Emblem(index) => (index, 0xE3B1),
    }
}

/// The code glyph of a hash, as an index into [`CODE`].
fn code(hash: u32) -> u8 {
    // The remainder is below 28: the cast loses nothing.
    (hash % CODE.len() as u32) as u8
}

/// A cell flipped `flips` at input time `now` in update `frame`: its glyph, tint and time flipped are hashes of its
/// `index`, the flips and the `seed`.
fn flipped(index: u32, flips: u32, seed: u32, salt: u32, now: f64, frame: u64) -> Warm {
    let hash = mix(index, flips, seed, salt);
    let tint = (hash >> 8).is_multiple_of(4);
    let until = now + between(HOT, mix(index, flips, seed, salt ^ 0x7EA1));
    let next = now + between(REFLIP, mix(index, flips, seed, salt ^ 0x4E58));
    Warm { glyph: code(hash), tint, at: now, until, seen: frame, next }
}

/// A time between the two of `range`, by a hash.
fn between((least, most): (f64, f64), hash: u32) -> f64 {
    least + (most - least) * f64::from(share(hash))
}

/// The twinkle slot `at` lies in.
fn slot_of(at: f64) -> u64 {
    // Clamped to 0 and 2^52 first: the cast loses nothing a slot needs.
    (at / TWINKLE_SLOT).floor().clamp(0.0, 4.5e15) as u64
}

/// When twinkle slot `slot` starts.
fn slot_time(slot: u64) -> f64 {
    // At most 2^52 + 288: an f64 holds it whole.
    slot as f64 * TWINKLE_SLOT
}

/// Which of the emblem's cells `marks` starts to twinkle in `slot`, if one does: 1 slot in 8.
fn twinkle_in(marks: &[u32], seed: u32, slot: u64) -> Option<u32> {
    // The slot's two halves: the casts keep what they are meant to.
    let hash = mix(slot as u32, (slot >> 32) as u32, seed, 0x7A1E);
    if !hash.is_multiple_of(8) || marks.is_empty() {
        return None;
    }
    marks.get((hash >> 3) as usize % marks.len()).copied()
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
    placed.rect = rect;
    let origin = pos2(ceil_px(rect.min.x, ppp), ceil_px(rect.min.y, ppp));
    // Clamped to the caps first, so the casts lose nothing.
    placed.columns = ((rect.max.x - origin.x) / CELL_WIDTH).floor().clamp(0.0, MAX_COLUMNS as f32) as usize;
    placed.rows = ((rect.max.y - origin.y) / height).floor().clamp(0.0, MAX_ROWS as f32) as usize;
    placed.origin = origin;
    placed.emblem_at = emblem.zip(area).and_then(|(emblem, area)| emblem_corner(emblem, area, rect, ppp));
    placed
}

/// Where a field over `rect` at `ppp` draws `emblem` centred in `area` ([`Opts::emblem`]): its top left corner, on
/// the whole pixel the field puts it on, if it fits there whole. What stands by the emblem is placed from this, so
/// it sits on the emblem's pixel grid.
pub(crate) fn emblem_corner(emblem: &Emblem, area: Rect, rect: Rect, ppp: f32) -> Option<Pos2> {
    let ppp_ok = ppp.is_finite() && ppp > 0.0 && ppp <= MAX_PPP;
    if !(ppp_ok && rect.is_finite() && rect.width() > 0.0 && rect.height() > 0.0) {
        return None;
    }
    let rect = within_reach(rect);
    let origin = pos2(ceil_px(rect.min.x, ppp), ceil_px(rect.min.y, ppp));
    place(emblem, within_reach(area), rect, origin, ppp)
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
/// field's `rect` whose first cell stands at `origin`. Its offset from the first cell is the centred corner's offset
/// from the rect's corner, rounded to whole pixels: a field and an area that move together by parts of a pixel keep
/// the emblem on the same cells, so a move alone builds nothing again.
fn place(emblem: &Emblem, area: Rect, rect: Rect, origin: Pos2, ppp: f32) -> Option<Pos2> {
    let size = emblem.size();
    if !area.is_finite() || size.x <= 0.0 || size.y <= 0.0 {
        return None;
    }
    let centred = area.min + (area.size() - size) / 2.0 - rect.min;
    // Rounded at a thousandth of a pixel first: the same offset reached by two sums must not round two ways.
    let whole = |points: f32| ((points * ppp * 1000.0).round() / 1000.0).round() / ppp;
    let corner = origin + vec2(whole(centred.x), whole(centred.y));
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

/// The salt of the roll of `key`: `salt` itself for the first.
const fn rolled(salt: u32, key: u32) -> u32 {
    salt.wrapping_add(key.wrapping_mul(0x9E37_79B9))
}

/// A share of 0 to 1 from a hash.
fn share(hash: u32) -> f32 {
    // The top 24 bits fit an f32 whole.
    (hash >> 8) as f32 / (1_u32 << 24) as f32
}

/// The share of the field's effect left at `at` near the `quiet` blocks: [`QUIET_IN`] inside one, coming back to 1 at
/// [`FADE`] points from it, eased (0.2 + 0.8 × k², k = distance / [`FADE`] up to 1), the least over the blocks; 1 with
/// none near.
pub(crate) fn quiet_at(quiet: &[Rect], at: Pos2) -> f32 {
    quiet.iter().fold(1.0, |least, block| {
        let dx = (block.left() - at.x).max(at.x - block.right()).max(0.0);
        let dy = (block.top() - at.y).max(at.y - block.bottom()).max(0.0);
        let k = (vec2(dx, dy).length() / FADE).min(1.0);
        let share = QUIET_IN + (1.0 - QUIET_IN) * k * k;
        // NaN (an `at` that is not a number) keeps what was found.
        if share < least { share } else { least }
    })
}

/// The field's cells within `reach` of `at` (with `strength`), each with whether it flips in the roll of `key`: with a chance of
/// 0.8 × f² × `strength` × the [`quiet_at`] the cell, f = 1 - distance / `reach`, so the cells nearest the pointer flip
/// most often and about 13 in 100 of those within `reach` flip in a roll at full strength away from any text.
fn field_near(cells: &Cells, at: Pos2, (reach, strength): (f32, f32), quiet: &[Rect], seed: u32, key: u32, near: &mut Vec<(Spot, bool)>) {
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
            let own = share(mix(column as u32, row as u32, seed, rolled(0xF1E1, key)));
            let distance = centre.distance(at) / reach;
            let f = 1.0 - distance;
            if distance <= 1.0 {
                near.push((Spot::Field(index as u32), own < 0.8 * f * f * strength * quiet_at(quiet, centre)));
            }
        }
    }
}

/// The emblem's cells within `reach` of `at`, each with whether it flips in the roll of `key`: about half of those
/// within it flip in a roll, the nearest most often.
fn emblem_near(emblem: &Emblem, corner: Pos2, at: Pos2, reach: f32, seed: u32, key: u32, near: &mut Vec<(Spot, bool)>) {
    let (first_column, end_column) = span(at.x - reach - corner.x, at.x + reach - corner.x, EMBLEM_CELL.x, emblem.columns);
    let (first_row, end_row) = span(at.y - reach - corner.y, at.y + reach - corner.y, EMBLEM_CELL.y, emblem.rows);
    for row in first_row..end_row {
        for column in first_column..end_column {
            if emblem.cell(column, row).0 == 0 {
                continue;
            }
            // At most 160 columns and 80 rows: the casts lose nothing.
            let centre = corner + vec2((column as f32 + 0.5) * EMBLEM_CELL.x, (row as f32 + 0.5) * EMBLEM_CELL.y);
            let own = share(mix(column as u32, row as u32, seed, rolled(0xE1B1, key)));
            let distance = centre.distance(at) / reach;
            if distance <= 1.0 {
                near.push((Spot::Emblem((row * emblem.columns + column) as u32), own < 0.25 + 0.75 * (1.0 - distance)));
            }
        }
    }
}

/// The text of the field's row `row`, `warm` cells (by column) flipped, each at the share [`quiet_at`] its centre
/// near the `quiet` blocks of its colour mixed into `bg` (a flat colour that looks as that opacity does on the page's
/// ground): `None` for a row with nothing to show.
fn field_row(cells: &Cells, row: usize, warm: &[(u32, Warm)], quiet: &[Rect], theme: &Theme) -> Option<LayoutJob> {
    let held = cells.held.get(row * cells.columns..(row + 1) * cells.columns)?;
    let p = &theme.palette;
    let middle = row_top(cells, row) + cells.height / 2.0;
    let mut runs = Runs::new(theme.fonts.field(), cells.height, CELL_WIDTH);
    let mut warm = warm.iter().peekable();
    for (column, cell) in held.iter().enumerate() {
        // At most 1 024 columns: the cast loses nothing.
        let flipped = warm.next_if(|(at, _)| *at == column as u32).map(|(_, warm)| *warm);
        match flipped {
            Some(warm) => {
                // At most 1 024 columns: the cast loses nothing.
                let centre = pos2(cells.origin.x + (column as f32 + 0.5) * CELL_WIDTH, middle);
                let colour = if warm.tint { p.cyan_text } else { p.dim };
                // Near a quiet block a flat colour: the glyph's mixed into the page's ground, as the source's opacity
                // shows on it.
                let q = quiet_at(quiet, centre);
                runs.push(glyph(warm.glyph), if q < 1.0 { crate::theme::mix(p.bg, colour, q) } else { colour });
            }
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
pub(crate) mod tests;
