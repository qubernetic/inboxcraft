// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The reader's head: the toolbar of mail actions with its menus and the time picker, the subject and who wrote,
//! and what stands in their place over a message the screener holds.

use egui::{Align, Frame, Id, Layout, Rect, RichText, Sense, Stroke, Ui, vec2};

use crate::component::button::{self, IconButton, TextButton};
use crate::component::pill::pill;
use crate::component::time_picker::{self, Ask, Picked, Preset, TimePicker};
use crate::component::typed_time;
use crate::component::{badge, popup};
use crate::draw::margin;
use crate::draw::paint;
use crate::iconset::IconId;
use crate::keymap::{self, Keymap};
use crate::mailbox::{Act, After, GroupKind, Mailbox};
use crate::reader::{Opened, Reader};
use crate::theme::Theme;
use crate::tr;

use super::thread::bare;
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

/// The width of the More menu, in body text sizes.
const MENU_WIDTH: f32 = 18.0;

/// A thin upright line between groups of the toolbar.
fn divider(ui: &mut Ui, theme: &Theme) {
    let side = crate::component::control_side(theme);
    let (rect, _) = ui.allocate_exact_size(vec2(theme.spacing.s2, side), Sense::hover());
    ui.painter().vline(rect.center().x, rect.shrink2(vec2(0.0, side / 4.0)).y_range(), Stroke::new(1.0, theme.palette.line));
}

/// The More menu: flag and read or unread for the open message, full width, close, and the next-message rule.
fn more(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, reader: &Reader, drawn: &mut Drawn) {
    let p = &theme.palette;
    let open = reader.open_id().and_then(|id| mailbox.messages().iter().find(|row| row.id == id));
    if let Some(row) = open {
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
    if popup::entry(ui, theme, keymap, tr!("Close"), None).clicked() {
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
    let (Some(id), false) = (open, open.is_none_or(|id| mailbox.move_targets(id).is_empty())) else {
        button.unavailable(tr!("No folder to move it to")).show(ui, theme, keymap);
        return;
    };
    let response = button.show(ui, theme, keymap);
    let width = theme.fonts.body.size * MENU_WIDTH;
    popup::menu(ui.ctx(), theme, &response, move_menu(), width, |ui| {
        popup::title(ui, theme, tr!("Move to"), None);
        let mut chosen = false;
        for folder in mailbox.move_targets(id) {
            // The full path, so a folder inside another says where it is.
            let item = popup::Item { icon: None, name: &folder.name, caption: None, action: None, state: popup::ItemState::Default };
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

/// The toolbar of mail actions, then the subject under it with who wrote in the thread.
pub(super) fn toolbar(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, reader: &Reader, drawn: &mut Drawn) {
    let p = &theme.palette;
    let s = &theme.spacing;
    let open = reader.open_id();
    // The actions sit on a small card of their own; they wrap onto a second line when the text scale leaves no room.
    // In the top row the window controls sit at the end of the actions' line.
    crate::draw::frame::beside(ui, theme, |ui| {
        Frame::new().fill(p.surface).stroke(Stroke::new(1.0, p.line)).corner_radius(0.0).inner_margin(margin(s.s1)).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = s.s1;
                let mut action = |ui: &mut Ui, icon: IconId, name: &str, act: Option<(Act, &str)>| {
                    let button = IconButton::new(icon, name);
                    let button = match (act, open) {
                        (Some((act, key)), Some(id)) => {
                            if button.action(key).show(ui, theme, keymap).clicked() {
                                drawn.asked.push(Asked::Act(id, act));
                            }
                            return;
                        }
                        (Some((_, key)), None) => button.action(key),
                        (None, _) => button,
                    };
                    button.unavailable(not_yet()).show(ui, theme, keymap);
                };
                // A done message, open under "Show done", is brought back by the same button.
                let done = open.is_some_and(|id| mailbox.messages().iter().any(|row| row.id == id && row.done));
                let (name, act) = if done { (tr!("Not done"), Act::Undone) } else { (tr!("Done"), Act::Done) };
                action(ui, IconId::Done, name, Some((act, keymap::DONE)));
                action(ui, IconId::Archive, tr!("Archive"), Some((Act::Archive, keymap::ARCHIVE)));
                action(ui, IconId::Delete, tr!("Delete"), Some((Act::Delete, keymap::DELETE)));
                divider(ui, theme);
                picker(ui, theme, keymap, mailbox, open, drawn);
                pin(ui, theme, keymap, mailbox, open, drawn);
                move_to(ui, theme, keymap, mailbox, open, drawn);
                divider(ui, theme);
                let response = IconButton::new(IconId::More, tr!("More")).show(ui, theme, keymap);
                let width = theme.fonts.body.size * MENU_WIDTH;
                popup::menu(ui.ctx(), theme, &response, more_menu(), width, |ui| {
                    // Any row chosen closes it: each asks for something.
                    let before = drawn.asked.len();
                    more(ui, theme, keymap, mailbox, reader, drawn);
                    ((), drawn.asked.len() > before)
                });
            });
        });
    });
    let subject = reader
        .open_id()
        .and_then(|id| reader.opened(id))
        .map(|opened| opened.header.subject.clone())
        .filter(|subject| !subject.is_empty())
        .unwrap_or_else(|| tr!("(no subject)").to_owned());
    ui.add_space(s.s6);
    ui.add(egui::Label::new(RichText::new(subject).font(theme.fonts.title.id()).color(p.ink)).wrap());
    // Held mail is in no group until its sender is let in.
    let group = reader.open_id().filter(|id| !mailbox.held(*id)).and_then(|id| {
        let row = mailbox.messages().iter().find(|row| row.id == id);
        mailbox
            .groups()
            .into_iter()
            .find(|group| group.messages.contains(&id))
            .map(|group| group.name)
            .or_else(|| row.and_then(|row| row.group).map(|kind| kind.text().to_owned()))
    });
    let count = reader.thread().len();
    let chips = reader.open_id().map(|id| states(mailbox, id)).unwrap_or_default();
    // A message without a header: who wrote it stands here. Otherwise who wrote in the thread.
    let bare_id = reader.open_id().and_then(|id| reader.opened(id)).filter(|opened| bare(mailbox, reader, opened));
    let line = bare_id
        .and_then(|opened| from_line(mailbox, opened))
        .or_else(|| (count > 1).then(|| tr!("{writers} · {count} messages", writers = reader.writers().join(", "), count = count)));
    if group.is_some() || line.is_some() || !chips.is_empty() {
        // The message's group and its state as chips, then the line.
        let near = pointer_near(ui, theme.fonts.button.line + s.s2);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = s.s2;
            if let Some(group) = &group {
                badge::chip(ui, theme, group, &tr!("In {group}", group = group));
            }
            for (name, spoken) in &chips {
                badge::chip(ui, theme, name, spoken);
            }
            if let Some(line) = line {
                ui.add(egui::Label::new(RichText::new(line).font(theme.fonts.caption.id()).color(p.muted)).truncate());
            }
            if let Some(opened) = bare_id
                && quiet(ui, theme, keymap, tr!("Details"), ("details", opened.id), near).clicked()
            {
                drawn.asked.push(Asked::Details(opened.id));
            }
        });
    }
}

/// The sender's name, their address and the date, for a message without a header of its own.
fn from_line(mailbox: &Mailbox, opened: &Opened) -> Option<String> {
    let person = opened.header.from.first()?;
    let mut parts = vec![person.shown().to_owned()];
    if !person.name.is_empty() {
        parts.push(person.address.clone());
    }
    if let Some(date) = opened.header.date {
        parts.push(mailbox.short_date(date));
    }
    Some(parts.join(" · "))
}

/// Whether the pointer is over the `height` of `ui` to come.
pub(super) fn pointer_near(ui: &Ui, height: f32) -> bool {
    ui.rect_contains_pointer(Rect::from_min_size(ui.cursor().min, vec2(ui.available_width(), height)))
}

/// A link that is there for the keyboard and a screen reader all the time and shows when the pointer is `near` or it
/// has the focus.
pub(super) fn quiet(ui: &mut Ui, theme: &Theme, keymap: &Keymap, name: &str, salt: (&str, u64), near: bool) -> egui::Response {
    let focus = Id::unique("reader-quiet").with(salt);
    let focused = ui.data(|data| data.get_temp::<bool>(focus)).unwrap_or(false);
    let inner = ui.scope(|ui| {
        if !near && !focused {
            ui.multiply_opacity(0.0);
        }
        button::link(ui, theme, keymap, name)
    });
    let has_focus = inner.inner.has_focus();
    ui.data_mut(|data| data.insert_temp(focus, has_focus));
    inner.inner
}

/// What stands in place of the toolbar and the subject over a message the screener holds: the sender, their address,
/// when their first mail came and how many wait. Details is quiet until the pointer is near.
pub(super) fn sender_header(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, reader: &Reader, id: u64, drawn: &mut Drawn) {
    let p = &theme.palette;
    let f = &theme.fonts;
    let s = &theme.spacing;
    let Some(row) = mailbox.messages().iter().find(|row| row.id == id) else { return };
    let theirs = || mailbox.messages().iter().filter(|other| other.address == row.address);
    let first = theirs().filter_map(|other| other.date).min();
    let waiting = mailbox.waiting().iter().find(|waiting| waiting.address == row.address).map_or(0, |waiting| waiting.messages.len());
    let count = waiting.max(theirs().count());
    let mut parts = vec![row.address.clone()];
    if let Some(first) = first {
        parts.push(tr!("first mail {when}", when = mailbox.short_date(first)));
    }
    parts.push(if count == 1 { tr!("1 message waiting").to_owned() } else { tr!("{count} messages waiting", count = count) });
    let side = f.strong.line + f.caption.line;
    let near = pointer_near(ui, side);
    crate::draw::frame::beside(ui, theme, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = s.s3;
            crate::component::avatar::avatar(ui, theme, &row.sender, &row.address, side);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                // A reader that fills the window has no list beside it to go back to.
                if reader.full_width() && IconButton::new(IconId::Close, tr!("Leave full width")).show(ui, theme, keymap).clicked() {
                    drawn.asked.push(Asked::FullWidth(false));
                }
                if quiet(ui, theme, keymap, tr!("Details"), ("details", id), near).clicked() {
                    drawn.asked.push(Asked::Details(id));
                }
                ui.with_layout(Layout::top_down(Align::Min), |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.add(egui::Label::new(RichText::new(&row.sender).font(f.strong.id()).color(p.ink)).truncate());
                    ui.add(egui::Label::new(RichText::new(parts.join(" · ")).font(f.caption.id()).color(p.muted)).truncate());
                });
            });
        });
    });
}

/// The card over a message the screener holds: "Let <sender> in?", the groups to let them into (none chosen at first),
/// Accept, Accept and open, and Block, with what Block does; then a line that says the message is only a preview.
pub(super) fn screening(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, id: u64, drawn: &mut Drawn) {
    let p = &theme.palette;
    let s = &theme.spacing;
    let Some(row) = mailbox.messages().iter().find(|row| row.id == id) else { return };
    let chosen_id = egui::Id::unique("screen-into").with(id);
    let mut into = ui.data(|data| data.get_temp::<Option<GroupKind>>(chosen_id)).flatten();
    crate::draw::invite::edged(ui, theme, crate::draw::invite::Edge::Asks, |ui| {
        ui.spacing_mut().item_spacing.y = s.s3;
        let title = tr!("Let {sender} in?", sender = row.sender);
        ui.add(egui::Label::new(RichText::new(title).font(theme.fonts.heading.id()).color(p.ink)).wrap());
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = s.s1;
            crate::component::label(ui, theme.fonts.label, tr!("Into"), p.muted);
            for kind in GroupKind::ALL {
                if pill(ui, theme, kind.text(), into == Some(kind)).clicked() {
                    into = if into == Some(kind) { None } else { Some(kind) };
                    ui.data_mut(|data| data.insert_temp(chosen_id, into));
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = s.s2;
            if TextButton::new(tr!("Accept")).primary().icon(IconId::Done).show(ui, theme, keymap).clicked() {
                drawn.asked.push(Asked::Act(id, Act::Accept { group: into, open: false }));
            }
            if TextButton::new(tr!("Accept and open")).secondary().show(ui, theme, keymap).clicked() {
                drawn.asked.push(Asked::Act(id, Act::Accept { group: into, open: true }));
            }
            if TextButton::new(tr!("Block")).danger().icon(IconId::Failed).show(ui, theme, keymap).clicked() {
                drawn.asked.push(Asked::Act(id, Act::Block));
            }
        });
        let note = tr!("Blocked mail never loads remote content and never notifies.");
        ui.add(egui::Label::new(RichText::new(note).font(theme.fonts.caption.id()).color(p.muted)).wrap());
    });
    ui.label(RichText::new(tr!("Preview · nothing is accepted yet")).font(theme.fonts.caption.id()).color(p.muted));
}
