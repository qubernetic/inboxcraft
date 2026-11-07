// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The command-line client: every command by id, headless, on a local store (`docs/cli.md`).
//!
//! `inboxcraft-cli --store <dir> [--json] <command> [<params>]` opens the store in `<dir>`, runs the command through
//! the engine as an agent ([`Caller::Agent`]) and writes its answer: as text, one field a line, or with `--json` as
//! one JSON object on one line (snake_case keys, ISO 8601 dates in UTC, fields only ever added). Only `safe` commands
//! run: an `owner` or `never` command is refused with a plain error, since nobody is there to approve it. A failure
//! writes one error to stderr (with `--json`, one object `{"error":{"code":..,"message":..}}`) and exits non-zero:
//! 1 when the command or the store failed, 2 when the arguments were wrong or `<dir>` holds no store.
//!
//! The CLI never prompts and never reads stdin. It does no network I/O: a job a command asks for (fetching a body,
//! checking an account) is not run. It never creates a store, so a mistyped directory is an error, not a new empty
//! mailbox.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

mod text;

use std::ffi::OsString;
use std::fmt::Write as _;
use std::io::Write;
use std::path::PathBuf;

use inboxcraft_engine::{Caller, EngineError, Registry, Session};
use inboxcraft_store::{DATABASE_FILE, Sqlite, StoreConfig};
use inboxcraft_time::SystemClock;
use serde_json::{Value, json};

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
  inboxcraft-cli --store <dir> [--json] <command> [<params>]
  inboxcraft-cli [--json] commands
  inboxcraft-cli --version
  inboxcraft-cli --help

Runs one command by id (`mail.list`) on the store in <dir>, as an agent: only safe commands run.
<params> is a JSON object, `{}` when left out. `commands` lists every command with its class.
--json writes the answer, or the error, as one JSON object on one line.
Exit codes: 0 done, 1 the command or the store failed, 2 wrong arguments or no store in <dir>.
";

/// Why the CLI stops: a stable snake_case code, an English sentence, and the exit code.
#[derive(Debug)]
struct Failure {
    code: &'static str,
    message: String,
    exit: u8,
}

impl Failure {
    fn usage(message: impl Into<String>) -> Self {
        Self { code: "usage", message: message.into(), exit: USAGE }
    }

    fn engine(error: &EngineError) -> Self {
        let mut message = error.to_string();
        if matches!(error, EngineError::NotAllowed { .. }) {
            message.push_str("; the CLI runs headless, so only safe commands run (the owner runs this one in the app)");
        }
        Self { code: error.code(), message, exit: FAILED }
    }
}

/// What the arguments ask for.
#[derive(Debug, PartialEq)]
enum Request {
    Version,
    Help,
    Commands,
    Run { store: PathBuf, id: String, params: Value },
}

/// Runs the app with its command-line arguments (without the program name) and returns the process exit code.
///
/// The answer goes to `out`; a failure writes one error to `err` and nothing to `out`. A failed write is exit code 1.
pub fn run(args: impl IntoIterator<Item = OsString>, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let args: Vec<OsString> = args.into_iter().take(MAX_ARGS.saturating_add(1)).collect();
    let json = args.iter().take_while(|arg| *arg != "--").any(|arg| arg == "--json");
    let answer = parse(&args).and_then(|request| answer(request, json));
    let failure = match answer {
        Ok(text) => match out.write_all(text.as_bytes()).and_then(|()| out.flush()) {
            Ok(()) => return 0,
            Err(error) => Failure { code: "output", message: format!("the answer could not be written: {error}"), exit: FAILED },
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

/// The request the arguments make.
fn parse(args: &[OsString]) -> Result<Request, Failure> {
    if args.len() > MAX_ARGS {
        return Err(Failure::usage(format!("more than {MAX_ARGS} arguments")));
    }
    let mut store: Option<PathBuf> = None;
    let (mut version, mut help, mut options) = (false, false, true);
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
        return match (help, version, words.is_empty() && store.is_none()) {
            (true, _, true) => Ok(Request::Help),
            (false, true, true) => Ok(Request::Version),
            _ => Err(Failure::usage("--help and --version stand alone")),
        };
    }
    match words.as_slice() {
        [] => Err(Failure::usage("give a command id, or `commands` for the list (see --help)")),
        ["commands"] => Ok(Request::Commands),
        [id, rest @ ..] if rest.len() <= 1 => {
            let store = store
                .ok_or_else(|| Failure::usage("give --store <dir>: the CLI runs on a local store (talking to the running app is not built yet)"))?;
            let params = match rest.first() {
                Some(text) => serde_json::from_str::<Value>(text)
                    .map_err(|error| Failure::usage(format!("the parameters are not JSON: {}", echo(&error.to_string()))))?,
                None => json!({}),
            };
            if !params.is_object() {
                return Err(Failure::usage("the parameters are one JSON object"));
            }
            Ok(Request::Run { store, id: (*id).to_owned(), params })
        }
        _ => Err(Failure::usage("give one command id and at most one JSON object of parameters")),
    }
}

/// The text to write for `request`.
fn answer(request: Request, json: bool) -> Result<String, Failure> {
    let value = match request {
        Request::Version => return Ok(format!("{} {}\n", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))),
        Request::Help => return Ok(HELP.to_owned()),
        Request::Commands if !json => return commands_text(),
        Request::Commands => commands()?,
        Request::Run { store, id, params } => {
            if !store.join(DATABASE_FILE).is_file() {
                return Err(Failure {
                    code: "no_store",
                    message: format!("there is no store in {}", echo(&store.display().to_string())),
                    exit: USAGE,
                });
            }
            let opened = Sqlite::open(&store, StoreConfig::default()).map_err(|error| Failure::engine(&error.into()))?;
            let mut session = Session::new(Box::new(opened), Box::new(SystemClock::new())).map_err(|error| Failure::engine(&error))?;
            session.run_as(Caller::Agent, &id, &params).map_err(|error| Failure::engine(&error))?
        }
    };
    Ok(if json { format!("{value}\n") } else { text::render(&value) })
}

/// Every command with its class and summary.
fn commands() -> Result<Value, Failure> {
    let registry = Registry::builtin().map_err(|error| Failure::engine(&error))?;
    let list: Vec<Value> =
        registry.specs().map(|spec| json!({ "id": spec.id, "class": spec.class.map(|class| class.name()), "summary": spec.summary })).collect();
    Ok(json!({ "commands": list }))
}

/// Every command with its class and summary, one a line in columns.
fn commands_text() -> Result<String, Failure> {
    let registry = Registry::builtin().map_err(|error| Failure::engine(&error))?;
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
