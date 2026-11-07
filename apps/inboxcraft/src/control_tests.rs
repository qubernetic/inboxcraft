// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use inboxcraft_control_key::line::{MAX_LINE, read_answer, request_line};
use inboxcraft_control_key::{ENTRY, Failure, SERVICE, Token};
use inboxcraft_ui_egui::fetch::Wake;
use serde_json::{Value, json};

use super::{Call, Limits, Listening, ensure, fresh, listen, listen_with, revoke, start};
use crate::accounts::Entries;

/// A credential store in memory.
#[derive(Default)]
struct Memory(Mutex<BTreeMap<String, String>>);

impl Entries for Memory {
    fn set(&self, name: &str, password: &str) -> Result<(), String> {
        self.0.lock().unwrap().insert(name.to_owned(), password.to_owned());
        Ok(())
    }

    fn get(&self, name: &str) -> Option<String> {
        self.0.lock().unwrap().get(name).cloned()
    }

    fn delete(&self, name: &str) -> Result<(), String> {
        self.0.lock().unwrap().remove(name);
        Ok(())
    }
}

/// A credential store that refuses every write.
struct Refusing;

impl Entries for Refusing {
    fn set(&self, _name: &str, _password: &str) -> Result<(), String> {
        Err("the credential store is locked or cannot be reached".to_owned())
    }

    fn get(&self, _name: &str) -> Option<String> {
        None
    }

    fn delete(&self, _name: &str) -> Result<(), String> {
        Ok(())
    }
}

fn token(fill: u8) -> Token {
    Token::from_bytes([fill; 32])
}

fn keeping(token: &Token) -> Arc<Memory> {
    let entries = Arc::new(Memory::default());
    entries.set(ENTRY, &token.text()).unwrap();
    entries
}

/// A wake that counts.
fn counting() -> (Wake, Arc<AtomicUsize>) {
    let woken = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&woken);
    (
        Arc::new(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        }),
        woken,
    )
}

/// Answers every call as the window would, echoing the command and params, and records the commands.
fn answering(calls: Receiver<Call>) -> Arc<Mutex<Vec<String>>> {
    let ran = Arc::new(Mutex::new(Vec::new()));
    let record = Arc::clone(&ran);
    std::thread::spawn(move || {
        for call in calls {
            record.lock().unwrap().push(call.command().to_owned());
            let answer = if call.command() == "mail.archive" {
                Err(Failure::new("not_allowed", "mail.archive is an owner command"))
            } else {
                Ok(json!({ "command": call.command(), "params": call.params() }))
            };
            call.answer(answer);
        }
    });
    ran
}

struct Served {
    _dir: tempfile::TempDir,
    socket: PathBuf,
    _listening: Listening,
    ran: Arc<Mutex<Vec<String>>>,
    woken: Arc<AtomicUsize>,
}

fn serve(entries: Arc<dyn Entries>, limits: Limits) -> Served {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("inboxcraft").join("control.sock");
    let (send, calls) = mpsc::channel();
    let (wake, woken) = counting();
    let listening = listen_with(&socket, entries, send, wake, limits).unwrap();
    Served { _dir: dir, socket, _listening: listening, ran: answering(calls), woken }
}

struct Client {
    stream: UnixStream,
    reader: BufReader<UnixStream>,
}

impl Client {
    fn to(socket: &Path) -> Self {
        let stream = UnixStream::connect(socket).unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        let reader = BufReader::new(stream.try_clone().unwrap());
        Self { stream, reader }
    }

    fn send(&mut self, line: &[u8]) -> Option<Result<Value, Failure>> {
        self.stream.write_all(line).ok()?;
        let mut answer = Vec::new();
        match self.reader.read_until(b'\n', &mut answer) {
            Ok(0) | Err(_) => None,
            Ok(_) => Some(read_answer(&answer)),
        }
    }

    fn run(&mut self, token: &Token, command: &str) -> Option<Result<Value, Failure>> {
        self.send(request_line(token, command, &json!({ "limit": 3 })).as_bytes())
    }

    /// Whether the app closed the connection.
    fn closed(&mut self) -> bool {
        let mut rest = Vec::new();
        matches!(self.reader.read_to_end(&mut rest), Ok(0))
    }
}

fn code(answer: Option<Result<Value, Failure>>) -> String {
    match answer {
        Some(Err(failure)) => failure.code,
        other => panic!("not refused: {other:?}"),
    }
}

#[test]
fn the_token_is_kept_where_the_passwords_are() {
    assert_eq!(SERVICE, crate::accounts::SERVICE);
}

#[test]
fn the_right_token_runs_the_command_in_the_window_and_wakes_it() {
    let served = serve(keeping(&token(1)), Limits::default());
    let mut client = Client::to(&served.socket);
    let answer = client.run(&token(1), "mail.list").unwrap().unwrap();
    assert_eq!(answer, json!({ "command": "mail.list", "params": { "limit": 3 } }));
    // One connection carries many lines.
    let answer = client.run(&token(1), "account.list").unwrap().unwrap();
    assert_eq!(answer["command"], "account.list");
    assert_eq!(served.ran.lock().unwrap().as_slice(), ["mail.list", "account.list"]);
    assert!(served.woken.load(Ordering::SeqCst) >= 2);
}

#[test]
fn a_refusal_of_the_window_comes_back_as_its_error() {
    let served = serve(keeping(&token(1)), Limits::default());
    let mut client = Client::to(&served.socket);
    assert_eq!(code(client.run(&token(1), "mail.archive")), "not_allowed");
    assert_eq!(client.run(&token(1), "mail.list").unwrap().unwrap()["command"], "mail.list", "the connection stays");
}

#[test]
fn a_wrong_token_is_refused_and_the_connection_closed() {
    let served = serve(keeping(&token(1)), Limits::default());
    let mut client = Client::to(&served.socket);
    let answer = client.run(&token(2), "mail.list");
    assert_eq!(code(answer), "unauthorized");
    assert!(client.closed());
    assert!(served.ran.lock().unwrap().is_empty(), "nothing ran");
}

#[test]
fn a_missing_or_unreadable_token_is_refused() {
    let served = serve(keeping(&token(1)), Limits::default());
    for line in [r#"{"command":"mail.list"}"#, r#"{"token":"0101","command":"mail.list"}"#, r#"{"token":null,"command":"mail.list"}"#] {
        let mut client = Client::to(&served.socket);
        assert_eq!(code(client.send(format!("{line}\n").as_bytes())), "unauthorized", "{line}");
    }
    assert!(served.ran.lock().unwrap().is_empty());
}

#[test]
fn a_revoked_token_is_refused_from_the_next_line_on() {
    let entries = keeping(&token(1));
    let served = serve(Arc::clone(&entries) as Arc<dyn Entries>, Limits::default());
    let mut client = Client::to(&served.socket);
    assert!(client.run(&token(1), "mail.list").unwrap().is_ok());
    let mut out = Vec::new();
    let mut err = Vec::new();
    assert_eq!(revoke(entries.as_ref(), &|| Ok(token(9)), &mut out, &mut err), 0);
    assert_eq!(code(client.run(&token(1), "mail.list")), "unauthorized", "the open connection too");
    let mut client = Client::to(&served.socket);
    assert_eq!(code(client.run(&token(1), "mail.list")), "unauthorized");
    let mut client = Client::to(&served.socket);
    assert!(client.run(&token(9), "mail.list").unwrap().is_ok(), "the new token runs");
    assert_eq!(served.ran.lock().unwrap().len(), 2);
}

#[test]
fn without_a_token_in_the_credential_store_everything_is_refused() {
    let served = serve(Arc::new(Memory::default()), Limits::default());
    let mut client = Client::to(&served.socket);
    assert_eq!(code(client.run(&token(0), "mail.list")), "unauthorized");
    let entries = Arc::new(Memory::default());
    entries.set(ENTRY, "not a token").unwrap();
    let served = serve(entries, Limits::default());
    let mut client = Client::to(&served.socket);
    assert_eq!(code(client.run(&token(0), "mail.list")), "unauthorized");
}

#[test]
fn a_line_that_is_no_request_is_answered_and_the_connection_stays() {
    let served = serve(keeping(&token(1)), Limits::default());
    let mut client = Client::to(&served.socket);
    for line in [&b"not json\n"[..], b"\xff\xfe\x00\n", b"[]\n", b"{\"token\":1}\n", b"\r\n"] {
        assert_eq!(code(client.send(line)), "bad_line", "{:?}", String::from_utf8_lossy(line));
    }
    assert!(client.run(&token(1), "mail.list").unwrap().is_ok());
}

#[test]
fn a_line_over_the_limit_is_refused_and_the_connection_closed() {
    let served = serve(keeping(&token(1)), Limits::default());
    let mut client = Client::to(&served.socket);
    let mut line = vec![b'x'; MAX_LINE + 10];
    line.push(b'\n');
    assert_eq!(code(client.send(&line)), "too_long");
    assert!(client.closed());
}

#[test]
fn a_connection_that_says_nothing_is_closed() {
    let limits = Limits { idle: Duration::from_millis(100), ..Limits::default() };
    let served = serve(keeping(&token(1)), limits);
    let mut client = Client::to(&served.socket);
    client.stream.write_all(b"{\"tok").unwrap();
    assert!(client.closed());
}

#[test]
fn a_window_that_does_not_answer_in_time_is_busy() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("control.sock");
    let (send, calls) = mpsc::channel();
    let limits = Limits { patience: Duration::from_millis(50), ..Limits::default() };
    let _listening = listen_with(&socket, keeping(&token(1)), send, Arc::new(|| {}), limits).unwrap();
    let mut client = Client::to(&socket);
    assert_eq!(code(client.run(&token(1), "mail.list")), "busy");
    drop(calls);
    assert_eq!(code(client.run(&token(1), "mail.list")), "closing", "a window that is gone");
}

#[test]
fn clients_beyond_the_limit_are_turned_away() {
    let limits = Limits { clients: 1, ..Limits::default() };
    let served = serve(keeping(&token(1)), limits);
    let mut first = Client::to(&served.socket);
    assert!(first.run(&token(1), "mail.list").unwrap().is_ok());
    let mut second = Client::to(&served.socket);
    let mut answer = Vec::new();
    second.reader.read_until(b'\n', &mut answer).unwrap();
    assert_eq!(read_answer(&answer).unwrap_err().code, "busy");
    drop(first);
    // The first one's place frees once its thread has seen it go.
    let mut ran = false;
    for _ in 0..100 {
        let mut third = Client::to(&served.socket);
        if third.run(&token(1), "mail.list").is_some_and(|answer| answer.is_ok()) {
            ran = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(ran);
}

#[test]
fn the_folder_and_the_socket_are_the_owners_alone() {
    let served = serve(keeping(&token(1)), Limits::default());
    let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&served.socket), 0o600);
    assert_eq!(mode(served.socket.parent().unwrap()), 0o700);
}

#[test]
fn a_wider_folder_is_narrowed() {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("inboxcraft");
    std::fs::create_dir(&folder).unwrap();
    std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o755)).unwrap();
    let (send, _calls) = mpsc::channel();
    let _listening = listen(&folder.join("control.sock"), keeping(&token(1)), send, Arc::new(|| {})).unwrap();
    assert_eq!(std::fs::metadata(&folder).unwrap().permissions().mode() & 0o777, 0o700);
}

#[test]
fn a_socket_left_behind_is_replaced_but_one_in_use_is_not() {
    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("control.sock");
    drop(std::os::unix::net::UnixListener::bind(&socket).unwrap());
    assert!(socket.exists(), "left behind");
    let (send, _calls) = mpsc::channel();
    let listening = listen(&socket, keeping(&token(1)), send.clone(), Arc::new(|| {})).unwrap();
    let error = listen(&socket, keeping(&token(1)), send, Arc::new(|| {})).unwrap_err();
    assert!(error.to_string().contains("already"), "{error}");
    drop(listening);
    assert!(!socket.exists(), "the socket goes with the server");
}

#[test]
fn ensure_keeps_a_token_and_makes_one_where_there_is_none() {
    let entries = keeping(&token(1));
    ensure(entries.as_ref(), &|| Ok(token(2))).unwrap();
    assert_eq!(entries.get(ENTRY), Some(token(1).text()), "kept");
    let entries = Memory::default();
    ensure(&entries, &|| Ok(token(2))).unwrap();
    assert_eq!(entries.get(ENTRY), Some(token(2).text()), "made");
    entries.set(ENTRY, "garbage").unwrap();
    ensure(&entries, &|| Ok(token(3))).unwrap();
    assert_eq!(entries.get(ENTRY), Some(token(3).text()), "replaced");
}

#[test]
fn ensure_says_why_it_could_not_keep_a_token() {
    let error = ensure(&Refusing, &|| Ok(token(2))).unwrap_err();
    assert!(error.contains("locked"), "{error}");
    let error = ensure(&Memory::default(), &|| Err(std::io::Error::other("no randomness"))).unwrap_err();
    assert!(error.contains("no randomness"), "{error}");
}

#[test]
fn revoke_says_what_it_did_and_never_shows_the_token() {
    let entries = keeping(&token(1));
    let (mut out, mut err) = (Vec::new(), Vec::new());
    assert_eq!(revoke(entries.as_ref(), &|| Ok(token(0xab)), &mut out, &mut err), 0);
    let said = String::from_utf8(out).unwrap();
    assert!(said.contains("refused"), "{said}");
    assert!(!said.contains(&token(0xab).text()));
    assert!(err.is_empty());
    let (mut out, mut err) = (Vec::new(), Vec::new());
    assert_eq!(revoke(&Refusing, &|| Ok(token(2)), &mut out, &mut err), 1);
    assert!(String::from_utf8(err).unwrap().contains("locked"));
    let (mut out, mut err) = (Vec::new(), Vec::new());
    assert_eq!(revoke(&Memory::default(), &|| Err(std::io::Error::other("no randomness")), &mut out, &mut err), 1);
    assert!(String::from_utf8(err).unwrap().contains("no randomness"));
}

#[test]
fn fresh_tokens_come_from_the_systems_random_source() {
    let (a, b) = (fresh().unwrap(), fresh().unwrap());
    assert_ne!(a, b);
}

#[test]
fn a_call_answered_after_its_client_left_is_dropped_quietly() {
    let (send, calls) = mpsc::channel::<Call>();
    let (answers, heard) = mpsc::channel();
    send.send(Call::new("mail.list".to_owned(), json!({}), answers)).unwrap();
    drop(heard);
    let call = calls.recv().unwrap();
    assert_eq!((call.command(), call.params()), ("mail.list", &json!({})));
    call.answer(Ok(json!({})));
}

#[test]
fn start_keeps_a_token_and_listens_in_the_runtime_folder() {
    let dir = tempfile::tempdir().unwrap();
    let entries = Arc::new(Memory::default());
    let (listening, calls) = start(Some(dir.path()), false, Arc::clone(&entries) as Arc<dyn Entries>, &|| Ok(token(4)), Arc::new(|| {})).unwrap();
    let ran = answering(calls);
    let socket = inboxcraft_control_key::socket(dir.path(), false);
    assert!(Client::to(&socket).run(&token(4), "mail.list").unwrap().is_ok());
    assert_eq!(ran.lock().unwrap().as_slice(), ["mail.list"]);
    drop(listening);
    let (_listening, _calls) = start(Some(dir.path()), true, entries, &|| Ok(token(5)), Arc::new(|| {})).unwrap();
    assert!(inboxcraft_control_key::socket(dir.path(), true).exists(), "the Flatpak's folder");
}

#[test]
fn start_says_why_there_is_no_channel() {
    let none = |runtime: Option<&Path>, entries: Arc<dyn Entries>| start(runtime, false, entries, &|| Ok(token(4)), Arc::new(|| {})).err().unwrap();
    assert!(none(None, Arc::new(Memory::default())).contains("XDG_RUNTIME_DIR"));
    assert!(none(Some(Path::new("relative")), Arc::new(Memory::default())).contains("XDG_RUNTIME_DIR"));
    let dir = tempfile::tempdir().unwrap();
    assert!(none(Some(dir.path()), Arc::new(Refusing)).contains("locked"));
    let file = dir.path().join("file");
    std::fs::write(&file, b"").unwrap();
    assert!(none(Some(&file), Arc::new(Memory::default())).contains("could not be opened"));
}
