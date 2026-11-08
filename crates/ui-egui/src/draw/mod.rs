// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The window's content: the folder pane on the left, the header with the search field and the message list, the
//! reader of the selected message on the right, and an error banner on top while a command's error stands. The
//! reader can fill the window (full width); Escape leaves that. A window that can add accounts shows "Add account"
//! in the empty window, which opens the settings sheet on Accounts; accounts are added and listed there.
//! The navigation rail runs down the window's left edge in the preset's gradient: its settings button (or its key)
//! opens the settings sheet over the window; Escape, its close button or the rail's Mail button leave it, and an
//! account being added with it.
//!
//! The hover actions, the reader's toolbar and their keys (D Done, A Archive, R read or unread, S flag, Delete) act on
//! the hovered or selected message, on a real account as on the sample; a row that leaves the list slides out first.
//! A toast then says what was done, with Undo and its key (Ctrl+Z) for [`TOAST_SECONDS`]; after it Ctrl+Z walks back
//! through the engine's history one act at a time. While a text field has the focus the keys are the field's.
//!
//! Each frame the window hands the engine's jobs to the `net` workers and applies their events
//! ([`Mailbox::pump`]), so the steps of an account check show as they come.
//!
//! Drawing only reads the [`Mailbox`] and the [`Reader`]; what the person asks for is applied after the frame's
//! drawing, through their methods, which run the engine's commands.
//!
//! The look comes from the session's appearance choices (the `ui.*.set` commands, from the window or from outside).
//! A frame that finds them changed installs them and hands them to the window's [`Keep`], so they are there again
//! after a restart; [`InboxApp::with_keep`] reads them back at the start. The keys are kept the same way.
//!
//! Every action of the window has an id ([`keymap::actions`]) and runs through [`InboxApp::run_action`]: from its
//! key, from the rail and from the command palette (Ctrl+K), which lists them all. The rail's Calendar shows the
//! calendar screen ([`Screen::Calendar`]), drawn in `calendar/`, in the place of the mail.

mod accounts;
mod calendar;
mod compose;
mod frame;
mod list;
mod onboard;
mod paint;
mod palette;
mod pane;
mod reader;
mod settings;

use std::sync::Arc;

use egui::{Align, CentralPanel, Frame, Key, Layout, Margin, Panel, Rect, RichText, Ui, UiBuilder};
use inboxcraft_engine::appearance::{Accent, Appearance, CustomAccent, Icons, Motion, Rgb, WindowFrame};
use inboxcraft_model::Role;
use serde_json::{Value, json};

use crate::calendar::Calendar;
use crate::component::toast::{self, Toast};
use crate::compose::Composer;
use crate::fetch::{Fetch, Offline};
use crate::glass::{self, Layers};
use crate::gradient;
use crate::iconset::IconSet;
use crate::keep::{Keep, NoKeep, Triage};
use crate::keymap::{self, Captured, Keymap};
use crate::mailbox::{Act, Mailbox, Place};
use crate::motion;
use crate::onboard::{Detect, Keychain, Onboarding, Stage};
use crate::palette::Palette;
use crate::reader::{PictureState, Reader};
use crate::save::{NoSave, Save};
use crate::settings::{Section, Settings};
use crate::theme::{self, Look, ModePreference, Preset, TextScale, Theme, Typeface};
use crate::workers::{NoWorkers, Workers};
use crate::{UiError, tr};

/// The width of the list beside the reader, in points.
const LIST_WIDTH: f32 = 380.0;

/// The widest the card of the window with no account gets, in points.
const WELCOME_WIDTH: f32 = 640.0;

/// How long the toast after an act shows, in seconds; its Undo works until then.
pub const TOAST_SECONDS: f64 = 6.0;

/// The screen the window shows beside the rail.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Screen {
    /// The folders, the list and the reader.
    #[default]
    Mail,
    /// The calendar.
    Calendar,
}

/// The keys of a frame.
struct Keys {
    /// The actions whose keys were pressed.
    ran: Vec<&'static str>,
    escape: bool,
    down: bool,
    up: bool,
    /// Whether the mail's own keys rest.
    typing: bool,
}

/// The acts on a row, which run on the selected message.
const ROW_ACTS: [(&str, Act); 5] =
    [(keymap::DONE, Act::Done), (keymap::ARCHIVE, Act::Archive), (keymap::READ, Act::Read), (keymap::FLAG, Act::Flag), (keymap::DELETE, Act::Delete)];

/// The InboxCraft window's content over a [`Mailbox`].
pub struct InboxApp {
    mailbox: Mailbox,
    reader: Reader,
    textures: reader::Textures,
    hover: Option<String>,
    search: String,
    /// The appearance installed in the context, once installed.
    themed: Option<Appearance>,
    /// Where the appearance is kept between runs.
    keep: Box<dyn Keep>,
    /// The appearance the keep holds.
    kept: Appearance,
    /// Which key runs which action.
    keymap: Keymap,
    /// The keys the keep holds.
    kept_keys: Keymap,
    /// The triage choices the keep holds.
    kept_triage: Triage,
    /// The screen shown beside the rail.
    screen: Screen,
    /// The calendar screen.
    calendar: Calendar,
    /// The command palette.
    palette: Palette,
    onboarding: Onboarding,
    /// The appearance settings page.
    settings: Settings,
    /// Whether the window offers to add an account.
    can_add: bool,
    workers: Box<dyn Workers>,
    /// The act the toast shows (by its serial) and when the toast came.
    toast: Option<(u64, f64)>,
    /// Whether the window is a native one whose frame the app may draw (`set_own_frame`).
    own_frame: bool,
    /// The frame the window was last told to show.
    frame_applied: Option<WindowFrame>,
    /// The message being written, in the reader's place or floating.
    composer: Option<Composer>,
    /// What the composer's last step says in a toast (the draft kept, the message on its way), and since when.
    notice: Option<(String, Option<f64>)>,
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

/// One card in the middle of the window, as tall as what `add` draws in it; what `add` gave.
fn middle_card<R>(ui: &mut Ui, theme: &Theme, layers: &mut Layers, mut add: impl FnMut(&mut Ui) -> R) -> R {
    let s = theme.spacing;
    CentralPanel::no_frame()
        .frame(around(s.gap, s.gap, s.gap))
        .show(ui, |ui| {
            let room = ui.max_rect();
            let pad = s.xl * 2.0;
            let width = WELCOME_WIDTH.min(room.width());
            let inside = Rect::from_min_size(room.min, egui::vec2((width - 2.0 * pad).max(1.0), room.height()));
            let mut sizing = ui.new_child(UiBuilder::new().max_rect(inside).layout(Layout::top_down(Align::Center)).sizing_pass().invisible());
            add(&mut sizing);
            let height = (sizing.min_rect().height() + 2.0 * pad).min(room.height());
            let card = Rect::from_center_size(room.center(), egui::vec2(width, height));
            ui.scope_builder(UiBuilder::new().max_rect(card).layout(Layout::top_down(Align::Center)), |ui| layers.card(ui, margin(pad), add)).inner
        })
        .inner
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
            themed: None,
            keep: Box::new(NoKeep),
            kept: Appearance::default(),
            keymap: Keymap::default(),
            kept_keys: Keymap::default(),
            kept_triage: Triage::default(),
            screen: Screen::default(),
            calendar: Calendar::default(),
            palette: Palette::default(),
            onboarding: Onboarding::default(),
            settings: Settings::default(),
            can_add: false,
            workers: Box::new(NoWorkers),
            toast: None,
            own_frame: false,
            frame_applied: None,
            composer: None,
            notice: None,
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

    /// The same window, keeping the appearance choices and the keys in `keep`: the ones kept there from an earlier run
    /// are the window's from now on, and every change is kept there from the frame it shows in.
    #[must_use]
    pub fn with_keep(mut self, keep: Box<dyn Keep>) -> Self {
        if let Some(kept) = keep.load() {
            self.mailbox.restore_appearance(kept);
            self.kept = kept;
        } else {
            self.kept = *self.mailbox.appearance();
        }
        if let Some(keys) = keep.load_keys() {
            self.keymap = keys;
        }
        self.kept_keys = self.keymap.clone();
        let sample = self.mailbox.is_sample();
        match keep.load_triage() {
            Some(kept) => {
                // The sample window takes the look of the inbox, never the screener's mode of the owner's mail.
                if !sample {
                    self.mailbox.restore_screener(kept.screener);
                }
                self.mailbox.set_inbox_view(kept.view);
                self.kept_triage = kept;
            }
            None => {
                let screener = if sample { Triage::default().screener } else { self.mailbox.screener_mode() };
                self.kept_triage = Triage { screener, view: self.mailbox.inbox_view() };
            }
        }
        self.keep = keep;
        self
    }

    /// The triage choices to keep: the inbox view, and the screener's mode but on the sample, which keeps the one kept.
    fn triage(&self) -> Triage {
        let screener = if self.mailbox.is_sample() { self.kept_triage.screener } else { self.mailbox.screener_mode() };
        Triage { screener, view: self.mailbox.inbox_view() }
    }

    /// Keeps the triage choices when they changed.
    fn keep_triage(&mut self) {
        let triage = self.triage();
        if triage == self.kept_triage {
            return;
        }
        // Tried once per change: the next change tries again.
        self.kept_triage = triage;
        if let Err(error) = self.keep.store_triage(&triage) {
            self.mailbox.fail(UiError::Keep(error));
        }
    }

    /// The screen shown beside the rail.
    #[must_use]
    pub fn screen(&self) -> Screen {
        self.screen
    }

    /// The calendar screen.
    #[must_use]
    pub fn calendar(&self) -> &Calendar {
        &self.calendar
    }

    /// The command palette.
    #[must_use]
    pub fn palette(&self) -> &Palette {
        &self.palette
    }

    /// The command palette, to type in from outside (the `ui_shot` scripts, later the control channel).
    pub fn palette_mut(&mut self) -> &mut Palette {
        &mut self.palette
    }

    /// The act `action` (Done, Archive, read or unread, flag or not, Delete) on the selected row.
    fn row_act(&self, action: &str) -> Option<(u64, Act)> {
        let row = self.mailbox.selected().and_then(|id| self.mailbox.messages().iter().find(|row| row.id == id))?;
        let (_, act) = ROW_ACTS.into_iter().find(|(id, _)| *id == action)?;
        let act = match act {
            Act::Read if row.seen => Act::Unread,
            Act::Flag if row.flagged => Act::Unflag,
            act => act,
        };
        Some((row.id, act))
    }

    /// Whether the action `id` can run now: an act on a row needs a selected message, Undo an act in the history, the
    /// reader's full width an open message; an id the window does not know never runs.
    #[must_use]
    pub fn can_run(&self, id: &str) -> bool {
        match id {
            keymap::DONE | keymap::ARCHIVE | keymap::READ | keymap::FLAG | keymap::DELETE => self.row_act(id).is_some(),
            keymap::UNDO => self.mailbox.can_undo(),
            keymap::FULL_WIDTH => self.reader.open_id().is_some(),
            keymap::COMPOSE => !self.mailbox.accounts().is_empty(),
            keymap::SEND => self.docked(),
            _ => keymap::action(id).is_some(),
        }
    }

    /// Runs the action `id` of [`keymap::actions`], as its key, the rail or the palette do; an action that cannot run
    /// now ([`InboxApp::can_run`]) does nothing.
    pub fn run_action(&mut self, ctx: &egui::Context, id: &str) {
        if !self.can_run(id) {
            return;
        }
        match id {
            keymap::PALETTE => {
                self.close_settings();
                self.palette.open();
            }
            keymap::FIND => {
                self.show_mail();
                self.reader.set_full_width(false);
                ctx.memory_mut(|memory| memory.request_focus(list::search_id()));
            }
            keymap::MAIL => {
                self.show_mail();
                if self.mailbox.place() == Place::View(Role::Sent) {
                    self.mailbox.open(Place::View(Role::Inbox));
                }
            }
            keymap::SENT => {
                self.show_mail();
                self.mailbox.open(Place::View(Role::Sent));
                self.search.clear();
            }
            keymap::CALENDAR => self.show_calendar(),
            keymap::COMPOSE => {
                self.compose();
                ctx.memory_mut(|memory| memory.request_focus(compose::field_id(crate::compose::Line::To)));
            }
            keymap::SEND => self.send_composer(),
            keymap::SETTINGS => {
                if !self.settings.is_open() {
                    self.open_settings();
                }
            }
            keymap::SHORTCUTS => self.open_shortcuts(),
            keymap::UNDO => self.mailbox.undo(),
            keymap::FULL_WIDTH => {
                self.show_mail();
                let full = !self.reader.full_width();
                self.reader.set_full_width(full);
            }
            keymap::LIGHT => self.set_mode(ModePreference::Light),
            keymap::DARK => self.set_mode(ModePreference::Dark),
            keymap::SYSTEM_MODE => self.set_mode(ModePreference::System),
            keymap::MOTION => {
                let reduce = !self.reduce_motion();
                self.set_reduce_motion(reduce);
            }
            _ => {
                if let Some((row, act)) = self.row_act(id) {
                    self.mailbox.start(row, act);
                }
            }
        }
    }

    /// Shows the calendar screen, leaving the settings sheet; its calendars and events are read again.
    pub fn show_calendar(&mut self) {
        self.close_settings();
        self.screen = Screen::Calendar;
        self.calendar.refresh(&mut self.mailbox);
    }

    /// The calendar screen, to pick a view or a day from outside (the `ui_shot` scripts).
    pub fn calendar_mut(&mut self) -> (&mut Calendar, &mut Mailbox) {
        (&mut self.calendar, &mut self.mailbox)
    }

    /// Applies what was pressed on the calendar screen.
    fn apply_calendar(&mut self, asked: Vec<calendar::Asked>) {
        let (calendar, mailbox) = (&mut self.calendar, &mut self.mailbox);
        for asked in asked {
            match asked {
                calendar::Asked::View(view) => calendar.set_view(mailbox, view),
                calendar::Asked::Step(forward) => calendar.step(mailbox, forward),
                calendar::Asked::Today => calendar.go_today(mailbox),
                calendar::Asked::Month(forward) => calendar.step_month(forward),
                calendar::Asked::Go(day) => calendar.go_to(mailbox, day),
                calendar::Asked::Open(day) => calendar.open_day(mailbox, day),
                calendar::Asked::Toggle(id) => calendar.toggle(id),
                calendar::Asked::Pick(picked) => calendar.pick(picked),
            }
        }
    }

    /// Opens the settings sheet on Shortcuts, closing the add-account form.
    pub fn open_shortcuts(&mut self) {
        self.onboarding.close();
        self.settings.open_at(Section::Shortcuts, self.mailbox.appearance());
    }

    /// Shows the mail, leaving the settings sheet and the calendar.
    fn show_mail(&mut self) {
        self.close_settings();
        self.screen = Screen::Mail;
    }

    /// Gives the action waiting on the Shortcuts page the key pressed in this frame, taking it from the action that had
    /// it, or stops the wait on Escape. The press reaches nothing else.
    fn capture_key(&mut self, ctx: &egui::Context) {
        let Some(waiting) = self.settings.capturing() else { return };
        match keymap::captured(ctx) {
            Some(Captured::Cancel) => self.settings.captured(None),
            Some(Captured::Key(shortcut)) => {
                let had = self.keymap.action_of(shortcut).filter(|other| *other != waiting).and_then(keymap::action);
                self.keymap.bind(waiting, shortcut);
                let note =
                    had.map(|had| tr!("{key} was the key of {action}; it has none now.", key = ctx.format_shortcut(&shortcut), action = had.name));
                self.settings.captured(note);
            }
            None => {}
        }
    }

    /// The keys of the frame, read before its drawing. While the Shortcuts page waits for a key or the palette is open,
    /// the keys are theirs; the settings sheet, the palette and the calendar lie over the list or in its place, so the
    /// mail's keys (the arrows, the acts on a row) rest while they show.
    fn keys(&mut self, ctx: &egui::Context) -> Keys {
        let held = self.settings.capturing().is_some() || self.palette.is_open();
        self.capture_key(ctx);
        let focused = ctx.memory(|memory| memory.focused().is_some());
        // The composer in the reader's place hides the selected message: the keys that act on it rest.
        let typing = focused || held || self.settings.is_open() || self.screen == Screen::Calendar || self.docked();
        if held {
            return Keys { ran: Vec::new(), escape: false, down: false, up: false, typing };
        }
        let ran = self.pressed(ctx, focused || self.settings.is_open() || self.docked());
        let (escape, down, up) =
            ctx.input(|input| (input.key_pressed(Key::Escape), input.key_pressed(Key::ArrowDown), input.key_pressed(Key::ArrowUp)));
        Keys { ran, escape, down, up, typing }
    }

    /// The actions whose key was pressed in this frame, besides the acts on a row (the list takes those) and Undo while
    /// a toast with Undo shows (the toast takes it). While `quiet` (a field has the focus, or the sheet is open) a key
    /// without Command, Ctrl or Alt runs nothing, so a letter typed is typed, and Undo is the field's.
    fn pressed(&self, ctx: &egui::Context, quiet: bool) -> Vec<&'static str> {
        let toasted = self.mailbox.undoable().is_some_and(|undo| undo.undoable);
        keymap::actions()
            .into_iter()
            .map(|action| action.id)
            .filter(|id| !ROW_ACTS.iter().any(|(act, _)| act == id))
            .filter(|id| *id != keymap::UNDO || !(quiet || toasted))
            .filter(|id| {
                let bare = self.keymap.shortcut(id).is_some_and(|key| {
                    let held = key.modifiers;
                    !(held.command || held.ctrl || held.mac_cmd || held.alt)
                });
                !(quiet && bare) && self.keymap.pressed(ctx, id)
            })
            .collect()
    }

    /// Keeps the keys when they changed.
    fn keep_keys(&mut self) {
        if self.keymap == self.kept_keys {
            return;
        }
        match self.keep.store_keys(&self.keymap) {
            Ok(()) => self.kept_keys = self.keymap.clone(),
            Err(error) => {
                // Tried once per change: the next change tries again.
                self.kept_keys = self.keymap.clone();
                self.mailbox.fail(UiError::Keep(error));
            }
        }
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

    /// The appearance settings page.
    #[must_use]
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// The appearance settings page, to type a custom accent from outside (the `ui_shot` scripts, later the control
    /// channel).
    pub fn settings_mut(&mut self) -> &mut Settings {
        &mut self.settings
    }

    /// Opens the settings sheet on Appearance, closing the add-account form.
    pub fn open_settings(&mut self) {
        self.onboarding.close();
        self.settings.open(self.mailbox.appearance());
    }

    /// Opens the settings sheet on Accounts, where accounts are added.
    pub fn open_accounts(&mut self) {
        self.settings.open_at(Section::Accounts, self.mailbox.appearance());
    }

    /// Closes the settings sheet and the add-account form on it.
    pub fn close_settings(&mut self) {
        self.onboarding.close();
        self.settings.close();
    }

    /// The add-account form and the check run on the sheet's Accounts page: opening the form opens it there.
    fn follow_adding(&mut self) {
        let adding = self.onboarding.stage() != Stage::Closed;
        if adding && !(self.settings.is_open() && self.settings.section() == Section::Accounts) {
            self.open_accounts();
        }
    }

    /// Hands the engine's jobs to the workers and applies their events; the check's steps are read again when
    /// anything changed. Whether anything changed.
    pub fn pump(&mut self) -> bool {
        let changed = self.mailbox.pump(&mut *self.workers);
        if changed {
            self.onboarding.load(&mut self.mailbox);
            if self.screen == Screen::Calendar {
                self.calendar.refresh(&mut self.mailbox);
            }
        }
        changed
    }

    /// For the end of a window over a store that is not kept: removes the passwords of the accounts added in it from
    /// the credential store. A window over a kept store does not call it; a password stays until its account goes.
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

    /// Opens the selected message in the reader, or closes it, when the selection changed; a message selected while
    /// the composer has the reader's place sends the composer to its floating panel.
    fn sync(&mut self) {
        if self.reader.open_id() != self.mailbox.selected() {
            let selected = self.mailbox.selected();
            if selected.is_some()
                && let Some(composer) = &mut self.composer
            {
                composer.set_floating(true);
            }
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

    /// The message being written, if there is one.
    #[must_use]
    pub fn composer(&self) -> Option<&Composer> {
        self.composer.as_ref()
    }

    /// The message being written and the mailbox, to fill it in from outside (the `ui_shot` scripts, later the
    /// control channel).
    pub fn composer_mut(&mut self) -> (Option<&mut Composer>, &mut Mailbox) {
        (self.composer.as_mut(), &mut self.mailbox)
    }

    /// Whether the composer has the reader's place.
    fn docked(&self) -> bool {
        self.screen == Screen::Mail && self.composer.as_ref().is_some_and(|composer| !composer.floating())
    }

    /// Writes a new message in the reader's place (`compose.new`), or brings back the one being written: there is one
    /// at a time.
    pub fn compose(&mut self) {
        self.show_mail();
        self.reader.set_full_width(false);
        if let Some(composer) = &mut self.composer {
            composer.set_floating(false);
            return;
        }
        match Composer::start(&mut self.mailbox) {
            Ok(composer) => self.composer = Some(composer),
            Err(error) => self.mailbox.fail(error),
        }
    }

    /// Writes a reply to the message `id`, a reply to all or a forward of it, in the reader's place, the engine having
    /// filled it in; whether it opened. A draft being written is closed first and kept; one the engine does not take
    /// stays open, and no reply starts.
    pub fn answer(&mut self, id: u64, answer: crate::compose::Answer) -> bool {
        if self.composer.is_some() {
            self.close_composer();
            if self.composer.is_some() {
                return false;
            }
        }
        self.show_mail();
        self.reader.set_full_width(false);
        match Composer::answer(&mut self.mailbox, id, answer) {
            Ok(composer) => {
                self.composer = Some(composer);
                true
            }
            Err(error) => {
                self.mailbox.fail(error);
                false
            }
        }
    }

    /// Closes the composer, its draft kept with what was typed in a recipient line as a chip. A draft the engine does
    /// not take leaves the composer open with everything in it.
    pub fn close_composer(&mut self) {
        let Some(composer) = &mut self.composer else { return };
        match composer.close(&mut self.mailbox) {
            Ok(()) => {
                self.composer = None;
                self.say(tr!("Draft saved"));
            }
            Err(error) => self.mailbox.fail(error),
        }
    }

    /// Sends the message being written (`mail.send`); a blocked one stays, saying why, and so does one the engine
    /// refuses.
    fn send_composer(&mut self) {
        let Some(composer) = &mut self.composer else { return };
        match composer.send(&mut self.mailbox) {
            Ok(true) => {
                self.composer = None;
                self.say(tr!("Sending your message"));
                // The message goes to the worker in this frame.
                self.pump();
            }
            Ok(false) => {}
            Err(error) => self.mailbox.fail(error),
        }
    }

    /// Removes the draft being written here and on the server.
    fn discard_composer(&mut self) {
        let Some(composer) = &self.composer else { return };
        match composer.discard(&mut self.mailbox) {
            Ok(()) => {
                self.composer = None;
                self.say(tr!("Draft discarded"));
            }
            Err(error) => self.mailbox.fail(error),
        }
    }

    /// Shows `text` in a toast for [`TOAST_SECONDS`].
    fn say(&mut self, text: &str) {
        self.notice = Some((text.to_owned(), None));
    }

    /// What the composer's last step says in its toast, while it shows.
    #[must_use]
    pub fn notice(&self) -> Option<&str> {
        self.notice.as_ref().map(|(text, _)| text.as_str())
    }

    /// Applies what was asked for in the composer.
    fn apply_compose(&mut self, ctx: &egui::Context, asked: compose::Asked) {
        use compose::Asked;
        match asked {
            Asked::Send => return self.send_composer(),
            Asked::Close => return self.close_composer(),
            Asked::Discard => return self.discard_composer(),
            Asked::Open => return self.compose(),
            _ => {}
        }
        let Some(composer) = &mut self.composer else { return };
        let mailbox = &mut self.mailbox;
        let result = match asked {
            Asked::Typed(line) => composer.typed(mailbox, line),
            Asked::Take(line) => composer.take(mailbox, line),
            Asked::Pick(at) => {
                if let Some(line) = composer.suggestions().0 {
                    ctx.memory_mut(|memory| memory.request_focus(compose::field_id(line)));
                }
                composer.pick(mailbox, at)
            }
            Asked::Step(down) => {
                composer.step(down);
                Ok(())
            }
            Asked::Dismiss => {
                composer.dismiss_suggestions();
                Ok(())
            }
            Asked::Remove(line, at) => composer.remove(mailbox, line, at),
            Asked::RemoveLast(line) => composer.remove_last(mailbox, line),
            Asked::Edited => {
                composer.edited();
                Ok(())
            }
            Asked::Copies => {
                composer.show_copies();
                Ok(())
            }
            Asked::From(account) => composer.set_account(mailbox, account),
            Asked::Template(at) => match composer.templates().get(at).cloned() {
                Some(name) => composer.use_template(mailbox, &name),
                None => Ok(()),
            },
            Asked::Fill => {
                ctx.memory_mut(|memory| memory.request_focus(compose::blank_id()));
                composer.fill(mailbox)
            }
            Asked::Send | Asked::Close | Asked::Discard | Asked::Open => Ok(()),
        };
        if let Err(error) = result {
            self.mailbox.fail(error);
        }
    }

    /// The composer on a card in the reader's place; what was asked in it.
    fn composer_card(&mut self, ui: &mut Ui, theme: &Theme, layers: &mut Layers, left: f32) -> Vec<compose::Asked> {
        let (keymap, mailbox) = (&self.keymap, &self.mailbox);
        let Some(composer) = self.composer.as_mut() else { return Vec::new() };
        CentralPanel::no_frame()
            .frame(around(left, theme.spacing.gap, theme.spacing.gap))
            .show(ui, |ui| layers.card(ui, Margin::ZERO, |ui| compose::show(ui, theme, keymap, mailbox, composer)))
            .inner
    }

    /// Applies what was asked in the composer this frame and keeps its draft; a message `clicked` in the list takes
    /// the reader's place, also when it was selected under the composer, which floats.
    fn composed(&mut self, ctx: &egui::Context, clicked: bool, asked: Vec<compose::Asked>) {
        if clicked && let Some(composer) = &mut self.composer {
            composer.set_floating(true);
        }
        for asked in asked {
            self.apply_compose(ctx, asked);
        }
        self.keep_draft();
    }

    /// Keeps the draft when its subject or text changed in this frame: silently, at once.
    fn keep_draft(&mut self) {
        if let Some(composer) = self.composer.as_mut().filter(|composer| composer.changed())
            && let Err(error) = composer.save(&mut self.mailbox)
        {
            self.mailbox.fail(error);
        }
    }

    /// The toast of the composer's last step, while no act's toast shows.
    fn notice_toast(&mut self, ui: &Ui, theme: &Theme) {
        let Some((text, since)) = self.notice.clone() else { return };
        if self.mailbox.undoable().is_some() {
            return;
        }
        let now = ui.input(|input| input.time);
        let since = since.unwrap_or(now);
        let left = TOAST_SECONDS - (now - since);
        if left <= 0.0 {
            self.notice = None;
            return;
        }
        self.notice = Some((text.clone(), Some(since)));
        ui.ctx().request_repaint_after(std::time::Duration::from_secs_f64(left));
        let bottom = ui.max_rect().center_bottom() - egui::vec2(0.0, theme.spacing.xl);
        toast::show(ui.ctx(), theme, &self.keymap, egui::Id::unique("compose-toast"), &Toast { text: &text, undo: None }, bottom);
    }

    /// The owner's appearance choices.
    #[must_use]
    pub fn appearance(&self) -> &Appearance {
        self.mailbox.appearance()
    }

    /// Runs the appearance command `id` (`ui.theme.set` ...) with `params`, as the window's controls do; the look
    /// changes from the next frame. Its answer, or why it was refused.
    ///
    /// # Errors
    ///
    /// The command's refusal.
    pub fn set_appearance(&mut self, id: &'static str, params: &Value) -> Result<Value, UiError> {
        self.mailbox.run(id, params)
    }

    /// Runs an appearance command whose parameters the window made itself; a refusal shows in the error banner.
    fn appear(&mut self, id: &'static str, params: &Value) {
        if let Err(error) = self.set_appearance(id, params) {
            self.mailbox.fail(error);
        }
    }

    /// The mode the owner chose.
    #[must_use]
    pub fn mode(&self) -> ModePreference {
        self.appearance().theme
    }

    /// Follows the system's mode, or always shows light or dark, from the next frame (`ui.theme.set`).
    pub fn set_mode(&mut self, mode: ModePreference) {
        self.appear("ui.theme.set", &json!({ "mode": mode.id() }));
    }

    /// The accent preset the colours come from.
    #[must_use]
    pub fn preset(&self) -> Preset {
        Preset::of(&self.appearance().accent)
    }

    /// Takes the colours from `preset`, from the next frame (`ui.accent.set`): a shipped preset by id, otherwise its
    /// four colours as the owner's own.
    pub fn set_preset(&mut self, preset: Preset) {
        if Accent::preset(preset.id).is_some() {
            self.appear("ui.accent.set", &json!({ "preset": preset.id }));
        } else {
            let rgb = |colour: egui::Color32| Rgb(colour.r(), colour.g(), colour.b());
            let custom = CustomAccent { gradient: preset.gradient.map(rgb), accent: rgb(preset.accent) };
            self.appear("ui.accent.set", &json!({ "custom": custom.hex() }));
        }
    }

    /// The text scale every font token is at.
    #[must_use]
    pub fn text_scale(&self) -> TextScale {
        Look::of(self.appearance()).text_scale
    }

    /// Draws every text at `scale`, from the next frame; the rows, headers and toolbars follow the font
    /// (`ui.text_scale.set`).
    pub fn set_text_scale(&mut self, scale: TextScale) {
        self.appear("ui.text_scale.set", &json!({ "percent": scale.percent() }));
    }

    /// The typeface every font token is in.
    #[must_use]
    pub fn typeface(&self) -> Typeface {
        self.appearance().font
    }

    /// Draws every text in `typeface`, from the next frame (`ui.font.set`).
    pub fn set_typeface(&mut self, typeface: Typeface) {
        self.appear("ui.font.set", &json!({ "font": typeface.id() }));
    }

    /// The icon set the icons come from.
    #[must_use]
    pub fn icons(&self) -> IconSet {
        IconSet::of(self.appearance().icons)
    }

    /// Draws the icons from `icons`, from the next frame (`ui.icons.set`).
    pub fn set_icons(&mut self, icons: Icons) {
        self.appear("ui.icons.set", &json!({ "icons": icons.id() }));
    }

    /// Who draws the window's frame, from the next frame (`ui.frame.set`).
    pub fn set_frame(&mut self, frame: WindowFrame) {
        self.appear("ui.frame.set", &json!({ "frame": frame.id() }));
    }

    /// Lets the app draw the window's frame: the native window calls it, so the strip, the window controls and the
    /// resizing edges show when the appearance asks for the app's own frame. Pictures and the web build leave it off.
    pub fn set_own_frame(&mut self, own: bool) {
        self.own_frame = own;
        self.frame_applied = None;
    }

    /// Whether the app draws the window's frame now.
    #[must_use]
    pub fn draws_frame(&self) -> bool {
        frame::drawn(self.own_frame, self.mailbox.appearance().frame)
    }

    /// Which key runs which action.
    #[must_use]
    pub fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    /// The keymap, to change a key; what shows the key shows the new one in the next frame.
    pub fn keymap_mut(&mut self) -> &mut Keymap {
        &mut self.keymap
    }

    /// Whether every animation lands at its end at once: the owner's choice, or the system's wish where it can be
    /// read (not on the desktop, where motion then runs).
    #[must_use]
    pub fn reduce_motion(&self) -> bool {
        match self.appearance().motion {
            Motion::Reduce => true,
            Motion::Full => false,
            Motion::System => motion::system_default(None),
        }
    }

    /// Turns reduced motion on or off, from the next frame (`ui.motion.set`).
    pub fn set_reduce_motion(&mut self, reduce: bool) {
        self.appear("ui.motion.set", &json!({ "reduce": if reduce { "on" } else { "off" } }));
    }

    /// Installs the fonts once, and the appearance in the context when it changed; keeps a changed appearance.
    fn install_theme(&mut self, ctx: &egui::Context) {
        let chosen = *self.mailbox.appearance();
        if self.themed == Some(chosen) {
            return;
        }
        if self.themed.is_none() {
            theme::install_fonts(ctx);
        }
        Theme::apply(ctx, &Look::of(&chosen));
        // egui follows the system by itself; a first frame that follows it leaves the context's mode alone.
        if self.themed.is_some() || chosen.theme != ModePreference::System {
            ctx.set_theme(theme::preference(chosen.theme));
        }
        self.themed = Some(chosen);
        if chosen != self.kept {
            match self.keep.store(&chosen) {
                Ok(()) => self.kept = chosen,
                Err(error) => self.mailbox.fail(UiError::Keep(error)),
            }
        }
    }

    /// Draws one frame into `ui` and applies what the person asked for.
    pub fn ui(&mut self, ui: &mut Ui) {
        self.install_theme(ui.ctx());
        motion::set_reduced(ui.ctx(), self.reduce_motion());
        self.apply_frame(ui.ctx());
        let theme = Theme::current(ui.ctx(), &Look::of(self.mailbox.appearance()));
        ui.set_style(theme.style());
        let p = theme.palette;
        let s = theme.spacing;
        self.sync();
        if self.reader.poll() | self.pump() | self.onboarding.poll() {
            ui.ctx().request_repaint();
        }
        let Keys { mut ran, escape, down, up, typing } = self.keys(ui.ctx());
        let mut stepped = false;
        if !typing && (down || up) {
            self.mailbox.step(down);
            stepped = true;
        }
        let keyed = if typing { None } else { self.keyed_act(ui.ctx()) };
        self.follow_adding();
        let adding = self.onboarding.stage() != Stage::Closed;
        let left_settings = escape && self.settings.is_open();
        if left_settings {
            self.close_settings();
        }
        let setting = self.settings.is_open();
        let left_full = escape && !left_settings && self.screen == Screen::Mail && self.reader.full_width();
        if left_full {
            self.reader.set_full_width(false);
        }
        let mut dismiss = false;
        if let Some(error) = self.mailbox.error() {
            Panel::top("error").show_separator_line(false).frame(Frame::new().fill(p.error).inner_margin(margin(s.s))).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(error).color(p.on_error));
                    dismiss = ui.button(tr!("Dismiss")).clicked();
                });
            });
        }
        let mut asked = pane::Asked::default();
        let mut add = false;
        let mut searched = None;
        let mut clicked = None;
        let mut acted = None;
        // The row sliding out is out when the list is not drawn.
        let mut out = true;
        let mut changed = None;
        let mut review = false;
        let docked = self.docked();
        let full = self.reader.full_width() && !docked;
        // The colour fields first, then the place for the glass layers, then everything else.
        let corner = self.window_corner(ui.ctx());
        glass::backdrop_rounded(ui.painter(), &theme, ui.max_rect(), corner);
        let mut layers = Layers::begin(ui.painter());
        let (gap, half) = (s.gap, s.gap / 2.0);
        let width = pane::rail_width(&theme);
        let clicked_rail = Panel::left("rail")
            .resizable(false)
            .exact_size(width)
            .show_separator_line(false)
            .frame(Frame::NONE)
            .show(ui, |ui| pane::rail(ui, &theme, &self.keymap, &self.mailbox, self.screen, setting, adding, self.rail_place(corner, gap)))
            .inner;
        self.own_frame_parts(ui, &theme, corner);
        ran.extend(clicked_rail.actions());
        let mut on_calendar = Vec::new();
        if self.screen == Screen::Calendar {
            on_calendar = self.calendar_screen(ui, &theme, &mut layers);
        } else if self.mailbox.accounts().is_empty() {
            add = self.welcome(ui, &theme, &mut layers);
        } else if !full {
            let left;
            (asked, left) = self.folders(ui, &theme, &mut layers);
            let (mailbox, search, keymap) = (&self.mailbox, &mut self.search, &self.keymap);
            let mut list = |ui: &mut Ui, layers: &mut Layers| {
                layers.card(ui, Margin::ZERO, |ui| {
                    let searched = Panel::top("list-header")
                        .show_separator_line(false)
                        .frame(Frame::new().inner_margin(margin(s.l)))
                        .show(ui, |ui| frame::beside(ui, &theme, |ui| list::header(ui, &theme, keymap, mailbox, search)))
                        .inner;
                    let listed = CentralPanel::no_frame()
                        .frame(Frame::new().inner_margin(Margin {
                            left: margin(s.m).left,
                            right: margin(s.m).right,
                            top: 0,
                            bottom: margin(s.m).bottom,
                        }))
                        .show(ui, |ui| list::rows(ui, &theme, keymap, mailbox))
                        .inner;
                    (searched, listed.clicked, listed.acted, listed.out, listed.review)
                })
            };
            (searched, clicked, acted, out, review) = if self.reader.open_id().is_some() || docked {
                Panel::left("list")
                    .resizable(true)
                    .default_size(LIST_WIDTH)
                    .show_separator_line(false)
                    .frame(around(left, half, gap))
                    .show(ui, |ui| list(ui, &mut layers))
                    .inner
            } else {
                CentralPanel::no_frame().frame(around(left, gap, gap)).show(ui, |ui| list(ui, &mut layers)).inner
            };
        }
        let mut drawn = reader::Drawn::default();
        let mut composed = Vec::new();
        // A texture lives no longer than the picture the reader keeps for it.
        self.textures.retain(|key, _| matches!(self.reader.picture(key), Some(PictureState::Ready(_))));
        if docked {
            composed = self.composer_card(ui, &theme, &mut layers, half);
        } else if self.screen == Screen::Mail && self.reader.open_id().is_some() {
            let hover = self.hover.take();
            let left = if full { gap } else { half };
            drawn = CentralPanel::no_frame()
                .frame(around(left, gap, gap))
                .show(ui, |ui| {
                    layers.card(ui, Margin::ZERO, |ui| {
                        reader::show(ui, &theme, &self.keymap, &self.mailbox, &self.reader, &mut self.textures, hover.as_deref())
                    })
                })
                .inner;
            if drawn.hover != hover {
                ui.ctx().request_repaint();
            }
            self.hover = drawn.hover.take();
        }
        layers.finish(ui.painter(), &theme);
        if setting {
            changed = self.settings_sheet(ui.ctx(), &theme);
        }
        let palette = if self.palette.is_open() {
            let runnable: Vec<&str> = keymap::actions().into_iter().map(|action| action.id).filter(|id| self.can_run(id)).collect();
            palette::show(ui.ctx(), &theme, &self.keymap, &mut self.palette, &runnable)
        } else {
            None
        };
        composed.extend(
            self.composer
                .as_ref()
                .filter(|_| !docked)
                .map(|composer| compose::floating(ui.ctx(), &theme, &self.keymap, composer, ui.max_rect()))
                .unwrap_or_default(),
        );
        // Every gradient of the frame is painted by now: the pictures it left out go.
        gradient::sweep(ui.ctx());
        let undone = self.toast(ui, &theme, typing);
        self.notice_toast(ui, &theme);
        let acted = acted.or(keyed);
        let leaving = self.mailbox.leaving().is_some();
        let busy = dismiss
            || stepped
            || acted.is_some()
            || undone
            || (out && leaving)
            || left_full
            || left_settings
            || !ran.is_empty()
            || palette.is_some()
            || clicked_rail.settings
            || changed.is_some()
            || asked.narrow.is_some()
            || asked.open.is_some()
            || searched.is_some()
            || review
            || clicked.is_some()
            || add
            || asked.compose
            || !on_calendar.is_empty()
            || !composed.is_empty()
            || !drawn.asked.is_empty();
        if busy {
            // What changed shows in the next frame.
            ui.ctx().request_repaint();
        }
        self.act(out, acted, undone);
        self.apply_calendar(on_calendar);
        for asked in drawn.asked {
            self.apply(ui, asked);
        }
        if dismiss {
            self.mailbox.dismiss_error();
        }
        self.go(&asked, searched, review);
        if clicked.is_some() {
            self.mailbox.select(clicked);
        }
        self.composed(ui.ctx(), clicked.is_some(), composed);
        ran.extend(asked.compose.then_some(keymap::COMPOSE));
        if add {
            self.open_accounts();
        }
        if clicked_rail.settings {
            if setting { self.close_settings() } else { self.open_settings() }
        }
        if let Some(asked) = palette {
            ran.extend(self.apply_palette(asked));
        }
        for id in ran {
            self.run_action(ui.ctx(), id);
        }
        if let Some(changed) = changed {
            self.apply_settings(changed);
        }
        self.keep_keys();
        self.keep_triage();
        self.own_frame_over(ui.ctx(), &theme, corner);
        self.sync();
    }

    /// Shows what was asked for this frame: an account narrowed, a place opened in the pane, a search, or "New
    /// senders" from Review on the screener card.
    fn go(&mut self, asked: &pane::Asked, searched: Option<String>, review: bool) {
        if let Some(account) = asked.narrow {
            self.mailbox.narrow(account);
        }
        let place = if review { Some(Place::Senders) } else { asked.open };
        if let Some(place) = place {
            self.mailbox.open(place);
            self.search.clear();
        }
        if let Some(query) = searched {
            self.mailbox.search(&query);
        }
    }

    /// The app's own frame before the panes, when it draws one: the strip right of the rail with the title and the
    /// window controls, or the band of the top row that moves the window; and the resizing edges.
    fn own_frame_parts(&self, ui: &mut Ui, theme: &Theme, corner: f32) {
        let drawn = self.draws_frame();
        frame::top_row_begins(ui, drawn && self.mailbox.appearance().frame == WindowFrame::TopRow);
        if !drawn {
            return;
        }
        if self.mailbox.appearance().frame == WindowFrame::Strip {
            let place = if self.screen == Screen::Calendar { tr!("Calendar").to_owned() } else { self.mailbox.title() };
            let title = tr!("{place} · InboxCraft", place = place);
            Panel::top("window-strip")
                .exact_size(frame::STRIP)
                .show_separator_line(false)
                .frame(Frame::NONE)
                .show(ui, |ui| frame::strip(ui, theme, &self.keymap, &title, corner));
        }
        frame::edges(ui.ctx());
    }

    /// The app's own frame over the panes, when it draws one: the controls of the top row at the end of the right
    /// pane's first row, and the window's outline.
    fn own_frame_over(&self, ctx: &egui::Context, theme: &Theme, corner: f32) {
        if !self.draws_frame() {
            return;
        }
        if self.mailbox.appearance().frame == WindowFrame::TopRow {
            frame::top_row(ctx, theme, &self.keymap);
        }
        frame::outline(ctx, theme, corner);
    }

    /// The folder pane on a card at the left, when there is an account: without one it would hold only the app's
    /// name. What was asked on it, and how far right of it the next card starts (a full gap from the rail without it).
    fn folders(&self, ui: &mut Ui, theme: &Theme, layers: &mut Layers) -> (pane::Asked, f32) {
        let s = theme.spacing;
        if self.mailbox.accounts().is_empty() {
            return (pane::Asked::default(), s.gap);
        }
        let shown = Panel::left("folders")
            .resizable(true)
            .default_size(s.folder_pane)
            .show_separator_line(false)
            .frame(around(s.gap, s.gap / 2.0, s.gap))
            .show(ui, |ui| layers.card(ui, margin(s.m), |ui| pane::show(ui, theme, &self.keymap, &self.mailbox)));
        (shown.inner, s.gap / 2.0)
    }

    /// The window with no account: one card in the middle, as tall as what it says; whether Add account was pressed.
    fn welcome(&self, ui: &mut Ui, theme: &Theme, layers: &mut Layers) -> bool {
        let (keymap, can_add) = (&self.keymap, self.can_add);
        middle_card(ui, theme, layers, |ui| list::welcome(ui, theme, keymap, can_add))
    }

    /// The calendar screen: its empty state on one card in the middle until there are calendars; what was pressed.
    fn calendar_screen(&self, ui: &mut Ui, theme: &Theme, layers: &mut Layers) -> Vec<calendar::Asked> {
        if self.calendar.is_empty() {
            middle_card(ui, theme, layers, |ui| calendar::empty(ui, theme));
            return Vec::new();
        }
        calendar::screen(ui, theme, &self.keymap, &self.calendar, layers)
    }

    /// Where the rail sits: the window's `corner`, and its mark one `gap` down, under the strip when there is one,
    /// where the cards start.
    fn rail_place(&self, corner: f32, gap: f32) -> pane::RailPlace {
        let strip = self.draws_frame() && self.mailbox.appearance().frame == WindowFrame::Strip;
        pane::RailPlace { corner, top: gap + if strip { frame::STRIP } else { 0.0 } }
    }

    /// The settings sheet over the window; what was asked on it.
    fn settings_sheet(&mut self, ctx: &egui::Context, theme: &Theme) -> Option<settings::Asked> {
        let reduce = self.reduce_motion();
        let view = settings::Sheet {
            keymap: &self.keymap,
            settings: &mut self.settings,
            current: self.mailbox.appearance(),
            reduce,
            mailbox: &self.mailbox,
            onboarding: &mut self.onboarding,
            can_add: self.can_add,
        };
        settings::sheet(ctx, theme, view)
    }

    /// Call with each frame's input before the frame (eframe's `raw_input_hook`): after the desktop moved or resized
    /// the window it adds the button release the desktop kept, so the hover shows again without a click.
    pub fn release_after_window_grab(ctx: &egui::Context, raw: &mut egui::RawInput) {
        frame::release_after_grab(ctx, raw);
    }

    /// The window's corner radius in `ctx`.
    #[must_use]
    pub fn window_corner(&self, ctx: &egui::Context) -> f32 {
        frame::corner(ctx, self.draws_frame())
    }

    /// Tells a native window to show the desktop's title bar or none, when the frame changed.
    fn apply_frame(&mut self, ctx: &egui::Context) {
        let frame = self.mailbox.appearance().frame;
        if self.own_frame && self.frame_applied != Some(frame) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(frame == WindowFrame::System));
            self.frame_applied = Some(frame);
        }
    }

    /// Applies the acts of the frame: the sliding row's once it is `out`, the one `acted` (a hover action or its
    /// key), and Undo when `undone`.
    fn act(&mut self, out: bool, acted: Option<(u64, Act)>, undone: bool) {
        if out {
            self.mailbox.left();
        }
        if let Some((id, act)) = acted {
            self.mailbox.start(id, act);
        }
        if undone {
            self.mailbox.undo();
        }
    }

    /// The act whose key was pressed in this frame, on the selected row.
    fn keyed_act(&self, ctx: &egui::Context) -> Option<(u64, Act)> {
        let (action, _) = ROW_ACTS.into_iter().find(|(action, _)| self.keymap.pressed(ctx, action))?;
        self.row_act(action)
    }

    /// The toast after the last act, rising from the bottom edge, for [`TOAST_SECONDS`]; after it Ctrl+Z still takes
    /// acts back, one at a time. Whether Undo was pressed (its key works while no text field has the focus).
    fn toast(&mut self, ui: &Ui, theme: &Theme, typing: bool) -> bool {
        let Some(undo) = self.mailbox.undoable().cloned() else {
            self.toast = None;
            return false;
        };
        let now = ui.input(|input| input.time);
        let since = match self.toast {
            Some((serial, since)) if serial == undo.serial => since,
            _ => now,
        };
        let left = TOAST_SECONDS - (now - since);
        if left <= 0.0 {
            self.mailbox.forget_undo();
            self.toast = None;
            return false;
        }
        self.toast = Some((undo.serial, since));
        ui.ctx().request_repaint_after(std::time::Duration::from_secs_f64(left));
        let mut keys = self.keymap.clone();
        if typing {
            keys.unbind(keymap::UNDO);
        }
        let bottom = ui.max_rect().center_bottom() - egui::vec2(0.0, theme.spacing.xl);
        let id = egui::Id::unique("act-toast").with(undo.serial);
        toast::show(ui.ctx(), theme, &keys, id, &Toast { text: &undo.text, undo: undo.undoable.then_some(keymap::UNDO) }, bottom)
    }

    /// Applies what was asked for in the palette; the action to run, when one was chosen.
    fn apply_palette(&mut self, asked: palette::Asked) -> Option<&'static str> {
        match asked {
            palette::Asked::Run(id) => {
                self.palette.close();
                return Some(id);
            }
            palette::Asked::Step(down) => {
                let count = self.palette.matches().len();
                self.palette.step(down, count);
            }
            palette::Asked::Typed => self.palette.typed(),
            palette::Asked::Close => self.palette.close(),
        }
        None
    }

    /// Applies what was asked for on the settings page: each control through its `ui.*.set` command.
    fn apply_settings(&mut self, asked: settings::Asked) {
        match asked {
            settings::Asked::Mode(mode) => self.set_mode(mode),
            settings::Asked::Preset(id) => {
                self.appear("ui.accent.set", &json!({ "preset": id }));
                self.settings.follow(self.mailbox.appearance());
            }
            settings::Asked::Custom(colours) => match self.set_appearance("ui.accent.set", &json!({ "custom": colours })) {
                Ok(answer) => self.settings.took(&answer, self.mailbox.appearance()),
                Err(error) => self.mailbox.fail(error),
            },
            settings::Asked::Font(typeface) => self.set_typeface(typeface),
            settings::Asked::Icons(icons) => self.set_icons(icons),
            settings::Asked::Scale(scale) => self.set_text_scale(scale),
            settings::Asked::ReduceMotion(reduce) => self.set_reduce_motion(reduce),
            settings::Asked::Frame(frame) => self.set_frame(frame),
            settings::Asked::Section(section) => {
                self.onboarding.close();
                self.settings.show(section);
            }
            settings::Asked::Add => self.onboarding.open(),
            settings::Asked::Back => self.onboarding.close(),
            settings::Asked::Page(asked) => self.apply_page(asked),
            settings::Asked::Capture(action) => self.settings.capture(action),
            settings::Asked::Unbind(action) => self.keymap.unbind(action),
            settings::Asked::ResetKeys => {
                self.keymap = Keymap::default();
                self.settings.captured(None);
            }
            settings::Asked::Signature(account) => self.save_signature(account),
            settings::Asked::Screener(mode) => self.mailbox.set_screener_mode(mode),
            settings::Asked::InboxView(view) => self.mailbox.set_inbox_view(view),
            settings::Asked::Grouped(account, grouped) => self.mailbox.set_grouped(account, grouped),
            settings::Asked::Close => self.close_settings(),
        }
    }

    /// Saves the signature of `account` as typed on the Sending page (`compose.signature`); an empty one clears it.
    fn save_signature(&mut self, account: u64) {
        let typed = self.settings.signature(account).map(str::trim_end).filter(|typed| !typed.trim().is_empty()).map(str::to_owned);
        match self.mailbox.run("compose.signature", &json!({ "account": account, "signature": typed })) {
            Ok(_) => {
                self.settings.saved_signature(account);
                self.mailbox.refresh();
            }
            Err(error) => self.mailbox.fail(error),
        }
    }

    /// Applies what was asked for on the add-account form or the check.
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
        if let reader::Asked::Answer(id, answer) = asked {
            if self.answer(id, answer) {
                // The text has the focus, or To for a forward.
                let field = if answer == crate::compose::Answer::Forward { compose::field_id(crate::compose::Line::To) } else { compose::text_id() };
                ui.ctx().memory_mut(|memory| memory.request_focus(field));
            }
            return;
        }
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
            reader::Asked::Act(id, act) => mailbox.start(id, act),
            reader::Asked::Answer(..) => {}
        }
    }
}

#[cfg(test)]
#[path = "appearance_tests.rs"]
mod appearance_tests;
#[cfg(test)]
#[path = "compose_tests.rs"]
mod compose_tests;
#[cfg(test)]
#[path = "fit_tests.rs"]
mod fit_tests;
#[cfg(test)]
#[path = "frame_tests.rs"]
mod frame_tests;
#[cfg(test)]
#[path = "list_tests.rs"]
mod list_tests;
#[cfg(test)]
#[path = "onboard_tests.rs"]
mod onboard_tests;
#[cfg(test)]
#[path = "palette_tests.rs"]
mod palette_tests;
#[cfg(test)]
#[path = "pane_tests.rs"]
mod pane_tests;
#[cfg(test)]
#[path = "reader_tests.rs"]
mod reader_tests;
#[cfg(test)]
#[path = "reply_tests.rs"]
mod reply_tests;
#[cfg(test)]
#[path = "settings_tests.rs"]
mod settings_tests;
#[cfg(test)]
#[path = "shortcuts_tests.rs"]
mod shortcuts_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
#[path = "triage_tests.rs"]
mod triage_tests;
