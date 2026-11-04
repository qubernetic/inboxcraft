// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The server's end of a connection: plain TCP, or TLS over it from the start or after STARTTLS, and either
//! compressed after COMPRESS=DEFLATE.

use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::sync::Arc;

use rustls::{ServerConfig, ServerConnection, StreamOwned};

use crate::Error;
use crate::deflate::Deflate;

#[derive(Debug)]
enum Tcp {
    Plain(TcpStream),
    Tls(Box<StreamOwned<ServerConnection, TcpStream>>),
}

/// One accepted client connection.
#[derive(Debug)]
pub(crate) struct Wire {
    tcp: Tcp,
    deflate: Option<Box<Deflate>>,
}

impl Wire {
    /// A plain connection.
    pub(crate) fn plain(stream: TcpStream) -> Self {
        Self { tcp: Tcp::Plain(stream), deflate: None }
    }

    /// Whether the connection is encrypted.
    pub(crate) fn is_tls(&self) -> bool {
        matches!(self.tcp, Tcp::Tls(_))
    }

    /// Turns a plain connection into the server side of a TLS one; the handshake runs with the next read or write. A
    /// connection that already is TLS stays as it is.
    pub(crate) fn accept_tls(&mut self, config: &Arc<ServerConfig>) -> Result<(), Error> {
        let Tcp::Plain(stream) = &self.tcp else { return Ok(()) };
        let stream = stream.try_clone()?;
        self.tcp = Tcp::Tls(Box::new(StreamOwned::new(ServerConnection::new(Arc::clone(config))?, stream)));
        Ok(())
    }

    /// Compresses both directions from now on.
    pub(crate) fn compress(&mut self) {
        self.deflate = Some(Box::new(Deflate::new()));
    }

    /// Ends the session in order: a TLS connection sends its close notification.
    pub(crate) fn close(&mut self) -> io::Result<()> {
        if let Tcp::Tls(stream) = &mut self.tcp {
            stream.conn.send_close_notify();
            stream.flush()?;
        }
        Ok(())
    }
}

impl Read for Tcp {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Plain(stream) => stream.read(buf),
            Self::Tls(stream) => stream.read(buf),
        }
    }
}

impl Write for Tcp {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Self::Plain(stream) => stream.write(buf),
            Self::Tls(stream) => stream.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Plain(stream) => stream.flush(),
            Self::Tls(stream) => stream.flush(),
        }
    }
}

impl Read for Wire {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match &mut self.deflate {
            Some(deflate) => deflate.read(&mut self.tcp, buf),
            None => self.tcp.read(buf),
        }
    }
}

impl Write for Wire {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match &mut self.deflate {
            Some(deflate) => deflate.write(&mut self.tcp, buf).map(|()| buf.len()),
            None => self.tcp.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.tcp.flush()
    }
}
