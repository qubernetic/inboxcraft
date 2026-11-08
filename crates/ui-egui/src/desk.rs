// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What the desktop app lets the owner choose about InboxCraft without its window, through a [`Desk`] the app hands
//! in ([`crate::InboxApp::with_desk`]): whether closing the window leaves InboxCraft running in the background, and
//! whether it starts with the computer. The Notifications page shows both; a window without a desk (the sample, the
//! tests, a run nothing could bring a window back to) shows them as not available.
//!
//! Starting at login may wait for the desktop, which can ask the person first (the Background portal in the
//! Flatpak): [`Desk::set_login`] only asks, and [`Desk::answer`] gives the answer once it came. The window looks for
//! it while the Notifications page is shown; the app wakes the window when it comes.

/// The app's side of running without a window.
pub trait Desk: Send {
    /// Whether closing the window leaves InboxCraft running; `None` where it cannot run without one.
    fn background(&self) -> Option<bool>;

    /// Closing the window leaves InboxCraft running from now on, or ends it, and the choice is kept.
    ///
    /// # Errors
    ///
    /// Why the choice could not be kept, as the owner reads it; the choice stays as it was.
    fn set_background(&mut self, on: bool) -> Result<(), String>;

    /// Whether InboxCraft starts at login, in the background; `None` where it cannot.
    fn login(&self) -> Option<bool>;

    /// Whether a [`Desk::set_login`] waits for its answer.
    fn asking(&self) -> bool;

    /// Asks to start InboxCraft at login, or no longer; the answer comes through [`Desk::answer`].
    fn set_login(&mut self, on: bool);

    /// The answer to the last [`Desk::set_login`], once, when it came: why it was not done in `Err`.
    fn answer(&mut self) -> Option<Result<(), String>>;
}
