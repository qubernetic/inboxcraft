// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The text of a message: its blocks, plain text with folded quotes, pictures, attachments, and the banners over it.

use std::sync::Arc;

use egui::text::{LayoutJob, TextFormat};
use egui::{
    Align, Color32, ColorImage, Frame, Galley, Layout, Pos2, Rect, RichText, Sense, Stroke, TextureHandle, TextureOptions, Ui, WidgetInfo, pos2, vec2,
};
use inboxcraft_html::{Block, Cell, Document, Image, ImageSource, Marker, TextKind};

use crate::component::attachment::{FileAsked, FileState, attachment};
use crate::component::banner::{BannerKind, BannerLink, banner as component_banner};
use crate::component::button;
use crate::component::progress::ProgressState;
use crate::component::quote_fold::quote_fold;
use crate::component::soon::soon_tag;
use crate::draw::margin;
use crate::draw::paint;
use crate::keymap::Keymap;
use crate::picture::Decoded;
use crate::quote::Segment;
use crate::reader::{Content, File, Opened, PictureState, Reader, inline_key, link_status, size_text};
use crate::theme::{self, Font, Theme};
use crate::tr;

use super::{Asked, Drawn, Textures};

/// How deep tables inside tables are drawn; deeper cells show nothing.
const MAX_DEPTH: usize = 4;

/// The tallest picture shown, in points.
const MAX_PICTURE_HEIGHT: f32 = 480.0;

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

/// A run of text as one wrapped galley, its links found under the pointer.
struct Run {
    job: LayoutJob,
    /// Per section, its characters (start, end) and its link.
    links: Vec<(usize, usize, Option<usize>)>,
    chars: usize,
    text: String,
}

impl Run {
    fn new(width: f32) -> Self {
        let mut job = LayoutJob::default();
        job.wrap.max_width = width.max(1.0);
        Self { job, links: Vec::new(), chars: 0, text: String::new() }
    }

    fn push(&mut self, text: &str, format: TextFormat, link: Option<usize>) {
        let count = text.chars().count();
        self.links.push((self.chars, self.chars.saturating_add(count), link));
        self.chars = self.chars.saturating_add(count);
        self.text.push_str(text);
        self.job.append(text, 0.0, format);
    }

    /// Draws the run; a link under the pointer sets the status and a click on it asks for its address to be copied.
    fn show(self, ui: &mut Ui, document: &Document, drawn: &mut Drawn) {
        if self.text.is_empty() {
            return;
        }
        let galley: Arc<Galley> = ui.painter().layout_job(self.job);
        let (rect, response) = ui.allocate_exact_size(galley.size(), Sense::click());
        let spoken = self.text.clone();
        response.widget_info(|| WidgetInfo::labeled(egui::Role::Label, true, &spoken));
        let under = response.hover_pos().map(|pos| galley.cursor_from_pos(pos - rect.min).index.0);
        let link = under.and_then(|at| self.links.iter().find(|(start, end, _)| (*start..*end).contains(&at)).and_then(|(_, _, link)| *link));
        let link = link.and_then(|index| document.links.get(index));
        if let Some(link) = link {
            drawn.hover = Some(link_status(link));
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            if response.clicked() {
                match &link.target {
                    inboxcraft_html::LinkTarget::Web { url, .. } | inboxcraft_html::LinkTarget::Mail { url, .. } => {
                        drawn.asked.push(Asked::Copy(url.clone()))
                    }
                    inboxcraft_html::LinkTarget::Refused => {}
                }
            }
        }
        if ui.is_rect_visible(rect) {
            ui.painter().galley(rect.min, galley, Color32::PLACEHOLDER);
        }
    }
}

fn texture<'t>(ui: &Ui, textures: &'t mut Textures, key: &str, picture: &Decoded) -> Option<&'t TextureHandle> {
    if picture.rgba.len() != picture.width.saturating_mul(picture.height).saturating_mul(4) {
        return None;
    }
    if !textures.contains_key(key) {
        let image = ColorImage::from_rgba_unmultiplied([picture.width, picture.height], &picture.rgba);
        textures.insert(key.to_owned(), ui.ctx().load_texture(key, image, TextureOptions::LINEAR));
    }
    textures.get(key)
}

/// A picture of the message: shown when here, else a box that says why not.
#[expect(clippy::too_many_arguments, reason = "the drawing context of one picture")]
fn picture(ui: &mut Ui, theme: &Theme, reader: &Reader, id: u64, image: &Image, document: &Document, textures: &mut Textures, drawn: &mut Drawn) {
    let p = &theme.palette;
    let key = match &image.source {
        ImageSource::Inline(content_id) => Some(inline_key(id, content_id)),
        ImageSource::Remote(url) => Some(url.clone()),
        ImageSource::Blocked | ImageSource::Unsupported => None,
    };
    let state = key.as_deref().and_then(|key| reader.picture(key));
    if let (Some(key), Some(PictureState::Ready(decoded))) = (key.as_deref(), state)
        && let Some(handle) = texture(ui, textures, key, decoded)
    {
        let (w, h) = (decoded.width as f32, decoded.height as f32);
        let scale = (ui.available_width() / w).min(MAX_PICTURE_HEIGHT / h).min(1.0);
        let (rect, response) = ui.allocate_exact_size(vec2(w * scale, h * scale), Sense::click());
        let alt = if image.alt.is_empty() { tr!("Picture").to_owned() } else { image.alt.clone() };
        response.widget_info(|| WidgetInfo::labeled(egui::Role::Image, true, &alt));
        ui.painter().image(handle.id(), rect, Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)), theme::NO_TINT);
        if let Some(link) = image.link.and_then(|index| document.links.get(index))
            && response.hovered()
        {
            drawn.hover = Some(link_status(link));
        }
        return;
    }
    let why = match (&image.source, state) {
        (ImageSource::Unsupported, _) if image.alt.is_empty() => return,
        (ImageSource::Unsupported, _) => tr!("Picture not shown").to_owned(),
        (ImageSource::Blocked, _) => tr!("Remote picture not loaded").to_owned(),
        (_, Some(PictureState::Failed(reason))) => tr!("Picture could not be shown: {reason}", reason = reason),
        (_, Some(PictureState::Waiting) | None) => tr!("Loading picture…").to_owned(),
        (_, Some(PictureState::Ready(_))) => tr!("Picture could not be shown").to_owned(),
    };
    Frame::new().stroke(Stroke::new(1.0, p.muted)).inner_margin(margin(theme.spacing.s2)).corner_radius(0.0).show(ui, |ui| {
        let text = if image.alt.is_empty() { why } else { format!("{why}: {}", image.alt) };
        ui.label(RichText::new(text).font(theme.fonts.caption.id()).color(p.muted));
    });
}

/// Display blocks of an HTML message.
/// How wide a picture's column asks to be while the picture is not there to measure.
const WANTED_PICTURE: f32 = 160.0;

/// About how wide a letter of running text is, as a part of the font's size: enough to size a column from the
/// number of letters without laying the text out twice.
const LETTER: f32 = 0.55;

/// How wide `block` asks to be on one line.
fn wanted_width(theme: &Theme, reader: &Reader, id: u64, block: &Block, depth: usize) -> f32 {
    match block {
        Block::Text { .. } => block.text().chars().count() as f32 * theme.fonts.body.size * LETTER,
        Block::Rule { .. } => 0.0,
        Block::Image { image, .. } => {
            let key = match &image.source {
                ImageSource::Inline(content_id) => Some(inline_key(id, content_id)),
                ImageSource::Remote(url) => Some(url.clone()),
                ImageSource::Blocked | ImageSource::Unsupported => None,
            };
            match key.as_deref().and_then(|key| reader.picture(key)) {
                Some(PictureState::Ready(decoded)) => decoded.width as f32,
                _ => WANTED_PICTURE,
            }
        }
        Block::Table { rows, .. } if depth < MAX_DEPTH => {
            wanted_columns(theme, reader, id, rows, depth.saturating_add(1)).iter().sum::<f32>() + theme.spacing.s3
        }
        Block::Table { .. } => 0.0,
    }
}

/// How wide each column of a table asks to be: its widest cell, each cell its widest block.
fn wanted_columns(theme: &Theme, reader: &Reader, id: u64, rows: &[Vec<Cell>], depth: usize) -> Vec<f32> {
    let mut wanted = vec![0.0_f32; rows.iter().map(Vec::len).max().unwrap_or(0)];
    for row in rows {
        for (cell, column) in row.iter().zip(wanted.iter_mut()) {
            let widest = cell.blocks.iter().map(|block| wanted_width(theme, reader, id, block, depth)).fold(0.0, f32::max);
            *column = column.max(widest);
        }
    }
    wanted
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

#[expect(clippy::too_many_arguments, reason = "the drawing context of one block")]
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
) {
    let p = &theme.palette;
    let f = &theme.fonts;
    let quotes = reader.shows_quotes(id);
    let mut toggled = false;
    for block in list {
        let quote = match block {
            Block::Text { quote, .. } | Block::Rule { quote } | Block::Image { quote, .. } | Block::Table { quote, .. } => *quote,
        };
        if quote > 0 && !toggled {
            toggled = true;
            let text = if quotes { tr!("Hide quoted text") } else { tr!("Show quoted text") };
            // A quiet act in the text: a link, with no key of its own.
            if button::link_sized(ui, theme, &Keymap::empty(), text, button::LinkSize::Small).clicked() {
                drawn.asked.push(Asked::Quotes(id));
            }
        }
        if quote == 0 {
            toggled = false;
        } else if !quotes {
            continue;
        }
        let indent = f32::from(quote.min(4)) * theme.spacing.s4;
        ui.horizontal(|ui| {
            ui.add_space(indent);
            ui.vertical(|ui| match block {
                Block::Text { kind, spans, .. } => {
                    let (font, prefix) = match kind {
                        // A heading in a message is what its writer wrote: Plex Sans SemiBold, like bold text.
                        TextKind::Heading(_) => (f.strong, String::new()),
                        TextKind::ListItem { depth, marker } => {
                            let pad = "    ".repeat(usize::from((*depth).clamp(1, 6)).saturating_sub(1));
                            let mark = match marker {
                                Marker::Bullet => "•".to_owned(),
                                Marker::Number(n) => format!("{n}."),
                            };
                            (f.body, format!("{pad}{mark} "))
                        }
                        TextKind::Preformatted => (f.mono, String::new()),
                        TextKind::Paragraph => (f.body, String::new()),
                    };
                    let colour = if quote > 0 { p.muted } else { p.ink };
                    let mut run = Run::new(ui.available_width());
                    if !prefix.is_empty() {
                        run.push(&prefix, TextFormat::simple(font.id(), colour), None);
                    }
                    for span in spans {
                        // Code is monospaced, bold text a heavier weight, both at the size of the block.
                        let face = if span.code {
                            f.mono.face
                        } else if span.bold && matches!(kind, TextKind::Paragraph | TextKind::ListItem { .. }) {
                            f.strong.face
                        } else {
                            font.face
                        };
                        let span_font = Font { face, ..font };
                        let mut format = TextFormat::simple(span_font.id(), if span.link.is_some() { p.cyan_text } else { colour });
                        format.italics = span.italic;
                        if span.underline || span.link.is_some() {
                            format.underline = Stroke::new(1.0, if span.link.is_some() { p.cyan_text } else { colour });
                        }
                        if span.strike {
                            format.strikethrough = Stroke::new(1.0, colour);
                        }
                        if span.bold && span.link.is_none() {
                            format.color = p.ink;
                        }
                        run.push(&span.text, format, span.link);
                    }
                    run.show(ui, document, drawn);
                }
                Block::Rule { .. } => {
                    paint::rule(ui, p.line);
                }
                Block::Image { image, .. } => picture(ui, theme, reader, id, image, document, textures, drawn),
                Block::Table { rows, .. } if depth < MAX_DEPTH => {
                    let gap = theme.spacing.s3;
                    let widths = column_widths(&wanted_columns(theme, reader, id, rows, depth), ui.available_width(), gap);
                    let spacing = ui.spacing().item_spacing.x;
                    for row in rows {
                        ui.horizontal_top(|ui| {
                            ui.spacing_mut().item_spacing.x = gap;
                            for (cell, width) in row.iter().zip(&widths) {
                                ui.allocate_ui_with_layout(vec2(*width, 0.0), Layout::top_down(Align::Min), |ui| {
                                    ui.spacing_mut().item_spacing.x = spacing;
                                    ui.set_width(*width);
                                    blocks(ui, theme, reader, id, document, &cell.blocks, depth.saturating_add(1), textures, drawn);
                                });
                            }
                        });
                    }
                }
                Block::Table { .. } => {}
            });
        });
        ui.add_space(theme.spacing.s2);
    }
}

/// A plain-text body in `read`, its quotes folded until the owner opens them ([`quote_fold`]).
fn text(ui: &mut Ui, theme: &Theme, reader: &Reader, id: u64, segments: &[Segment], drawn: &mut Drawn) {
    let p = &theme.palette;
    let f = &theme.fonts;
    for segment in segments {
        match segment {
            Segment::Text(text) => {
                ui.label(RichText::new(text).font(f.read.id()).color(p.ink));
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
/// or `save` saves it, and the box then says where it went, or why not with `try again`. Opening a file is not built yet: the strip ends with the `coming soon` tag once.
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
        Content::Text(segments) => text(ui, theme, reader, opened.id, segments, drawn),
        Content::Html(document) => blocks(ui, theme, reader, opened.id, document, &document.blocks, 0, textures, drawn),
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
