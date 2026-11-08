// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use inboxcraft_engine::appearance::{Accent, Appearance, Icons, Motion, ThemeMode, Typeface, WindowFrame};

use super::{FILE, KEYS_FILE, Keep, KeepFile, MAX_FILE, NoKeep};

fn chosen() -> Appearance {
    Appearance {
        theme: ThemeMode::Dark,
        accent: Accent::Preset("citrus"),
        font: Typeface::Inter,
        icons: Icons::Lucide,
        text_scale: 120,
        motion: Motion::Reduce,
        frame: WindowFrame::TopRow,
    }
}

#[test]
fn a_kept_appearance_comes_back_from_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested").join(FILE);
    let mut keep = KeepFile::new(path.clone());
    assert_eq!(keep.load(), None, "nothing kept yet");
    keep.store(&chosen()).unwrap();
    assert_eq!(KeepFile::new(path.clone()).load(), Some(chosen()));
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("\"font\": \"inter\""), "{text}");
    // Storing again replaces the file and leaves no temporary file behind.
    keep.store(&Appearance::default()).unwrap();
    assert_eq!(keep.load(), Some(Appearance::default()));
    let left: Vec<_> = std::fs::read_dir(path.parent().unwrap()).unwrap().map(|entry| entry.unwrap().file_name()).collect();
    assert_eq!(left, vec![std::ffi::OsString::from(FILE)]);
}

#[test]
fn a_damaged_or_huge_file_gives_what_can_be_read_or_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join(FILE);
    std::fs::write(&path, b"{ \"theme\": \"light\", \"font\": 3 }").unwrap();
    assert_eq!(KeepFile::new(path.clone()).load(), Some(Appearance { theme: ThemeMode::Light, ..Appearance::default() }));
    std::fs::write(&path, b"\xff\xfe not json").unwrap();
    assert_eq!(KeepFile::new(path.clone()).load(), None);
    std::fs::write(&path, vec![b' '; MAX_FILE + 1]).unwrap();
    assert_eq!(KeepFile::new(path.clone()).load(), None, "a file larger than any appearance is not read");
    std::fs::create_dir(dir.path().join("dir")).unwrap();
    assert_eq!(KeepFile::new(dir.path().join("dir")).load(), None);
}

#[test]
fn a_file_that_cannot_be_written_is_an_error_not_a_panic() {
    let dir = tempfile::tempdir().unwrap();
    let blocker = dir.path().join("blocker");
    std::fs::write(&blocker, b"a file where the folder should be").unwrap();
    let mut keep = KeepFile::new(blocker.join(FILE));
    let error = keep.store(&chosen()).unwrap_err();
    assert!(!error.is_empty());
}

#[test]
fn the_owner_s_file_is_in_the_config_folder() {
    let keep = KeepFile::of_owner();
    if let Some(keep) = keep {
        assert!(keep.path().is_absolute());
        assert!(keep.path().ends_with(std::path::Path::new("inboxcraft").join(FILE)), "{}", keep.path().display());
    }
}

#[test]
fn no_keep_keeps_nothing() {
    let mut keep = NoKeep;
    keep.store(&chosen()).unwrap();
    assert_eq!(keep.load(), None);
}

#[test]
fn a_file_that_cannot_take_the_place_of_the_old_one_leaves_no_temporary_file() {
    let dir = tempfile::tempdir().unwrap();
    // A folder with something in it where the file should be: the temporary file is written, the move fails.
    let path = dir.path().join(FILE);
    std::fs::create_dir(&path).unwrap();
    std::fs::write(path.join("inside"), b"kept").unwrap();
    let error = KeepFile::new(path.clone()).store(&chosen()).unwrap_err();
    assert!(error.starts_with(&path.display().to_string()), "{error}");
    let left: Vec<_> = std::fs::read_dir(dir.path()).unwrap().map(|entry| entry.unwrap().file_name()).collect();
    assert_eq!(left, vec![std::ffi::OsString::from(FILE)], "the temporary file is gone");
    // A path with no folder and no name: nothing is created, and the refusal is an error.
    assert!(KeepFile::new(std::path::PathBuf::new()).store(&chosen()).is_err());
}

/// Set in the child run of [`child_prints_the_owner_s_file`]; without it that test does nothing.
const CHILD_VAR: &str = "INBOXCRAFT_KEEP_CHILD";

/// In a child run: prints the owner's file as `file=<path>`, or `file=none`.
#[test]
fn child_prints_the_owner_s_file() {
    if std::env::var_os(CHILD_VAR).is_none() {
        return;
    }
    match KeepFile::of_owner() {
        Some(keep) => println!("file={}", keep.path().display()),
        None => println!("file=none"),
    }
}

/// The owner's file as a child run of this test binary sees it, with only `vars` of the folder variables set.
fn owner_file_with(vars: &[(&str, &str)]) -> String {
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command.args(["keep::tests::child_prints_the_owner_s_file", "--exact", "--nocapture", "--test-threads=1"]).env(CHILD_VAR, "1");
    for name in ["HOME", "USERPROFILE", "XDG_CONFIG_HOME", "APPDATA"] {
        command.env_remove(name);
    }
    command.envs(vars.iter().copied());
    let output = command.output().unwrap();
    assert!(output.status.success(), "{output:?}");
    let out = String::from_utf8(output.stdout).unwrap();
    out.lines().find_map(|line| line.rsplit_once("file=").map(|(_, file)| file.to_owned())).unwrap_or_else(|| panic!("no file line: {out}"))
}

#[cfg(all(unix, not(target_os = "macos")))]
#[test]
fn the_owner_s_file_follows_xdg_config_home_then_home_and_is_none_without_either() {
    let dir = tempfile::tempdir().unwrap();
    let dir_text = dir.path().to_str().unwrap();
    let config = dir.path().join("config");
    let expected = config.join("inboxcraft").join(FILE).display().to_string();
    assert_eq!(owner_file_with(&[("XDG_CONFIG_HOME", config.to_str().unwrap()), ("HOME", "relative/home")]), expected);
    let home = dir.path().join(".config").join("inboxcraft").join(FILE).display().to_string();
    assert_eq!(owner_file_with(&[("XDG_CONFIG_HOME", "relative/config"), ("HOME", dir_text)]), home, "a relative XDG_CONFIG_HOME is passed over");
    assert_eq!(owner_file_with(&[("USERPROFILE", dir_text)]), home);
    assert_eq!(owner_file_with(&[("HOME", "relative/home")]), "none");
    assert_eq!(owner_file_with(&[]), "none");
}

#[test]
fn the_keys_are_kept_beside_the_appearance_and_come_back() {
    use egui::{Key, KeyboardShortcut, Modifiers};

    use crate::keymap::{self, Keymap};

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested").join(FILE);
    let mut keep = KeepFile::new(path.clone());
    assert_eq!(keep.load_keys(), None, "nothing kept yet");
    let mut keymap = Keymap::default();
    keymap.bind(keymap::SENT, KeyboardShortcut::new(Modifiers::COMMAND, Key::Num3));
    keymap.unbind(keymap::DONE);
    keep.store_keys(&keymap).unwrap();
    assert_eq!(KeepFile::new(path.clone()).load_keys(), Some(keymap.clone()));
    let kept = dir.path().join("nested").join(KEYS_FILE);
    let text = std::fs::read_to_string(&kept).unwrap();
    assert!(text.contains("\"ui.sent.show\": \"Command+3\""), "{text}");
    assert_eq!(keep.load(), None, "the appearance file is another file");
    // Damaged, huge and wrong: the defaults or nothing.
    std::fs::write(&kept, b"{ \"keys\": { \"ui.sent.show\": 3 } }").unwrap();
    assert_eq!(keep.load_keys(), Some(Keymap::default()));
    std::fs::write(&kept, b"\xff not json").unwrap();
    assert_eq!(keep.load_keys(), None);
    std::fs::write(&kept, vec![b' '; MAX_FILE + 1]).unwrap();
    assert_eq!(keep.load_keys(), None);
    // NoKeep keeps no key either.
    let mut none = NoKeep;
    none.store_keys(&keymap).unwrap();
    assert_eq!(none.load_keys(), None);
    // A keys file that cannot be written is an error.
    let blocker = dir.path().join("blocker");
    std::fs::write(&blocker, b"a file where the folder should be").unwrap();
    assert!(KeepFile::new(blocker.join(FILE)).store_keys(&keymap).is_err());
    // A path that names no file has no keys' file beside it.
    assert!(KeepFile::new(std::path::PathBuf::new()).store_keys(&keymap).is_err());
    assert_eq!(KeepFile::new(std::path::PathBuf::new()).load_keys(), None);
}

#[test]
fn the_triage_choices_are_kept_beside_the_appearance_and_come_back() {
    use inboxcraft_engine::ScreenerMode;

    use super::{TRIAGE_FILE, Triage};
    use crate::mailbox::InboxView;

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested").join(FILE);
    let mut keep = KeepFile::new(path.clone());
    assert_eq!(keep.load_triage(), None, "nothing kept yet");
    let chosen = Triage { screener: ScreenerMode::Inside, view: InboxView::List, keep_pins: true, hold: Some(30) };
    keep.store_triage(&chosen).unwrap();
    assert_eq!(KeepFile::new(path.clone()).load_triage(), Some(chosen));
    let kept = dir.path().join("nested").join(TRIAGE_FILE);
    let text = std::fs::read_to_string(&kept).unwrap();
    assert!(text.contains("\"screener\": \"inside\"") && text.contains("\"view\": \"list\"") && text.contains("\"keep_pins\": true"), "{text}");
    assert!(text.contains("\"hold_seconds\": 30"), "{text}");
    // Damaged, huge and wrong: the defaults or nothing.
    std::fs::write(&kept, b"{ \"screener\": 3, \"view\": \"tiles\", \"keep_pins\": \"yes\", \"hold_seconds\": 600 }").unwrap();
    assert_eq!(keep.load_triage(), Some(Triage::default()));
    assert_eq!(Triage::default(), Triage { screener: ScreenerMode::Off, view: InboxView::Cards, keep_pins: false, hold: None });
    std::fs::write(&kept, b"\xff not json").unwrap();
    assert_eq!(keep.load_triage(), None);
    std::fs::write(&kept, vec![b' '; MAX_FILE + 1]).unwrap();
    assert_eq!(keep.load_triage(), None);
    let mut none = NoKeep;
    none.store_triage(&chosen).unwrap();
    assert_eq!(none.load_triage(), None);
    let blocker = dir.path().join("blocker");
    std::fs::write(&blocker, b"a file where the folder should be").unwrap();
    assert!(KeepFile::new(blocker.join(FILE)).store_triage(&chosen).is_err());
    assert!(KeepFile::new(std::path::PathBuf::from("/")).store_triage(&chosen).is_err());
    assert_eq!(KeepFile::new(std::path::PathBuf::from("/")).load_triage(), None);
}
