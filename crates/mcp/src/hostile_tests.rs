// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `hostile_params_never_panic` through MCP: every tool with junk arguments, and junk lines, answer or refuse and never
//! panic. A panic caught by the session's net (`panicked`) counts as a failure: the net is not a licence.

use proptest::prelude::*;
use serde_json::{Map, Value, json};

use crate::Server;
use crate::test_support::{call, fixture_server, text};
use crate::tools::TOOLS;

/// Every argument name a tool's schema lists, and a few none does.
fn fields() -> Vec<String> {
    let mut fields: Vec<String> = TOOLS
        .iter()
        .flat_map(|tool| {
            (tool.schema)()["properties"].as_object().map(|properties| properties.keys().cloned().collect::<Vec<_>>()).unwrap_or_default()
        })
        .collect();
    fields.extend(["message", "", "\u{0}", "name"].map(str::to_owned));
    fields.sort_unstable();
    fields.dedup();
    fields
}

fn junk() -> Vec<Value> {
    let mut deep = json!(1);
    for _ in 0..100 {
        deep = json!([deep]);
    }
    vec![
        json!(null),
        json!(true),
        json!(0),
        json!(1),
        json!(-1),
        json!(u64::MAX),
        json!(i64::MIN),
        json!(1.5),
        json!(1e308),
        json!(""),
        json!("x".repeat(100_000)),
        json!("\r\n\u{0}\u{1b}[2J\u{202e}"),
        json!("inbox"),
        json!("2026-11-05"),
        json!("2026-02-30T25:61:61Z"),
        json!([]),
        json!({}),
        json!({ "nested": { "deeper": [null] } }),
        deep,
    ]
}

/// Calls every tool with `arguments` and checks the answer is a result or a refusal, not a caught panic.
fn call_everything(server: &mut Server, arguments: &Value) {
    for tool in &TOOLS {
        let (out, error) = text(&call(server, tool.name, arguments.clone()));
        assert!(!out.contains("panicked"), "{} panicked on {arguments}: {out}", tool.name);
        assert!(!error || out.starts_with("error "), "{}: {out}", tool.name);
    }
}

#[test]
fn hostile_params_never_panic() {
    let (_dir, mut server) = fixture_server();
    let fields = fields();
    for value in junk() {
        call_everything(&mut server, &value);
        for field in &fields {
            call_everything(&mut server, &Value::Object(Map::from_iter([(field.clone(), value.clone())])));
        }
    }
}

fn hostile_value() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(|n| json!(n)),
        any::<u64>().prop_map(|n| json!(n)),
        any::<f64>().prop_map(|n| json!(n)),
        ".{0,40}".prop_map(Value::String),
    ];
    leaf.prop_recursive(4, 32, 6, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..6).prop_map(Value::Array),
            prop::collection::vec((prop::sample::select(fields()), inner), 0..6).prop_map(|pairs| Value::Object(pairs.into_iter().collect())),
        ]
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn random_arguments_never_panic(arguments in hostile_value()) {
        let (_dir, mut server) = fixture_server();
        call_everything(&mut server, &arguments);
    }

    #[test]
    fn random_requests_never_panic(id in hostile_value(), method in prop::sample::select(vec!["initialize", "ping", "tools/list", "tools/call", "x"]),
        params in hostile_value()) {
        let (_dir, mut server) = fixture_server();
        let line = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }).to_string();
        if let Some(answer) = server.handle(&line) {
            let answer: Value = serde_json::from_str(&answer).unwrap();
            prop_assert_eq!(&answer["jsonrpc"], "2.0");
            prop_assert!(!answer.to_string().contains("panicked"));
        }
    }

    #[test]
    fn random_lines_never_panic(line in ".{0,200}") {
        let (_dir, mut server) = fixture_server();
        if let Some(answer) = server.handle(&line) {
            prop_assert!(serde_json::from_str::<Value>(&answer).is_ok());
            prop_assert!(!answer.contains('\n'));
        }
    }
}
