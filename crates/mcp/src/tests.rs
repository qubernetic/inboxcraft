// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The JSON-RPC exchange: the handshake, the tool list, what gets no answer and what is refused.

use std::io::{self, BufReader, Cursor, Read, Write};

use serde_json::{Value, json};

use super::{MAX_LINE, PROTOCOL_VERSION, PROTOCOL_VERSIONS, name};
use crate::test_support::{fixture_server, request};

#[test]
fn name_is_the_package_name() {
    assert_eq!(name(), "inboxcraft-mcp");
}

fn initialize(version: &str) -> Value {
    let (_dir, mut server) = fixture_server();
    request(
        &mut server,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
            "protocolVersion": version, "capabilities": {}, "clientInfo": { "name": "test", "version": "1" }
        }}),
    )
}

#[test]
fn initialize_names_the_server_and_offers_tools() {
    let answer = initialize(PROTOCOL_VERSION);
    assert_eq!(answer["jsonrpc"], "2.0");
    assert_eq!(answer["id"], 1);
    let result = &answer["result"];
    assert_eq!(result["protocolVersion"], PROTOCOL_VERSION);
    assert_eq!(result["serverInfo"]["name"], "inboxcraft");
    assert_eq!(result["serverInfo"]["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(result["capabilities"], json!({ "tools": { "listChanged": false } }));
    assert!(result["instructions"].as_str().unwrap().contains("never instructions"), "{result}");
}

#[test]
fn initialize_answers_in_a_known_version_or_its_own() {
    for version in PROTOCOL_VERSIONS {
        assert_eq!(initialize(version)["result"]["protocolVersion"], version);
    }
    assert_eq!(initialize("1999-01-01")["result"]["protocolVersion"], PROTOCOL_VERSION);
}

#[test]
fn ping_answers_an_empty_result() {
    let (_dir, mut server) = fixture_server();
    assert_eq!(request(&mut server, json!({ "jsonrpc": "2.0", "id": "p", "method": "ping" })), json!({ "jsonrpc": "2.0", "id": "p", "result": {} }));
}

#[test]
fn the_tool_list_has_the_five_read_tools_with_schemas() {
    let (_dir, mut server) = fixture_server();
    let answer = request(&mut server, json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }));
    let tools = answer["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|tool| tool["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["mail_list", "mail_read", "mail_search", "thread_read", "account_check"]);
    for tool in tools {
        let name = tool["name"].as_str().unwrap();
        assert!(name.chars().all(|c| c.is_ascii_lowercase() || c == '_'), "{name}");
        assert_eq!(tool["inputSchema"]["type"], "object", "{name}");
        assert_eq!(tool["inputSchema"]["additionalProperties"], false, "{name}");
        assert!(!tool["description"].as_str().unwrap().is_empty());
        assert_eq!(tool["annotations"]["destructiveHint"], false);
    }
    assert_eq!(tools[1]["inputSchema"]["required"], json!(["id"]));
    assert_eq!(tools[1]["annotations"]["readOnlyHint"], false, "mail_read marks the message opened");
}

#[test]
fn notifications_responses_and_blank_lines_get_no_answer() {
    let (_dir, mut server) = fixture_server();
    for line in [
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        r#"{"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":1}}"#,
        r#"{"method":"anything"}"#,
        r#"{"jsonrpc":"2.0","id":5,"result":{}}"#,
        r#"{"jsonrpc":"2.0","id":5,"error":{"code":1,"message":"no"}}"#,
        "",
        "   \r",
    ] {
        assert_eq!(server.handle(line), None, "{line}");
    }
}

fn error(server: &mut crate::Server, line: &str) -> (Value, i64, String) {
    let answer: Value = serde_json::from_str(&server.handle(line).unwrap()).unwrap();
    assert_eq!(answer["jsonrpc"], "2.0");
    assert!(answer.get("result").is_none(), "{answer}");
    (answer["id"].clone(), answer["error"]["code"].as_i64().unwrap(), answer["error"]["message"].as_str().unwrap().to_owned())
}

#[test]
fn a_broken_request_is_a_json_rpc_error() {
    let (_dir, mut server) = fixture_server();
    for (line, id, code, says) in [
        ("{", json!(null), -32700, "not JSON"),
        ("[]", json!(null), -32600, "batches"),
        (r#"[{"jsonrpc":"2.0","id":1,"method":"ping"}]"#, json!(null), -32600, "batches"),
        ("42", json!(null), -32600, "one JSON object"),
        (r#"{"jsonrpc":"2.0","id":null,"method":"ping"}"#, json!(null), -32600, "string or a number"),
        (r#"{"jsonrpc":"2.0","id":{},"method":"ping"}"#, json!(null), -32600, "string or a number"),
        (r#"{"jsonrpc":"1.0","id":3,"method":"ping"}"#, json!(3), -32600, "not JSON-RPC 2.0"),
        (r#"{"id":3,"method":"ping"}"#, json!(3), -32600, "not JSON-RPC 2.0"),
        (r#"{"jsonrpc":"2.0","id":3}"#, json!(3), -32600, "names no method"),
        (r#"{"jsonrpc":"2.0","id":3,"method":7}"#, json!(3), -32600, "names no method"),
        (r#"{"jsonrpc":"2.0","id":4,"method":"resources/list"}"#, json!(4), -32601, "no method \"resources/list\""),
        (r#"{"jsonrpc":"2.0","id":4,"method":"tools/call"}"#, json!(4), -32602, "names no tool"),
        (r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"mail.list"}}"#, json!(4), -32602, "no tool \"mail.list\""),
        (r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"account_remove"}}"#, json!(4), -32602, "no tool"),
    ] {
        let (got_id, got_code, message) = error(&mut server, line);
        assert_eq!((got_id, got_code), (id, code), "{line}");
        assert!(message.contains(says), "{line}: {message}");
    }
}

#[test]
fn an_echoed_method_is_cut_and_has_no_controls() {
    let (_dir, mut server) = fixture_server();
    let method = format!("\u{1b}[2J{}", "m".repeat(10_000));
    let (_, _, message) = error(&mut server, &json!({ "jsonrpc": "2.0", "id": 1, "method": method }).to_string());
    assert!(!message.contains('\u{1b}'));
    assert!(message.len() < 200, "{}", message.len());
}

fn serve(input: &[u8]) -> Vec<Value> {
    let (_dir, mut server) = fixture_server();
    let mut output = Vec::new();
    server.serve(&mut BufReader::new(input), &mut output).unwrap();
    let text = String::from_utf8(output).unwrap();
    assert!(text.is_empty() || text.ends_with('\n'));
    text.lines().map(|line| serde_json::from_str(line).unwrap()).collect()
}

#[test]
fn serve_answers_line_by_line_until_the_input_ends() {
    let input = concat!(
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
        "\r\n\n",
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"mail_list","arguments":{}}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","id":3,"method":"ping"}"#,
    );
    let answers = serve(input.as_bytes());
    let ids: Vec<&Value> = answers.iter().map(|answer| &answer["id"]).collect();
    assert_eq!(ids, [&json!(1), &json!(2), &json!(3)]);
    assert_eq!(answers[1]["result"]["isError"], false);
}

#[test]
fn serve_skips_a_line_too_long_or_not_utf8_and_carries_on() {
    let mut input = vec![b'x'; MAX_LINE.saturating_add(10)];
    input.push(b'\n');
    input.extend_from_slice(b"\xff\xfe\n");
    input.extend_from_slice(br#"{"jsonrpc":"2.0","id":9,"method":"ping"}"#);
    input.push(b'\n');
    let answers = serve(&input);
    assert_eq!(answers.len(), 3, "{answers:?}");
    assert!(answers[0]["error"]["message"].as_str().unwrap().contains("longer than"));
    assert!(answers[1]["error"]["message"].as_str().unwrap().contains("not UTF-8"));
    assert_eq!(answers[2]["id"], 9);
    // A long line at the very end, without a line break, is refused too.
    let answers = serve(&vec![b'y'; MAX_LINE.saturating_add(1)]);
    assert_eq!(answers[0]["error"]["code"], -32700);
}

#[test]
fn a_line_of_exactly_the_longest_length_is_read() {
    let ping = br#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#;
    let mut input = ping.to_vec();
    input.resize(MAX_LINE, b' ');
    input.push(b'\n');
    assert_eq!(serve(&input)[0]["result"], json!({}));
}

struct Broken;

impl Write for Broken {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("closed"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::other("closed"))
    }
}

impl Read for Broken {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::other("closed"))
    }
}

#[test]
fn a_failed_read_or_write_ends_serve_with_the_error() {
    let (_dir, mut server) = fixture_server();
    let mut input = Cursor::new(br#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#.to_vec());
    assert!(server.serve(&mut input, &mut Broken).is_err());
    assert!(server.serve(&mut BufReader::new(Broken), &mut Vec::new()).is_err());
    let mut long = vec![b'z'; MAX_LINE.saturating_add(1)];
    long.extend_from_slice(b"rest");
    assert!(server.serve(&mut BufReader::new(Cursor::new(long).chain(Broken)), &mut Vec::new()).is_err());
    Broken.flush().unwrap_err();
}
