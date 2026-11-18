// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The reader, as the design puts it from the top: the message's tags and its acts (Done, Snooze, Pin, Move, Delete
//! and More) on one line, the subject in `title` on two lines at most (the second cut with one ellipsis; a screen
//! reader hears it whole), the facts of a message alone or who wrote and how many messages for a thread, the why line
//! and the content bar. The thread follows, oldest first: the open message and the ones the owner unfolded in full
//! (the remote-content banner, the text, the attachments), the rest folded to their line. Reply, Reply all and Forward
//! stand at the foot under a hairline, on lines of their own in a pane narrower than they are. More holds Archive
//! (with its key), flag, read or unread and the rest of the acts on the open message, then what is not a mail action:
//! full width, closing the reader and the next-message rule. Move to a folder opens a menu of the folders of the
//! message's account. While the pointer is on a link, a bar over the answers says where it goes (CAREFUL when its
//! text names another place). Tab goes from the head through the message to the answers.
//!
//! Done, Delete, Move and the acts in More act on the open message as the hover actions of the list do, on a real
//! account as on the sample; Reply, Reply all and Forward open the composer over a draft the engine filled in. More
//! also puts the sender's mail in another group, for a message of a grouped account. With no message open (after the
//! open one left the list) the pane shows the bare empty state and the list selects no row: nothing opens by itself.
//!
//! Snooze opens the time picker ("Snooze until"); on a message the owner sent the same button is "Remind me if no
//! reply" and its picker sets the follow-up reminder. Pin pins the message or takes its pin off. More parks it or
//! takes it off the park pile, brings a snoozed message back now and takes a reminder off. Tags on the head's first
//! line say until when it is snoozed, when its reminder is due, and that it came back from a snooze.
//!
//! A message of a longer thread is a thread row: folded, or open with the address and to whom, and `details ▾` that
//! opens its facts. The open message of a thread of one has no row: its facts (FROM, TO, DATE) stand under the subject,
//! and `details ▾` (also in More) opens the full header with every address.
//!
//! A message with an invitation shows its card over the text (see `super::invite`): the event, what changed, and
//! Going, Maybe and No, or the answer given with a way to change it. When the text only says what the card says, the
//! card alone stands for the message, without the text or Reply.
//!
//! A message the screener holds opens as a preview that decides nothing: its tags (New sender and the account) and
//! More, then a card asks "Let <sender> in?" with their address, first mail and messages waiting, the groups to let
//! them into, Accept, Accept and reply, and Block (see `held`); the subject, the facts and the message follow, without
//! Reply. A sent message a
//! delivery report says did not get through opens under a banner that says so, with the reason the report gave.
//!
//! Drawing only reads the [`Reader`]; what the person asks for comes back as [`Asked`] and is applied after the frame.

mod body;
mod head;
mod held;
mod thread;

use std::collections::HashMap;

use egui::{Area, Frame, Id, Margin, Order, Rect, ScrollArea, Stroke, TextureHandle, Ui, vec2};

use super::margin;
use crate::compose::Answer;
use crate::draw::paint;
use crate::keymap::{self, Keymap};
use crate::mailbox::{Act, After, Mailbox};
use crate::reader::Reader;
use crate::theme::Theme;
use crate::tr;

use body::bounced;
#[cfg(test)]
pub(super) use body::{blocks, column_widths};
pub(super) use head::{more_menu, move_menu, picker_id, picker_popup};
use thread::{answers, folded, message};

/// The room left and right of the reader's parts, in points.
const SIDE: f32 = 40.0;

/// The room above the head, in points.
const HEAD_TOP: f32 = 16.0;

/// The room between the parts of the head, in points.
const HEAD_GAP: f32 = 12.0;

/// The room above and below the answers at the foot, in points.
const FOOT_PAD: f32 = 12.0;

/// The room above a message alone, above a thread's lines, and under either, in points.
const MESSAGE_TOP: f32 = 16.0;
const THREAD_TOP: f32 = 4.0;
const MESSAGE_FOOT: f32 = 24.0;

/// What the person asked for in the reader this frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Asked {
    /// Unfold or fold a message of the thread.
    Expand(u64),
    /// Show or hide the full header.
    Details(u64),
    /// Show or hide the quotes.
    Quotes(u64),
    /// Load the remote pictures of this message once.
    Once(u64),
    /// Load the remote pictures of this sender always.
    Always(u64),
    /// Try loading a picture of a message again, by its key.
    Retry(u64, String),
    /// Save an attachment.
    Save(u64, u64),
    /// Fill the window, or not.
    FullWidth(bool),
    /// Close the reader.
    Close,
    /// The next-message rule.
    After(After),
    /// Copy a link's address.
    Copy(String),
    /// Copy the subject, whole.
    CopySubject(String),
    /// Hide the notice.
    Dismiss,
    /// Do `Act` to a message.
    Act(u64, Act),
    /// Write a draft made from a message.
    Answer(u64, Answer),
    /// A time picker opened: read the time presets still ahead.
    Presets,
    /// Something asked on the invitation card of a message.
    Invite(u64, super::invite::Ask),
}

/// What the reader drew: what was asked for, and the status of the link under the pointer.
#[derive(Debug, Default)]
pub(super) struct Drawn {
    pub(super) asked: Vec<Asked>,
    pub(super) hover: Option<String>,
    /// The message whose invitation card holds the pane's one primary button (see `super::invite::primary_card`),
    /// decided once a frame before anything is drawn.
    pub(super) primary_card: Option<u64>,
}

/// The textures of the pictures shown, by picture key.
pub(super) type Textures = HashMap<String, TextureHandle>;

/// The room left and right of the link status's parts, and above and below a careful one, in points.
const STATUS_SIDE: f32 = 40.0;
const STATUS_PAD: f32 = 10.0;

/// The room inside a careful link's box, above and below and at the sides, and between its parts, in points.
const CAREFUL_Y: f32 = 8.0;
const CAREFUL_X: f32 = 12.0;

/// A text of the link status: laid out in `font` and `ink` to `room`, at `at`, heard as `words`.
fn status_text(ui: &Ui, text: &str, font: crate::theme::Font, ink: egui::Color32, room: f32) -> std::sync::Arc<egui::Galley> {
    crate::component::fitted(ui, text, font, ink, room.max(1.0))
}

/// Tells a screen reader `words` stand in `rect`.
fn heard(ui: &Ui, rect: Rect, words: &str, salt: &str) {
    let response = ui.interact(rect, ui.scope_id().with(("link-status", salt)), egui::Sense::hover());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, crate::component::spoken(words)));
}

/// Where the link under the pointer goes, on a bar at the foot of the message's `room`, over the answers: `LINK`, the
/// address and what a click does on the `panel` fill under a `panel_line`; a link whose text names another place than
/// it goes to stands in a careful box instead (CAREFUL in `minus`, what the text says, and the address whole on a line
/// of its own). It takes no pointer, so the link under it stays hovered.
fn status(ui: &Ui, theme: &Theme, room: Rect, text: &str) {
    let p = &theme.palette;
    let f = &theme.fonts;
    // The careful status says where the link goes after its last ": ".
    let careful = text.strip_prefix(tr!("Careful: ")).and_then(|said| said.rsplit_once(": "));
    let copies = !text.starts_with(tr!("This link is not followed"));
    let note = tr!("a click copies it");
    let high = match careful {
        Some(_) => STATUS_PAD + CAREFUL_Y + f.caption.line + 2.0 + f.mono.line + CAREFUL_Y + STATUS_PAD,
        None => theme.boxes().strip.max(28.0),
    }
    .ceil();
    let rect = Rect::from_min_max(egui::pos2(room.left(), room.bottom() - high), room.max);
    Area::new(ui.scope_id().with("link-status")).order(Order::Foreground).interactable(false).fixed_pos(rect.min).fade_in(false).show(
        ui.ctx(),
        |ui| {
            ui.set_clip_rect(rect);
            let painter = ui.painter();
            let inner = rect.shrink2(vec2(STATUS_SIDE, 0.0));
            let note_galley = copies.then(|| status_text(ui, note, f.mono_s, p.muted, inner.width()));
            let note_width = note_galley.as_ref().map_or(0.0, |note| paint::run_box(note.size().x));
            match careful {
                Some((said, target)) => {
                    painter.rect_filled(rect, 0.0, p.surface);
                    let boxed =
                        Rect::from_min_max(egui::pos2(inner.left(), rect.top() + STATUS_PAD), egui::pos2(inner.right(), rect.bottom() - STATUS_PAD));
                    painter.rect(boxed, 0.0, p.panel, Stroke::new(1.0, p.minus), egui::StrokeKind::Inside);
                    let label = crate::component::galley(ui, tr!("Careful"), f.label, p.minus);
                    let label_width = paint::run_box(label.size().x);
                    let top = boxed.top() + CAREFUL_Y;
                    paint::run_at(
                        painter,
                        egui::pos2(boxed.left() + CAREFUL_X, (top + (f.caption.line - label.size().y) / 2.0).round()),
                        label,
                        p.minus,
                    );
                    let left = boxed.left() + CAREFUL_X + label_width + CAREFUL_X;
                    let right = boxed.right() - CAREFUL_X - if copies { note_width + CAREFUL_X } else { 0.0 };
                    let words = format!("{}:", crate::component::spoken(said));
                    // The words as the design writes them: "The text says …", the first letter up.
                    let mut chars = words.chars();
                    let words: String = chars.next().map(|first| first.to_uppercase().chain(chars).collect()).unwrap_or_default();
                    let said_galley = status_text(ui, &words, f.caption, p.ink, right - left);
                    paint::run_at(painter, egui::pos2(left, top), said_galley, p.ink);
                    let target_top = top + f.caption.line + 2.0;
                    let target_galley = status_text(ui, target, f.mono, p.ink, right - left);
                    paint::run_at(painter, egui::pos2(left, target_top), target_galley, p.ink);
                    heard(ui, boxed, &format!("{}: {words}", tr!("Careful")), "careful");
                    heard(ui, Rect::from_min_size(egui::pos2(left, target_top), vec2((right - left).max(1.0), f.mono.line)), target, "target");
                    if let Some(note_galley) = note_galley {
                        let at = egui::pos2(boxed.right() - CAREFUL_X - note_width, (top + (f.caption.line - note_galley.size().y) / 2.0).round());
                        heard(ui, Rect::from_min_size(at, vec2(note_width, f.mono_s.line)), note, "note");
                        paint::run_at(painter, at, note_galley, p.muted);
                    }
                }
                None => {
                    painter.rect_filled(rect, 0.0, p.panel);
                    let label = crate::component::galley(ui, tr!("Link"), f.label, p.muted);
                    let label_width = paint::run_box(label.size().x);
                    let middle = rect.center().y;
                    paint::run_at(painter, egui::pos2(inner.left(), (middle - label.size().y / 2.0).round()), label, p.muted);
                    let left = inner.left() + label_width + CAREFUL_X;
                    let right = inner.right() - if copies { note_width + CAREFUL_X } else { 0.0 };
                    let target = status_text(ui, text, f.mono_s, p.ink, right - left);
                    paint::run_at(painter, egui::pos2(left, (middle - target.size().y / 2.0).round()), target, p.ink);
                    heard(ui, Rect::from_min_max(egui::pos2(left, rect.top()), egui::pos2(right.max(left + 1.0), rect.bottom())), text, "target");
                    if let Some(note_galley) = note_galley {
                        let at = egui::pos2(inner.right() - note_width, (middle - note_galley.size().y / 2.0).round());
                        heard(ui, Rect::from_min_size(at, vec2(note_width, f.mono_s.line)), note, "note");
                        paint::run_at(painter, at, note_galley, p.muted);
                    }
                }
            }
            paint::hairline(painter, rect.left_top(), egui::pos2(rect.right() - 1.0, rect.top()), p.panel_line);
        },
    );
}

/// The reader with no message open: the bare empty state in the middle of the pane, 40 from its sides.
pub(super) fn nothing_open(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    let palette = keymap.shortcut(keymap::PALETTE).map(|shortcut| crate::component::key::key_words(&shortcut));
    let help = match palette {
        Some(key) => tr!("The arrow keys move in the list and open a message. {key} runs any command.", key = key),
        None => tr!("The arrow keys move in the list and open a message.").to_owned(),
    };
    let side = margin(SIDE).left;
    Frame::new().inner_margin(Margin { left: side, right: side, top: 0, bottom: 0 }).show(ui, |ui| {
        ui.set_min_size(ui.available_size());
        crate::component::empty::empty_state(ui, theme, tr!("Reader · nothing open"), tr!("Pick a message.\nIt opens here."), &help);
    });
}

/// Draws the reader over the whole of `ui` and returns what was asked for.
pub(super) fn show(
    ui: &mut Ui,
    theme: &Theme,
    keymap: &Keymap,
    mailbox: &Mailbox,
    reader: &Reader,
    textures: &mut Textures,
    hover: Option<&str>,
) -> Drawn {
    let mut drawn = Drawn { primary_card: super::invite::primary_card(mailbox, reader), ..Drawn::default() };
    let p = &theme.palette;
    let whole = ui.max_rect();
    let unwiped = ui.clip_rect();
    // A message opening wipes in from the top behind a scan line (M7): what is under the line is not shown yet.
    let (time, wipe) = reader.open_id().map_or((1.0, 1.0), |id| wiped(ui.ctx(), id));
    if wipe < 1.0 {
        let edge = whole.top() + (whole.height() * wipe).round();
        ui.set_clip_rect(Rect::from_min_max(whole.min, egui::pos2(whole.right(), edge)).intersect(ui.clip_rect()));
    }
    let held = reader.open_id().filter(|id| mailbox.held(*id));
    let side = margin(SIDE).left;
    let head = Frame::new().inner_margin(Margin { left: side, right: side, top: margin(HEAD_TOP).top, bottom: 0 });
    egui::Panel::top("reader-toolbar").show_separator_line(false).frame(head).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = HEAD_GAP;
        head::head(ui, theme, keymap, mailbox, reader, &mut drawn);
    });
    // The answers at the foot, under a `panel_line`, for the open message (not for a preview of held mail). Their room
    // is kept first and they are drawn after the message, so Tab goes from the head through the message to them.
    let alone = reader.open_id().and_then(|id| reader.opened(id)).is_some_and(|opened| thread::card_alone(mailbox, reader, opened));
    let foot_margin = Margin { left: side, right: side, top: margin(FOOT_PAD + 1.0).top, bottom: margin(FOOT_PAD).bottom };
    let room = ui.available_rect_before_wrap();
    let foot = match (reader.open_id(), held, alone) {
        (Some(id), None, false) => {
            // The answers stand on one line, or on more where the pane is narrower than they are.
            let inner = room.width() - f32::from(foot_margin.left) - f32::from(foot_margin.right);
            // An unfolded invitation card that asks has the pane's one primary button, Going.
            let asks = drawn.primary_card.is_some();
            let placed = thread::answer_lines(ui, theme, keymap, inner, asks);
            let high = (placed.height + f32::from(foot_margin.top) + f32::from(foot_margin.bottom)).min(room.height());
            Some((id, placed, Rect::from_min_max(egui::pos2(room.left(), room.bottom() - high), room.max)))
        }
        _ => None,
    };
    let message_room = foot.map_or(room, |(_, _, foot)| Rect::from_min_max(room.min, egui::pos2(room.right(), foot.top())));
    // Warnings show whole while the message wipes in: they are drawn clipped to the message's room only.
    body::keep_unwiped(ui.ctx(), message_room.intersect(unwiped));
    // A message alone in its thread stands 16 under the head; a thread's lines 4 under it.
    let top = if reader.thread().len() > 1 { THREAD_TOP } else { MESSAGE_TOP };
    ui.scope_builder(egui::UiBuilder::new().max_rect(message_room), |ui| {
        crate::component::scroll::show(ui, theme, ScrollArea::vertical().auto_shrink([false, false]).id_salt("reader"), |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            Frame::new().inner_margin(Margin { left: side, right: side, top: margin(top).top, bottom: margin(MESSAGE_FOOT).bottom }).show(ui, |ui| {
                ui.set_width(ui.available_width());
                // The reader's notice: what was done, or what failed, at the top of the message, after the head.
                if let Some(notice) = reader.notice() {
                    body::notice(ui, theme, keymap, notice, reader.notice_failed(), &mut drawn);
                    ui.add_space(theme.spacing.s4);
                }
                if let Some(reason) =
                    reader.open_id().and_then(|id| mailbox.messages().iter().find(|row| row.id == id)).and_then(|row| row.bounced.as_deref())
                {
                    bounced(ui, theme, reason);
                }
                for row in reader.thread() {
                    match (&row.opened, reader.is_expanded(row.id)) {
                        (Some(opened), true) => message(ui, theme, keymap, mailbox, reader, opened, textures, &mut drawn),
                        _ => folded(ui, theme, keymap, mailbox, row, &mut drawn),
                    }
                }
            });
        });
    });
    if let Some((id, placed, rect)) = foot {
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::top_down(egui::Align::Min)));
        Frame::new().inner_margin(foot_margin).show(&mut child, |ui| answers(ui, theme, keymap, id, placed, &mut drawn));
        paint::hairline(ui.painter(), rect.left_top(), egui::pos2(rect.right() - 1.0, rect.top()), p.panel_line);
    }
    if let Some(text) = hover {
        status(ui, theme, message_room, text);
    }
    let shows = scan(time);
    if shows > 0.0 {
        // The scan line at the wipe's edge, over everything in the pane: whole until nine tenths of the time, then
        // fading out over the last tenth, as the presentation's keyframes have it.
        let edge = whole.top() + (whole.height() * wipe).round();
        let painter = ui.ctx().layer_painter(egui::LayerId::new(Order::Foreground, Id::unique("reader-scan"))).with_clip_rect(whole);
        // Faded as the design fades a colour: mixed toward the pane's `surface`, never by alpha.
        let ink = p.surface.lerp_to_gamma(p.cyan, shows);
        paint::hairline(&painter, egui::pos2(whole.left(), edge), egui::pos2(whole.right() - 1.0, edge), ink);
    }
    drawn
}

/// How long a message takes to wipe in when it opens (M7, the `medium` token), in seconds.
const WIPE: f64 = 0.3;

/// How far into the wipe's time the scan line starts to fade, as the presentation fades it over the last tenth.
const SCAN_GONE: f32 = 0.9;

/// How far into its time the wipe of the open message `open` is, and how far it has wiped in on the design's curve,
/// each 0 to 1; a repaint is asked for until it has. With reduced motion it is there at once, and a key or a press
/// during the wipe takes it to its end.
pub(super) fn wiped(ctx: &egui::Context, open: u64) -> (f32, f32) {
    let key = Id::unique("reader-wipe");
    let now = ctx.input(|input| input.time);
    let acted = ctx.input(|input| {
        input.events.iter().any(|event| matches!(event, egui::Event::Key { pressed: true, .. } | egui::Event::PointerButton { pressed: true, .. }))
    });
    let start = ctx.data_mut(|data| match data.get_temp::<(u64, f64)>(key) {
        Some((id, start)) if id == open && !acted => start,
        Some((id, _)) if id == open => {
            data.insert_temp(key, (open, now - WIPE));
            now - WIPE
        }
        _ => {
            data.insert_temp(key, (open, now));
            now
        }
    });
    if crate::motion::reduced(ctx) {
        return (1.0, 1.0);
    }
    // A share of a fraction of a second: the cast loses nothing that shows.
    let shown = ((now - start) / WIPE).clamp(0.0, 1.0) as f32;
    if shown < 1.0 {
        ctx.request_repaint();
    }
    (shown, ease(shown))
}

/// How much of the scan line shows `t` of the wipe's time in (0 to 1).
pub(super) fn scan(t: f32) -> f32 {
    if !t.is_finite() || t >= 1.0 {
        return 0.0;
    }
    if t < SCAN_GONE {
        return 1.0;
    }
    // The last tenth of the time fades it out, on the design's curve over that tenth.
    1.0 - ease((t - SCAN_GONE) / (1.0 - SCAN_GONE))
}

/// The design's one curve, `cubic-bezier(.2, .7, .2, 1)`, at `t` of the way (0 to 1): a quick start and a soft landing.
/// The bezier's x is found by halving, a fixed number of steps.
pub(super) fn ease(t: f32) -> f32 {
    if !t.is_finite() || t >= 1.0 {
        return 1.0;
    }
    if t <= 0.0 {
        return 0.0;
    }
    let bezier = |a: f32, b: f32, s: f32| {
        let r = 1.0 - s;
        3.0 * r * r * s * a + 3.0 * r * s * s * b + s * s * s
    };
    let (mut low, mut high) = (0.0_f32, 1.0_f32);
    for _ in 0..24 {
        let middle = (low + high) / 2.0;
        if bezier(0.2, 0.2, middle) < t {
            low = middle;
        } else {
            high = middle;
        }
    }
    bezier(0.7, 1.0, (low + high) / 2.0)
}
