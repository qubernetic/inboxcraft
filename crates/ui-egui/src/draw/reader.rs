// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The reader: a toolbar of mail actions (Done, Archive, Delete, Snooze, Pin, Move and More) over the subject, the
//! largest text of the window, which wraps rather than being cut, and who wrote in the thread. The thread follows as
//! cards, oldest first: the open message and the ones the owner unfolded in full (each message's header, the
//! remote-content banner, the body and the attachment tiles; Reply, Reply all and Forward on the open one), the rest
//! folded to one line. More holds flag and read or unread for the open message, then what is not a mail action: full
//! width, closing the reader and the next-message rule. Move to a folder opens a menu of the folders of the message's
//! account. While the pointer is on a link, a float at the bottom says where it goes.
//!
//! Done, Archive, Delete, Move and the acts in More act on the open message as the hover actions of the list do, on a
//! real account as on the sample; Reply, Reply all and Forward open the composer over a draft the engine filled in.
//! More also puts the sender's mail in another group, for a message of a grouped account.
//!
//! Snooze opens the time picker ("Snooze until"); on a message the owner sent the same button is "Remind me if no
//! reply" and its picker sets the follow-up reminder. Pin pins the message or takes its pin off. More parks it or
//! takes it off the park pile, brings a snoozed message back now and takes a reminder off. Under the subject, chips
//! say until when it is snoozed, when its reminder is due, and that it came back from a snooze.
//!
//! A message the screener holds opens as a preview that decides nothing: over it a card asks "Let <sender> in?", with
//! the groups to let them into, Accept, Accept and open, and Block.
//!
//! Drawing only reads the [`Reader`]; what the person asks for comes back as [`Asked`] and is applied after the frame.

use std::collections::HashMap;
use std::sync::Arc;

use egui::text::{LayoutJob, TextFormat};
use egui::{
    Align, Align2, Area, Color32, ColorImage, Frame, Galley, Grid, Id, Layout, Margin, Order, Popup, PopupCloseBehavior, Pos2, Rect, RichText,
    ScrollArea, Sense, Shape, Stroke, StrokeKind, TextureHandle, TextureOptions, Ui, WidgetInfo, pos2, vec2,
};
use inboxcraft_html::{Block, Document, Image, ImageSource, Marker, TextKind};

use super::margin;
use crate::component::button::{IconButton, TextButton};
use crate::component::pill::pill;
use crate::component::time_picker::{self, Ask, Picked, Preset, TimePicker};
use crate::component::typed_time;
use crate::component::{badge, popup};
use crate::compose::Answer;
use crate::glass::{self, Under};
use crate::iconset::{IconId, Style};
use crate::keymap::{self, Keymap};
use crate::mailbox::{Act, After, GroupKind, Mailbox};
use crate::picture::Decoded;
use crate::quote::Segment;
use crate::reader::{Content, File, Header, Opened, Person, PictureState, Reader, ThreadRow, inline_key, link_status, size_text};
use crate::theme::{self, Font, Theme};
use crate::tr;

/// How deep tables inside tables are drawn; deeper cells show nothing.
const MAX_DEPTH: usize = 4;

/// The tallest picture shown, in points.
const MAX_PICTURE_HEIGHT: f32 = 480.0;

/// What the person asked for in the reader this frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Asked {
    /// Unfold or fold a message of the thread.
    Expand(u64),
    /// Show or hide the full header.
    Details(u64),
    /// Show or hide the quotes.
    Quotes(u64),
    /// Load the remote pictures of this message once.
    Once(u64),
    /// Load the remote pictures of this sender always.
    Always(u64),
    /// Save an attachment.
    Save(u64, u64),
    /// Fill the window, or not.
    FullWidth(bool),
    /// Close the reader.
    Close,
    /// The next-message rule.
    After(After),
    /// Copy a link's address.
    Copy(String),
    /// Hide the notice.
    Dismiss,
    /// Do `Act` to a message.
    Act(u64, Act),
    /// Write a draft made from a message.
    Answer(u64, Answer),
    /// A time picker opened: read the time presets still ahead.
    Presets,
}

/// What the reader drew: what was asked for, and the status of the link under the pointer.
#[derive(Debug, Default)]
pub(super) struct Drawn {
    pub(super) asked: Vec<Asked>,
    pub(super) hover: Option<String>,
}

/// The textures of the pictures shown, by picture key.
pub(super) type Textures = HashMap<String, TextureHandle>;

fn after_text(after: After) -> &'static str {
    match after {
        After::Next => tr!("Show the next message"),
        After::Previous => tr!("Show the previous message"),
        After::List => tr!("Go back to the list"),
    }
}

fn names(people: &[Person]) -> String {
    people.iter().map(Person::shown).collect::<Vec<_>>().join(", ")
}

/// What the tooltip of an action without a command says.
fn not_yet() -> &'static str {
    tr!("Not available yet")
}

/// The width of the More menu, in body text sizes.
const MENU_WIDTH: f32 = 18.0;

/// A thin upright line between groups of the toolbar.
fn divider(ui: &mut Ui, theme: &Theme) {
    let side = crate::component::control_side(ui, theme);
    let (rect, _) = ui.allocate_exact_size(vec2(theme.spacing.s, side), Sense::hover());
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
        ui.add_space(theme.spacing.xs);
        ui.separator();
        if row.group.is_some()
            && let Some(act) = super::list::group_menu(ui, theme, row.group, &row.address)
        {
            drawn.asked.push(Asked::Act(row.id, act));
        }
        if row.group.is_some() {
            ui.add_space(theme.spacing.xs);
            ui.separator();
        }
    }
    let (name, full) = if reader.full_width() { (tr!("Leave full width"), false) } else { (tr!("Full width"), true) };
    if popup::entry(ui, theme, keymap, name, None).clicked() {
        drawn.asked.push(Asked::FullWidth(full));
    }
    if popup::entry(ui, theme, keymap, tr!("Close"), None).clicked() {
        drawn.asked.push(Asked::Close);
    }
    ui.add_space(theme.spacing.xs);
    ui.separator();
    ui.label(RichText::new(tr!("When a message leaves the list")).font(theme.fonts.caption.id()).color(p.muted));
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
    Popup::menu(&response).frame(Frame::NONE).show(|ui| {
        popup::panel(ui, theme, Under::Sharp, width, |ui| {
            ui.label(RichText::new(tr!("Move to")).font(theme.fonts.caption.id()).color(theme.palette.muted));
            for folder in mailbox.move_targets(id) {
                // The full path, so a folder inside another says where it is.
                if popup::entry(ui, theme, keymap, &folder.name, None).clicked() {
                    drawn.asked.push(Asked::Act(id, Act::Move(folder.id)));
                }
            }
        })
    });
}

/// The width of the time picker, in body text sizes.
const PICKER_WIDTH: f32 = 22.0;

/// The id the time picker's state is kept under between frames and openings.
pub(super) fn picker_id() -> Id {
    Id::unique("reader-time-picker")
}

/// The id of the time picker's popup, so a key can open it.
pub(super) fn picker_popup() -> Id {
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
    if response.clicked() {
        // A new opening: the presets read again, the text typed before gone.
        drawn.asked.push(Asked::Presets);
        ui.data_mut(|data| data.get_temp_mut_or_default::<TimePicker>(picker_id()).open());
    }
    let width = theme.fonts.body.size * PICKER_WIDTH;
    Popup::menu(&response).id(picker_popup()).close_behavior(PopupCloseBehavior::CloseOnClickOutside).frame(Frame::NONE).show(|ui| {
        popup::panel(ui, theme, Under::Sharp, width, |ui| {
            let presets: Vec<Preset<'_>> = mailbox.presets().iter().map(|(name, at)| Preset { name, at: *at }).collect();
            let ask = Ask { title, confirm, alert: None, presets: &presets };
            let mut state = ui.data_mut(|data| data.get_temp::<TimePicker>(picker_id())).unwrap_or_default();
            let picked = time_picker::show(ui, theme, &mut state, &ask, mailbox.now(), mailbox.offset());
            ui.data_mut(|data| data.insert_temp(picker_id(), state));
            match picked {
                Picked::At { at, .. } => {
                    drawn.asked.push(Asked::Act(id, if sent { Act::FollowUp(at) } else { Act::Snooze(at) }));
                    ui.close();
                }
                Picked::Cancelled => ui.close(),
                Picked::Open => {}
            }
        })
    });
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
fn toolbar(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, reader: &Reader, drawn: &mut Drawn) {
    let p = &theme.palette;
    let s = &theme.spacing;
    let open = reader.open_id();
    // The actions sit on a small card of their own; they wrap onto a second line when the text scale leaves no room.
    // In the top row the window controls sit at the end of the actions' line.
    super::frame::beside(ui, theme, |ui| {
        Frame::new().fill(p.surface).stroke(Stroke::new(1.0, p.line)).corner_radius(s.card).inner_margin(margin(s.xs)).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = s.xs;
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
                Popup::menu(&response)
                    .frame(Frame::NONE)
                    .show(|ui| popup::panel(ui, theme, Under::Sharp, width, |ui| more(ui, theme, keymap, mailbox, reader, drawn)));
            });
        });
    });
    let subject = reader
        .open_id()
        .and_then(|id| reader.opened(id))
        .map(|opened| opened.header.subject.clone())
        .filter(|subject| !subject.is_empty())
        .unwrap_or_else(|| tr!("(no subject)").to_owned());
    ui.add_space(s.xl);
    ui.add(egui::Label::new(RichText::new(subject).font(theme.fonts.display.id()).color(p.text)).wrap());
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
    if group.is_some() || count > 1 || !chips.is_empty() {
        // The message's group and its state as chips, then who wrote in the thread.
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = s.s;
            if let Some(group) = &group {
                badge::chip(ui, theme, group, &tr!("In {group}", group = group));
            }
            for (name, spoken) in &chips {
                badge::chip(ui, theme, name, spoken);
            }
            if count > 1 {
                let writers = reader.writers().join(", ");
                let line = tr!("{writers} · {count} messages", writers = writers, count = count);
                ui.add(egui::Label::new(RichText::new(line).font(theme.fonts.caption.id()).color(p.muted)).truncate());
            }
        });
    }
}

/// A card's frame: the surface with a line round it.
fn card_frame(theme: &Theme, inner: f32) -> Frame {
    Frame::new()
        .fill(theme.palette.surface)
        .stroke(Stroke::new(1.0, theme.palette.line))
        .corner_radius(theme.spacing.card)
        .inner_margin(margin(inner))
}

/// One folded message of the thread, a card of one line: the avatar, the sender, the first words and the date; a
/// click unfolds it.
fn folded(ui: &mut Ui, theme: &Theme, mailbox: &Mailbox, row: &ThreadRow, drawn: &mut Drawn) {
    let p = &theme.palette;
    let s = &theme.spacing;
    let date = row.date.map(|date| mailbox.short_date(date)).unwrap_or_default();
    let spoken = tr!("{sender}, {date}, folded: {preview}", sender = row.sender, date = date, preview = row.preview);
    let f = &theme.fonts;
    let line = super::paint::line_height(ui, f.sender).max(super::paint::line_height(ui, f.caption));
    let side = line * 1.5;
    let height = side + s.m * 2.0;
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(egui::Role::Button, true, &spoken));
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        painter.rect_filled(rect, s.card, p.surface);
        if response.hovered() {
            painter.rect_filled(rect, s.card, p.hover);
        }
        painter.rect_stroke(rect, s.card, Stroke::new(1.0, p.line), StrokeKind::Inside);
        let face = Rect::from_min_size(pos2(rect.left() + s.m, rect.center().y - side / 2.0), vec2(side, side));
        crate::component::avatar::paint(painter, theme, face, &row.sender, &row.address);
        let left = face.right() + s.m;
        let date_galley = super::paint::line(ui, &[(&date, f.caption, p.muted)], f32::INFINITY);
        let date_left = rect.right() - s.l - date_galley.size().x;
        ui.painter().galley(pos2(date_left, rect.center().y - date_galley.size().y / 2.0), date_galley, p.muted);
        let preview = format!("   {}", row.preview);
        let text = super::paint::line(ui, &[(&row.sender, f.sender_unread, p.text), (&preview, f.preview, p.muted)], date_left - s.m - left);
        ui.painter().galley(pos2(left, rect.center().y - text.size().y / 2.0), text, p.text);
        super::paint::focus_ring(ui, theme, &response);
    }
    if response.clicked() {
        drawn.asked.push(Asked::Expand(row.id));
    }
}

/// The header: the sender and the date, whom it went to, and the full header when asked.
fn header(ui: &mut Ui, theme: &Theme, mailbox: &Mailbox, reader: &Reader, opened: &Opened, drawn: &mut Drawn) {
    let p = &theme.palette;
    let f = &theme.fonts;
    let Header { from, reply_to, to, cc, date, .. } = &opened.header;
    // The date and the buttons first, at the right; the avatar, the sender and the address then share what is left,
    // cut with an ellipsis rather than running into the date.
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let text = if reader.shows_details(opened.id) { tr!("Hide details") } else { tr!("Details") };
            if ui.button(text).clicked() {
                drawn.asked.push(Asked::Details(opened.id));
            }
            if Some(opened.id) != reader.open_id() && ui.button(tr!("Fold")).clicked() {
                drawn.asked.push(Asked::Expand(opened.id));
            }
            if let Some(date) = date {
                ui.label(RichText::new(mailbox.short_date(*date)).font(f.caption.id()).color(p.muted));
            }
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                if let Some(person) = from.first() {
                    let side = super::paint::line_height(ui, f.heading) * 1.5;
                    crate::component::avatar::avatar(ui, theme, &person.name, &person.address, side);
                }
                let sender = from.first().map_or_else(|| tr!("(no sender)").to_owned(), |person| person.shown().to_owned());
                ui.add(egui::Label::new(RichText::new(sender).font(f.heading.id()).color(p.text)).truncate());
                if let Some(address) = from.first().filter(|person| !person.name.is_empty()).map(|person| person.address.clone()) {
                    ui.add(egui::Label::new(RichText::new(address).font(f.caption.id()).color(p.muted)).truncate());
                }
            });
        });
    });
    if !reader.shows_details(opened.id) {
        let mut whom = names(to);
        if !cc.is_empty() {
            whom = tr!("{to}, copy to {cc}", to = whom, cc = names(cc));
        }
        ui.label(RichText::new(tr!("to {whom}", whom = whom)).font(f.caption.id()).color(p.muted));
        return;
    }
    Grid::new(("details", opened.id)).num_columns(2).spacing(vec2(theme.spacing.m, theme.spacing.xs)).show(ui, |ui| {
        let row = |ui: &mut Ui, name: &str, value: String| {
            ui.label(RichText::new(name).font(f.caption.id()).color(p.muted));
            ui.label(RichText::new(value).font(f.caption.id()).color(p.text));
            ui.end_row();
        };
        let full = |people: &[Person]| people.iter().map(Person::full).collect::<Vec<_>>().join(", ");
        row(ui, tr!("From"), full(from));
        if !reply_to.is_empty() {
            row(ui, tr!("Reply to"), full(reply_to));
        }
        row(ui, tr!("To"), full(to));
        if !cc.is_empty() {
            row(ui, tr!("Copy"), full(cc));
        }
        if let Some(date) = date {
            row(ui, tr!("Date"), mailbox.full_date(*date));
        }
    });
}

/// The remote-content banner, while remote pictures of the message are blocked.
fn banner(ui: &mut Ui, theme: &Theme, reader: &Reader, opened: &Opened, drawn: &mut Drawn) {
    let Content::Html(document) = &opened.content else { return };
    if document.remote_blocked == 0 || reader.remote_allowed(opened.id) {
        return;
    }
    let p = &theme.palette;
    Frame::new().fill(p.panel).inner_margin(margin(theme.spacing.m)).corner_radius(theme.spacing.corner).show(ui, |ui| {
        ui.set_width(ui.available_width());
        let count = document.remote_blocked;
        let text = if count == 1 {
            tr!("1 remote picture is not shown. Loading it tells the sender that you opened this message.").to_owned()
        } else {
            tr!("{count} remote pictures are not shown. Loading them tells the sender that you opened this message.", count = count)
        };
        ui.label(RichText::new(text).font(theme.fonts.caption.id()).color(p.text));
        ui.horizontal(|ui| {
            if ui.button(tr!("Load once")).clicked() {
                drawn.asked.push(Asked::Once(opened.id));
            }
            if let Some(sender) = opened.header.from.first()
                && ui.button(tr!("Always for {address}", address = sender.address)).clicked()
            {
                drawn.asked.push(Asked::Always(opened.id));
            }
        });
    });
}

/// A run of text as one wrapped galley, its links found under the pointer.
struct Run {
    job: LayoutJob,
    /// Per section, its characters (start, end) and its link.
    links: Vec<(usize, usize, Option<usize>)>,
    chars: usize,
    text: String,
}

impl Run {
    fn new(width: f32) -> Self {
        let mut job = LayoutJob::default();
        job.wrap.max_width = width.max(1.0);
        Self { job, links: Vec::new(), chars: 0, text: String::new() }
    }

    fn push(&mut self, text: &str, format: TextFormat, link: Option<usize>) {
        let count = text.chars().count();
        self.links.push((self.chars, self.chars.saturating_add(count), link));
        self.chars = self.chars.saturating_add(count);
        self.text.push_str(text);
        self.job.append(text, 0.0, format);
    }

    /// Draws the run; a link under the pointer sets the status and a click on it asks for its address to be copied.
    fn show(self, ui: &mut Ui, document: &Document, drawn: &mut Drawn) {
        if self.text.is_empty() {
            return;
        }
        let galley: Arc<Galley> = ui.painter().layout_job(self.job);
        let (rect, response) = ui.allocate_exact_size(galley.size(), Sense::click());
        let spoken = self.text.clone();
        response.widget_info(|| WidgetInfo::labeled(egui::Role::Label, true, &spoken));
        let under = response.hover_pos().map(|pos| galley.cursor_from_pos(pos - rect.min).index.0);
        let link = under.and_then(|at| self.links.iter().find(|(start, end, _)| (*start..*end).contains(&at)).and_then(|(_, _, link)| *link));
        let link = link.and_then(|index| document.links.get(index));
        if let Some(link) = link {
            drawn.hover = Some(link_status(link));
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            if response.clicked() {
                match &link.target {
                    inboxcraft_html::LinkTarget::Web { url, .. } | inboxcraft_html::LinkTarget::Mail { url, .. } => {
                        drawn.asked.push(Asked::Copy(url.clone()))
                    }
                    inboxcraft_html::LinkTarget::Refused => {}
                }
            }
        }
        if ui.is_rect_visible(rect) {
            ui.painter().galley(rect.min, galley, Color32::PLACEHOLDER);
        }
    }
}

fn texture<'t>(ui: &Ui, textures: &'t mut Textures, key: &str, picture: &Decoded) -> Option<&'t TextureHandle> {
    if picture.rgba.len() != picture.width.saturating_mul(picture.height).saturating_mul(4) {
        return None;
    }
    if !textures.contains_key(key) {
        let image = ColorImage::from_rgba_unmultiplied([picture.width, picture.height], &picture.rgba);
        textures.insert(key.to_owned(), ui.ctx().load_texture(key, image, TextureOptions::LINEAR));
    }
    textures.get(key)
}

/// A picture of the message: shown when here, else a box that says why not.
#[expect(clippy::too_many_arguments, reason = "the drawing context of one picture")]
fn picture(ui: &mut Ui, theme: &Theme, reader: &Reader, id: u64, image: &Image, document: &Document, textures: &mut Textures, drawn: &mut Drawn) {
    let p = &theme.palette;
    let key = match &image.source {
        ImageSource::Inline(content_id) => Some(inline_key(id, content_id)),
        ImageSource::Remote(url) => Some(url.clone()),
        ImageSource::Blocked | ImageSource::Unsupported => None,
    };
    let state = key.as_deref().and_then(|key| reader.picture(key));
    if let (Some(key), Some(PictureState::Ready(decoded))) = (key.as_deref(), state)
        && let Some(handle) = texture(ui, textures, key, decoded)
    {
        let (w, h) = (decoded.width as f32, decoded.height as f32);
        let scale = (ui.available_width() / w).min(MAX_PICTURE_HEIGHT / h).min(1.0);
        let (rect, response) = ui.allocate_exact_size(vec2(w * scale, h * scale), Sense::click());
        let alt = if image.alt.is_empty() { tr!("Picture").to_owned() } else { image.alt.clone() };
        response.widget_info(|| WidgetInfo::labeled(egui::Role::Image, true, &alt));
        ui.painter().image(handle.id(), rect, Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)), theme::NO_TINT);
        if let Some(link) = image.link.and_then(|index| document.links.get(index))
            && response.hovered()
        {
            drawn.hover = Some(link_status(link));
        }
        return;
    }
    let why = match (&image.source, state) {
        (ImageSource::Unsupported, _) if image.alt.is_empty() => return,
        (ImageSource::Unsupported, _) => tr!("Picture not shown").to_owned(),
        (ImageSource::Blocked, _) => tr!("Remote picture not loaded").to_owned(),
        (_, Some(PictureState::Failed(reason))) => tr!("Picture could not be shown: {reason}", reason = reason),
        (_, Some(PictureState::Waiting) | None) => tr!("Loading picture…").to_owned(),
        (_, Some(PictureState::Ready(_))) => tr!("Picture could not be shown").to_owned(),
    };
    Frame::new().stroke(Stroke::new(1.0, p.muted)).inner_margin(margin(theme.spacing.s)).corner_radius(theme.spacing.corner).show(ui, |ui| {
        let text = if image.alt.is_empty() { why } else { format!("{why}: {}", image.alt) };
        ui.label(RichText::new(text).font(theme.fonts.caption.id()).color(p.muted));
    });
}

/// Display blocks of an HTML message.
#[expect(clippy::too_many_arguments, reason = "the drawing context of one block")]
fn blocks(
    ui: &mut Ui,
    theme: &Theme,
    reader: &Reader,
    id: u64,
    document: &Document,
    list: &[Block],
    depth: usize,
    textures: &mut Textures,
    drawn: &mut Drawn,
) {
    let p = &theme.palette;
    let f = &theme.fonts;
    let quotes = reader.shows_quotes(id);
    let mut toggled = false;
    for (index, block) in list.iter().enumerate() {
        let quote = match block {
            Block::Text { quote, .. } | Block::Rule { quote } | Block::Image { quote, .. } | Block::Table { quote, .. } => *quote,
        };
        if quote > 0 && !toggled {
            toggled = true;
            let text = if quotes { tr!("Hide quoted text") } else { tr!("Show quoted text") };
            if ui.button(text).clicked() {
                drawn.asked.push(Asked::Quotes(id));
            }
        }
        if quote == 0 {
            toggled = false;
        } else if !quotes {
            continue;
        }
        let indent = f32::from(quote.min(4)) * theme.spacing.l;
        ui.horizontal(|ui| {
            ui.add_space(indent);
            ui.vertical(|ui| match block {
                Block::Text { kind, spans, .. } => {
                    let (font, prefix) = match kind {
                        TextKind::Heading(level) => (if *level <= 2 { f.title } else { f.heading }, String::new()),
                        TextKind::ListItem { depth, marker } => {
                            let pad = "    ".repeat(usize::from((*depth).clamp(1, 6)).saturating_sub(1));
                            let mark = match marker {
                                Marker::Bullet => "•".to_owned(),
                                Marker::Number(n) => format!("{n}."),
                            };
                            (f.body, format!("{pad}{mark} "))
                        }
                        TextKind::Preformatted => (f.mono, String::new()),
                        TextKind::Paragraph => (f.body, String::new()),
                    };
                    let colour = if quote > 0 { p.muted } else { p.text };
                    let mut run = Run::new(ui.available_width());
                    if !prefix.is_empty() {
                        run.push(&prefix, TextFormat::simple(font.id(), colour), None);
                    }
                    for span in spans {
                        // Code is monospaced, bold text a heavier weight, both at the size of the block.
                        let face = if span.code {
                            f.mono.face
                        } else if span.bold && matches!(kind, TextKind::Paragraph | TextKind::ListItem { .. }) {
                            f.strong.face
                        } else {
                            font.face
                        };
                        let span_font = Font { face, ..font };
                        let mut format = TextFormat::simple(span_font.id(), if span.link.is_some() { p.accent } else { colour });
                        format.italics = span.italic;
                        if span.underline || span.link.is_some() {
                            format.underline = Stroke::new(1.0, if span.link.is_some() { p.accent } else { colour });
                        }
                        if span.strike {
                            format.strikethrough = Stroke::new(1.0, colour);
                        }
                        if span.bold && span.link.is_none() {
                            format.color = p.text;
                        }
                        run.push(&span.text, format, span.link);
                    }
                    run.show(ui, document, drawn);
                }
                Block::Rule { .. } => {
                    ui.separator();
                }
                Block::Image { image, .. } => picture(ui, theme, reader, id, image, document, textures, drawn),
                Block::Table { rows, .. } if depth < MAX_DEPTH => {
                    Grid::new(("table", id, depth, index)).striped(false).show(ui, |ui| {
                        for row in rows {
                            for cell in row {
                                ui.vertical(|ui| blocks(ui, theme, reader, id, document, &cell.blocks, depth.saturating_add(1), textures, drawn));
                            }
                            ui.end_row();
                        }
                    });
                }
                Block::Table { .. } => {}
            });
        });
        ui.add_space(theme.spacing.s);
    }
}

/// A plain-text body, its quotes folded until the owner opens them.
fn text(ui: &mut Ui, theme: &Theme, reader: &Reader, id: u64, segments: &[Segment], drawn: &mut Drawn) {
    let p = &theme.palette;
    let f = &theme.fonts;
    for segment in segments {
        match segment {
            Segment::Text(text) => {
                ui.label(RichText::new(text).font(f.body.id()).color(p.text));
            }
            Segment::Quote { attribution, text, lines } => {
                let shown = reader.shows_quotes(id);
                let label = if shown {
                    tr!("Hide quoted text").to_owned()
                } else if *lines == 1 {
                    tr!("Show quoted text (1 line)").to_owned()
                } else {
                    tr!("Show quoted text ({count} lines)", count = lines)
                };
                if ui.button(label).clicked() {
                    drawn.asked.push(Asked::Quotes(id));
                }
                if shown {
                    let (bar, inner) = (p.muted, margin(theme.spacing.s));
                    let response = Frame::new().inner_margin(Margin { left: inner.left.saturating_add(4), ..inner }).show(ui, |ui| {
                        if let Some(attribution) = attribution {
                            ui.label(RichText::new(attribution).font(f.caption.id()).color(p.muted));
                        }
                        ui.label(RichText::new(text).font(f.body.id()).color(p.muted));
                    });
                    let rect = response.response.rect;
                    ui.painter().vline(rect.left() + 1.5, rect.y_range(), Stroke::new(3.0, bar));
                }
            }
        }
        ui.add_space(theme.spacing.s);
    }
}

/// The attachment tiles, side by side as far as they fit: a file mark, the name (cut to fit) and the size, and a Save
/// button. Each tile is measured first, so the row wraps before a tile rather than cutting it at the side.
fn files(ui: &mut Ui, theme: &Theme, id: u64, files: &[File], drawn: &mut Drawn) {
    if files.is_empty() {
        return;
    }
    let p = &theme.palette;
    let f = &theme.fonts;
    let s = &theme.spacing;
    ui.add_space(s.s);
    let gap = ui.spacing().item_spacing.x;
    let side = super::paint::line_height(ui, f.body) * 1.5;
    let width = |ui: &Ui, text: &str, font: Font| super::paint::line(ui, &[(text, font, p.text)], f32::INFINITY).size().x;
    let save = width(ui, tr!("Save"), f.button) + ui.spacing().button_padding.x * 2.0;
    let row = ui.available_width();
    ui.with_layout(Layout::left_to_right(Align::Min).with_main_wrap(true), |ui| {
        for file in files {
            let size = size_text(file.size);
            let text = width(ui, &file.name, f.body).max(width(ui, &size, f.caption));
            let tile = (side + text + save + gap * 2.0 + s.s * 2.0 + 2.0).min(row);
            ui.allocate_ui(vec2(tile, side), |ui| {
                Frame::new().stroke(Stroke::new(1.0, p.line)).inner_margin(margin(s.s)).corner_radius(s.corner).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let (rect, _) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
                        theme.icons.paint(ui.painter(), rect, IconId::File, Style::Regular, p.accent);
                        ui.vertical(|ui| {
                            ui.set_max_width((ui.available_width() - save - gap).max(1.0));
                            ui.add(egui::Label::new(RichText::new(&file.name).font(f.body.id()).color(p.text)).truncate());
                            ui.label(RichText::new(&size).font(f.caption.id()).color(p.muted));
                        });
                        let response = ui.button(tr!("Save"));
                        let spoken = tr!("Save {name}", name = file.name);
                        response.widget_info(|| WidgetInfo::labeled(egui::Role::Button, true, &spoken));
                        if response.clicked() {
                            drawn.asked.push(Asked::Save(id, file.index));
                        }
                    });
                });
            });
        }
    });
}

/// One unfolded message, on a card; the open one ends with Reply, Reply all and Forward.
#[expect(clippy::too_many_arguments, reason = "the drawing context of one message")]
fn message(
    ui: &mut Ui,
    theme: &Theme,
    keymap: &Keymap,
    mailbox: &Mailbox,
    reader: &Reader,
    opened: &Opened,
    textures: &mut Textures,
    drawn: &mut Drawn,
) {
    let p = &theme.palette;
    card_frame(theme, theme.spacing.m).show(ui, |ui| {
        ui.set_width(ui.available_width());
        header(ui, theme, mailbox, reader, opened, drawn);
        ui.add_space(theme.spacing.m);
        banner(ui, theme, reader, opened, drawn);
        ui.add_space(theme.spacing.s);
        let say = |ui: &mut Ui, text: &str| {
            ui.label(RichText::new(text).font(theme.fonts.body.id()).color(p.muted));
        };
        match &opened.content {
            Content::Loading => say(ui, tr!("Loading the message…")),
            Content::NotKept => say(ui, tr!("The text of this message is not here yet. It comes with the next connection.")),
            Content::TooLarge => say(ui, tr!("This message is too large to keep, so it cannot be shown.")),
            Content::Unreadable => say(ui, tr!("This message cannot be read.")),
            Content::Text(segments) => text(ui, theme, reader, opened.id, segments, drawn),
            Content::Html(document) => blocks(ui, theme, reader, opened.id, document, &document.blocks, 0, textures, drawn),
        }
        if opened.cut {
            say(ui, tr!("The text is cut here: the rest is longer than the reader shows."));
        }
        files(ui, theme, opened.id, &opened.files, drawn);
        if Some(opened.id) == reader.open_id() {
            ui.add_space(theme.spacing.m);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = theme.spacing.s;
                let answers = [
                    (TextButton::new(tr!("Reply")).primary().icon(IconId::Reply), Answer::Reply),
                    (TextButton::new(tr!("Reply all")).secondary().icon(IconId::ReplyAll), Answer::ReplyAll),
                    (TextButton::new(tr!("Forward")).secondary().icon(IconId::Forward), Answer::Forward),
                ];
                for (button, answer) in answers {
                    if button.show(ui, theme, keymap).clicked() {
                        drawn.asked.push(Asked::Answer(opened.id, answer));
                    }
                }
            });
        }
    });
}

/// The card over a message the screener holds: "Let <sender> in?", the groups to let them into (none chosen at first),
/// Accept, Accept and open, and Block, with what Block does; then a line that says the message is only a preview.
fn screening(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, id: u64, drawn: &mut Drawn) {
    let p = &theme.palette;
    let s = &theme.spacing;
    let Some(row) = mailbox.messages().iter().find(|row| row.id == id) else { return };
    let chosen_id = egui::Id::unique("screen-into").with(id);
    let mut into = ui.data(|data| data.get_temp::<Option<GroupKind>>(chosen_id)).flatten();
    Frame::new().fill(p.surface).stroke(Stroke::new(1.5, p.focus)).corner_radius(s.card).inner_margin(margin(s.m)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = s.s;
        let title = tr!("Let {sender} in?", sender = row.sender);
        ui.add(egui::Label::new(RichText::new(title).font(theme.fonts.heading.id()).color(p.text)).wrap());
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = s.xs;
            ui.label(RichText::new(tr!("Into")).font(theme.fonts.caption.id()).color(p.muted));
            for kind in GroupKind::ALL {
                if pill(ui, theme, kind.text(), into == Some(kind)).clicked() {
                    into = if into == Some(kind) { None } else { Some(kind) };
                    ui.data_mut(|data| data.insert_temp(chosen_id, into));
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = s.s;
            if TextButton::new(tr!("Accept")).primary().icon(IconId::Done).show(ui, theme, keymap).clicked() {
                drawn.asked.push(Asked::Act(id, Act::Accept { group: into, open: false }));
            }
            if TextButton::new(tr!("Accept and open")).secondary().show(ui, theme, keymap).clicked() {
                drawn.asked.push(Asked::Act(id, Act::Accept { group: into, open: true }));
            }
            if TextButton::new(tr!("Block")).secondary().icon(IconId::Junk).show(ui, theme, keymap).clicked() {
                drawn.asked.push(Asked::Act(id, Act::Block));
            }
        });
        let note =
            tr!("Accept lets their held mail and their future mail in. Blocked mail goes to Blocked, never loads remote content and never notifies.");
        ui.add(egui::Label::new(RichText::new(note).font(theme.fonts.caption.id()).color(p.muted)).wrap());
    });
    ui.label(RichText::new(tr!("Preview · nothing is accepted yet")).font(theme.fonts.caption.id()).color(p.muted));
}

/// Where the link under the pointer goes, on a float over the bottom left corner of `rect`. It takes no pointer, so
/// the link under it stays hovered.
fn status(ui: &Ui, theme: &Theme, rect: Rect, text: &str) {
    let p = &theme.palette;
    let s = &theme.spacing;
    let colour = if text.starts_with(tr!("Careful")) { p.error } else { p.text };
    Area::new(ui.scope_id().with("link-status"))
        .order(Order::Foreground)
        .interactable(false)
        .pivot(Align2::LEFT_BOTTOM)
        .fixed_pos(rect.left_bottom() + vec2(s.m, -s.m))
        .fade_in(false)
        .show(ui.ctx(), |ui| {
            let slot = ui.painter().add(Shape::Noop);
            let inner = Frame::new().inner_margin(margin(s.s)).show(ui, |ui| {
                ui.set_max_width((rect.width() - s.m * 2.0 - s.s * 2.0).max(1.0));
                ui.add(egui::Label::new(RichText::new(text).font(theme.fonts.caption.id()).color(colour)).truncate());
            });
            let shapes = glass::float_shapes(ui.painter(), theme, inner.response.rect, Under::Sharp);
            ui.painter().set(slot, Shape::Vec(shapes));
        });
}

/// Draws the reader over the whole of `ui` and returns what was asked for.
pub(super) fn show(
    ui: &mut Ui,
    theme: &Theme,
    keymap: &Keymap,
    mailbox: &Mailbox,
    reader: &Reader,
    textures: &mut Textures,
    hover: Option<&str>,
) -> Drawn {
    let mut drawn = Drawn::default();
    let p = &theme.palette;
    let whole = ui.max_rect();
    if let Some(notice) = reader.notice() {
        egui::Panel::bottom("reader-notice").show_separator_line(false).frame(Frame::new().inner_margin(margin(theme.spacing.s))).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(notice).font(theme.fonts.caption.id()).color(p.text));
                if ui.button(tr!("Dismiss")).clicked() {
                    drawn.asked.push(Asked::Dismiss);
                }
            });
        });
    }
    egui::Panel::top("reader-toolbar").show_separator_line(false).frame(Frame::new().inner_margin(margin(theme.spacing.l))).show(ui, |ui| {
        toolbar(ui, theme, keymap, mailbox, reader, &mut drawn);
    });
    ScrollArea::vertical().auto_shrink([false, false]).id_salt("reader").show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = theme.spacing.s;
        Frame::new()
            .inner_margin(Margin {
                left: margin(theme.spacing.l).left,
                right: margin(theme.spacing.l).right,
                top: 0,
                bottom: margin(theme.spacing.l).bottom,
            })
            .show(ui, |ui| {
                if let Some(id) = reader.open_id().filter(|id| mailbox.held(*id)) {
                    screening(ui, theme, keymap, mailbox, id, &mut drawn);
                }
                for row in reader.thread() {
                    match (&row.opened, reader.is_expanded(row.id)) {
                        (Some(opened), true) => message(ui, theme, keymap, mailbox, reader, opened, textures, &mut drawn),
                        _ => folded(ui, theme, mailbox, row, &mut drawn),
                    }
                }
            });
    });
    if let Some(text) = hover {
        status(ui, theme, whole, text);
    }
    drawn
}
