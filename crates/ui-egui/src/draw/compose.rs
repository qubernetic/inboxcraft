// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The composer, as the design's frames draw it: docked in the reader's place, or the floating panel of 520 × 560 at
//! the bottom right, 3 px clear of the window's border, while the list and the reader have the window; the panel folds
//! to its head.
//!
//! The head holds the title ("New message", or the subject) in `heading`, when the draft was last kept in `mono_s`,
//! and Close, which keeps the draft (the floating panel has "Back to the reader's place" before it, and its title folds
//! it). Under it the lines From, To (Cc and Bcc once shown) and Subject, 36 high, their names in the `label` style in a
//! column of 64, each with a hairline of `panel_line` under it: From is the account as a button that opens the menu of
//! the accounts (the name and the address in `body`, the caret, the account's bar and label at the right end); a
//! recipient line holds chips and the text being typed ([`recipient::recipient_line`]); Subject its text in `strong`.
//! Then the text on `term`, its edge `cyan_edge` while it has the keys, in `read` (Plex Sans 16/26), 16 in. The foot:
//! **Send at the left** with its key (`Ctrl Enter`) and the caret of send later beside it, then why Send waits (in
//! `muted` while it waits for something to do, in `minus` with a warning when something is wrong); Templates and
//! Discard at the right, attaching a file before them coming soon. While a template's blank is left, a panel over the
//! foot asks for it by its prompt: Enter or Fill in puts what was typed in its place.
//!
//! Typing in a recipient line shows the people the mail knows under it, 360 wide: the arrows move the highlight, Enter
//! takes it (or what was typed), Escape puts the list away and keeps the text, and Backspace in an empty line removes
//! its last chip.
//!
//! Drawing changes only the text in the fields; everything else it asks for ([`Asked`]) is applied after the frame,
//! through the [`Composer`]'s methods, which run the engine's commands.

use egui::text::{LayoutJob, TextFormat};
use egui::{
    Align, Align2, Area, Id, Key, Layout, Modifiers, Order, Pos2, Rect, Role, Sense, Stroke, StrokeKind, Ui, UiBuilder, Vec2, WidgetInfo, pos2, vec2,
};
use inboxcraft_time::Timestamp;

use super::paint;
use crate::component::button::{IconButton, TextButton};
use crate::component::field::{self, Field};
use crate::component::recipient::{self, Recipient, RecipientState, Recipients};
use crate::component::soon::{soon, soon_tag};
use crate::component::time_picker::{self, Ask, Picked, Preset, TimePicker};
use crate::component::{control_side, cut, galley, popup, typed_time};
use crate::compose::{Composer, Line};
use crate::iconset::IconId;
use crate::keymap::{self, Keymap};
use crate::mailbox::Mailbox;
use crate::theme::Theme;
use crate::tr;

/// The floating panel's size, in points (its frames).
pub(super) const FLOATING: Vec2 = vec2(520.0, 560.0);

/// How far a float keeps from the window's border, in points.
const CLEAR: f32 = 3.0;

/// The width of the From menu, in points (its frame).
const FROM_MENU: f32 = 360.0;

/// The width of the Templates menu, in points (its frame).
const TEMPLATES_MENU: f32 = 256.0;

/// The width of the recipient suggestions' float, in points (the composer's frame): rows of 358 inside its edge.
const SUGGESTIONS: f32 = 360.0;

/// The widest the account's label at the From line's right end gets, docked and floating, in points.
const LABEL_MOST: [f32; 2] = [160.0, 80.0];

/// The room the From line keeps beside the sender's name for the address, the caret and the label, in points: the
/// frames' name stops 246 short of the line's value.
const NAME_ROOM: f32 = 246.0;

/// The warning before a reason that something is wrong: Phosphor's warning, as the frames draw it.
const WARNING: char = '\u{e4e0}';

/// The width of the draft's caret's mark beside the sender, in points (Phosphor's caret-down at 14).
const FROM_CARET: f32 = 14.0;

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
    /// Fold the floating panel to its head, or open it again.
    Fold,
}

/// Where the composer is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Place {
    /// In the reader's place.
    Docked,
    /// The floating panel.
    Floating,
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

/// Where the composer was drawn: its layer, its rect and the pass it was drawn in.
#[derive(Clone, Copy)]
struct Drawn {
    layer: egui::LayerId,
    rect: Rect,
    pass: u64,
}

fn drawn_id() -> Id {
    Id::unique("compose-drawn")
}

/// Keeps where the composer is drawn in this pass, for [`has_keys`].
fn mark_drawn(ui: &Ui, rect: Rect) {
    let drawn = Drawn { layer: ui.layer_id(), rect, pass: ui.ctx().cumulative_pass_nr() };
    ui.ctx().data_mut(|data| data.insert_temp(drawn_id(), drawn));
}

/// Whether the keyboard focus is inside the composer drawn in the last pass: a widget on its layer, in its rect. A
/// field of a float over it (the settings sheet, the calendar's editor, a menu) or of the window beside it (the search
/// field) is not.
pub(super) fn has_keys(ctx: &egui::Context) -> bool {
    let Some(focused) = ctx.memory(|memory| memory.focused()) else { return false };
    let pass = ctx.cumulative_pass_nr();
    let Some(drawn) = ctx.data(|data| data.get_temp::<Drawn>(drawn_id())).filter(|drawn| drawn.pass.saturating_add(1) >= pass) else {
        return false;
    };
    ctx.read_response(focused).is_some_and(|response| response.layer_id == drawn.layer && drawn.rect.contains(response.rect.center()))
}

/// Whether `id` is one of the composer's fields that take typing: a recipient line, the subject, the text, a blank.
pub(super) fn is_field(id: Id) -> bool {
    id == subject_id() || id == text_id() || id == blank_id() || Line::ALL.iter().any(|line| id == field_id(*line))
}

fn suggestions_id() -> Id {
    Id::unique("compose-suggestions")
}

/// The id the send-later picker's state is kept under between frames and openings.
fn later_id() -> Id {
    Id::unique("compose-later")
}

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
/// time is picked; with it, the note that nothing goes while InboxCraft is closed. Selected while a time is picked.
fn later(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, composer: &Composer, asked: &mut Vec<Asked>) {
    let response = IconButton::new(IconId::Caret, tr!("Send later")).active(composer.later().is_some()).show(ui, theme, keymap);
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

/// A line's name in its column: `text` in the `label` style and `muted`, in the middle of the first 24 of `rect`.
fn name(ui: &Ui, theme: &Theme, rect: Rect, text: &str) {
    let label = galley(ui, text, theme.fonts.label, theme.palette.muted);
    let mid = rect.top() + theme.spacing.s2 - 2.0 + theme.boxes().pill / 2.0;
    paint::run_at(ui.painter(), pos2(rect.left(), mid - label.size().y / 2.0), label, theme.palette.muted);
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

/// A recipient line: its name, its chips and the field to type in ([`recipient::recipient_line`]); the To line ends
/// with "Cc  Bcc" while those are hidden. What the line refused of what was typed is said under it; a chip that is not
/// an address is said at the foot, beside Send.
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
    let wrong = composer.refusal(line);
    let copies = line == Line::To && !composer.copies();
    let Some(typing) = composer.typing_mut(line) else { return };
    let fields = Recipients { name: line.name(), chips: &chips, hint: tr!("Name or address"), copies, error: wrong.as_deref(), enabled: true };
    let out = recipient::recipient_line(ui, theme, id, typing, &fields);
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
        // Under the line, on its rule, at the start of its chips (the frame's float).
        let at = pos2(ui.min_rect().left() + recipient::LINE_NAMES + theme.spacing.s3, ui.cursor().top());
        suggestions(ui.ctx(), theme, composer, at, asked);
    }
}

/// The people offered under a recipient field, the highlighted one marked.
fn suggestions(ctx: &egui::Context, theme: &Theme, composer: &Composer, at: Pos2, asked: &mut Vec<Asked>) {
    let found = composer.suggestions().1;
    if found.is_empty() {
        return;
    }
    let rect = popup::show(ctx, theme, suggestions_id(), at, SUGGESTIONS, |ui| {
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

/// The From line's value: the account as a button that opens the menu of the accounts, its bar and label at the right
/// end; `label_most` the widest the label gets.
fn from(ui: &mut Ui, theme: &Theme, mailbox: &Mailbox, composer: &Composer, value: Rect, label_most: f32, asked: &mut Vec<Asked>) {
    let p = &theme.palette;
    let s = &theme.spacing;
    let Some(account) = mailbox.account(composer.account()) else { return };
    let sender = if account.name.is_empty() { account.address.clone() } else { account.name.clone() };
    let spoken = if account.name.is_empty() {
        tr!("From {sender}", sender = account.address)
    } else {
        tr!("From {sender}", sender = tr!("{name} <{address}>", name = account.name, address = account.address))
    };
    let response = ui.interact(value, Id::unique("compose-from"), Sense::click());
    response.widget_info(|| WidgetInfo::labeled(Role::ComboBox, true, &spoken));
    let painter = ui.painter();
    let open = popup::is_open(ui.ctx(), from_menu());
    // Open, it is on the `stage` (the From menu's frame); the frames draw neither the pointer over it, which takes the
    // open look, nor a press, which takes the secondary button's (the rule of states not drawn).
    if response.is_pointer_button_down_on() {
        painter.rect(value, 0.0, p.stage, Stroke::new(1.0, p.ink), StrokeKind::Inside);
    } else if response.hovered() || open {
        painter.rect_filled(value, 0.0, p.stage);
    }
    let mid = value.center().y;
    // The label and the bar at the right end, then the name, the address and the caret from the left.
    let label = cut(ui, &account.label, theme.fonts.mono_s, p.muted, label_most);
    let label_left = value.right() - paint::run_box(label.size().x);
    paint::run_at(painter, pos2(label_left, mid - label.size().y / 2.0), label, p.muted);
    let bar = Rect::from_min_size(pos2(label_left - s.s2 + 2.0 - 3.0, (mid - 6.0).round()), vec2(3.0, 12.0));
    painter.rect_filled(bar, 0.0, theme.account_colour(u64::try_from(account.index).unwrap_or(0)));
    let room = (bar.left() - s.s2 - value.left()).max(1.0);
    let name_most = (value.width() - NAME_ROOM).clamp(1.0, room);
    let name_text = cut(ui, &sender, theme.fonts.body, p.ink, name_most);
    let name_width = name_text.size().x;
    let mut x = value.left();
    paint::run_at(painter, pos2(x, mid - name_text.size().y / 2.0), name_text, p.ink);
    x += paint::run_box(name_width) + s.s1 + 2.0;
    if !account.name.is_empty() {
        let address = tr!("<{address}>", address = account.address);
        let most = (bar.left() - s.s2 - FROM_CARET - s.s1 - 2.0 - x).max(1.0);
        let address = cut(ui, &address, theme.fonts.body, p.ink, most);
        let width = address.size().x;
        paint::run_at(painter, pos2(x, mid - address.size().y / 2.0), address, p.ink);
        x += paint::run_box(width) + s.s1 + 2.0;
    }
    if x + FROM_CARET <= bar.left() {
        let mark = Rect::from_min_size(pos2(x, mid - FROM_CARET / 2.0), vec2(FROM_CARET, FROM_CARET));
        crate::iconset::paint(painter, mark, IconId::Caret, p.muted);
    }
    if response.has_focus() {
        paint::focus_ring(painter, theme, value, paint::Ring::Inside);
    }
    let accounts = mailbox.accounts();
    popup::menu(ui.ctx(), theme, &response, from_menu(), FROM_MENU, |ui| {
        let mut chosen = false;
        for choice in accounts {
            let index = u64::try_from(choice.index).unwrap_or(0);
            if popup::account_choice(ui, theme, &choice.label, &choice.address, index, choice.id == account.id).clicked() {
                chosen = true;
                if choice.id != account.id {
                    asked.push(Asked::From(choice.id));
                }
            }
        }
        if accounts.len() == 1 {
            popup::note(ui, theme, tr!("The only account. Another is added in Settings, Accounts."));
        }
        ((), chosen)
    });
}

/// The Templates button and its menu of the account's templates, over it.
fn templates(ui: &mut Ui, theme: &Theme, keymap: &Keymap, composer: &Composer, asked: &mut Vec<Asked>) {
    let button = IconButton::new(IconId::File, tr!("Templates"));
    if composer.templates().is_empty() {
        button.unavailable(tr!("This account has no templates")).show(ui, theme, keymap);
        return;
    }
    // While its menu is open it stands pressed (the menu's frame).
    let open = popup::is_open(ui.ctx(), templates_menu());
    let response = if open { button.shown(crate::component::Shown::Pressed) } else { button }.show(ui, theme, keymap);
    popup::menu_up(ui.ctx(), theme, &response, templates_menu(), TEMPLATES_MENU, |ui| {
        let count = composer.templates().len().to_string();
        popup::title(ui, theme, tr!("Put in a template"), Some(&count));
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

/// The height of the panel that asks for the first blank, in points: 12 in on every side, a field with its label.
fn blank_height(theme: &Theme) -> f32 {
    let s = &theme.spacing;
    s.s3 * 2.0 + theme.fonts.label.line + s.s2 + theme.boxes().field + 2.0
}

/// The panel that asks for the first blank by its prompt: the blank's mark, its field, Fill in and the Enter key.
fn blank(ui: &mut Ui, theme: &Theme, keymap: &Keymap, composer: &mut Composer, rect: Rect, asked: &mut Vec<Asked>) {
    let Some(prompt) = composer.blank() else { return };
    let Some(mark) = composer.blanks().first().cloned() else { return };
    let p = &theme.palette;
    let s = &theme.spacing;
    ui.painter().rect(rect, 0.0, p.panel, Stroke::new(1.0, p.line), StrokeKind::Inside);
    let inner = rect.shrink(1.0 + s.s3);
    let field_height = theme.boxes().field;
    let bottom = inner.bottom();
    // The mark at the left, at the foot of the panel as the field's box is.
    let shown = cut(ui, &mark, theme.fonts.mono, p.cyan_text, (inner.width() / 4.0).max(1.0));
    let mark_rect = Rect::from_min_size(pos2(inner.left(), bottom - field_height), vec2(paint::run_box(shown.size().x) + s.s2 * 2.0, field_height));
    ui.painter().rect_filled(mark_rect, 0.0, p.cyan_muted);
    paint::run_centred(ui.painter(), mark_rect, shown, p.cyan_text);
    let enter = crate::component::key::Chip::new(ui, theme, tr!("Enter"), crate::component::key::ChipState::Default);
    let enter_width = enter.size().x;
    // The button's words and its 12 beside them.
    let fill_width = galley(ui, tr!("Fill in"), theme.fonts.button, p.ink).size().x.ceil() + s.s3 * 2.0;
    let right = inner.right() - enter_width - s.s3 - fill_width - s.s3;
    let field_rect = Rect::from_min_max(pos2(mark_rect.right() + s.s3, inner.top()), pos2(right.max(mark_rect.right() + s.s3 + 1.0), bottom));
    let spoken = tr!("Fill in \"{blank}\"", blank = prompt);
    let response = ui
        .scope_builder(UiBuilder::new().max_rect(field_rect), |ui| {
            let refused = composer.fill_refused().map(str::to_owned);
            let field = Field { error: refused.as_deref(), ..Field::new(tr!("Fill in the blank"), "") };
            let response = field::text_field(ui, theme, blank_id(), composer.filling_mut(), &field);
            // A screen reader hears which blank it fills.
            ui.ctx().accesskit_node_builder(response.id, |node| node.set_label(spoken.as_str()));
            response
        })
        .inner;
    if response.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter)) {
        asked.push(Asked::Fill);
    }
    let fill_rect = Rect::from_min_size(pos2(field_rect.right() + s.s3, bottom - control_side(theme)), vec2(fill_width, control_side(theme)));
    let pressed =
        ui.scope_builder(UiBuilder::new().max_rect(fill_rect), |ui| TextButton::new(tr!("Fill in")).secondary().show(ui, theme, keymap)).inner;
    if pressed.clicked() {
        asked.push(Asked::Fill);
    }
    let top = (fill_rect.center().y - enter.size().y / 2.0).round();
    enter.paint(ui.painter(), pos2(fill_rect.right() + s.s3, top));
}

/// The text laid out in `read` with the marks of the open blanks in `mono` and `cyan_text` on the accent's wash, so
/// they stand out.
fn marked(ui: &Ui, theme: &Theme, marks: &[String], text: &str, width: f32) -> std::sync::Arc<egui::Galley> {
    let p = &theme.palette;
    let plain = theme.fonts.read.format(p.ink);
    let mark = TextFormat { background: p.cyan_muted, ..theme.fonts.mono.format(p.cyan_text) };
    let mut job = LayoutJob::default();
    job.wrap.max_width = width;
    let mut rest = text;
    while !rest.is_empty() {
        let next = marks.iter().filter(|held| !held.is_empty()).filter_map(|held| rest.find(held.as_str()).map(|at| (at, held.len()))).min();
        let Some((at, len)) = next else {
            job.append(rest, 0.0, plain.clone());
            break;
        };
        let (Some(before), Some(found), Some(after)) = (rest.get(..at), rest.get(at..at.saturating_add(len)), rest.get(at.saturating_add(len)..))
        else {
            job.append(rest, 0.0, plain.clone());
            break;
        };
        job.append(before, 0.0, plain.clone());
        job.append(found, 0.0, mark.clone());
        rest = after;
    }
    ui.fonts_mut(|fonts| fonts.layout_job(job))
}

/// What Send says: "Send", or "Schedule · Sat 08:00" with a time picked; and why it cannot go yet, a time picked that
/// has passed among the reasons, and whether that only waits for something to do.
fn send_label(mailbox: &Mailbox, composer: &Composer) -> (String, Option<String>, bool) {
    let now = mailbox.now();
    match composer.later() {
        Some(at) if at > now => {
            (tr!("Schedule · {when}", when = typed_time::short(at, now, mailbox.offset())), composer.problem(), composer.only_waits())
        }
        Some(_) => (
            tr!("Schedule").to_owned(),
            Some(composer.problem().unwrap_or_else(|| tr!("The time picked has passed: pick another").to_owned())),
            composer.problem().is_some() && composer.only_waits(),
        ),
        None => (tr!("Send").to_owned(), composer.problem(), composer.only_waits()),
    }
}

/// The foot in `rect`: Send with its key, the caret of send later, why Send waits; Templates and Discard at the right
/// end, attaching a file before them coming soon. Made in the order the keyboard walks it: Send, send later, Templates,
/// Discard.
fn foot(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, composer: &Composer, rect: Rect, asked: &mut Vec<Asked>) {
    let p = &theme.palette;
    let s = &theme.spacing;
    let side = control_side(theme);
    let mut row = ui.new_child(UiBuilder::new().max_rect(rect).layout(Layout::left_to_right(Align::Center)));
    row.spacing_mut().item_spacing.x = s.s2;
    let (label, problem, waits) = send_label(mailbox, composer);
    // Waiting, Send is the disabled button of its frames, without its key.
    let send = TextButton::new(&label).primary().action(keymap::SEND).key(problem.is_none());
    match &problem {
        Some(problem) => {
            send.unavailable(problem).show(&mut row, theme, keymap);
        }
        None => {
            if send.show(&mut row, theme, keymap).clicked() {
                asked.push(Asked::Send);
            }
        }
    }
    later(&mut row, theme, keymap, mailbox, composer, asked);
    // Templates and Discard at the right end, then what comes soon before them; the reason fills what is between.
    let discard = Rect::from_min_size(pos2(rect.right() - side, rect.center().y - side / 2.0), vec2(side, side));
    let template = discard.translate(vec2(-(side + s.s2), 0.0));
    let attach = template.translate(vec2(-(side + s.s2), 0.0));
    let reason_left = row.cursor().left();
    if let Some(problem) = problem {
        let tag_room = s.s10 * 2.0 + s.s2;
        let reason = Rect::from_min_max(pos2(reason_left, rect.top()), pos2((attach.left() - tag_room).max(reason_left + 1.0), rect.bottom()));
        let colour = if waits { p.muted } else { p.minus };
        let mut x = reason.left() + s.s1;
        let mid = reason.center().y;
        let painter = row.painter();
        if !waits {
            let mark = theme.boxes().icon;
            crate::iconset::paint_glyph(painter, Rect::from_min_size(pos2(x, mid - mark / 2.0), vec2(mark, mark)), WARNING, p.minus);
            x += mark + s.s2 + 2.0;
        }
        let text = cut(&row, &problem, theme.fonts.body, colour, (reason.right() - x).max(1.0));
        let response = row.interact(reason, Id::unique("compose-why"), Sense::hover());
        response.widget_info(|| WidgetInfo::labeled(Role::Label, true, &problem));
        paint::run_at(row.painter(), pos2(x, mid - text.size().y / 2.0), text, colour);
    }
    ui.scope_builder(UiBuilder::new().max_rect(template), |ui| templates(ui, theme, keymap, composer, asked));
    if ui
        .scope_builder(UiBuilder::new().max_rect(discard), |ui| IconButton::new(IconId::Delete, tr!("Discard the draft")).show(ui, theme, keymap))
        .inner
        .clicked()
    {
        asked.push(Asked::Discard);
    }
    // Coming soon: attaching a file, its tag before it.
    let soon_row = Rect::from_min_max(pos2(rect.left(), rect.top()), pos2(attach.right(), rect.bottom()));
    ui.scope_builder(UiBuilder::new().max_rect(soon_row).layout(Layout::right_to_left(Align::Center)), |ui| {
        ui.spacing_mut().item_spacing.x = s.s2;
        soon(ui, theme, |ui| {
            IconButton::new(IconId::Attach, tr!("Attach a file")).show(ui, theme, keymap);
            soon_tag(ui, theme);
        });
    });
}

/// The head in `rect`: the title (in the floating panel a button that folds it), when the draft was kept, the
/// floating panel's "Back to the reader's place", and Close.
#[allow(clippy::too_many_arguments)]
fn head(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, composer: &Composer, rect: Rect, place: Place, asked: &mut Vec<Asked>) {
    let p = &theme.palette;
    let s = &theme.spacing;
    let side = control_side(theme);
    let close = Rect::from_min_size(pos2(rect.right() - side, rect.top()), vec2(side, side));
    let title = composer.title();
    let floating = place == Place::Floating;
    // The title first: the keyboard reaches it before the buttons at the right end.
    let mut right = close.left();
    let dock = floating.then(|| {
        right -= side + s.s2;
        Rect::from_min_size(pos2(right, rect.top()), vec2(side, side))
    });
    let kept = composer.saved().map(|at| tr!("draft kept {time}", time = typed_time::short(at, mailbox.now(), mailbox.offset())));
    let kept = kept.map(|kept| galley(ui, &kept, theme.fonts.mono_s, p.muted));
    let kept_width = kept.as_ref().map_or(0.0, |kept| paint::run_box(kept.size().x) + s.s2);
    let title_rect = Rect::from_min_max(rect.min, pos2((right - s.s2 - kept_width).max(rect.left() + 1.0), rect.bottom()));
    let words = cut(ui, &title, theme.fonts.heading, p.ink, title_rect.width());
    if floating {
        let response = ui.interact(title_rect, Id::unique("compose-fold"), Sense::click());
        let spoken = if composer.folded() { tr!("Open the draft {title}", title = title) } else { tr!("Fold the draft {title}", title = title) };
        response.widget_info(|| WidgetInfo::labeled(Role::Button, true, &spoken));
        if response.hovered() || response.is_pointer_button_down_on() {
            ui.painter().rect_filled(title_rect, 0.0, p.stage);
        }
        if response.has_focus() {
            paint::focus_ring(ui.painter(), theme, title_rect, paint::Ring::Inside);
        }
        if response.clicked() {
            asked.push(Asked::Fold);
        }
    } else {
        let response = ui.interact(title_rect, Id::unique("compose-title"), Sense::hover());
        response.widget_info(|| WidgetInfo::labeled(Role::Label, true, &title));
    }
    paint::run_at(ui.painter(), pos2(title_rect.left(), title_rect.center().y - words.size().y / 2.0), words, p.ink);
    if let Some(kept) = kept {
        let top = title_rect.center().y - kept.size().y / 2.0;
        paint::run_at(ui.painter(), pos2(title_rect.right() + s.s2, top), kept, p.muted);
    }
    if let Some(dock) = dock
        && ui
            .scope_builder(UiBuilder::new().max_rect(dock), |ui| {
                IconButton::new(IconId::Dock, tr!("Back to the reader's place")).show(ui, theme, keymap)
            })
            .inner
            .clicked()
    {
        asked.push(Asked::Open);
    }
    if ui
        .scope_builder(UiBuilder::new().max_rect(close), |ui| IconButton::new(IconId::Close, tr!("Close; the draft is kept")).show(ui, theme, keymap))
        .inner
        .clicked()
    {
        asked.push(Asked::Close);
    }
}

/// The height of a line of the composer with one row: 6 above and below a chip's 24.
fn line_height(theme: &Theme) -> f32 {
    theme.boxes().pill + theme.spacing.s3
}

/// The lines From, To (Cc and Bcc once shown) and Subject, each with a hairline under it, a hairline over the first.
fn lines(ui: &mut Ui, theme: &Theme, mailbox: &Mailbox, composer: &mut Composer, place: Place, asked: &mut Vec<Asked>) {
    let p = &theme.palette;
    let s = &theme.spacing;
    let height = line_height(theme);
    let value_left = recipient::LINE_NAMES + s.s3;
    paint::rule(ui, p.panel_line);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    name(ui, theme, rect, tr!("From"));
    let value = Rect::from_min_max(pos2(rect.left() + value_left, rect.top() + s.s2 - 2.0), pos2(rect.right(), rect.bottom() - s.s2 + 2.0));
    let label_most = if place == Place::Docked { LABEL_MOST[0] } else { LABEL_MOST[1] };
    from(ui, theme, mailbox, composer, value, label_most, asked);
    paint::rule(ui, p.panel_line);
    for line in composer.drawn() {
        recipients(ui, theme, composer, *line, asked);
        paint::rule(ui, p.panel_line);
    }
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    name(ui, theme, rect, tr!("Subject"));
    let value = Rect::from_min_max(pos2(rect.left() + value_left, rect.top()), rect.max);
    let changed = ui
        .scope_builder(UiBuilder::new().max_rect(value), |ui| {
            let cap = field::Cap::Bytes(crate::compose::MAX_SUBJECT_BYTES);
            field::plain_line(ui, theme, subject_id(), composer.subject_mut(), tr!("Subject"), tr!("What it is about"), cap, height).changed()
        })
        .inner;
    if changed {
        asked.push(Asked::Edited);
    }
    paint::rule(ui, p.panel_line);
}

/// The text in `rect`: on `term` in a 1 px edge, `cyan_edge` while it has the keys, 16 in, scrolling.
fn text(ui: &mut Ui, theme: &Theme, composer: &mut Composer, rect: Rect, asked: &mut Vec<Asked>) {
    let p = &theme.palette;
    let focused = ui.memory(|memory| memory.has_focus(text_id()));
    ui.painter().rect(rect, 0.0, p.term, Stroke::new(1.0, if focused { p.cyan_edge } else { p.line }), StrokeKind::Inside);
    let view = rect.shrink(1.0 + theme.spacing.s4);
    let rows = (view.height() / theme.fonts.read.line).floor().max(1.0);
    let marks = composer.blanks().to_vec();
    let mut layouter = |ui: &Ui, text: &dyn egui::TextBuffer, width: f32| marked(ui, theme, &marks, text.as_str(), width);
    let mut child = ui.new_child(UiBuilder::new().max_rect(view));
    child.set_clip_rect(view.intersect(ui.clip_rect()));
    let area = egui::ScrollArea::vertical().id_salt("compose-text-scroll").max_height(view.height()).auto_shrink([false, false]);
    crate::component::scroll::show(&mut child, theme, area, |ui| {
        // A whole number of rows, fewer than a few hundred: the cast keeps it.
        let response = field::plain_text(ui, text_id(), composer.text_mut(), rows as usize, &mut layouter, tr!("Message text"), theme);
        if response.changed() {
            asked.push(Asked::Edited);
        }
    });
}

/// The composer in `area` (inside its pads): the head, the lines, the text, the blank's panel and the foot, the foot
/// at the bottom of `area` whatever the lines hold.
fn body(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, composer: &mut Composer, area: Rect, place: Place) -> Vec<Asked> {
    let s = &theme.spacing;
    let side = control_side(theme);
    let mut asked = Vec::new();
    let head_rect = Rect::from_min_size(area.min, vec2(area.width(), side));
    if place == Place::Docked {
        let mut child = ui.new_child(UiBuilder::new().max_rect(head_rect));
        super::frame::beside(&mut child, theme, |ui| {
            let rect = ui.max_rect();
            ui.allocate_rect(rect, Sense::hover());
            head(ui, theme, keymap, mailbox, composer, rect, place, &mut asked);
        });
    } else {
        head(ui, theme, keymap, mailbox, composer, head_rect, place, &mut asked);
    }
    if composer.folded() && place == Place::Floating {
        return asked;
    }
    let foot_rect = Rect::from_min_max(pos2(area.left(), area.bottom() - side), area.max);
    let blank_room = if composer.blank().is_some() { blank_height(theme) + s.s3 } else { 0.0 };
    let lines_top = head_rect.bottom() + s.s3;
    let mut column = ui.new_child(UiBuilder::new().max_rect(Rect::from_min_max(pos2(area.left(), lines_top), pos2(area.right(), foot_rect.top()))));
    column.set_clip_rect(area.intersect(ui.clip_rect()));
    column.spacing_mut().item_spacing = Vec2::ZERO;
    lines(&mut column, theme, mailbox, composer, place, &mut asked);
    // The text takes what is left over the foot, and three of its rows at least.
    let top = column.cursor().top() + s.s3;
    let least = theme.fonts.read.line * 3.0 + (s.s4 + 1.0) * 2.0;
    let bottom = (foot_rect.top() - s.s3 - blank_room).max(top + least);
    let text_rect = Rect::from_min_max(pos2(area.left(), top), pos2(area.right(), bottom));
    text(&mut column, theme, composer, text_rect, &mut asked);
    if blank_room > 0.0 {
        let rect = Rect::from_min_size(pos2(area.left(), text_rect.bottom() + s.s3), vec2(area.width(), blank_height(theme)));
        blank(&mut column, theme, keymap, composer, rect, &mut asked);
    }
    foot(ui, theme, keymap, mailbox, composer, foot_rect, &mut asked);
    asked
}

/// How long the composer takes to wipe into the reader's place, in seconds: the design's `medium`, 300 ms (M7, as the
/// presentation opens it).
const WIPE: f32 = 0.3;

/// The design's `ease`, `cubic-bezier(.2, .7, .2, 1)`, at `t` (0 to 1): a curve local to the composer until the app
/// has one of its own.
pub(super) fn ease(t: f32) -> f32 {
    const X: [f32; 2] = [0.2, 0.2];
    const Y: [f32; 2] = [0.7, 1.0];
    let t = if t.is_finite() { t.clamp(0.0, 1.0) } else { 1.0 };
    let bezier = |p: [f32; 2], s: f32| 3.0 * (1.0 - s) * (1.0 - s) * s * p[0] + 3.0 * (1.0 - s) * s * s * p[1] + s * s * s;
    // The curve's parameter whose x is `t`, halved 20 times: x grows with it, so this finds it to a millionth.
    let (mut low, mut high) = (0.0_f32, 1.0_f32);
    for _ in 0..20 {
        let mid = (low + high) / 2.0;
        if bezier(X, mid) < t {
            low = mid;
        } else {
            high = mid;
        }
    }
    bezier(Y, (low + high) / 2.0)
}

/// How far the wipe of `id` has come, 0 to 1 over [`WIPE`] from the first frame it was drawn in (a frame missed
/// starts it again), not eased; 1 with reduced motion. A key, typing or a press during it ends it at once: the owner
/// never waits. A repaint is asked for while it runs.
fn wiped(ctx: &egui::Context, id: Id) -> f32 {
    if crate::motion::reduced(ctx) {
        return 1.0;
    }
    let now = ctx.input(|input| input.time);
    let pass = ctx.cumulative_pass_nr();
    let acted = ctx.input(|input| {
        input.events.iter().any(|event| {
            matches!(
                event,
                egui::Event::Key { pressed: true, .. }
                    | egui::Event::Text(_)
                    | egui::Event::Paste(_)
                    | egui::Event::PointerButton { pressed: true, .. }
            )
        })
    });
    let start = ctx.data_mut(|data| {
        let start = match data.get_temp::<(f64, u64)>(id) {
            Some((start, last)) if last.saturating_add(1) >= pass => start,
            _ => now,
        };
        let start = if acted { now - f64::from(WIPE) } else { start };
        data.insert_temp(id, (start, pass));
        start
    });
    // A share of a fraction of a second: the cast loses nothing that shows.
    let shown = ((now - start) / f64::from(WIPE)) as f32;
    let shown = if shown.is_finite() { shown.clamp(0.0, 1.0) } else { 1.0 };
    if shown < 1.0 {
        ctx.request_repaint();
    }
    shown
}

/// Draws the composer into `ui`, in the reader's place: 24 in above and below, 40 beside. It wipes in from the top
/// behind a 1 px cyan line as it takes the place (M7). What was asked for, applied after the frame.
pub(super) fn show(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, composer: &mut Composer) -> Vec<Asked> {
    let s = &theme.spacing;
    let whole = ui.max_rect();
    ui.allocate_rect(whole, Sense::hover());
    mark_drawn(ui, whole);
    let area = Rect::from_min_max(whole.min + vec2(s.s10, s.s6), whole.max - vec2(s.s10, s.s6));
    composer.drawn_in(true);
    let shown = wiped(ui.ctx(), Id::unique("compose-wipe").with(composer.draft()));
    if shown >= 1.0 {
        let asked = body(ui, theme, keymap, mailbox, composer, area, Place::Docked);
        // Painted with nothing over it: from the next frame on, what Send would send has been seen.
        composer.mark_painted(ui.ctx().cumulative_frame_nr());
        return asked;
    }
    let edge = whole.top() + whole.height() * ease(shown);
    // Drawn and working whole; what the wipe has not reached yet is covered by the ground it stands on.
    let asked = body(ui, theme, keymap, mailbox, composer, area, Place::Docked);
    let cover = Rect::from_min_max(pos2(whole.left(), edge), whole.max);
    ui.painter().rect_filled(cover, 0.0, theme.palette.surface);
    // The scan line runs down with the edge, fading over the last tenth.
    let fade = if shown < 0.9 { 1.0 } else { ((1.0 - shown) / 0.1).clamp(0.0, 1.0) };
    let line = crate::theme::mix(theme.palette.surface, theme.palette.cyan_edge, fade);
    paint::hairline(ui.painter(), pos2(whole.left(), edge), pos2(whole.right(), edge), line);
    asked
}

/// The floating panel at the bottom right of `screen`, 3 px clear of its border: the whole composer, or folded its
/// head only. What was asked for.
pub(super) fn floating(ctx: &egui::Context, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, composer: &mut Composer, screen: Rect) -> Vec<Asked> {
    let s = &theme.spacing;
    let room = vec2((screen.width() - CLEAR * 2.0).max(1.0), (screen.height() - CLEAR * 2.0).max(1.0));
    let folded_height = s.s3 * 2.0 + control_side(theme);
    let size = vec2(FLOATING.x.min(room.x), if composer.folded() { folded_height } else { FLOATING.y }.min(room.y));
    let corner = screen.right_bottom() - vec2(CLEAR, CLEAR);
    let rect = Rect::from_min_size(corner - size, size);
    Area::new(Id::unique("compose-floating"))
        .order(Order::Middle)
        .pivot(Align2::LEFT_TOP)
        .fixed_pos(rect.min)
        .fade_in(false)
        .show(ctx, |ui| {
            ui.set_clip_rect(rect.intersect(screen));
            for shape in paint::float_shapes(theme, rect, paint::FloatEdge::Keyboard) {
                ui.painter().add(shape);
            }
            ui.allocate_rect(rect, Sense::hover());
            mark_drawn(ui, rect);
            // 12 above the head, 20 beside and under the rest.
            let pad = s.s4 + s.s1;
            let area = Rect::from_min_max(rect.min + vec2(pad, s.s3), rect.max - vec2(pad, if composer.folded() { s.s3 } else { pad }));
            composer.drawn_in(false);
            let asked = body(ui, theme, keymap, mailbox, composer, area, Place::Floating);
            // Folded, its lines are not drawn: nothing of it has been seen.
            if !composer.folded() {
                composer.mark_painted(ui.ctx().cumulative_frame_nr());
            }
            asked
        })
        .inner
}
