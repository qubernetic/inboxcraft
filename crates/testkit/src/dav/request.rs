// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The fake's own reader of HTTP/1.1 requests (RFC 9112): the request line, the header fields and a body of
//! `Content-Length` bytes, everything capped.

use std::io;

use crate::Error;
use crate::line::{Line, LineReader};

/// The largest request body the fake reads, in bytes.
pub const MAX_REQUEST_BODY: usize = 4 * 1024 * 1024;
/// How many header fields one request may carry.
const MAX_FIELDS: usize = 100;
/// How many empty lines may come before a request line.
const MAX_EMPTY: usize = 4;

/// One request.
#[derive(Debug, Clone, Default)]
pub(crate) struct Request {
    pub(crate) method: String,
    /// The path, percent-decoded, without a query.
    pub(crate) path: String,
    pub(crate) fields: Vec<(String, String)>,
    pub(crate) body: Vec<u8>,
}

impl Request {
    /// The value of the first field `name`, in any case.
    pub(crate) fn field(&self, name: &str) -> Option<&str> {
        self.fields.iter().find(|(written, _)| written.eq_ignore_ascii_case(name)).map(|(_, value)| value.as_str())
    }
}

/// What reading a request found.
#[derive(Debug)]
pub(crate) enum Read {
    /// A whole request.
    Request(Request),
    /// The client closed the connection between requests.
    Closed,
    /// A request the fake does not read: answer with this status and close the connection.
    Refuse(u16),
}

/// Reads the next request; `heard` gets each line of its head.
pub(crate) fn read(reader: &mut LineReader, io: &mut impl io::Read, heard: &mut impl FnMut(&[u8])) -> Result<Read, Error> {
    let mut line = Vec::new();
    for _ in 0..=MAX_EMPTY {
        line = match reader.read_line(io) {
            Ok(Line::Complete(line)) => line,
            Ok(Line::TooLong(cut)) => {
                heard(&cut);
                return Ok(Read::Refuse(414));
            }
            Ok(Line::Eof) => return Ok(Read::Closed),
            Err(err) if err.kind() == io::ErrorKind::UnexpectedEof => return Ok(Read::Closed),
            Err(err) => return Err(err.into()),
        };
        if !line.is_empty() {
            break;
        }
    }
    heard(&line);
    let Some((method, path)) = request_line(&line) else { return Ok(Read::Refuse(400)) };
    let mut request = Request { method, path, ..Request::default() };
    loop {
        let field = match reader.read_line(io)? {
            Line::Complete(field) => field,
            Line::TooLong(cut) => {
                heard(&cut);
                return Ok(Read::Refuse(431));
            }
            Line::Eof => return Err(Error::ClientGone),
        };
        heard(&field);
        if field.is_empty() {
            break;
        }
        let text = String::from_utf8_lossy(&field);
        let Some((name, value)) = text.split_once(':') else { return Ok(Read::Refuse(400)) };
        if request.fields.len() >= MAX_FIELDS || name.is_empty() || name.contains(char::is_whitespace) {
            return Ok(Read::Refuse(if request.fields.len() >= MAX_FIELDS { 431 } else { 400 }));
        }
        request.fields.push((name.to_owned(), value.trim().to_owned()));
    }
    body(reader, io, request)
}

fn body(reader: &mut LineReader, io: &mut impl io::Read, mut request: Request) -> Result<Read, Error> {
    if request.field("Transfer-Encoding").is_some() {
        return Ok(Read::Refuse(501));
    }
    let len = match request.field("Content-Length") {
        None => 0,
        Some(text) if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) => return Ok(Read::Refuse(400)),
        Some(text) => match text.parse::<usize>() {
            Ok(len) if len <= MAX_REQUEST_BODY => len,
            _ => return Ok(Read::Refuse(413)),
        },
    };
    request.body = reader.read_bytes(io, len)?.ok_or(Error::ClientGone)?;
    Ok(Read::Request(request))
}

/// The method and the decoded path of `METHOD /path HTTP/1.1`.
fn request_line(line: &[u8]) -> Option<(String, String)> {
    let text = std::str::from_utf8(line).ok()?;
    let mut parts = text.split(' ');
    let (method, target, version) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() || !version.starts_with("HTTP/1.") || method.is_empty() || !method.bytes().all(|byte| byte.is_ascii_uppercase()) {
        return None;
    }
    let target = target.split_once('?').map_or(target, |(path, _)| path);
    if !target.starts_with('/') {
        return None;
    }
    Some((method.to_owned(), decode(target)?))
}

/// Percent-decodes a path; `None` for a bad escape, bytes that are not UTF-8 or a control character.
pub(crate) fn decode(path: &str) -> Option<String> {
    let bytes = path.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while let Some(&byte) = bytes.get(at) {
        if byte == b'%' {
            let hex = std::str::from_utf8(bytes.get(at.saturating_add(1)..at.saturating_add(3))?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok().filter(|_| hex.bytes().all(|byte| byte.is_ascii_hexdigit()))?);
            at = at.saturating_add(3);
        } else {
            out.push(byte);
            at = at.saturating_add(1);
        }
    }
    String::from_utf8(out).ok().filter(|path| !path.chars().any(char::is_control))
}
