// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The fake server: one listener on the loopback interface, one client, one script.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::Error;
use crate::line::{Line, LineReader};
use crate::script::{Fault, Script, Step};

/// How often the server looks for a client or for the stop signal.
const POLL: Duration = Duration::from_millis(5);

/// How a fake server listens, and its timeouts.
#[derive(Debug, Clone)]
pub struct Listen {
    accept_timeout: Duration,
    io_timeout: Duration,
}

impl Listen {
    /// Plain TCP, ten seconds to accept, five seconds per read or write.
    #[must_use]
    pub fn plain() -> Self {
        Self { accept_timeout: Duration::from_secs(10), io_timeout: Duration::from_secs(5) }
    }

    /// How long the server waits for its client before [`Error::NoClient`].
    #[must_use]
    pub fn accept_timeout(self, wait: Duration) -> Self {
        Self { accept_timeout: wait, ..self }
    }

    /// How long one read or write may block before the script fails.
    #[must_use]
    pub fn io_timeout(self, wait: Duration) -> Self {
        Self { io_timeout: wait, ..self }
    }

    /// Binds a free port on 127.0.0.1 and plays `script` to the first client on a thread of its own.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] when the port cannot be bound or the thread cannot be started.
    pub fn start(self, script: Script) -> Result<FakeServer, Error> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        listener.set_nonblocking(true)?;
        let addr = listener.local_addr()?;
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let handle = thread::Builder::new().name("testkit-fake-server".to_owned()).spawn(move || self.serve(&listener, &script, &flag))?;
        Ok(FakeServer { addr, stop, handle: Some(handle) })
    }

    fn serve(&self, listener: &TcpListener, script: &Script, stop: &AtomicBool) -> Result<Transcript, Error> {
        let stream = accept(listener, self.accept_timeout, stop)?;
        stream.set_nonblocking(false)?;
        stream.set_read_timeout(Some(self.io_timeout))?;
        stream.set_write_timeout(Some(self.io_timeout))?;
        let mut stream = stream;
        play(&mut stream, script, stop).map(|(transcript, _)| transcript)
    }
}

/// A fake server playing its script. Dropping it stops the script at the next step and waits for the thread.
#[derive(Debug)]
pub struct FakeServer {
    addr: SocketAddr,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<Result<Transcript, Error>>>,
}

impl FakeServer {
    /// The address to connect to.
    #[must_use]
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Waits until the script has been played and returns what the client sent.
    ///
    /// # Errors
    ///
    /// The first thing that went wrong on the server side: no client, a line the script did not expect, a socket or
    /// TLS error (a client that refuses the certificate shows up here as a TLS error).
    pub fn finish(mut self) -> Result<Transcript, Error> {
        let handle = self.handle.take().ok_or(Error::Thread)?;
        handle.join().map_err(|_| Error::Thread)?
    }
}

impl Drop for FakeServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

/// The lines a client sent, in order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Transcript {
    received: Vec<String>,
}

impl Transcript {
    /// The lines the client sent, without their terminators; bytes that are not UTF-8 are replaced.
    #[must_use]
    pub fn received(&self) -> &[String] {
        &self.received
    }
}

/// How a script ended.
#[derive(Debug, PartialEq, Eq)]
enum Ending {
    /// Every step was played.
    Finished,
    /// A [`Fault::Drop`] or the stop signal cut it short.
    Dropped,
}

/// How many [`POLL`] rounds fit in `wait`, at least one.
fn rounds(wait: Duration) -> u128 {
    (wait.as_millis() / POLL.as_millis()).max(1)
}

fn accept(listener: &TcpListener, wait: Duration, stop: &AtomicBool) -> Result<TcpStream, Error> {
    let mut left = rounds(wait);
    loop {
        match listener.accept() {
            Ok((stream, _)) => return Ok(stream),
            Err(err) if err.kind() != std::io::ErrorKind::WouldBlock => return Err(err.into()),
            Err(_) if stop.load(Ordering::Relaxed) || left == 0 => return Err(Error::NoClient),
            Err(_) => thread::sleep(POLL),
        }
        left = left.saturating_sub(1);
    }
}

/// Sleeps for `wait`, in [`POLL`] rounds, or until the stop signal; returns whether the signal came.
fn pause(wait: Duration, stop: &AtomicBool) -> bool {
    for _ in 0..rounds(wait) {
        if stop.load(Ordering::Relaxed) {
            return true;
        }
        thread::sleep(POLL);
    }
    stop.load(Ordering::Relaxed)
}

fn play(stream: &mut (impl Read + Write), script: &Script, stop: &AtomicBool) -> Result<(Transcript, Ending), Error> {
    let mut reader = LineReader::default();
    let mut transcript = Transcript::default();
    for step in script.steps() {
        match step {
            Step::Send(line) => {
                let mut bytes = line.clone().into_bytes();
                bytes.extend_from_slice(b"\r\n");
                stream.write_all(&bytes)?;
                stream.flush()?;
            }
            Step::Expect(expected) => {
                let got = match reader.read_line(stream)? {
                    Line::Complete(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
                    Line::Eof => return Err(Error::ClientGone),
                    Line::TooLong => return Err(Error::LineTooLong),
                };
                transcript.received.push(got.clone());
                if got != *expected {
                    return Err(Error::Unexpected { expected: expected.clone(), got });
                }
            }
            Step::Fault(Fault::Drop) => return Ok((transcript, Ending::Dropped)),
            Step::Fault(Fault::Delay(wait)) => {
                if pause(*wait, stop) {
                    return Ok((transcript, Ending::Dropped));
                }
            }
            Step::Fault(Fault::Garbage(bytes)) => {
                stream.write_all(bytes)?;
                stream.flush()?;
            }
        }
    }
    Ok((transcript, Ending::Finished))
}
