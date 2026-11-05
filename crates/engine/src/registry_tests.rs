// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use serde_json::{Value, json};

use super::{CommandSpec, Registry, is_command_id};
use crate::{AgentClass, Context, EngineError, Outcome};

fn nothing(_: &Context<'_>, _: &Value) -> Result<Outcome, EngineError> {
    Ok(Outcome::new(json!({})))
}

fn spec(id: &'static str, class: Option<AgentClass>) -> CommandSpec {
    CommandSpec { id, class, summary: "Does nothing", run: nothing }
}

#[test]
fn an_unclassified_command_is_refused() {
    let mut registry = Registry::new();
    assert_eq!(registry.register(spec("test.unclassified", None)).unwrap_err(), EngineError::Unclassified("test.unclassified"));
    assert!(registry.get("test.unclassified").is_none());
    assert_eq!(registry.specs().count(), 0);
}

#[test]
fn every_builtin_command_picked_a_class() {
    // A new command in `cmd::all` without a class fails here and in `Registry::builtin`.
    let all = crate::cmd::all();
    assert!(!all.is_empty());
    for spec in &all {
        assert!(spec.class.is_some(), "{} has no agent class", spec.id);
        assert!(is_command_id(spec.id), "{} is not a command id", spec.id);
        assert!(!spec.summary.is_empty(), "{} has no summary", spec.id);
    }
    let registry = Registry::builtin().unwrap();
    assert_eq!(registry.specs().count(), all.len(), "no id twice");
    let ids: Vec<&str> = registry.specs().map(|spec| spec.id).collect();
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    assert_eq!(ids, sorted, "in id order");
}

#[test]
fn the_read_commands_are_safe() {
    let registry = Registry::builtin().unwrap();
    let read: Vec<&str> = vec!["account.list", "folder.list", "mail.list"];
    for id in read {
        assert_eq!(registry.get(id).and_then(|spec| spec.class), Some(AgentClass::Safe), "{id}");
    }
}

#[test]
fn a_bad_or_taken_id_is_refused() {
    let mut registry = Registry::new();
    assert_eq!(registry.register(spec("Test", Some(AgentClass::Safe))).unwrap_err(), EngineError::BadCommandId("Test"));
    registry.register(spec("test.once", Some(AgentClass::Safe))).unwrap();
    assert_eq!(registry.register(spec("test.once", Some(AgentClass::Never))).unwrap_err(), EngineError::DuplicateCommand("test.once"));
    assert_eq!(registry.get("test.once").and_then(|spec| spec.class), Some(AgentClass::Safe));
    assert_eq!(registry.lookup("test.once").map(|(_, class)| class), Some(AgentClass::Safe));
}

#[test]
fn command_ids_are_lowercase_words_joined_by_dots() {
    for good in ["mail.list", "a.b", "account.check2", "mail.send_later", "x.y.z"] {
        assert!(is_command_id(good), "{good}");
    }
    let long = format!("a.{}", "b".repeat(63));
    for bad in
        ["", "mail", "mail.", ".list", "mail..list", "Mail.list", "mail.List", "mail.1st", "mail.li st", "mail.lïst", "mail-list.x", long.as_str()]
    {
        assert!(!is_command_id(bad), "{bad}");
    }
}
