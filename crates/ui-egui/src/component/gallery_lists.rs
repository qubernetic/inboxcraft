// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The gallery page of the mail lists' rows for `ui_shot` (`"lists": true`): the boards of the mail row, the group
//! head, the outbox row, the later row and the draft row, every state drawn as its sheet says with the sheet's words,
//! labelled in the sheet's order, on the boards' `bg`, two columns. A page of its own: one picture is at most 4 096 px a
//! side. The states' samples are here, so the state tests of the sets draw what the boards draw.

use egui::{Id, Rect, Ui, UiBuilder, Vec2, pos2, vec2};

use crate::iconset::IconId;
use crate::keymap::{self, Keymap};
use crate::theme::Theme;
use crate::tr;

use super::Shown;
use super::gallery::{cell, cells, title};
use super::hover::Action;
use super::group_head::{self, GroupHead};
use super::mail_row::{self, Mark, MailRow};
use super::margin;
use super::tag::TagKind;

/// The page's size in points.
#[must_use]
pub fn size() -> Vec2 {
    vec2(WIDTH, HEIGHT)
}

/// The page's width, in points: two columns of boards, two rows of 440 each.
const WIDTH: f32 = 2000.0;

/// The page's height, in points: the taller column and its margins.
const HEIGHT: f32 = 1240.0;

/// What draws a column of the page.
type Draw = fn(&mut Ui, &Theme, &Keymap);

/// The room between two boards, in points.
const BOARD_GAP: f32 = 24.0;

/// The width of every row's board, in points.
pub(crate) const ROW_WIDTH: f32 = 440.0;

/// The hover actions of a row on the boards: those of the list's rows, in the list's order.
pub(crate) fn row_actions() -> [Action<'static>; 4] {
    [
        Action { icon: IconId::Done, name: tr!("Done"), action: Some(keymap::DONE), unavailable: None },
        Action { icon: IconId::MarkRead, name: tr!("Mark as read"), action: Some(keymap::READ), unavailable: None },
        Action { icon: IconId::Delete, name: tr!("Delete"), action: Some(keymap::DELETE), unavailable: None },
        Action { icon: IconId::Snooze, name: tr!("Snooze"), action: Some(keymap::SNOOZE), unavailable: Some(tr!("Open the message to snooze it")) },
    ]
}

/// One state of the mail row's sheet.
#[derive(Debug, Clone, Copy)]
pub(crate) struct MailState {
    /// Its name on the sheet.
    pub name: &'static str,
    /// The row it draws.
    pub sender: &'static str,
    pub line: &'static str,
    pub time: &'static str,
    pub unread: bool,
    pub current: bool,
    pub account: u64,
    pub tag: Option<(&'static str, TagKind)>,
    pub marks: &'static [Mark],
    pub done: bool,
    pub checked: bool,
    /// Hovered or focused.
    pub shown: Shown,
}

/// The sheet's default sender, line and time.
const TEODOR: &str = "Teodor Vasquez";
const DRAFT_LOGO: &str = "Draft logo for the bakery — Hi Odile, two directions attached. The first is close to the sketch you sent, the second is bolder.";
const INVOICE: &str = "Invoice 2026-114 — Attached, due on the 30th.";
const INVITATION: &str = "Updated invitation: Design review, Mon 16 Nov 15:00 — Moved by an hour, Ines has a call until then.";

/// A read row of the sheet's default words, on the second account.
const READ: MailState = MailState {
    name: "read",
    sender: TEODOR,
    line: DRAFT_LOGO,
    time: "14:02",
    unread: false,
    current: false,
    account: 1,
    tag: None,
    marks: &[],
    done: false,
    checked: false,
    shown: Shown::Rest,
};

/// The mail row's states, in the sheet's order.
pub(crate) fn mail_states() -> [MailState; 21] {
    [
        READ,
        MailState { name: "unread", unread: true, ..READ },
        MailState { name: "selected", current: true, ..READ },
        MailState { name: "hover", unread: true, shown: Shown::Hover, ..READ },
        MailState { name: "focus", unread: true, shown: Shown::Focus, ..READ },
        MailState { name: "done", done: true, marks: &[Mark::Done], ..READ },
        // The sheet's still of the dissolve of full motion (stage I-2.7): the words going to dots.
        MailState {
            name: "leaving",
            sender: "Teodor Va·····",
            line: "Draft logo for the b············································",
            time: " ",
            ..READ
        },
        MailState { name: "checked", checked: true, ..READ },
        MailState { name: "checked-unread", checked: true, unread: true, ..READ },
        MailState {
            name: "not-delivered",
            sender: "To Ansel Brook",
            line: "Shelving for the store room: two options — Both fit the back wall.",
            time: "Yesterday",
            tag: Some(("Not delivered", TagKind::Minus)),
            ..READ
        },
        MailState {
            name: "new-sender",
            sender: "Nora Keane",
            line: "Introduction from Marit — Marit said you might help with a sign for our studio.",
            time: "13:20",
            unread: true,
            account: 0,
            tag: Some(("New sender", TagKind::Accent)),
            ..READ
        },
        MailState {
            name: "returned",
            sender: "Ines Okafor",
            line: INVOICE,
            time: "08:00",
            unread: true,
            account: 0,
            tag: Some(("Returned", TagKind::Warn)),
            ..READ
        },
        MailState { name: "attachment", marks: &[Mark::Attachment], ..READ },
        MailState { name: "selected-attachment", current: true, marks: &[Mark::Attachment], ..READ },
        MailState { name: "invitation", sender: "Marit Solheim", line: INVITATION, time: "10:02", unread: true, marks: &[Mark::Invitation], ..READ },
        MailState { name: "invitation-read", sender: "Marit Solheim", line: INVITATION, time: "10:02", marks: &[Mark::Invitation], ..READ },
        MailState {
            name: "selected-invitation",
            sender: "Marit Solheim",
            line: INVITATION,
            time: "10:02",
            current: true,
            marks: &[Mark::Invitation],
            ..READ
        },
        MailState { name: "flagged", marks: &[Mark::Flag], ..READ },
        MailState { name: "pinned", marks: &[Mark::Pin], ..READ },
        MailState { name: "snoozed", sender: "Ines Okafor", line: INVOICE, time: "back Wed 08:00", account: 0, marks: &[Mark::Snoozed], ..READ },
        MailState {
            name: "long-text",
            sender: "Annelise Vandenberghe-Okonkwo-Halvorssen",
            line: "Minutes of the quarterly studio planning meeting, with the kiln rota and the open day plan for the spring",
            unread: true,
            tag: Some(("New sender", TagKind::Accent)),
            marks: &[Mark::Attachment],
            ..READ
        },
    ]
}

/// What a screen reader hears for the mail row of `state`.
pub(crate) fn mail_spoken(state: &MailState) -> String {
    mail_row::spoken(state.sender, state.line, state.time, state.unread)
}

/// The mail row of `state`, 440 wide, at `ui`'s cursor; the row drawn.
pub(crate) fn mail(ui: &mut Ui, theme: &Theme, keymap: &Keymap, state: &MailState) -> mail_row::Drawn {
    let spoken = mail_spoken(state);
    let row = MailRow {
        id: Id::unique("gallery-mail-row").with(state.name),
        spoken: &spoken,
        sender: state.sender,
        line: state.line,
        time: state.time,
        unread: state.unread,
        current: state.current,
        last: false,
        account: Some(state.account),
        tag: state.tag,
        marks: state.marks,
        done: state.done,
        checked: state.checked,
    };
    let high = Theme::size(theme, mail_row::HEIGHT);
    ui.allocate_ui(vec2(ROW_WIDTH, high), |ui| {
        ui.set_width(ROW_WIDTH);
        let ground = Rect::from_min_size(ui.cursor().min, vec2(ROW_WIDTH, high));
        ui.painter().rect_filled(ground, 0.0, theme.palette.surface);
        let shown = (state.shown != Shown::Rest).then_some(state.shown);
        mail_row::mail_row(ui, theme, keymap, &row, false, &row_actions(), shown)
    })
    .inner
}

/// The mail row's board: every state of its sheet, two to a line.
fn mail_rows(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    title(ui, theme, tr!("Mail row"));
    for pair in mail_states().chunks(2) {
        cells(ui, theme, |ui| {
            for state in pair {
                cell(ui, theme, state.name, |ui| {
                    mail(ui, theme, keymap, state);
                });
            }
        });
    }
}

/// One state of the group head's sheet.
#[derive(Debug, Clone, Copy)]
pub(crate) struct HeadState {
    pub name: &'static str,
    pub unread: u64,
    pub shown_rows: Option<&'static str>,
    pub link: Option<&'static str>,
    pub shown: Shown,
}

/// The group head's states, in the sheet's order, with the sheet's words.
pub(crate) fn head_states() -> [HeadState; 6] {
    let default = HeadState { name: "default", unread: 6, shown_rows: None, link: None, shown: Shown::Rest };
    [
        default,
        HeadState { name: "hover", shown: Shown::Hover, ..default },
        HeadState { name: "focus", shown: Shown::Focus, ..default },
        HeadState { name: "show-all", shown_rows: Some("3 of 6 ·"), link: Some(tr!("show all")), ..default },
        HeadState { name: "show-fewer", shown_rows: Some("6 of 6 ·"), link: Some(tr!("show fewer")), ..default },
        HeadState { name: "zero", unread: 0, ..default },
    ]
}

/// The name of the group on the head's boards.
pub(crate) const HEAD_NAME: &str = "Notifications";

/// The group head of `state`, 440 wide, at `ui`'s cursor.
pub(crate) fn head(ui: &mut Ui, theme: &Theme, keymap: &Keymap, state: &HeadState) {
    let shown = GroupHead {
        id: Id::unique("gallery-group-head").with(state.name),
        index: "[ B ]",
        name: HEAD_NAME,
        unread: state.unread,
        shown: state.shown_rows,
        link: state.link,
        hover_link: Some(tr!("done all")),
    };
    ui.allocate_ui(vec2(ROW_WIDTH, Theme::size(theme, group_head::HEIGHT)), |ui| {
        ui.set_width(ROW_WIDTH);
        let forced = (state.shown != Shown::Rest).then_some(state.shown);
        group_head::group_head(ui, theme, keymap, &shown, forced);
    });
}

/// The group head's board: every state of its sheet, two to a line.
fn group_heads(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    title(ui, theme, tr!("Group head"));
    for pair in head_states().chunks(2) {
        cells(ui, theme, |ui| {
            for state in pair {
                cell(ui, theme, state.name, |ui| head(ui, theme, keymap, state));
            }
        });
    }
}

/// The left column: the mail row.
fn left(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    mail_rows(ui, theme, keymap);
}

/// The right column: the group head.
fn right(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    group_heads(ui, theme, keymap);
}

/// Draws the page into `ui` in `theme` with the keys of `keymap`.
pub fn page(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    let s = theme.spacing;
    let whole = Rect::from_min_size(pos2(0.0, 0.0), size());
    ui.painter().rect_filled(whole, 0.0, theme.palette.bg);
    let column = ((WIDTH - s.s2 * 3.0) / 2.0).floor();
    let draws: [(f32, Draw); 2] = [(s.s2, left), (s.s2 * 2.0 + column, right)];
    for (x, draw) in draws {
        let rect = Rect::from_min_size(pos2(x.round(), s.s2), vec2(column, HEIGHT - s.s2 * 2.0));
        ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
            // The boards stand on `bg`, as the design's boards do.
            egui::Frame::NONE.inner_margin(margin(s.s4, s.s3)).show(ui, |ui| {
                ui.set_min_size(ui.available_size());
                draw(ui, theme, keymap);
            });
        });
    }
    let _ = BOARD_GAP;
}
