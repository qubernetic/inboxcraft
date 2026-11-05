// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The window's content: the folder pane on the left, the header with the search field and the message list on the
//! right, and an error banner on top while a command's error stands.
//!
//! Drawing only reads the [`Mailbox`]; what the person asks for is applied after the frame's drawing, through the
//! mailbox's methods, which run the engine's commands.

mod list;
mod paint;
mod pane;

use egui::{CentralPanel, Frame, Key, KeyboardShortcut, Margin, Modifiers, Panel, RichText, Ui};

use crate::mailbox::Mailbox;
use crate::theme::Theme;
use crate::tr;

/// Focuses the search field.
const FIND: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::F);

/// The InboxCraft window's content over a [`Mailbox`].
#[derive(Debug)]
pub struct InboxApp {
    mailbox: Mailbox,
    search: String,
    themed: bool,
}

fn margin(points: f32) -> Margin {
    // Distances are a few points; the clamp keeps the cast inside i8.
    Margin::same(points.clamp(0.0, 100.0) as i8)
}

impl InboxApp {
    /// The window's content over `mailbox`.
    #[must_use]
    pub fn new(mailbox: Mailbox) -> Self {
        Self { mailbox, search: String::new(), themed: false }
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
        if ui.input_mut(|input| input.consume_shortcut(&FIND)) {
            ui.memory_mut(|memory| memory.request_focus(list::search_id()));
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
        let asked = Panel::left("folders")
            .resizable(true)
            .default_size(s.folder_pane)
            .frame(Frame::new().fill(p.panel).inner_margin(margin(s.m)))
            .show(ui, |ui| pane::show(ui, &theme, &self.mailbox))
            .inner;
        let searched = Panel::top("list-header")
            .frame(Frame::new().fill(p.surface).inner_margin(margin(s.l)))
            .show(ui, |ui| list::header(ui, &theme, &self.mailbox, &mut self.search))
            .inner;
        let clicked = CentralPanel::no_frame().frame(Frame::new().fill(p.surface)).show(ui, |ui| list::rows(ui, &theme, &self.mailbox)).inner;
        let acted = dismiss || asked.narrow.is_some() || asked.open.is_some() || searched.is_some() || clicked.is_some();
        if acted {
            // What changed shows in the next frame.
            ui.ctx().request_repaint();
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
    }
}

#[cfg(test)]
mod tests;
