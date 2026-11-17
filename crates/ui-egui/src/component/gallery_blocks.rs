// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The gallery page of the shared rows and blocks for `ui_shot` (`"blocks": true`): the boards of the row, the hover
//! actions, the banner, the empty state, the progress line, the settings row, the check step and the facts, every state
//! drawn as its sheet says, labelled in the sheet's order with the design boards' words, on the boards' `bg`, two
//! columns, the boards evenly apart. A page of its own: one picture is at most 4 096 px a side.

use egui::{Id, Rect, Ui, UiBuilder, Vec2, pos2, vec2};

use crate::iconset::IconId;
use crate::keymap::Keymap;
use crate::theme::Theme;
use crate::tr;

use super::Shown;
use super::button::TextButton;
use super::gallery::{cell, cells, title};
use super::hover::{self, Action};
use super::segmented::segmented;
use super::{cut, margin};

/// The page's size in points.
#[must_use]
pub fn size() -> Vec2 {
    vec2(WIDTH, HEIGHT)
}

/// The page's width, in points: two columns of boards.
const WIDTH: f32 = 1800.0;

/// The page's height, in points: the taller column and its margins.
const HEIGHT: f32 = 1840.0;

/// What draws a column of the page.
type Draw = fn(&mut Ui, &Theme, &Keymap);

/// The room between two boards, in points.
const BOARD_GAP: f32 = 24.0;

/// The row's board: the mail row's read, hover, selected and focus states, a row on the panel under the pointer, and
/// the last row of a list, without its line.
fn rows(ui: &mut Ui, theme: &Theme) {
    use super::row::{RowGround, row_with};
    title(ui, theme, tr!("Row"));
    let p = &theme.palette;
    let states = [
        (tr!("read"), RowGround::Surface, false, false, Shown::Rest),
        (tr!("hover"), RowGround::Surface, false, false, Shown::Hover),
        (tr!("selected"), RowGround::Surface, true, false, Shown::Rest),
        (tr!("focus"), RowGround::Surface, false, false, Shown::Focus),
        (tr!("panel-hover"), RowGround::Panel, false, false, Shown::Hover),
        (tr!("last"), RowGround::Surface, false, true, Shown::Rest),
    ];
    for pair in states.chunks(1) {
        cells(ui, theme, |ui| {
            for (state, ground, current, last, shown) in pair {
                cell(ui, theme, state, |ui| {
                    ui.allocate_ui(vec2(440.0, theme.boxes().mail_row), |ui| {
                        ui.set_width(440.0);
                        let fill = if *ground == RowGround::Panel { p.panel } else { p.surface };
                        ui.painter().rect_filled(Rect::from_min_size(ui.cursor().min, vec2(440.0, theme.boxes().mail_row)), 0.0, fill);
                        let id = Id::unique("gallery-row-board").with(*state);
                        let drawn = row_with(ui, theme, id, 56.0, *ground, *current, *last, Some(*shown));
                        let name = format!("{} {state}", tr!("Row"));
                        drawn.response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &name));
                        let text = cut(ui, "Teodor Vasquez", theme.fonts.body, p.ink, 300.0);
                        let at = drawn.rect.left_top() + vec2(34.0, 7.0);
                        ui.painter().galley(at, text, p.ink);
                        drawn.finish(ui, theme);
                    });
                });
            }
        });
    }
}

/// The banner's board: remote pictures, bounce, error, failing, notice and careful, 640 wide, with the design
/// board's words.
fn banners(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    use super::banner::{BannerKind, BannerLink, banner};
    title(ui, theme, tr!("Banner"));
    type State<'a> = (&'a str, BannerKind, &'a str, [(&'a str, &'a str); 2], usize);
    let states: [State<'_>; 6] = [
        (
            tr!("remote-pictures"),
            BannerKind::RemotePictures,
            "3 remote pictures are not shown. Loading them tells the sender that you opened this message.",
            [(tr!("load once"), "pictures.once"), ("always for news@brightshelf.example.org", "pictures.always")],
            2,
        ),
        (
            tr!("bounce"),
            BannerKind::Bounce,
            "This message did not arrive: 550 5.1.1 The address ansel@example.org does not exist.",
            [(tr!("open the report"), "bounce.report"), (tr!("edit and send again"), "bounce.again")],
            2,
        ),
        (
            tr!("error"),
            BannerKind::Error,
            "The message could not be moved: the server refused (NO [CANNOT]).",
            [(tr!("try again"), "error.again"), (tr!("dismiss"), "error.dismiss")],
            2,
        ),
        (
            tr!("failing"),
            BannerKind::Failing,
            "Studio refused the sign-in, and one message was not sent.",
            [(tr!("open account"), "failing.account"), (tr!("open outbox"), "failing.outbox")],
            2,
        ),
        (tr!("notice"), BannerKind::Notice, "The text is cut here: the rest is longer than the reader shows.", [("", ""); 2], 0),
        (tr!("careful"), BannerKind::Careful, "The text says “example.org”, but the link goes to example.net.", [("", ""); 2], 0),
    ];
    for (state, kind, text, links, count) in states {
        cells(ui, theme, |ui| {
            cell(ui, theme, state, |ui| {
                ui.set_width(640.0);
                let links: Vec<BannerLink<'_>> = links.iter().take(count).map(|(name, id)| BannerLink { name, id }).collect();
                banner(ui, theme, keymap, kind, text, &links);
            });
        });
    }
}

/// The settings row's board: default, hover, focus, unavailable (coming soon) and with help, with the motion choice.
fn settings_rows(ui: &mut Ui, theme: &Theme) {
    use super::settings_row::{settings_row_soon, settings_row_with};
    title(ui, theme, tr!("Settings row"));
    let f = theme.fonts.button;
    let choice = |ui: &mut Ui| {
        segmented(ui, theme, &[(0, tr!("full"), f), (1, tr!("reduced"), f), (2, tr!("off"), f)], 1);
    };
    for (state, help, shown) in [
        (tr!("default"), None, Shown::Rest),
        (tr!("hover"), None, Shown::Hover),
        (tr!("focus"), None, Shown::Focus),
        (tr!("with-help"), Some(tr!("Reduced keeps what explains a change and drops the text effects. Off: everything at once.")), Shown::Rest),
    ] {
        cells(ui, theme, |ui| {
            cell(ui, theme, state, |ui| {
                ui.set_width(640.0);
                settings_row_with(ui, theme, tr!("Motion"), help, false, Some(shown), choice);
            });
        });
    }
    cells(ui, theme, |ui| {
        cell(ui, theme, tr!("unavailable"), |ui| {
            ui.set_width(640.0);
            settings_row_soon(ui, theme, tr!("Calendar alerts"), tr!("Comes with the Calendar settings page."));
        });
    });
}

/// The facts' board: default, details open and long text, with the design board's words.
fn facts_board(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    use super::facts::{Facts, facts_of};
    title(ui, theme, tr!("Facts"));
    let (from, to, date) = ("Teodor Vasquez  <teodor@example.org>", "studio@example.org", "Mon 16 Nov 2026, 14:02");
    let open = Facts {
        from,
        to,
        date,
        cc: "Marit Solheim  <marit@example.org>",
        reply_to: "teodor@example.org",
        checked: "example.org · SPF pass · DKIM pass",
        id: "<20261116.1402.7f3a@example.org>",
        recipients: None,
    };
    let long = Facts {
        from: "Annelise Vandenberghe-Okonkwo-Halvorssen  <annelise.vandenberghe-okonkwo-halvorssen@studio.example.org>",
        to: "studio@example.org, Marit Solheim, Ines Okafor, Pavel Hruska, Greta Lindqvist, Tobias Wren, Ada Brannock",
        date,
        recipients: Some(12),
        ..Facts::default()
    };
    for (state, shown, opened) in
        [(tr!("default"), Facts { from, to, date, ..Facts::default() }, false), (tr!("details-open"), open, true), (tr!("long-text"), long, false)]
    {
        cells(ui, theme, |ui| {
            cell(ui, theme, state, |ui| {
                ui.set_width(560.0);
                facts_of(ui, theme, keymap, &shown, opened);
            });
        });
    }
}

/// The hover actions' board: default, action-hover, focus and unavailable, Done D, Snooze H, Read R and Delete Del.
fn hover_actions(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    use crate::keymap::SNOOZE;
    title(ui, theme, tr!("Hover actions"));
    // The page's keymap gives D, R and Del to its own actions; Snooze keeps the app's H, as the sheet shows.
    let (done, read, delete) = ("mail.done", "mail.read", "mail.delete");
    let group = |snooze: Option<&'static str>| {
        [
            Action { icon: IconId::Done, name: tr!("Done"), action: Some(done), unavailable: None },
            Action { icon: IconId::Snooze, name: tr!("Snooze"), action: Some(SNOOZE), unavailable: snooze },
            Action { icon: IconId::MarkRead, name: tr!("Mark as read"), action: Some(read), unavailable: None },
            Action { icon: IconId::Delete, name: tr!("Delete"), action: Some(delete), unavailable: None },
        ]
    };
    let states = [
        (tr!("default"), None, None),
        (tr!("action-hover"), None, Some((0, Shown::Hover))),
        (tr!("focus"), None, Some((0, Shown::Focus))),
        (tr!("unavailable"), Some(tr!("Not yet: the time picker comes with the pickers")), None),
    ];
    for pair in states.chunks(2) {
        cells(ui, theme, |ui| {
            for (state, snooze, forced) in pair {
                cell(ui, theme, state, |ui| {
                    let width = hover::width(ui, theme, keymap, &group(*snooze));
                    let (_, line) = ui.allocate_space(vec2(width, theme.boxes().strip));
                    let id = Id::unique("gallery-hover-actions").with(*state);
                    hover::actions_with(ui, theme, keymap, id, line, true, &group(*snooze), *forced);
                });
            }
        });
    }
}

/// The empty state's board: the bare one, its lines centred, and the one with an action, its lines on the left.
fn empty_states(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    use super::empty::{empty_state, empty_state_with};
    title(ui, theme, tr!("Empty state"));
    cells(ui, theme, |ui| {
        cell(ui, theme, tr!("default"), |ui| {
            ui.allocate_ui(vec2(440.0, 216.0), |ui| {
                empty_state(
                    ui,
                    theme,
                    tr!("Inbox · nothing to read"),
                    tr!("All read.\nNew mail lands here."),
                    tr!("This folder holds no unread messages. “Show done” brings back what you finished."),
                );
            });
        });
    });
    cells(ui, theme, |ui| {
        cell(ui, theme, tr!("with-action"), |ui| {
            ui.allocate_ui(vec2(440.0, 250.0), |ui| {
                let action = TextButton::new(tr!("Add account")).primary().arrow();
                empty_state_with(
                    ui,
                    theme,
                    tr!("No accounts yet"),
                    tr!("Add an account to read its mail here."),
                    tr!("Nothing leaves this computer before you do."),
                    action,
                    keymap,
                );
            });
        });
    });
}

/// The progress line's board: running (the sheet's still frame: the page rests), done and failed.
fn progresses(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    use super::progress::{ProgressState, progress_with};
    title(ui, theme, tr!("Progress"));
    cells(ui, theme, |ui| {
        let states = [
            (tr!("running"), tr!("syncing"), ProgressState::Running(0.54), "54 %"),
            (tr!("done"), tr!("synced"), ProgressState::Done, "14:02"),
            (tr!("failed"), tr!("sync failed"), ProgressState::Failed(0.54), tr!("open log")),
        ];
        for (state, word, stands, note) in states {
            cell(ui, theme, state, |ui| {
                progress_with(ui, theme, keymap, word, stands, note, true);
            });
        }
    });
}

/// The check step's board: its eight states in the sheet's order, with the design board's words.
fn check_steps(ui: &mut Ui, theme: &Theme) {
    use super::check_step::{StepState, check_step};
    title(ui, theme, tr!("Check step"));
    let smtp = "smtp.example.org:465 · TLS";
    let states = [
        (tr!("waiting"), StepState::Waiting, "SMTP", smtp, tr!("waiting")),
        (tr!("running"), StepState::Running, "SMTP", smtp, "0.2 s …"),
        (tr!("passed"), StepState::Passed, "IMAP", tr!("sign-in, 6 folders, IDLE CONDSTORE QRESYNC MOVE"), "0.6 s"),
        (tr!("failed"), StepState::Failed, "IMAP", tr!("sign-in"), "1.2 s"),
        (tr!("skipped"), StepState::Skipped, "SMTP", smtp, tr!("skipped")),
        (tr!("not-run"), StepState::NotRun, tr!("Clock"), tr!("compared with the server's"), tr!("not run")),
        (tr!("no-server"), StepState::NoServer, "CalDAV", tr!("calendars"), tr!("no server set")),
        (tr!("not-built"), StepState::NotBuilt, tr!("Push"), tr!("new mail without asking"), tr!("not built yet")),
    ];
    for (state, stands, step, detail, note) in states {
        cells(ui, theme, |ui| {
            cell(ui, theme, state, |ui| {
                ui.set_width(560.0);
                let message = (stands == StepState::Failed).then_some("refused: [AUTHENTICATIONFAILED] Invalid credentials.");
                check_step(ui, theme, step, detail, note, stands, message);
            });
        });
    }
}

/// The left column: the row, the banner, the facts and the progress line.
fn left(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    rows(ui, theme);
    ui.add_space(BOARD_GAP);
    banners(ui, theme, keymap);
    ui.add_space(BOARD_GAP);
    facts_board(ui, theme, keymap);
    ui.add_space(BOARD_GAP);
    progresses(ui, theme, keymap);
}

/// The right column: the hover actions, the empty state, the settings row and the check step.
fn right(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    hover_actions(ui, theme, keymap);
    ui.add_space(BOARD_GAP);
    empty_states(ui, theme, keymap);
    ui.add_space(BOARD_GAP);
    settings_rows(ui, theme);
    ui.add_space(BOARD_GAP);
    check_steps(ui, theme);
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
}

#[cfg(test)]
#[path = "gallery_blocks_tests.rs"]
mod tests;
