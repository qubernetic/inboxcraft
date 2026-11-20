// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `home`: Home's own choices. A new command goes into [`COMMANDS`] and its parameter names into `FIELDS`.
//!
//! `greeting.set` (`owner`) chooses which word of the sending name Home's headline greets the owner by: `first`,
//! `last`, `full` or `none` ([`crate::greet::GreetBy`]); any other value is refused and changes nothing.

use serde::Deserialize;
use serde_json::{Value, json};

use crate::greet::GreetBy;
use crate::params::decode;
use crate::{AgentClass, CommandSpec, Context, EngineError, Outcome};

/// The commands of this module, in the registry.
pub(crate) const COMMANDS: &[CommandSpec] = &[GREETING_SET];

/// The parameter names these commands read, for `hostile_params_never_panic`.
#[cfg(test)]
pub(crate) const FIELDS: &[&str] = &["greeting"];

/// `greeting.set`: which word of the sending name Home greets by.
pub(crate) const GREETING_SET: CommandSpec = CommandSpec {
    id: "greeting.set",
    class: Some(AgentClass::Owner),
    summary: "Choose how Home greets you: the first word of your name, the last, the full name, or no name",
    run: greeting_set,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GreetingParams {
    /// `first`, `last`, `full` or `none`.
    greeting: String,
}

fn greeting_set(_: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: GreetingParams = decode(GREETING_SET.id, params)?;
    let by = GreetBy::named(&params.greeting)
        .ok_or_else(|| EngineError::bad_params(GREETING_SET.id, "greeting is first, last, full or none"))?;
    Ok(Outcome::new(json!({ "greeting": by.name() })).with_greet_by(by))
}
