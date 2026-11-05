// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::PathBuf;

use super::{Downloads, MAX_NAME, NoSave, Save, safe_name};

/// An empty folder of the test's own under the system's temporary folder, removed when dropped.
struct Folder(PathBuf);

impl Folder {
    fn new(test: &str) -> Self {
        let path = std::env::temp_dir().join(format!("inboxcraft-save-{test}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn a_file_name_from_a_sender_becomes_one_plain_name() {
    assert_eq!(safe_name("rota.csv"), "rota.csv");
    assert_eq!(safe_name("../../.bashrc"), "bashrc");
    assert_eq!(safe_name("C:\\Windows\\system32\\evil.dll"), "evil.dll");
    assert_eq!(safe_name("/etc/passwd"), "passwd");
    assert_eq!(safe_name("invoice\u{202e}fdp.exe"), "invoicefdp.exe");
    assert_eq!(safe_name("a\r\nb\u{0}c.txt"), "abc.txt");
    assert_eq!(safe_name("what?<>:\"|*.txt"), "what.txt");
    assert_eq!(safe_name(" . "), "attachment");
    assert_eq!(safe_name(""), "attachment");
    assert_eq!(safe_name("..."), "attachment");
    assert_eq!(safe_name("CON.txt"), "_CON.txt");
    assert_eq!(safe_name("nul"), "_nul");
    let long = format!("{}.pdf", "é".repeat(500));
    let cut = safe_name(&long);
    assert_eq!(cut.chars().count(), MAX_NAME);
    assert!(cut.ends_with(".pdf"));
    let no_extension = "x".repeat(500);
    assert_eq!(safe_name(&no_extension).chars().count(), MAX_NAME);
    let odd_extension = format!("{}.{}", "x".repeat(100), "y".repeat(100));
    assert_eq!(safe_name(&odd_extension).chars().count(), MAX_NAME);
}

#[test]
fn saving_never_overwrites_a_file_that_is_there() {
    let folder = Folder::new("overwrite");
    let mut downloads = Downloads::new(folder.0.clone());
    assert_eq!(downloads.folder(), folder.0.as_path());
    let first = downloads.save("rota.csv", b"one").unwrap();
    let second = downloads.save("rota.csv", b"two").unwrap();
    let third = downloads.save("../rota.csv", b"three").unwrap();
    assert!(first.ends_with("rota.csv"), "{first}");
    assert!(second.ends_with("rota (2).csv"), "{second}");
    assert!(third.ends_with("rota (3).csv"), "{third}");
    assert_eq!(std::fs::read(folder.0.join("rota.csv")).unwrap(), b"one");
    assert_eq!(std::fs::read(folder.0.join("rota (2).csv")).unwrap(), b"two");
    let plain = downloads.save("README", b"x").unwrap();
    let again = downloads.save("README", b"y").unwrap();
    assert!(plain.ends_with("README") && again.ends_with("README (2)"), "{plain} {again}");
    assert_eq!(std::fs::read_dir(&folder.0).unwrap().count(), 5, "nothing outside the names saved");
}

#[test]
fn saving_into_a_folder_that_is_not_there_says_so() {
    let folder = Folder::new("missing");
    let mut downloads = Downloads::new(folder.0.join("not-there"));
    let error = downloads.save("rota.csv", b"one").unwrap_err();
    assert!(error.starts_with("rota.csv could not be written"), "{error}");
}

#[test]
fn a_window_without_a_folder_saves_nothing() {
    assert!(NoSave.save("rota.csv", b"one").unwrap_err().contains("not available"));
}

#[test]
fn the_owner_has_a_downloads_folder() {
    // HOME is set in the test environment; the folder is not created by asking.
    let downloads = Downloads::of_owner().unwrap();
    assert!(downloads.folder().is_absolute());
}
