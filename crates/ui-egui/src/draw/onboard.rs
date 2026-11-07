// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The add-account page: the form with Detect, then the check's steps with the host, port and issuer each saw.

use egui::text_edit::TextEditState;
use egui::{CollapsingHeader, Color32, Frame, Response, RichText, ScrollArea, Sense, TextEdit, Ui, vec2};

use crate::component::button::{self, TextButton};
use crate::component::{field as field_style, margin, pill};
use crate::iconset::{IconId, Style};
use crate::keymap::Keymap;
use crate::mailbox::Mailbox;
use crate::onboard::{Found, MAX_PASSWORD_CHARS, Onboarding, Outcome, Secret, Security, Stage, StepRow, StepState};
use crate::theme::Theme;
use crate::tr;

/// The widest the form grows, in points.
const FORM_WIDTH: f32 = 480.0;

/// What the owner asked for on the page this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Asked {
    Detect,
    Add,
    CheckAgain,
    Change,
    Close,
}

/// The room between the fields of a row, in points.
fn between(theme: &Theme) -> f32 {
    theme.spacing.s
}

/// A field `width` points wide under its `label` (the caption in the muted colour); `add` draws the field.
fn labelled(ui: &mut Ui, theme: &Theme, label: &str, width: f32, add: impl FnOnce(&mut Ui) -> Response) -> Response {
    ui.vertical(|ui| {
        ui.set_width(width);
        ui.spacing_mut().item_spacing.y = theme.spacing.xs;
        let label = ui.label(RichText::new(label).font(theme.fonts.caption.id()).color(theme.palette.muted));
        add(ui).labelled_by(label.id)
    })
    .inner
}

/// A one-line field `width` points wide with `hint` in it while it is empty.
fn line_edit<'t>(theme: &Theme, text: &'t mut String, hint: &str, width: f32) -> TextEdit<'t> {
    TextEdit::singleline(text).hint_text(hint).margin(field_style::margin(theme)).desired_width(width)
}

/// The password field. egui's undo history keeps plain copies of what was typed, so it is emptied after every frame:
/// undo never brings back a password that was cleared, nor an earlier account's. The field takes no more than its
/// [`Secret`]'s buffer holds, so typing never moves the text.
fn password(ui: &mut Ui, theme: &Theme, secret: &mut Secret, width: f32) {
    labelled(ui, theme, tr!("Password"), width, |ui| {
        let edit = TextEdit::singleline(secret.expose_mut())
            .id_salt("account-password")
            .password(true)
            .char_limit(MAX_PASSWORD_CHARS)
            .margin(field_style::margin(theme))
            .desired_width(width);
        let response = ui.add(edit);
        if let Some(mut state) = TextEditState::load(ui.ctx(), response.id) {
            state.clear_undoer();
            state.store(ui.ctx(), response.id);
        }
        response
    });
}

fn note(ui: &mut Ui, theme: &Theme, text: &str, colour: Color32) {
    ui.add(egui::Label::new(RichText::new(text).font(theme.fonts.caption.id()).color(colour)).wrap());
}

/// What Detect found, on a tinted box: where the settings came from (in the flag colour when it is a guess), and what
/// it asked, folded.
fn found_box(ui: &mut Ui, theme: &Theme, found: &Found) {
    let p = theme.palette;
    let s = theme.spacing;
    Frame::new().fill(p.hover).corner_radius(s.corner).inner_margin(margin(s.s, s.s)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        match found.source {
            Some(source) => note(ui, theme, source.text(), if source.doubtful() { p.flag } else { p.muted }),
            None => note(ui, theme, tr!("Detect found no server: type the settings in."), p.flag),
        }
        CollapsingHeader::new(
            RichText::new(tr!("What Detect asked ({count})", count = found.asked.len())).font(theme.fonts.caption.id()).color(p.accent),
        )
        .id_salt("detect-asked")
        .show(ui, |ui| {
            for line in found.asked.iter().take(32) {
                note(ui, theme, line, p.muted);
            }
            for line in found.failed.iter().take(32) {
                note(ui, theme, line, p.flag);
            }
        });
    });
}

/// The address with Detect beside it, the box to ask Mozilla's list too, what Detect sends, and what it found.
fn detect_part(ui: &mut Ui, theme: &Theme, keymap: &Keymap, onboarding: &mut Onboarding, width: f32) -> bool {
    let p = theme.palette;
    let detecting = onboarding.detecting();
    let found = onboarding.found().cloned();
    let form = onboarding.form_mut();
    let mut detect = false;
    let label = ui.label(RichText::new(tr!("Address")).font(theme.fonts.caption.id()).color(p.muted));
    ui.horizontal(|ui| {
        let name = if detecting { tr!("Detecting…") } else { tr!("Detect") };
        let room = width - ui.fonts_mut(|fonts| fonts.layout_no_wrap(name.to_owned(), theme.fonts.button.id(), p.text).size().x) - 64.0;
        ui.add(line_edit(theme, &mut form.address, "you@example.org", room.max(120.0))).labelled_by(label.id);
        let button = TextButton::new(name).secondary().icon(IconId::Search);
        if ui.add_enabled_ui(!detecting, |ui| button.show(ui, theme, keymap)).inner.clicked() {
            detect = true;
        }
    });
    ui.checkbox(&mut form.ispdb, tr!("Also ask Mozilla's list of providers (tells Mozilla the domain)"));
    note(
        ui,
        theme,
        tr!(
            "Detect asks your name servers and your domain's web server (once without encryption) about your domain; it sends no address, user name or password."
        ),
        p.muted,
    );
    if let Some(found) = &found {
        found_box(ui, theme, found);
    }
    detect
}

fn form(ui: &mut Ui, theme: &Theme, keymap: &Keymap, onboarding: &mut Onboarding) -> Option<Asked> {
    let p = theme.palette;
    let s = theme.spacing;
    let mut asked = None;
    let width = ui.available_width().min(FORM_WIDTH);
    let gap = between(theme);
    ui.spacing_mut().item_spacing.y = s.s;
    ui.label(RichText::new(tr!("Add an account")).font(theme.fonts.title.id()).color(p.text));
    ui.add_space(s.s);
    if detect_part(ui, theme, keymap, onboarding, width) {
        asked = Some(Asked::Detect);
    }
    ui.add_space(s.s);
    let form = onboarding.form_mut();
    let half = (width - gap) / 2.0;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        labelled(ui, theme, tr!("Name for the account"), half, |ui| ui.add(line_edit(theme, &mut form.label, tr!("the address"), half)));
        labelled(ui, theme, tr!("Name on sent mail"), half, |ui| ui.add(line_edit(theme, &mut form.display_name, tr!("optional"), half)));
    });
    labelled(ui, theme, tr!("User name"), width, |ui| ui.add(line_edit(theme, &mut form.username, tr!("the address"), width)));
    password(ui, theme, &mut form.password, width);
    let security = form.security.unwrap_or(Security::Tls);
    let port = s.xl * 3.0;
    let choices = s.xl * 6.0;
    let server = (width - port - choices - 2.0 * gap).max(120.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        labelled(ui, theme, tr!("IMAP server"), server, |ui| ui.add(line_edit(theme, &mut form.host, "imap.example.org", server)));
        labelled(ui, theme, tr!("Port"), port, |ui| ui.add(line_edit(theme, &mut form.port, &security.imap_port().to_string(), port)));
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = s.xs;
            ui.label(RichText::new(tr!("Security")).font(theme.fonts.caption.id()).color(p.muted));
            ui.horizontal(|ui| {
                for choice in [Security::Tls, Security::StartTls] {
                    if pill::pill(ui, theme, choice.text(), security == choice).clicked() {
                        form.security = Some(choice);
                    }
                }
            });
        });
    });
    ui.add_space(s.xs);
    ui.horizontal(|ui| {
        let side = theme.fonts.caption.size * 1.2;
        let (at, _) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
        theme.icons.paint(ui.painter(), at, IconId::Lock, Style::Regular, p.muted);
        note(ui, theme, tr!("The password goes into your system's credential store, never into InboxCraft's files."), p.muted);
    });
    note(ui, theme, tr!("Accounts are not kept yet: when InboxCraft closes, it removes their passwords from the store again."), p.muted);
    ui.add_space(s.s);
    ui.horizontal(|ui| {
        if TextButton::new(tr!("Check and add")).primary().show(ui, theme, keymap).clicked() {
            asked = Some(Asked::Add);
        }
        if button::link(ui, theme, keymap, tr!("Cancel")).clicked() {
            asked = Some(Asked::Close);
        }
    });
    asked
}

/// The icon in front of a step: a filled tick or cross once it is decided, a ring while it waits, a dash when it
/// will not run.
fn mark(ui: &mut Ui, theme: &Theme, state: StepState) {
    let p = theme.palette;
    let size = super::paint::line_height(ui, theme.fonts.body);
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    let (id, style, colour) = match state {
        StepState::Passed => (IconId::Done, Style::Fill, p.accent),
        StepState::Failed => (IconId::Failed, Style::Fill, p.error),
        StepState::Pending => (IconId::Waiting, Style::Regular, p.accent),
        StepState::Skipped | StepState::Held | StepState::NotSet | StepState::NotBuilt => (IconId::Skipped, Style::Regular, p.muted),
    };
    theme.icons.paint(ui.painter(), rect, id, style, colour);
}

fn state_text(state: StepState) -> &'static str {
    match state {
        StepState::Pending => tr!("waiting"),
        StepState::Passed => tr!("passed"),
        StepState::Failed => tr!("failed"),
        StepState::Skipped => tr!("skipped"),
        StepState::Held => tr!("not run"),
        StepState::NotSet => tr!("no server set"),
        StepState::NotBuilt => tr!("not built yet"),
    }
}

fn step(ui: &mut Ui, theme: &Theme, row: &StepRow) {
    let p = theme.palette;
    ui.horizontal(|ui| {
        mark(ui, theme, row.state);
        ui.vertical(|ui| {
            let colour = if matches!(row.state, StepState::Skipped | StepState::NotSet | StepState::NotBuilt) { p.muted } else { p.text };
            ui.label(
                RichText::new(tr!("{step}: {state}", step = row.name(), state = state_text(row.state))).font(theme.fonts.body.id()).color(colour),
            );
            let mut details = Vec::new();
            // A step that did not run tried no server.
            if let Some(server) = row.server.as_ref().filter(|_| !matches!(row.state, StepState::Skipped | StepState::NotSet | StepState::NotBuilt)) {
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
    ui.add_space(theme.spacing.s);
}

/// Why the check failed, in the error colour with a cross before it: the first failed step and its reason, or a plain
/// word when the step gave none.
fn failure(ui: &mut Ui, theme: &Theme, onboarding: &Onboarding) {
    let p = theme.palette;
    let failed = onboarding.steps().iter().find(|row| row.state == StepState::Failed);
    let text = match failed.and_then(|row| row.message.as_deref().map(|message| (row.name(), message))) {
        Some((step, message)) => tr!("{step} failed: {message}", step = step, message = message),
        None => tr!("A step failed. Check again, or change the settings.").to_owned(),
    };
    ui.horizontal(|ui| {
        let side = super::paint::line_height(ui, theme.fonts.body);
        let (at, _) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
        theme.icons.paint(ui.painter(), at, IconId::Failed, Style::Fill, p.error);
        ui.add(egui::Label::new(RichText::new(text).font(theme.fonts.body.id()).color(p.error)).wrap());
    });
}

fn checking(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, onboarding: &Onboarding, account: u64) -> Option<Asked> {
    let p = theme.palette;
    let mut asked = None;
    let address = mailbox.account(account).map(|row| row.address.clone()).unwrap_or_default();
    ui.label(RichText::new(tr!("Checking {address}", address = address)).font(theme.fonts.title.id()).color(p.text));
    match onboarding.outcome() {
        Outcome::Running => note(ui, theme, tr!("Each step shows as it ends."), p.muted),
        Outcome::Passed => note(ui, theme, tr!("Signed in. InboxCraft now fetches the folders and the mail."), p.accent),
        Outcome::Failed => failure(ui, theme, onboarding),
    }
    ui.add_space(theme.spacing.m);
    // The steps on a card of their own.
    let s = theme.spacing;
    Frame::new().fill(p.surface).stroke(egui::Stroke::new(1.0, p.line)).corner_radius(s.card).inner_margin(margin(s.l, s.m)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        for row in onboarding.steps() {
            step(ui, theme, row);
        }
    });
    ui.add_space(theme.spacing.m);
    ui.horizontal(|ui| match onboarding.outcome() {
        Outcome::Passed => {
            if TextButton::new(tr!("Done")).primary().show(ui, theme, keymap).clicked() {
                asked = Some(Asked::Close);
            }
        }
        Outcome::Failed => {
            if TextButton::new(tr!("Check again")).primary().show(ui, theme, keymap).clicked() {
                asked = Some(Asked::CheckAgain);
            }
            if TextButton::new(tr!("Change settings")).secondary().show(ui, theme, keymap).clicked() {
                asked = Some(Asked::Change);
            }
        }
        // The way back to the list is the link above the page.
        Outcome::Running => {}
    });
    asked
}

/// Draws the page and returns what was asked for; the caller applies it after the frame's drawing.
pub(super) fn show(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, onboarding: &mut Onboarding) -> Option<Asked> {
    let p = theme.palette;
    let mut asked = None;
    ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.set_max_width(FORM_WIDTH);
        if let Some(error) = onboarding.error() {
            note(ui, theme, error, p.error);
            ui.add_space(theme.spacing.s);
        }
        asked = match onboarding.stage() {
            Stage::Form => form(ui, theme, keymap, onboarding),
            Stage::Checking(account) => checking(ui, theme, keymap, mailbox, onboarding, account),
            Stage::Closed => None,
        };
    });
    asked
}
