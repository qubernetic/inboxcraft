// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::ffi::OsString;
use std::io::{self, Write};

use inboxcraft_time::{Clock, FakeClock, Timestamp};
use inboxcraft_ui_egui::keep::NoKeep;

use inboxcraft_store::Memory;

use super::instance::{self, Instance, Request};
use super::store::Owned;
use super::{Mail, Source, Step, app, arrive, arrive_with, autostart, instance_dir, owner_keep, quit, run, set_autostart, store, usage};

fn run_with(args: &[&str]) -> (Step, String, String) {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let step = run(args.iter().map(OsString::from), &mut out, &mut err);
    (step, String::from_utf8_lossy(&out).into_owned(), String::from_utf8_lossy(&err).into_owned())
}

#[test]
fn version_prints_the_name_and_version() {
    for flag in ["--version", "-V"] {
        let (step, out, err) = run_with(&[flag]);
        assert_eq!(step, Step::Exit(0));
        assert_eq!(out, format!("inboxcraft {}\n", env!("CARGO_PKG_VERSION")));
        assert_eq!(err, "");
    }
}

#[test]
fn help_prints_the_usage() {
    for flag in ["--help", "-h"] {
        let (step, out, err) = run_with(&[flag]);
        assert_eq!(step, Step::Exit(0));
        assert_eq!(out, usage());
        assert!(out.contains("--sample"), "{out}");
        assert_eq!(err, "");
    }
}

#[test]
fn no_arguments_open_an_empty_window_and_sample_the_sample() {
    assert_eq!(run_with(&[]).0, Step::Window(Source::Owner));
    assert_eq!(run_with(&["--sample"]).0, Step::Window(Source::Sample));
}

#[test]
fn background_quit_and_autostart_are_steps_of_their_own() {
    assert_eq!(run_with(&["--background"]).0, Step::Background);
    assert_eq!(run_with(&["--quit"]).0, Step::Quit);
    assert_eq!(run_with(&["--autostart"]).0, Step::Autostart(true));
    assert_eq!(run_with(&["--no-autostart"]).0, Step::Autostart(false));
    assert_eq!(run_with(&["--revoke-control"]).0, Step::RevokeControl);
    for flag in ["--background", "--quit", "--autostart", "--no-autostart", "--revoke-control"] {
        assert!(usage().contains(flag), "{flag}");
    }
    assert_eq!(run_with(&["--background", "--sample"]).0, Step::Exit(2), "the sample has nothing to keep running");
}

#[test]
fn anything_else_is_refused_with_a_hint() {
    for args in [&["--frobnicate"][..], &["--sample", "extra"][..], &["--version", "extra"][..]] {
        let (step, out, err) = run_with(args);
        assert_eq!(step, Step::Exit(2));
        assert_eq!(out, "");
        assert!(err.contains("unknown arguments") && err.contains("--help"), "{err}");
    }
    let (_, _, err) = run_with(&["--x\u{1b}[31mred"]);
    assert!(!err.contains('\u{1b}'), "control characters are not echoed: {err:?}");
}

#[test]
fn non_utf8_arguments_do_not_panic() {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        let mut out = Vec::new();
        let mut err = Vec::new();
        assert_eq!(run([OsString::from_vec(vec![0xff, 0xfe])], &mut out, &mut err), Step::Exit(2));
    }
}

struct Broken;

impl Write for Broken {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("closed"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::other("closed"))
    }
}

#[test]
fn a_closed_output_is_an_error_code_not_a_panic() {
    let mut err = Vec::new();
    assert_eq!(run([OsString::from("--version")], &mut Broken, &mut err), Step::Exit(1));
    assert_eq!(run([OsString::from("--help")], &mut Broken, &mut err), Step::Exit(1));
    let mut out = Vec::new();
    assert_eq!(run([OsString::from("--nope")], &mut out, &mut Broken), Step::Exit(2));
    Broken.flush().unwrap_err();
}

/// The owner's mail in a store that is gone with the test.
fn owned() -> Mail {
    Mail::Store(Owned { store: Box::new(Memory::default()), id: "0a1b".to_owned() })
}

fn mail(source: Source) -> Mail {
    match source {
        Source::Sample => Mail::Sample,
        Source::Owner => owned(),
    }
}

fn clock() -> Box<dyn Clock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

#[test]
fn the_sample_window_shows_the_sample_mailbox_and_the_empty_one_nothing() {
    let sample = app(Mail::Sample, clock, None, Box::new(NoKeep)).unwrap();
    assert_eq!(sample.mailbox().accounts().len(), 2);
    assert_eq!(sample.mailbox().messages().len(), 11);
    let empty = app(owned(), clock, Some(std::sync::Arc::new(|| {})), Box::new(NoKeep)).unwrap();
    assert!(empty.mailbox().accounts().is_empty());
    assert_eq!(empty.mailbox().error(), None);
}

#[test]
fn only_the_empty_window_adds_accounts() {
    assert!(!app(Mail::Sample, clock, None, Box::new(NoKeep)).unwrap().can_add(), "the sample connects nowhere");
    assert!(app(owned(), clock, None, Box::new(NoKeep)).unwrap().can_add());
}

#[test]
fn both_windows_start_with_the_kept_appearance() {
    use inboxcraft_engine::appearance::{Appearance, Typeface};
    use inboxcraft_ui_egui::keep::{FILE, Keep, KeepFile};
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(FILE);
    let chosen = Appearance { font: Typeface::Inter, ..Appearance::default() };
    KeepFile::new(file.clone()).store(&chosen).unwrap();
    for source in [Source::Sample, Source::Owner] {
        let window = app(mail(source), clock, None, Box::new(KeepFile::new(file.clone()))).unwrap();
        assert_eq!(*window.appearance(), chosen, "{source:?}");
    }
}

#[test]
fn the_sample_acts_in_memory_writes_nothing_and_starts_no_account_worker() {
    use inboxcraft_ui_egui::keep::{FILE, KeepFile};
    use inboxcraft_ui_egui::mailbox::Act;
    use inboxcraft_ui_egui::workers::Scripted;
    let dir = tempfile::tempdir().unwrap();
    let mut window = app(Mail::Sample, clock, None, Box::new(KeepFile::new(dir.path().join(FILE)))).unwrap();
    let mut workers = Scripted::default();
    let ids: Vec<u64> = window.mailbox().messages().iter().map(|row| row.id).collect();
    for (id, act) in ids.into_iter().zip([Act::Read, Act::Unread, Act::Done, Act::Delete]) {
        let mailbox = window.mailbox_mut();
        mailbox.start(id, act);
        mailbox.left();
        assert!(mailbox.undoable().is_some(), "{act:?}");
        mailbox.pump(&mut workers);
        mailbox.undo();
        mailbox.pump(&mut workers);
        assert_eq!(mailbox.error(), None, "{act:?}");
    }
    let first = window.mailbox().messages()[0].id;
    window.mailbox_mut().act(first, Act::Done);
    window.mailbox_mut().pump(&mut workers);
    assert_eq!(window.mailbox().messages().len(), 10);
    assert!(workers.sent.is_empty(), "no job went to a worker: {:?}", workers.sent);
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0, "nothing was written");
    let again = app(Mail::Sample, clock, None, Box::new(NoKeep)).unwrap();
    assert_eq!(again.mailbox().messages().len(), 11, "the next start shows the sample as it was");
}

/// Set in the child run of [`child_opens_both_windows_with_the_owner_s_keep`]; without it that test does nothing.
const CHILD_VAR: &str = "INBOXCRAFT_APP_CHILD";

/// In a child run: opens both windows on [`owner_keep`] and prints the typeface each starts with as `font=<name>`.
#[test]
fn child_opens_both_windows_with_the_owner_s_keep() {
    if std::env::var_os(CHILD_VAR).is_none() {
        return;
    }
    for source in [Source::Sample, Source::Owner] {
        let window = app(mail(source), clock, None, owner_keep()).unwrap();
        println!("font={:?}", window.appearance().font);
    }
}

/// What a child run of this test binary prints, with only `vars` of the folder variables set.
fn child_with(vars: &[(&str, &str)]) -> Vec<String> {
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command.args(["tests::child_opens_both_windows_with_the_owner_s_keep", "--exact", "--nocapture", "--test-threads=1"]).env(CHILD_VAR, "1");
    for name in ["HOME", "USERPROFILE", "XDG_CONFIG_HOME", "XDG_DOWNLOAD_DIR", "APPDATA"] {
        command.env_remove(name);
    }
    command.envs(vars.iter().copied());
    let output = command.output().unwrap();
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout).unwrap().lines().filter_map(|line| line.rsplit_once("font=").map(|(_, font)| font.to_owned())).collect()
}

#[test]
fn the_owner_s_keep_is_the_file_in_the_config_folder_or_nothing_without_one() {
    use inboxcraft_engine::appearance::{Appearance, Typeface};
    use inboxcraft_ui_egui::keep::{FILE, Keep, KeepFile};
    let dir = tempfile::tempdir().unwrap();
    let chosen = Appearance { font: Typeface::Inter, ..Appearance::default() };
    KeepFile::new(dir.path().join("inboxcraft").join(FILE)).store(&chosen).unwrap();
    let inter = format!("{:?}", Typeface::Inter);
    assert_eq!(child_with(&[("XDG_CONFIG_HOME", dir.path().to_str().unwrap())]), [inter.clone(), inter.clone()]);
    // Without a config folder nothing is kept, and without a downloads folder the windows still open.
    let default = format!("{:?}", Appearance::default().font);
    assert_ne!(default, inter);
    assert_eq!(child_with(&[]), [default.clone(), default]);
}

fn first(dir: &std::path::Path) -> Instance {
    let mut err = Vec::new();
    let instance = arrive(dir, false, &mut err).unwrap();
    assert!(err.is_empty());
    instance
}

#[cfg(unix)]
#[test]
fn a_second_start_brings_the_running_window_forward_and_leaves() {
    let dir = tempfile::tempdir().unwrap();
    let (sent, heard) = std::sync::mpsc::channel();
    let _listening = first(dir.path()).listen(move |request| {
        let _ = sent.send(request);
    });
    let mut err = Vec::new();
    assert_eq!(arrive(dir.path(), false, &mut err).err(), Some(0));
    assert_eq!(heard.recv_timeout(std::time::Duration::from_secs(5)).unwrap(), Request::Show);
    // A start at login finds InboxCraft already running and leaves it alone.
    assert_eq!(arrive(dir.path(), true, &mut err).err(), Some(0));
    assert!(heard.try_recv().is_err());
    assert!(err.is_empty(), "{}", String::from_utf8_lossy(&err));
}

#[test]
fn a_running_start_that_cannot_be_reached_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    let _held = first(dir.path());
    let mut err = Vec::new();
    assert_eq!(arrive_with(dir.path(), false, std::time::Duration::from_millis(200), &mut err).err(), Some(1));
    assert!(String::from_utf8_lossy(&err).contains("already running"), "{}", String::from_utf8_lossy(&err));
}

#[test]
fn a_folder_that_cannot_be_claimed_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a file");
    std::fs::write(&file, b"").unwrap();
    let mut err = Vec::new();
    assert_eq!(arrive(&file, false, &mut err).err(), Some(1));
    assert!(String::from_utf8_lossy(&err).contains("a file"));
    let mut out = Vec::new();
    assert_eq!(quit(&file, &mut out, &mut err), 1);
}

#[cfg(unix)]
#[test]
fn quit_ends_the_running_inboxcraft() {
    let dir = tempfile::tempdir().unwrap();
    let (sent, heard) = std::sync::mpsc::channel();
    let _listening = first(dir.path()).listen(move |request| {
        let _ = sent.send(request);
    });
    let (mut out, mut err) = (Vec::new(), Vec::new());
    assert_eq!(quit(dir.path(), &mut out, &mut err), 0);
    assert_eq!(heard.recv_timeout(std::time::Duration::from_secs(5)).unwrap(), Request::Quit);
}

#[test]
fn quit_with_nothing_running_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    assert_eq!(quit(dir.path(), &mut out, &mut err), 1);
    assert!(String::from_utf8_lossy(&err).contains("not running"));
    assert!(matches!(instance::claim(dir.path()).unwrap(), instance::Claim::First(_)), "the folder is left free");
}

#[test]
fn autostart_writes_the_entry_and_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let program = std::path::Path::new("/usr/bin/inboxcraft");
    let (mut out, mut err) = (Vec::new(), Vec::new());
    assert_eq!(set_autostart(true, Some(dir.path()), program, None, &mut out, &mut err), 0);
    assert!(String::from_utf8_lossy(&out).contains("starts at login"));
    assert!(autostart::is_on(dir.path()));
    assert_eq!(set_autostart(false, Some(dir.path()), program, None, &mut out, &mut err), 0);
    assert!(!autostart::is_on(dir.path()));
    assert!(err.is_empty());
}

#[test]
fn autostart_without_a_config_folder_or_with_a_refusal_fails() {
    let program = std::path::Path::new("/usr/bin/inboxcraft");
    let (mut out, mut err) = (Vec::new(), Vec::new());
    assert_eq!(set_autostart(true, None, program, None, &mut out, &mut err), 1);
    assert!(String::from_utf8_lossy(&err).contains("no folder"), "{}", String::from_utf8_lossy(&err));
    let dir = tempfile::tempdir().unwrap();
    let refuse = |_: bool| Err::<autostart::Granted, String>("refused".to_owned());
    let mut err = Vec::new();
    assert_eq!(set_autostart(true, Some(dir.path()), program, Some(&refuse), &mut out, &mut err), 1);
    assert!(String::from_utf8_lossy(&err).contains("refused"));
}

#[test]
fn the_instance_folder_holds_the_store() {
    assert_eq!(instance_dir(), store::store_dir().and_then(|dir| dir.parent().map(std::path::Path::to_path_buf)));
}
