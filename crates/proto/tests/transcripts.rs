// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Golden transcripts: every file in `tests/transcripts/` is a session with invented data, played against the client.
//!
//! One directive per line:
//!
//! - `# ...` a comment; an empty line is nothing.
//! - `@ tls` or `@ plain`: how the connection started (the first directive).
//! - `S: <line>`: the server sends the line and CRLF; consecutive `S:` lines arrive in one piece.
//! - `C: <line>`: the next line the client wrote must be this one.
//! - `> <command> [arguments]`: a call on the client (see [`call`]).
//! - `= <line>`: the next line of the events, as [`render`] writes them, must be this one.
//! - `! <error>`: the last call or the last bytes received failed with this error (`Debug` form).
//! - `? <state>`: the session is in this state (`Debug` form).
//!
//! At the end nothing may be left: no output, no event and no error that a `C:`, `=` or `!` line did not check.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::VecDeque;
use std::fmt::Write as _;
use std::path::Path;

use inboxcraft_model::{FolderState, ModSeq, Uid, UidSet, UidValidity};
use inboxcraft_proto::ProtoError;
use inboxcraft_proto::imap::{
    Answer, Client, CopyUid, Event, Extension, FetchAttributes, ListEntry, MailboxStatus, Notify, NotifyMailboxes, PreviewFetch, Refusal, Removal,
    SearchDate, SearchKey, Section, SelectOptions, Transport,
};

/// A UID set as IMAP writes one: `4,7:9`.
fn uid_set(text: &str) -> UidSet {
    text.split(',')
        .map(|range| {
            let (first, last) = range.split_once(':').unwrap_or((range, range));
            (Uid::new(first.parse().unwrap()).unwrap(), Uid::new(last.parse().unwrap()).unwrap())
        })
        .collect()
}

fn set_text(set: &UidSet) -> String {
    set.ranges().iter().map(|(a, b)| if a == b { a.to_string() } else { format!("{a}:{b}") }).collect::<Vec<_>>().join(",")
}

/// A search key from words: `unseen`, `since 1994-02-01`, `uid 4,7:9`, `subject <word>`, `not <key>`, `or <key>
/// <key>`; several keys are an `And`.
fn search_key(words: &mut VecDeque<&str>) -> SearchKey {
    let word = words.pop_front().unwrap();
    let mut text = || words.pop_front().unwrap().replace('_', " ");
    match word {
        "all" => SearchKey::All,
        "seen" => SearchKey::Seen,
        "unseen" => SearchKey::Unseen,
        "flagged" => SearchKey::Flagged,
        "deleted" => SearchKey::Deleted,
        "undeleted" => SearchKey::Undeleted,
        "since" => {
            let date: Vec<u16> = text().split('-').map(|n| n.parse().unwrap()).collect();
            SearchKey::Since(SearchDate::new(date[0], u8::try_from(date[1]).unwrap(), u8::try_from(date[2]).unwrap()).unwrap())
        }
        "uid" => SearchKey::Uid(uid_set(&text())),
        "subject" => SearchKey::Subject(text()),
        "from" => SearchKey::From(text()),
        "header" => {
            let field = text();
            SearchKey::Header(field, text())
        }
        "not" => SearchKey::Not(Box::new(search_key(words))),
        "or" => {
            let left = search_key(words);
            SearchKey::Or(Box::new(left), Box::new(search_key(words)))
        }
        other => panic!("unknown search key {other}"),
    }
}

/// Options from words: `condstore`, `known=<uidvalidity>[/<modseq>]`, `uids=<set>`.
fn select_options(words: VecDeque<&str>) -> SelectOptions {
    let mut options = SelectOptions::default();
    for word in words {
        if word == "condstore" {
            options.condstore = true;
        } else if let Some(known) = word.strip_prefix("known=") {
            let (validity, modseq) = known.split_once('/').map_or((known, None), |(v, m)| (v, Some(m)));
            let state = FolderState::new(UidValidity::new(validity.parse().unwrap()).unwrap());
            options.known = Some(state.with_highest_modseq(modseq.and_then(|m| ModSeq::new(m.parse().unwrap()))));
        } else if let Some(uids) = word.strip_prefix("uids=") {
            options.known_uids = Some(if uids.is_empty() { UidSet::default() } else { uid_set(uids) });
        } else {
            panic!("unknown select option {word}");
        }
    }
    options
}

/// What NOTIFY asks for, from words: `status`, `selected`, `names`, and one of `personal`, `inboxes`, `subscribed`,
/// `subtree=<name>,<name>`, `mailboxes=<name>,<name>`.
fn notify(words: VecDeque<&str>) -> Notify {
    let mut notify = Notify::default();
    let names = |list: &str| list.split(',').map(str::to_owned).collect();
    for word in words {
        match word {
            "status" => notify.status = true,
            "selected" => notify.selected = true,
            "names" => notify.mailbox_names = true,
            "personal" => notify.mailboxes = Some(NotifyMailboxes::Personal),
            "inboxes" => notify.mailboxes = Some(NotifyMailboxes::Inboxes),
            "subscribed" => notify.mailboxes = Some(NotifyMailboxes::Subscribed),
            other => match other.split_once('=') {
                Some(("subtree", list)) => notify.mailboxes = Some(NotifyMailboxes::Subtree(names(list))),
                Some(("mailboxes", list)) => notify.mailboxes = Some(NotifyMailboxes::Mailboxes(names(list))),
                _ => panic!("unknown notify word {other}"),
            },
        }
    }
    notify
}

/// Runs `> <command>`: `capability`, `starttls`, `tls`, `login <user> <password>`, `authenticate <user> <password>`,
/// `enable condstore|qresync`, `list`, `select <name>`, `examine <name>` (the rest of the line), `select-with <name>
/// <options>` and `examine-with <name> <options>` (see [`select_options`]), `fetch <set> [flags] [date] [size]
/// [modseq] [since=<modseq>] [vanished] [preview|lazy-preview] [full|header|text]`, `search <keys>`, `notify <words>` (see [`notify`]),
/// `notify-none`, `idle`, `done`, `logout`.
fn call(client: &mut Client, line: &str) -> Result<(), ProtoError> {
    let (command, rest) = line.split_once(' ').unwrap_or((line, ""));
    let mut words: VecDeque<&str> = rest.split_whitespace().collect();
    let result = match command {
        "capability" => client.capability(),
        "starttls" => client.starttls(),
        "tls" => client.tls_established(),
        "login" => client.login(words[0], words[1]),
        "authenticate" => client.authenticate_plain(words[0], words[1]),
        "list" => client.list(),
        "select" => client.select(rest),
        "examine" => client.examine(rest),
        "select-with" | "examine-with" => {
            let name = words.pop_front().unwrap();
            let options = select_options(words);
            if command == "select-with" { client.select_with(name, &options) } else { client.examine_with(name, &options) }
        }
        "enable" => client.enable(match rest {
            "condstore" => Extension::CondStore,
            "qresync" => Extension::QResync,
            other => panic!("unknown extension {other}"),
        }),
        "fetch" => {
            let set = uid_set(words.pop_front().unwrap());
            let mut attributes = FetchAttributes::default();
            for word in words {
                match word {
                    "flags" => attributes.flags = true,
                    "date" => attributes.internal_date = true,
                    "size" => attributes.size = true,
                    "modseq" => attributes.modseq = true,
                    "vanished" => attributes.vanished = true,
                    "preview" => attributes.preview = Some(PreviewFetch::Generate),
                    "lazy-preview" => attributes.preview = Some(PreviewFetch::Lazy),
                    "full" => attributes.section = Some(Section::Full),
                    "header" => attributes.section = Some(Section::Header),
                    "text" => attributes.section = Some(Section::Text),
                    other => match other.strip_prefix("since=") {
                        Some(since) => attributes.changed_since = ModSeq::new(since.parse().unwrap()),
                        None => panic!("unknown fetch attribute {other}"),
                    },
                }
            }
            client.uid_fetch(&set, attributes)
        }
        "search" => {
            let mut keys = Vec::new();
            while !words.is_empty() {
                keys.push(search_key(&mut words));
            }
            let key = if keys.len() == 1 { keys.remove(0) } else { SearchKey::And(keys) };
            client.uid_search(&key)
        }
        "copy" => {
            let set = uid_set(words.pop_front().unwrap());
            client.uid_copy(&set, &words.into_iter().collect::<Vec<_>>().join(" "))
        }
        "move" => {
            let set = uid_set(words.pop_front().unwrap());
            client.uid_move(&set, &words.into_iter().collect::<Vec<_>>().join(" "))
        }
        "expunge" => client.uid_expunge(&uid_set(words.pop_front().unwrap())),
        "notify" => client.notify(&notify(words)),
        "notify-none" => client.notify_none(),
        "idle" => client.idle(),
        "done" => return client.done(),
        "logout" => client.logout(),
        other => panic!("unknown command {other}"),
    };
    result.map(|_| ())
}

fn list_text(items: &[String]) -> String {
    if items.is_empty() { "-".to_owned() } else { items.join(",") }
}

fn mailbox(entry: &ListEntry) -> String {
    let special: Vec<String> = entry.special_use.iter().map(|use_| format!("{use_:?}")).collect();
    format!(
        "  mailbox {:?} name={:?} delimiter={} attributes={} special-use={} selectable={}",
        entry.raw,
        entry.name,
        entry.delimiter.map_or("none".to_owned(), |d| format!("{d:?}")),
        list_text(&entry.attributes),
        list_text(&special),
        entry.is_selectable()
    )
}

fn status(status: &MailboxStatus) -> String {
    let some = |value: Option<String>| value.unwrap_or_else(|| "none".to_owned());
    format!(
        "selected exists={} recent={} flags={} permanent={} uidvalidity={} uidnext={} unseen={} modseq={} read-only={}{}",
        status.exists,
        status.recent,
        list_text(&status.flags),
        list_text(&status.permanent_flags),
        some(status.uid_validity.map(|v| v.to_string())),
        some(status.uid_next.map(|v| v.to_string())),
        some(status.unseen.map(|v| v.to_string())),
        some(status.highest_modseq.map(|v| v.to_string())),
        status.read_only,
        if status.uid_not_sticky { " uidnotsticky" } else { "" }
    )
}

/// Ranges as the server wrote them, backwards ones too: `304,6:4`.
fn ranges_text(ranges: &[(Uid, Uid)]) -> String {
    ranges.iter().map(|(a, b)| if a == b { a.to_string() } else { format!("{a}:{b}") }).collect::<Vec<_>>().join(",")
}

fn copy_uid(copy_uid: Option<&CopyUid>) -> String {
    copy_uid.map_or(String::new(), |c| format!(" {} {} -> {}", c.uid_validity(), ranges_text(c.source()), ranges_text(c.destination())))
}

fn refusal_text(refusal: &Refusal) -> String {
    let code = refusal.code.as_ref().map(|code| format!(" [{code}]")).unwrap_or_default();
    format!("{:?}{code} {}", refusal.kind, refusal.text).trim_end().to_owned()
}

fn removal(removal: &Removal) -> String {
    match removal {
        Removal::Moved => "by move".to_owned(),
        Removal::Expunged => "by copy and expunge".to_owned(),
        Removal::MarkedDeleted => "marked deleted".to_owned(),
        Removal::NotMarked(refusal) => format!("kept, not marked: {}", refusal_text(refusal)),
        Removal::NotExpunged(refusal) => format!("kept, not expunged: {}", refusal_text(refusal)),
    }
}

/// The lines an event is written as.
fn render(event: &Event) -> Vec<String> {
    match event {
        Event::Greeted { preauth: false } => vec!["greeted".to_owned()],
        Event::Greeted { preauth: true } => vec!["greeted preauth".to_owned()],
        Event::Exists(count) => vec![format!("exists {count}")],
        Event::Expunged(seq) => vec![format!("expunged {seq}")],
        Event::Alert(text) => vec![format!("alert {text}")],
        Event::Idling => vec!["idling".to_owned()],
        Event::NotificationOverflow => vec!["notifications overflow".to_owned()],
        Event::MailboxChanged(entry) => vec![format!("changed {}", mailbox(entry).trim_start())],
        Event::Status(counts) => {
            let some = |value: Option<String>| value.unwrap_or_else(|| "none".to_owned());
            vec![format!(
                "status {:?} name={:?} messages={} uidnext={} uidvalidity={} unseen={} modseq={}",
                counts.raw,
                counts.name,
                some(counts.messages.map(|v| v.to_string())),
                some(counts.uid_next.map(|v| v.to_string())),
                some(counts.uid_validity.map(|v| v.to_string())),
                some(counts.unseen.map(|v| v.to_string())),
                some(counts.highest_modseq.map(|v| v.to_string())),
            )]
        }
        Event::Vanished { uids, earlier: true } => vec![format!("vanished earlier {}", set_text(uids))],
        Event::Vanished { uids, earlier: false } => vec![format!("vanished {}", set_text(uids))],
        Event::UidValidityChanged { old, new } => {
            vec![format!("uidvalidity changed {old} -> {}", new.map_or("none".to_owned(), |new| new.to_string()))]
        }
        Event::Fetched(item) => {
            let mut line = format!("fetch seq={}", item.seq);
            if let Some(uid) = item.uid {
                let _ = write!(line, " uid={uid}");
            }
            if let Some(flags) = &item.flags {
                let _ = write!(line, " flags={}", list_text(flags));
            }
            if let Some(date) = item.internal_date {
                let _ = write!(line, " date={}", date.unix_seconds());
            }
            if let Some(size) = item.size {
                let _ = write!(line, " size={size}");
            }
            if let Some(modseq) = item.modseq {
                let _ = write!(line, " modseq={modseq}");
            }
            if let Some(preview) = &item.preview {
                let _ = write!(line, " preview={preview:?}");
            }
            if let Some((section, bytes)) = &item.section {
                let _ = write!(line, " {section:?}=\"{}\"", bytes.escape_ascii());
            }
            vec![line]
        }
        Event::Completed { tag, outcome: Err(refusal) } => vec![format!("{tag} {}", refusal_text(refusal))],
        Event::Completed { tag, outcome: Ok(answer) } => match answer {
            Answer::Capabilities(caps) => vec![format!("{tag} capabilities {}", caps.iter().collect::<Vec<_>>().join(" ")).trim_end().to_owned()],
            Answer::StartTls => vec![format!("{tag} starttls")],
            Answer::Enabled(enabled) => vec![format!("{tag} enabled {}", list_text(&enabled.iter().map(str::to_owned).collect::<Vec<_>>()))],
            Answer::Authenticated(caps) => vec![format!("{tag} authenticated {}", caps.iter().collect::<Vec<_>>().join(" ")).trim_end().to_owned()],
            Answer::Listed(entries) => {
                let mut lines = vec![format!("{tag} listed {}", entries.len())];
                lines.extend(entries.iter().map(mailbox));
                lines
            }
            Answer::Selected(selected) => vec![format!("{tag} {}", status(selected))],
            Answer::Fetched => vec![format!("{tag} fetched")],
            Answer::Searched(set) => vec![format!("{tag} searched {}", if set.is_empty() { "-".to_owned() } else { set_text(set) })],
            Answer::Copied(copied) => vec![format!("{tag} copied{}", copy_uid(copied.as_ref()))],
            Answer::Moved(moved) => vec![format!("{tag} moved{} {}", copy_uid(moved.copy_uid.as_ref()), removal(&moved.removal))],
            Answer::Expunged => vec![format!("{tag} expunged")],
            Answer::Idled => vec![format!("{tag} idled")],
            Answer::Notified => vec![format!("{tag} notified")],
            Answer::LoggedOut => vec![format!("{tag} logged out")],
        },
    }
}

/// What a transcript has seen and not checked yet.
struct Session {
    client: Client,
    output: Vec<u8>,
    events: VecDeque<String>,
    error: Option<ProtoError>,
    incoming: Vec<u8>,
}

impl Session {
    fn collect(&mut self) {
        self.output.extend(self.client.take_output());
        while let Some(event) = self.client.next_event() {
            self.events.extend(render(&event));
        }
    }

    fn flush(&mut self, at: &str) {
        if !self.incoming.is_empty() {
            assert!(self.error.is_none(), "{at}: unchecked error {:?}", self.error);
            let incoming = std::mem::take(&mut self.incoming);
            self.error = self.client.receive(&incoming).err();
        }
        self.collect();
    }

    fn output_line(&mut self, at: &str) -> String {
        let end = self
            .output
            .windows(2)
            .position(|w| w == b"\r\n")
            .unwrap_or_else(|| panic!("{at}: no line written, have {:?}", self.output.escape_ascii().to_string()));
        let line: Vec<u8> = self.output.drain(..end + 2).take(end).collect();
        String::from_utf8_lossy(&line).into_owned()
    }
}

fn play(path: &Path) {
    let name = path.file_name().unwrap().to_string_lossy().into_owned();
    let text = std::fs::read_to_string(path).unwrap();
    let mut session: Option<Session> = None;
    for (n, line) in text.lines().enumerate() {
        let at = format!("{name}:{}", n + 1);
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let (directive, rest) = line.split_once(' ').unwrap_or((line, ""));
        if directive == "@" {
            let transport = match rest {
                "tls" => Transport::Tls,
                "plain" => Transport::Plain,
                other => panic!("{at}: unknown transport {other}"),
            };
            if let Some(done) = session.take() {
                finish(done, &at);
            }
            session =
                Some(Session { client: Client::new(transport), output: Vec::new(), events: VecDeque::new(), error: None, incoming: Vec::new() });
            continue;
        }
        let session = session.as_mut().unwrap_or_else(|| panic!("{at}: no `@` line first"));
        if directive == "S:" {
            session.incoming.extend_from_slice(rest.as_bytes());
            session.incoming.extend_from_slice(b"\r\n");
            continue;
        }
        session.flush(&at);
        match directive {
            "C:" => assert_eq!(session.output_line(&at), rest, "{at}"),
            "=" => assert_eq!(session.events.pop_front().as_deref(), Some(rest), "{at}"),
            "!" => assert_eq!(session.error.take().map(|e| format!("{e:?}")).as_deref(), Some(rest), "{at}"),
            "?" => assert_eq!(format!("{:?}", session.client.state()), rest, "{at}"),
            ">" => {
                assert!(session.error.is_none(), "{at}: unchecked error {:?}", session.error);
                session.error = call(&mut session.client, rest).err();
                session.collect();
            }
            other => panic!("{at}: unknown directive {other}"),
        }
    }
    finish(session.unwrap_or_else(|| panic!("{name}: empty")), &format!("{name}:end"));
}

/// Checks that a session left nothing unchecked.
fn finish(mut session: Session, at: &str) {
    session.flush(at);
    assert!(session.error.is_none(), "{at}: unchecked error {:?}", session.error);
    assert!(session.events.is_empty(), "{at}: unchecked events {:?}", session.events);
    assert!(session.output.is_empty(), "{at}: unchecked output {:?}", session.output.escape_ascii().to_string());
}

#[test]
fn every_transcript_plays() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/transcripts");
    let mut paths: Vec<_> =
        std::fs::read_dir(&dir).unwrap().map(|entry| entry.unwrap().path()).filter(|p| p.extension().is_some_and(|e| e == "txt")).collect();
    paths.sort();
    assert!(paths.len() >= 15, "{} transcripts", paths.len());
    for path in &paths {
        play(path);
    }
}
