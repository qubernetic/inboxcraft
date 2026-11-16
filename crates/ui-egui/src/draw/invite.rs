// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The invitation card over a message's text (`crate::invite`), as the design's "Invitation in a message" draws it:
//! a date tile on the accent gradient, the title, when and where, and a line in the error colour that says what
//! changed since the version before. The card waiting for an answer has a gradient edge, a cancelled one a red edge.
//! Then by what the invitation does:
//!
//! - an invitation: Going, Maybe and No, and to whom the answer goes at once; once answered, the answer and Change
//!   answer, which offers the three again; one that cannot be answered says why;
//! - a cancellation: that the event is off, and Remove from calendar while the event is in it;
//! - a guest's answer: what each guest said.
//!
//! Under the card a faint strip tells of the version before it and the answer the owner gave to that; the card of a
//! version a later one replaced is only such a strip.

use egui::{Frame, Layout, RichText, Sense, Shape, Stroke, StrokeKind, Ui, WidgetInfo, pos2, vec2};

use super::margin;
use super::reader::{Asked, Drawn};
use crate::component::button::TextButton;
use crate::gradient::{self, Gradient, Stop};
use crate::iconset::{IconId, Style};
use crate::invite::{Invite, Method, Rsvp};
use crate::keymap::Keymap;
use crate::reader::Reader;
use crate::theme::{Font, Theme, text_gradient};
use crate::tr;

/// What the owner asked on a card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ask {
    /// Send this answer.
    Answer(Rsvp),
    /// Offer the answers again, or no longer.
    Change,
    /// Take the cancelled event out of the calendar.
    Remove,
}

/// How faint the strip of a version a later one replaced is.
const FAINT: f32 = 0.75;

/// The width of a card's coloured edge.
const EDGE: f32 = 1.5;

/// The edge of a card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Edge {
    /// The hairline of any card.
    Line,
    /// The preset's gradient: the owner is asked something.
    Gradient,
    /// The error colour: the event is off.
    Error,
}

/// A card across `ui`'s width with `add` drawn inside and `edge` round it.
pub(super) fn edged<R>(ui: &mut Ui, theme: &Theme, edge: Edge, add: impl FnOnce(&mut Ui) -> R) -> R {
    let p = &theme.palette;
    let s = &theme.spacing;
    let slot = ui.painter().add(Shape::Noop);
    let inner = Frame::new().inner_margin(margin(s.l)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        add(ui)
    });
    let rect = inner.response.rect;
    let shapes = match edge {
        Edge::Line => vec![Shape::rect_filled(rect, s.card, p.card), Shape::rect_stroke(rect, s.card, Stroke::new(1.0, p.line), StrokeKind::Inside)],
        Edge::Gradient => {
            let [first, middle, last] = p.gradient;
            let ring = Gradient::Linear {
                from: pos2(0.0, 0.0),
                to: pos2(1.0, 1.0),
                stops: vec![Stop::new(0.0, first), Stop::new(0.5, middle), Stop::new(1.0, last)],
            };
            vec![gradient::filled(ui.painter(), rect, s.card, &ring), Shape::rect_filled(rect.shrink(EDGE), s.card - EDGE, p.surface)]
        }
        Edge::Error => {
            vec![Shape::rect_filled(rect, s.card, p.surface), Shape::rect_stroke(rect, s.card, Stroke::new(EDGE, p.error), StrokeKind::Inside)]
        }
    };
    ui.painter().set(slot, Shape::Vec(shapes));
    inner.inner
}

/// The edge of `invite`'s card: the gradient while an answer is wanted, red when the event is off.
fn edge(reader: &Reader, invite: &Invite) -> Edge {
    match invite.method {
        Method::Cancel => Edge::Error,
        Method::Request if invite.can_answer && (invite.answer.is_none() || reader.changing(invite.message)) => Edge::Gradient,
        Method::Request | Method::Reply | Method::Other => Edge::Line,
    }
}

/// The card of `invite`, with the strip about the version before it.
pub(super) fn card(ui: &mut Ui, theme: &Theme, keymap: &Keymap, reader: &Reader, invite: &Invite, drawn: &mut Drawn) {
    if invite.outdated {
        let line = match invite.answered() {
            Some(answered) => tr!("Out of date: a later invitation replaces this one. {answered}", answered = answered),
            None => tr!("Out of date: a later invitation replaces this one.").to_owned(),
        };
        strip(ui, theme, &line);
        return;
    }
    edged(ui, theme, edge(reader, invite), |ui| {
        ui.spacing_mut().item_spacing.y = theme.spacing.m;
        head(ui, theme, invite);
        if let Some(changed) = invite.changed() {
            chip(ui, theme, IconId::Changed, &changed);
        }
        match invite.method {
            Method::Request => answers(ui, theme, keymap, reader, invite, drawn),
            Method::Cancel => cancelled(ui, theme, keymap, invite, drawn),
            Method::Reply => {
                for reply in &invite.replies {
                    say(ui, theme, &Invite::reply_line(reply), theme.fonts.body, false);
                }
            }
            Method::Other => say(ui, theme, tr!("This calendar message asks for no answer."), theme.fonts.caption, true),
        }
    });
    if let Some(line) = invite.earlier_line() {
        ui.add_space(theme.spacing.s);
        strip(ui, theme, &line);
    }
}

/// A line of text that wraps, muted or not.
fn say(ui: &mut Ui, theme: &Theme, text: &str, font: Font, muted: bool) {
    let colour = if muted { theme.palette.muted } else { theme.palette.text };
    ui.add(egui::Label::new(RichText::new(text).font(font.id()).color(colour)).wrap());
}

/// The date tile, the title and the line with when and where.
fn head(ui: &mut Ui, theme: &Theme, invite: &Invite) {
    let p = &theme.palette;
    let f = &theme.fonts;
    let s = &theme.spacing;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = s.m;
        let month_font = Font { face: f.strong.face, ..f.caption };
        let month_line = super::paint::line_height(ui, month_font);
        let day_line = super::paint::line_height(ui, f.title);
        let height = month_line + day_line + s.s * 2.0;
        let (rect, response) = ui.allocate_exact_size(vec2(height * 0.9, height), Sense::hover());
        let spoken = invite.event.when();
        response.widget_info(|| WidgetInfo::labeled(egui::Role::Image, true, &spoken));
        if ui.is_rect_visible(rect) {
            let ([first, middle, last], ink) = text_gradient(p.gradient);
            let fill = Gradient::Linear {
                from: pos2(0.0, 0.0),
                to: pos2(1.0, 1.0),
                stops: vec![Stop::new(0.0, first), Stop::new(0.5, middle), Stop::new(1.0, last)],
            };
            gradient::fill(ui.painter(), rect, s.card * 0.75, &fill);
            let month = super::paint::line(ui, &[(&invite.month(), month_font, ink)], f32::INFINITY);
            let day = super::paint::line(ui, &[(&invite.day(), f.title, ink)], f32::INFINITY);
            let top = rect.center().y - (month.size().y + day.size().y) / 2.0;
            let below = top + month.size().y;
            ui.painter().galley(pos2(rect.center().x - month.size().x / 2.0, top), month, ink);
            ui.painter().galley(pos2(rect.center().x - day.size().x / 2.0, below), day, ink);
        }
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = s.xs;
            let title = if invite.event.summary.is_empty() { tr!("(no title)").to_owned() } else { invite.event.summary.clone() };
            ui.add(egui::Label::new(RichText::new(title).font(f.title.id()).color(p.text)).wrap());
            let line = invite.line();
            if !line.is_empty() {
                ui.add(egui::Label::new(RichText::new(line).font(f.body.id()).color(p.text)).wrap());
            }
        });
    });
}

/// A line in the error colour with `id` before it, on a small chip: what changed, or that the event is off.
fn chip(ui: &mut Ui, theme: &Theme, id: IconId, text: &str) {
    let p = &theme.palette;
    let s = &theme.spacing;
    let font = Font { face: theme.fonts.strong.face, ..theme.fonts.caption };
    Frame::new().fill(p.panel).corner_radius(s.corner).inner_margin(egui::Margin::symmetric(10, 6)).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = s.s;
            let side = super::paint::line_height(ui, font);
            let (rect, _) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
            theme.icons.paint(ui.painter(), rect, id, Style::Regular, p.error);
            ui.add(egui::Label::new(RichText::new(text).font(font.id()).color(p.error)).wrap());
        });
    });
}

/// An icon of the caption's size, muted.
fn icon(ui: &mut Ui, theme: &Theme, id: IconId) {
    let side = super::paint::line_height(ui, theme.fonts.caption);
    let (rect, _) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
    theme.icons.paint(ui.painter(), rect, id, Style::Regular, theme.palette.muted);
}

/// A faint strip with the older-version clock: what a version a later one replaced said.
fn strip(ui: &mut Ui, theme: &Theme, text: &str) {
    let p = &theme.palette;
    let s = &theme.spacing;
    ui.scope(|ui| {
        ui.multiply_opacity(FAINT);
        Frame::new().fill(p.panel).corner_radius(s.card).inner_margin(margin(s.m)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = s.s;
                icon(ui, theme, IconId::Older);
                say(ui, theme, text, theme.fonts.caption, true);
            });
        });
    });
}

/// The answers of an invitation: the three, or the answer given and Change answer.
fn answers(ui: &mut Ui, theme: &Theme, keymap: &Keymap, reader: &Reader, invite: &Invite, drawn: &mut Drawn) {
    let s = &theme.spacing;
    let ask = |drawn: &mut Drawn, ask: Ask| drawn.asked.push(Asked::Invite(invite.message, ask));
    if !invite.can_answer {
        let why = invite.why_not.as_deref().unwrap_or_default();
        say(ui, theme, &tr!("This invitation cannot be answered: {why}.", why = why), theme.fonts.caption, true);
        if let Some(answered) = invite.answered() {
            say(ui, theme, &answered, theme.fonts.body, false);
        }
        return;
    }
    let changing = reader.changing(invite.message);
    if let Some(answered) = invite.answered().filter(|_| !changing) {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = s.s;
            say(ui, theme, &answered, theme.fonts.body, false);
            if TextButton::new(tr!("Change answer")).secondary().show(ui, theme, keymap).clicked() {
                ask(drawn, Ask::Change);
            }
        });
        return;
    }
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = s.s;
        for answer in Rsvp::ALL {
            let button = TextButton::new(answer.text());
            let button = if answer == Rsvp::Going { button.primary().icon(IconId::Done) } else { button.secondary() };
            if button.show(ui, theme, keymap).clicked() {
                ask(drawn, Ask::Answer(answer));
            }
        }
        if changing && TextButton::new(tr!("Keep my answer")).show(ui, theme, keymap).clicked() {
            ask(drawn, Ask::Change);
        }
    });
    if let Some(goes) = invite.goes_to() {
        say(ui, theme, &goes, theme.fonts.caption, true);
    }
}

/// A cancellation: that the event is off, and Remove from calendar while it is in the calendar.
fn cancelled(ui: &mut Ui, theme: &Theme, keymap: &Keymap, invite: &Invite, drawn: &mut Drawn) {
    let line = match &invite.organizer {
        Some(organizer) => tr!("{organizer} cancelled this event.", organizer = organizer),
        None => tr!("This event is cancelled.").to_owned(),
    };
    chip(ui, theme, IconId::Failed, &line);
    if invite.can_remove {
        ui.with_layout(Layout::left_to_right(egui::Align::Center), |ui| {
            if TextButton::new(tr!("Remove from calendar")).secondary().icon(IconId::Delete).show(ui, theme, keymap).clicked() {
                drawn.asked.push(Asked::Invite(invite.message, Ask::Remove));
            }
        });
    } else if invite.calendar_event.is_none() {
        say(ui, theme, tr!("It is not in your calendar."), theme.fonts.caption, true);
    }
}
