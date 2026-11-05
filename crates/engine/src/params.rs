// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Reading a command's parameters. They come from outside (the CLI, the control channel, MCP) and are hostile.

use inboxcraft_time::{CivilDateTime, Offset, Timestamp};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use crate::EngineError;

/// `params` as `T`; `null` counts as an empty object, and anything else that is not an object is refused. Every params type refuses unknown fields, so a typo is an error
/// and not a silently ignored criterion.
pub(crate) fn decode<T: DeserializeOwned>(command: &'static str, params: &Value) -> Result<T, EngineError> {
    let result = match params {
        Value::Null => T::deserialize(&Value::Object(Map::new())),
        Value::Object(_) => T::deserialize(params),
        _ => return Err(EngineError::bad_params(command, "the parameters are not a JSON object")),
    };
    result.map_err(|error| EngineError::bad_params(command, &error.to_string()))
}

/// As [`decode`], but a complaint repeats nothing the caller sent: it ends before its first `"` or `` ` `` mark, where
/// serde starts quoting a value or a field name. For a command whose parameters sit next to secrets, where a password
/// typed into the wrong field must not come back in an error.
pub(crate) fn decode_quietly<T: DeserializeOwned>(command: &'static str, params: &Value) -> Result<T, EngineError> {
    decode(command, params).map_err(|error| match error {
        EngineError::BadParams { command, reason } => EngineError::BadParams { command, reason: quiet(&reason) },
        other => other,
    })
}

/// `text` up to its first `"` or `` ` `` mark, without the spaces and punctuation before it. Nothing after the mark
/// is read: a mark inside a quoted value cannot end the quote early.
pub(crate) fn quiet(text: &str) -> String {
    let cut = text.find(['"', '`']).map_or(text, |at| text.get(..at).unwrap_or_default());
    cut.trim_end_matches([' ', ':', ',']).to_owned()
}

/// The parameters of a command that takes none.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NoParams {}

/// An ISO 8601 date or time from a parameter: `2026-11-05` (midnight UTC), `2026-11-05T09:30:00Z` or
/// `2026-11-05T09:30:00+01:00`. Anything else, or a day that does not exist, is `None`.
pub(crate) fn parse_date(text: &str) -> Option<Timestamp> {
    let bytes = text.as_bytes();
    let number = |from: usize, to: usize| -> Option<u16> {
        let digits = text.get(from..to)?;
        if digits.bytes().all(|byte| byte.is_ascii_digit()) { digits.parse().ok() } else { None }
    };
    let at = |index: usize, byte: u8| bytes.get(index) == Some(&byte);
    if !(at(4, b'-') && at(7, b'-')) {
        return None;
    }
    let year = i32::from(number(0, 4)?);
    let month = u8::try_from(number(5, 7)?).ok()?;
    let day = u8::try_from(number(8, 10)?).ok()?;
    if bytes.len() == 10 {
        return CivilDateTime::new(year, month, day, 0, 0, 0)?.to_timestamp(Offset::UTC);
    }
    if !(at(10, b'T') && at(13, b':') && at(16, b':')) {
        return None;
    }
    let hour = u8::try_from(number(11, 13)?).ok()?;
    let minute = u8::try_from(number(14, 16)?).ok()?;
    let second = u8::try_from(number(17, 19)?).ok()?;
    let offset = match (bytes.len(), bytes.get(19)) {
        (20, Some(b'Z')) => Offset::UTC,
        (25, Some(sign @ (b'+' | b'-'))) if at(22, b':') => {
            let minutes = i16::try_from(number(20, 22)?.checked_mul(60)?.checked_add(number(23, 25)?)?).ok()?;
            Offset::from_minutes(if *sign == b'-' { minutes.checked_neg()? } else { minutes })?
        }
        _ => return None,
    };
    CivilDateTime::new(year, month, day, hour, minute, second)?.to_timestamp(offset)
}

#[cfg(test)]
#[path = "params_tests.rs"]
mod tests;
