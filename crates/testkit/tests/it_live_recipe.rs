// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The `it-live` recipe of the `justfile` keeps the live credentials away from everything but the live test.
//!
//! The recipe runs only on the owner's host, so these checks read its text: the build carries no credential, only
//! the run from the archive does, and no value is ever an argument of an external command.

use std::path::Path;

/// The body of the `it-live` recipe, one trimmed line each, without blank lines and comments.
fn recipe() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let justfile = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../justfile"))?;
    let mut lines = justfile.lines().skip_while(|line| !line.starts_with("it-live "));
    lines.next().ok_or("no it-live recipe in the justfile")?;
    let body: Vec<String> = lines
        .take_while(|line| line.is_empty() || line.starts_with(' '))
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
        .collect();
    if body.is_empty() { Err("the it-live recipe is empty".into()) } else { Ok(body) }
}

/// The index of the first line containing `needle`.
fn position(body: &[String], needle: &str) -> Result<usize, String> {
    body.iter().position(|line| line.contains(needle)).ok_or(format!("no line contains {needle:?}"))
}

#[test]
fn only_the_run_from_the_archive_carries_the_credentials() -> Result<(), Box<dyn std::error::Error>> {
    let body = recipe()?;
    let execs: Vec<&String> = body.iter().filter(|line| line.contains("{{ dc }} exec")).collect();
    let with_values: Vec<&&String> = execs.iter().filter(|line| line.contains("${pass[@]}") || line.contains(" -e ")).collect();
    assert_eq!(with_values.len(), 1, "{execs:#?}");
    let run = with_values.first().ok_or("no exec line carries the credentials")?;
    assert!(run.contains("cargo nextest run --archive-file \"$archive\""), "{run}");
    assert!(run.contains("--profile live"), "{run}");
    for refused in ["nextest archive", "--no-run", "cargo build", "cargo test", "--workspace ", "-E ", "--filter-expr", "--ignore-default-filter"] {
        assert!(!run.contains(refused), "the run with the credentials has {refused:?}: {run}");
    }
    let build = execs.iter().find(|line| line.contains("nextest archive")).ok_or("no build step")?;
    assert!(build.contains("-p inboxcraft-testkit -p inboxcraft-net --test live --archive-file \"$archive\""), "{build}");
    Ok(())
}

#[test]
fn the_build_runs_before_any_credential_is_read() -> Result<(), Box<dyn std::error::Error>> {
    let body = recipe()?;
    let build = position(&body, "nextest archive")?;
    assert!(build < position(&body, "value=\"$(lookup")?, "{body:#?}");
    assert!(build < position(&body, "export \"$var=")?, "{body:#?}");
    Ok(())
}

#[test]
fn the_archive_folder_is_made_before_the_build() -> Result<(), Box<dyn std::error::Error>> {
    // A fresh checkout has no `target/nextest`, and `nextest archive` does not make it (exit 103).
    let body = recipe()?;
    let mkdir = position(&body, "mkdir -p target/nextest")?;
    assert!(mkdir < position(&body, "nextest archive")?, "{body:#?}");
    assert!(position(&body, "archive=target/nextest/")? < position(&body, "nextest archive")?, "{body:#?}");
    Ok(())
}

#[test]
fn the_values_of_both_accounts_are_read() -> Result<(), Box<dyn std::error::Error>> {
    let body = recipe()?;
    let accounts = position(&body, "for account in ")?;
    assert_eq!(body.get(accounts).map(String::as_str), Some("for account in IMAP IMAP2; do"), "{body:#?}");
    assert!(body.get(accounts + 1).is_some_and(|line| line.contains("for field in ADDRESS HOST USER SECRET; do")), "{body:#?}");
    Ok(())
}

#[test]
fn the_credentials_pass_by_name_only() -> Result<(), Box<dyn std::error::Error>> {
    let body = recipe()?;
    let passes: Vec<&String> = body.iter().filter(|line| line.contains("pass+=(")).collect();
    assert!(!passes.is_empty(), "{body:#?}");
    for line in passes {
        assert!(line == "pass+=(-e \"$var\")" || line == "pass+=(-e INBOXCRAFT_LIVE_RUN_ID)", "{line}");
    }
    Ok(())
}

#[test]
fn no_value_is_an_argument_of_an_external_command() -> Result<(), Box<dyn std::error::Error>> {
    let body = recipe()?;
    // Assignments and the shell's own builtins only: none of them puts a value on a process's command line.
    let allowed = ["value=", "escaped=", "output=", "if [ -z \"$value\" ]; then", "export \"$var=$value\"", "if [[ "];
    let mut seen = 0;
    for line in body.iter().filter(|line| ["${!", "$value", "${value", "$escaped", "${escaped", "$output"].iter().any(|value| line.contains(value))) {
        seen += 1;
        assert!(allowed.iter().any(|prefix| line.starts_with(prefix)), "a value may reach an external command: {line}");
    }
    assert!(seen >= 6, "{seen} lines: {body:#?}");
    assert!(!body.iter().any(|line| line.contains("grep")), "the scan runs in bash: {body:#?}");
    Ok(())
}

#[test]
fn the_scan_covers_the_secrets_addresses_and_login_names() -> Result<(), Box<dyn std::error::Error>> {
    let body = recipe()?;
    let scan = body.get(position(&body, "output=\"$(<\"$log\")\"")?..).ok_or("no scan")?;
    let start = scan.iter().position(|line| line.starts_with("for var in ")).ok_or("no scan loop")?;
    let end = scan.iter().skip(start).position(|line| line.ends_with("; do")).ok_or("no end of the scan loop's list")?;
    let names = scan.iter().skip(start).take(end + 1).cloned().collect::<Vec<_>>().join(" ");
    for account in ["IMAP", "IMAP2"] {
        for field in ["SECRET", "ADDRESS", "USER"] {
            let var = format!("INBOXCRAFT_LIVE_{account}_{field}");
            assert!(names.contains(&var), "the scan misses {var}: {names}");
        }
    }
    Ok(())
}

#[test]
fn options_are_refused_and_only_test_names_pass_on() -> Result<(), Box<dyn std::error::Error>> {
    let body = recipe()?;
    let check = position(&body, "for arg in \"$@\"; do")?;
    assert!(body.get(check + 1).is_some_and(|line| line == "case \"$arg\" in"), "{body:#?}");
    assert!(body.get(check + 2).is_some_and(|line| line.starts_with("-*) ") && line.contains("exit 1")), "{body:#?}");
    assert!(check < position(&body, "nextest archive")?, "{body:#?}");
    Ok(())
}
