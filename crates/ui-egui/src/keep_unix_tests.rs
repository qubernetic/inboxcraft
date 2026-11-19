// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Kept files that are not regular files: a start never waits on one (#298).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use super::{FILE, KEYS_FILE, Keep, KeepFile, MAX_FILE, NOTIFY_FILE, TRIAGE_FILE, WEEK_FILE, open_kept, read_kept, read_opened};

/// Longer than any read of a small file takes; a read that waits on a pipe never ends.
const LONG: Duration = Duration::from_secs(5);

/// What `work` gives, or `None` when it has not finished within [`LONG`]: a read stuck on a pipe fails the test
/// instead of hanging it.
fn within<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(work());
    });
    receiver.recv_timeout(LONG).ok()
}

fn mkfifo(path: &Path) {
    let made = std::process::Command::new("mkfifo").arg(path).status();
    assert!(made.is_ok_and(|status| status.success()), "mkfifo {}", path.display());
}

/// A kept file kind: its name, and whether loading it through a [`KeepFile`] gave something.
type Kind = (&'static str, fn(&KeepFile) -> bool);

/// A way a kept file can be there: its name, and what puts it at a path.
type Shape<'a> = (&'static str, Box<dyn Fn(&Path) + 'a>);

/// Every kept file kind.
fn kinds() -> [Kind; 5] {
    [
        (FILE, |keep| keep.load().is_some()),
        (KEYS_FILE, |keep| keep.load_keys().is_some()),
        (TRIAGE_FILE, |keep| keep.load_triage().is_some()),
        (NOTIFY_FILE, |keep| keep.load_notify().is_some()),
        (WEEK_FILE, |keep| keep.load_week().is_some()),
    ]
}

/// Whether loading the kind `index` of [`kinds`] from a folder whose kept files are at `dir` gave something, read on a
/// thread of its own; `None` when it did not finish.
fn loads(dir: &Path, index: usize) -> Option<bool> {
    let keep = KeepFile::new(dir.join(FILE));
    within(move || kinds().get(index).is_some_and(|(_, load)| load(&keep)))
}

#[test]
fn a_kept_file_that_is_a_pipe_a_link_to_one_a_folder_or_too_large_starts_with_the_default() {
    let fifos = tempfile::tempdir().unwrap();
    let fifo = fifos.path().join("pipe");
    mkfifo(&fifo);
    // A good choice, padded with spaces to one byte past the cap: read whole, it would be read as the choice.
    let mut huge = b"{}".to_vec();
    huge.resize(MAX_FILE + 1, b' ');
    let shapes: [Shape<'_>; 5] = [
        ("a pipe", Box::new(|path: &Path| mkfifo(path))),
        ("a link to a pipe", Box::new(|path: &Path| std::os::unix::fs::symlink(&fifo, path).unwrap())),
        ("a folder", Box::new(|path: &Path| std::fs::create_dir(path).unwrap())),
        ("a file larger than the cap", Box::new(|path: &Path| std::fs::write(path, &huge).unwrap())),
        ("a link to a missing file", Box::new(|path: &Path| std::os::unix::fs::symlink(fifos.path().join("gone"), path).unwrap())),
    ];
    for (index, (name, _)) in kinds().iter().enumerate() {
        for (shape, make) in &shapes {
            let dir = tempfile::tempdir().unwrap();
            make(&dir.path().join(name));
            assert_eq!(loads(dir.path(), index), Some(false), "{name} as {shape}: the default, at once");
        }
    }
}

#[test]
fn a_good_kept_file_and_a_link_to_one_are_read() {
    for (index, (name, _)) in kinds().iter().enumerate() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(name), b"{}").unwrap();
        assert_eq!(loads(dir.path(), index), Some(true), "{name}");
        let linked = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(dir.path().join(name), linked.path().join(name)).unwrap();
        assert_eq!(loads(linked.path(), index), Some(true), "{name} as a link to a good file");
    }
    // Just at the cap is still read.
    let dir = tempfile::tempdir().unwrap();
    let mut full = b"{}".to_vec();
    full.resize(MAX_FILE, b' ');
    std::fs::write(dir.path().join(FILE), &full).unwrap();
    assert_eq!(read_kept(&dir.path().join(FILE), MAX_FILE).map(|bytes| bytes.len()), Some(MAX_FILE));
}

#[test]
fn a_pipe_is_not_even_opened() {
    let dir = tempfile::tempdir().unwrap();
    let fifo = dir.path().join("pipe");
    mkfifo(&fifo);
    // A writer waits in its open until a reader opens the pipe: if the read opened it, the writer gets through.
    let (opened, writer_in) = mpsc::channel();
    let writer_path: PathBuf = fifo.clone();
    std::thread::spawn(move || {
        let writer = std::fs::OpenOptions::new().write(true).open(&writer_path);
        let _ = opened.send(());
        drop(writer);
    });
    std::thread::sleep(Duration::from_millis(100));
    let read_path = fifo.clone();
    assert_eq!(within(move || read_kept(&read_path, MAX_FILE)), Some(None));
    assert!(writer_in.recv_timeout(Duration::from_millis(300)).is_err(), "the pipe was opened");
    // Let the waiting writer go.
    let _release = std::fs::OpenOptions::new().read(true).write(true).open(&fifo).unwrap();
    assert!(writer_in.recv_timeout(LONG).is_ok());
}

#[test]
fn opening_a_pipe_does_not_wait_for_a_writer() {
    let dir = tempfile::tempdir().unwrap();
    let fifo = dir.path().join("pipe");
    mkfifo(&fifo);
    let path = fifo.clone();
    assert_eq!(within(move || open_kept(&path).is_ok()), Some(true));
    // A writer stuck in its open (only when the mutation made the read wait) is let go.
    let _release = std::fs::OpenOptions::new().read(true).write(true).open(&fifo).unwrap();
}

#[test]
fn what_was_opened_is_read_only_when_it_is_a_regular_file() {
    let dir = tempfile::tempdir().unwrap();
    let fifo = dir.path().join("pipe");
    mkfifo(&fifo);
    // Open for both reading and writing: the pipe then has a writer that never closes, so a read would never end.
    let mut both = std::fs::OpenOptions::new().read(true).write(true).open(&fifo).unwrap();
    both.write_all(b"{}").unwrap();
    assert_eq!(within(move || read_opened(both, MAX_FILE)), Some(None), "a pipe swapped in after the look is not read");
    let file = dir.path().join("file");
    std::fs::write(&file, b"{}").unwrap();
    assert_eq!(read_opened(std::fs::File::open(&file).unwrap(), MAX_FILE), Some(b"{}".to_vec()));
}
