// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The reader: a toolbar of mail actions (Done, Archive, Delete, Snooze, Pin, Move and More) over the subject, the
//! largest text of the window, which wraps rather than being cut, and who wrote in the thread. The thread follows as
//! cards, oldest first: the open message and the ones the owner unfolded in full (each message's header, the
//! remote-content banner, the body and the attachment tiles; Reply, Reply all and Forward on the open one), the rest
//! folded to one line. More holds flag and read or unread for the open message, then what is not a mail action: full
//! width, closing the reader and the next-message rule. Move to a folder opens a menu of the folders of the message's
//! account. While the pointer is on a link, a float at the bottom says where it goes.
//!
//! Done, Archive, Delete, Move and the acts in More act on the open message as the hover actions of the list do, on a
//! real account as on the sample; Reply, Reply all and Forward open the composer over a draft the engine filled in.
//! More also puts the sender's mail in another group, for a message of a grouped account.
//!
//! Snooze opens the time picker ("Snooze until"); on a message the owner sent the same button is "Remind me if no
//! reply" and its picker sets the follow-up reminder. Pin pins the message or takes its pin off. More parks it or
//! takes it off the park pile, brings a snoozed message back now and takes a reminder off. Under the subject, chips
//! say until when it is snoozed, when its reminder is due, and that it came back from a snooze.
//!
//! A message of a longer thread has a header: the avatar, the name, "to ..." under it and the date at the right. The
//! open message of a thread of one has none: who wrote, their address and the date stand under the subject. Details
//! fades in beside them when the pointer comes near (it is there for the keyboard and a screen reader all the time,
//! and in More), and opens the full header with every address.
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

use egui::{Align2, Area, Frame, Margin, Order, Rect, RichText, ScrollArea, Shape, TextureHandle, Ui, vec2};

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
    // The answers at the foot, under a `panel_line`, for the open message (not for a preview of held mail).
    let alone = reader.open_id().and_then(|id| reader.opened(id)).is_some_and(|opened| thread::card_alone(mailbox, reader, opened));
    if let (Some(id), None, false) = (reader.open_id(), held, alone) {
        let foot = Frame::new().inner_margin(Margin { left: side, right: side, top: margin(FOOT_PAD + 1.0).top, bottom: margin(FOOT_PAD).bottom });
        let shown =
            egui::Panel::bottom("reader-answers").show_separator_line(false).frame(foot).show(ui, |ui| answers(ui, theme, keymap, id, &mut drawn));
        let rect = shown.response.rect;
        paint::hairline(ui.painter(), rect.left_top(), egui::pos2(rect.right() - 1.0, rect.top()), p.panel_line);
    }
    // A message alone in its thread stands 16 under the head; a thread's lines 4 under it.
    let top = if reader.thread().len() > 1 { THREAD_TOP } else { MESSAGE_TOP };
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
    if let Some(text) = hover {
        status(ui, theme, whole, text);
    }
    drawn
}
