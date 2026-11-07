// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! One client's session with the fake: requests read one after the other on a kept-alive connection, each answered.

use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::base64;
use crate::dav::calendars::State;
use crate::dav::props::{self, CALDAV, DAV, DEFAULT_PROPS, Kind, Multistatus};
use crate::dav::request::{self, Request};
use crate::dav::rules::{Action, Reply};
use crate::dav::xml::{self, Element};
use crate::dav::{FakeDav, HOME, WELL_KNOWN};
use crate::line::LineReader;
use crate::server::{Ending, Transcript, pause};
use crate::{Error, Fault};

/// What playing the actions of a rule did.
enum Played {
    /// Nothing that changes the answer.
    Go,
    /// The script's answer was sent instead of the server's own.
    Replied,
    /// The session is over.
    End(Ending),
}

pub(crate) struct Session {
    fake: FakeDav,
    tls: bool,
    reader: LineReader,
    transcript: Transcript,
}

impl Session {
    pub(crate) fn new(fake: FakeDav, tls: bool) -> Self {
        Self { fake, tls, reader: LineReader::default(), transcript: Transcript::default() }
    }

    /// Records the head lines of every request into `transcript`, from the start.
    pub(crate) fn recording(self, transcript: Transcript) -> Self {
        Self { transcript, ..self }
    }

    pub(crate) fn transcript(&self) -> Transcript {
        self.transcript.clone()
    }

    /// The fake as the session left it: its played rules gone.
    pub(crate) fn into_fake(self) -> FakeDav {
        self.fake
    }

    /// Serves the client until it closes the connection, asks to close it, or sends what the fake does not read.
    pub(crate) fn run(&mut self, io: &mut (impl Read + Write), stop: &AtomicBool) -> Result<Ending, Error> {
        loop {
            if stop.load(Ordering::Relaxed) {
                return Ok(Ending::Dropped);
            }
            let (tls, transcript) = (self.tls, &mut self.transcript);
            let read = request::read(&mut self.reader, io, &mut |line: &[u8]| transcript.push(String::from_utf8_lossy(line).into_owned(), tls))?;
            let request = match read {
                request::Read::Request(request) => request,
                request::Read::Closed => return Ok(Ending::Dropped),
                request::Read::Refuse(status) => {
                    send(io, &Reply::new(status).field("Connection", "close"))?;
                    return Ok(Ending::Finished);
                }
            };
            match self.play(io, &request, stop)? {
                Played::End(ending) => return Ok(ending),
                Played::Replied => {}
                Played::Go => send(io, &self.answer(&request))?,
            }
            if request.field("Connection").is_some_and(|value| value.eq_ignore_ascii_case("close")) {
                return Ok(Ending::Finished);
            }
        }
    }

    fn play(&mut self, io: &mut impl Write, request: &Request, stop: &AtomicBool) -> Result<Played, Error> {
        let mut played = Played::Go;
        for action in self.fake.rules.take(&request.method, &request.path) {
            match action {
                Action::Fault(Fault::Drop) => return Ok(Played::End(Ending::Dropped)),
                Action::Fault(Fault::Delay(wait)) => {
                    if pause(wait, stop) {
                        return Ok(Played::End(Ending::Dropped));
                    }
                }
                Action::Fault(Fault::Garbage(bytes)) => {
                    io.write_all(&bytes)?;
                    io.flush()?;
                }
                Action::Reply(reply) => {
                    send(io, &reply)?;
                    played = Played::Replied;
                }
            }
        }
        Ok(played)
    }

    /// The server's own answer to `request`.
    fn answer(&self, request: &Request) -> Reply {
        if !self.tls && !self.fake.auth_in_clear {
            return Reply::new(403).field("Content-Type", "text/plain").body(b"This server takes requests over TLS only.\r\n".to_vec());
        }
        if request.path == WELL_KNOWN {
            return if self.fake.well_known { Reply::new(301).field("Location", "/") } else { Reply::new(404) };
        }
        if !self.signed_in(request) {
            return Reply::new(401).field("WWW-Authenticate", "Basic realm=\"example.org\", charset=\"UTF-8\"");
        }
        let calendars = self.fake.calendars.clone();
        let mut state = calendars.lock();
        match request.method.as_str() {
            "OPTIONS" => Reply::new(200).field("DAV", "1, 3, calendar-access").field("Allow", "OPTIONS, GET, PUT, DELETE, PROPFIND, REPORT"),
            "PROPFIND" => self.propfind(&state, request),
            "REPORT" => self.report(&state, request),
            "GET" => get(&state, &request.path),
            "PUT" => put(&mut state, request),
            "DELETE" => delete(&mut state, request),
            _ => Reply::new(405),
        }
    }

    fn signed_in(&self, request: &Request) -> bool {
        let Some(value) = request.field("Authorization") else { return false };
        let Some((scheme, encoded)) = value.split_once(' ') else { return false };
        let expected = format!("{}:{}", self.fake.user, self.fake.password);
        scheme.eq_ignore_ascii_case("basic") && base64::decode(encoded.trim().as_bytes()).is_some_and(|decoded| decoded == expected.as_bytes())
    }

    fn token(&self, state: &State) -> Option<String> {
        self.fake.sync.then(|| state.token(state.seq()))
    }

    fn propfind(&self, state: &State, request: &Request) -> Reply {
        let Some(kind) = Kind::of(state, &request.path) else { return Reply::new(404) };
        let depth = match request.field("Depth") {
            Some("0") => 0,
            Some("1") => 1,
            _ => return xml_reply(403, props::error(DAV, "propfind-finite-depth")),
        };
        let Some(props) = asked(&request.body) else { return Reply::new(400) };
        let token = self.token(state);
        let mut answer = Multistatus::new();
        answer.props(&request.path, kind, &props, token.as_deref());
        if depth == 1 {
            for (href, member) in members(state, kind) {
                answer.props(&href, member, &props, token.as_deref());
            }
        }
        xml_reply(207, answer.finish(None))
    }

    fn report(&self, state: &State, request: &Request) -> Reply {
        let Some(kind) = Kind::of(state, &request.path) else { return Reply::new(404) };
        let Some(root) = xml::parse(&request.body) else { return Reply::new(400) };
        let props = root.child(DAV, "prop").map(names).unwrap_or_else(|| vec![(DAV.to_owned(), "getetag".to_owned())]);
        match (kind, root.ns.as_str(), root.name.as_str()) {
            (Kind::Calendar(_), DAV, "sync-collection") if self.fake.sync => self.sync(state, request, &root, &props),
            (Kind::Calendar(_), CALDAV, "calendar-multiget") => {
                let mut answer = Multistatus::new();
                for href in root.children.iter().filter(|child| child.is(DAV, "href")) {
                    let path = request::decode(href.text.trim()).unwrap_or_default();
                    match state.object(&path).filter(|_| path.starts_with(&request.path)) {
                        Some(object) => answer.props(&path, Kind::Object(object), &props, None),
                        None => answer.status(href.text.trim(), "404 Not Found"),
                    }
                }
                xml_reply(207, answer.finish(None))
            }
            (Kind::Calendar(_), CALDAV, "calendar-query") => {
                let mut answer = Multistatus::new();
                for (href, member) in members(state, kind) {
                    answer.props(&href, member, &props, None);
                }
                xml_reply(207, answer.finish(None))
            }
            _ => xml_reply(403, props::error(DAV, "supported-report")),
        }
    }

    fn sync(&self, state: &State, request: &Request, root: &Element, props: &[(String, String)]) -> Reply {
        if request.field("Depth").is_some_and(|depth| depth != "0") {
            return Reply::new(400);
        }
        let token = root.child(DAV, "sync-token").map(|token| token.text.trim().to_owned()).unwrap_or_default();
        let Some((log, newest)) = state.since(&request.path, &token) else { return xml_reply(403, props::error(DAV, "valid-sync-token")) };
        let page = self.fake.page.unwrap_or(usize::MAX).max(1);
        let mut answer = Multistatus::new();
        for (href, object) in log.iter().take(page) {
            match object {
                Some(object) => answer.props(href, Kind::Object(object), props, None),
                None => answer.status(href, "404 Not Found"),
            }
        }
        let last = match log.get(page.saturating_sub(1)).filter(|_| log.len() > page) {
            Some((href, object)) => {
                answer.status(&request.path, "507 Insufficient Storage");
                state.seq_of(&request.path, href, object.as_ref())
            }
            None => newest,
        };
        xml_reply(207, answer.finish(Some(&state.token(last))))
    }
}

/// The properties a PROPFIND body asks for; `None` for a body the fake does not read.
fn asked(body: &[u8]) -> Option<Vec<(String, String)>> {
    let default = || DEFAULT_PROPS.iter().map(|(ns, name)| ((*ns).to_owned(), (*name).to_owned())).collect();
    if body.iter().all(u8::is_ascii_whitespace) {
        return Some(default());
    }
    let root = xml::parse(body).filter(|root| root.is(DAV, "propfind"))?;
    Some(root.child(DAV, "prop").map_or_else(default, names))
}

/// The names of the properties in a `DAV:prop`.
fn names(prop: &Element) -> Vec<(String, String)> {
    prop.children.iter().map(|child| (child.ns.clone(), child.name.clone())).collect()
}

/// The members of the collection at `path`, with their paths.
fn members<'a>(state: &'a State, kind: Kind<'a>) -> Vec<(String, Kind<'a>)> {
    match kind {
        Kind::Home => state
            .collections
            .iter()
            .filter(|(calendar, _)| calendar.strip_prefix(HOME).is_some_and(|name| name.trim_end_matches('/').split('/').count() == 1))
            .map(|(calendar, collection)| (calendar.clone(), Kind::Calendar(collection)))
            .collect(),
        Kind::Calendar(collection) => collection.objects.iter().map(|(href, object)| (href.clone(), Kind::Object(object))).collect(),
        Kind::Root | Kind::Principal | Kind::Object(_) => Vec::new(),
    }
}

fn get(state: &State, path: &str) -> Reply {
    match state.object(path) {
        Some(object) => {
            Reply::new(200).field("Content-Type", "text/calendar; charset=utf-8").field("ETag", &object.etag).body(object.data.clone().into_bytes())
        }
        None => Reply::new(404),
    }
}

fn put(state: &mut State, request: &Request) -> Reply {
    let path = request.path.as_str();
    if state.collections.contains_key(path) || matches!(path, "/" | crate::dav::PRINCIPAL | HOME) {
        return Reply::new(405);
    }
    let parent = path.rfind('/').and_then(|at| path.get(..=at)).unwrap_or("/");
    if !state.collections.contains_key(parent) || path.ends_with('/') {
        return Reply::new(409);
    }
    let current = state.object(path).map(|object| object.etag.clone());
    if !precondition(request, current.as_deref()) {
        return Reply::new(412);
    }
    let Some(data) = std::str::from_utf8(&request.body).ok().filter(|data| data.trim_start().starts_with("BEGIN:VCALENDAR")) else {
        return xml_reply(403, props::error(CALDAV, "valid-calendar-data"));
    };
    let status = if current.is_some() { 204 } else { 201 };
    state.put(path, data).map_or_else(|| Reply::new(409), |etag| Reply::new(status).field("ETag", &etag))
}

fn delete(state: &mut State, request: &Request) -> Reply {
    let Some(current) = state.object(&request.path).map(|object| object.etag.clone()) else {
        return Reply::new(if Kind::of(state, &request.path).is_some() { 403 } else { 404 });
    };
    if !precondition(request, Some(&current)) {
        return Reply::new(412);
    }
    state.remove(&request.path);
    Reply::new(204)
}

/// Whether `If-Match` and `If-None-Match` hold for a resource whose entity tag is `current` (`None`: not there).
fn precondition(request: &Request, current: Option<&str>) -> bool {
    let listed = |value: &str| value.split(',').map(str::trim).any(|etag| etag == "*" || Some(etag) == current);
    let matched = request.field("If-Match").is_none_or(|value| current.is_some() && listed(value));
    let none_matched = request.field("If-None-Match").is_none_or(|value| current.is_none() || !listed(value));
    matched && none_matched
}

fn xml_reply(status: u16, body: Vec<u8>) -> Reply {
    Reply::new(status).field("Content-Type", "application/xml; charset=utf-8").body(body)
}

/// Writes `reply` with its `Content-Length`.
pub(crate) fn send(io: &mut impl Write, reply: &Reply) -> io::Result<()> {
    let mut out = format!("HTTP/1.1 {} {}\r\n", reply.status, reason(reply.status));
    for (name, value) in &reply.fields {
        out.push_str(&format!("{name}: {value}\r\n"));
    }
    out.push_str(&format!("Content-Length: {}\r\n\r\n", reply.body.len()));
    let mut bytes = out.into_bytes();
    bytes.extend_from_slice(&reply.body);
    io.write_all(&bytes)?;
    io.flush()
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        207 => "Multi-Status",
        301 => "Moved Permanently",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        412 => "Precondition Failed",
        413 => "Content Too Large",
        414 => "URI Too Long",
        431 => "Request Header Fields Too Large",
        501 => "Not Implemented",
        _ => "Status",
    }
}
