// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde_json::json;

use super::{NoParams, decode, parse_date};
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

#[test]
fn iso_dates_and_times_are_read() {
    use crate::test_support::{NOW, at};
    assert_eq!(parse_date("2026-11-05T09:00:00Z"), Some(at(NOW)));
    assert_eq!(parse_date("2026-11-05T10:00:00+01:00"), Some(at(NOW)));
    assert_eq!(parse_date("2026-11-05T08:30:00-00:30"), Some(at(NOW)));
    assert_eq!(parse_date("2026-11-05"), Some(at(NOW - 9 * 3600)));
    assert_eq!(parse_date("0000-01-01"), Some(inboxcraft_time::Timestamp::MIN));
}

#[test]
fn anything_else_is_not_a_date() {
    for bad in [
        "",
        "2026",
        "2026-11-5",
        "2026/11/05",
        "2026-13-01",
        "2026-02-30",
        "+026-11-05",
        "2026-11-05T09:00",
        "2026-11-05T09:00:00",
        "2026-11-05 09:00:00Z",
        "2026-11-05T25:00:00Z",
        "2026-11-05T09:00:60Z",
        "2026-11-05T09:00:00z",
        "2026-11-05T09:00:00+0100",
        "2026-11-05T09:00:00+01-00",
        "2026-11-05T09:00:00*01:00",
        "2026-11-05T09:00:00+99:99",
        "2026-11-05T09:00:00Z ",
        "2026-11-0é",
        "２０２６-11-05",
    ] {
        assert_eq!(parse_date(bad), None, "{bad:?}");
    }
}
