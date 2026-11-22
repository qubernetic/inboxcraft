// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Without `--store`, the CLI and `mcp` run their commands in the running app through the control channel. The app is
//! a fake here: a thread on a socket in a temporary runtime folder that speaks the channel's lines.

#![cfg(unix)]

use std::ffi::OsString;
use std::io::{BufRead, BufReader, Cursor, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use inboxcraft_cli::{Reach, run_in};
use inboxcraft_control_key::line::{MAX_OWNER_LINE, Request, answer_line, read_request};
use inboxcraft_control_key::{Failure, Token, socket};
use serde_json::{Value, json};
use tempfile::TempDir;

fn token(fill: u8) -> Token {
    Token::from_bytes([fill; 32])
}

/// What the fake app saw: the whole requests.
type Seen = Arc<Mutex<Vec<Request>>>;

/// A new runtime folder, the user's alone (`0700`) as `$XDG_RUNTIME_DIR` is.
fn runtime_folder() -> TempDir {
    let runtime = tempfile::tempdir().unwrap();
    std::fs::set_permissions(runtime.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    runtime
}

/// The socket in `runtime`, bound in a folder of the user's alone, as the app makes it.
fn bind(runtime: &Path, sandboxed: bool) -> (PathBuf, UnixListener) {
    let path = socket(runtime, sandboxed);
    let folder = path.parent().unwrap();
    std::fs::create_dir_all(folder).unwrap();
    std::fs::set_permissions(folder, std::fs::Permissions::from_mode(0o700)).unwrap();
    let listener = UnixListener::bind(&path).unwrap();
    (path, listener)
}

/// A fake app in a new runtime folder (in the Flatpak's place when `sandboxed`) that knows `known` and answers every
/// request with the right token by `answer`.
fn app(known: Token, sandboxed: bool, answer: fn(&str, &Value) -> Result<Value, Failure>) -> (TempDir, Seen) {
    let runtime = runtime_folder();
    let (_, listener) = bind(runtime.path(), sandboxed);
    let seen = Seen::default();
    let record = Arc::clone(&seen);
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut writer = stream.try_clone().unwrap();
            for line in BufReader::new(stream).split(b'\n') {
                let Ok(line) = line else { break };
                let request = read_request(&line).unwrap();
                let reply = if request.token.as_ref() == Some(&known) {
                    let reply = answer(&request.command, &request.params);
                    record.lock().unwrap().push(request);
                    reply
                } else {
                    Err(Failure::new("unauthorized", "the control token is wrong or revoked"))
                };
                writer.write_all(answer_line(&reply).as_bytes()).unwrap();
            }
        }
    });
    (runtime, seen)
}

/// How the fake app answers a request.
type Answer = fn(&str, &Value) -> Result<Value, Failure>;

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
    let empty = runtime_folder();
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
    let runtime = runtime_folder();
    let (_, listener) = bind(runtime.path(), false);
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

/// A fake app in `runtime` that counts the connections it takes and the bytes it reads, and answers nothing.
fn counting(runtime: &Path) -> (Arc<AtomicUsize>, Arc<AtomicUsize>) {
    let (_, listener) = bind(runtime, false);
    let (connections, bytes) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
    let (taken, read) = (Arc::clone(&connections), Arc::clone(&bytes));
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            taken.fetch_add(1, Ordering::SeqCst);
            let mut got = Vec::new();
            let _ = stream.read_to_end(&mut got);
            read.fetch_add(got.len(), Ordering::SeqCst);
        }
    });
    (connections, bytes)
}

/// Gives the fake app a moment, then says what it took and read.
fn taken_and_read(connections: &AtomicUsize, bytes: &AtomicUsize) -> (usize, usize) {
    std::thread::sleep(std::time::Duration::from_millis(50));
    (connections.load(Ordering::SeqCst), bytes.load(Ordering::SeqCst))
}

#[test]
fn a_runtime_folder_open_to_others_is_not_trusted_with_the_token() {
    for mode in [0o777, 0o755] {
        let runtime = runtime_folder();
        let (connections, bytes) = counting(runtime.path());
        std::fs::set_permissions(runtime.path(), std::fs::Permissions::from_mode(mode)).unwrap();
        let (code, out, err) = cli_in(&reach(Some(runtime.path()), Some(token(1))), &["--json", "mail.list"], "");
        assert_eq!((code, out.as_str()), (1, ""), "{mode:o}");
        let error: Value = serde_json::from_str(&err).unwrap();
        assert_eq!(error["error"]["code"], "not_running", "{err}");
        assert!(err.contains("alone") && err.contains("--store"), "{err}");
        assert_eq!(taken_and_read(&connections, &bytes), (0, 0), "{mode:o}");
    }
}

#[test]
fn a_socket_folder_open_to_others_is_not_trusted_with_the_token() {
    let runtime = runtime_folder();
    let (connections, bytes) = counting(runtime.path());
    let folder = socket(runtime.path(), false).parent().unwrap().to_path_buf();
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o755)).unwrap();
    let (code, _, err) = cli_in(&reach(Some(runtime.path()), Some(token(1))), &["--json", "mail.list"], "");
    assert_eq!(code, 1);
    assert!(err.contains("not_running") && err.contains("is not running"), "{err}");
    assert_eq!(taken_and_read(&connections, &bytes), (0, 0));
}

#[test]
fn a_socket_folder_that_is_a_link_is_not_trusted_with_the_token() {
    let elsewhere = runtime_folder();
    let (connections, bytes) = counting(elsewhere.path());
    let runtime = runtime_folder();
    let folder = socket(runtime.path(), false).parent().unwrap().to_path_buf();
    std::os::unix::fs::symlink(socket(elsewhere.path(), false).parent().unwrap(), &folder).unwrap();
    let (code, _, err) = cli_in(&reach(Some(runtime.path()), Some(token(1))), &["--json", "mail.list"], "");
    assert_eq!(code, 1);
    assert!(err.contains("is not running"), "{err}");
    assert_eq!(taken_and_read(&connections, &bytes), (0, 0));
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
    assert_eq!(seen.lock().unwrap()[0].command, "mail.list");
}

#[test]
fn without_a_store_mail_why_runs_in_the_running_app() {
    let (runtime, seen) =
        app(token(1), false, |_, _| Ok(json!({ "reasons": [{ "kind": "thread", "text_id": "why.thread.yours", "args": { "messages": 2 } }] })));
    let input = format!(
        "{INITIALIZE}\n{}\n",
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": "mail_why", "arguments": { "id": 7 } } })
    );
    let (code, out, err) = cli_in(&reach(Some(runtime.path()), Some(token(1))), &["mcp"], &input);
    assert_eq!((code, err.as_str()), (0, ""), "{out}");
    let answers: Vec<Value> = out.lines().map(|line| serde_json::from_str(line).unwrap()).collect();
    assert_eq!(answers.len(), 2, "{out}");
    assert_eq!(answers[1]["result"]["isError"], false, "{out}");
    let text = answers[1]["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.starts_with("mail_why: message 7, 1 reason, most specific first\n"), "{text}");
    assert!(text.ends_with("\nthread | why.thread.yours | messages 2\n"), "{text}");
    let seen: Vec<(String, Value)> = seen.lock().unwrap().iter().map(|request| (request.command.clone(), request.params.clone())).collect();
    assert_eq!(seen, [("mail.why".to_owned(), json!({ "id": 7 }))]);
}

#[test]
fn mcp_without_a_running_app_says_so_before_it_reads_a_line() {
    let empty = runtime_folder();
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

#[test]
fn ask_owner_sends_the_request_as_one_that_asks() {
    let (runtime, seen) = app(token(1), false, echo);
    let reach = reach(Some(runtime.path()), Some(token(1)));
    let (code, out, err) = cli_in(&reach, &["--ask-owner", "--json", "calendar.import", r#"{"calendar":1,"ics":""}"#], "");
    assert_eq!((code, err.as_str()), (0, ""), "{out}");
    let (code, _, err) = cli_in(&reach, &["--json", "calendar.import", r#"{"calendar":1,"ics":""}"#], "");
    assert_eq!((code, err.as_str()), (0, ""));
    let seen = seen.lock().unwrap();
    let [asked, plain] = seen.as_slice() else { panic!("{seen:?}") };
    assert!(asked.ask_owner);
    assert_eq!(asked.client.as_deref(), Some("inboxcraft-cli"));
    assert_eq!((asked.command.as_str(), &asked.params), ("calendar.import", &json!({ "calendar": 1, "ics": "" })));
    assert!(!plain.ask_owner);
    assert_eq!(plain.client, None);
}

#[test]
fn a_denied_or_timed_out_request_is_exit_code_1_with_its_code() {
    let answers: [(&str, Answer); 3] = [
        ("denied", |_, _| Err(Failure::new("denied", "the owner denied the request"))),
        ("timed_out", |_, _| Err(Failure::new("timed_out", "the owner did not answer within two minutes"))),
        ("busy", |_, _| Err(Failure::new("busy", "eight requests wait for the owner already"))),
    ];
    for (code_wanted, answer) in answers {
        let (runtime, _seen) = app(token(1), false, answer);
        let (code, out, err) = cli_in(&reach(Some(runtime.path()), Some(token(1))), &["--ask-owner", "--json", "calendar.import", "{}"], "");
        assert_eq!((code, out.as_str()), (1, ""), "{code_wanted}");
        let error: Value = serde_json::from_str(&err).unwrap();
        assert_eq!(error["error"]["code"], code_wanted, "{err}");
        assert!(error["error"]["message"].as_str().unwrap().contains("owner"), "{err}");
    }
}

#[test]
fn params_come_from_a_file_too() {
    let (runtime, seen) = app(token(1), false, |_, _| Ok(json!({ "added": 1 })));
    let reach = reach(Some(runtime.path()), Some(token(1)));
    let files = tempfile::tempdir().unwrap();
    let ics = "x".repeat(200 * 1024);
    let file = files.path().join("params.json");
    std::fs::write(&file, json!({ "calendar": 1, "ics": ics }).to_string()).unwrap();
    let path = file.to_str().unwrap();
    let (code, out, err) = cli_in(&reach, &["--ask-owner", "--json", "calendar.import", "--params-file", path], "");
    assert_eq!((code, err.as_str()), (0, ""), "{out}");
    let given = format!("--params-file={path}");
    let (code, _, err) = cli_in(&reach, &["--ask-owner", &given, "calendar.import"], "");
    assert_eq!((code, err.as_str()), (0, ""));
    {
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 2);
        assert!(seen.iter().all(|request| request.ask_owner && request.params["ics"].as_str() == Some(ics.as_str())));
    }
    let too_big = files.path().join("big.json");
    std::fs::write(&too_big, format!("{{\"ics\":\"{}\"}}", "x".repeat(MAX_OWNER_LINE))).unwrap();
    let list = files.path().join("list.json");
    std::fs::write(&list, "[1, 2]").unwrap();
    let broken = files.path().join("broken.json");
    std::fs::write(&broken, "{ not json").unwrap();
    let missing = files.path().join("missing.json");
    for args in [
        vec!["--ask-owner", "calendar.import", "{}", "--params-file", path],
        vec!["--ask-owner", "calendar.import", "--params-file", missing.to_str().unwrap()],
        vec!["--ask-owner", "calendar.import", "--params-file", too_big.to_str().unwrap()],
        vec!["--ask-owner", "calendar.import", "--params-file", list.to_str().unwrap()],
        vec!["--ask-owner", "calendar.import", "--params-file", broken.to_str().unwrap()],
        vec!["--ask-owner", "calendar.import", "--params-file", path, "--params-file", path],
        vec!["--ask-owner", "calendar.import", "--params-file"],
        vec!["--ask-owner", "calendar.import", "--params-file="],
        vec!["--ask-owner", "mcp"],
        vec!["--ask-owner", "commands"],
        vec!["--ask-owner", "--help"],
    ] {
        let mut all = vec!["--json"];
        all.extend_from_slice(&args);
        let (code, out, err) = cli_in(&reach, &all, "");
        assert_eq!((code, out.as_str()), (2, ""), "{args:?}: {err}");
        let error: Value = serde_json::from_str(&err).unwrap();
        assert_eq!(error["error"]["code"], "usage", "{args:?}: {err}");
    }
    assert_eq!(seen.lock().unwrap().len(), 2, "nothing more was sent");
}

#[test]
fn without_a_running_app_asking_says_the_app_is_needed() {
    let empty = runtime_folder();
    for reach in [reach(Some(empty.path()), Some(token(1))), reach(None, Some(token(1)))] {
        let (code, out, err) = cli_in(&reach, &["--ask-owner", "--json", "calendar.import", "{}"], "");
        assert_eq!((code, out.as_str()), (1, ""));
        let error: Value = serde_json::from_str(&err).unwrap();
        assert_eq!(error["error"]["code"], "not_running", "{err}");
        let message = error["error"]["message"].as_str().unwrap();
        assert!(message.ends_with("an owner command needs the running InboxCraft to ask the owner"), "{message}");
        assert!(!message.contains("--store"), "a store cannot ask the owner: {message}");
    }
}

#[test]
fn a_not_allowed_answer_points_at_ask_owner() {
    let (runtime, _seen) = app(token(1), false, |command, _| Err(Failure::new("not_allowed", format!("{command} is an owner command"))));
    let (code, _, err) = cli_in(&reach(Some(runtime.path()), Some(token(1))), &["--json", "calendar.import", "{}"], "");
    assert_eq!(code, 1);
    let error: Value = serde_json::from_str(&err).unwrap();
    assert!(error["error"]["message"].as_str().unwrap().ends_with("ask the owner with --ask-owner"), "{err}");
}

#[test]
fn an_owner_command_the_owner_cannot_be_asked_about_is_not_allowed_without_pointing_at_ask_owner() {
    let (runtime, _seen) = app(token(1), false, |command, _| {
        Err(Failure::new("not_allowed", format!("the owner can be asked only for calendar.import: the command {command} is owner")))
    });
    let (code, out, err) = cli_in(&reach(Some(runtime.path()), Some(token(1))), &["--ask-owner", "--json", "mail.send", r#"{"id":7}"#], "");
    assert_eq!((code, out.as_str()), (1, ""));
    let error: Value = serde_json::from_str(&err).unwrap();
    assert_eq!(error["error"]["code"], "not_allowed", "{err}");
    let message = error["error"]["message"].as_str().unwrap();
    assert!(message.contains("calendar.import"), "the app's reason: {message}");
    assert!(!message.contains("--ask-owner"), "it was asked with --ask-owner already: {message}");
}

#[test]
fn the_cli_waits_longer_for_the_owner_than_the_app_does() {
    // The app waits 990 s (`OWNER_PATIENCE`, held to eight questions of two minutes and its half minute by the app's
    // own test); the app is no dependency of the CLI.
    assert_eq!(inboxcraft_cli::ASK_PATIENCE, std::time::Duration::from_secs(1020));
    assert!(inboxcraft_cli::ASK_PATIENCE > std::time::Duration::from_secs(990));
}
