// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! InboxCraft's own look: the colour tokens, spacing and type sizes (`docs/design.md`).
//!
//! Every colour on screen is a token of a [`Palette`], and every token is derived from two choices: the mode (light
//! or dark) and the accent [`Preset`] (three gradient stops and an accent). Adding a preset is adding one entry to
//! [`PRESETS`]. Text tokens are moved in lightness until they meet WCAG AA (4.5:1) on every surface they sit on, and
//! markers and control edges until they meet 3:1; the tests check this for every preset in both modes. This file is
//! the only place that names a colour; a test refuses colour constants anywhere else in the crate.

use egui::epaint::Shadow;
use egui::{Color32, CornerRadius, FontFamily, FontId, Stroke, Style, TextStyle, Visuals, vec2};

/// Light or dark: the mode shown now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Dark text on light surfaces.
    Light,
    /// Light text on dark surfaces.
    Dark,
}

impl Mode {
    /// The mode `ctx` shows now (the system's choice unless one was set).
    #[must_use]
    pub fn of(ctx: &egui::Context) -> Self {
        match ctx.theme() {
            egui::Theme::Light => Self::Light,
            egui::Theme::Dark => Self::Dark,
        }
    }
}

/// The mode the owner chose: follow the system, or always light or dark.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModePreference {
    /// Whatever the system shows.
    #[default]
    System,
    /// Always light.
    Light,
    /// Always dark.
    Dark,
}

impl ModePreference {
    /// Every choice, in the order a picker shows them.
    pub const ALL: [Self; 3] = [Self::System, Self::Light, Self::Dark];

    /// The stable id of the choice (`system`, `light`, `dark`), for commands and settings.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    /// The choice with this id.
    #[must_use]
    pub fn named(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|choice| choice.id() == id)
    }
}

impl From<ModePreference> for egui::ThemePreference {
    fn from(choice: ModePreference) -> Self {
        match choice {
            ModePreference::System => Self::System,
            ModePreference::Light => Self::Light,
            ModePreference::Dark => Self::Dark,
        }
    }
}

/// An accent preset: three gradient stops and an accent, from which every colour token is derived.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Preset {
    /// The stable id (`electric` ...), for commands and settings.
    pub id: &'static str,
    /// The gradient, first to last stop, at full strength.
    pub gradient: [Color32; 3],
    /// The accent: links and accent text in dark mode, and one account colour.
    pub accent: Color32,
}

const ELECTRIC: Preset = Preset { id: "electric", gradient: [rgb(0x2F6BFF), rgb(0x8A3FFC), rgb(0xFF2D95)], accent: rgb(0xFFD60A) };

/// The presets InboxCraft ships, the default first.
pub const PRESETS: [Preset; 3] = [
    ELECTRIC,
    Preset { id: "aurora", gradient: [rgb(0x9B5CFF), rgb(0xFF2BD6), rgb(0xFF8A00)], accent: rgb(0x00E5FF) },
    Preset { id: "citrus", gradient: [rgb(0xFF2E63), rgb(0xFF8F00), rgb(0xFFD000)], accent: rgb(0x3D5AFE) },
];

impl Preset {
    /// The shipped preset with this id.
    #[must_use]
    pub fn named(id: &str) -> Option<&'static Self> {
        PRESETS.iter().find(|preset| preset.id == id)
    }
}

impl Default for Preset {
    /// Electric.
    fn default() -> Self {
        ELECTRIC
    }
}

/// The colour tokens of one mode and preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    /// Behind everything; the reader and hovered rows.
    pub background: Color32,
    /// The folder pane, buttons and the bars.
    pub panel: Color32,
    /// The message list.
    pub surface: Color32,
    /// Inside text fields.
    pub field: Color32,
    /// The edge of a text field.
    pub field_border: Color32,
    /// Text.
    pub text: Color32,
    /// Secondary text: previews, dates, addresses.
    pub muted: Color32,
    /// The selected row or folder, and the start of the selection gradient.
    pub selection: Color32,
    /// The end of the selection gradient.
    pub selection_end: Color32,
    /// Links and accent text: the accent in dark mode, the middle stop darkened in light mode.
    pub accent: Color32,
    /// Text on the accent colour.
    pub on_accent: Color32,
    /// The unread marker.
    pub unread: Color32,
    /// The flag marker of a flagged message, and text that asks for a second look.
    pub flag: Color32,
    /// The keyboard focus ring.
    pub focus: Color32,
    /// The unread count badge.
    pub badge: Color32,
    /// Text on the badge.
    pub on_badge: Color32,
    /// The error banner and error text.
    pub error: Color32,
    /// Text on the error banner.
    pub on_error: Color32,
    /// The preset's gradient at full strength (the navigation rail, the primary button, group badges).
    pub gradient: [Color32; 3],
    /// The dark ink for text on the gradient where white does not read ([`ink_on`] picks).
    pub ink: Color32,
    /// A glass layer over the colour fields: translucent.
    pub glass: Color32,
    /// The edge of a glass layer.
    pub glass_edge: Color32,
    /// The light top edge of a glass layer.
    pub glass_highlight: Color32,
    /// The soft shadow under glass and floats.
    pub shadow: Color32,
    /// What floats over content (tooltips, menus, hover actions, toasts): about 80 % opaque.
    pub float: Color32,
    /// Separators between rows.
    pub line: Color32,
    /// Laid over a hovered row.
    pub hover: Color32,
    /// The large soft colour fields painted once behind the window, one per stop.
    pub colour_fields: [Color32; 3],
    /// One colour per account, so the unified list shows where a message lives.
    pub accounts: [Color32; 6],
}

/// Distances in points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spacing {
    /// Between lines that belong together.
    pub xs: f32,
    /// Between items.
    pub s: f32,
    /// Inside a row.
    pub m: f32,
    /// Around a pane.
    pub l: f32,
    /// Between groups.
    pub xl: f32,
    /// The height of a message row: two lines of text and air.
    pub row_height: f32,
    /// The default width of the folder pane.
    pub folder_pane: f32,
    /// The corner radius of fields, badges and selections.
    pub corner: f32,
}

/// Type sizes in points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TypeScale {
    /// Badges, dates, captions.
    pub small: f32,
    /// List rows, folders, fields.
    pub body: f32,
    /// Section headings in the folder pane.
    pub heading: f32,
    /// The name of the view above the list.
    pub title: f32,
}

/// The whole look of one mode and preset.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    /// Which mode this is.
    pub mode: Mode,
    /// The preset its colours come from.
    pub preset: Preset,
    /// Its colours.
    pub palette: Palette,
    /// Its distances.
    pub spacing: Spacing,
    /// Its type sizes.
    pub type_scale: TypeScale,
}

const fn rgb(hex: u32) -> Color32 {
    // The masks keep each value in 0..=255, so the casts lose nothing.
    Color32::from_rgb(((hex >> 16) & 0xff) as u8, ((hex >> 8) & 0xff) as u8, (hex & 0xff) as u8)
}

/// `colour` at `alpha` (0 to 1) opacity.
fn alpha(colour: Color32, alpha: f32) -> Color32 {
    // Clamped to 0..=255 before the cast.
    let alpha = if alpha.is_nan() { 0.0 } else { (alpha.clamp(0.0, 1.0) * 255.0).round() };
    Color32::from_rgba_unmultiplied(colour.r(), colour.g(), colour.b(), alpha as u8)
}

const WHITE: Color32 = rgb(0xFFFFFF);
const BLACK: Color32 = rgb(0x000000);

/// The dark ink on the gradient: dark enough that white or it reads at 4.5:1 on any colour.
const DEEP_INK: Color32 = rgb(0x05060A);

/// The tint a picture is drawn with: none.
pub const NO_TINT: Color32 = WHITE;

/// The app icon's own colours (`icons.rs`), the same in every mode and preset so the desktop shows one icon.
pub(crate) mod app_icon {
    use egui::Color32;

    use super::rgb;

    /// The rounded tile.
    pub(crate) const TILE: Color32 = rgb(0x1D6A64);
    /// The letter.
    pub(crate) const PAPER: Color32 = rgb(0xFFFFFF);
    /// The lines of text on the letter.
    pub(crate) const INK: Color32 = rgb(0xA9D4CF);
    /// The tray.
    pub(crate) const TRAY: Color32 = rgb(0x6CC4BA);
    /// The unread dot.
    pub(crate) const DOT: Color32 = rgb(0xF0A06A);
}

/// The neutral starting points of one mode, before the preset tints them.
struct Neutrals {
    background: Color32,
    panel: Color32,
    surface: Color32,
    field: Color32,
    text: Color32,
    muted: Color32,
    error: Color32,
    float: Color32,
    /// How much of the middle stop goes into the surfaces.
    tint: f32,
    /// How much of the first and last stop goes into the selection gradient.
    selection: [f32; 2],
    /// How much of the middle stop goes into the badge.
    badge: f32,
    /// The opacity of the colour fields.
    fields: f32,
}

const LIGHT: Neutrals = Neutrals {
    background: rgb(0xF3F5F9),
    panel: rgb(0xEBEEF3),
    surface: rgb(0xFBFCFD),
    field: rgb(0xFFFFFF),
    text: rgb(0x161A22),
    muted: rgb(0x5D6676),
    error: rgb(0xB3261E),
    float: rgb(0xFFFFFF),
    tint: 0.03,
    selection: [0.20, 0.12],
    badge: 0.16,
    fields: 0.30,
};

const DARK: Neutrals = Neutrals {
    background: rgb(0x0D1016),
    panel: rgb(0x141821),
    surface: rgb(0x10131A),
    field: rgb(0x090B10),
    text: rgb(0xEEF1F6),
    muted: rgb(0x9AA3B2),
    error: rgb(0xF2B8B5),
    float: rgb(0x1E2330),
    tint: 0.05,
    selection: [0.32, 0.20],
    badge: 0.30,
    fields: 0.42,
};

const SPACING: Spacing = Spacing { xs: 4.0, s: 8.0, m: 12.0, l: 16.0, xl: 24.0, row_height: 52.0, folder_pane: 240.0, corner: 6.0 };

const TYPE_SCALE: TypeScale = TypeScale { small: 12.0, body: 14.0, heading: 15.0, title: 20.0 };

impl Palette {
    /// Every token of `mode` from `preset`.
    fn derive(mode: Mode, preset: &Preset) -> Self {
        let n = match mode {
            Mode::Light => &LIGHT,
            Mode::Dark => &DARK,
        };
        let [first, middle, last] = preset.gradient;
        let background = mix(n.background, middle, n.tint);
        let panel = mix(n.panel, middle, n.tint * 1.2);
        let surface = mix(n.surface, middle, n.tint * 0.6);
        let field = mix(n.field, middle, n.tint * 0.3);
        let selection = mix(surface, first, n.selection[0]);
        let selection_end = mix(surface, last, n.selection[1]);
        let surfaces = [background, panel, surface, field, selection, selection_end];
        let text = readable(mix(n.text, middle, 0.06), &surfaces, 4.5, mode);
        let muted = readable(mix(n.muted, middle, 0.08), &surfaces, 4.5, mode);
        let accent = match mode {
            // The middle stop mixed with black: even a yellow-orange reads on white.
            Mode::Light => readable(mix(middle, BLACK, 0.38), &surfaces, 4.5, mode),
            Mode::Dark => readable(preset.accent, &surfaces, 4.5, mode),
        };
        let marked = [background, panel, surface, selection];
        let badge = mix(panel, middle, n.badge);
        let error = readable(n.error, &surfaces, 4.5, mode);
        let accounts = [first, middle, last, preset.accent, mix(first, middle, 0.5), mix(middle, last, 0.5)]
            .map(|colour| readable(colour, &[background, panel, surface], 3.0, mode));
        let (sheer, shade) = match mode {
            Mode::Light => (mix(WHITE, middle, 0.02), mix(n.text, middle, 0.3)),
            Mode::Dark => (mix(WHITE, middle, 0.1), BLACK),
        };
        Self {
            background,
            panel,
            surface,
            field,
            field_border: readable(mix(muted, surface, 0.3), &[background, panel, surface, field], 3.0, mode),
            text,
            muted,
            selection,
            selection_end,
            accent,
            on_accent: ink_on(accent),
            unread: readable(middle, &marked, 3.0, mode),
            flag: readable(last, &marked, 4.5, mode),
            focus: readable(first, &surfaces, 3.0, mode),
            badge,
            on_badge: readable(accent, &[badge], 4.5, mode),
            error,
            on_error: ink_on(error),
            gradient: preset.gradient,
            ink: DEEP_INK,
            glass: match mode {
                Mode::Light => alpha(sheer, 0.72),
                Mode::Dark => alpha(sheer, 0.055),
            },
            glass_edge: match mode {
                Mode::Light => alpha(shade, 0.08),
                Mode::Dark => alpha(sheer, 0.09),
            },
            glass_highlight: match mode {
                Mode::Light => alpha(sheer, 0.9),
                Mode::Dark => alpha(sheer, 0.07),
            },
            shadow: match mode {
                Mode::Light => alpha(shade, 0.10),
                Mode::Dark => alpha(shade, 0.35),
            },
            float: alpha(mix(n.float, middle, n.tint * 1.5), 0.8),
            line: match mode {
                Mode::Light => alpha(shade, 0.07),
                Mode::Dark => alpha(sheer, 0.06),
            },
            hover: match mode {
                Mode::Light => alpha(shade, 0.045),
                Mode::Dark => alpha(sheer, 0.06),
            },
            colour_fields: preset.gradient.map(|stop| alpha(stop, n.fields)),
            accounts,
        }
    }
}

impl Theme {
    /// The look of `mode` with the colours of `preset`.
    #[must_use]
    pub fn new(mode: Mode, preset: &Preset) -> Self {
        Self { mode, preset: *preset, palette: Palette::derive(mode, preset), spacing: SPACING, type_scale: TYPE_SCALE }
    }

    /// The look `ctx` shows now with the colours of `preset`.
    #[must_use]
    pub fn current(ctx: &egui::Context, preset: &Preset) -> Self {
        Self::new(Mode::of(ctx), preset)
    }

    /// Installs both modes of `preset` in `ctx`; egui shows the one the mode preference asks for.
    pub fn apply(ctx: &egui::Context, preset: &Preset) {
        ctx.set_style_of(egui::Theme::Light, Self::new(Mode::Light, preset).style());
        ctx.set_style_of(egui::Theme::Dark, Self::new(Mode::Dark, preset).style());
    }

    /// The colour of the account with this index, repeating after the last.
    #[must_use]
    pub fn account_colour(&self, index: u64) -> Color32 {
        let colours = &self.palette.accounts;
        let len = u64::try_from(colours.len()).unwrap_or(1).max(1);
        usize::try_from(index % len).ok().and_then(|at| colours.get(at)).copied().unwrap_or(self.palette.accent)
    }

    /// The preset's gradient at `at` (0 the first stop, 0.5 the middle, 1 the last).
    #[must_use]
    pub fn gradient_at(&self, at: f32) -> Color32 {
        let [first, middle, last] = self.palette.gradient;
        let at = if at.is_nan() { 0.0 } else { at.clamp(0.0, 1.0) };
        if at < 0.5 { mix(first, middle, at * 2.0) } else { mix(middle, last, (at - 0.5) * 2.0) }
    }

    fn corner(&self) -> CornerRadius {
        // A few points; the clamp keeps the cast inside u8.
        CornerRadius::same(self.spacing.corner.clamp(0.0, 255.0) as u8)
    }

    /// egui's colours for this theme.
    #[must_use]
    pub fn visuals(&self) -> Visuals {
        let p = &self.palette;
        let mut visuals = match self.mode {
            Mode::Light => Visuals::light(),
            Mode::Dark => Visuals::dark(),
        };
        let corner = self.corner();
        visuals.override_text_color = Some(p.text);
        visuals.weak_text_color = Some(p.muted);
        visuals.panel_fill = p.panel;
        visuals.window_fill = p.surface;
        visuals.window_stroke = Stroke::new(1.0, p.field_border);
        visuals.window_corner_radius = corner;
        visuals.menu_corner_radius = corner;
        visuals.window_shadow = Shadow::NONE;
        visuals.popup_shadow = Shadow::NONE;
        visuals.extreme_bg_color = p.field;
        visuals.text_edit_bg_color = Some(p.field);
        visuals.faint_bg_color = p.background;
        visuals.code_bg_color = p.background;
        visuals.hyperlink_color = p.accent;
        visuals.error_fg_color = p.error;
        visuals.warn_fg_color = p.flag;
        visuals.selection.bg_fill = p.selection;
        visuals.selection.stroke = Stroke::new(1.0, p.text);
        let widgets = &mut visuals.widgets;
        for (state, fill, edge) in [
            (&mut widgets.noninteractive, p.panel, p.field_border),
            (&mut widgets.inactive, p.panel, p.field_border),
            (&mut widgets.hovered, p.selection, p.focus),
            (&mut widgets.active, p.selection, p.focus),
            (&mut widgets.open, p.selection, p.field_border),
        ] {
            state.bg_fill = fill;
            state.weak_bg_fill = fill;
            state.bg_stroke = Stroke::new(1.0, edge);
            state.fg_stroke = Stroke::new(1.0, p.text);
            state.corner_radius = corner;
            state.expansion = 0.0;
        }
        widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.selection);
        visuals
    }

    /// egui's style for this theme: the colours, the distances and the type sizes.
    #[must_use]
    pub fn style(&self) -> Style {
        let s = &self.spacing;
        let t = &self.type_scale;
        let mut style = Style { visuals: self.visuals(), ..Style::default() };
        style.spacing.item_spacing = vec2(s.s, s.xs);
        style.spacing.button_padding = vec2(s.s, s.xs);
        style.spacing.window_margin = egui::Margin::same(12);
        style.text_styles = [
            (TextStyle::Small, FontId::new(t.small, FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(t.body, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(t.body, FontFamily::Proportional)),
            (TextStyle::Heading, FontId::new(t.heading, FontFamily::Proportional)),
            (TextStyle::Monospace, FontId::new(t.body, FontFamily::Monospace)),
        ]
        .into();
        style
    }
}

/// `a` mixed with `b`: 0 is all `a`, 1 all `b` (sRGB, opaque; nonsense is all `a`).
#[must_use]
pub fn mix(a: Color32, b: Color32, amount: f32) -> Color32 {
    let t = if amount.is_nan() { 0.0 } else { amount.clamp(0.0, 1.0) };
    // Between two values of 0..=255, so the cast loses nothing.
    let channel = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgb(channel(a.r(), b.r()), channel(a.g(), b.g()), channel(a.b(), b.b()))
}

/// `colour`, moved towards black in light mode or white in dark mode just far enough to reach `ratio` against
/// every colour in `on`.
#[must_use]
pub fn readable(colour: Color32, on: &[Color32], ratio: f32, mode: Mode) -> Color32 {
    let towards = match mode {
        Mode::Light => BLACK,
        Mode::Dark => WHITE,
    };
    const STEPS: u8 = 40;
    (0..=STEPS)
        .map(|step| mix(colour, towards, f32::from(step) / f32::from(STEPS)))
        .find(|candidate| on.iter().all(|behind| contrast(*candidate, *behind) >= ratio))
        .unwrap_or(towards)
}

/// The ink for text on `behind`: white or the deep ink, whichever has more contrast.
#[must_use]
pub fn ink_on(behind: Color32) -> Color32 {
    if contrast(WHITE, behind) >= contrast(DEEP_INK, behind) { WHITE } else { DEEP_INK }
}

/// The WCAG contrast ratio of two colours, 1 to 21 (alpha is ignored).
#[must_use]
pub fn contrast(a: Color32, b: Color32) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    let (light, dark) = if la >= lb { (la, lb) } else { (lb, la) };
    (light + 0.05) / (dark + 0.05)
}

/// The relative luminance of an sRGB colour (WCAG 2).
fn luminance(colour: Color32) -> f32 {
    let channel = |value: u8| {
        let c = f32::from(value) / 255.0;
        if c <= 0.040_45 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * channel(colour.r()) + 0.7152 * channel(colour.g()) + 0.0722 * channel(colour.b())
}

#[cfg(test)]
#[path = "theme_tests.rs"]
mod tests;
