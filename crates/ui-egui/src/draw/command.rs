// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The command line: its field across the command line, between the index's column and the window's controls, and
//! its results hanging under the field over the dimmed window. Plain text searches the mail: while the mail shows, the
//! list fills as it is typed; from another place typing changes nothing under the field, and Enter shows the mail
//! searched; `>` lists the window's commands as the palette did (one that cannot run here says why and runs
//! nothing), `@` the people the mail knows, `#` the mailboxes. The arrows move the pick, Enter or a press runs it;
//! one `Esc` closes the results, the next gives the keyboard back to where it was; a press beside them closes them.
//!
//! The results are an area of their own, not a [`crate::component::popup::float`]: a float takes the keyboard and the
//! pointer for itself, and the field above it must keep both to be typed in. While they show they hold the window's
//! keys ([`crate::surface::Surface::Command`]).

use egui::{Area, Context, Id, Key, Modifiers, Order, Rect, Sense, Ui, UiBuilder, pos2};

use crate::command::{Command, Mode, field_id};
use crate::component::command_field::{self, Scope};
use crate::component::popup::{self, Item, ItemState};
use crate::component::{key_hint, soon};
use crate::compose::Suggestion;
use crate::iconset::IconId;
use crate::keymap::{self, Keymap};
use crate::palette::Row;
use crate::theme::Theme;
use crate::tr;

/// The most rows of people or mailboxes listed.
const MOST_ROWS: usize = 50;

/// The line past which the results' lines come in together as they unfold at full (M8): the last that fit a window.
const UNFOLDED: usize = 12;

/// The results' lines as they unfold (M8): at full one more line a `stagger` (30 ms) from the first frame the list of
/// a prefix shows, each whole in its frame; at reduced and off all together. A key ends it: every line is there.
struct Lines {
    /// How many lines show in this frame.
    shown: usize,
    /// The lines counted so far.
    counted: usize,
}

impl Lines {
    /// The lines of results of `count` rows in `mode`, as far as they have come in.
    fn of(ctx: &Context, mode: Mode, count: usize, empty: bool) -> Self {
        let rows = match mode {
            Mode::Search => 0,
            // The head and the plugin's row.
            Mode::Command => count.saturating_add(2),
            Mode::Person | Mode::Mailbox => count.saturating_add(1),
        };
        // And the line saying nothing matches, and the prefixes.
        let lines = rows.saturating_add(usize::from(empty)).saturating_add(1).min(UNFOLDED);
        let stagger = f64::from(crate::motion::STAGGER);
        // Line k (from 1) comes in at k staggers; the movement ends once the last is in.
        let length = stagger * (lines as f64 + 0.5);
        let shown = match crate::motion::level(ctx) {
            crate::motion::Level::Full => crate::motion::run(ctx, Id::unique("command-line-unfold").with(mode as u8), length)
                // A whole number of staggers, at most a dozen: the cast keeps it.
                .map_or(usize::MAX, |age| ((age / stagger).floor().max(0.0) as usize).min(lines)),
            _ => usize::MAX,
        };
        Self { shown: if shown >= lines { usize::MAX } else { shown }, counted: 0 }
    }

    /// Whether the next line shows.
    fn next(&mut self) -> bool {
        self.counted = self.counted.saturating_add(1);
        self.counted <= self.shown
    }
}

/// A mailbox the `#` results list: its id, its path and its account's name.
#[derive(Debug, Clone)]
pub(super) struct Mailbox {
    pub(super) id: u64,
    pub(super) name: String,
    pub(super) account: String,
}

/// A command the `>` results list: its row, and why it cannot run here, if it cannot.
#[derive(Debug, Clone)]
pub(super) struct Listed {
    pub(super) row: Row,
    pub(super) refused: Option<&'static str>,
}

/// What the results list for the mode typed.
#[derive(Debug, Default)]
pub(super) struct Lists {
    pub(super) commands: Vec<Listed>,
    pub(super) people: Vec<Suggestion>,
    pub(super) mailboxes: Vec<Mailbox>,
    /// How many commands the line lists with nothing typed: the head's "of".
    pub(super) all_commands: usize,
}

impl Lists {
    /// How many rows the results hold for `mode`.
    fn count(&self, mode: Mode) -> usize {
        match mode {
            Mode::Search => 0,
            Mode::Command => self.commands.len(),
            Mode::Person => self.people.len(),
            Mode::Mailbox => self.mailboxes.len(),
        }
    }
}

/// What the owner asked of the command line this frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Asked {
    /// Run the window's action.
    Run(&'static str),
    /// Show the settings on the page.
    Page(&'static str),
    /// Show the mail of this address.
    Person(String),
    /// Go to the mailbox.
    Mailbox(u64),
    /// The text typed changed: the mail shown, while it is the mail, searched for it as it is typed.
    Searched(String),
    /// Enter on a search: the mail searched for the text, shown from any place.
    Find(String),
    /// Close the results; the field keeps the keyboard.
    Close,
}

/// The keys the command line took before the window read its own, this frame.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Early {
    enter: bool,
    step: Option<bool>,
}

/// Takes the command line's keys before anything else reads them: with the results shown, `Esc` closes them (and the
/// field gets the keyboard back) and the arrows move the pick; with the keyboard in the field and no results, `Esc`
/// closes the command line and the keyboard goes back to where it was. Enter is read here and acted on once the rows
/// are known. Results whose field lost the keyboard to something outside them close.
pub(super) fn early(ctx: &Context, command: &mut Command) -> Early {
    if command.take_focus_asked() {
        ctx.memory_mut(|memory| memory.request_focus(field_id()));
    }
    if command.slash_pending() {
        ctx.input_mut(|input| {
            if let Some(at) = input.events.iter().position(|event| matches!(event, egui::Event::Text(text) if text == "/")) {
                input.events.remove(at);
            }
        });
    }
    let (has, had) = (command.has_focus(ctx), command.had_focus());
    let mut early = Early::default();
    if command.results() {
        let (esc, down, up) = ctx.input_mut(|input| {
            (
                input.consume_key(Modifiers::NONE, Key::Escape),
                input.consume_key(Modifiers::NONE, Key::ArrowDown),
                input.consume_key(Modifiers::NONE, Key::ArrowUp),
            )
        });
        if esc {
            command.close_results();
            ctx.memory_mut(|memory| memory.request_focus(field_id()));
            return early;
        }
        early.step = if down {
            Some(true)
        } else if up {
            Some(false)
        } else {
            None
        };
        early.enter = (has || had) && ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Enter));
    } else if (has || had) && ctx.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape)) {
        command.close(ctx);
    }
    early
}

/// The results' id.
fn results_id() -> Id {
    Id::unique("command-line-results")
}

/// Whether the keyboard is on a row of the results.
fn inside_results(ctx: &Context) -> bool {
    let layer = egui::LayerId::new(Order::Foreground, results_id());
    ctx.memory(|memory| memory.focused()).and_then(|focused| ctx.read_response(focused)).is_some_and(|focused| focused.layer_id == layer)
}

/// The field's box in the command line `rect`: from the index's column to `room` before the line's right end (the
/// window's controls, when they stand there), its height centred.
fn field_rect(rect: Rect, theme: &Theme, index: f32, room: f32) -> Rect {
    let high = command_field::height(theme);
    let left = rect.left() + index;
    let right = (rect.right() - room).max(left + high);
    let top = (rect.center().y - high / 2.0).round();
    Rect::from_min_max(pos2(left, top), pos2(right, top + high))
}

/// Draws the command line into `ui` over `line` (the command line's region; `index` how far in from its left the field
/// starts, `room` what it leaves at the line's right end), `mailbox` the name of the mailbox shown for a narrowed
/// search, and its results for `lists`; what was asked for.
pub(super) fn show(
    ui: &mut Ui,
    theme: &Theme,
    keymap: &Keymap,
    command: &mut Command,
    (line, index, room, mailbox): (Rect, f32, f32, &str),
    lists: &Lists,
    early: Early,
) -> Vec<Asked> {
    let mut asked = Vec::new();
    let ctx = ui.ctx().clone();
    // While the results show they hold the keyboard, as a modal surface does: taken by anything outside them (Tab,
    // a screen reader's focus), it comes back to the field. A press beside them lands on the dimmed window and closes
    // them; `Esc` closes them.
    if command.results() && !command.has_focus(&ctx) && !inside_results(&ctx) {
        ctx.memory_mut(|memory| memory.request_focus(field_id()));
    }
    let rect = field_rect(line, theme, index, room);
    let scope = match command.mode() {
        Mode::Command => Scope::Commands,
        Mode::Person => Scope::People,
        Mode::Mailbox => Scope::Mailboxes,
        Mode::Search if command.narrowed() => Scope::Mailbox(mailbox),
        Mode::Search => Scope::Open,
    };
    let open_key = keymap.chip(ui.ctx(), keymap::PALETTE).unwrap_or_default();
    let typed = ui
        .scope_builder(UiBuilder::new().max_rect(rect).id_salt("command-line"), |ui| {
            command_field::command_field(ui, theme, field_id(), command.text_mut(), scope, &open_key, None)
        })
        .inner;
    let (mode_before, narrowed_before) = (command.mode(), command.narrowed());
    if typed.response.changed() {
        command.typed();
        if command.mode() == Mode::Search {
            asked.push(Asked::Searched(command.text().to_owned()));
        } else if mode_before == Mode::Search {
            // The prefix leaves the search: the list shows the mail again.
            asked.push(Asked::Searched(String::new()));
        }
    } else if typed.back && (mode_before != Mode::Search || narrowed_before) {
        command.drop_scope();
    }
    // A press on the field shows the results; Tab onto it does not.
    if typed.response.gained_focus() && ui.input(|input| input.pointer.any_pressed() || input.pointer.any_released()) {
        command.pressed_open();
    }
    // Where the box stands, for the tests; whether it kept the keyboard, for the next frame's `Esc` (egui takes the
    // keyboard from a field before the frame in which `Esc` is pressed).
    ui.ctx().data_mut(|data| data.insert_temp(field_id().with("box"), typed.rect));
    command.keep_focus(ui.ctx());
    if let Some(down) = early.step {
        command.step(down, lists.count(command.mode()));
    }
    if early.enter {
        asked.extend(chosen(command, lists, command.picked()));
    }
    if command.results() {
        asked.extend(results(ui.ctx(), theme, keymap, command, rect, lists));
    }
    asked
}

/// What running row `at` of the results asks for, if it can run.
fn chosen(command: &Command, lists: &Lists, at: usize) -> Option<Asked> {
    // A bare prefix runs nothing, unless the arrows picked a row.
    if command.mode() != Mode::Search && command.text().trim().is_empty() && !command.stepped() {
        return None;
    }
    match command.mode() {
        Mode::Search => Some(Asked::Find(command.text().to_owned())),
        Mode::Command => {
            let listed = lists.commands.get(at).filter(|listed| listed.refused.is_none())?;
            Some(listed.row.page.map_or(Asked::Run(listed.row.id), Asked::Page))
        }
        Mode::Person => lists.people.get(at).map(|person| Asked::Person(person.address.clone())),
        Mode::Mailbox => lists.mailboxes.get(at).map(|mailbox| Asked::Mailbox(mailbox.id)),
    }
}

/// The dimmed window under the command line and the results hanging under the field `rect`; what a press asked for.
fn results(ctx: &Context, theme: &Theme, keymap: &Keymap, command: &mut Command, rect: Rect, lists: &Lists) -> Vec<Asked> {
    let mut asked = Vec::new();
    let screen = ctx.content_rect();
    // The window under the command line, dimmed: a press on it closes the results and reaches nothing under it.
    let dim = Rect::from_min_max(pos2(screen.left(), rect.bottom()), screen.max);
    let beside = Area::new(Id::unique("command-line-dim")).order(Order::Middle).fixed_pos(dim.min).constrain(false).fade_in(false).show(ctx, |ui| {
        let (area, response) = ui.allocate_exact_size(dim.size(), Sense::click());
        ui.painter().rect_filled(area, 0.0, theme.palette.scrim);
        response.clicked()
    });
    ctx.move_to_top(beside.response.layer_id);
    if beside.inner {
        asked.push(Asked::Close);
    }
    // As tall as the window has room for under the field; past that the rows scroll.
    let room = (screen.bottom() - rect.bottom() - theme.spacing.s4).max(theme.boxes().menu_row * 3.0);
    let shown = Area::new(results_id())
        .order(Order::Foreground)
        .fixed_pos(rect.left_bottom())
        .constrain(false)
        .fade_in(false)
        .show(ctx, |ui| popup::panel(ui, theme, rect.width(), |ui| rows(ui, theme, keymap, command, lists, room)));
    ctx.move_to_top(shown.response.layer_id);
    let (pressed, picked) = shown.inner;
    // The keyboard stays in the field (a combobox): the row picked is its active descendant, and is said selected.
    if let Some(picked) = picked {
        ctx.accesskit_node_builder(field_id(), |node| node.set_active_descendant(picked.accesskit_id()));
    }
    asked.extend(pressed);
    asked
}

/// Says whether the row `response` is the one picked (`picked`), and keeps its id when it is.
fn said_picked(ui: &Ui, response: &egui::Response, picked: bool, kept: &mut Option<Id>) {
    ui.ctx().accesskit_node_builder(response.id, |node| node.set_selected(picked));
    if picked {
        *kept = Some(response.id);
    }
}

/// The results' rows for the mode typed, and the line of prefixes; what a press asked for, and the picked row's id.
fn rows(ui: &mut Ui, theme: &Theme, keymap: &Keymap, command: &mut Command, lists: &Lists, room: f32) -> (Option<Asked>, Option<Id>) {
    let picked = command.picked();
    let text = command.text().to_owned();
    let mut asked = None;
    let mut picked_id = None;
    let count = lists.count(command.mode());
    let counted = count.to_string();
    let nothing = count == 0 && command.mode() != Mode::Search && !text.is_empty();
    let mut lines = Lines::of(ui.ctx(), command.mode(), count, nothing);
    match command.mode() {
        Mode::Search => {}
        Mode::Command => {
            if lines.next() {
                // The frame `command-line/commands`: how many of all the commands show, and what Enter does.
                let of = tr!("{shown} of {all} · Enter runs the marked one", shown = count, all = lists.all_commands.max(count));
                popup::head(ui, theme, tr!("Commands"), Some(&of));
            }
            // The head, the line of prefixes and the plugin's row keep their room; the commands scroll in the rest.
            let most = (room - theme.boxes().menu_row * 3.0).max(theme.boxes().menu_row);
            let scrolled = egui::ScrollArea::vertical().id_salt("command-line-rows").max_height(most).auto_shrink([false, true]);
            (asked, picked_id) = crate::component::scroll::show(ui, theme, scrolled, |ui| {
                let (mut asked, mut picked_id) = (None, None);
                for (at, listed) in lists.commands.iter().enumerate() {
                    if !lines.next() {
                        continue;
                    }
                    let state = match (listed.refused, at == picked) {
                        (Some(_), _) => ItemState::Unavailable,
                        (None, true) => ItemState::Chosen,
                        (None, false) => ItemState::Default,
                    };
                    // Its id and whether it has a key, or why it cannot run; no key chip, so the row offers no action to
                    // the coming-soon gate ([`soon::blocked`]): a command listed here is not a control that offers it.
                    let id = listed.row.page.unwrap_or(listed.row.id);
                    let caption = match listed.refused {
                        Some(reason) => reason.to_owned(),
                        None if listed.row.page.is_none() && keymap.shortcut(listed.row.id).is_some() => id.to_owned(),
                        None => tr!("{id} · no key", id = id),
                    };
                    let item = Item { icon: Some(listed.row.icon), name: &listed.row.name, caption: Some(&caption), action: None, state };
                    let response = popup::menu_item(ui, theme, keymap, &item);
                    said_picked(ui, &response, at == picked, &mut picked_id);
                    if at == picked && command.stepped() {
                        ui.scroll_to_rect(response.rect, None);
                    }
                    if response.clicked() {
                        asked = chosen(command, lists, at);
                    }
                }
                (asked, picked_id)
            })
            .inner;
            // A plugin's command: planned, drawn coming soon.
            soon::soon(ui, theme, |ui| {
                if !lines.next() {
                    return;
                }
                let item = Item {
                    icon: Some(IconId::Keyboard),
                    name: tr!("A plugin's command"),
                    caption: Some(tr!("plugins")),
                    action: None,
                    state: ItemState::Default,
                };
                let _ = popup::menu_item(ui, theme, keymap, &item);
            });
        }
        Mode::Person => {
            if lines.next() {
                popup::head(ui, theme, tr!("People"), Some(&counted));
            }
            for (at, person) in lists.people.iter().enumerate().filter(|_| lines.next()) {
                let response = popup::suggestion_row(ui, theme, &person.name, &person.address, at == picked);
                said_picked(ui, &response, at == picked, &mut picked_id);
                if response.clicked() {
                    asked = chosen(command, lists, at);
                }
            }
        }
        Mode::Mailbox => {
            if lines.next() {
                popup::head(ui, theme, tr!("Mailboxes"), Some(&counted));
            }
            for (at, mailbox) in lists.mailboxes.iter().enumerate().filter(|_| lines.next()) {
                let state = if at == picked { ItemState::Chosen } else { ItemState::Default };
                let item = Item { icon: Some(IconId::Move), name: &mailbox.name, caption: Some(&mailbox.account), action: None, state };
                let response = popup::menu_item(ui, theme, keymap, &item);
                said_picked(ui, &response, at == picked, &mut picked_id);
                if response.clicked() {
                    asked = chosen(command, lists, at);
                }
            }
        }
    }
    if nothing && lines.next() {
        popup::empty(ui, theme, &tr!("Nothing matches “{text}”.", text = text));
    }
    if !lines.next() {
        return (asked, picked_id);
    }
    popup::hairline(ui, theme);
    // The prefixes, each a press away.
    ui.horizontal(|ui| {
        ui.add_space(theme.spacing.s3);
        ui.spacing_mut().item_spacing.x = theme.spacing.s4;
        for (key, word, prefix) in [(">", tr!("commands"), '>'), ("@", tr!("people"), '@'), ("#", tr!("mailboxes"), '#')] {
            if key_hint::key_hint(ui, theme, key, word, None).1 {
                command.drop_scope();
                command.set_text(&prefix.to_string());
                ui.ctx().memory_mut(|memory| memory.request_focus(field_id()));
            }
        }
    });
    (asked, picked_id)
}

/// The `#` results for `text`: every account's mailboxes whose path holds it (case left aside), at most
/// [`MOST_ROWS`].
pub(super) fn mailboxes(mailbox: &crate::mailbox::Mailbox, text: &str) -> Vec<Mailbox> {
    let wanted = text.trim().to_lowercase();
    mailbox
        .accounts()
        .iter()
        .flat_map(|account| mailbox.folders_of(account.id).into_iter().map(move |folder| (account, folder)))
        .filter(|(_, folder)| folder.name.to_lowercase().contains(&wanted))
        .take(MOST_ROWS)
        .map(|(account, folder)| Mailbox { id: folder.id, name: folder.name.clone(), account: account.name.clone() })
        .collect()
}

/// Why `id` cannot run here, in a few words: what it needs.
pub(super) fn refusal(id: &str) -> &'static str {
    match id {
        keymap::DONE | keymap::ARCHIVE | keymap::READ | keymap::FLAG | keymap::DELETE => tr!("no message is selected"),
        keymap::FULL_WIDTH | keymap::SNOOZE => tr!("no message is open"),
        keymap::UNDO => tr!("nothing to undo"),
        keymap::SEND | keymap::FOLD => tr!("no message is being written"),
        keymap::COMPOSE => tr!("no account yet"),
        keymap::OPEN_HELD => tr!("no message is held back"),
        _ => tr!("not here"),
    }
}

/// Whether a person shows for `text` in this mode: people are looked up only for a text typed.
pub(super) fn wants_people(command: &Command) -> bool {
    command.mode() == Mode::Person && !command.text().trim().is_empty() && command.people().is_none()
}

#[cfg(test)]
#[path = "command_motion_tests.rs"]
mod command_motion_tests;
