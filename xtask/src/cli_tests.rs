// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::ffi::OsString;
use std::path::PathBuf;

use super::{Command, HELP, Invocation, default_root, parse};
use crate::ci::GATES;
use crate::test_support::run_xtask;
use crate::{FAILED, OK, USAGE};

fn args(list: &[&str]) -> Vec<OsString> {
    list.iter().map(OsString::from).collect()
}

#[test]
fn help_lists_every_command() {
    for flag in [&[][..], &["help"][..], &["--help"][..], &["-h"][..]] {
        let (code, out, err) = run_xtask(flag);
        assert_eq!(code, OK, "{flag:?}");
        assert_eq!(err, "");
        assert!(out.contains("usage: cargo xtask <command>"), "{out}");
    }
}

#[test]
fn the_ci_help_line_names_every_gate_in_order() {
    let line = HELP.lines().find(|line| line.trim_start().starts_with("ci ")).unwrap_or_default();
    let gates = line.split_once("every gate:").map(|(_, gates)| gates).unwrap_or_default();
    let named: Vec<&str> = gates.split(',').filter_map(|gate| gate.split_whitespace().next()).collect();
    let expected: Vec<&str> = GATES.iter().map(|(name, _)| *name).collect();
    assert_eq!(named, expected, "{line}");
}

#[test]
fn an_unknown_command_is_a_usage_error() {
    let (code, out, err) = run_xtask(&["frobnicate"]);
    assert_eq!(code, USAGE);
    assert_eq!(out, "");
    assert!(err.contains("unknown command `frobnicate`"), "{err}");
    assert!(err.contains("cargo xtask help"), "{err}");
}

#[test]
fn an_unknown_option_is_a_usage_error() {
    assert_eq!(parse(args(&["help", "--frob"])), Err("unknown option `--frob`".to_owned()));
}

#[test]
fn root_takes_a_directory() {
    assert_eq!(parse(args(&["help", "--root", "/tmp/x"])), Ok(Invocation { command: Command::Help, root: Some(PathBuf::from("/tmp/x")) }));
    assert_eq!(parse(args(&["help", "--root"])), Err("`--root` needs a directory".to_owned()));
}

#[test]
fn a_non_utf8_argument_is_a_usage_error() {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        let bad = OsString::from_vec(vec![b'c', 0xff]);
        assert_eq!(parse(vec![bad.clone()]), Err("arguments must be UTF-8".to_owned()));
        assert_eq!(parse(vec![OsString::from("help"), bad]), Err("arguments must be UTF-8".to_owned()));
    }
}

#[test]
fn a_usage_error_still_reaches_a_broken_stderr() {
    let mut out = Vec::new();
    let mut ctx = crate::test_support::context(PathBuf::from("."));
    let code = crate::run(args(&["frobnicate"]), &mut ctx, &mut out, &mut crate::test_support::Broken);
    assert_eq!(code, USAGE);
    let code = crate::run(args(&["help"]), &mut ctx, &mut crate::test_support::Broken, &mut Vec::new());
    assert_eq!(code, FAILED);
}

#[test]
fn the_default_root_is_the_workspace() {
    let root = default_root();
    assert!(root.join("xtask").join("Cargo.toml").is_file(), "{}", root.display());
    assert!(root.join("Cargo.toml").is_file());
}
