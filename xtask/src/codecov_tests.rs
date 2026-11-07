// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `codecov.yml` and `.config/coverage.toml`, read as text: Codecov ignores exactly the files the in-repo coverage
//! gate counts nowhere (`exclude`) or leaves out of the workspace total (`workspace-exclude`).

use std::collections::BTreeSet;

/// The items of the top-level `ignore:` list of `codecov.yml`, without quotes.
fn ignored(text: &str) -> Vec<String> {
    text.lines()
        .skip_while(|line| line.trim_end() != "ignore:")
        .skip(1)
        .take_while(|line| line.trim().is_empty() || line.starts_with(' '))
        .filter_map(|line| line.trim().strip_prefix("- "))
        .map(|item| item.trim().trim_matches(['"', '\'']).to_owned())
        .collect()
}

/// The patterns of `exclude` and `workspace-exclude` in `.config/coverage.toml`.
fn excluded(text: &str) -> Vec<String> {
    let table: toml::Table = text.parse().unwrap_or_default();
    ["exclude", "workspace-exclude"]
        .iter()
        .filter_map(|key| table.get(*key).and_then(toml::Value::as_array))
        .flatten()
        .filter_map(|item| item.as_str().map(str::to_owned))
        .collect()
}

/// How the two lists differ: every pattern in one but not the other.
fn problems(codecov: &str, gate: &str) -> Vec<String> {
    let ignored: BTreeSet<String> = ignored(codecov).into_iter().collect();
    let excluded: BTreeSet<String> = excluded(gate).into_iter().collect();
    let mut found = Vec::new();
    for pattern in excluded.difference(&ignored) {
        found.push(format!("codecov.yml does not ignore {pattern}"));
    }
    for pattern in ignored.difference(&excluded) {
        found.push(format!("codecov.yml ignores {pattern}, which the coverage gate counts"));
    }
    if ignored.is_empty() {
        found.push("codecov.yml has no ignore list".to_owned());
    }
    found
}

fn read(path: &str) -> String {
    std::fs::read_to_string(crate::default_root().join(path)).unwrap_or_default()
}

#[test]
fn codecov_ignores_what_the_coverage_gate_leaves_out() {
    assert_eq!(problems(&read("codecov.yml"), &read(".config/coverage.toml")), Vec::<String>::new());
}

#[test]
fn an_entry_missing_from_codecov_is_found() {
    let codecov = read("codecov.yml").replace("  - \"xtask/src/main.rs\"\n", "");
    let found = problems(&codecov, &read(".config/coverage.toml"));
    assert_eq!(found, vec!["codecov.yml does not ignore xtask/src/main.rs".to_owned()]);
}

#[test]
fn an_entry_missing_from_the_gate_is_found() {
    let gate = read(".config/coverage.toml").replace("\"apps/*/src/main.rs\", ", "");
    let found = problems(&read("codecov.yml"), &gate);
    assert_eq!(found, vec!["codecov.yml ignores apps/*/src/main.rs, which the coverage gate counts".to_owned()]);
}

#[test]
fn a_missing_ignore_list_is_found() {
    let found = problems("coverage:\n  status:\n", "exclude = [\"a.rs\"]\n");
    assert_eq!(found, vec!["codecov.yml does not ignore a.rs".to_owned(), "codecov.yml has no ignore list".to_owned()]);
}

#[test]
fn the_ignore_list_ends_at_the_next_top_level_key() {
    let text = "ignore:\n  - \"a.rs\"\n\n  - 'b/**'\ncoverage:\n  - \"c.rs\"\n";
    assert_eq!(ignored(text), vec!["a.rs".to_owned(), "b/**".to_owned()]);
}
