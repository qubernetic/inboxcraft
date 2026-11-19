// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! InboxCraft's own look: the colour tokens, distances, sizes and font tokens (`docs/design.md`).
//!
//! Every colour on screen is a token of a [`Palette`]: one fixed table in two modes, dark and light ([`Palette::of`]),
//! with the values of the design file. Text tokens reach 4.5:1 on every ground they stand on, and the edges and marks
//! that carry meaning 3:1; the tests check every pair. This file is the only place that names a colour; a test refuses
//! colour constants anywhere else in the crate. Two values are translucent: [`Palette::scrim`] and [`FLOAT_SHADOW`].
//!
//! Every text style is a type style of [`Fonts`]: IBM Plex Mono for what the app says, IBM Plex Sans for what people
//! wrote, Audiowide for the product names (bundled, in front of egui's own faces), at a size and line height the
//! [`TextScale`] multiplies. The icons are Phosphor Regular, the family [`ICON`]. This file is also the only place
//! that names a font.
//!
//! The owner's choices arrive as the engine's [`Appearance`] (the `ui.*.set` commands); [`Look`] is what of it the
//! tokens need.

use egui::epaint::Shadow;
use egui::{Color32, CornerRadius, FontFamily, FontId, Stroke, Style, TextStyle, Visuals, vec2};

use inboxcraft_engine::appearance::Appearance;
pub use inboxcraft_engine::appearance::ThemeMode as ModePreference;

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
    /// A link's height.
    pub link: f32,
    /// The height of a row's small link, as high as the row's text line.
    pub row_link: f32,
    /// The avatar's sides: small, middle, large.
    pub avatar: [f32; 3],
    /// A badge's height and least width.
    pub badge: f32,
    /// The mail list's width.
    pub list: f32,
    /// The scroll bar's width.
    pub scrollbar: f32,
    /// An icon's side in a control.
    pub icon: f32,
    /// An icon's side as a mark in a row.
    pub mark: f32,
    /// The side of a check box's and a radio button's box.
    pub check: f32,
    /// A pill's height.
    pub pill: f32,
    /// The height of a field's caret, a block as wide as a mono character is high in sevenths (7 × 16).
    pub caret: f32,
    /// A multiline field's least and most height; past the most it scrolls.
    pub multiline: [f32; 2],
    /// A float's title row's height.
    pub float_title: f32,
    /// A recipient suggestion's height.
    pub suggestion: f32,
    /// The height of a row's hover action, a progress line and a check step's line.
    pub strip: f32,
    /// The least height of a settings row.
    pub settings_row: f32,
    /// The reader's rows: the why line, the content bar, a folded message of a thread, an attachment and a folded
    /// quote, in that order.
    pub reader: [f32; 5],
    /// A hairline table's rows: its head, a row of one line and the total.
    pub table: [f32; 3],
    /// The status line's height, and its items' and messages'.
    pub status: f32,
}

pub(crate) const SIZES: Sizes = Sizes {
    button: 32.0,
    field: 36.0,
    menu_row: 32.0,
    mail_row: 56.0,
    group_head: 32.0,
    chip: 20.0,
    link: 24.0,
    row_link: 16.0,
    avatar: [24.0, 32.0, 40.0],
    badge: 16.0,
    list: 440.0,
    scrollbar: 12.0,
    icon: 16.0,
    mark: 14.0,
    check: 16.0,
    pill: 24.0,
    caret: 16.0,
    multiline: [96.0, 240.0],
    float_title: 28.0,
    suggestion: 40.0,
    strip: 24.0,
    settings_row: 56.0,
    reader: [32.0, 40.0, 40.0, 44.0, 28.0],
    table: [28.0, 32.0, 36.0],
    status: 26.0,
};

/// The one shadow, under what floats over the window: 12 px down, 32 px blur, black at 40 %.
pub const FLOAT_SHADOW: Shadow = Shadow { offset: [0, 12], blur: 32, spread: 0, color: Color32::from_rgba_premultiplied(0, 0, 0, 102) };

/// A face of the type: IBM Plex Mono for what the app says, IBM Plex Sans for what people wrote, Audiowide for the
/// product names. Two weights, Regular and SemiBold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Face {
    /// IBM Plex Mono Regular: labels, buttons, counts, dates, the app's own lines.
    Mono,
    /// IBM Plex Mono SemiBold: headings and key chips.
    MonoSemiBold,
    /// IBM Plex Sans Regular: senders, subjects, previews, message text.
    Sans,
    /// IBM Plex Sans SemiBold: an unread sender, emphasis.
    SansSemiBold,
    /// Audiowide: only the names `qubernetic` and `InboxCraft`.
    Brand,
}

impl Face {
    /// The face's own underline, from its `post` table, as shares of the size: how far its middle lies under the
    /// baseline, and how thick it is.
    #[must_use]
    pub const fn underline(self) -> (f32, f32) {
        match self {
            Self::Mono => (0.140, 0.060),
            Self::MonoSemiBold => (0.112, 0.088),
            Self::Sans => (0.095, 0.060),
            Self::SansSemiBold => (0.075, 0.088),
            Self::Brand => (154.0 / 2048.0, 102.0 / 2048.0),
        }
    }

    /// The face's ascent and descent from its `hhea` table, as shares of the size: a design text box puts its
    /// baseline half the leading (line height less their sum) under its top, plus the ascent.
    #[must_use]
    pub const fn extent(self) -> (f32, f32) {
        match self {
            Self::Mono | Self::MonoSemiBold | Self::Sans | Self::SansSemiBold => (1.025, 0.275),
            Self::Brand => (2027.0 / 2048.0, 584.0 / 2048.0),
        }
    }

    /// Every face.
    pub const ALL: [Self; 5] = [Self::Mono, Self::MonoSemiBold, Self::Sans, Self::SansSemiBold, Self::Brand];

    /// The name of the face's font file, which is also its family's name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Mono => "IBMPlexMono-Regular",
            Self::MonoSemiBold => "IBMPlexMono-SemiBold",
            Self::Sans => "IBMPlexSans-Regular",
            Self::SansSemiBold => "IBMPlexSans-SemiBold",
            Self::Brand => "Audiowide-Regular",
        }
    }

    /// Whether the face is IBM Plex Mono, which falls back to egui's monospaced faces.
    #[must_use]
    pub const fn is_mono(self) -> bool {
        matches!(self, Self::Mono | Self::MonoSemiBold)
    }

    /// egui's family of the face: Plex Mono Regular and Plex Sans Regular are egui's monospace and proportional
    /// families, the others a family of their own.
    fn family(self) -> FontFamily {
        match self {
            Self::Mono => FontFamily::Monospace,
            Self::Sans => FontFamily::Proportional,
            Self::MonoSemiBold | Self::SansSemiBold | Self::Brand => FontFamily::Name(self.name().into()),
        }
    }
}

/// A type style: a face, a size and a line height in points, the tracking as a share of the size, and whether the
/// text is set in capitals.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Font {
    /// The face.
    pub face: Face,
    /// The size in points, the text scale applied.
    pub size: f32,
    /// The line height in points, the text scale applied.
    pub line: f32,
    /// The extra room between letters, as a share of the size (an em fraction; the text scale leaves it alone).
    pub tracking: f32,
    /// Whether the text is set in capitals.
    pub caps: bool,
}

impl Font {
    const fn new(face: Face, size: f32, line: f32) -> Self {
        Self { face, size, line, tracking: 0.0, caps: false }
    }

    const fn tracked(self, tracking: f32) -> Self {
        Self { tracking, ..self }
    }

    const fn capitals(self) -> Self {
        Self { caps: true, ..self }
    }

    /// egui's font for this style.
    #[must_use]
    pub fn id(self) -> FontId {
        FontId::new(self.size, self.face.family())
    }

    /// The text format of this style in `colour`: its font, its line height and its tracking in points.
    #[must_use]
    pub fn format(self, colour: Color32) -> egui::TextFormat {
        egui::TextFormat {
            font_id: self.id(),
            line_height: Some(self.line),
            extra_letter_spacing: self.tracking * self.size,
            color: colour,
            ..egui::TextFormat::default()
        }
    }

    /// `text` as this style sets it: in capitals if it asks for them.
    #[must_use]
    pub fn text(self, text: &str) -> String {
        if self.caps { text.to_uppercase() } else { text.to_owned() }
    }

    /// `text` as a label's rich text in this style: its font, line height and tracking, in capitals if it asks.
    #[must_use]
    pub fn rich(self, text: &str) -> egui::RichText {
        egui::RichText::new(self.text(text)).font(self.id()).line_height(Some(self.line)).extra_letter_spacing(self.tracking * self.size)
    }

    /// The same style in its face's SemiBold weight (Audiowide has one weight and stays as it is).
    #[must_use]
    pub const fn semibold(self) -> Self {
        let face = match self.face {
            Face::Mono | Face::MonoSemiBold => Face::MonoSemiBold,
            Face::Sans | Face::SansSemiBold => Face::SansSemiBold,
            Face::Brand => Face::Brand,
        };
        Self { face, ..self }
    }

    /// The same style at `size` points, its line height kept in proportion (an avatar's initials, the product name on
    /// Home). A size that is not a positive number keeps the style as it is.
    #[must_use]
    pub fn sized(self, size: f32) -> Self {
        if size.is_finite() && size > 0.0 && self.size > 0.0 { self.scaled(size / self.size) } else { self }
    }

    fn scaled(self, factor: f32) -> Self {
        Self { size: self.size * factor, line: self.line * factor, ..self }
    }
}

/// The type styles (`docs/design.md`, "Type"): every text on screen is one of these.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fonts {
    /// Home's headline: Plex Mono 40/48.
    pub display: Font,
    /// The subject in the reader, a page's title: Plex Mono 22/30.
    pub title: Font,
    /// Headings: Plex Mono SemiBold 15/22.
    pub heading: Font,
    /// Section labels, column heads, the index: Plex Mono 11/16 in capitals.
    pub label: Font,
    /// Lines the app prints, counts, dates: Plex Mono 13/20.
    pub mono: Font,
    /// Tags, a row's small links: Plex Mono 11/16.
    pub mono_s: Font,
    /// Key chips: Plex Mono SemiBold 11/14.
    pub key: Font,
    /// Button labels: Plex Mono 13/16.
    pub button: Font,
    /// Text people wrote, in lists and forms: Plex Sans 14/22.
    pub body: Font,
    /// An unread sender, emphasis: Plex Sans SemiBold 14/22.
    pub strong: Font,
    /// A row's second line, help under a field: Plex Sans 12/18.
    pub caption: Font,
    /// The text of a message: Plex Sans 16/26.
    pub read: Font,
    /// The product names, in Audiowide: 22/30 here; Home and About set their own size.
    pub brand: Font,
}

impl Fonts {
    /// A first-level heading a message's writer wrote, as the frames set it: Plex Sans SemiBold 22/29 (people's words),
    /// the title's size on a line a point lower. A deeper one is set in `strong`.
    #[must_use]
    pub fn written_title(&self) -> Font {
        Font { face: Face::SansSemiBold, line: self.title.size * 29.0 / 22.0, ..self.title }
    }

    /// The caption in SemiBold: an event's summary and today's number in the calendar.
    #[must_use]
    pub fn caption_strong(&self) -> Font {
        Font { face: Face::SansSemiBold, ..self.caption }
    }

    /// The title a tenth larger: the calendar's period in its toolbar.
    #[must_use]
    pub fn period(&self) -> Font {
        self.title.sized(self.title.size * 1.1)
    }

    /// The title a tenth smaller: an event's title in its editor.
    #[must_use]
    pub fn event_title(&self) -> Font {
        self.title.sized(self.title.size * 0.9)
    }

    /// An event's title in a timed cell of the calendar's grid: Plex Sans SemiBold 13 on a line of 17.
    #[must_use]
    pub fn event_cell(&self) -> Font {
        Font { face: Face::SansSemiBold, size: self.body.size * 13.0 / 14.0, line: self.body.size * 17.0 / 14.0, ..self.body }
    }

    /// A glyph of the character field: Plex Mono 13.33/15, whose advance is the cell's 8 points; at every text scale,
    /// since the field's cells grow and its dots do not.
    #[must_use]
    pub fn field(&self) -> Font {
        Font { size: 40.0 / 3.0, line: 15.0, ..self.mono }
    }

    /// A glyph of the emblem: Plex Mono 12/12.5, at the emblem's one size.
    #[must_use]
    pub fn emblem(&self) -> Font {
        Font { size: 12.0, line: 12.5, ..self.mono }
    }

    /// The mark of a ticked check box, `×`: Plex Mono SemiBold 14/16.
    #[must_use]
    pub fn check_mark(&self) -> Font {
        Font { face: Face::MonoSemiBold, size: self.button.size * 14.0 / 13.0, ..self.button }
    }

    /// The initials on an avatar `side` points square, in `strong`.
    #[must_use]
    pub fn initials(&self, side: f32) -> Font {
        self.strong.sized(side * 0.4)
    }

    /// Every style.
    #[must_use]
    pub fn all(&self) -> Vec<Font> {
        vec![
            self.display,
            self.title,
            self.heading,
            self.label,
            self.mono,
            self.mono_s,
            self.key,
            self.button,
            self.body,
            self.strong,
            self.caption,
            self.read,
            self.brand,
        ]
    }

    fn map(self, f: impl Fn(Font) -> Font) -> Self {
        Self {
            display: f(self.display),
            title: f(self.title),
            heading: f(self.heading),
            label: f(self.label),
            mono: f(self.mono),
            mono_s: f(self.mono_s),
            key: f(self.key),
            button: f(self.button),
            body: f(self.body),
            strong: f(self.strong),
            caption: f(self.caption),
            read: f(self.read),
            brand: f(self.brand),
        }
    }

    /// The styles at 100 %, at `scale`: size and line height scale, tracking does not.
    #[must_use]
    pub fn of(scale: TextScale) -> Self {
        FONTS.map(|font| font.scaled(scale.factor()))
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
    /// Its font tokens at the text scale.
    pub fonts: Fonts,
    /// How long its motion takes ([`crate::motion`]).
    pub motion: Motion,
}

const fn rgb(hex: u32) -> Color32 {
    // The masks keep each value in 0..=255, so the casts lose nothing.
    Color32::from_rgb(((hex >> 16) & 0xff) as u8, ((hex >> 8) & 0xff) as u8, (hex & 0xff) as u8)
}

/// The tint a picture is drawn with: none.
pub const NO_TINT: Color32 = rgb(0xFFFFFF);

/// The type styles at 100 % (`docs/design.md`, "Type").
const FONTS: Fonts = Fonts {
    display: Font::new(Face::Mono, 40.0, 48.0).tracked(-0.01),
    title: Font::new(Face::Mono, 22.0, 30.0),
    heading: Font::new(Face::MonoSemiBold, 15.0, 22.0),
    label: Font::new(Face::Mono, 11.0, 16.0).tracked(0.08).capitals(),
    mono: Font::new(Face::Mono, 13.0, 20.0),
    mono_s: Font::new(Face::Mono, 11.0, 16.0),
    key: Font::new(Face::MonoSemiBold, 11.0, 14.0),
    button: Font::new(Face::Mono, 13.0, 16.0),
    body: Font::new(Face::Sans, 14.0, 22.0),
    strong: Font::new(Face::SansSemiBold, 14.0, 22.0),
    caption: Font::new(Face::Sans, 12.0, 18.0),
    read: Font::new(Face::Sans, 16.0, 26.0),
    brand: Font::new(Face::Brand, 22.0, 30.0),
};

/// The font files of the faces (SIL Open Font Licence 1.1; the licence texts are the `OFL.txt` files beside them).
const TYPEFACES: [(Face, &[u8]); 5] = [
    (Face::Mono, include_bytes!("../../../assets/fonts/plex-mono/IBMPlexMono-Regular.ttf")),
    (Face::MonoSemiBold, include_bytes!("../../../assets/fonts/plex-mono/IBMPlexMono-SemiBold.ttf")),
    (Face::Sans, include_bytes!("../../../assets/fonts/plex-sans/IBMPlexSans-Regular.ttf")),
    (Face::SansSemiBold, include_bytes!("../../../assets/fonts/plex-sans/IBMPlexSans-SemiBold.ttf")),
    (Face::Brand, include_bytes!("../../../assets/fonts/audiowide/Audiowide-Regular.ttf")),
];

/// The name of the icon family: Phosphor Regular (MIT, `assets/icons/phosphor/LICENSE`), the one icon font.
pub const ICON: &str = "Phosphor";

/// The icon font file.
const ICON_FONT: &[u8] = include_bytes!("../../../assets/icons/phosphor/Phosphor.ttf");

/// The icon family at `size` points: 16 in controls, 14 for a mark in a row.
#[must_use]
pub fn icon_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(ICON.into()))
}

/// egui's fonts with IBM Plex in front: each face is first in its family, with egui's bundled faces behind it for
/// the signs it does not hold (Hack, then Ubuntu Light behind Plex Mono; Ubuntu Light, then Hack behind Plex Sans and
/// Audiowide; egui's icon font behind all). The icon family is Phosphor alone.
#[must_use]
pub fn font_definitions() -> egui::FontDefinitions {
    let mut definitions = egui::FontDefinitions::default();
    let bundled = |family: &FontFamily| definitions.families.get(family).cloned().unwrap_or_default();
    let (mut proportional, monospace) = (bundled(&FontFamily::Proportional), bundled(&FontFamily::Monospace));
    // Hack holds arrows, shapes and keys that Ubuntu Light and Plex Sans do not: behind Ubuntu Light, before the icons.
    let missing: Vec<String> = monospace.iter().filter(|font| !proportional.contains(font)).cloned().collect();
    let at = proportional.iter().position(|name| name == "Ubuntu-Light").map_or(0, |at| at + 1);
    proportional.splice(at..at, missing);
    for (face, data) in TYPEFACES {
        definitions.font_data.insert(face.name().to_owned(), std::sync::Arc::new(egui::FontData::from_static(data)));
        let behind = if face.is_mono() { &monospace } else { &proportional };
        let fonts = std::iter::once(face.name().to_owned()).chain(behind.iter().cloned()).collect();
        definitions.families.insert(face.family(), fonts);
    }
    // The icons are drawn as their outlines are, as the design draws its SVGs: hinting would snap them half a pixel up.
    definitions.font_data.insert(
        ICON.to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(ICON_FONT).tweak(egui::FontTweak { hinting: Some(false), ..egui::FontTweak::default() })),
    );
    definitions.families.insert(FontFamily::Name(ICON.into()), vec![ICON.to_owned()]);
    definitions
}

/// Installs [`font_definitions`] in `ctx`, from its next frame.
pub fn install_fonts(ctx: &egui::Context) {
    ctx.set_fonts(font_definitions());
}

/// The interface's size comes from the text-size setting alone ([`TextScale`]): egui's own keyboard zoom (Ctrl+Plus,
/// Ctrl+Minus, Ctrl+0, which is Home's key) is switched off, and a zoom factor `ctx` came with is set back to 1, so no
/// start comes up zoomed with no key to undo it.
pub fn size_from_the_setting_alone(ctx: &egui::Context) {
    ctx.options_mut(|options| options.zoom_with_keyboard = false);
    ctx.set_zoom_factor(1.0);
}

/// One face of a font file found on the computer, for the signs the app's own fonts lack.
#[derive(Clone)]
pub struct Fallback {
    /// Its name among the fonts.
    pub name: String,
    /// The whole font file.
    pub data: Vec<u8>,
    /// The face's index in the file.
    pub index: u32,
}

/// One sign of each script the app's fonts may lack: what the start looks for on the computer.
pub const FALLBACK_PROBES: [(&str, char); 8] =
    [("han", '日'), ("kana", 'あ'), ("hangul", '한'), ("arabic", 'ع'), ("hebrew", 'א'), ("thai", 'ท'), ("devanagari", 'अ'), ("emoji", '🙂')];

/// [`font_definitions`] with `fallbacks` behind every text family: IBM Plex first, then egui's bundled faces, then
/// these, in their order. The icon family keeps Phosphor alone. A fallback egui could not read as a font at its index
/// (egui panics on one) and one whose name is taken are left out. Copies the bytes: [`font_definitions_from`] takes
/// them.
#[must_use]
pub fn font_definitions_with(fallbacks: &[Fallback]) -> egui::FontDefinitions {
    font_definitions_from(fallbacks.to_vec())
}

/// [`font_definitions_with`] `fallbacks`, whose bytes move into egui's font data without a copy.
#[must_use]
pub fn font_definitions_from(fallbacks: Vec<Fallback>) -> egui::FontDefinitions {
    let mut definitions = font_definitions();
    let icon = FontFamily::Name(ICON.into());
    for Fallback { name, data, index } in fallbacks {
        if definitions.font_data.contains_key(&name) || !readable(&data, index) {
            continue;
        }
        definitions.font_data.insert(name.clone(), std::sync::Arc::new(egui::FontData::from_blob(std::sync::Arc::new(data), index)));
        for (family, fonts) in &mut definitions.families {
            if *family != icon {
                fonts.push(name.clone());
            }
        }
    }
    definitions
}

/// Installs [`font_definitions_from`] `fallbacks` in `ctx`, asks for one frame to draw with them, and gives the
/// definitions back for another window: they share the bytes.
pub fn install_fallbacks(ctx: &egui::Context, fallbacks: Vec<Fallback>) -> egui::FontDefinitions {
    let definitions = font_definitions_from(fallbacks);
    ctx.set_fonts(definitions.clone());
    ctx.request_repaint();
    definitions
}

/// Whether egui can read `data` as a font with a face at `index`: the check egui makes before it panics.
fn readable(data: &[u8], index: u32) -> bool {
    skrifa::FontRef::from_index(data, index).is_ok()
}

/// The `probes` whose sign no font of a text family in [`font_definitions`] maps: what the computer's fonts are
/// searched for.
#[must_use]
pub fn missing_probes(probes: &[(&'static str, char)]) -> Vec<(&'static str, char)> {
    use skrifa::MetadataProvider as _;
    let definitions = font_definitions();
    let icon = FontFamily::Name(ICON.into());
    let names: std::collections::BTreeSet<&String> =
        definitions.families.iter().filter(|(family, _)| **family != icon).flat_map(|(_, fonts)| fonts).collect();
    let charmaps: Vec<skrifa::charmap::Charmap<'_>> = names
        .into_iter()
        .filter_map(|name| definitions.font_data.get(name))
        .filter_map(|data| skrifa::FontRef::from_index(data.bytes(), data.index).ok())
        .map(|font| font.charmap())
        .collect();
    probes.iter().copied().filter(|(_, sign)| !charmaps.iter().any(|charmap| charmap.map(*sign).is_some())).collect()
}

/// What the tokens take from the owner's choices besides the mode: the text scale.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Look {
    /// The text scale.
    pub text_scale: TextScale,
}

impl Look {
    /// What the tokens take from `appearance`; a text scale this front end does not have is 100 %.
    #[must_use]
    pub fn of(appearance: &Appearance) -> Self {
        Self { text_scale: TextScale::from_percent(appearance.text_scale).unwrap_or_default() }
    }
}

impl Theme {
    /// The look of `mode` at `text_scale`.
    #[must_use]
    pub fn new(mode: Mode, text_scale: TextScale) -> Self {
        Self { mode, palette: Palette::of(mode), spacing: SPACING, sizes: SIZES, text_scale, fonts: Fonts::of(text_scale), motion: MOTION }
    }

    /// The look of `mode` with everything else from `look`.
    #[must_use]
    pub fn of(mode: Mode, look: &Look) -> Self {
        Self::new(mode, look.text_scale)
    }

    /// The same look with every font token at `scale`.
    #[must_use]
    pub fn with_text_scale(self, scale: TextScale) -> Self {
        Self { text_scale: scale, fonts: Fonts::of(scale), ..self }
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

    /// The fixed heights at the text scale (each through [`Theme::size`]), and the avatars' sides with them; the
    /// widths and the scroll bar stay as they are.
    #[must_use]
    pub fn boxes(&self) -> Sizes {
        let z = self.sizes;
        Sizes {
            button: self.size(z.button),
            field: self.size(z.field),
            menu_row: self.size(z.menu_row),
            mail_row: self.size(z.mail_row),
            group_head: self.size(z.group_head),
            chip: self.size(z.chip),
            link: self.size(z.link),
            row_link: self.size(z.row_link),
            icon: self.size(z.icon),
            mark: self.size(z.mark),
            check: self.size(z.check),
            pill: self.size(z.pill),
            avatar: z.avatar.map(|side| self.size(side)),
            badge: self.size(z.badge),
            caret: self.size(z.caret),
            multiline: z.multiline.map(|height| self.size(height)),
            float_title: self.size(z.float_title),
            suggestion: self.size(z.suggestion),
            strip: self.size(z.strip),
            settings_row: self.size(z.settings_row),
            reader: z.reader.map(|height| self.size(height)),
            table: z.table.map(|height| self.size(height)),
            status: self.size(z.status),
            ..z
        }
    }

    /// The side of an icon in a control at the text scale.
    #[must_use]
    pub fn icon_side(&self) -> f32 {
        self.boxes().icon
    }

    /// The side of an icon as a mark in a row's line (a flag, a pin, a paperclip) at the text scale.
    #[must_use]
    pub fn mark_side(&self) -> f32 {
        self.boxes().mark
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
        // Glyph coverage becomes alpha as it is in both modes, as the design draws text: egui's dark default would
        // thicken every glyph edge.
        visuals.text_options.color_transfer_function = egui::epaint::FontColorTransferFunction::Off;
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
        // The scroll bar beside the content, painted by the component; no fade laid over text.
        style.spacing.scroll = crate::component::scroll::style(self);
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
