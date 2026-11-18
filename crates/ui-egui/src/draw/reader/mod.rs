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
use crate::keymap::Keymap;
use crate::mailbox::{Act, After, Mailbox};
use crate::reader::Reader;
use crate::theme::Theme;
use crate::tr;

use body::bounced;
#[cfg(test)]
pub(super) use body::{blocks, column_widths};
pub(super) use head::{more_menu, move_menu, picker_id, picker_popup};
use head::{screening, sender_header, toolbar};
use thread::{folded, message};

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
    egui::Panel::top("reader-toolbar").show_separator_line(false).frame(Frame::new().inner_margin(margin(theme.spacing.s4))).show(
        ui,
        |ui| match held {
            Some(id) => sender_header(ui, theme, keymap, mailbox, reader, id, &mut drawn),
            None => toolbar(ui, theme, keymap, mailbox, reader, &mut drawn),
        },
    );
    crate::component::scroll::show(ui, theme, ScrollArea::vertical().auto_shrink([false, false]).id_salt("reader"), |ui| {
        ui.spacing_mut().item_spacing.y = theme.spacing.s2;
        Frame::new()
            .inner_margin(Margin {
                left: margin(theme.spacing.s4).left,
                right: margin(theme.spacing.s4).right,
                top: 0,
                bottom: margin(theme.spacing.s4).bottom,
            })
            .show(ui, |ui| {
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
                        _ => folded(ui, theme, mailbox, row, &mut drawn),
                    }
                }
            });
    });
    if let Some(text) = hover {
        status(ui, theme, whole, text);
    }
    drawn
}
