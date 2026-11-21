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

use crate::command::{Command, Found, Hit, Mode, field_id};
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

/// The most rows of each kind a search lists (the frame `command-line/typing-search`: "3 of 4").
const MOST_FOUND: usize = 3;

/// The height of the bar of prefixes under the results.
const PREFIXES_HIGH: f32 = 30.0;

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
    /// The lines of results of `rows` lines (heads and rows) in `mode`, as far as they have come in.
    fn of(ctx: &Context, mode: Mode, rows: usize, empty: bool) -> Self {
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
    /// A search with nothing typed: GO TO and LAST COMMANDS list.
    pub(super) go_to: bool,
    /// The GO TO rows offered: none over the event editor, as every one of them leaves the calendar.
    pub(super) goals: Vec<Goal>,
    /// A search with a text typed whose mail, people and mailboxes were looked up: [`Lists::found`] lists them.
    pub(super) searched: bool,
    /// What a search looked up.
    pub(super) found: Found,
    /// LAST COMMANDS: the commands run from the line, newest first.
    pub(super) last: Vec<Listed>,
}

/// Where a GO TO row goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Goal {
    Inbox,
    Senders,
    Calendar,
}

/// The GO TO rows, in their order.
pub(super) const GOALS: [Goal; 3] = [Goal::Inbox, Goal::Senders, Goal::Calendar];

/// A row of the results, by what it runs.
enum Entry<'a> {
    Go(Goal),
    Command(&'a Listed),
    Message(&'a Hit),
    Person(&'a Suggestion),
    Mailbox(u64),
}

impl Lists {
    /// How many rows the results hold for `mode`.
    fn count(&self, mode: Mode) -> usize {
        match mode {
            Mode::Search if self.go_to => self.goals.len().saturating_add(self.last.len()),
            Mode::Search if self.searched => self.found.mail.len().saturating_add(self.found.people.len()).saturating_add(self.found.mailboxes.len()),
            Mode::Search => 0,
            Mode::Command => self.commands.len(),
            Mode::Person => self.people.len(),
            Mode::Mailbox => self.mailboxes.len(),
        }
    }

    /// How many lines the rows of `mode` take, their heads counted.
    fn lines(&self, mode: Mode) -> usize {
        let count = self.count(mode);
        let heads = match mode {
            Mode::Search if self.go_to => usize::from(!self.goals.is_empty()) + usize::from(!self.last.is_empty()),
            Mode::Search => {
                [self.found.mail.len(), self.found.people.len(), self.found.mailboxes.len()].iter().filter(|rows| self.searched && **rows > 0).count()
            }
            // The head and the plugin's row.
            Mode::Command => 2,
            Mode::Person | Mode::Mailbox => 1,
        };
        count.saturating_add(heads)
    }

    /// Row `at` of the results of `mode`, from the top.
    fn entry(&self, mode: Mode, at: usize) -> Option<Entry<'_>> {
        match mode {
            Mode::Search if self.go_to => match self.goals.get(at) {
                Some(goal) => Some(Entry::Go(*goal)),
                None => self.last.get(at.checked_sub(self.goals.len())?).map(Entry::Command),
            },
            Mode::Search if self.searched => {
                let found = &self.found;
                let people = at.checked_sub(found.mail.len());
                let mailboxes = people.and_then(|at| at.checked_sub(found.people.len()));
                match (found.mail.get(at), people, mailboxes) {
                    (Some(hit), _, _) => Some(Entry::Message(hit)),
                    (None, Some(at), None) => found.people.get(at).map(Entry::Person),
                    (None, _, Some(at)) => found.mailboxes.get(at).map(|hit| Entry::Mailbox(hit.id)),
                    (None, None, _) => None,
                }
            }
            Mode::Search => None,
            Mode::Command => self.commands.get(at).map(Entry::Command),
            Mode::Person => self.people.get(at).map(Entry::Person),
            Mode::Mailbox => self.mailboxes.get(at).map(|mailbox| Entry::Mailbox(mailbox.id)),
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
    /// Go where a GO TO row goes.
    GoTo(Goal),
    /// Open the message a search found, the mail searched for the text shown.
    Message { id: u64, text: String },
    /// A command that cannot run here was chosen: the status line says why, in these words.
    Refused(String),
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
pub(super) fn results_id() -> Id {
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

/// The field as drawn this frame: what its typing asked for, its box and the command line's region.
pub(super) struct Typed {
    pub(super) asked: Vec<Asked>,
    rect: Rect,
    line: Rect,
}

/// Draws the command line's field into `ui` over `line` (the command line's region; `index` how far in from its left
/// the field starts, `room` what it leaves at the line's right end), `mailbox` the name of the mailbox shown for a
/// narrowed search; what its typing asked for. It comes before the results' rows are built, so that they are the rows
/// of the text as this frame's typing left it ([`pick`]).
pub(super) fn field(
    ui: &mut Ui,
    theme: &Theme,
    keymap: &Keymap,
    command: &mut Command,
    (line, index, room, mailbox): (Rect, f32, f32, &str),
) -> Typed {
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
    Typed { asked, rect, line }
}

/// The arrows, Enter and the results under the field `typed` drew, for `lists` (the rows of the text as it is now);
/// what was asked for.
pub(super) fn pick(ctx: &Context, theme: &Theme, keymap: &Keymap, command: &mut Command, typed: &Typed, lists: &Lists, early: Early) -> Vec<Asked> {
    let mut asked = Vec::new();
    if let Some(down) = early.step {
        command.step(down, lists.count(command.mode()));
    }
    if early.enter {
        asked.extend(chosen(command, lists, command.picked(), false));
    }
    if command.results() {
        asked.extend(results(ctx, theme, keymap, command, (typed.rect, typed.line), lists));
    }
    asked
}

/// What running row `at` of the results asks for, by Enter or by a press (`pressed`): a command that cannot run asks
/// for its reason in the status line.
fn chosen(command: &Command, lists: &Lists, at: usize, pressed: bool) -> Option<Asked> {
    let bare = command.text().trim().is_empty() && !command.stepped();
    // A bare prefix runs nothing, unless the arrows picked a row.
    if command.mode() != Mode::Search && bare {
        return None;
    }
    // Enter on an empty line goes nowhere (GO TO and LAST COMMANDS run on an arrow's pick or a press): what it did
    // before they were listed, the mail shown as it is, no place left.
    if command.mode() == Mode::Search && bare && !pressed {
        return Some(Asked::Find(String::new()));
    }
    match lists.entry(command.mode(), at) {
        Some(Entry::Go(goal)) => Some(Asked::GoTo(goal)),
        Some(Entry::Command(listed)) => Some(match listed.refused {
            Some(_) => Asked::Refused(refused_words(&listed.row.name, listed.row.page.unwrap_or(listed.row.id))),
            None => listed.row.page.map_or(Asked::Run(listed.row.id), Asked::Page),
        }),
        Some(Entry::Message(hit)) => Some(Asked::Message { id: hit.id, text: command.text().to_owned() }),
        Some(Entry::Person(person)) => Some(Asked::Person(person.address.clone())),
        Some(Entry::Mailbox(id)) => Some(Asked::Mailbox(id)),
        // A search that lists nothing shows the mail searched.
        None if command.mode() == Mode::Search => Some(Asked::Find(command.text().to_owned())),
        None => None,
    }
}

/// The dimmed window under the command line `line` and the results hanging under the field `rect`; what a press asked
/// for.
fn results(ctx: &Context, theme: &Theme, keymap: &Keymap, command: &mut Command, (rect, line): (Rect, Rect), lists: &Lists) -> Vec<Asked> {
    let mut asked = Vec::new();
    let screen = ctx.content_rect();
    // The window under the command line, dimmed from the line's foot as the frames draw it: a press on it closes the
    // results and reaches nothing under it. The line itself stays as it is: in the own frame the title bar, whose free
    // parts move the window while the results show.
    let dim = Rect::from_min_max(pos2(screen.left(), line.bottom()), screen.max);
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

/// The head over a group of `shown` rows of `total`: "3 of 4", or the count when all show; "3 of 1000+" when the
/// search read as many as it lists (`more`), so that there may be more.
fn group_head(ui: &mut Ui, theme: &Theme, label: &str, shown: usize, (total, more): (usize, bool)) {
    let counted = match (shown < total, more) {
        (true, false) => tr!("{shown} of {all}", shown = shown, all = total),
        (true, true) => tr!("{shown} of {all}+", shown = shown, all = total),
        (false, false) => total.to_string(),
        (false, true) => tr!("{all}+", all = total),
    };
    popup::head(ui, theme, label, Some(&counted));
}

/// A command's row: its id and whether it has a key, or why it cannot run. In the `>` list (`keyed` false) no key chip,
/// so the row offers no action to the coming-soon gate ([`soon::blocked`]): a command listed there is not a control
/// that offers it. In LAST COMMANDS (`keyed`) a command that ran and has a key shows its chip, as the frames draw it.
fn command_row(ui: &mut Ui, theme: &Theme, keymap: &Keymap, listed: &Listed, picked: bool, keyed: bool) -> egui::Response {
    let state = match (listed.refused, picked) {
        (Some(_), _) => ItemState::Unavailable,
        (None, true) => ItemState::Chosen,
        (None, false) => ItemState::Default,
    };
    let id = listed.row.page.unwrap_or(listed.row.id);
    let key = listed.row.page.is_none() && keymap.shortcut(listed.row.id).is_some();
    let (caption, action) = match listed.refused {
        Some(reason) => (Some(reason.to_owned()), None),
        None if keyed && key => (None, Some(listed.row.id)),
        None if key => (Some(id.to_owned()), None),
        None => (Some(tr!("{id} · no key", id = id)), None),
    };
    let item = Item { icon: Some(listed.row.icon), name: &listed.row.name, caption: caption.as_deref(), action, state };
    // A row of LAST COMMANDS recalls a command, it is not a control of it: its chip offers nothing to the coming-soon
    // gate, which a command listed there goes through as a `>` row does (its refusal).
    if keyed { popup::menu_item_unoffered(ui, theme, keymap, &item) } else { popup::menu_item(ui, theme, keymap, &item) }
}

/// A found message's or mailbox's row.
fn hit_row(ui: &mut Ui, theme: &Theme, keymap: &Keymap, hit: &Hit, picked: bool) -> egui::Response {
    let state = if picked { ItemState::Chosen } else { ItemState::Default };
    let item = Item { icon: Some(hit.icon), name: &hit.name, caption: Some(&hit.caption), action: None, state };
    popup::menu_item(ui, theme, keymap, &item)
}

/// A GO TO row: its icon, its name and what waits there.
fn goal_row(ui: &mut Ui, theme: &Theme, keymap: &Keymap, found: &Found, goal: Goal, picked: bool) -> egui::Response {
    let (icon, name, caption) = match goal {
        Goal::Inbox => (
            IconId::Inbox,
            tr!("Inbox"),
            found.to_read.map(|unread| if unread == 0 { tr!("nothing to read").to_owned() } else { tr!("{n} to read", n = unread) }),
        ),
        Goal::Senders => (IconId::People, tr!("New senders"), (found.waiting > 0).then(|| tr!("{n} waiting", n = found.waiting))),
        Goal::Calendar => {
            (IconId::Calendar, tr!("Calendar"), found.next.as_ref().map(|(title, time)| tr!("next: {title} {time}", title = title, time = time)))
        }
    };
    let state = if picked { ItemState::Chosen } else { ItemState::Default };
    let item = Item { icon: Some(icon), name, caption: caption.as_deref(), action: None, state };
    popup::menu_item(ui, theme, keymap, &item)
}

/// The walk down the rows of a search: the lines come in as [`Lines`] lets them, rows counted from the top for the pick.
struct Walk<'a> {
    lines: &'a mut Lines,
    at: usize,
    picked: usize,
    picked_id: Option<Id>,
    asked: Option<Asked>,
}

impl Walk<'_> {
    /// A head, when its line has come in.
    fn head(&mut self, add: impl FnOnce()) {
        if self.lines.next() {
            add();
        }
    }

    /// The next row, drawn by `draw` when its line has come in, and read.
    fn row(&mut self, ui: &mut Ui, command: &Command, lists: &Lists, draw: impl FnOnce(&mut Ui, bool) -> egui::Response) {
        let here = self.at;
        self.at = self.at.saturating_add(1);
        if !self.lines.next() {
            return;
        }
        let response = draw(ui, here == self.picked);
        said_picked(ui, &response, here == self.picked, &mut self.picked_id);
        if response.clicked() {
            self.asked = chosen(command, lists, here, true);
        }
    }
}

/// The rows of a search: with nothing typed GO TO and LAST COMMANDS, with a text the mail, the people and the
/// mailboxes found, each group under its head; what a press asked for, and the picked row's id.
fn search_rows(ui: &mut Ui, theme: &Theme, keymap: &Keymap, command: &Command, lists: &Lists, lines: &mut Lines) -> (Option<Asked>, Option<Id>) {
    let mut walk = Walk { lines, at: 0, picked: command.picked(), picked_id: None, asked: None };
    let found = &lists.found;
    if lists.go_to {
        if !lists.goals.is_empty() {
            walk.head(|| popup::head(ui, theme, tr!("Go to"), None));
        }
        for goal in lists.goals.iter().copied() {
            walk.row(ui, command, lists, |ui, picked| goal_row(ui, theme, keymap, found, goal, picked));
        }
        if !lists.last.is_empty() {
            walk.head(|| popup::head(ui, theme, tr!("Last commands"), None));
            for listed in &lists.last {
                walk.row(ui, command, lists, |ui, picked| command_row(ui, theme, keymap, listed, picked, true));
            }
        }
    } else if lists.searched {
        if !found.mail.is_empty() {
            walk.head(|| group_head(ui, theme, tr!("Mail"), found.mail.len(), (found.mail_total, found.mail_more)));
            for hit in &found.mail {
                walk.row(ui, command, lists, |ui, picked| hit_row(ui, theme, keymap, hit, picked));
            }
        }
        if !found.people.is_empty() {
            walk.head(|| group_head(ui, theme, tr!("People"), found.people.len(), (found.people_total, false)));
            for person in &found.people {
                walk.row(ui, command, lists, |ui, picked| popup::suggestion_row(ui, theme, &person.name, &person.address, picked));
            }
        }
        if !found.mailboxes.is_empty() {
            walk.head(|| group_head(ui, theme, tr!("Mailboxes"), found.mailboxes.len(), (found.mailboxes_total, false)));
            for hit in &found.mailboxes {
                walk.row(ui, command, lists, |ui, picked| hit_row(ui, theme, keymap, hit, picked));
            }
        }
    }
    (walk.asked, walk.picked_id)
}

/// The results' rows for the mode typed, and the line of prefixes; what a press asked for, and the picked row's id.
fn rows(ui: &mut Ui, theme: &Theme, keymap: &Keymap, command: &mut Command, lists: &Lists, room: f32) -> (Option<Asked>, Option<Id>) {
    let picked = command.picked();
    let text = command.text().to_owned();
    let mut asked = None;
    let mut picked_id = None;
    let count = lists.count(command.mode());
    let counted = count.to_string();
    let failed = lists.found.error.as_deref().filter(|_| command.mode() == Mode::Search && lists.searched);
    let nothing = failed.is_none() && count == 0 && !text.is_empty() && (command.mode() != Mode::Search || lists.searched);
    let mut lines = Lines::of(ui.ctx(), command.mode(), lists.lines(command.mode()), nothing || failed.is_some());
    match command.mode() {
        Mode::Search => (asked, picked_id) = search_rows(ui, theme, keymap, command, lists, &mut lines),
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
                    let response = command_row(ui, theme, keymap, listed, at == picked, false);
                    said_picked(ui, &response, at == picked, &mut picked_id);
                    if at == picked && command.stepped() {
                        ui.scroll_to_rect(response.rect, None);
                    }
                    if response.clicked() {
                        asked = chosen(command, lists, at, true);
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
                    asked = chosen(command, lists, at, true);
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
                    asked = chosen(command, lists, at, true);
                }
            }
        }
    }
    if let Some(error) = failed
        && lines.next()
    {
        popup::empty(ui, theme, &tr!("Search failed: {error}", error = error));
    } else if nothing && lines.next() {
        popup::empty(ui, theme, &tr!("Nothing matches “{text}”.", text = text));
    }
    if !lines.next() {
        return (asked, picked_id);
    }
    prefixes(ui, theme, command);
    (asked, picked_id)
}

/// The bar of prefixes under the rows, each a press away: a `panel_line` hairline, then 30 high on the `panel` fill,
/// as the frames draw it.
fn prefixes(ui: &mut Ui, theme: &Theme, command: &mut Command) {
    let width = ui.available_width();
    let (line, _) = ui.allocate_exact_size(egui::vec2(width, 1.0), Sense::hover());
    ui.painter().rect_filled(line, 0.0, theme.palette.panel_line);
    let (bar, _) = ui.allocate_exact_size(egui::vec2(width, PREFIXES_HIGH), Sense::hover());
    ui.painter().rect_filled(bar, 0.0, theme.palette.panel);
    let inner = bar.shrink2(egui::vec2(theme.spacing.s3, 0.0));
    ui.scope_builder(UiBuilder::new().max_rect(inner).layout(egui::Layout::left_to_right(egui::Align::Center)), |ui| {
        ui.spacing_mut().item_spacing.x = theme.spacing.s4;
        for (key, word, prefix) in [(">", tr!("commands"), '>'), ("@", tr!("people"), '@'), ("#", tr!("mailboxes"), '#')] {
            if key_hint::key_hint(ui, theme, key, word, None).1 {
                command.drop_scope();
                command.set_text(&prefix.to_string());
                ui.ctx().memory_mut(|memory| memory.request_focus(field_id()));
            }
        }
    });
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
        .map(|(account, folder)| Mailbox { id: folder.id, name: crate::home::kept(&folder.name), account: crate::home::kept(&account.name) })
        .collect()
}

/// What a search of the line finds for `text` (none: what GO TO says), from the mailbox: the messages as the list
/// searched for it finds them (`mail.search`, or the list's own rows when it shows that search), the people the mail
/// knows, the mailboxes whose path holds it; at most [`MOST_FOUND`] of each kept, with how many there were. Read
/// quietly: what the line looks up is not a command the owner ran ([`crate::mailbox::Mailbox::last_run`]).
pub(super) fn look_up(mailbox: &mut crate::mailbox::Mailbox, text: &str) -> Found {
    mailbox.quietly(|mailbox| looked_up(mailbox, text))
}

/// [`look_up`], read.
fn looked_up(mailbox: &mut crate::mailbox::Mailbox, text: &str) -> Found {
    if text.trim().is_empty() {
        return Found {
            to_read: mailbox.views().iter().find(|view| view.role == inboxcraft_model::Role::Inbox).map(|view| view.unread),
            waiting: mailbox.waiting().len(),
            next: crate::home::next_event(mailbox).map(|(time, title, _)| (title, time)),
            ..Found::default()
        };
    }
    let mut error = None;
    let (rows, more) = mailbox.searched(text).unwrap_or_else(|failed| {
        error = Some(failed.to_string());
        (Vec::new(), false)
    });
    let roles: std::collections::BTreeMap<u64, Option<inboxcraft_model::Role>> =
        mailbox.accounts().iter().flat_map(|account| mailbox.folders_of(account.id)).map(|folder| (folder.id, folder.role)).collect();
    let mail = rows
        .iter()
        .take(MOST_FOUND)
        .map(|row| {
            let subject = if row.subject.is_empty() { tr!("(no subject)").to_owned() } else { crate::home::kept(&row.subject) };
            let when = mailbox.when(row);
            let sender = crate::home::kept(&row.sender);
            let caption = if when.is_empty() { sender } else { tr!("{sender} · {when}", sender = sender, when = when) };
            Hit { id: row.id, name: subject, caption, icon: mail_icon(roles.get(&row.folder).copied().flatten()) }
        })
        .collect();
    let mut skip = std::collections::BTreeSet::new();
    skip.extend(mailbox.accounts().iter().map(|account| account.address.to_lowercase()));
    let people = crate::compose::people(mailbox, text, skip).unwrap_or_else(|failed| {
        error.get_or_insert_with(|| failed.to_string());
        Vec::new()
    });
    let boxes = mailboxes(mailbox, text);
    Found {
        mail,
        mail_total: rows.len(),
        mail_more: more,
        people_total: people.len(),
        people: people.into_iter().take(MOST_FOUND).collect(),
        mailboxes_total: boxes.len(),
        mailboxes: boxes
            .into_iter()
            .take(MOST_FOUND)
            .map(|mailbox| Hit { id: mailbox.id, name: mailbox.name, caption: mailbox.account, icon: IconId::Move })
            .collect(),
        // The error's own words, as the banner shows them, kept to a line without controls.
        error: error.map(|error| crate::home::kept(&error)),
        ..Found::default()
    }
}

/// The icon of a found message: its mailbox's.
fn mail_icon(role: Option<inboxcraft_model::Role>) -> IconId {
    use inboxcraft_model::Role;
    match role {
        Some(Role::Inbox) => IconId::Inbox,
        Some(Role::Sent) => IconId::Send,
        Some(Role::Trash) => IconId::Delete,
        Some(Role::Junk) => IconId::Junk,
        Some(Role::Archive) => IconId::Archive,
        _ => IconId::Move,
    }
}

/// What the status line says when the command `name` (`id`) was chosen where it cannot run: what it needs, and what
/// to do.
pub(super) fn refused_words(name: &str, id: &str) -> String {
    match id {
        keymap::DONE | keymap::ARCHIVE | keymap::READ | keymap::FLAG | keymap::DELETE => {
            tr!("Not run: {name} needs a selected message. Select one, then try again.", name = name)
        }
        keymap::FULL_WIDTH | keymap::SNOOZE => tr!("Not run: {name} needs an open message. Open one, then try again.", name = name),
        keymap::UNDO => tr!("Not run: {name}: there is nothing to undo.", name = name),
        keymap::SEND | keymap::FOLD => tr!("Not run: {name} needs a message being written.", name = name),
        keymap::COMPOSE => tr!("Not run: {name} needs an account. Add one, then try again.", name = name),
        keymap::OPEN_HELD => tr!("Not run: {name} needs a message held back.", name = name),
        _ => tr!("Not run: {name} cannot run here.", name = name),
    }
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
#[path = "command_last_tests.rs"]
mod command_last_tests;
#[cfg(test)]
#[path = "command_motion_tests.rs"]
mod command_motion_tests;
