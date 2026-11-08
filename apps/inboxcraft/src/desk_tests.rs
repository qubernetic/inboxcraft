// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use inboxcraft_ui_egui::desk::Desk;

use super::{FILE, MAX_FILE, OwnerDesk, Portal, kept};
use crate::autostart::{ENTRY_FILE, Granted};

/// The answer of `desk`, waited for up to a few seconds: the portal answers on a thread.
fn answer_of(desk: &mut OwnerDesk) -> Option<Result<(), String>> {
    for _ in 0..1000 {
        if let Some(answer) = desk.answer() {
            return Some(answer);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    None
}

fn desk(dir: &Path) -> (OwnerDesk, Arc<AtomicBool>) {
    let stays = Arc::new(AtomicBool::new(true));
    let desk = OwnerDesk::new(
        Arc::clone(&stays),
        Some(dir.join("inboxcraft").join(FILE)),
        Some(dir.join("autostart")),
        PathBuf::from("/usr/bin/inboxcraft"),
    );
    (desk, stays)
}

#[test]
fn running_in_the_background_is_on_until_the_owner_turns_it_off_and_the_choice_is_kept() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("inboxcraft").join(FILE);
    assert!(kept(&file), "on until chosen");
    let (mut desk, stays) = desk(dir.path());
    assert_eq!(desk.background(), Some(true));
    desk.set_background(false).unwrap();
    assert_eq!(desk.background(), Some(false));
    assert!(!stays.load(Ordering::SeqCst), "closing the window ends InboxCraft from now on");
    assert!(!kept(&file));
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.contains("\"background\": false"), "{text}");
    desk.set_background(true).unwrap();
    assert!(stays.load(Ordering::SeqCst));
    assert!(kept(&file));
}

#[test]
fn a_damaged_or_huge_choice_file_is_read_as_on() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join(FILE);
    for text in [&b"{ \"background\": \"no\" }"[..], b"\xff not json", b"[false]"] {
        std::fs::write(&file, text).unwrap();
        assert!(kept(&file), "{text:?}");
    }
    let mut huge = b"{ \"background\": false }".to_vec();
    huge.resize(MAX_FILE + 1, b' ');
    std::fs::write(&file, huge).unwrap();
    assert!(kept(&file), "a file larger than the choice is not read");
    std::fs::write(&file, b"{ \"background\": false }").unwrap();
    assert!(!kept(&file));
}

#[test]
fn a_choice_that_cannot_be_kept_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let blocker = dir.path().join("blocker");
    std::fs::write(&blocker, b"a file where the folder should be").unwrap();
    let stays = Arc::new(AtomicBool::new(true));
    let mut desk = OwnerDesk::new(Arc::clone(&stays), Some(blocker.join(FILE)), None, PathBuf::from("/usr/bin/inboxcraft"));
    let error = desk.set_background(false).unwrap_err();
    assert!(error.contains("could not be kept"), "{error}");
    assert!(stays.load(Ordering::SeqCst));
    assert_eq!(desk.background(), Some(true));
    // Without a file to keep it in, the choice lasts for this run.
    let mut desk = OwnerDesk::new(Arc::clone(&stays), None, None, PathBuf::from("/usr/bin/inboxcraft"));
    desk.set_background(false).unwrap();
    assert!(!stays.load(Ordering::SeqCst));
}

#[test]
fn a_run_nothing_could_bring_a_window_back_to_offers_no_background() {
    let dir = tempfile::tempdir().unwrap();
    let (desk, _stays) = desk(dir.path());
    let mut desk = desk.without_background();
    assert_eq!(desk.background(), None);
    assert!(desk.set_background(true).is_err());
}

#[test]
fn starting_with_the_computer_writes_and_removes_the_autostart_entry() {
    let dir = tempfile::tempdir().unwrap();
    let (mut desk, _stays) = desk(dir.path());
    assert_eq!(desk.login(), Some(false));
    assert!(!desk.asking());
    assert_eq!(desk.answer(), None, "nothing asked yet");
    desk.set_login(true);
    assert_eq!(answer_of(&mut desk), Some(Ok(())));
    assert_eq!(desk.answer(), None, "an answer is given once");
    assert_eq!(desk.login(), Some(true));
    let entry = std::fs::read_to_string(dir.path().join("autostart").join(ENTRY_FILE)).unwrap();
    assert!(entry.contains("Exec=\"/usr/bin/inboxcraft\" --background"), "{entry}");
    desk.set_login(false);
    assert_eq!(answer_of(&mut desk), Some(Ok(())));
    assert_eq!(desk.login(), Some(false));
    assert!(!dir.path().join("autostart").join(ENTRY_FILE).exists());
}

#[test]
fn an_entry_that_was_there_at_the_start_shows_as_on() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("autostart")).unwrap();
    std::fs::write(dir.path().join("autostart").join(ENTRY_FILE), b"[Desktop Entry]\n").unwrap();
    let (desk, _stays) = desk(dir.path());
    assert_eq!(desk.login(), Some(true));
}

#[test]
fn without_an_autostart_folder_there_is_no_starting_with_the_computer() {
    let stays = Arc::new(AtomicBool::new(true));
    let mut desk = OwnerDesk::new(stays, None, None, PathBuf::from("/usr/bin/inboxcraft"));
    assert_eq!(desk.login(), None);
    desk.set_login(true);
    let answer = answer_of(&mut desk).unwrap().unwrap_err();
    assert!(answer.contains("no folder for the autostart entry"), "{answer}");
}

#[test]
fn an_entry_that_cannot_be_written_says_why_and_shows_what_is_there() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("autostart"), b"a file where the folder should be").unwrap();
    let (mut desk, _stays) = desk(dir.path());
    desk.set_login(true);
    let answer = answer_of(&mut desk).unwrap().unwrap_err();
    assert!(answer.contains("could not be changed"), "{answer}");
    assert_eq!(desk.login(), Some(false));
}

/// A portal that waits for the test's word before it answers, and counts its questions.
fn held_portal(answer: Result<Granted, String>) -> (Portal, mpsc::Sender<()>, Arc<AtomicUsize>) {
    let (go, wait) = mpsc::channel::<()>();
    let wait = Mutex::new(wait);
    let asked = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&asked);
    let portal: Portal = Arc::new(move |_on| {
        counted.fetch_add(1, Ordering::SeqCst);
        let _ = wait.lock().unwrap().recv_timeout(Duration::from_secs(5));
        answer.clone()
    });
    (portal, go, asked)
}

#[test]
fn in_the_flatpak_the_portal_is_asked_on_a_thread_and_the_window_is_woken_by_its_answer() {
    let dir = tempfile::tempdir().unwrap();
    let (portal, go, asked) = held_portal(Ok(Granted { background: true, autostart: true }));
    let woken = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&woken);
    let (desk, _stays) = desk(dir.path());
    let mut desk = desk.with_portal(
        portal,
        Arc::new(move || {
            counted.fetch_add(1, Ordering::SeqCst);
        }),
    );
    desk.set_login(true);
    assert!(desk.asking(), "the desktop may ask the person first");
    assert_eq!(desk.answer(), None);
    desk.set_login(false);
    go.send(()).unwrap();
    assert_eq!(answer_of(&mut desk), Some(Ok(())));
    assert!(!desk.asking());
    assert_eq!(asked.load(Ordering::SeqCst), 1, "one question at a time: the second was not asked");
    assert_eq!(woken.load(Ordering::SeqCst), 1);
    assert_eq!(desk.login(), Some(true));
    assert!(dir.path().join("autostart").join(ENTRY_FILE).exists(), "the choice is kept beside the portal's");
}

#[test]
fn a_portal_that_refuses_leaves_the_choice_as_it_was() {
    let dir = tempfile::tempdir().unwrap();
    let (portal, go, _asked) = held_portal(Ok(Granted { background: true, autostart: false }));
    let (desk, _stays) = desk(dir.path());
    let mut desk = desk.with_portal(portal, Arc::new(|| {}));
    desk.set_login(true);
    go.send(()).unwrap();
    let answer = answer_of(&mut desk).unwrap().unwrap_err();
    assert!(answer.contains("did not allow"), "{answer}");
    assert_eq!(desk.login(), Some(false));
    assert!(!dir.path().join("autostart").join(ENTRY_FILE).exists());
}

#[test]
fn a_portal_thread_that_dies_is_an_answer_too() {
    let dir = tempfile::tempdir().unwrap();
    let portal: Portal = Arc::new(|_on| std::panic::panic_any("the portal broke"));
    let (desk, _stays) = desk(dir.path());
    let mut desk = desk.with_portal(portal, Arc::new(|| {}));
    desk.set_login(true);
    let answer = answer_of(&mut desk).unwrap().unwrap_err();
    assert!(answer.contains("did not answer"), "{answer}");
    assert!(!desk.asking());
}

#[test]
fn the_debug_form_names_the_choices() {
    let dir = tempfile::tempdir().unwrap();
    let (desk, _stays) = desk(dir.path());
    let text = format!("{desk:?}");
    assert!(text.contains("OwnerDesk") && text.contains("login"), "{text}");
}
