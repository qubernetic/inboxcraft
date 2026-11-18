// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The thread: folded messages, the header of an unfolded one, one unfolded message and the answers under it.

use egui::{Frame, Margin, RichText, Ui, pos2};
use inboxcraft_html::Block;

use crate::component::button::TextButton;
use crate::component::facts::{Facts, facts_of};
use crate::component::thread_row::{ThreadLine, thread_row};
use crate::compose::Answer;
use crate::draw::margin;
use crate::draw::paint;
use crate::keymap::Keymap;
use crate::mailbox::Mailbox;
use crate::quote::Segment;
use crate::reader::{Content, Header, Opened, Person, Reader, ThreadRow};
use crate::theme::Theme;
use crate::tr;

use super::body::{banner, body, files};
use super::{Asked, Drawn, Textures};

/// `people` by name, joined, as far as one line could show them: a list of thousands is not joined whole every frame.
pub(super) fn names(people: &[Person]) -> String {
    let mut joined = String::new();
    for person in people {
        if joined.chars().count() > crate::component::MOST {
            break;
        }
        if !joined.is_empty() {
            joined.push_str(", ");
        }
        joined.push_str(person.shown());
    }
    joined
}

/// Whether the text of `opened` only says what its invitation card says.
pub(super) fn restated(opened: &Opened) -> bool {
    let Some(invite) = opened.invite.as_ref().filter(|_| !opened.cut && opened.files.is_empty()) else { return false };
    let mut said = String::new();
    let plain = match &opened.content {
        Content::Text(segments) => segments.iter().all(|segment| match segment {
            Segment::Text(text) => {
                said.push_str(text);
                said.push(' ');
                true
            }
            Segment::Quote { .. } => false,
        }),
        Content::Html(document) => document.blocks.iter().all(|block| match block {
            Block::Text { quote: 0, spans, .. } => {
                for span in spans {
                    said.push_str(&span.text);
                    said.push(' ');
                }
                true
            }
            _ => false,
        }),
        Content::Loading | Content::NotKept | Content::TooLarge | Content::Unreadable => false,
    };
    plain && invite.restates(&said)
}

/// Whether `opened` is the open message drawn without a header of its own: the only message of its thread, one the
/// screener holds, or one whose invitation card says all it says. The full header brings its header back.
pub(super) fn bare(mailbox: &Mailbox, reader: &Reader, opened: &Opened) -> bool {
    Some(opened.id) == reader.open_id()
        && (reader.thread().len() == 1 || mailbox.held(opened.id) || (restated(opened) && !reader.shows_details(opened.id)))
}

/// The room inside an unfolded message of a thread: above, at the sides and under its text, in points.
const TEXT_TOP: f32 = 12.0;
const TEXT_SIDE: f32 = 12.0;
const TEXT_FOOT: f32 = 20.0;

/// The room between the parts of a message, in points.
pub(super) const PART_GAP: f32 = 16.0;

/// The room between two parts of an unfolded message of a thread, in points.
const TEXT_GAP: f32 = 12.0;

/// The line of `row` in the thread: folded, or open as its message's head.
fn line_of<'a>(mailbox: &Mailbox, row: &'a ThreadRow, open: Option<&'a str>) -> (String, ThreadLine<'a>, String) {
    let date = row.date.map(|date| mailbox.short_date(date)).unwrap_or_default();
    let unread = mailbox.messages().iter().any(|message| message.id == row.id && !message.seen);
    let preview = open.map_or_else(|| row.preview.clone(), str::to_owned);
    (date, ThreadLine { sender: &row.sender, address: &row.address, preview: "", date: "", unread, open: open.is_some() }, preview)
}

/// One folded message of the thread on one line ([`thread_row`]): a click unfolds it.
pub(super) fn folded(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, row: &ThreadRow, drawn: &mut Drawn) {
    let (date, line, preview) = line_of(mailbox, row, None);
    let line = ThreadLine { preview: &preview, date: &date, ..line };
    if thread_row(ui, theme, keymap, &line).row.clicked() {
        drawn.asked.push(Asked::Expand(row.id));
    }
}

/// The head of an unfolded message of a thread: its line open, the address and to whom in place of the first words,
/// `details ▾` opening its facts. A click folds it again, but for the open message.
fn head_line(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, reader: &Reader, opened: &Opened, drawn: &mut Drawn) {
    let Some(row) = reader.thread().iter().find(|row| row.id == opened.id) else { return };
    let mut whom = names(&opened.header.to);
    if !opened.header.cc.is_empty() {
        whom = tr!("{to}, copy to {cc}", to = whom, cc = names(&opened.header.cc));
    }
    let address = opened.header.from.first().map_or_else(|| row.address.clone(), |person| person.address.clone());
    let about = tr!("{address}  ·  to {whom}", address = address, whom = whom);
    let (date, line, preview) = line_of(mailbox, row, Some(&about));
    let line = ThreadLine { preview: &preview, date: &date, unread: false, ..line };
    let pressed = thread_row(ui, theme, keymap, &line);
    if pressed.details {
        drawn.asked.push(Asked::Details(opened.id));
    } else if pressed.row.clicked() && Some(opened.id) != reader.open_id() {
        drawn.asked.push(Asked::Expand(opened.id));
    }
}

/// `people` as the facts write them: each name with the address after two spaces, or the address alone.
pub(super) fn addressed(people: &[Person]) -> String {
    people
        .iter()
        .map(|person| if person.name.is_empty() { person.address.clone() } else { format!("{}  <{}>", person.name, person.address) })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Whether the invitation's card of `opened` stands alone for it: the open message, alone in its thread, whose text only
/// says what the card says. It then has no answers at its foot.
pub(super) fn card_alone(mailbox: &Mailbox, reader: &Reader, opened: &Opened) -> bool {
    bare(mailbox, reader, opened) && !mailbox.held(opened.id) && restated(opened) && opened.invite.is_some()
}

/// The facts of `opened` with the details open, under its line in a thread.
fn details(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, opened: &Opened, drawn: &mut Drawn) {
    let Header { from, reply_to, to, cc, date, .. } = &opened.header;
    let full = addressed;
    let (from, to, cc, reply_to) = (full(from), full(to), full(cc), full(reply_to));
    let date = date.map(|date| mailbox.full_date(date)).unwrap_or_default();
    let shown = Facts { from: &from, to: &to, date: &date, cc: &cc, reply_to: &reply_to, checked: "", id: "", recipients: None };
    if facts_of(ui, theme, keymap, &shown, true).clicked() {
        drawn.asked.push(Asked::Details(opened.id));
    }
}

/// Reply, the pane's one primary button, with its arrow; then Reply all and Forward: the answers at the reader's foot.
fn answer_buttons() -> [(TextButton<'static>, Answer); 3] {
    [
        (TextButton::new(tr!("Reply")).primary().arrow(), Answer::Reply),
        (TextButton::new(tr!("Reply all")).secondary(), Answer::ReplyAll),
        (TextButton::new(tr!("Forward")).secondary(), Answer::Forward),
    ]
}

/// The line each answer stands on in `room` points: all on one line where they fit; else, as the acts do, as many on a
/// line as fit, in order, so none is cut at the pane's edge.
fn answer_lines(ui: &Ui, theme: &Theme, keymap: &Keymap, room: f32) -> [usize; 3] {
    let mut lines = [0_usize; 3];
    let (mut line, mut used) = (0_usize, 0.0_f32);
    for (index, (button, _)) in answer_buttons().iter().enumerate() {
        let wide = button.width(ui, theme, keymap);
        if used > 0.0 && used + ANSWER_GAP + wide > room {
            line = line.saturating_add(1);
            used = 0.0;
        }
        used += if used > 0.0 { ANSWER_GAP + wide } else { wide };
        if let Some(slot) = lines.get_mut(index) {
            *slot = line;
        }
    }
    lines
}

/// How high the answers stand in `room` points: one control, or one per line and the gaps between.
pub(super) fn answers_height(ui: &Ui, theme: &Theme, keymap: &Keymap, room: f32) -> f32 {
    let count = answer_lines(ui, theme, keymap, room).iter().max().copied().unwrap_or(0) as f32 + 1.0;
    count * crate::component::control_side(theme) + (count - 1.0) * ANSWER_GAP
}

/// The answers at the reader's foot, on the lines [`answer_lines`] gives them.
pub(super) fn answers(ui: &mut Ui, theme: &Theme, keymap: &Keymap, id: u64, drawn: &mut Drawn) {
    let room = ui.available_width();
    let lines = answer_lines(ui, theme, keymap, room);
    let side = crate::component::control_side(theme);
    let (_, whole) = ui.allocate_space(egui::vec2(room.max(0.0), answers_height(ui, theme, keymap, room)));
    let mut line_ui: Option<(usize, Ui)> = None;
    for ((button, answer), line) in answer_buttons().into_iter().zip(lines) {
        if line_ui.as_ref().is_none_or(|(at, _)| *at != line) {
            let top = whole.top() + line as f32 * (side + ANSWER_GAP);
            let place = egui::Rect::from_min_size(egui::pos2(whole.left(), top), egui::vec2(whole.width(), side));
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(place).layout(egui::Layout::left_to_right(egui::Align::Center)));
            child.spacing_mut().item_spacing.x = ANSWER_GAP;
            line_ui = Some((line, child));
        }
        if let Some((_, child)) = line_ui.as_mut()
            && button.show(child, theme, keymap).clicked()
        {
            drawn.asked.push(Asked::Answer(id, answer));
        }
    }
}

/// The room between two answers, in points.
const ANSWER_GAP: f32 = 8.0;

/// One unfolded message. Alone in its thread (see [`bare`]) its parts stand on the pane: the remote-content banner,
/// the invitation's card, the text and the attachments, 16 apart; the invitation's card alone stands for a message it
/// says all of. In a longer thread its line opens it and its parts stand in a block with a `panel_line` under it.
#[expect(clippy::too_many_arguments, reason = "the drawing context of one message")]
pub(super) fn message(
    ui: &mut Ui,
    theme: &Theme,
    keymap: &Keymap,
    mailbox: &Mailbox,
    reader: &Reader,
    opened: &Opened,
    textures: &mut Textures,
    drawn: &mut Drawn,
) {
    let open = Some(opened.id) == reader.open_id();
    let held = open && mailbox.held(opened.id);
    let bare = bare(mailbox, reader, opened);
    if let (true, false, true, Some(invite)) = (bare, held, restated(opened), &opened.invite) {
        crate::draw::invite::card(ui, theme, keymap, reader, invite, drawn);
        return;
    }
    let parts = |ui: &mut Ui, textures: &mut Textures, drawn: &mut Drawn| {
        banner(ui, theme, keymap, reader, opened, drawn);
        if held && bare {
            let subject = if opened.header.subject.is_empty() { tr!("(no subject)") } else { opened.header.subject.as_str() };
            ui.add(egui::Label::new(RichText::new(subject).font(theme.fonts.heading.id()).color(theme.palette.ink)).wrap());
        }
        if let Some(invite) = &opened.invite {
            crate::draw::invite::card(ui, theme, keymap, reader, invite, drawn);
        }
        body(ui, theme, reader, opened, textures, drawn);
        files(ui, theme, keymap, opened.id, &opened.files, drawn);
    };
    if bare {
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.y = PART_GAP;
            parts(ui, textures, drawn);
        });
        return;
    }
    head_line(ui, theme, keymap, mailbox, reader, opened, drawn);
    let block = Frame::new()
        .inner_margin(Margin {
            left: margin(TEXT_SIDE).left,
            right: margin(TEXT_SIDE).right,
            top: margin(TEXT_TOP).top,
            bottom: margin(TEXT_FOOT).bottom,
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = TEXT_GAP;
            if reader.shows_details(opened.id) {
                details(ui, theme, keymap, mailbox, opened, drawn);
            }
            parts(ui, textures, drawn);
        });
    let rect = block.response.rect;
    paint::hairline(ui.painter(), pos2(rect.left(), rect.bottom() - 1.0), pos2(rect.right() - 1.0, rect.bottom() - 1.0), theme.palette.panel_line);
}
