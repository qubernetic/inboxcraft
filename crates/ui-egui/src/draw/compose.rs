// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The composer in the reader's place, and the floating panel it shrinks to while the list and the reader have the
//! window. The title bar says "New message" (or the subject) with Close, which keeps the draft. Under it a line each:
//! From (the account, a menu of the accounts), To with its chips and the field to type in ("Cc Bcc" shows those two
//! lines), Subject; then the text, and the toolbar at the foot with Discard, Templates (a menu of the account's
//! templates), why Send is blocked, Send and the caret beside it that opens the time picker ("Send later"): with a
//! time picked, Send becomes "Schedule · Sat 08:00", and the picker offers "Send now instead". While a template's blank
//! is left, a line over the toolbar asks for it by its prompt: Enter or Fill in puts what was typed in its place. The
//! marks of the blanks stand out in the text.
//!
//! A chip shows the recipient's avatar and name (the address when there is none) with a button that removes it; one
//! that is no address shows on the error colour with a warning instead of the avatar. Typing in a recipient line shows
//! the people the mail knows under it: the arrows move the highlight, Enter takes it (or what was typed), Escape puts
//! the list away, and Backspace in an empty line removes its last chip.
//!
//! Drawing changes only the text in the fields; everything else it asks for ([`Asked`]) is applied after the frame,
//! through the [`Composer`]'s methods, which run the engine's commands.

use egui::text::{LayoutJob, TextFormat};
use egui::{
    Align, Align2, Area, Frame, Id, Key, Layout, Modifiers, Order, Popup, PopupCloseBehavior, Pos2, Rect, RichText, Role, Sense, Stroke, TextEdit,
    Ui, WidgetInfo, vec2,
};
use inboxcraft_time::Timestamp;

use super::paint::{line, line_height};
use crate::component::avatar;
use crate::component::button::{IconButton, TextButton};
use crate::component::popup;
use crate::component::time_picker::{self, Ask, Picked, Preset, TimePicker};
use crate::component::typed_time;
use crate::compose::{Chip, Composer, Line};
use crate::iconset::{IconId, Style};
use crate::keymap::{self, Keymap};
use crate::mailbox::Mailbox;
use crate::theme::Theme;
use crate::tr;

/// The width of the line names (From, To, Subject), in body-size units.
const NAMES: f32 = 4.5;

/// The narrowest the recipient field gets after the chips, in body-size units.
const FIELD: f32 = 10.0;

/// The width of the floating panel, in body-size units.
const FLOATING: f32 = 22.0;

/// The width of the suggestions and of the accounts' menu, in body-size units.
const MENU: f32 = 22.0;

/// What the person asked for in the composer this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Asked {
    /// What is typed in the line changed.
    Typed(Line),
    /// Enter, or the line left: what is typed (or the highlighted suggestion) becomes a chip.
    Take(Line),
    /// A suggestion was pressed.
    Pick(usize),
    /// The highlight moves down (`true`) or up.
    Step(bool),
    /// Escape put the suggestions away.
    Dismiss,
    /// A chip's remove button.
    Remove(Line, usize),
    /// Backspace in an empty line.
    RemoveLast(Line),
    /// The subject or the text changed.
    Edited,
    /// Show the Cc and Bcc lines.
    Copies,
    /// Write from this account.
    From(u64),
    /// Put the template at this place of the composer's list into the draft.
    Template(usize),
    /// Put what is typed for the first blank in its place.
    Fill,
    /// Send it at this time, or now again with `None`.
    Later(Option<Timestamp>),
    /// The time picker opened: read the time presets still ahead.
    Presets,
    Send,
    Discard,
    /// Close, keeping the draft.
    Close,
    /// Back in the reader's place, from the floating panel.
    Open,
}

/// The id of the field of `line`, to give it the focus.
pub(super) fn field_id(line: Line) -> Id {
    Id::unique("compose-line").with(line.key())
}

/// The id of the subject's field.
pub(super) fn subject_id() -> Id {
    Id::unique("compose-subject")
}

/// The id of the text's field.
pub(super) fn text_id() -> Id {
    Id::unique("compose-text")
}

/// The id of the field of the first blank.
pub(super) fn blank_id() -> Id {
    Id::unique("compose-blank")
}

fn suggestions_id() -> Id {
    Id::unique("compose-suggestions")
}

/// The id the send-later picker's state is kept under between frames and openings.
fn later_id() -> Id {
    Id::unique("compose-later")
}

/// The id of the send-later picker's popup.
pub(super) fn later_popup() -> Id {
    Id::unique("compose-later-popup")
}

/// The width of the send-later picker, in body-size units.
const PICKER: f32 = 22.0;

/// The caret beside Send: it opens the time picker, whose time makes Send a Schedule, and "Send now instead" while a
/// time is picked; with it, the note that nothing goes while InboxCraft is closed.
fn later(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, composer: &Composer, asked: &mut Vec<Asked>) {
    let response = IconButton::new(IconId::Caret, tr!("Send later")).show(ui, theme, keymap);
    if response.clicked() {
        // A new opening: the presets read again, the text typed before gone.
        asked.push(Asked::Presets);
        ui.data_mut(|data| data.get_temp_mut_or_default::<TimePicker>(later_id()).open());
    }
    let width = theme.fonts.body.size * PICKER;
    Popup::menu(&response).id(later_popup()).close_behavior(PopupCloseBehavior::CloseOnClickOutside).frame(Frame::NONE).show(|ui| {
        popup::panel(ui, theme, width, |ui| {
            let presets: Vec<Preset<'_>> = mailbox.presets().iter().map(|(name, at)| Preset { name, at: *at }).collect();
            let ask = Ask { title: tr!("Send later"), confirm: tr!("Schedule"), alert: None, presets: &presets };
            let mut state = ui.data_mut(|data| data.get_temp::<TimePicker>(later_id())).unwrap_or_default();
            let picked = time_picker::show(ui, theme, &mut state, &ask, mailbox.now(), mailbox.offset());
            ui.data_mut(|data| data.insert_temp(later_id(), state));
            match picked {
                Picked::At { at, .. } => {
                    asked.push(Asked::Later(Some(at)));
                    ui.close();
                }
                Picked::Cancelled => ui.close(),
                Picked::Open => {}
            }
            if composer.later().is_some() && popup::entry(ui, theme, keymap, tr!("Send now instead"), None).clicked() {
                asked.push(Asked::Later(None));
                ui.close();
            }
            ui.add_space(theme.spacing.xs);
            let note = tr!("It goes while InboxCraft or its tray runs; a time missed sends at the next start, and says so.");
            ui.add(egui::Label::new(RichText::new(note).font(theme.fonts.caption.id()).color(theme.palette.muted)).wrap());
        });
    });
}

/// A line's name on the left, in the muted colour, `width` points wide and in the middle of the first `height` points.
fn name(ui: &mut Ui, theme: &Theme, text: &str, width: f32, height: f32) {
    let galley = line(ui, &[(text, theme.fonts.body, theme.palette.muted)], width);
    let (rect, _) = ui.allocate_exact_size(vec2(width, galley.size().y.max(height)), Sense::hover());
    ui.painter().galley(egui::pos2(rect.left(), rect.top() + (rect.height() - galley.size().y) / 2.0), galley, theme.palette.muted);
}

/// How tall a chip is.
fn chip_height(ui: &Ui, theme: &Theme) -> f32 {
    line_height(ui, theme.fonts.body) * 1.2 + 4.0
}

/// A rule under a line.
fn rule(ui: &mut Ui, theme: &Theme) {
    ui.add_space(theme.spacing.xs);
    ui.painter().hline(ui.max_rect().x_range(), ui.cursor().top(), Stroke::new(1.0, theme.palette.line));
    ui.add_space(theme.spacing.xs + 1.0);
}

/// A recipient's chip with its remove button; whether that was pressed.
fn chip(ui: &mut Ui, theme: &Theme, keymap: &Keymap, chip: &Chip) -> bool {
    let p = &theme.palette;
    let s = &theme.spacing;
    let (fill, ink, edge) = if chip.ok { (p.panel, p.text, p.field_border) } else { (p.error, p.on_error, p.error) };
    let side = line_height(ui, theme.fonts.body) * 1.2;
    let shown = chip.shown();
    let mut removed = false;
    Frame::new().fill(fill).stroke(Stroke::new(1.0, edge)).corner_radius(255).inner_margin(egui::Margin::symmetric(3, 2)).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = s.xs;
            let galley = line(ui, &[(&shown, theme.fonts.body, ink)], f32::INFINITY);
            let (rect, response) = ui.allocate_exact_size(vec2(side + s.xs + galley.size().x, side), Sense::hover());
            response.widget_info(|| WidgetInfo::labeled(Role::ListItem, true, chip.spoken()));
            let mark = Rect::from_min_size(rect.min, vec2(side, side));
            if chip.ok {
                avatar::paint(ui.painter(), theme, mark, &shown, &chip.address());
            } else {
                theme.icons.paint(ui.painter(), mark.shrink(1.0), IconId::Junk, Style::Regular, ink);
            }
            ui.painter().galley(egui::pos2(mark.right() + s.xs, rect.center().y - galley.size().y / 2.0), galley, ink);
            let remove = tr!("Remove {recipient}", recipient = shown);
            removed = IconButton::new(IconId::Close, &remove).ink(ink).side(side).show(ui, theme, keymap).clicked();
        });
    });
    removed
}

/// The keys of a recipient line that has the focus, read before its field takes them.
fn line_keys(ui: &Ui, composer: &Composer, line: Line, asked: &mut Vec<Asked>) {
    let (suggesting, found) = composer.suggestions();
    if suggesting == Some(line) && !found.is_empty() {
        for (key, step) in [(Key::ArrowDown, true), (Key::ArrowUp, false)] {
            if ui.input_mut(|input| input.consume_key(Modifiers::NONE, key)) {
                asked.push(Asked::Step(step));
            }
        }
    }
    if composer.typing(line).is_empty() && ui.input(|input| input.key_pressed(Key::Backspace)) {
        asked.push(Asked::RemoveLast(line));
    }
}

/// A recipient line: its name, its chips and the field to type in; the To line ends with "Cc Bcc" while those are
/// hidden.
fn recipients(ui: &mut Ui, theme: &Theme, keymap: &Keymap, composer: &mut Composer, line: Line, asked: &mut Vec<Asked>) {
    let body = theme.fonts.body.size;
    ui.horizontal_top(|ui| {
        name(ui, theme, line.name(), body * NAMES, chip_height(ui, theme));
        let copies = line == Line::To && !composer.copies();
        let link = copies.then(|| line_width(ui, theme, tr!("Cc  Bcc")));
        let room = ui.available_width() - link.unwrap_or(0.0);
        ui.allocate_ui_with_layout(vec2(room.max(1.0), 0.0), Layout::left_to_right(Align::Center).with_main_wrap(true), |ui| {
            ui.spacing_mut().item_spacing = vec2(theme.spacing.xs, theme.spacing.xs);
            for (at, shown) in composer.chips(line).to_vec().iter().enumerate() {
                if chip(ui, theme, keymap, shown) {
                    asked.push(Asked::Remove(line, at));
                }
            }
            let id = field_id(line);
            if ui.memory(|memory| memory.has_focus(id)) {
                line_keys(ui, composer, line, asked);
            }
            // The rest of the chips' row, or a row of its own when that is too narrow.
            let rest = ui.available_size_before_wrap().x;
            let width = if rest >= body * FIELD { rest } else { ui.available_width() };
            let hint = if composer.chips(line).is_empty() { tr!("Name or address") } else { "" };
            let Some(typing) = composer.typing_mut(line) else { return };
            let edit = TextEdit::singleline(typing).id(id).frame(Frame::NONE).hint_text(hint).desired_width(width);
            let response = ui.add(edit.min_size(vec2(0.0, chip_height(ui, theme))).align(Align2::LEFT_CENTER));
            ui.ctx().accesskit_node_builder(response.id, |node| node.set_label(line.name()));
            if response.changed() {
                asked.push(Asked::Typed(line));
            }
            if response.lost_focus() {
                let entered = ui.input(|input| input.key_pressed(Key::Enter));
                // A press on a suggestion takes the field's focus first; the suggestion is taken, not the text.
                let on_list = ui
                    .ctx()
                    .pointer_latest_pos()
                    .zip(ui.ctx().data(|data| data.get_temp::<Rect>(suggestions_id())))
                    .is_some_and(|(at, list)| list.contains(at));
                let escaped = ui.input(|input| input.key_pressed(Key::Escape));
                if entered {
                    asked.push(Asked::Take(line));
                    response.request_focus();
                } else if escaped && !composer.suggestions().1.is_empty() {
                    // Escape puts the suggestions away and leaves the text and the focus where they are.
                    asked.push(Asked::Dismiss);
                    response.request_focus();
                } else if !escaped && !on_list && !composer.typing(line).trim().is_empty() {
                    asked.push(Asked::Take(line));
                }
            }
            if composer.suggestions().0 == Some(line) {
                suggestions(ui.ctx(), theme, composer, response.rect.left_bottom(), asked);
            }
        });
        if copies && crate::component::button::link(ui, theme, keymap, tr!("Cc  Bcc")).clicked() {
            asked.push(Asked::Copies);
        }
    });
}

/// The width of `text` as a link, with its margins.
fn line_width(ui: &Ui, theme: &Theme, text: &str) -> f32 {
    line(ui, &[(text, theme.fonts.caption, theme.palette.accent)], f32::INFINITY).size().x + theme.spacing.m * 2.0
}

/// The people offered under a recipient field, the highlighted one marked.
fn suggestions(ctx: &egui::Context, theme: &Theme, composer: &Composer, at: Pos2, asked: &mut Vec<Asked>) {
    let found = composer.suggestions().1;
    if found.is_empty() {
        return;
    }
    let width = theme.fonts.body.size * MENU;
    let rect = popup::show(ctx, theme, suggestions_id(), at, width, |ui| {
        for (index, person) in found.iter().enumerate() {
            if popup::choice(ui, theme, &person.shown(), composer.highlighted() == Some(index)).clicked() {
                asked.push(Asked::Pick(index));
            }
        }
        ui.min_rect()
    });
    ctx.data_mut(|data| data.insert_temp(suggestions_id(), rect.expand(theme.spacing.xs)));
}

/// The From line: the account as a button that opens the accounts' menu, its label at the right end.
fn from(ui: &mut Ui, theme: &Theme, mailbox: &Mailbox, composer: &Composer, asked: &mut Vec<Asked>) {
    let p = &theme.palette;
    let body = theme.fonts.body.size;
    let Some(account) = mailbox.account(composer.account()) else { return };
    ui.horizontal(|ui| {
        name(ui, theme, tr!("From"), body * NAMES, 0.0);
        let sender =
            if account.name.is_empty() { account.address.clone() } else { tr!("{name} <{address}>", name = account.name, address = account.address) };
        let galley = line(ui, &[(&sender, theme.fonts.body, p.text)], ui.available_width() * 0.7);
        let caret = line_height(ui, theme.fonts.body) * 0.8;
        let (rect, response) = ui.allocate_exact_size(vec2(galley.size().x + theme.spacing.xs + caret, galley.size().y.max(caret)), Sense::click());
        let spoken = tr!("From {sender}", sender = sender);
        response.widget_info(|| WidgetInfo::labeled(Role::ComboBox, true, &spoken));
        if response.hovered() {
            ui.painter().rect_filled(rect.expand(2.0), theme.spacing.corner, p.hover);
        }
        ui.painter().galley(rect.left_top(), galley, p.text);
        let mark = Rect::from_min_size(egui::pos2(rect.right() - caret, rect.center().y - caret / 2.0), vec2(caret, caret));
        theme.icons.paint(ui.painter(), mark, IconId::Caret, Style::Regular, p.muted);
        super::paint::focus_ring(ui, theme, &response);
        Popup::menu(&response).frame(Frame::NONE).show(|ui| {
            popup::panel(ui, theme, body * MENU, |ui| {
                for choice in mailbox.accounts() {
                    let text = tr!("{label} ({address})", label = choice.label, address = choice.address);
                    if popup::choice(ui, theme, &text, choice.id == account.id).clicked() && choice.id != account.id {
                        asked.push(Asked::From(choice.id));
                    }
                }
            });
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(RichText::new(&account.label).font(theme.fonts.caption.id()).color(p.muted));
        });
    });
}

/// The Templates button and its menu of the account's templates.
fn templates(ui: &mut Ui, theme: &Theme, keymap: &Keymap, composer: &Composer, asked: &mut Vec<Asked>) {
    let button = IconButton::new(IconId::File, tr!("Templates"));
    if composer.templates().is_empty() {
        button.unavailable(tr!("This account has no templates")).show(ui, theme, keymap);
        return;
    }
    let response = button.show(ui, theme, keymap);
    Popup::menu(&response).frame(Frame::NONE).show(|ui| {
        popup::panel(ui, theme, theme.fonts.body.size * MENU, |ui| {
            ui.label(RichText::new(tr!("Put in a template")).font(theme.fonts.caption.id()).color(theme.palette.muted));
            for (at, name) in composer.templates().iter().enumerate() {
                if popup::entry(ui, theme, keymap, name, None).clicked() {
                    asked.push(Asked::Template(at));
                }
            }
        });
    });
}

/// The line that asks for the first blank by its prompt: its field, and Fill in.
fn blank(ui: &mut Ui, theme: &Theme, keymap: &Keymap, composer: &mut Composer, asked: &mut Vec<Asked>) {
    let Some(prompt) = composer.blank() else { return };
    let p = &theme.palette;
    Frame::new().fill(p.panel).corner_radius(theme.spacing.corner).inner_margin(super::margin(theme.spacing.s)).show(ui, |ui| {
        ui.horizontal(|ui| {
            let fill = TextButton::new(tr!("Fill in")).secondary();
            let room = (ui.available_width() - line_width(ui, theme, tr!("Fill in")) - theme.spacing.l).max(1.0);
            let spoken = tr!("Fill in \"{blank}\"", blank = prompt);
            let edit = TextEdit::singleline(composer.filling_mut()).id(blank_id()).hint_text(&spoken).desired_width(room);
            let response = ui.add(edit);
            ui.ctx().accesskit_node_builder(response.id, |node| node.set_label(spoken.as_str()));
            if response.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter)) {
                asked.push(Asked::Fill);
            }
            if fill.show(ui, theme, keymap).clicked() {
                asked.push(Asked::Fill);
            }
        });
    });
    ui.add_space(theme.spacing.s);
}

/// The text laid out with the marks of the open blanks on the accent's tint, so they stand out.
fn marked(ui: &Ui, theme: &Theme, marks: &[String], text: &str, width: f32) -> std::sync::Arc<egui::Galley> {
    let p = &theme.palette;
    let plain = TextFormat::simple(theme.fonts.body.id(), p.text);
    let mark = TextFormat { background: p.accent.gamma_multiply(0.25), color: p.text, ..plain.clone() };
    let mut job = LayoutJob::default();
    job.wrap.max_width = width;
    let mut rest = text;
    while !rest.is_empty() {
        let next = marks.iter().filter(|held| !held.is_empty()).filter_map(|held| rest.find(held.as_str()).map(|at| (at, held.len()))).min();
        let Some((at, len)) = next else {
            job.append(rest, 0.0, plain.clone());
            break;
        };
        let (before, after) = rest.split_at(at);
        let (found, after) = after.split_at(len);
        job.append(before, 0.0, plain.clone());
        job.append(found, 0.0, mark.clone());
        rest = after;
    }
    ui.fonts_mut(|fonts| fonts.layout_job(job))
}

/// What Send says: "Send", or "Schedule · Sat 08:00" with a time picked; and why it cannot go yet, a time picked that
/// has passed among the reasons.
fn send_label(mailbox: &Mailbox, composer: &Composer) -> (String, Option<String>) {
    let now = mailbox.now();
    match composer.later() {
        Some(at) if at > now => (tr!("Schedule · {when}", when = typed_time::short(at, now, mailbox.offset())), composer.problem()),
        Some(_) => {
            (tr!("Schedule").to_owned(), Some(composer.problem().unwrap_or_else(|| tr!("The time picked has passed: pick another").to_owned())))
        }
        None => (tr!("Send").to_owned(), composer.problem()),
    }
}

/// The toolbar at the foot: Discard and Templates, then why Send is blocked, Send and the caret of send later at the
/// right end.
fn toolbar(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, composer: &Composer, asked: &mut Vec<Asked>) {
    let p = &theme.palette;
    ui.horizontal(|ui| {
        if IconButton::new(IconId::Delete, tr!("Discard the draft")).show(ui, theme, keymap).clicked() {
            asked.push(Asked::Discard);
        }
        templates(ui, theme, keymap, composer, asked);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            later(ui, theme, keymap, mailbox, composer, asked);
            let (label, problem) = send_label(mailbox, composer);
            let send = TextButton::new(&label).primary().icon(IconId::Send).action(keymap::SEND);
            match &problem {
                Some(problem) => {
                    send.unavailable(problem).show(ui, theme, keymap);
                }
                None => {
                    if send.show(ui, theme, keymap).clicked() {
                        asked.push(Asked::Send);
                    }
                }
            }
            if let Some(problem) = problem {
                ui.label(RichText::new(problem).font(theme.fonts.caption.id()).color(p.text));
                let side = line_height(ui, theme.fonts.caption);
                let (rect, _) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
                theme.icons.paint(ui.painter(), rect, IconId::Junk, Style::Regular, p.error);
            }
        });
    });
}

/// Draws the composer into `ui`, in the reader's place; what was asked for, applied after the frame.
pub(super) fn show(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, composer: &mut Composer) -> Vec<Asked> {
    let s = &theme.spacing;
    let p = &theme.palette;
    let mut asked = Vec::new();
    Frame::new().inner_margin(super::margin(s.l)).show(ui, |ui| {
        ui.set_min_size(ui.available_size());
        super::frame::beside(ui, theme, |ui| {
            ui.horizontal(|ui| {
                ui.add(egui::Label::new(RichText::new(composer.title()).font(theme.fonts.title.id()).color(p.text)).truncate());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if IconButton::new(IconId::Close, tr!("Close; the draft is kept")).show(ui, theme, keymap).clicked() {
                        asked.push(Asked::Close);
                    }
                });
            });
        });
        ui.add_space(s.s);
        from(ui, theme, mailbox, composer, &mut asked);
        rule(ui, theme);
        let lines: &[Line] = if composer.copies() { &Line::ALL } else { &[Line::To] };
        for line in lines {
            recipients(ui, theme, keymap, composer, *line, &mut asked);
            rule(ui, theme);
        }
        ui.horizontal(|ui| {
            name(ui, theme, tr!("Subject"), theme.fonts.body.size * NAMES, 0.0);
            let edit = TextEdit::singleline(composer.subject_mut()).id(subject_id()).frame(Frame::NONE).desired_width(ui.available_width());
            let response = ui.add(edit);
            ui.ctx().accesskit_node_builder(response.id, |node| node.set_label(tr!("Subject")));
            if response.changed() {
                asked.push(Asked::Edited);
            }
        });
        rule(ui, theme);
        let side = crate::component::control_side(ui, theme);
        // The toolbar, and the line asking for a blank while one is left.
        let foot = side + s.m + if composer.blank().is_some() { side + s.s * 4.0 } else { 0.0 };
        let height = (ui.available_height() - foot).max(line_height(ui, theme.fonts.body) * 3.0);
        let rows = (height / line_height(ui, theme.fonts.body)).floor().max(3.0);
        let marks = composer.blanks().to_vec();
        let mut layouter = |ui: &Ui, text: &dyn egui::TextBuffer, width: f32| marked(ui, theme, &marks, text.as_str(), width);
        egui::ScrollArea::vertical().max_height(height).auto_shrink([false, true]).show(ui, |ui| {
            // A whole number of rows, fewer than a few hundred: the cast keeps it.
            let edit = TextEdit::multiline(composer.text_mut())
                .layouter(&mut layouter)
                .id(text_id())
                .frame(Frame::NONE)
                .desired_width(ui.available_width())
                .desired_rows(rows as usize);
            let response = ui.add(edit);
            ui.ctx().accesskit_node_builder(response.id, |node| node.set_label(tr!("Message text")));
            if response.changed() {
                asked.push(Asked::Edited);
            }
        });
        ui.add_space(s.s);
        blank(ui, theme, keymap, composer, &mut asked);
        toolbar(ui, theme, keymap, mailbox, composer, &mut asked);
    });
    asked
}

/// The floating panel at the bottom right of `screen`: the composer's title, which opens it again in the reader's
/// place, and Close; what was asked for.
pub(super) fn floating(ctx: &egui::Context, theme: &Theme, keymap: &Keymap, composer: &Composer, screen: Rect) -> Vec<Asked> {
    let s = &theme.spacing;
    let mut asked = Vec::new();
    let width = theme.fonts.body.size * FLOATING;
    let corner = screen.right_bottom() - vec2(s.gap * 2.0, s.gap * 2.0);
    Area::new(Id::unique("compose-floating")).order(Order::Foreground).pivot(Align2::RIGHT_BOTTOM).fixed_pos(corner).show(ctx, |ui| {
        popup::panel(ui, theme, width, |ui| {
            ui.horizontal(|ui| {
                let title = composer.title();
                let open = tr!("Open the draft {title}", title = title);
                let side = crate::component::control_side(ui, theme);
                let room = (ui.available_width() - side - s.xs).max(1.0);
                let response = ui.allocate_ui_with_layout(vec2(room, side), Layout::top_down(Align::Min), |ui| {
                    popup::item(ui, theme, keymap, IconId::Compose, &title, Some(keymap::COMPOSE))
                });
                response.inner.widget_info(|| WidgetInfo::labeled(Role::Button, true, &open));
                if response.inner.clicked() {
                    asked.push(Asked::Open);
                }
                if IconButton::new(IconId::Close, tr!("Close; the draft is kept")).side(side).show(ui, theme, keymap).clicked() {
                    asked.push(Asked::Close);
                }
            });
        });
    });
    asked
}
