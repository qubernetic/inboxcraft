// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::mpsc;
use std::time::Duration;

use super::{Claim, LOCK_FILE, Request, claim};

fn first(dir: &std::path::Path) -> super::Instance {
    match claim(dir).unwrap() {
        Claim::First(instance) => instance,
        Claim::Running(_) => panic!("no other start holds {}", dir.display()),
    }
}

#[test]
fn requests_are_one_short_line_each() {
    for request in [Request::Show, Request::Quit] {
        assert_eq!(Request::parse(request.line().as_bytes()), Some(request));
    }
    assert_eq!(Request::parse(b"show"), Some(Request::Show), "the line end is optional");
    assert_eq!(Request::parse(b"quit\r\n"), Some(Request::Quit));
    for junk in [&b""[..], b"\n", b"SHOW\n", b"show me\n", b"\xff\xfe\n", b"show\nquit\n"] {
        assert_eq!(Request::parse(junk), None, "{junk:?}");
    }
}

#[test]
fn the_first_start_holds_the_folder_and_a_second_finds_it_running() {
    let dir = tempfile::tempdir().unwrap();
    let held = first(dir.path());
    assert!(dir.path().join(LOCK_FILE).is_file());
    assert!(matches!(claim(dir.path()).unwrap(), Claim::Running(_)));
    drop(held);
    assert!(matches!(claim(dir.path()).unwrap(), Claim::First(_)), "the folder is free once the first start ends");
}

#[test]
fn a_folder_that_cannot_be_made_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a file");
    std::fs::write(&file, b"").unwrap();
    let error = claim(&file.join("instance")).err().unwrap();
    assert!(error.to_string().contains("a file"), "{error}");
}

#[cfg(unix)]
#[test]
fn a_second_start_brings_the_first_one_forward_or_ends_it() {
    let dir = tempfile::tempdir().unwrap();
    let (sent, heard) = mpsc::channel();
    let listening = first(dir.path()).listen(move |request| {
        let _ = sent.send(request);
    });
    assert!(listening.error().is_none(), "{:?}", listening.error());
    for request in [Request::Show, Request::Quit] {
        let Claim::Running(running) = claim(dir.path()).unwrap() else { panic!("the first start is running") };
        running.send(request).unwrap();
        assert_eq!(heard.recv_timeout(Duration::from_secs(5)).unwrap(), request);
    }
}

#[cfg(unix)]
#[test]
fn a_socket_a_crashed_start_left_behind_is_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let left = std::os::unix::net::UnixListener::bind(dir.path().join(super::SOCKET_FILE)).unwrap();
    drop(left);
    let (sent, heard) = mpsc::channel();
    let _listening = first(dir.path()).listen(move |request| {
        let _ = sent.send(request);
    });
    let Claim::Running(running) = claim(dir.path()).unwrap() else { panic!("the first start is running") };
    running.send(Request::Show).unwrap();
    assert_eq!(heard.recv_timeout(Duration::from_secs(5)).unwrap(), Request::Show);
}

#[cfg(unix)]
#[test]
fn junk_on_the_socket_is_refused_and_the_next_request_still_heard() {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;
    let dir = tempfile::tempdir().unwrap();
    let (sent, heard) = mpsc::channel();
    let _listening = first(dir.path()).listen(move |request| {
        let _ = sent.send(request);
    });
    let socket = dir.path().join(super::SOCKET_FILE);
    for junk in [&b"open the pod bay doors\n"[..], &[b'x'; 4096][..], b"\xff\n", b""] {
        let mut stream = UnixStream::connect(&socket).unwrap();
        stream.write_all(junk).unwrap();
        stream.shutdown(std::net::Shutdown::Write).unwrap();
        let mut answer = String::new();
        let _ = stream.read_to_string(&mut answer);
        assert_eq!(answer, "", "{junk:?}");
    }
    // One that says nothing and keeps the line open does not hold the others up for long.
    let _silent = UnixStream::connect(&socket).unwrap();
    let Claim::Running(running) = claim(dir.path()).unwrap() else { panic!("the first start is running") };
    running.send(Request::Show).unwrap();
    assert_eq!(heard.recv_timeout(Duration::from_secs(5)).unwrap(), Request::Show);
    assert!(heard.try_recv().is_err(), "nothing else was heard");
}

#[cfg(unix)]
#[test]
fn a_running_start_that_does_not_answer_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let _held = first(dir.path());
    // Holds the folder but never listens: a start that hangs before its window, or one on another system.
    let Claim::Running(running) = claim(dir.path()).unwrap() else { panic!("the first start holds the folder") };
    let error = running.with_patience(Duration::from_millis(300)).send(Request::Show).unwrap_err();
    assert!(error.to_string().contains("did not answer"), "{error}");
}

#[cfg(unix)]
#[test]
fn a_folder_too_deep_for_a_socket_still_holds_the_lock() {
    let dir = tempfile::tempdir().unwrap();
    let deep = dir.path().join("d".repeat(120));
    let listening = first(&deep).listen(|_| {});
    assert!(listening.error().is_some(), "a socket path longer than the system allows");
    assert!(matches!(claim(&deep).unwrap(), Claim::Running(_)));
}

#[cfg(unix)]
#[test]
fn the_socket_goes_with_the_instance() {
    let dir = tempfile::tempdir().unwrap();
    let listening = first(dir.path()).listen(|_| {});
    assert!(dir.path().join(super::SOCKET_FILE).exists());
    drop(listening);
    assert!(!dir.path().join(super::SOCKET_FILE).exists());
}

#[cfg(unix)]
#[test]
fn the_socket_is_the_owner_s_alone() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let _listening = first(dir.path()).listen(|_| {});
    let mode = std::fs::metadata(dir.path().join(super::SOCKET_FILE)).unwrap().permissions().mode();
    assert_eq!(mode & 0o077, 0, "{mode:o}");
}
