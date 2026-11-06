// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The owner's appearance choices: mode, accent, typeface, icon set, text scale and motion.
//!
//! The `ui.*.set` commands change one choice each; the session holds the result ([`crate::Session::appearance`]) and
//! the front end draws with it and keeps it on disk, handing it back with [`crate::Session::set_appearance`] at the
//! next start. The kept form is [`Appearance::to_value`]; [`Appearance::from_value`] reads it back choice by choice,
//! so a damaged file loses only what cannot be read.
//!
//! A custom accent is the colour of links and accent text in dark mode, so it must read there: an accent under WCAG
//! AA (4.5:1) on [`DARK_SURFACE`] is made lighter, hue and saturation kept, just far enough ([`readable`]). The front
//! end derives every other token from the four colours and checks each of them itself.

use serde_json::{Map, Value, json};

/// The WCAG AA ratio for text.
pub const AA: f32 = 4.5;

/// The lightest surface dark-mode text sits on before a preset tints it (the dark float of `docs/design.md`); a
/// custom accent must reach [`AA`] on it.
pub const DARK_SURFACE: Rgb = Rgb(0x1e, 0x23, 0x30);

/// The accent presets the front end ships, by id, the default first.
pub const PRESETS: [&str; 3] = ["electric", "aurora", "citrus"];

/// The text scales in per cent, smallest first.
pub const TEXT_SCALES: [u16; 5] = [90, 100, 110, 120, 130];

/// An opaque sRGB colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    /// The colour written as `#rrggbb`, nothing else (`#` and six hex digits, either case).
    #[must_use]
    pub fn from_hex(text: &str) -> Option<Self> {
        let digits = text.strip_prefix('#')?;
        if digits.len() != 6 || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        let channel = |at: usize| digits.get(at..at + 2).and_then(|pair| u8::from_str_radix(pair, 16).ok());
        Some(Self(channel(0)?, channel(2)?, channel(4)?))
    }

    /// `#rrggbb`, in lower case.
    #[must_use]
    pub fn hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }
}

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
    /// The typeface of every text style.
    Typeface {
        /// Figtree (OFL), the default.
        Figtree = "figtree",
        /// Inter (OFL).
        Inter = "inter",
    } default Figtree
}

choice! {
    /// The icon set.
    Icons {
        /// Phosphor (MIT), the default.
        Phosphor = "phosphor",
        /// Lucide (ISC).
        Lucide = "lucide",
    } default Phosphor
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

/// Four colours of the owner's own: the gradient's three stops and the accent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CustomAccent {
    /// The gradient, first to last stop.
    pub gradient: [Rgb; 3],
    /// The accent, made readable on dark surfaces ([`readable`]).
    pub accent: Rgb,
}

impl CustomAccent {
    /// The four colours as `#rrggbb`, the accent last.
    #[must_use]
    pub fn hex(&self) -> [String; 4] {
        let [first, middle, last] = self.gradient;
        [first.hex(), middle.hex(), last.hex(), self.accent.hex()]
    }

    /// Four `#rrggbb` colours, the accent last, as they were written; `None` unless there are exactly four.
    #[must_use]
    pub fn from_hex(colours: &[&str]) -> Option<Self> {
        let [first, middle, last, accent] = colours else { return None };
        Some(Self { gradient: [Rgb::from_hex(first)?, Rgb::from_hex(middle)?, Rgb::from_hex(last)?], accent: Rgb::from_hex(accent)? })
    }

    /// The same colours with the accent made readable ([`readable`]).
    #[must_use]
    pub fn readable(self) -> Self {
        Self { accent: readable(self.accent), ..self }
    }
}

/// Where the colours come from: a shipped preset by id, or four colours of the owner's own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Accent {
    /// One of [`PRESETS`].
    Preset(&'static str),
    /// The owner's own colours.
    Custom(CustomAccent),
}

impl Default for Accent {
    /// The first preset.
    fn default() -> Self {
        Self::Preset(PRESETS[0])
    }
}

impl Accent {
    /// The shipped preset with this id.
    #[must_use]
    pub fn preset(id: &str) -> Option<Self> {
        PRESETS.iter().find(|preset| **preset == id).map(|preset| Self::Preset(preset))
    }

    fn to_value(self) -> Value {
        match self {
            Self::Preset(id) => json!({ "preset": id }),
            Self::Custom(custom) => json!({ "custom": custom.hex() }),
        }
    }

    fn from_value(value: &Value) -> Option<Self> {
        let object = value.as_object()?;
        if let Some(id) = object.get("preset") {
            return Self::preset(id.as_str()?);
        }
        let colours: Vec<&str> = object.get("custom")?.as_array()?.iter().map(Value::as_str).collect::<Option<_>>()?;
        Some(Self::Custom(CustomAccent::from_hex(&colours)?.readable()))
    }
}

/// Every appearance choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Appearance {
    /// Light, dark or the system's.
    pub theme: ThemeMode,
    /// The colours.
    pub accent: Accent,
    /// The typeface.
    pub font: Typeface,
    /// The icon set.
    pub icons: Icons,
    /// The text scale in per cent, one of [`TEXT_SCALES`].
    pub text_scale: u16,
    /// Reduced motion.
    pub motion: Motion,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            theme: ThemeMode::default(),
            accent: Accent::default(),
            font: Typeface::default(),
            icons: Icons::default(),
            text_scale: 100,
            motion: Motion::default(),
        }
    }
}

impl Appearance {
    /// The choices as JSON: the answer of every `ui.*.set` command, and the form the front end keeps.
    #[must_use]
    pub fn to_value(&self) -> Value {
        json!({
            "theme": self.theme.id(),
            "accent": self.accent.to_value(),
            "font": self.font.id(),
            "icons": self.icons.id(),
            "text_scale": self.text_scale,
            "motion": self.motion.id(),
        })
    }

    /// The choices kept as [`Appearance::to_value`] wrote them. The file is outside data: each choice that is missing
    /// or cannot be read is the default, unknown fields are ignored, and a custom accent is made readable again.
    #[must_use]
    pub fn from_value(value: &Value) -> Self {
        let empty = Map::new();
        let object = value.as_object().unwrap_or(&empty);
        let text = |key: &str| object.get(key).and_then(Value::as_str);
        let default = Self::default();
        Self {
            theme: text("theme").and_then(ThemeMode::named).unwrap_or(default.theme),
            accent: object.get("accent").and_then(Accent::from_value).unwrap_or(default.accent),
            font: text("font").and_then(Typeface::named).unwrap_or(default.font),
            icons: text("icons").and_then(Icons::named).unwrap_or(default.icons),
            text_scale: object
                .get("text_scale")
                .and_then(Value::as_u64)
                .and_then(|percent| u16::try_from(percent).ok())
                .filter(|percent| TEXT_SCALES.contains(percent))
                .unwrap_or(default.text_scale),
            motion: text("motion").and_then(Motion::named).unwrap_or(default.motion),
        }
    }
}

/// The relative luminance of `colour` (WCAG 2).
fn luminance(colour: Rgb) -> f32 {
    let channel = |value: u8| {
        let c = f32::from(value) / 255.0;
        if c <= 0.040_45 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * channel(colour.0) + 0.7152 * channel(colour.1) + 0.0722 * channel(colour.2)
}

/// The WCAG contrast ratio of two colours, 1 to 21.
#[must_use]
pub fn contrast(a: Rgb, b: Rgb) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    let (light, dark) = if la >= lb { (la, lb) } else { (lb, la) };
    (light + 0.05) / (dark + 0.05)
}

/// `colour` as hue (0 to 1), saturation and lightness (0 to 1).
fn hsl(colour: Rgb) -> (f32, f32, f32) {
    let [r, g, b] = [colour.0, colour.1, colour.2].map(|channel| f32::from(channel) / 255.0);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let l = (max + min) / 2.0;
    let d = max - min;
    if d <= f32::EPSILON {
        return (0.0, 0.0, l);
    }
    let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
    let h = if (max - r).abs() <= f32::EPSILON {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if (max - g).abs() <= f32::EPSILON {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h / 6.0, s, l)
}

/// The colour of hue `h`, saturation `s` and lightness `l`; each is clamped to 0..=1, and nonsense is 0.
fn from_hsl(h: f32, s: f32, l: f32) -> Rgb {
    let unit = |x: f32| if x.is_nan() { 0.0 } else { x.clamp(0.0, 1.0) };
    let (h, s, l) = (unit(h), unit(s), unit(l));
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let hue = |t: f32| {
        let t = t.rem_euclid(1.0);
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    // Each value is in 0..=1, so the cast loses nothing.
    let channel = |t: f32| (hue(t) * 255.0).round().clamp(0.0, 255.0) as u8;
    Rgb(channel(h + 1.0 / 3.0), channel(h), channel(h - 1.0 / 3.0))
}

/// `accent` as it is, if it reaches [`AA`] on [`DARK_SURFACE`]; otherwise made lighter in steps of 1 % lightness,
/// hue and saturation kept, until it does. White always does.
#[must_use]
pub fn readable(accent: Rgb) -> Rgb {
    if contrast(accent, DARK_SURFACE) >= AA {
        return accent;
    }
    let (h, s, l) = hsl(accent);
    const STEPS: u8 = 100;
    (1..=STEPS)
        .map(|step| from_hsl(h, s, l + f32::from(step) / f32::from(STEPS)))
        .find(|candidate| contrast(*candidate, DARK_SURFACE) >= AA)
        .unwrap_or(Rgb(0xff, 0xff, 0xff))
}

#[cfg(test)]
#[path = "appearance_tests.rs"]
mod tests;
