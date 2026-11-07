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
//! Both answer the [`Request`]s that came in, hand the engine's jobs to the workers and apply what came back, and show
//! the unread count on the [`Badge`] (the tray) when it changed.

use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Duration;

use eframe::egui::{self, ViewportCommand};
use inboxcraft_model::Role;
use inboxcraft_ui_egui::InboxApp;
use inboxcraft_ui_egui::mailbox::Mailbox;

use crate::instance::Request;

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
    stays: bool,
    window: bool,
    quitting: bool,
    badge: Option<Box<dyn Badge>>,
    shown: Option<u64>,
}

impl std::fmt::Debug for Desktop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Desktop").field("stays", &self.stays).field("window", &self.window).field("quitting", &self.quitting).finish_non_exhaustive()
    }
}

impl Desktop {
    /// `app` in a window, running on when the window is closed, answering the requests that come through `heard`.
    #[must_use]
    pub fn new(app: InboxApp, heard: Receiver<Request>) -> Self {
        Self { app, heard, stays: true, window: true, quitting: false, badge: None, shown: None }
    }

    /// The same, but closing the window ends InboxCraft: for the sample, which has nothing to keep in step, and for a
    /// run nothing could bring a window back to.
    #[must_use]
    pub fn closing_ends(mut self) -> Self {
        self.stays = false;
        self
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
        if ctx.input(|input| input.viewport().close_requested()) {
            self.window = false;
            self.quitting |= !self.stays;
        }
        self.keep_in_step();
    }

    /// Without a window: waits up to `patience` for a request and answers it, then keeps the mail in step. With nobody
    /// left to send a request (no second start can reach this one and there is no tray), InboxCraft ends.
    pub fn wait(&mut self, patience: Duration) {
        match self.heard.recv_timeout(patience) {
            Ok(Request::Show) => self.window = true,
            Ok(Request::Quit) | Err(RecvTimeoutError::Disconnected) => self.quitting = true,
            Err(RecvTimeoutError::Timeout) => {}
        }
        self.keep_in_step();
    }

    /// Hands the jobs on, applies the events, and shows the unread count when it changed.
    fn keep_in_step(&mut self) {
        self.app.pump();
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
