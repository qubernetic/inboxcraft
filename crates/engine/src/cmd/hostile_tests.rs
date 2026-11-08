// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `hostile_params_never_panic`: every command, run with junk parameters, answers or refuses and never panics.
//!
//! The commands come from `cmd::all` and the parameter names from each module's `FIELDS`, so a new command is in
//! here without anyone adding it. A panic would be caught by `Session::run` as `EngineError::Panicked`; that counts
//! as a failure here, because the safety net is not a licence. Each runs in a session over a store and in a sample
//! session, where the `sample.*` commands run too.

use proptest::prelude::*;
use serde_json::{Map, Value, json};

use super::MODULES;
use crate::test_support::{sample_session, session};
use crate::{Caller, EngineError, Session};

/// Names no command reads.
const STRANGERS: [&str; 4] = ["message", "view", "", "\u{0}"];

/// Every parameter name a module says its commands read, and a few no command reads.
fn fields() -> Vec<&'static str> {
    let mut fields: Vec<&'static str> = MODULES.iter().flat_map(|module| module.fields).copied().collect();
    fields.extend(STRANGERS);
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
        json!("9999999999-01-01"),
        json!([]),
        json!([1, 2, 3]),
        json!({}),
        json!({ "nested": { "deeper": [null] } }),
        deep,
    ]
}

fn run_everything(session: &mut Session, params: &Value) {
    let ids: Vec<&'static str> = session.registry().specs().map(|spec| spec.id).collect();
    for id in ids {
        for caller in [Caller::Owner, Caller::Agent] {
            let result = session.run_as(caller, id, params);
            assert!(!matches!(result, Err(EngineError::Panicked(_))), "{id} panicked on {params}");
        }
    }
}

#[test]
fn hostile_params_never_panic() {
    for mut session in [session(), sample_session(), super::calendar_tests::session()] {
        for value in junk() {
            run_everything(&mut session, &value);
            for field in fields() {
                let params = Value::Object(Map::from_iter([(field.to_owned(), value.clone())]));
                run_everything(&mut session, &params);
            }
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
            prop::collection::vec((prop::sample::select(fields()), inner), 0..6)
                .prop_map(|pairs| Value::Object(pairs.into_iter().map(|(key, value)| (key.to_owned(), value)).collect())),
        ]
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn random_params_never_panic(params in hostile_value()) {
        for mut session in [session(), sample_session()] {
            run_everything(&mut session, &params);
        }
    }
}

#[test]
fn calendar_writes_with_one_hostile_field_never_panic() {
    let bases = [
        ("event.create", json!({ "calendar": 1, "summary": "A", "start": "2026-11-12T10:00:00Z" })),
        ("event.update", json!({ "id": 1, "summary": "A" })),
        ("event.update", json!({ "id": 4, "start": "2026-11-20" })),
        ("event.list", json!({ "from": "2026-11-05", "to": "2026-11-12" })),
        ("calendar.server", json!({ "account": 1, "url": "https://dav.example.org/" })),
    ];
    let mut session = super::calendar_tests::session();
    for (id, base) in bases {
        for field in fields() {
            for value in junk() {
                let mut params = base.clone();
                if let Value::Object(map) = &mut params {
                    map.insert(field.to_owned(), value.clone());
                }
                let result = session.run(id, &params);
                assert!(!matches!(result, Err(EngineError::Panicked(_))), "{id} panicked on {params}");
            }
        }
    }
}
