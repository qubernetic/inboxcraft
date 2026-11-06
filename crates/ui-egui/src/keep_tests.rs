// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use inboxcraft_engine::appearance::{Accent, Appearance, Icons, Motion, ThemeMode, Typeface};

use super::{FILE, Keep, KeepFile, MAX_FILE, NoKeep};

fn chosen() -> Appearance {
    Appearance {
        theme: ThemeMode::Dark,
        accent: Accent::Preset("citrus"),
        font: Typeface::Inter,
        icons: Icons::Lucide,
        text_scale: 120,
        motion: Motion::Reduce,
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
