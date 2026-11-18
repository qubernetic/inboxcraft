// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Text fields, as their sheets draw them (`text-field`, `multiline-field`, `password-field`): the label over the box
//! in `label` (Plex Mono 11/16 in capitals, `muted`), 8 above it; the box on `term` with a 1 px `line` edge inside,
//! 36 high for one line; the text 13 in, in `body` (Plex Sans 14/22); the hint `muted` while it is empty. Hovered the
//! edge is `muted`, focused `cyan_edge` with the caret, a 7 × 16 block in `cyan_edge` 2 after the text that blinks
//! once a second ([`crate::motion::blink`]); an error's edge is `minus` with its message under the box in `caption`
//! and `minus` (D14). Disabled: no fill, a dashed `line` edge, the text `muted`, never faded. Read-only: no fill, the
//! `line` edge and the lock at the right end. Nothing else moves: the design gives a field's edge no transition.
//!
//! Also the search field (a square field with a magnifier before the text), the margin of a form field, and the plain
//! line and text the composer writes in until its own restyle.

use std::sync::Arc;

use egui::text::{CCursor, CCursorRange, LayoutJob, TextWrapping};
use egui::text_edit::{TextEditOutput, TextEditState};
use egui::{
    Align2, Color32, Galley, Id, Margin, Painter, Rect, Response, ScrollArea, Sense, Stroke, StrokeKind, TextBuffer, TextEdit, Ui, UiBuilder, Vec2,
    pos2, vec2,
};

use crate::draw::paint;
use crate::iconset::IconId;
use crate::motion;
use crate::theme::{Font, Theme};
use crate::tr;

use super::{Shown, scroll, soon};

#[path = "field_capped.rs"]
mod capped;

use capped::{Capped, Outcome};

/// The margin inside a form field: the field is as tall as a button with a little more room.
#[must_use]
pub fn margin(theme: &Theme) -> Margin {
    let s = &theme.spacing;
    // A few points each; the clamp keeps the casts inside i8.
    Margin::symmetric(s.s2.clamp(0.0, 127.0) as i8, (s.s1 + 2.0).clamp(0.0, 127.0) as i8)
}

/// The search pill `width` points wide: the field colour with its border (the focus ring while it has the focus), a
/// magnifier and the text, `hint` while it is empty. Its response is the text field's.
pub fn search(ui: &mut Ui, theme: &Theme, id: Id, text: &mut String, hint: &str, width: f32) -> Response {
    let p = &theme.palette;
    let s = &theme.spacing;
    let height = theme.boxes().field;
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let round = 0.0;
    let focused = ui.memory(|memory| memory.has_focus(id));
    let (edge, ink) = if focused { (Stroke::new(2.0, p.cyan_edge), p.ink) } else { (Stroke::new(1.0, p.line), p.muted) };
    // Coming soon, the field is disabled with its `Ui`, and its edge is dashed.
    let soon = soon::is_soon(ui);
    if ui.is_rect_visible(rect) {
        ui.painter().rect_filled(rect, round, p.term);
        if soon {
            paint::dashed(ui.painter(), rect, p.muted);
        } else {
            ui.painter().rect_stroke(rect, round, edge, StrokeKind::Inside);
        }
    }
    let icon = theme.fonts.body.size * 1.15;
    let at = Rect::from_center_size(pos2(rect.left() + s.s3 + icon / 2.0, rect.center().y), vec2(icon, icon));
    crate::iconset::paint(ui.painter(), at, IconId::Search, ink);
    let inner = Rect::from_min_max(pos2(at.right() + s.s2, rect.top()), pos2(rect.right() - s.s3, rect.bottom()));
    let edit = TextEdit::singleline(text)
        .id(id)
        .hint_text(hint)
        .frame(egui::Frame::NONE)
        .margin(Margin::ZERO)
        .align(Align2::LEFT_CENTER)
        .desired_width(inner.width());
    ui.put(inner, edit)
}

/// What a field says and how it may be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Field<'a> {
    /// Its name, over the box in capitals and for a screen reader as written; empty for none.
    pub label: &'a str,
    /// What stands in it while it is empty.
    pub hint: &'a str,
    /// What is wrong with what is in it, under the box (D14).
    pub error: Option<&'a str>,
    /// Whether it can be used; a field in a disabled `Ui` or coming soon cannot either.
    pub enabled: bool,
    /// Whether its text can be read and copied but not changed.
    pub read_only: bool,
}

impl<'a> Field<'a> {
    /// A usable field named `label` with `hint` in it while it is empty.
    #[must_use]
    pub const fn new(label: &'a str, hint: &'a str) -> Self {
        Self { label, hint, error: None, enabled: true, read_only: false }
    }
}

/// What a password field holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Secret {
    /// What the person types: a dot for every character.
    Typed,
    /// A password kept in the system's credential store: never shown, not even as dots; the field takes a new one,
    /// and its hint and the sentence under it say that leaving it empty keeps the stored one.
    Stored,
}

/// The longest password a field takes, in characters.
pub const MAX_SECRET: usize = 256;

/// The Phosphor glyphs the field sheets name that are not icons of the set: the lock of a read-only or stored field
/// and the password mark.
const LOCK: char = '\u{e2fa}';
const PASSWORD: char = '\u{e752}';

/// The caret's width at 100 %, in points.
const CARET_WIDTH: f32 = 7.0;

/// The caret's block, 7 × 16 at 100 %, in points at the text scale.
fn caret_size(theme: &Theme) -> Vec2 {
    vec2((CARET_WIDTH * theme.text_scale.factor()).round(), theme.boxes().caret)
}

/// The room between the box's outer edge and its text, left and right: the 12 the sheet pads and the 1 px edge.
fn inset(theme: &Theme) -> f32 {
    theme.spacing.s3 + 1.0
}

/// The most characters a one-line field holds (a text, a password, a typed time, an address being typed); a field may
/// hold fewer ([`text_field_limited`]).
pub const MAX_LINE: usize = 4096;

/// The most bytes a field of several lines holds unless its caller names more ([`multiline_field_holding`]): 64 KiB,
/// so that a frame with a screen reader stays within its budget (egui gives every glyph to AccessKit every frame).
pub const MAX_TEXT: usize = 64 << 10;

/// The most bytes the composer's message text holds ([`plain_text`]): 1 MiB. A frame costs more near it with a screen
/// reader, but a long message is worth more than a quick frame.
pub const MAX_BODY: usize = 1 << 20;

/// What a field holds at most: a caller names it for [`multiline_field_holding`] (in the unit its text is kept in).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cap {
    /// So many characters.
    Chars(usize),
    /// So many bytes.
    Bytes(usize),
}

impl Cap {
    /// The size of `text` in this cap's unit.
    fn measure(self, text: &str) -> usize {
        match self {
            Self::Chars(_) => text.chars().count(),
            Self::Bytes(_) => text.len(),
        }
    }

    fn most(self) -> usize {
        match self {
            Self::Chars(most) | Self::Bytes(most) => most,
        }
    }

    /// What the field says under itself when it refused something.
    pub(crate) fn sentence(self) -> String {
        match self {
            Self::Chars(most) => tr!("Too long: this field holds at most {count} characters, and nothing was added.", count = most),
            Self::Bytes(most) => tr!("Too long: this text holds at most {size}, and nothing was added.", size = bytes(most)),
        }
    }
}

/// `most` bytes in words: "64 KiB", "1 MiB", else "n bytes".
fn bytes(most: usize) -> String {
    const KIB: usize = 1 << 10;
    const MIB: usize = 1 << 20;
    if most >= MIB && most.is_multiple_of(MIB) {
        format!("{} MiB", most / MIB)
    } else if most >= KIB && most.is_multiple_of(KIB) {
        format!("{} KiB", most / KIB)
    } else {
        format!("{most} bytes")
    }
}

/// Where a field keeps that it refused something.
fn refused_id(id: Id) -> Id {
    id.with("refused-too-long")
}

/// What a field keeps of something it refused, while it has it to say: until its text changes (by an edit or from
/// outside) or it goes a frame without being drawn (its composer closed).
#[derive(Debug, Clone, Copy)]
struct Refused {
    /// The text's length in bytes when it was last seen; `None` after an edit in the frame of the refusal, taken again
    /// in the next frame.
    len: Option<usize>,
    /// The pass in which it was refused.
    since: u64,
    /// The last pass that drew the field.
    seen: u64,
}

/// Whether a refusal made in pass `since` is new in this pass: made in it, or in the pass this one redoes (a field that
/// refused after its edit ran has egui draw the frame again, so that the sentence shows at once).
fn fresh(ctx: &egui::Context, since: u64) -> bool {
    let pass = ctx.cumulative_pass_nr();
    since == pass || (since.saturating_add(1) == pass && ctx.current_pass_index() > 0)
}

/// Whether a field has something refused to say.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Refusal {
    /// Refused in this frame: the sentence is new and is brought into view.
    Fresh,
    /// Refused before, the text unchanged since.
    Standing,
}

/// What a field keeps of the text it shows, between frames: the most it lets the text hold, as long as the text is
/// the one it last saw, drawn or not in between (the composer is undrawn on another screen and keeps its limit).
///
/// Two numbers per field id, in egui's temporary memory: they stay after the field is gone for good, as egui's own
/// [`TextEditState`] for the id does, and are bounded the same way, by the fixed ids fields have (the composer's
/// lines, a signature per account, onboarding's fields), each overwritten in place when it is drawn again.
#[derive(Debug, Clone, Copy)]
struct Opened {
    /// The most the text may hold, in the cap's unit: the cap, or the size of a text that was longer when the field
    /// showed it first.
    limit: usize,
    /// The text's length in bytes after the field's last frame: another length means the text changed from outside.
    len: usize,
}

/// Where a field keeps its [`Opened`] (a number, in egui's temporary memory: never persisted).
fn opened_id(id: Id) -> Id {
    id.with("opened-limit")
}

/// What [`guard`] found before a field's edit, for [`hold`] after it. Nothing of the text: a range, sizes and flags.
struct Guarded {
    /// What the field has refused to say, as far as is known before the edit.
    said: Option<Refusal>,
    cap: Cap,
    /// The most the text may hold this frame (see [`Opened`]).
    limit: usize,
    /// The caret and selection egui kept for the field, if it kept a state for it.
    state: Option<CCursorRange>,
    /// Whether egui kept a state for the field before this frame.
    had_state: bool,
    /// The characters of the IME composition that stood in the text at the frame's start, as the field last saw it (its
    /// range the stored selection); `None` when there was none, or when anything else touched the field since.
    composing: Option<usize>,
    /// The composition counted on after the frame when something else touched the field too: [`hold`] keeps it only
    /// if the text did not change and the selection is as long.
    recheck: Option<usize>,
    /// An IME commit reached the field in this frame.
    commit: bool,
}

/// Whether `event` could move the selection or change the text of the field `id` (`focused`, `hovered` by the pointer,
/// `target` its node), IME events aside.
fn touches(event: &egui::Event, focused: bool, hovered: bool, target: egui::accesskit::NodeId) -> bool {
    use egui::Event;
    match event {
        Event::Text(_) | Event::Paste(_) | Event::Cut | Event::Key { pressed: true, .. } => focused,
        Event::MiddleClickPaste { .. } | Event::PointerButton { pressed: true, .. } => hovered,
        Event::AccessKitActionRequest(request) => request.target_node == target,
        _ => false,
    }
}

/// Before the edit of the field `id` holding `text` within `cap`: the limit for this frame (the cap, or the size of a
/// text that was longer when the field first showed it, kept until the text changes from outside, which also clears
/// egui's undo history for the field), the caret and selection egui kept, where an IME composition stands, and what the
/// field has refused to say. It reads the frame's input and changes nothing in it; the cap itself is kept by
/// [`Capped`].
fn guard(ui: &Ui, id: Id, text: &str, cap: Cap) -> Guarded {
    use egui::{Event, ImeEvent};
    let ctx = ui.ctx();
    let focused = ui.memory(|memory| memory.has_focus(id));
    let hovered = ctx.read_response(id).is_some_and(|response| response.contains_pointer());
    let target = id.accesskit_id();
    let pass = ctx.cumulative_pass_nr();
    let opened = ui.data(|data| data.get_temp::<Opened>(opened_id(id)));
    let limit = match opened.filter(|opened| opened.len == text.len()) {
        Some(opened) => opened.limit,
        None => {
            // Another text than the field last held (changed from outside, or shown first): egui's history for the
            // field belongs to the text before, and a state from it could be past the new limit, refused at every
            // undo. It goes with the old limit.
            if let Some(mut state) = TextEditState::load(ctx, id) {
                state.clear_undoer();
                state.store(ctx, id);
            }
            cap.most().max(cap.measure(text))
        }
    };
    let stored = ui.data(|data| data.get_temp::<usize>(composing_id(id)));
    let was_composing = stored.filter(|_| focused);
    let mut composing = was_composing;
    let (mut commit, mut other) = (false, false);
    ui.input(|input| {
        for event in &input.events {
            match event {
                Event::Ime(ImeEvent::Preedit { text, .. }) if focused => {
                    let chars = text.chars().count();
                    composing = (chars > 0).then_some(chars);
                }
                Event::Ime(ImeEvent::Commit(_)) if focused => {
                    composing = None;
                    commit = true;
                }
                // Typing, a paste, a screen reader's value end the composition (egui puts the text in its place).
                Event::Text(_) | Event::Paste(_) if focused => composing = None,
                _ => other |= touches(event, focused, hovered, target),
            }
        }
    });
    if composing != stored {
        set_composing(ctx, id, composing);
    }
    let state = TextEditState::load(ctx, id);
    let had_state = state.is_some();
    let state = state.and_then(|state| state.cursor.char_range());
    let kept = ui.data(|data| data.get_temp::<Refused>(refused_id(id)));
    let said = kept.filter(|kept| kept.seen.saturating_add(1) >= pass && kept.len.is_none_or(|len| len == text.len()));
    let said = match said {
        Some(said) => {
            let said = Refused { len: Some(text.len()), seen: pass, ..said };
            ui.data_mut(|data| data.insert_temp(refused_id(id), said));
            Some(if fresh(ctx, said.since) { Refusal::Fresh } else { Refusal::Standing })
        }
        None => {
            if kept.is_some() {
                ui.data_mut(|data| data.remove::<Refused>(refused_id(id)));
            }
            None
        }
    };
    Guarded { said, cap, limit, state, had_state, composing: was_composing, recheck: composing.filter(|_| other), commit }
}

/// Where a field keeps whether an IME composition stands in its text.
fn composing_id(id: Id) -> Id {
    id.with("ime-composing")
}

/// Keeps how many characters the IME composition in the text of the field `id` has, or that there is none.
fn set_composing(ctx: &egui::Context, id: Id, composing: Option<usize>) {
    ctx.data_mut(|data| match composing {
        Some(chars) => {
            data.insert_temp(composing_id(id), chars);
        }
        None => {
            data.remove::<usize>(composing_id(id));
        }
    });
}

/// The edit's buffer for `text`: `text` within the frame's `limit`, taking no change when not `mutable`.
fn capped<'a>(text: &'a mut String, guarded: &Guarded, mutable: bool, secret: bool) -> Capped<'a> {
    Capped::new(text, guarded.cap, guarded.limit, mutable, secret)
}

/// After the edit `id` (its `response`) ran on `text` and [`Capped`] said how the frame went (`outcome`).
///
/// A refused frame left the text as it began, in the same buffer; egui's state for the field is made to agree with it:
/// the caret and selection from before the frame are put back (none stored for a field egui kept nothing for), and a
/// refused IME commit takes its composition out of the text, as egui does when it ends one. egui's undo history never
/// saw a refused text: [`Capped`] rolled the frame back before egui fed its history at the end of its events. The
/// field says it is too long at once (egui draws the frame again), and the response says nothing changed. Then what
/// the field says is settled, and the limit kept for the next frame.
fn hold(ui: &Ui, id: Id, text: &mut String, guarded: Guarded, outcome: Outcome, response: &mut Response) {
    let ctx = ui.ctx();
    let pass = ctx.cumulative_pass_nr();
    if outcome.refused {
        let mut selection = guarded.state;
        let mut composing = guarded.composing;
        if let (true, Some(chars), Some(range)) = (guarded.commit, composing, selection)
            && range.primary.index.0.abs_diff(range.secondary.index.0) == chars
        {
            let at = take_out(text, range.primary.index.0, range.secondary.index.0);
            selection = Some(CCursorRange::one(CCursor::new(at)));
            composing = None;
        }
        // Indices egui keeps are its own for this text; held to its length all the same.
        let most = text.chars().count();
        let selection = selection.map(|range| {
            let mut range = range;
            range.primary.index.0 = range.primary.index.0.min(most);
            range.secondary.index.0 = range.secondary.index.0.min(most);
            range
        });
        if guarded.had_state {
            if let Some(mut state) = TextEditState::load(ctx, id) {
                state.cursor.set_char_range(selection);
                state.store(ctx, id);
            }
        } else {
            ctx.data_mut(|data| data.remove::<TextEditState>(id));
        }
        set_composing(ctx, id, composing);
        ui.data_mut(|data| data.insert_temp(refused_id(id), Refused { len: Some(text.len()), since: pass, seen: pass }));
        response.flags.set(egui::response::Flags::CHANGED, false);
        // The edit scrolled to where its caret went in the frame: a request for a place in view takes its place, so
        // that the view stays and the sentence can be brought into it.
        ui.scroll_to_rect(Rect::from_center_size(ui.clip_rect().center(), Vec2::ZERO), None);
        ctx.request_discard("a field refused what would carry its text past its cap");
        ctx.request_repaint();
    } else {
        if let Some(chars) = guarded.recheck {
            let range = TextEditState::load(ctx, id).and_then(|state| state.cursor.char_range());
            let still = !outcome.changed && range.is_some_and(|range| range.primary.index.0.abs_diff(range.secondary.index.0) == chars);
            if !still {
                set_composing(ctx, id, None);
            }
        }
        settle(ui, id, outcome.changed, guarded.said);
    }
    ui.data_mut(|data| data.insert_temp(opened_id(id), Opened { limit: guarded.limit, len: text.len() }));
}

/// Takes the characters `from..to` (either way round) out of `text`; where they started.
fn take_out(text: &mut String, from: usize, to: usize) -> usize {
    let (from, to) = (from.min(to), from.max(to));
    let byte = |chars: usize| text.char_indices().nth(chars).map_or(text.len(), |(at, _)| at);
    let (start, end) = (byte(from), byte(to));
    if start < end && text.is_char_boundary(start) && text.is_char_boundary(end) {
        text.replace_range(start..end, "");
    }
    from
}

/// After the edit `id` drew: a change it took clears what the field had refused before this frame; one refused in this
/// frame stays, the text's length taken again next frame.
fn settle(ui: &Ui, id: Id, changed: bool, said: Option<Refusal>) {
    if !changed {
        return;
    }
    if said == Some(Refusal::Fresh) {
        let kept = ui.data(|data| data.get_temp::<Refused>(refused_id(id)));
        if let Some(kept) = kept {
            ui.data_mut(|data| data.insert_temp(refused_id(id), Refused { len: None, ..kept }));
        }
    } else {
        ui.data_mut(|data| data.remove::<Refused>(refused_id(id)));
    }
}

/// What the field `id` says it refused, if it has something refused to say (see [`guard`]), and whether it is new in
/// this frame: for a field shown without its own error line (the recipient line's).
pub(crate) fn refusal(ctx: &egui::Context, id: Id) -> Option<(String, bool)> {
    let pass = ctx.cumulative_pass_nr();
    ctx.data(|data| data.get_temp::<Refused>(refused_id(id)))
        .filter(|kept| kept.seen.saturating_add(1) >= pass)
        .map(|kept| (Cap::Chars(MAX_LINE).sentence(), fresh(ctx, kept.since)))
}

/// The kind of a one-line field.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Line {
    Text,
    /// A line the app reads, in `mono` (the typed time).
    Mono,
    Password(Secret),
}

/// A one-line text field filling the width `ui` offers: its label, a box 36 high and the error under it (see the
/// module). Its response is the text's: `changed`, `lost_focus` and the focus are the field's.
pub fn text_field(ui: &mut Ui, theme: &Theme, id: Id, text: &mut String, field: &Field<'_>) -> Response {
    one_line(ui, theme, id, text, field, Line::Text, usize::MAX, None)
}

/// [`text_field`] that takes at most `limit` characters.
pub fn text_field_limited(ui: &mut Ui, theme: &Theme, id: Id, text: &mut String, field: &Field<'_>, limit: usize) -> Response {
    one_line(ui, theme, id, text, field, Line::Text, limit, None)
}

/// [`text_field`] drawn in `shown` whatever the pointer and the keyboard do: a board of the gallery. A forced focus
/// shows a still caret after the text.
pub(crate) fn text_field_with(ui: &mut Ui, theme: &Theme, id: Id, text: &mut String, field: &Field<'_>, shown: Option<Shown>) -> Response {
    one_line(ui, theme, id, text, field, Line::Text, usize::MAX, shown)
}

/// A one-line field whose text is `mono` (Plex Mono 13/20), its hint too, at most `limit` characters, drawn in
/// `shown` for a board: a line the app reads, as the typed time.
pub(crate) fn mono_field(ui: &mut Ui, theme: &Theme, id: Id, text: &mut String, field: &Field<'_>, limit: usize, shown: Option<Shown>) -> Response {
    one_line(ui, theme, id, text, field, Line::Mono, limit, shown)
}

/// A password field: [`text_field`] whose text is never shown, a dot (Plex Mono 13/20) for every character, the
/// password mark at the right end; at most [`MAX_SECRET`] characters. Nothing it holds reaches the undo history,
/// the clipboard or a screen reader. With [`Secret::Stored`] it shows no dots while it is empty, its hint (the
/// sheet's "empty keeps the stored one") in it, the lock at the right end and under it the sentence that the stored
/// password is kept in the credential store and never shown. Disabled it is an empty box.
pub fn password_field(ui: &mut Ui, theme: &Theme, id: Id, text: &mut String, field: &Field<'_>, secret: Secret) -> Response {
    one_line(ui, theme, id, text, field, Line::Password(secret), MAX_SECRET, None)
}

/// [`password_field`] drawn in `shown`: a board of the gallery.
pub(crate) fn password_field_with(
    ui: &mut Ui,
    theme: &Theme,
    id: Id,
    text: &mut String,
    field: &Field<'_>,
    secret: Secret,
    shown: Option<Shown>,
) -> Response {
    one_line(ui, theme, id, text, field, Line::Password(secret), MAX_SECRET, shown)
}

/// What a field shows of the pointer and the keyboard.
#[derive(Clone, Copy)]
struct Seen {
    enabled: bool,
    hover: bool,
    focus: bool,
    /// The focus is forced by a board: the caret stands still.
    forced: bool,
}

/// How the field `id` at `rect` is seen this frame (the focus and the pointer as the last frame left them).
fn seen(ui: &Ui, id: Id, rect: Rect, field: &Field<'_>, shown: Option<Shown>) -> Seen {
    let enabled = field.enabled && ui.is_enabled() && !soon::is_soon(ui);
    let forced = shown == Some(Shown::Focus);
    let focus = enabled && (forced || ui.memory(|memory| memory.has_focus(id)));
    let hover = enabled && !field.read_only && (shown == Some(Shown::Hover) || (shown.is_none() && ui.rect_contains_pointer(rect)));
    Seen { enabled, hover, focus, forced }
}

/// The box's fill and edge `(colour, dashed)` for `field` as `seen`.
fn box_look(theme: &Theme, field: &Field<'_>, seen: Seen) -> (Option<Color32>, Color32, bool) {
    let p = &theme.palette;
    if !seen.enabled {
        return (None, p.line, true);
    }
    let edge = if seen.focus {
        p.cyan_edge
    } else if field.error.is_some() {
        p.minus
    } else if seen.hover {
        p.muted
    } else {
        p.line
    };
    ((!field.read_only).then_some(p.term), edge, false)
}

/// Paints the box at `rect`: its fill and its 1 px edge inside, dashed when `dashed`.
fn paint_box(painter: &Painter, rect: Rect, fill: Option<Color32>, edge: Color32, dashed: bool) {
    if let Some(fill) = fill {
        painter.rect_filled(rect, 0.0, fill);
    }
    if dashed {
        paint::dashed(painter, rect, edge);
    } else {
        painter.rect_stroke(rect, 0.0, Stroke::new(1.0, edge), StrokeKind::Inside);
    }
}

/// The label over a field, `width` wide, wrapped when it is longer (at a large text size); nothing with no label.
fn label_row(ui: &mut Ui, theme: &Theme, label: &str, width: f32) {
    if label.is_empty() {
        return;
    }
    let font = theme.fonts.label;
    let mut job = LayoutJob::single_section(font.text(label), font.format(theme.palette.muted));
    job.wrap = TextWrapping { max_width: width.max(1.0), ..TextWrapping::default() };
    let text = ui.painter().layout_job(job);
    let (rect, _) = ui.allocate_exact_size(vec2(width, text.size().y.max(font.line)), Sense::hover());
    if ui.is_rect_visible(rect) {
        paint::rows_at(&super::solid(ui), rect.min, width, text, theme.palette.muted);
    }
}

/// The sentence under a field: its error in `minus`, else `note` in `muted`, wrapped at `width`; brought into view when
/// `bring` (a sentence that is new).
fn under(ui: &mut Ui, theme: &Theme, error: Option<&str>, note: Option<&str>, width: f32, bring: bool) {
    let p = &theme.palette;
    let Some((text, colour)) = error.map(|error| (error, p.minus)).or(note.map(|note| (note, p.muted))) else { return };
    let mut job = LayoutJob::single_section(theme.fonts.caption.text(text), theme.fonts.caption.format(colour));
    job.wrap = TextWrapping { max_width: width.max(1.0), ..TextWrapping::default() };
    let text = ui.painter().layout_job(job);
    let (rect, response) = ui.allocate_exact_size(vec2(width, text.size().y), Sense::hover());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, text.text()));
    if bring {
        response.scroll_to_me(None);
    }
    if ui.is_rect_visible(rect) {
        paint::rows_at(&super::solid(ui), rect.min, width, text, colour);
    }
}

/// `text` laid out in `font` and `colour` for a field: wrapped at `wrap` when it is `Some`, on one line otherwise,
/// trailing spaces kept; a password's characters each a dot.
fn layout(ui: &Ui, text: &str, font: Font, colour: Color32, wrap: Option<f32>, password: bool) -> Arc<Galley> {
    #[cfg(test)]
    LAID_OUT.with(|most| most.set(most.get().max(text.len())));
    let shown =
        if password { std::iter::repeat_n(egui::epaint::text::PASSWORD_REPLACEMENT_CHAR, text.chars().count()).collect() } else { text.to_owned() };
    let mut job = LayoutJob::single_section(shown, font.format(colour));
    job.keep_trailing_whitespace = true;
    match wrap {
        Some(width) => job.wrap.max_width = width.max(1.0),
        None => {
            job.wrap.max_width = f32::INFINITY;
            job.break_on_newline = false;
        }
    }
    ui.fonts_mut(|fonts| fonts.layout_job(job))
}

/// The scope a field's text is edited in: disabled when it cannot be used, never faded, and egui's own caret hidden
/// (the field paints the design's block).
fn edit_scope(ui: &mut Ui, enabled: bool) {
    if !enabled {
        ui.disable();
    }
    ui.set_opacity(1.0);
    let cursor = &mut ui.visuals_mut().text_cursor;
    cursor.stroke = Stroke::NONE;
    cursor.blink = false;
    cursor.preview = false;
}

/// Paints the caret of the edit in `output` (the field `id`), clipped to `clip`: a block 2 after a character before
/// it on its row, at the row's start otherwise, 3 down in a 22 row (centred); blinking unless `still`.
fn paint_caret(ui: &Ui, theme: &Theme, id: Id, output: &TextEditOutput, clip: Rect, still: bool) {
    let cursor = output.cursor_range.map_or_else(|| output.galley.end(), |range| range.primary);
    let at = output.galley.pos_from_cursor(cursor);
    let column = output.galley.layout_from_cursor(cursor).column;
    let size = caret_size(theme);
    // After text, 2 past the box the design gives the text before the caret: its width rounded up to a whole point.
    let (left, gap) = if column > egui::text::CharIndex(0) { (at.left().ceil(), theme.spacing.s1 / 2.0) } else { (at.left(), 0.0) };
    let min = output.galley_pos + vec2(left + gap, at.top() + (at.height() - size.y) / 2.0);
    let ppp = ui.ctx().pixels_per_point();
    let snap = |value: f32| (value * ppp).round() / ppp;
    let block = Rect::from_min_size(pos2(snap(min.x), snap(min.y)), size);
    // The blink starts again whenever the caret moves or the field takes the focus.
    let now = ui.input(|input| input.time);
    let key = id.with("caret-since");
    let index = cursor.index.0;
    let pass_now = ui.ctx().cumulative_pass_nr();
    let since = ui.data_mut(|data| {
        let since = match data.get_temp::<(f64, usize, u64)>(key) {
            Some((since, last, pass)) if last == index && pass.saturating_add(1) >= pass_now => since,
            _ => now,
        };
        data.insert_temp(key, (since, index, pass_now));
        since
    });
    if motion::blink(ui.ctx(), since, still) {
        ui.painter().with_clip_rect(clip).rect_filled(block, 0.0, theme.palette.cyan_edge);
    }
}

/// `text` on one line in `font` and `colour`, cut with one ellipsis at `width` as [`super::cut`] cuts it, laid out
/// as [`layout`] lays out a field's value (so it stands where the value stands).
fn cut_line(ui: &Ui, text: &str, font: Font, colour: Color32, width: f32) -> Arc<Galley> {
    let mut job = LayoutJob::single_section(text.to_owned(), font.format(colour));
    job.wrap = TextWrapping { max_width: width.max(1.0), max_rows: 1, break_anywhere: true, overflow_character: Some('…') };
    job.break_on_newline = false;
    paint::tight_cut(ui.painter(), ui.fonts_mut(|fonts| fonts.layout_job(job)))
}

/// Paints `hint` in `muted` at `at` (the text's place), clipped to `clip`; after the caret's block and the box's gap
/// when the field has the focus.
fn paint_hint(ui: &Ui, theme: &Theme, hint: &str, font: Font, at: egui::Pos2, clip: Rect, focus: bool) {
    if hint.is_empty() {
        return;
    }
    let shift = if focus { caret_size(theme).x + theme.spacing.s2 } else { 0.0 };
    let text = layout(ui, hint, font, theme.palette.muted, None, false);
    // Laid out and painted as egui paints the text typed in its place, at the place it gives that text: a person reads
    // the typed text there, so the hint stands on its baseline at every scale (not through `paint::run_at`).
    super::solid(ui).with_clip_rect(clip).galley(at + vec2(shift, 0.0), text, theme.palette.muted);
}

/// The label and screen-reader name of a field.
fn name<'a>(field: &Field<'a>) -> &'a str {
    if field.label.is_empty() { field.hint } else { field.label }
}

/// The one-line field of every kind.
#[allow(clippy::too_many_arguments)]
fn one_line(ui: &mut Ui, theme: &Theme, id: Id, text: &mut String, field: &Field<'_>, kind: Line, limit: usize, shown: Option<Shown>) -> Response {
    let p = &theme.palette;
    let width = if ui.available_width().is_finite() { ui.available_width().max(theme.boxes().field) } else { theme.boxes().field };
    let height = theme.boxes().field;
    let stored = kind == Line::Password(Secret::Stored);
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = theme.spacing.s2;
        label_row(ui, theme, field.label, width);
        let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
        let cap = Cap::Chars(limit.min(MAX_LINE));
        let password = matches!(kind, Line::Password(_));
        let guarded = guard(ui, id, text, cap);
        let said = guarded.said;
        let refusal = said.map(|_| cap.sentence());
        let bring = said == Some(Refusal::Fresh) && field.error.is_none();
        let field = &Field { error: field.error.or(refusal.as_deref()), ..*field };
        let seen = seen(ui, id, rect, field, shown);
        let (fill, edge, dashed) = box_look(theme, field, seen);
        let painter = super::solid(ui);
        if ui.is_rect_visible(rect) {
            paint_box(&painter, rect, fill, edge, dashed);
        }
        let font = if password || kind == Line::Mono { theme.fonts.mono } else { theme.fonts.body };
        // The mark at the right end: the lock of a read-only or stored field, the password mark.
        let disabled_password = password && !(field.enabled && ui.is_enabled() && !soon::is_soon(ui));
        let mark = match kind {
            // Disabled, a password field of either kind is the sheet's empty box with the password mark.
            Line::Password(_) if disabled_password => Some(PASSWORD),
            _ if field.read_only || stored => Some(LOCK),
            Line::Password(Secret::Typed) => Some(PASSWORD),
            Line::Text | Line::Mono => None,
            Line::Password(Secret::Stored) => Some(LOCK),
        };
        let pad = inset(theme);
        let side = theme.boxes().icon;
        let right = if mark.is_some() { pad + side + theme.spacing.s2 } else { pad };
        let colour = if seen.enabled { p.ink } else { p.muted };
        let vertical = ((height - font.line) / 2.0).max(0.0);
        let inner = Rect::from_min_max(pos2(rect.left() + pad, rect.top() + vertical), pos2(rect.right() - right, rect.bottom() - vertical));
        let (mut response, outcome) = ui
            .scope_builder(UiBuilder::new().max_rect(rect), |ui| {
                edit_scope(ui, seen.enabled);
                // A disabled password field is an empty box.
                let mut nothing = String::new();
                let blank = password && !seen.enabled;
                let mut buffer = capped(if blank { &mut nothing } else { &mut *text }, &guarded, !blank && !field.read_only, password);
                let empty = buffer.as_str().is_empty();
                // Without the focus, a value longer than the box shows cut with one ellipsis, as the design cuts a
                // line; egui keeps the whole value, laid out unseen, for the pointer and a screen reader.
                let cut = (!seen.focus && !password && !empty && layout(ui, buffer.as_str(), font, colour, None, false).size().x > inner.width())
                    .then(|| cut_line(ui, buffer.as_str(), font, colour, inner.width()));
                let laid = if cut.is_some() { Color32::TRANSPARENT } else { colour };
                let mut layouter = |ui: &Ui, text: &dyn TextBuffer, _: f32| layout(ui, text.as_str(), font, laid, None, password);
                let edit = TextEdit::singleline(&mut buffer)
                    .id(id)
                    .frame(egui::Frame::NONE.inner_margin(margin_of(rect, inner)))
                    .desired_width(f32::INFINITY)
                    .min_size(vec2(0.0, height))
                    .password(password)
                    .layouter(&mut layouter);
                let output = edit.show(ui);
                let outcome = buffer.finish();
                if password && let Some(mut state) = TextEditState::load(ui.ctx(), id) {
                    state.clear_undoer();
                    state.store(ui.ctx(), id);
                }
                let clip = inner.expand2(vec2(caret_size(theme).x + theme.spacing.s1, vertical));
                if let Some(cut) = cut {
                    // At the box's text start, where egui puts an unscrolled value, painted as egui paints it.
                    super::solid(ui).with_clip_rect(rect.shrink(1.0)).galley(pos2(inner.left(), output.galley_pos.y), cut, colour);
                }
                if empty && !disabled_password {
                    // A stored password's hint is words, in `body`, centred as words are.
                    let hint_font = if stored { theme.fonts.body } else { font };
                    // A typed hint where egui puts the typed text; a stored one's words in the middle.
                    let at = if stored { pos2(inner.left(), rect.top() + ((height - hint_font.line) / 2.0).max(0.0)) } else { output.galley_pos };
                    paint_hint(ui, theme, field.hint, hint_font, at, Rect::from_x_y_ranges(inner.x_range(), rect.shrink(1.0).y_range()), seen.focus);
                }
                if seen.focus && !field.read_only {
                    paint_caret(ui, theme, id, &output, clip.intersect(rect.shrink(1.0)), seen.forced);
                }
                (output.response.response, outcome)
            })
            .inner;
        hold(ui, id, text, guarded, outcome, &mut response);
        let spoken = name(field);
        ui.ctx().accesskit_node_builder(response.id, |node| node.set_label(spoken));
        if let Some(glyph) = mark {
            let at = Rect::from_min_size(pos2(rect.right() - pad - side, rect.center().y - side / 2.0), vec2(side, side));
            let ink = if !seen.enabled && password { p.dim } else { p.muted };
            crate::iconset::paint_glyph(&painter, at, glyph, ink);
        }
        let note = (stored && !disabled_password).then_some(tr!("The stored password is in your system's credential store and is never shown."));
        under(ui, theme, field.error, note, width, bring);
        response
    })
    .inner
}

/// A one-line edit without a box, filling the rect `ui` offers, its text in `body` centred in it: the text being typed
/// on a recipient line, after the chips. Its caret and its hint are the fields' (the hint 4 after the caret while it
/// has the focus); drawn in `shown` for a board.
pub(crate) fn bare_line(ui: &mut Ui, theme: &Theme, id: Id, text: &mut String, field: &Field<'_>, shown: Option<Shown>) -> Response {
    let p = &theme.palette;
    let rect = ui.max_rect();
    let seen = seen(ui, id, rect, field, shown);
    let font = theme.fonts.body;
    let colour = if seen.enabled { p.ink } else { p.muted };
    let vertical = ((rect.height() - font.line) / 2.0).max(0.0);
    let inner = Rect::from_min_max(pos2(rect.left(), rect.top() + vertical), pos2(rect.right(), rect.bottom() - vertical));
    edit_scope(ui, seen.enabled);
    let guarded = guard(ui, id, text, Cap::Chars(MAX_LINE));
    let empty = text.is_empty();
    let mut layouter = |ui: &Ui, text: &dyn TextBuffer, _: f32| layout(ui, text.as_str(), font, colour, None, false);
    let mut buffer = capped(text, &guarded, true, false);
    let output = TextEdit::singleline(&mut buffer)
        .id(id)
        .frame(egui::Frame::NONE.inner_margin(margin_of(rect, inner)))
        .desired_width(f32::INFINITY)
        .min_size(vec2(0.0, rect.height()))
        .layouter(&mut layouter)
        .show(ui);
    let outcome = buffer.finish();
    drop(buffer);
    let mut response = output.response.response.clone();
    hold(ui, id, text, guarded, outcome, &mut response);
    if empty && !field.hint.is_empty() {
        let shift = if seen.focus { caret_size(theme).x + theme.spacing.s1 } else { 0.0 };
        let hint = layout(ui, field.hint, font, p.muted, None, false);
        // Where egui puts the text typed in its place, painted as egui paints it ([`paint_hint`]).
        super::solid(ui).with_clip_rect(rect).galley(output.galley_pos + vec2(shift, 0.0), hint, p.muted);
    }
    if seen.focus {
        paint_caret(ui, theme, id, &output, rect, seen.forced);
    }
    response
}

/// The margin that puts an edit's text at `inner` inside `outer`.
fn margin_of(outer: Rect, inner: Rect) -> Margin {
    // A few dozen points at most; the clamp keeps the casts inside i8.
    let points = |value: f32| value.round().clamp(0.0, 127.0) as i8;
    Margin {
        left: points(inner.left() - outer.left()),
        right: points(outer.right() - inner.right()),
        top: points(inner.top() - outer.top()),
        bottom: points(outer.bottom() - inner.bottom()),
    }
}

/// A field of several lines filling the width `ui` offers: its label, a box that grows with its text from 96 to 240
/// high and then scrolls, its scroll bar 12 wide inside the right edge, and the error under it; otherwise as
/// [`text_field`]. Enter starts a new line. It holds at most [`MAX_TEXT`] bytes; what would pass them is refused whole,
/// and the line under it says so. A text already longer opens whole and can be edited down.
pub fn multiline_field(ui: &mut Ui, theme: &Theme, id: Id, text: &mut String, field: &Field<'_>) -> Response {
    multiline_inner(ui, theme, id, text, field, Cap::Bytes(MAX_TEXT), None)
}

/// [`multiline_field`] that holds at most `cap` (characters or bytes) instead of [`MAX_TEXT`] bytes: the cap of what
/// the text is kept as (a signature's characters), so that what would pass it is refused whole and said, never cut.
pub fn multiline_field_holding(ui: &mut Ui, theme: &Theme, id: Id, text: &mut String, field: &Field<'_>, cap: Cap) -> Response {
    multiline_inner(ui, theme, id, text, field, cap, None)
}

/// [`multiline_field`] drawn in `shown`: a board of the gallery.
pub(crate) fn multiline_with(ui: &mut Ui, theme: &Theme, id: Id, text: &mut String, field: &Field<'_>, shown: Option<Shown>) -> Response {
    multiline_inner(ui, theme, id, text, field, Cap::Bytes(MAX_TEXT), shown)
}

/// The field of several lines, holding at most `cap`, drawn in `shown`.
fn multiline_inner(ui: &mut Ui, theme: &Theme, id: Id, text: &mut String, field: &Field<'_>, cap: Cap, shown: Option<Shown>) -> Response {
    let p = &theme.palette;
    let s = &theme.spacing;
    let width = if ui.available_width().is_finite() { ui.available_width().max(theme.boxes().field * 2.0) } else { theme.boxes().field * 2.0 };
    let font = theme.fonts.body;
    let [least, most] = theme.boxes().multiline;
    let pad = inset(theme);
    let lock = field.read_only;
    let side = theme.boxes().icon;
    // The room right of the text: the pad, or the lock's column of a read-only field.
    let right = if lock { 1.0 + s.s3 + side + s.s2 } else { pad };
    let wrap = width - pad - right;
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = s.s2;
        label_row(ui, theme, field.label, width);
        let guarded = guard(ui, id, text, cap);
        let said = guarded.said;
        let refusal = said.map(|_| cap.sentence());
        let bring = said == Some(Refusal::Fresh) && field.error.is_none();
        let field = &Field { error: field.error.or(refusal.as_deref()), ..*field };
        let colour = if field.enabled && ui.is_enabled() && !soon::is_soon(ui) { p.ink } else { p.muted };
        // The text is laid out once per change: the measure here and the edit's own layout ask for the same job (same
        // colour and wrap; the wrap of a text that scrolled last frame leaves the bar's room), which the font cache
        // keeps. Every row is a line high, the last one too.
        let scrolled = ui.data(|data| data.get_temp::<bool>(id.with("scrolled"))).unwrap_or(false);
        let narrow = wrap - theme.sizes.scrollbar - s.s1 + s.s3;
        let rows_at = |ui: &Ui, wrap: f32| layout(ui, text, font, colour, Some(wrap), false).rows.len().max(1) as f32 * font.line;
        let mut rows = rows_at(ui, if scrolled { narrow } else { wrap });
        if scrolled && rows + s.s2 * 2.0 + 2.0 <= most {
            rows = rows_at(ui, wrap);
        }
        let whole = rows + s.s2 * 2.0 + 2.0;
        let scrolls = whole > most;
        ui.data_mut(|data| data.insert_temp(id.with("scrolled"), scrolls));
        let height = whole.clamp(least, most);
        let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
        let seen = seen(ui, id, rect, field, shown);
        let (fill, edge, dashed) = box_look(theme, field, seen);
        let painter = super::solid(ui);
        if ui.is_rect_visible(rect) {
            paint_box(&painter, rect, fill, edge, dashed);
        }
        let inside = rect.shrink(1.0);
        // While it scrolls, the bar takes 12 at the right and the text keeps 4 from it.
        let right_pad = if scrolls { s.s1 } else { right - 1.0 };
        let bar = if scrolls { theme.sizes.scrollbar } else { 0.0 };
        let view = Rect::from_min_max(pos2(rect.left() + pad, inside.top() + s.s2), pos2(inside.right() - bar - right_pad, inside.bottom() - s.s2));
        let margin = margin_of(Rect::from_min_max(inside.min, pos2(inside.right() - bar, inside.bottom())), view);
        // A child that leaves the cursor where the box put it.
        let mut child = ui.new_child(UiBuilder::new().max_rect(inside));
        let (mut response, outcome) = {
            let ui = &mut child;
            {
                edit_scope(ui, seen.enabled);
                let mut buffer = capped(&mut *text, &guarded, !field.read_only, false);
                let empty = buffer.as_str().is_empty();
                let mut layouter = |ui: &Ui, text: &dyn TextBuffer, wrap: f32| layout(ui, text.as_str(), font, colour, Some(wrap), false);
                let mut edit = |ui: &mut Ui| {
                    let output = TextEdit::multiline(&mut buffer)
                        .id(id)
                        .frame(egui::Frame::NONE.inner_margin(margin))
                        .desired_width(f32::INFINITY)
                        .desired_rows(1)
                        .min_size(vec2(0.0, if scrolls { 0.0 } else { inside.height() }))
                        .layouter(&mut layouter)
                        .show(ui);
                    if empty {
                        paint_hint(ui, theme, field.hint, font, output.galley_pos, view, seen.focus);
                    }
                    if seen.focus && !field.read_only {
                        let clip = ui.clip_rect().intersect(view.expand2(vec2(caret_size(theme).x + s.s1, 0.0)));
                        paint_caret(ui, theme, id, &output, clip, seen.forced);
                        // A board's forced focus shows the field as the keyboard leaves it: the caret (at the end) in
                        // sight, the text scrolled to it.
                        if scrolls && (seen.forced || output.response.response.changed() || output.response.response.gained_focus()) {
                            // Keep the caret clear of the pads above and below the text.
                            let cursor = output.cursor_range.map_or_else(|| output.galley.end(), |range| range.primary);
                            let at = output.galley.pos_from_cursor(cursor).translate(output.galley_pos.to_vec2());
                            // A board's picture is one frame: there the scroll is at once.
                            let motion = if seen.forced { egui::style::ScrollAnimation::none() } else { ui.style().scroll_animation };
                            ui.scroll_to_rect_animation(at.expand2(vec2(0.0, s.s2)), None, motion);
                        }
                    }
                    output.response.response
                };
                let response = if scrolls {
                    let area = ScrollArea::vertical().id_salt(id.with("scroll")).max_height(inside.height()).auto_shrink([false, false]);
                    // The thumb keeps 4 from the strip's ends, as the field's board draws it.
                    scroll::show_with_ends(ui, theme, area, s.s1, |ui| {
                        ui.set_clip_rect(ui.clip_rect().intersect(Rect::from_x_y_ranges(inside.x_range(), view.y_range())));
                        edit(ui)
                    })
                    .inner
                } else {
                    edit(ui)
                };
                (response, buffer.finish())
            }
        };
        hold(ui, id, text, guarded, outcome, &mut response);
        let spoken = name(field);
        ui.ctx().accesskit_node_builder(response.id, |node| node.set_label(spoken));
        if lock {
            let at = Rect::from_min_size(pos2(inside.right() - s.s3 - side, inside.top() + s.s2), vec2(side, side));
            crate::iconset::paint_glyph(&painter, at, LOCK, p.muted);
        }
        under(ui, theme, field.error, None, width, bring);
        response
    })
    .inner
}

/// A plain one-line edit without a box, filling the width `ui` offers: the composer's subject until the composer is
/// restyled. Its text is `body` in `ink`; `name` is its screen-reader name. It holds at most `cap` (the subject what
/// the engine keeps of one, 998 bytes); what would pass it is refused whole, and a line under it says so.
pub fn plain_line(ui: &mut Ui, theme: &Theme, id: Id, text: &mut String, name: &str, cap: Cap) -> Response {
    let guarded = guard(ui, id, text, cap);
    let said = guarded.said;
    let mut buffer = capped(text, &guarded, true, false);
    let edit = TextEdit::singleline(&mut buffer).id(id).font(theme.fonts.body.id()).frame(egui::Frame::NONE).desired_width(ui.available_width());
    let mut response = ui.add(edit);
    let outcome = buffer.finish();
    drop(buffer);
    hold(ui, id, text, guarded, outcome, &mut response);
    ui.ctx().accesskit_node_builder(response.id, |node| node.set_label(name));
    if said == Some(Refusal::Fresh) || (said.is_some() && !response.changed()) {
        let width = ui.available_width();
        under(ui, theme, Some(&cap.sentence()), None, width, said == Some(Refusal::Fresh));
    }
    response
}

/// A plain edit of several lines without a box, `rows` rows high at least and laid out by `layouter`: the composer's
/// message text until the composer is restyled. `name` is its screen-reader name. It holds at most [`MAX_BODY`]; what
/// would pass it is refused whole, and a line over the text says so, brought into view. A text already longer (a draft
/// from the store) opens whole and can be edited down.
pub fn plain_text(
    ui: &mut Ui,
    id: Id,
    text: &mut String,
    rows: usize,
    layouter: &mut dyn FnMut(&Ui, &dyn TextBuffer, f32) -> Arc<Galley>,
    name: &str,
    theme: &Theme,
) -> Response {
    let cap = Cap::Bytes(MAX_BODY);
    // Said over the text, where it shows however long the text is.
    let guarded = guard(ui, id, text, cap);
    let said = guarded.said;
    if said.is_some() {
        let width = ui.available_width();
        under(ui, theme, Some(&cap.sentence()), None, width, said == Some(Refusal::Fresh));
    }
    let mut buffer = capped(text, &guarded, true, false);
    let edit =
        TextEdit::multiline(&mut buffer).layouter(layouter).id(id).frame(egui::Frame::NONE).desired_width(ui.available_width()).desired_rows(rows);
    let mut response = ui.add(edit);
    let outcome = buffer.finish();
    drop(buffer);
    hold(ui, id, text, guarded, outcome, &mut response);
    ui.ctx().accesskit_node_builder(response.id, |node| node.set_label(name));
    response
}

#[cfg(test)]
thread_local! {
    /// The longest text, in bytes, handed to a field's layout on this thread: for the tests of [`guard`]'s bound.
    pub(crate) static LAID_OUT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
#[path = "field_tests.rs"]
pub(crate) mod tests;

#[cfg(test)]
#[path = "field_cap_tests.rs"]
mod cap_tests;
