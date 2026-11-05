// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! InboxCraft's own look: colours, spacing and type sizes for light and dark (`docs/design.md`).
//!
//! Every value here is our own. The palette is warm paper and harbour teal in light, slate and sea glass in dark;
//! every text colour meets WCAG AA (4.5:1) on the surface it sits on and every marker and control edge 3:1, which
//! the tests check.

use egui::epaint::Shadow;
use egui::{Color32, CornerRadius, FontFamily, FontId, Stroke, Style, TextStyle, Visuals, vec2};

/// Light or dark.
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

/// The colours of one mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    /// Behind everything.
    pub background: Color32,
    /// The folder pane and the bars.
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
    /// The selected row or folder.
    pub selection: Color32,
    /// Buttons and links.
    pub accent: Color32,
    /// Text on the accent colour.
    pub on_accent: Color32,
    /// The unread marker.
    pub unread: Color32,
    /// The keyboard focus ring.
    pub focus: Color32,
    /// The unread count badge.
    pub badge: Color32,
    /// Text on the badge.
    pub on_badge: Color32,
    /// The error banner.
    pub error: Color32,
    /// Text on the error banner.
    pub on_error: Color32,
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

/// The whole look of one mode.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    /// Which mode this is.
    pub mode: Mode,
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

const SPACING: Spacing = Spacing { xs: 4.0, s: 8.0, m: 12.0, l: 16.0, xl: 24.0, row_height: 52.0, folder_pane: 240.0, corner: 6.0 };

const TYPE_SCALE: TypeScale = TypeScale { small: 12.0, body: 14.0, heading: 15.0, title: 20.0 };

impl Theme {
    /// Warm paper and harbour teal.
    #[must_use]
    pub const fn light() -> Self {
        Self {
            mode: Mode::Light,
            palette: Palette {
                background: rgb(0xF6F5F1),
                panel: rgb(0xECEAE4),
                surface: rgb(0xFBFAF7),
                field: rgb(0xFFFFFF),
                field_border: rgb(0x80868D),
                text: rgb(0x1C2024),
                muted: rgb(0x565C63),
                selection: rgb(0xD5E6E3),
                accent: rgb(0x1D6A64),
                on_accent: rgb(0xFFFFFF),
                unread: rgb(0x1D6A64),
                focus: rgb(0x0F5C8C),
                badge: rgb(0x1D6A64),
                on_badge: rgb(0xFFFFFF),
                error: rgb(0xB3261E),
                on_error: rgb(0xFFFFFF),
                accounts: [rgb(0x1D6A64), rgb(0x8A4B9C), rgb(0xB5541A), rgb(0x2F5FA8), rgb(0x6B6B1F), rgb(0xA23B5B)],
            },
            spacing: SPACING,
            type_scale: TYPE_SCALE,
        }
    }

    /// Slate and sea glass.
    #[must_use]
    pub const fn dark() -> Self {
        Self {
            mode: Mode::Dark,
            palette: Palette {
                background: rgb(0x15181B),
                panel: rgb(0x1C2024),
                surface: rgb(0x191C1F),
                field: rgb(0x101315),
                field_border: rgb(0x6E757D),
                text: rgb(0xE7E5DF),
                muted: rgb(0xA5ABB2),
                selection: rgb(0x24413F),
                accent: rgb(0x6CC4BA),
                on_accent: rgb(0x10201E),
                unread: rgb(0x6CC4BA),
                focus: rgb(0x7FB8F0),
                badge: rgb(0x6CC4BA),
                on_badge: rgb(0x10201E),
                error: rgb(0xF2B8B5),
                on_error: rgb(0x3B0A08),
                accounts: [rgb(0x6CC4BA), rgb(0xC99BD8), rgb(0xF0A06A), rgb(0x8DB4F0), rgb(0xC9C66A), rgb(0xEE8FAA)],
            },
            spacing: SPACING,
            type_scale: TYPE_SCALE,
        }
    }

    /// The theme of `mode`.
    #[must_use]
    pub const fn for_mode(mode: Mode) -> Self {
        match mode {
            Mode::Light => Self::light(),
            Mode::Dark => Self::dark(),
        }
    }

    /// The theme `ctx` shows now.
    #[must_use]
    pub fn current(ctx: &egui::Context) -> Self {
        Self::for_mode(Mode::of(ctx))
    }

    /// Installs both modes in `ctx`; egui follows the system's choice between them unless one is set.
    pub fn apply(ctx: &egui::Context) {
        ctx.set_style_of(egui::Theme::Light, Self::light().style());
        ctx.set_style_of(egui::Theme::Dark, Self::dark().style());
    }

    /// The colour of the account with this index, repeating after the last.
    #[must_use]
    pub fn account_colour(&self, index: u64) -> Color32 {
        let colours = &self.palette.accounts;
        let len = u64::try_from(colours.len()).unwrap_or(1).max(1);
        usize::try_from(index % len).ok().and_then(|at| colours.get(at)).copied().unwrap_or(self.palette.accent)
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
