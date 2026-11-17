// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! InboxCraft's own look: the colour tokens, distances, sizes and font tokens (`docs/design.md`).
//!
//! Every colour on screen is a token of a [`Palette`]: one fixed table in two modes, dark and light ([`Palette::of`]),
//! with the values of the design file. Text tokens reach 4.5:1 on every ground they stand on, and the edges and marks
//! that carry meaning 3:1; the tests check every pair. This file is the only place that names a colour; a test refuses
//! colour constants anywhere else in the crate. Two values are translucent: [`Palette::scrim`] and [`FLOAT_SHADOW`].
//!
//! Every text style is a font token of [`Fonts`]: a weight of the [`Typeface`] the owner chose (Figtree, replacing
//! egui's Ubuntu Light, or Inter; both bundled) or the monospaced face, at a size the [`TextScale`] multiplies. The icon
//! fonts are [`IconFont`]s. This file is also the only place that names a font.
//!
//! The owner's choices arrive as the engine's [`Appearance`] (the `ui.*.set` commands); [`Look`] is what of it the
//! tokens need.

use egui::epaint::Shadow;
use egui::{Color32, CornerRadius, FontFamily, FontId, Stroke, Style, TextStyle, Visuals, vec2};

use inboxcraft_engine::appearance::Appearance;
pub use inboxcraft_engine::appearance::{ThemeMode as ModePreference, Typeface};

use crate::iconset::IconSet;
use crate::motion::{self, MOTION, Motion};

/// Light or dark: the mode shown now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Dark text on light grounds.
    Light,
    /// Light text on dark grounds.
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

/// The colour tokens of one mode (`docs/design.md`, "Colour"). A view names tokens, never colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    /// The window.
    pub bg: Color32,
    /// Panes that hold content; what floats (menus, dialogs).
    pub surface: Color32,
    /// The index column, the command line's bar, the status line.
    pub panel: Color32,
    /// The command field, text fields, code, the composer's body.
    pub term: Color32,
    /// Structural hairlines: pane edges, control boxes, tags.
    pub line: Color32,
    /// Quieter hairlines inside a pane, between rows.
    pub panel_line: Color32,
    /// Hover on `bg` and `panel`.
    pub grid: Color32,
    /// Hover on `surface` and in floats; the current row; the chosen segment; avatars.
    pub stage: Color32,
    /// Text.
    pub ink: Color32,
    /// Secondary text, icons, key hints, placeholders, disabled text, edges with a meaning.
    pub muted: Color32,
    /// Decoration only: separators, inactive index numbers, a disabled control's glyph.
    pub dim: Color32,
    /// The background characters.
    pub faint: Color32,
    /// The accent as a fill, only under [`Palette::on_cyan`] text.
    pub cyan: Color32,
    /// Text on a cyan fill.
    pub on_cyan: Color32,
    /// The accent as text: section numbers, links, counts, the prompt.
    pub cyan_text: Color32,
    /// The accent as an edge or small mark: focus, the 2 px bar, the unread square, the caret, the now line.
    pub cyan_edge: Color32,
    /// Accent wash: selected text, a badge's ground.
    pub cyan_muted: Color32,
    /// Errors, removed, delete.
    pub minus: Color32,
    /// Passed, added.
    pub plus: Color32,
    /// Waiting, snoozed.
    pub warn: Color32,
    /// Behind a dialog: translucent.
    pub scrim: Color32,
    /// One bar colour per account, repeating from the seventh ([`Theme::account_colour`]).
    pub accounts: [Color32; 6],
}

/// The dark mode's tokens.
pub(crate) const DARK: Palette = Palette {
    bg: rgb(0x0E0F10),
    surface: rgb(0x151617),
    panel: rgb(0x101112),
    term: rgb(0x0A0B0C),
    line: rgb(0x2A2D2F),
    panel_line: rgb(0x202224),
    grid: rgb(0x17191A),
    stage: rgb(0x212427),
    ink: rgb(0xD4D4D4),
    muted: rgb(0x8B9196),
    dim: rgb(0x4A4F53),
    faint: rgb(0x2B2E30),
    cyan: rgb(0x22A8E0),
    on_cyan: rgb(0x0E0F10),
    cyan_text: rgb(0x22A8E0),
    cyan_edge: rgb(0x22A8E0),
    cyan_muted: rgb(0x00354C),
    minus: rgb(0xC98080),
    plus: rgb(0x6AB26B),
    warn: rgb(0xEEC448),
    scrim: Color32::from_rgba_unmultiplied_const(0x00, 0x00, 0x00, 0x99),
    accounts: [rgb(0x9DAF19), rgb(0xA359B9), rgb(0xC05928), rgb(0x547FFA), rgb(0xF55BB8), rgb(0x64DABF)],
};

/// The light mode's tokens.
pub(crate) const LIGHT: Palette = Palette {
    bg: rgb(0xF5F5F2),
    surface: rgb(0xFFFFFF),
    panel: rgb(0xF5F5F2),
    term: rgb(0xFFFFFF),
    line: rgb(0xD6D7D3),
    panel_line: rgb(0xE2E3DF),
    grid: rgb(0xECECE8),
    stage: rgb(0xDEDFDB),
    ink: rgb(0x191919),
    muted: rgb(0x5A6065),
    dim: rgb(0x9EA3A6),
    faint: rgb(0xCFD1CD),
    cyan: rgb(0x22A8E0),
    on_cyan: rgb(0x0E0F10),
    cyan_text: rgb(0x075E88),
    cyan_edge: rgb(0x0B79AD),
    cyan_muted: rgb(0xD3EEF9),
    minus: rgb(0x9A3A36),
    plus: rgb(0x246628),
    warn: rgb(0x6E5400),
    scrim: Color32::from_rgba_unmultiplied_const(0x19, 0x19, 0x19, 0x55),
    accounts: [rgb(0x778224), rgb(0xB54ED5), rgb(0xD84F05), rgb(0x163E8B), rgb(0x7C1457), rgb(0x368977)],
};

impl Palette {
    /// The tokens of `mode`.
    #[must_use]
    pub const fn of(mode: Mode) -> &'static Palette {
        match mode {
            Mode::Dark => &DARK,
            Mode::Light => &LIGHT,
        }
    }
}

/// The distances, six steps on a 4 px grid (`docs/design.md`, "Distances"). They do not grow with the text.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spacing {
    /// 4: between an icon or key chip and its word.
    pub s1: f32,
    /// 8: between the parts of one control; between a label and its number.
    pub s2: f32,
    /// 12: padding inside a control; between a row's columns.
    pub s3: f32,
    /// 16: padding of a pane; the left edge of every row.
    pub s4: f32,
    /// 24: between sections.
    pub s6: f32,
    /// 40: the margin of a page such as Home.
    pub s10: f32,
}

pub(crate) const SPACING: Spacing = Spacing { s1: 4.0, s2: 8.0, s3: 12.0, s4: 16.0, s6: 24.0, s10: 40.0 };

/// The fixed sizes at 100 % text ([`Theme::size`] scales them with the text).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sizes {
    /// A button's and a segmented control's height.
    pub button: f32,
    /// A field's and a select's height.
    pub field: f32,
    /// A menu row's height.
    pub menu_row: f32,
    /// A mail row's height.
    pub mail_row: f32,
    /// A group head's height.
    pub group_head: f32,
    /// A key chip's and a tag's height.
    pub chip: f32,
    /// The avatar's sides: small, middle, large.
    pub avatar: [f32; 3],
    /// The mail list's width.
    pub list: f32,
    /// The scroll bar's width.
    pub scrollbar: f32,
}

pub(crate) const SIZES: Sizes = Sizes {
    button: 32.0,
    field: 36.0,
    menu_row: 32.0,
    mail_row: 56.0,
    group_head: 32.0,
    chip: 20.0,
    avatar: [24.0, 32.0, 40.0],
    list: 440.0,
    scrollbar: 12.0,
};

/// The one shadow, under what floats over the window: 12 px down, 32 px blur, black at 40 %.
pub const FLOAT_SHADOW: Shadow = Shadow { offset: [0, 12], blur: 32, spread: 0, color: Color32::from_rgba_premultiplied(0, 0, 0, 102) };

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

/// The whole look of one mode.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    /// Which mode this is.
    pub mode: Mode,
    /// Its colours.
    pub palette: &'static Palette,
    /// Its distances.
    pub spacing: Spacing,
    /// Its fixed sizes at 100 % ([`Theme::size`] scales one).
    pub sizes: Sizes,
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

const fn rgb(hex: u32) -> Color32 {
    // The masks keep each value in 0..=255, so the casts lose nothing.
    Color32::from_rgb(((hex >> 16) & 0xff) as u8, ((hex >> 8) & 0xff) as u8, (hex & 0xff) as u8)
}

/// The tint a picture is drawn with: none.
pub const NO_TINT: Color32 = rgb(0xFFFFFF);

/// The app icon's own colours (`icons.rs`), the same in every mode so the desktop shows one icon.
pub(crate) mod app_icon {
    use egui::Color32;

    use super::rgb;

    /// The white tile.
    pub(crate) const TILE: Color32 = rgb(0xFFFFFF);
    /// The tile's edge: the deep ink at 16 %, so the white tile holds its shape on a light background.
    pub(crate) const EDGE: Color32 = Color32::from_rgba_premultiplied(1, 1, 2, 41);
    /// The gradient of the mark, first to last stop.
    pub(crate) const GRADIENT: [Color32; 3] = [rgb(0x2F6BFF), rgb(0x8A3FFC), rgb(0xFF2D95)];
    /// Where the gradient starts and ends on the icon's square, in units.
    pub(crate) const GRADIENT_LINE: [[f32; 2]; 2] = [[48.0, 60.0], [208.0, 200.0]];
}

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

/// What the tokens take from the owner's choices besides the mode: the text scale, the typeface and the icon set.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Look {
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
            text_scale: TextScale::from_percent(appearance.text_scale).unwrap_or_default(),
            typeface: appearance.font,
            icons: IconSet::of(appearance.icons),
        }
    }
}

impl Theme {
    /// The look of `mode` at `text_scale`, in Figtree with Phosphor icons.
    #[must_use]
    pub fn new(mode: Mode, text_scale: TextScale) -> Self {
        let typeface = Typeface::default();
        Self {
            mode,
            palette: Palette::of(mode),
            spacing: SPACING,
            sizes: SIZES,
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
        Self::new(mode, look.text_scale).with_typeface(look.typeface).with_icons(look.icons)
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

    /// The colour of the account with this index, counted from 0, repeating after the sixth.
    #[must_use]
    pub fn account_colour(&self, index: u64) -> Color32 {
        let colours = &self.palette.accounts;
        let len = u64::try_from(colours.len()).unwrap_or(1).max(1);
        usize::try_from(index % len).ok().and_then(|at| colours.get(at)).copied().unwrap_or(self.palette.muted)
    }

    /// The fixed size `px` (at 100 %) at the text scale, to the nearest even pixel.
    #[must_use]
    pub fn size(&self, px: f32) -> f32 {
        (px * self.text_scale.factor() / 2.0).round() * 2.0
    }

    /// egui's colours for this theme: flat, square, no shadow of egui's own.
    #[must_use]
    pub fn visuals(&self) -> Visuals {
        let p = self.palette;
        let mut visuals = match self.mode {
            Mode::Light => Visuals::light(),
            Mode::Dark => Visuals::dark(),
        };
        let square = CornerRadius::ZERO;
        visuals.override_text_color = Some(p.ink);
        visuals.weak_text_color = Some(p.muted);
        visuals.panel_fill = p.bg;
        // Menus, tooltips and popups float over whatever is there, on the solid surface.
        visuals.window_fill = p.surface;
        visuals.window_stroke = Stroke::new(1.0, p.line);
        visuals.window_corner_radius = square;
        visuals.menu_corner_radius = square;
        visuals.window_shadow = Shadow::NONE;
        visuals.popup_shadow = Shadow::NONE;
        visuals.extreme_bg_color = p.term;
        visuals.text_edit_bg_color = Some(p.term);
        visuals.faint_bg_color = p.panel;
        visuals.code_bg_color = p.term;
        visuals.hyperlink_color = p.cyan_text;
        visuals.error_fg_color = p.minus;
        visuals.warn_fg_color = p.warn;
        visuals.selection.bg_fill = p.cyan_muted;
        visuals.selection.stroke = Stroke::new(1.0, p.ink);
        let widgets = &mut visuals.widgets;
        for (state, fill) in [
            (&mut widgets.noninteractive, p.bg),
            (&mut widgets.inactive, p.surface),
            (&mut widgets.hovered, p.stage),
            (&mut widgets.active, p.stage),
            (&mut widgets.open, p.stage),
        ] {
            state.bg_fill = fill;
            state.weak_bg_fill = fill;
            state.bg_stroke = Stroke::new(1.0, p.line);
            state.fg_stroke = Stroke::new(1.0, p.ink);
            state.corner_radius = square;
            state.expansion = 0.0;
        }
        visuals
    }

    /// egui's style for this theme: the colours, the distances and the font tokens.
    #[must_use]
    pub fn style(&self) -> Style {
        let s = &self.spacing;
        let f = &self.fonts;
        let mut style = Style { visuals: self.visuals(), ..Style::default() };
        style.spacing.item_spacing = vec2(s.s2, s.s1);
        style.spacing.button_padding = vec2(s.s2, s.s1);
        style.spacing.window_margin = egui::Margin::same(12);
        // egui's own fade at the ends of a scroll area is a gradient laid over text: neither is allowed here.
        style.spacing.scroll.fade.strength = 0.0;
        // A bar wide enough to see at rest and the full width under the pointer; square, like everything.
        style.spacing.scroll.floating_width = 5.0;
        style.spacing.scroll.bar_width = self.sizes.scrollbar;
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
