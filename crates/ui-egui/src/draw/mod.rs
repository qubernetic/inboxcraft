// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The window's content: the folder pane on the left, the header with the search field and the message list, the
//! reader of the selected message on the right, and an error banner on top while a command's error stands. The
//! reader can fill the window (full width); Escape leaves that.
//!
//! Drawing only reads the [`Mailbox`] and the [`Reader`]; what the person asks for is applied after the frame's
//! drawing, through their methods, which run the engine's commands.

mod list;
mod paint;
mod pane;
mod reader;

use egui::{CentralPanel, Frame, Key, KeyboardShortcut, Margin, Modifiers, Panel, RichText, Ui};

use crate::fetch::{Fetch, Offline};
use crate::mailbox::Mailbox;
use crate::reader::Reader;
use crate::save::{NoSave, Save};
use crate::theme::Theme;
use crate::tr;

/// Focuses the search field.
const FIND: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::F);

/// The width of the list beside the reader, in points.
const LIST_WIDTH: f32 = 380.0;

/// The InboxCraft window's content over a [`Mailbox`].
pub struct InboxApp {
    mailbox: Mailbox,
    reader: Reader,
    textures: reader::Textures,
    hover: Option<String>,
    search: String,
    themed: bool,
}

impl std::fmt::Debug for InboxApp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InboxApp").field("mailbox", &self.mailbox).field("reader", &self.reader).finish_non_exhaustive()
    }
}

fn margin(points: f32) -> Margin {
    // Distances are a few points; the clamp keeps the cast inside i8.
    Margin::same(points.clamp(0.0, 100.0) as i8)
}

impl InboxApp {
    /// The window's content over `mailbox`; it fetches no remote picture and saves no attachment.
    #[must_use]
    pub fn new(mailbox: Mailbox) -> Self {
        Self::with_outside(mailbox, Box::new(Offline::default()), Box::new(NoSave))
    }

    /// The window's content over `mailbox`, fetching the remote pictures the owner allows through `fetch` and saving
    /// attachments through `save`.
    #[must_use]
    pub fn with_outside(mailbox: Mailbox, fetch: Box<dyn Fetch>, save: Box<dyn Save>) -> Self {
        Self { mailbox, reader: Reader::new(fetch, save), textures: reader::Textures::new(), hover: None, search: String::new(), themed: false }
    }

    /// The reader.
    #[must_use]
    pub fn reader(&self) -> &Reader {
        &self.reader
    }

    /// The reader, to allow pictures or fill the window from outside (the `ui_shot` scripts, later the control
    /// channel).
    pub fn reader_mut(&mut self) -> (&mut Reader, &mut Mailbox) {
        self.sync();
        (&mut self.reader, &mut self.mailbox)
    }

    /// Opens the selected message in the reader, or closes it, when the selection changed.
    fn sync(&mut self) {
        if self.reader.open_id() != self.mailbox.selected() {
            let selected = self.mailbox.selected();
            self.reader.open(&mut self.mailbox, selected);
            self.hover = None;
        }
    }

    /// The mailbox shown.
    #[must_use]
    pub fn mailbox(&self) -> &Mailbox {
        &self.mailbox
    }

    /// The mailbox shown, to open, narrow or select from outside (the `ui_shot` scripts, later the control
    /// channel).
    pub fn mailbox_mut(&mut self) -> &mut Mailbox {
        &mut self.mailbox
    }

    /// Puts `text` in the search field and searches for it.
    pub fn search(&mut self, text: &str) {
        text.clone_into(&mut self.search);
        self.mailbox.search(text);
    }

    /// The text in the search field.
    #[must_use]
    pub fn search_text(&self) -> &str {
        &self.search
    }

    /// Draws one frame into `ui` and applies what the person asked for.
    pub fn ui(&mut self, ui: &mut Ui) {
        if !self.themed {
            Theme::apply(ui.ctx());
            self.themed = true;
        }
        let theme = Theme::current(ui.ctx());
        ui.set_style(theme.style());
        let p = theme.palette;
        let s = theme.spacing;
        self.sync();
        if self.reader.poll() {
            ui.ctx().request_repaint();
        }
        if ui.input_mut(|input| input.consume_shortcut(&FIND)) {
            self.reader.set_full_width(false);
            ui.memory_mut(|memory| memory.request_focus(list::search_id()));
        }
        let typing = ui.memory(|memory| memory.focused().is_some());
        let (escape, down, up) =
            ui.input(|input| (input.key_pressed(Key::Escape), input.key_pressed(Key::ArrowDown), input.key_pressed(Key::ArrowUp)));
        let mut stepped = false;
        if !typing && (down || up) {
            self.mailbox.step(down);
            stepped = true;
        }
        let left_full = escape && self.reader.full_width();
        if left_full {
            self.reader.set_full_width(false);
        }
        let mut dismiss = false;
        if let Some(error) = self.mailbox.error() {
            Panel::top("error").frame(Frame::new().fill(p.error).inner_margin(margin(s.s))).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(error).color(p.on_error));
                    dismiss = ui.button(tr!("Dismiss")).clicked();
                });
            });
        }
        let mut asked = pane::Asked::default();
        let mut searched = None;
        let mut clicked = None;
        let full = self.reader.full_width();
        if !full {
            asked = Panel::left("folders")
                .resizable(true)
                .default_size(s.folder_pane)
                .frame(Frame::new().fill(p.panel).inner_margin(margin(s.m)))
                .show(ui, |ui| pane::show(ui, &theme, &self.mailbox))
                .inner;
            let (mailbox, search) = (&self.mailbox, &mut self.search);
            let list = |ui: &mut Ui| {
                let searched = Panel::top("list-header")
                    .frame(Frame::new().fill(p.surface).inner_margin(margin(s.l)))
                    .show(ui, |ui| list::header(ui, &theme, mailbox, search))
                    .inner;
                let clicked = CentralPanel::no_frame().frame(Frame::new().fill(p.surface)).show(ui, |ui| list::rows(ui, &theme, mailbox)).inner;
                (searched, clicked)
            };
            (searched, clicked) = if self.reader.open_id().is_some() {
                Panel::left("list").resizable(true).default_size(LIST_WIDTH).frame(Frame::new().fill(p.surface)).show(ui, list).inner
            } else {
                CentralPanel::no_frame().show(ui, list).inner
            };
        }
        let mut drawn = reader::Drawn::default();
        if self.reader.open_id().is_some() {
            let hover = self.hover.take();
            drawn = CentralPanel::no_frame()
                .frame(Frame::new().fill(p.background))
                .show(ui, |ui| reader::show(ui, &theme, &self.mailbox, &self.reader, &mut self.textures, hover.as_deref()))
                .inner;
            if drawn.hover != hover {
                ui.ctx().request_repaint();
            }
            self.hover = drawn.hover.take();
        }
        let acted = dismiss
            || stepped
            || left_full
            || asked.narrow.is_some()
            || asked.open.is_some()
            || searched.is_some()
            || clicked.is_some()
            || !drawn.asked.is_empty();
        if acted {
            // What changed shows in the next frame.
            ui.ctx().request_repaint();
        }
        for asked in drawn.asked {
            self.apply(ui, asked);
        }
        if dismiss {
            self.mailbox.dismiss_error();
        }
        if let Some(account) = asked.narrow {
            self.mailbox.narrow(account);
        }
        if let Some(place) = asked.open {
            self.mailbox.open(place);
            self.search.clear();
        }
        if let Some(query) = searched {
            self.mailbox.search(&query);
        }
        if clicked.is_some() {
            self.mailbox.select(clicked);
        }
        self.sync();
    }

    /// Applies what was asked for in the reader.
    fn apply(&mut self, ui: &Ui, asked: reader::Asked) {
        let (reader, mailbox) = (&mut self.reader, &mut self.mailbox);
        match asked {
            reader::Asked::Expand(id) => reader.expand(mailbox, id),
            reader::Asked::Details(id) => reader.toggle_details(id),
            reader::Asked::Quotes(id) => reader.toggle_quotes(id),
            reader::Asked::Once(id) => reader.allow_once(mailbox, id),
            reader::Asked::Always(id) => reader.allow_always(mailbox, id),
            reader::Asked::Save(id, index) => reader.save(mailbox, id, index),
            reader::Asked::FullWidth(full) => reader.set_full_width(full),
            reader::Asked::Close => mailbox.select(None),
            reader::Asked::After(after) => mailbox.set_after(after),
            reader::Asked::Copy(url) => {
                reader.set_notice(tr!("Link copied: {url}", url = crate::mailbox::shown(&url)));
                ui.ctx().copy_text(url);
            }
            reader::Asked::Dismiss => reader.dismiss_notice(),
        }
    }
}

#[cfg(test)]
#[path = "reader_tests.rs"]
mod reader_tests;
#[cfg(test)]
mod tests;
