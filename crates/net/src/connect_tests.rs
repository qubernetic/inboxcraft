// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::io::{IoSlice, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::time::Duration;

use inboxcraft_time::{Clock, SystemClock};

use super::*;

/// A peer that accepts and never reads: a write that fills its buffers ends at the deadline, not at the socket's own
/// 10-second write timeout.
#[test]
fn a_write_to_a_peer_that_stops_reading_ends_at_the_deadline() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let socket = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let (_peer, _) = listener.accept().unwrap();
    socket.set_write_timeout(Some(Duration::from_secs(10))).unwrap();
    let deadline = Deadline::new(Arc::new(SystemClock::new()), Duration::from_millis(500));
    let mut timed = Timed::new(socket, Some(deadline), Duration::from_secs(30));
    let clock = SystemClock::new();
    let start = clock.monotonic();
    let chunk = vec![0_u8; 1 << 20];
    let error = loop {
        if let Err(error) = timed.write_all(&chunk) {
            break error;
        }
        assert!(clock.monotonic().saturating_sub(start) < Duration::from_secs(5), "the peer kept reading");
    };
    assert!(matches!(error.kind(), io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock), "{error}");
    assert!(clock.monotonic().saturating_sub(start) < Duration::from_secs(5), "{:?}", clock.monotonic().saturating_sub(start));
}

/// A read from a peer that sends nothing ends at the deadline; once it has passed, a read fails at once.
#[test]
fn a_read_ends_at_the_deadline_and_none_starts_after_it() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let socket = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let (_peer, _) = listener.accept().unwrap();
    socket.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    let deadline = Deadline::new(Arc::new(SystemClock::new()), Duration::from_millis(300));
    let mut timed = Timed::new(socket, Some(deadline), Duration::from_secs(30));
    let clock = SystemClock::new();
    let start = clock.monotonic();
    let mut buffer = [0_u8; 8];
    assert!(timed.read(&mut buffer).is_err());
    assert!(clock.monotonic().saturating_sub(start) < Duration::from_secs(5), "{:?}", clock.monotonic().saturating_sub(start));
    std::thread::sleep(Duration::from_millis(350));
    let error = timed.read(&mut buffer).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
}

/// Without a deadline the socket's own timeouts hold, untouched (sync, IDLE, send and the account check).
#[test]
fn without_a_deadline_the_sockets_timeouts_are_left_alone() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let socket = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let (_peer, _) = listener.accept().unwrap();
    socket.set_read_timeout(Some(Duration::from_millis(1234))).unwrap();
    socket.set_write_timeout(Some(Duration::from_millis(2345))).unwrap();
    let mut timed = Timed::new(socket, None, Duration::from_secs(30));
    timed.write_all(b"x").unwrap();
    assert_eq!(timed.socket.read_timeout().unwrap(), Some(Duration::from_millis(1234)));
    assert_eq!(timed.socket.write_timeout().unwrap(), Some(Duration::from_millis(2345)));
}

/// A resolver that hangs until it is let go; it counts its calls.
#[derive(Debug, Default)]
struct Held {
    calls: AtomicUsize,
    gone: std::sync::Mutex<bool>,
    let_go: std::sync::Condvar,
}

impl Resolve for Held {
    fn resolve(&self, _host: &Host, port: u16) -> io::Result<Vec<SocketAddr>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let mut gone = self.gone.lock().unwrap();
        while !*gone {
            gone = self.let_go.wait_timeout(gone, Duration::from_secs(30)).unwrap().0;
        }
        Ok(vec![SocketAddr::from(([127, 0, 0, 1], port))])
    }
}

/// Lookups left hanging are capped across runs: with [`MAX_LOOKUPS`] still running, the next fails at once as timed
/// out without asking the resolver; once they end, a lookup runs again.
#[test]
fn hanging_lookups_are_capped_across_runs() {
    let clock: Arc<dyn Clock> = Arc::new(SystemClock::new());
    let held = Arc::new(Held::default());
    let host = Host::new("mail.example.org").unwrap();
    for _ in 0..MAX_LOOKUPS {
        let deadline = Deadline::new(Arc::clone(&clock), Duration::from_millis(20));
        let error = lookup_by(Arc::clone(&held) as Arc<dyn Resolve>, &host, 993, &deadline).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::TimedOut);
    }
    let start = clock.monotonic();
    let deadline = Deadline::new(Arc::clone(&clock), Duration::from_secs(5));
    let error = lookup_by(Arc::clone(&held) as Arc<dyn Resolve>, &host, 993, &deadline).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::TimedOut);
    assert!(clock.monotonic().saturating_sub(start) < Duration::from_secs(2), "it waited");
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(held.calls.load(Ordering::SeqCst), MAX_LOOKUPS);

    *held.gone.lock().unwrap() = true;
    held.let_go.notify_all();
    for _ in 0..500 {
        if LOOKUPS.load(Ordering::SeqCst) == 0 {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let deadline = Deadline::new(clock, Duration::from_secs(5));
    let found = lookup_by(Arc::clone(&held) as Arc<dyn Resolve>, &host, 993, &deadline).unwrap();
    assert_eq!(found, [SocketAddr::from(([127, 0, 0, 1], 993))]);
}

/// A vectored write reaches the socket as one call, as without the wrapper: both pieces are written at once.
#[test]
fn a_vectored_write_goes_through_as_one_call() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let socket = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let (mut peer, _) = listener.accept().unwrap();
    let mut timed = Timed::new(socket, None, Duration::from_secs(30));
    let written = timed.write_vectored(&[IoSlice::new(b"abc"), IoSlice::new(b"defg")]).unwrap();
    assert_eq!(written, 7);
    let mut got = [0_u8; 7];
    peer.read_exact(&mut got).unwrap();
    assert_eq!(&got, b"abcdefg");
}
