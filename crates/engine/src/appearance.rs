// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The owner's appearance choices: mode, text scale, motion and window frame.
//!
//! The `ui.*.set` commands change one choice each; the session holds the result ([`crate::Session::appearance`]) and
//! the front end draws with it and keeps it on disk, handing it back with [`crate::Session::set_appearance`] at the
//! next start. The kept form is [`Appearance::to_value`]; [`Appearance::from_value`] reads it back choice by choice,
//! so a damaged file loses only what cannot be read. The colours, the typeface and the icons are fixed: a file kept
//! by an older version that names an accent, a typeface or an icon set loads, and those fields are dropped.

use serde_json::{Map, Value, json};

/// The text scales in per cent, smallest first.
pub const TEXT_SCALES: [u16; 5] = [90, 100, 110, 120, 130];

/// Defines a choice: an enum whose variants each have a stable id for commands and the kept file.
macro_rules! choice {
    ($(#[$doc:meta])* $name:ident { $($(#[$vdoc:meta])* $variant:ident = $id:literal),+ $(,)? } default $default:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name {
            $($(#[$vdoc])* $variant,)+
        }

        impl $name {
            /// Every choice, in the order a picker shows them.
            pub const ALL: [Self; [$(Self::$variant),+].len()] = [$(Self::$variant),+];

            /// The choice's stable id.
            #[must_use]
            pub const fn id(self) -> &'static str {
                match self {
                    $(Self::$variant => $id,)+
                }
            }

            /// The choice with this id.
            #[must_use]
            pub fn named(id: &str) -> Option<Self> {
                Self::ALL.into_iter().find(|choice| choice.id() == id)
            }
        }

        impl Default for $name {
            /// The choice of a fresh install.
            fn default() -> Self {
                Self::$default
            }
        }
    };
}

choice! {
    /// Light or dark: follow the system, or always one of them.
    ThemeMode {
        /// Whatever the system shows.
        System = "system",
        /// Always light.
        Light = "light",
        /// Always dark.
        Dark = "dark",
    } default System
}

choice! {
    /// Reduced motion: follow the system where it can be read (the web build; elsewhere motion runs), always reduce
    /// it, or always let it run.
    Motion {
        /// The system's wish where it can be read, otherwise motion runs.
        System = "system",
        /// Every animation lands at its end at once.
        Reduce = "reduce",
        /// Animations run.
        Full = "full",
    } default System
}

choice! {
    /// Who draws the window's frame: the desktop's title bar, or the app's own (its window controls in the top row,
    /// or on a slim strip of its own above the panes). Only a native window has a frame; elsewhere this is kept and
    /// does nothing.
    WindowFrame {
        /// The desktop's title bar.
        System = "system",
        /// The window controls in the top row, no title bar.
        TopRow = "top_row",
        /// A slim strip of the app's own with the title and the window controls.
        Strip = "strip",
    } default Strip
}

/// Every appearance choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Appearance {
    /// Light, dark or the system's.
    pub theme: ThemeMode,
    /// The text scale in per cent, one of [`TEXT_SCALES`].
    pub text_scale: u16,
    /// Reduced motion.
    pub motion: Motion,
    /// Who draws the window's frame.
    pub frame: WindowFrame,
}

impl Default for Appearance {
    fn default() -> Self {
        Self { theme: ThemeMode::default(), text_scale: 100, motion: Motion::default(), frame: WindowFrame::default() }
    }
}

impl Appearance {
    /// The choices as JSON: the answer of every `ui.*.set` command, and the form the front end keeps.
    #[must_use]
    pub fn to_value(&self) -> Value {
        json!({
            "theme": self.theme.id(),
            "text_scale": self.text_scale,
            "motion": self.motion.id(),
            "frame": self.frame.id(),
        })
    }

    /// The choices kept as [`Appearance::to_value`] wrote them. The file is outside data: each choice that is missing
    /// or cannot be read is the default, and unknown fields (among them the accent, typeface and icon set of older
    /// versions) are ignored.
    #[must_use]
    pub fn from_value(value: &Value) -> Self {
        let empty = Map::new();
        let object = value.as_object().unwrap_or(&empty);
        let text = |key: &str| object.get(key).and_then(Value::as_str);
        let default = Self::default();
        Self {
            theme: text("theme").and_then(ThemeMode::named).unwrap_or(default.theme),
            text_scale: object
                .get("text_scale")
                .and_then(Value::as_u64)
                .and_then(|percent| u16::try_from(percent).ok())
                .filter(|percent| TEXT_SCALES.contains(percent))
                .unwrap_or(default.text_scale),
            motion: text("motion").and_then(Motion::named).unwrap_or(default.motion),
            frame: text("frame").and_then(WindowFrame::named).unwrap_or(default.frame),
        }
    }
}

#[cfg(test)]
#[path = "appearance_tests.rs"]
mod tests;
