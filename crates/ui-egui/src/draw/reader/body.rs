// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The text of a message: its blocks, plain text with folded quotes, pictures, attachments, and the banners over it.

use egui::{Align, Frame, Layout, RichText, Sense, Stroke, Ui, vec2};
use inboxcraft_html::{Block, Document};

use crate::component::attachment::{FileAsked, FileState, attachment};
use crate::component::button;
use crate::component::quote_fold::quote_fold;
use crate::component::soon::soon_tag;
use crate::draw::margin;
use crate::iconset::IconId;
use crate::keymap::Keymap;
use crate::quote::Segment;
use crate::reader::{Content, File, Opened, Reader, size_text};
use crate::theme::{Font, Theme};
use crate::tr;

use super::{Asked, Drawn, Textures};

/// The banner over a sent message that did not get through: the warning and the reason the delivery report gave.
pub(super) fn bounced(ui: &mut Ui, theme: &Theme, reason: &str) {
    let p = &theme.palette;
    let text = tr!("Not delivered: {reason}", reason = reason);
    Frame::new().fill(p.panel).stroke(Stroke::new(1.0, p.minus)).inner_margin(margin(theme.spacing.s3)).corner_radius(0.0).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            let side = theme.fonts.body.line;
            let (rect, _) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
            crate::iconset::paint(ui.painter(), rect, IconId::Failed, p.minus);
            ui.add(egui::Label::new(RichText::new(text).font(theme.fonts.body.id()).color(p.ink)).wrap());
        });
    });
}

/// The remote-content banner, while remote pictures of the message are blocked.
pub(super) fn banner(ui: &mut Ui, theme: &Theme, keymap: &Keymap, reader: &Reader, opened: &Opened, drawn: &mut Drawn) {
    let Content::Html(document) = &opened.content else { return };
    if document.remote_blocked == 0 || reader.remote_allowed(opened.id) {
        return;
    }
    let p = &theme.palette;
    let s = &theme.spacing;
    let f = &theme.fonts;
    Frame::new().fill(p.panel).inner_margin(egui::Margin::symmetric(margin(s.s3).left, margin(s.s2).top)).corner_radius(0.0).show(ui, |ui| {
        ui.set_width(ui.available_width());
        let count = document.remote_blocked;
        let text = if count == 1 {
            tr!("1 remote picture is not shown. Loading it tells the sender that you opened this message.").to_owned()
        } else {
            tr!("{count} remote pictures are not shown. Loading them tells the sender that you opened this message.", count = count)
        };
        let always = opened.header.from.first().map(|sender| tr!("Always for {address}", address = sender.address));
        let mut links: Vec<(&str, Asked)> = vec![(tr!("Load once"), Asked::Once(opened.id))];
        links.extend(always.as_deref().map(|always| (always, Asked::Always(opened.id))));
        let side = f.body.line;
        let width = |ui: &Ui, text: &str, font: Font| crate::draw::paint::line(ui, &[(text, font, p.ink)], f32::INFINITY).size().x;
        let wanted = side + width(ui, &text, f.body) + links.iter().map(|(name, _)| width(ui, name, f.button) + s.s4).sum::<f32>();
        let beside = wanted + s.s4 * 2.0 <= ui.available_width();
        let mut link = |ui: &mut Ui, name: &str, asked: &Asked| {
            if button::link(ui, theme, keymap, name).clicked() {
                drawn.asked.push(asked.clone());
            }
        };
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = s.s2;
            let (rect, _) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
            crate::iconset::paint(ui.painter(), rect, IconId::Shield, p.cyan_text);
            ui.add(egui::Label::new(RichText::new(text).font(f.body.id()).color(p.ink)).wrap());
            if beside {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    for (name, asked) in links.iter().rev() {
                        link(ui, name, asked);
                    }
                });
            }
        });
        if !beside {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = s.s2;
                for (name, asked) in &links {
                    link(ui, name, asked);
                }
            });
        }
    });
}

/// The width of each column in `room`, with `gap` between them: each gets what it asks for when all fit; otherwise
/// the narrow ones still do and the rest share what is left equally. Together they never pass `room`.
pub(in crate::draw) fn column_widths(wanted: &[f32], room: f32, gap: f32) -> Vec<f32> {
    let count = wanted.len();
    let mut left = (room - gap * count.saturating_sub(1) as f32).max(0.0);
    let mut widths = vec![0.0; count];
    let mut narrowest: Vec<(usize, f32)> = wanted.iter().copied().enumerate().collect();
    narrowest.sort_by(|a, b| a.1.total_cmp(&b.1));
    for (done, (column, asked)) in narrowest.into_iter().enumerate() {
        let share = left / count.saturating_sub(done).max(1) as f32;
        let width = asked.min(share).max(0.0);
        if let Some(slot) = widths.get_mut(column) {
            *slot = width;
        }
        left -= width;
    }
    widths
}

/// The blocks of an HTML message, fitted to the app ([`super::fitted`]): `list`, blocks of message `id`'s `document`,
/// tables `depth` deep; whether it did (where it did not, the reader holds too much fitted text to keep this too).
#[expect(clippy::too_many_arguments, reason = "the drawing context of a message's blocks")]
pub(in crate::draw) fn blocks(
    ui: &mut Ui,
    theme: &Theme,
    reader: &Reader,
    id: u64,
    document: &Document,
    list: &[Block],
    depth: usize,
    textures: &mut Textures,
    drawn: &mut Drawn,
) -> bool {
    let generation = reader.opened(id).map_or(0, crate::reader::Opened::generation);
    super::fitted::blocks(ui, theme, reader, id, generation, document, list, depth, textures, drawn)
}

/// A plain-text body in `font`, its quotes folded until the owner opens them ([`quote_fold`]).
fn text(ui: &mut Ui, theme: &Theme, reader: &Reader, id: u64, segments: &[Segment], font: Font, drawn: &mut Drawn) {
    let p = &theme.palette;
    for segment in segments {
        match segment {
            Segment::Text(text) => {
                ui.label(RichText::new(text).font(font.id()).color(p.ink));
            }
            Segment::Quote { attribution, text, lines } => {
                if quote_fold(ui, theme, *lines, reader.shows_quotes(id), attribution.as_deref(), text).clicked() {
                    drawn.asked.push(Asked::Quotes(id));
                }
            }
        }
    }
}

/// The attachments, side by side as far as they fit, 8 apart, each as wide as what it holds ([`attachment`]): Enter
/// or `save` saves it. Opening a file is not built yet: the strip ends with the `coming soon` tag once.
pub(super) fn files(ui: &mut Ui, theme: &Theme, keymap: &Keymap, id: u64, files: &[File], drawn: &mut Drawn) {
    if files.is_empty() {
        return;
    }
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(STRIP_GAP, STRIP_GAP);
        for file in files {
            let size = size_text(file.size);
            let (_, asked) = attachment(ui, theme, keymap, &file.name, &size, FileState::Ready);
            if asked == Some(FileAsked::Save) {
                drawn.asked.push(Asked::Save(id, file.index));
            }
        }
        // The tag once after the last box, only where it fits beside it: never on a line of its own (the board
        // carries it too).
        if ui.available_size_before_wrap().x >= crate::component::tag::width(ui, theme, tr!("coming soon")) + STRIP_GAP {
            soon_tag(ui, theme);
        }
    });
}

/// The room between two attachments, in points.
const STRIP_GAP: f32 = 8.0;

/// The text of a message, and what stands for its text when it is not here.
pub(super) fn body(ui: &mut Ui, theme: &Theme, reader: &Reader, opened: &Opened, textures: &mut Textures, drawn: &mut Drawn) {
    let say = |ui: &mut Ui, text: &str| {
        ui.label(RichText::new(text).font(theme.fonts.body.id()).color(theme.palette.muted));
    };
    match &opened.content {
        Content::Loading => say(ui, tr!("Loading the message…")),
        Content::NotKept => say(ui, tr!("The text of this message is not here yet. It comes with the next connection.")),
        Content::TooLarge => say(ui, tr!("This message is too large to keep, so it cannot be shown.")),
        Content::Unreadable => say(ui, tr!("This message cannot be read.")),
        Content::Text(segments) => text(ui, theme, reader, opened.id, segments, theme.fonts.read, drawn),
        // The plain-text view: the message as a text-only program shows it, in `mono` as the design sets it, its
        // quotes folded as a text's are.
        Content::Html(_) if reader.shows_plain(opened.id) => text(ui, theme, reader, opened.id, &opened.plain, theme.fonts.mono, drawn),
        Content::Html(document) => {
            // A message that does not fit beside the fitted ones the reader keeps is shown plain, and says why.
            if !blocks(ui, theme, reader, opened.id, document, &document.blocks, 0, textures, drawn) {
                say(ui, tr!("This message is shown as plain text: the reader holds as much fitted text as it keeps."));
                text(ui, theme, reader, opened.id, &opened.plain, theme.fonts.mono, drawn);
            }
        }
    }
    if opened.cut {
        say(ui, tr!("The text is cut here: the rest is longer than the reader shows."));
    }
}
