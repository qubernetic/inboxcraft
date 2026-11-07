// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::cell::RefCell;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::{ENTRY_FILE, Granted, autostart_dir_from, choose, entry, exec_line, is_on};

#[test]
fn the_entry_starts_this_program_in_the_background() {
    let text = entry(Path::new("/usr/bin/inboxcraft")).unwrap();
    assert!(text.starts_with("[Desktop Entry]\n"), "{text}");
    for line in [
        "Type=Application",
        "Name=InboxCraft",
        "Exec=\"/usr/bin/inboxcraft\" --background",
        "Icon=io.github.qubernetic.inboxcraft",
        "X-GNOME-Autostart-enabled=true",
    ] {
        assert!(text.lines().any(|have| have == line), "{line} in {text}");
    }
    assert!(text.ends_with('\n'));
}

#[test]
fn the_program_path_is_quoted_as_desktop_entries_want() {
    assert_eq!(exec_line(Path::new("/opt/Inbox Craft/inboxcraft")).unwrap(), "\"/opt/Inbox Craft/inboxcraft\" --background");
    // Inside the quotes `"`, `` ` ``, `$` and `\` take a backslash, which the entry's own escaping doubles; `%` is
    // doubled for the field codes.
    assert_eq!(exec_line(Path::new("/a\"b`c$d\\e%f")).unwrap(), "\"/a\\\\\"b\\\\`c\\\\$d\\\\\\\\e%%f\" --background");
    assert_eq!(exec_line(Path::new("inboxcraft")).unwrap(), "\"inboxcraft\" --background", "a bare name is looked up on PATH");
    for bad in ["/a\nb", "/a\rb", "/a\tb", "/a\u{7f}b", "relative/inboxcraft", ""] {
        assert_eq!(exec_line(Path::new(bad)), None, "{bad:?}");
    }
}

#[cfg(unix)]
#[test]
fn a_program_path_that_is_not_text_has_no_entry() {
    use std::os::unix::ffi::OsStringExt;
    let path = PathBuf::from(OsString::from_vec(b"/opt/\xff/inboxcraft".to_vec()));
    assert_eq!(entry(&path), None);
}

#[test]
fn the_autostart_folder_is_in_the_config_folder() {
    let os = |text: &str| Some(OsString::from(text));
    assert_eq!(autostart_dir_from(os("/c"), os("/h")), Some(PathBuf::from("/c/autostart")));
    assert_eq!(autostart_dir_from(None, os("/h")), Some(PathBuf::from("/h/.config/autostart")));
    assert_eq!(autostart_dir_from(os("relative"), os("/h")), Some(PathBuf::from("/h/.config/autostart")), "a relative XDG_CONFIG_HOME is ignored");
    assert_eq!(autostart_dir_from(None, os("relative")), None);
    assert_eq!(autostart_dir_from(None, None), None);
}

/// A portal that writes down what it was asked and answers `answer`.
struct Portal {
    asked: RefCell<Vec<bool>>,
    answer: Result<Granted, String>,
}

impl Portal {
    fn answering(answer: Result<Granted, String>) -> Self {
        Self { asked: RefCell::new(Vec::new()), answer }
    }

    fn ask(&self, autostart: bool) -> Result<Granted, String> {
        self.asked.borrow_mut().push(autostart);
        self.answer.clone()
    }
}

#[test]
fn outside_the_flatpak_the_choice_is_the_entry_in_the_autostart_folder() {
    let dir = tempfile::tempdir().unwrap();
    let autostart = dir.path().join("autostart");
    let program = Path::new("/usr/bin/inboxcraft");
    assert!(!is_on(&autostart));
    let said = choose(true, &autostart, program, None).unwrap();
    assert!(said.contains("starts at login"), "{said}");
    assert_eq!(std::fs::read_to_string(autostart.join(ENTRY_FILE)).unwrap(), entry(program).unwrap());
    assert!(is_on(&autostart));
    choose(true, &autostart, program, None).unwrap();
    let said = choose(false, &autostart, program, None).unwrap();
    assert!(said.contains("no longer"), "{said}");
    assert!(!is_on(&autostart));
    choose(false, &autostart, program, None).unwrap();
}

#[test]
fn in_the_flatpak_the_portal_is_asked_and_the_entry_kept_as_the_record() {
    let dir = tempfile::tempdir().unwrap();
    let granted = Portal::answering(Ok(Granted { background: true, autostart: true }));
    choose(true, dir.path(), Path::new("inboxcraft"), Some(&|on| granted.ask(on))).unwrap();
    assert_eq!(*granted.asked.borrow(), [true]);
    assert!(is_on(dir.path()));
    choose(false, dir.path(), Path::new("inboxcraft"), Some(&|on| granted.ask(on))).unwrap();
    assert_eq!(*granted.asked.borrow(), [true, false]);
    assert!(!is_on(dir.path()));
}

#[test]
fn a_portal_that_refuses_leaves_the_choice_as_it_was() {
    let dir = tempfile::tempdir().unwrap();
    let refused = Portal::answering(Ok(Granted { background: true, autostart: false }));
    let error = choose(true, dir.path(), Path::new("inboxcraft"), Some(&|on| refused.ask(on))).unwrap_err();
    assert!(error.contains("did not allow"), "{error}");
    assert!(!is_on(dir.path()));
    let gone = Portal::answering(Err("no portal".to_owned()));
    let error = choose(true, dir.path(), Path::new("inboxcraft"), Some(&|on| gone.ask(on))).unwrap_err();
    assert!(error.contains("no portal"), "{error}");
    assert!(!is_on(dir.path()));
}

#[test]
fn an_autostart_folder_that_cannot_be_written_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a file");
    std::fs::write(&file, b"").unwrap();
    let error = choose(true, &file, Path::new("/usr/bin/inboxcraft"), None).unwrap_err();
    assert!(error.contains("a file"), "{error}");
    let error = choose(true, dir.path(), Path::new("relative/inboxcraft"), None).unwrap_err();
    assert!(error.contains("relative/inboxcraft"), "{error}");
    assert!(!is_on(dir.path()));
}

#[cfg(target_os = "linux")]
mod portal {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use zbus::message::Header;
    use zbus::object_server::SignalEmitter;
    use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Value};

    use super::super::{Granted, portal};
    use crate::bus_tests::PrivateBus;

    /// What the fake portal was asked: autostart, the command line and the reason.
    type Asked = Arc<Mutex<Vec<(bool, Vec<String>, String)>>>;

    /// The Background portal as xdg-desktop-portal serves it: it answers on the request object the token names.
    struct Fake {
        asked: Asked,
        response: u32,
        autostart: Option<bool>,
    }

    #[zbus::interface(name = "org.freedesktop.portal.Background")]
    impl Fake {
        async fn request_background(
            &self,
            #[zbus(header)] header: Header<'_>,
            #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
            _parent_window: &str,
            options: HashMap<String, OwnedValue>,
        ) -> zbus::fdo::Result<OwnedObjectPath> {
            let get = |key: &str| options.get(key).map(|value| Value::from(value.try_clone().unwrap()));
            let token: String = get("handle_token").unwrap().try_into().unwrap();
            let autostart: bool = get("autostart").unwrap().try_into().unwrap();
            let commandline: Vec<String> = get("commandline").unwrap().try_into().unwrap();
            let reason: String = get("reason").unwrap().try_into().unwrap();
            self.asked.lock().unwrap().push((autostart, commandline, reason));
            let sender = header.sender().unwrap().trim_start_matches(':').replace('.', "_");
            let path = OwnedObjectPath::try_from(format!("/org/freedesktop/portal/desktop/request/{sender}/{token}")).unwrap();
            let mut results: HashMap<&str, Value<'_>> = HashMap::new();
            results.insert("background", Value::from(true));
            results.insert("autostart", Value::from(self.autostart.unwrap_or(autostart)));
            let request = SignalEmitter::new(emitter.connection(), ObjectPath::from(&path)).unwrap();
            request.emit("org.freedesktop.portal.Request", "Response", &(self.response, results)).await.unwrap();
            Ok(path)
        }
    }

    /// A private bus with the fake portal on it, and what it was asked.
    fn served(response: u32, autostart: Option<bool>) -> Option<(PrivateBus, zbus::blocking::Connection, Asked)> {
        let bus = PrivateBus::start()?;
        let asked = Asked::default();
        let fake = Fake { asked: Arc::clone(&asked), response, autostart };
        let server = zbus::blocking::connection::Builder::address(bus.address())
            .unwrap()
            .name(portal::DESTINATION)
            .unwrap()
            .serve_at(portal::PATH, fake)
            .unwrap()
            .build()
            .unwrap();
        Some((bus, server, asked))
    }

    #[test]
    fn the_portal_is_asked_to_start_inboxcraft_in_the_background_at_login() {
        let Some((bus, _server, asked)) = served(0, None) else { return };
        let granted = portal::request(&bus.connect(), true, Duration::from_secs(10)).unwrap();
        assert_eq!(granted, Granted { background: true, autostart: true });
        let asked = asked.lock().unwrap();
        let (autostart, commandline, reason) = &asked[0];
        assert!(*autostart);
        assert_eq!(commandline, &["inboxcraft", "--background"]);
        assert!(reason.contains("mail"), "{reason}");
    }

    #[test]
    fn a_refusal_or_a_cancel_is_an_error() {
        for response in [1, 2] {
            let Some((bus, _server, _asked)) = served(response, None) else { return };
            let error = portal::request(&bus.connect(), false, Duration::from_secs(10)).unwrap_err();
            assert!(error.contains("refused") || error.contains("cancelled"), "{error}");
        }
    }

    #[test]
    fn the_portal_s_own_answer_on_autostart_is_reported() {
        let Some((bus, _server, _asked)) = served(0, Some(false)) else { return };
        assert_eq!(portal::request(&bus.connect(), true, Duration::from_secs(10)).unwrap(), Granted { background: true, autostart: false });
    }

    #[test]
    fn without_a_portal_on_the_bus_the_request_is_an_error() {
        let Some(bus) = PrivateBus::start() else { return };
        let error = portal::request(&bus.connect(), true, Duration::from_secs(10)).unwrap_err();
        assert!(error.contains("Background portal"), "{error}");
    }

    /// A portal that takes the request and never answers it, as one waiting on a dialog nobody answers.
    struct Silent;

    #[zbus::interface(name = "org.freedesktop.portal.Background")]
    impl Silent {
        fn request_background(&self, _parent_window: &str, _options: HashMap<String, OwnedValue>) -> OwnedObjectPath {
            OwnedObjectPath::try_from("/org/freedesktop/portal/desktop/request/x/y").unwrap()
        }
    }

    #[test]
    fn a_portal_that_does_not_answer_in_time_is_an_error() {
        let Some(bus) = PrivateBus::start() else { return };
        let _server = zbus::blocking::connection::Builder::address(bus.address())
            .unwrap()
            .name(portal::DESTINATION)
            .unwrap()
            .serve_at(portal::PATH, Silent)
            .unwrap()
            .build()
            .unwrap();
        let error = portal::request(&bus.connect(), true, Duration::from_millis(300)).unwrap_err();
        assert!(error.contains("did not answer"), "{error}");
    }
}

#[test]
fn the_folder_and_the_sandbox_are_read_from_this_run() {
    assert_eq!(super::autostart_dir(), autostart_dir_from(std::env::var_os("XDG_CONFIG_HOME"), std::env::var_os("HOME")));
    assert_eq!(super::sandboxed(), Path::new("/.flatpak-info").exists());
}
