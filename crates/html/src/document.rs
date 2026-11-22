// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What [`crate::display`] gives: display blocks, links and pictures, nothing that runs.

/// Whether remote pictures may be shown. Blocked unless the owner asks.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RemoteContent {
    /// Remote pictures are [`ImageSource::Blocked`]: nothing in the document names a remote address
    /// a front end could fetch by mistake.
    #[default]
    Blocked,
    /// The owner gave consent for this message: remote pictures are [`ImageSource::Remote`].
    Allowed,
}

/// How [`crate::display`] reads a message.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Options {
    /// The consent flag for remote pictures; blocked by default.
    pub remote_content: RemoteContent,
}

/// A message's HTML as display blocks.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Document {
    /// The blocks in reading order.
    pub blocks: Vec<Block>,
    /// Every link, in the order met; [`Span::link`] and [`Image::link`] index into this.
    pub links: Vec<Link>,
    /// The `cid:` pictures the message refers to (Content-IDs without the angle brackets), each
    /// once, in the order met: the parts a front end looks up in the message.
    pub inline_images: Vec<String>,
    /// How many remote pictures were blocked; more than none means a front end can offer to load
    /// them.
    pub remote_blocked: usize,
    /// Whether a cap cut something: the input, the blocks, the links or the pictures.
    pub truncated: bool,
}

/// One display block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    /// A run of text.
    Text {
        /// What kind of text block.
        kind: TextKind,
        /// How many quotes (`blockquote`) it sits in.
        quote: u8,
        /// Its alignment and the background it stands on.
        look: Look,
        /// The text, in runs of one look; a line break is `\n` inside a span.
        spans: Vec<Span>,
    },
    /// A horizontal rule.
    Rule {
        /// How many quotes it sits in.
        quote: u8,
    },
    /// A picture.
    Image {
        /// How many quotes it sits in.
        quote: u8,
        /// Its alignment and the background it stands on.
        look: Look,
        /// The picture.
        image: Image,
    },
    /// A table with more than one column (a table of one column is laid out as its cells' blocks).
    Table {
        /// How many quotes it sits in.
        quote: u8,
        /// Where it stands (`align`, `margin: 0 auto`) and the background it stands on.
        look: Look,
        /// The width it asks for (`width`), if any.
        width: Option<Width>,
        /// The rows, each with its cells; rows may differ in length.
        rows: Vec<Vec<Cell>>,
    },
}

impl Block {
    /// The text of a text block, its spans joined; empty for every other block.
    #[must_use]
    pub fn text(&self) -> String {
        match self {
            Block::Text { spans, .. } => spans.iter().map(|s| s.text.as_str()).collect(),
            _ => String::new(),
        }
    }
}

/// What kind of text block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextKind {
    /// A paragraph.
    Paragraph,
    /// A heading, level 1 to 6.
    Heading(u8),
    /// A list item.
    ListItem {
        /// How many lists it sits in, 1 for a top-level list.
        depth: u8,
        /// Its bullet or number.
        marker: Marker,
    },
    /// Preformatted text: white space as written.
    Preformatted,
}

/// A list item's marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Marker {
    /// An item of an unordered list.
    Bullet,
    /// An item of an ordered list.
    Number(u32),
}

/// A run of text of one look.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Span {
    /// The text, character references decoded, control and bidi override characters removed.
    pub text: String,
    /// Bold (`b`, `strong`, `th`).
    pub bold: bool,
    /// Italic (`i`, `em`, `cite`).
    pub italic: bool,
    /// Underlined (`u`, `ins`).
    pub underline: bool,
    /// Struck through (`s`, `strike`, `del`).
    pub strike: bool,
    /// Monospace (`code`, `tt`, `kbd`, `samp`, `pre`).
    pub code: bool,
    /// The link it belongs to, an index into [`Document::links`].
    pub link: Option<usize>,
}

/// A picture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    /// Its alternative text, perhaps empty.
    pub alt: String,
    /// Where it comes from.
    pub source: ImageSource,
    /// The link it belongs to, an index into [`Document::links`].
    pub link: Option<usize>,
    /// Its width and height in pixels as its tag gives them (both whole numbers, each at most
    /// [`crate::MAX_SIDE`]), so a picture not loaded keeps its place; `None` when the tag does not give both.
    pub size: Option<(u32, u32)>,
}

/// Where a picture comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageSource {
    /// A part of the message, by Content-ID (without the angle brackets).
    Inline(String),
    /// A remote `http` or `https` picture the owner allowed.
    Remote(String),
    /// A remote picture, blocked because the owner has not allowed remote content.
    Blocked,
    /// Anything else (`data:`, `file:`, `javascript:`, a relative address, none at all): never shown.
    Unsupported,
}

/// A link: where it goes, what it says, and whether those disagree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    /// Where it goes.
    pub target: LinkTarget,
    /// Its visible text (at most 1,024 characters), for comparing with the target.
    pub text: String,
    /// The text names a web address, host or mail address other than the target, or the target is
    /// refused while the text looks like an address, or the target or the text is a web address
    /// with a user name (`https://bank.example@evil.example/`): a likely phishing link. The text is
    /// compared as a reader sees it: invisible format characters ignored, the ideographic,
    /// fullwidth and halfwidth full stops read as dots.
    pub mismatch: bool,
    /// The link is styled as a button (`role="button"`, a background with padding, a `<button>`): its words stand
    /// in a block of their own, which a front end draws as a button that still only says where it goes.
    pub button: bool,
    /// The button's own background, when kept.
    pub fill: Option<Paint>,
}

/// Where a link goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkTarget {
    /// An `http` or `https` address, white space and control characters removed.
    Web {
        /// The address as it would be opened.
        url: String,
        /// Its host in lower case, without user name or port: what a front end shows the owner.
        host: String,
    },
    /// A `mailto:` address.
    Mail {
        /// The address as it would be opened.
        url: String,
        /// The mail address it names (before any `?`), percent-decoded.
        address: String,
    },
    /// Any other target (`javascript:`, `data:`, `file:`, relative, fragment, too long): not followed.
    Refused,
}

/// A colour in sRGB.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Rgb {
    /// Red.
    pub r: u8,
    /// Green.
    pub g: u8,
    /// Blue.
    pub b: u8,
}

impl Rgb {
    /// The WCAG 2 contrast ratio of two colours, from 1 to 21.
    #[must_use]
    pub fn contrast(self, other: Rgb) -> f32 {
        let (a, b) = (self.luminance(), other.luminance());
        let (light, dark) = if a >= b { (a, b) } else { (b, a) };
        (light + 0.05) / (dark + 0.05)
    }

    /// The relative luminance (WCAG 2).
    fn luminance(self) -> f32 {
        let channel = |value: u8| {
            let c = f32::from(value) / 255.0;
            if c <= 0.040_45 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * channel(self.r) + 0.7152 * channel(self.g) + 0.0722 * channel(self.b)
    }
}

/// A background the message sets that the fitted view keeps: a coloured one, with the ink its text is set in, which
/// reaches at least 4.5:1 on it ([`crate::look::kept`]). A white, grey or black background, or one no ink can be
/// read on, is never kept: the text stands on the app's own ground.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Paint {
    /// The background.
    pub fill: Rgb,
    /// The text's colour on it: a near black or white.
    pub ink: Rgb,
}

/// How a block's lines stand across its room.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Align {
    /// From the left edge.
    #[default]
    Left,
    /// In the middle.
    Centre,
    /// To the right edge.
    Right,
}

/// What a block keeps of the message's look.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Look {
    /// Its alignment (`text-align`, `align`, `center`).
    pub align: Align,
    /// The background it stands on, when kept.
    pub background: Option<Paint>,
}

/// The width a table or cell asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Width {
    /// Whole pixels, at most [`crate::MAX_SIDE`]; a front end gives it no more than its room.
    Pixels(u32),
    /// A share of the room around it, from 1 to 100 per cent.
    Share(u8),
}

/// A table cell.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Cell {
    /// Whether it is a header cell (`th`).
    pub header: bool,
    /// Its content.
    pub blocks: Vec<Block>,
    /// Its background (its own, its row's or its table's), when kept; its blocks do not repeat it.
    pub background: Option<Paint>,
    /// The width it asks for, if any.
    pub width: Option<Width>,
}
