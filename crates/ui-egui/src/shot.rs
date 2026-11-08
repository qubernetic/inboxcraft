// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Headless pictures of the window for `ui_shot`: egui's shapes tessellated into triangles and filled on the CPU,
//! written as PNG. No window, no GPU and no network; the sample mailbox at a fixed time, so a picture only changes
//! when the look or the content does.
//!
//! A script is JSON lines, one picture per line:
//!
//! ```json
//! {"out": "docs/images/list-light.png", "mode": "light", "size": [1200, 760]}
//! {"out": "docs/images/list-dark.png", "mode": "dark", "narrow": 2, "open": {"role": "inbox"}, "search": "logo"}
//! ```
//!
//! `mode` is `light` or `dark`, `preset` the accent preset (`electric` when left out, `aurora`, `citrus`), `size` the window in points (1200 by 760 when left out), `scale` the pixels per
//! point (1 to 4, 1 when left out), `text_scale` the text scale in per cent (90 to 130 in steps of 10, 100 when left
//! out), `narrow` an account id, `open` a `role` or a `folder` id, `search` the text in
//! the search field and `select` the subject of the row to select. `onboard` shows the add-account page over the
//! sample: `form` (typed by hand), `detected` (after Detect), `checking` (half the steps in), `passed` or `failed`
//! (the certificate refused); the check is scripted and nothing connects; it shows on the settings sheet's Accounts
//! page. `settings` set to `true` shows the appearance settings page, `accounts` set to `true` the Accounts page,
//! `shortcuts` set to `true` the Shortcuts page, where `capture` names the action waiting for a key
//! (`ui.calendar.show`), `sending` set to `true` the Sending page, `triage` set to `true` the Triage page,
//! `notifications` set to `true` the Notifications page (Studio notifying of people only, and a desk that runs in the
//! background and starts with the computer, [`PicturedDesk`]). `palette` opens the command palette with that text
//! typed (`""` for the whole list), and `calendar` set to `true` shows the calendar screen (with `first`, its empty
//! state), `calendar_view` shows it in a view (`day`, `week`, `month` or `agenda`) and `event` picks the event with
//! that summary, whose details show in the side panel; `edit_event` opens the event editor on a `new` event or, with
//! `edit`, on the picked one, and `delete_event` set to `true` deletes the picked event, so its toast shows. `compose`
//! opens the
//! composer: `new`, `written`, `bad` (a recipient that is no address), `suggest` (the people a half-typed name could
//! be), `floating` (a message opened over it), `reply` (a reply to Odile's rota, a line written over the quote) or
//! `template` (two people and a template with a blank); the first account then has an invented signature.
//! `first` set to `true` draws the window with no account yet instead of the sample, one that can add accounts.
//! `open` takes `{"role": "senders"}` for "New senders", and `view` shows the inbox's groups as `cards` (when left
//! out), a grouped `list` or the `simple` list. `showcase` set to `true` shows invented waiting senders and groups
//! instead of the sample's ([`sample::showcase`]) and gives their buttons keys; `step`
//! selects that many rows down by keyboard, as the arrow keys do, so the selected row shows the hover actions. `act`
//! does an act to the selected row (`read`, `unread`, `flag`, `unflag`, `done`, `archive` or `delete`; `accept` into
//! Updates, `accept_open`, `block`, or `newsletters` for its sender's group), so the toast shows; `slide` set to `true` catches a `done`, `archive` or `delete` row sliding out instead, four frames in with
//! motion on.
//! `icons` set to `true` draws every icon of
//! the set in every style in the preset's colours instead of the window, `components` set to `true` the gallery of
//! the components ([`crate::component::gallery`]). `font` is the typeface (`figtree` when left out, `inter`) and
//! `icon_set` the icon set (`phosphor` when left out, `lucide`), for the window, the gallery and the icon sheet alike;
//! `frame` draws the app's own window frame as a native window does (`strip` or `top_row`); `pointer` rests the pointer
//! at `[x, y]` points, so what lies under it shows its hover; `put_away` set to `true` snoozes, pins, parks and marks
//! done some of the sample's mail first ([`sample::put_away`]), `later` opens a pile of "Later" (`snoozed`, `pins`
//! or `parked`), `show_done` set to `true` lists the done mail in its old place, and `picker` opens the time picker
//! over the selected message with that text typed (`""` for none). `outbox` shows the sample with its first account
//! able to send and its sent message bounced ([`sample::sending`]): `waiting` the outbox with a send counting down its
//! hold, one scheduled and one the server refused, `countdown` the toast of a message just sent, `schedule` the
//! composer with a time picked and its send-later picker open, `bounce` the bounced message open in the reader;
//! the window takes them through the `ui.*.set` commands. Every picture but a `slide` one is taken with reduced
//! motion, so it shows where each motion ends.

use std::collections::HashMap;
use std::io::Write;
use std::sync::Arc;

use egui::epaint::{ClippedPrimitive, ImageData, Mesh, Primitive, TextureId};
use egui::{Color32, Pos2, RawInput, Rect, Vec2, pos2, vec2};
use inboxcraft_engine::appearance::{Icons, Typeface, WindowFrame};
use inboxcraft_model::{CheckStep, ErrorKind, Role, StepStatus};
use inboxcraft_time::{FakeClock, Timestamp};
use serde::Deserialize;

use crate::calendar::{EventRow, View};
use crate::component::gallery;
use crate::compose::Line;
use crate::mailbox::{Act, GroupKind, InboxView, Mailbox, Pile, Place};
use crate::onboard::{Keychain, Proposal, Proposed, Secret, Security, Source, Suggestion};
use crate::save::NoSave;
use crate::theme::{self, Look, Mode, Preset, TextScale, Theme};
use crate::workers::Scripted;
use crate::{InboxApp, UiError, iconset, motion, sample};

/// The largest picture side in pixels.
pub const MAX_SIDE: usize = 4096;

/// The time the sample mailbox is shown at: 2026-11-05T09:00:00Z.
pub const SHOT_TIME: i64 = 1_793_869_200;

/// What a script line asks to open.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "lowercase")]
pub enum Open {
    /// A unified view by role name (`inbox`, `sent` ...), or `senders` for "New senders".
    Role(String),
    /// A folder by id.
    Folder(u64),
}

/// One picture of a script.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Shot {
    /// Where the PNG goes.
    pub out: String,
    /// `light` or `dark`.
    pub mode: String,
    /// The accent preset's id; Electric when left out.
    #[serde(default)]
    pub preset: Option<String>,
    /// The window in points.
    #[serde(default)]
    pub size: Option<[f32; 2]>,
    /// Pixels per point.
    #[serde(default)]
    pub scale: Option<f32>,
    /// The text scale in per cent (90 to 130 in steps of 10); 100 when left out.
    #[serde(default)]
    pub text_scale: Option<u16>,
    /// The account to narrow to.
    #[serde(default)]
    pub narrow: Option<u64>,
    /// The view or folder to open.
    #[serde(default)]
    pub open: Option<Open>,
    /// The search text.
    #[serde(default)]
    pub search: Option<String>,
    /// The subject of the row to select, which opens it in the reader.
    #[serde(default)]
    pub select: Option<String>,
    /// The remote pictures of the selected message: `once`, or `always` for its sender (drawn by the sample, never
    /// fetched).
    #[serde(default)]
    pub pictures: Option<String>,
    /// Whether the reader fills the window.
    #[serde(default)]
    pub full_width: Option<bool>,
    /// Whether the selected message shows its full header.
    #[serde(default)]
    pub details: Option<bool>,
    /// Whether the selected message shows its quotes.
    #[serde(default)]
    pub quotes: Option<bool>,
    /// The add-account page: `form`, `detected`, `checking`, `passed` or `failed`.
    #[serde(default)]
    pub onboard: Option<String>,
    /// Whether the appearance settings page is open.
    #[serde(default)]
    pub settings: Option<bool>,
    /// Whether the settings sheet is open on Accounts.
    #[serde(default)]
    pub accounts: Option<bool>,
    /// Whether the settings sheet is open on Shortcuts.
    #[serde(default)]
    pub shortcuts: Option<bool>,
    /// Whether the settings sheet is open on Sending, the first account signed.
    #[serde(default)]
    pub sending: Option<bool>,
    /// Whether the settings sheet is open on Triage.
    #[serde(default)]
    pub triage: Option<bool>,
    /// Whether the settings sheet is open on Notifications.
    #[serde(default)]
    pub notifications: Option<bool>,
    /// How the unified inbox shows its groups: `cards` when left out, `list` or `simple`.
    #[serde(default)]
    pub view: Option<String>,
    /// The composer, the first account signed: `new`, `written` (two people, a subject and a few lines), `bad`
    /// (written, with a recipient that is no address), `suggest` (one person, and a name half typed with the people it
    /// could be under it), `floating` (written, then a message opened), `reply` (a reply with a line written) or
    /// `template` (two people and a template whose blank is left).
    #[serde(default)]
    pub compose: Option<String>,
    /// The action waiting for a key on the Shortcuts page.
    #[serde(default)]
    pub capture: Option<String>,
    /// The text typed into the command palette, which opens it.
    #[serde(default)]
    pub palette: Option<String>,
    /// Whether the calendar screen shows.
    #[serde(default)]
    pub calendar: Option<bool>,
    /// The calendar's view (`day`, `week`, `month` or `agenda`), which shows the calendar screen.
    #[serde(default)]
    pub calendar_view: Option<String>,
    /// The summary of the calendar event picked, which shows the calendar screen and the event's details.
    #[serde(default)]
    pub event: Option<String>,
    /// The event editor on a `new` event or, with `edit`, on the picked one; shows the calendar screen.
    #[serde(default)]
    pub edit_event: Option<String>,
    /// Whether the picked event is deleted, so its toast shows.
    #[serde(default)]
    pub delete_event: Option<bool>,
    /// Whether the window has no account yet (and can add one) instead of the sample's.
    #[serde(default)]
    pub first: Option<bool>,
    /// Whether the sample's inbox holds Odile's two invitations to lunch (`sample::invitations`).
    #[serde(default)]
    pub invitations: Option<bool>,
    /// The invented screener, groups and hover actions in the list, with keys for their actions.
    #[serde(default)]
    pub showcase: Option<bool>,
    /// How many rows down the keyboard selects.
    #[serde(default)]
    pub step: Option<u8>,
    /// An act on the selected row: `read`, `unread`, `flag`, `unflag`, `done`, `archive` or `delete`; for its sender
    /// `accept` (into Updates), `accept_open`, `block` or `newsletters` (their group); the toast shows after it.
    #[serde(default)]
    pub act: Option<String>,
    /// Whether `act` (`done` or `delete`) is caught with its row sliding out, before the toast.
    #[serde(default)]
    pub slide: Option<bool>,
    /// Every icon of the set in every style instead of the window.
    #[serde(default)]
    pub icons: Option<bool>,
    /// The gallery of the components instead of the window.
    #[serde(default)]
    pub components: Option<bool>,
    /// The typeface: `figtree` when left out, `inter`.
    #[serde(default)]
    pub font: Option<String>,
    /// The icon set: `phosphor` when left out, `lucide`.
    #[serde(default)]
    pub icon_set: Option<String>,
    /// The app's own window frame, as a native window draws it: `strip` or `top_row`; none when left out.
    #[serde(default)]
    pub frame: Option<String>,
    /// Where the pointer rests, in points, so what lies under it shows its hover; nowhere when left out.
    #[serde(default)]
    pub pointer: Option<[f32; 2]>,
    /// Some of the sample's mail snoozed, pinned, parked, done and back from a snooze first ([`sample::put_away`]).
    #[serde(default)]
    pub put_away: Option<bool>,
    /// The pile of "Later" to open: `snoozed`, `pins` or `parked`.
    #[serde(default)]
    pub later: Option<String>,
    /// Whether "Show done" lists the done mail in its old place.
    #[serde(default)]
    pub show_done: Option<bool>,
    /// The time picker open over the selected message, with this text typed (`""` for none).
    #[serde(default)]
    pub picker: Option<String>,
    /// The sending sample: `waiting` (the outbox), `countdown` (a send's toast), `schedule` (the composer with a time
    /// picked) or `bounce` (the bounced message open).
    #[serde(default)]
    pub outbox: Option<String>,
}

/// The credential store of a picture: it takes the password and keeps nothing.
struct Pictured;

impl Keychain for Pictured {
    fn store(&self, _account: u64, _password: &str) -> Result<(), String> {
        Ok(())
    }

    fn forget(&self, _account: u64) -> Result<(), String> {
        Ok(())
    }
}

/// What Detect proposes in a picture: the invented provider's autoconfig file.
fn pictured_proposal() -> Proposal {
    let suggestion = |host: &str, port, security| Suggestion {
        host: host.to_owned(),
        port,
        security,
        username: "robin@example.org".to_owned(),
        source: Source::Autoconfig,
    };
    Proposal {
        imap: vec![suggestion("imap.example.org", 993, Security::Tls), suggestion("imap.example.org", 143, Security::StartTls)],
        asked: vec!["dns MX example.org".to_owned(), "https://autoconfig.example.org/mail/config-v1.1.xml".to_owned()],
        failed: Vec::new(),
    }
}

/// Puts the sample window on the add-account page in `state`.
fn onboard(app: InboxApp, out: &str, state: &str) -> Result<InboxApp, UiError> {
    let passed = |step| (step, StepStatus::Passed);
    let workers = match state {
        "form" | "detected" | "passed" => Scripted::passing(),
        "checking" => Scripted::answering(&[passed(CheckStep::Dns), passed(CheckStep::Tcp), passed(CheckStep::Tls)]).unfinished(),
        "failed" => Scripted::answering(&[
            passed(CheckStep::Dns),
            passed(CheckStep::Tcp),
            passed(CheckStep::Tls),
            (CheckStep::Certificate, StepStatus::Failed(ErrorKind::CertificateExpired)),
        ]),
        other => return Err(bad(format!("{out}: onboard {other:?} is not form, detected, checking, passed or failed"))),
    };
    let mut app = app.with_accounts(Arc::new(Pictured), Box::new(Proposed::new(pictured_proposal())), Box::new(workers));
    let (onboarding, mailbox) = app.onboarding_mut();
    onboarding.open();
    let form = onboarding.form_mut();
    form.address = "robin@example.org".to_owned();
    form.display_name = "Robin Example".to_owned();
    form.password = Secret::new("invented-pass");
    if state == "form" {
        form.host = "mail.example.org".to_owned();
        form.security = Some(Security::StartTls);
        return Ok(app);
    }
    onboarding.detect();
    onboarding.poll();
    if state != "detected" {
        onboarding.add(mailbox);
        app.pump();
    }
    Ok(app)
}

/// Does the hover action `shot` names to the selected row: at once, so the toast shows, or caught sliding out with
/// motion on (four frames a sixtieth of a second apart, so the row is most of the way out).
fn act(app: &mut InboxApp, shot: &Shot) -> Result<(), UiError> {
    let Some(name) = &shot.act else {
        return if shot.slide.is_some() { Err(bad(format!("{}: slide needs an act", shot.out))) } else { Ok(()) };
    };
    let act = match name.as_str() {
        "read" => Act::Read,
        "unread" => Act::Unread,
        "flag" => Act::Flag,
        "unflag" => Act::Unflag,
        "done" => Act::Done,
        "archive" => Act::Archive,
        "delete" => Act::Delete,
        "accept" => Act::Accept { group: Some(GroupKind::Updates), open: false },
        "accept_open" => Act::Accept { group: None, open: true },
        "block" => Act::Block,
        "newsletters" => Act::Group(Some(GroupKind::Newsletters)),
        other => {
            return Err(bad(format!(
                "{}: act {other:?} is not read, unread, flag, unflag, done, archive, delete, accept, accept_open, block or newsletters",
                shot.out
            )));
        }
    };
    let Some(id) = app.mailbox().selected() else { return Err(bad(format!("{}: an act needs a selected row", shot.out))) };
    if shot.slide == Some(true) {
        if !act.slides() {
            return Err(bad(format!("{}: only done, archive and delete slide out", shot.out)));
        }
        app.set_reduce_motion(false);
        app.mailbox_mut().start(id, act);
    } else {
        app.mailbox_mut().act(id, act);
    }
    Ok(())
}

/// Puts the calendar screen in the view of `shot` and picks its event.
fn calendar(app: &mut InboxApp, shot: &Shot) -> Result<(), UiError> {
    let (calendar, mailbox) = app.calendar_mut();
    if let Some(name) = &shot.calendar_view {
        let view = View::named(name).ok_or_else(|| bad(format!("{}: calendar view {name:?} is not day, week, month or agenda", shot.out)))?;
        calendar.set_view(mailbox, view);
    }
    if let Some(summary) = &shot.event {
        let key = calendar.events().find(|event| event.summary == *summary).map(EventRow::key);
        let Some(key) = key else { return Err(bad(format!("{}: no calendar event {summary:?} in the view", shot.out))) };
        calendar.pick(Some(key));
    }
    let picked = |calendar: &crate::calendar::Calendar, what: &str| {
        calendar.picked().map(|_| ()).ok_or_else(|| bad(format!("{}: {what} needs an event picked with \"event\"", shot.out)))
    };
    match shot.edit_event.as_deref() {
        None => {}
        Some("new") => calendar.new_event(),
        Some("edit") => {
            picked(calendar, "edit_event")?;
            calendar.edit_picked();
        }
        Some(other) => return Err(bad(format!("{}: edit_event {other:?} is not new or edit", shot.out))),
    }
    if shot.delete_event == Some(true) {
        picked(calendar, "delete_event")?;
        calendar.delete_picked(mailbox);
    }
    Ok(())
}

/// Shows the places of `shot` beyond the mail: the calendar, the Shortcuts page with an action waiting for a key, the
/// command palette with its text.
fn places(app: &mut InboxApp, shot: &Shot) -> Result<(), UiError> {
    if let Some(scene) = &shot.compose {
        compose(app, &shot.out, scene)?;
    }
    if shot.calendar == Some(true) || shot.calendar_view.is_some() || shot.event.is_some() || shot.edit_event.is_some() || shot.delete_event.is_some()
    {
        app.show_calendar();
        calendar(app, shot)?;
    }
    if shot.triage == Some(true) {
        let appearance = *app.appearance();
        app.settings_mut().open_at(crate::settings::Section::Triage, &appearance);
    }
    if shot.notifications == Some(true) {
        app.mailbox_mut().set_notify_level(2, inboxcraft_engine::notify::Level::People);
        app.set_desk(Box::new(PicturedDesk));
        let appearance = *app.appearance();
        app.settings_mut().open_at(crate::settings::Section::Notifications, &appearance);
    }
    if shot.sending == Some(true) {
        sign(app);
        let appearance = *app.appearance();
        app.settings_mut().open_at(crate::settings::Section::Sending, &appearance);
    }
    if shot.shortcuts == Some(true) {
        app.open_shortcuts();
    }
    if let Some(waiting) = &shot.capture {
        let Some(action) = crate::keymap::action(waiting).filter(|_| shot.shortcuts == Some(true)) else {
            return Err(bad(format!("{}: capture {waiting:?} is not an action on the Shortcuts page", shot.out)));
        };
        app.settings_mut().capture(action.id);
    }
    if let Some(text) = &shot.palette {
        let palette = app.palette_mut();
        palette.open();
        palette.set_query(text);
    }
    Ok(())
}

/// The desk of the pictures: InboxCraft runs in the background and starts with the computer, and a change changes
/// nothing.
#[derive(Debug, Clone, Copy, Default)]
pub struct PicturedDesk;

impl crate::desk::Desk for PicturedDesk {
    fn background(&self) -> Option<bool> {
        Some(true)
    }

    fn set_background(&mut self, _on: bool) -> Result<(), String> {
        Ok(())
    }

    fn login(&self) -> Option<bool> {
        Some(true)
    }

    fn asking(&self) -> bool {
        false
    }

    fn set_login(&mut self, _on: bool) {}

    fn answer(&mut self) -> Option<Result<(), String>> {
        None
    }
}

/// Signs the first account's messages with an invented signature.
fn sign(app: &mut InboxApp) {
    // The sample takes every signature; a refusal would only leave the picture without one.
    let _ = app.mailbox_mut().run("compose.signature", &serde_json::json!({ "account": 1, "signature": "Mira\nPlot 14" }));
    app.mailbox_mut().refresh();
}

/// Opens the composer as `scene` asks.
fn compose(app: &mut InboxApp, out: &str, scene: &str) -> Result<(), UiError> {
    if !["new", "written", "bad", "suggest", "floating", "reply", "template"].contains(&scene) {
        return Err(bad(format!("{out}: compose {scene:?} is not new, written, bad, suggest, floating, reply or template")));
    }
    sign(app);
    if scene == "reply" {
        // Not selected first: a message selected before the first frame counts as pressed and floats the composer.
        let id = app.mailbox().messages().iter().find(|row| row.subject == "Allotment rota for November").map(|row| row.id);
        if id.is_none_or(|id| !app.answer(id, crate::compose::Answer::Reply)) {
            return Err(bad(format!("{out}: the reply did not open")));
        }
        let (Some(composer), mailbox) = app.composer_mut() else { return Err(bad(format!("{out}: the composer did not open"))) };
        let text = format!("The second week works for me. I can bring the hose.{}", composer.text());
        *composer.text_mut() = text;
        composer.edited();
        return composer.save(mailbox);
    }
    app.compose();
    let (Some(composer), mailbox) = app.composer_mut() else { return Err(bad(format!("{out}: the composer did not open"))) };
    let mut to = |text: &str| -> Result<(), UiError> {
        if let Some(field) = composer.typing_mut(Line::To) {
            text.clone_into(field);
        }
        composer.typed(mailbox, Line::To)
    };
    match scene {
        "written" | "bad" | "floating" | "template" => to("Odile Brannock <odile@example.org>, Teodor Lisk <teodor@example.org>,")?,
        "suggest" => to("Odile Brannock <odile@example.org>, te")?,
        _ => {}
    }
    if scene == "bad" {
        to("ansel.example.org,")?;
    }
    if scene == "template" {
        let params = serde_json::json!({
            "account": composer.account(),
            "name": "Work party",
            "subject": "Work party on Saturday",
            "text": "Hi both,\n\nWe meet at the plot at {?time} and finish by noon. Bring gloves; the shed has spades and the wheelbarrow.\n",
        });
        mailbox.run("template.save", &params)?;
        return composer.use_template(mailbox, "Work party");
    }
    if scene != "new" {
        "Work party on Saturday".clone_into(composer.subject_mut());
        let text = format!(
            "Hi both,\n\nWe meet at the plot at ten and finish by noon. Bring gloves; the shed has spades and the wheelbarrow.{}",
            composer.text()
        );
        *composer.text_mut() = text;
        composer.edited();
        composer.save(mailbox)?;
    }
    if scene == "suggest" {
        composer.step(true);
    }
    if scene == "floating" {
        let id = app.mailbox().messages().iter().find(|row| row.subject == "Draft logo for the bakery").map(|row| row.id);
        app.mailbox_mut().select(id);
    }
    Ok(())
}

/// The reply the server refuses the outbox's picture send with.
const REFUSAL: &str = "550 5.1.1 hollis@example.org: no such mailbox here";

/// Writes a message to `to` with `subject` and a line of text in the composer, and sends it: at `later`, or after the
/// hold.
fn send(app: &mut InboxApp, out: &str, to: &str, subject: &str, later: Option<Timestamp>) -> Result<(), UiError> {
    app.compose();
    let (Some(composer), mailbox) = app.composer_mut() else { return Err(bad(format!("{out}: the composer did not open"))) };
    if let Some(field) = composer.typing_mut(Line::To) {
        to.clone_into(field);
    }
    composer.typed(mailbox, Line::To)?;
    subject.clone_into(composer.subject_mut());
    composer.edited();
    composer.set_later(later);
    app.send_composer();
    if app.composer().is_some() {
        return Err(bad(format!("{out}: {subject:?} was not sent: {}", app.mailbox().error().unwrap_or("no reason"))));
    }
    Ok(())
}

/// Tomorrow morning, 08:00 in UTC, from the picture's time.
fn tomorrow_morning() -> Result<Timestamp, UiError> {
    Timestamp::from_unix(SHOT_TIME + 23 * 3_600, 0).ok_or_else(|| bad("no time"))
}

/// Puts the sending sample in `scene`, on `clock`.
fn outbox(app: &mut InboxApp, clock: &FakeClock, out: &str, scene: &str) -> Result<(), UiError> {
    match scene {
        "waiting" => {
            send(app, out, "Hollis Fenwick <hollis@example.org>,", "Plot 14 handover", None)?;
            clock.advance(std::time::Duration::from_secs(6));
            app.mailbox_mut().pump(&mut Scripted::default().refusing(REFUSAL));
            send(app, out, "Petra Harbour <petra@example.org>,", "Re: Draft logo for the bakery", Some(tomorrow_morning()?))?;
            send(app, out, "Odile Brannock <odile@example.org>, Teodor Lisk <teodor@example.org>,", "Work party on Saturday", None)?;
            clock.advance(std::time::Duration::from_secs(1));
            app.mailbox_mut().open(Place::Outbox);
        }
        "countdown" => {
            send(app, out, "Odile Brannock <odile@example.org>, Teodor Lisk <teodor@example.org>,", "Work party on Saturday", None)?;
            clock.advance(std::time::Duration::from_secs(1));
        }
        "schedule" => {
            compose(app, out, "written")?;
            let (Some(composer), _) = app.composer_mut() else { return Err(bad(format!("{out}: the composer did not open"))) };
            composer.set_later(Some(tomorrow_morning()?));
            app.open_send_later();
        }
        "bounce" => {
            app.mailbox_mut().open(Place::View(Role::Sent));
            let id = app.mailbox().messages().iter().find(|row| row.bounced.is_some()).map(|row| row.id);
            let Some(id) = id else { return Err(bad(format!("{out}: no bounced message in Sent"))) };
            app.mailbox_mut().select(Some(id));
        }
        other => return Err(bad(format!("{out}: outbox {other:?} is not waiting, countdown, schedule or bounce"))),
    }
    Ok(())
}

/// Refuses the window settings for a page of `shot` that shows no window.
fn no_window(shot: &Shot, page: &str) -> Result<(), UiError> {
    let window = shot.narrow.is_some()
        || shot.open.is_some()
        || shot.search.is_some()
        || shot.select.is_some()
        || shot.onboard.is_some()
        || shot.settings.is_some()
        || shot.accounts.is_some()
        || shot.shortcuts.is_some()
        || shot.sending.is_some()
        || shot.triage.is_some()
        || shot.notifications.is_some()
        || shot.view.is_some()
        || shot.compose.is_some()
        || shot.capture.is_some()
        || shot.palette.is_some()
        || shot.calendar.is_some()
        || shot.calendar_view.is_some()
        || shot.event.is_some()
        || shot.edit_event.is_some()
        || shot.delete_event.is_some()
        || shot.first.is_some()
        || shot.showcase.is_some()
        || shot.step.is_some()
        || shot.act.is_some()
        || shot.slide.is_some()
        || shot.text_scale.is_some()
        || shot.size.is_some()
        || shot.frame.is_some()
        || shot.put_away.is_some()
        || shot.later.is_some()
        || shot.show_done.is_some()
        || shot.picker.is_some()
        || shot.outbox.is_some()
        || (shot.icons == Some(true) && shot.components == Some(true));
    if window {
        return Err(bad(format!("{}: {page} has no window to narrow, open, search, select, scale or size", shot.out)));
    }
    Ok(())
}

/// The gallery of the components of `shot` in `mode` and `look`.
fn component_gallery(shot: &Shot, mode: Mode, look: &Look) -> Result<Picture, UiError> {
    no_window(shot, "the component gallery")?;
    let theme = Theme::of(mode, look);
    let keymap = gallery::keymap();
    let mut fonts = true;
    paint(gallery::size(), shot.scale.unwrap_or(1.0), mode, |ui| {
        if std::mem::take(&mut fonts) {
            theme::install_fonts(ui.ctx());
        }
        gallery::page(ui, &theme, &keymap);
    })
}

/// The icon sheet of `shot` in `mode` and `look`; it shows no window, so the window settings are refused.
fn icon_sheet(shot: &Shot, mode: Mode, look: &Look) -> Result<Picture, UiError> {
    no_window(shot, "an icon sheet")?;
    let theme = Theme::of(mode, look);
    let mut fonts = true;
    paint(iconset::sheet_size(), shot.scale.unwrap_or(1.0), mode, |ui| {
        if std::mem::take(&mut fonts) {
            theme::install_fonts(ui.ctx());
        }
        iconset::sheet(ui.painter(), &theme);
    })
}

fn bad(detail: impl Into<String>) -> UiError {
    UiError::Shot(detail.into())
}

/// The pictures a script asks for. Blank lines and lines starting with `#` are skipped.
///
/// # Errors
///
/// [`UiError::Shot`] naming the first line that is not a picture.
pub fn parse_script(text: &str) -> Result<Vec<Shot>, UiError> {
    let mut shots = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let shot: Shot = serde_json::from_str(line).map_err(|error| bad(format!("line {}: {error}", index.saturating_add(1))))?;
        shots.push(shot);
    }
    Ok(shots)
}

/// An RGBA picture, eight bits a channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    width: usize,
    height: usize,
    rgba: Vec<u8>,
}

impl Picture {
    /// Its width in pixels.
    #[must_use]
    pub fn width(&self) -> usize {
        self.width
    }

    /// Its height in pixels.
    #[must_use]
    pub fn height(&self) -> usize {
        self.height
    }

    /// The colour at `x`, `y`, or `None` outside.
    #[must_use]
    pub fn pixel(&self, x: usize, y: usize) -> Option<Color32> {
        if x >= self.width {
            return None;
        }
        let at = y.checked_mul(self.width)?.checked_add(x)?.checked_mul(4)?;
        let px = self.rgba.get(at..at.checked_add(4)?)?;
        Some(Color32::from_rgba_premultiplied(*px.first()?, *px.get(1)?, *px.get(2)?, *px.get(3)?))
    }

    /// Writes it as PNG.
    ///
    /// # Errors
    ///
    /// [`UiError::Shot`] when the writer fails.
    pub fn write_png(&self, out: &mut dyn Write) -> Result<(), UiError> {
        let width = u32::try_from(self.width).map_err(|_| bad("too wide"))?;
        let height = u32::try_from(self.height).map_err(|_| bad("too high"))?;
        let mut encoder = png::Encoder::new(out, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|error| bad(error.to_string()))?;
        writer.write_image_data(&self.rgba).map_err(|error| bad(error.to_string()))?;
        writer.finish().map_err(|error| bad(error.to_string()))
    }
}

/// A texture egui asked for: premultiplied colours.
struct Texture {
    width: usize,
    height: usize,
    pixels: Vec<Color32>,
}

impl Texture {
    fn texel(&self, x: usize, y: usize) -> [f32; 4] {
        let x = x.min(self.width.saturating_sub(1));
        let y = y.min(self.height.saturating_sub(1));
        let colour = y.checked_mul(self.width).and_then(|row| row.checked_add(x)).and_then(|at| self.pixels.get(at)).copied().unwrap_or_default();
        unit(colour)
    }

    /// The colour at `uv` (0 to 1 across), filtered between the four nearest texels.
    fn sample(&self, uv: Pos2) -> [f32; 4] {
        let x = uv.x * self.width as f32 - 0.5;
        let y = uv.y * self.height as f32 - 0.5;
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let index = |value: f32| if value <= 0.0 { 0 } else { value as usize };
        let (ix, iy) = (index(x0), index(y0));
        let (jx, jy) = (if x0 < 0.0 { 0 } else { ix.saturating_add(1) }, if y0 < 0.0 { 0 } else { iy.saturating_add(1) });
        let (a, b, c, d) = (self.texel(ix, iy), self.texel(jx, iy), self.texel(ix, jy), self.texel(jx, jy));
        std::array::from_fn(|i| {
            let top = a[i] + (b[i] - a[i]) * fx;
            let bottom = c[i] + (d[i] - c[i]) * fx;
            top + (bottom - top) * fy
        })
    }
}

fn unit(colour: Color32) -> [f32; 4] {
    colour.to_array().map(|channel| f32::from(channel) / 255.0)
}

/// The canvas the triangles are filled into: premultiplied colours, 0 to 1.
struct Canvas {
    width: usize,
    height: usize,
    pixels: Vec<[f32; 4]>,
}

/// Twice the signed area of the triangle `a`, `b`, `p`: positive when `p` is on the inner side of `a` to `b` in a
/// clockwise triangle on a screen whose y grows downwards.
fn edge(a: Pos2, b: Pos2, p: Pos2) -> f32 {
    (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x)
}

/// Whether the edge `a` to `b` of a clockwise triangle is a top or a left edge, which owns the pixels exactly on it,
/// so two triangles that share an edge never both fill a pixel.
fn top_left(a: Pos2, b: Pos2) -> bool {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    dy < 0.0 || (dy == 0.0 && dx > 0.0)
}

fn covers(weight: f32, a: Pos2, b: Pos2) -> bool {
    weight > 0.0 || (weight == 0.0 && top_left(a, b))
}

impl Canvas {
    fn new(width: usize, height: usize) -> Self {
        Self { width, height, pixels: vec![[0.0; 4]; width.saturating_mul(height)] }
    }

    fn blend(&mut self, x: usize, y: usize, src: [f32; 4]) {
        if let Some(dst) = y.checked_mul(self.width).and_then(|row| row.checked_add(x)).and_then(|at| self.pixels.get_mut(at)) {
            let keep = 1.0 - src[3];
            *dst = std::array::from_fn(|i| src[i] + dst[i] * keep);
        }
    }

    /// Fills `mesh` (positions in points) inside `clip` (in pixels), scaled by `scale`.
    fn mesh(&mut self, mesh: &Mesh, texture: &Texture, clip: [usize; 4], scale: f32) {
        let (triangles, _) = mesh.indices.as_chunks::<3>();
        for [a, b, c] in triangles {
            let vertex = |at: &u32| usize::try_from(*at).ok().and_then(|i| mesh.vertices.get(i));
            if let (Some(a), Some(b), Some(c)) = (vertex(a), vertex(b), vertex(c)) {
                self.triangle([a, b, c], texture, clip, scale);
            }
        }
    }

    fn triangle(&mut self, vertices: [&egui::epaint::Vertex; 3], texture: &Texture, clip: [usize; 4], scale: f32) {
        let [a, mut b, mut c] = vertices;
        let mut area = edge(a.pos * scale, b.pos * scale, c.pos * scale);
        if area < 0.0 {
            std::mem::swap(&mut b, &mut c);
            area = -area;
        }
        if area <= 0.0 || !area.is_finite() {
            return;
        }
        let (pa, pb, pc) = (a.pos * scale, b.pos * scale, c.pos * scale);
        let [left, top, right, bottom] = clip;
        let first = |value: f32, low: usize| if value.is_finite() && value > low as f32 { value.floor() as usize } else { low };
        let last = |value: f32, high: usize| if value.is_finite() && value < high as f32 { value.ceil() as usize } else { high };
        let (x0, x1) = (first(pa.x.min(pb.x).min(pc.x), left), last(pa.x.max(pb.x).max(pc.x), right));
        let (y0, y1) = (first(pa.y.min(pb.y).min(pc.y), top), last(pa.y.max(pb.y).max(pc.y), bottom));
        let (ca, cb, cc) = (unit(a.color), unit(b.color), unit(c.color));
        for y in y0..y1 {
            for x in x0..x1 {
                let p = pos2(x as f32 + 0.5, y as f32 + 0.5);
                let (wa, wb, wc) = (edge(pb, pc, p), edge(pc, pa, p), edge(pa, pb, p));
                if !(covers(wa, pb, pc) && covers(wb, pc, pa) && covers(wc, pa, pb)) {
                    continue;
                }
                let (wa, wb, wc) = (wa / area, wb / area, wc / area);
                let uv = pos2(a.uv.x * wa + b.uv.x * wb + c.uv.x * wc, a.uv.y * wa + b.uv.y * wb + c.uv.y * wc);
                let texel = texture.sample(uv);
                let src = std::array::from_fn(|i| (ca[i] * wa + cb[i] * wb + cc[i] * wc) * texel[i]);
                self.blend(x, y, src);
            }
        }
    }

    fn picture(&self) -> Picture {
        let rgba = self.pixels.iter().flat_map(|pixel| pixel.map(|channel| (channel.clamp(0.0, 1.0) * 255.0).round() as u8)).collect();
        Picture { width: self.width, height: self.height, rgba }
    }
}

/// Fills `primitives` into a picture of `width` by `height` pixels with the `textures` egui set.
fn fill(primitives: &[ClippedPrimitive], textures: &HashMap<TextureId, Texture>, width: usize, height: usize, scale: f32) -> Picture {
    let mut canvas = Canvas::new(width, height);
    for primitive in primitives {
        let Primitive::Mesh(mesh) = &primitive.primitive else { continue };
        let Some(texture) = textures.get(&mesh.texture_id) else { continue };
        let rect = primitive.clip_rect;
        // Infinite edges (an unclipped primitive) stop at the picture's edge; the casts see 0 to `high` only.
        let px = |value: f32, high: usize| if value > 0.0 { (value * scale).round().min(high as f32) as usize } else { 0 };
        let clip = [px(rect.min.x, width), px(rect.min.y, height), px(rect.max.x, width), px(rect.max.y, height)];
        canvas.mesh(mesh, texture, clip, scale);
    }
    canvas.picture()
}

/// Applies and empties `delta` (egui insists every delta is handled).
fn apply_textures(textures: &mut HashMap<TextureId, Texture>, delta: &mut egui::TexturesDelta) {
    for (id, patches) in delta.set.drain() {
        for patch in patches {
            let ImageData::Color(image) = &patch.image;
            let [width, height] = image.size;
            match patch.pos {
                None => {
                    textures.insert(id, Texture { width, height, pixels: image.pixels.clone() });
                }
                Some([x, y]) => {
                    let Some(texture) = textures.get_mut(&id) else { continue };
                    for row in 0..height {
                        for column in 0..width {
                            let source = row.checked_mul(width).and_then(|r| r.checked_add(column)).and_then(|at| image.pixels.get(at));
                            let target = y
                                .checked_add(row)
                                .filter(|ty| *ty < texture.height)
                                .zip(x.checked_add(column).filter(|tx| *tx < texture.width))
                                .and_then(|(ty, tx)| ty.checked_mul(texture.width).and_then(|r| r.checked_add(tx)));
                            if let (Some(source), Some(target)) = (source, target.and_then(|at| texture.pixels.get_mut(at))) {
                                *target = *source;
                            }
                        }
                    }
                }
            }
        }
    }
    for id in delta.free.drain() {
        textures.remove(&id);
    }
}

/// Draws `app` for a few frames in a window of `size` points at `scale` pixels per point in `mode`, and fills the
/// last frame into a picture.
///
/// # Errors
///
/// [`UiError::Shot`] for a size of nothing, a scale outside 1 to 4, or a picture side over [`MAX_SIDE`].
pub fn render(app: &mut InboxApp, size: Vec2, scale: f32, mode: Mode) -> Result<Picture, UiError> {
    paint(size, scale, mode, |ui| app.ui(ui))
}

/// [`render`] with the pointer resting at `pointer`, so what lies under it shows its hover.
///
/// # Errors
///
/// The errors of [`render`].
pub fn render_pointed(app: &mut InboxApp, size: Vec2, scale: f32, mode: Mode, pointer: Option<Pos2>) -> Result<Picture, UiError> {
    paint_pointed(size, scale, mode, pointer, |ui| app.ui(ui))
}

/// Runs `draw` for a few frames on a transparent window of `size` points at `scale` pixels per point in `mode`, and
/// fills the last frame into a picture.
///
/// # Errors
///
/// [`UiError::Shot`] for a size of nothing, a scale outside 1 to 4, or a picture side over [`MAX_SIDE`].
pub(crate) fn paint(size: Vec2, scale: f32, mode: Mode, draw: impl FnMut(&mut egui::Ui)) -> Result<Picture, UiError> {
    paint_pointed(size, scale, mode, None, draw)
}

/// [`paint`] with the pointer resting at `pointer`.
fn paint_pointed(size: Vec2, scale: f32, mode: Mode, pointer: Option<Pos2>, mut draw: impl FnMut(&mut egui::Ui)) -> Result<Picture, UiError> {
    if !(1.0..=4.0).contains(&scale) {
        return Err(bad(format!("scale {scale} is not between 1 and 4")));
    }
    let side = |points: f32| {
        let pixels = (points * scale).round();
        if pixels.is_finite() && pixels >= 1.0 && pixels <= MAX_SIDE as f32 { Some(pixels as usize) } else { None }
    };
    let (Some(width), Some(height)) = (side(size.x), side(size.y)) else {
        return Err(bad(format!("a window of {} by {} points at scale {scale} is not 1 to {MAX_SIDE} pixels a side", size.x, size.y)));
    };
    let ctx = egui::Context::default();
    ctx.set_pixels_per_point(scale);
    ctx.set_theme(match mode {
        Mode::Light => egui::Theme::Light,
        Mode::Dark => egui::Theme::Dark,
    });
    let mut textures = HashMap::new();
    let mut last = None;
    for _ in 0..4 {
        let events = pointer.map(egui::Event::PointerMoved).into_iter().collect();
        let input = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)), events, ..RawInput::default() };
        let mut output = ctx.run_ui(input, |ui| {
            motion::set_reduced(ui.ctx(), true);
            draw(ui);
        });
        apply_textures(&mut textures, &mut output.textures_delta);
        last = Some(output);
    }
    let Some(output) = last else { return Err(bad("no frame")) };
    let primitives = ctx.tessellate(output.shapes, output.pixels_per_point);
    Ok(fill(&primitives, &textures, width, height, output.pixels_per_point))
}

/// The sample window as `shot` asks for it.
///
/// # Errors
///
/// [`UiError::Shot`] for a mode, preset, role, row or size the shot names wrongly; the errors of [`render`].
pub fn shoot(shot: &Shot) -> Result<Picture, UiError> {
    let mode = match shot.mode.as_str() {
        "light" => Mode::Light,
        "dark" => Mode::Dark,
        other => return Err(bad(format!("{}: mode {other:?} is not light or dark", shot.out))),
    };
    let preset = match shot.preset.as_deref() {
        None => Preset::default(),
        Some(id) => *Preset::named(id).ok_or_else(|| bad(format!("{}: no preset {id:?}", shot.out)))?,
    };
    let typeface = match shot.font.as_deref() {
        None => Typeface::default(),
        Some(id) => Typeface::named(id).ok_or_else(|| bad(format!("{}: font {id:?} is not figtree or inter", shot.out)))?,
    };
    let icons = match shot.icon_set.as_deref() {
        None => Icons::default(),
        Some(id) => Icons::named(id).ok_or_else(|| bad(format!("{}: icon set {id:?} is not phosphor or lucide", shot.out)))?,
    };
    let look = Look { preset, typeface, icons: iconset::IconSet::of(icons), ..Look::default() };
    if shot.components == Some(true) {
        return component_gallery(shot, mode, &look);
    }
    if shot.icons == Some(true) {
        return icon_sheet(shot, mode, &look);
    }
    let time = Timestamp::from_unix(SHOT_TIME, 0).ok_or_else(|| bad("no time"))?;
    // One clock for the session and the list, so a scene can let time pass.
    let clock = FakeClock::new(time);
    let session = if shot.first == Some(true) {
        let store = inboxcraft_store::Memory::new(inboxcraft_store::StoreConfig::default());
        inboxcraft_engine::Session::new(Box::new(store), Box::new(clock.clone())).map_err(UiError::Engine)?
    } else if shot.outbox.is_some() {
        sample::sending(Box::new(clock.clone()))?
    } else if shot.invitations == Some(true) {
        let mut store = sample::mailbox(time)?;
        let batch = sample::invitations(time)?.into_iter().fold(inboxcraft_store::Batch::new(), inboxcraft_store::Batch::with);
        inboxcraft_store::Storage::apply(&mut store, &batch).map_err(UiError::sample)?;
        sample::session_over(store, Box::new(clock.clone()))?
    } else {
        sample::session(Box::new(clock.clone()))?
    };
    let mailbox = Mailbox::new(session, Box::new(clock.clone()));
    let mut app = InboxApp::with_outside(mailbox, Box::new(sample::Pictures::default()), Box::new(NoSave));
    if shot.first == Some(true) {
        app = app.with_accounts(Arc::new(Pictured), Box::new(Proposed::new(pictured_proposal())), Box::new(Scripted::passing()));
    }
    app.set_preset(preset);
    app.set_typeface(typeface);
    app.set_icons(icons);
    app.set_reduce_motion(true);
    if let Some(name) = &shot.frame {
        let frame = WindowFrame::named(name)
            .filter(|frame| *frame != WindowFrame::System)
            .ok_or_else(|| bad(format!("{}: frame {name} is not strip or top_row", shot.out)))?;
        app.set_frame(frame);
        app.set_own_frame(true);
    }
    if let Some(percent) = shot.text_scale {
        let scale =
            TextScale::from_percent(percent).ok_or_else(|| bad(format!("{}: text scale {percent} is not 90, 100, 110, 120 or 130", shot.out)))?;
        app.set_text_scale(scale);
    }
    if let Some(account) = shot.narrow {
        app.mailbox_mut().narrow(Some(account));
    }
    if let Some(name) = &shot.view {
        let view = InboxView::named(name).ok_or_else(|| bad(format!("{}: view {name:?} is not cards, list or simple", shot.out)))?;
        app.mailbox_mut().set_inbox_view(view);
    }
    if shot.put_away == Some(true) {
        sample::put_away(app.mailbox_mut())?;
    }
    if shot.show_done == Some(true) {
        app.mailbox_mut().set_show_done(true);
    }
    match &shot.open {
        Some(Open::Role(name)) if name == "senders" => app.mailbox_mut().open(Place::Senders),
        Some(Open::Role(name)) => {
            let role = [Role::Inbox, Role::Drafts, Role::Sent, Role::Archive, Role::Junk, Role::Trash]
                .into_iter()
                .find(|role| role.label().eq_ignore_ascii_case(name))
                .ok_or_else(|| bad(format!("{}: no role {name:?}", shot.out)))?;
            app.mailbox_mut().open(Place::View(role));
        }
        Some(Open::Folder(id)) => app.mailbox_mut().open(Place::Folder(*id)),
        None => {}
    }
    if let Some(name) = &shot.later {
        let pile = Pile::named(name).ok_or_else(|| bad(format!("{}: pile {name:?} is not snoozed, pins or parked", shot.out)))?;
        app.mailbox_mut().open(Place::Later(pile));
    }
    if let Some(text) = &shot.search {
        app.search(text);
    }
    if shot.showcase == Some(true) {
        let showcase = sample::showcase(app.mailbox());
        app.mailbox_mut().set_showcase(showcase);
        *app.keymap_mut() = gallery::keymap();
    }
    if let Some(steps) = shot.step {
        if shot.select.is_some() {
            return Err(bad(format!("{}: a shot selects by step or by subject, not both", shot.out)));
        }
        for _ in 0..steps {
            app.mailbox_mut().step(true);
        }
    }
    if let Some(subject) = &shot.select {
        let id = app.mailbox().messages().iter().find(|row| row.subject == *subject).map(|row| row.id);
        let Some(id) = id else { return Err(bad(format!("{}: no row with the subject {subject:?}", shot.out))) };
        app.mailbox_mut().select(Some(id));
        let (reader, mailbox) = app.reader_mut();
        match shot.pictures.as_deref() {
            Some("once") => reader.allow_once(mailbox, id),
            Some("always") => reader.allow_always(mailbox, id),
            Some(other) => return Err(bad(format!("{}: pictures {other:?} is not once or always", shot.out))),
            None => {}
        }
        reader.set_full_width(shot.full_width.unwrap_or(false));
        if shot.details == Some(true) {
            reader.toggle_details(id);
        }
        if shot.quotes == Some(true) {
            reader.toggle_quotes(id);
        }
    } else if shot.pictures.is_some() || shot.full_width.is_some() || shot.details.is_some() || shot.quotes.is_some() {
        return Err(bad(format!("{}: the reader settings need a selected row", shot.out)));
    }
    act(&mut app, shot)?;
    if let Some(typed) = &shot.picker {
        if app.mailbox().selected().is_none() {
            return Err(bad(format!("{}: the time picker needs a selected row", shot.out)));
        }
        app.open_picker(typed);
    }
    if let Some(state) = &shot.onboard {
        app = onboard(app, &shot.out, state)?;
    }
    if shot.settings == Some(true) {
        app.open_settings();
    }
    if shot.accounts == Some(true) {
        app.open_accounts();
    }
    places(&mut app, shot)?;
    if let Some(scene) = &shot.outbox {
        outbox(&mut app, &clock, &shot.out, scene)?;
    }
    let [width, height] = shot.size.unwrap_or([1200.0, 760.0]);
    let pointer = shot.pointer.map(|[x, y]| pos2(x, y));
    render_pointed(&mut app, vec2(width, height), shot.scale.unwrap_or(1.0), mode, pointer)
}

#[cfg(test)]
#[path = "shot_tests.rs"]
mod tests;
