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
use egui::{Align, Align2, Area, Frame, Id, Key, Layout, Modifiers, Order, Pos2, Rect, RichText, Role, Sense, Stroke, Ui, WidgetInfo, vec2};
use inboxcraft_time::Timestamp;

use super::paint::line;
use crate::component::button::{IconButton, TextButton};
use crate::component::field::{self, Field};
use crate::component::popup;
use crate::component::recipient::{self, Recipient, RecipientState, Recipients};
use crate::component::time_picker::{self, Ask, Picked, Preset, TimePicker};
use crate::component::typed_time;
use crate::compose::{Composer, Line};
use crate::iconset::IconId;
use crate::keymap::{self, Keymap};
use crate::mailbox::Mailbox;
use crate::theme::Theme;
use crate::tr;

/// The width of the line names (From, To, Subject), in body-size units.
const NAMES: f32 = 4.5;

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

/// The width of the recipient suggestions' float, in points: rows of 320 inside its edge.
const SUGGESTIONS: f32 = 322.0;

/// The id of the From line's menu of the accounts.
pub(super) fn from_menu() -> Id {
    Id::unique("compose-from-menu")
}

/// The id of the menu of the templates.
pub(super) fn templates_menu() -> Id {
    Id::unique("compose-templates-menu")
}

/// The id of the send-later picker's popup.
pub(super) fn later_popup() -> Id {
    Id::unique("compose-later-popup")
}

/// The caret beside Send: it opens the time picker, whose time makes Send a Schedule, and "Send now instead" while a
/// time is picked; with it, the note that nothing goes while InboxCraft is closed.
fn later(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, composer: &Composer, asked: &mut Vec<Asked>) {
    let response = IconButton::new(IconId::Caret, tr!("Send later")).show(ui, theme, keymap);
    let ctx = ui.ctx().clone();
    if popup::clicked(&ctx, &response, later_popup()) {
        let open = !popup::is_open(&ctx, later_popup());
        popup::set_open(&ctx, later_popup(), open);
        if open {
            // A new opening: the presets read again, the text typed before gone.
            asked.push(Asked::Presets);
            ui.data_mut(|data| data.get_temp_mut_or_default::<TimePicker>(later_id()).open());
        }
    }
    if !popup::is_open(&ctx, later_popup()) {
        return;
    }
    let float = popup::Float { id: later_popup(), anchor: response.rect, width: time_picker::WIDTH, returns_to: response.id };
    let shown = popup::float(&ctx, theme, &float, |ui| {
        let presets: Vec<Preset<'_>> = mailbox.presets().iter().map(|(name, at)| Preset { name, at: *at }).collect();
        let note = tr!("It goes while InboxCraft or its tray runs; a time missed sends at the next start, and says so.");
        let extra = composer.later().is_some().then_some(tr!("Send now instead"));
        let ask = Ask { title: tr!("Send later"), confirm: tr!("Schedule"), alert: None, presets: &presets, extra, note: Some(note) };
        let mut state = ui.data_mut(|data| data.get_temp::<TimePicker>(later_id())).unwrap_or_default();
        let picked = time_picker::show(ui, theme, &mut state, &ask, mailbox.now(), mailbox.offset());
        ui.data_mut(|data| data.insert_temp(later_id(), state));
        picked
    });
    let close = |asked: &mut Vec<Asked>, later: Option<Option<inboxcraft_time::Timestamp>>| {
        if let Some(later) = later {
            asked.push(Asked::Later(later));
        }
        popup::set_open(&ctx, later_popup(), false);
        response.request_focus();
    };
    match shown {
        Some(Picked::At { at, .. }) => close(asked, Some(Some(at))),
        Some(Picked::Extra) => close(asked, Some(None)),
        Some(Picked::Cancelled) => close(asked, None),
        Some(Picked::Open) => {}
        None => popup::set_open(&ctx, later_popup(), false),
    }
}

/// A line's name on the left, in the muted colour, `width` points wide and in the middle of the first `height` points.
fn name(ui: &mut Ui, theme: &Theme, text: &str, width: f32, height: f32) {
    let galley = line(ui, &[(text, theme.fonts.body, theme.palette.muted)], width);
    let (rect, _) = ui.allocate_exact_size(vec2(width, galley.size().y.max(height)), Sense::hover());
    ui.painter().galley(egui::pos2(rect.left(), rect.top() + (rect.height() - galley.size().y) / 2.0), galley, theme.palette.muted);
}

/// A rule under a line.
fn rule(ui: &mut Ui, theme: &Theme) {
    ui.add_space(theme.spacing.s1);
    ui.painter().hline(ui.max_rect().x_range(), ui.cursor().top(), Stroke::new(1.0, theme.palette.line));
    ui.add_space(theme.spacing.s1 + 1.0);
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

/// A recipient line: its name, its chips and the field to type in ([`recipient::recipient_field`]); the To line ends
/// with "Cc Bcc" while those are hidden.
fn recipients(ui: &mut Ui, theme: &Theme, composer: &mut Composer, line: Line, asked: &mut Vec<Asked>) {
    let id = field_id(line);
    if ui.memory(|memory| memory.has_focus(id)) {
        line_keys(ui, composer, line, asked);
    }
    let chips: Vec<Recipient> = composer
        .chips(line)
        .iter()
        .map(|chip| Recipient {
            shown: chip.shown(),
            spoken: chip.spoken(),
            state: if chip.ok { RecipientState::Valid } else { RecipientState::Invalid },
        })
        .collect();
    // The sheet's invalid state: the first recipient of the line that is not an address, named under it.
    // What the line refused of what was typed comes first: it waits in the typed text.
    let wrong = composer
        .refusal(line)
        .or_else(|| composer.chips(line).iter().find(|chip| !chip.ok).map(|chip| tr!("{address} is not an address", address = chip.address())));
    let copies = line == Line::To && !composer.copies();
    let Some(typing) = composer.typing_mut(line) else { return };
    let fields = Recipients { name: line.name(), chips: &chips, hint: tr!("Name or address"), copies, error: wrong.as_deref(), enabled: true };
    let out = recipient::recipient_field(ui, theme, id, typing, &fields);
    if let Some(at) = out.removed {
        asked.push(Asked::Remove(line, at));
    }
    if out.copies {
        asked.push(Asked::Copies);
    }
    let response = out.field;
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
}

/// The width of `text` as a link, with its margins.
fn line_width(ui: &Ui, theme: &Theme, text: &str) -> f32 {
    line(ui, &[(text, theme.fonts.caption, theme.palette.cyan_text)], f32::INFINITY).size().x + theme.spacing.s3 * 2.0
}

/// The people offered under a recipient field, the highlighted one marked.
fn suggestions(ctx: &egui::Context, theme: &Theme, composer: &Composer, at: Pos2, asked: &mut Vec<Asked>) {
    let found = composer.suggestions().1;
    if found.is_empty() {
        return;
    }
    // The suggestion rows are the sheet's 320 wide, inside the float's 1 px edge.
    let width = SUGGESTIONS;
    let rect = popup::show(ctx, theme, suggestions_id(), at, width, |ui| {
        for (index, person) in found.iter().enumerate() {
            let row = popup::suggestion_row(ui, theme, person.name.trim(), &person.address, composer.highlighted() == Some(index));
            // A screen reader hears the person as the line will hold them.
            let spoken = person.shown();
            ui.ctx().accesskit_node_builder(row.id, |node| node.set_label(spoken.as_str()));
            if row.clicked() {
                asked.push(Asked::Pick(index));
            }
        }
        ui.min_rect()
    });
    ctx.data_mut(|data| data.insert_temp(suggestions_id(), rect.expand(theme.spacing.s1)));
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
        let galley = line(ui, &[(&sender, theme.fonts.body, p.ink)], ui.available_width() * 0.7);
        let caret = theme.fonts.body.line * 0.8;
        let (rect, response) = ui.allocate_exact_size(vec2(galley.size().x + theme.spacing.s1 + caret, galley.size().y.max(caret)), Sense::click());
        let spoken = tr!("From {sender}", sender = sender);
        response.widget_info(|| WidgetInfo::labeled(Role::ComboBox, true, &spoken));
        if response.hovered() {
            ui.painter().rect_filled(rect.expand(2.0), 0.0, p.stage);
        }
        ui.painter().galley(rect.left_top(), galley, p.ink);
        let mark = Rect::from_min_size(egui::pos2(rect.right() - caret, rect.center().y - caret / 2.0), vec2(caret, caret));
        crate::iconset::paint(ui.painter(), mark, IconId::Caret, p.muted);
        if response.has_focus() {
            super::paint::focus_ring(ui.painter(), theme, response.rect, super::paint::Ring::Inside);
        }
        popup::menu(ui.ctx(), theme, &response, from_menu(), body * MENU, |ui| {
            let mut chosen = false;
            for choice in mailbox.accounts() {
                let text = tr!("{label} ({address})", label = choice.label, address = choice.address);
                if popup::choice(ui, theme, &text, choice.id == account.id).clicked() {
                    chosen = true;
                    if choice.id != account.id {
                        asked.push(Asked::From(choice.id));
                    }
                }
            }
            ((), chosen)
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
    popup::menu(ui.ctx(), theme, &response, templates_menu(), theme.fonts.body.size * MENU, |ui| {
        popup::title(ui, theme, tr!("Put in a template"), None);
        let mut chosen = false;
        for (at, name) in composer.templates().iter().enumerate() {
            let item = popup::Item { icon: None, name, caption: None, action: None, state: popup::ItemState::Default };
            if popup::menu_item(ui, theme, keymap, &item).clicked() {
                asked.push(Asked::Template(at));
                chosen = true;
            }
        }
        ((), chosen)
    });
}

/// The line that asks for the first blank by its prompt: its field, and Fill in.
fn blank(ui: &mut Ui, theme: &Theme, keymap: &Keymap, composer: &mut Composer, asked: &mut Vec<Asked>) {
    let Some(prompt) = composer.blank() else { return };
    let p = &theme.palette;
    Frame::new().fill(p.panel).corner_radius(0.0).inner_margin(super::margin(theme.spacing.s2)).show(ui, |ui| {
        ui.horizontal(|ui| {
            let fill = TextButton::new(tr!("Fill in")).secondary();
            let room = (ui.available_width() - line_width(ui, theme, tr!("Fill in")) - theme.spacing.s4).max(1.0);
            let spoken = tr!("Fill in \"{blank}\"", blank = prompt);
            let response = ui
                .allocate_ui(vec2(room, theme.boxes().field), |ui| {
                    ui.set_width(room);
                    let refused = composer.fill_refused().map(str::to_owned);
                    let field = Field { error: refused.as_deref(), ..Field::new("", &spoken) };
                    field::text_field(ui, theme, blank_id(), composer.filling_mut(), &field)
                })
                .inner;
            if response.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter)) {
                asked.push(Asked::Fill);
            }
            if fill.show(ui, theme, keymap).clicked() {
                asked.push(Asked::Fill);
            }
        });
    });
    ui.add_space(theme.spacing.s2);
}

/// The text laid out with the marks of the open blanks on the accent's tint, so they stand out.
fn marked(ui: &Ui, theme: &Theme, marks: &[String], text: &str, width: f32) -> std::sync::Arc<egui::Galley> {
    let p = &theme.palette;
    let plain = TextFormat::simple(theme.fonts.body.id(), p.ink);
    let mark = TextFormat { background: p.cyan_muted, color: p.ink, ..plain.clone() };
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

/// The toolbar at the foot: Discard and Templates, then why Send is blocked (unless a recipient line says it), Send and
/// the caret of send later at the right end.
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
            // A recipient that is not an address is named under its line (the sheet's place), so not again here; Send's
            // tooltip still says why it cannot go.
            let shown: &[Line] = if composer.copies() { &Line::ALL } else { &[Line::To] };
            let said_under = shown.iter().flat_map(|line| composer.chips(*line)).any(|chip| !chip.ok);
            if let Some(problem) = problem.filter(|_| !said_under) {
                ui.label(RichText::new(problem).font(theme.fonts.caption.id()).color(p.ink));
                let side = theme.fonts.caption.line;
                let (rect, _) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
                crate::iconset::paint(ui.painter(), rect, IconId::Junk, p.minus);
            }
        });
    });
}

/// Draws the composer into `ui`, in the reader's place; what was asked for, applied after the frame.
pub(super) fn show(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, composer: &mut Composer) -> Vec<Asked> {
    let s = &theme.spacing;
    let p = &theme.palette;
    let mut asked = Vec::new();
    Frame::new().inner_margin(super::margin(s.s4)).show(ui, |ui| {
        ui.set_min_size(ui.available_size());
        super::frame::beside(ui, theme, |ui| {
            ui.horizontal(|ui| {
                ui.add(egui::Label::new(RichText::new(composer.title()).font(theme.fonts.title.id()).color(p.ink)).truncate());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if IconButton::new(IconId::Close, tr!("Close; the draft is kept")).show(ui, theme, keymap).clicked() {
                        asked.push(Asked::Close);
                    }
                });
            });
        });
        ui.add_space(s.s2);
        from(ui, theme, mailbox, composer, &mut asked);
        rule(ui, theme);
        let lines: &[Line] = if composer.copies() { &Line::ALL } else { &[Line::To] };
        for line in lines {
            recipients(ui, theme, composer, *line, &mut asked);
            rule(ui, theme);
        }
        ui.horizontal(|ui| {
            name(ui, theme, tr!("Subject"), theme.fonts.body.size * NAMES, 0.0);
            let response = field::plain_line(
                ui,
                theme,
                subject_id(),
                composer.subject_mut(),
                tr!("Subject"),
                field::Cap::Bytes(crate::compose::MAX_SUBJECT_BYTES),
            );
            if response.changed() {
                asked.push(Asked::Edited);
            }
        });
        rule(ui, theme);
        let side = crate::component::control_side(theme);
        // The toolbar, and the line asking for a blank while one is left.
        let foot = side + s.s3 + if composer.blank().is_some() { side + s.s2 * 4.0 } else { 0.0 };
        let height = (ui.available_height() - foot).max(theme.fonts.body.line * 3.0);
        let rows = (height / theme.fonts.body.line).floor().max(3.0);
        let marks = composer.blanks().to_vec();
        let mut layouter = |ui: &Ui, text: &dyn egui::TextBuffer, width: f32| marked(ui, theme, &marks, text.as_str(), width);
        crate::component::scroll::show(ui, theme, egui::ScrollArea::vertical().max_height(height).auto_shrink([false, true]), |ui| {
            // A whole number of rows, fewer than a few hundred: the cast keeps it.
            let response = field::plain_text(ui, text_id(), composer.text_mut(), rows as usize, &mut layouter, tr!("Message text"), theme);
            if response.changed() {
                asked.push(Asked::Edited);
            }
        });
        ui.add_space(s.s2);
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
    let corner = screen.right_bottom() - vec2(s.s2 * 2.0, s.s2 * 2.0);
    Area::new(Id::unique("compose-floating")).order(Order::Foreground).pivot(Align2::RIGHT_BOTTOM).fixed_pos(corner).show(ctx, |ui| {
        popup::panel(ui, theme, width, |ui| {
            ui.horizontal(|ui| {
                let title = composer.title();
                let open = tr!("Open the draft {title}", title = title);
                let side = crate::component::control_side(theme);
                let room = (ui.available_width() - side - s.s1).max(1.0);
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
