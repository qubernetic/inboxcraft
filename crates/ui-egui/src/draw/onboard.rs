// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The add-account page: the form with Detect, then the check's steps with the host, port and issuer each saw.

use egui::text_edit::TextEditState;
use egui::{CollapsingHeader, Color32, Grid, Response, RichText, ScrollArea, Sense, Stroke, TextEdit, Ui, pos2, vec2};

use crate::mailbox::Mailbox;
use crate::onboard::{MAX_PASSWORD_CHARS, Onboarding, Outcome, Secret, Security, Stage, StepRow, StepState};
use crate::theme::Theme;
use crate::tr;

/// The widest the form grows, in points.
const FORM_WIDTH: f32 = 560.0;

/// What the owner asked for on the page this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Asked {
    Detect,
    Add,
    CheckAgain,
    Change,
    Close,
}

/// A labelled one-line field in the form grid.
fn field(ui: &mut Ui, label: &str, text: &mut String, hint: &str) -> Response {
    let label = ui.label(label);
    let response = ui.add(TextEdit::singleline(text).hint_text(hint).desired_width(FORM_WIDTH - 200.0)).labelled_by(label.id);
    ui.end_row();
    response
}

/// The password field. egui's undo history keeps plain copies of what was typed, so it is emptied after every frame:
/// undo never brings back a password that was cleared, nor an earlier account's. The field takes no more than its
/// [`Secret`]'s buffer holds, so typing never moves the text.
fn password(ui: &mut Ui, secret: &mut Secret) {
    let label = ui.label(tr!("Password"));
    let edit = TextEdit::singleline(secret.expose_mut())
        .id_salt("account-password")
        .password(true)
        .char_limit(MAX_PASSWORD_CHARS)
        .desired_width(FORM_WIDTH - 200.0);
    let id = ui.add(edit).labelled_by(label.id).id;
    if let Some(mut state) = TextEditState::load(ui.ctx(), id) {
        state.clear_undoer();
        state.store(ui.ctx(), id);
    }
    ui.end_row();
}

fn note(ui: &mut Ui, theme: &Theme, text: &str, colour: Color32) {
    ui.label(RichText::new(text).size(theme.type_scale.small).color(colour));
}

fn form(ui: &mut Ui, theme: &Theme, onboarding: &mut Onboarding) -> Option<Asked> {
    let p = theme.palette;
    let mut asked = None;
    ui.label(RichText::new(tr!("Add an account")).size(theme.type_scale.title).color(p.text));
    note(
        ui,
        theme,
        tr!(
            "Type the settings your provider gives you, or let Detect propose them. Nothing is sent to the mail server before you choose Add and check."
        ),
        p.muted,
    );
    ui.add_space(theme.spacing.l);
    let detecting = onboarding.detecting();
    let found = onboarding.found().cloned();
    let form = onboarding.form_mut();
    Grid::new("account-form").num_columns(2).spacing(vec2(theme.spacing.m, theme.spacing.s)).show(ui, |ui| {
        let label = ui.label(tr!("Address"));
        ui.horizontal(|ui| {
            ui.add(TextEdit::singleline(&mut form.address).hint_text("you@example.org").desired_width(FORM_WIDTH - 300.0)).labelled_by(label.id);
            let detect = ui.add_enabled(!detecting, egui::Button::new(if detecting { tr!("Detecting…") } else { tr!("Detect") }));
            if detect.clicked() {
                asked = Some(Asked::Detect);
            }
        });
        ui.end_row();
        ui.label("");
        ui.checkbox(&mut form.ispdb, tr!("Detect also asks Mozilla's list of providers, which tells Mozilla your domain"));
        ui.end_row();
        ui.label("");
        ui.vertical(|ui| {
            // A grid cell does not wrap on its own.
            ui.set_max_width(FORM_WIDTH - 200.0);
            ui.add(
                egui::Label::new(
                    RichText::new(tr!(
                        "Detect asks your name servers and your domain's web server (once without encryption) about your domain; it sends no address, user name or password."
                    ))
                    .size(theme.type_scale.small)
                    .color(p.muted),
                )
                .wrap(),
            );
        });
        ui.end_row();
        if let Some(found) = &found {
            ui.label("");
            ui.vertical(|ui| {
                match found.source {
                    Some(source) => note(ui, theme, source.text(), if source.doubtful() { p.flag } else { p.muted }),
                    None => note(ui, theme, tr!("Detect found no server: type the settings in."), p.flag),
                }
                CollapsingHeader::new(tr!("What Detect asked ({count})", count = found.asked.len())).id_salt("detect-asked").show(ui, |ui| {
                    for line in found.asked.iter().take(32) {
                        note(ui, theme, line, p.muted);
                    }
                    for line in found.failed.iter().take(32) {
                        note(ui, theme, line, p.flag);
                    }
                });
            });
            ui.end_row();
        }
        field(ui, tr!("Name for the account"), &mut form.label, tr!("the address"));
        field(ui, tr!("Your name on sent mail"), &mut form.display_name, tr!("optional"));
        field(ui, tr!("User name"), &mut form.username, tr!("the address"));
        password(ui, &mut form.password);
        field(ui, tr!("IMAP server"), &mut form.host, "imap.example.org");
        let security = form.security.unwrap_or(Security::Tls);
        field(ui, tr!("Port"), &mut form.port, &security.imap_port().to_string());
        ui.label(tr!("Encryption"));
        ui.horizontal(|ui| {
            for choice in [Security::Tls, Security::StartTls] {
                if ui.selectable_label(security == choice, choice.text()).clicked() {
                    form.security = Some(choice);
                }
            }
        });
        ui.end_row();
    });
    ui.add_space(theme.spacing.m);
    note(ui, theme, tr!("The password goes into your system's credential store, never into InboxCraft's files."), p.muted);
    note(ui, theme, tr!("Accounts are not kept yet: when InboxCraft closes, it removes their passwords from the store again."), p.muted);
    ui.add_space(theme.spacing.m);
    ui.horizontal(|ui| {
        if ui.button(tr!("Add and check")).clicked() {
            asked = Some(Asked::Add);
        }
        if ui.button(tr!("Cancel")).clicked() {
            asked = Some(Asked::Close);
        }
    });
    asked
}

/// The mark in front of a step, drawn in code: a filled disc with a tick or a cross, or a ring.
fn mark(ui: &mut Ui, theme: &Theme, state: StepState) {
    let p = theme.palette;
    let size = theme.type_scale.body + 2.0;
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    let painter = ui.painter();
    let (c, r) = (rect.center(), size / 2.0 - 1.0);
    match state {
        StepState::Passed => {
            painter.circle_filled(c, r, p.accent);
            let stroke = Stroke::new(1.8, p.on_accent);
            painter.line_segment([pos2(c.x - r * 0.45, c.y), pos2(c.x - r * 0.1, c.y + r * 0.4)], stroke);
            painter.line_segment([pos2(c.x - r * 0.1, c.y + r * 0.4), pos2(c.x + r * 0.5, c.y - r * 0.4)], stroke);
        }
        StepState::Failed => {
            painter.circle_filled(c, r, p.error);
            let stroke = Stroke::new(1.8, p.on_error);
            let d = r * 0.4;
            painter.line_segment([pos2(c.x - d, c.y - d), pos2(c.x + d, c.y + d)], stroke);
            painter.line_segment([pos2(c.x - d, c.y + d), pos2(c.x + d, c.y - d)], stroke);
        }
        StepState::Pending => {
            painter.circle_stroke(c, r - 0.5, Stroke::new(1.5, p.accent));
        }
        StepState::Skipped | StepState::Held | StepState::NotBuilt => {
            painter.circle_stroke(c, r - 0.5, Stroke::new(1.5, p.muted));
            painter.line_segment([pos2(c.x - r * 0.4, c.y), pos2(c.x + r * 0.4, c.y)], Stroke::new(1.5, p.muted));
        }
    }
}

fn state_text(state: StepState) -> &'static str {
    match state {
        StepState::Pending => tr!("waiting"),
        StepState::Passed => tr!("passed"),
        StepState::Failed => tr!("failed"),
        StepState::Skipped => tr!("skipped"),
        StepState::Held => tr!("not run"),
        StepState::NotBuilt => tr!("not built yet"),
    }
}

fn step(ui: &mut Ui, theme: &Theme, row: &StepRow) {
    let p = theme.palette;
    ui.horizontal(|ui| {
        mark(ui, theme, row.state);
        ui.vertical(|ui| {
            let colour = if matches!(row.state, StepState::Skipped | StepState::NotBuilt) { p.muted } else { p.text };
            ui.label(
                RichText::new(tr!("{step}: {state}", step = row.name(), state = state_text(row.state))).size(theme.type_scale.body).color(colour),
            );
            let mut details = Vec::new();
            // A step that did not run tried no server.
            if let Some(server) = row.server.as_ref().filter(|_| !matches!(row.state, StepState::Skipped | StepState::NotBuilt)) {
                details.push(server.clone());
            }
            if let Some(issuer) = &row.issuer {
                details.push(tr!("issued by {issuer}", issuer = issuer));
            }
            if !details.is_empty() {
                note(ui, theme, &details.join(" · "), p.muted);
            }
            if let Some(message) = row.message.as_deref().filter(|_| matches!(row.state, StepState::Failed | StepState::Held)) {
                note(ui, theme, message, if row.state == StepState::Failed { p.error } else { p.flag });
            }
        });
    });
    ui.add_space(theme.spacing.xs);
}

fn checking(ui: &mut Ui, theme: &Theme, mailbox: &Mailbox, onboarding: &Onboarding, account: u64) -> Option<Asked> {
    let p = theme.palette;
    let mut asked = None;
    let address = mailbox.account(account).map(|row| row.address.clone()).unwrap_or_default();
    ui.label(RichText::new(tr!("Checking {address}", address = address)).size(theme.type_scale.title).color(p.text));
    let (summary, colour) = match onboarding.outcome() {
        Outcome::Running => (tr!("Each step shows as it ends."), p.muted),
        Outcome::Passed => (tr!("Signed in. InboxCraft now fetches the folders and the mail."), p.accent),
        Outcome::Failed => (tr!("A step failed. Check again, or change the settings."), p.error),
    };
    note(ui, theme, summary, colour);
    ui.add_space(theme.spacing.l);
    for row in onboarding.steps() {
        step(ui, theme, row);
    }
    ui.add_space(theme.spacing.m);
    ui.horizontal(|ui| match onboarding.outcome() {
        Outcome::Passed => {
            if ui.button(tr!("Done")).clicked() {
                asked = Some(Asked::Close);
            }
        }
        Outcome::Failed => {
            if ui.button(tr!("Check again")).clicked() {
                asked = Some(Asked::CheckAgain);
            }
            if ui.button(tr!("Change settings")).clicked() {
                asked = Some(Asked::Change);
            }
            if ui.button(tr!("Close")).clicked() {
                asked = Some(Asked::Close);
            }
        }
        Outcome::Running => {
            if ui.button(tr!("Close")).clicked() {
                asked = Some(Asked::Close);
            }
        }
    });
    asked
}

/// Draws the page and returns what was asked for; the caller applies it after the frame's drawing.
pub(super) fn show(ui: &mut Ui, theme: &Theme, mailbox: &Mailbox, onboarding: &mut Onboarding) -> Option<Asked> {
    let p = theme.palette;
    let mut asked = None;
    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.set_max_width(FORM_WIDTH);
        if let Some(error) = onboarding.error() {
            note(ui, theme, error, p.error);
            ui.add_space(theme.spacing.s);
        }
        asked = match onboarding.stage() {
            Stage::Form => form(ui, theme, onboarding),
            Stage::Checking(account) => checking(ui, theme, mailbox, onboarding, account),
            Stage::Closed => None,
        };
    });
    asked
}
