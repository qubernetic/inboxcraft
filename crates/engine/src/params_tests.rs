// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde_json::json;

use super::{NoParams, decode, decode_quietly, parse_date, quiet};
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

#[test]
fn a_quiet_complaint_repeats_nothing_quoted() {
    assert_eq!(quiet(r#"invalid type: string "hunter2", expected u16"#), "invalid type: string");
    assert_eq!(quiet("unknown field `hunter2`, expected one of `a`, `b`"), "unknown field");
    assert_eq!(quiet(r#"string "a\"b" and "c"#), "string");
    assert_eq!(quiet("nothing quoted"), "nothing quoted");
    assert_eq!(quiet("`all quoted`"), "");
    let error = decode_quietly::<NoParams>("test.none", &json!({ "hunter2": 1 })).unwrap_err();
    assert!(!error.to_string().contains("hunter2"), "{error}");
    assert_eq!(decode_quietly::<NoParams>("test.none", &json!(3)).unwrap_err().code(), "bad_params");
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code, reason = "only the refusal is looked at")]
struct Choice {
    security: Pick,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum Pick {
    Tls,
}

#[test]
fn a_back_quote_inside_a_sent_value_brings_nothing_back() {
    const SECRET: &str = "maple-Harbour-41";
    let key = format!("x`, `{SECRET}`, `y");
    let params = serde_json::Value::Object([(key, json!(1))].into_iter().collect());
    let error = decode_quietly::<Choice>("test.choice", &params).unwrap_err();
    assert!(!error.to_string().contains(SECRET), "{error}");
    let error = decode_quietly::<Choice>("test.choice", &json!({ "security": format!("a`{SECRET}`b") })).unwrap_err();
    assert!(!error.to_string().contains(SECRET), "{error}");
    let error = decode_quietly::<Choice>("test.choice", &json!({ "security": format!("a\"{SECRET}\"b") })).unwrap_err();
    assert!(!error.to_string().contains(SECRET), "{error}");
}
