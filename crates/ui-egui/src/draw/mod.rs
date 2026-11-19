// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The window's content: the folder pane on the left, the header with the search field and the message list, the
//! reader of the selected message on the right, and an error banner on top while a command's error stands. The
//! reader can fill the window (full width); Escape leaves that. A window that can add accounts shows "Add account"
//! in the empty window, which opens the settings sheet on Accounts; accounts are added and listed there.
//! The navigation rail runs down the window's left edge on the flat panel: its settings button (or its key)
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
//! calendar screen ([`Screen::Calendar`]), drawn in `calendar/`, in the place of the mail. A change to an event made
//! there shows the same toast, and Undo or Ctrl+Z reads the calendar again.
//!
//! A message sent from the composer waits in the outbox for the hold: a toast counts the seconds down, and its Undo
//! (or Ctrl+Z) takes the send back and opens the draft again as it was. The outbox's rows take a send back the same
//! way, open a refused draft or send it again. A send whose time passed while InboxCraft was not running says so in a
//! toast once it went.

mod accounts;
mod calendar;
pub(crate) use self::calendar::{event_board, mini_month_board};
mod compose;
mod frame;
mod invite;
mod list;
mod onboard;
pub(crate) mod paint;
mod palette;
mod pane;
mod reader;
pub(crate) use reader::fitted_board;
mod settings;
mod shell;

pub use list::rows_built;

use std::sync::Arc;

use egui::{CentralPanel, Frame, Key, Margin, Panel, Rect, Ui, pos2};
use inboxcraft_engine::appearance::{Appearance, Motion, WindowFrame};
use inboxcraft_engine::notify::Settings as NotifySettings;
use inboxcraft_model::Role;
use serde_json::{Value, json};

use crate::calendar::Calendar;
use crate::component::banner::{self, BannerKind, BannerLink};
use crate::component::soon;
use crate::component::toast::{self, Toast};
use crate::compose::Composer;
use crate::desk::Desk;
use crate::fetch::{Fetch, Offline};
use crate::keep::{Keep, NoKeep, Triage};
use crate::keymap::{self, Captured, Keymap};
use crate::mailbox::{Act, InboxView, Mailbox, Pile, Place};
use crate::motion;
use crate::onboard::{Detect, Keychain, Onboarding, Stage};
use crate::palette::Palette;
use crate::reader::{PictureState, Reader};
use crate::save::{NoSave, Save};
use crate::settings::{Section, Settings};
use crate::surface::{Facts, Surface, Surfaces};
use crate::theme::{self, Look, ModePreference, TextScale, Theme};
use crate::workers::{NoWorkers, Workers};
use crate::{UiError, tr};
use paint::Panes;

/// The width of the list's rows beside the reader, in points: the design's 440, whatever the text size. The list's
/// pane is a point wider, for the line between it and the pane at its left.
pub(super) const LIST_WIDTH: f32 = 440.0;

/// A window narrower than this, in points, holds one pane beside the folders: the open message and the list swap
/// (`Enter` or a press opens, `Esc` goes back), as the design's breakpoint has it.
pub(super) const ONE_PANE: f32 = 980.0;

/// The default width of the folder pane, in points.
pub(super) const FOLDER_PANE: f32 = 240.0;

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
    /// The surface an Esc of this frame closes ([`Surfaces::escape`]).
    escape: Option<Surface>,
    /// An Esc of this frame that is the place's: the reader gives the list the keyboard back.
    to_place: bool,
    /// Whether the mail's own keys rest.
    typing: bool,
    /// Whether a text field, the settings sheet, the palette or the composer has the keys: the toast's Undo key rests.
    quiet: bool,
}

/// The acts on a row, which run on the selected message.
const ROW_ACTS: [(&str, Act); 5] =
    [(keymap::DONE, Act::Done), (keymap::ARCHIVE, Act::Archive), (keymap::READ, Act::Read), (keymap::FLAG, Act::Flag), (keymap::DELETE, Act::Delete)];

/// What the reader keeps when the open message left the list: the row the next-message rule named, and whether it
/// followed the gap (the rule's row stands on the gap's other side at either end of the list).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Held {
    pick: u64,
    below: bool,
    /// The row the keyboard landed on after the server took the open message away: a step from it still starts from
    /// the gap.
    land: Option<u64>,
}

/// Which list is shown: its place, the account it is narrowed to (none: all), its search.
type ListOf = (Place, Option<u64>, String);

/// The InboxCraft window's content over a [`Mailbox`].
pub struct InboxApp {
    mailbox: Mailbox,
    reader: Reader,
    textures: reader::Textures,
    hover: Option<String>,
    /// The row the next-message rule names when the open message left the list, and which side of the gap it stands
    /// on: the reader shows nothing open and the list selects no row until the owner opens a message; an arrow opens
    /// the row beside the gap, and "Open the message the rule names" opens the rule's row.
    held_back: Option<Held>,
    /// The row the keyboard goes to when the list is next drawn: the next one after the list's own act walked on, or,
    /// once the server took the open message away, the row that took its place (or the last), without opening it.
    land_held: Option<u64>,
    /// The row landed on for [`InboxApp::land_held`]: its focus opens no preview.
    landed: Option<u64>,
    /// Whether [`InboxApp::land_held`] is a row the owner's act walked on to, brought into sight before it lands.
    bring_held: bool,
    /// The card of the grouped inbox the open message stood in when it opened (`Some(None)`: the rows no group holds;
    /// `None`: no cards), so that the keyboard stays in that card when the server takes the message away.
    card: Option<Option<String>>,
    /// The messages of the list's rows in the order the list last drew them ([`list::order`]), read as each frame
    /// begins: where the keyboard goes when the server takes the open message away.
    drawn: std::sync::Arc<[u64]>,
    /// The message whose row of the list has the keyboard's focus, as this frame began: the list's keys act on it.
    list_row: Option<u64>,
    /// A row of the list was pressed or keyed last, not the reader: with nothing else focused the list has the keyboard
    /// ([`InboxApp::list_has_keyboard`]).
    list_keys: bool,
    /// In one pane ([`ONE_PANE`]), the open message stands in the list's place: `Enter` or a press put it there, `Esc`
    /// or "Back to the list" take it away. With nothing open, or while a person walks the list, the list is in front.
    reading: bool,
    /// In one pane, the list stands in the docked composer's place (`Esc` in the composer); "New message" brings the
    /// composer back.
    list_over_composer: bool,
    /// In one pane, the reader takes the focus once it is drawn: `Enter` or a press opened a message into it.
    focus_reader: bool,
    /// An act from the reader walked on to the next message: the keyboard stays in the reader, on the reader's first
    /// tool where it would be on nothing.
    focus_walked: bool,
    /// The last act on a row came by a key (or the palette), not by a press: the keyboard is in the window.
    act_by_key: bool,
    /// An Undo beside the docked composer gave a selection back: it opens as a preview, never as the owner's open.
    restored: bool,
    /// Whether the window held one pane in the last frame drawn; `None` before the first.
    single: Option<bool>,
    /// Whether the list was shown in the last frame drawn.
    list_was_shown: bool,
    /// The message the reader showed last, so the keyboard can land near its row once it has gone, and the list it
    /// was opened from (place, account, search): a list changed since has no row near it.
    left: Option<(u64, ListOf)>,
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
    /// The notification choices the keep holds; `None` for the sample, which keeps none: its accounts are not the
    /// owner's.
    kept_notify: Option<NotifySettings>,
    /// The app's side of running without a window, when it has one.
    desk: Option<Box<dyn Desk>>,
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
    /// Whether the keyboard focus was inside the composer at the start of this frame: Send's key acts only then.
    composer_keys: bool,
    /// Whether the composer has taken the list's place this frame (too narrow for the list's heads beside it, or in
    /// front of it in one pane): the list is not drawn, and nothing acts on it or its rows (no key, no palette command,
    /// no Undo).
    list_hidden: bool,
    /// Where the list was drawn last, to tell whether the keyboard was in it when it goes.
    list_rect: Option<Rect>,
    /// The list's least width beside the docked composer and what it was measured for: the mailbox's revision, whether
    /// something is searched, the text size and the scale ([`InboxApp::list_least`]).
    list_least: Option<((u64, bool, u32, u32), f32)>,
    /// For how many more frames the keyboard is given to the composer's text, the list having gone with it.
    keys_to_text: u8,
    /// How often [`list::least_width`] was measured (for the tests).
    #[cfg(test)]
    measured: usize,
    /// The frame being drawn; between frames, the next one.
    frame: u64,
    /// What the composer's last step says in a toast (the draft kept, the message on its way), and since when.
    notice: Option<(String, Option<f64>)>,
    /// The time picker opens over the open message in the next frame, with this text typed.
    picker: Option<String>,
    /// The draft whose send the countdown toast shows, while it waits for its hold.
    sending: Option<u64>,
    /// The composer's send-later picker opens in the next frame.
    later_picker: bool,
    /// A menu that opens in the next frame (the `ui_shot` scripts).
    menu: Option<egui::Id>,
    /// The input time the draft was last kept by [`InboxApp::keep_draft`], in seconds.
    kept_at: Option<f64>,
}

/// While the person types in the composer, its draft is kept at most once in this many seconds: keeping it runs
/// `compose.update`, which writes the whole draft to the store on the window's thread. Leaving a field, closing,
/// sending, a close of the window and the end of the app keep it at once, so only a crash can lose what was typed in
/// the last second.
pub(crate) const KEEP_EVERY: f64 = 1.0;

/// The end of the app keeps the draft being written, however little time passed since it was last kept.
impl Drop for InboxApp {
    fn drop(&mut self) {
        self.keep_draft_now();
    }
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

/// The widest a pane at the left of the reader's place (the folders, the list) gets in `ui`: with the composer `docked`
/// there, what leaves the composer its least width and the card's 8 beside it, so in a narrow window the pane gives way
/// to it. A width that is not a number counts as 1.
fn list_widest(ui: &Ui, theme: &Theme, docked: bool) -> f32 {
    if docked { (ui.available_width() - compose::least_width(ui, theme) - theme.spacing.s2).max(1.0) } else { f32::INFINITY }
}

/// Keeps the width the owner gave the resizable pane `id` apart from what `widest` squeezes it to (egui keeps the
/// squeezed width as the pane's own, kept while the app runs): while the cap bites, the owner's width is kept; once it
/// lets go (the composer closed or floating, the window wider), the pane is given back the owner's width, as far as the
/// cap allows. A width the owner drags to while the cap does not bite is the owner's, and so is a narrower one while
/// it is squeezed, but only from the pane's own resize handle (dragged, or stepped by a screen reader): a width that
/// changed under a window resized however slowly is the cap's. Both are kept with the pane's own.
fn keep_owners_width(ctx: &egui::Context, id: egui::Id, widest: f32) {
    let Some(mut state) = egui::containers::panel::PanelState::load(ctx, id) else { return };
    let width = state.outer_rect.width();
    if !width.is_finite() {
        return;
    }
    let key = id.with("owners-width");
    let (owner, squeezed) = ctx.data_mut(|data| data.get_persisted::<(f32, bool)>(key)).unwrap_or((width, false));
    // The handle dragged now, or released last frame: egui stores the width a drag leaves only in the frame the
    // drag stops, so this frame reads it. A screen reader's step or value on the handle is the owner's as a drag is.
    let handle = id.with("__resize");
    let stepped = ctx.input(|input| {
        use egui::accesskit::Action;
        [Action::Increment, Action::Decrement, Action::SetValue].into_iter().any(|action| input.num_accesskit_action_requests(handle, action) > 0)
    });
    let dragging = ctx.is_being_dragged(handle) || ctx.drag_stopped_id() == Some(handle) || stepped;
    let flag = key.with("dragged");
    let dragged = ctx.data_mut(|data| data.get_temp::<bool>(flag)).unwrap_or(false) || dragging;
    ctx.data_mut(|data| data.insert_temp(flag, dragging));
    let biting = width >= widest - 0.5;
    let kept = if biting {
        (owner, true)
    } else if squeezed && dragged {
        (width, false)
    } else if squeezed {
        let to = owner.min(widest);
        if to.is_finite() && to > width + 0.5 {
            state.outer_rect.set_width(to);
            ctx.data_mut(|data| data.insert_persisted(id, state));
        }
        (owner, owner > widest + 0.5)
    } else {
        (width, false)
    };
    ctx.data_mut(|data| data.insert_persisted(key, kept));
}

/// Where the list stands: not drawn in one pane while something stands in its place (`shown` false); beside the
/// reader or the composer (`beside`), but not drawn while the composer `docked` beside it would leave it (`widest`,
/// [`list_widest`]) narrower than its heads need (`least`, [`InboxApp::list_least`]); else over the whole middle.
fn list_place(shown: bool, beside: bool, docked: bool, widest: f32, least: f32) -> ListPlace {
    if !shown {
        ListPlace::Away
    } else if beside && docked && widest < least {
        ListPlace::Hidden
    } else if beside {
        ListPlace::Beside
    } else {
        ListPlace::Whole
    }
}

/// The list's pane at the left: its 440 and the line at its left, no splitter, and a width a person once dragged to is
/// not read; beside the docked composer never wider than what leaves the composer its own (`widest`).
fn list_panel(docked: bool, widest: f32) -> Panel {
    let width = if docked { (LIST_WIDTH + 1.0).min(widest) } else { LIST_WIDTH + 1.0 };
    Panel::left("list").resizable(false).exact_size(width).show_separator_line(false)
}

/// Where the list stands in a frame.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ListPlace {
    /// Not drawn: in one pane, the open message or the composer stands in its place.
    Away,
    /// Not drawn: the composer in the reader's place has taken its place too.
    Hidden,
    /// At the left, beside the reader or the composer.
    Beside,
    /// Over the whole middle.
    Whole,
}

/// A pane's frame: no fill, `left` and `right` of its pane and `gap` above and below it.
fn around(left: f32, right: f32, gap: f32) -> Frame {
    // Distances are a few points; the clamp keeps the casts inside i8.
    let side = |points: f32| points.clamp(0.0, 100.0) as i8;
    Frame::new().inner_margin(Margin { left: side(left), right: side(right), top: side(gap), bottom: side(gap) })
}

/// Whether the window holds one pane beside the folders ([`ONE_PANE`]).
pub(super) fn one_pane(ctx: &egui::Context) -> bool {
    ctx.content_rect().width() < ONE_PANE
}

/// The reader's pane on the screen in the last frame that drew it.
#[cfg(test)]
pub(super) fn reader_rect(ctx: &egui::Context) -> Option<egui::Rect> {
    ctx.data(|data| data.get_temp::<egui::Rect>(egui::Id::unique("reader-rect")))
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
            held_back: None,
            land_held: None,
            landed: None,
            bring_held: false,
            card: None,
            drawn: std::sync::Arc::from(Vec::new()),
            list_row: None,
            list_keys: true,
            reading: false,
            list_over_composer: false,
            focus_reader: false,
            focus_walked: false,
            restored: false,
            act_by_key: false,
            single: None,
            list_was_shown: true,
            left: None,
            search: String::new(),
            themed: None,
            keep: Box::new(NoKeep),
            kept: Appearance::default(),
            keymap: Keymap::default(),
            kept_keys: Keymap::default(),
            kept_triage: Triage::default(),
            kept_notify: None,
            desk: None,
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
            composer_keys: false,
            list_hidden: false,
            list_rect: None,
            list_least: None,
            keys_to_text: 0,
            #[cfg(test)]
            measured: 0,
            frame: 0,
            notice: None,
            picker: None,
            sending: None,
            later_picker: false,
            kept_at: None,
            menu: None,
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
                self.mailbox.restore_pins(kept.keep_pins);
                if let Some(hold) = kept.hold {
                    self.mailbox.restore_hold(hold);
                }
                self.kept_triage = Triage { hold: Some(self.mailbox.hold_seconds()), ..kept };
            }
            None => {
                let screener = if sample { Triage::default().screener } else { self.mailbox.screener_mode() };
                self.kept_triage = Triage {
                    screener,
                    view: self.mailbox.inbox_view(),
                    keep_pins: self.mailbox.pins_outlast_done(),
                    hold: Some(self.mailbox.hold_seconds()),
                };
            }
        }
        if !sample {
            // What was kept stands as kept, so a level the session left out is dropped from the file at the next frame.
            self.kept_notify = Some(match keep.load_notify() {
                Some(kept) => {
                    self.mailbox.restore_notify(kept.clone());
                    kept
                }
                None => self.mailbox.notify_settings().clone(),
            });
        }
        self.keep = keep;
        self
    }

    /// The same window, showing and changing on the Notifications page what `desk` lets the owner choose: running in
    /// the background and starting with the computer.
    #[must_use]
    pub fn with_desk(mut self, desk: Box<dyn Desk>) -> Self {
        self.set_desk(desk);
        self
    }

    /// Shows and changes what `desk` lets the owner choose from now on, as [`InboxApp::with_desk`].
    pub fn set_desk(&mut self, desk: Box<dyn Desk>) {
        self.desk = Some(desk);
    }

    /// Keeps the notification choices when they changed; the sample keeps none.
    fn keep_notify(&mut self) {
        let Some(kept) = self.kept_notify.as_ref() else { return };
        let now = self.mailbox.notify_settings();
        if now == kept {
            return;
        }
        // Tried once per change: the next change tries again.
        let now = now.clone();
        let stored = self.keep.store_notify(&now);
        self.kept_notify = Some(now);
        if let Err(error) = stored {
            self.mailbox.fail(UiError::Keep(error));
        }
    }

    /// What the desk says this frame.
    fn desk_view(&self) -> settings::DeskView {
        self.desk.as_ref().map_or_else(settings::DeskView::default, |desk| settings::DeskView {
            background: desk.background(),
            login: desk.login(),
            asking: desk.asking(),
        })
    }

    /// Takes the desk's answer on starting with the computer, when it came: a refusal is a note on the open sheet, or
    /// in the error banner once the sheet is closed.
    fn hear_desk(&mut self) {
        let Some(answer) = self.desk.as_mut().and_then(|desk| desk.answer()) else { return };
        match answer {
            Ok(()) => self.settings.set_login_note(None),
            Err(why) if self.settings.is_open() => self.settings.set_login_note(Some(tr!("Not changed: {why}.", why = why))),
            Err(why) => self.mailbox.fail(UiError::Desk(why)),
        }
    }

    /// Runs in the background from now on, or not: turning it off also stops starting with the computer, whose start
    /// opens no window.
    fn set_background(&mut self, on: bool) {
        let Some(desk) = self.desk.as_mut() else { return };
        if let Err(why) = desk.set_background(on) {
            self.mailbox.fail(UiError::Desk(why));
            return;
        }
        if !on && desk.login() == Some(true) {
            desk.set_login(false);
        }
    }

    /// Asks to start with the computer, or no longer.
    fn set_login(&mut self, on: bool) {
        if let Some(desk) = self.desk.as_mut() {
            self.settings.set_login_note(None);
            desk.set_login(on);
        }
    }

    /// The triage and sending choices to keep: the inbox view, whether a pin outlasts done, the hold before a send
    /// goes, and the screener's mode but on the sample, which keeps the one kept.
    fn triage(&self) -> Triage {
        let screener = if self.mailbox.is_sample() { self.kept_triage.screener } else { self.mailbox.screener_mode() };
        Triage { screener, view: self.mailbox.inbox_view(), keep_pins: self.mailbox.pins_outlast_done(), hold: Some(self.mailbox.hold_seconds()) }
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

    /// Steps the list's keyed row `rows` below (`forward`) or above through `order`, the rows as the list shows them.
    /// After the open message left, a step of one row starts from the gap it left: down picks the row that followed it,
    /// up the row before it, whichever side of the gap the next-message rule's row stands on; at an end of the list,
    /// where no row stands on that side, the key picks nothing.
    fn step_list(&mut self, order: &[u64], forward: bool, rows: usize) {
        let held = self.held_back.take().filter(|held| rows == 1 && (self.list_row.is_none() || self.list_row == held.land));
        let Some(held) = held else {
            self.mailbox.step_in(order, forward, rows);
            return;
        };
        let listed = self.mailbox.messages();
        let Some(at) = listed.iter().position(|row| row.id == held.pick) else {
            // The rule's row left too: the step starts from the top.
            self.mailbox.step_in(order, forward, rows);
            return;
        };
        // The first row after the gap.
        let after = if held.below { at } else { at.saturating_add(1) };
        let to = if forward { Some(after) } else { after.checked_sub(1) };
        match to.and_then(|to| listed.get(to)).map(|row| row.id) {
            // Picked by a key, as a step picks.
            Some(id) => self.mailbox.select_keyed(id),
            // Nothing on that side: still nothing open, the gap kept.
            None => self.held_back = Some(held),
        }
    }

    /// The act `action` (Done, Archive, read or unread, flag or not, Delete) on the selected row.
    fn row_act(&self, action: &str) -> Option<(u64, Act)> {
        let row = self.mailbox.selected().and_then(|id| self.mailbox.messages().iter().find(|row| row.id == id))?;
        let (_, act) = ROW_ACTS.into_iter().find(|(id, _)| *id == action)?;
        let act = match act {
            Act::Read if row.seen => Act::Unread,
            Act::Flag if row.flagged => Act::Unflag,
            Act::Done if row.done => Act::Undone,
            act => act,
        };
        Some((row.id, act))
    }

    /// Whether the action `id` can run now: an act on a row needs a selected message, Undo an act in the history, the
    /// reader's full width an open message; an id the window does not know never runs.
    #[must_use]
    pub fn can_run(&self, id: &str) -> bool {
        match id {
            // With the list hidden nothing acts on a row the owner cannot see, by a key or from the palette; Undo, which
            // may take back an act on such a row, rests too.
            keymap::DONE | keymap::ARCHIVE | keymap::READ | keymap::FLAG | keymap::DELETE => !self.list_hidden && self.row_act(id).is_some(),
            // Under the event editor Undo rests too: it would take back the event being edited ([`Surfaces::undo_rests`]).
            keymap::UNDO => !self.list_hidden && !self.editing_event() && self.mailbox.can_undo(),
            keymap::FULL_WIDTH => self.reader.open_id().is_some(),
            // The held-back message opens as a press on its row does (it floats a docked composer); with the list
            // hidden it rests, as every command of the list does.
            keymap::OPEN_HELD => self.screen == Screen::Mail && !self.list_hidden && self.held_back.is_some(),
            // The search field is the list's: hidden, it is not there to take the keys.
            keymap::FIND => !self.list_hidden,
            // In one pane only, where it brings the list back in front: the list hidden beside a composer in two panes
            // has no Back.
            keymap::BACK => self.screen == Screen::Mail && self.single == Some(true) && self.in_place_of_list(),
            keymap::SNOOZE => self.screen == Screen::Mail && self.reader.open_id().is_some() && !self.docked(),
            keymap::COMPOSE => !self.mailbox.accounts().is_empty(),
            keymap::FOLD => self.composer.is_some() && !self.docked(),
            // In front in the reader's place; or the floating panel open with the keys inside it.
            keymap::SEND => self.composer_in_front() || (self.composer_keys && self.composer.as_ref().is_some_and(|composer| !composer.folded())),
            _ if keymap::CALENDAR_KEYS.contains(&id) => self.screen == Screen::Calendar && !self.calendar.is_empty(),
            // The list's keys and its views act on the list: hidden, it is not there to act on.
            _ if keymap::LIST_KEYS.contains(&id) => self.screen == Screen::Mail && !self.list_hidden && (self.list_row.is_some() || self.list_keys),
            keymap::VIEW_GROUPED | keymap::VIEW_SMART | keymap::VIEW_LIST => !self.list_hidden,
            _ => keymap::action(id).is_some(),
        }
    }

    /// Runs the action `id` of [`keymap::actions`], as its key, the rail or the palette do; an action that cannot run
    /// now ([`InboxApp::can_run`]) does nothing.
    pub fn run_action(&mut self, ctx: &egui::Context, id: &str) {
        // An action only a coming-soon control offered runs by no way in.
        if !self.can_run(id) || soon::blocked(ctx, id) {
            return;
        }
        if keymap::CALENDAR_KEYS.contains(&id) {
            self.calendar_key(ctx, id);
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
                if matches!(self.mailbox.place(), Place::View(Role::Sent) | Place::Later(_)) {
                    self.mailbox.open(Place::View(Role::Inbox));
                }
            }
            keymap::SENT => {
                self.show_mail();
                self.mailbox.open(Place::View(Role::Sent));
                self.search.clear();
            }
            keymap::LATER => {
                self.show_mail();
                if !matches!(self.mailbox.place(), Place::Later(_)) {
                    self.mailbox.open(Place::Later(Pile::Pins));
                }
                self.search.clear();
            }
            keymap::SNOOZE => self.open_picker(""),
            keymap::OPEN_HELD => {
                if let Some(held) = self.held_back.take() {
                    // The owner's open, as a press: the message in the reader, read, a docked composer floating.
                    self.mailbox.select(Some(held.pick));
                    self.reading = true;
                    self.focus_reader = one_pane(ctx);
                    self.float_behind();
                }
            }
            keymap::CALENDAR => self.show_calendar(),
            keymap::COMPOSE => {
                self.compose();
                ctx.memory_mut(|memory| memory.request_focus(compose::field_id(crate::compose::Line::To)));
            }
            keymap::SEND => self.send_composer(),
            keymap::FOLD => {
                if let Some(composer) = self.composer.as_mut() {
                    composer.toggle_fold();
                }
            }
            keymap::SETTINGS => {
                if !self.settings.is_open() {
                    self.open_settings();
                }
            }
            keymap::SHORTCUTS => self.open_shortcuts(),
            keymap::UNDO => self.undo(),
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
            keymap::VIEW_GROUPED => self.mailbox.set_inbox_view(InboxView::Grouped),
            keymap::VIEW_SMART => self.mailbox.set_inbox_view(InboxView::Smart),
            keymap::VIEW_LIST => self.mailbox.set_inbox_view(InboxView::List),
            keymap::OPEN => self.open_from_list(ctx),
            keymap::BACK => {
                let _ = self.back_to_list(ctx);
            }
            keymap::MENU | keymap::MENU_KEY => {
                if let Some(id) = self.list_row.or_else(|| self.mailbox.selected()) {
                    list::ask_menu(ctx, id);
                    ctx.request_repaint();
                }
            }
            _ if keymap::LIST_KEYS.contains(&id) => self.move_in_list(ctx, id),
            _ => {
                if let Some((row, act)) = self.row_act(id) {
                    self.act_by_key = true;
                    self.mailbox.start(row, act);
                }
            }
        }
    }

    /// Moves the list's keyed row as the list's key `id` asks (one down or up, a page down or up, the last or the
    /// first row), through the rows as the list shows them (a folded group's hidden rows left out): it is selected and
    /// takes the focus, so the focus ring and a screen reader follow it, and the list brings it into sight.
    fn move_in_list(&mut self, ctx: &egui::Context, id: &str) {
        let (forward, rows) = match id {
            keymap::NEXT | keymap::DOWN => (true, 1),
            keymap::PREVIOUS | keymap::UP => (false, 1),
            keymap::PAGE_DOWN => (true, list::page(ctx)),
            keymap::PAGE_UP => (false, list::page(ctx)),
            keymap::LAST => (true, usize::MAX),
            _ => (false, usize::MAX),
        };
        // The focused row is the keyed one; a selection made by the pointer counts from where it is.
        let landed = self.held_back.and_then(|held| held.land);
        if let Some(from) = self.list_row.filter(|from| self.mailbox.selected() != Some(*from) && landed != Some(*from)) {
            self.mailbox.select_keyed(from);
        }
        self.step_list(&list::order(ctx), forward, rows);
        // In one pane a message in front stays in front as the keys read on; behind the list (the composer's too),
        // they only walk it.
        let hidden = one_pane(ctx) && (self.docked() || !self.message_in_front());
        self.reading = !hidden && one_pane(ctx);
        if let Some(to) = self.mailbox.selected() {
            // In one pane with the list in front the reader is not shown: the keyed message is not read yet.
            if hidden {
                self.mailbox.select_by_focus(to);
            }
            ctx.memory_mut(|memory| {
                // egui read the arrow as a move of its focus when the frame began: the list's key moves it instead.
                memory.move_focus(egui::FocusDirection::None);
                memory.request_focus(list::row_id(to));
            });
        }
        self.list_keys = true;
        ctx.request_repaint();
    }

    /// `Enter` in the list: the keyed (or focused) message opens and the keyboard goes to the reader, so the list's keys
    /// rest until `Esc` there gives it back.
    fn open_from_list(&mut self, ctx: &egui::Context) {
        let Some(id) = self.list_row.or_else(|| self.mailbox.selected()) else { return };
        self.mailbox.read_preview(id);
        self.mailbox.select_keyed(id);
        ctx.memory_mut(|memory| memory.surrender_focus(list::row_id(id)));
        self.list_keys = false;
        self.reading = true;
        self.focus_reader = one_pane(ctx);
        self.float_behind();
        ctx.request_repaint();
    }

    /// In one pane, whether something stands in the list's place: the docked composer, or a message the person opened.
    fn in_place_of_list(&self) -> bool {
        self.composer_in_front() || (!self.docked() && self.message_in_front() && self.reader.open_id().is_some())
    }

    /// `Esc` in the reader gives the keyboard back to the list: its keyed row takes the focus. Whether it did.
    fn back_to_list(&mut self, ctx: &egui::Context) -> bool {
        let single = one_pane(ctx);
        let swapped = single && self.in_place_of_list();
        if self.screen != Screen::Mail || (self.list_keys && !swapped) || (self.docked() && !swapped) {
            return false;
        }
        self.list_keys = true;
        self.reading = false;
        self.list_over_composer = single && self.docked();
        if let Some(id) = self.mailbox.selected() {
            self.mailbox.select_keyed(id);
        }
        // The reader's widget with the focus goes: the keyboard lands on the row left, or the nearest in sight.
        ctx.memory_mut(|memory| {
            if let Some(focused) = memory.focused() {
                memory.surrender_focus(focused);
            }
        });
        if let Some(id) = self.row_left(ctx) {
            list::land(ctx, id);
        }
        true
    }

    /// The row `clicked` in the list is selected, read, and the list has the keyboard; a row that gained the focus
    /// (`Tab` onto it, or assistive technology) is keyed and shown as a preview, not marked read. Focus and selection
    /// move together: a row pressed while another row has the focus takes it away from that row.
    fn follow_list(&mut self, ctx: &egui::Context, clicked: Option<u64>, gained: Option<u64>) {
        if let Some(id) = clicked {
            self.mailbox.read_preview(id);
            self.mailbox.select(clicked);
            self.list_keys = true;
            self.reading = true;
            self.focus_reader = one_pane(ctx);
            self.float_behind();
            if let Some(focused) = self.list_row.filter(|focused| Some(*focused) != clicked) {
                ctx.memory_mut(|memory| memory.surrender_focus(list::row_id(focused)));
            }
        }
        // The row the keyboard landed on after the server took the open message away has the keyboard, not opened: the
        // row the landing gave it to, the nearest in sight when the asked one was scrolled away.
        if let Some(wanted) = self.landed
            && let Some(actual) = list::landed_on(ctx, wanted)
        {
            self.landed = Some(actual);
            if let Some(held) = self.held_back.as_mut().filter(|held| held.land == Some(wanted)) {
                held.land = Some(actual);
            }
        }
        if gained.is_some() && gained == self.landed {
            self.landed = None;
            self.list_keys = true;
        } else if let Some(id) = gained.filter(|id| self.mailbox.selected() != Some(*id) && clicked != Some(*id)) {
            self.landed = None;
            self.mailbox.select_by_focus(id);
            self.list_keys = true;
            // A row the focus walked to is a preview: in one pane it stays behind the list.
            self.reading = false;
        }
    }

    /// Which panes the body shows: whether it holds one pane, whether the list shows, whether the reader (or the
    /// composer in its place) does. In one pane what is in front follows what is open: nothing open, the list; a
    /// message the person opened, or the docked composer, in its place.
    fn panes(&mut self, ctx: &egui::Context, docked: bool, full: bool) -> (bool, bool, bool) {
        let single = one_pane(ctx);
        let was = self.single.replace(single);
        if self.reader.open_id().is_none() {
            self.reading = false;
        }
        // The list stands over the composer only in one pane: two panes show both, and end its turn.
        if !docked || !single {
            self.list_over_composer = false;
        }
        let message = !docked && self.message_in_front() && self.reader.open_id().is_some();
        let front = (docked && !self.list_over_composer) || message;
        let list_shown = !single || !front;
        self.keep_keyboard(ctx, was.is_some_and(|was| was != single), single, list_shown, message);
        (single, list_shown, !single || front || full)
    }

    /// The keyboard is never lost when the panes change: crossing 980 into one pane with the message in front, the
    /// focus goes to the reader's first tool; when the list comes back in front in one pane (the message gone) or
    /// the window crosses 980 with the list shown, it lands on the row left ([`list::land`]): in one pane on it or the
    /// nearest row in sight, beside the reader on it brought into sight, as the reader shows its message.
    fn keep_keyboard(&mut self, ctx: &egui::Context, crossed: bool, single: bool, list_shown: bool, message: bool) {
        let came = single && list_shown && !self.list_was_shown;
        self.list_was_shown = list_shown;
        if let Some(open) = self.reader.open_id()
            && self.left.as_ref().is_none_or(|(left, _)| *left != open)
        {
            self.left = Some((open, self.list_of()));
        }
        if self.screen != Screen::Mail {
            return;
        }
        // The list's act walked on, or the server took the open message away: the keyboard lands on the row asked.
        if let Some(id) = self.land_held.take() {
            // The row walked on to comes into sight, as a keyed one does; a removal lands in sight where the owner
            // scrolled.
            if std::mem::take(&mut self.bring_held) {
                list::bring(ctx, id);
            }
            list::land(ctx, id);
        }
        let free = ctx.memory(|memory| memory.focused().is_none()) || list::focused_row(ctx).is_some();
        if crossed && single && message && free {
            self.focus_reader = true;
        }
        if (crossed || came)
            && list_shown
            && let Some(id) = self.row_left(ctx)
        {
            if !single {
                list::bring(ctx, id);
            }
            list::land(ctx, id);
        }
    }

    /// Which list is shown: its place, the account it is narrowed to, its search.
    fn list_of(&self) -> ListOf {
        (self.mailbox.place(), self.mailbox.narrowed(), self.mailbox.query().to_owned())
    }

    /// The row the keyboard goes back to: the selected message's, or once it has gone (the reader showed it last),
    /// the row next to where it stood in the list as last drawn, below it first. None when the list itself changed
    /// since (another place, account or search): its rows are not near the message, and the person keys one.
    fn row_left(&self, ctx: &egui::Context) -> Option<u64> {
        let listed = |id: &u64| self.mailbox.messages().iter().any(|row| row.id == *id);
        if let Some(id) = self.mailbox.selected().filter(listed) {
            return Some(id);
        }
        let (gone, from) = self.left.as_ref()?;
        let (place, narrowed, query) = from;
        if *place != self.mailbox.place() || *narrowed != self.mailbox.narrowed() || query != self.mailbox.query() {
            return None;
        }
        let gone = *gone;
        let order = list::order(ctx);
        let at = order.iter().position(|id| *id == gone)?;
        let below = order.get(at.saturating_add(1)..).unwrap_or_default();
        let above = order.get(..at).unwrap_or_default();
        below.iter().chain(above.iter().rev()).copied().find(listed)
    }

    /// A message opened into the one pane: the focus goes to the reader's first tool, not nowhere.
    fn focus_reader_tool(&mut self, ctx: &egui::Context) {
        let back = reader::take_back_id(ctx);
        let walked = std::mem::take(&mut self.focus_walked) && ctx.memory(|memory| memory.focused().is_none());
        if (std::mem::take(&mut self.focus_reader) || walked)
            && let Some(back) = back
        {
            ctx.memory_mut(|memory| memory.request_focus(back));
        }
    }

    /// A message the owner opens from the list (a press on its row, Enter) takes the docked composer's place, in one
    /// pane or two, also when it was already selected as a preview: the composer floats, all it holds kept.
    fn float_behind(&mut self) {
        if self.docked()
            && let Some(composer) = &mut self.composer
        {
            composer.set_floating(true);
        }
    }

    /// Whether the window held one pane in the last frame and the list stood in front there: of the docked composer
    /// (`Esc` in it), or of a message the keys or the focus walked to.
    fn list_in_front(&self) -> bool {
        self.single == Some(true) && if self.docked() { self.list_over_composer } else { !self.message_in_front() }
    }

    /// In one pane, whether the open message stands in the list's place: one the person opened (`Enter`, a press) or
    /// one selected from outside (a script); never one the keys or the focus walked to, which stays behind the list.
    fn message_in_front(&self) -> bool {
        self.reading || !self.mailbox.keyed()
    }

    /// Whether the calendar shows its event editor ([`Surface::Editor`]).
    fn editing_event(&self) -> bool {
        self.screen == Screen::Calendar && self.calendar.editor().is_some()
    }

    /// What stands in front this frame, read before its keys ([`Surfaces`]).
    fn surfaces(&self, ctx: &egui::Context) -> Surfaces {
        let mail = self.screen == Screen::Mail;
        Surfaces::new(Facts {
            float: crate::component::popup::any_open(ctx),
            palette: self.palette.is_open(),
            capture: self.settings.capturing().is_some(),
            sheet: self.settings.is_open(),
            editor: self.editing_event(),
            composer: self.composer_in_front(),
            full_width: self.reader.full_width(),
            one_pane_reader: mail && one_pane(ctx) && !self.docked() && self.message_in_front() && self.reader.open_id().is_some(),
            mail,
        })
    }

    /// Whether the list has the keyboard: it may have the keys ([`Surfaces::list_has_keyboard`]: on the mail, shown,
    /// nothing in front that takes them), and a row has the focus, or a row was pressed or keyed last and nothing else
    /// has the focus.
    fn list_has_keyboard(&self, ctx: &egui::Context, surfaces: &Surfaces) -> bool {
        if !surfaces.list_has_keyboard() {
            return false;
        }
        if self.list_row.is_some() {
            return true;
        }
        let focused = ctx.memory(|memory| memory.focused().is_some());
        self.list_keys && !focused
    }

    /// Opens the time picker over the open message in the next frame, `typed` in its field: Snooze, or "Remind me if no
    /// reply" for a message the owner sent. The presets still ahead are read now.
    pub fn open_picker(&mut self, typed: &str) {
        self.mailbox.load_presets();
        self.picker = Some(typed.to_owned());
    }

    /// Opens the menu `name` in the next frame (the `ui_shot` scripts): the reader's `more` or `move`, the composer's
    /// `from` or `templates`, or the selected row's `group` menu; whether there is such a menu.
    pub fn open_menu(&mut self, name: &str) -> bool {
        let id = match name {
            "more" => reader::more_menu(),
            "move" => reader::move_menu(),
            "from" => compose::from_menu(),
            "templates" => compose::templates_menu(),
            "group" => match self.mailbox.selected() {
                Some(row) => list::group_menu_id(row),
                None => return false,
            },
            _ => return false,
        };
        self.menu = Some(id);
        true
    }

    /// Opens the composer's send-later picker in the next frame, its presets read now (the `ui_shot` scripts).
    pub fn open_send_later(&mut self) {
        self.mailbox.load_presets();
        self.later_picker = true;
    }

    /// Opens the time picker asked for by [`InboxApp::open_picker`], before the reader is drawn, and the composer's
    /// send-later picker asked for by [`InboxApp::open_send_later`].
    fn show_picker(&mut self, ctx: &egui::Context) {
        if let Some(menu) = self.menu.take() {
            crate::component::popup::set_open(ctx, menu, true);
        }
        if std::mem::take(&mut self.later_picker) {
            crate::component::popup::set_open(ctx, compose::later_popup(), true);
        }
        let Some(typed) = self.picker.take() else { return };
        let mut state = crate::component::time_picker::TimePicker::default();
        state.set_typed(&typed);
        ctx.data_mut(|data| data.insert_temp(reader::picker_id(), state));
        crate::component::popup::set_open(ctx, reader::picker_popup(), true);
    }

    /// Shows the calendar screen, leaving the settings sheet; its calendars and events are read again.
    pub fn show_calendar(&mut self) {
        self.close_settings();
        self.screen_to(Screen::Calendar);
        self.calendar.refresh(&mut self.mailbox);
    }

    /// Shows the calendar screen on the occurrence of event `id` at `start`, picked, as a click on its alert asks.
    pub fn open_event(&mut self, id: u64, start: inboxcraft_time::Timestamp) {
        self.show_calendar();
        self.calendar.open_event(&mut self.mailbox, id, start);
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
                calendar::Asked::New => calendar.new_event(),
                calendar::Asked::Edit => calendar.edit_picked(),
                calendar::Asked::Delete => calendar.delete_picked(mailbox),
                calendar::Asked::Save => calendar.save(mailbox),
                calendar::Asked::Cancel => calendar.close_editor(),
                calendar::Asked::Leave(leave) => calendar.leave_editor(leave == crate::component::popup::Leave::Escape),
                calendar::Asked::More(day) => calendar.set_more(day),
                calendar::Asked::PickDay(day) => calendar.pick_day(Some(day)),
            }
        }
    }

    /// Before the calendar screen is drawn: the time now for its now line, and its own keys.
    fn calendar_before(&mut self, ctx: &egui::Context, setting: bool) {
        if self.screen != Screen::Calendar {
            return;
        }
        self.calendar.tick(self.mailbox.now(), self.mailbox.offset());
        let open = setting || self.palette.is_open() || self.settings.capturing().is_some();
        for id in calendar::keys::pressed(ctx, &self.keymap, &self.calendar, open) {
            self.calendar_key(ctx, id);
        }
        calendar::keys::begin(ctx);
    }

    /// Runs the calendar's key `id` ([`keymap::CALENDAR_KEYS`]): moves the pick, opens it or leaves it, and hands the
    /// keyboard to what is picked.
    fn calendar_key(&mut self, ctx: &egui::Context, id: &str) {
        use crate::calendar::Way;
        let (calendar, mailbox) = (&mut self.calendar, &mut self.mailbox);
        let mini = calendar.picked_day().is_some_and(|day| ctx.memory(|memory| memory.focused()) == Some(calendar::mini_id(day)));
        let target = match id {
            keymap::CALENDAR_LEFT => calendar.move_pick(mailbox, Way::Left, mini),
            keymap::CALENDAR_RIGHT => calendar.move_pick(mailbox, Way::Right, mini),
            keymap::CALENDAR_UP => calendar.move_pick(mailbox, Way::Up, mini),
            keymap::CALENDAR_DOWN => calendar.move_pick(mailbox, Way::Down, mini),
            keymap::CALENDAR_OPEN => calendar.open_pick(mailbox, mini),
            _ => {
                calendar.leave_pick();
                ctx.memory_mut(|memory| {
                    if let Some(focused) = memory.focused() {
                        memory.surrender_focus(focused);
                    }
                });
                None
            }
        };
        if let Some(target) = target {
            calendar::keys::want(ctx, target);
            // The key moved the pick: egui does not move the focus on as well.
            ctx.memory_mut(|memory| memory.move_focus(egui::FocusDirection::None));
        }
    }

    /// Opens the settings sheet on Shortcuts, keeping the account forms as they are typed, all but the password.
    pub fn open_shortcuts(&mut self) {
        self.onboarding.close();
        self.settings.open_at(Section::Shortcuts);
    }

    /// Shows the mail, leaving the settings sheet and the calendar.
    fn show_mail(&mut self) {
        self.close_settings();
        self.screen_to(Screen::Mail);
    }

    /// Shows `screen`. A composer in the reader's place moves to the floating panel with the calendar and back with the
    /// mail: in its new place it has not been seen yet.
    fn screen_to(&mut self, screen: Screen) {
        let was = self.docked();
        self.screen = screen;
        if was != self.docked()
            && let Some(composer) = self.composer.as_mut()
        {
            composer.unsee();
        }
    }

    /// Gives the action waiting on the Shortcuts page the key pressed in this frame, taking it from the action that had
    /// it, or stops the wait on Escape. The press reaches nothing else.
    fn capture_key(&mut self, ctx: &egui::Context) {
        let Some(waiting) = self.settings.capturing() else { return };
        match keymap::captured(ctx) {
            Some(Captured::Cancel) => self.settings.captured(None),
            Some(Captured::Key(shortcut)) => {
                let had = self.keymap.clash(waiting, shortcut).and_then(keymap::action);
                self.keymap.bind(waiting, shortcut);
                let note = had.map(|had| {
                    tr!("{key} was the key of {action}; it has none now.", key = crate::component::key::key_words(&shortcut), action = had.name)
                });
                self.settings.captured(note);
            }
            None => {}
        }
    }

    /// The keys of the frame, read before its drawing, with `surfaces` in front ([`Surfaces`]). While the Shortcuts page
    /// waits for a key or the palette is open, the keys are theirs; the settings sheet, the palette and the calendar lie
    /// over the list or in its place, so the mail's keys (the arrows, the acts on a row) rest while they show.
    fn keys(&mut self, ctx: &egui::Context, surfaces: &Surfaces) -> Keys {
        // Send's key acts only from inside the composer, open and not under the settings sheet: never from a field in
        // front of it or beside it.
        self.composer_keys =
            !surfaces.has(Surface::Sheet) && self.composer.as_ref().is_some_and(|composer| !composer.folded()) && compose::has_keys(ctx);
        let held = surfaces.held();
        self.capture_key(ctx);
        // Read before anything returns: a row that lost the focus while the palette was open is not the list's row.
        self.list_row = if held { None } else { list::focused_row(ctx) };
        // A row with the focus is the list's keyed row: it quiets no key.
        let focused = ctx.memory(|memory| memory.focused().is_some()) && self.list_row.is_none();
        // A field with the focus, or a surface whose fields the letters are for.
        let quiet = focused || surfaces.quiet();
        let typing = quiet || self.screen == Screen::Calendar;
        if held {
            return Keys { ran: Vec::new(), escape: None, to_place: false, typing, quiet };
        }
        let listing = self.list_has_keyboard(ctx, surfaces);
        let ran = self.pressed(ctx, surfaces, focused, listing);
        // Esc goes to the topmost surface only: a list of choices open over the window takes it, not what lies under.
        let escape = surfaces.escape(ctx);
        let to_place = escape.is_none() && ctx.input(|input| input.key_pressed(Key::Escape));
        Keys { ran, escape, to_place, typing, quiet }
    }

    /// The actions whose key was pressed in this frame and that may run with `surfaces` in front
    /// ([`Surfaces::allows`]), besides Undo while a toast with Undo shows (the toast takes it). While a text field is
    /// `focused` a key without Command, Ctrl or Alt runs nothing, so a letter typed is typed, and Undo is the field's;
    /// the list's keys run only while it has the keyboard (`listing`). A key that may not run is not taken.
    fn pressed(&self, ctx: &egui::Context, surfaces: &Surfaces, focused: bool, listing: bool) -> Vec<&'static str> {
        let toasted = self.mailbox.undoable().is_some_and(|undo| undo.undoable) || self.counting().is_some();
        keymap::actions()
            .into_iter()
            .map(|action| action.id)
            // The acts on a row (the list takes those), the calendar's keys (`calendar::keys`), what the sheet, the
            // palette, a capture or the composer in front keep from the window.
            .filter(|id| surfaces.allows(id, &self.keymap))
            .filter(|id| *id != keymap::UNDO || !(focused || toasted))
            .filter(|id| *id != keymap::SEND || self.composer_keys)
            .filter(|id| {
                // The list's keys run while the list has the keyboard, and never else: a field keeps its letters, its
                // Home and End, the reader its open message.
                if keymap::LIST_KEYS.contains(id) {
                    return listing && self.keymap.pressed(ctx, id);
                }
                let bare = self.keymap.shortcut(id).is_some_and(|key| {
                    let held = key.modifiers;
                    !(held.command || held.ctrl || held.mac_cmd || held.alt)
                });
                !(focused && bare) && self.keymap.pressed(ctx, id)
            })
            .filter(|id| !soon::blocked(ctx, id))
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

    /// The settings sheet, to drive from outside (the `ui_shot` scripts, later the control channel).
    pub fn settings_mut(&mut self) -> &mut Settings {
        &mut self.settings
    }

    /// Opens the settings sheet on Appearance, keeping the account forms as they are typed, all but the password.
    pub fn open_settings(&mut self) {
        self.onboarding.close();
        self.settings.open();
    }

    /// Opens the settings sheet on Accounts, where accounts are added.
    pub fn open_accounts(&mut self) {
        self.settings.open_at(Section::Accounts);
    }

    /// Closes the settings sheet, keeping the account forms on it as they are typed, all but the password.
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

    /// Asks `ctx` for a frame when the engine next has something to do ([`Mailbox::due_in`]): an idle window draws
    /// none of its own, and the engine only ticks as the window pumps. eframe runs [`InboxApp::ui`] for a shown window
    /// and only its logic for a hidden one, so both call this.
    pub fn wake_when_due(&mut self, ctx: &egui::Context) {
        if let Some(left) = self.mailbox.due_in() {
            ctx.request_repaint_after(left);
        }
    }

    /// Hands the engine's jobs to the workers and applies their events; the check's steps are read again when
    /// anything changed. Whether anything changed.
    pub fn pump(&mut self) -> bool {
        let changed = self.mailbox.pump(&mut *self.workers);
        let late = self.mailbox.take_late();
        match late.as_slice() {
            [] => {}
            [subject] => {
                let subject = if subject.is_empty() { tr!("(no subject)") } else { subject.as_str() };
                self.say(&tr!("Sent late, InboxCraft was not running at its time: {subject}", subject = subject));
            }
            more => self.say(&tr!("Sent {count} messages late: InboxCraft was not running at their time", count = more.len())),
        }
        if changed {
            self.onboarding.load(&mut self.mailbox);
            if self.screen == Screen::Calendar {
                self.calendar.refresh(&mut self.mailbox);
            }
            self.reader.refresh_invites(&mut self.mailbox);
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

    /// Opens the selected message in the reader, or closes it, when the selection changed. A preview never floats a
    /// docked composer, in one pane or two: a selection the focus made (Tab, Shift Tab), in one pane with the list over
    /// the composer the list's keys too (the arrows, J and K; in two panes they rest beside it), the walk-on after an
    /// act or an Undo's selection only moves the list's selection; the composer keeps the reader's place and the
    /// message opens behind it as a preview, not read. Only the owner's open floats it: a press on a row, Enter, the
    /// held-back message opened as a press opens it ([`InboxApp::float_behind`]), or a message selected from outside.
    ///
    /// When the owner's own act takes the open message out of the list (done, snoozed, moved), the list walks on to the
    /// row the next-message rule selects; when the server takes it away, the reader shows nothing open and that row is
    /// not opened by itself, only when the owner picks a message. The keyboard goes to the row that took its place, in
    /// the grouped inbox in the same card ([`InboxApp::landing_after_removal`]).
    fn sync(&mut self) {
        let open = self.reader.open_id();
        let gone = open.is_some_and(|id| !self.mailbox.messages().iter().any(|row| row.id == id));
        // The owner's own act that took the open message away (Done, Archive, Delete, Snooze, Move, from the list or
        // the reader, by key or by button) walks on to the rule's row, as the lists do; in one pane an act from the
        // reader brings the list back in front with the keyboard on that row. The server, another device, a sync or a
        // rule taking it away opens nothing: the keyboard goes to the row that took its place.
        let taken = self.mailbox.take_taken();
        let owner = gone && taken.is_some() && taken == open;
        let from_list = self.list_keys || self.list_row == open;
        let walking = owner && (from_list || self.single != Some(true));
        if walking && from_list {
            // The keyboard stays on the list: on the row walked on to.
            self.land_held = self.mailbox.selected();
            self.bring_held = true;
        }
        // Beside the docked composer the reader is not drawn: nothing asks for its tool, and a focus in the composer
        // is left alone.
        if walking && !from_list && !self.docked() && std::mem::take(&mut self.act_by_key) {
            // The keyboard stays in the reader: on the widget it was on, else (a key acted with nothing focused) on the
            // reader's first tool, never on nothing.
            self.focus_walked = true;
        }
        if walking && !from_list && self.docked() {
            // Nor on nothing beside the docked composer: on the row walked on to, unless a widget keeps the focus (the
            // composer's text).
            self.land_held = self.mailbox.selected();
            self.bring_held = true;
        }
        // Read by the first sync after the act, which sees what it took away: it counts once.
        self.act_by_key = false;
        // Only where the list is the one the message was opened from: another account, place or search is no removal.
        let same_list = self.left.as_ref().is_some_and(|(left, from)| Some(*left) == open && *from == self.list_of());
        if let Some(gone) = open.filter(|_| gone && !owner && same_list) {
            self.land_held = self.landing_after_removal(gone);
            self.landed = self.land_held;
            self.bring_held = false;
        }
        if let (true, false, Some(pick)) = (gone, walking, self.mailbox.selected()) {
            // The rule's row is kept, not selected: no act lands on a message that is not shown.
            let at = self.mailbox.messages().iter().position(|row| row.id == pick);
            let below = match (at, self.mailbox.gap()) {
                (Some(at), Some(gap)) => at >= gap,
                _ => self.mailbox.after() != crate::mailbox::After::Previous,
            };
            self.held_back = Some(Held { pick, below, land: self.land_held });
            self.mailbox.select(None);
            // Nothing is open to read on: the list's keys (the arrows) step from the gap.
            self.list_keys = true;
        } else if self.mailbox.selected().is_some() {
            self.held_back = None;
        }
        let opening = self.mailbox.selected();
        // Read by the first sync after the Undo, which sees what it gave back.
        let restored = std::mem::take(&mut self.restored);
        if open != opening {
            let preview = walking || restored || (self.mailbox.keyed() && !self.reading);
            // In one pane with the list in front, and behind the docked composer, the reader is not shown: a message
            // selected there (the next one after an act on the keyed row, from the keys, the pointer, a notification
            // or the engine; a preview beside the composer) opens as a preview, not marked read.
            if let Some(id) = opening
                && (self.list_in_front() || (preview && self.docked()))
            {
                self.mailbox.select_by_focus(id);
            }
            if opening.is_some()
                && !preview
                && let Some(composer) = &mut self.composer
            {
                composer.set_floating(true);
            }
            self.reader.open(&mut self.mailbox, opening);
            self.hover = None;
            self.card = opening.and_then(|id| {
                let groups = self.mailbox.groups();
                (!groups.is_empty()).then(|| groups.into_iter().find(|group| group.messages.contains(&id)).map(|group| group.name))
            });
        }
    }

    /// The row the keyboard goes to when the server took the open message `gone` away: the row under it as the list
    /// last drew it, else the row above it; in the grouped inbox the rows of its own card first, so the keyboard stays
    /// in that card (a card the message was the last of went with it). A row a folded card hides is not drawn and is
    /// left out. Before the list was drawn with it: the row that took its place in the list, or the last.
    fn landing_after_removal(&self, gone: u64) -> Option<u64> {
        let rows = self.mailbox.messages();
        let listed = |id: &&u64| rows.iter().any(|row| row.id == **id);
        let Some(at) = self.drawn.iter().position(|id| *id == gone) else {
            let gap = self.mailbox.gap().unwrap_or(usize::MAX);
            return rows.get(gap).or_else(|| rows.last()).map(|row| row.id);
        };
        let (above, below) = (self.drawn.get(..at).unwrap_or_default(), self.drawn.get(at.saturating_add(1)..).unwrap_or_default());
        // The groups are read only here, when the server took the open message away, not in every frame.
        let groups = if self.card.is_some() { self.mailbox.groups() } else { Vec::new() };
        let in_card = |id: &&u64| match &self.card {
            Some(Some(name)) => groups.iter().any(|group| group.name == *name && group.messages.contains(id)),
            Some(None) => !groups.iter().any(|group| group.messages.contains(id)),
            None => true,
        };
        let near = |card: &dyn Fn(&&u64) -> bool| below.iter().filter(listed).find(card).or_else(|| above.iter().rev().filter(listed).find(card));
        near(&in_card).or_else(|| near(&|_| true)).copied()
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

    /// Whether the composer has the reader's place and is shown there: not floating, and in one pane not behind the
    /// list. Only then do its keys (Send) run and the list's rest.
    fn composer_in_front(&self) -> bool {
        self.docked() && !self.list_over_composer
    }

    /// Whether the composer has the reader's place.
    fn docked(&self) -> bool {
        self.screen == Screen::Mail && self.composer.as_ref().is_some_and(|composer| !composer.floating())
    }

    /// Writes a new message in the reader's place (`compose.new`), or brings back the one being written: there is one
    /// at a time.
    pub fn compose(&mut self) {
        self.show_mail();
        self.list_over_composer = false;
        self.reader.set_full_width(false);
        if let Some(composer) = &mut self.composer {
            composer.set_floating(false);
            return;
        }
        match Composer::start(&mut self.mailbox) {
            Ok(composer) => {
                self.composer = Some(composer);
                self.list_over_composer = false;
            }
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
                self.list_over_composer = false;
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

    /// Sends the message being written: after the hold, the countdown toast showing (`mail.send`), or at the time
    /// picked, a toast saying when (`mail.send_later`). A blocked one stays, saying why, and so does one the engine
    /// refuses.
    pub fn send_composer(&mut self) {
        // A composer opened or moved in this frame has not been seen yet: Send acts once it has been painted.
        let frame = self.frame;
        let Some(composer) = self.composer.as_mut().filter(|composer| composer.seen(frame)) else { return };
        let (draft, later) = (composer.draft(), composer.later());
        match composer.send(&mut self.mailbox) {
            Ok(Some(at)) => {
                self.composer = None;
                if later.is_some() {
                    let when = crate::component::typed_time::long(at, self.mailbox.now(), self.mailbox.offset());
                    self.say(&tr!("Scheduled for {when}", when = when));
                } else {
                    self.sending = Some(draft);
                }
                // The engine holds the message now; it goes to the worker once the hold is over.
                self.pump();
            }
            Ok(None) => {}
            Err(error) => self.mailbox.fail(error),
        }
    }

    /// The draft whose send the countdown toast shows and the whole seconds left, while it waits for its hold.
    #[must_use]
    pub fn counting(&self) -> Option<(u64, u64)> {
        let draft = self.sending?;
        self.mailbox.seconds_left(draft).map(|seconds| (draft, seconds))
    }

    /// Takes the waiting send of `draft` back (`mail.unsend`) and opens its draft again as it was, in the reader's place;
    /// the draft being written is closed first and kept. One the engine does not take back stays, saying why.
    pub fn unsend(&mut self, draft: u64) {
        if self.sending == Some(draft) {
            self.sending = None;
        }
        if let Err(error) = self.mailbox.unsend(draft) {
            self.mailbox.fail(error);
            return;
        }
        self.reopen(draft);
    }

    /// Opens the kept draft `draft` in the reader's place (a refused one, to change it); the draft being written is
    /// closed first and kept, and one the engine does not take stays open instead.
    pub fn reopen(&mut self, draft: u64) {
        if self.composer.as_ref().is_some_and(|composer| composer.draft() == draft) {
            return self.compose();
        }
        if self.composer.is_some() {
            self.close_composer();
            if self.composer.is_some() {
                return;
            }
        }
        self.show_mail();
        self.reader.set_full_width(false);
        match Composer::open(&mut self.mailbox, draft) {
            Ok(composer) => {
                self.composer = Some(composer);
                self.list_over_composer = false;
            }
            Err(error) => self.mailbox.fail(error),
        }
    }

    /// Sends the refused draft `draft` again (`outbox.retry`), the countdown toast showing its hold.
    pub fn retry(&mut self, draft: u64) {
        match self.mailbox.retry(draft) {
            Ok(_) => self.sending = Some(draft),
            Err(error) => self.mailbox.fail(error),
        }
    }

    /// Applies what was asked of a send in the outbox.
    fn outbox(&mut self, draft: u64, asked: list::Outbox) {
        match asked {
            list::Outbox::Unsend => self.unsend(draft),
            list::Outbox::Edit => self.reopen(draft),
            list::Outbox::Retry => self.retry(draft),
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
            Asked::Fold => return self.run_action(ctx, keymap::FOLD),
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
            // What these put in changes the draft after it was painted: Send waits for the next frame to show it.
            Asked::From(account) => {
                composer.unsee();
                composer.set_account(mailbox, account)
            }
            Asked::Template(at) => {
                composer.unsee();
                match composer.templates().get(at).cloned() {
                    Some(name) => composer.use_template(mailbox, &name),
                    None => Ok(()),
                }
            }
            Asked::Fill => {
                composer.unsee();
                ctx.memory_mut(|memory| memory.request_focus(compose::blank_id()));
                composer.fill(mailbox)
            }
            Asked::Later(at) => {
                composer.set_later(at);
                Ok(())
            }
            Asked::Presets => {
                mailbox.load_presets();
                Ok(())
            }
            Asked::Send | Asked::Close | Asked::Discard | Asked::Open | Asked::Fold => Ok(()),
        };
        if let Err(error) = result {
            self.mailbox.fail(error);
        }
    }

    /// The narrowest the list beside the docked composer gets, `left` its pane's left margin; beside the reader, any (0).
    /// [`list::least_width`] is measured once and kept while the mailbox's [`Mailbox::revision`], whether something is
    /// searched (Clear), the text size and the scale stay the same.
    fn list_least(&mut self, ui: &Ui, theme: &Theme, docked: bool, left: f32) -> f32 {
        if !docked {
            return 0.0;
        }
        let key = (self.mailbox.revision(), self.search.is_empty(), theme.fonts.title.size.to_bits(), ui.pixels_per_point().to_bits());
        let least = match self.list_least {
            Some((kept, least)) if kept == key => least,
            _ => {
                let least = list::least_width(ui, theme, &self.mailbox, &self.search);
                self.list_least = Some((key, least));
                #[cfg(test)]
                {
                    self.measured = self.measured.saturating_add(1);
                }
                least
            }
        };
        left + least
    }

    /// Notes whether the list is `hidden` this frame. When it goes, its floats (a row's time picker, a group's menu)
    /// close, and the keyboard in it or in one of them goes to the composer that took its place: nothing of the hidden
    /// list keeps it.
    fn hide_list(&mut self, ctx: &egui::Context, hidden: bool) {
        self.list_hidden = hidden;
        if !hidden {
            self.keys_to_text = 0;
            return;
        }
        // Drawn the frame before: it goes now. Its floats close first and do not come back with it.
        if let Some(list) = self.list_rect.take() {
            let floated = list::close_floats(ctx, &self.mailbox);
            let focused = ctx.memory(|memory| memory.focused()).and_then(|id| ctx.read_response(id));
            let in_list = focused.is_some_and(|response| list.contains_rect(response.rect));
            if in_list || (floated && !compose::has_keys(ctx)) {
                // This frame and the next: a float shown the frame before still holds the keys from what lies under it.
                self.keys_to_text = 2;
            }
        }
        if self.keys_to_text > 0 {
            self.keys_to_text = self.keys_to_text.saturating_sub(1);
            if ctx.memory(|memory| memory.focused()) != Some(compose::text_id()) {
                ctx.memory_mut(|memory| memory.request_focus(compose::text_id()));
                // Drawn again with the keys where they are now.
                ctx.request_repaint();
            }
        }
    }

    /// The reader on a card `left` from the list: the open message, or nothing open; what was asked in it.
    fn reader_card(&mut self, ui: &mut Ui, theme: &Theme, layers: &mut Panes, left: f32) -> reader::Drawn {
        let gap = 0.0;
        if self.reader.open_id().is_none() {
            CentralPanel::no_frame().frame(around(left, gap, gap)).show(ui, |ui| {
                layers.card(ui, Margin::ZERO, |ui| reader::nothing_open(ui, theme, &self.keymap));
            });
            return reader::Drawn::default();
        }
        let hover = self.hover.take();
        let shown = CentralPanel::no_frame().frame(around(left, gap, gap)).show(ui, |ui| {
            layers
                .card(ui, Margin::ZERO, |ui| reader::show(ui, theme, &self.keymap, &self.mailbox, &self.reader, &mut self.textures, hover.as_deref()))
        });
        // A press in the reader: someone reads there, and the list's keys rest until a row is pressed or keyed.
        let reader = shown.response.rect;
        self.focus_reader_tool(ui.ctx());
        #[cfg(test)]
        ui.data_mut(|data| data.insert_temp(egui::Id::unique("reader-rect"), reader));
        if ui.input(|input| input.pointer.any_pressed() && input.pointer.press_origin().is_some_and(|at| reader.contains(at))) {
            self.list_keys = false;
        }
        let mut drawn = shown.inner;
        if drawn.hover != hover {
            ui.ctx().request_repaint();
        }
        self.hover = drawn.hover.take();
        drawn
    }

    /// The composer on a card in the reader's place; what was asked in it.
    fn composer_card(&mut self, ui: &mut Ui, theme: &Theme, layers: &mut Panes, left: f32) -> Vec<compose::Asked> {
        let (keymap, mailbox) = (&self.keymap, &self.mailbox);
        let Some(composer) = self.composer.as_mut() else { return Vec::new() };
        CentralPanel::no_frame()
            .frame(around(left, theme.spacing.s2, theme.spacing.s2))
            .show(ui, |ui| layers.card(ui, Margin::ZERO, |ui| compose::show(ui, theme, keymap, mailbox, composer)))
            .inner
    }

    /// The composer's floating panel while it is not `docked`; what was asked in it.
    fn floating_composer(&mut self, ui: &Ui, theme: &Theme, docked: bool) -> Vec<compose::Asked> {
        match self.composer.as_mut() {
            Some(composer) if !docked => {
                // Over the reader's answers when it drew them in this pass: the panel never covers them.
                let above = reader::answers_rect(ui.ctx()).map(|answers| answers.top());
                compose::floating(ui.ctx(), theme, &self.keymap, &self.mailbox, composer, ui.ctx().content_rect(), above)
            }
            _ => Vec::new(),
        }
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
        self.keep_draft(ctx);
    }

    /// Keeps the draft when something in it changed, silently: at once when no field of the composer has the keys or
    /// the window is closing, otherwise at most once in [`KEEP_EVERY`], a frame asked for when the rest is due.
    fn keep_draft(&mut self, ctx: &egui::Context) {
        if !self.composer.as_ref().is_some_and(Composer::changed) {
            return;
        }
        let now = ctx.input(|input| input.time);
        let typing = ctx.memory(|memory| memory.focused()).is_some_and(compose::is_field);
        let closing = ctx.input(|input| input.viewport().close_requested());
        // A time that went back, or is no number, keeps it now.
        let wait = self.kept_at.map_or(0.0, |at| KEEP_EVERY - (now - at));
        if typing && !closing && wait > 0.0 && wait <= KEEP_EVERY {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(wait));
            return;
        }
        self.kept_at = Some(now);
        self.keep_draft_now();
    }

    /// Keeps the draft at once if something in it changed since it was last kept: the end of a window or of the app.
    pub fn keep_draft_now(&mut self) {
        if let Some(composer) = self.composer.as_mut().filter(|composer| composer.changed())
            && let Err(error) = composer.save(&mut self.mailbox)
        {
            self.mailbox.fail(error);
        }
    }

    /// Where every toast stands: its bottom edge over the window's bottom, centred on what is right of the rail.
    fn toast_bottom(ui: &Ui, theme: &Theme) -> egui::Pos2 {
        let window = ui.max_rect();
        let content = window.with_min_x((window.left() + pane::rail_width(theme)).min(window.right()));
        content.center_bottom() - egui::vec2(0.0, theme.spacing.s6 + theme.spacing.s1)
    }

    /// The toast of the composer's last step, while no act's toast and no countdown shows.
    fn notice_toast(&mut self, ui: &Ui, theme: &Theme) {
        let Some((text, since)) = self.notice.clone() else { return };
        if self.mailbox.undoable().is_some() || self.counting().is_some() {
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
        let bottom = Self::toast_bottom(ui, theme);
        toast::show(ui.ctx(), theme, &self.keymap, egui::Id::unique("compose-toast"), &Toast { text: &text, undo: None, rests: false }, bottom);
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
        // The list's order as last drawn, before the engine's events: a cheap handle on the kept one.
        self.drawn = list::order(ui.ctx());
        self.sync();
        if self.reader.poll() | self.pump() | self.onboarding.poll() {
            ui.ctx().request_repaint();
        }
        // What the engine's events changed is settled before this frame's keys: when the open message left with them,
        // no key acts on the row the rule picked, which nobody has seen selected.
        self.sync();
        self.wake_when_due(ui.ctx());
        self.show_picker(ui.ctx());
        self.frame = ui.ctx().cumulative_frame_nr();
        // What stands in front, once, before any key is read.
        let surfaces = self.surfaces(ui.ctx());
        let Keys { mut ran, escape, to_place, typing, quiet } = self.keys(ui.ctx(), &surfaces);
        let keyed = if typing { None } else { self.keyed_act(ui.ctx()) };
        self.act_by_key |= keyed.is_some();
        self.follow_adding();
        let adding = self.onboarding.stage() != Stage::Closed;
        // One Esc closes one surface ([`Surfaces::escape`]): an open float (a menu, a picker, a select), the palette, a
        // capture and the event editor take theirs before anything under them.
        let left_settings = escape == Some(Surface::Sheet);
        if left_settings {
            self.close_settings();
            // The sheet on top took the Esc: nothing under it sees it (the calendar's pick, the editor).
            ui.ctx().input_mut(|input| input.consume_key(egui::Modifiers::NONE, Key::Escape));
        }
        let setting = self.settings.is_open();
        // The sheet or the palette coming over the window closes the lists, menus and pickers open under it.
        crate::component::popup::covered(ui.ctx(), setting || self.palette.is_open());
        self.calendar_before(ui.ctx(), setting);
        let left_full = escape == Some(Surface::FullWidth);
        if left_full {
            self.reader.set_full_width(false);
        }
        // Esc that closes a menu or a picker in the reader closes only that: the keyboard stays the reader's.
        let back_to_list = (to_place || matches!(escape, Some(Surface::Composer | Surface::OnePaneReader))) && self.back_to_list(ui.ctx());
        // The window's ground first, under the error banner too; then the place for the panes, then everything else.
        // The panes touch; a hairline divides them.
        let corner = self.window_corner(ui.ctx());
        ui.painter().rect_filled(ui.max_rect(), corner, p.bg);
        // In the app's own frame the banner stands under the strip and beside the rail, so it comes after them.
        let early = self.error_banner(ui, &theme, corner, false);
        let mut asked = pane::Asked::default();
        let mut add = false;
        let mut header = list::Header::default();
        // The row sliding out is out when the list is not drawn.
        let mut listed = list::Asked { out: true, ..list::Asked::default() };
        let mut changed = None;
        let docked = self.docked();
        let full = self.reader.full_width() && !docked;
        // In one pane the open message (or the composer in its place) and the list swap.
        let (single, list_shown, reader_shown) = self.panes(ui.ctx(), docked, full);
        let mut layers = Panes::begin(ui.painter());
        #[cfg(test)]
        ui.data_mut(|data| data.remove::<egui::Rect>(egui::Id::unique("reader-rect")));
        let (gap, half) = (0.0, 0.0);
        let width = pane::rail_width(&theme);
        let rail = Panel::left("rail")
            .resizable(false)
            .exact_size(width)
            .show_separator_line(false)
            .frame(Frame::NONE)
            .show(ui, |ui| pane::rail(ui, &theme, &self.keymap, &self.mailbox, self.screen, setting, adding, self.rail_place(corner, s.s2)));
        // The rail draws its own edge, so the pane beside it draws none: one line between them.
        layers.edged(rail.response.rect);
        let clicked_rail = rail.inner;
        self.own_frame_parts(ui, &theme, corner);
        let late = self.error_banner(ui, &theme, corner, true);
        let (dismiss, banner) = (early.0 | late.0, early.1.or(late.1));
        layers.capped(banner);
        ran.extend(clicked_rail.actions());
        let mut on_calendar = Vec::new();
        let mut composer_left = half;
        self.list_hidden = false;
        if self.screen == Screen::Calendar {
            on_calendar = self.calendar_screen(ui, &theme, &mut layers);
        } else if self.mailbox.accounts().is_empty() {
            add = self.welcome(ui, &theme, &mut layers);
        } else if !full {
            let left;
            (asked, left) = self.folders(ui, &theme, &mut layers, docked);
            let (widest, least) = (list_widest(ui, &theme, docked), self.list_least(ui, &theme, docked, left));
            // The list stands beside the reader while a message is open, and stays, with nothing open, when the open
            // message left the list (held back); and beside the docked composer.
            let reading = self.reader.open_id().is_some() || self.held_back.is_some();
            let place = list_place(list_shown, !single && (reading || docked), docked, widest, least);
            // The list is hidden where the composer stands in its place: beside a composer too narrow for it, and in
            // one pane with the composer in front. Nothing of it acts then.
            let hidden = place == ListPlace::Hidden || (place == ListPlace::Away && self.composer_in_front());
            let (mailbox, search, keymap) = (&self.mailbox, &mut self.search, &self.keymap);
            let mut list = |ui: &mut Ui, layers: &mut Panes| {
                layers.card(ui, Margin::ZERO, |ui| {
                    // The pane's first column is the line at its left: the header and the rows start right of it.
                    let pad = margin(s.s4);
                    let searched = Panel::top("list-header")
                        .show_separator_line(false)
                        .frame(Frame::new().inner_margin(Margin { left: pad.left.saturating_add(1), ..pad }))
                        .show(ui, |ui| frame::beside(ui, &theme, |ui| list::header(ui, &theme, keymap, mailbox, search)))
                        .inner;
                    let listed = CentralPanel::no_frame()
                        .frame(Frame::new().inner_margin(Margin { left: 1, right: 0, top: 0, bottom: 0 }))
                        .show(ui, |ui| list::rows(ui, &theme, keymap, mailbox))
                        .inner;
                    (searched, listed)
                })
            };
            match place {
                ListPlace::Away if !hidden => self.list_rect = None,
                ListPlace::Away => {}
                ListPlace::Hidden => composer_left = left,
                ListPlace::Beside => {
                    // Where the panel keeps the width a drag gave it (egui's `PanelState`): the tests plant one there.
                    #[cfg(test)]
                    ui.data_mut(|data| data.insert_temp(egui::Id::unique("list-panel-id"), ui.scope_id().with_salt(egui::IdSalt::new("list"))));
                    let shown = list_panel(docked, widest).frame(around(left, half, gap)).show(ui, |ui| list(ui, &mut layers));
                    self.list_rect = Some(shown.response.rect);
                    (header, listed) = shown.inner;
                }
                ListPlace::Whole => {
                    let shown = CentralPanel::no_frame().frame(around(left, gap, gap)).show(ui, |ui| list(ui, &mut layers));
                    self.list_rect = Some(shown.response.rect);
                    (header, listed) = shown.inner;
                }
            }
            self.hide_list(ui.ctx(), hidden);
        }
        let list::Asked { clicked, acted, out, review, done_all, outbox: outboxed, screener: screener_mode, presets, gained } = listed;
        let mut drawn = reader::Drawn::default();
        let mut composed = Vec::new();
        // A texture lives no longer than the picture the reader keeps for it.
        self.textures.retain(|key, _| matches!(self.reader.picture(key), Some(PictureState::Ready(_))));
        if docked && reader_shown {
            composed = self.composer_card(ui, &theme, &mut layers, composer_left);
        } else if self.screen == Screen::Mail && (self.reader.open_id().is_some() || self.held_back.is_some()) && reader_shown && !docked {
            drawn = self.reader_card(ui, &theme, &mut layers, if full { gap } else { half });
        }
        layers.finish(ui.painter(), &theme);
        // A row's menu whose row this pass did not draw closes; an ask no row took is dropped.
        list::settle_menus(ui.ctx());
        if self.screen == Screen::Calendar {
            // The palette and the settings sheet lie over the editor: they have the keys and the pointer.
            let over = setting || self.palette.is_open();
            on_calendar.extend(calendar::editor(ui.ctx(), &theme, &self.keymap, &mut self.calendar, over));
        }
        if setting {
            changed = self.settings_sheet(ui.ctx(), &theme);
        } else {
            settings::rest(ui.ctx());
        }
        let palette = if self.palette.is_open() {
            self.palette.set_screen(if self.screen == Screen::Calendar { keymap::Scope::Calendar } else { keymap::Scope::Mail });
            let runnable: Vec<&str> =
                keymap::actions().into_iter().map(|action| action.id).filter(|id| self.can_run(id) && !soon::blocked(ui.ctx(), id)).collect();
            palette::show(ui.ctx(), &theme, &self.keymap, &mut self.palette, &runnable)
        } else {
            None
        };
        composed.extend(self.floating_composer(ui, &theme, docked));
        let (undone, unsent) = self.toasts(ui, &theme, quiet, surfaces.undo_rests());
        let acted = acted.or(keyed);
        let leaving = self.mailbox.leaving().is_some();
        let busy = dismiss
            || back_to_list
            || acted.is_some()
            || undone
            || unsent
            || outboxed.is_some()
            || screener_mode.is_some()
            || (out && leaving)
            || left_full
            || left_settings
            || !ran.is_empty()
            || palette.is_some()
            || clicked_rail.settings
            || changed.is_some()
            || asked.narrow.is_some()
            || asked.open.is_some()
            || header.searched.is_some()
            || header.open.is_some()
            || header.show_done.is_some()
            || header.view.is_some()
            || done_all.is_some()
            || review
            || clicked.is_some()
            || gained.is_some()
            || add
            || asked.compose
            || !on_calendar.is_empty()
            || !composed.is_empty()
            || !drawn.asked.is_empty();
        if busy {
            // What changed shows in the next frame.
            ui.ctx().request_repaint();
        }
        self.act(out, acted, undone, outboxed, screener_mode);
        if presets {
            // A row's time picker opened: the presets still ahead, read as the reader's picker reads them.
            self.mailbox.load_presets();
            ui.ctx().request_repaint();
        }
        if let Some(ids) = done_all {
            self.mailbox.done_all(&ids);
        }
        self.apply_calendar(on_calendar);
        for asked in drawn.asked {
            self.apply(ui, asked);
        }
        if dismiss {
            self.mailbox.dismiss_error();
        }
        self.go(&asked, header, review);
        if clicked.is_some() {
            self.held_back = None;
        }
        self.follow_list(ui.ctx(), clicked, gained);
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
        self.keep_notify();
        self.hear_desk();
        self.own_frame_over(ui.ctx(), &theme, corner);
        self.sync();
        // What runs after this pass, before the next frame, comes after everything this frame painted.
        self.frame = ui.ctx().cumulative_frame_nr().saturating_add(1);
    }

    /// Shows what was asked for this frame: an account narrowed, a place opened in the pane or by a pill of the
    /// header, "show done", a search, or "New senders" from Review on the screener card.
    fn go(&mut self, asked: &pane::Asked, header: list::Header, review: bool) {
        if let Some(account) = asked.narrow {
            self.mailbox.narrow(account);
        }
        if let Some(view) = header.view {
            self.mailbox.set_inbox_view(view);
        }
        if let Some(show) = header.show_done {
            self.mailbox.set_show_done(show);
        }
        let place = if review { Some(Place::Senders) } else { asked.open.or(header.open) };
        if let Some(place) = place {
            self.mailbox.open(place);
            self.search.clear();
        }
        if let Some(query) = header.searched {
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

    /// The error banner of the design, when there is an error: the `panel` ground, 8 above and below and 12 at the
    /// sides, its act a small link. Its last row holds the one `minus` hairline between it and what stands under it;
    /// the panes there draw no line of their own ([`Panes::capped`]). In the system's frame it runs across the window
    /// with a `minus` hairline on its top and sides. In the app's own frame (`framed`) it stands under the strip and
    /// beside the rail, whose lines are its top and left edges, and the frame's outline its right one; where it
    /// reaches the window's top (the top row) its top right corner is the window's `corner`. Whether Dismiss was
    /// pressed, and the banner's rect. Called twice a frame, before the rail and strip and after them; it draws in
    /// the call `framed` names (the own frame's or not).
    fn error_banner(&self, ui: &mut Ui, theme: &Theme, corner: f32, framed: bool) -> (bool, Option<Rect>) {
        let Some(error) = self.mailbox.error().filter(|_| self.draws_frame() == framed) else { return (false, None) };
        let p = theme.palette;
        let mut dismiss = false;
        // A corner of a few points; the clamp keeps the cast inside u8.
        let round = corner.clamp(0.0, 255.0) as u8;
        let on_top = (ui.available_rect_before_wrap().top() - ui.ctx().content_rect().top()).abs() < 0.5;
        let corners = match (framed, on_top) {
            (false, _) => egui::CornerRadius { nw: round, ne: round, sw: 0, se: 0 },
            (true, true) => egui::CornerRadius { nw: 0, ne: round, sw: 0, se: 0 },
            (true, false) => egui::CornerRadius::ZERO,
        };
        // The banner component in its `Error` kind on the window's ground: it pads itself (8 above and below, 12 at the
        // sides) and draws its ERROR label, words and link; the ground's corner and the edges are the window's, here.
        let ground = Frame::new().fill(p.panel).corner_radius(corners);
        let shown = Panel::top("error").show_separator_line(false).frame(ground).show(ui, |ui| {
            let links = [BannerLink { name: tr!("dismiss"), id: "dismiss" }];
            let (pressed, _) = banner::banner_in(ui, theme, &self.keymap, BannerKind::Error, error, &links, banner::Ground::Bare);
            dismiss = pressed.as_deref() == Some("dismiss");
        });
        let r = shown.response.rect;
        let painter = ui.painter();
        let (right, bottom) = (r.right() - 1.0, r.bottom() - 1.0);
        if !framed {
            paint::hairline(painter, r.left_top(), r.right_top(), p.minus);
            paint::hairline(painter, r.left_top(), pos2(r.left(), bottom), p.minus);
            paint::hairline(painter, pos2(right, r.top()), pos2(right, bottom), p.minus);
        }
        // Its `minus` edge at the foot is the one line between it and the panes, which draw none there.
        paint::hairline(painter, pos2(r.left(), bottom), pos2(r.right(), bottom), p.minus);
        (dismiss, Some(r))
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
    /// Beside the `docked` composer it is never wider than [`list_widest`].
    fn folders(&self, ui: &mut Ui, theme: &Theme, layers: &mut Panes, docked: bool) -> (pane::Asked, f32) {
        let s = theme.spacing;
        if self.mailbox.accounts().is_empty() {
            return (pane::Asked::default(), 0.0);
        }
        let widest = list_widest(ui, theme, docked);
        keep_owners_width(ui.ctx(), ui.scope_id().with_salt(egui::IdSalt::new("folders")), widest);
        let shown = Panel::left("folders")
            .resizable(true)
            .default_size(FOLDER_PANE)
            // However far its edge is dragged, the docked composer keeps its least width beside it.
            .max_size(widest)
            .show_separator_line(false)
            .frame(around(0.0, 0.0, 0.0))
            .show(ui, |ui| layers.card(ui, margin(s.s3), |ui| pane::show(ui, theme, &self.keymap, &self.mailbox)));
        (shown.inner, 0.0)
    }

    /// The window with no account: one card in the middle, as tall as what it says; whether Add account was pressed.
    fn welcome(&self, ui: &mut Ui, theme: &Theme, layers: &mut Panes) -> bool {
        onboard::welcome(ui, theme, &self.keymap, layers, self.can_add)
    }

    /// The calendar screen: its empty state on the calendar's card until there are calendars; what was pressed.
    fn calendar_screen(&self, ui: &mut Ui, theme: &Theme, layers: &mut Panes) -> Vec<calendar::Asked> {
        if self.calendar.is_empty() {
            calendar::empty(ui, theme, layers);
            return Vec::new();
        }
        // The palette and the settings sheet lie over the screen's floats: they have the keys.
        let over = self.palette.is_open() || self.settings.is_open();
        calendar::screen(ui, theme, &self.keymap, &self.calendar, layers, over)
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
        let desk = self.desk_view();
        let view = settings::Sheet {
            desk,
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
    /// key), Undo when `undone`, and what a button of the outbox asked of a send.
    fn act(
        &mut self,
        out: bool,
        acted: Option<(u64, Act)>,
        undone: bool,
        outboxed: Option<(u64, list::Outbox)>,
        mode: Option<inboxcraft_engine::ScreenerMode>,
    ) {
        if let Some(mode) = mode {
            self.mailbox.set_screener_mode(mode);
        }
        if let Some((draft, asked)) = outboxed {
            self.outbox(draft, asked);
        }
        if out {
            self.mailbox.left();
        }
        if let Some((id, act)) = acted {
            start(&mut self.reader, &mut self.mailbox, id, act);
        }
        if undone {
            self.undo();
        }
    }

    /// Takes the toast's act back, or the newest act of the history; the calendar is read again when it shows.
    fn undo(&mut self) {
        self.mailbox.undo();
        self.restored = self.docked();
        if self.screen == Screen::Calendar {
            self.calendar.refresh(&mut self.mailbox);
        }
        self.reader.refresh_invites(&mut self.mailbox);
    }

    /// The act whose key was pressed in this frame, on the selected row.
    fn keyed_act(&self, ctx: &egui::Context) -> Option<(u64, Act)> {
        let (action, _) = ROW_ACTS.into_iter().find(|(action, _)| self.keymap.pressed(ctx, action) && !soon::blocked(ctx, action))?;
        self.row_act(action)
    }

    /// The toasts of the frame: the countdown while a send waits for its hold, else the act's; then the composer's
    /// notice. Whether the act's Undo was pressed, and whether the countdown's took a send back. The act's Undo `rests`
    /// (its button and its key) while the event editor is open ([`Surfaces::undo_rests`]).
    fn toasts(&mut self, ui: &Ui, theme: &Theme, quiet: bool, rests: bool) -> (bool, bool) {
        let unsent = self.send_toast(ui, theme, quiet);
        let undone = if unsent || self.counting().is_some() { false } else { self.toast(ui, theme, quiet, rests) };
        self.notice_toast(ui, theme);
        (undone, unsent)
    }

    /// The countdown toast while the message sent last waits for its hold: "Sending “subject” in 4 s", with Undo and its
    /// key, which take the send back. It asks for a frame as each second ends. Whether Undo was pressed (its key works
    /// while nothing is `quiet`).
    fn send_toast(&mut self, ui: &Ui, theme: &Theme, quiet: bool) -> bool {
        let Some((draft, seconds)) = self.counting() else {
            self.sending = None;
            return false;
        };
        let subject = self.mailbox.outbox().iter().find(|row| row.draft == draft).map(|row| row.subject.clone()).unwrap_or_default();
        let text = if subject.is_empty() {
            tr!("Sending in {seconds} s", seconds = seconds)
        } else {
            tr!("Sending “{subject}” in {seconds} s", subject = subject, seconds = seconds)
        };
        // The next whole second, when the count changes.
        let left = self.mailbox.outbox().iter().find(|row| row.draft == draft).and_then(|row| match row.state {
            crate::mailbox::SendState::Waiting(Some(at)) => at.duration_since(self.mailbox.now()),
            _ => None,
        });
        let tick = left.map_or(std::time::Duration::from_secs(1), |left| {
            let part = std::time::Duration::from_nanos(u64::from(left.subsec_nanos()));
            if part.is_zero() { std::time::Duration::from_secs(1) } else { part }
        });
        ui.ctx().request_repaint_after(tick);
        let mut keys = self.keymap.clone();
        if quiet {
            keys.unbind(keymap::UNDO);
        }
        let bottom = Self::toast_bottom(ui, theme);
        let id = egui::Id::unique("send-toast").with(draft);
        let undone = toast::show(ui.ctx(), theme, &keys, id, &Toast { text: &text, undo: Some(keymap::UNDO), rests: false }, bottom);
        if undone {
            self.unsend(draft);
        }
        undone
    }

    /// The toast after the last act, rising from the bottom edge, for [`TOAST_SECONDS`]; after it Ctrl+Z still takes
    /// acts back, one at a time. Whether Undo was pressed (its key works while nothing is `quiet`: no text field has the
    /// focus, no sheet or palette is open). While Undo `rests` the toast still shows, its Undo drawn resting, and works
    /// again for what is left of its time once the reason has gone.
    fn toast(&mut self, ui: &Ui, theme: &Theme, quiet: bool, rests: bool) -> bool {
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
        if quiet || rests {
            keys.unbind(keymap::UNDO);
        }
        let bottom = Self::toast_bottom(ui, theme);
        let id = egui::Id::unique("act-toast").with(undo.serial);
        let shown = Toast { text: &undo.text, undo: undo.undoable.then_some(keymap::UNDO), rests };
        toast::show(ui.ctx(), theme, &keys, id, &shown, bottom)
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
            settings::Asked::Scale(scale) => self.set_text_scale(scale),
            settings::Asked::ReduceMotion(reduce) => self.set_reduce_motion(reduce),
            settings::Asked::Frame(frame) => self.set_frame(frame),
            settings::Asked::Section(section) => {
                self.onboarding.close();
                self.settings.show(section);
            }
            settings::Asked::Add => self.onboarding.open(),
            settings::Asked::Edit(account) => self.onboarding.edit(&mut self.mailbox, account),
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
            settings::Asked::KeepPins(keep) => self.mailbox.set_pins_outlast_done(keep),
            settings::Asked::Hold(seconds) => self.mailbox.set_hold(seconds),
            settings::Asked::Level(account, level) => self.mailbox.set_notify_level(account, level),
            settings::Asked::Returns(on) => self.mailbox.set_notify_returns(on),
            settings::Asked::Reminders(on) => self.mailbox.set_notify_reminders(on),
            settings::Asked::Background(on) => self.set_background(on),
            settings::Asked::Login(on) => self.set_login(on),
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
            onboard::Asked::Save => onboarding.save(mailbox),
            onboard::Asked::CheckAgain => onboarding.check_again(mailbox),
            onboard::Asked::Change => onboarding.change(mailbox),
            onboard::Asked::Close => onboarding.close(),
            onboard::Asked::Cancel => onboarding.cancel(),
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
        if asked == reader::Asked::Back {
            self.run_action(ui.ctx(), keymap::BACK);
            return;
        }
        let (reader, mailbox) = (&mut self.reader, &mut self.mailbox);
        match asked {
            reader::Asked::Expand(id) => reader.expand(mailbox, id),
            reader::Asked::Details(id) => reader.toggle_details(id),
            reader::Asked::Quotes(id) => reader.toggle_quotes(id),
            reader::Asked::View(id, view) => reader.set_plain(id, view == crate::component::content_bar::View::Plain),
            reader::Asked::Once(id) => reader.allow_once(mailbox, id),
            reader::Asked::Retry(id, key) => reader.retry(mailbox, id, &key),
            reader::Asked::Always(id) => reader.allow_always(mailbox, id),
            reader::Asked::Save(id, index) => reader.save(mailbox, id, index),
            reader::Asked::FullWidth(full) => reader.set_full_width(full),
            reader::Asked::Close => mailbox.select(None),
            reader::Asked::After(after) => mailbox.set_after(after),
            reader::Asked::Copy(url) => {
                reader.set_notice(tr!("Link copied: {url}", url = crate::mailbox::shown(&url)));
                ui.ctx().copy_text(url);
            }
            reader::Asked::CopySubject(subject) => {
                reader.set_notice(tr!("Subject copied").to_owned());
                ui.ctx().copy_text(subject);
            }
            reader::Asked::Dismiss => reader.dismiss_notice(),
            reader::Asked::Act(id, act) => start(reader, mailbox, id, act),
            reader::Asked::Presets => mailbox.load_presets(),
            reader::Asked::Invite(id, invite::Ask::Answer(answer)) => reader.answer_invite(mailbox, id, answer),
            reader::Asked::Invite(id, invite::Ask::Change) => reader.change_answer(id),
            reader::Asked::Invite(id, invite::Ask::Remove) => reader.remove_invite(mailbox, id),
            reader::Asked::Answer(..) => {}
            reader::Asked::Back => {}
        }
    }
}

/// Starts `act` on message `id`. Letting a sender in or blocking them reads the reader's invitation cards again:
/// whether an invitation can be answered hangs on it, and the message may stay open.
fn start(reader: &mut Reader, mailbox: &mut Mailbox, id: u64, act: Act) {
    mailbox.start(id, act);
    if matches!(act, Act::Accept { .. } | Act::Block) {
        reader.refresh_invites(mailbox);
    }
}

#[cfg(test)]
#[path = "appearance_tests.rs"]
mod appearance_tests;
#[cfg(test)]
#[path = "compose_tests.rs"]
mod compose_tests;
#[cfg(test)]
#[path = "cover_tests.rs"]
mod cover_tests;
#[cfg(test)]
#[path = "fit_tests.rs"]
mod fit_tests;
#[cfg(test)]
#[path = "frame_tests.rs"]
mod frame_tests;
#[cfg(test)]
#[path = "invite_tests.rs"]
mod invite_tests;
#[cfg(test)]
#[path = "later_tests.rs"]
mod later_tests;
#[cfg(test)]
#[path = "list_tests.rs"]
mod list_tests;
#[cfg(test)]
#[path = "narrow_tests.rs"]
mod narrow_tests;
#[cfg(test)]
#[path = "notify_tests.rs"]
mod notify_tests;
#[cfg(test)]
#[path = "onboard_tests.rs"]
mod onboard_tests;
#[cfg(test)]
#[path = "outbox_tests.rs"]
mod outbox_tests;
#[cfg(test)]
#[path = "palette_tests.rs"]
mod palette_tests;
#[cfg(test)]
#[path = "pane_tests.rs"]
mod pane_tests;
#[cfg(test)]
#[path = "reader_html_tests.rs"]
mod reader_html_tests;
#[cfg(test)]
#[path = "reader_tests.rs"]
mod reader_tests;
#[cfg(test)]
#[path = "reply_tests.rs"]
mod reply_tests;
#[cfg(test)]
#[path = "rests_tests.rs"]
mod rests_tests;
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
