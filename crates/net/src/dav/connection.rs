// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The HTTPS side of a calendar job: one kept-alive connection per origin, over the worker's TLS stack, that writes
//! what `dav` encodes and reads what comes back with `dav`'s response reader.

use std::fmt;
use std::io::{Read, Write};
use std::num::NonZeroU16;

use inboxcraft_dav::{Credentials, Origin, Request, Response, ResponseReader};
use inboxcraft_model::{ErrorKind, Host, Security, Server};

use super::failure;
use crate::NetError;
use crate::connect::{Connector, Stream};

/// The size of one read from the socket.
const READ_CHUNK: usize = 16 * 1024;

/// The requests of one calendar job: each goes to its origin, on the connection held for it while the server keeps
/// it open. Every request carries the user name and password (Basic auth), so the connection is `https` or none.
pub(crate) struct Session<'a> {
    connector: &'a Connector,
    login: Credentials,
    open: Option<Open>,
}

impl fmt::Debug for Session<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Session").field("user", &self.login.user()).field("open", &self.open.as_ref().map(|open| &open.origin)).finish()
    }
}

/// A connection and the origin it is to.
struct Open {
    origin: Origin,
    stream: Stream,
    reader: ResponseReader,
}

impl<'a> Session<'a> {
    /// Requests through `connector`, signed in as `login`.
    pub(crate) fn new(connector: &'a Connector, login: Credentials) -> Self {
        Self { connector, login, open: None }
    }

    /// Sends `request` and reads its response; a response that closes the connection drops it, so the next request
    /// opens another.
    ///
    /// # Errors
    ///
    /// [`ErrorKind::NotEncrypted`] for an origin that is not `https`, before anything is sent; the connection's
    /// errors (the host, TCP, TLS and its certificate faults, a timeout, a dropped connection); and
    /// [`ErrorKind::Protocol`] or [`ErrorKind::TooLarge`] for an answer `dav` cannot read.
    pub(crate) fn exchange(&mut self, request: &Request) -> Result<Response, NetError> {
        let origin = request.origin();
        if !origin.is_encrypted() {
            return Err(NetError::new(ErrorKind::NotEncrypted, "no password goes over http"));
        }
        let mut bytes = request.encode(Some(&self.login)).map_err(|error| failure(&error))?;
        let open = match self.open.take() {
            Some(open) if open.origin == *origin => open,
            _ => self.connect(origin)?,
        };
        let sent = self.send(open, &bytes);
        bytes.fill(0);
        let (open, response) = sent?;
        self.open = open.filter(|_| !response.closes());
        Ok(response)
    }

    /// Ends TLS with a `close_notify` on the connection held.
    pub(crate) fn close(mut self) {
        if let Some(mut open) = self.open.take() {
            open.stream.close();
        }
    }

    fn connect(&self, origin: &Origin) -> Result<Open, NetError> {
        let host = Host::new(origin.host()).map_err(|error| NetError::new(ErrorKind::HostNotFound, error.to_string()))?;
        let port = NonZeroU16::new(origin.port()).ok_or_else(|| NetError::new(ErrorKind::ConnectionRefused, "port 0"))?;
        let server = Server::new(host, port, Security::Tls);
        let config = self.connector.tls().client_config()?;
        let stream = self.connector.tcp(&server)?;
        let stream = self.connector.encrypt(stream, server.host(), &config)?;
        Ok(Open { origin: origin.clone(), stream, reader: ResponseReader::new() })
    }

    /// Writes `bytes` on `open` and reads the response; the connection back unless the server closed it.
    fn send(&self, mut open: Open, bytes: &[u8]) -> Result<(Option<Open>, Response), NetError> {
        let now = || self.connector.tls().clock().now();
        open.stream.write_all(bytes).and_then(|()| open.stream.flush()).map_err(|error| NetError::io(&error, now()))?;
        let mut buffer = vec![0; READ_CHUNK].into_boxed_slice();
        loop {
            let read = open.stream.read(&mut buffer).map_err(|error| NetError::io(&error, now()))?;
            if read == 0 {
                return closed(&mut open.reader).map(|response| (None, response));
            }
            let bytes = buffer.get(..read).unwrap_or_default();
            if let Some(response) = open.reader.receive(bytes).map_err(|error| failure(&error))? {
                return Ok((Some(open), response));
            }
        }
    }
}

/// The response the server ended by closing the connection.
fn closed(reader: &mut ResponseReader) -> Result<Response, NetError> {
    match reader.end() {
        Ok(Some(response)) => Ok(response),
        Ok(None) | Err(_) => Err(NetError::new(ErrorKind::ConnectionDropped, "the server closed the connection")),
    }
}
