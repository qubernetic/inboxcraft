// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::ffi::OsString;
use std::io::{self, Write};
use std::path::PathBuf;

use proptest::prelude::*;
use serde_json::{Value, json};

use super::{MAX_ARGS, Request, parse, run};

fn run_with(args: &[&str]) -> (u8, String, String) {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = run(args.iter().map(OsString::from), &mut out, &mut err);
    (code, String::from_utf8_lossy(&out).into_owned(), String::from_utf8_lossy(&err).into_owned())
}

fn parsed(args: &[&str]) -> Result<Request, String> {
    let args: Vec<OsString> = args.iter().map(OsString::from).collect();
    parse(&args).map_err(|failure| failure.message)
}

#[test]
fn version_prints_the_name_and_version() {
    for flag in ["--version", "-V"] {
        let (code, out, err) = run_with(&[flag]);
        assert_eq!(code, 0);
        assert_eq!(out, format!("inboxcraft-cli {}\n", env!("CARGO_PKG_VERSION")));
        assert_eq!(err, "");
    }
}

#[test]
fn help_says_how_to_run_a_command() {
    for flag in ["--help", "-h"] {
        let (code, out, err) = run_with(&[flag]);
        assert_eq!((code, err.as_str()), (0, ""));
        assert!(out.contains("--store <dir> [--json] <command> [<params>]"), "{out}");
    }
}

#[test]
fn the_store_and_the_parameters_are_read_in_any_order() {
    let run = |store: &str, params: Value| Ok(Request::Run { store: PathBuf::from(store), id: "mail.list".to_owned(), params });
    assert_eq!(parsed(&["--store", "/tmp/a", "mail.list"]), run("/tmp/a", json!({})));
    assert_eq!(parsed(&["mail.list", "--json", "--store=/tmp/b", r#"{"limit":2}"#]), run("/tmp/b", json!({ "limit": 2 })));
    assert_eq!(parsed(&["--store", "/tmp/c", "--", "mail.list", "{}"]), run("/tmp/c", json!({})));
    assert_eq!(parsed(&["--json", "commands"]), Ok(Request::Commands));
}

#[test]
fn wrong_arguments_are_a_usage_error() {
    for (args, says) in [
        (&[][..], "give a command id"),
        (&["mail.list"][..], "give --store <dir>"),
        (&["--store"][..], "--store needs a directory"),
        (&["--store="][..], "once"),
        (&["--store", "a", "--store", "b", "mail.list"][..], "once"),
        (&["--sample"][..], "there is no option \"--sample\""),
        (&["--storex", "a"][..], "there is no option"),
        (&["--version", "extra"][..], "stand alone"),
        (&["--help", "--store", "a"][..], "stand alone"),
        (&["--store", "a", "mail.list", "[1]"][..], "one JSON object"),
        (&["--store", "a", "mail.list", "{", "{}"][..], "at most one JSON object"),
        (&["--store", "a", "--", "mail.list", "-v"][..], "not JSON"),
    ] {
        let (code, out, err) = run_with(args);
        assert_eq!((code, out.as_str()), (2, ""), "{args:?}");
        assert!(err.starts_with("inboxcraft-cli: ") && err.contains(says), "{args:?}: {err}");
    }
}

#[test]
fn too_many_arguments_are_refused_before_any_is_read() {
    let many = vec!["x"; MAX_ARGS.saturating_add(5)];
    let (code, _, err) = run_with(&many);
    assert_eq!(code, 2);
    assert!(err.contains("more than"), "{err}");
}

#[test]
fn an_echoed_argument_is_cut_and_has_no_controls() {
    let long = format!("--\u{1b}[2J{}", "x".repeat(10_000));
    let (_, _, err) = run_with(&[&long]);
    assert!(!err.contains('\u{1b}'));
    assert!(err.len() < 400, "{}", err.len());
}

#[test]
fn the_command_list_shows_every_command_with_its_class() {
    let (code, out, err) = run_with(&["commands"]);
    assert_eq!((code, err.as_str()), (0, ""));
    assert!(out.lines().any(|line| line.starts_with("mail.list ") && line.contains(" safe ")), "{out}");
    assert!(out.lines().any(|line| line.starts_with("account.remove ") && line.contains(" owner ")), "{out}");
}

#[test]
fn non_utf8_arguments_do_not_panic() {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = run([OsString::from_vec(vec![0xff, 0xfe])], &mut out, &mut err);
        assert_eq!(code, 2);
        let code = run([OsString::from("--store"), OsString::from_vec(vec![0xff])], &mut out, &mut err);
        assert_eq!(code, 2);
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
    assert!(String::from_utf8_lossy(&err).contains("could not be written"));
    let mut out = Vec::new();
    assert_eq!(run([], &mut out, &mut Broken), 2);
    Broken.flush().unwrap_err();
}

proptest! {
    #[test]
    fn junk_arguments_never_panic(args in proptest::collection::vec(".{0,40}", 0..8)) {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = run(args.iter().map(OsString::from), &mut out, &mut err);
        prop_assert!(code <= 2);
        prop_assert!(code == 0 || out.is_empty());
    }
}
