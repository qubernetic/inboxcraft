// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The owner's appearance choices: mode, text scale, motion and window frame.
//!
//! Motion has three levels ([`MotionLevel`]: full, reduced, off) and a flag saying whether the owner chose one; while
//! the owner has not, the computer's wish for less motion turns it off ([`effective`]). The front end still reads the
//! older two-way field [`Appearance::motion`] until its Appearance page moves to the levels (Task 5.2 of stage I-2);
//! that field and [`Appearance::motion_reduced`] together hold the level. [`Appearance::with_motion`] is their only
//! writer; a pair built otherwise is made one level by [`Appearance::normalised`], which the session applies to every
//! appearance it takes in.
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
    /// The older two-way motion setting, kept while the front end reads it: follow the system where it can be read
    /// (the web build; elsewhere motion runs), always reduce it, or always let it run. [`MotionLevel`] replaces it;
    /// Task 5.2 of stage I-2 removes it with the front end's last reader. In the kept file and in the commands'
    /// answers it is the field `motion`, for one release.
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
    /// How much the window moves: every movement, the calm ones only, or none. Which movements each level keeps is the
    /// front end's.
    MotionLevel {
        /// Every movement runs.
        Full = "full",
        /// Only the calm movements run.
        Reduced = "reduced",
        /// Nothing moves: every movement lands at its end at once.
        Off = "off",
    } default Reduced
}

/// The level the window moves at: the owner's `level` when `chosen`, else [`MotionLevel::Off`] when the computer asks
/// for less motion (`system_reduces`), else `level`.
#[must_use]
pub const fn effective(level: MotionLevel, chosen: bool, system_reduces: bool) -> MotionLevel {
    if !chosen && system_reduces { MotionLevel::Off } else { level }
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
    /// The older two-way motion setting, which the front end still reads: [`Motion::System`] while the owner has not
    /// chosen a level, [`Motion::Full`] for [`MotionLevel::Full`], [`Motion::Reduce`] for the other two. Set the level
    /// with [`Appearance::with_motion`], read it with [`Appearance::motion_level`]. Only `with_motion` writes this
    /// field and [`Appearance::motion_reduced`]: a struct update of one of them alone may disagree with the other, and
    /// [`Appearance::normalised`] then decides.
    pub motion: Motion,
    /// The owner chose [`MotionLevel::Reduced`]; `with_motion` sets it beside [`Motion::Reduce`] only. Only
    /// `with_motion` writes this field and [`Appearance::motion`]. While it is `false` (its default) `motion` alone
    /// says the level; when it is `true` the level is Reduced, chosen, whatever `motion` says.
    pub motion_reduced: bool,
    /// Who draws the window's frame.
    pub frame: WindowFrame,
    /// The app's own frame last on, the top row or the strip, never [`WindowFrame::System`]: the one the own frame
    /// comes back as after the desktop's title bar.
    pub own_frame: WindowFrame,
}

impl Default for Appearance {
    fn default() -> Self {
        let frame = WindowFrame::default();
        Self { theme: ThemeMode::default(), text_scale: 100, motion: Motion::default(), motion_reduced: false, frame, own_frame: frame }
    }
}

impl Appearance {
    /// The choices as JSON: the answer of every `ui.*.set` command, and the form the front end keeps.
    #[must_use]
    pub fn to_value(&self) -> Value {
        json!({
            "theme": self.theme.id(),
            "text_scale": self.text_scale,
            "motion": self.normalised().motion.id(),
            "motion_level": self.motion_level().id(),
            "motion_chosen": self.motion_chosen(),
            "frame": self.frame.id(),
            "own_frame": self.own_frame.id(),
        })
    }

    /// The choices kept as [`Appearance::to_value`] wrote them. The file is outside data: each choice that is missing
    /// or cannot be read is the default, and unknown fields (among them the accent, typeface and icon set of older
    /// versions) are ignored.
    ///
    /// Motion is `motion_level` with `motion_chosen` (missing or not a boolean: chosen; not chosen: the default
    /// level). Without a level that can be read, the field `motion` of older versions maps: `system` to the default
    /// level not chosen, `reduce` to off, `full` to full; a level's own name there reads as that level, chosen.
    #[must_use]
    pub fn from_value(value: &Value) -> Self {
        let empty = Map::new();
        let object = value.as_object().unwrap_or(&empty);
        let text = |key: &str| object.get(key).and_then(Value::as_str);
        let default = Self::default();
        let frame = text("frame").and_then(WindowFrame::named).unwrap_or(default.frame);
        let motion = match text("motion_level").and_then(MotionLevel::named) {
            Some(level) => Some((level, object.get("motion_chosen").and_then(Value::as_bool).unwrap_or(true))),
            None => text("motion").and_then(|old| match Motion::named(old) {
                Some(Motion::System) => Some((MotionLevel::Reduced, false)),
                Some(Motion::Reduce) => Some((MotionLevel::Off, true)),
                Some(Motion::Full) => Some((MotionLevel::Full, true)),
                None => MotionLevel::named(old).map(|level| (level, true)),
            }),
        };
        let (level, chosen) = motion.unwrap_or((MotionLevel::default(), false));
        Self {
            theme: text("theme").and_then(ThemeMode::named).unwrap_or(default.theme),
            text_scale: object
                .get("text_scale")
                .and_then(Value::as_u64)
                .and_then(|percent| u16::try_from(percent).ok())
                .filter(|percent| TEXT_SCALES.contains(percent))
                .unwrap_or(default.text_scale),
            frame,
            own_frame: Self::own_of(frame, text("own_frame").and_then(WindowFrame::named)),
            ..default
        }
        .with_motion(level, chosen)
    }

    /// These choices with motion at `level`, `chosen` by the owner or not. A level not chosen is the default level,
    /// whatever `level` is.
    #[must_use]
    pub const fn with_motion(self, level: MotionLevel, chosen: bool) -> Self {
        let (motion, motion_reduced) = match (chosen, level) {
            (false, _) => (Motion::System, false),
            (true, MotionLevel::Full) => (Motion::Full, false),
            (true, MotionLevel::Reduced) => (Motion::Reduce, true),
            (true, MotionLevel::Off) => (Motion::Reduce, false),
        };
        Self { motion, motion_reduced, ..self }
    }

    /// The motion level kept: the owner's choice, or the default level while there is none.
    #[must_use]
    pub const fn motion_level(&self) -> MotionLevel {
        match self.motion {
            _ if self.motion_reduced => MotionLevel::Reduced,
            Motion::System => MotionLevel::Reduced,
            Motion::Full => MotionLevel::Full,
            Motion::Reduce => MotionLevel::Off,
        }
    }

    /// These choices with [`Appearance::motion`] and [`Appearance::motion_reduced`] as `with_motion` writes them for
    /// the level they say: a pair built by a struct update that disagrees becomes one level.
    #[must_use]
    pub const fn normalised(self) -> Self {
        self.with_motion(self.motion_level(), self.motion_chosen())
    }

    /// Whether the owner chose the motion level; while not, the computer's wish decides ([`effective`]).
    #[must_use]
    pub const fn motion_chosen(&self) -> bool {
        self.motion_reduced || !matches!(self.motion, Motion::System)
    }

    /// The own frame to keep beside `frame`: `frame` itself when it is one of the app's own, else `kept` when that is
    /// one, else the default.
    #[must_use]
    pub fn own_of(frame: WindowFrame, kept: Option<WindowFrame>) -> WindowFrame {
        [Some(frame), kept].into_iter().flatten().find(|own| *own != WindowFrame::System).unwrap_or_default()
    }
}

#[cfg(test)]
#[path = "appearance_tests.rs"]
mod tests;
