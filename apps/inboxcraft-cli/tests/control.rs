// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Without `--store`, the CLI and `mcp` run their commands in the running app through the control channel. The app is
//! a fake here: a thread on a socket in a temporary runtime folder that speaks the channel's lines.

#![cfg(unix)]

use std::ffi::OsString;
use std::io::{BufRead, BufReader, Cursor, Write};
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::sync::{Arc, Mutex};

use inboxcraft_cli::{Reach, run_in};
use inboxcraft_control_key::line::{answer_line, read_request};
use inboxcraft_control_key::{Failure, Token, socket};
use serde_json::{Value, json};
use tempfile::TempDir;

fn token(fill: u8) -> Token {
    Token::from_bytes([fill; 32])
}

/// What the fake app saw: the commands with their params.
type Seen = Arc<Mutex<Vec<(String, Value)>>>;

/// A fake app in a new runtime folder (in the Flatpak's place when `sandboxed`) that knows `known` and answers every
/// request with the right token by `answer`.
fn app(known: Token, sandboxed: bool, answer: fn(&str, &Value) -> Result<Value, Failure>) -> (TempDir, Seen) {
    let runtime = tempfile::tempdir().unwrap();
    let path = socket(runtime.path(), sandboxed);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let listener = UnixListener::bind(&path).unwrap();
    let seen = Seen::default();
    let record = Arc::clone(&seen);
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut writer = stream.try_clone().unwrap();
            for line in BufReader::new(stream).split(b'\n') {
                let Ok(line) = line else { break };
                let request = read_request(&line).unwrap();
                let reply = if request.token.as_ref() == Some(&known) {
                    record.lock().unwrap().push((request.command.clone(), request.params.clone()));
                    answer(&request.command, &request.params)
                } else {
                    Err(Failure::new("unauthorized", "the control token is wrong or revoked"))
                };
                writer.write_all(answer_line(&reply).as_bytes()).unwrap();
            }
        }
    });
    (runtime, seen)
}

fn echo(command: &str, params: &Value) -> Result<Value, Failure> {
    Ok(json!({ "command": command, "params": params }))
}

fn reach(runtime: Option<&Path>, shown: Option<Token>) -> Reach {
    Reach::new(
        runtime.map(Path::to_path_buf),
        Box::new(move || shown.clone().ok_or_else(|| Failure::new("no_token", "there is no control token in the credential store"))),
    )
}

fn cli_in(reach: &Reach, args: &[&str], input: &str) -> (u8, String, String) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run_in(args.iter().map(OsString::from), &mut Cursor::new(input.as_bytes()), &mut out, &mut err, reach);
    (code, String::from_utf8_lossy(&out).into_owned(), String::from_utf8_lossy(&err).into_owned())
}

#[test]
fn without_a_store_a_command_runs_in_the_running_app() {
    let (runtime, seen) = app(token(1), false, echo);
    let reach = reach(Some(runtime.path()), Some(token(1)));
    let (code, out, err) = cli_in(&reach, &["--json", "mail.list", r#"{"limit":2}"#], "");
    assert_eq!((code, err.as_str()), (0, ""), "{out}");
    let answer: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(answer, json!({ "command": "mail.list", "params": { "limit": 2 } }));
    let (code, out, _) = cli_in(&reach, &["account.list"], "");
    assert_eq!(code, 0);
    assert!(out.contains("command: account.list"), "{out}");
    assert_eq!(seen.lock().unwrap().len(), 2);
}

#[test]
fn the_flatpaks_socket_is_found_too() {
    let (runtime, seen) = app(token(1), true, echo);
    let (code, _, err) = cli_in(&reach(Some(runtime.path()), Some(token(1))), &["mail.list"], "");
    assert_eq!((code, err.as_str()), (0, ""));
    assert_eq!(seen.lock().unwrap().len(), 1);
}

#[test]
fn a_wrong_token_is_refused_with_its_code() {
    let (runtime, seen) = app(token(1), false, echo);
    let (code, out, err) = cli_in(&reach(Some(runtime.path()), Some(token(2))), &["--json", "mail.list"], "");
    assert_eq!((code, out.as_str()), (1, ""));
    let error: Value = serde_json::from_str(&err).unwrap();
    assert_eq!(error["error"]["code"], "unauthorized");
    assert!(error["error"]["message"].as_str().unwrap().contains("--revoke-control") || err.contains("revoked"), "{err}");
    assert!(seen.lock().unwrap().is_empty());
}

#[test]
fn an_owner_command_refused_by_the_app_says_why() {
    let (runtime, _seen) = app(token(1), false, |command, _| Err(Failure::new("not_allowed", format!("{command} is an owner command"))));
    let (code, _, err) = cli_in(&reach(Some(runtime.path()), Some(token(1))), &["mail.archive", r#"{"id":1}"#], "");
    assert_eq!(code, 1);
    assert!(err.contains("owner command") && err.contains("only safe commands run"), "{err}");
}

#[test]
fn without_a_running_app_or_a_runtime_folder_the_cli_says_so() {
    let empty = tempfile::tempdir().unwrap();
    for (reach, says) in [(reach(Some(empty.path()), Some(token(1))), "is not running"), (reach(None, Some(token(1))), "XDG_RUNTIME_DIR")] {
        let (code, out, err) = cli_in(&reach, &["--json", "mail.list"], "");
        assert_eq!((code, out.as_str()), (1, ""));
        let error: Value = serde_json::from_str(&err).unwrap();
        assert_eq!(error["error"]["code"], "not_running", "{err}");
        assert!(err.contains(says) && err.contains("--store"), "{err}");
    }
}

#[test]
fn without_a_token_the_cli_says_so_and_sends_nothing() {
    let (runtime, seen) = app(token(1), false, echo);
    let (code, _, err) = cli_in(&reach(Some(runtime.path()), None), &["--json", "mail.list"], "");
    assert_eq!(code, 1);
    assert!(err.contains("no_token"), "{err}");
    assert!(seen.lock().unwrap().is_empty());
}

#[test]
fn an_answer_that_is_no_answer_is_an_error_not_a_panic() {
    let runtime = tempfile::tempdir().unwrap();
    let path = socket(runtime.path(), false);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let listener = UnixListener::bind(&path).unwrap();
    std::thread::spawn(move || {
        let mut asked = 0;
        for stream in listener.incoming().flatten() {
            let mut writer = stream.try_clone().unwrap();
            let mut line = Vec::new();
            // The CLI's look for the socket says nothing.
            if BufReader::new(stream).read_until(b'\n', &mut line).unwrap_or(0) == 0 {
                continue;
            }
            asked += 1;
            // First garbage, then nothing at all.
            if asked == 1 {
                let _ = writer.write_all(b"\xff not an answer\n");
            }
        }
    });
    let reach = reach(Some(runtime.path()), Some(token(1)));
    let (code, _, err) = cli_in(&reach, &["--json", "mail.list"], "");
    assert_eq!(code, 1);
    assert!(err.contains("bad_answer"), "{err}");
    let (code, _, err) = cli_in(&reach, &["--json", "mail.list"], "");
    assert_eq!(code, 1);
    assert!(err.contains("bad_answer") || err.contains("closed"), "{err}");
}

const INITIALIZE: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}"#;

#[test]
fn without_a_store_mcp_runs_its_tools_in_the_running_app() {
    let (runtime, seen) = app(token(1), false, |_, _| Ok(json!({ "messages": [], "total": 0 })));
    let input = format!(
        "{INITIALIZE}\n{}\n",
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": "mail_list", "arguments": {} } })
    );
    let (code, out, err) = cli_in(&reach(Some(runtime.path()), Some(token(1))), &["mcp"], &input);
    assert_eq!((code, err.as_str()), (0, ""), "{out}");
    let answers: Vec<Value> = out.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    assert_eq!(answers.len(), 2, "{out}");
    assert_eq!(answers[1]["result"]["isError"], false, "{out}");
    assert_eq!(seen.lock().unwrap()[0].0, "mail.list");
}

#[test]
fn mcp_without_a_running_app_says_so_before_it_reads_a_line() {
    let empty = tempfile::tempdir().unwrap();
    let (code, out, err) = cli_in(&reach(Some(empty.path()), Some(token(1))), &["mcp"], INITIALIZE);
    assert_eq!((code, out.as_str()), (1, ""));
    assert!(err.contains("is not running"), "{err}");
}

#[test]
fn a_bridged_tool_that_the_app_refuses_is_a_tool_error() {
    let (runtime, _seen) = app(token(1), false, echo);
    let call = json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": "mail_list", "arguments": {} } });
    let (code, out, _) = cli_in(&reach(Some(runtime.path()), Some(token(2))), &["mcp"], &format!("{call}\n"));
    assert_eq!(code, 0);
    let answer: Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(answer["result"]["isError"], true);
    assert!(answer["result"]["content"][0]["text"].as_str().unwrap().starts_with("error unauthorized"), "{out}");
}

#[test]
fn the_users_reach_reads_the_runtime_folder() {
    let reach = Reach::of_user();
    assert_eq!(reach.runtime(), std::env::var_os("XDG_RUNTIME_DIR").map(std::path::PathBuf::from).as_deref());
}
