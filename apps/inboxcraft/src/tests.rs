// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::ffi::OsString;
use std::io::{self, Write};

use super::run;

fn run_with(args: &[&str]) -> (u8, String, String) {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = run(args.iter().map(OsString::from), &mut out, &mut err);
    (code, String::from_utf8_lossy(&out).into_owned(), String::from_utf8_lossy(&err).into_owned())
}

#[test]
fn version_prints_the_name_and_version() {
    for flag in ["--version", "-V"] {
        let (code, out, err) = run_with(&[flag]);
        assert_eq!(code, 0);
        assert_eq!(out, format!("inboxcraft {}\n", env!("CARGO_PKG_VERSION")));
        assert_eq!(err, "");
    }
}

#[test]
fn anything_else_says_nothing_is_built_yet() {
    for args in [&[][..], &["--sample"][..], &["--version", "extra"][..]] {
        let (code, out, err) = run_with(args);
        assert_eq!(code, 1);
        assert_eq!(out, "");
        assert!(err.contains("nothing is built yet"), "{err}");
    }
}

#[test]
fn non_utf8_arguments_do_not_panic() {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = run([OsString::from_vec(vec![0xff, 0xfe])], &mut out, &mut err);
        assert_eq!(code, 1);
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
    assert_eq!(run([OsString::from("--version")], &mut Broken, &mut err), 1);
    let mut out = Vec::new();
    assert_eq!(run([], &mut out, &mut Broken), 1);
    Broken.flush().unwrap_err();
}
