// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::io::Write;
use std::net::{TcpListener, TcpStream};
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
