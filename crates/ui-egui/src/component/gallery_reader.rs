// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The gallery page of the reader's sets for `ui_shot` (`"reader": true`): the boards of the why line, the content
//! bar, the thread row, the quote fold, the attachment, the hairline table and the invitation card, every state drawn
//! as its sheet says, labelled in the sheet's order with the design boards' words, on the boards' `bg`, three columns,
//! the boards evenly apart. A page of its own: one picture is at most 4 096 px a side.

use egui::{Rect, Ui, UiBuilder, Vec2, pos2, vec2};

use crate::keymap::Keymap;
use crate::theme::Theme;
use crate::tr;

use super::Shown;
use super::gallery::{cell, cells, title};
use super::margin;

/// The page's size in points.
#[must_use]
pub fn size() -> Vec2 {
    vec2(WIDTH, HEIGHT)
}

/// The page's width, in points: three columns of boards.
const WIDTH: f32 = 2400.0;

/// The page's height, in points: the tallest column and its margins.
const HEIGHT: f32 = 2560.0;

/// The room between two boards, in points.
const BOARD_GAP: f32 = 24.0;

/// What draws a column of the page.
type Draw = fn(&mut Ui, &Theme, &Keymap);

/// The why line's board: group, new sender, screener and folder, 640 wide, with the design board's words.
fn why_lines(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    use super::why_line::why_line;
    title(ui, theme, tr!("Why line"));
    let states = [
        (tr!("group"), "let in by you on 3 Nov  ·  grouped as People  ·  synced 14:02 by IDLE"),
        (tr!("new-sender"), "first mail on 14 Nov  ·  let in by you today  ·  grouped as People"),
        (tr!("screener"), "first mail from this address  ·  SPF and DKIM pass  ·  held back"),
        (tr!("folder"), "moved by you on 9 Nov  ·  in Studio / Clients / Bakery"),
    ];
    for (state, reasons) in states {
        cells(ui, theme, |ui| {
            cell(ui, theme, state, |ui| {
                ui.set_width(640.0);
                why_line(ui, theme, keymap, reasons);
            });
        });
    }
}

/// The content bar's board: text, HTML, list, invitation, pictures held, trackers removed and plain text, 720 wide,
/// with the design board's words.
fn content_bars(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    use super::banner::BannerLink;
    use super::content_bar::{BarKind, ContentBar, View, content_bar};
    title(ui, theme, tr!("Content bar"));
    let unsubscribe = [BannerLink { name: tr!("unsubscribe"), id: "unsubscribe" }];
    let pictures = [BannerLink { name: tr!("load once"), id: "once" }, BannerLink { name: tr!("always"), id: "always" }];
    type State<'a> = (&'a str, BarKind, View, &'a str, bool, &'a [BannerLink<'a>]);
    let states: [State<'_>; 7] = [
        (tr!("text"), BarKind::Text, View::Fitted, tr!("plain text, shown as sent"), false, &[]),
        (tr!("html"), BarKind::Html, View::Fitted, tr!("nothing held back"), false, &[]),
        (tr!("list"), BarKind::List, View::Fitted, "craft-dev@lists.example.org", false, &unsubscribe),
        (tr!("invite"), BarKind::Invite, View::Fitted, tr!("read into the card below"), false, &[]),
        (tr!("pictures-held"), BarKind::Html, View::Fitted, "6 pictures held back", true, &pictures),
        (tr!("trackers-removed"), BarKind::Html, View::Fitted, "2 trackers removed · 6 pictures loaded once", false, &[]),
        (tr!("plain-text"), BarKind::Html, View::Plain, tr!("as a text-only program shows it"), false, &[]),
    ];
    for (state, kind, view, held, waiting, links) in states {
        cells(ui, theme, |ui| {
            cell(ui, theme, state, |ui| {
                ui.set_width(720.0);
                content_bar(ui, theme, keymap, &ContentBar { kind, view, views: true, held, waiting, links });
            });
        });
    }
}

/// The thread row's board: folded, open, unread, hover and focus, 640 wide, with the design board's words.
fn thread_rows(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    use super::thread_row::{ThreadLine, thread_row_with};
    title(ui, theme, tr!("Thread row"));
    let folded = ThreadLine {
        sender: "Marit Solheim",
        address: "marit@example.org",
        preview: "Room 3 is free from ten. I can bring the projector, and Ines has the keys to the store room.",
        date: "13:40",
        unread: false,
        open: false,
    };
    let open = ThreadLine { preview: "marit@example.org  ·  to you and Teodor", open: true, ..folded };
    let states = [
        (tr!("folded"), folded, Shown::Rest),
        (tr!("open"), open, Shown::Rest),
        (tr!("unread"), ThreadLine { unread: true, ..folded }, Shown::Rest),
        (tr!("hover"), folded, Shown::Hover),
        (tr!("focus"), folded, Shown::Focus),
    ];
    for (state, line, shown) in states {
        cells(ui, theme, |ui| {
            cell(ui, theme, state, |ui| {
                ui.set_width(640.0);
                // A board's row stands on the reader's surface, as in the design's board.
                ui.painter().rect_filled(Rect::from_min_size(ui.cursor().min, vec2(640.0, theme.boxes().reader[2])), 0.0, theme.palette.surface);
                thread_row_with(ui, theme, keymap, &line, Some(shown));
            });
        });
    }
}

/// The left column.
fn left(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    why_lines(ui, theme, keymap);
    ui.add_space(BOARD_GAP);
    content_bars(ui, theme, keymap);
    ui.add_space(BOARD_GAP);
    thread_rows(ui, theme, keymap);
}

/// The middle column.
fn middle(_ui: &mut Ui, _theme: &Theme, _keymap: &Keymap) {}

/// The right column.
fn right(_ui: &mut Ui, _theme: &Theme, _keymap: &Keymap) {}

/// Draws the page into `ui` in `theme` with the keys of `keymap`.
pub fn page(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    let s = theme.spacing;
    let whole = Rect::from_min_size(pos2(0.0, 0.0), size());
    ui.painter().rect_filled(whole, 0.0, theme.palette.bg);
    let column = ((WIDTH - s.s2 * 4.0) / 3.0).floor();
    let draws: [Draw; 3] = [left, middle, right];
    for (index, draw) in draws.into_iter().enumerate() {
        let x = s.s2 + index as f32 * (column + s.s2);
        let rect = Rect::from_min_size(pos2(x.round(), s.s2), vec2(column, HEIGHT - s.s2 * 2.0));
        ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
            // The boards stand on `bg`, as the design's boards do.
            egui::Frame::NONE.inner_margin(margin(s.s4, s.s3)).show(ui, |ui| {
                ui.set_min_size(ui.available_size());
                draw(ui, theme, keymap);
            });
        });
    }
}

#[cfg(test)]
#[path = "gallery_reader_tests.rs"]
mod tests;
