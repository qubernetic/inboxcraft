// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The add-account page: the form with Detect, then the check's steps with the host, port and issuer each saw. An
//! account already there gets the same fields, with a new password instead of the password.

use std::sync::Arc;

use egui::{
    Align, CentralPanel, CollapsingHeader, Color32, Frame, Galley, Layout, Margin, Pos2, Rect, Response, RichText, Role, ScrollArea, Sense, Ui,
    UiBuilder, WidgetInfo, pos2, vec2,
};

use super::paint::{self, Panes};
use crate::component;
use crate::component::button::{self, TextButton};
use crate::component::checkbox::checkbox_wrapping;
use crate::component::field::{self, Field};
use crate::component::{margin, pill};
use crate::iconset::IconId;
use crate::keymap::Keymap;
use crate::mailbox::Mailbox;
use crate::onboard::{Form, Found, Onboarding, Outcome, Secret, Security, Stage, StepRow, StepState};
use crate::theme::Theme;
use crate::tr;

/// The widest the form grows, in points.
const FORM_WIDTH: f32 = 480.0;

/// What the owner asked for on the page this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Asked {
    Detect,
    Add,
    Save,
    CheckAgain,
    Change,
    Close,
}

/// The room between the fields of a row, in points.
fn between(theme: &Theme) -> f32 {
    theme.spacing.s2
}

/// A one-line field `width` points wide under its `label`, with `hint` in it while it is empty.
fn line_field(ui: &mut Ui, theme: &Theme, label: &str, text: &mut String, hint: &str, width: f32) -> Response {
    ui.vertical(|ui| {
        ui.set_width(width);
        let id = ui.make_persistent_id(("onboard-field", label, hint));
        field::text_field(ui, theme, id, text, &Field::new(label, hint))
    })
    .inner
}

/// The password field under its `caption`, `hint` in it while it is empty: with a hint it stands for a password the
/// credential store holds ([`Secret::Stored`](field::Secret::Stored)). The field keeps nothing it is typed in its undo
/// history, and takes no more than the [`Secret`]'s buffer holds, so typing never moves the text.
fn password(ui: &mut Ui, theme: &Theme, caption: &str, hint: &str, secret: &mut Secret, width: f32) {
    ui.vertical(|ui| {
        ui.set_width(width);
        let kept = if hint.is_empty() { field::Secret::Typed } else { field::Secret::Stored };
        let id = ui.make_persistent_id("account-password");
        field::password_field(ui, theme, id, secret.expose_mut(), &Field::new(caption, hint), kept);
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
    Frame::new().fill(p.grid).corner_radius(0.0).inner_margin(margin(s.s2, s.s2)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        match found.source {
            Some(source) => note(ui, theme, source.text(), if source.doubtful() { p.ink } else { p.muted }),
            None => note(ui, theme, tr!("Detect found no server: type the settings in."), p.ink),
        }
        if let Some((host, _)) = found.outgoing.as_ref().filter(|(_, source)| source.doubtful()) {
            note(ui, theme, &tr!("{host} was a guess from common names: check it.", host = host), p.ink);
        }
        CollapsingHeader::new(
            RichText::new(tr!("What Detect asked ({count})", count = found.asked.len())).font(theme.fonts.caption.id()).color(p.cyan_text),
        )
        .id_salt("detect-asked")
        .show(ui, |ui| {
            for line in found.asked.iter().take(32) {
                note(ui, theme, line, p.muted);
            }
            for line in found.failed.iter().take(32) {
                note(ui, theme, line, p.ink);
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
    let label = crate::component::label(ui, theme.fonts.label, tr!("Address"), p.muted);
    ui.horizontal(|ui| {
        let name = if detecting { tr!("Detecting…") } else { tr!("Detect") };
        let room = width - ui.fonts_mut(|fonts| fonts.layout_no_wrap(name.to_owned(), theme.fonts.button.id(), p.ink).size().x) - 64.0;
        let address = ui
            .allocate_ui(vec2(room.max(120.0), theme.boxes().field), |ui| {
                ui.set_width(room.max(120.0));
                let id = ui.make_persistent_id("onboard-address");
                field::text_field(ui, theme, id, &mut form.address, &Field::new("", "you@example.org"))
            })
            .inner;
        ui.ctx().accesskit_node_builder(address.id, |node| node.set_label(tr!("Address")));
        address.labelled_by(label.id);
        let button = TextButton::new(name).secondary().icon(IconId::Search);
        if ui.add_enabled_ui(!detecting, |ui| button.show(ui, theme, keymap)).inner.clicked() {
            detect = true;
        }
    });
    if checkbox_wrapping(ui, theme, tr!("Also ask Mozilla's list of providers (tells Mozilla the domain)"), form.ispdb).clicked() {
        form.ispdb = !form.ispdb;
    }
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

/// The names of the account, then the user name beside the password (`caption` over it, `hint` in it while it is
/// empty).
fn names(ui: &mut Ui, theme: &Theme, form: &mut Form, width: f32, (caption, hint): (&str, &str)) {
    let gap = between(theme);
    let half = (width - gap) / 2.0;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        line_field(ui, theme, tr!("Name for the account"), &mut form.label, tr!("the address"), half);
        line_field(ui, theme, tr!("Name on sent mail"), &mut form.display_name, tr!("optional"), half);
    });
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        line_field(ui, theme, tr!("User name"), &mut form.username, tr!("the address"), half);
        password(ui, theme, caption, hint, &mut form.password, half);
    });
}

/// One server's row: its host, its port (`usual` as the hint) and the choice of encryption. There is no unencrypted choice.
fn server_row(
    ui: &mut Ui,
    theme: &Theme,
    (caption, placeholder): (&str, &str),
    (host, port_text, security): (&mut String, &mut String, &mut Option<Security>),
    usual: fn(Security) -> u16,
    width: f32,
) {
    let s = theme.spacing;
    let gap = between(theme);
    let chosen = security.unwrap_or(Security::Tls);
    let port = s.s6 * 3.0;
    let choices = s.s6 * 6.0;
    let room = (width - port - choices - 2.0 * gap).max(120.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        line_field(ui, theme, caption, host, placeholder, room);
        line_field(ui, theme, tr!("Port"), port_text, &usual(chosen).to_string(), port);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = s.s1;
            crate::component::label(ui, theme.fonts.label, tr!("Security"), theme.palette.muted);
            ui.horizontal(|ui| {
                for choice in [Security::Tls, Security::StartTls] {
                    if pill::pill(ui, theme, choice.text(), chosen == choice).clicked() {
                        *security = Some(choice);
                    }
                }
            });
        });
    });
}

/// The incoming server, and the outgoing server and the CalDAV address, which are optional.
fn servers(ui: &mut Ui, theme: &Theme, form: &mut Form, width: f32) {
    server_row(ui, theme, (tr!("IMAP server"), "imap.example.org"), (&mut form.host, &mut form.port, &mut form.security), Security::imap_port, width);
    server_row(
        ui,
        theme,
        (tr!("Outgoing server (SMTP), to send"), "smtp.example.org"),
        (&mut form.smtp_host, &mut form.smtp_port, &mut form.smtp_security),
        Security::submission_port,
        width,
    );
    line_field(ui, theme, tr!("CalDAV address, for calendars"), &mut form.caldav, tr!("optional, https://dav.example.org"), width);
}

/// A note with the padlock before it.
fn lock_note(ui: &mut Ui, theme: &Theme, text: &str) {
    ui.horizontal(|ui| {
        let side = theme.fonts.caption.size * 1.2;
        let (at, _) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
        crate::iconset::paint(ui.painter(), at, IconId::Lock, theme.palette.muted);
        note(ui, theme, text, theme.palette.muted);
    });
}

/// The form of an account already there: the add form's fields, the address fixed, a new password instead of the
/// password, and where the sign-in will go when it would go anywhere new.
fn edit_form(ui: &mut Ui, theme: &Theme, keymap: &Keymap, onboarding: &mut Onboarding) -> Option<Asked> {
    let p = theme.palette;
    let s = theme.spacing;
    let mut asked = None;
    let width = ui.available_width().min(FORM_WIDTH);
    ui.spacing_mut().item_spacing.y = s.s2;
    ui.label(RichText::new(tr!("Account settings")).font(theme.fonts.title.id()).color(p.ink));
    ui.add_space(s.s2);
    let address = onboarding.form().address.clone();
    let caption = crate::component::label(ui, theme.fonts.label, tr!("Address"), p.muted);
    ui.add(egui::Label::new(RichText::new(address).font(theme.fonts.body.id()).color(p.ink)).wrap()).labelled_by(caption.id);
    ui.add_space(s.s1);
    let form = onboarding.form_mut();
    names(ui, theme, form, width, (tr!("New password"), tr!("empty keeps the stored one")));
    servers(ui, theme, form, width);
    ui.add_space(s.s1);
    lock_note(ui, theme, tr!("A new password goes into your system's credential store. The stored one is never shown."));
    if let Some(text) = onboarding.sign_in_note() {
        note(ui, theme, &text, p.ink);
    }
    ui.add_space(s.s2);
    let label = if onboarding.save_checks() { tr!("Save and check") } else { tr!("Save") };
    ui.horizontal(|ui| {
        if TextButton::new(label).primary().show(ui, theme, keymap).clicked() {
            asked = Some(Asked::Save);
        }
        if button::link(ui, theme, keymap, tr!("Cancel")).clicked() {
            asked = Some(Asked::Close);
        }
    });
    asked
}

fn form(ui: &mut Ui, theme: &Theme, keymap: &Keymap, onboarding: &mut Onboarding) -> Option<Asked> {
    let p = theme.palette;
    let s = theme.spacing;
    let mut asked = None;
    let width = ui.available_width().min(FORM_WIDTH);
    ui.spacing_mut().item_spacing.y = s.s2;
    ui.label(RichText::new(tr!("Add an account")).font(theme.fonts.title.id()).color(p.ink));
    ui.add_space(s.s2);
    if detect_part(ui, theme, keymap, onboarding, width) {
        asked = Some(Asked::Detect);
    }
    ui.add_space(s.s2);
    let form = onboarding.form_mut();
    names(ui, theme, form, width, (tr!("Password"), ""));
    servers(ui, theme, form, width);
    ui.add_space(s.s1);
    lock_note(ui, theme, tr!("The password goes into your system's credential store, never into InboxCraft's files."));
    note(ui, theme, tr!("Accounts are not kept yet: when InboxCraft closes, it removes their passwords from the store again."), p.muted);
    ui.add_space(s.s2);
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
    let size = theme.fonts.body.line;
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    let (id, colour) = match state {
        StepState::Passed => (IconId::Done, p.cyan_text),
        StepState::Failed => (IconId::Failed, p.minus),
        StepState::Pending => (IconId::Waiting, p.cyan_text),
        StepState::Skipped | StepState::Held | StepState::NotSet | StepState::NotBuilt => (IconId::Skipped, p.muted),
    };
    crate::iconset::paint(ui.painter(), rect, id, colour);
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
            let colour = if matches!(row.state, StepState::Skipped | StepState::NotSet | StepState::NotBuilt) { p.muted } else { p.ink };
            ui.label(
                RichText::new(tr!("{step}: {state}", step = row.name(), state = state_text(row.state))).font(theme.fonts.mono.id()).color(colour),
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
                note(ui, theme, message, if row.state == StepState::Failed { p.minus } else { p.ink });
            }
        });
    });
    ui.add_space(theme.spacing.s2);
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
        let side = theme.fonts.body.line;
        let (at, _) = ui.allocate_exact_size(vec2(side, side), Sense::hover());
        crate::iconset::paint(ui.painter(), at, IconId::Failed, p.minus);
        ui.add(egui::Label::new(RichText::new(text).font(theme.fonts.body.id()).color(p.minus)).wrap());
    });
}

fn checking(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, onboarding: &Onboarding, account: u64) -> Option<Asked> {
    let p = theme.palette;
    let mut asked = None;
    let address = mailbox.account(account).map(|row| row.address.clone()).unwrap_or_default();
    ui.label(RichText::new(tr!("Checking {address}", address = address)).font(theme.fonts.title.id()).color(p.ink));
    match onboarding.outcome() {
        Outcome::Running => note(ui, theme, tr!("Each step shows as it ends."), p.muted),
        Outcome::Passed => note(ui, theme, tr!("Signed in. InboxCraft now fetches the folders and the mail."), p.cyan_text),
        Outcome::Failed => failure(ui, theme, onboarding),
    }
    ui.add_space(theme.spacing.s3);
    // The steps on a card of their own.
    let s = theme.spacing;
    Frame::new().fill(p.surface).stroke(egui::Stroke::new(1.0, p.line)).corner_radius(0.0).inner_margin(margin(s.s4, s.s3)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        for row in onboarding.steps() {
            step(ui, theme, row);
        }
    });
    ui.add_space(theme.spacing.s3);
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

/// The welcome box's width, in points.
const WELCOME: f32 = 520.0;

/// The room above and below the welcome's lines, in points.
const WELCOME_PAD_Y: f32 = 40.0;

/// The room left and right of the welcome's lines, and under where Add account leads, in points.
const WELCOME_PAD_X: f32 = 24.0;

/// The room between two of the welcome's lines, in points.
const WELCOME_GAP: f32 = 12.0;

/// The room over Add account, under the gap after the help, in points.
const WELCOME_ACTION_TOP: f32 = 8.0;

/// The most lines the welcome's title and help each take.
const WELCOME_ROWS: usize = 4;

/// A line of words at `at` that a screen reader reads as `text`.
fn said(ui: &mut Ui, at: Pos2, width: f32, galley: Arc<Galley>, colour: Color32, text: &str) {
    let rect = Rect::from_min_size(at, vec2(paint::run_box(galley.size().x).min(width), galley.size().y));
    let response = ui.interact(rect, ui.scope_id().with("said").with(text), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, text));
    if ui.is_rect_visible(rect) {
        paint::rows_at(ui.painter(), at, width, galley, colour);
    }
}

/// The window with no account: a box 520 wide in the middle of the body, the empty state of no account in it and,
/// where the window can add accounts (`can_add`), Add account with its arrow and where it leads; whether Add account
/// was pressed. A window that cannot add accounts says how to look around instead.
pub(super) fn welcome(ui: &mut Ui, theme: &Theme, keymap: &Keymap, layers: &mut Panes, can_add: bool) -> bool {
    let p = &theme.palette;
    let f = &theme.fonts;
    let gap = theme.spacing.s2;
    let mut add = false;
    CentralPanel::no_frame().frame(Frame::new().inner_margin(Margin::same(gap.clamp(0.0, 100.0) as i8))).show(ui, |ui| {
        let room = ui.max_rect();
        let width = WELCOME.min(room.width()).floor().max(1.0);
        let inner = (width - 2.0 - 2.0 * WELCOME_PAD_X).max(1.0);
        let (title, help) = if can_add {
            (tr!("Add an account to read its mail here."), tr!("Nothing leaves this computer before you do."))
        } else {
            (tr!("This window cannot add an account."), tr!("Start InboxCraft with --sample to look around an invented mailbox."))
        };
        let align = if can_add { Align::LEFT } else { Align::Center };
        let label = tr!("No account yet");
        let lines = [
            (component::wrapped(ui, label, f.label, p.muted, inner, 1, align), label),
            (component::wrapped(ui, title, f.title, p.muted, inner, WELCOME_ROWS, align), title),
            (component::wrapped(ui, help, f.caption, p.muted, inner, WELCOME_ROWS, align), help),
        ];
        let text_high: f32 = lines.iter().map(|(line, _)| line.size().y).sum::<f32>() + WELCOME_GAP * 2.0;
        let action = if can_add { WELCOME_GAP + WELCOME_ACTION_TOP + component::control_side(theme) } else { 0.0 };
        let leads = if can_add { f.caption.line + WELCOME_PAD_X } else { 0.0 };
        let high = (2.0 + WELCOME_PAD_Y + text_high + action + WELCOME_PAD_Y + leads).ceil().min(room.height().max(1.0));
        let card = Rect::from_min_size((room.center() - vec2(width, high) / 2.0).round(), vec2(width, high));
        let group = ui.interact(card, ui.scope_id().with("welcome"), Sense::hover());
        group.widget_info(|| WidgetInfo::labeled(Role::Group, true, tr!("Welcome")));
        ui.scope_builder(UiBuilder::new().max_rect(card), |ui| {
            layers.card(ui, Margin::ZERO, |ui| {
                let left = card.left() + 1.0 + WELCOME_PAD_X;
                let mut top = card.top() + 1.0 + WELCOME_PAD_Y;
                for (line, text) in lines {
                    let high = line.size().y;
                    said(ui, pos2(left, top), inner, line, p.muted, text);
                    top += high + WELCOME_GAP;
                }
                if !can_add {
                    return;
                }
                let at = Rect::from_min_size(pos2(left, top + WELCOME_ACTION_TOP), vec2(inner, component::control_side(theme)));
                let mut child = ui.new_child(UiBuilder::new().max_rect(at).layout(Layout::left_to_right(Align::Center)));
                add = TextButton::new(tr!("Add account")).primary().arrow().show(&mut child, theme, keymap).clicked();
                let where_to = tr!("Opens Settings · Accounts");
                let galley = component::galley(ui, where_to, f.caption, p.muted);
                let bottom = card.bottom() - 1.0 - WELCOME_PAD_X;
                said(ui, pos2(left, bottom - galley.size().y), inner, galley, p.muted, where_to);
            });
        });
    });
    add
}

/// Draws the page and returns what was asked for; the caller applies it after the frame's drawing.
pub(super) fn show(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, onboarding: &mut Onboarding) -> Option<Asked> {
    let p = theme.palette;
    let mut asked = None;
    crate::component::scroll::show(ui, theme, ScrollArea::vertical().auto_shrink([false, false]), |ui| {
        ui.set_max_width(FORM_WIDTH);
        if let Some(error) = onboarding.error() {
            note(ui, theme, error, p.minus);
            ui.add_space(theme.spacing.s2);
        }
        asked = match onboarding.stage() {
            Stage::Form => form(ui, theme, keymap, onboarding),
            Stage::Edit(_) => edit_form(ui, theme, keymap, onboarding),
            Stage::Checking(account) => checking(ui, theme, keymap, mailbox, onboarding, account),
            Stage::Closed => None,
        };
    });
    asked
}
