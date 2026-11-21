// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The window's content, laid out in the frame's regions ([`shell::regions`]): the index on the left, the header with the search field and the message list, the
//! reader of the selected message on the right, and an error banner on top while a command's error stands. The
//! reader can fill the window (full width); Escape leaves that. A window that can add accounts shows "Add account"
//! in the empty window, which shows the settings on Accounts; accounts are added and listed there.
//! The window starts on Home ([`Screen::Home`], the brief of today, `draw/home.rs`) when there is an account, and on the
//! mail with none. The index holds the places and the mailboxes: its SETTINGS (or its key) shows the settings, a place of their own, in
//! the body; another place, a mailbox, New message or SETTINGS again leave them, and a form being typed stays as it was
//! left, all but its password.
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
//! key, from the index and from the command palette (Ctrl+K), which lists them all. The index's CALENDAR shows the
//! calendar screen ([`Screen::Calendar`]), drawn in `calendar/`, in the place of the mail. A change to an event made
//! there shows the same toast, and Undo or Ctrl+Z reads the calendar again.
//!
//! A message sent from the composer waits in the outbox for the hold: a toast counts the seconds down, and its Undo
//! (or Ctrl+Z) takes the send back and opens the draft again as it was. The outbox's rows take a send back the same
//! way, open a refused draft or send it again. A send whose time passed while InboxCraft was not running says so in a
//! toast once it went.

mod accounts;
mod calendar;
mod change;
mod picture;
pub(crate) use self::calendar::{event_board, mini_month_board};
mod command;
mod compose;
mod frame;
pub(crate) mod home;
mod host;
mod invite;
mod list;
mod onboard;
pub(crate) mod paint;
mod reader;
mod reader_window;
pub(crate) use reader::fitted_board;
mod index;
mod settings;
mod shell;
pub(crate) mod status;

pub use list::rows_built;

use std::sync::Arc;

use egui::{CentralPanel, Frame, Key, Margin, Panel, Rect, Ui, pos2};
use inboxcraft_engine::appearance::{Appearance, ComposerHost, EventEditorHost, WindowFrame};
use inboxcraft_engine::notify::Settings as NotifySettings;
use inboxcraft_model::Role;
use serde_json::{Value, json};

use crate::calendar::Calendar;
use crate::command::Command;
use crate::component::soon;
use crate::compose::Composer;
use crate::desk::Desk;
use crate::fetch::{Fetch, Offline};
use crate::keep::{Keep, NoKeep, Triage};
use crate::keymap::{self, Captured, Keymap};
use crate::mailbox::{Act, InboxView, Mailbox, Pile, Place};
use crate::motion;
use crate::onboard::{Detect, Keychain, Onboarding};
use crate::reader::{PictureState, Reader};
use crate::save::{NoSave, Save};
use crate::settings::{Section, Settings};
use crate::surface::{Facts, Surface, Surfaces};
use crate::theme::{self, Look, ModePreference, TextScale, Theme};
use crate::workers::{NoWorkers, Workers};
use crate::{UiError, tr};
use host::Host;
use paint::Panes;

/// The width of the list's rows beside the reader, in points: the design's 440, whatever the text size. The list's
/// pane is a point wider, for the line between it and the pane at its left.
pub(super) const LIST_WIDTH: f32 = 440.0;

/// A window narrower than this, in points, holds one pane beside the folders: the open message and the list swap
/// (`Enter` or a press opens, `Esc` goes back), as the design's breakpoint has it.
pub(super) const ONE_PANE: f32 = 980.0;

/// Where the keyboard last was, and whether that was in the composer.
fn last_keyboard_id() -> egui::Id {
    egui::Id::unique("window-last-keyboard")
}

/// The pattern of Home's character field.
const HOME_FIELD_SEED: u32 = 0;

/// What a press of the status line's Undo takes back: the message it was drawn on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Undone {
    /// The send of the draft, waiting for its hold.
    Send(u64),
    /// The discard of a draft from Home.
    Discard,
    /// The last act.
    Act,
    /// Nothing: the act cannot be taken back.
    Nothing,
}

impl Undone {
    /// The one exception to the gate under a modal surface ([`Surfaces::modal`]): the send's countdown and a draft's
    /// discard keep their Undo, by press, key and a screen reader's activate. Their time runs out under the surface too,
    /// and their Undo stops that loss and changes nothing else. The last act's Undo waits: its key reaches it after.
    fn outlasts_the_gate(self) -> bool {
        matches!(self, Self::Send(_) | Self::Discard)
    }
}

/// What was asked of the mail in the index this frame: an account narrowed, a mailbox opened.
#[derive(Debug, Default)]
struct MailAsked {
    narrow: Option<Option<u64>>,
    open: Option<Place>,
}

/// The screen the window shows beside the index.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Screen {
    /// Home: the brief of today, the start place when there is an account.
    Home,
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
    /// Whether the toast's Undo key rests: a control, the palette, a capture or the composer has the keys. On the
    /// settings it works while no control has them: it takes back what the toast shown names, as its button does.
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

/// What the reader's place shows: the docked draft, else the open message ([`InboxApp::reader_key`]).
type ReaderKey = (Option<u64>, Option<u64>);

/// The InboxCraft window's content over a [`Mailbox`].
pub struct InboxApp {
    mailbox: Mailbox,
    reader: Reader,
    textures: reader::Textures,
    /// Messages in windows of their own.
    windows: reader_window::Windows,
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
    /// The row an Undo selected again while the ring stood on another row: the ring goes to it.
    ring_back: Option<u64>,
    /// The card of the grouped inbox the open message stood in when it opened (`Some(None)`: the rows no group holds;
    /// `None`: no cards), so that the keyboard stays in that card when the server takes the message away.
    card: Option<Option<String>>,
    /// The messages of the list's rows in the order the list last drew them ([`list::order`]), read as each frame
    /// begins: where the keyboard goes when the server takes the open message away.
    drawn: std::sync::Arc<[u64]>,
    /// The message whose row of the list has the keyboard's focus, as this frame began: the list's keys act on it.
    list_row: Option<u64>,
    /// The row an Esc from its pointer menu gave the keyboard to, while the ring stays on it: the owner put it there, so
    /// the acts on a row take it with nothing open too ([`InboxApp::row_act`]). It ends when the ring moves.
    owner_ring: Option<u64>,
    /// A row of the list was pressed or keyed last, not the reader: with nothing else focused the list has the keyboard
    /// ([`InboxApp::list_has_keyboard`]).
    list_keys: bool,
    /// In one pane ([`ONE_PANE`]), the open message stands in the list's place: `Enter` or a press put it there, `Esc`
    /// or "Back to the list" take it away. With nothing open, or while a person walks the list, the list is in front.
    reading: bool,
    /// In one pane, the list stands in the docked composer's place (`Esc` in the composer); "New message" brings the
    /// composer back.
    list_over_composer: bool,
    /// The composer is shown in a window of its own this frame (the layout asks for it and a window can open).
    composer_window: bool,
    /// A field of the composer in its window of its own had the keyboard in the window's last pass.
    window_typing: bool,
    /// Why the composer shows docked though the layout asks for a window of its own has been said.
    window_refused_said: bool,
    /// Where the event editor shows this frame, as the layout says and the window can show it.
    editor_room: calendar::Room,
    /// Why the event editor shows full though the layout asks for a window of its own has been said.
    editor_window_refused_said: bool,
    /// The event editor's window of its own was asked to close this frame: it stays when the editor stays.
    editor_window_closed: bool,
    /// The opening of the event editor the last frame placed ([`Calendar::editor_opened`]).
    editor_opened: u64,
    /// The event editor's window is to come forward at the end of the frame: the main window asked for what it holds.
    editor_window_forward: bool,
    /// When a close of the main window was last taken back for an event's typing, in input seconds.
    editor_close_taken: Option<f64>,
    /// The composer's field that takes the keyboard in its window of its own at the window's next pass.
    window_focus: Option<egui::Id>,
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
    /// The week rule and the all-day alert the keep holds; `None` for the sample, which keeps none.
    kept_week: Option<(inboxcraft_time::WeekRule, inboxcraft_engine::all_day_alert::AllDayAlert)>,
    /// How weeks are counted where the owner chose nothing: the locale's ([`InboxApp::with_week_default`]).
    week_default: inboxcraft_time::WeekRule,
    /// The app's side of running without a window, when it has one.
    desk: Option<Box<dyn Desk>>,
    /// The screen shown beside the index.
    screen: Screen,
    /// The calendar screen.
    calendar: Calendar,
    /// The command line.
    command: Command,
    onboarding: Onboarding,
    /// The appearance settings page.
    settings: Settings,
    /// Whether the window offers to add an account.
    can_add: bool,
    workers: Box<dyn Workers>,
    /// The act the toast shows (by its serial) and when the toast came.
    toast: Option<(u64, f64)>,
    /// The draft discarded from Home whose discard waits behind its toast, and when the toast came: carried out when
    /// the toast ends, the window closes or the app ends, dropped by its Undo ([`InboxApp::discard_draft`]).
    discarding: Option<(u64, Option<f64>)>,
    /// The composer whose own Discard put its draft behind the toast, as it was: its Undo brings it back, and it goes
    /// with the discard ([`InboxApp::discard_composer`]).
    discarded: Option<Box<Composer>>,
    /// Whether the window is a native one whose frame the app may draw (`set_own_frame`).
    own_frame: bool,
    /// The frame the window was last told to show.
    frame_applied: Option<WindowFrame>,
    /// The message being written, in the reader's place or floating.
    composer: Option<Composer>,
    /// Whether the keyboard focus was inside the composer at the start of this frame: Send's key acts only then.
    composer_keys: bool,
    /// Whether the composer has taken the list's place this frame (in front of it in one pane): the list is not drawn,
    /// and nothing acts on it or its rows (no key, no palette command, no Undo).
    list_hidden: bool,
    /// A change of place under way (M12): the old body going, the new one coming.
    change: Option<change::Change>,
    /// The reader's place as it was before a message opened or a draft was sent (M7, M10), playing its movement.
    picture: Option<picture::Picture>,
    /// The window in the mode it left (M14), fading out over the new one.
    mode_picture: Option<picture::Picture>,
    /// Where the reader's place was drawn in this pass: the open message's card or the docked composer's.
    reader_place: Option<Rect>,
    /// Whether a draft was sent in this pass.
    just_sent: bool,
    /// Where the docked draft was sent from, until when (egui's double-click time) a press there reaches nothing, and
    /// what the place showed after the send: once it shows something else (a new composer), presses reach it again.
    sent_place: Option<(Rect, f64, ReaderKey)>,
    /// Where the list was drawn last, to tell whether the keyboard was in it when it goes.
    list_rect: Option<Rect>,
    /// For how many more frames the keyboard is given to the composer's text, the list having gone with it.
    keys_to_text: u8,
    /// The frame being drawn; between frames, the next one.
    frame: u64,
    /// The status line's notices and errors waiting ([`crate::status::Status`]).
    status: crate::status::Status,
    /// The status line's facts, and the mailbox's revision and writes and the time they were read at.
    facts: (status::Facts, Option<(u64, u64, f64)>),
    /// The mailbox's error the status line was last given, so each is queued once.
    seen_error: Option<String>,
    /// The draft whose countdown shows and its whole hold in seconds: what its bar is a share of.
    countdown_whole: Option<(u64, f64)>,
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
    /// How the end of the window and of the app went for the drafts kept.
    keeps_end: KeepsEnd,
    /// Home as last built, built again only while it shows and something it shows changed.
    home: crate::home::Kept,
    /// Home's character field, kept across frames: its cells are built for its rect, not every frame.
    field: crate::field::Field,
}

/// While the person types in the composer, its draft is kept at most once in this many seconds: keeping it runs
/// `compose.update`, which checks the draft on the window's thread; the owner's store writes it to the disk behind,
/// on its draft keeper's thread. Leaving a field, closing, sending, a close of the window and the end of the app keep
/// it at once, so only a crash can lose what was typed in the last second.
pub(crate) const KEEP_EVERY: f64 = 1.0;

/// How long a close of the window, of the composer and the end of the app wait for the drafts kept to reach the disk.
/// The store's own lock wait (2 s) is shorter, so a write blocked by another program fails, and is said, within it.
pub(crate) const KEEP_WAIT: std::time::Duration = std::time::Duration::from_secs(3);

/// How long after a close of the window taken back for a draft not kept the next close still counts as one "in a row"
/// and goes through, in seconds.
pub(crate) const CLOSE_IN_A_ROW: f64 = 10.0;

/// How the end of the window and of the app went for the drafts kept.
#[derive(Debug, Default)]
struct KeepsEnd {
    /// A close of the window was taken back because a draft was not on the disk: the store's count of keeps then and
    /// the time. The next close goes through only in a row: no keep of any draft since, within [`CLOSE_IN_A_ROW`].
    refused: Option<(u64, f64)>,
    /// A close was taken back in this frame (for the window's owner, [`InboxApp::took_back_close`]).
    took_back: bool,
    /// The end of the app, or a close of the window that went through, already waited for the drafts.
    ended: bool,
}

/// The end of the app keeps the draft being written, however little time passed since it was last kept, and waits
/// (at most [`KEEP_WAIT`]) until it is on the disk, unless a close of the window or [`InboxApp::end_keeps`] already
/// did.
impl Drop for InboxApp {
    fn drop(&mut self) {
        if !self.keeps_end.ended
            && let Err(error) = self.end_keeps()
        {
            // Nobody sees the window any more: the app's standard error.
            eprintln!("inboxcraft: {error}");
        }
        self.discard_waiting();
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

/// Where the list stands: not drawn in one pane while something stands in its place (`shown` false); beside the
/// reader or the composer (`beside`), its 440 whatever the width (under [`ONE_PANE`] the composer takes the list's place
/// instead, as the reader does); else over the whole middle.
fn list_place(shown: bool, beside: bool) -> ListPlace {
    if !shown {
        ListPlace::Away
    } else if beside {
        ListPlace::Beside
    } else {
        ListPlace::Whole
    }
}

/// The list's pane at the left: its 440 and the line at its left, no splitter, and a width a person once dragged to is
/// not read.
fn list_panel() -> Panel {
    Panel::left("list").resizable(false).exact_size(LIST_WIDTH + 1.0).show_separator_line(false)
}

/// Where the list stands in a frame.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ListPlace {
    /// Not drawn: in one pane, the open message or the composer stands in its place.
    Away,
    /// At the left, beside the reader or the composer.
    Beside,
    /// Over the whole middle.
    Whole,
}

/// What the reader's pane says with nothing open, by what the list beside it holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReaderSays {
    /// A list of messages: pick one, it opens here (the reader's own empty state).
    Message,
    /// The kept drafts: pick one, it opens in the composer.
    Draft,
    /// A list that holds nothing: nothing to open.
    Nothing,
    /// The outbox with sends in it: a send is not a message to read, nothing opens from it.
    Outbox,
}

/// The reader's empty state beside the drafts (`Draft`), the outbox (`Outbox`) or a list that holds nothing
/// (`Nothing`): in the middle of the pane, 40 from its sides, as the frames `drafts-list/filled`, `/empty` and
/// `outbox/mixed` draw it.
fn nothing_to_open(ui: &mut Ui, theme: &Theme, keymap: &Keymap, says: ReaderSays) {
    let words = |action: &str| keymap.shortcut(action).map(|shortcut| crate::component::key::key_words(&shortcut));
    let (title, help) = if says == ReaderSays::Outbox {
        (
            tr!("Nothing opens from here."),
            tr!("A row of the outbox is a send, not a message to read. Undo or Edit takes it back into the composer, where it can be read and changed.")
                .to_owned(),
        )
    } else if says == ReaderSays::Draft {
        let help = match (words(keymap::NEXT), words(keymap::PREVIOUS), words(keymap::OPEN)) {
            (Some(next), Some(previous), Some(open)) => tr!(
                "{next} and {previous} move in the list, {open} opens the draft where you left it.",
                next = next,
                previous = previous,
                open = open
            ),
            _ => tr!("A draft opens in the composer where you left it.").to_owned(),
        };
        (tr!("Pick a draft.\nIt opens in the composer."), help)
    } else {
        (tr!("Nothing to open."), tr!("A message opens here when the list holds one.").to_owned())
    };
    let side = margin(theme.spacing.s10).left;
    Frame::new().inner_margin(Margin { left: side, right: side, top: 0, bottom: 0 }).show(ui, |ui| {
        ui.set_min_size(ui.available_size());
        crate::component::empty::empty_state(ui, theme, tr!("Reader · nothing open"), title, &help);
    });
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
        // Home is the start place once there is an account; with none the window offers to add one, as before.
        let screen = if mailbox.accounts().is_empty() { Screen::Mail } else { Screen::Home };
        // The pane's reader and the windows' share the fetch and the saving.
        let (fetch, save) = (crate::fetch::Shared::new(fetch), crate::save::Shared::new(save));
        Self {
            mailbox,
            reader: Reader::new(Box::new(fetch.another()), Box::new(save.clone())),
            textures: reader::Textures::new(),
            windows: reader_window::Windows::new(fetch, save),
            hover: None,
            held_back: None,
            land_held: None,
            landed: None,
            bring_held: false,
            ring_back: None,
            card: None,
            drawn: std::sync::Arc::from(Vec::new()),
            list_row: None,
            owner_ring: None,
            list_keys: true,
            reading: false,
            list_over_composer: false,
            composer_window: false,
            window_typing: false,
            window_refused_said: false,
            editor_room: calendar::Room::Full,
            editor_window_refused_said: false,
            editor_window_closed: false,
            editor_opened: 0,
            editor_window_forward: false,
            editor_close_taken: None,
            window_focus: None,
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
            kept_week: None,
            week_default: inboxcraft_time::WeekRule::default(),
            desk: None,
            screen,
            calendar: Calendar::default(),
            command: Command::default(),
            onboarding: Onboarding::default(),
            settings: Settings::default(),
            can_add: false,
            workers: Box::new(NoWorkers),
            toast: None,
            discarding: None,
            discarded: None,
            own_frame: false,
            frame_applied: None,
            composer: None,
            composer_keys: false,
            list_hidden: false,
            change: None,
            picture: None,
            mode_picture: None,
            reader_place: None,
            just_sent: false,
            sent_place: None,
            list_rect: None,
            keys_to_text: 0,
            frame: 0,
            status: crate::status::Status::default(),
            facts: (status::Facts::default(), None),
            seen_error: None,
            countdown_whole: None,
            picker: None,
            sending: None,
            later_picker: false,
            kept_at: None,
            keeps_end: KeepsEnd::default(),
            menu: None,
            home: crate::home::Kept::default(),
            field: crate::field::Field::new(HOME_FIELD_SEED),
        }
    }

    /// The window over `mailbox` on the mail, not on Home: the tests built on the mail start there, as the window did
    /// before Home was its start place.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn at_mail(mailbox: Mailbox) -> Self {
        let mut app = Self::new(mailbox);
        app.show_mail();
        app
    }

    /// [`InboxApp::with_outside`] on the mail, as [`InboxApp::at_mail`].
    #[cfg(test)]
    #[must_use]
    pub(crate) fn with_outside_at_mail(mailbox: Mailbox, fetch: Box<dyn Fetch>, save: Box<dyn Save>) -> Self {
        let mut app = Self::with_outside(mailbox, fetch, save);
        app.show_mail();
        app
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

    /// The same window, counting weeks by `rule` where the owner chose nothing, or chose what cannot be read: the rule
    /// of the owner's locale ([`inboxcraft_engine::week::for_locale`]). Given before [`InboxApp::with_keep`].
    #[must_use]
    pub fn with_week_default(mut self, rule: inboxcraft_time::WeekRule) -> Self {
        self.week_default = rule;
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
                self.mailbox.restore_after(kept.after);
                self.mailbox.restore_greet_by(kept.greet_by);
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
                    after: self.mailbox.after(),
                    greet_by: self.mailbox.greet_by(),
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
            // What the owner chose, read as outside data; what cannot be read, or nothing kept, is the locale's.
            match keep.load_week() {
                Some(kept) => self.mailbox.restore_week_or(&kept, self.week_default),
                None => self.mailbox.restore_week_rule(self.week_default),
            }
            self.kept_week = Some((self.mailbox.week_rule(), self.mailbox.all_day_alert()));
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

    /// The same window, offering in the reader's More menu to open a message's original in the browser through
    /// `original` (a cleaned copy: see [`crate::original`]). A window without one does not offer it.
    #[must_use]
    pub fn with_original(mut self, original: Box<dyn crate::original::Original>) -> Self {
        let original = crate::original::Shared::new(original);
        self.reader.set_original(original.clone());
        self.windows.set_original(&original);
        self
    }

    /// Shows and changes what `desk` lets the owner choose from now on, as [`InboxApp::with_desk`].
    pub fn set_desk(&mut self, desk: Box<dyn Desk>) {
        self.desk = Some(desk);
    }

    /// The action an index place runs, as its key would.
    fn index_action(place: index::Place) -> Option<&'static str> {
        match place {
            index::Place::Home => Some(keymap::HOME),
            index::Place::Mail => Some(keymap::MAIL),
            index::Place::Calendar => Some(keymap::CALENDAR),
            index::Place::Later => Some(keymap::LATER),
            index::Place::Settings => Some(keymap::SETTINGS),
        }
    }

    /// Draws the index and the command line; what of the index and of the command line was asked for, the index's
    /// through its gate: whatever pressed it (the pointer, a key, a screen reader's activate, which
    /// egui's modal layers do not stop), nothing of it runs unless the `surfaces` in front allow it. A place runs as a
    /// press may run its action ([`Surfaces::allows_press`]); a mailbox, an account or an account row changes the mail
    /// under the surface, so none of them runs under the palette or the event editor. A press on the index while a key
    /// is waited for ends the wait first. An account row's press is applied here.
    fn index_and_command(
        &mut self,
        index: &mut Ui,
        command: &mut Ui,
        theme: &Theme,
        surfaces: &Surfaces,
        setting: bool,
        (regions, keys): (shell::Regions, command::Early),
    ) -> (index::Asked, Vec<command::Asked>) {
        // The command line first, the title bar in the own frame: its field and the window controls stand above the
        // index, and Tab walks the drawn order.
        self.own_frame_parts(command, theme, regions);
        let commanded = self.command_line(command, theme, regions, keys);
        if self.draws_frame() {
            frame::controls(command.ctx(), theme, regions.command);
        }
        let at = self.index_place(setting);
        let flags = (Self::index_live(surfaces), self.checking_elsewhere());
        let narrow = regions.index.width() < shell::INDEX;
        // The index's Drafts count is read before it is drawn: a draft written since counts in this frame (#328).
        self.mailbox.keep_drafts_fresh();
        index::refresh(index.ctx(), &mut self.mailbox);
        let indexed = index::show(index, theme, &self.keymap, &self.mailbox, at, narrow, flags);
        let indexed = Self::index_allowed(surfaces, indexed);
        // A row of the index pressed while a key is waited for ends the wait first, then acts.
        let pressed =
            indexed.place.is_some() || indexed.mailbox.is_some() || indexed.account.is_some() || indexed.toggle.is_some() || indexed.compose;
        if pressed && self.settings.capturing().is_some() {
            self.settings.captured(None);
        }
        if let Some(id) = indexed.toggle {
            index::toggle(index.ctx(), id);
        }

        (indexed, commanded)
    }

    /// Whether what the index `asked` shows a list: a mailbox, Mail or Later.
    fn shows_list(asked: &index::Asked) -> bool {
        asked.mailbox.is_some() || matches!(asked.place, Some(index::Place::Mail | index::Place::Later))
    }

    /// The keyboard at the end of a frame: after a press on the index that showed a list (`to_list`) beside the docked
    /// composer, back to the composer's field; and where it is now, kept for the next press.
    fn keyboard_after_index(&self, ctx: &egui::Context, index_to_list: bool) {
        // A press on the index that shows a list beside the docked composer, which holds the list's keys there: the
        // keyboard goes back to the composer's field it was in, never to nothing (asked again until something has it,
        // as the list may let go of the focus a frame later). Elsewhere the list takes the keys.
        let field = ctx.data(|data| data.get_temp::<(egui::Id, bool)>(last_keyboard_id())).filter(|(_, in_composer)| *in_composer);
        let pending = ctx.data(|data| data.get_temp::<bool>(last_keyboard_id().with("back"))).unwrap_or(false);
        let back = (pending || index_to_list) && self.docked() && field.is_some();
        match (back, field, ctx.memory(|memory| memory.focused())) {
            (true, Some((field, _)), None) => {
                ctx.memory_mut(|memory| memory.request_focus(field));
                ctx.data_mut(|data| data.insert_temp(last_keyboard_id().with("back"), true));
                ctx.request_repaint();
            }
            _ => {
                ctx.data_mut(|data| data.insert_temp(last_keyboard_id().with("back"), false));
            }
        }
        // Where the keyboard last was, and whether in the composer: a press elsewhere takes it away at once.
        if let Some(focused) = ctx.memory(|memory| memory.focused()) {
            let in_composer = compose::has_keys(ctx);
            ctx.data_mut(|data| data.insert_temp(last_keyboard_id(), (focused, in_composer)));
        }
    }

    /// Whether the index's mailboxes, account rows and account picker act: with no palette or event editor in front. A
    /// key waited for on the Shortcuts page holds the keys, not the pointer: a row pressed ends the wait and acts.
    fn index_live(surfaces: &Surfaces) -> bool {
        !(surfaces.has(Surface::Command) || surfaces.has(Surface::Editor))
    }

    /// What of the index's `asked` the `surfaces` in front allow: a place where a press may run its action
    /// ([`Surfaces::allows_press`]); a mailbox, the account picker's choice and an account row only with no palette, capture
    /// or event editor in front.
    fn index_allowed(surfaces: &Surfaces, asked: index::Asked) -> index::Asked {
        // A key waited for on the Shortcuts page holds the keys, not the pointer: a place pressed shows, and leaving the
        // settings ends the wait.
        let waiting = surfaces.has(Surface::Capture) && !surfaces.has(Surface::Command) && !surfaces.has(Surface::Float);
        let place = asked.place.filter(|place| Self::index_action(*place).is_some_and(|action| waiting || surfaces.allows_press(action)));
        // New message runs as its key would: where a press may run it.
        let compose = asked.compose && (waiting || surfaces.allows_press(keymap::COMPOSE));
        if Self::index_live(surfaces) { index::Asked { place, compose, ..asked } } else { index::Asked { place, compose, ..index::Asked::default() } }
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

    /// Keeps the week rule and the all-day alert beside it when either changed; the sample keeps none.
    fn keep_week(&mut self) {
        let Some(kept) = self.kept_week else { return };
        let now = (self.mailbox.week_rule(), self.mailbox.all_day_alert());
        if now == kept {
            return;
        }
        // Tried once per change: the next change tries again.
        self.kept_week = Some(now);
        if let Err(error) = self.keep.store_week(&self.mailbox.kept_week()) {
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

    /// Takes the desk's answer on starting with the computer, when it came: a refusal is a note on the settings shown,
    /// or in the error banner once they are left.
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
    /// goes, what the window does after an act, which word of the name Home greets by, and the screener's mode but on
    /// the sample, which keeps the one kept.
    fn triage(&self) -> Triage {
        let screener = if self.mailbox.is_sample() { self.kept_triage.screener } else { self.mailbox.screener_mode() };
        Triage {
            screener,
            view: self.mailbox.inbox_view(),
            keep_pins: self.mailbox.pins_outlast_done(),
            hold: Some(self.mailbox.hold_seconds()),
            after: self.mailbox.after(),
            greet_by: self.mailbox.greet_by(),
        }
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

    /// The screen shown beside the index.
    #[must_use]
    pub fn screen(&self) -> Screen {
        self.screen
    }

    /// The calendar screen.
    #[must_use]
    pub fn calendar(&self) -> &Calendar {
        &self.calendar
    }

    /// The command line.
    #[must_use]
    pub fn command(&self) -> &Command {
        &self.command
    }

    /// The command line, to type in from outside (the `ui_shot` scripts, later the control channel).
    pub fn command_mut(&mut self) -> &mut Command {
        &mut self.command
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

    /// The act `action` (Done, Archive, read or unread, flag or not, Delete) on the selected row, or on the row that
    /// has the keyboard's ring when it stands on another (after Esc from a row's pointer menu), as J, K, Enter and the
    /// menu key take it; with nothing selected only under Stay, or on the row the owner put the ring on by Esc from its
    /// pointer menu ([`InboxApp::owner_ring`]), so the acts go on down the list: a row the keyboard landed on after the
    /// server took the open message away is not acted on.
    fn row_act(&self, action: &str) -> Option<(u64, Act)> {
        let stay = self.mailbox.after() == crate::mailbox::After::Stay;
        let target = match self.mailbox.selected() {
            Some(selected) => Some(self.list_row.unwrap_or(selected)),
            None => self.list_row.filter(|row| stay || self.owner_ring == Some(*row)),
        };
        let row = target.and_then(|id| self.mailbox.messages().iter().find(|row| row.id == id))?;
        let (_, act) = ROW_ACTS.into_iter().find(|(id, _)| *id == action)?;
        let act = match act {
            Act::Read if row.seen => Act::Unread,
            Act::Flag if row.flagged => Act::Unflag,
            Act::Done if row.done => Act::Undone,
            act => act,
        };
        Some((row.id, act))
    }

    /// Whether the action `id` can run now: an act on a row needs a selected message (under Stay the keyed row), Undo
    /// an act in the history, the reader's full width an open message; on the settings and on Home nothing of the mail
    /// and nothing of the calendar runs; the composer's own (Send, fold) only where a composer is drawn; an id the
    /// window does not know never runs.
    #[must_use]
    pub fn can_run(&self, id: &str) -> bool {
        let apart = self.settings.is_open() || self.screen == Screen::Home;
        if apart && (crate::surface::mail_only(id) || keymap::CALENDAR_KEYS.contains(&id)) {
            return false;
        }
        match id {
            // With the list hidden nothing acts on a row the owner cannot see, by a key or from the palette; Undo, which
            // may take back an act on such a row, rests too.
            keymap::DONE | keymap::ARCHIVE | keymap::READ | keymap::FLAG | keymap::DELETE => !self.list_hidden && self.row_act(id).is_some(),
            // Under the event editor Undo rests too: it would take back the event being edited ([`Surfaces::undo_rests`]).
            // Off the calendar, and in the main window while the editor is in its window of its own, it runs.
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
            // In one pane with the list in front, on the keyed row, as its Snooze shows the key.
            keymap::SNOOZE => {
                self.screen == Screen::Mail
                    && ((self.reader.open_id().is_some() && !self.docked() && !self.list_in_front()) || self.snooze_row().is_some())
            }
            keymap::COMPOSE => !self.mailbox.accounts().is_empty(),
            // The composer's own: wherever a composer is drawn ([`crate::surface::of_the_composer`]).
            keymap::FOLD => self.composer_drawn() && !self.docked(),
            // In front in the reader's place; or the floating panel open with the keys inside it.
            keymap::SEND => {
                self.composer_drawn()
                    && (self.composer_in_front() || (self.composer_keys && self.composer.as_ref().is_some_and(|composer| !composer.folded())))
            }
            _ if keymap::CALENDAR_KEYS.contains(&id) => self.screen == Screen::Calendar && !self.calendar.is_empty(),
            // The list's keys and its views act on the list: hidden, it is not there to act on.
            _ if keymap::LIST_KEYS.contains(&id) => self.screen == Screen::Mail && !self.list_hidden && (self.list_row.is_some() || self.list_keys),
            keymap::VIEW_GROUPED | keymap::VIEW_SMART | keymap::VIEW_LIST => !self.list_hidden,
            _ => keymap::action(id).is_some(),
        }
    }

    /// Whether the command line's `>` may run the action `id`: as [`InboxApp::can_run`], but Send sends whatever draft is
    /// drawn, floating too: from the line the keyboard is never in the composer. Never a folded draft, which the owner
    /// cannot see.
    fn can_run_from_the_line(&self, id: &str) -> bool {
        if id == keymap::SEND { self.composer_drawn() && !self.composer_folded() } else { self.can_run(id) }
    }

    /// Whether the draft drawn is the floating panel folded to its head.
    fn composer_folded(&self) -> bool {
        self.composer.as_ref().is_some_and(Composer::folded)
    }

    /// Why `>` cannot run `id` now, its row listed disabled with it; none when it can.
    fn refused_on_the_line(&self, ctx: &egui::Context, id: &str) -> Option<&'static str> {
        if id == keymap::SEND && self.composer_drawn() && self.composer_folded() {
            return Some(tr!("Unfold the draft to send it"));
        }
        (!self.can_run_from_the_line(id) || soon::blocked(ctx, id)).then(|| command::refusal(id))
    }

    /// Runs the action `id` of [`keymap::actions`], as its key, the index or the palette do; an action that cannot run
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
            // The command line comes over the place shown, the settings too: they stay under it.
            keymap::PALETTE => self.command.open(ctx, false),
            keymap::COMMAND_LINE => self.command.open_by_slash(ctx),
            keymap::FIND => {
                self.show_mail();
                self.reader.set_full_width(false);
                self.command.open(ctx, true);
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
            keymap::SNOOZE => match self.snooze_row() {
                Some(row) => {
                    self.mailbox.load_presets();
                    list::ask_snooze(ctx, row);
                }
                None => self.open_picker(""),
            },
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
            keymap::HOME => self.show_home(),
            keymap::COMPOSE => {
                // The settings draw no composer: New message shows the place the owner came from.
                self.leave_settings();
                self.compose();
                let windowed = self.windowed(ctx);
                focus_composer(ctx, windowed, &mut self.window_focus, compose::field_id(crate::compose::Line::To));
            }
            keymap::SEND => self.send_composer(),
            keymap::FOLD => {
                if let Some(composer) = self.composer.as_mut() {
                    composer.toggle_fold();
                }
            }
            // A place: pressed again it shows the place the owner came from (the owner's bare `ui.settings.open`).
            keymap::SETTINGS => self.apply_settings_ask(inboxcraft_engine::shown::SettingsAsked { page: None, bare: true, by_agent: false }),
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
            // A step of the text size through the engine; the status line says the size, or that it is at its end.
            _ if keymap::TEXT_SIZE_KEYS.contains(&id) => {
                let (scale, words) = keymap::text_size_step(id, self.text_scale());
                if scale != self.text_scale() {
                    self.set_text_scale(scale);
                }
                if self.text_scale() == scale {
                    let notice = crate::status::Message::Notice { text: words, until: crate::status::NOTICE_SECONDS };
                    self.status.push_in_place_of(notice, keymap::is_text_size_notice);
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
        if self.mailbox.drafts_listed() {
            // The drafts place walks its kept drafts, not messages.
            list::step_draft(ctx, &self.mailbox, forward, rows);
            return;
        }
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
        if self.mailbox.drafts_listed() {
            // The keyed draft opens in the composer to go on writing; the keyed Drafts folder on the server opens.
            if let Some(draft) = list::focused_draft(ctx, &self.mailbox) {
                self.open_draft(ctx, draft);
            } else if let Some(folder) = list::focused_server(ctx, &self.mailbox) {
                self.open_server_drafts(folder);
            }
            return;
        }
        let Some(id) = self.list_row.or_else(|| self.mailbox.selected()) else { return };
        if self.open_in_window(id) {
            return;
        }
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
        // A row pressed where messages open in a window of their own opens there; the row keeps the keyboard.
        let windowed = clicked.filter(|id| self.open_in_window(*id));
        if windowed.is_some() {
            self.list_keys = true;
        }
        if let Some(id) = clicked.filter(|_| windowed.is_none()) {
            self.mailbox.read_preview(id);
            self.mailbox.select(Some(id));
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
        if let Some(id) = self.ring_back.take() {
            ctx.memory_mut(|memory| memory.request_focus(list::row_id(id)));
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
        // The keyboard on nothing, on a row or in a float of the list, which leaves with it: the reader takes it.
        if crossed && single && message {
            let free = ctx.memory(|memory| memory.focused().is_none()) || list::focused_row(ctx).is_some() || list::floats_open(ctx, &self.mailbox);
            self.focus_reader |= free;
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

    /// In one pane with the list in front, the row that has the keyboard: the Snooze key opens the time picker on it,
    /// as its Snooze does, since the reader is not shown. So it does on the row the owner put the ring on by Esc from
    /// its pointer menu ([`InboxApp::owner_ring`]), in one pane or two.
    fn snooze_row(&self) -> Option<u64> {
        self.list_row.filter(|row| (self.list_in_front() || self.owner_ring == Some(*row)) && !self.list_hidden)
    }

    /// Whether the calendar shows its event editor in the main window ([`Surface::Editor`]): small or full, not in a
    /// window of its own, whose keys are its own.
    fn editing_event(&self) -> bool {
        self.screen == Screen::Calendar && !self.settings.is_open() && self.calendar.editor().is_some() && self.editor_room != calendar::Room::Window
    }

    /// Where the event editor shows this frame, as the layout says, for a new event and an edit alike: small, full, or
    /// in a window of its own where one can open; where none can (the web app, a headless context), full, and the
    /// status line says why, once. The full editor and the window give the keyboard to the title when the editor opens
    /// in them, or moves there.
    fn place_event_editor(&mut self, ctx: &egui::Context) {
        let open = self.calendar.editor().is_some();
        self.editor_room = match self.mailbox.appearance().layout.event_editor {
            EventEditorHost::Small => calendar::Room::Small,
            EventEditorHost::Full => calendar::Room::Full,
            EventEditorHost::Window if matches!(Host::Window(event_editor_window()).usable(ctx), Host::Window(_)) => calendar::Room::Window,
            EventEditorHost::Window => {
                if open && !self.editor_window_refused_said {
                    self.editor_window_refused_said = true;
                    self.say(tr!("No window of its own can open here: the event is edited in the calendar's place"));
                }
                calendar::Room::Full
            }
        };
        // An editor opened in the place of another (one with nothing typed) is met as new: its title takes the keyboard.
        let opened = self.calendar.editor_opened();
        if std::mem::replace(&mut self.editor_opened, opened) != opened {
            calendar::forget_editor_opened(ctx, None);
        }
        calendar::forget_editor_opened(ctx, open.then_some(self.editor_room));
    }

    /// The event editor in its window of its own while it shows there ([`InboxApp::place_event_editor`]), on every
    /// screen; what was asked in it. The window's `Esc` is the editor's: it closes with nothing typed, else warns, and a
    /// second one throws the typing away, as in the main window. The desktop's close and the frame's Close are a press
    /// outside: they close the editor with nothing typed and otherwise only say how to keep or throw away the typing,
    /// and the window stays ([`InboxApp::keep_editor_window`]). The look keys run from it; nothing else of the main
    /// window does.
    fn event_editor_in_window(&mut self, ctx: &egui::Context, theme: &Theme) -> Vec<calendar::Asked> {
        self.editor_window_closed = false;
        if self.editor_room != calendar::Room::Window {
            return Vec::new();
        }
        let (keymap, frame) = (&self.keymap, self.mailbox.appearance().frame);
        let Some(editor) = self.calendar.editor_mut() else { return Vec::new() };
        let title = if editor.form().summary.trim().is_empty() { editor.title().to_owned() } else { editor.form().summary.clone() };
        let look = host::Look { theme, frame, size: calendar::EDITOR_WINDOW };
        let (mut escaped, mut ran) = (false, Vec::new());
        let hosted = host::show(ctx, Host::Window(event_editor_window()), &title, look, |ui, surfaces| {
            // The desktop's close in the same pass wins: it never counts as a second Esc that throws the typing away.
            let asked = ui.input(|input| input.viewport().close_requested());
            escaped = surfaces.escape(ui.ctx()) == Some(Surface::Window) && !asked;
            ran.extend(crate::surface::WINDOW_KEYS.into_iter().filter(|id| surfaces.allows(id, keymap) && keymap.pressed(ui.ctx(), id)));
            calendar::editor_window(ui, theme, keymap, editor)
        });
        let host::Hosted::Window(host::Windowed { mut inner, closed }) = hosted else { return Vec::new() };
        for id in ran {
            self.run_action(ctx, id);
        }
        if closed {
            self.editor_window_closed = true;
            let leave = if escaped { crate::component::popup::Leave::Escape } else { crate::component::popup::Leave::Beside };
            inner.push(calendar::Asked::Leave(leave));
        }
        inner
    }

    /// After what was asked this frame: a window of its own asked to close while its editor stays (typing not thrown
    /// away) is kept open.
    fn keep_editor_window(&mut self, ctx: &egui::Context) {
        if std::mem::take(&mut self.editor_window_closed) && self.calendar.editor().is_some() {
            ctx.send_viewport_cmd_to(event_editor_window(), egui::ViewportCommand::CancelClose);
        }
        if std::mem::take(&mut self.editor_window_forward) && self.editor_room == calendar::Room::Window {
            ctx.send_viewport_cmd_to(event_editor_window(), egui::ViewportCommand::Focus);
        }
    }

    /// A close of the main window (the app's end, or its way to the tray) while the event editor holds typing, in any
    /// host: the close is taken back once, as for a draft not kept ([`InboxApp::close_with_keeps`]), and the editor
    /// says the event is not saved, in sight (the calendar shown, or its window brought forward). A second close in a
    /// row, with nothing typed since, within [`CLOSE_IN_A_ROW`], goes through and the typing is lost.
    fn close_with_event(&mut self, ctx: &egui::Context) {
        if !ctx.input(|input| input.viewport().close_requested()) {
            return;
        }
        let now = ctx.input(|input| input.time);
        let in_a_row = self.editor_close_taken.is_some_and(|at| (0.0..=CLOSE_IN_A_ROW).contains(&(now - at)));
        let Some(editor) = self.calendar.editor_mut().filter(|editor| editor.edited()) else { return };
        if in_a_row && editor.warned() {
            return;
        }
        editor.warn(false);
        ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        self.keeps_end.took_back = true;
        self.editor_close_taken = Some(now);
        if self.editor_room == calendar::Room::Window {
            ctx.send_viewport_cmd_to(event_editor_window(), egui::ViewportCommand::Focus);
        } else if self.screen != Screen::Calendar || self.settings.is_open() {
            self.leave_settings();
            self.show_calendar();
        }
        self.say(tr!("The event being edited is not saved: save it, or close again to quit without it"));
    }

    /// What stands in front this frame, read before its keys ([`Surfaces`]).
    fn surfaces(&self, ctx: &egui::Context) -> Surfaces {
        let settings = self.settings.is_open();
        let mail = self.screen == Screen::Mail && !settings;
        let home = self.screen == Screen::Home && !settings;
        Surfaces::new(Facts {
            float: crate::component::popup::any_open(ctx),
            dialog: self.settings.confirms(self.onboarding.asking_remove()),
            command: self.command.results(),
            capture: self.settings.capturing().is_some(),
            editor: self.editing_event(),
            composer: !settings && self.composer_in_front(),
            full_width: self.reader.full_width(),
            one_pane_reader: mail && one_pane(ctx) && !self.docked() && self.message_in_front() && self.reader.open_id().is_some(),
            mail,
            settings,
            home,
            composer_drawn: self.composer_drawn(),
        })
    }

    /// Whether a composer is drawn in the main window this frame: one is open, on any place but the settings, which
    /// draw none, and not in a window of its own, whose keys are its own.
    fn composer_drawn(&self) -> bool {
        self.composer.is_some() && !self.settings.is_open() && !self.composer_window
    }

    /// Whether the list has the keyboard: it may have the keys ([`Surfaces::list_has_keyboard`]: on the mail, shown,
    /// nothing in front that takes them), and a row has the focus, or a row was pressed or keyed last and nothing else
    /// has the focus.
    fn list_has_keyboard(&self, ctx: &egui::Context, surfaces: &Surfaces) -> bool {
        if !surfaces.list_has_keyboard() {
            return false;
        }
        // A row of the list has the keyboard: a message's, or in the drafts place a kept draft's or a server folder's.
        let drafts =
            self.mailbox.drafts_listed() && (list::focused_draft(ctx, &self.mailbox).is_some() || list::focused_server(ctx, &self.mailbox).is_some());
        if self.list_row.is_some() || drafts {
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

    /// Shows the calendar screen, leaving the settings; its calendars and events are read again.
    pub fn show_calendar(&mut self) {
        self.leave_settings();
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

    /// Tells the session whether the owner has an event open in the editor, with the window `shown`: while one is, an
    /// agent's `history.undo` is refused. Worked out from the editor each time, never kept apart from it, so it cannot
    /// outlive the editor; without a window it is `false`.
    pub fn tell_editing(&mut self, shown: bool) {
        self.mailbox.set_editing(shown && self.calendar.editor().is_some());
    }

    /// Applies what was pressed on the calendar screen.
    fn apply_calendar(&mut self, asked: Vec<calendar::Asked>) {
        let accounts = asked.contains(&calendar::Asked::Accounts);
        let mut copied = None;
        let (mut kept_apart, mut delete_refused) = (false, false);
        let windowed = self.editor_room == calendar::Room::Window;
        let (calendar, mailbox) = (&mut self.calendar, &mut self.mailbox);
        for asked in asked {
            // The event editor's window holds typing that New event or Edit in the main window would replace: it stays,
            // and comes forward to say so.
            let typed = calendar.editor().is_some_and(crate::calendar::Editor::edited);
            kept_apart |= windowed && typed && matches!(asked, calendar::Asked::New | calendar::Asked::Edit);
            match asked {
                calendar::Asked::Accounts => {}
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
                // The event the editor holds typing for is not deleted under it (the window's, beside the main one).
                calendar::Asked::Delete
                    if typed && calendar.picked().map(|event| event.id) == calendar.editor().and_then(crate::calendar::Editor::id) =>
                {
                    delete_refused = true;
                }
                calendar::Asked::Delete => calendar.delete_picked(mailbox),
                calendar::Asked::Save => calendar.save(mailbox),
                calendar::Asked::Cancel => calendar.close_editor(),
                calendar::Asked::Leave(leave) => calendar.leave_editor(leave == crate::component::popup::Leave::Escape),
                calendar::Asked::More(day) => calendar.set_more(day),
                calendar::Asked::PickDay(day) => calendar.pick_day(Some(day)),
                calendar::Asked::Sync => {
                    calendar.sync_now(mailbox);
                }
                calendar::Asked::Copied => copied = calendar.editor_mut().and_then(crate::calendar::Editor::take_copied),
            }
        }
        if let Some(url) = copied {
            self.say(&tr!("Link copied: {url}", url = crate::mailbox::shown(&url)));
        }
        if delete_refused {
            self.say(tr!("Not deleted: the event is being edited in its window. Save or throw away the typing there first"));
        }
        if kept_apart {
            self.editor_window_forward = true;
            self.say(tr!("The event editor's window holds typing: save or throw it away there first"));
        }
        // Every press that opens or ends the editor (New event, Edit, Save, Cancel, Esc, Delete) comes through here.
        self.tell_editing(true);
        if accounts {
            self.open_settings_on("accounts");
        }
    }

    /// Before the calendar screen is drawn: the time now for its now line, and its own keys.
    fn calendar_before(&mut self, ctx: &egui::Context, setting: bool) {
        if self.screen != Screen::Calendar {
            return;
        }
        self.calendar.tick(self.mailbox.now(), self.mailbox.offset());
        self.calendar.follow_week(&mut self.mailbox);
        let open = setting || self.command.results() || self.settings.capturing().is_some();
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

    /// Shows the settings on Shortcuts, keeping the account forms as they are typed, all but the password.
    pub fn open_shortcuts(&mut self) {
        self.settings.open_at(Section::Shortcuts);
    }

    /// Shows Home, leaving the settings.
    pub fn show_home(&mut self) {
        self.leave_settings();
        self.screen_to(Screen::Home);
    }

    /// Shows the mail, leaving the settings, the calendar and Home.
    pub fn show_mail(&mut self) {
        self.leave_settings();
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
    /// waits for a key or the palette is open, the keys are theirs; the palette lies over the list and the calendar and
    /// the settings stand in its place, so the mail's keys (the arrows, the acts on a row) rest while they show.
    fn keys(&mut self, ctx: &egui::Context, surfaces: &Surfaces) -> Keys {
        // Send's key acts only from inside the composer, open and not on the settings, which draw none: never from a
        // field in front of it or beside it.
        self.composer_keys = !surfaces.on_settings() && self.composer.as_ref().is_some_and(|composer| !composer.folded()) && compose::has_keys(ctx);
        let held = surfaces.held();
        self.capture_key(ctx);
        // Read before anything returns: a row that lost the focus while the palette was open is not the list's row.
        self.list_row = if held { None } else { list::focused_row(ctx) };
        // The ring an Esc from a row's pointer menu put on its row lasts while the row keeps the keyboard: J, K, a click,
        // another menu or a removal moves it, and it ends.
        if let Some(ringed) = list::take_ringed(ctx) {
            self.owner_ring = Some(ringed);
        }
        self.owner_ring = self.owner_ring.filter(|ringed| !held && ctx.memory(|memory| memory.has_focus(list::row_id(*ringed))));
        // A row with the focus is the list's keyed row: it quiets no key.
        let focused = ctx.memory(|memory| memory.focused().is_some()) && self.list_row.is_none();
        // A field with the focus, or a surface whose fields the letters are for.
        let quiet = focused || surfaces.quiet();
        // The mail's own keys (an act on a row) rest wherever the mail is not shown: the calendar, Home.
        let typing = quiet || self.screen != Screen::Mail;
        let quiet = focused || held || surfaces.has(Surface::Composer);
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
        // A discard waiting behind its toast takes Undo too, on every place ([`InboxApp::discard_draft`]).
        let toasted = self.mailbox.undoable().is_some_and(|undo| undo.undoable) || self.counting().is_some() || self.discarding.is_some();
        keymap::actions()
            .into_iter()
            .map(|action| action.id)
            // The acts on a row (the list takes those), the calendar's keys (`calendar::keys`), what the settings, the
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

    /// The settings, to drive from outside (the `ui_shot` scripts, later the control channel).
    pub fn settings_mut(&mut self) -> &mut Settings {
        &mut self.settings
    }

    /// Shows the settings, a place, on the page last shown.
    pub fn open_settings(&mut self) {
        self.settings.open();
    }

    /// Shows the settings on the page whose id is `page` (`appearance`, `accounts` … [`Section::id`]); no page, or one
    /// that is no section's (outside data), shows the page last shown. Never a panic.
    pub fn open_settings_page(&mut self, page: Option<&str>) {
        self.settings.open_page(page);
    }

    /// Shows the settings on Accounts, where accounts are added.
    pub fn open_accounts(&mut self) {
        self.settings.open_at(Section::Accounts);
    }

    /// Shows the place the owner came to the settings from: a key being waited for stops waiting; the account forms
    /// stay as they are typed, all but the password, and a check goes on ([`Onboarding::leave_page`]).
    pub fn leave_settings(&mut self) {
        self.settings.leave();
        self.onboarding.leave_page();
    }

    /// The add-account form and the check are the Accounts page's: while the page does not show, the form keeps all
    /// but its password and Detect stops; the check goes on.
    fn follow_accounts_page(&mut self) {
        if !(self.settings.is_open() && self.settings.section() == Section::Accounts) {
            self.onboarding.leave_page();
        }
    }

    /// Whether the check of an added account runs while the Accounts page does not show: the index marks SETTINGS.
    fn checking_elsewhere(&self) -> bool {
        self.onboarding.checking() && !(self.settings.is_open() && self.settings.section() == Section::Accounts)
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
            self.reader.refresh_cards_and_why(&mut self.mailbox);
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
        // Stay, the owner's choice of what follows an act: the act opens nothing, and the keyboard goes to the row that
        // took the message's place, as when the server takes it away.
        let stay = self.mailbox.after() == crate::mailbox::After::Stay;
        let walking = owner && !stay && (from_list || self.single != Some(true));
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
        if let Some(gone) = open.filter(|_| gone && (!owner || stay) && same_list) {
            self.land_held = self.landing_after_removal(gone);
            self.landed = self.land_held;
            // The owner's own act brings the list in front in one pane, with the keyboard on that row.
            self.bring_held = owner;
        }
        // Under Stay the owner's act on the row that had the keyboard and was not open: the keyboard lands on the row
        // that took its place, so the next act goes on from there.
        if let Some(keyed) = taken.filter(|id| stay && open != Some(*id) && self.list_row == Some(*id)) {
            self.land_held = self.landing_after_removal(keyed);
            self.landed = self.land_held;
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
        self.screen == Screen::Mail && !self.composer_window && self.composer.as_ref().is_some_and(|composer| !composer.floating())
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
                self.float_new_composer();
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
                self.float_new_composer();
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
        // Closed only once its draft is on the disk: a write the store failed keeps the composer open, saying why.
        let draft = composer.draft();
        match composer.close(&mut self.mailbox).and_then(|()| self.mailbox.wait_for_keep(draft, KEEP_WAIT)) {
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
        self.send_drawn();
    }

    /// [`InboxApp::send_composer`]; whether the draft was kept for a reason its Send shows (no recipient, a blank to
    /// fill in), not sent and not refused by the engine.
    fn send_drawn(&mut self) -> bool {
        // A composer opened or moved in this frame has not been seen yet: Send acts once it has been painted.
        let frame = self.frame;
        let Some(composer) = self.composer.as_mut().filter(|composer| composer.seen(frame)) else { return false };
        let (draft, later) = (composer.draft(), composer.later());
        match composer.send(&mut self.mailbox) {
            Ok(Some(at)) => {
                self.composer = None;
                if later.is_some() {
                    let when = crate::component::typed_time::long(at, self.mailbox.now(), self.mailbox.offset());
                    self.say(&tr!("Scheduled for {when}", when = when));
                } else {
                    self.sending = Some(draft);
                    self.just_sent = true;
                }
                // The engine holds the message now; it goes to the worker once the hold is over.
                self.pump();
                false
            }
            Ok(None) => true,
            Err(error) => {
                self.mailbox.fail(error);
                false
            }
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
                self.float_new_composer();
            }
            Err(error) => self.mailbox.fail(error),
        }
    }

    /// Opens the kept draft `draft` in the composer to go on writing (Home's draft rows, the drafts place), the keys
    /// in its To line. Only when the draft opened: a refused switch (the draft being written could not be kept) leaves
    /// the keys where they were.
    fn open_draft(&mut self, ctx: &egui::Context, draft: u64) {
        self.reopen(draft);
        if self.composer.as_ref().is_some_and(|composer| composer.draft() == draft) {
            let windowed = self.windowed(ctx);
            focus_composer(ctx, windowed, &mut self.window_focus, compose::field_id(crate::compose::Line::To));
        }
    }

    /// Applies what was pressed in the drafts place: a kept draft's row or its links (`drafted`), or the row of a Drafts
    /// folder on the server (`server`).
    fn apply_drafts(&mut self, ctx: &egui::Context, drafted: Option<(u64, list::Drafted)>, server: Option<u64>) {
        match drafted {
            Some((draft, list::Drafted::Open)) => self.open_draft(ctx, draft),
            Some((draft, list::Drafted::Discard)) => self.discard_draft(draft),
            None => {}
        }
        if let Some(folder) = server {
            self.open_server_drafts(folder);
        }
    }

    /// Opens the Drafts folder `folder` on the server from the drafts place: its mail, as any folder's. Once: Enter on
    /// its row also presses it in the same frame.
    fn open_server_drafts(&mut self, folder: u64) {
        if self.mailbox.place() != Place::Folder(folder) {
            self.mailbox.open(Place::Folder(folder));
            self.search.clear();
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
            list::Outbox::Edit => {
                // A send that changed since it was refused, or keeps no record of how it went: its chips show where
                // each recipient goes until the composer closes.
                let changed = self
                    .mailbox
                    .outbox()
                    .iter()
                    .any(|row| row.draft == draft && matches!(row.state, crate::mailbox::SendState::Refused { .. }) && !row.changed.is_empty());
                self.reopen(draft);
                if let Some(composer) = self.composer.as_mut().filter(|composer| changed && composer.draft() == draft) {
                    composer.show_addresses();
                }
            }
            list::Outbox::Retry => self.retry(draft),
        }
    }

    /// Discards the kept draft `draft` from Home, with an Undo: Home hides it at once and a toast says "Draft
    /// discarded" with Undo for [`TOAST_SECONDS`]; the discard (`compose.discard`, here and on the server) is carried
    /// out when the toast ends, when the window closes or when the app ends, and Undo drops it, the row back where it
    /// was. Until then nothing is gone: after a crash the draft is still there. A discard already waiting is carried
    /// out first. The draft being written, when it is that one, is closed and kept first (refused when keeping it
    /// fails: it stays on screen).
    pub fn discard_draft(&mut self, draft: u64) {
        if self.composer.as_ref().is_some_and(|composer| composer.draft() == draft) {
            self.close_composer();
            if self.composer.is_some() {
                return;
            }
        }
        self.wait_discard(draft);
    }

    /// Hides the kept draft `draft` behind the toast with Undo, its discard waiting; a discard already waiting for
    /// another draft is carried out first.
    fn wait_discard(&mut self, draft: u64) {
        if self.discarding.is_some_and(|(waiting, _)| waiting != draft) {
            self.discard_waiting();
        }
        self.status.forget_notices();
        self.discarding = Some((draft, None));
        self.discarded = None;
    }

    /// Drops the discard that waits behind its toast: the draft stays. A composer its own Discard closed comes back as
    /// it was, in the reader's place or floating ([`InboxApp::discard_composer`]); the draft being written then is closed
    /// and kept first, and when the engine does not keep it, it stays open and the discarded draft stays a kept draft.
    fn undiscard(&mut self) {
        self.discarding = None;
        let Some(composer) = self.discarded.take() else { return };
        if self.composer.is_some() {
            self.close_composer();
            if self.composer.is_some() {
                return;
            }
        }
        self.composer = Some(*composer);
        self.list_over_composer = false;
    }

    /// The draft whose discard waits behind its toast, if one does.
    #[must_use]
    pub fn discarding(&self) -> Option<u64> {
        self.discarding.map(|(draft, _)| draft)
    }

    /// Carries out the discard that waits behind its toast, if one does.
    fn discard_waiting(&mut self) {
        self.discarded = None;
        if let Some((draft, _)) = self.discarding.take()
            && let Err(error) = self.mailbox.run("compose.discard", &json!({ "id": draft }))
        {
            self.mailbox.fail(error);
        }
    }

    /// Discards the draft being written with an Undo, as a discard from Home does ([`InboxApp::discard_draft`]): the
    /// draft is kept as it stands and waited for on the disk, the composer closes, and "Draft discarded" offers Undo for
    /// [`crate::status::UNDO_SECONDS`]. Undo brings the composer back as it was; when the time ends, the window closes
    /// or the app ends, the draft goes here and on the server (`compose.discard`). A new message nothing was written
    /// in goes at once, with no Undo. A draft the engine does not keep leaves the composer open, saying why.
    pub fn discard_composer(&mut self) {
        let Some(composer) = &mut self.composer else { return };
        let draft = composer.draft();
        if composer.untouched() {
            match composer.discard(&mut self.mailbox) {
                Ok(()) => {
                    self.composer = None;
                    self.say(tr!("Draft discarded"));
                }
                Err(error) => self.mailbox.fail(error),
            }
            return;
        }
        // Kept as it stands, what is typed in a recipient line left typed, so Undo brings back exactly this.
        if let Err(error) = composer.save(&mut self.mailbox).and_then(|()| self.mailbox.wait_for_keep(draft, KEEP_WAIT)) {
            return self.mailbox.fail(error);
        }
        let Some(mut composer) = self.composer.take() else { return };
        composer.dismiss_suggestions();
        composer.unsee();
        self.wait_discard(draft);
        self.discarded = Some(Box::new(composer));
    }

    /// Says `text` in the status line, for the `notice` token's time once it shows.
    pub(crate) fn say(&mut self, text: &str) {
        self.status.push(crate::status::Message::Notice { text: text.to_owned(), until: crate::status::NOTICE_SECONDS });
    }

    /// What the composer's last step says in the status line, while it shows or waits to.
    #[must_use]
    pub fn notice(&self) -> Option<&str> {
        self.status.notice()
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
        let windowed = self.windowed(ctx);
        let Some(composer) = &mut self.composer else { return };
        let mailbox = &mut self.mailbox;
        let result = match asked {
            Asked::Typed(line) => composer.typed(mailbox, line),
            Asked::Take(line) => composer.take(mailbox, line),
            Asked::Pick(at) => {
                if let Some(line) = composer.suggestions().0 {
                    focus_composer(ctx, windowed, &mut self.window_focus, compose::field_id(line));
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
                focus_composer(ctx, windowed, &mut self.window_focus, compose::blank_id());
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
                let windowed = self.windowed(ctx);
                focus_composer(ctx, windowed, &mut self.window_focus, compose::text_id());
                // Drawn again with the keys where they are now.
                ctx.request_repaint();
            }
        }
    }

    /// What the reader says with nothing open: the list holds nothing, the drafts, or messages.
    fn reader_says(&self) -> ReaderSays {
        let mailbox = &self.mailbox;
        let drafts = mailbox.drafts_listed();
        let outbox = mailbox.place() == Place::Outbox && mailbox.query().is_empty();
        let none = if outbox {
            mailbox.outbox().is_empty()
        } else if drafts {
            mailbox.drafts().next().is_none()
        } else {
            mailbox.messages().is_empty()
        };
        match (none, outbox, drafts) {
            (true, _, _) => ReaderSays::Nothing,
            (false, true, _) => ReaderSays::Outbox,
            (false, false, true) => ReaderSays::Draft,
            (false, false, false) => ReaderSays::Message,
        }
    }

    /// The reader on a card `left` from the list: the open message, or nothing open; what was asked in it.
    fn reader_card(&mut self, ui: &mut Ui, theme: &Theme, layers: &mut Panes, left: f32) -> reader::Drawn {
        let gap = 0.0;
        if self.reader.open_id().is_none() {
            let empty = self.reader_says();
            CentralPanel::no_frame().frame(around(left, gap, gap)).show(ui, |ui| {
                layers.card(ui, Margin::ZERO, |ui| match empty {
                    ReaderSays::Message => reader::nothing_open(ui, theme, &self.keymap),
                    ReaderSays::Draft | ReaderSays::Nothing | ReaderSays::Outbox => nothing_to_open(ui, theme, &self.keymap, empty),
                });
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
        self.reader_place = Some(reader);
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
        // Flush with the body like the list: no band of the ground above, beside or under it.
        let mut card = Rect::NOTHING;
        let shown = CentralPanel::no_frame().frame(around(left, 0.0, 0.0)).show(ui, |ui| {
            card = ui.max_rect();
            layers.card(ui, Margin::ZERO, |ui| compose::show(ui, theme, keymap, mailbox, composer))
        });
        // The card's own rect, inside its pane's margins.
        self.reader_place = Some(card);
        shown.inner
    }

    /// The composer's floating panel while it is not `docked` nor in its window of its own; what was asked in it.
    fn floating_composer(&mut self, ui: &Ui, theme: &Theme, docked: bool, body: Rect) -> Vec<compose::Asked> {
        match self.composer.as_mut() {
            Some(composer) if !docked && !self.composer_window => {
                // Over the reader's answers when it drew them in this pass: the panel never covers them.
                let above = reader::answers_rect(ui.ctx()).map(|answers| answers.top());
                compose::floating(ui.ctx(), theme, &self.keymap, &self.mailbox, composer, body, above)
            }
            _ => Vec::new(),
        }
    }

    /// A composer just opened is the floating panel when the layout asks for the floating host; its own Dock still
    /// puts it in the reader's place.
    fn float_new_composer(&mut self) {
        if self.composer_host() == Host::Floating
            && let Some(composer) = &mut self.composer
        {
            composer.set_floating(true);
        }
    }

    /// Where the layout asks the composer to show.
    fn composer_host(&self) -> Host {
        match self.mailbox.appearance().layout.composer {
            ComposerHost::Docked => Host::Docked,
            ComposerHost::Floating => Host::Floating,
            ComposerHost::Window => Host::Window(composer_window()),
        }
    }

    /// Whether the layout asks for the composer in a window of its own and one can open here.
    fn windowed(&self, ctx: &egui::Context) -> bool {
        matches!(self.composer_host().usable(ctx), Host::Window(_))
    }

    /// Where the composer shows this frame, as the layout says: in a window of its own where one can open; where none
    /// can (the web app, a headless context), docked, and the status line says why, once.
    fn place_composer(&mut self, ctx: &egui::Context) {
        let windowed = self.windowed(ctx);
        let was = self.composer_window;
        self.composer_window = windowed && self.composer.is_some();
        if self.composer_window
            && !was
            && let Some(composer) = &mut self.composer
        {
            // In its window it is never the floating panel, folded or not.
            composer.set_floating(false);
        }
        if was
            && !self.composer_window
            && self.composer_host() == Host::Floating
            && let Some(composer) = &mut self.composer
        {
            // Out of its window into the floating host: the open composer floats at once, as a new one opens.
            composer.set_floating(true);
        }
        if !self.composer_window {
            self.window_typing = false;
            self.window_focus = None;
        }
        let refused = matches!(self.composer_host(), Host::Window(_)) && !windowed;
        if refused && self.composer.is_some() && !self.window_refused_said {
            self.window_refused_said = true;
            self.say(tr!("No window of its own can open here: the message is written in the reader's place"));
        }
    }

    /// The composer in its window of its own while it shows there ([`InboxApp::place_composer`]); what was asked in it.
    /// The window's `Esc`, the desktop's close and its frame's Close close the composer as its Close does: the draft is
    /// on the disk first, and a draft the store does not keep leaves the window open with everything in it. Send's key
    /// in the window sends; the countdown and its Undo are the main window's status line's. The keyboard is never on
    /// nothing in the window: with none of its own, it goes to the To line.
    fn composer_in_window(&mut self, ctx: &egui::Context, theme: &Theme) -> Vec<compose::Asked> {
        if !self.composer_window {
            return Vec::new();
        }
        let (keymap, mailbox, frame) = (&self.keymap, &self.mailbox, self.frame);
        let Some(composer) = self.composer.as_mut() else { return Vec::new() };
        let title = composer.title();
        let look = host::Look { theme, frame: mailbox.appearance().frame, size: compose::WINDOW };
        let focus = &mut self.window_focus;
        let (mut typing, mut ran) = (false, Vec::new());
        let hosted = host::show_with(ctx, Host::Window(composer_window()), &title, look, true, |ui, surfaces| {
            let wanted =
                focus.take().or_else(|| ui.ctx().memory(|memory| memory.focused()).is_none().then(|| compose::field_id(crate::compose::Line::To)));
            if let Some(id) = wanted {
                ui.ctx().memory_mut(|memory| memory.request_focus(id));
            }
            let mut asked = compose::window(ui, theme, keymap, mailbox, composer, frame);
            // The window's own keys: Send, and the look of the app.
            if surfaces.allows(keymap::SEND, keymap) && keymap.pressed(ui.ctx(), keymap::SEND) && !soon::blocked(ui.ctx(), keymap::SEND) {
                asked.push(compose::Asked::Send);
            }
            ran.extend(crate::surface::WINDOW_KEYS.into_iter().filter(|id| surfaces.allows(id, keymap) && keymap.pressed(ui.ctx(), id)));
            let focused = ui.ctx().memory(|memory| memory.focused());
            typing = focused.is_some_and(compose::is_field);
            if focused.is_none() {
                // Tab walked off the last control: the next pass puts the keys back in To.
                ui.ctx().request_repaint();
            }
            asked
        });
        let host::Hosted::Window(host::Windowed { inner, closed }) = hosted else { return Vec::new() };
        self.window_typing = typing;
        for id in ran {
            self.run_action(ctx, id);
        }
        if !closed {
            return inner;
        }
        // What was asked before the close goes in first, then the close keeps the draft.
        for asked in inner {
            self.apply_compose(ctx, asked);
        }
        self.close_composer();
        if self.composer.is_some() {
            ctx.send_viewport_cmd_to(composer_window(), egui::ViewportCommand::CancelClose);
        } else {
            self.composer_window = false;
        }
        Vec::new()
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
    /// the window is closing, otherwise at most once in [`KEEP_EVERY`], a frame asked for when the rest is due. A close
    /// of the window waits for the keeps to reach the disk; a keep that failed is said once, and the store tries it
    /// again.
    fn keep_draft(&mut self, ctx: &egui::Context) {
        let closing = ctx.input(|input| input.viewport().close_requested());
        if self.composer.as_ref().is_some_and(Composer::changed) {
            let now = ctx.input(|input| input.time);
            let typing = ctx.memory(|memory| memory.focused()).is_some_and(compose::is_field) || (self.composer_window && self.window_typing);
            // A time that went back, or is no number, keeps it now.
            let wait = self.kept_at.map_or(0.0, |at| KEEP_EVERY - (now - at));
            if typing && !closing && wait > 0.0 && wait <= KEEP_EVERY {
                ctx.request_repaint_after(std::time::Duration::from_secs_f64(wait));
            } else {
                self.kept_at = Some(now);
                self.keep_draft_now();
            }
        }
        // A keep's failure is said first: a close taken back says its own after it, with how many drafts are not kept.
        self.unkept();
        if closing {
            self.close_with_keeps(ctx);
        } else {
            self.keeps_end.ended = false;
            if self.keeps_end.refused.is_some() && self.mailbox.unkept_drafts().is_empty() {
                self.keeps_end.refused = None;
            }
        }
    }

    /// A close of the window waits (at most [`KEEP_WAIT`]) for the drafts kept to reach the disk. When one does not,
    /// the close is taken back: the window stays, the failure in the status line with how many more drafts are not
    /// kept, the first such draft open in the composer when none is. A second close in a row (no keep of any draft
    /// since, within [`CLOSE_IN_A_ROW`]) goes through, and the newest text of those drafts is lost: the disk holds
    /// them as they were before. A later close, after new typing, is taken back again.
    fn close_with_keeps(&mut self, ctx: &egui::Context) {
        let Err(error) = self.mailbox.wait_for_keeps(KEEP_WAIT) else {
            self.keeps_end.ended = true;
            return;
        };
        let now = ctx.input(|input| input.time);
        if self.keeps_end.refused.is_some_and(|(then, at)| then == self.mailbox.keeps_made() && (0.0..=CLOSE_IN_A_ROW).contains(&(now - at))) {
            eprintln!("inboxcraft: closed with a draft not kept, the disk holds it as it was before: {error}");
            self.keeps_end.ended = true;
            return;
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        self.keeps_end.took_back = true;
        let unkept = self.mailbox.unkept_drafts();
        if self.composer.is_none()
            && let Some(draft) = unkept.first().copied()
        {
            self.reopen(draft);
        }
        // Counted after the draft opened: only a keep that comes after the take-back makes the next close another one.
        self.keeps_end.refused = Some((self.mailbox.keeps_made(), now));
        let error = match unkept.len().saturating_sub(1) {
            0 => error,
            1 => UiError::Refused(tr!("{error}; 1 more draft is not on the disk either", error = error)),
            more => UiError::Refused(tr!("{error}; {count} more drafts are not on the disk either", error = error, count = more)),
        };
        // The failures the wait brought are said by this line: not again, over it, in the next frame.
        let _ = self.mailbox.failed_keeps();
        self.mailbox.fail(error);
    }

    /// Whether a close of the window was taken back in the last frame, because a draft was not on the disk; once.
    pub fn took_back_close(&mut self) -> bool {
        std::mem::take(&mut self.keeps_end.took_back)
    }

    /// Keeps the draft being written and waits (at most [`KEEP_WAIT`]) for every draft kept to reach the disk: the
    /// end of the app without a window (a quit from the tray). The end of the app itself does not wait again.
    ///
    /// # Errors
    ///
    /// What was not kept, in words for the person: the disk holds that draft as it was before.
    pub fn end_keeps(&mut self) -> Result<(), String> {
        self.keeps_end.ended = true;
        self.keep_draft_now();
        self.mailbox.wait_for_keeps(KEEP_WAIT).map_err(|error| error.to_string())
    }

    /// Says in the status line each draft the store wrote behind whose write started failing, once: the store keeps
    /// its newest text and tries again, and a close waits for it.
    fn unkept(&mut self) {
        for (_, error) in self.mailbox.failed_keeps() {
            self.mailbox.fail(error);
        }
    }

    /// Keeps the draft at once if something in it changed since it was last kept: the end of a window or of the app.
    pub fn keep_draft_now(&mut self) {
        if let Some(composer) = self.composer.as_mut().filter(|composer| composer.changed())
            && let Err(error) = composer.save(&mut self.mailbox)
        {
            self.mailbox.fail(error);
        }
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

    /// Where a message is written, from the next frame (`ui.layout.set`): docked, floating or in a window of its own.
    pub fn set_composer_host(&mut self, host: ComposerHost) {
        self.appear("ui.layout.set", &json!({ "composer": host.id() }));
    }

    /// Where the event editor opens, for a new event and an edit, from the next frame (`ui.layout.set`): small, full or
    /// in a window of its own.
    pub fn set_event_editor_host(&mut self, host: EventEditorHost) {
        self.appear("ui.layout.set", &json!({ "event_editor": host.id() }));
    }

    /// Lets the app draw the window's frame: the native window calls it, so the title bar, the window controls and the
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

    /// Sets the owner's motion level, chosen, from the next frame (`ui.motion.set level=`).
    pub fn set_motion_level(&mut self, level: motion::Level) {
        self.appear("ui.motion.set", &json!({ "level": level.kept().id() }));
    }

    /// [`InboxApp::set_motion_level`] for a picture or a test that wants motion on or off: `reduce` is
    /// [`motion::Level::Off`], else [`motion::Level::Full`].
    pub fn set_reduce_motion(&mut self, reduce: bool) {
        self.set_motion_level(if reduce { motion::Level::Off } else { motion::Level::Full });
    }

    /// Installs the fonts once, and the appearance in the context when it changed; keeps a changed appearance.
    fn install_theme(&mut self, ctx: &egui::Context) {
        let chosen = *self.mailbox.appearance();
        if self.themed == Some(chosen) {
            return;
        }
        if self.themed.is_none() {
            theme::install_fonts(ctx);
            theme::size_from_the_setting_alone(ctx);
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
        let appearance = self.appearance();
        motion::follow(ui.ctx(), appearance.motion_level(), appearance.motion_chosen());
        self.apply_frame(ui.ctx());
        let theme = Theme::current(ui.ctx(), &Look::of(self.mailbox.appearance()));
        ui.set_style(theme.style());
        let p = theme.palette;
        let s = theme.spacing;
        // The list's order as last drawn, before the engine's events: a cheap handle on the kept one.
        self.drawn = list::order(ui.ctx());
        self.follow_context(ui.ctx());
        self.sync();
        if self.reader.poll() | self.windows.poll() | self.pump() | self.onboarding.poll() {
            ui.ctx().request_repaint();
        }
        // What the engine's events changed is settled before this frame's keys: when the open message left with them,
        // no key acts on the row the rule picked, which nobody has seen selected.
        self.sync();
        self.wake_when_due(ui.ctx());
        self.show_picker(ui.ctx());
        self.frame = ui.ctx().cumulative_frame_nr();
        self.place_composer(ui.ctx());
        self.place_event_editor(ui.ctx());
        // The list's rows as this frame draws them: a command of the frame that changes them draws the frame again.
        let rows_drawn = self.mailbox.rows_serial();
        // And the reader's place and the mode: a command of the frame that changes them draws it again (M7, M10, M14).
        let reader_drawn = self.reader_key();
        let mode_drawn = theme.mode;
        self.reader_place = None;
        self.just_sent = false;
        // The command line's keys first: with its results shown, or the keyboard in it, they are its own.
        // The movements read whether a key came after the keys were taken (the command line's first): noted now.
        motion::note_input(ui.ctx());
        // A key or a press during a change of place acts at once: the change jumps to its end. Read before the command
        // line takes its keys, so they end it too.
        let interrupted = crate::scramble::interrupted(ui.ctx());
        let command_keys = command::early(ui.ctx(), &mut self.command);
        if interrupted {
            self.change = None;
            self.picture = None;
            self.mode_picture = None;
        }
        // What stands in front, once, before any key is read.
        let surfaces = self.surfaces(ui.ctx());
        let Keys { mut ran, escape, to_place, typing, quiet } = self.keys(ui.ctx(), &surfaces);
        let undo_taken = self.take_lifted_undo(ui, &surfaces);
        let keyed = if typing { None } else { self.keyed_act(ui.ctx()) };
        self.act_by_key |= keyed.is_some();
        // One Esc closes one surface ([`Surfaces::escape`]): an open float (a menu, a picker, a select), the palette, a
        // capture and the event editor take theirs before anything under them. On the settings, a place, an Esc with
        // nothing in front closes nothing; in one pane, on a page alone, it goes back to the list of sections.
        let setting = self.settings.is_open();
        self.esc_to_sections(ui.ctx(), to_place && setting);
        let to_place = to_place && !setting;
        // The settings shown in the place of what was, and the palette coming over the window, close the lists, menus and
        // pickers open.
        crate::component::popup::covered(ui.ctx(), [setting, self.command.results()]);
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
        // The frame's four regions ([`shell::regions`]): the command line (the title bar in the own frame), the index at the left,
        // everything else in the body, the status line across the foot. Under 1280 the index is the narrow one and the
        // command field takes its room ([`shell::narrow`]).
        let window = ui.max_rect();
        let narrow = shell::narrow(ui.ctx());
        let regions = shell::regions_with(window.size(), &theme, ui.ctx().pixels_per_point(), narrow).translate(window.min.to_vec2());
        shell::lay(ui.ctx(), regions);
        Self::shell_lines(ui, &theme, regions);
        let layout = *ui.layout();
        let mut region = |salt: &str, rect: Rect| ui.new_child(egui::UiBuilder::new().id_salt(salt).max_rect(rect).layout(layout));
        let (mut command, mut index, mut body) =
            (region("shell-command", regions.command), region("shell-index", regions.index), region("shell-body", regions.body));
        let mut asked = MailAsked::default();
        let mut add = false;
        let mut header = list::Header::default();
        // The row sliding out is out when the list is not drawn.
        let mut listed = list::Asked { out: true, ..list::Asked::default() };
        let mut changed = None;
        let docked = self.docked();
        let full = self.reader.full_width() && !docked;
        // In one pane the open message (or the composer in its place) and the list swap.
        let (single, list_shown, reader_shown) = self.panes(body.ctx(), docked, full);
        // The panes stand in the body: its sides are the frame's hairlines, so no pane draws an edge along them.
        let panes = ui.painter().with_clip_rect(regions.body);
        let mut layers = Panes::begin(&panes);
        #[cfg(test)]
        body.data_mut(|data| data.remove::<egui::Rect>(egui::Id::unique("reader-rect")));
        let (gap, half) = (0.0, 0.0);
        let (indexed, commanded) = self.index_and_command(&mut index, &mut command, &theme, &surfaces, setting, (regions, command_keys));
        let index_to_list = Self::shows_list(&indexed);
        ran.extend(indexed.place.and_then(Self::index_action));
        ran.extend(indexed.compose.then_some(keymap::COMPOSE));
        asked.open = indexed.mailbox;
        asked.narrow = indexed.account;
        let mut dismiss = false;
        let mut on_calendar = Vec::new();
        let mut on_home = None;
        // The place the body draws in this frame: a command below that changes it changes it after the body was drawn.
        let drawn_place = self.index_place(setting);
        let composer_left = half;
        self.list_hidden = false;
        if setting {
            changed = self.settings_place(&mut body, &theme);
        } else if self.screen == Screen::Calendar {
            on_calendar = self.calendar_screen(&mut body, &theme, &mut layers);
        } else if self.screen == Screen::Home {
            on_home = self.home_place(&mut body, &theme);
        } else if self.mailbox.accounts().is_empty() {
            add = self.welcome(&mut body, &theme, &mut layers);
        } else if !full {
            let left = 0.0;
            let reading = self.reader.open_id().is_some() || self.held_back.is_some();
            // In two panes the list stands 440 wide beside the reader, with a message open or nothing open (the
            // reader's empty state says so), and beside the docked composer.
            // New senders' review is a place of cards across the whole body.
            let place = list_place(list_shown, !single && (self.mailbox.place() != Place::Senders || reading || docked));
            // The list is hidden where the composer stands in its place: in one pane with the composer in front.
            // Nothing of it acts then.
            let hidden = place == ListPlace::Away && self.composer_in_front();
            self.mailbox.hide_draft(self.discarding());
            self.mailbox.keep_drafts_fresh();
            let (mailbox, keymap) = (&self.mailbox, &self.keymap);
            let list = |ui: &mut Ui, layers: &mut Panes| {
                layers.card(ui, Margin::ZERO, |ui| {
                    // The pane's first column is the line at its left: the header and the rows start right of it.
                    let pad = margin(s.s4);
                    let searched = Panel::top("list-header")
                        .show_separator_line(false)
                        .frame(Frame::new().inner_margin(Margin { left: pad.left.saturating_add(1), ..pad }))
                        .show(ui, |ui| list::header(ui, &theme, mailbox))
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
                ListPlace::Beside => {
                    // Where the panel keeps the width a drag gave it (egui's `PanelState`): the tests plant one there.
                    #[cfg(test)]
                    body.data_mut(|data| data.insert_temp(egui::Id::unique("list-panel-id"), body.scope_id().with_salt(egui::IdSalt::new("list"))));
                    let shown = list_panel().frame(around(left, half, gap)).show(&mut body, |ui| list(ui, &mut layers));
                    self.list_rect = Some(shown.response.rect);
                    (header, listed) = shown.inner;
                }
                ListPlace::Whole => {
                    let shown = CentralPanel::no_frame().frame(around(left, gap, gap)).show(&mut body, |ui| list(ui, &mut layers));
                    self.list_rect = Some(shown.response.rect);
                    (header, listed) = shown.inner;
                }
            }
            self.hide_list(body.ctx(), hidden);
        }
        let mut drawn = reader::Drawn::default();
        let mut composed = Vec::new();
        // A texture lives no longer than the picture the reader keeps for it.
        self.textures.retain(|key, _| matches!(self.reader.picture(key), Some(PictureState::Ready(_))));
        if setting {
            // The settings stand in the body alone: no composer, no reader.
        } else if docked && reader_shown {
            composed = self.composer_card(&mut body, &theme, &mut layers, composer_left);
        } else if self.screen == Screen::Mail
            && !self.mailbox.accounts().is_empty()
            && (self.reader.open_id().is_some() || self.held_back.is_some() || (!single && self.mailbox.place() != Place::Senders))
            && reader_shown
            && !docked
        {
            drawn = self.reader_card(&mut body, &theme, &mut layers, if full { gap } else { half });
        }
        layers.finish(&panes, &theme);
        // A row's menu whose row this pass did not draw closes; an ask no row took is dropped.
        list::settle_menus(ui.ctx());
        if self.screen == Screen::Calendar && !setting {
            // The command line's results lie over the editor: they have the keys and the pointer.
            let over = self.command.results();
            on_calendar.extend(calendar::editor(ui, &theme, &self.keymap, &mut self.calendar, (over, self.editor_room)));
        }
        if !setting {
            composed.extend(self.floating_composer(ui, &theme, docked, regions.body));
        }
        // The composer's window lies under no modal surface of the main window: what is asked in it passes the gate.
        let windowed = self.composer_in_window(ui.ctx(), &theme);
        // So does the event editor's window: what is typed or pressed in it passes the gate.
        let edited_apart = self.event_editor_in_window(ui.ctx(), &theme);
        // The status line, a strip of its own at the window's foot: it covers nothing of the panes.
        let (mut line, mut undid) = self.status_line(ui, &theme, &surfaces, (quiet, undo_taken), regions.status);
        // The one gate for the place under a modal surface ([`Surfaces::modal`]): whatever pressed it (the pointer, a key,
        // a screen reader's activate, which egui's layers do not stop), nothing the list, the reader, the composer in this
        // window, Home, the calendar, the settings' page or the status line asked for in this frame is applied. The index
        // has its own gate ([`InboxApp::index_allowed`]). Its one exception is the Undo of a loss whose time runs out
        // ([`Undone::outlasts_the_gate`]).
        if surfaces.modal() {
            undid = undid.filter(|undone| undone.outlasts_the_gate());
            line = status::Pressed { undo: line.undo && undid.is_some(), ..status::Pressed::default() };
            listed = list::Asked { out: listed.out, ..list::Asked::default() };
            header = list::Header::default();
            drawn.asked.clear();
            composed.clear();
            on_calendar.clear();
            on_home = None;
            // A confirmation dialog's own answer is no ask of the place under it: it passes ([`settings::Asked::answers`]).
            changed = changed.filter(|asked| surfaces.has(Surface::Dialog) && asked.answers());
            add = false;
            dismiss = false;
        }
        composed.extend(windowed);
        on_calendar.extend(edited_apart);
        let (undone, dismissed) = self.apply_line(&line, undid);
        dismiss |= dismissed;
        ran.extend(line.hint);
        add |= line.open_account;
        let list::Asked { clicked, acted, out, review, done_all, outbox: outboxed, draft: drafted, server, screener: screener_mode, presets, gained } =
            listed;
        let acted = acted.or(keyed);
        let leaving = self.mailbox.leaving().is_some();
        let busy = dismiss
            || back_to_list
            || acted.is_some()
            || line != status::Pressed::default()
            || outboxed.is_some()
            || drafted.is_some()
            || server.is_some()
            || screener_mode.is_some()
            || (out && leaving)
            || left_full
            || !ran.is_empty()
            || !commanded.is_empty()
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
            || !on_calendar.is_empty()
            || on_home.is_some()
            || !composed.is_empty()
            || !drawn.asked.is_empty();
        if busy {
            // What changed shows in the next frame.
            ui.ctx().request_repaint();
        }
        self.act(out, acted, undone, outboxed, screener_mode);
        self.apply_drafts(ui.ctx(), drafted, server);
        if presets {
            // A row's time picker opened: the presets still ahead, read as the reader's picker reads them.
            self.mailbox.load_presets();
            ui.ctx().request_repaint();
        }
        if let Some(ids) = done_all {
            self.mailbox.done_all(&ids);
        }
        self.apply_calendar(on_calendar);
        self.keep_editor_window(ui.ctx());
        self.close_with_event(ui.ctx());
        ran.extend(self.settle_home(ui.ctx(), on_home));
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
        if add {
            self.open_accounts();
        }
        for asked in commanded {
            ran.extend(self.apply_command(ui.ctx(), asked));
        }
        for id in ran {
            self.run_action(ui.ctx(), id);
        }
        if let Some(changed) = changed {
            self.apply_settings(changed);
        }
        self.change_place(ui.ctx(), drawn_place, regions.body, p.bg);
        self.rows_changed(ui.ctx(), rows_drawn);
        self.reader_changed(ui.ctx(), reader_drawn, drawn_place, regions.status, &theme);
        // The outline before the change of mode keeps the window's picture: it is part of the window that fades (M14).
        self.own_frame_over(ui.ctx(), &theme, corner);
        self.mode_changed(ui.ctx(), mode_drawn, ui.ctx().content_rect());
        self.follow_accounts_page();
        self.keep_keys();
        self.keep_triage();
        self.keep_notify();
        self.keep_week();
        self.reader_windows(ui.ctx(), &theme);
        self.hear_desk();
        self.sync();
        // What runs after this pass, before the next frame, comes after everything this frame painted.
        // A press on the index that shows a list beside the docked composer, which holds the list's keys there: the
        // keyboard goes back to the composer's field it was in, never to nothing. Elsewhere the list takes the keys.
        self.keyboard_after_index(ui.ctx(), index_to_list);
        self.frame = ui.ctx().cumulative_frame_nr().saturating_add(1);
    }

    /// Shows what was asked for this frame: an account narrowed, a place opened in the pane or by a pill of the
    /// header, "show done", a search, or "New senders" from Review on the screener card.
    fn go(&mut self, asked: &MailAsked, header: list::Header, review: bool) {
        if let Some(account) = asked.narrow {
            self.mailbox.narrow(account);
        }
        // A mailbox pressed in the index shows the mail, from the calendar and the settings too; an account chosen in its
        // picker leaves the settings for the mail, as the owner asked to see that mail.
        if asked.open.is_some() || (asked.narrow.is_some() && self.settings.is_open()) {
            self.show_mail();
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

    /// The command line's mark, and the app's own frame before the panes when it draws one: the command line `line` as
    /// the title bar (its handle, before the field and the controls take their presses) and the resizing edges.
    fn own_frame_parts(&self, ui: &Ui, theme: &Theme, regions: shell::Regions) {
        let line = regions.command;
        frame::mark(ui, theme, line, shell::narrow(ui.ctx()));
        if !self.draws_frame() {
            return;
        }
        frame::title_bar(ui, line);
        frame::edges(ui.ctx());
    }

    /// The app's own frame over the panes, when it draws one: the window's outline.
    fn own_frame_over(&self, ctx: &egui::Context, theme: &Theme, corner: f32) {
        if self.draws_frame() {
            frame::outline(ctx, theme, corner);
        }
    }

    /// The command line in its region (`ui`): the field from the index's column to the window's controls, and its
    /// results for the mode typed; what was asked of it.
    fn command_line(&mut self, ui: &mut Ui, theme: &Theme, regions: shell::Regions, early: command::Early) -> Vec<command::Asked> {
        let ctx = ui.ctx().clone();
        // A search set elsewhere (another place shown empties it) shows in the field while it is not typed in.
        if self.command.mode() == crate::command::Mode::Search
            && !self.command.results()
            && !self.command.has_focus(&ctx)
            && self.command.text() != self.search
        {
            self.command.text_mut().clone_from(&self.search);
        }
        let index = regions.index.right() + 1.0 - regions.command.left();
        let room = if self.draws_frame() { frame::CONTROLS_ROOM } else { frame::NO_CONTROLS_ROOM };
        let mailbox = self.mailbox.title();
        // The field first, and the list searched as it was typed: the rows below are those of the text as this frame's
        // typing left it, and the line's lookup finds the list's own search done.
        let mut typed = command::field(ui, theme, &self.keymap, &mut self.command, (regions.command, index, room, &mailbox));
        for asked in std::mem::take(&mut typed.asked) {
            if matches!(asked, command::Asked::Searched(_)) {
                let _ = self.apply_command(&ctx, asked);
            } else {
                typed.asked.push(asked);
            }
        }
        if command::wants_people(&self.command) {
            let mut skip = std::collections::BTreeSet::new();
            skip.extend(self.mailbox.accounts().iter().map(|account| account.address.to_lowercase()));
            let found = crate::compose::people(&mut self.mailbox, self.command.text(), skip).unwrap_or_default();
            self.command.set_people(found);
        }
        let mut lists = command::Lists::default();
        let mut kept = None;
        if self.command.results() {
            let screen = if self.screen == Screen::Calendar { keymap::Scope::Calendar } else { keymap::Scope::Mail };
            // Over the event editor only what comes over it and the window's look: nothing of the mail or the
            // calendar under it (until a half-typed event can be kept as a draft).
            let editing = self.editing_event();
            let listed =
                |row: &crate::palette::Row| keymap::scope(row.id).meets(screen) && (!editing || crate::surface::EDITOR_KEYS.contains(&row.id));
            match self.command.mode() {
                crate::command::Mode::Command => {
                    lists.commands = crate::palette::rows(self.command.text())
                        .into_iter()
                        .filter(|row| listed(row))
                        .map(|row| {
                            let refused = self.refused_on_the_line(&ctx, row.id);
                            command::Listed { row, refused }
                        })
                        .collect();
                    // How many the line lists with nothing typed: the head says "2 of 82".
                    lists.all_commands = if self.command.text().trim().is_empty() {
                        lists.commands.len()
                    } else {
                        crate::palette::rows("").iter().filter(|row| listed(row)).count()
                    };
                }
                crate::command::Mode::Mailbox => lists.mailboxes = command::mailboxes(&self.mailbox, self.command.text()),
                // At most six, looked up only when the text changed.
                crate::command::Mode::Person => lists.people = self.command.people().map(<[_]>::to_vec).unwrap_or_default(),
                // Not narrowed to the mailbox shown: GO TO and LAST COMMANDS with nothing typed, else the mail, the
                // people and the mailboxes found, looked up only when what they read changed. Over the event editor
                // nothing that leaves the calendar is offered: no GO TO row, nothing found.
                crate::command::Mode::Search if !self.command.narrowed() => {
                    let text = self.command.text().to_owned();
                    lists.go_to = text.trim().is_empty();
                    lists.searched = !lists.go_to && !editing;
                    if !editing {
                        lists.goals = command::GOALS.to_vec();
                    }
                    if lists.go_to || lists.searched {
                        let stamp = self.line_stamp(&text, editing);
                        lists.found = match self.command.take_found(&stamp) {
                            Some(found) => found,
                            None => {
                                self.command.looked_up();
                                command::look_up(&mut self.mailbox, &text)
                            }
                        };
                        // Taken after the lookup, as Home's: its own reads count as seen.
                        kept = Some(self.line_stamp(&text, editing));
                    }
                    if lists.go_to {
                        lists.last = self
                            .command
                            .last_rows()
                            .iter()
                            .filter(|row| listed(row))
                            .map(|row| command::Listed { row: row.clone(), refused: self.refused_on_the_line(&ctx, row.id) })
                            .collect();
                    }
                }
                crate::command::Mode::Search => {}
            }
        }
        let mut asked = std::mem::take(&mut typed.asked);
        asked.extend(command::pick(&ctx, theme, &self.keymap, &mut self.command, &typed, &lists, early));
        if let Some(stamp) = kept {
            self.command.set_found(stamp, std::mem::take(&mut lists.found));
        }
        asked
    }

    /// What the command line's lookup for `text` reads, now (`editing`: the event editor is open).
    fn line_stamp(&self, text: &str, editing: bool) -> crate::command::Stamp {
        let now = self.mailbox.now();
        crate::command::Stamp {
            text: text.to_owned(),
            editing,
            revision: self.mailbox.revision(),
            writes: self.mailbox.writes(),
            account: self.mailbox.narrowed(),
            place: self.mailbox.place(),
            minute: (now.unix_seconds().div_euclid(60), self.mailbox.offset().minutes()),
        }
    }

    /// The place the index shows selected: Settings while they are shown (`setting`), else Home, the calendar, Later
    /// while a pile is shown, or the mail.
    /// M12, a change of place: when a command of this frame left `drawn` (the place the body drew in `body`), its
    /// body is kept for the movement and the frame is drawn again with the new place, so the owner never waits for it;
    /// while a change runs, its frame is painted over the body on the ground `ground`.
    fn change_place(&mut self, ctx: &egui::Context, drawn: index::Place, body: Rect, ground: egui::Color32) {
        if self.index_place(self.settings.is_open()) != drawn && !ctx.will_discard() {
            self.change = change::keep(ctx, body, motion::level(ctx));
            ctx.request_discard("a change of place draws its new place in the frame of its key");
            return;
        }
        if self.change.as_mut().is_some_and(|change| !change.paint(ctx, ground)) {
            self.change = None;
        }
    }

    /// The owner never waits: when a command of this frame changed the list's rows from `drawn` (Done, Snooze,
    /// Delete …), the frame is drawn again with them.
    fn rows_changed(&self, ctx: &egui::Context, drawn: u64) {
        if self.mailbox.rows_serial() != drawn && !ctx.will_discard() {
            ctx.request_discard("an act draws its list in the frame of its key");
        }
    }

    /// What the reader's place shows, as state: the docked draft, else the message open on the mail (the selected one,
    /// which the next [`InboxApp::sync`] opens).
    fn reader_key(&self) -> ReaderKey {
        let docked = self.composer.as_ref().filter(|_| self.docked()).map(Composer::draft);
        let open = if self.screen == Screen::Mail && docked.is_none() { self.mailbox.selected() } else { None };
        (docked, open)
    }

    /// M7 and M10: when a command of this pass opened a message or sent the docked draft (the reader's place showed
    /// `drawn`), what the place painted is kept for the movement of the level, and the frame is drawn again with the
    /// change, so the owner never waits for it; while one runs, its frame is painted over the place.
    fn reader_changed(&mut self, ctx: &egui::Context, drawn: ReaderKey, place: index::Place, status: Rect, theme: &Theme) {
        let now = self.reader_key();
        if ctx.current_pass_index() == 0 && now != drawn && self.index_place(self.settings.is_open()) == place {
            let sent = drawn.0.is_some() && now.0.is_none() && self.just_sent;
            let opened = drawn.0.is_none() && now.0.is_none() && now.1.is_some() && now.1 != drawn.1;
            if sent || opened {
                // From nothing drawn in the place (nothing was open) there is nothing to keep: the message wipes in over
                // the ground, or is there.
                let ground = theme.palette.bg;
                let layers = [egui::LayerId::background()];
                self.picture = self.reader_place.and_then(|rect| match (motion::level(ctx), sent) {
                    (motion::Level::Off, _) => None,
                    (motion::Level::Reduced, _) => picture::Picture::keep(ctx, rect, &layers, ground, picture::Form::Fade, picture::FAST, false),
                    (motion::Level::Full, false) => picture::Picture::keep(ctx, rect, &layers, ground, picture::Form::Under, reader::WIPE, false),
                    (motion::Level::Full, true) => {
                        let form = picture::Form::Fold { onto: status, line: status.height() };
                        picture::Picture::keep(ctx, rect, &layers, ground, form, picture::MEDIUM + picture::FAST, false)
                    }
                });
                if sent {
                    let until = ctx.input(|input| input.time) + ctx.options(|options| options.input_options.max_double_click_delay);
                    self.sent_place = self.reader_place.map(|rect| (rect, until, now));
                }
                draw_reader_again(ctx);
                return;
            }
        }
        let place = self.reader_place;
        if self.picture.as_mut().is_some_and(|picture| !picture.paint(ctx, place)) {
            self.picture = None;
        }
        self.shield(ctx);
    }

    /// While the reader's place plays a kept picture, and for egui's double-click time after a send from it, a press on
    /// the place reaches nothing: not the new message's controls hidden under the picture, nor what now stands where
    /// Send stood (a double click on Send would open a reply). A press during a picture ends it, and is let go with it.
    /// It stands at the background's order, over the place only: a float over the reader (the floating composer) keeps
    /// its presses.
    fn shield(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|input| input.time);
        let shows = self.reader_key();
        self.sent_place = self.sent_place.filter(|(_, until, after)| now < *until && *after == shows);
        if let Some((_, until, _)) = self.sent_place {
            // Gone at its time, even with nothing else to draw: a shield left standing would take the next press.
            ctx.request_repaint_after(std::time::Duration::try_from_secs_f64(until - now).unwrap_or_default());
        }
        let Some(rect) = self.picture.as_ref().map(picture::Picture::rect).or(self.sent_place.map(|(rect, _, _)| rect)) else { return };
        let id = egui::Id::unique("reader-shield");
        egui::Area::new(id).order(egui::Order::Background).fixed_pos(rect.min).constrain(false).movable(false).fade_in(false).show(ctx, |ui| {
            // Allocated, so the area is as large as the place and stands over it as a layer too.
            let _ = ui.allocate_rect(rect, egui::Sense::CLICK);
        });
    }

    /// M14: when a command of this pass changed the appearance (`drawn` was the mode it drew in `window`), it is put in
    /// at once and the frame drawn again with it; a change of mode keeps the old window to fade out over the new one at
    /// full (300 ms) and reduced (125 ms).
    fn mode_changed(&mut self, ctx: &egui::Context, drawn: theme::Mode, window: Rect) {
        if ctx.current_pass_index() == 0 && self.themed != Some(*self.mailbox.appearance()) {
            self.install_theme(ctx);
            // The context's mode follows its preference from the next pass: the one it will show.
            let shown = match self.mailbox.appearance().theme {
                ModePreference::Light => theme::Mode::Light,
                ModePreference::Dark => theme::Mode::Dark,
                ModePreference::System => {
                    ctx.system_theme().map_or(drawn, |system| if system == egui::Theme::Light { theme::Mode::Light } else { theme::Mode::Dark })
                }
            };
            if shown != drawn {
                let seconds = match motion::level(ctx) {
                    motion::Level::Full => Some(picture::MEDIUM),
                    motion::Level::Reduced => Some(picture::FAST),
                    motion::Level::Off => None,
                };
                let ground = theme::Palette::of(drawn).bg;
                self.mode_picture = seconds
                    .and_then(|seconds| picture::Picture::keep(ctx, window, &picture::every_layer(ctx), ground, picture::Form::Fade, seconds, true));
            }
            ctx.request_discard("an appearance chosen shows in the frame of its key");
            return;
        }
        if self.mode_picture.as_mut().is_some_and(|picture| !picture.paint(ctx, None)) {
            self.mode_picture = None;
        }
    }

    fn index_place(&self, setting: bool) -> index::Place {
        if setting {
            index::Place::Settings
        } else if self.screen == Screen::Home {
            index::Place::Home
        } else if self.screen == Screen::Calendar {
            index::Place::Calendar
        } else if matches!(self.mailbox.place(), Place::Later(_)) {
            index::Place::Later
        } else {
            index::Place::Mail
        }
    }

    /// The frame's hairlines between the regions: under the command line, between the index and the body, over the
    /// status line. The window's outline is the frame's own ([`frame::outline`]).
    fn shell_lines(ui: &Ui, theme: &Theme, regions: shell::Regions) {
        let (painter, colour) = (ui.painter(), theme.palette.line);
        let (command, index, status) = (regions.command, regions.index, regions.status);
        paint::hairline(painter, pos2(command.left(), command.bottom()), pos2(command.right(), command.bottom()), colour);
        paint::hairline(painter, pos2(index.right(), index.top()), pos2(index.right(), index.bottom()), colour);
        paint::hairline(painter, pos2(status.left(), index.bottom()), pos2(status.right(), index.bottom()), colour);
    }

    /// The window with no account: one card in the middle, as tall as what it says; whether Add account was pressed.
    fn welcome(&self, ui: &mut Ui, theme: &Theme, layers: &mut Panes) -> bool {
        onboard::welcome(ui, theme, &self.keymap, layers, self.can_add)
    }

    /// The calendar screen: its empty state on the calendar's card until there are calendars; what was pressed.
    fn calendar_screen(&self, ui: &mut Ui, theme: &Theme, layers: &mut Panes) -> Vec<calendar::Asked> {
        if self.calendar.is_empty() {
            return calendar::empty(ui, theme, &self.keymap, layers);
        }
        // The command line's results lie over the screen's floats: they have the keys.
        let over = self.command.results();
        let full = self.calendar.editor().is_some() && self.editor_room == calendar::Room::Full;
        calendar::screen(ui, theme, &self.keymap, &self.calendar, layers, (over, full))
    }

    /// Home in the body: the model kept ([`crate::home::Kept`], built again only when what it shows changed), the
    /// character field kept, at the motion level the settings say, the Qubernetic emblem and the names on it where they
    /// fit; a frame is asked for when the clock next changes what it shows. What was pressed on it.
    fn home_place(&mut self, ui: &mut Ui, theme: &Theme) -> Option<home::Asked> {
        let rect = ui.available_rect_before_wrap();
        let level = motion::level(ui.ctx());
        let hidden = self.discarding();
        let shown = self.home.current(&mut self.mailbox, hidden);
        let stage = home::Stage { field: &mut self.field, emblem: Some(crate::brand::emblem()), level };
        let drawn = home::show(ui, theme, &self.keymap, shown, rect, stage);
        if let Some(due) = self.home.due_in(&self.mailbox) {
            ui.ctx().request_repaint_after(due);
        }
        drawn.asked
    }

    /// After a frame: what was pressed on Home applied (under the command line's results the window's one gate has
    /// dropped it already); Home not shown, its next frame builds it again. The action to run, if one was asked.
    fn settle_home(&mut self, ctx: &egui::Context, asked: Option<home::Asked>) -> Option<&'static str> {
        let action = asked.and_then(|asked| self.apply_home(ctx, asked));
        if self.settings.is_open() || self.screen != Screen::Home {
            self.home.forget();
        }
        action
    }

    /// Applies what was pressed on Home: a count's tile shows its mailbox or pile, a draft row opens that draft in the
    /// composer (the draft being written is kept first, as every other way to a draft does), its discard hides it behind
    /// a toast with Undo and discards it when the toast ends ([`InboxApp::discard_draft`]), the agenda's link shows the calendar on today, the buttons do what they say. An
    /// action that runs as its key does is given back, to run through [`InboxApp::run_action`] and its gates.
    fn apply_home(&mut self, ctx: &egui::Context, asked: home::Asked) -> Option<&'static str> {
        use crate::home::Counted;
        use home::Asked;
        let mail = |app: &mut Self, place: Place| {
            app.show_mail();
            app.mailbox.open(place);
            app.search.clear();
        };
        match asked {
            Asked::Count(Counted::ToRead) => mail(self, Place::View(Role::Inbox)),
            Asked::Count(Counted::ToScreen) => mail(self, Place::Senders),
            Asked::Count(Counted::BackToday) => mail(self, Place::Later(Pile::Snoozed)),
            Asked::Count(Counted::Outbox | Counted::NotSent) | Asked::OpenOutbox => mail(self, Place::Outbox),
            Asked::AllDrafts => mail(self, Place::View(Role::Drafts)),
            Asked::Week | Asked::Today => {
                self.show_calendar();
                let view = if asked == Asked::Week { crate::calendar::View::Week } else { crate::calendar::View::Day };
                self.calendar.set_view(&mut self.mailbox, view);
                self.calendar.go_today(&mut self.mailbox);
            }
            Asked::OpenDraft(draft) => self.open_draft(ctx, draft),
            Asked::DiscardDraft(draft) => self.discard_draft(draft),
            Asked::ReadMail => return Some(keymap::MAIL),
            Asked::Write => return Some(keymap::COMPOSE),
            Asked::Calendar => return Some(keymap::CALENDAR),
            Asked::AddAccount | Asked::OpenAccount => self.open_accounts(),
            // The window has no way to the sample mail of its own: it is another start (`just run-sample`).
            Asked::SampleMail => {}
        }
        None
    }

    /// The settings in the body; what was asked on them.
    fn settings_place(&mut self, ui: &mut Ui, theme: &Theme) -> Option<settings::Asked> {
        let level = motion::level(ui.ctx());
        let desk = self.desk_view();
        let view = settings::Sheet {
            desk,
            keymap: &self.keymap,
            settings: &mut self.settings,
            current: self.mailbox.appearance(),
            level,
            mailbox: &self.mailbox,
            onboarding: &mut self.onboarding,
            can_add: self.can_add,
            writing: self.composer.as_ref().map(Composer::account),
        };
        settings::place(ui, theme, view)
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
        // Every owner's way to Undo ends here: while the event editor is open under the owner's hand it rests, whatever
        // the frame began with. Off the calendar, and in the main window while the editor is in its window of its own,
        // it runs as everywhere: the status line offers it there ([`Surfaces::undo_rests`]).
        if self.editing_event() {
            return;
        }
        let before = self.mailbox.selected();
        self.mailbox.undo();
        // The row given back is selected again: the ring goes to it too, so the next key acts on what is marked. An Undo
        // that selected nothing again leaves the ring where the owner put it.
        if let (Some(ringed), Some(back)) = (self.list_row, self.mailbox.selected())
            && ringed != back
            && before != Some(back)
        {
            self.ring_back = Some(back);
        }
        self.restored = self.docked();
        if self.screen == Screen::Calendar {
            self.calendar.refresh(&mut self.mailbox);
        }
        self.reader.refresh_cards_and_why(&mut self.mailbox);
    }

    /// The act whose key was pressed in this frame, on the selected row.
    fn keyed_act(&self, ctx: &egui::Context) -> Option<(u64, Act)> {
        let (action, _) = ROW_ACTS.into_iter().find(|(action, _)| self.keymap.pressed(ctx, action) && !soon::blocked(ctx, action))?;
        self.row_act(action)
    }

    /// The status line of the frame in `rect` ([`status::line`]): the facts, the key hints that work now, and the one
    /// message shown: the countdown while a send waits for its hold, else a discard's or the last act's Undo, else an
    /// error or a notice of the queue ([`crate::status::Status`]). What was pressed in it, with the message it was
    /// pressed on; nothing is applied here, so the one gate under a modal surface ([`Surfaces::modal`]) holds for it.
    fn status_line(&mut self, ui: &mut Ui, theme: &Theme, surfaces: &Surfaces, keys: (bool, bool), rect: Rect) -> (status::Pressed, Option<Undone>) {
        let (quiet, undo_taken) = keys;
        let now = ui.input(|input| input.time);
        let countdown = self.countdown_message(now);
        let discard = if countdown.is_some() {
            // Its time runs while the countdown has the line.
            self.discard_time(ui, now);
            None
        } else {
            self.discard_message(ui, now)
        };
        let act = if countdown.is_some() || discard.is_some() { None } else { self.act_message(now) };
        if let Some(error) = self.mailbox.error() {
            if self.seen_error.as_deref() != Some(error) {
                self.seen_error = Some(error.to_owned());
                self.status.push(crate::status::Message::Error { text: error.to_owned() });
            }
        } else {
            self.seen_error = None;
        }
        self.read_facts(now);
        if self.facts.0.next.is_some() {
            // The next event's minutes left change as the minute ends: one frame then, read again.
            ui.ctx().request_repaint_after(status::to_next_minute(self.mailbox.now()));
        }
        let (over, whole) = match (&countdown, &discard, &act) {
            (Some((draft, message)), _, _) => (Some((Undone::Send(*draft), message.clone())), self.countdown_whole.map_or(0.0, |(_, whole)| whole)),
            (None, Some(message), _) => (Some((Undone::Discard, message.clone())), crate::status::UNDO_SECONDS),
            (None, None, Some((message, undoable))) => {
                (Some((if *undoable { Undone::Act } else { Undone::Nothing }, message.clone())), crate::status::UNDO_SECONDS)
            }
            (None, None, None) => (None, 0.0),
        };
        let (which, over) = over.unzip();
        // The window's message, when it gave one, is the one drawn: a press on the line's Undo is that one's.
        let shown = self.status.shown(now, None, over);
        match &shown {
            Some(crate::status::Message::Notice { until, .. }) => {
                ui.ctx().request_repaint_after(std::time::Duration::from_secs_f64((until - now).clamp(0.0, 3600.0)));
            }
            Some(crate::status::Message::Undo { until, .. } | crate::status::Message::Countdown { until, .. }) => {
                // The seconds left and the bar change as each second ends, and then only.
                let part = (until - now).max(0.0).fract();
                ui.ctx().request_repaint_after(std::time::Duration::from_secs_f64(if part > 0.001 { part } else { 1.0 }));
            }
            _ => {}
        }
        // Done's key is the list's (an act on the selected row): its hint shows while the list has the keys. Under a
        // confirmation dialog the line says only its Esc, in the safe button's word; a press on it does nothing (the
        // gate under a modal surface drops it).
        let dialog = self.settings.confirm_keep(self.onboarding.asking_remove()).filter(|_| surfaces.has(Surface::Dialog));
        let hints: Vec<(&'static str, String, &'static str)> = match dialog {
            Some(word) => vec![(
                status::DIALOG_ESC,
                crate::component::key::key_words(&egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::Escape)),
                word,
            )],
            None => status::hints(self.status_keys(surfaces))
                .iter()
                .filter(|(action, _)| surfaces.allows(action, &self.keymap) || (*action == keymap::DONE && surfaces.list_has_keyboard()))
                .filter_map(|(action, word)| Some((*action, self.keymap.chip(ui.ctx(), action)?, *word)))
                .collect(),
        };
        let countdown_shown = matches!(shown, Some(crate::status::Message::Countdown { .. }));
        let rests = surfaces.undo_rests() && !countdown_shown;
        // Under a modal surface the Undo of a loss whose time runs out stays in reach: the message lies over the surface's
        // dimmed window, and the field in front does not keep its key ([`Undone::outlasts_the_gate`]).
        let lifted = surfaces.modal() && which.is_some_and(Undone::outlasts_the_gate);
        let drawn = status::Shown {
            facts: &self.facts.0,
            message: shown.as_ref(),
            now,
            whole,
            hints: &hints,
            undo_key: !quiet || lifted,
            rests,
            lifted,
            undo_taken,
        };
        let pressed = status::line(ui, theme, &self.keymap, rect, &drawn);
        (pressed, which)
    }

    /// Which key hints the status line shows: the docked composer's, the open message's, or the list's with nothing
    /// open (none in it: write); away from the mail's lists the open message's set.
    fn status_keys(&self, surfaces: &Surfaces) -> status::Keys {
        if self.screen != Screen::Mail || surfaces.on_settings() || self.mailbox.accounts().is_empty() {
            status::Keys::Message
        } else if self.composer.is_some() && self.docked() {
            status::Keys::Composer
        } else if self.reader.open_id().is_some() {
            status::Keys::Message
        } else {
            match self.reader_says() {
                ReaderSays::Nothing => status::Keys::Empty,
                ReaderSays::Outbox => status::Keys::Outbox,
                ReaderSays::Message | ReaderSays::Draft => status::Keys::List,
            }
        }
    }

    /// Applies what was pressed in the status line on the message `undid` names: a send taken back, a discard dropped.
    /// Whether the last act is to be taken back, and whether the mailbox's error goes (it was the error dismissed).
    fn apply_line(&mut self, line: &status::Pressed, undid: Option<Undone>) -> (bool, bool) {
        match undid.filter(|_| line.undo) {
            Some(Undone::Send(draft)) => self.unsend(draft),
            Some(Undone::Discard) => self.undiscard(),
            _ => {}
        }
        let dismissed =
            line.dismiss && self.status.dismiss().is_some_and(|text| self.mailbox.error().is_some_and(|error| crate::status::clean(error) == text));
        (line.undo && undid == Some(Undone::Act), dismissed)
    }

    /// The facts of the line, read again when the mailbox changed or a second has passed ([`status::read`]).
    fn read_facts(&mut self, now: f64) {
        let stamp = (self.mailbox.revision(), self.mailbox.writes());
        let fresh = self.facts.1.is_some_and(|(revision, writes, at)| (revision, writes) == stamp && (0.0..1.0).contains(&(now - at)));
        if !fresh {
            let facts = self.mailbox.quietly(status::read);
            // The reads are the facts' own: they count as seen.
            self.facts = (facts, Some((self.mailbox.revision(), self.mailbox.writes(), now)));
        }
    }

    /// The countdown while the message sent last waits for its hold: "sending · subject", the seconds left and the
    /// share of the hold, with Undo and its key, which take the send back. The draft and the message.
    fn countdown_message(&mut self, now: f64) -> Option<(u64, crate::status::Message)> {
        let Some((draft, seconds)) = self.counting() else {
            self.sending = None;
            self.countdown_whole = None;
            return None;
        };
        let row = self.mailbox.outbox().iter().find(|row| row.draft == draft);
        let subject = row.map(|row| row.subject.clone()).unwrap_or_default();
        let left = row.and_then(|row| match row.state {
            crate::mailbox::SendState::Waiting(Some(at)) => at.duration_since(self.mailbox.now()),
            _ => None,
        });
        #[expect(clippy::cast_precision_loss, reason = "seconds of a hold")]
        let left = left.map_or(seconds as f64, |left| left.as_secs_f64());
        let whole = match self.countdown_whole {
            Some((held, whole)) if held == draft => whole.max(left),
            _ => left,
        };
        self.countdown_whole = Some((draft, whole));
        let text = if subject.is_empty() { tr!("sending").to_owned() } else { tr!("sending · {subject}", subject = crate::status::clean(&subject)) };
        Some((draft, crate::status::Message::Countdown { text, until: now + left }))
    }

    /// Takes Undo's key, when it was pressed, before the command line's field is drawn, while that key is the lifted
    /// Undo's under the command line's results: the send's countdown, or a draft's discard from Home whose Undo does not
    /// rest ([`Undone::outlasts_the_gate`]). The key then does one thing, it stops the loss, and the field keeps its text;
    /// the status line is handed the press. Without such a loss the field keeps the key as its own undo.
    fn take_lifted_undo(&self, ui: &Ui, surfaces: &Surfaces) -> bool {
        if !surfaces.modal() {
            return false;
        }
        let now = ui.input(|input| input.time);
        let discard_waits = self.discarding.is_some_and(|(_, since)| since.is_none_or(|since| crate::status::UNDO_SECONDS - (now - since) > 0.0));
        let lifted = self.counting().is_some() || (discard_waits && !surfaces.undo_rests());
        lifted && self.keymap.pressed(ui.ctx(), keymap::UNDO)
    }

    /// Runs the time of a draft discarded from Home ([`InboxApp::discard_draft`]): when it ends, or the window closes,
    /// the discard is carried out. Whether it still waits.
    fn discard_time(&mut self, ui: &Ui, now: f64) -> bool {
        let Some((draft, since)) = self.discarding else { return false };
        let since = since.unwrap_or(now);
        let left = crate::status::UNDO_SECONDS - (now - since);
        // Not a number, over, or the window closes: carried out, not lost with the message.
        let closing = ui.input(|input| input.viewport().close_requested());
        let Some(wait) = std::time::Duration::try_from_secs_f64(left).ok().filter(|_| left > 0.0 && !closing) else {
            self.discard_waiting();
            return false;
        };
        self.discarding = Some((draft, Some(since)));
        ui.ctx().request_repaint_after(wait);
        true
    }

    /// The message of a draft discarded from Home while its discard waits: "Draft discarded" with Undo and its key,
    /// for the `undo` token's time.
    fn discard_message(&mut self, ui: &Ui, now: f64) -> Option<crate::status::Message> {
        if !self.discard_time(ui, now) {
            return None;
        }
        let since = self.discarding.and_then(|(_, since)| since).unwrap_or(now);
        Some(crate::status::Message::Undo { text: tr!("Draft discarded").to_owned(), until: since + crate::status::UNDO_SECONDS })
    }

    /// The message of the last act, for the `undo` token's time; after it Ctrl+Z still takes acts back, one at a time.
    /// The message and whether its act can be taken back (one that cannot says so and offers no Undo).
    fn act_message(&mut self, now: f64) -> Option<(crate::status::Message, bool)> {
        let Some(undo) = self.mailbox.undoable().cloned() else {
            self.toast = None;
            return None;
        };
        let since = match self.toast {
            Some((serial, since)) if serial == undo.serial => since,
            _ => now,
        };
        let until = since + crate::status::UNDO_SECONDS;
        if until <= now {
            self.mailbox.forget_undo();
            self.toast = None;
            return None;
        }
        self.toast = Some((undo.serial, since));
        let text = crate::status::clean(&undo.text);
        let message = if undo.undoable { crate::status::Message::Undo { text, until } } else { crate::status::Message::Notice { text, until } };
        Some((message, undo.undoable))
    }

    /// Whether the mail is the place shown: not the settings, the calendar or Home.
    fn mail_shown(&self) -> bool {
        self.screen == Screen::Mail && !self.settings.is_open()
    }

    /// Applies what was asked for in the command line; the action to run, when one was chosen.
    fn apply_command(&mut self, ctx: &egui::Context, asked: command::Asked) -> Option<&'static str> {
        match asked {
            // Send from the line sends the draft drawn, floating or docked, on the owner's Enter or press alone (the
            // line runs nothing of an agent's); the keyboard is in the line, not in the composer, so the key's rule
            // ([`InboxApp::can_run`]) is not the one that decides here.
            command::Asked::Run(keymap::SEND) => {
                self.command.close(ctx);
                // A draft kept for a reason says it, in its Send's words: the line closed, they may not be in sight.
                if self.can_run_from_the_line(keymap::SEND) && !soon::blocked(ctx, keymap::SEND) {
                    if self.send_drawn() {
                        let problem = self.composer.as_ref().and_then(|composer| compose::send_problem(&self.mailbox, composer));
                        if let Some(problem) = problem {
                            self.say(&problem);
                        }
                    } else if self.composer.is_none() {
                        // Sent (or scheduled): only now is it a command that ran from the line.
                        self.command.ran(keymap::SEND);
                    }
                }
            }
            command::Asked::Run(id) => {
                // Recorded as run only when it runs: by the gate [`InboxApp::run_action`] puts it through after this.
                if self.can_run(id) && !soon::blocked(ctx, id) {
                    self.command.ran(id);
                }
                self.command.close(ctx);
                return Some(id);
            }
            command::Asked::Page(page) => {
                self.command.ran(page);
                self.command.close(ctx);
                self.palette_page(page);
            }
            command::Asked::Refused(words) => self.say(&words),
            command::Asked::GoTo(goal) => {
                self.command.close(ctx);
                let place = match goal {
                    command::Goal::Inbox => Place::View(Role::Inbox),
                    command::Goal::Senders => Place::Senders,
                    command::Goal::Calendar => return Some(keymap::CALENDAR),
                };
                self.show_mail();
                self.mailbox.open(place);
                self.search.clear();
            }
            command::Asked::Message { id, text } => {
                // As Enter on the search, then as a press on the message's row in the list.
                self.command.close_results();
                self.show_mail();
                self.search(&text);
                if !self.open_in_window(id) {
                    self.mailbox.read_preview(id);
                    self.mailbox.select(Some(id));
                    self.reading = true;
                    self.focus_reader = one_pane(ctx);
                    self.float_behind();
                }
            }
            command::Asked::Person(address) => {
                // Their mail: the search of the mail shown, by their address, read as it is (never as a prefix).
                self.command.set_search(&address);
                self.command.close_results();
                self.show_mail();
                self.search(&address);
            }
            command::Asked::Mailbox(id) => {
                self.command.close(ctx);
                self.show_mail();
                self.mailbox.open(Place::Folder(id));
                self.search.clear();
            }
            command::Asked::Searched(text) => {
                // Typing never changes the place: the list fills as it is typed only while the mail shows.
                if self.mail_shown() {
                    self.search(&text);
                }
            }
            command::Asked::Find(text) => {
                // Enter, the owner's act: a search typed elsewhere shows the mail it fills, by the place rules (leaving
                // the settings clears the add-account password).
                self.command.close_results();
                if !text.is_empty() && !self.mail_shown() {
                    self.show_mail();
                }
                if self.mail_shown() {
                    self.search(&text);
                }
            }
            command::Asked::Close => self.command.close_results(),
        }
        None
    }

    /// Applies what was asked for on the settings page: each control through its `ui.*.set` command.
    fn apply_settings(&mut self, asked: settings::Asked) {
        match asked {
            settings::Asked::Mode(mode) => self.set_mode(mode),
            settings::Asked::Scale(scale) => self.set_text_scale(scale),
            settings::Asked::Motion(level) => self.set_motion_level(level),
            settings::Asked::Frame(frame) => self.set_frame(frame),
            settings::Asked::Composer(host) => self.set_composer_host(host),
            settings::Asked::ReaderHost(host) => self.appear("ui.layout.set", &json!({ "reader": host.id() })),
            settings::Asked::EventEditor(host) => self.appear("ui.layout.set", &json!({ "event_editor": host.id() })),
            settings::Asked::Section(section) => self.settings.show(section),
            settings::Asked::Sections => self.settings.to_list(false),
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
            settings::Asked::After(after) => self.mailbox.set_after(after),
            settings::Asked::GreetBy(by) => self.mailbox.set_greet_by(by),
            settings::Asked::Hold(seconds) => self.mailbox.set_hold(seconds),
            settings::Asked::Level(account, level) => self.mailbox.set_notify_level(account, level),
            settings::Asked::Returns(on) => self.mailbox.set_notify_returns(on),
            settings::Asked::Reminders(on) => self.mailbox.set_notify_reminders(on),
            settings::Asked::Alerts(on) => self.mailbox.set_notify_alerts(on),
            settings::Asked::FirstDay(day) => self.mailbox.set_week(Some(day), None),
            settings::Asked::FirstWeek(first) => self.mailbox.set_week(None, Some(first)),
            settings::Asked::AllDayAlert(choice) => self.mailbox.set_all_day_alert(choice),
            settings::Asked::Background(on) => self.set_background(on),
            settings::Asked::Login(on) => self.set_login(on),
            asked @ (settings::Asked::Templates
            | settings::Asked::NewTemplate(_)
            | settings::Asked::EditTemplate(..)
            | settings::Asked::SaveTemplate
            | settings::Asked::CloseTemplate
            | settings::Asked::AskDeleteTemplate(..)
            | settings::Asked::DeleteTemplate(..)
            | settings::Asked::KeepTemplate) => self.apply_templates(asked),
            asked @ (settings::Asked::Scheduled
            | settings::Asked::AskCancelScheduled(_)
            | settings::Asked::CancelScheduled(_)
            | settings::Asked::KeepScheduled) => self.apply_scheduled(asked),
            asked @ (settings::Asked::ReadPrivacy | settings::Asked::AskAgain(_)) => self.apply_privacy(asked),
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

    /// Removes the account `account` as the owner's press of its form's question does, for a picture's setup: the form
    /// opens, asks, and the question is answered. Whether it was removed.
    pub(crate) fn remove_account_as_owner(&mut self, account: u64) -> bool {
        self.onboarding.edit(&mut self.mailbox, account);
        self.onboarding.ask_remove();
        self.apply_page(onboard::Asked::Remove);
        !self.mailbox.accounts().iter().any(|row| row.id == account)
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
            onboard::Asked::AskRemove => onboarding.ask_remove(),
            onboard::Asked::KeepAccount => onboarding.keep_account(),
            onboard::Asked::Remove => {
                // The owner's press of the question: the only way the form removes an account.
                let gone = if let crate::onboard::Stage::Edit(id) = onboarding.stage() { Some(id) } else { None };
                if let Some(said) = onboarding.remove(mailbox) {
                    // The message being written from it goes with its drafts, as the question said: nothing keeps it
                    // into a draft that is gone.
                    if self.composer.as_ref().is_some_and(|composer| gone == Some(composer.account())) {
                        self.composer = None;
                    }
                    self.say(&said);
                    self.settings.hand_to_nav();
                }
            }
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
                let windowed = self.windowed(ui.ctx());
                focus_composer(ui.ctx(), windowed, &mut self.window_focus, field);
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
            reader::Asked::Original(id) => reader.open_original(id, ui.ctx()),
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
        reader.refresh_cards_and_why(mailbox);
    }
}

/// The composer's window of its own.
fn composer_window() -> egui::ViewportId {
    egui::ViewportId::from_hash_of("inboxcraft-composer-window")
}

/// The event editor's window of its own.
fn event_editor_window() -> egui::ViewportId {
    egui::ViewportId::from_hash_of("inboxcraft-event-editor-window")
}

/// Gives the keyboard to the composer's field `id`: at the next pass of its window of its own when it shows there
/// (`windowed`), never to a field the main window does not draw; in the main window otherwise.
fn focus_composer(ctx: &egui::Context, windowed: bool, pending: &mut Option<egui::Id>, id: egui::Id) {
    if windowed {
        *pending = Some(id);
        ctx.request_repaint();
    } else {
        ctx.memory_mut(|memory| memory.request_focus(id));
    }
}

#[cfg(test)]
#[path = "account_remove_tests.rs"]
mod account_remove_tests;
#[cfg(test)]
#[path = "after_stay_tests.rs"]
mod after_stay_tests;
#[cfg(test)]
#[path = "agent_undo_tests.rs"]
mod agent_undo_tests;
#[cfg(test)]
#[path = "appearance_tests.rs"]
mod appearance_tests;
#[cfg(test)]
#[path = "command_tests.rs"]
mod command_line_tests;
#[cfg(test)]
#[path = "compose_discard_tests.rs"]
mod compose_discard_tests;
#[cfg(test)]
#[path = "compose_tests.rs"]
mod compose_tests;
#[cfg(test)]
#[path = "compose_window_tests.rs"]
mod compose_window_tests;
#[cfg(test)]
#[path = "cover_tests.rs"]
mod cover_tests;
#[cfg(test)]
#[path = "drafts_tests.rs"]
mod drafts_tests;
#[cfg(test)]
#[path = "fit_tests.rs"]
mod fit_tests;
#[cfg(test)]
#[path = "float_cover_tests.rs"]
mod float_cover_tests;
#[cfg(test)]
#[path = "frame_tests.rs"]
mod frame_tests;
#[cfg(test)]
#[path = "home_place_tests.rs"]
mod home_place_tests;
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
#[path = "new_message_tests.rs"]
mod new_message_tests;
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
#[path = "place_tests.rs"]
mod place_tests;
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
mod round_tests;
#[cfg(test)]
#[path = "status_tests.rs"]
mod status_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
#[path = "triage_tests.rs"]
mod triage_tests;
#[cfg(test)]
#[path = "week_tests.rs"]
mod week_tests;

/// Asks the frame drawn again with the message opened or the draft sent (M7, M10), so the owner never waits for it. The
/// rows ask the same for a send (it reloads the list): the tests tell this ask apart by the pass it was made in.
fn draw_reader_again(ctx: &egui::Context) {
    #[cfg(test)]
    {
        let pass = ctx.cumulative_pass_nr();
        ctx.data_mut(|data| data.insert_temp(egui::Id::unique("reader-discard"), pass));
    }
    ctx.request_discard("a message opened or a draft sent shows in the frame of its key");
}
