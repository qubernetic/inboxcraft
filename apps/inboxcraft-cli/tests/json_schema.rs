// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `--json` schema: one object a line, snake_case keys, dates in ISO 8601 UTC, and the fields an agent already
//! reads stay (fields are only ever added).

use std::ffi::OsString;

use inboxcraft_testkit::store;
use serde_json::Value;

fn cli(args: &[&str]) -> (u8, String, String) {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = inboxcraft_cli::run(args.iter().map(OsString::from), &mut out, &mut err);
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

fn snake_case(key: &str) -> bool {
    key.chars().next().is_some_and(|c| c.is_ascii_lowercase()) && key.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// `YYYY-MM-DDTHH:MM:SSZ`.
fn iso_8601_utc(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 20
        && bytes.iter().enumerate().all(|(index, byte)| match index {
            4 | 7 => *byte == b'-',
            10 => *byte == b'T',
            13 | 16 => *byte == b':',
            19 => *byte == b'Z',
            _ => byte.is_ascii_digit(),
        })
}

/// Every key under `value` is snake_case and every date field holds an ISO 8601 date or null; returns how many dates
/// it saw.
fn check(value: &Value, path: &str) -> usize {
    match value {
        Value::Object(fields) => fields
            .iter()
            .map(|(key, field)| {
                assert!(snake_case(key), "{path}.{key} is not snake_case");
                let date = matches!(key.as_str(), "date" | "received" | "sent") && !field.is_array() && !field.is_object();
                if date {
                    assert!(field.is_null() || field.as_str().is_some_and(iso_8601_utc), "{path}.{key} = {field} is not ISO 8601 UTC");
                }
                usize::from(date && field.is_string()) + check(field, &format!("{path}.{key}"))
            })
            .sum(),
        Value::Array(items) => items.iter().enumerate().map(|(index, item)| check(item, &format!("{path}[{index}]"))).sum(),
        _ => 0,
    }
}

#[test]
fn every_answer_is_one_object_with_snake_case_keys_and_iso_8601_dates() {
    let dir = tempfile::tempdir().unwrap();
    store::fixture(dir.path()).unwrap();
    let dir = dir.path().to_str().unwrap();
    let read = format!(r#"{{"id":{}}}"#, store::LUNCH);
    let mut dates = 0;
    for (id, params) in [
        ("account.list", "{}"),
        ("folder.list", "{}"),
        ("mail.list", "{}"),
        ("mail.read", read.as_str()),
        ("mail.thread", read.as_str()),
        ("mail.search", r#"{"subject":"lunch"}"#),
        ("remote.list", "{}"),
    ] {
        let (code, out, err) = cli(&["--store", dir, "--json", id, params]);
        assert_eq!((code, err.as_str()), (0, ""), "{id}");
        assert!(out.ends_with('\n') && out.matches('\n').count() == 1, "{id}: one line");
        let answer: Value = serde_json::from_str(&out).unwrap();
        assert!(answer.is_object(), "{id}: {answer}");
        dates += check(&answer, id);
    }
    assert!(dates > 0, "the fixture's messages carry dates");
}

#[test]
fn the_fields_agents_read_stay() {
    let dir = tempfile::tempdir().unwrap();
    store::fixture(dir.path()).unwrap();
    let dir = dir.path().to_str().unwrap();
    let (_, out, _) = cli(&["--store", dir, "--json", "mail.list", "{}"]);
    let list: Value = serde_json::from_str(&out).unwrap();
    for key in ["view", "account", "total", "offset", "limit", "messages"] {
        assert!(list.get(key).is_some(), "mail.list lost {key}");
    }
    let message = &list["messages"][0];
    for key in ["id", "account", "folder", "date", "from", "to", "subject", "preview", "seen", "flagged", "has_body"] {
        assert!(message.get(key).is_some(), "a listed message lost {key}");
    }
    assert_eq!(message["date"], "2026-11-02T08:30:00Z");
}

#[test]
fn an_error_is_an_object_with_a_snake_case_code_and_a_message() {
    let (code, out, err) = cli(&["--json", "--store"]);
    assert_eq!((code, out.as_str()), (2, ""));
    let error: Value = serde_json::from_str(&err).unwrap();
    let fields = error["error"].as_object().unwrap();
    assert!(snake_case(fields["code"].as_str().unwrap()));
    assert!(fields["message"].is_string());
    assert_eq!(error.as_object().map(|object| object.len()), Some(1));
}

#[test]
fn the_command_list_is_json_too() {
    let (code, out, err) = cli(&["--json", "commands"]);
    assert_eq!((code, err.as_str()), (0, ""));
    let list: Value = serde_json::from_str(&out).unwrap();
    let commands = list["commands"].as_array().unwrap();
    assert!(commands.iter().any(|command| command["id"] == "mail.list" && command["class"] == "safe"));
    assert!(commands.iter().any(|command| command["id"] == "account.remove" && command["class"] == "owner"));
    assert!(commands.iter().all(|command| command["summary"].as_str().is_some_and(|summary| !summary.is_empty())));
    check(&list, "commands");
}
