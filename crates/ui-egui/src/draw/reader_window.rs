// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A message in a window of its own, as the Reader page's "own window" frames draw it: where Appearance › Layout says
//! a message opens in a window, a press on a row or `Enter` in the list opens the message in a window through the host
//! ([`super::host`]) and the pane lets go of it. The window keeps its message when the list moves on (another place,
//! a search, the next row): it has a reader of its own, with the why line, the cards and the pictures as in the pane,
//! centred in a column [`COLUMN`] wide.
//!
//! What is done in the window acts on its message: its buttons ask for it by id, and the acts on a row by key (Done,
//! Archive, read or unread, flag, Delete) run on it, never on the row the list rings in the main window, whose keys are
//! its own. An act that takes the message away (Done, Archive, Delete, Move, Snooze) closes the window; Reply, Reply
//! all and Forward open the composer in the main window. One `Esc`, its Close control or the desktop's close closes it.
//!
//! When the store keeps the message no more (the server took it away), the window says so and keeps nothing of it:
//! no text, no picture, no act. Where no window of its own can open (the web app, a headless run) the message opens in
//! the pane, and the status line says why once. At most [`MOST`] windows are open: one more closes the oldest.

use egui::{Context, Layout, Rect, Ui, UiBuilder, Vec2, ViewportCommand, ViewportId, vec2};
use inboxcraft_engine::appearance::{ReaderHost, WindowFrame};

use super::host::{self, Host, Hosted, Look};
use super::{InboxApp, ROW_ACTS, invite, reader};
use crate::keymap::Keymap;
use crate::mailbox::{Act, Mailbox, MessageRow};
use crate::reader::{PictureState, Reader};
use crate::theme::Theme;
use crate::{fetch, save, tr};

/// The most windows of their own open at once.
pub(super) const MOST: usize = 8;

/// The viewports the windows take turns at: twice [`MOST`], so that a window always finds one no window showed in the
/// frame before (a viewport shown again at once would be the closed window's, not a new one).
const SLOTS: usize = MOST * 2;

/// The window's inner size when it opens, in points (the frames' 960 by 900).
const SIZE: Vec2 = vec2(960.0, 900.0);

/// The width of the reader's column in the window, centred, in points.
pub(super) const COLUMN: f32 = 760.0;

/// One message in a window of its own.
struct Shown {
    /// Which of the [`SLOTS`] viewports it takes.
    slot: usize,
    viewport: ViewportId,
    /// The message, also once it is gone.
    message: u64,
    reader: Reader,
    textures: reader::Textures,
    hover: Option<String>,
    /// The subject as the window opened on it: the window's title.
    subject: String,
    /// The store keeps the message no more.
    gone: bool,
    /// The list's rows as last checked ([`Mailbox::rows_serial`]): the message is read again when they change.
    checked: u64,
    /// The desktop is asked to bring the window to the front in the next frame.
    front: bool,
}

/// The windows of their own the reader has open, and what they share: the fetch of remote pictures and the saving of
/// attachments.
pub(super) struct Windows {
    shown: Vec<Shown>,
    fetch: fetch::Shared,
    save: save::Shared,
    /// Where the readers' originals go to the browser, when the window can open one.
    original: Option<crate::original::Shared>,
    /// How many frames the windows followed ([`Windows::follow`]).
    frames: u64,
    /// The slots of the windows that closed, each with the frame it closed in: taken again two frames on at the soonest.
    freed: Vec<(usize, u64)>,
    /// The viewports of the windows that closed, whose reader's state in egui's memory is forgotten at the next draw.
    closing: Vec<ViewportId>,
    /// The context opens windows of their own (it does not embed them), as the last frame found.
    real: bool,
    /// The status line has said that no window of its own can open.
    told: bool,
}

impl std::fmt::Debug for Windows {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Windows").field("open", &self.shown.len()).field("real", &self.real).finish_non_exhaustive()
    }
}

/// What the windows drew this frame: what was asked in each (by its viewport), and whether a window could not open.
#[derive(Debug, Default)]
pub(super) struct Drawn {
    asked: Vec<(ViewportId, reader::Asked)>,
    /// The messages whose window could not open after all: they open in the pane.
    fallen: Vec<u64>,
}

impl Windows {
    /// No window open yet; the readers they open fetch through `fetch` and save through `save`.
    pub(super) fn new(fetch: fetch::Shared, save: save::Shared) -> Self {
        Self { shown: Vec::new(), fetch, save, original: None, frames: 0, freed: Vec::new(), closing: Vec::new(), real: false, told: false }
    }

    /// The readers opened from now on open their originals through `original`; so do those open now.
    pub(super) fn set_original(&mut self, original: &crate::original::Shared) {
        self.original = Some(original.clone());
        for shown in &mut self.shown {
            shown.reader.set_original(original.clone());
        }
    }

    /// The messages open in windows of their own, oldest window first; a window whose message is gone has none.
    #[cfg(test)]
    pub(super) fn messages(&self) -> Vec<Option<u64>> {
        self.shown.iter().map(|shown| shown.reader.open_id()).collect()
    }

    /// Whether `ctx` opens windows of their own, read once a frame before anything opens.
    pub(super) fn follow(&mut self, ctx: &Context) {
        self.frames = self.frames.wrapping_add(1);
        self.real = Host::Window(ViewportId::ROOT).usable(ctx) != Host::Docked;
    }

    /// Opens message `id` in a window of its own, or brings its window to the front when one shows it. Whether it is
    /// shown in a window.
    fn open(&mut self, mailbox: &mut Mailbox, id: u64) -> bool {
        if let Some(shown) = self.shown.iter_mut().find(|shown| !shown.gone && shown.message == id) {
            shown.front = true;
            return true;
        }
        if self.shown.len() >= MOST {
            self.close(0);
        }
        let row = mailbox.messages().iter().find(|row| row.id == id).cloned();
        let mut reader = Reader::new(Box::new(self.fetch.another()), Box::new(self.save.clone()));
        if let Some(original) = &self.original {
            reader.set_original(original.clone());
        }
        reader.open(mailbox, Some(id));
        // What the head shows once the list has moved on.
        let row = mailbox.messages().iter().find(|row| row.id == id).cloned().or(row);
        let subject = reader.opened(id).map(|opened| opened.header.subject.clone()).or_else(|| row.as_ref().map(|row| row.subject.clone()));
        reader.keep_row(row);
        let slot = self.free_slot();
        let viewport = viewport_of(slot);
        let subject = subject.unwrap_or_default();
        let checked = mailbox.rows_serial();
        self.shown.push(Shown {
            slot,
            viewport,
            message: id,
            reader,
            textures: reader::Textures::new(),
            hover: None,
            subject,
            gone: false,
            checked,
            front: true,
        });
        true
    }

    /// A slot no window takes, and none closed in this frame or the one before; else (a burst of closes) any no window
    /// takes. At most [`MOST`] of the [`SLOTS`] are taken, so there is always one.
    fn free_slot(&self) -> usize {
        let taken = |slot: usize| self.shown.iter().any(|shown| shown.slot == slot);
        let fresh = |slot: usize| !self.freed.iter().any(|(freed, at)| *freed == slot && at.saturating_add(1) >= self.frames);
        (0..SLOTS).find(|slot| !taken(*slot) && fresh(*slot)).or_else(|| (0..SLOTS).find(|slot| !taken(*slot))).unwrap_or(0)
    }

    /// Closes the window at `at`: its slot rests, and its reader's state in egui's memory is forgotten at the next draw.
    fn close(&mut self, at: usize) {
        if at < self.shown.len() {
            let shown = self.shown.remove(at);
            self.freed.retain(|(slot, _)| *slot != shown.slot);
            self.freed.push((shown.slot, self.frames));
            self.closing.push(shown.viewport);
        }
    }

    /// Forgets what the readers of the windows that closed kept in egui's memory.
    fn forget(&mut self, ctx: &Context) {
        for viewport in self.closing.drain(..) {
            reader::forget(ctx, viewport);
            crate::component::popup::forget_viewport(ctx, viewport);
        }
    }

    /// Takes the pictures the windows' readers fetched; whether any came.
    pub(super) fn poll(&mut self) -> bool {
        self.shown.iter_mut().fold(false, |any, shown| shown.reader.poll() | any)
    }

    /// Reads each window's message again, by its id alone, once the list's rows changed: its row kept fresh for the
    /// head, or, once the store keeps it no more, the window says so and forgets what it showed. A store that cannot
    /// say now leaves the window as it is, read again at the next change.
    fn check(&mut self, mailbox: &mut Mailbox) {
        let serial = mailbox.rows_serial();
        for shown in self.shown.iter_mut().filter(|shown| !shown.gone && shown.checked != serial) {
            shown.checked = serial;
            match mailbox.message_row(shown.message) {
                Ok(Some(read)) => shown.reader.keep_row(Some(fresher(mailbox, read))),
                Ok(None) => {
                    shown.gone = true;
                    shown.reader.open(mailbox, None);
                    shown.textures.clear();
                    shown.hover = None;
                }
                Err(_) => {}
            }
        }
    }

    /// Forgets the textures of every window: they were made in a context that is gone.
    fn forget_textures(&mut self) {
        for shown in &mut self.shown {
            shown.textures.clear();
        }
    }

    /// Draws every window of its own; what was asked in each.
    fn show(&mut self, ctx: &Context, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, frame: WindowFrame) -> Drawn {
        self.forget(ctx);
        let mut drawn = Drawn::default();
        let mut closed = Vec::new();
        for shown in &mut self.shown {
            // A texture lives no longer than the picture the reader keeps for it.
            let reader = &shown.reader;
            shown.textures.retain(|key, _| matches!(reader.picture(key), Some(PictureState::Ready(_))));
            if std::mem::take(&mut shown.front) {
                ctx.send_viewport_cmd_to(shown.viewport, ViewportCommand::Focus);
            }
            let title = if shown.gone { tr!("Message not here any more").to_owned() } else { shown.subject.clone() };
            let look = Look { theme, frame, size: SIZE };
            let hosted = host::show(ctx, Host::Window(shown.viewport), &title, look, |ui, _| content(ui, theme, keymap, mailbox, shown));
            match hosted {
                Hosted::Here(_) => {
                    drawn.fallen.push(shown.message);
                    closed.push(shown.viewport);
                }
                Hosted::Window(window) => {
                    drawn.asked.extend(window.inner.into_iter().map(|asked| (shown.viewport, asked)));
                    if window.closed {
                        closed.push(shown.viewport);
                    }
                }
            }
        }
        for viewport in closed {
            if let Some(at) = self.shown.iter().position(|shown| shown.viewport == viewport) {
                self.close(at);
            }
        }
        drawn
    }

    /// Applies `asked` in the window `viewport`; what only the main window can do (an answer, which opens the composer
    /// there) comes back.
    fn apply(&mut self, ctx: &Context, mailbox: &mut Mailbox, viewport: ViewportId, asked: reader::Asked) -> Option<reader::Asked> {
        let at = self.shown.iter().position(|shown| shown.viewport == viewport)?;
        let shown = self.shown.get_mut(at)?;
        let reader = &mut shown.reader;
        match asked {
            reader::Asked::Expand(id) => reader.expand(mailbox, id),
            reader::Asked::Details(id) => reader.toggle_details(id),
            reader::Asked::Quotes(id) => reader.toggle_quotes(id),
            reader::Asked::View(id, view) => reader.set_plain(id, view == crate::component::content_bar::View::Plain),
            reader::Asked::Once(id) => reader.allow_once(mailbox, id),
            reader::Asked::Retry(id, key) => reader.retry(mailbox, id, &key),
            reader::Asked::Always(id) => reader.allow_always(mailbox, id),
            reader::Asked::Save(id, index) => reader.save(mailbox, id, index),
            reader::Asked::Original(id) => reader.open_original(id, ctx),
            reader::Asked::After(after) => mailbox.set_after(after),
            reader::Asked::Copy(url) => {
                reader.set_notice(tr!("Link copied: {url}", url = crate::mailbox::shown(&url)));
                ctx.copy_text(url);
            }
            reader::Asked::CopySubject(subject) => {
                reader.set_notice(tr!("Subject copied").to_owned());
                ctx.copy_text(subject);
            }
            reader::Asked::Dismiss => reader.dismiss_notice(),
            reader::Asked::Act(id, act) => {
                let acts = mailbox.acts();
                super::start(reader, mailbox, id, act);
                let done = mailbox.acts() != acts;
                // What the act changed (a pin, a reminder, a snooze, the sender's screening), read again for the head.
                if let Ok(Some(read)) = mailbox.message_row(id) {
                    reader.keep_row(Some(fresher(mailbox, read)));
                }
                // An act that took the message away ends the window's work; one that failed leaves it where it was.
                if act.slides() && done {
                    self.close(at);
                }
            }
            reader::Asked::Presets => mailbox.load_presets(),
            reader::Asked::Invite(id, invite::Ask::Answer(answer)) => reader.answer_invite(mailbox, id, answer),
            reader::Asked::Invite(id, invite::Ask::Change) => reader.change_answer(id),
            reader::Asked::Invite(id, invite::Ask::Remove) => reader.remove_invite(mailbox, id),
            reader::Asked::Close => self.close(at),
            // A window has no width to fill and no list to go back to.
            reader::Asked::FullWidth(_) | reader::Asked::Back => {}
            reader::Asked::Answer(..) => return Some(asked),
        }
        None
    }
}

/// The viewport of slot `slot`.
fn viewport_of(slot: usize) -> ViewportId {
    ViewportId::from_hash_of(("inboxcraft-reader-window", slot))
}

/// The row of a window's message once it was `read` again from the store: the list's own when the list shows it, else
/// what the store says, triage state and all.
fn fresher(mailbox: &Mailbox, read: MessageRow) -> MessageRow {
    mailbox.messages().iter().find(|row| row.id == read.id).cloned().unwrap_or(read)
}

/// The window's content in `ui`: the reader in its column, or the words for a message that is gone. The acts on a row
/// pressed by key in this window act on its message.
fn content(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, shown: &mut Shown) -> Vec<reader::Asked> {
    ui.set_style(theme.style());
    let room = ui.max_rect();
    ui.painter().rect_filled(room, 0.0, theme.palette.surface);
    if shown.gone {
        gone(ui, theme, room);
        return Vec::new();
    }
    let column = Rect::from_center_size(room.center(), vec2(room.width().min(COLUMN), room.height()));
    let mut inside = ui.new_child(UiBuilder::new().max_rect(column).layout(Layout::top_down(egui::Align::Min)));
    inside.set_clip_rect(column.intersect(ui.clip_rect()));
    let hover = shown.hover.take();
    let mut drawn = reader::show(&mut inside, theme, keymap, mailbox, &shown.reader, &mut shown.textures, hover.as_deref());
    if drawn.hover != hover {
        ui.ctx().request_repaint();
    }
    shown.hover = drawn.hover.take();
    drawn.asked.extend(keyed(ui.ctx(), keymap, mailbox, &shown.reader));
    drawn.asked
}

/// The act on a row whose key was pressed in this window, on the window's message: none while a float of the window
/// or a text field has the keys.
fn keyed(ctx: &Context, keymap: &Keymap, mailbox: &Mailbox, reader: &Reader) -> Option<reader::Asked> {
    if crate::component::popup::any_open(ctx) || ctx.text_edit_focused() {
        return None;
    }
    let id = reader.open_id()?;
    let row: Option<&MessageRow> = mailbox.messages().iter().find(|row| row.id == id).or_else(|| reader.kept_row());
    let (_, act) = ROW_ACTS.into_iter().find(|(action, _)| keymap.pressed(ctx, action))?;
    let act = match (act, row) {
        (Act::Read, Some(row)) if row.seen => Act::Unread,
        (Act::Flag, Some(row)) if row.flagged => Act::Unflag,
        (Act::Done, Some(row)) if row.done => Act::Undone,
        (act, _) => act,
    };
    Some(reader::Asked::Act(id, act))
}

/// What a window whose message is gone says, in the middle of its `room`.
fn gone(ui: &mut Ui, theme: &Theme, room: Rect) {
    let mut inside = ui.new_child(UiBuilder::new().max_rect(room.shrink2(vec2(40.0, 0.0))).layout(Layout::top_down(egui::Align::Min)));
    inside.set_min_size(inside.available_size());
    crate::component::empty::empty_state(
        &mut inside,
        theme,
        tr!("Reader · not here any more"),
        tr!("This message is not here any more."),
        tr!("The server or another device took it away. Esc closes this window."),
    );
}

impl InboxApp {
    /// Reads, once a frame before anything is drawn, what `ctx` offers: whether it opens windows of their own, and
    /// whether it is another context than the one the pictures' textures were made in (a window opened again on the
    /// same app: #165). A texture names nothing in another context, so the pictures are uploaded again.
    pub(super) fn follow_context(&mut self, ctx: &Context) {
        self.windows.follow(ctx);
        let mark = egui::Id::unique("inboxcraft-textures-made-here");
        if ctx.data(|data| data.get_temp::<()>(mark)).is_none() {
            ctx.data_mut(|data| data.insert_temp(mark, ()));
            self.textures.clear();
            self.windows.forget_textures();
        }
    }

    /// Opens message `id`, which the owner opened from the list, in a window of its own when Appearance › Layout says
    /// so and the list shows it; the pane lets go of it. Where no window of its own can open, the status line says why,
    /// once, and the pane opens it. Whether a window shows it.
    pub(super) fn open_in_window(&mut self, id: u64) -> bool {
        if self.mailbox.appearance().layout.reader != ReaderHost::Window || !self.mailbox.messages().iter().any(|row| row.id == id) {
            return false;
        }
        if !self.windows.real {
            if !std::mem::replace(&mut self.windows.told, true) {
                self.say(tr!("Messages open in the pane: no window of their own can open here."));
            }
            return false;
        }
        if !self.windows.open(&mut self.mailbox, id) {
            return false;
        }
        if self.mailbox.selected().is_some() {
            self.mailbox.select(None);
        }
        self.reading = false;
        true
    }

    /// Draws the windows of their own and applies what was asked in them.
    pub(super) fn reader_windows(&mut self, ctx: &Context, theme: &Theme) {
        self.windows.check(&mut self.mailbox);
        let frame = self.mailbox.appearance().frame;
        let drawn = self.windows.show(ctx, theme, &self.keymap, &self.mailbox, frame);
        for (viewport, asked) in drawn.asked {
            if let Some(reader::Asked::Answer(id, answer)) = self.windows.apply(ctx, &mut self.mailbox, viewport, asked) {
                // The composer is the main window's: it comes to the front with the keyboard in the text (To for a
                // forward).
                if self.answer(id, answer) {
                    let field = if answer == crate::compose::Answer::Forward {
                        super::compose::field_id(crate::compose::Line::To)
                    } else {
                        super::compose::text_id()
                    };
                    ctx.memory_mut(|memory| memory.request_focus(field));
                    ctx.send_viewport_cmd_to(ViewportId::ROOT, ViewportCommand::Focus);
                }
            }
        }
        for id in drawn.fallen {
            // The window could not open after all: the message opens in the pane, and the status line says why.
            self.windows.real = false;
            self.windows.told = true;
            self.say(tr!("Messages open in the pane: no window of their own can open here."));
            self.mailbox.select(Some(id));
        }
    }
}

#[cfg(test)]
#[path = "reader_window_tests.rs"]
mod tests;
