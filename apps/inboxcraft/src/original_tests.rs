// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The private copies and their hand-off, with a fake desktop: nothing here opens a browser or reaches the owner's
//! desktop bus.

use std::fs;
use std::os::unix::fs::{PermissionsExt as _, symlink};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use inboxcraft_ui_egui::original::Original as _;

use super::{CACHE_DIR, DIR, Hand, KEEP_FOR, MAX_FILES, Originals, folder_from, name, sweep_folder, xdg_open};

/// What the fake desktop was handed, and whether it refuses (a desktop with no browser).
#[derive(Clone, Default)]
struct Fake {
    handed: Arc<Mutex<Vec<PathBuf>>>,
    refuses: bool,
}

impl Hand for Fake {
    fn hand(&mut self, file: &Path) -> Result<(), String> {
        if self.refuses {
            return Err("no browser is set up".to_owned());
        }
        self.handed.lock().unwrap().push(file.to_path_buf());
        Ok(())
    }
}

/// A temporary home with the private folder in it, not yet made.
fn place() -> (tempfile::TempDir, PathBuf) {
    let home = tempfile::tempdir().unwrap();
    let folder = home.path().join(DIR);
    (home, folder)
}

fn mode(path: &Path) -> u32 {
    fs::symlink_metadata(path).unwrap().permissions().mode() & 0o777
}

/// A page whose title says the worst a message can name itself.
fn hostile_page() -> String {
    format!("<!DOCTYPE html><title>../../etc/passwd\0{}</title>", "x".repeat(10_000))
}

#[test]
fn the_copy_is_a_private_file_named_from_the_message_number_alone() {
    let (_home, folder) = place();
    let fake = Fake::default();
    let mut originals = Originals::new(Some(folder.clone()), Box::new(fake.clone()));
    let page = hostile_page();
    originals.open(7, &page).unwrap();
    let handed = fake.handed.lock().unwrap().clone();
    assert_eq!(handed, vec![folder.join("original-7.html")], "nothing of the message's text in the path");
    assert_eq!(name(u64::MAX), "original-18446744073709551615.html");
    let file = handed.first().unwrap();
    assert_eq!(fs::read_to_string(file).unwrap(), page);
    assert_eq!(mode(file), 0o600, "the file is the owner's alone");
    assert_eq!(mode(&folder), 0o700, "so is its folder");
    assert!(fs::symlink_metadata(file).unwrap().file_type().is_file());
}

#[test]
fn opening_it_again_writes_it_afresh() {
    let (_home, folder) = place();
    let mut originals = Originals::new(Some(folder.clone()), Box::new(Fake::default()));
    originals.open(3, "first").unwrap();
    originals.open(3, "second").unwrap();
    assert_eq!(fs::read_to_string(folder.join(name(3))).unwrap(), "second");
}

#[test]
fn a_planted_symlink_at_the_name_is_not_followed() {
    let (home, folder) = place();
    fs::create_dir(&folder).unwrap();
    fs::set_permissions(&folder, fs::Permissions::from_mode(0o700)).unwrap();
    let target = home.path().join("elsewhere");
    fs::write(&target, "the owner's own file").unwrap();
    symlink(&target, folder.join(name(5))).unwrap();
    let mut originals = Originals::new(Some(folder.clone()), Box::new(Fake::default()));
    originals.open(5, "<p>copy</p>").unwrap();
    assert_eq!(fs::read_to_string(&target).unwrap(), "the owner's own file", "the link's target is untouched");
    let written = folder.join(name(5));
    assert!(fs::symlink_metadata(&written).unwrap().file_type().is_file(), "the link itself gave way to a file");
    assert_eq!(fs::read_to_string(&written).unwrap(), "<p>copy</p>");
}

#[test]
fn a_folder_that_is_a_link_or_open_to_others_is_refused() {
    let (home, folder) = place();
    let real = home.path().join("real");
    fs::create_dir(&real).unwrap();
    fs::set_permissions(&real, fs::Permissions::from_mode(0o700)).unwrap();
    symlink(&real, &folder).unwrap();
    let fake = Fake::default();
    let mut originals = Originals::new(Some(folder.clone()), Box::new(fake.clone()));
    let error = originals.open(1, "page").unwrap_err();
    assert!(error.contains("not a folder"), "{error}");
    assert_eq!(fs::read_dir(&real).unwrap().count(), 0, "nothing written through the link");

    let open = home.path().join("open");
    fs::create_dir(&open).unwrap();
    fs::set_permissions(&open, fs::Permissions::from_mode(0o755)).unwrap();
    let mut originals = Originals::new(Some(open.clone()), Box::new(fake.clone()));
    let error = originals.open(1, "page").unwrap_err();
    assert!(error.contains("open to others"), "{error}");
    assert_eq!(fs::read_dir(&open).unwrap().count(), 0);
    assert!(fake.handed.lock().unwrap().is_empty(), "nothing handed on");
}

#[test]
fn without_a_place_to_write_it_says_so_and_hands_nothing_on() {
    assert_eq!(folder_from(None, None, None), None);
    assert_eq!(folder_from(Some("relative".into()), Some("also/relative".into()), Some("".into())), None, "only absolute folders");
    assert_eq!(folder_from(Some("/run/user/1000".into()), None, None), Some(Path::new("/run/user/1000").join(DIR)));
    assert_eq!(folder_from(None, Some("/c".into()), Some("/h".into())), Some(Path::new("/c").join(CACHE_DIR)));
    assert_eq!(folder_from(None, None, Some("/h".into())), Some(Path::new("/h/.cache").join(CACHE_DIR)));
    let fake = Fake::default();
    let mut originals = Originals::new(None, Box::new(fake.clone()));
    let error = originals.open(1, "page").unwrap_err();
    assert!(error.contains("no private folder"), "{error}");
    assert!(fake.handed.lock().unwrap().is_empty());
}

#[test]
fn a_desktop_with_no_browser_is_an_error_with_its_reason() {
    let (_home, folder) = place();
    let mut originals = Originals::new(Some(folder), Box::new(Fake { refuses: true, ..Fake::default() }));
    assert_eq!(originals.open(1, "page").unwrap_err(), "no browser is set up");
}

#[test]
fn old_copies_go_and_no_more_than_the_most_are_kept() {
    let (_home, folder) = place();
    let mut originals = Originals::new(Some(folder.clone()), Box::new(Fake::default()));
    originals.open(1, "page").unwrap();
    let old = super::wall_now() - KEEP_FOR - Duration::from_secs(1);
    fs::File::options().write(true).open(folder.join(name(1))).unwrap().set_modified(old).unwrap();
    fs::write(folder.join("notes.txt"), "not a copy").unwrap();
    fs::write(folder.join("original-x.html"), "not a copy either").unwrap();
    for id in 100..100 + MAX_FILES as u64 + 4 {
        originals.open(id, "page").unwrap();
    }
    assert!(!folder.join(name(1)).exists(), "an old copy went");
    let copies = fs::read_dir(&folder).unwrap().filter(|entry| entry.as_ref().unwrap().file_name().to_string_lossy().ends_with(".html")).count();
    assert_eq!(copies, MAX_FILES + 1, "the most kept, besides the file that only looks like one");
    assert!(folder.join("notes.txt").exists() && folder.join("original-x.html").exists(), "only copies are touched");

    sweep_folder(&folder);
    let left: Vec<String> = fs::read_dir(&folder).unwrap().map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(left.len(), 2, "the start removes every copy: {left:?}");
}

#[test]
fn xdg_open_that_fails_or_is_missing_is_an_error() {
    // `true` and `false` stand in for `xdg-open`: nothing is opened.
    let file = Path::new("/nonexistent/original-1.html");
    assert_eq!(xdg_open(Path::new("true"), file, Duration::from_secs(5)), Ok(()));
    let error = xdg_open(Path::new("false"), file, Duration::from_secs(5)).unwrap_err();
    assert!(error.contains("no browser"), "{error}");
    let error = xdg_open(Path::new("/nonexistent/xdg-open"), file, Duration::from_secs(5)).unwrap_err();
    assert!(error.contains("no program"), "{error}");
    // One still running when the patience ends has started the browser.
    assert_eq!(xdg_open(Path::new("sleep"), Path::new("1"), Duration::from_millis(50)), Ok(()));
}

#[cfg(target_os = "linux")]
mod portal {
    use std::collections::HashMap;
    use std::io::Read as _;
    use std::sync::{Arc, Mutex};

    use zbus::zvariant::{OwnedFd, OwnedObjectPath, OwnedValue};

    use super::super::portal;
    use crate::bus_tests::PrivateBus;

    /// A portal that reads what it was handed.
    struct Fake {
        read: Arc<Mutex<Vec<(String, bool)>>>,
    }

    #[zbus::interface(name = "org.freedesktop.portal.OpenURI")]
    impl Fake {
        fn open_file(&self, _parent_window: &str, fd: OwnedFd, options: HashMap<String, OwnedValue>) -> OwnedObjectPath {
            let mut text = String::new();
            std::fs::File::from(std::os::fd::OwnedFd::from(fd)).read_to_string(&mut text).unwrap();
            let writable = options.get("writable").and_then(|value| value.downcast_ref::<bool>().ok()).unwrap_or(true);
            self.read.lock().unwrap().push((text, writable));
            OwnedObjectPath::try_from("/org/freedesktop/portal/desktop/request/x/y").unwrap()
        }
    }

    #[test]
    fn the_portal_gets_the_file_itself_read_only() {
        let Some(bus) = PrivateBus::start() else { return };
        let read = Arc::new(Mutex::new(Vec::new()));
        let _server = zbus::blocking::connection::Builder::address(bus.address())
            .unwrap()
            .name(portal::DESTINATION)
            .unwrap()
            .serve_at(portal::PATH, Fake { read: Arc::clone(&read) })
            .unwrap()
            .build()
            .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("original-1.html");
        std::fs::write(&file, "<p>copy</p>").unwrap();
        portal::open_file(&bus.connect(), &file).unwrap();
        assert_eq!(*read.lock().unwrap(), vec![("<p>copy</p>".to_owned(), false)]);
    }

    #[test]
    fn no_portal_is_an_error() {
        let Some(bus) = PrivateBus::start() else { return };
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("original-1.html");
        std::fs::write(&file, "page").unwrap();
        let error = portal::open_file(&bus.connect(), &file).unwrap_err();
        assert!(error.contains("portal"), "{error}");
    }
}
