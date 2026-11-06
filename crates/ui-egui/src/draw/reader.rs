// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The reader: a toolbar (the next-message rule, full width, close) over the subject, the thread oldest first (the open
//! message and the ones the owner unfolded in full, the rest folded to one line), each message's header, the
//! remote-content banner, the body, the attachment tiles, and a status line that shows where the link under the
//! pointer goes.
//!
//! Drawing only reads the [`Reader`]; what the person asks for comes back as [`Asked`] and is applied after the frame.

use std::collections::HashMap;
use std::sync::Arc;

use egui::text::{LayoutJob, TextFormat};
use egui::{
    Align, Color32, ColorImage, ComboBox, Frame, Galley, Grid, Layout, Margin, Pos2, Rect, RichText, ScrollArea, Sense, Stroke, TextureHandle,
    TextureOptions, Ui, WidgetInfo, pos2, vec2,
};
use inboxcraft_html::{Block, Document, Image, ImageSource, Marker, TextKind};

use super::margin;
use crate::iconset::{IconId, Style};
use crate::mailbox::{After, Mailbox};
use crate::picture::Decoded;
use crate::quote::Segment;
use crate::reader::{Content, File, Header, Opened, Person, PictureState, Reader, ThreadRow, inline_key, link_status, size_text};
use crate::theme::{self, Font, Theme};
use crate::tr;

/// How deep tables inside tables are drawn; deeper cells show nothing.
const MAX_DEPTH: usize = 4;

/// The tallest picture shown, in points.
const MAX_PICTURE_HEIGHT: f32 = 480.0;

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
}

/// What the reader drew: what was asked for, and the status of the link under the pointer.
#[derive(Debug, Default)]
pub(super) struct Drawn {
    pub(super) asked: Vec<Asked>,
    pub(super) hover: Option<String>,
}

/// The textures of the pictures shown, by picture key.
pub(super) type Textures = HashMap<String, TextureHandle>;

fn after_text(after: After) -> &'static str {
    match after {
        After::Next => tr!("Show the next message"),
        After::Previous => tr!("Show the previous message"),
        After::List => tr!("Go back to the list"),
    }
}

fn names(people: &[Person]) -> String {
    people.iter().map(Person::shown).collect::<Vec<_>>().join(", ")
}

/// The toolbar: the next-message rule, full width and close, and the subject under them.
fn toolbar(ui: &mut Ui, theme: &Theme, mailbox: &Mailbox, reader: &Reader, drawn: &mut Drawn) {
    let subject = reader
        .open_id()
        .and_then(|id| reader.opened(id))
        .map(|opened| opened.header.subject.clone())
        .filter(|subject| !subject.is_empty())
        .unwrap_or_else(|| tr!("(no subject)").to_owned());
    // The controls wrap onto a second line when the text scale leaves no room for them on one; the subject has the
    // line below to itself and wraps rather than being cut.
    ui.horizontal_wrapped(|ui| {
        let label = ui.label(RichText::new(tr!("When a message leaves the list")).font(theme.fonts.caption.id()).color(theme.palette.muted));
        let response = ComboBox::from_id_salt("after-rule")
            .selected_text(after_text(mailbox.after()))
            .show_ui(ui, |ui| {
                for after in [After::Next, After::Previous, After::List] {
                    if ui.selectable_label(mailbox.after() == after, after_text(after)).clicked() {
                        drawn.asked.push(Asked::After(after));
                    }
                }
            })
            .response;
        let _ = response.labelled_by(label.id);
        let (text, full) = if reader.full_width() { (tr!("Leave full width"), false) } else { (tr!("Full width"), true) };
        if ui.button(text).clicked() {
            drawn.asked.push(Asked::FullWidth(full));
        }
        if ui.button(tr!("Close")).clicked() {
            drawn.asked.push(Asked::Close);
        }
    });
    ui.add_space(theme.spacing.s);
    ui.add(egui::Label::new(RichText::new(subject).font(theme.fonts.title.id()).color(theme.palette.text)).wrap());
}

/// One folded message of the thread: sender, date and the first words; a click unfolds it.
fn folded(ui: &mut Ui, theme: &Theme, mailbox: &Mailbox, row: &ThreadRow, drawn: &mut Drawn) {
    let p = &theme.palette;
    let date = row.date.map(|date| mailbox.short_date(date)).unwrap_or_default();
    let spoken = tr!("{sender}, {date}, folded: {preview}", sender = row.sender, date = date, preview = row.preview);
    let f = &theme.fonts;
    let height = super::paint::line_height(ui, f.sender).max(super::paint::line_height(ui, f.caption)) + theme.spacing.m * 2.0;
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(egui::Role::Button, true, &spoken));
    if ui.is_rect_visible(rect) {
        if response.hovered() {
            ui.painter().rect_filled(rect, theme.spacing.corner, p.hover);
        }
        ui.painter().rect_stroke(rect, theme.spacing.corner, Stroke::new(1.0, p.line), egui::StrokeKind::Inside);
        let left = rect.left() + theme.spacing.m;
        let date_galley = super::paint::line(ui, &[(&date, f.caption, p.muted)], f32::INFINITY);
        let date_left = rect.right() - theme.spacing.m - date_galley.size().x;
        ui.painter().galley(pos2(date_left, rect.center().y - date_galley.size().y / 2.0), date_galley, p.muted);
        let preview = format!("   {}", row.preview);
        let text = super::paint::line(ui, &[(&row.sender, f.sender, p.text), (&preview, f.preview, p.muted)], date_left - theme.spacing.m - left);
        ui.painter().galley(pos2(left, rect.center().y - text.size().y / 2.0), text, p.text);
        super::paint::focus_ring(ui, theme, &response);
    }
    if response.clicked() {
        drawn.asked.push(Asked::Expand(row.id));
    }
}

/// The header: the sender and the date, whom it went to, and the full header when asked.
fn header(ui: &mut Ui, theme: &Theme, mailbox: &Mailbox, reader: &Reader, opened: &Opened, drawn: &mut Drawn) {
    let p = &theme.palette;
    let f = &theme.fonts;
    let Header { from, reply_to, to, cc, date, .. } = &opened.header;
    // The date and the buttons first, at the right; the avatar, the sender and the address then share what is left,
    // cut with an ellipsis rather than running into the date.
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let text = if reader.shows_details(opened.id) { tr!("Hide details") } else { tr!("Details") };
            if ui.button(text).clicked() {
                drawn.asked.push(Asked::Details(opened.id));
            }
            if Some(opened.id) != reader.open_id() && ui.button(tr!("Fold")).clicked() {
                drawn.asked.push(Asked::Expand(opened.id));
            }
            if let Some(date) = date {
                ui.label(RichText::new(mailbox.short_date(*date)).font(f.caption.id()).color(p.muted));
            }
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                if let Some(person) = from.first() {
                    let side = super::paint::line_height(ui, f.heading) * 1.5;
                    crate::component::avatar::avatar(ui, theme, &person.name, &person.address, side);
                }
                let sender = from.first().map_or_else(|| tr!("(no sender)").to_owned(), |person| person.shown().to_owned());
                ui.add(egui::Label::new(RichText::new(sender).font(f.heading.id()).color(p.text)).truncate());
                if let Some(address) = from.first().filter(|person| !person.name.is_empty()).map(|person| person.address.clone()) {
                    ui.add(egui::Label::new(RichText::new(address).font(f.caption.id()).color(p.muted)).truncate());
                }
            });
        });
    });
    if !reader.shows_details(opened.id) {
        let mut whom = names(to);
        if !cc.is_empty() {
            whom = tr!("{to}, copy to {cc}", to = whom, cc = names(cc));
        }
        ui.label(RichText::new(tr!("to {whom}", whom = whom)).font(f.caption.id()).color(p.muted));
        return;
    }
    Grid::new(("details", opened.id)).num_columns(2).spacing(vec2(theme.spacing.m, theme.spacing.xs)).show(ui, |ui| {
        let row = |ui: &mut Ui, name: &str, value: String| {
            ui.label(RichText::new(name).font(f.caption.id()).color(p.muted));
            ui.label(RichText::new(value).font(f.caption.id()).color(p.text));
            ui.end_row();
        };
        let full = |people: &[Person]| people.iter().map(Person::full).collect::<Vec<_>>().join(", ");
        row(ui, tr!("From"), full(from));
        if !reply_to.is_empty() {
            row(ui, tr!("Reply to"), full(reply_to));
        }
        row(ui, tr!("To"), full(to));
        if !cc.is_empty() {
            row(ui, tr!("Copy"), full(cc));
        }
        if let Some(date) = date {
            row(ui, tr!("Date"), mailbox.full_date(*date));
        }
    });
}

/// The remote-content banner, while remote pictures of the message are blocked.
fn banner(ui: &mut Ui, theme: &Theme, reader: &Reader, opened: &Opened, drawn: &mut Drawn) {
    let Content::Html(document) = &opened.content else { return };
    if document.remote_blocked == 0 || reader.remote_allowed(opened.id) {
        return;
    }
    let p = &theme.palette;
    Frame::new().fill(p.panel).inner_margin(margin(theme.spacing.m)).corner_radius(theme.spacing.corner).show(ui, |ui| {
        ui.set_width(ui.available_width());
        let count = document.remote_blocked;
        let text = if count == 1 {
            tr!("1 remote picture is not shown. Loading it tells the sender that you opened this message.").to_owned()
        } else {
            tr!("{count} remote pictures are not shown. Loading them tells the sender that you opened this message.", count = count)
        };
        ui.label(RichText::new(text).font(theme.fonts.caption.id()).color(p.text));
        ui.horizontal(|ui| {
            if ui.button(tr!("Load once")).clicked() {
                drawn.asked.push(Asked::Once(opened.id));
            }
            if let Some(sender) = opened.header.from.first()
                && ui.button(tr!("Always for {address}", address = sender.address)).clicked()
            {
                drawn.asked.push(Asked::Always(opened.id));
            }
        });
    });
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
    Frame::new().stroke(Stroke::new(1.0, p.muted)).inner_margin(margin(theme.spacing.s)).corner_radius(theme.spacing.corner).show(ui, |ui| {
        let text = if image.alt.is_empty() { why } else { format!("{why}: {}", image.alt) };
        ui.label(RichText::new(text).font(theme.fonts.caption.id()).color(p.muted));
    });
}

/// Display blocks of an HTML message.
#[expect(clippy::too_many_arguments, reason = "the drawing context of one block")]
fn blocks(
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
    for (index, block) in list.iter().enumerate() {
        let quote = match block {
            Block::Text { quote, .. } | Block::Rule { quote } | Block::Image { quote, .. } | Block::Table { quote, .. } => *quote,
        };
        if quote > 0 && !toggled {
            toggled = true;
            let text = if quotes { tr!("Hide quoted text") } else { tr!("Show quoted text") };
            if ui.button(text).clicked() {
                drawn.asked.push(Asked::Quotes(id));
            }
        }
        if quote == 0 {
            toggled = false;
        } else if !quotes {
            continue;
        }
        let indent = f32::from(quote.min(4)) * theme.spacing.l;
        ui.horizontal(|ui| {
            ui.add_space(indent);
            ui.vertical(|ui| match block {
                Block::Text { kind, spans, .. } => {
                    let (font, prefix) = match kind {
                        TextKind::Heading(level) => (if *level <= 2 { f.title } else { f.heading }, String::new()),
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
                    let colour = if quote > 0 { p.muted } else { p.text };
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
                        let mut format = TextFormat::simple(span_font.id(), if span.link.is_some() { p.accent } else { colour });
                        format.italics = span.italic;
                        if span.underline || span.link.is_some() {
                            format.underline = Stroke::new(1.0, if span.link.is_some() { p.accent } else { colour });
                        }
                        if span.strike {
                            format.strikethrough = Stroke::new(1.0, colour);
                        }
                        if span.bold && span.link.is_none() {
                            format.color = p.text;
                        }
                        run.push(&span.text, format, span.link);
                    }
                    run.show(ui, document, drawn);
                }
                Block::Rule { .. } => {
                    ui.separator();
                }
                Block::Image { image, .. } => picture(ui, theme, reader, id, image, document, textures, drawn),
                Block::Table { rows, .. } if depth < MAX_DEPTH => {
                    Grid::new(("table", id, depth, index)).striped(false).show(ui, |ui| {
                        for row in rows {
                            for cell in row {
                                ui.vertical(|ui| blocks(ui, theme, reader, id, document, &cell.blocks, depth.saturating_add(1), textures, drawn));
                            }
                            ui.end_row();
                        }
                    });
                }
                Block::Table { .. } => {}
            });
        });
        ui.add_space(theme.spacing.s);
    }
}

/// A plain-text body, its quotes folded until the owner opens them.
fn text(ui: &mut Ui, theme: &Theme, reader: &Reader, id: u64, segments: &[Segment], drawn: &mut Drawn) {
    let p = &theme.palette;
    let f = &theme.fonts;
    for segment in segments {
        match segment {
            Segment::Text(text) => {
                ui.label(RichText::new(text).font(f.body.id()).color(p.text));
            }
            Segment::Quote { attribution, text, lines } => {
                let shown = reader.shows_quotes(id);
                let label = if shown {
                    tr!("Hide quoted text").to_owned()
                } else if *lines == 1 {
                    tr!("Show quoted text (1 line)").to_owned()
                } else {
                    tr!("Show quoted text ({count} lines)", count = lines)
                };
                if ui.button(label).clicked() {
                    drawn.asked.push(Asked::Quotes(id));
                }
                if shown {
                    let (bar, inner) = (p.muted, margin(theme.spacing.s));
                    let response = Frame::new().inner_margin(Margin { left: inner.left.saturating_add(4), ..inner }).show(ui, |ui| {
                        if let Some(attribution) = attribution {
                            ui.label(RichText::new(attribution).font(f.caption.id()).color(p.muted));
                        }
                        ui.label(RichText::new(text).font(f.body.id()).color(p.muted));
                    });
                    let rect = response.response.rect;
                    ui.painter().vline(rect.left() + 1.5, rect.y_range(), Stroke::new(3.0, bar));
                }
            }
        }
        ui.add_space(theme.spacing.s);
    }
}

/// The attachment tiles: a file mark, the name and the size, and a Save button.
fn files(ui: &mut Ui, theme: &Theme, id: u64, files: &[File], drawn: &mut Drawn) {
    if files.is_empty() {
        return;
    }
    let p = &theme.palette;
    ui.add_space(theme.spacing.s);
    ui.with_layout(Layout::left_to_right(Align::Min).with_main_wrap(true), |ui| {
        for file in files {
            Frame::new().stroke(Stroke::new(1.0, p.muted)).inner_margin(margin(theme.spacing.s)).corner_radius(theme.spacing.corner).show(ui, |ui| {
                ui.horizontal(|ui| {
                    let side = super::paint::line_height(ui, theme.fonts.body) * 1.5;
                    let (rect, _) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
                    theme.icons.paint(ui.painter(), rect, IconId::File, Style::Regular, p.accent);
                    ui.vertical(|ui| {
                        ui.add(egui::Label::new(RichText::new(&file.name).font(theme.fonts.body.id()).color(p.text)).truncate());
                        ui.label(RichText::new(size_text(file.size)).font(theme.fonts.caption.id()).color(p.muted));
                    });
                    let response = ui.button(tr!("Save"));
                    let spoken = tr!("Save {name}", name = file.name);
                    response.widget_info(|| WidgetInfo::labeled(egui::Role::Button, true, &spoken));
                    if response.clicked() {
                        drawn.asked.push(Asked::Save(id, file.index));
                    }
                });
            });
        }
    });
}

/// One unfolded message.
fn message(ui: &mut Ui, theme: &Theme, mailbox: &Mailbox, reader: &Reader, opened: &Opened, textures: &mut Textures, drawn: &mut Drawn) {
    let p = &theme.palette;
    let line = Stroke::new(1.0, p.line);
    Frame::new().stroke(line).corner_radius(theme.spacing.corner).inner_margin(margin(theme.spacing.l)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        header(ui, theme, mailbox, reader, opened, drawn);
        ui.add_space(theme.spacing.m);
        banner(ui, theme, reader, opened, drawn);
        ui.add_space(theme.spacing.s);
        let say = |ui: &mut Ui, text: &str| {
            ui.label(RichText::new(text).font(theme.fonts.body.id()).color(p.muted));
        };
        match &opened.content {
            Content::Loading => say(ui, tr!("Loading the message…")),
            Content::NotKept => say(ui, tr!("The text of this message is not here yet. It comes with the next connection.")),
            Content::TooLarge => say(ui, tr!("This message is too large to keep, so it cannot be shown.")),
            Content::Unreadable => say(ui, tr!("This message cannot be read.")),
            Content::Text(segments) => text(ui, theme, reader, opened.id, segments, drawn),
            Content::Html(document) => blocks(ui, theme, reader, opened.id, document, &document.blocks, 0, textures, drawn),
        }
        if opened.cut {
            say(ui, tr!("The text is cut here: the rest is longer than the reader shows."));
        }
        files(ui, theme, opened.id, &opened.files, drawn);
    });
}

/// Draws the reader over the whole of `ui` and returns what was asked for.
pub(super) fn show(ui: &mut Ui, theme: &Theme, mailbox: &Mailbox, reader: &Reader, textures: &mut Textures, hover: Option<&str>) -> Drawn {
    let mut drawn = Drawn::default();
    let p = &theme.palette;
    egui::Panel::bottom("reader-status").frame(Frame::new().inner_margin(margin(theme.spacing.s))).show(ui, |ui| {
        ui.horizontal(|ui| match (hover, reader.notice()) {
            (Some(status), _) => {
                let colour = if status.starts_with(tr!("Careful")) { p.error } else { p.muted };
                ui.add(egui::Label::new(RichText::new(status).font(theme.fonts.caption.id()).color(colour)).truncate());
            }
            (None, Some(notice)) => {
                ui.label(RichText::new(notice).font(theme.fonts.caption.id()).color(p.text));
                if ui.button(tr!("Dismiss")).clicked() {
                    drawn.asked.push(Asked::Dismiss);
                }
            }
            (None, None) => {
                ui.label(RichText::new(tr!("Point at a link to see where it goes.")).font(theme.fonts.caption.id()).color(p.muted));
            }
        });
    });
    egui::Panel::top("reader-toolbar").frame(Frame::new().inner_margin(margin(theme.spacing.l))).show(ui, |ui| {
        toolbar(ui, theme, mailbox, reader, &mut drawn);
    });
    ScrollArea::vertical().auto_shrink([false, false]).id_salt("reader").show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = theme.spacing.xs;
        for row in reader.thread() {
            match (&row.opened, reader.is_expanded(row.id)) {
                (Some(opened), true) => message(ui, theme, mailbox, reader, opened, textures, &mut drawn),
                _ => folded(ui, theme, mailbox, row, &mut drawn),
            }
        }
    });
    drawn
}
