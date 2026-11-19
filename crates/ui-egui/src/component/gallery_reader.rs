// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The gallery page of the reader's sets for `ui_shot` (`"reader": true`): the boards of the why line, the content
//! bar, the thread row, the quote fold, the attachment, the hairline table, the invitation card and the picture box,
//! every state drawn as its sheet says, labelled in the sheet's order with the design boards' words, and a fitted
//! message with its quote folded and open, on the boards' `bg`, three columns, the boards evenly apart. A page of its
//! own: one picture is at most 4 096 px a side.

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
                let parts: Vec<&str> = reasons.split(super::why_line::SEPARATOR).collect();
                why_line(ui, theme, keymap, &parts);
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

/// The quote fold's board: folded, open and hover, 560 wide, with the design board's words.
fn quote_folds(ui: &mut Ui, theme: &Theme) {
    use super::quote_fold::quote_fold_with;
    title(ui, theme, tr!("Quote fold"));
    let said = "On Mon 16 Nov 2026 at 14:02, Teodor Vasquez wrote:";
    let quoted = "Hi Odile, two directions attached. The first is close to the sign they have now. The second keeps the colours and \
                  changes the type.";
    for (state, open, shown) in [(tr!("folded"), false, Shown::Rest), (tr!("open"), true, Shown::Rest), (tr!("hover"), false, Shown::Hover)] {
        cells(ui, theme, |ui| {
            cell(ui, theme, state, |ui| {
                ui.set_width(560.0);
                quote_fold_with(ui, theme, 14, open, Some(said), quoted, Some(shown));
            });
        });
    }
}

/// The attachment's board: default, hover, focus, saving, saved and failed, with the design board's words; opening a
/// file or its folder is not built, so the board carries the coming-soon tag once.
fn attachments(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    use super::attachment::{FileState, attachment_with};
    title(ui, theme, tr!("Attachment"));
    super::soon::soon_tag(ui, theme);
    let states = [
        (tr!("default"), FileState::Ready, Shown::Rest),
        (tr!("hover"), FileState::Ready, Shown::Hover),
        (tr!("focus"), FileState::Ready, Shown::Focus),
        (tr!("saving"), FileState::Saving("saving [=====>  ]"), Shown::Rest),
        (tr!("saved"), FileState::Saved(tr!("saved to Downloads")), Shown::Rest),
        (tr!("failed"), FileState::Failed(tr!("not saved: the disk is full")), Shown::Rest),
    ];
    for (state, file, shown) in states {
        cells(ui, theme, |ui| {
            cell(ui, theme, state, |ui| {
                attachment_with(ui, theme, keymap, "bakery-logo-v2.pdf", "1.2 MB", file, Some(shown));
            });
        });
    }
}

/// The hairline table's board: the default and a long cell, 560 wide, with the design board's words.
fn tables(ui: &mut Ui, theme: &Theme) {
    use super::hairline_table::{Table, hairline_table};
    title(ui, theme, tr!("Hairline table"));
    let heads = [tr!("Item"), tr!("Qty"), "EUR"];
    let widths = [40.0, 72.0];
    let total = [tr!("Total"), "", "61.90"];
    let short: [&[&str]; 3] = [&["Stoneware teapot", "1", "38.00"], &["Loose leaf Assam 250 g", "2", "19.00"], &["Delivery", "", "4.90"]];
    let long: [&[&str]; 3] = [
        &["Stoneware teapot", "1", "38.00"],
        &["Loose leaf Assam, second flush, from the Halmari estate, 250 g in a tin that can be sent back for a refill.", "2", "19.00"],
        &["Delivery", "", "4.90"],
    ];
    for (state, rows) in [(tr!("default"), &short), (tr!("long-cell"), &long)] {
        cells(ui, theme, |ui| {
            cell(ui, theme, state, |ui| {
                ui.set_width(560.0);
                hairline_table(ui, theme, &Table { heads: &heads, widths: &widths, rows: rows.as_slice(), total: Some(&total) });
            });
        });
    }
}

/// A fitted HTML message's board, 684 wide as the reader sets a message: its quote folded and open.
fn fitted_messages(ui: &mut Ui, theme: &Theme) {
    title(ui, theme, tr!("Fitted message"));
    for (state, open) in [(tr!("quote folded"), false), (tr!("quote open"), true)] {
        cells(ui, theme, |ui| {
            cell(ui, theme, state, |ui| {
                ui.set_width(684.0);
                crate::draw::fitted_board(ui, theme, open);
            });
        });
    }
}

/// The invitation card's board: its nine states, 640 wide, with the design board's words.
fn invite_cards(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    use super::invite_card::{CardEvent, CardState, Said, invite_card};
    title(ui, theme, tr!("Invitation card"));
    let event = CardEvent {
        weekday: "MON",
        day: "16",
        month: "NOV",
        title: "Design review",
        when: "15:00 – 16:00  ·  today",
        place: "Room 3",
        who: "Marit Solheim (organiser), Teodor Vasquez, Ines Okafor and you",
        cal: "Studio",
        not_from_organiser: false,
    };
    let goes = "marit@example.org";
    let replies = [
        (Said::Going, "Teodor Vasquez is going."),
        (Said::Maybe, "Ines Okafor might go."),
        (Said::No, "Pavel Hruska is not going."),
        (Said::Nothing, "Dmitri Vale has not answered."),
    ];
    let states = [
        (tr!("request"), CardState::Request { goes_to: goes }),
        (tr!("changed"), CardState::Changed { changed: "time 14:00 → 15:00", goes_to: goes }),
        (tr!("out-of-date"), CardState::OutOfDate { line: "Out of date: a later invitation replaces this one. You answered Going." }),
        (tr!("answered"), CardState::Answered { answer: "You answered Going." }),
        (tr!("cancelled"), CardState::Cancelled { line: "Marit Solheim cancelled this event.", remove: true }),
        (tr!("reply"), CardState::Replies { replies: &replies }),
        (tr!("no-answer-asked"), CardState::NoAnswerAsked),
        (
            tr!("cannot-answer"),
            CardState::CannotAnswer {
                why: "it was sent to lists@example.org, which is not one of your addresses, and the organiser asked only that address.",
            },
        ),
        (tr!("clash"), CardState::Clash { clash: "Call Ines 15:30 – 15:45 is in the same hour", goes_to: goes }),
    ];
    for (state, card) in states {
        cells(ui, theme, |ui| {
            cell(ui, theme, state, |ui| {
                ui.set_width(640.0);
                // Each card's controls stand apart from the others' for the keyboard and a screen reader.
                ui.push_id(state, |ui| invite_card(ui, theme, keymap, &event, &card));
            });
        });
    }
}

/// The picture box's board: a remote picture not loaded, loading, not shown and failed, each a box of a 360 × 144
/// picture, with the design's words.
fn picture_boxes(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    use super::picture_box::{PictureLook, picture_box_with};
    title(ui, theme, tr!("Picture box"));
    let states = [
        (tr!("not-loaded"), PictureLook::NotLoaded),
        (tr!("loading"), PictureLook::Loading),
        (tr!("not-shown"), PictureLook::NotShown),
        (tr!("failed"), PictureLook::Failed(tr!("no connection"))),
    ];
    for (state, look) in states {
        cells(ui, theme, |ui| {
            cell(ui, theme, state, |ui| {
                ui.set_width(360.0);
                picture_box_with(ui, theme, keymap, look, tr!("The new kiln, lit for the first time"), Some((360, 144)), true);
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
    ui.add_space(BOARD_GAP);
    quote_folds(ui, theme);
    ui.add_space(BOARD_GAP);
    fitted_messages(ui, theme);
}

/// The middle column.
fn middle(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    attachments(ui, theme, keymap);
    ui.add_space(BOARD_GAP);
    tables(ui, theme);
    ui.add_space(BOARD_GAP);
    picture_boxes(ui, theme, keymap);
}

/// The right column.
fn right(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    invite_cards(ui, theme, keymap);
}

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
