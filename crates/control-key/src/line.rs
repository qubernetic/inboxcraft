// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The lines of the control channel (`docs/control-protocol.md`): one JSON object a line each way.
//!
//! A client writes `{"token":"<64 hex digits>","command":"mail.list","params":{...}}`; `params` may be left out. The
//! app answers `{"ok":<the command's answer>}` or `{"error":{"code":"...","message":"..."}}`. Everything a client
//! writes is hostile: [`read_request`] takes any bytes and never repeats them in a refusal.

use serde_json::{Map, Value, json};

use crate::Token;

/// The longest request line read, in bytes, line end included.
pub const MAX_LINE: usize = 1 << 20;

/// The longest request line read when it asks the owner (`"ask_owner": true`), in bytes, line end included: an owner
/// command such as `calendar.import` may carry a whole calendar.
pub const MAX_OWNER_LINE: usize = 8 << 20;

/// The most characters of a client's name kept.
pub const MAX_CLIENT: usize = 64;

/// The longest answer line a client reads, in bytes, line end included.
pub const MAX_ANSWER: usize = 16 << 20;

/// The longest command id read, in bytes.
pub const MAX_COMMAND: usize = 128;

/// One request, as a client wrote it.
#[derive(Debug)]
pub struct Request {
    /// The token the client showed; `None` when it showed none that reads as one.
    pub token: Option<Token>,
    /// The command id (`mail.list`).
    pub command: String,
    /// The command's parameters: one JSON object.
    pub params: Value,
    /// Whether the client asks the owner to allow an `owner` command once (`"ask_owner": true`).
    pub ask_owner: bool,
    /// The name the client gave itself, control and format characters left out and at most [`MAX_CLIENT`] characters; `None`
    /// when it gave none or nothing is left.
    pub client: Option<String>,
}

/// Why a line was refused, or a command failed: a stable snake_case code and one English sentence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    /// The code (`unauthorized`, `bad_line`, or an engine code such as `not_allowed`).
    pub code: String,
    /// The sentence.
    pub message: String,
}

impl Failure {
    /// A failure with `code` and `message`.
    #[must_use]
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self { code: code.into(), message: message.into() }
    }

    fn bad_line(message: &str) -> Self {
        Self::new("bad_line", message)
    }

    fn bad_answer(message: &str) -> Self {
        Self::new("bad_answer", message)
    }
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.message, self.code)
    }
}

impl std::error::Error for Failure {}

/// The line a client writes to run `command` with `params`, line end included.
#[must_use]
pub fn request_line(token: &Token, command: &str, params: &Value) -> String {
    format!("{}\n", json!({ "token": token.text(), "command": command, "params": params }))
}

/// The line a client writes to ask the owner to allow `command` with `params` once, naming itself `client`, line end
/// included.
#[must_use]
pub fn owner_request_line(token: &Token, command: &str, params: &Value, client: &str) -> String {
    let value = json!({ "token": token.text(), "command": command, "params": params, "ask_owner": true, "client": client });
    format!("{value}\n")
}

/// The request in `line` (with or without its line end).
///
/// # Errors
///
/// `too_long` for a line over [`MAX_OWNER_LINE`], or over [`MAX_LINE`] when it does not ask the owner; `bad_line` for
/// one that is not UTF-8, not one JSON object, names no command or one longer than [`MAX_COMMAND`], has params that
/// are not an object, an `ask_owner` that is not a boolean or a `client` that is not a string. A missing or unreadable
/// token is not refused here: the request carries `None`, which the app refuses.
pub fn read_request(line: &[u8]) -> Result<Request, Failure> {
    if line.len() > MAX_OWNER_LINE {
        return Err(too_long(MAX_OWNER_LINE));
    }
    let fields = match object(line) {
        Ok(fields) => fields,
        Err(_) if line.len() > MAX_LINE => return Err(too_long(MAX_LINE)),
        Err(why) => return Err(Failure::bad_line(why)),
    };
    let ask_owner = fields.get("ask_owner");
    if line.len() > MAX_LINE && ask_owner != Some(&Value::Bool(true)) {
        return Err(too_long(MAX_LINE));
    }
    let ask_owner = match ask_owner {
        None => false,
        Some(Value::Bool(ask)) => *ask,
        Some(_) => return Err(Failure::bad_line("ask_owner is true or false")),
    };
    let token = fields.get("token").and_then(Value::as_str).and_then(Token::parse);
    let command = match fields.get("command") {
        Some(Value::String(command)) if !command.is_empty() && command.len() <= MAX_COMMAND => command.clone(),
        Some(Value::String(_)) => return Err(Failure::bad_line(&format!("the command is 1 to {MAX_COMMAND} bytes"))),
        _ => return Err(Failure::bad_line("the line names no command")),
    };
    let params = match fields.get("params") {
        None | Some(Value::Null) => json!({}),
        Some(params @ Value::Object(_)) => params.clone(),
        Some(_) => return Err(Failure::bad_line("the params are one JSON object")),
    };
    let client = match fields.get("client") {
        None => None,
        Some(Value::String(name)) => client_name(name),
        Some(_) => return Err(Failure::bad_line("the client is a name")),
    };
    Ok(Request { token, command, params, ask_owner, client })
}

fn too_long(most: usize) -> Failure {
    Failure::new("too_long", format!("a line is at most {most} bytes"))
}

/// Whether `c` is left out of a client's name: every format character (general category `Cf` of Unicode 15.1), the
/// unassigned U+2065, and the line and paragraph separators. The window draws none of them
/// (`inboxcraft_ui_egui::component::hidden`, the same set; a test of the app pins the two).
fn hidden(c: char) -> bool {
    matches!(
        c,
        '\u{00AD}' | '\u{0600}'..='\u{0605}' | '\u{061C}' | '\u{06DD}' | '\u{070F}' | '\u{0890}'..='\u{0891}' | '\u{08E2}' | '\u{180E}'
            | '\u{200B}'..='\u{200F}' | '\u{2028}' | '\u{2029}' | '\u{202A}'..='\u{202E}' | '\u{2060}'..='\u{206F}' | '\u{FEFF}'
            | '\u{FFF9}'..='\u{FFFB}' | '\u{110BD}' | '\u{110CD}' | '\u{13430}'..='\u{1343F}' | '\u{1BCA0}'..='\u{1BCA3}'
            | '\u{1D173}'..='\u{1D17A}' | '\u{E0001}' | '\u{E0020}'..='\u{E007F}'
    )
}

/// `name` without its control characters and the characters that could turn round, hide or break the words it is
/// shown among ([`hidden`]), cut to [`MAX_CLIENT`] characters; `None` when nothing is left.
fn client_name(name: &str) -> Option<String> {
    let name: String = name.chars().filter(|c| !c.is_control() && !hidden(*c)).take(MAX_CLIENT).collect();
    let name = name.trim();
    (!name.is_empty()).then(|| name.to_owned())
}

/// The line the app answers with, line end included.
#[must_use]
pub fn answer_line(answer: &Result<Value, Failure>) -> String {
    let value = match answer {
        Ok(value) => json!({ "ok": value }),
        Err(failure) => json!({ "error": { "code": failure.code, "message": failure.message } }),
    };
    format!("{value}\n")
}

/// The answer in `line`: the command's answer, or its failure.
///
/// # Errors
///
/// The failure the app answered, or `bad_answer` for a line that is no answer.
pub fn read_answer(line: &[u8]) -> Result<Value, Failure> {
    let mut fields = object(line).map_err(Failure::bad_answer)?;
    match (fields.remove("ok"), fields.remove("error")) {
        (Some(value), None) => Ok(value),
        (None, Some(Value::Object(error))) => match error.get("code").and_then(Value::as_str) {
            Some(code) => Err(Failure::new(code, error.get("message").and_then(Value::as_str).unwrap_or_default())),
            None => Err(Failure::bad_answer("the error names no code")),
        },
        _ => Err(Failure::bad_answer("the answer is neither ok nor an error")),
    }
}

/// The one JSON object on `line`, which may end in a line break.
fn object(line: &[u8]) -> Result<Map<String, Value>, &'static str> {
    let line = line.strip_suffix(b"\n").unwrap_or(line);
    let line = line.strip_suffix(b"\r").unwrap_or(line);
    let text = std::str::from_utf8(line).map_err(|_| "the line is not UTF-8")?;
    match serde_json::from_str::<Value>(text) {
        Ok(Value::Object(fields)) => Ok(fields),
        Ok(_) => Err("the line is not one JSON object"),
        Err(_) => Err("the line is not JSON"),
    }
}

#[cfg(test)]
#[path = "line_tests.rs"]
mod tests;
