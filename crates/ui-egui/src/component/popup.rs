// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The float and its rows, as their sheets draw them (`popup-panel`, `menu-item`, `menu-choice`, `suggestion-row`).
//!
//! A float is the solid `surface` with a 1 px `cyan_edge` inside and the one shadow ([`paint::float`]), square; its
//! rows stand 1 px in from its edge, edge to edge. [`float`] hangs one 4 px under the control it opens from, keeps
//! 3 px of ground to the window's border on every side (it moves inwards, never shrinks), keeps `Tab` and
//! `Shift Tab` inside itself, and closes on `Esc` or a press outside it, giving the keyboard back to the control it
//! came from. At full motion its rows come in one after the other, 30 ms apart, each over the `fast` token from 4 px
//! above and from the float's own colour (the presentation's `line-in`); with reduced motion they are all there at
//! once.
//!
//! A row is 32 high (a suggestion 40): 12 in, a 16 px icon and the name in `body`; hovered the `stage` fill, chosen
//! the `stage` fill with a 2 px `cyan_edge` bar at its left, focused a 1 px `cyan_edge` edge with its content 1 px
//! further in. Pressed, which the design does not draw, it takes the press of a secondary button (`stage` and an
//! `ink` edge), or the accent wash with the `cyan_edge` edge when it is chosen. Nothing in a row fades: the design
//! gives its hover no transition.

use egui::{
    Align2, Area, Color32, Context, Id, Key, LayerId, Modifiers, Order, Pos2, Rect, Response, Role, ScrollArea, Sense, Shape, Stroke, StrokeKind, Ui,
    Vec2, WidgetInfo, pos2, vec2,
};

use crate::draw::paint::{self, FloatEdge};
use crate::iconset::IconId;
use crate::keymap::Keymap;
use crate::motion;
use crate::theme::{Theme, mix};
use crate::tr;

use super::key::{Chip, ChipState};
use super::{Hover, Shown, cut, galley, lit, solid, soon};

/// How far a float hangs under the control it opens from, in points.
const DROP: f32 = 4.0;

/// The ground a float keeps to the window's border, in points.
const CLEAR: f32 = 3.0;

/// How far above its place a row comes in from, in points.
const RISE: f32 = 4.0;

/// The width of the accent bar of a chosen row, in points.
const BAR: f32 = 2.0;

/// The width and length of a float's scroll thumb, in points.
const THUMB: [f32; 2] = [2.0, 40.0];

/// How far the thin thumb keeps from the list's top and bottom: 6, the board's and the sheet's.
const THUMB_ENDS: f32 = 6.0;

/// A float a control opens: where, how wide, and who gets the keyboard back.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Float {
    /// Its id, the same every frame it is open.
    pub id: Id,
    /// The control it opens from: it hangs 4 px under it, its left edge on the control's.
    pub anchor: Rect,
    /// Its width in points, its 1 px edge included.
    pub width: f32,
    /// The control that gets the keyboard back when it closes.
    pub returns_to: Id,
}

/// The rows of a float coming in: when it opened, how many rows have asked so far this frame, and the rows that take
/// a press (at most a few hundred are kept, for Home and End).
#[derive(Clone, Default)]
struct Unfolding {
    opened: f64,
    next: usize,
    rows: Vec<Id>,
}

/// The most rows of a float that come in one after another; the rest come in with the last of them.
const STAGGERED: usize = 12;

/// The most rows of a float Home and End know of.
const MOST_ROWS: usize = 512;

/// Where a float's unfolding is kept while its rows are drawn.
fn unfolding_id() -> Id {
    Id::unique("popup-unfolding")
}

/// Where the layer of the float whose rows are being drawn is kept, for a float opened from inside it.
fn showing_id() -> Id {
    Id::unique("popup-showing-float")
}

/// What a float keeps from one frame to the next: when it opened, the pass it was last drawn in, and its rect.
#[derive(Clone, Copy)]
struct Kept {
    opened: f64,
    pass: u64,
    rect: Rect,
}

/// How far the next row of the float being drawn has come in, 0 to 1 (1 outside a float); a row that takes a press
/// (`pressable`) is kept for Home and End.
fn line_in(ui: &Ui, theme: &Theme, row: Id, pressable: bool) -> f32 {
    let ctx = ui.ctx().clone();
    let unfolding = ctx.data_mut(|data| {
        let unfolding = data.get_temp::<Unfolding>(unfolding_id());
        if let Some(mut next) = unfolding.clone() {
            next.next = next.next.saturating_add(1);
            if pressable && next.rows.len() < MOST_ROWS {
                next.rows.push(row);
            }
            data.insert_temp(unfolding_id(), next);
        }
        // Rows past the first few that fit come in with the last of them, so a long list stops moving at once.
        unfolding.map(|unfolding| (unfolding.opened, unfolding.next.saturating_add(1).min(STAGGERED)))
    });
    unfolding.map_or(1.0, |(opened, index)| motion::unfold(&ctx, opened, index, theme.motion.fast))
}

/// Draws a float's content with its rows coming in from `opened`; what `add` gave and the rows that take a press.
fn unfolding<R>(ui: &mut Ui, theme: &Theme, width: f32, opened: f64, add: impl FnOnce(&mut Ui) -> R) -> (R, Vec<Id>) {
    let ctx = ui.ctx().clone();
    // A float opened from inside another keeps the outer one's rows aside and gives them back after.
    let outer = ctx.data_mut(|data| data.remove_temp::<Unfolding>(unfolding_id()));
    ctx.data_mut(|data| data.insert_temp(unfolding_id(), Unfolding { opened, next: 0, rows: Vec::new() }));
    let inner = panel(ui, theme, width, add);
    let rows = ctx.data_mut(|data| data.remove_temp::<Unfolding>(unfolding_id())).map(|unfolding| unfolding.rows).unwrap_or_default();
    if let Some(outer) = outer {
        ctx.data_mut(|data| data.insert_temp(unfolding_id(), outer));
    }
    (inner, rows)
}

/// The place of a float of `size` that would hang at `at`, moved inwards to keep [`CLEAR`] to `screen`'s border, on
/// whole pixels at `ppp`.
fn placed(at: Pos2, size: Vec2, screen: Rect, ppp: f32) -> Pos2 {
    let room = |low: f32, high: f32, want: f32, length: f32| {
        let most = high - CLEAR - length;
        let least = low + CLEAR;
        // A float larger than the window keeps its top left corner clear; nothing is clamped with a bound below the
        // other.
        if most < least { least } else { want.min(most).max(least) }
    };
    let x = room(screen.left(), screen.right(), at.x, size.x);
    let y = room(screen.top(), screen.bottom(), at.y, size.y);
    let ppp = if ppp.is_finite() && ppp > 0.0 { ppp } else { 1.0 };
    let whole = |value: f32| if value.is_finite() { (value * ppp).round() / ppp } else { 0.0 };
    pos2(whole(x), whole(y))
}

/// A sane width for a float: finite, at least a row's height, at most 2 048.
fn sane(width: f32) -> f32 {
    if width.is_finite() { width.clamp(32.0, 2048.0) } else { 240.0 }
}

/// How a person asked a [`held`] float to close.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Leave {
    /// `Esc`.
    Escape,
    /// A press outside it.
    Beside,
}

/// Whether a float closes itself on `Esc` and a press outside it, or its caller decides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// It closes itself.
    Closes,
    /// The caller decides; while `quiet` another surface lies over it and has the keys and the pointer.
    Held { quiet: bool },
}

/// Shows `float` with `add` drawing its rows, as the module says; what `add` gave, or `None` once `Esc` or a press
/// outside it closed it (the caller then forgets it is open). The first frame it shows, the keyboard leaves the
/// control it opened from: the first `Tab` lands on its first row.
pub fn float<R>(ctx: &Context, theme: &Theme, float: &Float, add: impl FnOnce(&mut Ui) -> R) -> Option<R> {
    shown(ctx, theme, float, Mode::Closes, add).map(|(inner, _)| inner)
}

/// A float that holds what a person typed (the event editor): `Esc` and a press outside it close nothing, they come
/// back as [`Leave`] and the caller decides. It closes when the caller stops drawing it, and the caller gives the
/// keyboard back. While `quiet` another surface lies over it (the palette, the settings sheet): it takes neither key
/// nor press, and is not modal, so that surface has them. What `add` gave (always there), and how it was asked to close.
pub fn held<R>(ctx: &Context, theme: &Theme, float: &Float, quiet: bool, add: impl FnOnce(&mut Ui) -> R) -> Option<(R, Option<Leave>)> {
    shown(ctx, theme, float, Mode::Held { quiet }, add)
}

fn shown<R>(ctx: &Context, theme: &Theme, float: &Float, mode: Mode, add: impl FnOnce(&mut Ui) -> R) -> Option<(R, Option<Leave>)> {
    let key = float.id.with("float-kept");
    let pass = ctx.cumulative_pass_nr();
    let now = ctx.input(|input| input.time);
    let kept = ctx.data(|data| data.get_temp::<Kept>(key)).filter(|kept| kept.pass.saturating_add(1) >= pass);
    let opening = kept.is_none();
    // Never larger than the window less its 3 px of ground: past that its rows scroll.
    let screen = ctx.content_rect();
    let room = vec2((screen.width() - CLEAR * 2.0).max(32.0), (screen.height() - CLEAR * 2.0).max(32.0));
    let width = sane(float.width).min(room.x);
    // Only the float on top answers Esc and a press outside it: one opened from it closes first.
    let mine = LayerId::new(Order::Foreground, float.id);
    let on_top = ctx.memory(|memory| memory.top_modal_layer()).is_none_or(|top| top == mine);
    let quiet = mode == Mode::Held { quiet: true };
    let mut leave = None;
    if !opening && on_top && !quiet {
        let rect = kept.map(|kept| kept.rect);
        let (escaped, beside) = ctx.input_mut(|input| {
            let beside = input.pointer.any_pressed() && input.pointer.interact_pos().is_some_and(|at| rect.is_some_and(|rect| !rect.contains(at)));
            (input.consume_key(Modifiers::NONE, Key::Escape), beside)
        });
        if mode != Mode::Closes {
            // Held: the caller decides, and the float stays as it is (no second unfolding).
            leave = if escaped {
                Some(Leave::Escape)
            } else if beside {
                Some(Leave::Beside)
            } else {
                None
            };
        } else if escaped || beside {
            ctx.data_mut(|data| data.remove::<Kept>(key));
            if beside {
                // The press that closed it may come up on the control it opened from: that is not a press to open it.
                ctx.data_mut(|data| data.insert_temp(float.id.with("closing-press"), ()));
            }
            ctx.memory_mut(|memory| memory.request_focus(float.returns_to));
            return None;
        }
    } else if opening {
        // The keyboard leaves the control the float opened from, so that Tab starts on the float's first row.
        ctx.memory_mut(|memory| {
            if let Some(focused) = memory.focused() {
                memory.surrender_focus(focused);
            }
        });
    }
    let opened = kept.map_or(now, |kept| kept.opened);
    let size = kept.map_or(vec2(width, 0.0), |kept| kept.rect.size());
    let at = placed(float.anchor.left_bottom() + vec2(0.0, DROP), size, ctx.content_rect(), ctx.pixels_per_point());
    // A float opened from inside another stands directly over it, however the two were ordered before (a list of a
    // select in the event editor, opened a second time).
    let parent = ctx.data(|data| data.get_temp::<LayerId>(showing_id()));
    if let Some(parent) = parent.filter(|parent| *parent != mine) {
        ctx.memory_mut(|memory| memory.areas_mut().set_sublayer(parent, mine));
    }
    if !quiet {
        // Only the float takes the keyboard and the pointer while it is open: Tab goes round its rows.
        ctx.memory_mut(|memory| memory.set_modal_layer(mine));
        // Drawn after the float it may be opened from, it stands over it: a float opened from another is on top.
        ctx.move_to_top(mine);
    }
    let shown = Area::new(float.id).order(Order::Foreground).pivot(Align2::LEFT_TOP).fixed_pos(at).fade_in(false).show(ctx, |ui| {
        // The float whose rows are drawn now: one opened from them stands over it.
        ctx.data_mut(|data| data.insert_temp(showing_id(), mine));
        unfolding(ui, theme, width, opened, |ui| {
            // Its rows scroll once it would be taller than the window: it was as tall as the room last frame.
            if size.y >= room.y - 0.5 {
                let area = ScrollArea::vertical().id_salt(float.id.with("rows")).max_height(room.y - 2.0);
                super::scroll::show_thin(ui, theme, area, THUMB, [BAR, THUMB_ENDS], add).inner
            } else {
                ui.set_max_height(room.y - 2.0);
                add(ui)
            }
        })
    });
    ctx.data_mut(|data| {
        if let Some(parent) = parent {
            data.insert_temp(showing_id(), parent);
        } else {
            data.remove::<LayerId>(showing_id());
        }
    });
    ctx.data_mut(|data| data.insert_temp(key, Kept { opened, pass, rect: shown.response.rect }));
    let (inner, rows) = shown.inner;
    // Home and End go to the first and the last row, from a row (a field in the float keeps them for its text).
    let focused = ctx.memory(|memory| memory.focused());
    if focused.is_some_and(|focused| rows.contains(&focused)) {
        let (home, end) = ctx.input_mut(|input| (input.consume_key(Modifiers::NONE, Key::Home), input.consume_key(Modifiers::NONE, Key::End)));
        let to = if home {
            rows.first()
        } else if end {
            rows.last()
        } else {
            None
        };
        if let Some(to) = to {
            ctx.memory_mut(|memory| memory.request_focus(*to));
        }
    }
    Some((inner, leave))
}

/// Draws a float `width` points wide into `ui` with `add` drawing its rows, 1 px in from its edge.
pub fn panel<R>(ui: &mut Ui, theme: &Theme, width: f32, add: impl FnOnce(&mut Ui) -> R) -> R {
    let slot = ui.painter().add(Shape::Noop);
    let width = sane(width);
    let inner = egui::Frame::new().inner_margin(egui::Margin::same(1)).show(ui, |ui| {
        ui.vertical(|ui| {
            ui.set_width(width - 2.0);
            ui.spacing_mut().item_spacing.y = 0.0;
            add(ui)
        })
        .inner
    });
    let shapes = paint::float_shapes(theme, inner.response.rect, FloatEdge::Keyboard);
    ui.painter().set(slot, Shape::Vec(shapes));
    inner.inner
}

/// Shows a float `id` with its top left corner at `anchor` that does not take the keyboard (the recipient
/// suggestions, which the field's arrows move through): drawn as [`float`], kept inside the window, its rows coming
/// in as [`float`]'s; what `add` gave.
pub fn show<R>(ctx: &Context, theme: &Theme, id: Id, anchor: Pos2, width: f32, add: impl FnOnce(&mut Ui) -> R) -> R {
    let key = id.with("float-kept");
    let pass = ctx.cumulative_pass_nr();
    let now = ctx.input(|input| input.time);
    let kept = ctx.data(|data| data.get_temp::<Kept>(key)).filter(|kept| kept.pass.saturating_add(1) >= pass);
    let width = sane(width);
    let opened = kept.map_or(now, |kept| kept.opened);
    let size = kept.map_or(vec2(width, 0.0), |kept| kept.rect.size());
    let at = placed(anchor, size, ctx.content_rect(), ctx.pixels_per_point());
    let shown = Area::new(id)
        .order(Order::Foreground)
        .pivot(Align2::LEFT_TOP)
        .fixed_pos(at)
        .fade_in(false)
        .show(ctx, |ui| unfolding(ui, theme, width, opened, add));
    ctx.data_mut(|data| data.insert_temp(key, Kept { opened, pass, rect: shown.response.rect }));
    shown.inner.0
}

/// A line across a float: 1 px of `line`.
pub fn hairline(ui: &mut Ui, theme: &Theme) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    if ui.is_rect_visible(rect) {
        ui.painter().rect_filled(rect, 0.0, theme.palette.line);
    }
}

/// A float's title row: 28 high on `panel`, `label` in the `label` style and `count` in `mono_s` at its right end,
/// both `muted`; a hairline under it.
pub fn title(ui: &mut Ui, theme: &Theme, label: &str, count: Option<&str>) {
    let p = &theme.palette;
    let s = &theme.spacing;
    let height = theme.boxes().float_title;
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, label));
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        painter.rect_filled(rect, 0.0, p.panel);
        let name = galley(ui, label, theme.fonts.label, p.muted);
        paint::run_at(painter, pos2(rect.left() + s.s3, rect.center().y - name.size().y / 2.0), name, p.muted);
        if let Some(count) = count {
            let count = galley(ui, count, theme.fonts.mono_s, p.muted);
            paint::run_right(painter, rect.right() - s.s3, rect.center().y - count.size().y / 2.0, count, p.muted);
        }
    }
    hairline(ui, theme);
}

/// What a float says when it has no rows: `text` in `body` and `muted`, 12 in on every side.
pub fn empty(ui: &mut Ui, theme: &Theme, text: &str) {
    let s = &theme.spacing;
    let width = ui.available_width();
    let mut job = egui::text::LayoutJob::single_section(text.to_owned(), theme.fonts.body.format(theme.palette.muted));
    job.wrap.max_width = (width - s.s3 * 2.0).max(1.0);
    let text = ui.painter().layout_job(job);
    let (rect, response) = ui.allocate_exact_size(vec2(width, text.size().y + s.s3 * 2.0), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, text.text()));
    if ui.is_rect_visible(rect) {
        paint::rows_at(ui.painter(), rect.min + vec2(s.s3, s.s3), width - s.s3 * 2.0, text, theme.palette.muted);
    }
}

/// The rows of a float that may be more than fit: `add` draws them in an area at most `most` high that scrolls, its
/// thumb 2 × 40 in `muted` 2 px in from the float's right edge, no strip ([`super::scroll::show_thin`]); what `add`
/// gave.
pub fn scrolled<R>(ui: &mut Ui, theme: &Theme, id: Id, most: f32, add: impl FnOnce(&mut Ui) -> R) -> R {
    let most = if most.is_finite() { most.max(theme.boxes().menu_row) } else { theme.boxes().menu_row };
    let area = ScrollArea::vertical().id_salt(id).max_height(most);
    super::scroll::show_thin(ui, theme, area, THUMB, [BAR, THUMB_ENDS], add).inner
}

/// The state of a menu row, as its sheet names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemState {
    /// A row that can be chosen.
    Default,
    /// The row Enter runs: the `stage` fill, the accent bar and the Enter chip.
    Chosen,
    /// A row that cannot be chosen now: `muted` words, a `dim` icon, its caption saying why.
    Unavailable,
    /// A row that throws something away: its icon and words in `minus`.
    Danger,
}

/// A menu row: what it shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Item<'a> {
    /// The icon before the name, `None` for none (the name then starts 12 in).
    pub icon: Option<IconId>,
    /// What it does.
    pub name: &'a str,
    /// A short line at its right end in `mono_s` (a time, why it is unavailable).
    pub caption: Option<&'a str>,
    /// The action whose key chip it shows at its right end.
    pub action: Option<&'a str>,
    /// Its state.
    pub state: ItemState,
}

/// What a row of a float shows of the pointer and the keyboard, and how far it has come in.
#[derive(Clone, Copy)]
struct Row {
    rect: Rect,
    hover: bool,
    pressed: bool,
    focus: bool,
    /// How far it has come in (1: in place).
    shown: f32,
}

/// Allocates a row `height` high across `ui` and reads it; `None` for the response's own state.
fn row_rect(ui: &mut Ui, theme: &Theme, height: f32, enabled: bool, forced: Option<Shown>) -> (Response, Row) {
    let wanted = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width().max(height), height), soon::sense(ui, wanted));
    let pressable = enabled && !soon::is_soon(ui);
    let lit = lit(ui, theme, &response, pressable, forced, Hover::AtOnce);
    let shown = line_in(ui, theme, response.id, pressable);
    (response, Row { rect, hover: lit.hover, pressed: lit.pressed, focus: lit.focus, shown })
}

/// The fill and edge of a row: hovered or chosen the `stage`, pressed as the module says.
fn row_ground(theme: &Theme, row: Row, chosen: bool) -> (Option<Color32>, Option<Color32>) {
    let p = &theme.palette;
    if row.pressed {
        if chosen { (Some(p.cyan_muted), Some(p.cyan_edge)) } else { (Some(p.stage), Some(p.ink)) }
    } else if row.focus {
        (chosen.then_some(p.stage), Some(p.cyan_edge))
    } else if row.hover || chosen {
        (Some(p.stage), None)
    } else {
        (None, None)
    }
}

/// Paints a row's ground, its edge, and the accent bar of a chosen row.
fn paint_ground(painter: &egui::Painter, theme: &Theme, row: Row, chosen: bool) {
    let (fill, edge) = row_ground(theme, row, chosen);
    if let Some(fill) = fill {
        painter.rect_filled(row.rect, 0.0, fill);
    }
    if chosen {
        painter.rect_filled(Rect::from_min_size(row.rect.min, vec2(BAR, row.rect.height())), 0.0, theme.palette.cyan_edge);
    }
    if let Some(edge) = edge {
        painter.rect_stroke(row.rect, 0.0, Stroke::new(1.0, edge), StrokeKind::Inside);
    }
}

/// `colour` as a row that has come in `shown` of the way shows it: mixed from the float's `surface`.
fn arriving(theme: &Theme, colour: Color32, shown: f32) -> Color32 {
    if shown >= 1.0 { colour } else { mix(theme.palette.surface, colour, shown) }
}

/// A menu row, 32 high, filling the width `ui` offers (see the module): the icon (`muted`, `ink` hovered or chosen,
/// `dim` unavailable, `minus` for danger), the name in `body` cut with an ellipsis, the caption in `mono_s` and the
/// key chip of its action (Enter on the chosen row) at its right end. It is clicked when chosen.
pub fn menu_item(ui: &mut Ui, theme: &Theme, keymap: &Keymap, item: &Item<'_>) -> Response {
    menu_item_with(ui, theme, keymap, item, false, None)
}

/// [`menu_item`] drawn in `shown` whatever the pointer and the keyboard do (a board of the gallery); `column` keeps
/// the icon's room when it has none, so its name lines up with the rows that have one.
pub(crate) fn menu_item_with(ui: &mut Ui, theme: &Theme, keymap: &Keymap, item: &Item<'_>, column: bool, shown: Option<Shown>) -> Response {
    let p = &theme.palette;
    let s = &theme.spacing;
    let soon = soon::is_soon(ui);
    let unavailable = item.state == ItemState::Unavailable;
    if let Some(action) = item.action {
        soon::offer(ui, action);
    }
    let (response, row) = row_rect(ui, theme, theme.boxes().menu_row, !unavailable && ui.is_enabled(), shown);
    response.widget_info(|| WidgetInfo::labeled(Role::MenuItem, !soon && !unavailable && ui.is_enabled(), item.name));
    if !ui.is_rect_visible(row.rect) {
        return response;
    }
    let chosen = item.state == ItemState::Chosen;
    let painter = solid(ui);
    paint_ground(&painter, theme, row, chosen);
    let lit = row.hover || chosen || row.pressed;
    let (icon_ink, name_ink) = match item.state {
        _ if soon => (p.dim, p.muted),
        ItemState::Unavailable => (p.dim, p.muted),
        ItemState::Danger => (p.minus, p.minus),
        _ if lit => (p.ink, p.ink),
        _ => (p.muted, p.ink),
    };
    let shown = row.shown;
    let drop = RISE * (1.0 - shown);
    // Focused, the content stands 1 px further in, inside the edge.
    let inset = s.s3 + if row.focus { 1.0 } else { 0.0 };
    let mid = row.rect.center().y - drop;
    let mut left = row.rect.left() + inset;
    let side = theme.boxes().icon;
    if item.icon.is_some() || column {
        if let Some(icon) = item.icon {
            let at = Rect::from_min_size(pos2(left, mid - side / 2.0), vec2(side, side));
            crate::iconset::paint(&painter, at, icon, arriving(theme, icon_ink, shown));
        }
        left += side + s.s2;
    }
    let mut right = row.rect.right() - inset;
    // The chip: Enter on the chosen row, else the key of its action; none coming soon (the key would do nothing).
    let key = if chosen { Some(super::key::key_words(&egui::KeyboardShortcut::new(Modifiers::NONE, Key::Enter))) } else { None }
        .or_else(|| item.action.filter(|_| !soon && !unavailable).and_then(|action| keymap.chip(ui.ctx(), action)));
    if let Some(key) = key {
        let chip = Chip::new(ui, theme, &key, ChipState::Default);
        let size = chip.size();
        right -= size.x;
        chip.paint(&painter, pos2(right, (mid - size.y / 2.0).round()));
        right -= s.s2;
    }
    if let Some(caption) = item.caption {
        let caption = galley(ui, caption, theme.fonts.mono_s, p.muted);
        let colour = arriving(theme, p.muted, shown);
        right = paint::run_right(&painter, right, mid - caption.size().y / 2.0, caption, colour) - s.s2;
    }
    let ink = arriving(theme, name_ink, shown);
    let text = cut(ui, item.name, theme.fonts.body, ink, right - left);
    paint::run_at(&painter, pos2(left, mid - text.size().y / 2.0), text, ink);
    response
}

/// A choice row of a select, 32 high (`menu-choice`): the tick in `cyan_text` when it is `on`, the name in `body`;
/// unavailable, the name `muted` and no press.
pub fn menu_choice(ui: &mut Ui, theme: &Theme, name: &str, on: bool, available: bool) -> Response {
    menu_choice_with(ui, theme, name, on, if available { None } else { Some("") }, None)
}

/// [`menu_choice`] unavailable when there is a `reason`, which stands at its right end in `mono_s` and `muted` (empty:
/// none); drawn in `shown` for a board of the gallery.
pub(crate) fn menu_choice_with(ui: &mut Ui, theme: &Theme, name: &str, on: bool, reason: Option<&str>, shown: Option<Shown>) -> Response {
    let p = &theme.palette;
    let s = &theme.spacing;
    let soon = soon::is_soon(ui);
    let available = reason.is_none() && ui.is_enabled();
    let (response, row) = row_rect(ui, theme, theme.boxes().menu_row, available, shown);
    response.widget_info(|| WidgetInfo::selected(Role::MenuItemRadio, available && !soon, on, name));
    if !ui.is_rect_visible(row.rect) {
        return response;
    }
    let painter = solid(ui);
    // A choice row has no bar: its tick says which one is on. Pressed, the one on takes the accent wash.
    if row.pressed && on {
        painter.rect(row.rect, 0.0, p.cyan_muted, Stroke::new(1.0, p.cyan_edge), StrokeKind::Inside);
    } else {
        paint_ground(&painter, theme, row, false);
    }
    let shown = row.shown;
    let drop = RISE * (1.0 - shown);
    let inset = s.s3 + if row.focus { 1.0 } else { 0.0 };
    let mid = row.rect.center().y - drop;
    let side = theme.boxes().icon;
    if on {
        let at = Rect::from_min_size(pos2(row.rect.left() + inset, mid - side / 2.0), vec2(side, side));
        let tick = if soon || !available { p.dim } else { p.cyan_text };
        crate::iconset::paint_glyph(&painter, at, CHECK, arriving(theme, tick, shown));
    }
    let left = row.rect.left() + inset + side + s.s2;
    let mut right = row.rect.right() - inset;
    if let Some(reason) = reason.filter(|reason| !reason.is_empty()) {
        let reason = galley(ui, reason, theme.fonts.mono_s, p.muted);
        right = paint::run_right(&painter, right, mid - reason.size().y / 2.0, reason, arriving(theme, p.muted, shown)) - s.s2;
    }
    let ink = arriving(theme, if available && !soon { p.ink } else { p.muted }, shown);
    let text = cut(ui, name, theme.fonts.body, ink, right - left);
    paint::run_at(&painter, pos2(left, mid - text.size().y / 2.0), text, ink);
    response
}

/// The tick of a chosen choice: Phosphor's check.
pub(crate) const CHECK: char = '\u{e182}';

/// The widest a suggestion's name gets beside its address, at 100 %, in points.
const NAME: f32 = 144.0;

/// A row of the recipient suggestions, 40 high (`suggestion-row`): the person's avatar (24, on `stage`, on `surface`
/// when the row is lit), the name in `body` and the address in `mono_s` and `muted` after it, both cut with an
/// ellipsis; `selected`, the row Enter takes: the `stage` fill, the accent bar and the Enter chip. Clicked when chosen.
pub fn suggestion_row(ui: &mut Ui, theme: &Theme, name: &str, address: &str, selected: bool) -> Response {
    suggestion_row_with(ui, theme, name, address, selected, None)
}

/// [`suggestion_row`] drawn in `shown`: a board of the gallery.
pub(crate) fn suggestion_row_with(ui: &mut Ui, theme: &Theme, name: &str, address: &str, selected: bool, shown: Option<Shown>) -> Response {
    let p = &theme.palette;
    let s = &theme.spacing;
    let (response, row) = row_rect(ui, theme, theme.boxes().suggestion, ui.is_enabled(), shown);
    let spoken = if address.is_empty() { name.to_owned() } else { tr!("{name}, {address}", name = name, address = address) };
    response.widget_info(|| WidgetInfo::selected(Role::ListBoxOption, ui.is_enabled(), selected, &spoken));
    if !ui.is_rect_visible(row.rect) {
        return response;
    }
    let painter = solid(ui);
    paint_ground(&painter, theme, row, selected);
    let shown = row.shown;
    let drop = RISE * (1.0 - shown);
    let inset = s.s3 + if row.focus { 1.0 } else { 0.0 };
    let mid = row.rect.center().y - drop;
    let side = theme.boxes().avatar[0];
    let face = Rect::from_min_size(pos2(row.rect.left() + inset, (mid - side / 2.0).round()), vec2(side, side));
    let ground = if row.hover || selected || row.pressed { p.surface } else { p.stage };
    super::avatar::paint_on(&painter, theme, face, name, address, ground);
    let left = face.right() + s.s2;
    let mut right = row.rect.right() - inset;
    if selected {
        let chip = Chip::new(ui, theme, &super::key::key_words(&egui::KeyboardShortcut::new(Modifiers::NONE, Key::Enter)), ChipState::Default);
        let size = chip.size();
        right -= size.x;
        chip.paint(&painter, pos2(right, (mid - size.y / 2.0).round()));
        right -= s.s2;
    }
    let ink = arriving(theme, p.ink, shown);
    let most = NAME.min((right - left).max(1.0));
    // The name's box: its width rounded up, or all of `most` when the name is cut there; the address 8 after it.
    let whole = galley(ui, name, theme.fonts.body, ink).size().x;
    let name_box = if whole > most { most } else { paint::run_box(whole) };
    let text = cut(ui, name, theme.fonts.body, ink, most);
    paint::run_at(&painter, pos2(left, mid - text.size().y / 2.0), text, ink);
    let start = left + name_box + s.s2;
    if right - start > 1.0 && !address.is_empty() {
        let muted = arriving(theme, p.muted, shown);
        let address = cut(ui, address, theme.fonts.mono_s, muted, right - start);
        paint::run_at(&painter, pos2(start, mid - address.size().y / 2.0), address, muted);
    }
    response
}

/// A menu row: `icon`, `name` and the key of `action` from `keymap` at its right end; clicked when chosen.
pub fn item(ui: &mut Ui, theme: &Theme, keymap: &Keymap, icon: IconId, name: &str, action: Option<&str>) -> Response {
    menu_item(ui, theme, keymap, &Item { icon: Some(icon), name, caption: None, action, state: ItemState::Default })
}

/// A menu row without an icon, its name in line with the names of the rows that have one.
pub fn entry(ui: &mut Ui, theme: &Theme, keymap: &Keymap, name: &str, action: Option<&str>) -> Response {
    menu_item_with(ui, theme, keymap, &Item { icon: None, name, caption: None, action, state: ItemState::Default }, true, None)
}

/// A choice row: `name`, ticked when it is the one `chosen`; clicked when picked.
pub fn choice(ui: &mut Ui, theme: &Theme, name: &str, chosen: bool) -> Response {
    menu_choice(ui, theme, name, chosen, true)
}

/// A choice row of a select: [`choice`] (the select's sheet draws its rows as `menu-choice`, without an icon).
pub fn option(ui: &mut Ui, theme: &Theme, _icon: IconId, name: &str, chosen: bool) -> Response {
    menu_choice(ui, theme, name, chosen, true)
}

/// Whether the float `id` is open: a flag kept in `ctx` for a float opened by a control or a key elsewhere.
#[must_use]
pub fn is_open(ctx: &Context, id: Id) -> bool {
    ctx.data(|data| data.get_temp::<bool>(id.with("open"))).unwrap_or(false)
}

/// Opens or closes the float `id` ([`is_open`]).
pub fn set_open(ctx: &Context, id: Id, open: bool) {
    ctx.data_mut(|data| data.insert_temp(id.with("open"), open));
}

/// Whether `response` (the control the float `id` opens from) was clicked to open or close it: a click that is the
/// release of the press that closed the float (outside it, on the control) is not one.
#[must_use]
pub fn clicked(ctx: &Context, response: &Response, id: Id) -> bool {
    let key = id.with("closing-press");
    let stale = ctx.data(|data| data.get_temp::<()>(key)).is_some();
    if stale && (response.clicked() || !ctx.input(|input| input.pointer.any_down())) {
        ctx.data_mut(|data| data.remove::<()>(key));
    }
    response.clicked() && !stale
}

/// How many frames a control asks for the keyboard back after its float closed on a choice: the float held the
/// keyboard that frame and the next.
const REFOCUS_FRAMES: u8 = 3;

/// Gives the keyboard back to `opener` while the menu `id` closed on a choice asks for it.
fn refocus(ctx: &Context, opener: &Response, id: Id) {
    let key = id.with("refocus");
    if let Some(left) = ctx.data(|data| data.get_temp::<u8>(key)) {
        if opener.has_focus() || left == 0 || is_open(ctx, id) {
            ctx.data_mut(|data| data.remove::<u8>(key));
        } else {
            opener.request_focus();
            ctx.data_mut(|data| data.insert_temp(key, left.saturating_sub(1)));
        }
    }
}

/// Where a menu keeps whether it was opened from the keyboard.
fn by_key_id(id: Id) -> Id {
    id.with("opened-by-key")
}

/// A menu `width` wide that `opener` opens and closes (a press, or Enter or Space on it): the [`float`] `id` hung
/// under it while it is open, `add` drawing its rows and saying whether one was chosen, which closes it. Esc closes it
/// and gives `opener` the keyboard. A menu opened from the keyboard (Enter or Space on `opener`, or a screen reader's
/// press on it, focused or not) gives `opener` the keyboard back however it closes, whether a key or the pointer chose
/// its row; one opened by the pointer, chosen or closed by a press outside, leaves the keyboard with nothing, as it
/// was (the window's keys then act). What `add` gave while it is open.
pub fn menu<R>(ctx: &Context, theme: &Theme, opener: &Response, id: Id, width: f32, add: impl FnOnce(&mut Ui) -> (R, bool)) -> Option<R> {
    refocus(ctx, opener, id);
    if clicked(ctx, opener, id) {
        let open = !is_open(ctx, id);
        set_open(ctx, id, open);
        // Enter or Space on the focused opener, or a screen reader's press (which need not move the focus): any press
        // that is not the pointer's.
        let by_key = open && !opener.clicked_by(egui::PointerButton::Primary);
        ctx.data_mut(|data| data.insert_temp(by_key_id(id), by_key));
    }
    menu_at(ctx, theme, opener, opener.rect, id, width, add)
}

/// A menu that a secondary press on `row` opens where the pointer is (a row's context menu), as [`menu`] shows it.
pub fn context_menu<R>(ctx: &Context, theme: &Theme, row: &Response, id: Id, width: f32, add: impl FnOnce(&mut Ui) -> (R, bool)) -> Option<R> {
    refocus(ctx, row, id);
    let at = id.with("at");
    if row.secondary_clicked() {
        let pointer = row.interact_pointer_pos().unwrap_or(row.rect.left_bottom());
        ctx.data_mut(|data| data.insert_temp(at, pointer));
        ctx.data_mut(|data| data.insert_temp(by_key_id(id), false));
        set_open(ctx, id, true);
    }
    let anchor = ctx.data(|data| data.get_temp::<Pos2>(at)).map_or(row.rect, |pointer| Rect::from_min_size(pointer - vec2(0.0, DROP), Vec2::ZERO));
    menu_at(ctx, theme, row, anchor, id, width, add)
}

/// The open menu `id` under `anchor`, giving `opener` the keyboard back when it closes as [`menu`] says.
fn menu_at<R>(
    ctx: &Context,
    theme: &Theme,
    opener: &Response,
    anchor: Rect,
    id: Id,
    width: f32,
    add: impl FnOnce(&mut Ui) -> (R, bool),
) -> Option<R> {
    if !is_open(ctx, id) {
        return None;
    }
    let by_key = ctx.data(|data| data.get_temp::<bool>(by_key_id(id))).unwrap_or(false);
    let shown = float(ctx, theme, &Float { id, anchor, width, returns_to: opener.id }, add);
    match shown {
        None => {
            set_open(ctx, id, false);
            // The float gave the opener the keyboard; a press outside a menu the pointer opened takes it away again
            // (Esc leaves it there).
            let beside = ctx.data(|data| data.get_temp::<()>(id.with("closing-press"))).is_some();
            if beside && !by_key {
                ctx.memory_mut(|memory| memory.surrender_focus(opener.id));
            }
            None
        }
        Some((inner, chosen)) => {
            if chosen {
                set_open(ctx, id, false);
                if by_key {
                    opener.request_focus();
                    ctx.data_mut(|data| data.insert_temp(id.with("refocus"), REFOCUS_FRAMES));
                }
            }
            Some(inner)
        }
    }
}

/// Whether `Esc` was pressed for a control under the floats: false while a float is open, which takes it.
#[must_use]
pub fn escape(ctx: &Context) -> bool {
    let open = ctx.memory(|memory| memory.top_modal_layer().is_some());
    !open && ctx.input(|input| input.key_pressed(Key::Escape))
}

#[cfg(test)]
#[path = "popup_tests.rs"]
pub(crate) mod tests;
