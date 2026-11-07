// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The client state machine (RFC 9051 section 3): bytes in, bytes out, events.

use std::collections::{HashSet, VecDeque};
use std::fmt;

use inboxcraft_model::{Uid, UidSet, UidValidity};

use super::capability::{Capabilities, MAX_CAPABILITIES, MAX_CAPABILITY_LEN, is_atom_char};
use super::command::{Command, escaped};
use super::fetch::{FetchAttributes, FetchItem};
use super::frame::Framer;
use super::list::{ListEntry, MAX_MAILBOXES, check_mailbox};
use super::mailbox::{MailboxStatus, flag_list};
use super::notify::{MailboxCounts, Notify};
use super::response::{Response, classify};
use super::sasl::{base64_forms, check_credentials, plain_response};
use super::search::{self, SearchKey, set_text};
use super::sync::{Extension, SelectOptions, vanished};
use super::text::Secrets;
use super::uidplus::CopyUid;
use super::value::{Piece, Status, StatusLine, Value, number};
use crate::ProtoError;

/// How the connection started.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transport {
    /// Implicit TLS (port 993): encrypted from the first byte.
    Tls,
    /// Not encrypted (port 143): STARTTLS must come before any sign-in.
    Plain,
}

/// The session's state (RFC 9051 section 3), with the two waits this client adds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// Waiting for the server's greeting.
    Greeting,
    /// Greeted, not signed in.
    NotAuthenticated,
    /// Signed in, no mailbox open.
    Authenticated,
    /// A mailbox is open.
    Selected,
    /// The server agreed to STARTTLS: the caller runs the TLS handshake, then calls [`Client::tls_established`].
    StartingTls,
    /// LOGOUT was sent.
    LoggingOut,
    /// The session is over.
    Closed,
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Greeting => "greeting",
            Self::NotAuthenticated => "not authenticated",
            Self::Authenticated => "authenticated",
            Self::Selected => "selected",
            Self::StartingTls => "starting TLS",
            Self::LoggingOut => "logging out",
            Self::Closed => "closed",
        })
    }
}

/// The tag of a command: its answer carries it back in [`Event::Completed`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Tag(String);

impl Tag {
    /// The tag as sent (`A0001`).
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// What the server answered or told, in the order it did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// The server greeted; `preauth` when it signed the session in by itself (only over TLS).
    Greeted {
        /// The greeting was PREAUTH.
        preauth: bool,
    },
    /// A command is done.
    Completed {
        /// The command's tag.
        tag: Tag,
        /// What it brought, or the server's refusal.
        outcome: Result<Answer, Refusal>,
    },
    /// A FETCH response: one asked for, or the server telling of a change.
    Fetched(FetchItem),
    /// The open mailbox now holds this many messages.
    Exists(u32),
    /// The message with this sequence number is gone; the ones after it move up by one.
    Expunged(u32),
    /// The messages with these UIDs are gone (QRESYNC, in the place of EXPUNGE); `earlier` when they went before the
    /// command that reports them (a resync), not just now. Sequence numbers are not told.
    Vanished {
        /// The UIDs.
        uids: UidSet,
        /// `(EARLIER)`.
        earlier: bool,
    },
    /// The open mailbox's UIDVALIDITY is not the one known: every UID held for it is void. It comes before the answer
    /// of a SELECT or EXAMINE given [`SelectOptions::known`], or when the server tells of a new one while the mailbox
    /// is open.
    UidValidityChanged {
        /// The UIDVALIDITY known.
        old: UidValidity,
        /// The server's, `None` when it gave none (then no UID can be kept).
        new: Option<UidValidity>,
    },
    /// A text the server says must be shown to the person (`[ALERT]`), cleaned; only over TLS.
    Alert(String),
    /// The server agreed to IDLE: changes come as events until [`Client::done`].
    Idling,
    /// NOTIFY: what a STATUS response says of a mailbox.
    Status(MailboxCounts),
    /// NOTIFY: a mailbox was made, renamed (under its new name) or removed (`\NonExistent`), as LIST tells it.
    MailboxChanged(ListEntry),
    /// NOTIFY: the server could not keep up and tells of nothing more, as after NOTIFY NONE; read the mailboxes
    /// again and send NOTIFY anew.
    NotificationOverflow,
}

/// What a command brought when it succeeded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Answer {
    /// CAPABILITY.
    Capabilities(Capabilities),
    /// STARTTLS: run the TLS handshake now, then call [`Client::tls_established`].
    StartTls,
    /// ENABLE: what the server says it switched on (`ENABLED`), which may be nothing.
    Enabled(Capabilities),
    /// LOGIN or AUTHENTICATE, with the capabilities as the server lists them after the sign-in.
    Authenticated(Capabilities),
    /// LIST: the mailboxes, each once, at most [`MAX_MAILBOXES`].
    Listed(Vec<ListEntry>),
    /// SELECT or EXAMINE.
    Selected(MailboxStatus),
    /// UID FETCH: the messages came as [`Event::Fetched`].
    Fetched,
    /// UID SEARCH: the UIDs found.
    Searched(UidSet),
    /// UID COPY, with where the messages went when the server says so (UIDPLUS).
    Copied(Option<CopyUid>),
    /// UID MOVE, or the steps that stand in for it.
    Moved(Moved),
    /// UID EXPUNGE: the messages that went came as [`Event::Expunged`] or [`Event::Vanished`].
    Expunged,
    /// COMPRESS DEFLATE: from the byte after this answer on, both directions are compressed (RFC 4978, raw DEFLATE
    /// as in RFC 1951). The caller deflates everything [`Client::take_output`] gives from now on and inflates what the
    /// server sends before it goes to [`Client::receive`]. These are the bytes that came after the OK in the same
    /// piece, still compressed: they go through the inflater first.
    Compressed(Vec<u8>),
    /// IDLE, ended by DONE.
    Idled,
    /// NOTIFY SET or NOTIFY NONE.
    Notified,
    /// LOGOUT.
    LoggedOut,
}

/// How the server refused a command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefusalKind {
    /// `NO`: the server would not.
    No,
    /// `BAD`: the server did not understand.
    Bad,
}

/// A command the server refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    /// `NO` or `BAD`.
    pub kind: RefusalKind,
    /// The response code, upper-cased (`AUTHENTICATIONFAILED`), when there is one made of atom characters, at most
    /// [`MAX_CAPABILITY_LEN`] long and holding no password.
    pub code: Option<String>,
    /// The server's words, cleaned and capped, with the password hidden; empty before TLS.
    pub text: String,
}

/// What a [`Client::uid_move`] did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Moved {
    /// Where the messages went, when the server said so (UIDPLUS) and it fits what was moved.
    pub copy_uid: Option<CopyUid>,
    /// Whether, and how, the messages left the mailbox they were in.
    pub removal: Removal,
}

/// How moved messages left the mailbox they were in. Without MOVE the client copies them, marks them `\Deleted` and
/// expunges them, and each of the later steps can be refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Removal {
    /// MOVE (RFC 6851): they went in one step.
    Moved,
    /// UID COPY, UID STORE of `\Deleted`, UID EXPUNGE of only these messages (UIDPLUS): they are gone.
    Expunged,
    /// UID COPY and UID STORE of `\Deleted`, not expunged: without UIDPLUS, EXPUNGE would remove every message
    /// marked `\Deleted`, not only these.
    MarkedDeleted,
    /// Copied, but the server refused to mark them `\Deleted`: they are still there, and in the other mailbox too.
    NotMarked(Refusal),
    /// Copied and marked `\Deleted`, but the server refused UID EXPUNGE: they are still there.
    NotExpunged(Refusal),
}

/// What a command waits for.
enum Kind {
    /// CAPABILITY; `after_sign_in` when this client sent it after a sign-in that did not list them, which completes
    /// with it.
    Capability {
        after_sign_in: Option<Tag>,
    },
    StartTls,
    /// LOGIN or AUTHENTICATE PLAIN.
    SignIn,
    Enable {
        asked: Extension,
        words: Vec<String>,
    },
    List {
        entries: Vec<ListEntry>,
        seen: HashSet<String>,
    },
    Select(Opening),
    Fetch,
    Search(Vec<Uid>),
    /// UID COPY of these UIDs.
    Copy(UidSet),
    Move(Moving),
    Expunge,
    Compress,
    /// NOTIFY SET (`set`) or NOTIFY NONE.
    Notify {
        set: bool,
    },
    /// IDLE; `going` once the server agreed, `done` once DONE was asked for.
    Idle {
        going: bool,
        done: bool,
    },
    Logout {
        previous: State,
    },
}

/// A SELECT or EXAMINE on its way.
struct Opening {
    status: MailboxStatus,
    /// The UIDVALIDITY the caller knew.
    known: Option<UidValidity>,
    /// Responses still belong to the mailbox open before, until `[CLOSED]` (RFC 7162 section 3.2.11).
    closing: bool,
}

/// A move on its way: MOVE, or a step of the fallback.
struct Moving {
    /// The caller's tag, the first step's, once a later step is on its way.
    tag: Option<Tag>,
    set: UidSet,
    step: MoveStep,
    copy_uid: Option<CopyUid>,
    /// A COPYUID came that did not fit: no other one is taken.
    void: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MoveStep {
    Move,
    Copy,
    Store,
    Expunge,
}

impl Moving {
    /// An untagged `COPYUID` during MOVE; a server that moves in parts sends one for each.
    fn told(&mut self, arg: &str) {
        let merged = match (self.copy_uid.take(), CopyUid::parse(arg, &self.set)) {
            (None, Some(new)) if !self.void => Some(new),
            (Some(old), Some(new)) => old.merged(new, &self.set),
            _ => None,
        };
        self.void = merged.is_none();
        self.copy_uid = merged;
    }
}

/// The command waiting for its answer.
struct InFlight {
    tag: Tag,
    kind: Kind,
    /// The rest of the command, a chunk for each go-ahead.
    waiting: VecDeque<Vec<u8>>,
}

/// An IMAP4rev1 and IMAP4rev2 client without I/O.
///
/// Feed it what the server sent with [`receive`](Self::receive), write what [`take_output`](Self::take_output) gives
/// to the server, and read what happened from [`next_event`](Self::next_event). One command at a time.
///
/// LOGIN and AUTHENTICATE are refused with [`ProtoError::NotEncrypted`] until the connection is encrypted, and the
/// capabilities are read again after STARTTLS and after a sign-in. Everything the server sends is hostile: lines,
/// literals and lists are capped, and a fatal [`ProtoError`] closes the session.
pub struct Client {
    state: State,
    tls: bool,
    framer: Framer,
    output: Vec<u8>,
    events: VecDeque<Event>,
    capabilities: Option<Capabilities>,
    /// An untagged CAPABILITY came during the command in flight.
    fresh_capabilities: bool,
    next_tag: u32,
    in_flight: Option<InFlight>,
    secrets: Secrets,
    /// CONDSTORE is on (ENABLE CONDSTORE or QRESYNC).
    condstore: bool,
    /// QRESYNC is on.
    qresync: bool,
    /// The open mailbox's UIDVALIDITY.
    uid_validity: Option<UidValidity>,
    /// The open mailbox is read-only.
    read_only: bool,
    /// NOTIFY SET is on.
    notifying: bool,
    /// COMPRESS DEFLATE is on.
    compressed: bool,
}

/// Neither the output (it may hold a password) nor anything secret is shown.
impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("state", &self.state)
            .field("tls", &self.tls)
            .field("in_flight", &self.in_flight.as_ref().map(|flight| &flight.tag))
            .finish_non_exhaustive()
    }
}

impl Client {
    /// A client for a connection that has just opened, waiting for the greeting.
    #[must_use]
    pub fn new(transport: Transport) -> Self {
        Self {
            state: State::Greeting,
            tls: transport == Transport::Tls,
            framer: Framer::default(),
            output: Vec::new(),
            events: VecDeque::new(),
            capabilities: None,
            fresh_capabilities: false,
            next_tag: 1,
            in_flight: None,
            secrets: Secrets::default(),
            condstore: false,
            qresync: false,
            uid_validity: None,
            read_only: false,
            notifying: false,
            compressed: false,
        }
    }

    /// The session's state.
    #[must_use]
    pub fn state(&self) -> State {
        self.state
    }

    /// Whether the connection is encrypted.
    #[must_use]
    pub fn is_encrypted(&self) -> bool {
        self.tls
    }

    /// Whether COMPRESS DEFLATE is on: what goes to and comes from the server is compressed.
    #[must_use]
    pub fn is_compressed(&self) -> bool {
        self.compressed
    }

    /// What the server said it can do, `None` until it has (and again after STARTTLS and a sign-in, until read).
    #[must_use]
    pub fn capabilities(&self) -> Option<&Capabilities> {
        self.capabilities.as_ref()
    }

    /// Whether the server switched `extension` on after an [`enable`](Self::enable); QRESYNC brings CONDSTORE with it.
    #[must_use]
    pub fn is_enabled(&self, extension: Extension) -> bool {
        match extension {
            Extension::CondStore => self.condstore,
            Extension::QResync => self.qresync,
        }
    }

    /// The bytes to write to the server, taken out.
    pub fn take_output(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.output)
    }

    /// The next event, taken out.
    pub fn next_event(&mut self) -> Option<Event> {
        self.events.pop_front()
    }

    /// Takes in bytes the server sent, in pieces of any size.
    ///
    /// # Errors
    ///
    /// A fatal [`ProtoError`] when the server broke the protocol or a limit; the session is then
    /// [`Closed`](State::Closed) and every later call answers [`ProtoError::Closed`].
    pub fn receive(&mut self, bytes: &[u8]) -> Result<(), ProtoError> {
        match self.state {
            State::Closed => return Err(ProtoError::Closed),
            State::StartingTls if !bytes.is_empty() => return Err(self.fail(ProtoError::PlaintextAfterStartTls)),
            _ => {}
        }
        self.framer.push(bytes);
        while !matches!(self.state, State::StartingTls | State::Closed) {
            let pieces = match self.framer.next() {
                Ok(Some(pieces)) => pieces,
                Ok(None) => break,
                Err(error) => return Err(self.fail(error)),
            };
            if let Err(error) = self.handle(pieces) {
                return Err(self.fail(error));
            }
        }
        Ok(())
    }

    /// CAPABILITY.
    ///
    /// # Errors
    ///
    /// [`ProtoError::Busy`], [`ProtoError::WrongState`] before the greeting, or [`ProtoError::Closed`].
    pub fn capability(&mut self) -> Result<Tag, ProtoError> {
        self.ready("CAPABILITY", &[State::NotAuthenticated, State::Authenticated, State::Selected])?;
        Ok(self.start(Command::new("CAPABILITY"), Kind::Capability { after_sign_in: None }))
    }

    /// STARTTLS, on a connection that is not encrypted and whose server offers it.
    ///
    /// # Errors
    ///
    /// [`ProtoError::AlreadyEncrypted`], [`ProtoError::CapabilitiesUnknown`], [`ProtoError::NotOffered`], or a state
    /// error.
    pub fn starttls(&mut self) -> Result<Tag, ProtoError> {
        self.ready("STARTTLS", &[State::NotAuthenticated])?;
        if self.tls {
            return Err(ProtoError::AlreadyEncrypted);
        }
        let capabilities = self.capabilities.as_ref().ok_or(ProtoError::CapabilitiesUnknown)?;
        if !capabilities.has("STARTTLS") {
            return Err(ProtoError::NotOffered("STARTTLS"));
        }
        Ok(self.start(Command::new("STARTTLS"), Kind::StartTls))
    }

    /// After the TLS handshake that followed STARTTLS: the session goes on encrypted, forgets the capabilities it
    /// was told before, and asks for them again with a CAPABILITY whose tag this is.
    ///
    /// # Errors
    ///
    /// [`ProtoError::WrongState`] unless STARTTLS was just agreed, or [`ProtoError::Closed`].
    pub fn tls_established(&mut self) -> Result<Tag, ProtoError> {
        match self.state {
            State::StartingTls => {}
            State::Closed => return Err(ProtoError::Closed),
            state => return Err(ProtoError::WrongState { command: "STARTTLS", state }),
        }
        self.tls = true;
        self.state = State::NotAuthenticated;
        self.capabilities = None;
        self.framer = Framer::default();
        Ok(self.start(Command::new("CAPABILITY"), Kind::Capability { after_sign_in: None }))
    }

    /// LOGIN, over an encrypted connection to a server that does not say `LOGINDISABLED`.
    ///
    /// # Errors
    ///
    /// [`ProtoError::NotEncrypted`] before TLS, [`ProtoError::CapabilitiesUnknown`], [`ProtoError::NotOffered`],
    /// [`ProtoError::BadArgument`] for an empty user name or password or one with a NUL, or a state error.
    pub fn login(&mut self, user: &str, password: &str) -> Result<Tag, ProtoError> {
        self.ready("LOGIN", &[State::NotAuthenticated])?;
        if self.sign_in_capabilities()?.has("LOGINDISABLED") {
            return Err(ProtoError::NotOffered("LOGIN"));
        }
        self.hide_credentials(user, password)?;
        let command = Command::new("LOGIN").string(user.as_bytes()).string(password.as_bytes());
        Ok(self.start(command, Kind::SignIn))
    }

    /// Checks the credentials and adds the password to what is never shown, in every form a server may write it
    /// back in, whether it went out with LOGIN or AUTHENTICATE PLAIN.
    fn hide_credentials(&mut self, user: &str, password: &str) -> Result<(), ProtoError> {
        check_credentials(user, password)?;
        self.secrets.add(password);
        // The form inside the quotes, when it differs: a server may write the argument back as it came.
        self.secrets.add(&String::from_utf8_lossy(&escaped(password.as_bytes())));
        for form in base64_forms(user, password)? {
            self.secrets.add(&form);
        }
        Ok(())
    }

    /// AUTHENTICATE PLAIN (RFC 4616), over an encrypted connection to a server that offers `AUTH=PLAIN`; with
    /// `SASL-IR` (RFC 4959) the response goes with the command, otherwise after the server's go-ahead.
    ///
    /// # Errors
    ///
    /// As [`login`](Self::login), and [`ProtoError::NotOffered`] without `AUTH=PLAIN`.
    pub fn authenticate_plain(&mut self, user: &str, password: &str) -> Result<Tag, ProtoError> {
        self.ready("AUTHENTICATE", &[State::NotAuthenticated])?;
        let capabilities = self.sign_in_capabilities()?;
        if !capabilities.has("AUTH=PLAIN") {
            return Err(ProtoError::NotOffered("AUTH=PLAIN"));
        }
        let inline = capabilities.has("SASL-IR");
        let response = plain_response(user, password)?;
        self.hide_credentials(user, password)?;
        let command = Command::new("AUTHENTICATE PLAIN");
        if inline {
            return Ok(self.start(command.arg(&response), Kind::SignIn));
        }
        let tag = self.start(command, Kind::SignIn);
        if let Some(flight) = self.in_flight.as_mut() {
            flight.waiting.push_back(format!("{response}\r\n").into_bytes());
        }
        Ok(tag)
    }

    /// ENABLE (RFC 5161) of an extension the server offers, after the sign-in and before a mailbox is open.
    ///
    /// # Errors
    ///
    /// [`ProtoError::CapabilitiesUnknown`], [`ProtoError::NotOffered`] without `ENABLE` or the extension, or a state
    /// error.
    pub fn enable(&mut self, extension: Extension) -> Result<Tag, ProtoError> {
        self.ready("ENABLE", &[State::Authenticated])?;
        let capabilities = self.capabilities.as_ref().ok_or(ProtoError::CapabilitiesUnknown)?;
        for needed in ["ENABLE", extension.name()] {
            if !capabilities.has(needed) {
                return Err(ProtoError::NotOffered(needed));
            }
        }
        Ok(self.start(Command::new("ENABLE").arg(extension.name()), Kind::Enable { asked: extension, words: Vec::new() }))
    }

    /// LIST `""` `"*"`: every mailbox, with `RETURN (SPECIAL-USE)` when the server offers LIST-EXTENDED and
    /// SPECIAL-USE (RFC 5258, RFC 6154).
    ///
    /// # Errors
    ///
    /// [`ProtoError::Busy`], a state error, or [`ProtoError::Closed`].
    pub fn list(&mut self) -> Result<Tag, ProtoError> {
        self.ready("LIST", &[State::Authenticated, State::Selected])?;
        let mut command = Command::new("LIST").arg("\"\"").arg("\"*\"");
        if self.offers("LIST-EXTENDED") && self.offers("SPECIAL-USE") {
            command = command.arg("RETURN (SPECIAL-USE)");
        }
        Ok(self.start(command, Kind::List { entries: Vec::new(), seen: HashSet::new() }))
    }

    /// SELECT: opens a mailbox to read and change. `mailbox` is the name as the server wrote it ([`ListEntry::raw`]).
    ///
    /// # Errors
    ///
    /// [`ProtoError::BadArgument`] for an empty or overlong name or one with a control character, a state error.
    pub fn select(&mut self, mailbox: &str) -> Result<Tag, ProtoError> {
        self.open("SELECT", mailbox, false, &SelectOptions::default())
    }

    /// SELECT with [`SelectOptions`]: CONDSTORE, and QRESYNC once enabled.
    ///
    /// # Errors
    ///
    /// As [`select`](Self::select), and [`ProtoError::NotOffered`] for CONDSTORE from a server without it.
    pub fn select_with(&mut self, mailbox: &str, options: &SelectOptions) -> Result<Tag, ProtoError> {
        self.open("SELECT", mailbox, false, options)
    }

    /// EXAMINE: opens a mailbox read-only. As [`select`](Self::select).
    ///
    /// # Errors
    ///
    /// As [`select`](Self::select).
    pub fn examine(&mut self, mailbox: &str) -> Result<Tag, ProtoError> {
        self.open("EXAMINE", mailbox, true, &SelectOptions::default())
    }

    /// EXAMINE with [`SelectOptions`]. As [`select_with`](Self::select_with).
    ///
    /// # Errors
    ///
    /// As [`select_with`](Self::select_with).
    pub fn examine_with(&mut self, mailbox: &str, options: &SelectOptions) -> Result<Tag, ProtoError> {
        self.open("EXAMINE", mailbox, true, options)
    }

    fn open(&mut self, verb: &'static str, mailbox: &str, read_only: bool, options: &SelectOptions) -> Result<Tag, ProtoError> {
        self.ready(verb, &[State::Authenticated, State::Selected])?;
        check_mailbox(mailbox)?;
        if options.condstore {
            self.require_condstore()?;
        }
        let mut command = Command::new(verb).string(mailbox.as_bytes());
        let resync = options.known.filter(|_| self.qresync).and_then(|known| Some((known.uid_validity(), known.highest_modseq()?)));
        if let Some((uid_validity, modseq)) = resync {
            let mut parameter = format!("(QRESYNC ({uid_validity} {modseq}");
            if let Some(uids) = options.known_uids.as_ref().filter(|uids| !uids.is_empty()) {
                parameter.push(' ');
                parameter.push_str(&set_text(uids)?);
            }
            command = command.arg(&format!("{parameter}))"));
        } else if options.condstore {
            command = command.arg("(CONDSTORE)");
        }
        // With QRESYNC the server marks where the mailbox open before ends.
        let closing = self.state == State::Selected && self.qresync;
        // Whatever the answer, the mailbox open before is closed (RFC 9051 section 6.3.2).
        self.state = State::Authenticated;
        self.uid_validity = None;
        self.read_only = read_only;
        let status = MailboxStatus { read_only, ..MailboxStatus::default() };
        Ok(self.start(command, Kind::Select(Opening { status, known: options.known.map(|known| known.uid_validity()), closing })))
    }

    /// UID FETCH of the messages in `set`, with their UID and `attributes`; they come as [`Event::Fetched`].
    ///
    /// # Errors
    ///
    /// [`ProtoError::BadArgument`] for an empty set or one of more than [`crate::imap::MAX_SET_RANGES`] ranges or
    /// VANISHED without CHANGEDSINCE, [`ProtoError::NotOffered`] for MODSEQ or CHANGEDSINCE from a server without
    /// CONDSTORE and for a preview from one without PREVIEW, [`ProtoError::NotEnabled`] for VANISHED before QRESYNC is
    /// enabled, a state error.
    pub fn uid_fetch(&mut self, set: &UidSet, attributes: FetchAttributes) -> Result<Tag, ProtoError> {
        self.ready("UID FETCH", &[State::Selected])?;
        if attributes.modseq || attributes.changed_since.is_some() {
            self.require_condstore()?;
        }
        if attributes.vanished && attributes.changed_since.is_none() {
            return Err(ProtoError::BadArgument("VANISHED without CHANGEDSINCE"));
        }
        if attributes.vanished && !self.qresync {
            return Err(ProtoError::NotEnabled("QRESYNC"));
        }
        if attributes.preview.is_some() {
            self.require("PREVIEW")?;
        }
        let mut command = Command::new("UID FETCH").arg(&set_text(set)?).arg(&attributes.items());
        if let Some(modifiers) = attributes.modifiers() {
            command = command.arg(&modifiers);
        }
        Ok(self.start(command, Kind::Fetch))
    }

    /// UID SEARCH: the UIDs of the messages that match `key`.
    ///
    /// # Errors
    ///
    /// [`ProtoError::BadArgument`] for a key that cannot be sent (see [`SearchKey`]), a state error.
    pub fn uid_search(&mut self, key: &SearchKey) -> Result<Tag, ProtoError> {
        self.ready("UID SEARCH", &[State::Selected])?;
        let command = search::command(key)?;
        Ok(self.start(command, Kind::Search(Vec::new())))
    }

    /// UID COPY (RFC 9051 section 6.4.7) of the messages in `set` to `mailbox`, the name as the server wrote it. With
    /// UIDPLUS the answer says which UIDs they got there.
    ///
    /// # Errors
    ///
    /// [`ProtoError::BadArgument`] for a set or mailbox name that cannot be sent, or a state error.
    pub fn uid_copy(&mut self, set: &UidSet, mailbox: &str) -> Result<Tag, ProtoError> {
        self.ready("UID COPY", &[State::Selected])?;
        check_mailbox(mailbox)?;
        let command = Command::new("UID COPY").arg(&set_text(set)?).string(mailbox.as_bytes());
        Ok(self.start(command, Kind::Copy(set.clone())))
    }

    /// UID MOVE (RFC 6851) of the messages in `set` to `mailbox`, the name as the server wrote it. A server without
    /// MOVE gets UID COPY, UID STORE `+FLAGS.SILENT (\Deleted)` and, with UIDPLUS, UID EXPUNGE of these messages,
    /// one after the other under this one tag; [`Answer::Moved`] says how far that went.
    ///
    /// # Errors
    ///
    /// [`ProtoError::CapabilitiesUnknown`], [`ProtoError::ReadOnly`], [`ProtoError::BadArgument`] for a set or mailbox
    /// name that cannot be sent, or a state error.
    pub fn uid_move(&mut self, set: &UidSet, mailbox: &str) -> Result<Tag, ProtoError> {
        self.ready("UID MOVE", &[State::Selected])?;
        check_mailbox(mailbox)?;
        if self.capabilities.is_none() {
            return Err(ProtoError::CapabilitiesUnknown);
        }
        if self.read_only {
            return Err(ProtoError::ReadOnly);
        }
        let (verb, step) = if self.offers_in_rev2("MOVE") { ("UID MOVE", MoveStep::Move) } else { ("UID COPY", MoveStep::Copy) };
        let command = Command::new(verb).arg(&set_text(set)?).string(mailbox.as_bytes());
        Ok(self.start(command, Kind::Move(Moving { tag: None, set: set.clone(), step, copy_uid: None, void: false })))
    }

    /// UID EXPUNGE (UIDPLUS, RFC 4315 section 2.1): removes the messages in `set` that are marked `\Deleted`, and no
    /// other.
    ///
    /// # Errors
    ///
    /// [`ProtoError::CapabilitiesUnknown`], [`ProtoError::NotOffered`] without UIDPLUS or IMAP4rev2,
    /// [`ProtoError::ReadOnly`], [`ProtoError::BadArgument`] for a set that cannot be sent, or a state error.
    pub fn uid_expunge(&mut self, set: &UidSet) -> Result<Tag, ProtoError> {
        self.ready("UID EXPUNGE", &[State::Selected])?;
        self.require_in_rev2("UIDPLUS")?;
        if self.read_only {
            return Err(ProtoError::ReadOnly);
        }
        Ok(self.start(Command::new("UID EXPUNGE").arg(&set_text(set)?), Kind::Expunge))
    }

    /// NOTIFY SET (RFC 5465): the server tells of what `notify` names without being asked, until NOTIFY NONE or
    /// [`Event::NotificationOverflow`].
    ///
    /// # Errors
    ///
    /// [`ProtoError::BadArgument`] for nothing to tell of, MailboxName without mailboxes, or mailbox names that cannot
    /// be sent; [`ProtoError::CapabilitiesUnknown`], [`ProtoError::NotOffered`] without `NOTIFY`, or a state error.
    pub fn notify(&mut self, notify: &Notify) -> Result<Tag, ProtoError> {
        self.ready("NOTIFY", &[State::Authenticated, State::Selected])?;
        self.require("NOTIFY")?;
        let command = notify.command(self.condstore)?;
        Ok(self.start(command, Kind::Notify { set: true }))
    }

    /// NOTIFY NONE: the server tells of nothing more without being asked.
    ///
    /// # Errors
    ///
    /// As [`notify`](Self::notify).
    pub fn notify_none(&mut self) -> Result<Tag, ProtoError> {
        self.ready("NOTIFY", &[State::Authenticated, State::Selected])?;
        self.require("NOTIFY")?;
        Ok(self.start(Command::new("NOTIFY NONE"), Kind::Notify { set: false }))
    }

    /// COMPRESS DEFLATE (RFC 4978), after the sign-in, to a server that offers `COMPRESS=DEFLATE`. Only the
    /// negotiation is here; see [`Answer::Compressed`] for what the caller does once the server agreed.
    ///
    /// # Errors
    ///
    /// [`ProtoError::AlreadyCompressed`], [`ProtoError::CapabilitiesUnknown`], [`ProtoError::NotOffered`], or a state
    /// error.
    pub fn compress(&mut self) -> Result<Tag, ProtoError> {
        self.ready("COMPRESS", &[State::Authenticated, State::Selected])?;
        if self.compressed {
            return Err(ProtoError::AlreadyCompressed);
        }
        self.require("COMPRESS=DEFLATE")?;
        Ok(self.start(Command::new("COMPRESS DEFLATE"), Kind::Compress))
    }

    /// IDLE (RFC 2177) in the open mailbox: [`Event::Idling`] once the server agrees, then what changes, as events,
    /// until [`done`](Self::done). Nothing else can be sent meanwhile. Ending it before the server's time limit (often
    /// 30 minutes) is the caller's work.
    ///
    /// # Errors
    ///
    /// [`ProtoError::CapabilitiesUnknown`], [`ProtoError::NotOffered`] without `IDLE`, or a state error.
    pub fn idle(&mut self) -> Result<Tag, ProtoError> {
        self.ready("IDLE", &[State::Selected])?;
        self.require("IDLE")?;
        Ok(self.start(Command::new("IDLE"), Kind::Idle { going: false, done: false }))
    }

    /// DONE: ends the IDLE in flight; before the server agreed to it, as soon as it does. Its answer is
    /// [`Answer::Idled`].
    ///
    /// # Errors
    ///
    /// [`ProtoError::WrongState`] when no IDLE is waiting for DONE, or [`ProtoError::Closed`].
    pub fn done(&mut self) -> Result<(), ProtoError> {
        if self.state == State::Closed {
            return Err(ProtoError::Closed);
        }
        let Some(InFlight { kind: Kind::Idle { going, done: done @ false }, waiting, .. }) = self.in_flight.as_mut() else {
            return Err(ProtoError::WrongState { command: "DONE", state: self.state });
        };
        *done = true;
        if *going {
            self.output.extend_from_slice(b"DONE\r\n");
        } else {
            waiting.push_back(b"DONE\r\n".to_vec());
        }
        Ok(())
    }

    /// LOGOUT.
    ///
    /// # Errors
    ///
    /// [`ProtoError::Busy`], a state error, or [`ProtoError::Closed`].
    pub fn logout(&mut self) -> Result<Tag, ProtoError> {
        self.ready("LOGOUT", &[State::NotAuthenticated, State::Authenticated, State::Selected])?;
        let previous = self.state;
        self.state = State::LoggingOut;
        Ok(self.start(Command::new("LOGOUT"), Kind::Logout { previous }))
    }

    /// The capabilities a sign-in is checked against: only over TLS, and only once the server has listed them.
    fn sign_in_capabilities(&self) -> Result<&Capabilities, ProtoError> {
        if !self.tls {
            return Err(ProtoError::NotEncrypted);
        }
        self.capabilities.as_ref().ok_or(ProtoError::CapabilitiesUnknown)
    }

    /// Whether the server listed `capability`.
    fn offers(&self, capability: &str) -> bool {
        self.capabilities.as_ref().is_some_and(|capabilities| capabilities.has(capability))
    }

    /// That the server listed `capability`.
    fn require(&self, capability: &'static str) -> Result<(), ProtoError> {
        let capabilities = self.capabilities.as_ref().ok_or(ProtoError::CapabilitiesUnknown)?;
        if capabilities.has(capability) { Ok(()) } else { Err(ProtoError::NotOffered(capability)) }
    }

    /// Whether the server listed `capability`, or IMAP4rev2, which has it in it (RFC 9051 appendix E).
    fn offers_in_rev2(&self, capability: &str) -> bool {
        self.offers(capability) || self.offers("IMAP4rev2")
    }

    /// That the server listed `capability`, or IMAP4rev2.
    fn require_in_rev2(&self, capability: &'static str) -> Result<(), ProtoError> {
        self.require(capability).or_else(|error| if self.offers("IMAP4rev2") { Ok(()) } else { Err(error) })
    }

    /// That the server keeps modification sequences: CONDSTORE, or QRESYNC, which brings it (RFC 7162 section 3.2).
    fn require_condstore(&self) -> Result<(), ProtoError> {
        self.require("QRESYNC").or_else(|_| self.require("CONDSTORE"))
    }

    fn ready(&self, command: &'static str, allowed: &[State]) -> Result<(), ProtoError> {
        if self.state == State::Closed {
            return Err(ProtoError::Closed);
        }
        if self.in_flight.is_some() {
            return Err(ProtoError::Busy);
        }
        if !allowed.contains(&self.state) {
            return Err(ProtoError::WrongState { command, state: self.state });
        }
        Ok(())
    }

    fn start(&mut self, command: Command, kind: Kind) -> Tag {
        let tag = Tag(format!("A{:04}", self.next_tag));
        self.next_tag = self.next_tag.wrapping_add(1);
        let (first, waiting) = command.finish(tag.as_str());
        self.output.extend_from_slice(&first);
        self.fresh_capabilities = false;
        self.in_flight = Some(InFlight { tag: tag.clone(), kind, waiting });
        tag
    }

    /// Ends the session after a fatal error: nothing more is sent or read.
    fn fail(&mut self, error: ProtoError) -> ProtoError {
        self.state = State::Closed;
        self.in_flight = None;
        self.output.clear();
        error
    }

    fn handle(&mut self, pieces: Vec<Piece>) -> Result<(), ProtoError> {
        let response = classify(pieces)?;
        if self.state == State::Greeting {
            return match response {
                Response::Status(line) => self.greeting(&line),
                _ => Err(ProtoError::Unexpected("response before the greeting")),
            };
        }
        match response {
            Response::Continue => self.go_ahead(),
            Response::Tagged { tag, line } => self.tagged(&tag, &line),
            Response::Status(line) => self.untagged_status(&line),
            Response::Data(values) => self.data(values),
        }
    }

    fn greeting(&mut self, line: &StatusLine) -> Result<(), ProtoError> {
        match line.status {
            Status::Ok => self.state = State::NotAuthenticated,
            Status::Preauth if self.tls => self.state = State::Authenticated,
            Status::Preauth => return Err(ProtoError::PreauthWithoutTls),
            Status::Bye => return Err(ProtoError::Bye(self.words(&line.text))),
            Status::No | Status::Bad => return Err(ProtoError::Malformed("greeting")),
        }
        self.note_code(line);
        self.events.push_back(Event::Greeted { preauth: line.status == Status::Preauth });
        Ok(())
    }

    fn untagged_status(&mut self, line: &StatusLine) -> Result<(), ProtoError> {
        match line.status {
            Status::Bye if self.state == State::LoggingOut => Ok(()),
            Status::Bye => Err(ProtoError::Bye(self.words(&line.text))),
            Status::Preauth => Err(ProtoError::Unexpected("PREAUTH")),
            Status::Ok | Status::No | Status::Bad => {
                self.note_code(line);
                if line.code == "NOTIFICATIONOVERFLOW" && std::mem::take(&mut self.notifying) {
                    self.events.push_back(Event::NotificationOverflow);
                }
                match self.in_flight.as_mut() {
                    Some(InFlight { kind: Kind::Select(opening), .. }) if line.code == "CLOSED" => {
                        // What came so far was about the mailbox open before.
                        opening.status = MailboxStatus { read_only: opening.status.read_only, ..MailboxStatus::default() };
                        opening.closing = false;
                    }
                    Some(InFlight { kind: Kind::Select(opening), .. }) if !opening.closing => opening.status.note_code(line),
                    Some(InFlight { kind: Kind::Select(_), .. }) => {}
                    Some(InFlight { kind: Kind::Move(moving), .. }) if line.code == "COPYUID" && moving.step == MoveStep::Move => {
                        moving.told(&line.arg);
                    }
                    _ if self.state == State::Selected && line.code == "UIDVALIDITY" => {
                        self.uid_validity_told(number(&line.arg).and_then(UidValidity::new))
                    }
                    _ => {}
                }
                Ok(())
            }
        }
    }

    /// The server told the open mailbox's UIDVALIDITY.
    fn uid_validity_told(&mut self, told: Option<UidValidity>) {
        if told.is_none() || told == self.uid_validity {
            return;
        }
        if let Some(old) = self.uid_validity {
            self.events.push_back(Event::UidValidityChanged { old, new: told });
        }
        self.uid_validity = told;
    }

    /// Server text for people, cleaned and with the secrets hidden; empty before TLS, where anyone on the way could
    /// have written it and the person would take it for the server's.
    fn words(&self, text: &str) -> String {
        if self.tls { self.secrets.tidy(text) } else { String::new() }
    }

    /// A response code for the caller: a word of atom characters at most [`MAX_CAPABILITY_LEN`] long that holds no
    /// secret, or `None`.
    fn code(&self, code: &str) -> Option<String> {
        let clean = !code.is_empty() && code.len() <= MAX_CAPABILITY_LEN && code.bytes().all(is_atom_char);
        (clean && !self.secrets.shows(code)).then(|| code.to_owned())
    }

    /// What a status line's response code says about the session.
    fn note_code(&mut self, line: &StatusLine) {
        match line.code.as_str() {
            "CAPABILITY" => {
                self.capabilities = Some(Capabilities::from_words(line.arg.split_ascii_whitespace()));
                self.fresh_capabilities = true;
            }
            // In the clear anyone on the way could have written it, and the person would take it for the server's.
            "ALERT" if self.tls => self.events.push_back(Event::Alert(self.secrets.tidy(&line.text))),
            _ => {}
        }
    }

    fn go_ahead(&mut self) -> Result<(), ProtoError> {
        if let Some(InFlight { kind: Kind::Idle { going, .. }, waiting, .. }) = self.in_flight.as_mut() {
            if *going {
                return Err(ProtoError::Unexpected("continuation request"));
            }
            *going = true;
            self.events.push_back(Event::Idling);
            // A DONE asked for before.
            self.output.extend(waiting.pop_front().unwrap_or_default());
            return Ok(());
        }
        let chunk = self.in_flight.as_mut().and_then(|flight| flight.waiting.pop_front());
        let chunk = chunk.ok_or(ProtoError::Unexpected("continuation request"))?;
        self.output.extend_from_slice(&chunk);
        Ok(())
    }

    fn data(&mut self, values: Vec<Value>) -> Result<(), ProtoError> {
        let first = values.first().and_then(Value::atom).map(str::to_ascii_uppercase).unwrap_or_default();
        let flight = self.in_flight.as_mut().map(|flight| &mut flight.kind);
        match (first.as_str(), flight) {
            ("CAPABILITY", _) => {
                self.capabilities = Some(Capabilities::from_words(values.iter().skip(1).filter_map(Value::atom)));
                self.fresh_capabilities = true;
            }
            ("LIST", Some(Kind::List { entries, seen })) => {
                if entries.len() < MAX_MAILBOXES
                    && let Some(entry) = ListEntry::from_values(values)
                    && seen.insert(entry.raw.clone())
                {
                    entries.push(entry);
                }
            }
            ("ENABLED", Some(Kind::Enable { words, .. })) => {
                words.extend(values.iter().skip(1).filter_map(Value::atom).take(MAX_CAPABILITIES).map(str::to_owned));
            }
            ("STATUS", flight) if self.notifying || matches!(flight, Some(Kind::Notify { .. })) => {
                if let Some(counts) = MailboxCounts::from_values(values) {
                    self.events.push_back(Event::Status(counts));
                }
            }
            ("LIST", flight) if self.notifying || matches!(flight, Some(Kind::Notify { .. })) => {
                if let Some(entry) = ListEntry::from_values(values) {
                    self.events.push_back(Event::MailboxChanged(entry));
                }
            }
            ("FLAGS", Some(Kind::Select(opening))) => {
                if let Some(Value::List(flags)) = values.into_iter().nth(1)
                    && !opening.closing
                {
                    opening.status.flags = flag_list(flags);
                }
            }
            ("VANISHED", flight) => {
                let open = self.state == State::Selected || matches!(flight, Some(Kind::Select(_) | Kind::Logout { previous: State::Selected }));
                if !open || !self.qresync {
                    return Err(ProtoError::Unexpected("VANISHED response"));
                }
                let (uids, earlier) = vanished(values).ok_or(ProtoError::Malformed("VANISHED response"))?;
                if !matches!(flight, Some(Kind::Select(Opening { closing: true, .. }))) {
                    self.events.push_back(Event::Vanished { uids, earlier });
                }
            }
            ("SEARCH", Some(Kind::Search(found))) => {
                found.extend(values.iter().skip(1).filter_map(Value::number).filter_map(Uid::new));
            }
            (word, flight) => {
                if let Some(count) = number(word) {
                    let open = self.state == State::Selected || matches!(flight, Some(Kind::Select(_) | Kind::Logout { previous: State::Selected }));
                    message_data(count, values, flight, open, &mut self.events)?;
                }
            }
        }
        Ok(())
    }

    fn tagged(&mut self, tag: &str, line: &StatusLine) -> Result<(), ProtoError> {
        let flight = self.in_flight.take().filter(|flight| flight.tag.as_str() == tag).ok_or(ProtoError::Unexpected("tagged response"))?;
        if line.status == Status::Ok && !flight.waiting.is_empty() {
            // The command cannot have succeeded without the rest of it.
            return Err(ProtoError::Unexpected("tagged response"));
        }
        // Capabilities on the answer to STARTTLS were sent in the clear: they are read again over TLS.
        if !matches!(flight.kind, Kind::StartTls) {
            self.note_code(line);
        }
        let fresh = std::mem::take(&mut self.fresh_capabilities);
        let refusal = match line.status {
            Status::Ok => None,
            Status::No => Some(RefusalKind::No),
            _ => Some(RefusalKind::Bad),
        };
        let refusal = refusal.map(|kind| Refusal { kind, code: self.code(&line.code), text: self.words(&line.text) });
        let (tag, kind) = match flight.kind {
            Kind::Capability { after_sign_in: Some(sign_in) } => {
                let capabilities = if refusal.is_none() {
                    self.capabilities.get_or_insert_default().clone()
                } else {
                    // Not read after all: what came with the refusal is not taken either.
                    self.capabilities = None;
                    Capabilities::default()
                };
                self.events.push_back(Event::Completed { tag: sign_in, outcome: Ok(Answer::Authenticated(capabilities)) });
                return Ok(());
            }
            kind => (flight.tag, kind),
        };
        let answer = match (kind, refusal) {
            (Kind::Logout { previous }, Some(refusal)) => {
                self.state = previous;
                Err(refusal)
            }
            (Kind::Select(_), Some(refusal)) => {
                // Already `None`, from the SELECT on.
                self.state = State::Authenticated;
                Err(refusal)
            }
            (Kind::Move(moving), Some(refusal)) => return self.move_answered(tag, moving, line, Some(refusal)),
            (_, Some(refusal)) => Err(refusal),
            (kind, None) => match self.succeeded(kind, &tag, line, fresh)? {
                Some(answer) => Ok(answer),
                None => return Ok(()),
            },
        };
        self.events.push_back(Event::Completed { tag, outcome: answer });
        Ok(())
    }

    /// The answer to a step of a move: the next step, or the caller's answer.
    fn move_answered(&mut self, step_tag: Tag, mut moving: Moving, line: &StatusLine, refusal: Option<Refusal>) -> Result<(), ProtoError> {
        let tag = moving.tag.take().unwrap_or(step_tag);
        if refusal.is_none() && matches!(moving.step, MoveStep::Move | MoveStep::Copy) && moving.copy_uid.is_none() && !moving.void {
            moving.copy_uid = copy_uid(line, &moving.set);
        }
        let removal = match (moving.step, refusal) {
            (MoveStep::Move | MoveStep::Copy, Some(refusal)) => Err(refusal),
            (MoveStep::Store, Some(refusal)) => Ok(Removal::NotMarked(refusal)),
            (MoveStep::Expunge, Some(refusal)) => Ok(Removal::NotExpunged(refusal)),
            (MoveStep::Move, None) => Ok(Removal::Moved),
            (MoveStep::Store, None) if !self.offers_in_rev2("UIDPLUS") => Ok(Removal::MarkedDeleted),
            (MoveStep::Expunge, None) => Ok(Removal::Expunged),
            (step @ (MoveStep::Copy | MoveStep::Store), None) => {
                let set = set_text(&moving.set)?;
                let command = if step == MoveStep::Copy {
                    moving.step = MoveStep::Store;
                    Command::new("UID STORE").arg(&set).arg("+FLAGS.SILENT (\\Deleted)")
                } else {
                    moving.step = MoveStep::Expunge;
                    Command::new("UID EXPUNGE").arg(&set)
                };
                moving.tag = Some(tag);
                self.start(command, Kind::Move(moving));
                return Ok(());
            }
        };
        let outcome = removal.map(|removal| Answer::Moved(Moved { copy_uid: moving.copy_uid, removal }));
        self.events.push_back(Event::Completed { tag, outcome });
        Ok(())
    }

    /// The answer of a command the server completed with OK; `None` while a CAPABILITY or the next step of a move
    /// must follow first, or when the move told the caller already.
    fn succeeded(&mut self, kind: Kind, tag: &Tag, line: &StatusLine, fresh: bool) -> Result<Option<Answer>, ProtoError> {
        let answer = match kind {
            Kind::Capability { .. } => {
                let capabilities = self.capabilities.get_or_insert_default().clone();
                Answer::Capabilities(capabilities)
            }
            Kind::StartTls => {
                // Bytes after the go-ahead came before encryption: anyone on the way could have put them there.
                if self.framer.has_pending() {
                    return Err(ProtoError::PlaintextAfterStartTls);
                }
                self.state = State::StartingTls;
                Answer::StartTls
            }
            Kind::SignIn => {
                self.state = State::Authenticated;
                if !fresh {
                    // The capabilities change with the sign-in (RFC 9051 section 6.2.3): read them again.
                    self.capabilities = None;
                    self.start(Command::new("CAPABILITY"), Kind::Capability { after_sign_in: Some(tag.clone()) });
                    return Ok(None);
                }
                Answer::Authenticated(self.capabilities.clone().unwrap_or_default())
            }
            Kind::Enable { asked, words } => {
                let enabled = Capabilities::from_words(words.iter().map(String::as_str));
                if enabled.has(asked.name()) {
                    self.condstore = true;
                    self.qresync |= asked == Extension::QResync;
                }
                Answer::Enabled(enabled)
            }
            Kind::List { entries, .. } => Answer::Listed(entries),
            Kind::Select(Opening { mut status, known, .. }) => {
                match line.code.as_str() {
                    "READ-ONLY" => status.read_only = true,
                    "READ-WRITE" => status.read_only = false,
                    _ => {}
                }
                if let Some(old) = known
                    && status.uid_validity != Some(old)
                {
                    self.events.push_back(Event::UidValidityChanged { old, new: status.uid_validity });
                }
                self.uid_validity = status.uid_validity;
                self.read_only = status.read_only;
                self.state = State::Selected;
                Answer::Selected(status)
            }
            Kind::Fetch => Answer::Fetched,
            Kind::Search(found) => Answer::Searched(found.into_iter().collect()),
            Kind::Move(moving) => {
                self.move_answered(tag.clone(), moving, line, None)?;
                return Ok(None);
            }
            Kind::Copy(asked) => Answer::Copied(copy_uid(line, &asked)),
            Kind::Expunge => Answer::Expunged,
            Kind::Compress => {
                self.compressed = true;
                // Not read here: the caller inflates it first.
                Answer::Compressed(self.framer.take_pending())
            }
            Kind::Idle { .. } => Answer::Idled,
            Kind::Notify { set } => {
                self.notifying = set;
                Answer::Notified
            }
            Kind::Logout { .. } => {
                self.state = State::Closed;
                Answer::LoggedOut
            }
        };
        Ok(Some(answer))
    }
}

/// The COPYUID of a status line, when it has one that fits the UIDs `asked` for.
fn copy_uid(line: &StatusLine, asked: &UidSet) -> Option<CopyUid> {
    (line.code == "COPYUID").then(|| CopyUid::parse(&line.arg, asked)).flatten()
}

/// `* n EXISTS`, `* n RECENT`, `* n EXPUNGE`, `* n FETCH (...)`. EXISTS, EXPUNGE and FETCH belong to a mailbox
/// that is `open` or being opened (RFC 9051 section 7.4); anywhere else, before TLS too, they end the session.
fn message_data(count: u32, values: Vec<Value>, flight: Option<&mut Kind>, open: bool, events: &mut VecDeque<Event>) -> Result<(), ProtoError> {
    let word = values.get(1).and_then(Value::atom).map(str::to_ascii_uppercase).unwrap_or_default();
    if let Some(Kind::Select(Opening { closing: true, .. })) = flight {
        // About the mailbox being closed.
        return Ok(());
    }
    match (word.as_str(), flight) {
        ("EXISTS", _) if !open => return Err(ProtoError::Unexpected("EXISTS response")),
        ("EXPUNGE", _) if !open => return Err(ProtoError::Unexpected("EXPUNGE response")),
        ("FETCH", _) if !open => return Err(ProtoError::Unexpected("FETCH response")),
        ("EXISTS", Some(Kind::Select(Opening { status, .. }))) => status.exists = count,
        ("EXISTS", _) => events.push_back(Event::Exists(count)),
        ("RECENT", Some(Kind::Select(Opening { status, .. }))) => status.recent = count,
        ("EXPUNGE", _) if count != 0 => events.push_back(Event::Expunged(count)),
        ("EXPUNGE", _) => return Err(ProtoError::Malformed("EXPUNGE response")),
        ("FETCH", _) => {
            let item = FetchItem::from_values(values).ok_or(ProtoError::Malformed("FETCH response"))?;
            events.push_back(Event::Fetched(item));
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
