// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The egui front end.
//!
//! Layer 6: the only layer that may depend on UI crates (see `docs/architecture.md`).
//!
//! - [`theme`]: InboxCraft's own colours, spacing and type sizes for light and dark (`docs/design.md`).
//! - [`gradient`]: the one place that paints a gradient, on shapes, lines and text, dithered against banding.
//! - [`glass`]: glass without blur: the colour fields behind the window, the panes' glass layers and the floats.
//! - [`i18n`] and [`tr!`]: every text the person reads goes through them.
//! - [`sample`]: the invented sample mailbox behind `--sample`, on `example.org`.
//! - [`mailbox`]: what the mail list shows, read from the engine's commands: the accounts, the unified views, the
//!   folders with their roles and unread counts, the list of a view, a folder or the search, narrowed to one account
//!   or not.
//! - [`InboxApp`]: the window's content over a [`mailbox::Mailbox`]: the folder pane, the header with the search field
//!   and the message list, drawn in the [`theme`], every control with an accessible name.
//! - [`onboard`]: adding an account: the form, Detect's proposal, the password into the OS credential store
//!   ([`onboard::Keychain`]) and nowhere else, and the account check's steps as they come.
//! - [`workers`]: the seam to the `net` workers: the engine's jobs out, their events in, each frame.
//! - [`picture`]: the PNG and JPEG pictures of a message decoded into RGBA, with their size checked first.
//! - [`quote`]: a plain-text body split into what the sender wrote and the quotes the reader folds.
//! - [`shot`]: headless pictures of the window for `ui_shot`, filled on the CPU and written as PNG.
//! - [`iconset`]: the icon painter: views ask for an [`iconset::IconId`], the set (Phosphor or Lucide) maps it to a
//!   glyph.
//! - [`settings`]: the appearance settings page's state: open or not, and the custom accent as the owner types it.
//! - [`keep`]: where the appearance choices (the `ui.*.set` commands) and the keys are kept between runs.
//! - [`desk`]: the app's side of running without a window: in the background, and starting with the computer.
//! - [`motion`]: the motion tokens and the one reduced-motion flag every animation reads.
//! - [`keymap`]: every action of the window and which key runs it; tooltips and chips read their keys from it.
//! - [`palette`]: the command palette's state and which actions match what is typed into it.
//! - [`calendar`]: the calendar screen's state, behind the rail's Calendar.
//! - [`component`]: the design system's components: card, icon button, pill, tooltip, avatar, badge, hover actions,
//!   toast and popup, and a gallery of them for `ui_shot`.
//! - [`icons`]: art drawn in code: the app icon, exported as SVG for the desktop and filled as the window icon.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod calendar;
pub mod component;
pub mod compose;
pub mod desk;
mod draw;
mod error;
pub mod fetch;
pub mod glass;
pub mod gradient;
pub mod i18n;
pub mod icons;
pub mod iconset;
pub mod invite;
pub mod keep;
pub mod keymap;
pub mod mailbox;
pub mod motion;
pub mod onboard;
pub mod palette;
pub mod picture;
pub mod quote;
pub mod reader;
pub mod sample;
pub mod save;
pub mod settings;
pub mod shot;
pub mod theme;
pub mod workers;

pub use draw::{InboxApp, Screen};
pub use error::UiError;

/// The crate's package name.
///
/// ```
/// assert_eq!(inboxcraft_ui_egui::name(), "inboxcraft-ui-egui");
/// ```
#[must_use]
pub fn name() -> &'static str {
    env!("CARGO_PKG_NAME")
}

#[cfg(test)]
mod tests;
