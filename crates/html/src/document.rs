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
        /// The picture.
        image: Image,
    },
    /// A table with more than one column (a table of one column is laid out as its cells' blocks).
    Table {
        /// How many quotes it sits in.
        quote: u8,
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

/// A table cell.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Cell {
    /// Whether it is a header cell (`th`).
    pub header: bool,
    /// Its content.
    pub blocks: Vec<Block>,
}
