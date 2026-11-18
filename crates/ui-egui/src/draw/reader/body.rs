// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The text of a message: its blocks, plain text with folded quotes, pictures, attachments, and the banners over it.

use egui::{Rect, RichText, Ui, vec2};
use inboxcraft_html::{Block, Document};

use crate::component::attachment::{FileAsked, FileState, attachment};
use crate::component::banner::{BannerKind, BannerLink, banner as component_banner};
use crate::component::progress::ProgressState;
use crate::component::quote_fold::quote_fold;
use crate::component::soon::soon_tag;
use crate::keymap::Keymap;
use crate::quote::Segment;
use crate::reader::{Content, File, Opened, Reader, size_text};
use crate::theme::{Font, Theme};
use crate::tr;

use super::{Asked, Drawn, Textures};

/// The id under which the reader keeps the room its warnings are clipped to.
fn unwiped_id() -> egui::Id {
    egui::Id::unique("reader-unwiped")
}

/// Keeps `room`, the message's room unclipped by the wipe, for this frame's warnings.
pub(super) fn keep_unwiped(ctx: &egui::Context, room: Rect) {
    ctx.data_mut(|data| data.insert_temp(unwiped_id(), room));
}

/// Draws `add` clipped to the message's room only, not to the wipe: a warning shows whole while a message wipes in.
fn unwiped<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let room = ui.ctx().data(|data| data.get_temp::<Rect>(unwiped_id()));
    ui.scope(|ui| {
        if let Some(room) = room {
            ui.set_clip_rect(room);
        }
        add(ui)
    })
    .inner
}

/// The reader's notice as a banner: a note of what was done, or an error when something failed, with `dismiss`.
pub(super) fn notice(ui: &mut Ui, theme: &Theme, keymap: &Keymap, text: &str, failed: bool, drawn: &mut Drawn) {
    let kind = if failed { BannerKind::Error } else { BannerKind::Notice };
    let links = [BannerLink { name: tr!("dismiss"), id: "dismiss" }];
    if unwiped(ui, |ui| component_banner(ui, theme, keymap, kind, text, &links)).is_some() {
        drawn.asked.push(Asked::Dismiss);
    }
}

/// The banner over a sent message that did not get through: NOT DELIVERED and the reason the delivery report gave.
pub(super) fn bounced(ui: &mut Ui, theme: &Theme, reason: &str) {
    let text = tr!("This message did not arrive: {reason}", reason = reason);
    unwiped(ui, |ui| component_banner(ui, theme, &Keymap::empty(), BannerKind::Bounce, &text, &[]));
}

/// The remote-content banner, while remote pictures of the message are blocked: PICTURES, what loading them tells the
/// sender, `load once` and `always for` the sender's address.
pub(super) fn banner(ui: &mut Ui, theme: &Theme, keymap: &Keymap, reader: &Reader, opened: &Opened, drawn: &mut Drawn) {
    let Content::Html(document) = &opened.content else { return };
    if document.remote_blocked == 0 || reader.remote_allowed(opened.id) {
        return;
    }
    let count = document.remote_blocked;
    let text = if count == 1 {
        tr!("1 remote picture is not shown. Loading it tells the sender that you opened this message.").to_owned()
    } else {
        tr!("{count} remote pictures are not shown. Loading them tells the sender that you opened this message.", count = count)
    };
    let always = opened.header.from.first().map(|sender| tr!("always for {address}", address = sender.address));
    let mut links = vec![BannerLink { name: tr!("load once"), id: "once" }];
    links.extend(always.as_deref().map(|always| BannerLink { name: always, id: "always" }));
    match unwiped(ui, |ui| component_banner(ui, theme, keymap, BannerKind::RemotePictures, &text, &links)).as_deref() {
        Some("once") => drawn.asked.push(Asked::Once(opened.id)),
        Some("always") => drawn.asked.push(Asked::Always(opened.id)),
        _ => {}
    }
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
/// or `save` saves it, and the box then says where it went, or why not with `try again`. Opening a file is not built
/// yet: the strip ends with the `coming soon` tag once.
pub(super) fn files(ui: &mut Ui, theme: &Theme, keymap: &Keymap, reader: &Reader, id: u64, files: &[File], drawn: &mut Drawn) {
    if files.is_empty() {
        return;
    }
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(STRIP_GAP, STRIP_GAP);
        for file in files {
            let size = size_text(file.size);
            // Where saving it went, from the save's outcome.
            let state = match reader.saved(id, file.index) {
                None => FileState::Ready,
                Some(Ok(place)) => FileState::Saved(place),
                Some(Err(why)) => FileState::Failed(why),
            };
            let (_, asked) = attachment(ui, theme, keymap, &file.name, &size, state);
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

/// The text of a message, and what stands for its text when it is not here: one sentence in a note, or the progress
/// line while it loads; a note after a text the reader cut.
pub(super) fn body(ui: &mut Ui, theme: &Theme, reader: &Reader, opened: &Opened, textures: &mut Textures, drawn: &mut Drawn) {
    match &opened.content {
        Content::Loading => loading(ui, theme),
        Content::NotKept => note(ui, theme, tr!("The text of this message is not here yet. It comes with the next connection.")),
        Content::TooLarge => note(ui, theme, tr!("This message is too large to keep, so it cannot be shown.")),
        Content::Unreadable => note(ui, theme, tr!("This message cannot be read.")),
        Content::Text(segments) => text(ui, theme, reader, opened.id, segments, theme.fonts.read, drawn),
        // The plain-text view: the message as a text-only program shows it, in `mono` as the design sets it, its
        // quotes folded as a text's are.
        Content::Html(_) if reader.shows_plain(opened.id) => text(ui, theme, reader, opened.id, &opened.plain, theme.fonts.mono, drawn),
        Content::Html(document) => {
            // A message that does not fit beside the fitted ones the reader keeps is shown plain, and says why.
            if !blocks(ui, theme, reader, opened.id, document, &document.blocks, 0, textures, drawn) {
                note(ui, theme, tr!("This message is shown as plain text: the reader holds as much fitted text as it keeps."));
                text(ui, theme, reader, opened.id, &opened.plain, theme.fonts.mono, drawn);
            }
        }
    }
    if opened.cut {
        note(ui, theme, tr!("The text is cut here: the rest is longer than the reader shows."));
    }
}

/// A note banner saying `text`, in the text's place.
fn note(ui: &mut Ui, theme: &Theme, text: &str) {
    component_banner(ui, theme, &Keymap::empty(), BannerKind::Notice, text, &[]);
}

/// The progress line while the message's text is fetched: the fetch says nothing of how far it is, so the bar stands
/// empty and the line has no note.
fn loading(ui: &mut Ui, theme: &Theme) {
    crate::component::progress::progress(ui, theme, &Keymap::empty(), tr!("loading the message"), ProgressState::Running(0.0), "");
}
