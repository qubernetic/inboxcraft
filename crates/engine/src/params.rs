// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Reading a command's parameters. They come from outside (the CLI, the control channel, MCP) and are hostile.

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

/// The parameters of a command that takes none.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NoParams {}

#[cfg(test)]
#[path = "params_tests.rs"]
mod tests;
