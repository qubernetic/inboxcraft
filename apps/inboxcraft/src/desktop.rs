// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The window as the desktop sees it: closing it leaves InboxCraft running without one while the mail keeps coming in,
//! a second start or the tray opens it again, and `--quit` or the tray's Quit end InboxCraft.
//!
//! The window is really closed, not hidden: on Wayland a window cannot be hidden. `main` opens a new one on the same
//! [`InboxApp`] when [`Desktop::has_window`] says so again; eframe keeps its event loop between windows.
//!
//! `ui-egui` draws the window and knows nothing of this. While there is a window, [`Desktop::tick`] runs before every
//! frame (eframe's `App::logic`, which also runs when no frame is drawn); without one, `main` calls [`Desktop::wait`].
//! Both answer the [`Request`]s that came in, hand the engine's jobs to the workers and apply what came back, show the
//! unread count on the [`Badge`] (the tray) when it changed, and show what the engine has to tell the owner as
//! notifications ([`crate::notify`]). They also run the calls that came in over the control channel
//! ([`crate::control`]) for an agent, so only `safe` commands run; without a window a call waits for the next look,
//! at most [`WINDOWLESS_TICK`]. A click on one opens its message in the full-width reader, with the window
//! brought forward or opened again; its buttons mark the message done or read, as the window's own acts do.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Duration;

use eframe::egui::{self, ViewportCommand};
use inboxcraft_control_key::Failure;
use inboxcraft_model::Role;
use inboxcraft_ui_egui::InboxApp;
use inboxcraft_ui_egui::mailbox::{Act, Mailbox};

use crate::control::Call;
use crate::instance::Request;
use crate::notify::{Click, Deliver, shown};

/// How often InboxCraft without a window looks for the workers' events.
pub const WINDOWLESS_TICK: Duration = Duration::from_millis(500);

/// Where the unread count is shown outside the window: the tray.
pub trait Badge: Send {
    /// Shows `unread`.
    fn show(&mut self, unread: u64);
}

/// The window's content with what keeps it running without a window.
pub struct Desktop {
    app: InboxApp,
    heard: Receiver<Request>,
    /// Closing the window leaves InboxCraft running; shared with the owner's choice ([`crate::desk::OwnerDesk`]).
    stays: Arc<AtomicBool>,
    window: bool,
    quitting: bool,
    badge: Option<Box<dyn Badge>>,
    shown: Option<u64>,
    deliver: Option<Box<dyn Deliver>>,
    clicks: Option<Receiver<Click>>,
    calls: Option<Receiver<Call>>,
    /// A notification failed; the next failures are not reported again.
    failed: bool,
}

impl std::fmt::Debug for Desktop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Desktop")
            .field("stays", &self.stays.load(Ordering::SeqCst))
            .field("window", &self.window)
            .field("quitting", &self.quitting)
            .finish_non_exhaustive()
    }
}

impl Desktop {
    /// `app` in a window, running on when the window is closed, answering the requests that come through `heard`.
    #[must_use]
    pub fn new(app: InboxApp, heard: Receiver<Request>) -> Self {
        Self {
            app,
            heard,
            stays: Arc::new(AtomicBool::new(true)),
            window: true,
            quitting: false,
            badge: None,
            shown: None,
            deliver: None,
            clicks: None,
            calls: None,
            failed: false,
        }
    }

    /// The same, but closing the window ends InboxCraft: for the sample, which has nothing to keep in step, and for a
    /// run nothing could bring a window back to.
    #[must_use]
    pub fn closing_ends(self) -> Self {
        self.stays.store(false, Ordering::SeqCst);
        self
    }

    /// Whether closing the window leaves InboxCraft running, as a flag to share: what it says when the window closes
    /// decides.
    #[must_use]
    pub fn stays(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.stays)
    }

    /// The same, started without a window (`--background`).
    #[must_use]
    pub fn without_window(mut self) -> Self {
        self.window = false;
        self
    }

    /// The same, showing the unread count on `badge`.
    #[must_use]
    pub fn with_badge(mut self, badge: Box<dyn Badge>) -> Self {
        self.badge = Some(badge);
        self
    }

    /// The same, showing notifications through `deliver` and answering the clicks on them that come through `clicks`.
    #[must_use]
    pub fn with_notices(mut self, deliver: Box<dyn Deliver>, clicks: Receiver<Click>) -> Self {
        self.deliver = Some(deliver);
        self.clicks = Some(clicks);
        self
    }

    /// The same, running the calls that come in over the control channel through `calls`.
    #[must_use]
    pub fn with_control(mut self, calls: Receiver<Call>) -> Self {
        self.calls = Some(calls);
        self
    }

    /// The window's content.
    #[must_use]
    pub fn app(&self) -> &InboxApp {
        &self.app
    }

    /// The window's content, to change.
    pub fn app_mut(&mut self) -> &mut InboxApp {
        &mut self.app
    }

    /// Whether InboxCraft has, or should open, a window.
    #[must_use]
    pub fn has_window(&self) -> bool {
        self.window
    }

    /// Whether InboxCraft is ending.
    #[must_use]
    pub fn is_quitting(&self) -> bool {
        self.quitting
    }

    /// Runs before every frame of the window; see the module.
    pub fn tick(&mut self, ctx: &egui::Context) {
        for request in self.heard.try_iter() {
            match request {
                Request::Show => {
                    ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(ViewportCommand::Focus);
                }
                Request::Quit => {
                    self.quitting = true;
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                }
            }
        }
        if self.answer_clicks() {
            ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(ViewportCommand::Focus);
        }
        if ctx.input(|input| input.viewport().close_requested()) {
            self.window = false;
            self.quitting |= !self.stays.load(Ordering::SeqCst);
        }
        self.keep_in_step();
        self.app.wake_when_due(ctx);
    }

    /// Without a window: waits up to `patience` for a request and answers it, then keeps the mail in step. With nobody
    /// left to send a request (no second start can reach this one and there is no tray), InboxCraft ends.
    pub fn wait(&mut self, patience: Duration) {
        match self.heard.recv_timeout(patience) {
            Ok(Request::Show) => self.window = true,
            Ok(Request::Quit) | Err(RecvTimeoutError::Disconnected) => self.quitting = true,
            Err(RecvTimeoutError::Timeout) => {}
        }
        if self.answer_clicks() {
            self.window = true;
        }
        self.keep_in_step();
    }

    /// Answers the clicks on notifications; whether one asks for the window.
    fn answer_clicks(&mut self) -> bool {
        let clicks: Vec<Click> = self.clicks.as_ref().map(|clicks| clicks.try_iter().collect()).unwrap_or_default();
        let mut show = false;
        for click in clicks {
            match click {
                Click::Open(id) => {
                    let (reader, mailbox) = self.app.reader_mut();
                    reader.open(mailbox, Some(id));
                    reader.set_full_width(true);
                    show = true;
                }
                Click::Event(id, start) => {
                    if let Some(start) = inboxcraft_time::Timestamp::from_unix(start, 0) {
                        self.app.open_event(id, start);
                    }
                    show = true;
                }
                Click::Show => show = true,
                Click::Done(id) => self.app.mailbox_mut().act(id, Act::Done),
                Click::Read(id) => self.app.mailbox_mut().act(id, Act::Read),
            }
        }
        show
    }

    /// Hands the jobs on, applies the events, shows the unread count when it changed and the notices as
    /// notifications.
    fn keep_in_step(&mut self) {
        let calls: Vec<Call> = self.calls.as_ref().map(|calls| calls.try_iter().collect()).unwrap_or_default();
        for call in calls {
            let answer = self.app.mailbox_mut().run_for_agent(call.command(), call.params());
            call.answer(answer.map_err(|error| Failure::new(error.code(), error.to_string())));
        }
        self.app.pump();
        let notices = self.app.mailbox_mut().take_notices();
        if let Some(deliver) = self.deliver.as_mut() {
            for notice in &notices {
                let account = self.app.mailbox().account(notice.account().get()).map(|row| row.address.clone());
                if let Err(error) = deliver.deliver(&shown(notice, account.as_deref(), self.app.mailbox().offset()))
                    && !self.failed
                {
                    eprintln!("inboxcraft: {error}");
                    self.failed = true;
                }
            }
        }
        let count = unread(self.app.mailbox());
        if let Some(badge) = self.badge.as_mut()
            && self.shown != Some(count)
        {
            badge.show(count);
            self.shown = Some(count);
        }
    }
}

/// The unread messages in the inboxes of every account.
#[must_use]
pub fn unread(mailbox: &Mailbox) -> u64 {
    mailbox
        .accounts()
        .iter()
        .flat_map(|account| mailbox.folders_of(account.id))
        .filter(|folder| folder.role == Some(Role::Inbox))
        .fold(0_u64, |sum, folder| sum.saturating_add(folder.unread))
}

#[cfg(test)]
#[path = "desktop_tests.rs"]
mod tests;
