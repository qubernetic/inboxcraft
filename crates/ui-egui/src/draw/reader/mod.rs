// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The reader, as the design puts it from the top: the message's tags and its acts (Done, Snooze, Pin, Move, Delete
//! and More) on one line, the subject in `title` on two lines at most (the second cut with one ellipsis; a screen
//! reader hears it whole), the facts of a message alone or who wrote and how many messages for a thread, the why line
//! and the content bar. The thread follows, oldest first: the open message and the ones the owner unfolded in full
//! (the remote-content banner, the text, the attachments), the rest folded to their line. Reply, Reply all and Forward
//! stand at the foot under a hairline. More holds Archive (with its key), flag, read or unread and the rest of the acts
//! on the open message, then what is not a mail action: full width, closing the reader and the next-message rule. Move
//! to a folder opens a menu of the folders of the message's account. While the pointer is on a link, a float at the
//! bottom says where it goes. Tab goes from the head through the message to the answers.
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
//! A message the screener holds opens as a preview that decides nothing: its sender stands where the toolbar and the
//! subject are (avatar, name, address, first mail, messages waiting), then a card asks "Let <sender> in?", with the
//! groups to let them into, Accept, Accept and open, and Block; the message follows without Reply. A sent message a
//! delivery report says did not get through opens under a banner that says so, with the reason the report gave.
//!
//! Drawing only reads the [`Reader`]; what the person asks for comes back as [`Asked`] and is applied after the frame.

mod body;
mod head;
mod thread;

use std::collections::HashMap;

use egui::{Align2, Area, Frame, Id, Margin, Order, Rect, RichText, ScrollArea, Shape, TextureHandle, Ui, vec2};

use super::margin;
use crate::component::button;
use crate::compose::Answer;
use crate::draw::paint::{self, FloatEdge};
use crate::keymap::{self, Keymap};
use crate::mailbox::{Act, After, Mailbox};
use crate::reader::Reader;
use crate::theme::Theme;
use crate::tr;

use body::bounced;
#[cfg(test)]
pub(super) use body::{blocks, column_widths};
pub(super) use head::{more_menu, move_menu, picker_id, picker_popup};
use head::{screening, sender_header};
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
}

/// The textures of the pictures shown, by picture key.
pub(super) type Textures = HashMap<String, TextureHandle>;

/// Where the link under the pointer goes, on a float over the bottom left corner of `rect`. It takes no pointer, so
/// the link under it stays hovered.
fn status(ui: &Ui, theme: &Theme, rect: Rect, text: &str) {
    let p = &theme.palette;
    let s = &theme.spacing;
    let colour = if text.starts_with(tr!("Careful")) { p.minus } else { p.ink };
    Area::new(ui.scope_id().with("link-status"))
        .order(Order::Foreground)
        .interactable(false)
        .pivot(Align2::LEFT_BOTTOM)
        .fixed_pos(rect.left_bottom() + vec2(s.s3, -s.s3))
        .fade_in(false)
        .show(ui.ctx(), |ui| {
            let slot = ui.painter().add(Shape::Noop);
            let inner = Frame::new().inner_margin(margin(s.s2)).show(ui, |ui| {
                ui.set_max_width((rect.width() - s.s3 * 2.0 - s.s2 * 2.0).max(1.0));
                ui.add(egui::Label::new(RichText::new(text).font(theme.fonts.caption.id()).color(colour)).truncate());
            });
            let shapes = paint::float_shapes(theme, inner.response.rect, FloatEdge::Quiet);
            ui.painter().set(slot, Shape::Vec(shapes));
        });
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
    let mut drawn = Drawn::default();
    let p = &theme.palette;
    let whole = ui.max_rect();
    // A message opening wipes in from the top behind a scan line (M7): what is under the line is not shown yet.
    let (time, wipe) = reader.open_id().map_or((1.0, 1.0), |id| wiped(ui.ctx(), id));
    if wipe < 1.0 {
        let edge = whole.top() + (whole.height() * wipe).round();
        ui.set_clip_rect(Rect::from_min_max(whole.min, egui::pos2(whole.right(), edge)).intersect(ui.clip_rect()));
    }
    if let Some(notice) = reader.notice() {
        egui::Panel::bottom("reader-notice").show_separator_line(false).frame(Frame::new().inner_margin(margin(theme.spacing.s2))).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(notice).font(theme.fonts.caption.id()).color(p.ink));
                if button::link_sized(ui, theme, keymap, tr!("Dismiss"), button::LinkSize::Small).clicked() {
                    drawn.asked.push(Asked::Dismiss);
                }
            });
        });
    }
    let held = reader.open_id().filter(|id| mailbox.held(*id));
    let side = margin(SIDE).left;
    let head = match held {
        Some(_) => Frame::new().inner_margin(margin(theme.spacing.s4)),
        None => Frame::new().inner_margin(Margin { left: side, right: side, top: margin(HEAD_TOP).top, bottom: 0 }),
    };
    egui::Panel::top("reader-toolbar").show_separator_line(false).frame(head).show(ui, |ui| match held {
        Some(id) => sender_header(ui, theme, keymap, mailbox, reader, id, &mut drawn),
        None => {
            ui.spacing_mut().item_spacing.y = HEAD_GAP;
            head::head(ui, theme, keymap, mailbox, reader, &mut drawn);
        }
    });
    // The answers at the foot, under a `panel_line`, for the open message (not for a preview of held mail). Their room
    // is kept first and they are drawn after the message, so Tab goes from the head through the message to them.
    let alone = reader.open_id().and_then(|id| reader.opened(id)).is_some_and(|opened| thread::card_alone(mailbox, reader, opened));
    let foot_margin = Margin { left: side, right: side, top: margin(FOOT_PAD + 1.0).top, bottom: margin(FOOT_PAD).bottom };
    let room = ui.available_rect_before_wrap();
    let foot = match (reader.open_id(), held, alone) {
        (Some(id), None, false) => {
            let high = (crate::component::control_side(theme) + f32::from(foot_margin.top) + f32::from(foot_margin.bottom)).min(room.height());
            Some((id, Rect::from_min_max(egui::pos2(room.left(), room.bottom() - high), room.max)))
        }
        _ => None,
    };
    let message_room = foot.map_or(room, |(_, foot)| Rect::from_min_max(room.min, egui::pos2(room.right(), foot.top())));
    // A message alone in its thread stands 16 under the head; a thread's lines 4 under it.
    let top = if reader.thread().len() > 1 { THREAD_TOP } else { MESSAGE_TOP };
    ui.scope_builder(egui::UiBuilder::new().max_rect(message_room), |ui| {
        crate::component::scroll::show(ui, theme, ScrollArea::vertical().auto_shrink([false, false]).id_salt("reader"), |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            Frame::new().inner_margin(Margin { left: side, right: side, top: margin(top).top, bottom: margin(MESSAGE_FOOT).bottom }).show(ui, |ui| {
                ui.set_width(ui.available_width());
                if let Some(id) = held {
                    screening(ui, theme, keymap, mailbox, id, &mut drawn);
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
    if let Some((id, rect)) = foot {
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::top_down(egui::Align::Min)));
        Frame::new().inner_margin(foot_margin).show(&mut child, |ui| answers(ui, theme, keymap, id, &mut drawn));
        paint::hairline(ui.painter(), rect.left_top(), egui::pos2(rect.right() - 1.0, rect.top()), p.panel_line);
    }
    if let Some(text) = hover {
        status(ui, theme, whole, text);
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
