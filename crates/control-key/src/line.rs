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

/// The request in `line` (with or without its line end).
///
/// # Errors
///
/// `too_long` for a line over [`MAX_LINE`]; `bad_line` for one that is not UTF-8, not one JSON object, names no
/// command or one longer than [`MAX_COMMAND`], or has params that are not an object. A missing or unreadable token is
/// not refused here: the request carries `None`, which the app refuses.
pub fn read_request(line: &[u8]) -> Result<Request, Failure> {
    if line.len() > MAX_LINE {
        return Err(Failure::new("too_long", format!("a line is at most {MAX_LINE} bytes")));
    }
    let fields = object(line).map_err(Failure::bad_line)?;
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
    Ok(Request { token, command, params })
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
