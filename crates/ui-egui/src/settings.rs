// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The appearance settings page: whether it is open, the four colours of the custom accent as the owner types them,
//! and what the engine said about the last ones it took.
//!
//! Every other control of the page runs its `ui.*.set` command at once; the custom accent is a draft the page's
//! preview shows until "Use these colours" runs `ui.accent.set` with it. The colours come from the keyboard or the
//! colour picker, so they are read as hostile: only `#rrggbb` (the `#` may be left out, space around it is dropped)
//! is a colour, and a field holds at most [`MAX_CHARS`] characters.

use egui::Color32;
use inboxcraft_engine::appearance::{Accent, Appearance, Rgb};
use serde_json::Value;

use crate::theme::Preset;
use crate::tr;

/// The most characters a colour field takes.
pub const MAX_CHARS: usize = 16;

/// The settings page.
#[derive(Debug, Default)]
pub struct Settings {
    open: bool,
    colours: [String; 4],
    note: Option<String>,
}

/// `colour` as `#rrggbb`.
pub(crate) fn hex(colour: Color32) -> String {
    Rgb(colour.r(), colour.g(), colour.b()).hex()
}

/// The colour `text` names: `#rrggbb` or `rrggbb`, space around it dropped.
#[must_use]
pub fn colour(text: &str) -> Option<Color32> {
    crate::theme::typed(text)
}

impl Settings {
    /// Whether the page is open.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open
    }

    /// Opens the page with the colours of `appearance` in the custom accent's fields.
    pub fn open(&mut self, appearance: &Appearance) {
        self.open = true;
        self.note = None;
        self.fill(appearance);
    }

    /// Closes the page.
    pub fn close(&mut self) {
        self.open = false;
        self.note = None;
    }

    /// Puts the colours of `appearance` in the fields.
    fn fill(&mut self, appearance: &Appearance) {
        let preset = Preset::of(&appearance.accent);
        let [first, middle, last] = preset.gradient;
        self.colours = [hex(first), hex(middle), hex(last), hex(preset.accent)];
    }

    /// The four fields as typed: the gradient's three stops and the accent.
    #[must_use]
    pub fn colours(&self) -> &[String; 4] {
        &self.colours
    }

    /// The four fields, to type in.
    pub fn colours_mut(&mut self) -> &mut [String; 4] {
        &mut self.colours
    }

    /// The colours the fields name, as a preset called `custom`; `None` while one of them is not a colour.
    #[must_use]
    pub fn draft(&self) -> Option<Preset> {
        let [first, middle, last, accent] = &self.colours;
        Some(Preset { id: "custom", gradient: [colour(first)?, colour(middle)?, colour(last)?], accent: colour(accent)? })
    }

    /// The draft as the parameters of `ui.accent.set`: four `#rrggbb` colours, the accent last.
    #[must_use]
    pub fn custom(&self) -> Option<[String; 4]> {
        let draft = self.draft()?;
        let [first, middle, last] = draft.gradient;
        Some([hex(first), hex(middle), hex(last), hex(draft.accent)])
    }

    /// What the engine said about the colours it took last, when it changed one.
    #[must_use]
    pub fn note(&self) -> Option<&str> {
        self.note.as_deref()
    }

    /// Takes `answer`, the engine's answer to `ui.accent.set`, and `appearance` after it: the fields show the colours
    /// in use, and a note says so when the engine made the accent lighter.
    pub fn took(&mut self, answer: &Value, appearance: &Appearance) {
        self.fill(appearance);
        let requested = answer.get("requested").and_then(Value::as_str);
        self.note = match (answer.get("nudged").and_then(Value::as_bool), requested, appearance.accent) {
            (Some(true), Some(requested), Accent::Custom(custom)) => Some(tr!(
                "The accent {requested} is hard to read as text on dark surfaces, so {accent} is used instead.",
                requested = requested,
                accent = custom.accent.hex()
            )),
            _ => None,
        };
    }
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
