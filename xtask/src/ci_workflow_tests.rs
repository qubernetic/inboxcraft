// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `.github/workflows/ci.yml`, read as text: only a push to `develop`, `main` or an epic branch that is not the
//! branch's creation prunes `target/` and saves the dependency cache, and the cache key follows the runner image.

/// What a save or prune step's `if:` must say.
const PUSH_ONLY: &[&str] = &[
    "github.event_name == 'push'",
    "!github.event.created",
    "github.ref == 'refs/heads/develop'",
    "github.ref == 'refs/heads/main'",
    "startsWith(github.ref, 'refs/heads/epic/')",
];

/// One list item of the workflow: its first line without the `- `, and the lines indented under it.
struct Item<'a> {
    lines: Vec<&'a str>,
}

impl Item<'_> {
    /// The value of `key:` in the item, trimmed; a folded or literal block (`>-`, `|`) as its lines joined by spaces.
    fn get(&self, key: &str) -> Option<String> {
        let (at, value) = self
            .lines
            .iter()
            .enumerate()
            .find_map(|(at, line)| line.trim_start().strip_prefix(key).and_then(|rest| rest.strip_prefix(':')).map(|value| (at, value.trim())))?;
        if !value.starts_with(['>', '|']) {
            return Some(value.to_owned());
        }
        let depth = self.lines.get(at).map_or(0, |line| indent(line));
        let block: Vec<&str> = self.lines[at + 1..].iter().take_while(|line| indent(line) > depth).map(|line| line.trim()).collect();
        Some(block.join(" "))
    }

    fn mentions(&self, text: &str) -> bool {
        self.lines.iter().any(|line| line.contains(text))
    }
}

fn indent(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// Every list item of `text` that holds a `uses:`, `run:` or `name:`: the steps.
fn steps(text: &str) -> Vec<Item<'_>> {
    let lines: Vec<&str> = text.lines().collect();
    let mut found = Vec::new();
    for (at, line) in lines.iter().enumerate() {
        let Some(first) = line.trim_start().strip_prefix("- ") else { continue };
        let depth = indent(line);
        let mut item = Item { lines: vec![first] };
        for next in &lines[at + 1..] {
            if !next.trim().is_empty() && indent(next) <= depth {
                break;
            }
            item.lines.push(next);
        }
        if ["uses", "run", "name"].iter().any(|key| item.get(key).is_some()) {
            found.push(item);
        }
    }
    found
}

/// What is wrong with the workflow `text`.
fn problems(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let steps = steps(text);
    let uses = |prefix: &str| steps.iter().filter(|step| step.get("uses").is_some_and(|value| value.starts_with(prefix))).count();
    if uses("actions/cache@") > 0 {
        found.push("a step uses actions/cache, which saves on every event".to_owned());
    }
    if uses("actions/cache/save@") != 1 {
        found.push("not exactly one actions/cache/save step".to_owned());
    }
    for step in &steps {
        let save = step.get("uses").is_some_and(|value| value.starts_with("actions/cache/save@"));
        let prune = step.mentions("cargo xtask cache-prune");
        if !save && !prune {
            continue;
        }
        let what = if save { "the save" } else { "the prune" };
        let condition = step.get("if").unwrap_or_default();
        for part in PUSH_ONLY {
            if !condition.contains(part) {
                found.push(format!("{what} step's if: lacks {part}"));
            }
        }
        if prune && step.get("shell").as_deref() != Some("bash") {
            found.push("the prune step does not run with shell: bash".to_owned());
        }
    }
    if !steps.iter().any(|step| step.mentions("cargo xtask cache-prune")) {
        found.push("no step runs cargo xtask cache-prune".to_owned());
    }
    let keys: Vec<&str> = text.lines().filter(|line| line.trim_start().starts_with("prefix=\"deps-")).collect();
    match keys.as_slice() {
        [key] => {
            for part in ["$ImageOS", "$ImageVersion", "$CARGO_CONFIG"] {
                if !key.contains(part) {
                    found.push(format!("the key line lacks {part}"));
                }
            }
        }
        _ => found.push("not exactly one cache key line".to_owned()),
    }
    if !text.contains("CARGO_CONFIG: ${{ hashFiles('.cargo/config.toml') }}") {
        found.push("the key does not hash .cargo/config.toml".to_owned());
    }
    found
}

fn workflow() -> String {
    std::fs::read_to_string(crate::default_root().join(".github/workflows/ci.yml")).unwrap()
}

#[test]
fn only_a_push_that_creates_no_branch_prunes_and_saves_the_cache() {
    assert_eq!(problems(&workflow()), Vec::<String>::new());
}

#[test]
fn a_save_on_another_event_is_found() {
    let text = workflow().replace(" && !github.event.created", "");
    let found = problems(&text);
    assert!(found.contains(&"the save step's if: lacks !github.event.created".to_owned()), "{found:?}");
    assert!(found.contains(&"the prune step's if: lacks !github.event.created".to_owned()), "{found:?}");
}

#[test]
fn a_second_save_or_a_plain_cache_is_found() {
    let text = format!("{}\n      - uses: actions/cache/save@v6\n      - name: x\n        uses: actions/cache@v4\n", workflow());
    let found = problems(&text);
    assert!(found.contains(&"not exactly one actions/cache/save step".to_owned()), "{found:?}");
    assert!(found.contains(&"a step uses actions/cache, which saves on every event".to_owned()), "{found:?}");
}

#[test]
fn a_key_without_the_image_or_a_prune_without_bash_is_found() {
    let text = workflow().replace("$ImageVersion", "").replace("shell: bash", "shell: sh").replace("hashFiles('.cargo/config.toml')", "''");
    let found = problems(&text);
    for problem in ["the key line lacks $ImageVersion", "the prune step does not run with shell: bash", "the key does not hash .cargo/config.toml"] {
        assert!(found.contains(&problem.to_owned()), "{problem}: {found:?}");
    }
    let found = problems(&workflow().replace("cargo xtask cache-prune", "true").replace("prefix=\"deps-", "prefix=\"x-"));
    assert!(
        found.contains(&"no step runs cargo xtask cache-prune".to_owned()) && found.contains(&"not exactly one cache key line".to_owned()),
        "{found:?}"
    );
}
