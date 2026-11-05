// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::ffi::OsString;
use std::io::{self, Write};

use inboxcraft_time::{Clock, FakeClock, Timestamp};

use super::{Source, Step, app, run, usage};

fn run_with(args: &[&str]) -> (Step, String, String) {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let step = run(args.iter().map(OsString::from), &mut out, &mut err);
    (step, String::from_utf8_lossy(&out).into_owned(), String::from_utf8_lossy(&err).into_owned())
}

#[test]
fn version_prints_the_name_and_version() {
    for flag in ["--version", "-V"] {
        let (step, out, err) = run_with(&[flag]);
        assert_eq!(step, Step::Exit(0));
        assert_eq!(out, format!("inboxcraft {}\n", env!("CARGO_PKG_VERSION")));
        assert_eq!(err, "");
    }
}

#[test]
fn help_prints_the_usage() {
    for flag in ["--help", "-h"] {
        let (step, out, err) = run_with(&[flag]);
        assert_eq!(step, Step::Exit(0));
        assert_eq!(out, usage());
        assert!(out.contains("--sample"), "{out}");
        assert_eq!(err, "");
    }
}

#[test]
fn no_arguments_open_an_empty_window_and_sample_the_sample() {
    assert_eq!(run_with(&[]).0, Step::Window(Source::Empty));
    assert_eq!(run_with(&["--sample"]).0, Step::Window(Source::Sample));
}

#[test]
fn anything_else_is_refused_with_a_hint() {
    for args in [&["--frobnicate"][..], &["--sample", "extra"][..], &["--version", "extra"][..]] {
        let (step, out, err) = run_with(args);
        assert_eq!(step, Step::Exit(2));
        assert_eq!(out, "");
        assert!(err.contains("unknown arguments") && err.contains("--help"), "{err}");
    }
    let (_, _, err) = run_with(&["--x\u{1b}[31mred"]);
    assert!(!err.contains('\u{1b}'), "control characters are not echoed: {err:?}");
}

#[test]
fn non_utf8_arguments_do_not_panic() {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        let mut out = Vec::new();
        let mut err = Vec::new();
        assert_eq!(run([OsString::from_vec(vec![0xff, 0xfe])], &mut out, &mut err), Step::Exit(2));
    }
}

struct Broken;

impl Write for Broken {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("closed"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::other("closed"))
    }
}

#[test]
fn a_closed_output_is_an_error_code_not_a_panic() {
    let mut err = Vec::new();
    assert_eq!(run([OsString::from("--version")], &mut Broken, &mut err), Step::Exit(1));
    assert_eq!(run([OsString::from("--help")], &mut Broken, &mut err), Step::Exit(1));
    let mut out = Vec::new();
    assert_eq!(run([OsString::from("--nope")], &mut out, &mut Broken), Step::Exit(2));
    Broken.flush().unwrap_err();
}

fn clock() -> Box<dyn Clock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

#[test]
fn the_sample_window_shows_the_sample_mailbox_and_the_empty_one_nothing() {
    let sample = app(Source::Sample, clock).unwrap();
    assert_eq!(sample.mailbox().accounts().len(), 2);
    assert_eq!(sample.mailbox().messages().len(), 11);
    let empty = app(Source::Empty, clock).unwrap();
    assert!(empty.mailbox().accounts().is_empty());
    assert_eq!(empty.mailbox().error(), None);
}
