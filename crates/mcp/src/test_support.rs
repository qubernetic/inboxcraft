// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A server on the `testkit` fixture store, and the calls the tests make to it.

use inboxcraft_engine::Session;
use inboxcraft_store::{Sqlite, StoreConfig};
use inboxcraft_testkit::store;
use inboxcraft_time::FakeClock;
use serde_json::{Value, json};
use tempfile::TempDir;

use crate::Server;

/// A server on a fresh fixture store, and the directory that holds the store.
pub(crate) fn fixture_server() -> (TempDir, Server) {
    let dir = tempfile::tempdir().unwrap();
    store::fixture(dir.path()).unwrap();
    let opened = Sqlite::open(dir.path(), StoreConfig::default()).unwrap();
    let session = Session::new(Box::new(opened), Box::new(FakeClock::new(store::NOW))).unwrap();
    (dir, Server::new(session))
}

/// The answer to one request, parsed.
pub(crate) fn request(server: &mut Server, request: Value) -> Value {
    let line = server.handle(&request.to_string()).unwrap();
    assert!(!line.contains('\n'), "one line: {line}");
    serde_json::from_str(&line).unwrap()
}

/// The result of a `tools/call` of `tool` with `arguments`.
pub(crate) fn call(server: &mut Server, tool: &str, arguments: Value) -> Value {
    let answer = request(server, json!({ "jsonrpc": "2.0", "id": 7, "method": "tools/call", "params": { "name": tool, "arguments": arguments } }));
    assert_eq!(answer["id"], 7, "{answer}");
    answer["result"].clone()
}

/// The text of a tool result, with whether it is an error.
pub(crate) fn text(result: &Value) -> (String, bool) {
    let content = result["content"].as_array().unwrap();
    assert_eq!(content.len(), 1, "{result}");
    assert_eq!(content[0]["type"], "text");
    (content[0]["text"].as_str().unwrap().to_owned(), result["isError"].as_bool().unwrap())
}
