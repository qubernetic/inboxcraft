// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The server over any [`Commands`]: the running app, bridged through the control channel.

use std::sync::{Arc, Mutex};

use inboxcraft_control_key::Failure;
use serde_json::{Value, json};

use crate::test_support::{call, request, text};
use crate::{Commands, Server};

/// The commands that ran, with their params.
type Ran = Arc<Mutex<Vec<(String, Value)>>>;

/// Commands that answer what they were given and remember what ran.
struct Scripted {
    answer: Result<Value, Failure>,
    ran: Ran,
}

impl Commands for Scripted {
    fn run(&mut self, id: &str, params: &Value) -> Result<Value, Failure> {
        self.ran.lock().unwrap().push((id.to_owned(), params.clone()));
        self.answer.clone()
    }
}

fn bridged(answer: Result<Value, Failure>) -> (Server, Ran) {
    let ran = Arc::default();
    (Server::over(Box::new(Scripted { answer, ran: Arc::clone(&ran) })), ran)
}

#[test]
fn a_tool_runs_its_command_on_the_commands_given() {
    let answer = json!({ "accounts": [{ "id": 1, "name": "Home", "address": "anna@example.org", "state": "ok" }] });
    let (mut server, ran) = bridged(Ok(answer));
    let (_out, error) = text(&call(&mut server, "account_check", json!({ "account": 1 })));
    assert!(!error);
    assert_eq!(ran.lock().unwrap().as_slice(), [("account.check".to_owned(), json!({ "account": 1 }))]);
}

#[test]
fn a_refusal_of_the_app_is_a_tool_error_with_its_code() {
    let (mut server, _ran) = bridged(Err(Failure::new("unauthorized", "the control token is wrong or revoked")));
    let (out, error) = text(&call(&mut server, "mail_list", json!({})));
    assert!(error);
    assert_eq!(out, "error unauthorized: the control token is wrong or revoked");
}

#[test]
fn a_bridged_server_says_what_it_serves() {
    let (mut server, _ran) = bridged(Ok(json!({})));
    let answer = request(&mut server, json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} }));
    let instructions = answer["result"]["instructions"].as_str().unwrap();
    assert!(instructions.contains("read-only"), "{instructions}");
    assert!(!instructions.contains("local store"), "{instructions}");
}
