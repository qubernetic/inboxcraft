// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde_json::json;

use super::{NoParams, decode};
use crate::{EngineError, MAX_ERROR_TEXT};

#[test]
fn null_and_an_empty_object_are_no_params() {
    decode::<NoParams>("test.none", &json!(null)).unwrap();
    decode::<NoParams>("test.none", &json!({})).unwrap();
}

#[test]
fn unknown_fields_and_other_shapes_are_refused() {
    for params in [json!({ "folder": 1 }), json!([]), json!(3), json!("x"), json!(true)] {
        let error = decode::<NoParams>("test.none", &params).unwrap_err();
        assert!(matches!(error, EngineError::BadParams { command: "test.none", .. }), "{params}: {error:?}");
    }
}

#[test]
fn a_complaint_repeats_little_of_the_params() {
    let hostile = format!("\r\n{}", "x".repeat(10_000));
    let params = serde_json::Value::Object([(hostile, json!(1))].into_iter().collect());
    let Err(EngineError::BadParams { reason, .. }) = decode::<NoParams>("test.none", &params) else { panic!("refused") };
    assert!(reason.chars().count() <= MAX_ERROR_TEXT, "{reason}");
    assert!(!reason.contains('\r') && !reason.contains('\n'), "{reason}");
}
