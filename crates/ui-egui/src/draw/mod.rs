// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The window's content: the folder pane on the left, the header with the search field and the message list, the
//! reader of the selected message on the right, and an error banner on top while a command's error stands. The
//! reader can fill the window (full width); Escape leaves that. A window that can add accounts shows "Add account"
//! in the folder pane and in the empty list; the add-account page then takes the place of the list and the reader.
//!
//! Each frame the window hands the engine's jobs to the `net` workers and applies their events
//! ([`Mailbox::pump`]), so the steps of an account check show as they come.
//!
//! Drawing only reads the [`Mailbox`] and the [`Reader`]; what the person asks for is applied after the frame's
//! drawing, through their methods, which run the engine's commands.

mod list;
mod onboard;
mod paint;
mod pane;
mod reader;

use std::sync::Arc;

use egui::{CentralPanel, Frame, Key, KeyboardShortcut, Margin, Modifiers, Panel, RichText, Ui};

use crate::fetch::{Fetch, Offline};
use crate::glass::{self, Layers};
use crate::gradient;
use crate::mailbox::Mailbox;
use crate::onboard::{Detect, Keychain, Onboarding, Stage};
use crate::reader::{PictureState, Reader};
use crate::save::{NoSave, Save};
use crate::theme::{self, ModePreference, Preset, TextScale, Theme};
use crate::tr;
use crate::workers::{NoWorkers, Workers};

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
    /// The mode the owner chose.
    mode: ModePreference,
    /// The accent preset the colours come from.
    preset: Preset,
    /// The text scale every font token is at.
    text_scale: TextScale,
    /// The mode, preset and text scale installed in the context, once installed.
    themed: Option<(ModePreference, Preset, TextScale)>,
    onboarding: Onboarding,
    /// Whether the window offers to add an account.
    can_add: bool,
    workers: Box<dyn Workers>,
}

impl std::fmt::Debug for InboxApp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InboxApp")
            .field("mailbox", &self.mailbox)
            .field("reader", &self.reader)
            .field("onboarding", &self.onboarding)
            .finish_non_exhaustive()
    }
}

fn margin(points: f32) -> Margin {
    // Distances are a few points; the clamp keeps the cast inside i8.
    Margin::same(points.clamp(0.0, 100.0) as i8)
}

/// A pane's frame: no fill, so the colour fields show `left` and `right` of its glass layer and `gap` above and
/// below it.
fn around(left: f32, right: f32, gap: f32) -> Frame {
    // Distances are a few points; the clamp keeps the casts inside i8.
    let side = |points: f32| points.clamp(0.0, 100.0) as i8;
    Frame::new().inner_margin(Margin { left: side(left), right: side(right), top: side(gap), bottom: side(gap) })
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
        Self {
            mailbox,
            reader: Reader::new(fetch, save),
            textures: reader::Textures::new(),
            hover: None,
            search: String::new(),
            mode: ModePreference::default(),
            preset: Preset::default(),
            text_scale: TextScale::default(),
            themed: None,
            onboarding: Onboarding::default(),
            can_add: false,
            workers: Box::new(NoWorkers),
        }
    }

    /// The same window, offering to add accounts: their passwords go into `keychain`, Detect runs through `detect`,
    /// and the engine's jobs go to `workers`, whose events come back each frame.
    #[must_use]
    pub fn with_accounts(mut self, keychain: Arc<dyn Keychain>, detect: Box<dyn Detect>, workers: Box<dyn Workers>) -> Self {
        self.onboarding = Onboarding::new(keychain, detect);
        self.can_add = true;
        self.workers = workers;
        self
    }

    /// Whether the window offers to add an account.
    #[must_use]
    pub fn can_add(&self) -> bool {
        self.can_add
    }

    /// The add-account page.
    #[must_use]
    pub fn onboarding(&self) -> &Onboarding {
        &self.onboarding
    }

    /// The add-account page and the mailbox, to fill in or press from outside (the `ui_shot` scripts, later the
    /// control channel).
    pub fn onboarding_mut(&mut self) -> (&mut Onboarding, &mut Mailbox) {
        (&mut self.onboarding, &mut self.mailbox)
    }

    /// Hands the engine's jobs to the workers and applies their events; the check's steps are read again when
    /// anything changed. Whether anything changed.
    pub fn pump(&mut self) -> bool {
        let changed = self.mailbox.pump(&mut *self.workers);
        if changed {
            self.onboarding.load(&mut self.mailbox);
        }
        changed
    }

    /// For the window's end: removes the passwords of the accounts added in it from the credential store.
    pub fn on_exit(&mut self) {
        self.onboarding.forget_passwords();
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

    /// The mode the owner chose.
    #[must_use]
    pub fn mode(&self) -> ModePreference {
        self.mode
    }

    /// Follows the system's mode, or always shows light or dark, from the next frame.
    pub fn set_mode(&mut self, mode: ModePreference) {
        self.mode = mode;
    }

    /// The accent preset the colours come from.
    #[must_use]
    pub fn preset(&self) -> &Preset {
        &self.preset
    }

    /// Takes the colours from `preset`, from the next frame.
    pub fn set_preset(&mut self, preset: Preset) {
        self.preset = preset;
    }

    /// The text scale every font token is at.
    #[must_use]
    pub fn text_scale(&self) -> TextScale {
        self.text_scale
    }

    /// Draws every text at `scale`, from the next frame; the rows, headers and toolbars follow the font.
    pub fn set_text_scale(&mut self, scale: TextScale) {
        self.text_scale = scale;
    }

    /// Installs the fonts once, and the chosen preset, mode and text scale in the context when they changed.
    fn install_theme(&mut self, ctx: &egui::Context) {
        let chosen = (self.mode, self.preset, self.text_scale);
        if self.themed == Some(chosen) {
            return;
        }
        if self.themed.is_none() {
            theme::install_fonts(ctx);
        }
        Theme::apply(ctx, &self.preset, self.text_scale);
        // egui follows the system by itself; a first frame that follows it leaves the context's mode alone.
        if self.themed.is_some() || self.mode != ModePreference::System {
            ctx.set_theme(egui::ThemePreference::from(self.mode));
        }
        self.themed = Some(chosen);
    }

    /// Draws one frame into `ui` and applies what the person asked for.
    pub fn ui(&mut self, ui: &mut Ui) {
        self.install_theme(ui.ctx());
        let theme = Theme::current(ui.ctx(), &self.preset, self.text_scale);
        ui.set_style(theme.style());
        let p = theme.palette;
        let s = theme.spacing;
        self.sync();
        if self.reader.poll() | self.pump() | self.onboarding.poll() {
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
        let mut page = None;
        let adding = self.onboarding.stage() != Stage::Closed;
        let full = self.reader.full_width() && !adding;
        // The colour fields first, then the place for the glass layers, then everything else.
        glass::backdrop(ui.painter(), &theme, ui.max_rect());
        let mut layers = Layers::begin(ui.painter());
        let (gap, half) = (s.gap, s.gap / 2.0);
        if adding {
            asked = Panel::left("folders")
                .resizable(true)
                .default_size(s.folder_pane)
                .show_separator_line(false)
                .frame(around(gap, half, gap))
                .show(ui, |ui| layers.card(ui, margin(s.m), |ui| pane::show(ui, &theme, &self.mailbox, false)))
                .inner;
            let (mailbox, onboarding) = (&self.mailbox, &mut self.onboarding);
            page = CentralPanel::no_frame()
                .frame(around(half, gap, gap))
                .show(ui, |ui| layers.card(ui, margin(s.xl), |ui| onboard::show(ui, &theme, mailbox, onboarding)))
                .inner;
        } else if !full {
            asked = Panel::left("folders")
                .resizable(true)
                .default_size(s.folder_pane)
                .show_separator_line(false)
                .frame(around(gap, half, gap))
                .show(ui, |ui| layers.card(ui, margin(s.m), |ui| pane::show(ui, &theme, &self.mailbox, self.can_add)))
                .inner;
            let (mailbox, search, can_add) = (&self.mailbox, &mut self.search, self.can_add);
            let mut list = |ui: &mut Ui, layers: &mut Layers| {
                layers.card(ui, Margin::ZERO, |ui| {
                    let searched = Panel::top("list-header")
                        .frame(Frame::new().inner_margin(margin(s.l)))
                        .show(ui, |ui| list::header(ui, &theme, mailbox, search))
                        .inner;
                    let (clicked, add) = CentralPanel::no_frame().show(ui, |ui| list::rows(ui, &theme, mailbox, can_add)).inner;
                    (searched, clicked, add)
                })
            };
            let add;
            (searched, clicked, add) = if self.reader.open_id().is_some() {
                Panel::left("list")
                    .resizable(true)
                    .default_size(LIST_WIDTH)
                    .show_separator_line(false)
                    .frame(around(half, half, gap))
                    .show(ui, |ui| list(ui, &mut layers))
                    .inner
            } else {
                CentralPanel::no_frame().frame(around(half, gap, gap)).show(ui, |ui| list(ui, &mut layers)).inner
            };
            asked.add |= add;
        }
        let mut drawn = reader::Drawn::default();
        // A texture lives no longer than the picture the reader keeps for it.
        self.textures.retain(|key, _| matches!(self.reader.picture(key), Some(PictureState::Ready(_))));
        if self.reader.open_id().is_some() && !adding {
            let hover = self.hover.take();
            let left = if full { gap } else { half };
            drawn = CentralPanel::no_frame()
                .frame(around(left, gap, gap))
                .show(ui, |ui| {
                    layers.card(ui, Margin::ZERO, |ui| reader::show(ui, &theme, &self.mailbox, &self.reader, &mut self.textures, hover.as_deref()))
                })
                .inner;
            if drawn.hover != hover {
                ui.ctx().request_repaint();
            }
            self.hover = drawn.hover.take();
        }
        layers.finish(ui.painter(), &theme);
        // Every gradient of the frame is painted by now: the pictures it left out go.
        gradient::sweep(ui.ctx());
        let acted = dismiss
            || stepped
            || left_full
            || asked.narrow.is_some()
            || asked.open.is_some()
            || searched.is_some()
            || clicked.is_some()
            || asked.add
            || page.is_some()
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
        if asked.add {
            self.onboarding.open();
        }
        if let Some(page) = page {
            self.apply_page(page);
        }
        self.sync();
    }

    /// Applies what was asked for on the add-account page.
    fn apply_page(&mut self, asked: onboard::Asked) {
        let (onboarding, mailbox) = (&mut self.onboarding, &mut self.mailbox);
        match asked {
            onboard::Asked::Detect => onboarding.detect(),
            onboard::Asked::Add => onboarding.add(mailbox),
            onboard::Asked::CheckAgain => onboarding.check_again(mailbox),
            onboard::Asked::Change => onboarding.change(mailbox),
            onboard::Asked::Close => onboarding.close(),
        }
        // The new account's jobs go out in this frame.
        self.pump();
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
#[path = "fit_tests.rs"]
mod fit_tests;
#[cfg(test)]
#[path = "onboard_tests.rs"]
mod onboard_tests;
#[cfg(test)]
#[path = "reader_tests.rs"]
mod reader_tests;
#[cfg(test)]
mod tests;
