// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The egui front end.
//!
//! Layer 6: the only layer that may depend on UI crates (see `docs/architecture.md`).
//!
//! - [`theme`]: InboxCraft's own colours, spacing and type sizes for light and dark (`docs/design.md`).
//! - [`i18n`] and [`tr!`]: every text the person reads goes through them.
//! - [`sample`]: the invented sample mailbox behind `--sample`, on `example.org`.
//! - [`mailbox`]: what the mail list shows, read from the engine's commands: the accounts, the unified views, the
//!   folders with their roles and unread counts, the list of a view, a folder or the search, narrowed to one account
//!   or not.
//! - [`InboxApp`]: the window's content over a [`mailbox::Mailbox`]: the folder pane, the header with the search field
//!   and the message list, drawn in the [`theme`], every control with an accessible name.
//! - [`picture`]: the PNG and JPEG pictures of a message decoded into RGBA, with their size checked first.
//! - [`quote`]: a plain-text body split into what the sender wrote and the quotes the reader folds.
//! - [`shot`]: headless pictures of the window for `ui_shot`, filled on the CPU and written as PNG.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod draw;
mod error;
pub mod fetch;
pub mod i18n;
pub mod mailbox;
pub mod picture;
pub mod quote;
pub mod sample;
pub mod shot;
pub mod theme;

pub use draw::InboxApp;
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
