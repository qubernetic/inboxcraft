// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The reader's head: the toolbar of mail actions with its menus and the time picker, the subject and who wrote,
//! and what stands in their place over a message the screener holds.

use egui::{Align, Id, Layout, Rect, Role as Spoken, Sense, Ui, UiBuilder, WidgetInfo, vec2};
use inboxcraft_model::Role;

use crate::component::button::IconButton;
use crate::component::content_bar::{BarKind, ContentBar, View, content_bar};
use crate::component::facts::{Facts, facts_of};
use crate::component::popup;
use crate::component::tag::{TagKind, tag};
use crate::component::time_picker::{self, Ask, Picked, Preset, TimePicker};
use crate::component::typed_time;
use crate::component::why_line::why_line;
use crate::draw::paint;
use crate::iconset::IconId;
use crate::keymap::{self, Keymap};
use crate::mailbox::{Act, After, Mailbox};
use crate::reader::{Content, Header, Opened, Person, Reader};
use crate::theme::Theme;
use crate::tr;

use super::thread::addressed;
use super::{Asked, Drawn};

fn after_text(after: After) -> &'static str {
    match after {
        After::Next => tr!("Show the next message"),
        After::Previous => tr!("Show the previous message"),
        After::List => tr!("Go back to the list"),
    }
}

/// What the tooltip of an action without a command says.
fn not_yet() -> &'static str {
    tr!("Not available yet")
}

/// The width of the More menu, in points.
const MORE_WIDTH: f32 = 288.0;

/// The width of the Move menu, in points.
const MOVE_WIDTH: f32 = 256.0;

/// The room between the head's tools and between its tags, in points.
const TOOL_GAP: f32 = 8.0;

/// The lines the subject takes before it is cut.
const SUBJECT_LINES: usize = 2;

/// The More menu: archive, flag and read or unread for the open message, the rest of its acts, full width, close, and
/// the next-message rule.
fn more(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, reader: &Reader, drawn: &mut Drawn) {
    let p = &theme.palette;
    let open = reader.open_id().and_then(|id| mailbox.messages().iter().find(|row| row.id == id));
    if let Some(row) = open {
        if popup::item(ui, theme, keymap, IconId::Archive, tr!("Archive"), Some(keymap::ARCHIVE)).clicked() {
            drawn.asked.push(Asked::Act(row.id, Act::Archive));
        }
        let (name, act) = if row.flagged { (tr!("Take the flag off"), Act::Unflag) } else { (tr!("Flag"), Act::Flag) };
        if popup::item(ui, theme, keymap, IconId::Flag, name, Some(keymap::FLAG)).clicked() {
            drawn.asked.push(Asked::Act(row.id, act));
        }
        let (icon, name, act) =
            if row.seen { (IconId::Mail, tr!("Mark as unread"), Act::Unread) } else { (IconId::MarkRead, tr!("Mark as read"), Act::Read) };
        if popup::item(ui, theme, keymap, icon, name, Some(keymap::READ)).clicked() {
            drawn.asked.push(Asked::Act(row.id, act));
        }
        let (name, act) = if row.parked { (tr!("Take off the park pile"), Act::Park(false)) } else { (tr!("Park"), Act::Park(true)) };
        if popup::entry(ui, theme, keymap, name, None).clicked() {
            drawn.asked.push(Asked::Act(row.id, act));
        }
        if mailbox.is_snoozed(row.id) && popup::entry(ui, theme, keymap, tr!("Bring back now"), None).clicked() {
            drawn.asked.push(Asked::Act(row.id, Act::Unsnooze));
        }
        if row.follow_up.is_some() && popup::entry(ui, theme, keymap, tr!("Take the reminder off"), None).clicked() {
            drawn.asked.push(Asked::Act(row.id, Act::CancelFollowUp));
        }
        let name = if reader.shows_details(row.id) { tr!("Hide the full header") } else { tr!("Show the full header") };
        if popup::entry(ui, theme, keymap, name, None).clicked() {
            drawn.asked.push(Asked::Details(row.id));
        }
        paint::rule(ui, p.line);
        if row.group.is_some()
            && let Some(act) = crate::draw::list::group_menu(ui, theme, row.group, &row.address)
        {
            drawn.asked.push(Asked::Act(row.id, act));
        }
        if row.group.is_some() {
            paint::rule(ui, p.line);
        }
    }
    let (name, full) = if reader.full_width() { (tr!("Leave full width"), false) } else { (tr!("Full width"), true) };
    if popup::entry(ui, theme, keymap, name, None).clicked() {
        drawn.asked.push(Asked::FullWidth(full));
    }
    if popup::item(ui, theme, keymap, IconId::Close, tr!("Close"), None).clicked() {
        drawn.asked.push(Asked::Close);
    }
    paint::rule(ui, p.line);
    popup::title(ui, theme, tr!("When a message leaves the list"), None);
    for after in [After::Next, After::Previous, After::List] {
        let response = popup::choice(ui, theme, after_text(after), mailbox.after() == after);
        if response.clicked() {
            drawn.asked.push(Asked::After(after));
        }
    }
}

/// Move to a folder: a menu of the folders of the open message's account but its own; the one chosen moves it there.
fn move_to(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, open: Option<u64>, drawn: &mut Drawn) {
    let button = IconButton::new(IconId::Move, tr!("Move to a folder"));
    // The folders, read once a frame.
    let targets = open.map(|id| mailbox.move_targets(id)).unwrap_or_default();
    let (Some(id), false) = (open, targets.is_empty()) else {
        button.unavailable(tr!("No folder to move it to")).show(ui, theme, keymap);
        return;
    };
    let response = button.show(ui, theme, keymap);
    // The title's caption: the account and how many folders it offers.
    let account =
        mailbox.messages().iter().find(|row| row.id == id).and_then(|row| mailbox.accounts().iter().find(|account| account.id == row.account));
    let caption = account.map(|account| tr!("{account} · {count}", account = crate::component::spoken(&account.label), count = targets.len()));
    popup::menu_right(ui.ctx(), theme, &response, move_menu(), MOVE_WIDTH, |ui| {
        popup::title(ui, theme, tr!("Move to"), caption.as_deref());
        let mut chosen = false;
        for folder in targets {
            // The full path, so a folder inside another says where it is; the icon of what the folder is for.
            let icon = match folder.role {
                Some(Role::Archive) => IconId::Archive,
                Some(Role::Trash) => IconId::Delete,
                _ => IconId::Move,
            };
            let item = popup::Item { icon: Some(icon), name: &folder.name, caption: None, action: None, state: popup::ItemState::Default };
            if popup::menu_item(ui, theme, keymap, &item).clicked() {
                drawn.asked.push(Asked::Act(id, Act::Move(folder.id)));
                chosen = true;
            }
        }
        ((), chosen)
    });
}

/// The id of the Move menu.
pub(in crate::draw) fn move_menu() -> Id {
    Id::unique("reader-move-menu")
}

/// The id of the More menu.
pub(in crate::draw) fn more_menu() -> Id {
    Id::unique("reader-more-menu")
}

/// The id the time picker's state is kept under between frames and openings.
pub(in crate::draw) fn picker_id() -> Id {
    Id::unique("reader-time-picker")
}

/// The id of the time picker's popup, so a key can open it.
pub(in crate::draw) fn picker_popup() -> Id {
    Id::unique("reader-time-picker-popup")
}

/// Snooze, or "Remind me if no reply" on a message the owner sent: a button opening the time picker over the presets
/// still ahead; the time picked snoozes the message `id` or sets its reminder. Without a message it is disabled.
fn picker(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, open: Option<u64>, drawn: &mut Drawn) {
    let sent = open.is_some_and(|id| mailbox.is_sent(id));
    let (name, title, confirm) = if sent {
        (tr!("Remind me if no reply"), tr!("Remind me if no reply by"), tr!("Remind me"))
    } else {
        (tr!("Snooze"), tr!("Snooze until"), tr!("Snooze"))
    };
    let button = IconButton::new(IconId::Snooze, name).action(keymap::SNOOZE);
    let Some(id) = open else {
        button.unavailable(tr!("Open a message first")).show(ui, theme, keymap);
        return;
    };
    let response = button.show(ui, theme, keymap);
    let ctx = ui.ctx().clone();
    if popup::clicked(&ctx, &response, picker_popup()) {
        let open = !popup::is_open(&ctx, picker_popup());
        popup::set_open(&ctx, picker_popup(), open);
        if open {
            // A new opening: the presets read again, the text typed before gone.
            drawn.asked.push(Asked::Presets);
            ui.data_mut(|data| data.get_temp_mut_or_default::<TimePicker>(picker_id()).open());
        }
    }
    if !popup::is_open(&ctx, picker_popup()) {
        return;
    }
    let float = popup::Float { id: picker_popup(), anchor: response.rect, width: time_picker::WIDTH, returns_to: response.id };
    let shown = popup::float(&ctx, theme, &float, |ui| {
        let presets: Vec<Preset<'_>> = mailbox.presets().iter().map(|(name, at)| Preset { name, at: *at }).collect();
        let ask = Ask { title, confirm, alert: None, presets: &presets, extra: None, note: None };
        let mut state = ui.data_mut(|data| data.get_temp::<TimePicker>(picker_id())).unwrap_or_default();
        let picked = time_picker::show(ui, theme, &mut state, &ask, mailbox.now(), mailbox.offset());
        ui.data_mut(|data| data.insert_temp(picker_id(), state));
        picked
    });
    match shown {
        Some(Picked::At { at, .. }) => {
            drawn.asked.push(Asked::Act(id, if sent { Act::FollowUp(at) } else { Act::Snooze(at) }));
            popup::set_open(&ctx, picker_popup(), false);
            response.request_focus();
        }
        Some(Picked::Cancelled | Picked::Extra) => {
            popup::set_open(&ctx, picker_popup(), false);
            response.request_focus();
        }
        Some(Picked::Open) => {}
        None => popup::set_open(&ctx, picker_popup(), false),
    }
}

/// Pin, or "Take the pin off" (shown active) for a pinned message.
fn pin(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, open: Option<u64>, drawn: &mut Drawn) {
    let Some(row) = open.and_then(|id| mailbox.messages().iter().find(|row| row.id == id)) else {
        IconButton::new(IconId::Pin, tr!("Pin")).unavailable(tr!("Open a message first")).show(ui, theme, keymap);
        return;
    };
    let name = if row.pinned { tr!("Take the pin off") } else { tr!("Pin") };
    if IconButton::new(IconId::Pin, name).active(row.pinned).show(ui, theme, keymap).clicked() {
        drawn.asked.push(Asked::Act(row.id, Act::Pin(!row.pinned)));
    }
}

/// The chips of the open message's state: until when it is snoozed, when its reminder is due, that it came back.
fn states(mailbox: &Mailbox, id: u64) -> Vec<(String, String)> {
    let Some(row) = mailbox.messages().iter().find(|row| row.id == id) else { return Vec::new() };
    let (now, offset) = (mailbox.now(), mailbox.offset());
    let mut chips = Vec::new();
    if let Some(until) = row.snoozed_until {
        let short = typed_time::short(until, now, offset);
        chips.push((tr!("Snoozed until {when}", when = short), tr!("Snoozed until {when}", when = typed_time::long(until, now, offset))));
    }
    if let Some(due) = row.follow_up {
        let short = typed_time::short(due, now, offset);
        chips.push((tr!("Reminder {when}", when = short), tr!("Reminder if no reply by {when}", when = typed_time::long(due, now, offset))));
    }
    if mailbox.returned(row) {
        chips.push((tr!("Returned").to_owned(), tr!("Back from a snooze").to_owned()));
    }
    chips
}

/// The reader's head over the open message, as the design puts it from the top: its tags and its acts, the subject on
/// at most two lines, the facts of a message alone (who wrote and how many messages for a longer thread), why it is
/// here and what it holds. The caller sets the room between the parts.
pub(super) fn head(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, reader: &Reader, drawn: &mut Drawn) {
    let p = &theme.palette;
    let f = &theme.fonts;
    let open = reader.open_id();
    let opened = open.and_then(|id| reader.opened(id));
    crate::draw::frame::beside(ui, theme, |ui| tools(ui, theme, keymap, mailbox, reader, drawn));
    // A message the screener holds asks first whether to let its sender in; the rest is a preview.
    if let Some(id) = open.filter(|id| mailbox.held(*id)) {
        super::held::let_in(ui, theme, keymap, mailbox, id, drawn);
    }
    subject(ui, theme, keymap, opened, drawn);
    let count = reader.thread().len();
    match opened {
        Some(opened) if count <= 1 => facts(ui, theme, keymap, mailbox, reader, opened, drawn),
        _ => {
            let line = tr!("{writers}  ·  {count} messages", writers = reader.writers().join(", "), count = count);
            let width = ui.available_width();
            let galley = crate::component::fitted(ui, &line, f.mono_s, p.muted, width);
            let (rect, response) = ui.allocate_exact_size(vec2(width, f.mono_s.line), Sense::hover());
            response.widget_info(|| WidgetInfo::labeled(Spoken::Label, true, crate::component::spoken(&line)));
            paint::run_at(ui.painter(), rect.min, galley, p.muted);
        }
    }
    // Why the message is here is the engine's to say (`mail.why`, not built yet): the line stands with its tag and no
    // reason the window makes up.
    if open.is_some() {
        why_line(ui, theme, keymap, "");
    }
    if let Some(opened) = opened
        && let Some((kind, held)) = holds(reader, opened)
    {
        // An HTML message shows fitted or as plain text, as the owner chooses on the bar.
        let view = if reader.shows_plain(opened.id) { View::Plain } else { View::Fitted };
        let bar = ContentBar { kind, view, views: kind == BarKind::Html, held: &held, waiting: false, links: &[] };
        if let Some(view) = content_bar(ui, theme, keymap, &bar).view {
            drawn.asked.push(Asked::View(opened.id, view));
        }
    }
}

/// The tags of the open message: its group in the accent, its state (until when it is snoozed, when its reminder is due,
/// that it came back) and its account, each with what a screen reader hears.
fn tags(mailbox: &Mailbox, id: u64) -> Vec<(String, String, TagKind)> {
    let mut tags = Vec::new();
    let Some(row) = mailbox.messages().iter().find(|row| row.id == id) else { return tags };
    // Held mail is in no group until its sender is let in: it is a new sender's.
    if mailbox.held(id) {
        tags.push((tr!("New sender").to_owned(), tr!("New sender").to_owned(), TagKind::Accent));
    } else {
        let group = mailbox
            .groups()
            .into_iter()
            .find(|group| group.messages.contains(&id))
            .map(|group| group.name)
            .or_else(|| row.group.map(|kind| kind.text().to_owned()));
        tags.extend(group.map(|group| (group.clone(), group, TagKind::Accent)));
    }
    for (shown, spoken) in states(mailbox, id) {
        let kind = if shown == tr!("Returned") { TagKind::Warn } else { TagKind::Default };
        tags.push((shown, spoken, kind));
    }
    tags.extend(
        mailbox
            .accounts()
            .iter()
            .find(|account| account.id == row.account)
            .map(|account| (account.label.clone(), account.label.clone(), TagKind::Default)),
    );
    tags
}

/// How wide a tag of `words` is drawn: its words (at most 240) and 6 on each side.
fn tag_width(ui: &Ui, theme: &Theme, words: &str) -> f32 {
    crate::component::fitted(ui, words, theme.fonts.mono_s, theme.palette.muted, TAG_WIDEST).size().x.ceil().min(TAG_WIDEST) + 12.0
}

/// The widest a tag's words get, in points.
const TAG_WIDEST: f32 = 240.0;

/// The head's first line: the tags at the left, the acts at the right (Done, Snooze, Pin, Move, Delete and More). The
/// acts keep whole and the tags give way, a tag that does not fit left out whole. A pane narrower than the six acts
/// (a narrow window at 130 % text before the window takes one pane) stands them on lines of their own, as many on a line
/// as fit, against the right edge, so Delete and More are always there.
fn tools(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, reader: &Reader, drawn: &mut Drawn) {
    let side = crate::component::control_side(theme);
    let open = reader.open_id();
    // A held message's preview decides nothing: More alone stands at the right.
    let held = open.is_some_and(|id| mailbox.held(id));
    let width = ui.available_width();
    let line = |count: f32| count * side + (count - 1.0).max(0.0) * TOOL_GAP;
    let shown = if held { 1.0 } else { ACTS };
    if !held && line(ACTS) > width {
        let mut per = 1.0_f32;
        while per < ACTS && line(per + 1.0) <= width {
            per += 1.0;
        }
        let mut lines = 1.0_f32;
        while lines * per < ACTS {
            lines += 1.0;
        }
        let high = lines * side + (lines - 1.0) * TOOL_GAP;
        let (_, whole) = ui.allocate_space(vec2(width.max(0.0), high));
        let mut top = whole.top();
        let mut first = 0_usize;
        while first < ACT_COUNT {
            let count = per.min(ACTS - first as f32);
            let place = Rect::from_min_size(egui::pos2(whole.right() - line(per), top), vec2(line(per), side));
            let mut child = ui.new_child(UiBuilder::new().max_rect(place).layout(Layout::left_to_right(Align::Center)));
            child.spacing_mut().item_spacing.x = TOOL_GAP;
            let mut index = first;
            let mut placed = 0.0_f32;
            while placed < count && index < ACT_COUNT {
                act(&mut child, theme, keymap, mailbox, reader, drawn, index);
                index += 1;
                placed += 1.0;
            }
            first = index;
            top += side + TOOL_GAP;
        }
        return;
    }
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = TOOL_GAP;
        let room = (ui.available_width() - line(shown) - TOOL_GAP).max(0.0);
        let (_, place) = ui.allocate_space(vec2(room, side));
        let mut child = ui.new_child(UiBuilder::new().max_rect(place).layout(Layout::left_to_right(Align::Center)));
        child.set_clip_rect(place.intersect(ui.clip_rect()));
        child.spacing_mut().item_spacing.x = TOOL_GAP;
        let mut left = place.width();
        for (text, spoken, kind) in open.map(|id| tags(mailbox, id)).unwrap_or_default() {
            // A tag that does not fit whole is left out, never cut at the side.
            let words = crate::component::spoken(&text);
            let wide = tag_width(&child, theme, &words);
            if wide > left {
                continue;
            }
            left -= wide + TOOL_GAP;
            let drawn_tag = tag(&mut child, theme, &words, kind);
            drawn_tag.widget_info(|| WidgetInfo::labeled(Spoken::Label, true, crate::component::spoken(&spoken)));
        }
        if held {
            act(ui, theme, keymap, mailbox, reader, drawn, ACT_COUNT - 1);
        } else {
            acts(ui, theme, keymap, mailbox, reader, drawn);
        }
    });
}

/// How many acts the head's first line holds.
const ACTS: f32 = 6.0;

/// The acts on the open message, in their order: Done, Snooze, Pin, Move, Delete and More.
fn acts(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, reader: &Reader, drawn: &mut Drawn) {
    for index in 0..ACT_COUNT {
        act(ui, theme, keymap, mailbox, reader, drawn, index);
    }
}

/// The acts there are.
const ACT_COUNT: usize = 6;

/// The act `index` of [`acts`].
fn act(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, reader: &Reader, drawn: &mut Drawn, index: usize) {
    let open = reader.open_id();
    match index {
        0 => {
            // A done message, open under "Show done", is brought back by the same button.
            let done = open.is_some_and(|id| mailbox.messages().iter().any(|row| row.id == id && row.done));
            let (name, act) = if done { (tr!("Not done"), Act::Undone) } else { (tr!("Done"), Act::Done) };
            act_button(ui, theme, keymap, open, (IconId::Done, name, keymap::DONE), act, drawn);
        }
        1 => picker(ui, theme, keymap, mailbox, open, drawn),
        2 => pin(ui, theme, keymap, mailbox, open, drawn),
        3 => move_to(ui, theme, keymap, mailbox, open, drawn),
        4 => act_button(ui, theme, keymap, open, (IconId::Delete, tr!("Delete"), keymap::DELETE), Act::Delete, drawn),
        _ => {
            let response = IconButton::new(IconId::More, tr!("More")).show(ui, theme, keymap);
            popup::menu_right(ui.ctx(), theme, &response, more_menu(), MORE_WIDTH, |ui| {
                // Any row chosen closes it: each asks for something.
                let before = drawn.asked.len();
                more(ui, theme, keymap, mailbox, reader, drawn);
                ((), drawn.asked.len() > before)
            });
        }
    }
}

/// The button of an act on the open message: `icon`, `name` and the key of `action`; unavailable without a message.
fn act_button(
    ui: &mut Ui,
    theme: &Theme,
    keymap: &Keymap,
    open: Option<u64>,
    (icon, name, action): (IconId, &str, &str),
    act: Act,
    drawn: &mut Drawn,
) {
    let button = IconButton::new(icon, name).action(action);
    match open {
        Some(id) => {
            if button.show(ui, theme, keymap).clicked() {
                drawn.asked.push(Asked::Act(id, act));
            }
        }
        None => {
            button.unavailable(not_yet()).show(ui, theme, keymap);
        }
    }
}

/// The subject of the open message in `title`, on at most two lines, the second cut with one ellipsis; a screen reader
/// hears it whole, and the menu of a secondary press copies it whole. It is a Tab stop with its ring; Enter or the menu
/// key opens the same menu.
fn subject(ui: &mut Ui, theme: &Theme, keymap: &Keymap, opened: Option<&Opened>, drawn: &mut Drawn) {
    let p = &theme.palette;
    let f = &theme.fonts;
    let whole = opened.map(|opened| opened.header.subject.as_str()).filter(|subject| !subject.trim().is_empty()).unwrap_or(tr!("(no subject)"));
    let (kept, cut) = crate::component::capped(whole, crate::component::MOST);
    // A subject longer than the cap ends with the ellipsis even when its lines would hold what is kept.
    let kept = if cut { format!("{}…", kept.trim_end()) } else { kept.to_owned() };
    let width = ui.available_width().max(1.0);
    let galley = crate::component::wrapped(ui, &crate::component::plain(&kept), f.title, p.ink, width, SUBJECT_LINES, Align::LEFT);
    let (rect, response) = ui.allocate_exact_size(vec2(width, galley.size().y.max(f.title.line)), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(Spoken::Label, true, crate::component::spoken(whole)));
    let run = egui::Rect::from_min_size(rect.min, vec2(galley.size().x.ceil().min(width), rect.height()));
    if ui.is_rect_visible(rect) {
        paint::run_at(ui.painter(), rect.min, galley, p.ink);
        // A Tab stop: the ring around the words, as wide as they run.
        if response.has_focus() {
            paint::focus_ring(ui.painter(), theme, run, paint::Ring::Outside);
        }
    }
    // Enter opens the menu a secondary press opens (the menu key and Shift+F10 do too), so the copy is the keyboard's.
    if response.clicked() && !response.clicked_by(egui::PointerButton::Primary) {
        popup::open_by_key(ui.ctx(), &response, subject_menu());
    }
    let copied = popup::context_menu(ui.ctx(), theme, &response, subject_menu(), MOVE_WIDTH, |ui| {
        let copy = popup::entry(ui, theme, keymap, tr!("Copy the subject"), None).clicked();
        (copy, copy)
    });
    if copied == Some(true) {
        drawn.asked.push(Asked::CopySubject(whole.to_owned()));
    }
}

/// The id of the subject's menu.
fn subject_menu() -> Id {
    Id::unique("reader-subject-menu")
}

/// The facts of a message alone in its thread: who wrote, to whom and when, and all its addresses with the details
/// open; its link opens and closes them.
fn facts(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, reader: &Reader, opened: &Opened, drawn: &mut Drawn) {
    let Header { from, reply_to, to, cc, date, .. } = &opened.header;
    let full = addressed;
    let shown = |people: &[Person]| people.iter().map(Person::shown).collect::<Vec<_>>().join(", ");
    let from = if from.is_empty() { tr!("(no sender)").to_owned() } else { full(from) };
    let (to_line, cc, reply_to) = (shown(to), full(cc), full(reply_to));
    let date = date.map(|date| mailbox.full_date(date)).unwrap_or_default();
    let open = reader.shows_details(opened.id);
    let shown = Facts {
        from: &from,
        to: &to_line,
        date: &date,
        cc: &cc,
        reply_to: &reply_to,
        checked: "",
        id: "",
        recipients: Some(opened.header.to.len().saturating_add(opened.header.cc.len())),
    };
    if facts_of(ui, theme, keymap, &shown, open).clicked() {
        drawn.asked.push(Asked::Details(opened.id));
    }
}

/// What kind of message `opened` is and what was held back of it, for the content bar; nothing while its text is not
/// here (that state says so in the text's place).
fn holds(reader: &Reader, opened: &Opened) -> Option<(BarKind, String)> {
    let invite = opened.invite.is_some();
    match &opened.content {
        Content::Text(_) if invite => Some((BarKind::Invite, tr!("read into the card below").to_owned())),
        Content::Text(_) => Some((BarKind::Text, tr!("plain text, shown as sent").to_owned())),
        Content::Html(_) if invite => Some((BarKind::Invite, tr!("read into the card below").to_owned())),
        Content::Html(_) if reader.shows_plain(opened.id) => Some((BarKind::Html, tr!("as a text-only program shows it").to_owned())),
        Content::Html(document) => {
            let count = document.remote_blocked;
            let held = match (count, reader.remote_allowed(opened.id)) {
                (0, _) => tr!("nothing held back").to_owned(),
                (1, false) => tr!("1 picture held back").to_owned(),
                (count, false) => tr!("{count} pictures held back", count = count),
                (1, true) => tr!("1 remote picture loaded").to_owned(),
                (count, true) => tr!("{count} remote pictures loaded", count = count),
            };
            Some((BarKind::Html, held))
        }
        Content::Loading | Content::NotKept | Content::TooLarge | Content::Unreadable => None,
    }
}
