// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! One command list per module: a new command edits only its own `cmd/*.rs` file.

use std::collections::BTreeSet;
use std::path::Path;

use super::{MODULES, all};

/// The modules on disk: every `cmd/*.rs` but `mod.rs` and the test files.
fn files() -> BTreeSet<String> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/cmd");
    std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .filter_map(|name| name.strip_suffix(".rs").map(str::to_owned))
        .filter(|name| name != "mod" && name != "tests" && !name.ends_with("_tests"))
        .collect()
}

#[test]
fn every_module_file_has_its_command_list_in_the_registry() {
    let listed: BTreeSet<String> = MODULES.iter().map(|module| module.name.to_owned()).collect();
    assert_eq!(listed, files());
    assert_eq!(MODULES.len(), listed.len(), "a module is listed twice");
}

#[test]
fn the_epic_areas_have_a_module_to_add_their_commands_to() {
    let files = files();
    for area in ["act", "compose", "send", "outbox", "triage", "schedule", "notify"] {
        assert!(files.contains(area), "cmd/{area}.rs is missing");
    }
}

#[test]
fn the_registry_takes_every_command_of_every_module() {
    let ids: Vec<&str> = all().iter().map(|spec| spec.id).collect();
    let from_modules: Vec<&str> = MODULES.iter().flat_map(|module| module.commands).map(|spec| spec.id).collect();
    assert_eq!(ids, from_modules);
    for id in ["account.add", "folder.list", "mail.read", "remote.allow", "sample.mail.set", "ui.frame.set"] {
        assert!(ids.contains(&id), "{id}");
    }
}

#[test]
fn every_module_names_the_parameters_its_commands_read() {
    for module in MODULES {
        if !module.commands.is_empty() {
            assert!(!module.fields.is_empty(), "cmd/{}.rs lists no parameter names for hostile_params_never_panic", module.name);
        }
    }
    let fields: Vec<&str> = MODULES.iter().flat_map(|module| module.fields).copied().collect();
    assert!(fields.contains(&"frame"), "ui.frame.set reads `frame`");
}
