// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The opt-in live test harness, driven with invented `example.org` values: never a real account.

use std::cell::RefCell;
use std::collections::HashMap;
use std::panic;
use std::sync::{Arc, Mutex};

use inboxcraft_testkit::live::{self, Folders, Live, LiveConfig, LiveError, RUN_ROOT, RunFolder, RunFolderError};

const IMAP_SECRET: &str = "imap-secret-Zq81vX";
const IMAP2_SECRET: &str = "imap2-secret-Wk27pL";

fn complete() -> HashMap<&'static str, String> {
    HashMap::from([
        ("INBOXCRAFT_LIVE_IMAP_ADDRESS", "first@example.org".to_owned()),
        ("INBOXCRAFT_LIVE_IMAP_HOST", "imap.example.org".to_owned()),
        ("INBOXCRAFT_LIVE_IMAP_USER", "first-login".to_owned()),
        ("INBOXCRAFT_LIVE_IMAP_SECRET", IMAP_SECRET.to_owned()),
        ("INBOXCRAFT_LIVE_IMAP2_ADDRESS", "second@example.org".to_owned()),
        ("INBOXCRAFT_LIVE_IMAP2_HOST", "mail.example.org".to_owned()),
        ("INBOXCRAFT_LIVE_IMAP2_USER", "second-login".to_owned()),
        ("INBOXCRAFT_LIVE_IMAP2_SECRET", IMAP2_SECRET.to_owned()),
        ("INBOXCRAFT_LIVE_RUN_ID", "20261104T120000Z-42".to_owned()),
    ])
}

fn load(vars: &HashMap<&'static str, String>) -> Result<Live, LiveError> {
    Live::from_vars(|name| vars.get(name).cloned())
}

fn config(vars: &HashMap<&'static str, String>) -> Result<LiveConfig, Box<dyn std::error::Error>> {
    match load(vars)? {
        Live::Run(config) => Ok(*config),
        Live::Skip(reason) => Err(format!("skipped: {reason}").into()),
    }
}

fn skip_reason(vars: &HashMap<&'static str, String>) -> Result<String, Box<dyn std::error::Error>> {
    match load(vars)? {
        Live::Skip(reason) => Ok(reason.to_string()),
        Live::Run(_) => Err("ran".into()),
    }
}

#[test]
fn without_credentials_the_live_tests_skip() -> Result<(), Box<dyn std::error::Error>> {
    let reason = skip_reason(&HashMap::new())?;
    assert!(reason.contains("no INBOXCRAFT_LIVE_* credentials"), "{reason}");
    assert!(reason.contains("just it-live"), "{reason}");
    Ok(())
}

#[test]
fn the_agent_sandbox_skips_even_with_credentials() -> Result<(), Box<dyn std::error::Error>> {
    let mut vars = complete();
    vars.insert("AGENT_HORDE", "1".to_owned());
    assert!(skip_reason(&vars)?.contains("AGENT_HORDE"));
    Ok(())
}

#[test]
fn ci_skips_even_with_credentials() -> Result<(), Box<dyn std::error::Error>> {
    let mut vars = complete();
    vars.insert("CI", "true".to_owned());
    assert!(skip_reason(&vars)?.contains("CI is set"));
    Ok(())
}

/// The names `from_vars` asks for, in order, with the complete set plus `extra`.
fn names_read(extra: &[(&'static str, &str)]) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut vars = complete();
    for (name, value) in extra {
        vars.insert(name, (*value).to_owned());
    }
    let read = RefCell::new(Vec::new());
    let live = Live::from_vars(|name| {
        read.borrow_mut().push(name.to_owned());
        vars.get(name).cloned()
    })?;
    assert!(matches!(live, Live::Skip(_)), "{live:?}");
    Ok(read.into_inner())
}

#[test]
fn the_sandbox_and_ci_checks_come_before_any_credential_is_read() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(names_read(&[("AGENT_HORDE", "1")])?, ["AGENT_HORDE"]);
    assert_eq!(names_read(&[("AGENT_HORDE", "1"), ("CI", "true")])?, ["AGENT_HORDE"]);
    assert_eq!(names_read(&[("CI", "true")])?, ["AGENT_HORDE", "CI"]);
    Ok(())
}

#[test]
fn an_empty_ci_or_agent_variable_does_not_skip() -> Result<(), Box<dyn std::error::Error>> {
    let mut vars = complete();
    vars.insert("CI", String::new());
    vars.insert("AGENT_HORDE", String::new());
    config(&vars)?;
    Ok(())
}

#[test]
fn partial_credentials_fail_and_name_only_the_missing_variables() -> Result<(), Box<dyn std::error::Error>> {
    let mut vars = complete();
    vars.remove("INBOXCRAFT_LIVE_IMAP2_SECRET");
    vars.remove("INBOXCRAFT_LIVE_IMAP_HOST");
    let err = load(&vars).err().ok_or("loaded")?;
    assert_eq!(err.to_string(), "live test configuration is incomplete, missing: INBOXCRAFT_LIVE_IMAP_HOST, INBOXCRAFT_LIVE_IMAP2_SECRET");
    Ok(())
}

#[test]
fn the_accounts_are_named_live_imap_and_live_imap2() -> Result<(), Box<dyn std::error::Error>> {
    let config = config(&complete())?;
    let [imap, imap2] = config.accounts();
    assert_eq!((imap.name(), imap2.name()), ("live-imap", "live-imap2"));
    assert_eq!(imap.address(), "first@example.org");
    assert_eq!(imap.host(), "imap.example.org");
    assert_eq!(imap.user(), "first-login");
    assert_eq!(imap.secret().expose(), IMAP_SECRET);
    assert_eq!(imap2.secret().expose(), IMAP2_SECRET);
    Ok(())
}

#[test]
fn the_allow_list_admits_exactly_the_two_run_time_addresses() -> Result<(), Box<dyn std::error::Error>> {
    let config = config(&complete())?;
    config.check_recipient("first@example.org")?;
    config.check_recipient("second@example.org")?;
    config.check_recipient("second@EXAMPLE.org")?;
    for refused in [
        "third@example.org",
        "First@example.org",
        "first@example.org.example.net",
        "first@sub.example.org",
        " first@example.org",
        "first@example.org ",
        "first@example.org\r\nRCPT TO:<third@example.org>",
        "first@example.org, third@example.org",
        "<first@example.org>",
        "first@example.org.",
        "\"first\"@example.org",
        "first",
        "@example.org",
        "",
    ] {
        let err = config.check_recipient(refused).err().ok_or(format!("{refused:?} was admitted"))?;
        assert!(matches!(err, LiveError::RecipientRefused), "{refused:?}: {err}");
        assert!(!err.to_string().contains("example.org"), "the refusal echoes an address: {err}");
    }
    Ok(())
}

#[test]
fn a_plain_dot_atom_address_is_accepted() -> Result<(), Box<dyn std::error::Error>> {
    for address in ["first.last+tag@mail.example.org", "o'neil_x-y=z@example.org", "a!#$%&*/?^`{|}~@x-1.example.org"] {
        let mut vars = complete();
        vars.insert("INBOXCRAFT_LIVE_IMAP_ADDRESS", address.to_owned());
        config(&vars)?.check_recipient(address)?;
    }
    Ok(())
}

#[test]
fn the_two_accounts_need_two_addresses() -> Result<(), Box<dyn std::error::Error>> {
    let mut vars = complete();
    vars.insert("INBOXCRAFT_LIVE_IMAP2_ADDRESS", "first@EXAMPLE.org".to_owned());
    let err = load(&vars).err().ok_or("loaded")?;
    assert!(matches!(err, LiveError::SameAddress), "{err}");
    Ok(())
}

#[test]
fn hostile_values_are_refused_without_echoing_them() -> Result<(), Box<dyn std::error::Error>> {
    for (variable, value) in [
        ("INBOXCRAFT_LIVE_IMAP_SECRET", format!("{IMAP_SECRET}\r\nA1 LOGOUT")),
        ("INBOXCRAFT_LIVE_IMAP_SECRET", String::new()),
        ("INBOXCRAFT_LIVE_IMAP2_SECRET", format!("{IMAP2_SECRET}\0")),
        ("INBOXCRAFT_LIVE_IMAP2_SECRET", "x".repeat(4097)),
        ("INBOXCRAFT_LIVE_IMAP_ADDRESS", "two@at@example.org".to_owned()),
        ("INBOXCRAFT_LIVE_IMAP_ADDRESS", "no-at-sign".to_owned()),
        ("INBOXCRAFT_LIVE_IMAP_ADDRESS", "@example.org".to_owned()),
        ("INBOXCRAFT_LIVE_IMAP_ADDRESS", "first@".to_owned()),
        ("INBOXCRAFT_LIVE_IMAP_ADDRESS", "first @example.org".to_owned()),
        ("INBOXCRAFT_LIVE_IMAP_ADDRESS", "<first@example.org>".to_owned()),
        ("INBOXCRAFT_LIVE_IMAP_ADDRESS", "\"first\"@example.org".to_owned()),
        ("INBOXCRAFT_LIVE_IMAP_ADDRESS", "a,b@example.org".to_owned()),
        ("INBOXCRAFT_LIVE_IMAP_ADDRESS", "a;b@example.org".to_owned()),
        ("INBOXCRAFT_LIVE_IMAP_ADDRESS", "a:b@example.org".to_owned()),
        ("INBOXCRAFT_LIVE_IMAP_ADDRESS", "a\\b@example.org".to_owned()),
        ("INBOXCRAFT_LIVE_IMAP_ADDRESS", "first@example.org.".to_owned()),
        ("INBOXCRAFT_LIVE_IMAP_ADDRESS", "first@[192.0.2.1]".to_owned()),
        ("INBOXCRAFT_LIVE_IMAP_ADDRESS", ".first@example.org".to_owned()),
        ("INBOXCRAFT_LIVE_IMAP_ADDRESS", "first.@example.org".to_owned()),
        ("INBOXCRAFT_LIVE_IMAP_ADDRESS", "fi..rst@example.org".to_owned()),
        ("INBOXCRAFT_LIVE_IMAP_ADDRESS", "first@example".to_owned()),
        ("INBOXCRAFT_LIVE_IMAP_ADDRESS", "first@-example.org".to_owned()),
        ("INBOXCRAFT_LIVE_IMAP_ADDRESS", "first@example-.org".to_owned()),
        ("INBOXCRAFT_LIVE_IMAP_ADDRESS", "first@example..org".to_owned()),
        ("INBOXCRAFT_LIVE_IMAP_ADDRESS", format!("first@{}.org", "x".repeat(64))),
        ("INBOXCRAFT_LIVE_IMAP_ADDRESS", "fürst@example.org".to_owned()),
        ("INBOXCRAFT_LIVE_IMAP_HOST", "imap.example.org\n".to_owned()),
        ("INBOXCRAFT_LIVE_IMAP2_USER", "second\tuser".to_owned()),
    ] {
        let mut vars = complete();
        vars.insert(variable, value.clone());
        let err = load(&vars).err().ok_or(format!("{variable}={value:?} was accepted"))?;
        assert!(matches!(&err, LiveError::Invalid { variable: v, .. } if *v == variable), "{variable}: {err}");
        let text = err.to_string();
        assert!(text.starts_with(variable), "{text}");
        assert!(!text.contains("secret-") && !text.contains("example.org") && !text.contains("user"), "{text}");
    }
    Ok(())
}

#[test]
fn the_run_folder_is_inboxcraft_test_and_the_run_id() -> Result<(), Box<dyn std::error::Error>> {
    let config = config(&complete())?;
    assert_eq!(RUN_ROOT, "InboxCraft-Test");
    assert_eq!(config.run_id(), "20261104T120000Z-42");
    assert_eq!(config.run_folder('/')?, "InboxCraft-Test/20261104T120000Z-42");
    assert_eq!(config.run_folder('.')?, "InboxCraft-Test.20261104T120000Z-42");
    Ok(())
}

#[test]
fn without_a_run_id_one_is_made_from_the_clock_and_the_process() -> Result<(), Box<dyn std::error::Error>> {
    let mut vars = complete();
    vars.remove("INBOXCRAFT_LIVE_RUN_ID");
    let config = config(&vars)?;
    let id = config.run_id();
    let (stamp, pid) = id.split_once('-').ok_or(id.to_owned())?;
    assert_eq!(stamp.len(), "20261104T120000Z".len(), "{id}");
    assert!(stamp.ends_with('Z') && stamp.chars().nth(8) == Some('T'), "{id}");
    assert_eq!(pid, std::process::id().to_string());
    Ok(())
}

#[test]
fn a_hostile_run_id_is_refused() -> Result<(), Box<dyn std::error::Error>> {
    for id in ["", "../INBOX", "a/b", "a.b", "a b", "a\r\nb", &"x".repeat(65)] {
        let mut vars = complete();
        vars.insert("INBOXCRAFT_LIVE_RUN_ID", id.to_owned());
        let err = load(&vars).err().ok_or(format!("{id:?} was accepted"))?;
        assert!(matches!(err, LiveError::Invalid { variable: "INBOXCRAFT_LIVE_RUN_ID", .. }), "{id:?}: {err}");
    }
    Ok(())
}

#[test]
fn start_prints_the_skip_reason_and_returns_none() -> Result<(), Box<dyn std::error::Error>> {
    assert!(live::start_from("a_live_test", |_| None)?.is_none());
    let vars = complete();
    let config = live::start_from("a_live_test", |name| vars.get(name).cloned())?.ok_or("skipped")?;
    assert_eq!(config.imap().name(), "live-imap");
    assert_eq!(config.imap2().name(), "live-imap2");
    Ok(())
}

#[test]
fn from_env_reads_the_process_environment() -> Result<(), Box<dyn std::error::Error>> {
    // Skips in the sandbox and in CI; runs in the owner's `just it-live`.
    let ran = matches!(Live::from_env()?, Live::Run(_));
    assert_eq!(live::start("from_env_reads_the_process_environment")?.is_some(), ran);
    Ok(())
}

#[test]
fn debug_and_display_never_show_a_secret_or_an_address() -> Result<(), Box<dyn std::error::Error>> {
    let config = config(&complete())?;
    let mut text = format!("{config:?} {:?} {}", config.imap().secret(), config.imap2().secret());
    for err in [
        LiveError::RecipientRefused,
        LiveError::SameAddress,
        LiveError::Delimiter,
        LiveError::Incomplete { missing: vec!["INBOXCRAFT_LIVE_IMAP_SECRET"] },
        LiveError::Invalid { variable: "INBOXCRAFT_LIVE_IMAP_SECRET", why: "is empty" },
    ] {
        text.push_str(&format!("{err} {err:?}"));
    }
    assert_eq!(config.leaked_secret(&text), None, "{text}");
    assert_eq!(config.leaked_identifier(&text), None, "{text}");
    assert!(!text.contains("example.org") && !text.contains("-login"), "{text}");
    assert!(text.contains("live-imap") && text.contains("live-imap2"), "{text}");
    Ok(())
}

#[test]
fn panic_output_never_shows_a_secret() -> Result<(), Box<dyn std::error::Error>> {
    let config = config(&complete())?;
    let captured = Arc::new(Mutex::new(String::new()));
    let sink = Arc::clone(&captured);
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        if let Ok(mut out) = sink.lock() {
            out.push_str(&info.to_string());
        }
    }));
    let outcome = panic::catch_unwind(|| {
        let refused = config.check_recipient("third@example.org");
        assert!(refused.is_ok(), "a live test failed with {config:?}: {refused:?}");
    });
    panic::set_hook(previous);
    assert!(outcome.is_err(), "the assertion did not fail");
    let output = captured.lock().map_err(|_| "poisoned")?.clone();
    assert!(output.contains("a live test failed with"), "{output}");
    assert_eq!(config.leaked_secret(&output), None, "{output}");
    assert_eq!(config.leaked_identifier(&output), None, "{output}");
    assert!(!output.contains("example.org") && !output.contains("-login"), "{output}");
    Ok(())
}

#[test]
fn the_leak_scan_finds_a_secret_in_a_log() -> Result<(), Box<dyn std::error::Error>> {
    let config = config(&complete())?;
    let log = format!("C: A1 LOGIN first {IMAP_SECRET}\nS: A1 OK\n");
    assert_eq!(config.leaked_secret(&log), Some("live-imap"));
    assert_eq!(config.leaked_secret(&format!("token={IMAP2_SECRET}")), Some("live-imap2"));
    assert_eq!(config.leaked_secret("S: * OK ready"), None);
    Ok(())
}

#[test]
fn the_leak_scan_finds_a_secret_in_its_debug_form() -> Result<(), Box<dyn std::error::Error>> {
    let secret = r#"quote"and\slash"#;
    let mut vars = complete();
    vars.insert("INBOXCRAFT_LIVE_IMAP_SECRET", secret.to_owned());
    let config = config(&vars)?;
    let log = format!("assertion failed: left: {secret:?}");
    assert!(!log.contains(secret), "{log}");
    assert_eq!(config.leaked_secret(&log), Some("live-imap"));
    assert_eq!(config.leaked_secret(&format!("{IMAP2_SECRET:?}")), Some("live-imap2"));
    Ok(())
}

#[test]
fn the_leak_scan_finds_an_address_and_a_login_name_in_a_log() -> Result<(), Box<dyn std::error::Error>> {
    let config = config(&complete())?;
    for (planted, found) in [
        ("first@example.org", ("live-imap", "address")),
        ("first-login", ("live-imap", "user")),
        ("second@example.org", ("live-imap2", "address")),
        ("second-login", ("live-imap2", "user")),
    ] {
        let log = format!("S: * OK ready\nleft: {planted:?}\nS: A1 OK\n");
        assert_eq!(config.leaked_identifier(&log), Some(found), "{planted}");
    }
    assert_eq!(config.leaked_identifier("S: * OK ready, live-imap and live-imap2"), None);
    assert_eq!(config.leaked_identifier(IMAP_SECRET), None, "a secret is not an identifier");
    Ok(())
}

#[derive(Default)]
struct FakeFolders {
    calls: Vec<String>,
    fail_create: bool,
    fail_delete: bool,
}

impl Folders for FakeFolders {
    type Error = String;

    fn create(&mut self, name: &str) -> Result<(), String> {
        self.calls.push(format!("CREATE {name}"));
        if self.fail_create { Err("create refused".to_owned()) } else { Ok(()) }
    }

    fn delete(&mut self, name: &str) -> Result<(), String> {
        self.calls.push(format!("DELETE {name}"));
        if self.fail_delete { Err("delete refused".to_owned()) } else { Ok(()) }
    }
}

#[test]
fn the_run_folder_is_created_and_removed() -> Result<(), Box<dyn std::error::Error>> {
    let config = config(&complete())?;
    let mut folders = FakeFolders::default();
    let mut run = RunFolder::create(&mut folders, &config, '/')?;
    assert_eq!(run.name(), "InboxCraft-Test/20261104T120000Z-42");
    run.folders().calls.push("work".to_owned());
    run.remove()?;
    assert_eq!(folders.calls, ["CREATE InboxCraft-Test/20261104T120000Z-42", "work", "DELETE InboxCraft-Test/20261104T120000Z-42"]);
    Ok(())
}

#[test]
fn a_run_folder_left_by_a_failing_test_is_removed_on_drop() -> Result<(), Box<dyn std::error::Error>> {
    let config = config(&complete())?;
    let mut folders = FakeFolders::default();
    drop(RunFolder::create(&mut folders, &config, '.')?);
    assert_eq!(folders.calls, ["CREATE InboxCraft-Test.20261104T120000Z-42", "DELETE InboxCraft-Test.20261104T120000Z-42"]);
    Ok(())
}

#[test]
fn a_delimiter_that_could_leave_the_run_folder_is_refused() -> Result<(), Box<dyn std::error::Error>> {
    let config = config(&complete())?;
    for delimiter in ['a', 'Z', '0', '9', '-', '_', ' ', '\t', '\u{a0}', '\r', '\n', '\0', '\u{7f}', '*', '%'] {
        assert!(matches!(config.run_folder(delimiter), Err(LiveError::Delimiter)), "{delimiter:?}");
        let mut folders = FakeFolders::default();
        let refused = RunFolder::create(&mut folders, &config, delimiter).err().ok_or(format!("{delimiter:?} was accepted"))?;
        assert!(matches!(refused, RunFolderError::Refused(LiveError::Delimiter)), "{delimiter:?}: {refused}");
        assert!(folders.calls.is_empty(), "{delimiter:?}: {:?}", folders.calls);
    }
    for delimiter in ['/', '.', '\\', ':'] {
        assert_eq!(config.run_folder(delimiter)?, format!("{RUN_ROOT}{delimiter}20261104T120000Z-42"));
    }
    Ok(())
}

#[test]
fn a_failed_create_removes_nothing_and_a_failed_remove_is_reported() -> Result<(), Box<dyn std::error::Error>> {
    let config = config(&complete())?;
    let mut folders = FakeFolders { fail_create: true, ..FakeFolders::default() };
    let refused = RunFolder::create(&mut folders, &config, '/').err();
    assert_eq!(refused, Some(RunFolderError::Folders("create refused".to_owned())));
    assert_eq!(refused.map(|err| err.to_string()), Some("create refused".to_owned()));
    assert_eq!(folders.calls, ["CREATE InboxCraft-Test/20261104T120000Z-42"]);

    let mut folders = FakeFolders { fail_delete: true, ..FakeFolders::default() };
    let run = RunFolder::create(&mut folders, &config, '/')?;
    assert_eq!(run.remove().err(), Some("delete refused".to_owned()));
    assert_eq!(folders.calls, ["CREATE InboxCraft-Test/20261104T120000Z-42", "DELETE InboxCraft-Test/20261104T120000Z-42"]);
    Ok(())
}

#[test]
fn an_invented_message_names_the_run_and_its_number_and_no_real_person() -> Result<(), Box<dyn std::error::Error>> {
    let config = config(&complete())?;
    let message = config.invented_message(2);
    assert!(message.starts_with("From: InboxCraft live test <live-test@example.org>\r\nTo: InboxCraft live test <live-test@example.org>\r\n"));
    assert!(message.contains("\r\nSubject: InboxCraft live test 20261104T120000Z-42 message 2\r\n"), "{message}");
    assert!(message.contains("\r\nMessage-ID: <20261104T120000Z-42.2@live-test.example.org>\r\n"), "{message}");
    assert!(message.contains("\r\n\r\nAn invented message"), "{message}");
    assert!(message.split("\r\n").all(|line| !line.contains('\n') && !line.contains('\r')));
    assert!(config.leaked_identifier(&message).is_none() && config.leaked_secret(&message).is_none());
    assert_ne!(message, config.invented_message(3));
    assert!(message.contains(config.message_subject_prefix()));
    Ok(())
}
