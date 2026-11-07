// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The tools: each a `safe` command, its input schema, and how its answer is written.
//!
//! A tool's name is its command id with an underscore for the dot (`mail_list` runs `mail.list`), since the model
//! APIs the clients hand tools to take letters, digits, `_` and `-` only; `thread_read` runs `mail.thread`.

use inboxcraft_engine::{Caller, MAX_LIST, Session};
use serde_json::{Value, json};

use crate::render;
use crate::{RpcError, code, echo};

/// How many messages a list or a search gives when the agent does not say: a page an agent reads cheaply.
pub(crate) const DEFAULT_LIMIT: u64 = 25;

/// One tool.
pub(crate) struct Tool {
    /// The name the client calls it by.
    pub(crate) name: &'static str,
    /// The command it runs.
    pub(crate) command: &'static str,
    /// What it does, for the model.
    description: &'static str,
    /// Its arguments as a JSON Schema.
    pub(crate) schema: fn() -> Value,
    /// Whether it leaves the mailbox as it was (`mail_read` marks the message opened).
    read_only: bool,
    /// Whether a missing `limit` becomes [`DEFAULT_LIMIT`].
    paged: bool,
    /// Writes the command's answer as the result text.
    render: fn(&Value, &Value) -> String,
}

/// Every tool, in the order `tools/list` gives them.
pub(crate) const TOOLS: [Tool; 5] = [
    Tool {
        name: "mail_list",
        command: "mail.list",
        description: "List the messages of a folder, or of one role across every account (the inbox when neither is given), newest \
first, one line each: id, date, from, subject, flags.",
        schema: list_schema,
        read_only: true,
        paged: true,
        render: render::list,
    },
    Tool {
        name: "mail_read",
        command: "mail.read",
        description: "Read one message by id: its headers, its attachments and its plain text, quoted parts folded and the length \
capped. Marks the message opened on this computer; changes nothing on the server.",
        schema: id_schema,
        read_only: false,
        paged: false,
        render: render::read,
    },
    Tool {
        name: "mail_search",
        command: "mail.search",
        description: "Search the mail kept on this computer by sender, recipient, subject, body words and date, newest first, one \
line a message.",
        schema: search_schema,
        read_only: true,
        paged: true,
        render: render::search,
    },
    Tool {
        name: "thread_read",
        command: "mail.thread",
        description: "List the thread a message is in, oldest first, across folders and accounts, one line a message.",
        schema: id_schema,
        read_only: true,
        paged: false,
        render: render::thread,
    },
    Tool {
        name: "account_check",
        command: "account.check",
        description: "Ask for a check of an account, step by step (DNS, TCP, TLS, certificate, sign-in), and give the steps as \
they stand.",
        schema: account_schema,
        read_only: true,
        paged: false,
        render: render::check,
    },
];

/// The roles `mail_list` takes.
pub(crate) const ROLES: [&str; 11] = ["inbox", "drafts", "sent", "archive", "junk", "trash", "all", "flagged", "screener", "snoozed", "blocked"];

fn id(description: &str) -> Value {
    json!({ "type": "integer", "minimum": 0, "description": description })
}

fn limit() -> Value {
    json!({ "type": "integer", "minimum": 1, "maximum": MAX_LIST, "description": format!("At most this many; {DEFAULT_LIMIT} when not given.") })
}

fn list_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "folder": id("One folder, by id."),
            "role": { "type": "string", "enum": ROLES, "description": "One role across the accounts; not with folder." },
            "account": id("Only this account."),
            "limit": limit(),
            "offset": { "type": "integer", "minimum": 0, "description": "Skip this many first, for the next page." },
        },
        "additionalProperties": false,
    })
}

fn id_schema() -> Value {
    json!({ "type": "object", "properties": { "id": id("The message.") }, "required": ["id"], "additionalProperties": false })
}

fn search_schema() -> Value {
    let text = |description: &str| json!({ "type": "string", "description": description });
    json!({
        "type": "object",
        "properties": {
            "from": text("Sender names or addresses that contain this."),
            "to": text("To or Cc names or addresses that contain this."),
            "subject": text("Subjects that contain this."),
            "text": text("Bodies with these words in this order."),
            "since": text("Dated at or after this ISO 8601 date or time (2026-11-01)."),
            "before": text("Dated before this ISO 8601 date or time."),
            "account": id("Only this account."),
            "folder": id("Only this folder."),
            "limit": limit(),
        },
        "additionalProperties": false,
    })
}

fn account_schema() -> Value {
    json!({ "type": "object", "properties": { "account": id("The account.") }, "required": ["account"], "additionalProperties": false })
}

/// The tools as `tools/list` gives them.
pub(crate) fn list() -> Vec<Value> {
    TOOLS
        .iter()
        .map(|tool| {
            json!({
                "name": tool.name,
                "description": tool.description,
                "inputSchema": (tool.schema)(),
                "annotations": { "readOnlyHint": tool.read_only, "destructiveHint": false, "idempotentHint": true, "openWorldHint": false },
            })
        })
        .collect()
}

/// Runs `tools/call`. A tool that fails answers a result marked `isError`, which the model reads; only a call that
/// names no tool is a protocol error.
pub(crate) fn call(session: &mut Session, params: &Value) -> Result<Value, RpcError> {
    let name = params.get("name").and_then(Value::as_str).ok_or_else(|| RpcError::new(code::INVALID_PARAMS, "tools/call names no tool"))?;
    let tool =
        TOOLS.iter().find(|tool| tool.name == name).ok_or_else(|| RpcError::new(code::INVALID_PARAMS, format!("there is no tool {}", echo(name))))?;
    let mut arguments = match params.get("arguments") {
        None | Some(Value::Null) => json!({}),
        Some(arguments @ Value::Object(_)) => arguments.clone(),
        Some(_) => return Ok(failed("bad_params", "the arguments are one JSON object")),
    };
    if let Value::Object(fields) = &mut arguments
        && tool.paged
        && !fields.contains_key("limit")
    {
        fields.insert("limit".to_owned(), json!(DEFAULT_LIMIT));
    }
    Ok(match session.run_as(Caller::Agent, tool.command, &arguments) {
        Ok(answer) => json!({ "content": [{ "type": "text", "text": (tool.render)(&arguments, &answer) }], "isError": false }),
        Err(error) => failed(error.code(), &error.to_string()),
    })
}

/// A tool result that reports a failure: its code and one sentence.
fn failed(code: &str, message: &str) -> Value {
    json!({ "content": [{ "type": "text", "text": format!("error {code}: {message}") }], "isError": true })
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tests;
