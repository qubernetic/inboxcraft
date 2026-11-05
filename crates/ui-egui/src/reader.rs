// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What the reader shows of the selected message: its thread oldest first, each message's header (short, or in
//! full when asked), its text with the quotes folded or its HTML as safe display blocks, its attachments, and its
//! pictures.
//!
//! Everything comes through the engine's commands (`mail.thread`, `mail.read`, `mail.attachment`, `remote.allow`).
//! The text of a message is data: it is shown and never acted on. Remote pictures stay blocked until the owner
//! allows them for this message once ([`Reader::allow_once`]) or for the sender always ([`Reader::allow_always`]);
//! only then does the document name their addresses and only then does the reader ask its [`Fetch`]. Pictures inside
//! the message (`cid:`) come from the message itself and never from the network.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use inboxcraft_html::{Block, Document, ImageSource, Link, LinkTarget, Options, RemoteContent, display};
use inboxcraft_time::Timestamp;
use serde_json::{Value, json};

use crate::fetch::Fetch;
use crate::mailbox::{Mailbox, parse_date, sender, shown};
use crate::picture::{Decoded, decode};
use crate::quote::{MAX_TEXT, Segment, split};
use crate::save::Save;
use crate::{UiError, tr};

/// The most messages of a thread the reader lists.
pub const MAX_THREAD: usize = 100;

/// The most remote pictures one message may ask for.
pub const MAX_REMOTE: usize = 64;

/// A person in a header: the name (perhaps empty) and the address, both made fit for one line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Person {
    /// The name.
    pub name: String,
    /// The address.
    pub address: String,
}

impl Person {
    /// The name, or the address without one.
    #[must_use]
    pub fn shown(&self) -> &str {
        if self.name.is_empty() { &self.address } else { &self.name }
    }

    /// `Name <address>`, or the address alone.
    #[must_use]
    pub fn full(&self) -> String {
        if self.name.is_empty() { self.address.clone() } else { format!("{} <{}>", self.name, self.address) }
    }
}

/// A message's header.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Header {
    /// The senders.
    pub from: Vec<Person>,
    /// Where replies go, when not to the sender.
    pub reply_to: Vec<Person>,
    /// The recipients.
    pub to: Vec<Person>,
    /// The copies.
    pub cc: Vec<Person>,
    /// The subject.
    pub subject: String,
    /// When it was sent or came in.
    pub date: Option<Timestamp>,
}

/// What the body of a message shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Content {
    /// The body is not kept yet and is being fetched.
    Loading,
    /// The body is not kept and nothing fetches it now (no connection yet).
    NotKept,
    /// The body is too large to keep.
    TooLarge,
    /// The body could not be read.
    Unreadable,
    /// Plain text, quotes folded.
    Text(Vec<Segment>),
    /// HTML as safe display blocks.
    Html(Document),
}

/// An attachment tile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct File {
    /// Its place in the message's list, for `mail.attachment`.
    pub index: u64,
    /// Its file name, made fit for one line.
    pub name: String,
    /// Its media type.
    pub media_type: String,
    /// Its size in bytes, when it could be decoded.
    pub size: Option<u64>,
}

/// One message of the thread, read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opened {
    /// Its id.
    pub id: u64,
    /// Its header.
    pub header: Header,
    /// Its body.
    pub content: Content,
    /// Its attachments.
    pub files: Vec<File>,
    /// Whether its text was cut to [`MAX_TEXT`] bytes.
    pub cut: bool,
    /// Whether the owner lets the sender's remote pictures load always.
    pub sender_allowed: bool,
    html: Option<String>,
}

/// A message of the thread: what the folded row shows, and the message once read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadRow {
    /// Its id.
    pub id: u64,
    /// The sender as the folded row shows it.
    pub sender: String,
    /// The first words of its text.
    pub preview: String,
    /// When it was sent or came in.
    pub date: Option<Timestamp>,
    /// The message, once read.
    pub opened: Option<Opened>,
}

/// A picture of a message.
#[derive(Debug, Clone)]
pub enum PictureState {
    /// Asked for, not here yet.
    Waiting,
    /// Here.
    Ready(Arc<Decoded>),
    /// Failed, with the reason.
    Failed(String),
}

/// The key of an inline picture of message `id`.
#[must_use]
pub fn inline_key(id: u64, content_id: &str) -> String {
    format!("cid:{id}:{content_id}")
}

/// What the line under the reader says while the pointer is on `link`: where it goes, and a warning when its text
/// names another place.
#[must_use]
pub fn link_status(link: &Link) -> String {
    match &link.target {
        LinkTarget::Web { url, host } if link.mismatch => {
            tr!("Careful: the text says “{text}”, but the link goes to {host}: {url}", text = shown(&link.text), host = shown(host), url = shown(url))
        }
        LinkTarget::Web { url, .. } => shown(url),
        LinkTarget::Mail { address, .. } => tr!("Write to {address}", address = shown(address)),
        LinkTarget::Refused => tr!("This link is not followed: it does not go to a web page or a mail address.").to_owned(),
    }
}

/// A size as the attachment tile shows it.
#[must_use]
pub fn size_text(size: Option<u64>) -> String {
    let Some(size) = size else { return tr!("size unknown").to_owned() };
    // Sizes are far below 2^52, where f64 stops being exact; the shown figure is rounded anyway.
    #[expect(clippy::cast_precision_loss, reason = "a rounded figure for people")]
    let (kb, mb) = (size as f64 / 1024.0, size as f64 / (1024.0 * 1024.0));
    match size {
        1 => tr!("1 byte").to_owned(),
        0..1024 => tr!("{count} bytes", count = size),
        1024..1_048_576 => tr!("{count} KB", count = format!("{kb:.0}")),
        _ => tr!("{count} MB", count = format!("{mb:.1}")),
    }
}

/// Base64 as `mail.attachment` writes it, decoded; `None` for anything else.
fn unbase64(text: &str) -> Option<Vec<u8>> {
    let value = |c: u8| match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    };
    let bytes = text.as_bytes();
    if !bytes.len().is_multiple_of(4) {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for (index, chunk) in bytes.chunks(4).enumerate() {
        let last = index.saturating_add(1) == bytes.len() / 4;
        let pad = chunk.iter().rev().take_while(|&&c| c == b'=').count();
        if pad > 2 || (pad > 0 && !last) {
            return None;
        }
        let mut group = 0_u32;
        for &c in chunk.get(..4 - pad)? {
            group = (group << 6) | u32::from(value(c)?);
        }
        group <<= 6 * u32::try_from(pad).ok()?;
        let [_, a, b, c] = group.to_be_bytes();
        out.extend([a, b, c].into_iter().take(3 - pad));
    }
    Some(out)
}

fn people(value: &Value) -> Vec<Person> {
    value
        .as_array()
        .map(|list| {
            list.iter()
                .map(|person| Person {
                    name: shown(person.get("name").and_then(Value::as_str).unwrap_or_default()),
                    address: shown(person.get("address").and_then(Value::as_str).unwrap_or_default()),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The remote addresses of the pictures in `blocks`, tables included.
fn remote_pictures<'a>(blocks: &'a [Block], out: &mut Vec<&'a str>) {
    for block in blocks {
        match block {
            Block::Image { image, .. } => {
                if let ImageSource::Remote(url) = &image.source {
                    out.push(url);
                }
            }
            Block::Table { rows, .. } => rows.iter().flatten().for_each(|cell| remote_pictures(&cell.blocks, out)),
            Block::Text { .. } | Block::Rule { .. } => {}
        }
    }
}

/// The reader's state.
pub struct Reader {
    open: Option<u64>,
    thread: Vec<ThreadRow>,
    expanded: BTreeSet<u64>,
    details: BTreeSet<u64>,
    quotes: BTreeSet<u64>,
    once: BTreeSet<u64>,
    full_width: bool,
    fetch: Box<dyn Fetch>,
    save: Box<dyn Save>,
    pictures: BTreeMap<String, PictureState>,
    notice: Option<String>,
}

impl std::fmt::Debug for Reader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Reader")
            .field("open", &self.open)
            .field("thread", &self.thread.len())
            .field("full_width", &self.full_width)
            .finish_non_exhaustive()
    }
}

impl Reader {
    /// A reader that fetches remote pictures through `fetch` and saves attachments through `save`.
    #[must_use]
    pub fn new(fetch: Box<dyn Fetch>, save: Box<dyn Save>) -> Self {
        Self {
            open: None,
            thread: Vec::new(),
            expanded: BTreeSet::new(),
            details: BTreeSet::new(),
            quotes: BTreeSet::new(),
            once: BTreeSet::new(),
            full_width: false,
            fetch,
            save,
            pictures: BTreeMap::new(),
            notice: None,
        }
    }

    /// Opens message `id` with its thread, or closes the reader (`None`). The message itself is expanded and read;
    /// the rest of the thread is folded. Leaving a message forgets what was allowed once.
    pub fn open(&mut self, mailbox: &mut Mailbox, id: Option<u64>) {
        self.open = id;
        self.thread.clear();
        self.expanded.clear();
        self.details.clear();
        self.quotes.clear();
        self.once.clear();
        self.notice = None;
        let Some(id) = id else {
            self.full_width = false;
            return;
        };
        match mailbox.run("mail.thread", &json!({ "id": id })) {
            Ok(answer) => {
                let messages = answer.get("messages").and_then(Value::as_array).cloned().unwrap_or_default();
                let skip = messages.len().saturating_sub(MAX_THREAD);
                self.thread = messages
                    .iter()
                    .skip(skip)
                    .map(|message| ThreadRow {
                        id: message.get("id").and_then(Value::as_u64).unwrap_or(0),
                        sender: sender(message.get("from").unwrap_or(&Value::Null)),
                        preview: shown(message.get("preview").and_then(Value::as_str).unwrap_or_default()),
                        date: message.get("date").and_then(Value::as_str).and_then(parse_date),
                        opened: None,
                    })
                    .collect();
            }
            Err(error) => self.notice = Some(error.to_string()),
        }
        if self.thread.iter().all(|row| row.id != id) {
            self.thread.push(ThreadRow { id, sender: String::new(), preview: String::new(), date: None, opened: None });
        }
        self.expand(mailbox, id);
    }

    /// Unfolds message `id` of the thread (reading it the first time), or folds it again. The open message stays
    /// unfolded.
    pub fn expand(&mut self, mailbox: &mut Mailbox, id: u64) {
        if self.expanded.contains(&id) {
            if Some(id) != self.open {
                self.expanded.remove(&id);
            }
            return;
        }
        if self.thread.iter().all(|row| row.id != id) {
            return;
        }
        self.expanded.insert(id);
        if self.opened(id).is_none() {
            match self.read(mailbox, id) {
                Ok(opened) => {
                    if let Some(row) = self.thread.iter_mut().find(|row| row.id == id) {
                        row.opened = Some(opened);
                    }
                    self.load_pictures(mailbox, id);
                }
                Err(error) => self.notice = Some(error.to_string()),
            }
        }
    }

    fn read(&self, mailbox: &mut Mailbox, id: u64) -> Result<Opened, UiError> {
        let answer = mailbox.run("mail.read", &json!({ "id": id }))?;
        let message = answer.get("message").ok_or_else(|| UiError::Answer { command: "mail.read", detail: "no message".to_owned() })?;
        let header = Header {
            from: people(message.get("from").unwrap_or(&Value::Null)),
            reply_to: people(message.get("reply_to").unwrap_or(&Value::Null)),
            to: people(message.get("to").unwrap_or(&Value::Null)),
            cc: people(message.get("cc").unwrap_or(&Value::Null)),
            subject: shown(message.get("subject").and_then(Value::as_str).unwrap_or_default()),
            date: message.get("date").and_then(Value::as_str).and_then(parse_date),
        };
        let body = message.get("body").filter(|body| !body.is_null());
        let text = body.and_then(|body| body.get("text")).and_then(Value::as_str);
        let html = body.and_then(|body| body.get("html")).and_then(Value::as_str).map(|html| crate::quote::cut(html, 4 * MAX_TEXT).to_owned());
        let sender_allowed = answer.get("remote_allowed").and_then(Value::as_bool).unwrap_or(false);
        let mut opened = Opened { id, header, content: Content::NotKept, files: Vec::new(), cut: false, sender_allowed, html: None };
        opened.content = match body {
            None if answer.get("fetching").and_then(Value::as_bool) == Some(true) => Content::Loading,
            None if answer.get("body_error").and_then(Value::as_str) == Some("too_large") => Content::TooLarge,
            None => Content::NotKept,
            Some(body) if body.get("readable").and_then(Value::as_bool) != Some(true) => Content::Unreadable,
            Some(_) => match (&html, text) {
                (Some(_), _) => Content::Html(Document::default()),
                (None, Some(text)) => {
                    opened.cut = text.len() > MAX_TEXT;
                    Content::Text(split(text))
                }
                (None, None) => Content::Text(Vec::new()),
            },
        };
        opened.html = html;
        opened.files = body
            .and_then(|body| body.get("attachments"))
            .and_then(Value::as_array)
            .map(|files| {
                files
                    .iter()
                    .map(|file| File {
                        index: file.get("index").and_then(Value::as_u64).unwrap_or(0),
                        name: shown(file.get("name").and_then(Value::as_str).unwrap_or(tr!("attachment"))),
                        media_type: shown(file.get("type").and_then(Value::as_str).unwrap_or_default()),
                        size: file.get("size").and_then(Value::as_u64),
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(opened)
    }

    /// Lays out the HTML of message `id` with the consent that now holds, and asks for the pictures it names: the
    /// inline ones from the message, the remote ones (only once allowed) through the fetch.
    fn load_pictures(&mut self, mailbox: &mut Mailbox, id: u64) {
        let allowed = self.remote_allowed(id);
        let Some(opened) = self.thread.iter_mut().filter_map(|row| row.opened.as_mut()).find(|opened| opened.id == id) else { return };
        let Some(html) = opened.html.as_deref() else { return };
        let remote_content = if allowed { RemoteContent::Allowed } else { RemoteContent::Blocked };
        let document = display(html, &Options { remote_content });
        let inline = document.inline_images.clone();
        let mut remote = Vec::new();
        remote_pictures(&document.blocks, &mut remote);
        let remote: Vec<String> = remote.into_iter().take(MAX_REMOTE).map(str::to_owned).collect();
        opened.content = Content::Html(document);
        for content_id in inline {
            let key = inline_key(id, &content_id);
            if self.pictures.contains_key(&key) {
                continue;
            }
            let state = match mailbox.run("mail.attachment", &json!({ "id": id, "content_id": content_id })) {
                Ok(answer) => match answer.get("data").and_then(Value::as_str).and_then(unbase64) {
                    Some(bytes) => {
                        decode(&bytes).map_or_else(|error| PictureState::Failed(error.to_string()), |picture| PictureState::Ready(Arc::new(picture)))
                    }
                    None => PictureState::Failed(tr!("The picture could not be read.").to_owned()),
                },
                Err(error) => PictureState::Failed(error.to_string()),
            };
            self.pictures.insert(key, state);
        }
        for url in remote {
            if !self.pictures.contains_key(&url) {
                self.pictures.insert(url.clone(), PictureState::Waiting);
                self.fetch.start(&url);
            }
        }
    }

    /// Whether the remote pictures of message `id` may load: allowed once for it, or always for its sender.
    #[must_use]
    pub fn remote_allowed(&self, id: u64) -> bool {
        self.once.contains(&id) || self.opened(id).is_some_and(|opened| opened.sender_allowed)
    }

    /// Lets the remote pictures of message `id` load, this time only.
    pub fn allow_once(&mut self, mailbox: &mut Mailbox, id: u64) {
        if self.opened(id).is_some() {
            self.once.insert(id);
            self.load_pictures(mailbox, id);
        }
    }

    /// Lets the remote pictures of the sender of message `id` load always (`remote.allow`), and loads them in every
    /// message of the thread from that sender.
    pub fn allow_always(&mut self, mailbox: &mut Mailbox, id: u64) {
        let Some(address) = self.opened(id).and_then(|opened| opened.header.from.first()).map(|person| person.address.clone()) else { return };
        if let Err(error) = mailbox.run("remote.allow", &json!({ "address": address })) {
            self.notice = Some(error.to_string());
            return;
        }
        let same: Vec<u64> = self
            .thread
            .iter_mut()
            .filter_map(|row| row.opened.as_mut())
            .filter(|opened| opened.header.from.first().is_some_and(|from| from.address.eq_ignore_ascii_case(&address)))
            .map(|opened| {
                opened.sender_allowed = true;
                opened.id
            })
            .collect();
        for id in same {
            self.load_pictures(mailbox, id);
        }
    }

    /// Saves attachment `index` of message `id` through the save the app handed in; the result shows as a notice.
    pub fn save(&mut self, mailbox: &mut Mailbox, id: u64, index: u64) {
        let saved = mailbox.run("mail.attachment", &json!({ "id": id, "index": index })).map_err(|error| error.to_string()).and_then(|answer| {
            let name = answer.get("name").and_then(Value::as_str).unwrap_or(tr!("attachment")).to_owned();
            let bytes =
                answer.get("data").and_then(Value::as_str).and_then(unbase64).ok_or_else(|| tr!("The attachment could not be read.").to_owned())?;
            self.save.save(&name, &bytes).map(|place| tr!("Saved {name} to {place}", name = shown(&name), place = shown(&place)))
        });
        self.notice = Some(match saved {
            Ok(text) | Err(text) => text,
        });
    }

    /// Takes in the pictures the fetch brought since the last call; `true` when there were any.
    pub fn poll(&mut self) -> bool {
        let done = self.fetch.finished();
        let any = !done.is_empty();
        for (url, answer) in done {
            // An answer for a picture nobody asked for (any more) is dropped.
            if let Some(state) = self.pictures.get_mut(&url) {
                *state = match answer.map_err(|error| shown(&error)).and_then(|bytes| decode(&bytes).map_err(|error| error.to_string())) {
                    Ok(picture) => PictureState::Ready(Arc::new(picture)),
                    Err(error) => PictureState::Failed(error),
                };
            }
        }
        any
    }

    /// The picture at `key`: a remote address, or [`inline_key`].
    #[must_use]
    pub fn picture(&self, key: &str) -> Option<&PictureState> {
        self.pictures.get(key)
    }

    /// The message open, if any.
    #[must_use]
    pub fn open_id(&self) -> Option<u64> {
        self.open
    }

    /// The thread, oldest first.
    #[must_use]
    pub fn thread(&self) -> &[ThreadRow] {
        &self.thread
    }

    /// Message `id` of the thread, once read.
    #[must_use]
    pub fn opened(&self, id: u64) -> Option<&Opened> {
        self.thread.iter().find(|row| row.id == id).and_then(|row| row.opened.as_ref())
    }

    /// Whether message `id` is unfolded.
    #[must_use]
    pub fn is_expanded(&self, id: u64) -> bool {
        self.expanded.contains(&id)
    }

    /// Shows or hides the full header of message `id`.
    pub fn toggle_details(&mut self, id: u64) {
        if !self.details.remove(&id) {
            self.details.insert(id);
        }
    }

    /// Whether the full header of message `id` shows.
    #[must_use]
    pub fn shows_details(&self, id: u64) -> bool {
        self.details.contains(&id)
    }

    /// Shows or hides the quotes of message `id`.
    pub fn toggle_quotes(&mut self, id: u64) {
        if !self.quotes.remove(&id) {
            self.quotes.insert(id);
        }
    }

    /// Whether the quotes of message `id` show.
    #[must_use]
    pub fn shows_quotes(&self, id: u64) -> bool {
        self.quotes.contains(&id)
    }

    /// Whether the reader fills the window.
    #[must_use]
    pub fn full_width(&self) -> bool {
        self.full_width
    }

    /// Lets the reader fill the window, or not; only while a message is open.
    pub fn set_full_width(&mut self, full: bool) {
        self.full_width = full && self.open.is_some();
    }

    /// The last notice: a save, a link copied, or an error.
    #[must_use]
    pub fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    /// Shows `text` as the notice.
    pub fn set_notice(&mut self, text: String) {
        self.notice = Some(text);
    }

    /// Hides the notice.
    pub fn dismiss_notice(&mut self) {
        self.notice = None;
    }
}

#[cfg(test)]
#[path = "reader_tests.rs"]
pub(crate) mod tests;
