// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The head of a message the screener holds: between the tags and the subject, a card on the `panel` fill in a 1 px
//! `cyan_edge`, 14 above and below and 16 at the sides, 12 between its parts, asks "Let <sender> in?" (in `heading`,
//! over their address, when their first mail came and how many wait, in `mono_s` and `muted`), offers the groups to
//! let them into (`Into` and a pill each, 6 apart, then where their mail goes), Accept (or Accept into the group
//! chosen, the card's one primary button), Accept and reply, and Block (on lines of their own where the card is
//! narrower than they are), and says what Block does. Under it a label
//! says the message is only a preview. The sender's name and address are data: plain words on one line, cut between
//! grapheme clusters with one ellipsis, heard whole.

use egui::{Rect, Role, Sense, Stroke, StrokeKind, Ui, WidgetInfo, vec2};

use crate::component::button::TextButton;
use crate::component::button_row::{button_lines, show_lines};
use crate::component::pill::pill;
use crate::compose::Answer;
use crate::draw::paint;
use crate::keymap::Keymap;
use crate::mailbox::{Act, GroupKind, Mailbox};
use crate::theme::Theme;
use crate::tr;

use super::{Asked, Drawn};

/// The room above and below the card's parts, and at its sides, in points.
const PAD_Y: f32 = 14.0;
const PAD_X: f32 = 16.0;

/// The room between the card's parts, in points.
const GAP: f32 = 12.0;

/// The room between the question and the line under it, in points.
const UNDER: f32 = 2.0;

/// The room between `Into` and the pills, between two pills, and above where the mail goes, in points.
const PILL_GAP: f32 = 6.0;

/// The room between two choices, in points.
const CHOICE_GAP: f32 = 8.0;

/// A line of `text` in `font` and `ink`, cut to `room`, at `at`; heard whole.
fn line(ui: &mut Ui, text: &str, font: crate::theme::Font, ink: egui::Color32, room: f32) {
    let galley = crate::component::fitted(ui, text, font, ink, room);
    let (rect, response) = ui.allocate_exact_size(vec2(room.max(1.0), font.line), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, crate::component::spoken(text)));
    paint::run_at(ui.painter(), rect.min, galley, ink);
}

/// The card that asks whether to let the sender of held message `id` in, and the preview's label under it.
pub(super) fn let_in(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, id: u64, drawn: &mut Drawn) {
    let p = &theme.palette;
    let f = &theme.fonts;
    let Some(row) = mailbox.messages().iter().find(|row| row.id == id) else { return };
    let theirs = || mailbox.messages().iter().filter(|other| other.address == row.address);
    let first = theirs().filter_map(|other| other.date).min();
    let waiting = mailbox.waiting().iter().find(|waiting| waiting.address == row.address).map_or(0, |waiting| waiting.messages.len());
    let count = waiting.max(theirs().count());
    let mut parts = vec![crate::bidi::address(&row.address).into_owned()];
    if let Some(first) = first {
        parts.push(tr!("first mail {when}", when = mailbox.short_date(first)));
    }
    parts.push(if count == 1 { tr!("1 message waiting").to_owned() } else { tr!("{count} messages waiting", count = count) });
    let chosen_id = egui::Id::unique("screen-into").with(id);
    let mut into = ui.data(|data| data.get_temp::<Option<GroupKind>>(chosen_id)).flatten();
    let slot = ui.painter().add(egui::Shape::Noop);
    let inner = egui::Frame::new().inner_margin(crate::component::margin(PAD_X, PAD_Y)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = GAP;
        let room = ui.available_width();
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.y = UNDER;
            line(ui, &tr!("Let {sender} in?", sender = row.sender), f.heading, p.ink, room);
            line(ui, &parts.join("  ·  "), f.mono_s, p.muted, room);
        });
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.y = PILL_GAP;
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(PILL_GAP, PILL_GAP);
                crate::component::label(ui, f.caption, tr!("Into"), p.muted);
                ui.add_space(2.0);
                for kind in GroupKind::SHOWN {
                    if pill(ui, theme, kind.text(), into == Some(kind)).clicked() {
                        into = if into == Some(kind) { None } else { Some(kind) };
                        ui.data_mut(|data| data.insert_temp(chosen_id, into));
                    }
                }
            });
            let goes = match into {
                Some(kind) => tr!("Their mail goes to {group}.", group = kind.text()),
                None => tr!("No group chosen: InboxCraft sorts their mail itself.").to_owned(),
            };
            line(ui, &goes, f.caption, p.muted, room);
        });
        // The choices, measured once: on one line where they fit, else on lines of their own, in order.
        let accept = match into {
            Some(kind) => tr!("Accept into {group}", group = kind.text()),
            None => tr!("Accept").to_owned(),
        };
        // Each at most the card's inner width: a label wider than the card is cut inside its button, never past the card.
        let choices = || {
            vec![
                TextButton::new(&accept).primary().within(room),
                TextButton::new(tr!("Accept and reply")).secondary().within(room),
                TextButton::new(tr!("Block")).danger().within(room),
            ]
        };
        let placed = button_lines(ui, theme, keymap, &choices(), room, CHOICE_GAP);
        let (_, place) = ui.allocate_space(vec2(room.max(1.0), placed.height));
        match show_lines(ui, theme, keymap, choices(), &placed, place, CHOICE_GAP) {
            Some(0) => drawn.asked.push(Asked::Act(id, Act::Accept { group: into, open: false })),
            Some(1) => {
                // Let in and opened, then answered: the reply is the owner's to write and send.
                drawn.asked.push(Asked::Act(id, Act::Accept { group: into, open: true }));
                drawn.asked.push(Asked::Answer(id, Answer::Reply));
            }
            Some(_) => drawn.asked.push(Asked::Act(id, Act::Block)),
            None => {}
        }
        let note = tr!("Blocked mail never loads remote content and never notifies.");
        ui.add(egui::Label::new(f.caption.rich(note).color(p.muted)).wrap());
    });
    let rect: Rect = inner.response.rect;
    let shapes =
        vec![egui::Shape::rect_filled(rect, 0.0, p.panel), egui::Shape::rect_stroke(rect, 0.0, Stroke::new(1.0, p.cyan_edge), StrokeKind::Inside)];
    ui.painter().set(slot, egui::Shape::Vec(shapes));
    crate::component::label(ui, f.label, tr!("Preview · nothing is accepted yet"), p.muted);
}
