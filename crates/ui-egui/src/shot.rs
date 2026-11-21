// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Headless pictures of the window for `ui_shot`: egui's shapes tessellated into triangles and filled on the CPU,
//! written as PNG. No window, no GPU and no network; the sample mailbox at a fixed time, so a picture only changes
//! when the look or the content does.
//!
//! A script is JSON lines, one picture per line:
//!
//! ```json
//! {"out": "docs/images/list-light.png", "mode": "light", "size": [1200, 760]}
//! {"out": "docs/images/list-dark.png", "mode": "dark", "narrow": 2, "open": {"role": "inbox"}, "search": "logo"}
//! ```
//!
//! `mode` is `light` or `dark`, `size` the window in points (1200 by 760 when left out), `scale` the pixels per
//! point (1 to 4, 1 when left out), `text_scale` the text scale in per cent (90 to 130 in steps of 10, 100 when left
//! out), `narrow` an account id, `open` a `role` or a `folder` id, `search` the text in
//! the search field and `select` the subject of the row to select. `onboard` shows the add-account page over the
//! sample: `empty`, `form` (typed by hand), `invalid` (a port and a CalDAV address typed wrong), `detected` (after
//! Detect), `checking` (the third step under way), `passed` or `failed` (the certificate refused); the check is scripted and nothing connects; it shows on the settings' Accounts
//! page. `settings` set to `true` shows the appearance settings page, `accounts` set to `true` the Accounts page,
//! `shortcuts` set to `true` the Shortcuts page, where `capture` names the action waiting for a key
//! (`ui.calendar.show`), `sending` set to `true` the Sending page, `triage` set to `true` the Triage page,
//! `notifications` set to `true` the Notifications page (Studio notifying of people only, and a desk that runs in the
//! background and starts with the computer, [`PicturedDesk`]). `command` opens the command line with that text
//! typed (`">"` for every command), and `calendar` set to `true` shows the calendar screen (with `first`, its empty
//! state), `calendar_view` shows it in a view (`day`, `week`, `month` or `agenda`) and `event` picks the event with
//! that summary, whose details show in the side panel; `edit_event` opens the event editor on a `new` event or, with
//! `edit`, on the picked one (where `event_editor` says: `small`, `full` or `window`, the last drawn full), and `delete_event` set to `true` deletes the picked event, so its toast shows. `compose`
//! opens the
//! composer: `new`, `written`, `bad` (a recipient that is no address), `suggest` (the people a half-typed name could
//! be), `floating` (a message opened over it), `folded` (the same, the floating panel folded), `reply` (a reply to Odile's rota, a line written over the quote), `reply-floating` (the same, a message opened over it) or
//! `template` (two people and a template with a blank); the first account then has an invented signature.
//! `first` set to `true` draws the window with no account yet instead of the sample, one that can add accounts.
//! `open` takes `{"role": "senders"}` for "New senders", and `view` shows the inbox's groups as `cards` (when left
//! out), a grouped `list` or the `simple` list. `showcase` set to `true` shows invented waiting senders and groups
//! instead of the sample's ([`sample::showcase`]) and gives their buttons keys; `step`
//! selects that many rows down by keyboard, as the arrow keys do, so the selected row shows the hover actions. `act`
//! does an act to the selected row (`read`, `unread`, `flag`, `unflag`, `done`, `archive` or `delete`; `accept` into
//! Updates, `accept_open`, `block`, or `newsletters` for its sender's group), so the toast shows; `slide` set to `true` catches a `done`, `archive` or `delete` row sliding out instead, four frames in with
//! motion on.
//! `icons` set to `true` draws every icon
//! in every style in the theme's colours instead of the window, `components` set to `true` the gallery of
//! the components ([`crate::component::gallery`]), `blocks` set to `true` the page of the shared rows and blocks
//! ([`crate::component::gallery_blocks`]), `fields` set to `true` the page of the fields, pickers and menus
//! ([`crate::component::gallery_fields`]), `screens` set to `true` the page of the screens' own sets
//! ([`crate::component::gallery_screens`]), `calendar_sets` set to `true` the page of the calendar's sets
//! ([`crate::component::gallery_calendar`]), `reader` set to `true` the page of the reader's sets
//! ([`crate::component::gallery_reader`]), `lists` set to `true` the page of the mail lists' rows
//! ([`crate::component::gallery_lists`]), `index` set to `true` the page of the index's rows
//! ([`crate::component::gallery_index`]), `character_field` set to `true` the page of the character field
//! ([`crate::component::gallery_field`]), where `pointer` may rest, `status_sets` set to `true` the page of the
//! status line's sets ([`crate::component::gallery_status`]),
//! `home_sets` set to `true` the page of Home's sets ([`crate::component::gallery_home`]), `home_place` set to `true`
//! shows Home, the start place, in the window (every other scene starts on the mail, as it was taken before Home was
//! the start place; synced, as its frames are, unless `sync` says otherwise), `fail` makes accounts fail after the
//! sync (`refused` the last one's sign-in, `everything` every account), `notice` says its text in the status line,
//! `command_sets` set to `true` the page of the command line's sets ([`crate::component::gallery_commands`]).
//! `sample` set to `frames` shows the design file's own sample mailbox ([`sample::frames`]) at its time, Monday
//! 16 November 2026, 14:04 UTC, with its mail put away and one message in the outbox (none with `drafts` `empty`);
//! every scene then writes, selects and sends with the frames' people (`invitations` and `put_away` are part of it and
//! refused there).
//! The typeface and the icons are fixed, so a script that names a
//! `font` or an `icon_set` is refused like any unknown key. Every window draws its frame as the native app does: the own frame, or `frame` names it (`own`, the older `strip` and `top_row` read as it, or `system`); `pointer` rests the pointer
//! at `[x, y]` points, so what lies under it shows its hover; `click` presses its first button at `[x, y]` points
//! before the picture, so what that opens (a menu, a list) is in it; `put_away` set to `true` snoozes, pins, parks and marks
//! done some of the sample's mail first ([`sample::put_away`]), `later` opens a pile of "Later" (`snoozed`, `pins`
//! or `parked`), `show_done` set to `true` lists the done mail in its old place, `menu` opens a menu (`more`, `move`,
//! `from`, `templates`, `group`), and `picker` opens the time picker
//! over the selected message with that text typed (`""` for none). `outbox` shows the sample with its first account
//! able to send and its sent message bounced ([`sample::sending`]): `waiting` the outbox with a send counting down its
//! hold, one scheduled and one the server refused, `changed` the outbox with a refused send changed since (only
//! Edit), `countdown` the toast of a message just sent, `schedule` the
//! composer with a time picked and its send-later picker open, `bounce` the bounced message open in the reader;
//! `drafts` opens the drafts place, `empty` or `filled` with two kept drafts;
//! the window takes them through the `ui.*.set` commands. Every picture but a `slide` one is taken with motion at
//! `off`, or at `reduced` where `motion` says so, after the movements are over, so it shows where each motion ends
//! (at off: the undo line without its bar, a sync as one print). `steps` then drive the set window as a person
//! would, one after another: a `key`, `tab` or `shift_tab` presses, `focus` or `press` a control by its accessible name ([`Step`]);
//! the picture is taken when the window rests after the last.

use std::collections::HashMap;
use std::io::Write;
use std::sync::Arc;

use egui::epaint::{ClippedPrimitive, ImageData, Mesh, Primitive, TextureId};
use egui::{Color32, Pos2, RawInput, Rect, Vec2, pos2, vec2};
use inboxcraft_engine::appearance::{EventEditorHost, WindowFrame};
use inboxcraft_model::{CheckStep, ErrorKind, Role, StepStatus};
use inboxcraft_time::{Clock, FakeClock, Timestamp};
use serde::Deserialize;

use crate::calendar::{EventRow, View};
use crate::component::gallery;
use crate::compose::Line;
use crate::mailbox::{Act, GroupKind, InboxView, Mailbox, Pile, Place};
use crate::onboard::{Checked, Keychain, Proposal, Proposed, Secret, Security, Source, Suggestion};
use crate::save::NoSave;
use crate::theme::{self, Mode, TextScale, Theme};
use crate::workers::Scripted;
use crate::{InboxApp, UiError, iconset, motion, sample};

/// The largest picture side in pixels.
pub const MAX_SIDE: usize = 4096;

/// The time the sample mailbox is shown at: 2026-11-05T09:00:00Z.
pub const SHOT_TIME: i64 = 1_793_869_200;

/// What a script line asks to open.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "lowercase")]
pub enum Open {
    /// A unified view by role name (`inbox`, `sent` ...), or `senders` for "New senders".
    Role(String),
    /// A folder by id.
    Folder(u64),
}

/// One picture of a script.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Shot {
    /// Where the PNG goes.
    pub out: String,
    /// `light` or `dark`.
    pub mode: String,
    /// The window in points.
    #[serde(default)]
    pub size: Option<[f32; 2]>,
    /// Pixels per point.
    #[serde(default)]
    pub scale: Option<f32>,
    /// The text scale in per cent (90 to 130 in steps of 10); 100 when left out.
    #[serde(default)]
    pub text_scale: Option<u16>,
    /// The account to narrow to.
    #[serde(default)]
    pub narrow: Option<u64>,
    /// The view or folder to open.
    #[serde(default)]
    pub open: Option<Open>,
    /// The search text.
    #[serde(default)]
    pub search: Option<String>,
    /// The subject of the row to select, which opens it in the reader.
    #[serde(default)]
    pub select: Option<String>,
    /// The remote pictures of the selected message: `once`, or `always` for its sender (drawn by the sample, never
    /// fetched).
    #[serde(default)]
    pub pictures: Option<String>,
    /// Whether the reader fills the window.
    #[serde(default)]
    pub full_width: Option<bool>,
    /// Whether the selected message shows its full header.
    #[serde(default)]
    pub details: Option<bool>,
    /// Whether the selected message shows its quotes.
    #[serde(default)]
    pub quotes: Option<bool>,
    /// The add-account page: `empty`, `form`, `invalid` (a port and a CalDAV address typed wrong), `detected`,
    /// `checking` (at its third step), `passed` or `failed`.
    #[serde(default)]
    pub onboard: Option<String>,
    /// Whether the appearance settings page is open.
    #[serde(default)]
    pub settings: Option<bool>,
    /// Whether the settings show Accounts.
    #[serde(default)]
    pub accounts: Option<bool>,
    /// With `accounts`: the account whose settings form is open.
    #[serde(default)]
    pub edit_account: Option<u64>,
    /// With `accounts`: the account removed before the picture, as the owner's press of the question removes it.
    #[serde(default)]
    pub remove_account: Option<u64>,
    /// Whether the settings show Shortcuts.
    #[serde(default)]
    pub shortcuts: Option<bool>,
    /// Whether the settings show Sending, the first account signed.
    #[serde(default)]
    pub sending: Option<bool>,
    /// Whether the settings show Triage.
    #[serde(default)]
    pub triage: Option<bool>,
    /// Whether the settings show Notifications.
    #[serde(default)]
    pub notifications: Option<bool>,
    /// The settings shown on the section of this name (`appearance` … `plugins`, [`crate::settings::Section::id`]), or on
    /// the page under one (`templates`; `scheduled`, over the sending sample; [`crate::settings::Under::id`]); `privacy`
    /// with the sample's senders whose pictures always load.
    #[serde(default)]
    pub section: Option<String>,
    /// How the unified inbox shows its groups: `smart` when left out, `grouped` or `list` (the earlier `cards` and
    /// `simple` read as `smart` and `list`).
    #[serde(default)]
    pub view: Option<String>,
    /// The composer, the first account signed: `new`, `written` (two people, a subject and a few lines), `bad`
    /// (written, with a recipient that is no address), `suggest` (one person, and a name half typed with the people it
    /// could be under it), `floating` (written, then a message opened), `folded` (the same, folded), `reply` (a reply with a line written),
    /// `template` (two people and a template whose blank is left) or `discarded` (written, then its Discard: the
    /// status line's "Draft discarded" with Undo).
    #[serde(default)]
    pub compose: Option<String>,
    /// The action waiting for a key on the Shortcuts page.
    #[serde(default)]
    pub capture: Option<String>,
    /// The text typed into the command line, which opens it.
    #[serde(default)]
    pub command: Option<String>,
    /// Whether the calendar screen shows.
    #[serde(default)]
    pub calendar: Option<bool>,
    /// Whether Home, the start place, shows in the window, synced unless `sync` says otherwise; the mail when left out.
    #[serde(default)]
    pub home_place: Option<bool>,
    /// The calendar's view (`day`, `week`, `month` or `agenda`), which shows the calendar screen.
    #[serde(default)]
    pub calendar_view: Option<String>,
    /// The summary of the calendar event picked, which shows the calendar screen and the event's details.
    #[serde(default)]
    pub event: Option<String>,
    /// The event editor on a `new` event or, with `edit`, on the picked one; shows the calendar screen.
    #[serde(default)]
    pub edit_event: Option<String>,
    /// Whether the picked event is deleted, so its toast shows.
    #[serde(default)]
    pub delete_event: Option<bool>,
    /// Whether the window has no account yet (and can add one) instead of the sample's.
    #[serde(default)]
    pub first: Option<bool>,
    /// Whether the sample's inbox holds Odile's two invitations to lunch (`sample::invitations`).
    #[serde(default)]
    pub invitations: Option<bool>,
    /// The sample the window shows: `present` (when left out, the one `--sample` opens) or `frames`, the design
    /// file's own ([`sample::frames`]).
    #[serde(default)]
    pub sample: Option<String>,
    /// Whether the sample's inbox holds a Hebrew and an Arabic message, newest (`sample::right_to_left`).
    #[serde(default)]
    pub right_to_left: Option<bool>,
    /// The invented screener, groups and hover actions in the list, with keys for their actions.
    #[serde(default)]
    pub showcase: Option<bool>,
    /// How many rows down the keyboard selects.
    #[serde(default)]
    pub step: Option<u8>,
    /// An act on the selected row: `read`, `unread`, `flag`, `unflag`, `done`, `archive` or `delete`; for its sender
    /// `accept` (into Updates), `accept_open`, `block` or `newsletters` (their group); the toast shows after it. `removed`
    /// takes the row away as another device would: no act of the owner's, so nothing walks on.
    #[serde(default)]
    pub act: Option<String>,
    /// How the sample's accounts sync, as their workers would tell it: `synced` (signed in, the last round two minutes
    /// ago), `syncing` (signed in, 27 of 50 folders done) or `offline`; the sample is offline without it (Home in the
    /// window and the frames' sample: synced).
    #[serde(default)]
    pub sync: Option<String>,
    /// Accounts failing, as their workers would tell it, after `sync`: `refused` the last account's sign-in refused,
    /// `everything` that and every other account's connection refused.
    #[serde(default)]
    pub fail: Option<String>,
    /// A notice said in the status line, as the window says one (a draft saved, a link copied).
    #[serde(default)]
    pub notice: Option<String>,
    /// Whether `act` (`done` or `delete`) is caught with its row sliding out, before the toast.
    #[serde(default)]
    pub slide: Option<bool>,
    /// The motion level chosen, `reduced` or `off` (`off` when left out); `full` is refused, the picture shows
    /// where each motion ends.
    #[serde(default)]
    pub motion: Option<String>,
    /// Every icon in every style instead of the window.
    #[serde(default)]
    pub icons: Option<bool>,
    /// The gallery of the components instead of the window.
    #[serde(default)]
    pub components: Option<bool>,
    /// The page of the fields, pickers and menus instead of the window.
    #[serde(default)]
    pub fields: Option<bool>,
    /// The page of the shared rows and blocks instead of the window.
    #[serde(default)]
    pub blocks: Option<bool>,
    /// The page of the screens' own sets instead of the window.
    #[serde(default)]
    pub screens: Option<bool>,
    /// The page of the calendar's sets (event block, mini month) instead of the window.
    #[serde(default)]
    pub calendar_sets: Option<bool>,
    /// The page of the reader's sets instead of the window.
    #[serde(default)]
    pub reader: Option<bool>,
    /// The page of the mail lists' rows instead of the window.
    #[serde(default)]
    pub lists: Option<bool>,
    /// The page of the index's rows instead of the window.
    #[serde(default)]
    pub index: Option<bool>,
    /// The page of the character field instead of the window; `pointer` may rest on it.
    #[serde(default)]
    pub character_field: Option<bool>,
    /// The page of the status line's sets (status item, status message) instead of the window.
    #[serde(default)]
    pub status_sets: Option<bool>,
    /// The page of Home's sets (count tile, block, block line) instead of the window.
    #[serde(default)]
    pub home_sets: Option<bool>,
    /// The page of the command line's sets (command field, key hint, window controls) instead of the window.
    #[serde(default)]
    pub command_sets: Option<bool>,
    /// Home, the page, in one of its states (`today`, `nothing-to-do`, `overflow-counts`, `first-run`,
    /// `everything-failing`; `@130` after it for 130 % text) in the body of a 1440 × 872 window, instead of the window;
    /// `drafts` and `drafts-empty` its drafts' block alone, at the body's top left.
    #[serde(default)]
    pub home: Option<String>,
    /// The window's frame: `own` (the older `strip` and `top_row` read as it) or `system`; left out, the mailbox's own
    /// setting, which is the own frame unless it was changed. The app draws the frame as a native window does.
    #[serde(default)]
    pub frame: Option<String>,
    /// Where the event editor opens: `small`, `full` (the default) or `window` (which the pictures, drawing no second
    /// window, show full).
    #[serde(default)]
    pub event_editor: Option<String>,
    /// Where the pointer rests, in points, so what lies under it shows its hover; nowhere when left out.
    #[serde(default)]
    pub pointer: Option<[f32; 2]>,
    /// A press of the pointer's first button at this place, in points, before the picture, so what it opens (a menu,
    /// a list) is in it; the pointer then rests there unless `pointer` puts it elsewhere.
    #[serde(default)]
    pub click: Option<[f32; 2]>,
    /// Some of the sample's mail snoozed, pinned, parked, done and back from a snooze first ([`sample::put_away`]).
    #[serde(default)]
    pub put_away: Option<bool>,
    /// The pile of "Later" to open: `snoozed`, `pins` or `parked`.
    #[serde(default)]
    pub later: Option<String>,
    /// Whether "Show done" lists the done mail in its old place.
    #[serde(default)]
    pub show_done: Option<bool>,
    /// A menu open: the reader's `more` or `move`, the composer's `from` or `templates`, the selected row's `group`.
    #[serde(default)]
    pub menu: Option<String>,
    /// The time picker open over the selected message, with this text typed (`""` for none).
    #[serde(default)]
    pub picker: Option<String>,
    /// The sending sample: `waiting` (the outbox), `changed` (a refused send changed since), `countdown` (a send's
    /// toast), `schedule` (the composer with a time picked) or `bounce` (the bounced message open).
    #[serde(default)]
    pub outbox: Option<String>,
    /// The drafts place: `empty`, or `filled` with two kept drafts.
    #[serde(default)]
    pub drafts: Option<String>,
    /// How the selected HTML message shows: `fitted` (the default) or `plain` (as a text-only program shows it).
    #[serde(default)]
    pub html_view: Option<String>,
    /// What a person does to the window once the scene is set, in order ([`Step`]); none when left out.
    #[serde(default)]
    pub steps: Option<Vec<Step>>,
}

/// The most steps one picture takes.
pub const MAX_STEPS: usize = 32;
/// The most presses of Tab (or Shift Tab) one step makes.
pub const MAX_TIMES: u32 = 64;
/// The longest accessible name a step aims at, in bytes.
pub const MAX_NAME: usize = 256;
/// The highest `nth` a step names.
pub const MAX_NTH: u32 = 64;
/// The longest script line, in bytes.
pub const MAX_LINE: usize = 16 * 1024;
/// The longest script, in bytes.
pub const MAX_SCRIPT: usize = 4 * 1024 * 1024;
/// The most pictures one script asks for.
pub const MAX_SHOTS: usize = 1024;
/// The most frames the window is given to come to rest after a step.
pub const MAX_SETTLE: usize = 240;
/// How often a press aims again at a control that moved when the pointer came over it.
pub const MAX_AIM: usize = 4;
/// The most frames one picture draws, its steps and their settling together.
pub const MAX_FRAMES: usize = 2000;

/// One thing a person does to the window, written as one key of an object:
///
/// - `{"key": "Command+K"}` presses a key with its modifiers (written as the keymap writes them,
///   [`crate::keymap::read`]) and lets go, through the window's input, so the keymap decides what it does;
/// - `{"tab": 3}` and `{"shift_tab": 1}` press Tab or Shift Tab that many times (1 to [`MAX_TIMES`]);
/// - `{"focus": "Search mail"}` gives the keyboard focus to the control with that accessible name, as a screen
///   reader asks for it;
/// - `{"press": "Snooze"}` brings the pointer over the control with that name once the frame has laid it out, and
///   presses and lets go of the first button there.
///
/// `focus` and `press` take `nth` (1 is the topmost, then leftmost) when more than one control has the name; a name
/// no control has, or one that several have without `nth`, is an error. After each step the window is drawn until it
/// rests, at most [`MAX_SETTLE`] frames. While steps drive the window its commands run for an agent, so only a `safe`
/// command runs and a refused one fails the picture.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    /// A key with its modifiers.
    #[serde(default)]
    pub key: Option<String>,
    /// How many times Tab is pressed.
    #[serde(default)]
    pub tab: Option<u32>,
    /// How many times Shift Tab is pressed.
    #[serde(default)]
    pub shift_tab: Option<u32>,
    /// The accessible name of the control that takes the focus.
    #[serde(default)]
    pub focus: Option<String>,
    /// The accessible name of the control pressed.
    #[serde(default)]
    pub press: Option<String>,
    /// Which of the controls with the name, from 1.
    #[serde(default)]
    pub nth: Option<u32>,
}

/// A [`Step`] read and checked.
#[derive(Debug, Clone, PartialEq)]
enum Do {
    Key(egui::KeyboardShortcut),
    Tab { back: bool, times: u32 },
    Focus { name: String, nth: Option<u32> },
    Press { name: String, nth: Option<u32> },
}

impl Step {
    /// What the step does, or why it is not a step.
    fn read(&self) -> Result<Do, String> {
        let named = |kind: &str, name: &str| {
            if name.is_empty() || name.len() > MAX_NAME {
                return Err(format!("{kind} needs a name of 1 to {MAX_NAME} bytes"));
            }
            match self.nth {
                Some(nth) if nth == 0 || nth > MAX_NTH => Err(format!("nth {nth} is not 1 to {MAX_NTH}")),
                nth => Ok((name.to_owned(), nth)),
            }
        };
        let times = |kind: &str, times: u32| {
            if times == 0 || times > MAX_TIMES { Err(format!("{kind} {times} is not 1 to {MAX_TIMES}")) } else { Ok(times) }
        };
        let kinds = [self.key.is_some(), self.tab.is_some(), self.shift_tab.is_some(), self.focus.is_some(), self.press.is_some()];
        if kinds.iter().filter(|kind| **kind).count() != 1 {
            return Err("a step is one of key, tab, shift_tab, focus or press".to_owned());
        }
        if self.nth.is_some() && self.focus.is_none() && self.press.is_none() {
            return Err("nth goes with focus or press only".to_owned());
        }
        if let Some(text) = &self.key {
            return crate::keymap::read(text).map(Do::Key).ok_or_else(|| format!("key {:?} is not a key", cut(text)));
        }
        if let Some(n) = self.tab {
            return Ok(Do::Tab { back: false, times: times("tab", n)? });
        }
        if let Some(n) = self.shift_tab {
            return Ok(Do::Tab { back: true, times: times("shift_tab", n)? });
        }
        if let Some(name) = &self.focus {
            let (name, nth) = named("focus", name)?;
            return Ok(Do::Focus { name, nth });
        }
        let (name, nth) = named("press", self.press.as_deref().unwrap_or_default())?;
        Ok(Do::Press { name, nth })
    }
}

/// At most the first 40 characters of `text`, for an error.
fn cut(text: &str) -> String {
    text.chars().take(40).collect()
}

/// The credential store of a picture: it takes the password and keeps nothing.
struct Pictured;

impl Keychain for Pictured {
    fn store(&self, _account: u64, _password: &str) -> Result<(), String> {
        Ok(())
    }

    fn forget(&self, _account: u64) -> Result<(), String> {
        Ok(())
    }
}

/// What Detect proposes in a picture: the invented provider's autoconfig file.
fn pictured_proposal() -> Proposal {
    let suggestion = |host: &str, port, security| Suggestion {
        host: host.to_owned(),
        port,
        security,
        username: "robin@example.org".to_owned(),
        source: Source::Autoconfig,
        checked: Checked::Answered,
    };
    Proposal {
        imap: vec![suggestion("imap.example.org", 993, Security::Tls), suggestion("imap.example.org", 143, Security::StartTls)],
        submission: vec![suggestion("smtp.example.org", 465, Security::Tls)],
        asked: vec!["dns MX example.org".to_owned(), "https://autoconfig.example.org/mail/config-v1.1.xml".to_owned()],
        failed: Vec::new(),
    }
}

/// Puts the sample window on the add-account page in `state`.
fn onboard(app: InboxApp, out: &str, state: &str) -> Result<InboxApp, UiError> {
    let passed = |step| (step, StepStatus::Passed);
    let workers = match state {
        "empty" | "form" | "invalid" | "detected" | "passed" => Scripted::passing(),
        "checking" => Scripted::answering(&[passed(CheckStep::Dns), passed(CheckStep::Tcp)]).unfinished(),
        "failed" => Scripted::answering(&[
            passed(CheckStep::Dns),
            passed(CheckStep::Tcp),
            passed(CheckStep::Tls),
            (CheckStep::Certificate, StepStatus::Failed(ErrorKind::CertificateExpired)),
        ]),
        other => return Err(bad(format!("{out}: onboard {other:?} is not empty, form, invalid, detected, checking, passed or failed"))),
    };
    let mut app = app.with_accounts(Arc::new(Pictured), Box::new(Proposed::new(pictured_proposal())), Box::new(workers));
    // The form is the Accounts page's: nothing shows that page for it.
    app.open_accounts();
    let (onboarding, mailbox) = app.onboarding_mut();
    onboarding.open();
    if state == "empty" {
        return Ok(app);
    }
    let form = onboarding.form_mut();
    form.address = "robin@example.org".to_owned();
    form.display_name = "Robin Example".to_owned();
    form.password = Secret::new("invented-pass");
    if state == "form" || state == "invalid" {
        form.host = "mail.example.org".to_owned();
        form.security = Some(Security::StartTls);
        if state == "invalid" {
            form.port = "99999".to_owned();
            form.caldav = "http://dav.example.org/robin/".to_owned();
        }
        return Ok(app);
    }
    onboarding.detect();
    onboarding.poll();
    if state != "detected" {
        onboarding.add(mailbox);
        app.pump();
    }
    Ok(app)
}

/// Does the hover action `shot` names to the selected row: at once, so the toast shows, or caught sliding out with
/// motion on (four frames a sixtieth of a second apart, so the row is most of the way out).
fn act(app: &mut InboxApp, shot: &Shot) -> Result<(), UiError> {
    let Some(name) = &shot.act else {
        return if shot.slide.is_some() { Err(bad(format!("{}: slide needs an act", shot.out))) } else { Ok(()) };
    };
    // Taken away by another device (a sync, the server): no act of the owner's, so nothing walks on.
    if name == "removed" {
        if shot.slide.is_some() {
            return Err(bad(format!("{}: only done, archive and delete slide out", shot.out)));
        }
        let Some(id) = app.mailbox().selected() else { return Err(bad(format!("{}: removed needs a selected row", shot.out))) };
        let (_, mailbox) = app.reader_mut();
        mailbox.run("mail.done", &serde_json::json!({ "id": id, "done": true }))?;
        mailbox.refresh();
        return Ok(());
    }
    let act = match name.as_str() {
        "read" => Act::Read,
        "unread" => Act::Unread,
        "flag" => Act::Flag,
        "unflag" => Act::Unflag,
        "done" => Act::Done,
        "archive" => Act::Archive,
        "delete" => Act::Delete,
        "accept" => Act::Accept { group: Some(GroupKind::Updates), open: false },
        "accept_open" => Act::Accept { group: None, open: true },
        "block" => Act::Block,
        "newsletters" => Act::Group(Some(GroupKind::Newsletters)),
        other => {
            return Err(bad(format!(
                "{}: act {other:?} is not read, unread, flag, unflag, done, archive, delete, accept, accept_open, block, newsletters or removed",
                shot.out
            )));
        }
    };
    let Some(id) = app.mailbox().selected() else { return Err(bad(format!("{}: an act needs a selected row", shot.out))) };
    if shot.slide == Some(true) {
        if !act.slides() {
            return Err(bad(format!("{}: only done, archive and delete slide out", shot.out)));
        }
        app.set_reduce_motion(false);
        app.mailbox_mut().start(id, act);
    } else {
        app.mailbox_mut().act(id, act);
    }
    Ok(())
}

/// With `shot.sync`, every account of `app` signed in, and synced two minutes ago or `syncing` 27 of 50 folders, as
/// their workers would tell it. Home in the window (`home_place`) and the frames' sample are synced unless the shot
/// names a sync (`offline` leaves them offline): the frames carry the synced status line.
fn sync(app: &mut InboxApp, shot: &Shot) -> Result<(), UiError> {
    use inboxcraft_model::{ConnectionState, Event, EventKind};
    let synced = (shot.home_place == Some(true) || frames(shot)).then_some("synced");
    let Some(state) = shot.sync.as_deref().or(synced).filter(|state| *state != "offline") else { return Ok(()) };
    let out = &shot.out;
    let ago = app.mailbox().now().checked_sub(std::time::Duration::from_secs(120)).ok_or_else(|| bad(format!("{out}: no time two minutes ago")))?;
    let told = match state {
        "synced" => EventKind::SyncFinished { at: ago },
        "syncing" => EventKind::SyncProgress { done: 27, of: 50 },
        other => return Err(bad(format!("{out}: sync {other:?} is not synced, syncing or offline"))),
    };
    let accounts: Vec<u64> = app.mailbox().accounts().iter().map(|account| account.id).collect();
    for account in accounts {
        let account = inboxcraft_model::AccountId::new(account);
        app.mailbox_mut().tell(&Event::new(account, None, EventKind::Connection(ConnectionState::Online)))?;
        app.mailbox_mut().tell(&Event::new(account, None, told.clone()))?;
    }
    Ok(())
}

/// With `shot.fail`, the sample's accounts failing as their workers would tell it, over what `sync` told: `refused`
/// the last account's sign-in refused, `everything` that and every other account's connection refused. Nothing
/// connects: the failures are told as events.
fn fail(app: &mut InboxApp, shot: &Shot) -> Result<(), UiError> {
    use inboxcraft_model::{AccountId, ErrorKind, Event, EventKind};
    let Some(state) = shot.fail.as_deref() else { return Ok(()) };
    let out = &shot.out;
    let accounts: Vec<u64> = app.mailbox().accounts().iter().map(|account| account.id).collect();
    let Some((last, rest)) = accounts.split_last() else { return Err(bad(format!("{out}: fail needs an account"))) };
    let others = match state {
        "refused" => &[][..],
        "everything" => rest,
        other => return Err(bad(format!("{out}: fail {other:?} is not refused or everything"))),
    };
    let failed = |kind| EventKind::Failed { kind, folder: None };
    for account in others {
        app.mailbox_mut().tell(&Event::new(AccountId::new(*account), None, failed(ErrorKind::ConnectionRefused)))?;
    }
    app.mailbox_mut().tell(&Event::new(AccountId::new(*last), None, failed(ErrorKind::LoginRefused)))?;
    Ok(())
}

/// With `shot.notice`, its text said in the status line as the window says a notice.
fn notice(app: &mut InboxApp, shot: &Shot) {
    if let Some(text) = &shot.notice {
        app.say(text);
    }
}

/// Puts the calendar screen in the view of `shot` and picks its event.
fn calendar(app: &mut InboxApp, shot: &Shot) -> Result<(), UiError> {
    let (calendar, mailbox) = app.calendar_mut();
    if let Some(name) = &shot.calendar_view {
        let view = View::named(name).ok_or_else(|| bad(format!("{}: calendar view {name:?} is not day, week, month or agenda", shot.out)))?;
        calendar.set_view(mailbox, view);
    }
    if let Some(summary) = &shot.event {
        let key = calendar.events().find(|event| event.summary == *summary).map(EventRow::key);
        let Some(key) = key else { return Err(bad(format!("{}: no calendar event {summary:?} in the view", shot.out))) };
        calendar.pick(Some(key));
    }
    let picked = |calendar: &crate::calendar::Calendar, what: &str| {
        calendar.picked().map(|_| ()).ok_or_else(|| bad(format!("{}: {what} needs an event picked with \"event\"", shot.out)))
    };
    match shot.edit_event.as_deref() {
        None => {}
        Some("new") => calendar.new_event(),
        Some("edit") => {
            picked(calendar, "edit_event")?;
            calendar.edit_picked();
        }
        Some(other) => return Err(bad(format!("{}: edit_event {other:?} is not new or edit", shot.out))),
    }
    if shot.delete_event == Some(true) {
        picked(calendar, "delete_event")?;
        calendar.delete_picked(mailbox);
    }
    Ok(())
}

/// Shows the places of `shot` beyond the mail: the calendar, the Shortcuts page with an action waiting for a key, the
/// command line with its text.
fn places(app: &mut InboxApp, clock: &FakeClock, shot: &Shot) -> Result<(), UiError> {
    if let Some(scene) = &shot.drafts {
        drafts(app, clock, shot, scene)?;
    }
    if let Some(scene) = &shot.compose {
        compose(app, &shot.out, scene, words(shot))?;
    }
    if shot.calendar == Some(true) || shot.calendar_view.is_some() || shot.event.is_some() || shot.edit_event.is_some() || shot.delete_event.is_some()
    {
        app.show_calendar();
        calendar(app, shot)?;
    }
    if shot.triage == Some(true) {
        app.settings_mut().open_at(crate::settings::Section::Triage);
    }
    if shot.notifications == Some(true) {
        app.mailbox_mut().set_notify_level(2, inboxcraft_engine::notify::Level::People);
        app.set_desk(Box::new(PicturedDesk));
        app.settings_mut().open_at(crate::settings::Section::Notifications);
    }
    if shot.sending == Some(true) {
        sign(app, words(shot));
        app.settings_mut().open_at(crate::settings::Section::Sending);
    }
    if shot.shortcuts == Some(true) {
        app.open_shortcuts();
    }
    if let Some(name) = &shot.section {
        if crate::settings::Under::named(name) == Some(crate::settings::Under::Templates) {
            // The page under Sending, with the sample's templates kept (the window's own saves, not an agent's).
            crate::sample::templates(app.mailbox_mut()).map_err(|error| bad(format!("{}: {error}", shot.out)))?;
            app.open_templates();
        } else if crate::settings::Under::named(name) == Some(crate::settings::Under::Scheduled) {
            // The page under Sending, over the sending sample with its snooze, reminder, send later and alert.
            let scheduled = if frames(shot) { crate::sample::frames::scheduled } else { crate::sample::scheduled };
            scheduled(app.mailbox_mut()).map_err(|error| bad(format!("{}: {error}", shot.out)))?;
            // The window reads the sends scheduled here as it reads them after a command of its own.
            app.mailbox_mut().refresh();
            app.open_scheduled();
        } else {
            if crate::settings::Section::named(name) == Some(crate::settings::Section::Privacy) {
                // The senders whose pictures always load (the window's own choices, not an agent's).
                crate::sample::always_load(app.mailbox_mut()).map_err(|error| bad(format!("{}: {error}", shot.out)))?;
            }
            let section = crate::settings::Section::named(name).ok_or_else(|| bad(format!("{}: no settings section {name:?}", shot.out)))?;
            app.settings_mut().open_at(section);
        }
    }
    if let Some(waiting) = &shot.capture {
        let Some(action) = crate::keymap::action(waiting).filter(|_| shot.shortcuts == Some(true)) else {
            return Err(bad(format!("{}: capture {waiting:?} is not an action on the Shortcuts page", shot.out)));
        };
        app.settings_mut().capture(action.id);
    }
    if let Some(text) = &shot.command {
        app.command_mut().open_with(text);
    }
    Ok(())
}

/// The desk of the pictures: InboxCraft runs in the background and starts with the computer, and a change changes
/// nothing.
#[derive(Debug, Clone, Copy, Default)]
pub struct PicturedDesk;

impl crate::desk::Desk for PicturedDesk {
    fn background(&self) -> Option<bool> {
        Some(true)
    }

    fn set_background(&mut self, _on: bool) -> Result<(), String> {
        Ok(())
    }

    fn login(&self) -> Option<bool> {
        Some(true)
    }

    fn asking(&self) -> bool {
        false
    }

    fn set_login(&mut self, _on: bool) {}

    fn answer(&mut self) -> Option<Result<(), String>> {
        None
    }
}

/// What a scene writes, selects and sends in a sample: the people, subjects and lines it types.
struct Words {
    /// The signature, and the accounts that sign with it.
    signature: &'static str,
    signed: &'static [u64],
    /// The recipients typed in a written message, half a name typed after them for the suggestions, and an address
    /// that is none.
    to: &'static str,
    suggest: &'static str,
    bad: &'static str,
    /// A written message's subject and its text above the signature.
    subject: &'static str,
    text: &'static str,
    /// The template scene's recipients, and the template: name, subject and text with a blank.
    template_to: &'static str,
    template: (&'static str, &'static str, &'static str),
    /// The subject replied to, the line written over the quote, and the subject a floating composer floats over.
    reply_to: &'static str,
    reply: &'static str,
    floating: &'static str,
    /// The send the server refuses, the reply it refuses it with, the one sent later (days from now and the hour),
    /// the one counting down its hold, and the Bcc an agent adds to the refused one.
    refused: (&'static str, &'static str),
    refusal: &'static str,
    later: (&'static str, &'static str, i64, i64),
    countdown: (&'static str, &'static str),
    bcc: &'static str,
    /// The kept drafts, the newer last: recipient, subject and how many minutes before the picture's time each was
    /// last edited.
    drafts: &'static [(&'static str, &'static str, u64)],
}

/// The present sample's words.
const PRESENT_WORDS: Words = Words {
    signature: "Mira\nPlot 14",
    signed: &[1],
    to: "Odile Brannock <odile@example.org>, Teodor Lisk <teodor@example.org>,",
    suggest: "Odile Brannock <odile@example.org>, te",
    bad: "ansel.example.org,",
    subject: "Work party on Saturday",
    text: "Hi both,\n\nWe meet at the plot at ten and finish by noon. Bring gloves; the shed has spades and the wheelbarrow.",
    template_to: "Odile Brannock <odile@example.org>, Teodor Lisk <teodor@example.org>,",
    template: (
        "Work party",
        "Work party on Saturday",
        "Hi both,\n\nWe meet at the plot at {?time} and finish by noon. Bring gloves; the shed has spades and the wheelbarrow.\n",
    ),
    reply_to: "Allotment rota for November",
    reply: "The second week works for me. I can bring the hose.",
    floating: "Draft logo for the bakery",
    refused: ("Hollis Fenwick <hollis@example.org>,", "Plot 14 handover"),
    refusal: REFUSAL,
    later: ("Petra Harbour <petra@example.org>,", "Re: Draft logo for the bakery", 1, 8),
    countdown: ("Odile Brannock <odile@example.org>, Teodor Lisk <teodor@example.org>,", "Work party on Saturday"),
    bcc: "Petra Harbour <petra@example.org>",
    drafts: &[
        ("Halvard Engen <halvard@example.org>", "Re: Kiln service: two dates to choose from", 22 * 60),
        ("Sunniva Aalto <sunniva@example.org>", "Price list for the winter workshops", 114),
    ],
};

/// The frames' words.
const FRAMES_WORDS: Words = Words {
    signature: "Odile",
    // The reply to Teodor goes from the Studio account.
    signed: &[1, 2],
    to: "Cosima Reuter <cosima@example.org>,",
    suggest: "Cosima Reuter <cosima@example.org>, an",
    bad: "anders.example.org,",
    subject: "Quote for the shop front",
    text: "Hi Cosima,\n\nthank you for asking. The shop front in painted lettering, both windows and the door, comes to 1,400 \
           euros with the paint.\n\nIf that suits you, send me the measurements of the fascia and I will draw it to scale.",
    template_to: "Marit Solheim <marit@example.org>, Pavel Hruska <pavel@example.org>,",
    template: (
        "Workshop invitation",
        "Thursday workshop",
        "Hello,\n\nthe next workshop is on Thursday in Room 3. We start at {?time} and end by six.\n",
    ),
    reply_to: "Draft logo for the bakery",
    reply: "Hi Teodor,\n\nthe second one. It reads from across the street and the wheat still says bakery. Could you try it \
            in two colours?",
    floating: "Draft logo for the bakery",
    refused: ("Odette Marchand <odette@example.org>,", "Re: Estimate for the window lettering"),
    refusal: "550 5.1.1 The address odette@example.org does not exist.",
    later: ("Tobias Wren <tobias@example.org>,", "Reminder: the studio is closed on the 24th", 4, 9),
    countdown: ("Dmitri Vale <dmitri@example.org>,", "Re: Kiln rota for December"),
    bcc: "Rosalind Thackeray <rosalind@example.org>",
    // With the message in the outbox, written yesterday ([`sample::frames::prepare`]): the frames' two drafts.
    drafts: &[("Sunniva Aalto <sunniva@example.org>", "Price list for the winter workshops", 114)],
};

/// Whether `shot` shows the frames' sample.
fn frames(shot: &Shot) -> bool {
    shot.sample.as_deref() == Some("frames")
}

/// The words of `shot`'s sample.
fn words(shot: &Shot) -> &'static Words {
    if frames(shot) { &FRAMES_WORDS } else { &PRESENT_WORDS }
}

/// Signs the messages of the accounts of `words` with its invented signature.
fn sign(app: &mut InboxApp, words: &Words) {
    for account in words.signed {
        // The sample takes every signature; a refusal would only leave the picture without one.
        let _ = app.mailbox_mut().run("compose.signature", &serde_json::json!({ "account": account, "signature": words.signature }));
    }
    app.mailbox_mut().refresh();
}

/// Opens the event editor where `shot`'s `event_editor` says, when it says.
fn event_editor(app: &mut InboxApp, shot: &Shot) -> Result<(), UiError> {
    if let Some(name) = &shot.event_editor {
        let host = EventEditorHost::named(name).ok_or_else(|| bad(format!("{}: event_editor {name} is not small, full or window", shot.out)))?;
        app.set_event_editor_host(host);
    }
    Ok(())
}

/// Opens the composer as `scene` asks.
fn compose(app: &mut InboxApp, out: &str, scene: &str, words: &Words) -> Result<(), UiError> {
    if !["new", "written", "bad", "suggest", "floating", "folded", "reply", "reply-floating", "template", "discarded"].contains(&scene) {
        return Err(bad(format!(
            "{out}: compose {scene:?} is not new, written, bad, suggest, floating, folded, reply, reply-floating, template or discarded"
        )));
    }
    sign(app, words);
    if scene == "reply" || scene == "reply-floating" {
        // Not selected first: a message selected before the first frame counts as pressed and floats the composer.
        let id = app.mailbox().messages().iter().find(|row| row.subject == words.reply_to).map(|row| row.id);
        if id.is_none_or(|id| !app.answer(id, crate::compose::Answer::Reply)) {
            return Err(bad(format!("{out}: the reply did not open")));
        }
        let (Some(composer), mailbox) = app.composer_mut() else { return Err(bad(format!("{out}: the composer did not open"))) };
        let text = format!("{}{}", words.reply, composer.text());
        *composer.text_mut() = text;
        composer.edited();
        composer.save(mailbox)?;
        if scene == "reply-floating" {
            // As `floating`: a message selected before the first frame floats the reply over it.
            let id = app.mailbox().messages().iter().find(|row| row.subject == words.floating).map(|row| row.id);
            app.mailbox_mut().select(id);
        }
        return Ok(());
    }
    app.compose();
    let (Some(composer), mailbox) = app.composer_mut() else { return Err(bad(format!("{out}: the composer did not open"))) };
    let mut to = |text: &str| -> Result<(), UiError> {
        if let Some(field) = composer.typing_mut(Line::To) {
            text.clone_into(field);
        }
        composer.typed(mailbox, Line::To)
    };
    match scene {
        "written" | "bad" | "floating" | "folded" | "discarded" => to(words.to)?,
        "template" => to(words.template_to)?,
        "suggest" => to(words.suggest)?,
        _ => {}
    }
    if scene == "bad" {
        to(words.bad)?;
    }
    if scene == "template" {
        let (name, subject, text) = words.template;
        let params = serde_json::json!({ "account": composer.account(), "name": name, "subject": subject, "text": text });
        mailbox.run("template.save", &params)?;
        // The menu of templates knows the one just saved.
        composer.load_templates(mailbox);
        return composer.use_template(mailbox, name);
    }
    if scene != "new" {
        words.subject.clone_into(composer.subject_mut());
        let text = format!("{}{}", words.text, composer.text());
        *composer.text_mut() = text;
        composer.edited();
        composer.save(mailbox)?;
    }
    if scene == "suggest" {
        composer.step(true);
    }
    if scene == "folded" {
        composer.toggle_fold();
    }
    if scene == "floating" || scene == "folded" {
        let id = app.mailbox().messages().iter().find(|row| row.subject == words.floating).map(|row| row.id);
        app.mailbox_mut().select(id);
    }
    if scene == "discarded" {
        // The composer's own Discard: closed, "Draft discarded" with Undo in the status line.
        app.discard_composer();
        if app.discarding().is_none() {
            return Err(bad(format!("{out}: the discard does not wait: {}", app.mailbox().error().unwrap_or("no reason"))));
        }
    }
    Ok(())
}

/// The reply the server refuses the outbox's picture send with.
const REFUSAL: &str = "550 5.1.1 hollis@example.org: no such mailbox here";

/// Writes a message to `to` with `subject` and a line of text in the composer, and sends it: at `later`, or after the
/// hold.
fn send(app: &mut InboxApp, out: &str, to: &str, subject: &str, later: Option<Timestamp>) -> Result<(), UiError> {
    app.compose();
    let (Some(composer), mailbox) = app.composer_mut() else { return Err(bad(format!("{out}: the composer did not open"))) };
    if let Some(field) = composer.typing_mut(Line::To) {
        to.clone_into(field);
    }
    composer.typed(mailbox, Line::To)?;
    subject.clone_into(composer.subject_mut());
    composer.edited();
    composer.set_later(later);
    // The scene is set without a frame: what the script wrote counts as seen.
    composer.mark_seen();
    app.send_composer();
    if app.composer().is_some() {
        return Err(bad(format!("{out}: {subject:?} was not sent: {}", app.mailbox().error().unwrap_or("no reason"))));
    }
    Ok(())
}

/// `hour`:00 in UTC `days` after the day of the picture's time on `clock`: tomorrow morning is `(1, 8)`.
fn morning(clock: &FakeClock, days: i64, hour: i64) -> Result<Timestamp, UiError> {
    let day = clock.now().unix_seconds().div_euclid(86_400).saturating_add(days);
    Timestamp::from_unix(day.saturating_mul(86_400).saturating_add(hour.saturating_mul(3_600)), 0).ok_or_else(|| bad("no time"))
}

/// Puts the sending sample in `scene`, on `clock`, with the people of `words`.
fn outbox(app: &mut InboxApp, clock: &FakeClock, out: &str, scene: &str, words: &Words) -> Result<(), UiError> {
    let (refused_to, refused) = words.refused;
    let (countdown_to, countdown) = words.countdown;
    match scene {
        "waiting" => {
            send(app, out, refused_to, refused, None)?;
            clock.advance(std::time::Duration::from_secs(6));
            app.mailbox_mut().pump(&mut Scripted::default().refusing(words.refusal));
            let (to, subject, days, hour) = words.later;
            send(app, out, to, subject, Some(morning(clock, days, hour)?))?;
            send(app, out, countdown_to, countdown, None)?;
            clock.advance(std::time::Duration::from_secs(1));
            app.mailbox_mut().open(Place::Outbox);
        }
        "changed" => {
            // A refused send whose Bcc an agent changed since: its row offers only Edit and says what changed.
            send(app, out, refused_to, refused, None)?;
            clock.advance(std::time::Duration::from_secs(6));
            app.mailbox_mut().pump(&mut Scripted::default().refusing(words.refusal));
            let draft = app.mailbox().outbox().iter().find(|row| row.subject == refused).map(|row| row.draft);
            let draft = draft.ok_or_else(|| bad(format!("{out}: nothing refused")))?;
            let change = serde_json::json!({ "id": draft, "bcc": [words.bcc] });
            // Changed as an agent changes it, through the control channel; the outbox is read again.
            app.mailbox_mut().run_for_agent("compose.update", &change).map_err(|error| bad(format!("{out}: {error}")))?;
            app.mailbox_mut().open(Place::Outbox);
        }
        "countdown" => {
            send(app, out, countdown_to, countdown, None)?;
            clock.advance(std::time::Duration::from_secs(1));
        }
        "schedule" => {
            compose(app, out, "written", words)?;
            let (Some(composer), _) = app.composer_mut() else { return Err(bad(format!("{out}: the composer did not open"))) };
            composer.set_later(Some(morning(clock, 1, 8)?));
            app.open_send_later();
        }
        "bounce" => {
            app.mailbox_mut().open(Place::View(Role::Sent));
            let id = app.mailbox().messages().iter().find(|row| row.bounced.is_some()).map(|row| row.id);
            let Some(id) = id else { return Err(bad(format!("{out}: no bounced message in Sent"))) };
            app.mailbox_mut().select(Some(id));
        }
        other => return Err(bad(format!("{out}: outbox {other:?} is not waiting, changed, countdown, schedule or bounce"))),
    }
    Ok(())
}

/// Opens the drafts place: `empty`, or `filled` with the two kept drafts of `words` ([`keep_drafts`]); the frames'
/// sample keeps them from the start ([`shoot`]), unless the place is to be empty.
fn drafts(app: &mut InboxApp, clock: &FakeClock, shot: &Shot, scene: &str) -> Result<(), UiError> {
    let out = &shot.out;
    match scene {
        "empty" => {}
        "filled" if frames(shot) => {}
        "filled" => keep_drafts(app, clock, out, words(shot).drafts)?,
        other => return Err(bad(format!("{out}: drafts {other:?} is not empty or filled"))),
    }
    app.mailbox_mut().open(Place::View(Role::Drafts));
    Ok(())
}

/// Keeps the drafts `kept` on the first account, each edited at its own time on `clock`, which then comes back to the
/// picture's time.
fn keep_drafts(app: &mut InboxApp, clock: &FakeClock, out: &str, kept: &[(&str, &str, u64)]) -> Result<(), UiError> {
    let now = clock.now();
    let mailbox = app.mailbox_mut();
    let account = mailbox.accounts().first().map(|account| account.id).ok_or_else(|| bad(format!("{out}: no account")))?;
    for (to, subject, minutes) in kept {
        let edited = now.checked_sub(std::time::Duration::from_secs(minutes.saturating_mul(60))).ok_or_else(|| bad(format!("{out}: no time")))?;
        clock.set(edited);
        let id = mailbox.run("compose.new", &serde_json::json!({ "account": account }))?.get("id").and_then(serde_json::Value::as_u64);
        let id = id.ok_or_else(|| bad(format!("{out}: no draft")))?;
        mailbox.run("compose.update", &serde_json::json!({ "id": id, "to": [to], "subject": subject }))?;
    }
    clock.set(now);
    Ok(())
}

/// Refuses the window settings for a page of `shot` that shows no window.
fn no_window(shot: &Shot, page: &str) -> Result<(), UiError> {
    let window = shot.narrow.is_some()
        || shot.click.is_some()
        || shot.open.is_some()
        || shot.search.is_some()
        || shot.select.is_some()
        || shot.onboard.is_some()
        || shot.settings.is_some()
        || shot.accounts.is_some()
        || shot.shortcuts.is_some()
        || shot.sending.is_some()
        || shot.triage.is_some()
        || shot.notifications.is_some()
        || shot.section.is_some()
        || shot.view.is_some()
        || shot.compose.is_some()
        || shot.capture.is_some()
        || shot.command.is_some()
        || shot.calendar.is_some()
        || shot.home_place.is_some()
        || shot.calendar_view.is_some()
        || shot.event.is_some()
        || shot.edit_event.is_some()
        || shot.delete_event.is_some()
        || shot.first.is_some()
        || shot.showcase.is_some()
        || shot.step.is_some()
        || shot.act.is_some()
        || shot.sync.is_some()
        || shot.fail.is_some()
        || shot.notice.is_some()
        || shot.slide.is_some()
        || shot.motion.is_some()
        || shot.text_scale.is_some()
        || shot.size.is_some()
        || shot.frame.is_some()
        || shot.put_away.is_some()
        || shot.later.is_some()
        || shot.show_done.is_some()
        || shot.picker.is_some()
        || shot.menu.is_some()
        || shot.outbox.is_some()
        || shot.drafts.is_some()
        || shot.steps.is_some()
        || [
            shot.icons,
            shot.components,
            shot.blocks,
            shot.fields,
            shot.screens,
            shot.calendar_sets,
            shot.reader,
            shot.lists,
            shot.index,
            shot.character_field,
            shot.status_sets,
            shot.home_sets,
            shot.command_sets,
            shot.home.as_ref().map(|_| true),
        ]
        .iter()
        .filter(|page| **page == Some(true))
        .count()
            > 1;
    if window {
        return Err(bad(format!("{}: {page} has no window to narrow, open, search, select, scale or size", shot.out)));
    }
    Ok(())
}

/// The gallery of the components of `shot` in `mode`.
fn component_gallery(shot: &Shot, mode: Mode) -> Result<Picture, UiError> {
    no_window(shot, "the component gallery")?;
    let theme = Theme::new(mode, TextScale::default());
    let keymap = gallery::keymap();
    let mut fonts = true;
    paint(gallery::size(), shot.scale.unwrap_or(1.0), mode, |ui| {
        if std::mem::take(&mut fonts) {
            theme::install_fonts(ui.ctx());
            // The app's own style, so the boards' text is drawn as the app draws it (coverage, hinting).
            Theme::apply(ui.ctx(), &theme::Look::default());
        }
        gallery::page(ui, &theme, &keymap);
    })
}

/// The page of the fields, pickers and menus of `shot` in `mode`.
fn fields_gallery(shot: &Shot, mode: Mode) -> Result<Picture, UiError> {
    no_window(shot, "the page of the fields, pickers and menus")?;
    let theme = Theme::new(mode, TextScale::default());
    let keymap = gallery::keymap();
    let mut fonts = true;
    paint(crate::component::gallery_fields::size(), shot.scale.unwrap_or(1.0), mode, |ui| {
        if std::mem::take(&mut fonts) {
            theme::install_fonts(ui.ctx());
            // The app's own style, so the boards' text is drawn as the app draws it (coverage, hinting).
            Theme::apply(ui.ctx(), &theme::Look::default());
        }
        crate::component::gallery_fields::page(ui, &theme, &keymap);
    })
}

/// The page of the screens' own sets of `shot` in `mode`.
fn screens_gallery(shot: &Shot, mode: Mode) -> Result<Picture, UiError> {
    no_window(shot, "the page of the screens' own sets")?;
    let theme = Theme::new(mode, TextScale::default());
    let keymap = gallery::keymap();
    let mut fonts = true;
    paint(crate::component::gallery_screens::size(), shot.scale.unwrap_or(1.0), mode, |ui| {
        if std::mem::take(&mut fonts) {
            theme::install_fonts(ui.ctx());
            Theme::apply(ui.ctx(), &theme::Look::default());
        }
        crate::component::gallery_screens::page(ui, &theme, &keymap);
    })
}

/// The page of the calendar's sets of `shot` in `mode`.
fn calendar_gallery(shot: &Shot, mode: Mode) -> Result<Picture, UiError> {
    no_window(shot, "the page of the calendar's sets")?;
    let theme = Theme::new(mode, TextScale::default());
    let keymap = gallery::keymap();
    let mut fonts = true;
    paint(crate::component::gallery_calendar::size(), shot.scale.unwrap_or(1.0), mode, |ui| {
        if std::mem::take(&mut fonts) {
            theme::install_fonts(ui.ctx());
            Theme::apply(ui.ctx(), &theme::Look::default());
        }
        crate::component::gallery_calendar::page(ui, &theme, &keymap);
    })
}

/// The page of the shared rows and blocks of `shot` in `mode`.
fn blocks_gallery(shot: &Shot, mode: Mode) -> Result<Picture, UiError> {
    no_window(shot, "the page of the shared rows and blocks")?;
    let theme = Theme::new(mode, TextScale::default());
    let keymap = gallery::keymap();
    let mut fonts = true;
    paint(crate::component::gallery_blocks::size(), shot.scale.unwrap_or(1.0), mode, |ui| {
        if std::mem::take(&mut fonts) {
            theme::install_fonts(ui.ctx());
            Theme::apply(ui.ctx(), &theme::Look::default());
        }
        crate::component::gallery_blocks::page(ui, &theme, &keymap);
    })
}

/// The page of the reader's sets of `shot` in `mode`.
fn reader_gallery(shot: &Shot, mode: Mode) -> Result<Picture, UiError> {
    no_window(shot, "the page of the reader's sets")?;
    let theme = Theme::new(mode, TextScale::default());
    let keymap = gallery::keymap();
    let mut fonts = true;
    paint(crate::component::gallery_reader::size(), shot.scale.unwrap_or(1.0), mode, |ui| {
        if std::mem::take(&mut fonts) {
            theme::install_fonts(ui.ctx());
            Theme::apply(ui.ctx(), &theme::Look::default());
        }
        crate::component::gallery_reader::page(ui, &theme, &keymap);
    })
}

/// The page of the mail lists' rows of `shot` in `mode`.
fn lists_gallery(shot: &Shot, mode: Mode) -> Result<Picture, UiError> {
    no_window(shot, "the page of the mail lists' rows")?;
    let theme = Theme::new(mode, TextScale::default());
    let keymap = gallery::keymap();
    let mut fonts = true;
    paint(crate::component::gallery_lists::size(), shot.scale.unwrap_or(1.0), mode, |ui| {
        if std::mem::take(&mut fonts) {
            theme::install_fonts(ui.ctx());
            Theme::apply(ui.ctx(), &theme::Look::default());
        }
        crate::component::gallery_lists::page(ui, &theme, &keymap);
    })
}

/// The page of the index's rows of `shot` in `mode`.
fn index_gallery(shot: &Shot, mode: Mode) -> Result<Picture, UiError> {
    no_window(shot, "the page of the index's rows")?;
    let theme = Theme::new(mode, TextScale::default());
    let mut fonts = true;
    paint(crate::component::gallery_index::size(), shot.scale.unwrap_or(1.0), mode, |ui| {
        if std::mem::take(&mut fonts) {
            theme::install_fonts(ui.ctx());
            Theme::apply(ui.ctx(), &theme::Look::default());
        }
        crate::component::gallery_index::page(ui, &theme);
    })
}

/// The page of the character field of `shot` in `mode`, the pointer resting where `pointer` says.
fn field_gallery(shot: &Shot, mode: Mode) -> Result<Picture, UiError> {
    no_window(shot, "the page of the character field")?;
    let size = crate::component::gallery_field::size();
    if let Some([x, y]) = shot.pointer
        && !((0.0..=size.x).contains(&x) && (0.0..=size.y).contains(&y))
    {
        return Err(bad(format!("{}: pointer [{x}, {y}] is not on the page", shot.out)));
    }
    let theme = Theme::new(mode, TextScale::default());
    let mut fonts = true;
    let pointer = Pointer { rest: shot.pointer.map(|[x, y]| pos2(x, y)), click: None };
    paint_pointed(size, shot.scale.unwrap_or(1.0), mode, pointer, |ui| {
        if std::mem::take(&mut fonts) {
            theme::install_fonts(ui.ctx());
            Theme::apply(ui.ctx(), &theme::Look::default());
        }
        crate::component::gallery_field::page(ui, &theme);
    })
}

/// The page of the status line's sets of `shot` in `mode`.
fn status_gallery(shot: &Shot, mode: Mode) -> Result<Picture, UiError> {
    no_window(shot, "the page of the status line's sets")?;
    let theme = Theme::new(mode, TextScale::default());
    let keymap = gallery::keymap();
    let mut fonts = true;
    paint(crate::component::gallery_status::size(), shot.scale.unwrap_or(1.0), mode, |ui| {
        if std::mem::take(&mut fonts) {
            theme::install_fonts(ui.ctx());
            Theme::apply(ui.ctx(), &theme::Look::default());
        }
        crate::component::gallery_status::page(ui, &theme, &keymap);
    })
}

/// The page of Home's sets of `shot` in `mode`.
fn home_gallery(shot: &Shot, mode: Mode) -> Result<Picture, UiError> {
    no_window(shot, "the page of Home's sets")?;
    let theme = Theme::new(mode, TextScale::default());
    let keymap = gallery::keymap();
    let mut fonts = true;
    paint(crate::component::gallery_home::size(), shot.scale.unwrap_or(1.0), mode, |ui| {
        if std::mem::take(&mut fonts) {
            theme::install_fonts(ui.ctx());
            Theme::apply(ui.ctx(), &theme::Look::default());
        }
        crate::component::gallery_home::page(ui, &theme, &keymap);
    })
}

/// Home, the page, of `shot` in `mode` in `state` (with `@130`: at 130 % text), in the body of a 1440 × 872 window.
fn home_page(shot: &Shot, mode: Mode, state: &str) -> Result<Picture, UiError> {
    no_window(shot, "Home")?;
    let (state, scale) = state.split_once('@').map_or((state, 100), |(state, scale)| (state, scale.parse().unwrap_or(0)));
    if crate::home::sample(state).is_none() {
        return Err(bad(format!("{}: Home has no state {state:?}", shot.out)));
    }
    let scale = TextScale::from_percent(scale).ok_or_else(|| bad(format!("{}: Home has no text scale {scale}", shot.out)))?;
    let theme = Theme::new(mode, scale);
    let keymap = gallery::keymap();
    let mut field = crate::field::Field::new(0);
    let size = egui::vec2(1205.0, 826.0);
    let mut fonts = true;
    paint(size, shot.scale.unwrap_or(1.0), mode, |ui| {
        if std::mem::take(&mut fonts) {
            theme::install_fonts(ui.ctx());
            Theme::apply(ui.ctx(), &theme::Look::default());
        }
        crate::draw::home::gallery(ui, &theme, &keymap, state, &mut field, size);
    })
}

/// The page of the command line's sets of `shot` in `mode`.
fn commands_gallery(shot: &Shot, mode: Mode) -> Result<Picture, UiError> {
    no_window(shot, "the page of the command line's sets")?;
    let theme = Theme::new(mode, TextScale::default());
    let keymap = gallery::keymap();
    let mut fonts = true;
    paint(crate::component::gallery_commands::size(), shot.scale.unwrap_or(1.0), mode, |ui| {
        if std::mem::take(&mut fonts) {
            theme::install_fonts(ui.ctx());
            Theme::apply(ui.ctx(), &theme::Look::default());
        }
        crate::component::gallery_commands::page(ui, &theme, &keymap);
    })
}

/// The icon sheet of `shot` in `mode`; it shows no window, so the window settings are refused.
fn icon_sheet(shot: &Shot, mode: Mode) -> Result<Picture, UiError> {
    no_window(shot, "an icon sheet")?;
    let theme = Theme::new(mode, TextScale::default());
    let mut fonts = true;
    paint(iconset::sheet_size(), shot.scale.unwrap_or(1.0), mode, |ui| {
        if std::mem::take(&mut fonts) {
            theme::install_fonts(ui.ctx());
            Theme::apply(ui.ctx(), &theme::Look::default());
        }
        iconset::sheet(ui.painter(), &theme);
    })
}

fn bad(detail: impl Into<String>) -> UiError {
    UiError::Shot(detail.into())
}

/// The pictures a script asks for. Blank lines and lines starting with `#` are skipped.
///
/// # Errors
///
/// [`UiError::Shot`] naming the first line that is not a picture (also a line longer than [`MAX_LINE`], more than
/// [`MAX_STEPS`] steps, or a step [`Step`] does not take), for a script longer than [`MAX_SCRIPT`], or for more than
/// [`MAX_SHOTS`] pictures.
pub fn parse_script(text: &str) -> Result<Vec<Shot>, UiError> {
    if text.len() > MAX_SCRIPT {
        return Err(bad(format!("the script is longer than {MAX_SCRIPT} bytes")));
    }
    let mut shots = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let number = index.saturating_add(1);
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.len() > MAX_LINE {
            return Err(bad(format!("line {number}: longer than {MAX_LINE} bytes")));
        }
        if shots.len() >= MAX_SHOTS {
            return Err(bad(format!("line {number}: more than {MAX_SHOTS} pictures")));
        }
        let shot: Shot = serde_json::from_str(line).map_err(|error| bad(format!("line {number}: {error}")))?;
        let steps = shot.steps.as_deref().unwrap_or_default();
        if steps.len() > MAX_STEPS {
            return Err(bad(format!("line {number}: {} steps, more than {MAX_STEPS}", steps.len())));
        }
        for (at, step) in steps.iter().enumerate() {
            step.read().map_err(|error| bad(format!("line {number}: step {}: {error}", at.saturating_add(1))))?;
        }
        shots.push(shot);
    }
    Ok(shots)
}

/// An RGBA picture, eight bits a channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    width: usize,
    height: usize,
    rgba: Vec<u8>,
}

impl Picture {
    /// Its width in pixels.
    #[must_use]
    pub fn width(&self) -> usize {
        self.width
    }

    /// Its height in pixels.
    #[must_use]
    pub fn height(&self) -> usize {
        self.height
    }

    /// The colour at `x`, `y`, or `None` outside.
    #[must_use]
    pub fn pixel(&self, x: usize, y: usize) -> Option<Color32> {
        if x >= self.width {
            return None;
        }
        let at = y.checked_mul(self.width)?.checked_add(x)?.checked_mul(4)?;
        let px = self.rgba.get(at..at.checked_add(4)?)?;
        Some(Color32::from_rgba_premultiplied(*px.first()?, *px.get(1)?, *px.get(2)?, *px.get(3)?))
    }

    /// Writes it as PNG.
    ///
    /// # Errors
    ///
    /// [`UiError::Shot`] when the writer fails.
    pub fn write_png(&self, out: &mut dyn Write) -> Result<(), UiError> {
        let width = u32::try_from(self.width).map_err(|_| bad("too wide"))?;
        let height = u32::try_from(self.height).map_err(|_| bad("too high"))?;
        let mut encoder = png::Encoder::new(out, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|error| bad(error.to_string()))?;
        writer.write_image_data(&self.rgba).map_err(|error| bad(error.to_string()))?;
        writer.finish().map_err(|error| bad(error.to_string()))
    }
}

/// A texture egui asked for: premultiplied colours.
struct Texture {
    width: usize,
    height: usize,
    pixels: Vec<Color32>,
}

impl Texture {
    fn texel(&self, x: usize, y: usize) -> [f32; 4] {
        let x = x.min(self.width.saturating_sub(1));
        let y = y.min(self.height.saturating_sub(1));
        let colour = y.checked_mul(self.width).and_then(|row| row.checked_add(x)).and_then(|at| self.pixels.get(at)).copied().unwrap_or_default();
        unit(colour)
    }

    /// The colour at `uv` (0 to 1 across), filtered between the four nearest texels.
    fn sample(&self, uv: Pos2) -> [f32; 4] {
        let x = uv.x * self.width as f32 - 0.5;
        let y = uv.y * self.height as f32 - 0.5;
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let index = |value: f32| if value <= 0.0 { 0 } else { value as usize };
        let (ix, iy) = (index(x0), index(y0));
        let (jx, jy) = (if x0 < 0.0 { 0 } else { ix.saturating_add(1) }, if y0 < 0.0 { 0 } else { iy.saturating_add(1) });
        let (a, b, c, d) = (self.texel(ix, iy), self.texel(jx, iy), self.texel(ix, jy), self.texel(jx, jy));
        std::array::from_fn(|i| {
            let top = a[i] + (b[i] - a[i]) * fx;
            let bottom = c[i] + (d[i] - c[i]) * fx;
            top + (bottom - top) * fy
        })
    }
}

fn unit(colour: Color32) -> [f32; 4] {
    colour.to_array().map(|channel| f32::from(channel) / 255.0)
}

/// The canvas the triangles are filled into: premultiplied colours, 0 to 1.
struct Canvas {
    width: usize,
    height: usize,
    pixels: Vec<[f32; 4]>,
}

/// Twice the signed area of the triangle `a`, `b`, `p`: positive when `p` is on the inner side of `a` to `b` in a
/// clockwise triangle on a screen whose y grows downwards.
fn edge(a: Pos2, b: Pos2, p: Pos2) -> f32 {
    (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x)
}

/// Whether the edge `a` to `b` of a clockwise triangle is a top or a left edge, which owns the pixels exactly on it,
/// so two triangles that share an edge never both fill a pixel.
fn top_left(a: Pos2, b: Pos2) -> bool {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    dy < 0.0 || (dy == 0.0 && dx > 0.0)
}

fn covers(weight: f32, a: Pos2, b: Pos2) -> bool {
    weight > 0.0 || (weight == 0.0 && top_left(a, b))
}

impl Canvas {
    fn new(width: usize, height: usize) -> Self {
        Self { width, height, pixels: vec![[0.0; 4]; width.saturating_mul(height)] }
    }

    fn blend(&mut self, x: usize, y: usize, src: [f32; 4]) {
        if let Some(dst) = y.checked_mul(self.width).and_then(|row| row.checked_add(x)).and_then(|at| self.pixels.get_mut(at)) {
            let keep = 1.0 - src[3];
            *dst = std::array::from_fn(|i| src[i] + dst[i] * keep);
        }
    }

    /// Fills `mesh` (positions in points) inside `clip` (in pixels), scaled by `scale`.
    fn mesh(&mut self, mesh: &Mesh, texture: &Texture, clip: [usize; 4], scale: f32) {
        let (triangles, _) = mesh.indices.as_chunks::<3>();
        for [a, b, c] in triangles {
            let vertex = |at: &u32| usize::try_from(*at).ok().and_then(|i| mesh.vertices.get(i));
            if let (Some(a), Some(b), Some(c)) = (vertex(a), vertex(b), vertex(c)) {
                self.triangle([a, b, c], texture, clip, scale);
            }
        }
    }

    fn triangle(&mut self, vertices: [&egui::epaint::Vertex; 3], texture: &Texture, clip: [usize; 4], scale: f32) {
        let [a, mut b, mut c] = vertices;
        let mut area = edge(a.pos * scale, b.pos * scale, c.pos * scale);
        if area < 0.0 {
            std::mem::swap(&mut b, &mut c);
            area = -area;
        }
        if area <= 0.0 || !area.is_finite() {
            return;
        }
        let (pa, pb, pc) = (a.pos * scale, b.pos * scale, c.pos * scale);
        let [left, top, right, bottom] = clip;
        let first = |value: f32, low: usize| if value.is_finite() && value > low as f32 { value.floor() as usize } else { low };
        let last = |value: f32, high: usize| if value.is_finite() && value < high as f32 { value.ceil() as usize } else { high };
        let (x0, x1) = (first(pa.x.min(pb.x).min(pc.x), left), last(pa.x.max(pb.x).max(pc.x), right));
        let (y0, y1) = (first(pa.y.min(pb.y).min(pc.y), top), last(pa.y.max(pb.y).max(pc.y), bottom));
        let (ca, cb, cc) = (unit(a.color), unit(b.color), unit(c.color));
        for y in y0..y1 {
            for x in x0..x1 {
                let p = pos2(x as f32 + 0.5, y as f32 + 0.5);
                let (wa, wb, wc) = (edge(pb, pc, p), edge(pc, pa, p), edge(pa, pb, p));
                if !(covers(wa, pb, pc) && covers(wb, pc, pa) && covers(wc, pa, pb)) {
                    continue;
                }
                let (wa, wb, wc) = (wa / area, wb / area, wc / area);
                let uv = pos2(a.uv.x * wa + b.uv.x * wb + c.uv.x * wc, a.uv.y * wa + b.uv.y * wb + c.uv.y * wc);
                let texel = texture.sample(uv);
                let src = std::array::from_fn(|i| (ca[i] * wa + cb[i] * wb + cc[i] * wc) * texel[i]);
                self.blend(x, y, src);
            }
        }
    }

    fn picture(&self) -> Picture {
        let rgba = self.pixels.iter().flat_map(|pixel| pixel.map(|channel| (channel.clamp(0.0, 1.0) * 255.0).round() as u8)).collect();
        Picture { width: self.width, height: self.height, rgba }
    }
}

/// Fills `primitives` into a picture of `width` by `height` pixels with the `textures` egui set.
fn fill(primitives: &[ClippedPrimitive], textures: &HashMap<TextureId, Texture>, width: usize, height: usize, scale: f32) -> Picture {
    let mut canvas = Canvas::new(width, height);
    for primitive in primitives {
        let Primitive::Mesh(mesh) = &primitive.primitive else { continue };
        let Some(texture) = textures.get(&mesh.texture_id) else { continue };
        let rect = primitive.clip_rect;
        // Infinite edges (an unclipped primitive) stop at the picture's edge; the casts see 0 to `high` only.
        let px = |value: f32, high: usize| if value > 0.0 { (value * scale).round().min(high as f32) as usize } else { 0 };
        let clip = [px(rect.min.x, width), px(rect.min.y, height), px(rect.max.x, width), px(rect.max.y, height)];
        canvas.mesh(mesh, texture, clip, scale);
    }
    canvas.picture()
}

/// Applies and empties `delta` (egui insists every delta is handled).
fn apply_textures(textures: &mut HashMap<TextureId, Texture>, delta: &mut egui::TexturesDelta) {
    for (id, patches) in delta.set.drain() {
        for patch in patches {
            let ImageData::Color(image) = &patch.image;
            let [width, height] = image.size;
            match patch.pos {
                None => {
                    textures.insert(id, Texture { width, height, pixels: image.pixels.clone() });
                }
                Some([x, y]) => {
                    let Some(texture) = textures.get_mut(&id) else { continue };
                    for row in 0..height {
                        for column in 0..width {
                            let source = row.checked_mul(width).and_then(|r| r.checked_add(column)).and_then(|at| image.pixels.get(at));
                            let target = y
                                .checked_add(row)
                                .filter(|ty| *ty < texture.height)
                                .zip(x.checked_add(column).filter(|tx| *tx < texture.width))
                                .and_then(|(ty, tx)| ty.checked_mul(texture.width).and_then(|r| r.checked_add(tx)));
                            if let (Some(source), Some(target)) = (source, target.and_then(|at| texture.pixels.get_mut(at))) {
                                *target = *source;
                            }
                        }
                    }
                }
            }
        }
    }
    for id in delta.free.drain() {
        textures.remove(&id);
    }
}

/// Draws `app` for a few frames in a window of `size` points at `scale` pixels per point in `mode`, and fills the
/// last frame into a picture.
///
/// # Errors
///
/// [`UiError::Shot`] for a size of nothing, a scale outside 1 to 4, or a picture side over [`MAX_SIDE`].
pub fn render(app: &mut InboxApp, size: Vec2, scale: f32, mode: Mode) -> Result<Picture, UiError> {
    paint(size, scale, mode, |ui| app.ui(ui))
}

/// [`render`] with the pointer resting at `pointer`, so what lies under it shows its hover.
///
/// # Errors
///
/// The errors of [`render`].
pub fn render_pointed(app: &mut InboxApp, size: Vec2, scale: f32, mode: Mode, pointer: Option<Pos2>) -> Result<Picture, UiError> {
    paint_pointed(size, scale, mode, Pointer { rest: pointer, click: None }, |ui| app.ui(ui))
}

/// [`render_pointed`] after a press of the pointer's first button at `click`, so what it opens is in the picture.
///
/// # Errors
///
/// The errors of [`render`].
pub fn render_clicked(app: &mut InboxApp, size: Vec2, scale: f32, mode: Mode, pointer: Option<Pos2>, click: Pos2) -> Result<Picture, UiError> {
    paint_pointed(size, scale, mode, Pointer { rest: pointer, click: Some(click) }, |ui| app.ui(ui))
}

/// The sample window after `pointer` and then `steps`, each followed by frames until the window rests. The window's
/// commands run for an agent meanwhile, so a step that reaches a command which is not `safe` fails the picture.
fn render_steps(app: &mut InboxApp, size: Vec2, scale: f32, mode: Mode, pointer: Pointer, steps: &[Do]) -> Result<Picture, String> {
    let mut run = Run::new(size, scale, mode).map_err(|error| error.to_string())?;
    run.ctx.enable_accesskit();
    app.mailbox_mut().set_caller(inboxcraft_engine::Caller::Agent);
    let mut draw = |ui: &mut egui::Ui| app.ui(ui);
    for frame in 0..pointer.frames() {
        run.frame(pointer.events(frame), &mut draw)?;
    }
    run.settle(&mut draw)?;
    let mut at = pointer.rest.or(pointer.click);
    for (index, step) in steps.iter().enumerate() {
        run.step(step, &mut at, &mut draw).map_err(|error| format!("step {}: {error}", index.saturating_add(1)))?;
    }
    if let Some(command) = app.mailbox().refused() {
        return Err(format!("the steps reached {command}, which only the owner runs; a step runs safe commands only"));
    }
    run.picture().map_err(|error| error.to_string())
}

/// What the pointer does while a picture is drawn: where it rests, and where it presses first.
#[derive(Debug, Clone, Copy, Default)]
struct Pointer {
    rest: Option<Pos2>,
    click: Option<Pos2>,
}

impl Pointer {
    /// How many frames are drawn: four, and five more for a press so what it opens has settled.
    fn frames(self) -> usize {
        if self.click.is_some() { 9 } else { 4 }
    }

    /// The input of frame `frame`: for a press the pointer comes to its place in the first (so the widget under it is
    /// known), presses in the second, lets go in the third, and rests after.
    fn events(self, frame: usize) -> Vec<egui::Event> {
        let button = |pos: Pos2, pressed: bool| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        match (self.click, frame) {
            (Some(at), 0) => vec![egui::Event::PointerMoved(at)],
            (Some(at), 1) => vec![button(at, true)],
            (Some(at), 2) => vec![button(at, false)],
            (Some(at), _) => vec![egui::Event::PointerMoved(self.rest.unwrap_or(at))],
            (None, _) => self.rest.map(egui::Event::PointerMoved).into_iter().collect(),
        }
    }
}

/// Runs `draw` for a few frames on a transparent window of `size` points at `scale` pixels per point in `mode`, and
/// fills the last frame into a picture.
///
/// # Errors
///
/// [`UiError::Shot`] for a size of nothing, a scale outside 1 to 4, or a picture side over [`MAX_SIDE`].
pub(crate) fn paint(size: Vec2, scale: f32, mode: Mode, draw: impl FnMut(&mut egui::Ui)) -> Result<Picture, UiError> {
    paint_pointed(size, scale, mode, Pointer::default(), draw)
}

/// [`paint`] with the pointer resting at `pointer`.
fn paint_pointed(size: Vec2, scale: f32, mode: Mode, pointer: Pointer, mut draw: impl FnMut(&mut egui::Ui)) -> Result<Picture, UiError> {
    let mut run = Run::new(size, scale, mode)?;
    for frame in 0..pointer.frames() {
        run.frame(pointer.events(frame), &mut draw).map_err(bad)?;
    }
    run.picture()
}

/// The frames of one picture: egui's context, the textures it set, and the last frame's output.
struct Run {
    ctx: egui::Context,
    size: Vec2,
    width: usize,
    height: usize,
    textures: HashMap<TextureId, Texture>,
    last: Option<egui::FullOutput>,
    /// The frames drawn so far, at most [`MAX_FRAMES`].
    frames: usize,
}

impl Run {
    /// A window of `size` points at `scale` pixels per point in `mode`, no frame drawn yet.
    fn new(size: Vec2, scale: f32, mode: Mode) -> Result<Self, UiError> {
        if !(1.0..=4.0).contains(&scale) {
            return Err(bad(format!("scale {scale} is not between 1 and 4")));
        }
        let side = |points: f32| {
            let pixels = (points * scale).round();
            if pixels.is_finite() && pixels >= 1.0 && pixels <= MAX_SIDE as f32 { Some(pixels as usize) } else { None }
        };
        let (Some(width), Some(height)) = (side(size.x), side(size.y)) else {
            return Err(bad(format!("a window of {} by {} points at scale {scale} is not 1 to {MAX_SIDE} pixels a side", size.x, size.y)));
        };
        let ctx = egui::Context::default();
        ctx.set_pixels_per_point(scale);
        ctx.set_theme(match mode {
            Mode::Light => egui::Theme::Light,
            Mode::Dark => egui::Theme::Dark,
        });
        Ok(Self { ctx, size, width, height, textures: HashMap::new(), last: None, frames: 0 })
    }

    /// Draws one frame with `events`, or says that the picture has drawn its [`MAX_FRAMES`].
    fn frame(&mut self, events: Vec<egui::Event>, draw: &mut impl FnMut(&mut egui::Ui)) -> Result<(), String> {
        if self.frames >= MAX_FRAMES {
            return Err(format!("the picture has drawn its {MAX_FRAMES} frames"));
        }
        self.frames = self.frames.saturating_add(1);
        let input = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.size)), events, ..RawInput::default() };
        let mut output = self.ctx.run_ui(input, |ui| {
            motion::set_reduced(ui.ctx(), true);
            draw(ui);
        });
        apply_textures(&mut self.textures, &mut output.textures_delta);
        self.last = Some(output);
        Ok(())
    }

    /// Draws frames without input until egui asks for no frame at once (nothing moves any more), at least one and at
    /// most [`MAX_SETTLE`].
    fn settle(&mut self, draw: &mut impl FnMut(&mut egui::Ui)) -> Result<(), String> {
        for _ in 0..MAX_SETTLE {
            self.frame(Vec::new(), draw)?;
            let delay =
                self.last.as_ref().and_then(|output| output.viewport_output.get(&egui::ViewportId::ROOT)).map(|viewport| viewport.repaint_delay);
            if delay != Some(std::time::Duration::ZERO) {
                return Ok(());
            }
        }
        Err(format!("the window did not come to rest in {MAX_SETTLE} frames"))
    }

    /// The last frame's AccessKit tree.
    fn tree(&self) -> Option<&egui::accesskit::TreeUpdate> {
        self.last.as_ref().and_then(|output| output.platform_output.accesskit_update.as_ref())
    }

    /// The control named `name` in the last frame that `fits`: the `nth` of them, topmost then leftmost, or the only
    /// one.
    fn find(
        &self,
        name: &str,
        nth: Option<u32>,
        what: &str,
        fits: impl Fn(&egui::accesskit::Node) -> bool,
    ) -> Result<(egui::accesskit::NodeId, Rect), String> {
        let tree = self.tree().ok_or("the window gave no AccessKit tree")?;
        let mut found: Vec<(egui::accesskit::NodeId, Rect)> = tree
            .nodes
            .iter()
            .filter(|(_, node)| node.label() == Some(name) && fits(node))
            .filter_map(|(id, node)| {
                let bounds = node.bounds()?;
                Some((*id, Rect::from_min_max(pos2(bounds.x0 as f32, bounds.y0 as f32), pos2(bounds.x1 as f32, bounds.y1 as f32))))
            })
            .collect();
        found.sort_by(|(a_id, a), (b_id, b)| a.min.y.total_cmp(&b.min.y).then(a.min.x.total_cmp(&b.min.x)).then(a_id.0.cmp(&b_id.0)));
        match (nth, found.len()) {
            (_, 0) => {
                // The names that hold it, so a script's author sees what the window calls its controls.
                let near: Vec<String> = tree
                    .nodes
                    .iter()
                    .filter_map(|(_, node)| node.label())
                    .filter(|label| label.contains(name))
                    .take(3)
                    .map(|label| format!("{:?}", label.chars().take(120).collect::<String>()))
                    .collect();
                let near = if near.is_empty() { String::new() } else { format!(" (named so: {})", near.join(", ")) };
                Err(format!("no {what} is named {:?}{near}", cut(name)))
            }
            (None, 1) => found.first().copied().ok_or_else(|| "no control".to_owned()),
            (None, many) => Err(format!("{many} controls are named {:?}; nth says which", cut(name))),
            (Some(nth), many) => usize::try_from(nth)
                .ok()
                .and_then(|nth| found.get(nth.saturating_sub(1)))
                .copied()
                .ok_or_else(|| format!("nth {nth}, but only {many} controls are named {:?}", cut(name))),
        }
    }

    /// Does `step`, then lets the window come to rest. `pointer` is where the pointer rests, and moves with a press.
    fn step(&mut self, step: &Do, pointer: &mut Option<Pos2>, draw: &mut impl FnMut(&mut egui::Ui)) -> Result<(), String> {
        use egui::accesskit::{Action, ActionRequest, TreeId};
        // The modifiers go down before the key and stay down for the frame; they are let go of in the next.
        let key = |key: egui::Key, modifiers: egui::Modifiers| {
            let event = |pressed| egui::Event::Key { key, physical_key: None, pressed, repeat: false, modifiers };
            vec![egui::Event::ModifiersChanged(modifiers), event(true), event(false)]
        };
        let up = vec![egui::Event::ModifiersChanged(egui::Modifiers::NONE)];
        match step {
            Do::Key(shortcut) => {
                // Held as a Linux keyboard sends them: Command is Ctrl, and Ctrl is Command.
                let mut modifiers = shortcut.modifiers;
                modifiers.ctrl |= modifiers.command;
                modifiers.command = modifiers.ctrl;
                self.frame(key(shortcut.logical_key, modifiers), draw)?;
                self.frame(up, draw)?;
            }
            Do::Tab { back, times } => {
                let modifiers = if *back { egui::Modifiers::SHIFT } else { egui::Modifiers::NONE };
                for _ in 0..*times {
                    self.frame(key(egui::Key::Tab, modifiers), draw)?;
                }
                self.frame(up, draw)?;
            }
            Do::Focus { name, nth } => {
                let (node, _) = self.find(name, *nth, "control that takes the focus", |node| node.supports_action(Action::Focus))?;
                let request = ActionRequest { action: Action::Focus, target_tree: TreeId::ROOT, target_node: node, data: None };
                self.frame(vec![egui::Event::AccessKitActionRequest(request)], draw)?;
                self.settle(draw)?;
                if self.tree().map(|tree| tree.focus) != Some(node) {
                    return Err(format!("{:?} did not take the focus", cut(name)));
                }
            }
            Do::Press { name, nth } => {
                let screen = Rect::from_min_size(Pos2::ZERO, self.size);
                let mut aimed = None;
                for _ in 0..MAX_AIM {
                    let (_, rect) = self
                        .find(name, *nth, "enabled control that can be pressed", |node| !node.is_disabled() && node.supports_action(Action::Click))?;
                    let at = rect.center();
                    if !screen.contains(at) {
                        return Err(format!("{:?} is not in the window", cut(name)));
                    }
                    if *pointer == Some(at) {
                        aimed = Some(at);
                        break;
                    }
                    *pointer = Some(at);
                    self.frame(vec![egui::Event::PointerMoved(at)], draw)?;
                    self.settle(draw)?;
                }
                let Some(at) = aimed else { return Err(format!("{:?} moved each time the pointer came over it", cut(name))) };
                for pressed in [true, false] {
                    let event =
                        egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
                    self.frame(vec![event], draw)?;
                }
            }
        }
        self.settle(draw)
    }

    /// Fills the last frame into a picture.
    fn picture(self) -> Result<Picture, UiError> {
        let Some(output) = self.last else { return Err(bad("no frame")) };
        let primitives = self.ctx.tessellate(output.shapes, output.pixels_per_point);
        Ok(fill(&primitives, &self.textures, self.width, self.height, output.pixels_per_point))
    }
}

/// The mail a scene adds to the sample's inbox: the Hebrew and Arabic messages or Odile's two invitations.
fn added_mail(shot: &Shot, time: Timestamp) -> Result<Option<Vec<inboxcraft_store::Change>>, UiError> {
    if shot.right_to_left == Some(true) {
        return sample::right_to_left(time).map(Some);
    }
    if shot.invitations == Some(true) {
        return sample::invitations(time).map(Some);
    }
    Ok(None)
}

/// The window of `shot` over its sample at its time, and the clock both run on: the present sample (`sending` for
/// the outbox and the Scheduled page, with mail added where the shot asks), the frames' set as they show it, or no
/// account at all (`first`).
fn window(shot: &Shot) -> Result<(InboxApp, FakeClock), UiError> {
    let frames = match shot.sample.as_deref() {
        None | Some("present") => false,
        Some("frames") => true,
        Some(other) => return Err(bad(format!("{}: sample {other:?} is not present or frames", shot.out))),
    };
    // The frames' sample is put away and holds its invitations already: a key that would change that is refused.
    for (key, given) in [("put_away", shot.put_away.is_some()), ("invitations", shot.invitations.is_some())] {
        if frames && given {
            return Err(bad(format!("{}: {key} is part of the frames' sample", shot.out)));
        }
    }
    let time = Timestamp::from_unix(if frames { sample::frames::TIME } else { SHOT_TIME }, 0).ok_or_else(|| bad("no time"))?;
    // One clock for the session and the list, so a scene can let time pass.
    let clock = FakeClock::new(time);
    let session = if frames && shot.first != Some(true) {
        if shot.right_to_left == Some(true) {
            sample::frames::right_to_left(Box::new(clock.clone()))?
        } else {
            sample::frames::session(Box::new(clock.clone()))?
        }
    } else if shot.first == Some(true) {
        let store = inboxcraft_store::Memory::new(inboxcraft_store::StoreConfig::default());
        inboxcraft_engine::Session::new(Box::new(store), Box::new(clock.clone())).map_err(UiError::Engine)?
    } else if shot.outbox.is_some() || shot.section.as_deref() == Some(crate::settings::Under::Scheduled.id()) {
        sample::sending(Box::new(clock.clone()))?
    } else if let Some(added) = added_mail(shot, time)? {
        let mut store = sample::mailbox(time)?;
        let batch = added.into_iter().fold(inboxcraft_store::Batch::new(), inboxcraft_store::Batch::with);
        inboxcraft_store::Storage::apply(&mut store, &batch).map_err(UiError::sample)?;
        sample::session_over(store, Box::new(clock.clone()))?
    } else {
        sample::session(Box::new(clock.clone()))?
    };
    let mailbox = Mailbox::new(session, Box::new(clock.clone()));
    let mut app = InboxApp::with_outside(mailbox, Box::new(sample::Pictures::default()), Box::new(NoSave));
    if frames && shot.first != Some(true) {
        // The frames' two drafts, kept from the start, the index counts them everywhere: the message in the outbox
        // (a send is a draft until it goes) and one kept. A scene with no drafts has neither.
        let drafts = shot.drafts.as_deref() != Some("empty");
        // The outbox and the Scheduled page list the message with its time, as their frames do.
        let listed = shot.outbox.is_some() || shot.section.as_deref() == Some(crate::settings::Under::Scheduled.id());
        let outbox = match (drafts, listed) {
            (false, _) => sample::frames::Outbox::Empty,
            (true, true) => sample::frames::Outbox::Scheduled,
            (true, false) => sample::frames::Outbox::Waiting,
        };
        sample::frames::prepare(app.mailbox_mut(), &clock, outbox)?;
        if drafts {
            keep_drafts(&mut app, &clock, &shot.out, FRAMES_WORDS.drafts)?;
        }
        // What the sample leaves due (the drafts' copies for the server) is done here, as the window's first frame
        // would do it: that frame's refresh would otherwise settle over what the scene shows, such as an error.
        app.mailbox_mut().pump(&mut crate::workers::NoWorkers);
    }
    Ok((app, clock))
}

/// Draws the window's frame as the app does: the app may always draw it (a native window), and the mailbox's
/// appearance picks the own frame unless `shot.frame` names one: `own` (the older `strip` and `top_row` read as it) or
/// `system`, the desktop's title bar, where the app draws none.
fn frame(app: &mut InboxApp, shot: &Shot) -> Result<(), UiError> {
    if let Some(name) = &shot.frame {
        let frame = WindowFrame::read(name).ok_or_else(|| bad(format!("{}: frame {name} is not own or system", shot.out)))?;
        app.set_frame(frame);
    }
    app.set_own_frame(true);
    Ok(())
}

/// The sample window as `shot` asks for it.
///
/// # Errors
///
/// [`UiError::Shot`] for a mode, role, row or size the shot names wrongly; the errors of [`render`].
pub fn shoot(shot: &Shot) -> Result<Picture, UiError> {
    let mode = match shot.mode.as_str() {
        "light" => Mode::Light,
        "dark" => Mode::Dark,
        other => return Err(bad(format!("{}: mode {other:?} is not light or dark", shot.out))),
    };
    if shot.components == Some(true) {
        return component_gallery(shot, mode);
    }
    if shot.fields == Some(true) {
        return fields_gallery(shot, mode);
    }
    if shot.blocks == Some(true) {
        return blocks_gallery(shot, mode);
    }
    if shot.screens == Some(true) {
        return screens_gallery(shot, mode);
    }
    if shot.calendar_sets == Some(true) {
        return calendar_gallery(shot, mode);
    }
    if shot.reader == Some(true) {
        return reader_gallery(shot, mode);
    }
    if shot.lists == Some(true) {
        return lists_gallery(shot, mode);
    }
    if shot.index == Some(true) {
        return index_gallery(shot, mode);
    }
    if shot.character_field == Some(true) {
        return field_gallery(shot, mode);
    }
    if shot.status_sets == Some(true) {
        return status_gallery(shot, mode);
    }
    if shot.home_sets == Some(true) {
        return home_gallery(shot, mode);
    }
    if shot.command_sets == Some(true) {
        return commands_gallery(shot, mode);
    }
    if let Some(state) = &shot.home {
        return home_page(shot, mode, state);
    }
    if shot.icons == Some(true) {
        return icon_sheet(shot, mode);
    }
    let (mut app, clock) = window(shot)?;
    // The window starts on Home; a scene shows it only when it asks, every other one is about the place it names.
    if shot.home_place == Some(true) {
        app.show_home();
    } else {
        app.show_mail();
    }
    if shot.first == Some(true) {
        app = app.with_accounts(Arc::new(Pictured), Box::new(Proposed::new(pictured_proposal())), Box::new(Scripted::passing()));
    }
    let level = match shot.motion.as_deref() {
        None | Some("off") => motion::Level::Off,
        Some("reduced") => motion::Level::Reduced,
        Some(other) => return Err(bad(format!("{}: motion {other} is not reduced or off", shot.out))),
    };
    app.set_motion_level(level);
    event_editor(&mut app, shot)?;
    frame(&mut app, shot)?;
    if let Some(percent) = shot.text_scale {
        let scale =
            TextScale::from_percent(percent).ok_or_else(|| bad(format!("{}: text scale {percent} is not 90, 100, 110, 120 or 130", shot.out)))?;
        app.set_text_scale(scale);
    }
    if let Some(account) = shot.narrow {
        app.mailbox_mut().narrow(Some(account));
    }
    if let Some(name) = &shot.view {
        // Today's ids; the earlier `cards` and `simple` are read too (`list` is today's List).
        let view = InboxView::named(name)
            .or_else(|| InboxView::named_before(name).filter(|_| name != "list"))
            .ok_or_else(|| bad(format!("{}: view {name:?} is not grouped, smart or list", shot.out)))?;
        app.mailbox_mut().set_inbox_view(view);
    }
    if shot.put_away == Some(true) {
        sample::put_away(app.mailbox_mut())?;
    }
    if shot.show_done == Some(true) {
        app.mailbox_mut().set_show_done(true);
    }
    match &shot.open {
        Some(Open::Role(name)) if name == "senders" => app.mailbox_mut().open(Place::Senders),
        Some(Open::Role(name)) => {
            let role = [Role::Inbox, Role::Drafts, Role::Sent, Role::Archive, Role::Junk, Role::Trash]
                .into_iter()
                .find(|role| role.label().eq_ignore_ascii_case(name))
                .ok_or_else(|| bad(format!("{}: no role {name:?}", shot.out)))?;
            app.mailbox_mut().open(Place::View(role));
        }
        Some(Open::Folder(id)) => app.mailbox_mut().open(Place::Folder(*id)),
        None => {}
    }
    if let Some(name) = &shot.later {
        let pile = Pile::named(name).ok_or_else(|| bad(format!("{}: pile {name:?} is not snoozed, pins or parked", shot.out)))?;
        app.mailbox_mut().open(Place::Later(pile));
    }
    if let Some(text) = &shot.search {
        app.search(text);
    }
    if shot.showcase == Some(true) {
        let showcase = if frames(shot) { sample::frames::showcase(app.mailbox()) } else { sample::showcase(app.mailbox()) };
        app.mailbox_mut().set_showcase(showcase);
        *app.keymap_mut() = gallery::keymap();
    }
    if let Some(steps) = shot.step {
        if shot.select.is_some() {
            return Err(bad(format!("{}: a shot selects by step or by subject, not both", shot.out)));
        }
        for _ in 0..steps {
            app.mailbox_mut().step(true);
        }
    }
    if let Some(subject) = &shot.select {
        let id = app.mailbox().messages().iter().find(|row| row.subject == *subject).map(|row| row.id);
        let Some(id) = id else { return Err(bad(format!("{}: no row with the subject {subject:?}", shot.out))) };
        app.mailbox_mut().select(Some(id));
        let (reader, mailbox) = app.reader_mut();
        match shot.pictures.as_deref() {
            Some("once") => reader.allow_once(mailbox, id),
            Some("always") => reader.allow_always(mailbox, id),
            Some(other) => return Err(bad(format!("{}: pictures {other:?} is not once or always", shot.out))),
            None => {}
        }
        reader.set_full_width(shot.full_width.unwrap_or(false));
        if shot.details == Some(true) {
            reader.toggle_details(id);
        }
        if shot.quotes == Some(true) {
            reader.toggle_quotes(id);
        }
        match shot.html_view.as_deref() {
            Some("plain") => reader.set_plain(id, true),
            Some("fitted") | None => {}
            Some(other) => return Err(bad(format!("{}: html_view {other:?} is not fitted or plain", shot.out))),
        }
    } else if shot.pictures.is_some() || shot.full_width.is_some() || shot.details.is_some() || shot.quotes.is_some() {
        return Err(bad(format!("{}: the reader settings need a selected row", shot.out)));
    }
    act(&mut app, shot)?;
    sync(&mut app, shot)?;
    fail(&mut app, shot)?;
    notice(&mut app, shot);
    if let Some(typed) = &shot.picker {
        if app.mailbox().selected().is_none() {
            return Err(bad(format!("{}: the time picker needs a selected row", shot.out)));
        }
        app.open_picker(typed);
    }
    if let Some(menu) = &shot.menu
        && !app.open_menu(menu)
    {
        return Err(bad(format!("{}: there is no menu {menu:?} to open here", shot.out)));
    }
    if let Some(state) = &shot.onboard {
        app = onboard(app, &shot.out, state)?;
    }
    if shot.settings == Some(true) {
        app.open_settings();
    }
    if shot.accounts == Some(true) {
        app.open_accounts();
    }
    if let Some(account) = shot.edit_account {
        let (onboarding, mailbox) = app.onboarding_mut();
        onboarding.edit(mailbox, account);
    }
    if let Some(account) = shot.remove_account
        && !app.remove_account_as_owner(account)
    {
        return Err(bad(format!("{}: there is no account {account} to remove", shot.out)));
    }
    places(&mut app, &clock, shot)?;
    if let Some(scene) = &shot.outbox {
        outbox(&mut app, &clock, &shot.out, scene, words(shot))?;
    }
    let [width, height] = shot.size.unwrap_or([1200.0, 760.0]);
    for (key, at) in [("pointer", shot.pointer), ("click", shot.click)] {
        if let Some([x, y]) = at
            && !((0.0..=width).contains(&x) && (0.0..=height).contains(&y))
        {
            return Err(bad(format!("{}: {key} [{x}, {y}] is not inside the window of {width} by {height} points", shot.out)));
        }
    }
    let pointer = shot.pointer.map(|[x, y]| pos2(x, y));
    if let Some(steps) = shot.steps.as_deref().filter(|steps| !steps.is_empty()) {
        let steps = steps.iter().map(Step::read).collect::<Result<Vec<_>, _>>().map_err(|error| bad(format!("{}: {error}", shot.out)))?;
        let pointer = Pointer { rest: pointer, click: shot.click.map(|[x, y]| pos2(x, y)) };
        return render_steps(&mut app, vec2(width, height), shot.scale.unwrap_or(1.0), mode, pointer, &steps)
            .map_err(|error| bad(format!("{}: {error}", shot.out)));
    }
    match shot.click {
        Some([x, y]) => {
            // A scripted press, like a step's: the window's commands run for an agent, so only a safe one runs.
            app.mailbox_mut().set_caller(inboxcraft_engine::Caller::Agent);
            let picture = render_clicked(&mut app, vec2(width, height), shot.scale.unwrap_or(1.0), mode, pointer, pos2(x, y))?;
            match app.mailbox().refused() {
                Some(command) => Err(bad(format!("{}: the click reached {command}, which only the owner runs", shot.out))),
                None => Ok(picture),
            }
        }
        None => render_pointed(&mut app, vec2(width, height), shot.scale.unwrap_or(1.0), mode, pointer),
    }
}

/// Where the picture `out` goes: `out` itself, or with `dir`, `out` under it, which an `out` that is empty, absolute
/// or has `.` or `..` in it cannot leave.
///
/// # Errors
///
/// [`UiError::Shot`] for such an `out` with a `dir`.
pub fn out_path(dir: Option<&std::path::Path>, out: &str) -> Result<std::path::PathBuf, UiError> {
    let Some(dir) = dir else { return Ok(std::path::PathBuf::from(out)) };
    let relative = std::path::Path::new(out);
    if out.is_empty() || !relative.components().all(|part| matches!(part, std::path::Component::Normal(_))) {
        return Err(bad(format!("{:?} is not a relative path that stays under --out-dir", cut(out))));
    }
    Ok(dir.join(relative))
}

/// The text of a script read as bytes (at most [`MAX_SCRIPT`] and one more): refused whole when longer, so a cut
/// never splits a character, then read as UTF-8.
///
/// # Errors
///
/// [`UiError::Shot`] for a script longer than [`MAX_SCRIPT`] or one that is not UTF-8.
pub fn script_text(bytes: Vec<u8>) -> Result<String, UiError> {
    if bytes.len() > MAX_SCRIPT {
        return Err(bad(format!("the script is longer than {MAX_SCRIPT} bytes")));
    }
    String::from_utf8(bytes).map_err(|_| bad("the script is not UTF-8"))
}

#[cfg(test)]
#[path = "shot_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "shot_steps_tests.rs"]
mod steps_tests;

#[cfg(test)]
#[path = "shot_fail_tests.rs"]
mod fail_tests;

#[cfg(test)]
#[path = "shot_frames_tests.rs"]
mod frames_tests;

#[cfg(test)]
#[path = "shot_scenes_tests.rs"]
mod scenes_tests;
