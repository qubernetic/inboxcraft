// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The thread: folded messages, the header of an unfolded one, one unfolded message and the answers under it.

use egui::{Align, Frame, Grid, Layout, Margin, Rect, RichText, Sense, Stroke, StrokeKind, Ui, WidgetInfo, pos2, vec2};
use inboxcraft_html::Block;

use crate::component::button::TextButton;
use crate::compose::Answer;
use crate::draw::margin;
use crate::iconset::IconId;
use crate::keymap::Keymap;
use crate::mailbox::Mailbox;
use crate::quote::Segment;
use crate::reader::{Content, Header, Opened, Person, Reader, ThreadRow};
use crate::theme::Theme;
use crate::tr;

use super::body::{banner, body, files};
use super::head::{pointer_near, quiet};
use super::{Asked, Drawn, Textures};

pub(super) fn names(people: &[Person]) -> String {
    people.iter().map(Person::shown).collect::<Vec<_>>().join(", ")
}

/// Whether the text of `opened` only says what its invitation card says.
pub(super) fn restated(opened: &Opened) -> bool {
    let Some(invite) = opened.invite.as_ref().filter(|_| !opened.cut && opened.files.is_empty()) else { return false };
    let mut said = String::new();
    let plain = match &opened.content {
        Content::Text(segments) => segments.iter().all(|segment| match segment {
            Segment::Text(text) => {
                said.push_str(text);
                said.push(' ');
                true
            }
            Segment::Quote { .. } => false,
        }),
        Content::Html(document) => document.blocks.iter().all(|block| match block {
            Block::Text { quote: 0, spans, .. } => {
                for span in spans {
                    said.push_str(&span.text);
                    said.push(' ');
                }
                true
            }
            _ => false,
        }),
        Content::Loading | Content::NotKept | Content::TooLarge | Content::Unreadable => false,
    };
    plain && invite.restates(&said)
}

/// Whether `opened` is the open message drawn without a header of its own: the only message of its thread, one the
/// screener holds, or one whose invitation card says all it says. The full header brings its header back.
pub(super) fn bare(mailbox: &Mailbox, reader: &Reader, opened: &Opened) -> bool {
    Some(opened.id) == reader.open_id()
        && !reader.shows_details(opened.id)
        && (reader.thread().len() == 1 || mailbox.held(opened.id) || restated(opened))
}

/// A card's frame: the translucent card colour with a line round it.
pub(super) fn card_frame(theme: &Theme, inner: f32) -> Frame {
    Frame::new().fill(theme.palette.surface).stroke(Stroke::new(1.0, theme.palette.line)).corner_radius(0.0).inner_margin(margin(inner))
}

/// One folded message of the thread, a card of one line: the avatar, the sender, the first words and the date; a
/// click unfolds it.
pub(super) fn folded(ui: &mut Ui, theme: &Theme, mailbox: &Mailbox, row: &ThreadRow, drawn: &mut Drawn) {
    let p = &theme.palette;
    let s = &theme.spacing;
    let date = row.date.map(|date| mailbox.short_date(date)).unwrap_or_default();
    let spoken = tr!("{sender}, {date}, folded: {preview}", sender = row.sender, date = date, preview = row.preview);
    let f = &theme.fonts;
    let line = f.body.line.max(f.caption.line);
    let side = line * 1.5;
    let height = side + s.s3 * 2.0;
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(egui::Role::Button, true, &spoken));
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        painter.rect_filled(rect, 0.0, p.surface);
        if response.hovered() {
            painter.rect_filled(rect, 0.0, p.stage);
        }
        painter.rect_stroke(rect, 0.0, Stroke::new(1.0, p.line), StrokeKind::Inside);
        let face = Rect::from_min_size(pos2(rect.left() + s.s3, rect.center().y - side / 2.0), vec2(side, side));
        crate::component::avatar::paint(painter, theme, face, &row.sender, &row.address);
        let left = face.right() + s.s3;
        let date_galley = crate::draw::paint::line(ui, &[(&date, f.mono_s, p.muted)], f32::INFINITY);
        let date_left = rect.right() - s.s4 - date_galley.size().x;
        ui.painter().galley(pos2(date_left, rect.center().y - date_galley.size().y / 2.0), date_galley, p.muted);
        let preview = format!("   {}", row.preview);
        let text = crate::draw::paint::line(ui, &[(&row.sender, f.body, p.ink), (&preview, f.caption, p.muted)], date_left - s.s3 - left);
        ui.painter().galley(pos2(left, rect.center().y - text.size().y / 2.0), text, p.ink);
        if response.has_focus() {
            crate::draw::paint::focus_ring(ui.painter(), theme, response.rect, crate::draw::paint::Ring::Inside);
        }
    }
    if response.clicked() {
        drawn.asked.push(Asked::Expand(row.id));
    }
}

/// The side of the header's avatar: as tall as the two lines beside it, with a little room.
pub(super) fn header_side(theme: &Theme) -> f32 {
    theme.fonts.strong.line + theme.fonts.caption.line + theme.spacing.s2
}

/// The header: the avatar, the sender with whom it went to under them, and the date at the right; Details and Fold,
/// quiet until the pointer is near. With the details open, the date goes into the full header, the addresses too.
fn header(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, reader: &Reader, opened: &Opened, drawn: &mut Drawn) {
    let p = &theme.palette;
    let f = &theme.fonts;
    let s = &theme.spacing;
    let Header { from, reply_to, to, cc, date, .. } = &opened.header;
    let details = reader.shows_details(opened.id);
    let side = header_side(theme);
    let near = pointer_near(ui, side);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = s.s2;
        if let Some(person) = from.first() {
            crate::component::avatar::avatar(ui, theme, &person.name, &person.address, side);
        }
        // The date and the links first, at the right; the two lines then share what is left, cut with an ellipsis.
        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
            if details {
                if TextButton::new(tr!("Hide details")).secondary().show(ui, theme, keymap).clicked() {
                    drawn.asked.push(Asked::Details(opened.id));
                }
            } else {
                if let Some(date) = date {
                    ui.label(f.mono_s.rich(&mailbox.short_date(*date)).color(p.muted));
                }
                if quiet(ui, theme, keymap, tr!("Details"), ("details", opened.id), near).clicked() {
                    drawn.asked.push(Asked::Details(opened.id));
                }
            }
            if Some(opened.id) != reader.open_id() && quiet(ui, theme, keymap, tr!("Fold"), ("fold", opened.id), near).clicked() {
                drawn.asked.push(Asked::Expand(opened.id));
            }
            ui.with_layout(Layout::top_down(Align::Min), |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                let sender = from.first().map_or_else(|| tr!("(no sender)").to_owned(), |person| person.shown().to_owned());
                let name = ui.add(egui::Label::new(RichText::new(sender).font(f.strong.id()).color(p.ink)).truncate());
                // The address, one hover away, for whoever wonders who this really is.
                if let Some(person) = from.first().filter(|person| !person.name.is_empty()) {
                    name.on_hover_text(&person.address);
                }
                if !details {
                    let mut whom = names(to);
                    if !cc.is_empty() {
                        whom = tr!("{to}, copy to {cc}", to = whom, cc = names(cc));
                    }
                    ui.add(egui::Label::new(RichText::new(tr!("to {whom}", whom = whom)).font(f.caption.id()).color(p.muted)).truncate());
                }
            });
        });
    });
    if !details {
        return;
    }
    Frame::new().fill(p.panel).corner_radius(0.0).inner_margin(margin(s.s3)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        Grid::new(("details", opened.id)).num_columns(2).spacing(vec2(s.s6, s.s1)).show(ui, |ui| {
            let row = |ui: &mut Ui, name: &str, value: String| {
                ui.label(RichText::new(name).font(f.caption.id()).color(p.muted));
                ui.add(egui::Label::new(RichText::new(value).font(f.caption.id()).color(p.ink)).wrap());
                ui.end_row();
            };
            let full = |people: &[Person]| people.iter().map(Person::full).collect::<Vec<_>>().join(", ");
            row(ui, tr!("From"), full(from));
            row(ui, tr!("To"), full(to));
            if !cc.is_empty() {
                row(ui, tr!("Copy"), full(cc));
            }
            if !reply_to.is_empty() {
                row(ui, tr!("Reply to"), full(reply_to));
            }
            if let Some(date) = date {
                row(ui, tr!("Date"), mailbox.full_date(*date));
            }
        });
    });
}

/// Reply, Reply all and Forward, under the open message.
fn answers(ui: &mut Ui, theme: &Theme, keymap: &Keymap, id: u64, drawn: &mut Drawn) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = theme.spacing.s2;
        let answers = [
            (TextButton::new(tr!("Reply")).primary().icon(IconId::Reply), Answer::Reply),
            (TextButton::new(tr!("Reply all")).secondary().icon(IconId::ReplyAll), Answer::ReplyAll),
            (TextButton::new(tr!("Forward")).secondary().icon(IconId::Forward), Answer::Forward),
        ];
        for (button, answer) in answers {
            if button.show(ui, theme, keymap).clicked() {
                drawn.asked.push(Asked::Answer(id, answer));
            }
        }
    });
}

/// One unfolded message. With a header it is one card, the text under the sender's name; without one (see [`bare`])
/// the remote-content banner stands over the card and the attachments under it, and the invitation's card alone stands
/// for a message it says all of. The open one ends with Reply, Reply all and Forward, but for a preview of held mail.
#[expect(clippy::too_many_arguments, reason = "the drawing context of one message")]
pub(super) fn message(
    ui: &mut Ui,
    theme: &Theme,
    keymap: &Keymap,
    mailbox: &Mailbox,
    reader: &Reader,
    opened: &Opened,
    textures: &mut Textures,
    drawn: &mut Drawn,
) {
    let s = &theme.spacing;
    let open = Some(opened.id) == reader.open_id();
    let held = open && mailbox.held(opened.id);
    let bare = bare(mailbox, reader, opened);
    if let (true, false, true, Some(invite)) = (bare, held, restated(opened), &opened.invite) {
        crate::draw::invite::card(ui, theme, keymap, reader, invite, drawn);
        return;
    }
    if bare {
        banner(ui, theme, keymap, reader, opened, drawn);
    }
    card_frame(theme, s.s4).show(ui, |ui| {
        ui.set_width(ui.available_width());
        if !bare {
            header(ui, theme, keymap, mailbox, reader, opened, drawn);
            ui.add_space(s.s2);
            banner(ui, theme, keymap, reader, opened, drawn);
        }
        if held && bare {
            let subject = if opened.header.subject.is_empty() { tr!("(no subject)") } else { opened.header.subject.as_str() };
            ui.add(egui::Label::new(RichText::new(subject).font(theme.fonts.heading.id()).color(theme.palette.ink)).wrap());
        }
        if let Some(invite) = &opened.invite {
            crate::draw::invite::card(ui, theme, keymap, reader, invite, drawn);
            ui.add_space(s.s2);
        }
        let indent = if bare || reader.shows_details(opened.id) { 0 } else { margin(header_side(theme) + s.s2).left };
        Frame::new().inner_margin(Margin { left: indent, ..Margin::ZERO }).show(ui, |ui| {
            body(ui, theme, reader, opened, textures, drawn);
            if !bare {
                files(ui, theme, keymap, opened.id, &opened.files, drawn);
            }
            if open && !held {
                ui.add_space(s.s2);
                answers(ui, theme, keymap, opened.id, drawn);
            }
        });
    });
    if bare {
        files(ui, theme, keymap, opened.id, &opened.files, drawn);
    }
}
