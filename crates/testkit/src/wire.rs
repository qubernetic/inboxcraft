// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The server's end of a connection: plain TCP, or TLS over it from the start or after STARTTLS.

use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::sync::Arc;

use rustls::{ServerConfig, ServerConnection, StreamOwned};

use crate::Error;

#[derive(Debug)]
enum Tcp {
    Plain(TcpStream),
    Tls(Box<StreamOwned<ServerConnection, TcpStream>>),
}

/// One accepted client connection.
#[derive(Debug)]
pub(crate) struct Wire {
    tcp: Tcp,
}

impl Wire {
    /// A plain connection.
    pub(crate) fn plain(stream: TcpStream) -> Self {
        Self { tcp: Tcp::Plain(stream) }
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

    /// Ends the session in order: a TLS connection sends its close notification.
    pub(crate) fn close(&mut self) -> io::Result<()> {
        if let Tcp::Tls(stream) = &mut self.tcp {
            stream.conn.send_close_notify();
            stream.flush()?;
        }
        Ok(())
    }
}

impl Read for Wire {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match &mut self.tcp {
            Tcp::Plain(stream) => stream.read(buf),
            Tcp::Tls(stream) => stream.read(buf),
        }
    }
}

impl Write for Wire {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match &mut self.tcp {
            Tcp::Plain(stream) => stream.write(buf),
            Tcp::Tls(stream) => stream.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match &mut self.tcp {
            Tcp::Plain(stream) => stream.flush(),
            Tcp::Tls(stream) => stream.flush(),
        }
    }
}
