// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The egui front end.
//!
//! Layer 6: the only layer that may depend on UI crates (see `docs/architecture.md`).
//!
//! - [`theme`]: InboxCraft's own colours, spacing and type sizes for light and dark (`docs/design.md`).
//! - [`i18n`] and [`tr!`]: every text the person reads goes through them.
//! - [`sample`]: the invented sample mailbox behind `--sample`, on `example.org`.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod error;
pub mod i18n;
pub mod sample;
pub mod theme;

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
