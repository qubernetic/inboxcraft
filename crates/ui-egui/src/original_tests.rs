// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The reader's "Open the original in the browser", through a fake [`Original`]: nothing here opens a browser.

use std::sync::{Arc, Mutex};

use egui::accesskit::Role;
use egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use inboxcraft_time::{FakeClock, Timestamp};

use super::Original;
use crate::InboxApp;
use crate::mailbox::Mailbox;
use crate::reader::Content;
use crate::sample;
use crate::save::NoSave;

const ROW: &str = "Open the original in the browser";

/// What the fake desktop was handed, and whether it refuses (a desktop with no browser).
#[derive(Clone, Default)]
struct Desktop {
    handed: Arc<Mutex<Vec<(u64, String)>>>,
    refuses: bool,
}

impl Original for Desktop {
    fn open(&self, id: u64, page: &str) -> Result<(), String> {
        if self.refuses {
            return Err("no browser is set up\0 here".to_owned());
        }
        self.handed.lock().unwrap().push((id, page.to_owned()));
        Ok(())
    }
}

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

/// The app over the sample with the message of `subject` open, opening originals through `desktop` when given.
fn opened(desktop: Option<&Desktop>, subject: &str) -> (Harness<'static, InboxApp>, u64) {
    let mut app = InboxApp::with_outside_at_mail(
        Mailbox::new(sample::session(clock()).unwrap(), clock()),
        Box::new(sample::Pictures::default()),
        Box::new(NoSave),
    );
    if let Some(desktop) = desktop {
        app = app.with_original(Box::new(desktop.clone()));
    }
    let id = app.mailbox().messages().iter().find(|row| row.subject == subject).unwrap().id;
    app.mailbox_mut().select(Some(id));
    let mut harness = Harness::builder().with_size(vec2(1280.0, 800.0)).with_max_steps(12).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    (harness, id)
}

/// An HTML message of the sample with remote pictures.
const HTML: &str = "Your bike is due for a service";

/// Frames until the open the pane's reader started has its answer in the notice (the open runs on a thread).
fn answered(harness: &mut Harness<'static, InboxApp>) {
    for _ in 0..1000 {
        if !harness.state().reader().opening_original() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
        harness.step();
    }
    assert!(!harness.state().reader().opening_original(), "the open never answered");
    harness.run();
}

fn more(harness: &mut Harness<'static, InboxApp>) {
    harness.get_by_label("More").click();
    harness.run();
}

#[test]
fn a_window_without_an_original_does_not_offer_the_row() {
    // The web app hands in none: the row is not there at all, not even disabled.
    let (mut harness, id) = opened(None, HTML);
    assert!(matches!(harness.state().reader().opened(id).map(|opened| &opened.content), Some(Content::Html(_))));
    more(&mut harness);
    harness.get_by_role_and_label(Role::MenuItem, "Show the full header");
    assert!(harness.query_by_label(ROW).is_none(), "no browser, no row");
}

#[test]
fn the_row_hands_a_cleaned_copy_of_the_open_message_to_the_desktop() {
    let desktop = Desktop::default();
    let (mut harness, id) = opened(Some(&desktop), HTML);
    // The owner loaded the remote pictures for this message: the copy still names no address.
    harness.get_by_label("load once").click();
    harness.run();
    more(&mut harness);
    harness.get_by_role_and_label(Role::MenuItem, ROW).click();
    harness.run();
    answered(&mut harness);
    let handed = desktop.handed.lock().unwrap().clone();
    assert_eq!(handed.len(), 1, "one press, one page");
    let (handed_id, page) = handed.first().unwrap();
    assert_eq!(*handed_id, id);
    let lower = page.to_ascii_lowercase();
    for needle in ["<script", "http:", "https:", "src=", "href=", "url("] {
        assert!(!lower.contains(needle), "{needle} in {page}");
    }
    assert!(page.contains("default-src 'none'"), "{page}");
    assert!(page.contains(&format!("<title>{HTML}</title>")), "{page}");
    let reader = harness.state().reader();
    assert_eq!(reader.notice(), Some("Opened a cleaned copy in the browser"));
    assert!(!reader.notice_failed());
}

#[test]
fn a_desktop_with_no_browser_says_why_in_the_status() {
    let desktop = Desktop { refuses: true, ..Desktop::default() };
    let (mut harness, _) = opened(Some(&desktop), HTML);
    more(&mut harness);
    harness.get_by_role_and_label(Role::MenuItem, ROW).click();
    harness.run();
    answered(&mut harness);
    let reader = harness.state().reader();
    assert!(reader.notice_failed(), "{:?}", reader.notice());
    let notice = reader.notice().unwrap();
    assert!(notice.starts_with("The original could not be opened: no browser is set up"), "{notice}");
    assert!(!notice.contains('\0'), "{notice:?}");
}

#[test]
fn a_plain_text_message_offers_no_row() {
    let mut mailbox = Mailbox::new(sample::session(clock()).unwrap(), clock());
    let mut reader = crate::reader::Reader::new(Box::new(sample::Pictures::default()), Box::new(NoSave));
    reader.set_original(super::Shared::new(Box::new(Desktop::default())));
    let ids: Vec<u64> = mailbox.messages().iter().map(|row| row.id).collect();
    let (mut html, mut text) = (0, 0);
    for id in ids {
        reader.open(&mut mailbox, Some(id));
        match reader.opened(id).map(|opened| &opened.content) {
            Some(Content::Html(_)) => {
                html += 1;
                assert!(reader.offers_original(id));
            }
            Some(_) => {
                text += 1;
                assert!(!reader.offers_original(id), "{id}");
                // Asked all the same, it says why not and hands nothing on.
                reader.open_original(id, &egui::Context::default());
                assert!(reader.notice_failed());
            }
            None => {}
        }
    }
    assert!(html > 0 && text > 0, "{html} {text}");
}

#[test]
fn a_held_key_s_repeat_does_not_open_the_browser() {
    let desktop = Desktop::default();
    let (mut harness, _) = opened(Some(&desktop), HTML);
    more(&mut harness);
    // Enter goes down before the row has the keyboard (held on from an earlier press), then repeats on the row.
    harness.key_down(egui::Key::Enter);
    harness.run();
    harness.get_by_role_and_label(Role::MenuItem, ROW).focus();
    harness.run();
    assert!(desktop.handed.lock().unwrap().is_empty(), "focus alone");
    for _ in 0..3 {
        harness.event(egui::Event::Key { key: egui::Key::Enter, physical_key: None, pressed: true, repeat: true, modifiers: egui::Modifiers::NONE });
        harness.run();
    }
    assert!(desktop.handed.lock().unwrap().is_empty(), "a repeat is no press of the owner's");
    harness.key_up(egui::Key::Enter);
    harness.run();
    harness.key_down(egui::Key::Enter);
    harness.run();
    harness.key_up(egui::Key::Enter);
    harness.run();
    answered(&mut harness);
    assert_eq!(desktop.handed.lock().unwrap().len(), 1, "a fresh press opens it");
}

/// A desktop that answers only when the test says so: its hand-off blocks on `gate` till then (10 s at most).
struct Stuck {
    gate: Mutex<std::sync::mpsc::Receiver<Result<(), String>>>,
    asked: Arc<std::sync::atomic::AtomicUsize>,
}

impl Original for Stuck {
    fn open(&self, _id: u64, _page: &str) -> Result<(), String> {
        self.asked.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        // Bounded, so a hand-off run on the frame by mistake fails the test rather than hanging it.
        self.gate.lock().unwrap().recv_timeout(std::time::Duration::from_secs(10)).unwrap_or_else(|_| Err("never answered".to_owned()))
    }
}

/// What frees a [`Stuck`] desktop: the answer it gives.
pub(crate) type Free = std::sync::mpsc::Sender<Result<(), String>>;

/// A stuck desktop, the sender that frees it and how often it was asked.
pub(crate) fn stuck() -> (Box<dyn Original>, Free, Arc<std::sync::atomic::AtomicUsize>) {
    let (sender, gate) = std::sync::mpsc::channel();
    let asked = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    (Box::new(Stuck { gate: Mutex::new(gate), asked: Arc::clone(&asked) }), sender, asked)
}

#[test]
fn a_desktop_that_does_not_answer_stops_no_frame_and_its_answer_comes_later() {
    let (original, free, asked) = stuck();
    let mut app = InboxApp::with_outside_at_mail(
        Mailbox::new(sample::session(clock()).unwrap(), clock()),
        Box::new(sample::Pictures::default()),
        Box::new(NoSave),
    )
    .with_original(original);
    let id = app.mailbox().messages().iter().find(|row| row.subject == HTML).unwrap().id;
    app.mailbox_mut().select(Some(id));
    let mut harness = Harness::builder().with_size(vec2(1280.0, 800.0)).with_max_steps(12).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    more(&mut harness);
    harness.get_by_role_and_label(Role::MenuItem, ROW).click();
    // The frames go on while the desktop says nothing: each of these returns.
    for _ in 0..5 {
        harness.run();
    }
    let reader = harness.state().reader();
    assert!(reader.opening_original());
    assert_eq!(reader.notice(), Some("Opening a cleaned copy in the browser\u{2026}"));
    // A second press while it runs is ignored: the desktop is not asked again.
    more(&mut harness);
    harness.get_by_role_and_label(Role::MenuItem, ROW).click();
    harness.run();
    assert_eq!(harness.state().reader().notice(), Some("An original is still being opened; wait for it."));
    assert!(harness.state().reader().opening_original(), "the first one still runs");
    // The rest of the window works: the list moves to another message.
    let other = harness.state().mailbox().messages().iter().find(|row| row.id != id).unwrap().id;
    harness.state_mut().mailbox_mut().select(Some(other));
    harness.run();
    assert_eq!(harness.state().mailbox().selected(), Some(other));
    // The desktop answers at last; the answer reaches the notice on a later frame.
    free.send(Err("no browser\0at all".to_owned())).unwrap();
    answered(&mut harness);
    let reader = harness.state().reader();
    assert!(reader.notice_failed());
    assert_eq!(reader.notice(), Some("The original could not be opened: no browser at all"));
    assert_eq!(asked.load(std::sync::atomic::Ordering::SeqCst), 1, "asked once");
}
