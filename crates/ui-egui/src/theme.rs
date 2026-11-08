// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! InboxCraft's own look: the colour tokens, spacing and font tokens (`docs/design.md`).
//!
//! Every colour on screen is a token of a [`Palette`], and every token is derived from two choices: the mode (light
//! or dark) and the accent [`Preset`] (three gradient stops and an accent). Adding a preset is adding one entry to
//! [`PRESETS`]. Text tokens are moved in lightness until they meet WCAG AA (4.5:1) on every surface they sit on, and
//! markers and control edges until they meet 3:1; the tests check this for every preset in both modes. This file is
//! the only place that names a colour; a test refuses colour constants anywhere else in the crate.
//!
//! Every text style is a font token of [`Fonts`]: a weight of the [`Typeface`] the owner chose (Figtree, replacing
//! egui's Ubuntu Light, or Inter; both bundled) or the monospaced face, at a size the [`TextScale`] multiplies. The icon
//! fonts are [`IconFont`]s. This file is also the only place that names a font.
//!
//! The owner's choices arrive as the engine's [`Appearance`] (the `ui.*.set` commands); [`Look`] is what of it the
//! tokens need, and [`Preset::of`] turns a preset id or four custom colours into a preset.

use egui::epaint::Shadow;
use egui::{Color32, CornerRadius, FontFamily, FontId, Stroke, Style, TextStyle, Visuals, vec2};

use inboxcraft_engine::appearance::{Accent, Appearance, Rgb};
pub use inboxcraft_engine::appearance::{ThemeMode as ModePreference, Typeface};

use crate::iconset::IconSet;
use crate::motion::{self, MOTION, Motion};

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

/// egui's preference for the mode the owner chose.
#[must_use]
pub fn preference(choice: ModePreference) -> egui::ThemePreference {
    match choice {
        ModePreference::System => egui::ThemePreference::System,
        ModePreference::Light => egui::ThemePreference::Light,
        ModePreference::Dark => egui::ThemePreference::Dark,
    }
}

/// An accent preset: three gradient stops and an accent, from which every colour token is derived.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Preset {
    /// The stable id (`electric` ...), for commands and settings; `custom` for the owner's own colours.
    pub id: &'static str,
    /// The gradient, first to last stop, at full strength.
    pub gradient: [Color32; 3],
    /// The accent: one account colour.
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

    /// The preset of `accent`: a shipped one by id (Electric for an id this front end does not ship), or `custom`
    /// with the owner's four colours.
    #[must_use]
    pub fn of(accent: &Accent) -> Self {
        let colour = |Rgb(r, g, b): Rgb| Color32::from_rgb(r, g, b);
        match accent {
            Accent::Preset(id) => Self::named(id).copied().unwrap_or_default(),
            Accent::Custom(custom) => Self { id: "custom", gradient: custom.gradient.map(colour), accent: colour(custom.accent) },
        }
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
    /// The opaque base of the cards and sheets.
    pub surface: Color32,
    /// The list's and the reader's cards: the surface, translucent, so the colour fields show through; no shadow.
    pub card: Color32,
    /// Inside text fields.
    pub field: Color32,
    /// The edge of a text field.
    pub field_border: Color32,
    /// The quiet edge of a control that is not selected: a pill, a segmented control.
    pub control_edge: Color32,
    /// Text.
    pub text: Color32,
    /// Secondary text: previews, dates, addresses.
    pub muted: Color32,
    /// The selected row, folder, pill or segment: one even tone.
    pub selection: Color32,
    /// Links and accent text: the middle stop lightened in dark mode, darkened in light mode.
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
    /// Behind the window's Close control under the pointer: the plain red most desktops use there, in both modes.
    pub close: Color32,
    /// The cross on [`Palette::close`].
    pub on_close: Color32,
    /// The preset's gradient at full strength (the navigation rail, the primary button, group badges).
    pub gradient: [Color32; 3],
    /// The navigation rail's stops ([`rail_stops`]): the gradient, darkened only as far as white icons need.
    pub rail: [Color32; 3],
    /// The icons on [`Palette::rail`].
    pub rail_ink: Color32,
    /// The dark ink for text on the gradient where white does not read ([`ink_on`] picks).
    pub ink: Color32,
    /// A glass layer over the colour fields: translucent.
    pub glass: Color32,
    /// The edge of a glass layer.
    pub glass_edge: Color32,
    /// Over the window behind a sheet (the settings): a light shade, so the window shows through dimmed.
    pub scrim: Color32,
    /// The outline of a window whose frame the app draws: about as strong as the desktops' own, so the window stands
    /// apart from what lies behind it.
    pub window_edge: Color32,
    /// The light top edge of a glass layer.
    pub glass_highlight: Color32,
    /// The soft shadow under glass and floats.
    pub shadow: Color32,
    /// What floats over content that fades before it (hover actions over a row, toasts): about 80 % opaque, so a
    /// little of the colour behind glows through.
    pub float: Color32,
    /// What floats over sharp text that cannot fade (menus and tooltips over the reader): the same colour, opaque
    /// enough that the text under it shows at no more than [`GHOST`].
    pub float_over_text: Color32,
    /// Separators between rows.
    pub line: Color32,
    /// Laid over a hovered row.
    pub hover: Color32,
    /// The large soft colour fields painted once behind the window, one per stop.
    pub colour_fields: [Color32; 3],
    /// One colour per account, so the unified list shows where a message lives.
    pub accounts: [Color32; 6],
    /// An event's fill, one per account: a tint of the account's colour, as strong as text on it still reads over
    /// every glass. Its edge is the account's colour at full strength.
    pub events: [Color32; 6],
    /// The avatar colours: vivid, one picked per address ([`crate::component::avatar`]); white or the deep ink
    /// reads on each.
    pub avatars: [Color32; 10],
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
    /// The default width of the folder pane.
    pub folder_pane: f32,
    /// The corner radius of fields, badges and selections.
    pub corner: f32,
    /// The corner radius of glass layers and floats.
    pub card: f32,
    /// How far the soft shadow of a glass layer or a float reaches.
    pub shadow: f32,
    /// The colour fields that show between two glass layers.
    pub gap: f32,
}

/// A weight of the typeface, lightest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Weight {
    /// Running text and previews.
    Regular,
    /// Read senders and subjects, buttons.
    Medium,
    /// Unread subjects, headings, bold text in a message.
    SemiBold,
    /// Unread senders and titles.
    Bold,
}

/// A face of a font token: the typeface at a weight, or the monospaced face.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Face {
    /// The typeface (Figtree).
    Sans(Weight),
    /// The monospaced face, for preformatted text and code.
    Mono,
}

/// A font token: a face of a typeface and a size in points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Font {
    /// The size in points, the text scale applied.
    pub size: f32,
    /// The face.
    pub face: Face,
    /// The typeface a [`Face::Sans`] weight is drawn in.
    pub typeface: Typeface,
}

/// The weights the tokens use, lightest first.
const WEIGHTS: [Weight; 4] = [Weight::Regular, Weight::Medium, Weight::SemiBold, Weight::Bold];

/// The family name of a weight of a typeface: one font file each. Figtree Regular is egui's proportional family too.
const fn family_name(typeface: Typeface, weight: Weight) -> &'static str {
    match (typeface, weight) {
        (Typeface::Figtree, Weight::Regular) => "Figtree-Regular",
        (Typeface::Figtree, Weight::Medium) => "Figtree-Medium",
        (Typeface::Figtree, Weight::SemiBold) => "Figtree-SemiBold",
        (Typeface::Figtree, Weight::Bold) => "Figtree-Bold",
        (Typeface::Inter, Weight::Regular) => "Inter-Regular",
        (Typeface::Inter, Weight::Medium) => "Inter-Medium",
        (Typeface::Inter, Weight::SemiBold) => "Inter-SemiBold",
        (Typeface::Inter, Weight::Bold) => "Inter-Bold",
    }
}

impl Font {
    const fn sans(size: f32, weight: Weight) -> Self {
        Self { size, face: Face::Sans(weight), typeface: Typeface::Figtree }
    }

    /// egui's font for this token.
    #[must_use]
    pub fn id(self) -> FontId {
        let family = match (self.face, self.typeface) {
            (Face::Mono, _) => FontFamily::Monospace,
            (Face::Sans(Weight::Regular), Typeface::Figtree) => FontFamily::Proportional,
            (Face::Sans(weight), typeface) => FontFamily::Name(family_name(typeface, weight).into()),
        };
        FontId::new(self.size, family)
    }

    fn scaled(self, factor: f32) -> Self {
        Self { size: self.size * factor, ..self }
    }
}

/// The font tokens: every text style on screen is one of these.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fonts {
    /// Dates, addresses, badges, captions and section headings.
    pub caption: Font,
    /// Running text, folders, fields, empty-state details.
    pub body: Font,
    /// Bold text in a message.
    pub strong: Font,
    /// Buttons.
    pub button: Font,
    /// The sender of a read message.
    pub sender: Font,
    /// The sender of an unread message.
    pub sender_unread: Font,
    /// The subject of a read message.
    pub subject: Font,
    /// The subject of an unread message.
    pub subject_unread: Font,
    /// The first words of a message, after its subject (in the muted colour).
    pub preview: Font,
    /// Account names, empty-state titles, message headings.
    pub heading: Font,
    /// The view's name, page titles.
    pub title: Font,
    /// The reader's subject.
    pub display: Font,
    /// Preformatted text and code.
    pub mono: Font,
}

impl Fonts {
    /// Every token.
    #[must_use]
    pub fn all(&self) -> Vec<Font> {
        vec![
            self.caption,
            self.body,
            self.strong,
            self.button,
            self.sender,
            self.sender_unread,
            self.subject,
            self.subject_unread,
            self.preview,
            self.heading,
            self.title,
            self.display,
            self.mono,
        ]
    }

    fn map(self, f: impl Fn(Font) -> Font) -> Self {
        Self {
            caption: f(self.caption),
            body: f(self.body),
            strong: f(self.strong),
            button: f(self.button),
            sender: f(self.sender),
            sender_unread: f(self.sender_unread),
            subject: f(self.subject),
            subject_unread: f(self.subject_unread),
            preview: f(self.preview),
            heading: f(self.heading),
            title: f(self.title),
            display: f(self.display),
            mono: f(self.mono),
        }
    }

    /// The tokens at 100 % in `typeface`, at `scale`.
    fn of(typeface: Typeface, scale: TextScale) -> Self {
        FONTS.map(|font| Font { typeface, ..font.scaled(scale.factor()) })
    }
}

/// The text scale the owner chose: 90 % to 130 % in steps of 10 %, applied to every font token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextScale(u8);

impl TextScale {
    /// Every scale, smallest first.
    pub const ALL: [Self; 5] = [Self(90), Self(100), Self(110), Self(120), Self(130)];

    /// The scale in per cent.
    #[must_use]
    pub fn percent(self) -> u16 {
        u16::from(self.0)
    }

    /// The scale of `percent`, if it is one of [`Self::ALL`].
    #[must_use]
    pub fn from_percent(percent: u16) -> Option<Self> {
        Self::ALL.into_iter().find(|scale| scale.percent() == percent)
    }

    /// What the 100 % sizes are multiplied by.
    #[must_use]
    pub fn factor(self) -> f32 {
        f32::from(self.0) / 100.0
    }
}

impl Default for TextScale {
    /// 100 %.
    fn default() -> Self {
        Self(100)
    }
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
    /// The text scale its fonts are at.
    pub text_scale: TextScale,
    /// The typeface its fonts are in.
    pub typeface: Typeface,
    /// Its font tokens, in the typeface at the text scale.
    pub fonts: Fonts,
    /// The icon set its icons are drawn from.
    pub icons: IconSet,
    /// How long its motion takes ([`crate::motion`]).
    pub motion: Motion,
}

/// The red behind Close, the same in every preset and mode.
const CLOSE: Color32 = rgb(0xD92D20);

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

/// The avatar colours, the same in both modes: vivid, no teal or coral, far enough apart to tell people apart.
pub const AVATARS: [Color32; 10] = [
    rgb(0x2F5BEA),
    rgb(0x0A84FF),
    rgb(0x7B3FF2),
    rgb(0xC026D3),
    rgb(0xE3218F),
    rgb(0xD62839),
    rgb(0xF06A00),
    rgb(0xF5C400),
    rgb(0x3DA62B),
    rgb(0x7CB518),
];

/// The tint a picture is drawn with: none.
pub const NO_TINT: Color32 = WHITE;

/// The app icon's own colours (`icons.rs`), the same in every mode and preset so the desktop shows one icon.
pub(crate) mod app_icon {
    use egui::Color32;

    use super::rgb;

    /// The white tile.
    pub(crate) const TILE: Color32 = rgb(0xFFFFFF);
    /// The tile's edge: the deep ink at 16 %, so the white tile holds its shape on a light background.
    pub(crate) const EDGE: Color32 = Color32::from_rgba_premultiplied(1, 1, 2, 41);
    /// The Electric gradient of the mark, first to last stop.
    pub(crate) const GRADIENT: [Color32; 3] = [rgb(0x2F6BFF), rgb(0x8A3FFC), rgb(0xFF2D95)];
    /// Where the gradient starts and ends on the icon's square, in units.
    pub(crate) const GRADIENT_LINE: [[f32; 2]; 2] = [[48.0, 60.0], [208.0, 200.0]];
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
    /// How much of the middle stop goes into the selection.
    selection: f32,
    /// How much of the middle stop goes into the badge.
    badge: f32,
    /// The opacity of a card.
    card: f32,
    /// The opacity of an event's tint, at most.
    event: f32,
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
    selection: 0.16,
    badge: 0.16,
    card: 0.80,
    event: 0.20,
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
    selection: 0.30,
    badge: 0.30,
    card: 0.62,
    event: 0.30,
    fields: 0.34,
};

pub(crate) const SPACING: Spacing =
    Spacing { xs: 4.0, s: 8.0, m: 12.0, l: 16.0, xl: 24.0, folder_pane: 240.0, corner: 6.0, card: 14.0, shadow: 18.0, gap: 10.0 };

/// The most contrast text under a float over sharp text may keep: barely there.
pub const GHOST: f32 = 1.2;

/// The font tokens at 100 %.
const FONTS: Fonts = Fonts {
    caption: Font::sans(12.0, Weight::Regular),
    body: Font::sans(14.0, Weight::Regular),
    strong: Font::sans(14.0, Weight::SemiBold),
    button: Font::sans(14.0, Weight::Medium),
    sender: Font::sans(14.0, Weight::Medium),
    sender_unread: Font::sans(14.0, Weight::Bold),
    subject: Font::sans(14.0, Weight::Regular),
    subject_unread: Font::sans(14.0, Weight::SemiBold),
    preview: Font::sans(14.0, Weight::Regular),
    heading: Font::sans(15.0, Weight::SemiBold),
    title: Font::sans(20.0, Weight::Bold),
    display: Font::sans(26.0, Weight::Bold),
    mono: Font { size: 13.0, face: Face::Mono, typeface: Typeface::Figtree },
};

/// Figtree (SIL Open Font Licence 1.1, `assets/fonts/figtree/OFL.txt`) and Inter (the same licence,
/// `assets/fonts/inter/OFL.txt`), the weights the tokens use, by [`family_name`].
const TYPEFACES: [(&str, &[u8]); 8] = [
    ("Figtree-Regular", include_bytes!("../../../assets/fonts/figtree/Figtree-Regular.ttf")),
    ("Figtree-Medium", include_bytes!("../../../assets/fonts/figtree/Figtree-Medium.ttf")),
    ("Figtree-SemiBold", include_bytes!("../../../assets/fonts/figtree/Figtree-SemiBold.ttf")),
    ("Figtree-Bold", include_bytes!("../../../assets/fonts/figtree/Figtree-Bold.ttf")),
    ("Inter-Regular", include_bytes!("../../../assets/fonts/inter/Inter-Regular.ttf")),
    ("Inter-Medium", include_bytes!("../../../assets/fonts/inter/Inter-Medium.ttf")),
    ("Inter-SemiBold", include_bytes!("../../../assets/fonts/inter/Inter-SemiBold.ttf")),
    ("Inter-Bold", include_bytes!("../../../assets/fonts/inter/Inter-Bold.ttf")),
];

/// The font an icon set draws a weight with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IconFont {
    /// Phosphor Regular: lines.
    PhosphorRegular,
    /// Phosphor Fill: solid shapes.
    PhosphorFill,
    /// Phosphor Duotone: each icon a back glyph and a line glyph.
    PhosphorDuotone,
    /// Lucide: lines, its only weight.
    Lucide,
}

/// Phosphor (MIT, `assets/icons/phosphor/LICENSE`), the weights the icon painter uses, and Lucide (ISC,
/// `assets/icons/lucide/LICENSE`), each a family of its own.
const ICON_FONTS: [(IconFont, &str, &[u8]); 4] = [
    (IconFont::PhosphorRegular, "Phosphor", include_bytes!("../../../assets/icons/phosphor/Phosphor.ttf")),
    (IconFont::PhosphorFill, "Phosphor-Fill", include_bytes!("../../../assets/icons/phosphor/Phosphor-Fill.ttf")),
    (IconFont::PhosphorDuotone, "Phosphor-Duotone", include_bytes!("../../../assets/icons/phosphor/Phosphor-Duotone.ttf")),
    (IconFont::Lucide, "Lucide", include_bytes!("../../../assets/icons/lucide/Lucide.ttf")),
];

impl IconFont {
    /// egui's font for this icon font at `size` points.
    #[must_use]
    pub fn id(self, size: f32) -> FontId {
        let name = ICON_FONTS.iter().find(|(font, _, _)| *font == self).map_or("Phosphor", |(_, name, _)| name);
        FontId::new(size, FontFamily::Name(name.into()))
    }
}

/// egui's fonts with Figtree in place of Ubuntu Light: each weight of Figtree and Inter is a family of its own,
/// followed by egui's emoji and icon fonts; the monospaced face (Hack) falls back to Figtree. Each icon font is a family
/// of its own too, with nothing behind it. Every typeface and icon set is installed, so a change of either is a change
/// of tokens only.
#[must_use]
pub fn font_definitions() -> egui::FontDefinitions {
    let mut definitions = egui::FontDefinitions::default();
    definitions.font_data.remove("Ubuntu-Light");
    for fonts in definitions.families.values_mut() {
        fonts.retain(|font| font != "Ubuntu-Light");
    }
    for (name, data) in TYPEFACES {
        definitions.font_data.insert(name.to_owned(), std::sync::Arc::new(egui::FontData::from_static(data)));
    }
    let fallbacks = definitions.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let family = |first: &str| std::iter::once(first.to_owned()).chain(fallbacks.iter().cloned()).collect::<Vec<_>>();
    definitions.families.insert(FontFamily::Proportional, family("Figtree-Regular"));
    for typeface in Typeface::ALL {
        for weight in WEIGHTS {
            let name = family_name(typeface, weight);
            definitions.families.insert(FontFamily::Name(name.into()), family(name));
        }
    }
    let monospace = definitions.families.entry(FontFamily::Monospace).or_default();
    monospace.insert(monospace.len().min(1), "Figtree-Regular".to_owned());
    for (_, name, data) in ICON_FONTS {
        definitions.font_data.insert(name.to_owned(), std::sync::Arc::new(egui::FontData::from_static(data)));
        definitions.families.insert(FontFamily::Name(name.into()), vec![name.to_owned()]);
    }
    definitions
}

/// Installs [`font_definitions`] in `ctx`, from its next frame.
pub fn install_fonts(ctx: &egui::Context) {
    ctx.set_fonts(font_definitions());
}

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
        let selection = mix(surface, middle, n.selection);
        let card = alpha(surface, n.card);
        let (sheer, shade) = match mode {
            Mode::Light => (mix(WHITE, middle, 0.02), mix(n.text, middle, 0.3)),
            Mode::Dark => (mix(WHITE, middle, 0.1), BLACK),
        };
        let glass = match mode {
            Mode::Light => alpha(sheer, 0.72),
            Mode::Dark => alpha(sheer, 0.055),
        };
        let hover = match mode {
            Mode::Light => alpha(shade, 0.045),
            Mode::Dark => alpha(sheer, 0.06),
        };
        let float_base = mix(n.float, middle, n.tint * 1.5);
        let float = alpha(float_base, 0.8);
        let colour_fields = preset.gradient.map(|stop| alpha(stop, n.fields));
        // What text can sit on: the opaque surfaces, glass and the cards on it (hovered or not) over the colour fields,
        // the selection (hovered or not), and a float over the fields, the full gradient or itself.
        let mut backdrops = vec![background];
        backdrops.extend(colour_fields.map(|colour| over(colour, background)));
        let glassy: Vec<Color32> = backdrops.iter().map(|behind| over(glass, *behind)).collect();
        let mut surfaces = vec![background, panel, surface, field, selection, float_base];
        surfaces.push(over(hover, selection));
        surfaces.extend(glassy.iter().flat_map(|behind| [*behind, over(hover, *behind), over(card, *behind), over(hover, over(card, *behind))]));
        surfaces.extend(backdrops.iter().chain(preset.gradient.iter()).map(|behind| over(float, *behind)));
        let text = readable(mix(n.text, middle, 0.06), &surfaces, 4.5, mode);
        let muted = readable(mix(n.muted, middle, 0.08), &surfaces, 4.5, mode);
        let accent = match mode {
            // The middle stop mixed with black: even a yellow-orange reads on white.
            Mode::Light => readable(mix(middle, BLACK, 0.38), &surfaces, 4.5, mode),
            // The middle stop mixed 30 % with white: it stays in the gradient's family (the preset's separate
            // accent stays an account colour).
            Mode::Dark => readable(mix(middle, WHITE, 0.3), &surfaces, 4.5, mode),
        };
        let marked = [background, panel, surface, selection];
        let badge = mix(panel, middle, n.badge);
        let error = readable(n.error, &surfaces, 4.5, mode);
        let accounts = [first, middle, last, preset.accent, mix(first, middle, 0.5), mix(middle, last, 0.5)]
            .map(|colour| readable(colour, &[background, panel, surface], 3.0, mode));
        let events = accounts.map(|colour| event_tint(colour, n.event, text, &glassy));
        let mut sharp = vec![background, panel, surface];
        sharp.extend(glassy);
        let float_over_text = quieting(float_base, 0.8, &[text, muted, accent], &sharp);
        let (rail, rail_ink) = rail_stops(preset.gradient);
        Self {
            background,
            panel,
            surface,
            card,
            field,
            field_border: readable(mix(muted, surface, 0.3), &[background, panel, surface, field], 3.0, mode),
            control_edge: match mode {
                Mode::Light => alpha(shade, 0.18),
                Mode::Dark => alpha(sheer, 0.18),
            },
            text,
            muted,
            selection,
            accent,
            on_accent: ink_on(accent),
            unread: readable(middle, &marked, 3.0, mode),
            flag: readable(last, &marked, 4.5, mode),
            focus: readable(first, &surfaces, 3.0, mode),
            badge,
            on_badge: readable(accent, &[badge], 4.5, mode),
            error,
            on_error: ink_on(error),
            close: CLOSE,
            on_close: ink_on(CLOSE),
            gradient: preset.gradient,
            rail,
            rail_ink,
            ink: DEEP_INK,
            glass,
            glass_edge: match mode {
                Mode::Light => alpha(shade, 0.08),
                Mode::Dark => alpha(sheer, 0.09),
            },
            scrim: alpha(shade, 0.25),
            window_edge: match mode {
                Mode::Light => alpha(shade, 0.24),
                Mode::Dark => alpha(sheer, 0.16),
            },
            glass_highlight: match mode {
                Mode::Light => alpha(sheer, 0.9),
                Mode::Dark => alpha(sheer, 0.07),
            },
            shadow: match mode {
                Mode::Light => alpha(shade, 0.10),
                Mode::Dark => alpha(shade, 0.35),
            },
            float,
            float_over_text,
            line: match mode {
                Mode::Light => alpha(shade, 0.07),
                Mode::Dark => alpha(sheer, 0.06),
            },
            hover,
            colour_fields,
            accounts,
            events,
            avatars: AVATARS,
        }
    }
}

/// What the tokens take from the owner's choices besides the mode: the preset, the text scale, the typeface and the
/// icon set.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Look {
    /// The colours.
    pub preset: Preset,
    /// The text scale.
    pub text_scale: TextScale,
    /// The typeface.
    pub typeface: Typeface,
    /// The icon set.
    pub icons: IconSet,
}

impl Look {
    /// What the tokens take from `appearance`; a text scale this front end does not have is 100 %.
    #[must_use]
    pub fn of(appearance: &Appearance) -> Self {
        Self {
            preset: Preset::of(&appearance.accent),
            text_scale: TextScale::from_percent(appearance.text_scale).unwrap_or_default(),
            typeface: appearance.font,
            icons: IconSet::of(appearance.icons),
        }
    }
}

impl Theme {
    /// The look of `mode` with the colours of `preset`, in Figtree at 100 % with Phosphor icons.
    #[must_use]
    pub fn new(mode: Mode, preset: &Preset) -> Self {
        let (text_scale, typeface) = (TextScale::default(), Typeface::default());
        Self {
            mode,
            preset: *preset,
            palette: Palette::derive(mode, preset),
            spacing: SPACING,
            text_scale,
            typeface,
            fonts: Fonts::of(typeface, text_scale),
            icons: IconSet::default(),
            motion: MOTION,
        }
    }

    /// The look of `mode` with everything else from `look`.
    #[must_use]
    pub fn of(mode: Mode, look: &Look) -> Self {
        Self::new(mode, &look.preset).with_text_scale(look.text_scale).with_typeface(look.typeface).with_icons(look.icons)
    }

    /// The same look with every font token at `scale`.
    #[must_use]
    pub fn with_text_scale(self, scale: TextScale) -> Self {
        Self { text_scale: scale, fonts: Fonts::of(self.typeface, scale), ..self }
    }

    /// The same look with every font token in `typeface`.
    #[must_use]
    pub fn with_typeface(self, typeface: Typeface) -> Self {
        Self { typeface, fonts: Fonts::of(typeface, self.text_scale), ..self }
    }

    /// The same look with the icons from `icons`.
    #[must_use]
    pub fn with_icons(self, icons: IconSet) -> Self {
        Self { icons, ..self }
    }

    /// The look `ctx` shows now with everything else from `look`.
    #[must_use]
    pub fn current(ctx: &egui::Context, look: &Look) -> Self {
        Self::of(Mode::of(ctx), look)
    }

    /// Installs both modes of `look` in `ctx`; egui shows the one the mode preference asks for.
    pub fn apply(ctx: &egui::Context, look: &Look) {
        ctx.set_style_of(egui::Theme::Light, Self::of(Mode::Light, look).style());
        ctx.set_style_of(egui::Theme::Dark, Self::of(Mode::Dark, look).style());
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
        // Menus, tooltips and popups float over whatever is there, the reader's sharp text too.
        visuals.window_fill = p.float_over_text;
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

    /// egui's style for this theme: the colours, the distances and the font tokens.
    #[must_use]
    pub fn style(&self) -> Style {
        let s = &self.spacing;
        let f = &self.fonts;
        let mut style = Style { visuals: self.visuals(), ..Style::default() };
        style.spacing.item_spacing = vec2(s.s, s.xs);
        style.spacing.button_padding = vec2(s.s, s.xs);
        style.spacing.window_margin = egui::Margin::same(12);
        // egui's own fade at the ends of a scroll area is a gradient laid over text: neither is allowed here.
        style.spacing.scroll.fade.strength = 0.0;
        // A bar wide enough to see at rest and wider under the pointer; the widgets' corner rounds both its ends.
        style.spacing.scroll.floating_width = 5.0;
        style.spacing.scroll.bar_width = 9.0;
        motion::tune(&mut style);
        style.text_styles = [
            (TextStyle::Small, f.caption.id()),
            (TextStyle::Body, f.body.id()),
            (TextStyle::Button, f.button.id()),
            (TextStyle::Heading, f.heading.id()),
            (TextStyle::Monospace, f.mono.id()),
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

/// `top` laid over the opaque `under`.
#[must_use]
pub fn over(top: Color32, under: Color32) -> Color32 {
    // Premultiplied: the top's channels plus what its alpha leaves of the colour under it; at most 255 each.
    let keep = 255 - u16::from(top.a());
    let channel = |t: u8, u: u8| u8::try_from((u16::from(t) + (u16::from(u) * keep + 127) / 255).min(255)).unwrap_or(u8::MAX);
    Color32::from_rgb(channel(top.r(), under.r()), channel(top.g(), under.g()), channel(top.b(), under.b()))
}

/// The tint of an event in the account colour `colour`: at `most` opacity, less in steps of 2 % while `text` would
/// not read on it at 4.5:1 over one of `glassy`, and never below 4 %.
fn event_tint(colour: Color32, most: f32, text: Color32, glassy: &[Color32]) -> Color32 {
    const STEP: f32 = 0.02;
    const LEAST: f32 = 0.04;
    // A dozen or so steps: the clamp keeps the cast inside u8.
    let steps = ((most - LEAST) / STEP).round().clamp(0.0, 255.0) as u8;
    let reads = |strength: f32| glassy.iter().all(|behind| contrast(text, over(alpha(colour, strength), *behind)) >= 4.5);
    let strength = (0..steps).map(|step| most - f32::from(step) * STEP).find(|strength| reads(*strength)).unwrap_or(LEAST);
    alpha(colour, strength)
}

/// `colour` at the least opacity from `from` up that keeps every ink in `inks`, sitting on any of `on`, at no more
/// than [`GHOST`] under it; opaque when nothing less does.
fn quieting(colour: Color32, from: f32, inks: &[Color32], on: &[Color32]) -> Color32 {
    const STEPS: u8 = 40;
    (0..=STEPS)
        .map(|step| alpha(colour, from + (1.0 - from) * f32::from(step) / f32::from(STEPS)))
        .find(|float| on.iter().all(|behind| inks.iter().all(|ink| contrast(over(*float, *ink), over(*float, *behind)) <= GHOST)))
        .unwrap_or(colour)
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

/// The owner's avatar on the rail: a white disc, and the preset's middle stop darkened until it reads on it at 4.5:1.
#[must_use]
pub fn rail_avatar(stops: [Color32; 3]) -> (Color32, Color32) {
    let [_, middle, _] = stops;
    (WHITE, readable(middle, &[WHITE], 4.5, Mode::Light))
}

/// The gradient text sits on (a primary button, a chip) and its ink: the stops darkened together in steps of 2 % until
/// white reads at 4.5:1 on every one, as long as that takes no more than 40 % (Electric 14 %, Aurora 30 %); otherwise
/// the stops unchanged with the ink that reads better across them (Citrus).
#[must_use]
pub fn text_gradient(stops: [Color32; 3]) -> ([Color32; 3], Color32) {
    // 20 steps of 2 %: at most 40 %.
    const STEP: f32 = 0.02;
    const STEPS: u8 = 20;
    (0..=STEPS)
        .map(|step| stops.map(|stop| mix(stop, BLACK, f32::from(step) * STEP)))
        .find(|darkened| darkened.iter().all(|stop| contrast(WHITE, *stop) >= 4.5))
        .map_or((stops, ink_over(&stops)), |darkened| (darkened, WHITE))
}

/// The rail's stops and the ink for its icons: each stop darkened on its own, in steps of 2 %, only as far as white
/// needs for 3:1 (WCAG AA for a graphical object) at that place, as long as no stop takes more than 25 %; otherwise
/// the stops unchanged with the ink that reads better across them (Citrus).
#[must_use]
pub fn rail_stops(stops: [Color32; 3]) -> ([Color32; 3], Color32) {
    // 12 steps of 2 %: the last one within 25 %.
    const STEP: f32 = 0.02;
    const STEPS: u8 = 12;
    let darkened = stops.map(|stop| (0..=STEPS).map(|step| mix(stop, BLACK, f32::from(step) * STEP)).find(|colour| contrast(WHITE, *colour) >= 3.0));
    match darkened {
        [Some(first), Some(middle), Some(last)] => ([first, middle, last], WHITE),
        _ => (stops, ink_over(&stops)),
    }
}

/// The ink for an icon or text that runs across every colour in `behind` (a gradient's stops): white or the deep
/// ink, whichever reads better on the colour it reads worst on.
#[must_use]
pub fn ink_over(behind: &[Color32]) -> Color32 {
    let worst = |ink: Color32| behind.iter().map(|colour| contrast(ink, *colour)).fold(f32::INFINITY, f32::min);
    if worst(WHITE) >= worst(DEEP_INK) { WHITE } else { DEEP_INK }
}

/// The colour the owner typed as `#rrggbb` or `rrggbb`, space around it dropped; `None` for anything else.
#[must_use]
pub fn typed(text: &str) -> Option<Color32> {
    let text = text.trim();
    let Rgb(r, g, b) = if text.starts_with('#') { Rgb::from_hex(text)? } else { Rgb::from_hex(&format!("#{text}"))? };
    Some(Color32::from_rgb(r, g, b))
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
