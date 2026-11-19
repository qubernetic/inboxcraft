// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A kept background choice that is not a regular file: the start never waits on it (#298).

use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use super::{FILE, kept};

fn mkfifo(path: &Path) {
    let made = std::process::Command::new("mkfifo").arg(path).status();
    assert!(made.is_ok_and(|status| status.success()), "mkfifo {}", path.display());
}

/// Whether the choice kept in `file` leaves InboxCraft running, read on a thread of its own; `None` when the read did
/// not finish within five seconds.
fn kept_within(file: &Path) -> Option<bool> {
    let (sender, receiver) = mpsc::channel();
    let file = file.to_path_buf();
    std::thread::spawn(move || {
        let _ = sender.send(kept(&file));
    });
    receiver.recv_timeout(Duration::from_secs(5)).ok()
}

#[test]
fn a_choice_file_that_is_a_pipe_or_a_link_to_one_is_read_as_on_at_once() {
    let dir = tempfile::tempdir().unwrap();
    let fifo = dir.path().join("pipe");
    mkfifo(&fifo);
    let file = dir.path().join(FILE);
    std::os::unix::fs::symlink(&fifo, &file).unwrap();
    assert_eq!(kept_within(&file), Some(true), "a link to a pipe");
    assert_eq!(kept_within(&fifo), Some(true), "a pipe");
    // A link to a good choice is read.
    let good = dir.path().join("good.json");
    std::fs::write(&good, b"{ \"background\": false }").unwrap();
    std::fs::remove_file(&file).unwrap();
    std::os::unix::fs::symlink(&good, &file).unwrap();
    assert_eq!(kept_within(&file), Some(false));
}
