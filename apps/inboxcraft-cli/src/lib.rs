// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The command-line client: every command by id, in the running app or headless on a local store (`docs/cli.md`).
//!
//! `inboxcraft-cli [--json] <command> [<params>]` runs the command in the running app through its control channel
//! ([`Reach`], `docs/control-protocol.md`): the socket in the user's runtime folder, the token from the OS credential
//! store. `inboxcraft-cli --store <dir> [--json] <command> [<params>]` instead opens the store in `<dir>` and runs the
//! command through the engine itself. Either way it runs as an agent ([`Caller::Agent`]) and writes its answer: as text, one field a line, or with `--json` as
//! one JSON object on one line (snake_case keys, ISO 8601 dates in UTC, fields only ever added). Only `safe` commands
//! run: an `owner` or `never` command is refused with a plain error, since nobody is there to approve it. With
//! `--ask-owner` the running app asks the owner to allow an `owner` command once and answers with the command's answer,
//! or `denied`, `timed_out` or `busy`; on a store, or with no app running, it is refused as before. `--params-file
//! <path>` reads the parameters from a file (one JSON object, at most `MAX_OWNER_LINE` bytes). A failure
//! writes one error to stderr (with `--json`, one object `{"error":{"code":..,"message":..}}`) and exits non-zero:
//! 1 when the command or the store failed, 2 when the arguments were wrong or `<dir>` holds no store.
//!
//! `inboxcraft-cli mcp` runs the read-only MCP server ([`inboxcraft_mcp::Server`]) over stdin and stdout until stdin
//! ends (`docs/mcp.md`): bridged to the running app, or with `--store <dir>` on the store.
//!
//! The CLI never prompts, and only `mcp` reads stdin. It does no network I/O itself: on a store, a job a command asks
//! for (fetching a body, checking an account) is not run; in the app, the app runs it. It never creates a store, so a
//! mistyped directory is an error, not a new empty mailbox.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod remote;
mod text;

use std::ffi::OsString;
use std::fmt::Write as _;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use inboxcraft_control_key::Failure as Refused;
use inboxcraft_control_key::line::MAX_OWNER_LINE;
use inboxcraft_engine::{Caller, EngineError, Registry, Session};
use inboxcraft_mcp::Server;
use inboxcraft_store::{DATABASE_FILE, Sqlite, StoreConfig};
use inboxcraft_time::SystemClock;
use serde_json::{Value, json};

use remote::Remote;
pub use remote::{ASK_PATIENCE, PATIENCE, Reach, os_token};

/// The exit code of a command or a store that failed.
const FAILED: u8 = 1;

/// The exit code of wrong arguments or a directory without a store.
const USAGE: u8 = 2;

/// The most arguments read; more is a usage error.
const MAX_ARGS: usize = 16;

/// The longest part of an argument an error repeats, in characters.
const MAX_ECHO: usize = 160;

const HELP: &str = "\
Usage:
  inboxcraft-cli [--store <dir>] [--json] [--ask-owner] <command> [<params> | --params-file <path>]
  inboxcraft-cli [--json] commands
  inboxcraft-cli [--store <dir>] mcp
  inboxcraft-cli --version
  inboxcraft-cli --help

Runs one command by id (`mail.list`) as an agent: only safe commands run. Without --store it
runs in the running InboxCraft, through its control channel; with --store, on the store in <dir>.
<params> is a JSON object, `{}` when left out. `commands` lists every command with its class.
`mcp` serves the read-only MCP tools over stdin and stdout until stdin ends.
--json writes the answer, or the error, as one JSON object on one line.
--ask-owner asks the owner in the running InboxCraft to allow an owner command once.
--params-file <path> reads <params> from a file: one JSON object.
Exit codes: 0 done, 1 the command, the store or the app failed, 2 wrong arguments or no store in <dir>.
";

/// Why the CLI stops: a stable snake_case code, an English sentence, and the exit code.
#[derive(Debug)]
struct Failure {
    code: String,
    message: String,
    exit: u8,
}

impl Failure {
    fn usage(message: impl Into<String>) -> Self {
        Self { code: "usage".to_owned(), message: message.into(), exit: USAGE }
    }

    /// The app's refusal, or why it could not be reached.
    fn remote(refused: Refused) -> Self {
        let mut message = refused.message;
        if refused.code == "not_allowed" {
            message.push_str("; only safe commands run for an agent: ask the owner with --ask-owner");
        }
        Self { code: refused.code, message, exit: FAILED }
    }

    /// The app's refusal of a request that asked the owner: its reason stands as it is (a `not_allowed` there is an
    /// `owner` command the owner cannot be asked about, so `--ask-owner` is no way out).
    fn asked(refused: Refused) -> Self {
        Self { code: refused.code, message: refused.message, exit: FAILED }
    }

    /// Why a request that asks the owner could not reach the running app.
    fn unreached(refused: Refused) -> Self {
        if refused.code != "not_running" {
            return Self::remote(refused);
        }
        let mut message = refused.message;
        for store in ["; give --store <dir>", ", or give --store <dir>"] {
            if let Some(kept) = message.strip_suffix(store) {
                message = kept.to_owned();
            }
        }
        message.push_str("; an owner command needs the running InboxCraft to ask the owner");
        Self { code: refused.code, message, exit: FAILED }
    }

    /// The engine's refusal on a store; `ask` when the owner was to be asked, which a store cannot do.
    fn engine(error: &EngineError, ask: bool) -> Self {
        let mut message = error.to_string();
        if matches!(error, EngineError::NotAllowed { .. }) {
            message.push_str(if ask {
                "; the CLI runs headless here, and an owner command needs the running InboxCraft to ask the owner"
            } else {
                "; the CLI runs headless, so only safe commands run (the owner runs this one in the app)"
            });
        }
        Self { code: error.code().to_owned(), message, exit: FAILED }
    }
}

/// What the arguments ask for.
#[derive(Debug, PartialEq)]
enum Request {
    Version,
    Help,
    Commands,
    Run { store: Option<PathBuf>, id: String, params: Value, ask: bool },
    Mcp { store: Option<PathBuf> },
}

/// Runs the app with its command-line arguments (without the program name) and returns the process exit code; without
/// `--store`, in the user's running app ([`Reach::of_user`]).
///
/// The answer goes to `out`; a failure writes one error to `err` and nothing to `out`. A failed write is exit code 1.
/// Only `mcp` reads `input`, its requests, until it ends.
pub fn run_with_input(args: impl IntoIterator<Item = OsString>, input: &mut dyn BufRead, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    run_in(args, input, out, err, &Reach::of_user())
}

/// [`run_with_input`], reaching the running app through `reach`.
pub fn run_in(args: impl IntoIterator<Item = OsString>, input: &mut dyn BufRead, out: &mut dyn Write, err: &mut dyn Write, reach: &Reach) -> u8 {
    let args: Vec<OsString> = args.into_iter().take(MAX_ARGS.saturating_add(1)).collect();
    let json = args.iter().take_while(|arg| *arg != "--").any(|arg| arg == "--json");
    let answer = parse(&args).and_then(|request| answer(request, json, input, out, reach));
    let failure = match answer {
        Ok(text) => match out.write_all(text.as_bytes()).and_then(|()| out.flush()) {
            Ok(()) => return 0,
            Err(error) => Failure { code: "output".to_owned(), message: format!("the answer could not be written: {error}"), exit: FAILED },
        },
        Err(failure) => failure,
    };
    let shown = if json {
        json!({ "error": { "code": failure.code, "message": failure.message } }).to_string()
    } else {
        format!("inboxcraft-cli: {}", failure.message)
    };
    // The exit code reports the failure; a closed stderr leaves nothing else to report on.
    let _ = writeln!(err, "{shown}");
    failure.exit
}

/// [`run_with_input`] with an empty input: `mcp` ends at once, every other command reads none.
pub fn run(args: impl IntoIterator<Item = OsString>, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    run_with_input(args, &mut io::empty(), out, err)
}

/// The request the arguments make.
fn parse(args: &[OsString]) -> Result<Request, Failure> {
    if args.len() > MAX_ARGS {
        return Err(Failure::usage(format!("more than {MAX_ARGS} arguments")));
    }
    let mut store: Option<PathBuf> = None;
    let mut params_file: Option<&str> = None;
    let (mut version, mut help, mut options, mut ask) = (false, false, true, false);
    let mut words: Vec<&str> = Vec::new();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        let arg = arg.to_str().ok_or_else(|| Failure::usage("an argument is not UTF-8"))?;
        if !options || !arg.starts_with('-') || arg == "-" {
            words.push(arg);
            continue;
        }
        match arg {
            "--" => options = false,
            "--json" => {}
            "--version" | "-V" => version = true,
            "--help" | "-h" => help = true,
            "--ask-owner" => ask = true,
            _ if arg == "--params-file" || arg.starts_with("--params-file=") => {
                let path = match arg.strip_prefix("--params-file=") {
                    Some(path) => path,
                    None => args.next().and_then(|path| path.to_str()).ok_or_else(|| Failure::usage("--params-file needs a file"))?,
                };
                if path.is_empty() || params_file.replace(path).is_some() {
                    return Err(Failure::usage("give --params-file one file, once"));
                }
            }
            _ => {
                let dir = match arg.strip_prefix("--store").map(|rest| rest.strip_prefix('=')) {
                    Some(Some(dir)) => dir,
                    Some(None) if arg == "--store" => {
                        args.next().and_then(|dir| dir.to_str()).ok_or_else(|| Failure::usage("--store needs a directory"))?
                    }
                    _ => return Err(Failure::usage(format!("there is no option {}", echo(arg)))),
                };
                if dir.is_empty() || store.replace(PathBuf::from(dir)).is_some() {
                    return Err(Failure::usage("give --store one directory, once"));
                }
            }
        }
    }
    if help || version {
        return match (help, version, words.is_empty() && store.is_none() && !ask && params_file.is_none()) {
            (true, _, true) => Ok(Request::Help),
            (false, true, true) => Ok(Request::Version),
            _ => Err(Failure::usage("--help and --version stand alone")),
        };
    }
    let run_only = ask || params_file.is_some();
    match words.as_slice() {
        ["commands" | "mcp", ..] if run_only => Err(Failure::usage("--ask-owner and --params-file go with a command id")),
        [_, _] if params_file.is_some() => Err(Failure::usage("give the parameters inline or with --params-file, not both")),
        [] => Err(Failure::usage("give a command id, or `commands` for the list (see --help)")),
        ["commands"] => Ok(Request::Commands),
        ["mcp"] => Ok(Request::Mcp { store }),
        ["mcp", ..] => Err(Failure::usage("mcp takes no parameters: the tools take theirs from the client")),
        [id, rest @ ..] if rest.len() <= 1 => {
            let read = |text: &str| {
                serde_json::from_str::<Value>(text)
                    .map_err(|error| Failure::usage(format!("the parameters are not JSON: {}", echo(&error.to_string()))))
            };
            let params = match (rest.first(), params_file) {
                (Some(text), _) => read(text)?,
                (None, Some(path)) => read(&params_text(Path::new(path))?)?,
                (None, None) => json!({}),
            };
            if !params.is_object() {
                return Err(Failure::usage("the parameters are one JSON object"));
            }
            Ok(Request::Run { store, id: (*id).to_owned(), params, ask })
        }
        _ => Err(Failure::usage("give one command id and at most one JSON object of parameters")),
    }
}

/// The text of the parameters file `path`, at most [`MAX_OWNER_LINE`] bytes.
fn params_text(path: &Path) -> Result<String, Failure> {
    use std::io::Read as _;
    let shown = || echo(&path.display().to_string());
    let unread = |error: io::Error| Failure::usage(format!("the parameters file {} cannot be read: {error}", shown()));
    let file = std::fs::File::open(path).map_err(unread)?;
    let cap = u64::try_from(MAX_OWNER_LINE).unwrap_or(u64::MAX).saturating_add(1);
    let mut bytes = Vec::new();
    file.take(cap).read_to_end(&mut bytes).map_err(unread)?;
    if bytes.len() > MAX_OWNER_LINE {
        return Err(Failure::usage(format!("the parameters file {} is over {MAX_OWNER_LINE} bytes", shown())));
    }
    String::from_utf8(bytes).map_err(|_| Failure::usage(format!("the parameters file {} is not UTF-8", shown())))
}

/// A session on the store in `store`, which must hold one: the CLI never creates a store.
fn open(store: &Path) -> Result<Session, Failure> {
    if !store.join(DATABASE_FILE).is_file() {
        return Err(Failure {
            code: "no_store".to_owned(),
            message: format!("there is no store in {}", echo(&store.display().to_string())),
            exit: USAGE,
        });
    }
    let opened = Sqlite::open(store, StoreConfig::default()).map_err(|error| Failure::engine(&error.into(), false))?;
    Session::new(Box::new(opened), Box::new(SystemClock::new())).map_err(|error| Failure::engine(&error, false))
}

/// The text to write for `request`. `mcp` writes its answers to `out` itself, one a line, as it reads `input`.
fn answer(request: Request, json: bool, input: &mut dyn BufRead, out: &mut dyn Write, reach: &Reach) -> Result<String, Failure> {
    let value = match request {
        Request::Version => return Ok(format!("{} {}\n", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))),
        Request::Help => return Ok(HELP.to_owned()),
        Request::Commands if !json => return commands_text(),
        Request::Commands => commands()?,
        Request::Run { store: Some(store), id, params, ask } => {
            open(&store)?.run_as(Caller::Agent, &id, &params).map_err(|error| Failure::engine(&error, ask))?
        }
        Request::Run { store: None, id, params, ask: false } => Remote::find(reach).and_then(|app| app.run(&id, &params)).map_err(Failure::remote)?,
        Request::Run { store: None, id, params, ask: true } => {
            Remote::find(reach).map_err(Failure::unreached)?.ask(&id, &params).map_err(Failure::asked)?
        }
        Request::Mcp { store } => {
            let mut server = match store {
                Some(store) => Server::new(open(&store)?),
                None => Server::over(Box::new(Remote::find(reach).map_err(Failure::remote)?)),
            };
            server.serve(input, out).map_err(|error| Failure {
                code: "io".to_owned(),
                message: format!("the MCP connection failed: {error}"),
                exit: FAILED,
            })?;
            return Ok(String::new());
        }
    };
    Ok(if json { format!("{value}\n") } else { text::render(&value) })
}

/// Every command with its class and summary.
fn commands() -> Result<Value, Failure> {
    let registry = Registry::builtin().map_err(|error| Failure::engine(&error, false))?;
    let list: Vec<Value> =
        registry.specs().map(|spec| json!({ "id": spec.id, "class": spec.class.map(|class| class.name()), "summary": spec.summary })).collect();
    Ok(json!({ "commands": list }))
}

/// Every command with its class and summary, one a line in columns.
fn commands_text() -> Result<String, Failure> {
    let registry = Registry::builtin().map_err(|error| Failure::engine(&error, false))?;
    let width = registry.specs().map(|spec| spec.id.len()).max().unwrap_or(0);
    let mut listing = String::new();
    for spec in registry.specs() {
        let class = spec.class.map_or("", |class| class.name());
        let _ = writeln!(listing, "{:width$}  {class:5}  {}", spec.id, spec.summary);
    }
    Ok(listing)
}

/// `text` cut to [`MAX_ECHO`] characters, without control characters, quoted.
fn echo(text: &str) -> String {
    format!("{:?}", text.chars().filter(|c| !c.is_control()).take(MAX_ECHO).collect::<String>())
}

#[cfg(test)]
mod tests;
