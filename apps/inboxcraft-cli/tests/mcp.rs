// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `inboxcraft-cli mcp`: the read-only MCP server on the `testkit` fixture store, over stdin and stdout.

use std::ffi::OsString;
use std::io::{self, Cursor, Write};
use std::process::{Command, Stdio};

use inboxcraft_testkit::store;
use serde_json::{Value, json};

const INITIALIZE: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"1"}}}"#;

fn requests() -> String {
    [
        INITIALIZE.to_owned(),
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#.to_owned(),
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": "mail_read", "arguments": { "id": store::LUNCH } } })
            .to_string(),
    ]
    .join("\n")
}

fn mcp(args: &[&str], input: &str) -> (u8, String, String) {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = inboxcraft_cli::run_with_input(args.iter().map(OsString::from), &mut Cursor::new(input.as_bytes()), &mut out, &mut err);
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

fn answers(out: &str) -> Vec<Value> {
    out.lines().map(|line| serde_json::from_str(line).unwrap()).collect()
}

#[test]
fn mcp_serves_the_tools_on_the_store_until_the_input_ends() {
    let dir = tempfile::tempdir().unwrap();
    store::fixture(dir.path()).unwrap();
    let (code, out, err) = mcp(&["--store", dir.path().to_str().unwrap(), "mcp"], &requests());
    assert_eq!((code, err.as_str()), (0, ""));
    let answers = answers(&out);
    assert_eq!(answers.len(), 2, "{out}");
    assert_eq!(answers[0]["result"]["serverInfo"]["name"], "inboxcraft");
    let text = answers[1]["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("\"Lunch on Thursday\""), "{text}");
}

#[test]
fn mcp_needs_a_store_and_no_parameters() {
    let dir = tempfile::tempdir().unwrap();
    for (args, code, says) in [
        (vec!["mcp"], 2, "give --store <dir>"),
        (vec!["--store", "/nowhere", "mcp", "{}"], 2, "mcp takes no parameters"),
        (vec!["--store", dir.path().to_str().unwrap(), "mcp"], 2, "there is no store in"),
    ] {
        let (got, out, err) = mcp(&args, INITIALIZE);
        assert_eq!((got, out.as_str()), (code, ""), "{args:?}");
        assert!(err.contains(says), "{args:?}: {err}");
    }
}

#[test]
fn run_gives_mcp_an_empty_input() {
    let dir = tempfile::tempdir().unwrap();
    store::fixture(dir.path()).unwrap();
    let mut out = Vec::new();
    let mut err = Vec::new();
    let args = ["--store", dir.path().to_str().unwrap(), "mcp"].map(OsString::from);
    assert_eq!(inboxcraft_cli::run(args, &mut out, &mut err), 0);
    assert!(out.is_empty() && err.is_empty());
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
fn a_closed_output_ends_mcp_with_exit_code_1() {
    let dir = tempfile::tempdir().unwrap();
    store::fixture(dir.path()).unwrap();
    let mut err = Vec::new();
    let args = ["--json", "--store", dir.path().to_str().unwrap(), "mcp"].map(OsString::from);
    let code = inboxcraft_cli::run_with_input(args, &mut Cursor::new(INITIALIZE.as_bytes()), &mut Broken, &mut err);
    assert_eq!(code, 1);
    let error: Value = serde_json::from_slice(&err).unwrap();
    assert_eq!(error["error"]["code"], "io");
    Broken.flush().unwrap_err();
}

#[test]
fn the_binary_serves_mcp_over_stdin_and_stdout() {
    let dir = tempfile::tempdir().unwrap();
    store::fixture(dir.path()).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_inboxcraft-cli"))
        .args(["--store", dir.path().to_str().unwrap(), "mcp"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(requests().as_bytes()).unwrap();
    stdin.write_all(b"\n").unwrap();
    drop(stdin);
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let answers = answers(&String::from_utf8(output.stdout).unwrap());
    assert_eq!(answers.len(), 2);
    assert_eq!(answers[1]["id"], 2);
    assert_eq!(answers[1]["result"]["isError"], false);
}
