// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! HTTP/1.1 (RFC 9112) without I/O: requests written as bytes, responses read from bytes.

use std::fmt;

use crate::{Credentials, DavError, Origin};

/// The longest response head (status line and header fields) read, in bytes.
pub const MAX_HEAD: usize = 64 * 1024;
/// How many header fields one response may carry.
pub const MAX_HEADERS: usize = 128;
/// The largest response body read, in bytes.
pub const MAX_BODY: usize = 16 * 1024 * 1024;

/// The longest chunk-size line, in bytes.
const MAX_CHUNK_LINE: usize = 1024;

/// The request methods the client uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Method {
    /// PROPFIND (RFC 4918 section 9.1).
    Propfind,
    /// REPORT (RFC 3253 section 3.6).
    Report,
    /// GET.
    Get,
    /// PUT.
    Put,
    /// DELETE.
    Delete,
}

impl Method {
    /// The method's name on the wire.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Propfind => "PROPFIND",
            Self::Report => "REPORT",
            Self::Get => "GET",
            Self::Put => "PUT",
            Self::Delete => "DELETE",
        }
    }
}

impl fmt::Display for Method {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One request to the server: its method, origin, path, header fields and body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    method: Method,
    origin: Origin,
    path: String,
    headers: Vec<(&'static str, String)>,
    body: Vec<u8>,
}

impl Request {
    /// A request without a body; `path` is in its one form ([`Origin::resolve`]).
    pub(crate) fn new(method: Method, origin: &Origin, path: &str) -> Self {
        Self { method, origin: origin.clone(), path: path.to_owned(), headers: Vec::new(), body: Vec::new() }
    }

    /// Adds a header field; the caller has checked that `value` holds no control character.
    pub(crate) fn with_header(mut self, name: &'static str, value: impl Into<String>) -> Self {
        self.headers.push((name, value.into()));
        self
    }

    /// Sets the body and its `Content-Type`.
    pub(crate) fn with_body(self, content_type: &'static str, body: impl Into<Vec<u8>>) -> Self {
        Self { body: body.into(), ..self.with_header("Content-Type", content_type) }
    }

    /// The method.
    #[must_use]
    pub fn method(&self) -> Method {
        self.method
    }

    /// The origin the request goes to: the caller connects there.
    #[must_use]
    pub fn origin(&self) -> &Origin {
        &self.origin
    }

    /// The path on the origin.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// The value of the header field `name` (case-insensitive), as it will be written.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(written, _)| written.eq_ignore_ascii_case(name)).map(|(_, value)| value.as_str())
    }

    /// The body.
    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    /// Writes the request as HTTP/1.1, with `Host`, the header fields, `Content-Length` and, when `credentials` are
    /// given, `Authorization: Basic`.
    ///
    /// # Errors
    ///
    /// [`DavError::NotEncrypted`] when `credentials` are given and the origin is not `https`: a password never goes
    /// in the clear.
    pub fn encode(&self, credentials: Option<&Credentials>) -> Result<Vec<u8>, DavError> {
        let mut head = format!("{} {} HTTP/1.1\r\nHost: {}\r\n", self.method, self.path, self.origin.authority());
        if let Some(credentials) = credentials {
            if !self.origin.is_encrypted() {
                return Err(DavError::NotEncrypted);
            }
            head.push_str(&format!("Authorization: {}\r\n", credentials.authorization()));
        }
        for (name, value) in &self.headers {
            head.push_str(&format!("{name}: {value}\r\n"));
        }
        if !self.body.is_empty() || self.method == Method::Put {
            head.push_str(&format!("Content-Length: {}\r\n", self.body.len()));
        }
        head.push_str("\r\n");
        let mut bytes = head.into_bytes();
        bytes.extend_from_slice(&self.body);
        Ok(bytes)
    }
}

/// One response from the server.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Response {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Response {
    /// A response with `status`, no header fields and no body: for tests and for callers that build one.
    #[must_use]
    pub fn new(status: u16) -> Self {
        Self { status, headers: Vec::new(), body: Vec::new() }
    }

    /// Adds a header field.
    #[must_use]
    pub fn with_header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_owned(), value.to_owned()));
        self
    }

    /// Sets the body.
    #[must_use]
    pub fn with_body(self, body: impl Into<Vec<u8>>) -> Self {
        Self { body: body.into(), ..self }
    }

    /// The status code.
    #[must_use]
    pub fn status(&self) -> u16 {
        self.status
    }

    /// The value of the first header field `name` (case-insensitive).
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(written, _)| written.eq_ignore_ascii_case(name)).map(|(_, value)| value.as_str())
    }

    /// The body, transfer coding undone.
    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    /// Whether the server closes the connection after this response (`Connection: close`).
    #[must_use]
    pub fn closes(&self) -> bool {
        self.header("Connection").is_some_and(|value| value.split(',').any(|token| token.trim().eq_ignore_ascii_case("close")))
    }

    /// Whether the status is a success (2xx).
    pub(crate) fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

/// How the body of the response being read ends.
#[derive(Debug)]
enum Framing {
    Length(usize),
    Chunked(Chunks),
    UntilClose,
}

/// Where a chunked body is.
#[derive(Debug, Default)]
enum Chunk {
    /// Waiting for a chunk-size line.
    #[default]
    Size,
    /// This many bytes of chunk data to come.
    Data(usize),
    /// The CRLF after a chunk's data.
    DataEnd,
    /// The trailer fields after the last chunk.
    Trailer,
}

#[derive(Debug, Default)]
struct Chunks {
    at: Chunk,
    body: Vec<u8>,
    trailer_fields: usize,
    done: bool,
}

/// Reads responses from the bytes the server sends: feed them to [`ResponseReader::receive`], and tell
/// [`ResponseReader::end`] when the server closed the connection.
///
/// Everything is capped: the head at [`MAX_HEAD`] bytes and [`MAX_HEADERS`] fields, the body at [`MAX_BODY`] bytes.
/// Interim responses (1xx) are skipped. A response the client cannot read is an error, never a panic.
#[derive(Debug, Default)]
pub struct ResponseReader {
    buf: Vec<u8>,
    /// How much of `buf` was searched for the end of the head already.
    scanned: usize,
    head: Option<(Response, Framing)>,
}

impl ResponseReader {
    /// A reader waiting for a response.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Takes `bytes` from the server and returns the response once it is whole. Bytes after it are kept for the
    /// next response: call again with no bytes to read it.
    ///
    /// # Errors
    ///
    /// [`DavError::BadResponse`] when the bytes are not an HTTP/1.1 response, [`DavError::TooLarge`] past a cap.
    pub fn receive(&mut self, bytes: &[u8]) -> Result<Option<Response>, DavError> {
        self.buf.extend_from_slice(bytes);
        loop {
            if self.head.is_none() {
                let Some(head) = self.read_head()? else { return Ok(None) };
                if (100..200).contains(&head.0.status) {
                    continue;
                }
                self.head = Some(head);
            }
            return self.read_body();
        }
    }

    /// The server closed the connection: returns the response whose body ends there, or `None` when no response was
    /// being read.
    ///
    /// # Errors
    ///
    /// [`DavError::BadResponse`] when the connection closed inside a response.
    pub fn end(&mut self) -> Result<Option<Response>, DavError> {
        match self.head.take() {
            None if self.buf.is_empty() => Ok(None),
            Some((response, Framing::UntilClose)) => Ok(Some(Response { body: std::mem::take(&mut self.buf), ..response })),
            _ => Err(DavError::BadResponse("the connection closed inside a response")),
        }
    }

    fn read_head(&mut self) -> Result<Option<(Response, Framing)>, DavError> {
        let Some(end) = head_end(&self.buf, self.scanned.saturating_sub(3)) else {
            self.scanned = self.buf.len();
            if self.buf.len() > MAX_HEAD {
                return Err(DavError::TooLarge { what: "response head", limit: MAX_HEAD });
            }
            return Ok(None);
        };
        if end > MAX_HEAD {
            return Err(DavError::TooLarge { what: "response head", limit: MAX_HEAD });
        }
        let head: Vec<u8> = self.buf.drain(..end).collect();
        self.scanned = 0;
        let text = std::str::from_utf8(&head).map_err(|_| DavError::BadResponse("a head that is not text"))?;
        let mut lines = text.split('\n').map(|line| line.strip_suffix('\r').unwrap_or(line));
        let status = status_line(lines.next().unwrap_or_default())?;
        let mut response = Response::new(status);
        for line in lines.filter(|line| !line.is_empty()) {
            if line.starts_with([' ', '\t']) {
                return Err(DavError::BadResponse("a folded header field"));
            }
            let (name, value) = line.split_once(':').ok_or(DavError::BadResponse("a header field without a colon"))?;
            if name.is_empty() || !name.bytes().all(is_token) {
                return Err(DavError::BadResponse("a bad header name"));
            }
            if response.headers.len() >= MAX_HEADERS {
                return Err(DavError::TooLarge { what: "header count", limit: MAX_HEADERS });
            }
            response.headers.push((name.to_owned(), value.trim_matches([' ', '\t']).to_owned()));
        }
        let framing = framing(&response)?;
        Ok(Some((response, framing)))
    }

    fn read_body(&mut self) -> Result<Option<Response>, DavError> {
        let Some((_, framing)) = &mut self.head else { return Ok(None) };
        let body = match framing {
            Framing::Length(len) => {
                if self.buf.len() < *len {
                    return Ok(None);
                }
                self.buf.drain(..*len).collect()
            }
            Framing::UntilClose => {
                if self.buf.len() > MAX_BODY {
                    return Err(DavError::TooLarge { what: "response body", limit: MAX_BODY });
                }
                return Ok(None);
            }
            Framing::Chunked(chunks) => {
                let used = chunks.read(&self.buf)?;
                self.buf.drain(..used);
                if !chunks.done {
                    return Ok(None);
                }
                std::mem::take(&mut chunks.body)
            }
        };
        Ok(self.head.take().map(|(response, _)| Response { body, ..response }))
    }
}

impl Chunks {
    /// Reads what it can of `buf`, returns how many bytes it used. The body is whole once `done` is set.
    fn read(&mut self, buf: &[u8]) -> Result<usize, DavError> {
        let mut used = 0;
        loop {
            let rest = buf.get(used..).unwrap_or_default();
            match self.at {
                Chunk::Size => {
                    let Some(line) = line(rest, MAX_CHUNK_LINE, "chunk-size line")? else { return Ok(used) };
                    used = used.saturating_add(line.len());
                    let size = chunk_size(line)?;
                    if self.body.len().saturating_add(size) > MAX_BODY {
                        return Err(DavError::TooLarge { what: "response body", limit: MAX_BODY });
                    }
                    self.at = if size == 0 { Chunk::Trailer } else { Chunk::Data(size) };
                }
                Chunk::Data(left) => {
                    let take = left.min(rest.len());
                    self.body.extend_from_slice(rest.get(..take).unwrap_or_default());
                    used = used.saturating_add(take);
                    if take < left {
                        self.at = Chunk::Data(left.saturating_sub(take));
                        return Ok(used);
                    }
                    self.at = Chunk::DataEnd;
                }
                Chunk::DataEnd => {
                    let end = match rest {
                        [b'\r', b'\n', ..] => 2,
                        [b'\n', ..] => 1,
                        [] | [b'\r'] => return Ok(used),
                        _ => return Err(DavError::BadResponse("a chunk without its end")),
                    };
                    used = used.saturating_add(end);
                    self.at = Chunk::Size;
                }
                Chunk::Trailer => {
                    let Some(line) = line(rest, MAX_CHUNK_LINE, "trailer field")? else { return Ok(used) };
                    used = used.saturating_add(line.len());
                    if matches!(line, b"\r\n" | b"\n") {
                        self.done = true;
                        return Ok(used);
                    }
                    self.trailer_fields = self.trailer_fields.saturating_add(1);
                    if self.trailer_fields > MAX_HEADERS {
                        return Err(DavError::TooLarge { what: "header count", limit: MAX_HEADERS });
                    }
                }
            }
        }
    }
}

/// The next line of `rest` with its LF, `None` when it has not come whole yet.
fn line<'a>(rest: &'a [u8], limit: usize, what: &'static str) -> Result<Option<&'a [u8]>, DavError> {
    match rest.iter().position(|&byte| byte == b'\n') {
        Some(at) if at < limit.saturating_add(2) => Ok(rest.get(..=at)),
        Some(_) => Err(DavError::TooLarge { what, limit }),
        None if rest.len() > limit.saturating_add(1) => Err(DavError::TooLarge { what, limit }),
        None => Ok(None),
    }
}

fn chunk_size(line: &[u8]) -> Result<usize, DavError> {
    let text = std::str::from_utf8(line).map_err(|_| DavError::BadResponse("a bad chunk size"))?;
    let digits = text.split(';').next().unwrap_or_default().trim_matches([' ', '\t', '\r', '\n']);
    if digits.is_empty() || digits.len() > 8 || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(DavError::BadResponse("a bad chunk size"));
    }
    usize::from_str_radix(digits, 16).map_err(|_| DavError::BadResponse("a bad chunk size"))
}

/// Where the head ends: the index after its empty line. The search starts at `from`.
fn head_end(buf: &[u8], from: usize) -> Option<usize> {
    let mut at = from;
    while let Some(found) = buf.get(at..)?.iter().position(|&byte| byte == b'\n') {
        let next = at.saturating_add(found).saturating_add(1);
        match buf.get(next..) {
            Some([b'\n', ..]) => return Some(next.saturating_add(1)),
            Some([b'\r', b'\n', ..]) => return Some(next.saturating_add(2)),
            _ => at = next,
        }
    }
    None
}

fn status_line(line: &str) -> Result<u16, DavError> {
    let mut parts = line.splitn(3, ' ');
    let version = parts.next().unwrap_or_default();
    let code = parts.next().unwrap_or_default();
    if !version.starts_with("HTTP/1.") {
        return Err(DavError::BadResponse("no status line"));
    }
    match code.parse::<u16>() {
        Ok(status) if code.len() == 3 && (100..600).contains(&status) => Ok(status),
        _ => Err(DavError::BadResponse("a bad status code")),
    }
}

/// How the body of `response` ends (RFC 9112 section 6.3).
fn framing(response: &Response) -> Result<Framing, DavError> {
    if matches!(response.status, 100..=199 | 204 | 304) {
        return Ok(Framing::Length(0));
    }
    if let Some(coding) = response.header("Transfer-Encoding") {
        let last = coding.rsplit(',').next().unwrap_or_default().trim();
        if !last.eq_ignore_ascii_case("chunked") {
            return Err(DavError::BadResponse("a transfer coding other than chunked"));
        }
        return Ok(Framing::Chunked(Chunks::default()));
    }
    let mut length = None;
    for (name, value) in &response.headers {
        if !name.eq_ignore_ascii_case("Content-Length") {
            continue;
        }
        if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(DavError::BadResponse("a bad content length"));
        }
        let len = value.parse::<usize>().ok().filter(|&len| len <= MAX_BODY).ok_or(DavError::TooLarge { what: "response body", limit: MAX_BODY })?;
        if length.is_some_and(|seen| seen != len) {
            return Err(DavError::BadResponse("two content lengths"));
        }
        length = Some(len);
    }
    Ok(length.map_or(Framing::UntilClose, Framing::Length))
}

/// Whether `byte` may be part of a header name (RFC 9110 section 5.6.2).
fn is_token(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte)
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod tests;
