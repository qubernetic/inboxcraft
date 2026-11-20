// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The add-account page and the window with no account, as the design draws them.
//!
//! The form stands in text fields, a password field and selects (Security: TLS or STARTTLS; there is no unencrypted
//! choice), 560 wide, 12 between its rows, 16 under the head; a field's errors stand 6 under its field, under the whole
//! row for a field in a row. Beside it, 40 to its right (the form's own 16 and 24), an always-open column says what
//! Detect asked and where, once it ran; on a page too narrow for both it stands under the form. Detect's answer stands over the form's fields as a note (or a careful
//! one, for a guess or an unencrypted redirect); while Detect runs, a progress line stands there and the server rows
//! wait, disabled. Under a server row whose encryption is unusual for its port (STARTTLS on 993, TLS on 143, …) one
//! muted caption line says so; saving is not held back. Under Detect's note one line says when the server rows are
//! an earlier Detect's, for another address than the one typed now. Under a row Detect filled with a server it did not
//! reach, one line in the same style says why (no answer, refused, no such name, a certificate that misfits the clock),
//! until that row is edited or Detect runs again.
//!
//! The check prints its seven steps as `check-step` lines, one by one as they run (motion M13: the line of the step
//! running is printed when it starts and takes its answer when it ends; the steps after it are not shown yet), in a
//! terminal box; the sentence over the box says how it ended, with the failing step's sentence whole.
//!
//! An account already there gets the same fields, its address read-only, with a new password instead of the password.
//! Under a failed check an error banner says what failed and what to try, in InboxCraft's own words for the step.
//! Coming soon: signing in at the provider (OAuth).

use std::sync::Arc;

use egui::{
    Align, CentralPanel, Color32, Frame, Galley, Layout, Margin, Pos2, Rect, Response, Role, ScrollArea, Sense, Ui, UiBuilder, WidgetInfo, pos2, vec2,
};

use super::paint::{self, Panes};
use crate::component;
use crate::component::banner::{BannerKind, banner};
use crate::component::button::TextButton;
use crate::component::check_step::{self, check_step_named};
use crate::component::checkbox::checkbox_wrapping;
use crate::component::dialog::{self, Answer, Dialog, Tone};
use crate::component::field::{self, Field};
use crate::component::progress::{ProgressState, progress};
use crate::component::select::Select;
use crate::component::soon::{soon, soon_tag};
use crate::iconset::IconId;
use crate::keymap::Keymap;
use crate::mailbox::Mailbox;
use crate::onboard::{
    Form, Found, Onboarding, Outcome, RemoveHand, Secret, Security, Slot, Source, Stage, StepRow, StepState, kept_line, port_typed,
    unusual_encryption,
};
use crate::theme::{Font, Theme};
use crate::tr;

/// What the owner asked for on the page this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Asked {
    Detect,
    Add,
    Save,
    CheckAgain,
    Change,
    Close,
    /// Cancel: closes, and the form forgets what it held.
    Cancel,
    /// "Remove account…" on an account's form: asks, removes nothing.
    AskRemove,
    /// The question's Cancel, or Esc while it asks: the account stays.
    KeepAccount,
    /// The question's "Remove account": the owner's press, the only way `account.remove` runs from the form.
    Remove,
}

/// The widest the form gets, in points.
const FORM: f32 = 560.0;

/// The narrowest the form gets beside the column; narrower, the column stands under it.
const FORM_MIN: f32 = 360.0;

/// The widest and the narrowest the column of what Detect asked gets, in points.
const COLUMN: f32 = 278.0;
const COLUMN_MIN: f32 = 200.0;

/// The room between the form and the column, in points.
const COLUMNS_GAP: f32 = 24.0;

/// The room the form keeps at its right, before the room to the column, in points: the design's form is 592 wide with
/// 16 on each side, and its left 16 is the page's.
const FORM_RIGHT: f32 = 16.0;

/// The room between the form's rows, and between the fields of a row, in points.
const GAP: f32 = 12.0;

/// The room between a field or a row and the error under it, in points.
const UNDER: f32 = 6.0;

/// The widths of a server row's port and security, in points.
const PORT: f32 = 96.0;
const SECURITY: f32 = 132.0;

/// The narrowest a host field gets in its row, in points.
const HOST_MIN: f32 = 120.0;

/// The height of the title's row, and how far down in it the title stands, in points.
const TITLE_ROW: f32 = 32.0;
const TITLE_DOWN: f32 = 1.0;

/// The room between the title's row and the words under it, and between the head and the form, in points.
const HEAD_GAP: f32 = 6.0;
const HEAD_BELOW: f32 = 16.0;

/// The widest the check gets, and the room between its parts, in points.
const CHECK: f32 = 640.0;
const CHECK_GAP: f32 = 16.0;

/// The room inside the check's terminal box, across and down, and between two of its lines, in points.
const LIST_PAD: (f32, f32) = (14.0, 12.0);
const STEP_GAP: f32 = 2.0;

/// The room between the column's entries, in points.
const ASKED_GAP: f32 = 10.0;

/// The most lines a sentence of the page wraps onto before its end is cut with one ellipsis. A sentence from the engine
/// or a server is at most [`crate::mailbox::MAX_SHOWN`] characters, so it never comes near.
const MOST_ROWS: usize = 64;

/// A line of words at `at` that a screen reader reads as `text`.
pub(super) fn said(ui: &mut Ui, at: Pos2, width: f32, galley: Arc<Galley>, colour: Color32, text: &str) {
    let rect = Rect::from_min_size(at, vec2(paint::run_box(galley.size().x).min(width), galley.size().y));
    // Named by where it stands: the same words may stand twice on a page.
    let response = ui.interact(rect, ui.scope_id().with(("said", rect.min.x.to_bits(), rect.min.y.to_bits())), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, text));
    if ui.is_rect_visible(rect) {
        paint::rows_at(&component::solid(ui), at, width, galley, colour);
    }
}

/// `text` in `font` and `colour` across `width` of `ui`, wrapped, read whole by a screen reader; its rect.
fn words(ui: &mut Ui, text: &str, font: Font, colour: Color32, width: f32) -> Rect {
    let width = width.max(1.0);
    let shown = component::plain(text);
    let galley = component::wrapped(ui, &shown, font, colour, width, MOST_ROWS, Align::LEFT);
    let (rect, _) = ui.allocate_exact_size(vec2(width, galley.size().y.max(font.line)), Sense::hover());
    said(ui, rect.min, width, galley, colour, &shown);
    rect
}

/// The page's head: its title on one line, 1 down in a 32-high row, and, when there are any, the words under it in
/// `muted`.
fn head(ui: &mut Ui, theme: &Theme, title: &str, under: Option<&str>, width: f32) {
    let f = &theme.fonts;
    ui.spacing_mut().item_spacing.y = HEAD_GAP;
    let width = width.max(1.0);
    let shown = component::plain(title);
    let galley = component::cut(ui, &shown, f.title, theme.palette.ink, width);
    let (rect, _) = ui.allocate_exact_size(vec2(width, TITLE_ROW.max(f.title.line + TITLE_DOWN)), Sense::hover());
    said(ui, rect.min + vec2(0.0, TITLE_DOWN), width, galley, theme.palette.ink, &shown);
    if let Some(under) = under {
        words(ui, under, f.body, theme.palette.muted, width);
    }
}

/// A part of the form `width` wide in `ui`'s row, top-down.
fn part<R>(ui: &mut Ui, width: f32, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.allocate_ui_with_layout(vec2(width, 0.0), Layout::top_down(Align::Min), |ui| {
        ui.set_width(width);
        add(ui)
    })
    .inner
}

/// The form's errors this frame: those the owner's last try left under their fields, and what is wrong as it is typed.
struct Faults {
    sent: Vec<(Slot, String)>,
    typed: Vec<(Slot, String)>,
    /// The field to take the keyboard this frame, after a refused try.
    focus: Option<Slot>,
}

impl Faults {
    fn of(onboarding: &mut Onboarding) -> Self {
        let focus = onboarding.take_focus();
        Self { sent: onboarding.field_errors().to_vec(), typed: onboarding.form().typed_faults(), focus }
    }

    /// Marks the field `slot` drawn as `response`: a screen reader hears its error after its name, and the first field
    /// a refused try named takes the keyboard, brought into view.
    fn mark(&self, ui: &Ui, slot: Slot, response: &Response) {
        if let Some(error) = self.of_slot(slot) {
            let error = error.to_owned();
            ui.ctx().accesskit_node_builder(response.id, |node| node.set_description(error));
        }
        if self.focus == Some(slot) {
            response.request_focus();
            response.scroll_to_me(Some(Align::Center));
        }
    }

    /// The error under the field `slot`: what is typed wrong first, then what the last try said.
    fn of_slot(&self, slot: Slot) -> Option<&str> {
        self.typed.iter().chain(self.sent.iter()).find(|(at, _)| *at == slot).map(|(_, error)| error.as_str())
    }
}

/// A field or a row of fields that `add` draws, with its `errors` under it, the first `UNDER` below it and each under
/// the one before, in `caption` and `minus`, then its `notes` in `caption` and `muted`.
fn with_error<R>(ui: &mut Ui, theme: &Theme, errors: &[Option<&str>], notes: &[Option<&str>], width: f32, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = UNDER;
        let added = add(ui);
        if errors.iter().chain(notes).any(Option::is_some) {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for error in errors.iter().flatten() {
                    words(ui, error, theme.fonts.caption, theme.palette.minus, width);
                }
                for note in notes.iter().flatten() {
                    words(ui, note, theme.fonts.caption, theme.palette.muted, width);
                }
            });
        }
        added
    })
    .inner
}

/// A one-line field `width` wide named `label`, `hint` in it while it is empty, `error` marking its edge (the caller
/// says it under the row); its response. `key` tells it from a field of the same name on the page.
fn line_field(ui: &mut Ui, theme: &Theme, (label, hint, key): (&str, &str, &str), text: &mut String, error: Option<&str>, width: f32) -> Response {
    part(ui, width, |ui| {
        // Not from the page's place in the window: a field keeps its id, and the keyboard, when the page is laid out
        // anew (the settings' sections column giving way in a narrow window). One form shows at a time.
        let id = egui::Id::unique(("onboard-field", label, key));
        field::quietly(ui, |ui| field::text_field(ui, theme, id, text, &Field { error, ..Field::new(label, hint) }))
    })
}

/// The password field `width` wide under its `caption`, `hint` in it while it is empty: with a hint it stands for a
/// password the credential store holds ([`Secret::Stored`](field::Secret::Stored)). The field keeps nothing it is typed
/// in its undo history, and takes no more than the [`Secret`]'s buffer holds, so typing never moves the text.
fn password(ui: &mut Ui, theme: &Theme, (caption, hint): (&str, &str), secret: &mut Secret, error: Option<&str>, width: f32) -> Response {
    part(ui, width, |ui| {
        let kept = if hint.is_empty() { field::Secret::Typed } else { field::Secret::Stored };
        let id = ui.make_persistent_id("account-password");
        field::quietly(ui, |ui| field::password_field(ui, theme, id, secret.expose_mut(), &Field { error, ..Field::new(caption, hint) }, kept))
    })
}

/// The security of a server row: its label over a select of TLS and STARTTLS, `SECURITY` wide. There is no
/// unencrypted choice. `drop` lower, to stand level with a host box under a caption of more lines. The choice the
/// owner took, if one.
fn security(ui: &mut Ui, theme: &Theme, name: &str, chosen: Security, drop: f32) -> Option<Security> {
    let f = &theme.fonts;
    part(ui, SECURITY, |ui| {
        ui.add_space(drop);
        ui.spacing_mut().item_spacing.y = theme.spacing.s2;
        let caption = tr!("Security");
        let galley = component::galley(ui, caption, f.label, theme.palette.muted);
        let (rect, _) = ui.allocate_exact_size(vec2(SECURITY, f.label.line), Sense::hover());
        said(ui, rect.min, SECURITY, galley, theme.palette.muted, caption);
        let options = [(Security::Tls, Security::Tls.text()), (Security::StartTls, Security::StartTls.text())];
        let key = ui.make_persistent_id(("security-open", name));
        let mut open = ui.data(|data| data.get_temp::<bool>(key)).unwrap_or(false);
        let select = Select { name, shown: chosen.text(), options: &options, current: Some(chosen), icon: IconId::Lock, width: SECURITY };
        let (_, taken) = select.show(ui, theme, &mut open);
        ui.data_mut(|data| data.insert_temp(key, open));
        taken
    })
}

/// One server's row, `width` wide: its host, its port (`usual` as the hint) and its security, with the error of its
/// host or port under the whole row, and a line saying so while the row holds the encryption Detect `set` over one the
/// owner had picked, one while its encryption is unusual for its port, and one while it holds a server Detect proposed
/// without having reached it (`unchecked`, which a screen reader also hears with the host field). What the owner
/// changed in it.
#[allow(clippy::too_many_arguments)]
fn server_row(
    ui: &mut Ui,
    theme: &Theme,
    faults: &Faults,
    (caption, placeholder, name, slots): (&str, &str, &str, [Option<Slot>; 2]),
    (host, port_text, chosen): (&mut String, &mut String, &mut Option<Security>),
    usual: fn(Security) -> u16,
    (set, unchecked): (Option<Security>, Option<&str>),
    width: f32,
) -> Vec<Slot> {
    let mut edited = Vec::new();
    let room = (width - PORT - SECURITY - 2.0 * GAP).max(HOST_MIN);
    let [host_slot, port_slot] = slots;
    let errors = [host_slot.and_then(|slot| faults.of_slot(slot)), port_slot.and_then(|slot| faults.of_slot(slot))];
    let current = chosen.unwrap_or(Security::Tls);
    // The host's and the port's errors both, the one under the other.
    let said = if errors[0] == errors[1] { [errors[0], None] } else { errors };
    let note = set.filter(|set| *chosen == Some(*set)).map(|set| {
        let port = port_typed(port_text).ok().flatten().unwrap_or_else(|| usual(set));
        tr!("Detect set the encryption to {security} for port {port}.", security = set.text(), port = port)
    });
    // An encryption unusual for the port: one line says so, and nothing is held back or changed.
    let unusual = unusual_encryption(port_text, *chosen, usual).map(|(port, registered)| {
        tr!(
            "Port {port} usually goes with {usual}, not {chosen}: check both with your provider.",
            port = port,
            usual = registered.text(),
            chosen = current.text()
        )
    });
    // A host caption of more lines than one (a narrow row) puts the host's box lower: the port and the security come
    // down with it, so the row's boxes stand level, as the frame's.
    let label = theme.fonts.label;
    let mut job = egui::text::LayoutJob::single_section(label.text(caption), label.format(theme.palette.muted));
    job.wrap = egui::text::TextWrapping { max_width: room.max(1.0), ..Default::default() };
    let drop = (ui.painter().layout_job(job).size().y.max(label.line) - label.line).max(0.0);
    with_error(ui, theme, &said, &[note.as_deref(), unusual.as_deref(), unchecked], width, |ui| {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            let response = line_field(ui, theme, (caption, placeholder, name), host, errors[0], room);
            if let Some(slot) = host_slot {
                faults.mark(ui, slot, &response);
            }
            // The host field is read with the line that says its server was not checked, after its error if it has one.
            if let Some(unchecked) = unchecked {
                let heard = errors[0].map_or_else(|| unchecked.to_owned(), |error| format!("{error} {unchecked}"));
                ui.ctx().accesskit_node_builder(response.id, |node| node.set_description(heard));
            }
            if response.changed() {
                edited.extend(host_slot);
            }
            let usual = usual(current).to_string();
            let response = part(ui, PORT, |ui| {
                ui.add_space(drop);
                line_field(ui, theme, (tr!("Port"), &usual, name), port_text, errors[1], PORT)
            });
            if let Some(slot) = port_slot {
                faults.mark(ui, slot, &response);
            }
            if response.changed() {
                edited.extend(port_slot);
            }
            if let Some(taken) = security(ui, theme, name, current, drop) {
                *chosen = Some(taken);
            }
        });
    });
    edited
}

/// The names of the account, then the user name beside the password (`caption` over it, `hint` in it while it is
/// empty). What the owner changed.
fn names(ui: &mut Ui, theme: &Theme, faults: &Faults, form: &mut Form, width: f32, password_words: (&str, &str)) -> Vec<Slot> {
    let half = ((width - GAP) / 2.0).floor().max(1.0);
    let mut edited = Vec::new();
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = GAP;
        line_field(ui, theme, (tr!("Name for the account"), tr!("the address"), ""), &mut form.label, None, half);
        line_field(ui, theme, (tr!("Name on sent mail"), tr!("optional"), ""), &mut form.display_name, None, width - GAP - half);
    });
    let error = faults.of_slot(Slot::Password);
    with_error(ui, theme, &[error], &[], width, |ui| {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            line_field(ui, theme, (tr!("User name"), tr!("the address"), ""), &mut form.username, None, half);
            let response = password(ui, theme, password_words, &mut form.password, error, width - GAP - half);
            faults.mark(ui, Slot::Password, &response);
            if response.changed() {
                edited.push(Slot::Password);
            }
        });
    });
    edited
}

/// The incoming server, and the outgoing server and the CalDAV address, which are optional; waiting, disabled, while
/// Detect runs. What the owner changed.
fn servers(
    ui: &mut Ui,
    theme: &Theme,
    faults: &Faults,
    form: &mut Form,
    (width, detecting): (f32, bool),
    (set, unchecked): ([Option<Security>; 2], &[Option<String>; 2]),
) -> Vec<Slot> {
    ui.add_enabled_ui(!detecting, |ui| {
        ui.spacing_mut().item_spacing.y = GAP;
        let mut edited = server_row(
            ui,
            theme,
            faults,
            (tr!("IMAP server"), "imap.example.org", tr!("IMAP security"), [Some(Slot::Host), Some(Slot::Port)]),
            (&mut form.host, &mut form.port, &mut form.security),
            Security::imap_port,
            (set[0], unchecked[0].as_deref()),
            width,
        );
        edited.extend(server_row(
            ui,
            theme,
            faults,
            (tr!("Outgoing server (SMTP), to send"), "smtp.example.org", tr!("Outgoing security"), [None, Some(Slot::SmtpPort)]),
            (&mut form.smtp_host, &mut form.smtp_port, &mut form.smtp_security),
            Security::submission_port,
            (set[1], unchecked[1].as_deref()),
            width,
        ));
        let error = faults.of_slot(Slot::Caldav);
        with_error(ui, theme, &[error], &[], width, |ui| {
            let words = (tr!("CalDAV address, for calendars"), tr!("optional, https://dav.example.org"), "");
            let response = line_field(ui, theme, words, &mut form.caldav, error, width);
            faults.mark(ui, Slot::Caldav, &response);
            if response.changed() {
                edited.push(Slot::Caldav);
            }
        });
        edited
    })
    .inner
}

/// What Detect found, over the form's fields, as the form now holds it: where the server in the form came from, as a
/// note, or as a careful one when it is a guess or came through an unencrypted redirect; a server Detect found and left
/// out because the owner typed another; or that it found none.
fn found_note(ui: &mut Ui, theme: &Theme, keymap: &Keymap, found: &Found) {
    let (kind, text) = match (found.source, found.kept_imap.as_deref()) {
        (Some(source), _) => (if source.doubtful() { BannerKind::Careful } else { BannerKind::Notice }, source.text().to_owned()),
        (None, Some(host)) => (BannerKind::Notice, kept_line(host, found.kept_unchecked[0], false)),
        (None, None) => (BannerKind::Notice, tr!("Detect found no server: type the settings in.").to_owned()),
    };
    banner(ui, theme, keymap, kind, &text, &[]);
    let first_doubtful = found.source.is_some_and(|source| source.doubtful());
    if let Some((host, source)) = found.outgoing.as_ref().filter(|(_, source)| source.doubtful() && !first_doubtful) {
        let text = match source {
            Source::Provider => tr!("{host} was a guess from your domain's mail provider: check it.", host = host),
            _ => tr!("{host} was a guess from common names: check it.", host = host),
        };
        banner(ui, theme, keymap, BannerKind::Careful, &text, &[]);
    }
    if let Some(host) = found.kept_outgoing.as_deref() {
        banner(ui, theme, keymap, BannerKind::Notice, &kept_line(host, found.kept_unchecked[1], true), &[]);
    }
}

/// The domain of `address`: what Detect asks about.
fn domain(address: &str) -> &str {
    address.trim().rsplit_once('@').map_or("", |(_, domain)| domain)
}

/// The address with Detect beside it, what Detect is doing or found, the box to ask Mozilla's list too, and what
/// Detect sends. Whether Detect was pressed, and whether the address changed.
fn detect_part(ui: &mut Ui, theme: &Theme, keymap: &Keymap, onboarding: &mut Onboarding, faults: &Faults, width: f32) -> (bool, bool) {
    let f = &theme.fonts;
    let detecting = onboarding.detecting();
    let found = onboarding.found_shared();
    let older = onboarding.detect_older().then(|| {
        if onboarding.detect_older_user_name() {
            tr!("The servers and the user name below come from an earlier Detect, for another address.")
        } else {
            tr!("The servers below come from an earlier Detect, for another address.")
        }
    });
    let form = onboarding.form_mut();
    let (mut detect, mut edited) = (false, false);
    let name = if detecting { tr!("Detecting…") } else { tr!("Detect") };
    let button = paint::run_box(component::galley(ui, name, f.button, theme.palette.ink).size().x) + 2.0 * theme.spacing.s3;
    let error = faults.of_slot(Slot::Address);
    with_error(ui, theme, &[error], &[], width, |ui| {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            let room = (width - button - GAP).max(HOST_MIN);
            let response = line_field(ui, theme, (tr!("Address"), "you@example.org", ""), &mut form.address, error, room);
            faults.mark(ui, Slot::Address, &response);
            edited = response.changed();
            part(ui, button, |ui| {
                // The button stands in the middle of the field's box, under its label.
                ui.add_space(f.label.line + theme.spacing.s2 + ((theme.boxes().field - component::control_side(theme)) / 2.0).max(0.0));
                detect = TextButton::new(name).secondary().enabled(!detecting).show(ui, theme, keymap).clicked();
            });
        });
    });
    if checkbox_wrapping(ui, theme, tr!("Also ask Mozilla's list of providers (tells Mozilla the domain)"), form.ispdb).clicked() {
        form.ispdb = !form.ispdb;
    }
    // While Detect runs its progress stands over what it sends; once it answered, what it found stands there instead.
    if detecting {
        progress(ui, theme, keymap, tr!("detecting"), ProgressState::Running(0.0), domain(&form.address));
    }
    match found.as_ref().filter(|_| !detecting) {
        Some(found) => {
            found_note(ui, theme, keymap, found);
            // The address changed since Detect answered and the rows below still hold its servers: one line in the
            // form's hint style says so, until Detect runs again or a server field is edited by hand.
            if let Some(older) = older {
                words(ui, older, f.caption, theme.palette.muted, width);
            }
        }
        None => {
            let sends = tr!(
                "Detect asks your name servers and your domain's web server (once without encryption) about your domain; it sends no address, user name or password."
            );
            words(ui, sends, f.caption, theme.palette.muted, width);
        }
    }
    (detect, edited)
}

/// The column of what Detect asked, `width` wide: each question in `ink` with, when it failed, why under it in
/// `muted`; then the lookups that failed for no question. Nothing before Detect ran.
fn asked_column(ui: &mut Ui, theme: &Theme, found: Option<&Found>, width: f32) {
    let Some(found) = found else { return };
    let p = &theme.palette;
    let f = &theme.fonts;
    ui.spacing_mut().item_spacing.y = ASKED_GAP;
    component::label(ui, f.label, &tr!("What Detect asked ({count})", count = found.asked_count()), p.muted);
    for (question, cause) in found.column() {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            words(ui, question, f.mono_s, p.ink, width);
            if let Some(cause) = cause {
                words(ui, cause, f.mono_s, p.muted, width);
            }
        });
    }
    if found.asked.is_empty() {
        words(ui, tr!("Nothing left this computer."), f.mono_s, p.muted, width);
    }
    for line in found.alone() {
        words(ui, line, f.mono_s, p.muted, width);
    }
}

/// The widths of the form and the column in `room`, and whether the column stands beside the form (the form's room at
/// its right and the gap between them).
fn columns(room: f32) -> (f32, f32, bool) {
    let room = if room.is_finite() { room.max(1.0) } else { FORM };
    let column = (room - FORM - FORM_RIGHT - COLUMNS_GAP).clamp(COLUMN_MIN, COLUMN);
    let form = (room - FORM_RIGHT - COLUMNS_GAP - column).min(FORM);
    if form >= FORM_MIN { (form, column, true) } else { (room.min(FORM), room.min(FORM), false) }
}

/// The page's error that belongs to no field, as an error banner over the buttons.
fn page_error(ui: &mut Ui, theme: &Theme, keymap: &Keymap, onboarding: &Onboarding) {
    if let Some(error) = onboarding.page_error() {
        banner(ui, theme, keymap, BannerKind::Error, error, &[]);
    }
}

/// Signing in at the provider, coming soon: the second way in, drawn disabled with its tag.
fn provider_soon(ui: &mut Ui, theme: &Theme, keymap: &Keymap) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = GAP;
        soon(ui, theme, |ui| TextButton::new(tr!("Sign in at the provider")).secondary().show(ui, theme, keymap));
        soon_tag(ui, theme);
    });
}

/// The add form beside the column of what Detect asked; what was asked for.
fn form(ui: &mut Ui, theme: &Theme, keymap: &Keymap, onboarding: &mut Onboarding) -> Option<Asked> {
    let f = &theme.fonts;
    let p = &theme.palette;
    let mut asked = None;
    let (width, column, beside) = columns(ui.available_width());
    head(
        ui,
        theme,
        tr!("Add an account"),
        Some(tr!("IMAP for the mail. Detect proposes the servers from the address; nothing is sent until you have seen them.")),
        width,
    );
    ui.add_space(HEAD_BELOW - HEAD_GAP);
    let faults = Faults::of(onboarding);
    let found = onboarding.found_shared();
    let said = onboarding.encryption_said();
    let unchecked = onboarding.unchecked();
    let mut edited = Vec::new();
    let mut picked = Vec::new();
    let mut form_part = |ui: &mut Ui| {
        ui.spacing_mut().item_spacing.y = GAP;
        let (detect, address) = detect_part(ui, theme, keymap, onboarding, &faults, width);
        if detect {
            asked = Some(Asked::Detect);
        }
        if address {
            edited.push(Slot::Address);
        }
        let detecting = onboarding.detecting();
        let form = onboarding.form_mut();
        edited.extend(names(ui, theme, &faults, form, width, (tr!("Password"), "")));
        let before = [form.security, form.smtp_security];
        edited.extend(servers(ui, theme, &faults, form, (width, detecting), (said, &unchecked)));
        // A row whose encryption the owner picked: Detect's line there is said no more.
        picked.extend([form.security, form.smtp_security].iter().zip(before).enumerate().filter(|(_, (now, was))| **now != *was).map(|(row, _)| row));
        words(ui, tr!("The password goes into your system's credential store, never into InboxCraft's files."), f.caption, p.muted, width);
        words(
            ui,
            tr!("Accounts are not kept yet: when InboxCraft closes, it removes their passwords from the store again."),
            f.caption,
            p.muted,
            width,
        );
        page_error(ui, theme, keymap, onboarding);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            if TextButton::new(tr!("Check and add")).primary().arrow().enabled(!detecting).show(ui, theme, keymap).clicked() {
                asked = Some(Asked::Add);
            }
            if TextButton::new(tr!("Cancel")).plain().show(ui, theme, keymap).clicked() {
                asked = Some(Asked::Cancel);
            }
        });
        provider_soon(ui, theme, keymap);
    };
    if beside {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = FORM_RIGHT + COLUMNS_GAP;
            part(ui, width, &mut form_part);
            part(ui, column, |ui| asked_column(ui, theme, found.as_deref(), column));
        });
    } else {
        part(ui, width, &mut form_part);
        ui.add_space(COLUMNS_GAP);
        part(ui, column, |ui| asked_column(ui, theme, found.as_deref(), column));
    }
    for slot in edited {
        onboarding.edited(slot);
    }
    for row in picked {
        onboarding.encryption_picked(row);
    }
    asked
}

/// The form of an account already there: the add form's fields, the address read-only, a new password instead of the
/// password, and where the sign-in will go when it would go anywhere new. `writing`: a message is being written from
/// this account.
fn edit_form(ui: &mut Ui, theme: &Theme, keymap: &Keymap, onboarding: &mut Onboarding, writing: bool) -> Option<Asked> {
    let f = &theme.fonts;
    let p = &theme.palette;
    let mut asked = None;
    let width = ui.available_width().min(FORM);
    head(ui, theme, tr!("Account settings"), None, width);
    ui.add_space(HEAD_BELOW - HEAD_GAP);
    let faults = Faults::of(onboarding);
    let (checks, note) = onboarding.save_summary();
    let label = if checks { tr!("Save and check") } else { tr!("Save") };
    let mut edited = Vec::new();
    let asking = onboarding.asking_remove();
    let hand = onboarding.take_remove_hand();
    let question =
        Question { name: onboarding.removed_name(), address: crate::component::plain(onboarding.form().address.trim()).into_owned(), writing };
    part(ui, width, |ui| {
        ui.spacing_mut().item_spacing.y = GAP;
        let form = onboarding.form_mut();
        part(ui, width, |ui| {
            let mut address = form.address.clone();
            let id = ui.make_persistent_id("onboard-address-fixed");
            field::text_field(ui, theme, id, &mut address, &Field { read_only: true, ..Field::new(tr!("Address"), "") });
        });
        edited.extend(names(ui, theme, &faults, form, width, (tr!("New password"), tr!("empty keeps the stored one"))));
        edited.extend(servers(ui, theme, &faults, form, (width, false), ([None, None], &[None, None])));
        words(ui, tr!("A new password goes into your system's credential store. The stored one is never shown."), f.caption, p.muted, width);
        if let Some(text) = &note {
            words(ui, text, f.caption, p.ink, width);
        }
        page_error(ui, theme, keymap, onboarding);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            if TextButton::new(label).primary().show(ui, theme, keymap).clicked() {
                asked = Some(Asked::Save);
            }
            if TextButton::new(tr!("Cancel")).plain().show(ui, theme, keymap).clicked() {
                asked = Some(Asked::Cancel);
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let ask = TextButton::new(tr!("Remove account…")).danger().show(ui, theme, keymap);
                if hand == Some(RemoveHand::Ask) {
                    ask.request_focus();
                }
                if ask.clicked() {
                    asked = Some(Asked::AskRemove);
                }
            });
        });
    });
    for slot in edited {
        onboarding.edited(slot);
    }
    // The question, over the dimmed window: its answer is the only ask while it shows.
    if asking && let Some(answer) = remove_question(ui, theme, keymap, &question) {
        asked = Some(answer);
    }
    asked
}

/// The words of the question whether to remove an account: its name and its address, each one plain line, and
/// whether a message is being written from it.
struct Question {
    name: String,
    address: String,
    writing: bool,
}

/// The question whether to remove the account, in its dialog: first that it cannot be undone, then what goes from
/// this computer and what stays on the server; Cancel takes the keyboard, "Remove account" removes. What the owner
/// answered, if anything.
fn remove_question(ui: &Ui, theme: &Theme, keymap: &Keymap, question: &Question) -> Option<Asked> {
    let heading = tr!("Remove {name} from this computer?", name = question.name);
    let goes = tr!(
        "From this computer go: the mail and folders of {name} kept here, its calendars, its drafts, and its password in your system's credential store.",
        name = question.name
    );
    let writing = tr!("The message you are writing from {name} is discarded with it.", name = question.name);
    let stays = tr!(
        "Everything on the server stays: {address} keeps its mail and its calendars, and the account can be added again.",
        address = question.address
    );
    let mut lines = vec![(tr!("This cannot be undone."), Tone::Ink), (goes.as_str(), Tone::Ink)];
    if question.writing {
        lines.push((writing.as_str(), Tone::Ink));
    }
    lines.push((stays.as_str(), Tone::Muted));
    let dialog =
        Dialog { id: egui::Id::unique("account-remove-confirm"), question: &heading, lines: &lines, keep: tr!("Cancel"), act: tr!("Remove account") };
    // A key already down when the button took the keyboard, or held on from the press that asked, is no answer
    // ([`crate::component::button::owner_pressed`], in the dialog).
    match dialog::show(ui.ctx(), theme, keymap, &dialog)? {
        Answer::Keep => Some(Asked::KeepAccount),
        Answer::Act => Some(Asked::Remove),
    }
}

/// A step's short name on its line, as a terminal prints it.
fn short(row: &StepRow) -> &'static str {
    match row.id.as_str() {
        "dns" => tr!("DNS"),
        "tcp" => tr!("TCP"),
        "tls" => tr!("TLS"),
        "certificate" => tr!("Cert"),
        "imap_login" => tr!("IMAP"),
        "smtp_login" => tr!("SMTP"),
        "caldav_discovery" => tr!("CalDAV"),
        _ => tr!("Step"),
    }
}

/// What a step checks, in words.
fn checks(row: &StepRow) -> &'static str {
    match row.id.as_str() {
        "dns" => tr!("name lookup"),
        "tcp" => tr!("connection"),
        "tls" => tr!("encryption"),
        "certificate" => tr!("certificate"),
        "imap_login" | "smtp_login" => tr!("sign-in"),
        "caldav_discovery" => tr!("calendars"),
        _ => tr!("check"),
    }
}

/// How a step's line stands, `running` when it is the one under way.
fn line_state(state: StepState, running: bool) -> check_step::StepState {
    match state {
        StepState::Pending if running => check_step::StepState::Running,
        StepState::Pending => check_step::StepState::Waiting,
        StepState::Passed => check_step::StepState::Passed,
        StepState::Failed => check_step::StepState::Failed,
        StepState::Skipped => check_step::StepState::Skipped,
        StepState::Held => check_step::StepState::NotRun,
        StepState::NotSet => check_step::StepState::NoServer,
        StepState::NotBuilt => check_step::StepState::NotBuilt,
    }
}

/// The steps to print and how each line stands: while the check runs, those that ended and the one under way (motion
/// M13: a line is printed as its step starts; at motion off, `still`, as it ends); the steps after it are not shown yet.
/// A check that is not running shows every step.
pub(super) fn printed(steps: &[StepRow], running: bool, still: bool) -> Vec<(&StepRow, check_step::StepState)> {
    let mut lines = Vec::new();
    for row in steps {
        let under_way = running && row.state == StepState::Pending;
        if under_way && still {
            break;
        }
        lines.push((row, line_state(row.state, under_way)));
        if under_way {
            break;
        }
    }
    lines
}

/// One step's line: its short name, what it checks and where, a note at the right, and under it, when it failed, the
/// engine's sentence; a screen reader hears its whole name and how it stands, then the server and port it reached and
/// the certificate's issuer.
fn step(ui: &mut Ui, theme: &Theme, (at_line, row): (usize, &StepRow), state: check_step::StepState) {
    let ran = !matches!(row.state, StepState::Skipped | StepState::NotSet | StepState::NotBuilt | StepState::Held);
    // A skipped step's line still names where it would have gone, as the frames have it.
    let named = ran || row.state == StepState::Skipped;
    let mut detail = checks(row).to_owned();
    if let Some(issuer) = row.issuer.as_ref().filter(|_| ran) {
        detail = format!("{detail} · {}", tr!("issued by {issuer}", issuer = issuer));
    } else if let Some(server) = row.server.as_ref().filter(|_| named) {
        detail = format!("{detail} · {server}");
    }
    let note = match state {
        check_step::StepState::Running => "…",
        check_step::StepState::Skipped => tr!("skipped"),
        check_step::StepState::NotRun => tr!("not run"),
        check_step::StepState::NoServer => tr!("no server"),
        check_step::StepState::NotBuilt => tr!("not built"),
        check_step::StepState::Waiting | check_step::StepState::Passed | check_step::StepState::Failed => "",
    };
    let spoken = check_step::spoken(row.name(), state);
    let at = ui.cursor().min;
    check_step_named(ui, theme, short(row), &spoken, &detail, note, state, row.message.as_deref());
    // Where the step went and who vouched for the server, for a screen reader: the server and port, and the
    // certificate's issuer, which the line prints in short.
    let reached: Vec<String> = [row.server.clone(), row.issuer.as_ref().map(|issuer| tr!("issued by {issuer}", issuer = issuer))]
        .into_iter()
        .flatten()
        .filter(|_| ran)
        .collect();
    if !reached.is_empty() {
        let line = Rect::from_min_max(at, pos2(ui.max_rect().right(), ui.min_rect().bottom()));
        let response = ui.interact(line, ui.scope_id().with(("step-reached", at_line, row.id.as_str())), Sense::hover());
        let text = reached.join(" · ");
        response.widget_info(|| WidgetInfo::labeled(Role::Label, true, &text));
    }
}

/// The sentence over the steps: how the check stands, or the failing step's sentence whole.
fn outcome_line(ui: &mut Ui, theme: &Theme, onboarding: &Onboarding, width: f32) {
    let p = &theme.palette;
    let f = &theme.fonts;
    match onboarding.outcome() {
        Outcome::Running => {
            words(ui, tr!("Each step is printed as it starts and gets its answer as it ends."), f.body, p.muted, width);
        }
        Outcome::Passed => {
            words(ui, tr!("Signed in. InboxCraft now fetches the folders and the mail."), f.body, p.ink, width);
        }
        Outcome::Failed => {
            let steps = onboarding.steps();
            let failed = steps.iter().find(|row| row.state == StepState::Failed);
            let held = steps.iter().find(|row| row.state == StepState::Held);
            let text = match (failed, held) {
                (Some(row), _) => match row.message.as_deref() {
                    Some(message) => tr!("{step} failed: {message}", step = row.name(), message = message),
                    None => tr!("{step} failed.", step = row.name()),
                },
                (None, Some(row)) => match row.message.as_deref() {
                    Some(message) => tr!("{step}: {message}", step = row.name(), message = message),
                    None => tr!("{step} did not run.", step = row.name()),
                },
                (None, None) => tr!("A step failed. Check again, or change the settings.").to_owned(),
            };
            words(ui, &text, f.body, p.minus, width);
        }
    }
}

/// The most characters of a server's name the plain words under a failed check carry: a DNS name's longest.
const PLAIN_HOST: usize = 253;

/// The server `row` reached, without its port, as the plain words name it: already made fit for one line when it was
/// read ([`crate::mailbox::shown`]), cut to [`PLAIN_HOST`] characters with one ellipsis.
fn plain_host(row: &StepRow) -> String {
    let server = row.server.as_deref().unwrap_or_default();
    let host =
        server.rsplit_once(':').filter(|(_, port)| !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit())).map_or(server, |(host, _)| host);
    let host = crate::mailbox::shown(host);
    if host.chars().count() > PLAIN_HOST { format!("{}…", host.chars().take(PLAIN_HOST).collect::<String>()) } else { host }
}

/// What failed and what to try, in InboxCraft's own words for the step that failed, as `account-failed/*` has them;
/// never the server's reply, which the line over the steps carries. A sign-in or the calendars name a cause (a refused
/// password, no calendars) only when the check reported that kind of failure; otherwise their words are neutral.
/// `None` for a step with no words of its own.
pub(super) fn plain_words(row: &StepRow) -> Option<String> {
    let host = plain_host(row);
    let refused = row.kind.as_deref() == Some("login_refused");
    let words = match row.id.as_str() {
        "dns" => tr!(
            "No server answers to the name {host}. Check the spelling of the server, or let Detect propose it. Nothing was sent to anyone.",
            host = host
        ),
        "tcp" => tr!("{host} exists but does not answer on that port. Check the port: IMAP over TLS is usually 993. No password was sent.", host = host),
        "tls" => tr!(
            "Encrypt or refuse: InboxCraft signs in only over TLS or STARTTLS, and no encryption came about with {host}, so the password was not sent. There is no setting that sends it in the clear.",
            host = host
        ),
        "certificate" => tr!(
            "The server could not prove that it is {host}. The sign-in was not tried and the password was not sent, and there is no way to go on with this certificate. Check the server's name; if it is right, the provider has to put its certificate right.",
            host = host
        ),
        // A sign-in names its cause only when the check reported it: the SMTP sign-in and the calendars take their
        // connection, encryption and certificate in the same step, and those fail there too.
        "imap_login" if refused => {
            tr!("Check the user name and the password. Some providers want a password made for mail programs in place of the account's own.").to_owned()
        }
        "imap_login" => tr!("Could not sign in to {host}. Check the server, its port and the user name, then try again.", host = host),
        "smtp_login" if refused => tr!(
            "Reading mail works; sending does not. The outgoing server {host} refused the same user name and password. Check the outgoing server and its port; some providers use another user name there.",
            host = host
        ),
        "smtp_login" => tr!(
            "Reading mail works; sending does not. Could not sign in to the outgoing server {host}. Check its address, its port and the user name.",
            host = host
        ),
        "caldav_discovery" if row.kind.as_deref() == Some("no_calendar_server") => {
            tr!("Mail works. No calendars were found at the CalDAV address. Check the address, or empty the field to keep the account without calendars.").to_owned()
        }
        "caldav_discovery" => {
            tr!("Mail works. The calendars could not be reached at the CalDAV address. Check the address, or empty the field to keep the account without calendars.").to_owned()
        }
        _ => return None,
    };
    Some(words)
}

/// The plain words under a failed check, as an error banner: for the step that failed, when it has words of its own.
fn plain_words_banner(ui: &mut Ui, theme: &Theme, keymap: &Keymap, onboarding: &Onboarding) {
    if let Some(words) = onboarding.steps().iter().find(|row| row.state == StepState::Failed).and_then(plain_words) {
        banner(ui, theme, keymap, BannerKind::Error, &words, &[]);
    }
}

fn checking(ui: &mut Ui, theme: &Theme, keymap: &Keymap, mailbox: &Mailbox, onboarding: &Onboarding, account: u64) -> Option<Asked> {
    let p = &theme.palette;
    let mut asked = None;
    let width = ui.available_width().min(CHECK);
    let address = mailbox.account(account).map(|row| row.address.clone()).unwrap_or_default();
    let title = match onboarding.outcome() {
        Outcome::Running => tr!("Checking {address}", address = address),
        Outcome::Passed | Outcome::Failed => tr!("Checked {address}", address = address),
    };
    head(ui, theme, &title, None, width);
    ui.add_space(CHECK_GAP - HEAD_GAP);
    part(ui, width, |ui| {
        ui.spacing_mut().item_spacing.y = CHECK_GAP;
        outcome_line(ui, theme, onboarding, width);
        page_error(ui, theme, keymap, onboarding);
        // The steps in a terminal box.
        let running = onboarding.outcome() == Outcome::Running;
        let lines = printed(onboarding.steps(), running, crate::motion::level(ui.ctx()) == crate::motion::Level::Off);
        let inner = (width - 2.0 * LIST_PAD.0 - 2.0).max(1.0);
        let shown = Frame::new().fill(p.term).inner_margin(component::margin(LIST_PAD.0 + 1.0, LIST_PAD.1 + 1.0)).show(ui, |ui| {
            ui.set_width(inner);
            ui.spacing_mut().item_spacing.y = STEP_GAP;
            for (at_line, (row, state)) in lines.into_iter().enumerate() {
                step(ui, theme, (at_line, row), state);
            }
        });
        let edge = shown.response.rect;
        ui.painter().rect_stroke(edge, 0.0, egui::Stroke::new(1.0, p.line), egui::StrokeKind::Inside);
        match onboarding.outcome() {
            Outcome::Passed => {
                if TextButton::new(tr!("Done")).primary().show(ui, theme, keymap).clicked() {
                    asked = Some(Asked::Close);
                }
            }
            Outcome::Failed => {
                plain_words_banner(ui, theme, keymap, onboarding);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = GAP;
                    if TextButton::new(tr!("Check again")).primary().show(ui, theme, keymap).clicked() {
                        asked = Some(Asked::CheckAgain);
                    }
                    if TextButton::new(tr!("Change settings")).secondary().show(ui, theme, keymap).clicked() {
                        asked = Some(Asked::Change);
                    }
                });
            }
            // The way back to the list is the link above the page.
            Outcome::Running => {}
        }
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

/// Draws the page and returns what was asked for; the caller applies it after the frame's drawing. `writing` is the
/// account the message being written is from: the question to remove that account says the message goes with it.
pub(super) fn show(
    ui: &mut Ui,
    theme: &Theme,
    keymap: &Keymap,
    mailbox: &Mailbox,
    onboarding: &mut Onboarding,
    writing: Option<u64>,
) -> Option<Asked> {
    let mut asked = None;
    component::scroll::show(ui, theme, ScrollArea::vertical().auto_shrink([false, false]), |ui| {
        asked = match onboarding.stage() {
            Stage::Form => form(ui, theme, keymap, onboarding),
            Stage::Edit(account) => edit_form(ui, theme, keymap, onboarding, writing == Some(account)),
            Stage::Checking(account) => checking(ui, theme, keymap, mailbox, onboarding, account),
            Stage::Closed => None,
        };
        // The form scrolls to the control Tab comes to, as a settings page does.
        super::settings::follow_focus(ui);
    });
    asked
}
