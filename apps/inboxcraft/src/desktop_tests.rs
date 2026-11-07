// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use eframe::egui::{self, RawInput, ViewportCommand, ViewportEvent, ViewportId, ViewportInfo};
use inboxcraft_engine::Session;
use inboxcraft_model::{Account, AccountId, EmailAddress, Event, Host, Job, JobKind, Security, Server};
use inboxcraft_store::{Batch, Change, Memory, Storage};
use inboxcraft_time::{Clock, FakeClock, Timestamp};
use inboxcraft_ui_egui::InboxApp;
use inboxcraft_ui_egui::mailbox::Mailbox;
use inboxcraft_ui_egui::onboard::{NoDetect, NoKeychain};
use inboxcraft_ui_egui::sample;
use inboxcraft_ui_egui::workers::Workers;

use super::{Badge, Desktop, unread};
use crate::instance::Request;

/// How long a windowless wait lasts in these tests when no request comes.
const SHORT: Duration = Duration::from_millis(20);

fn clock() -> Box<dyn Clock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

fn sample_app() -> InboxApp {
    InboxApp::new(Mailbox::new(sample::session(clock()).unwrap(), clock()))
}

/// A desktop on the sample mailbox that keeps running when its window is closed, with the sender of its requests.
fn desktop() -> (Desktop, Sender<Request>) {
    let (send, heard) = mpsc::channel();
    (Desktop::new(sample_app(), heard), send)
}

/// The commands one tick sent to the window, with `close` as the window's close button.
fn tick(desktop: &mut Desktop, ctx: &egui::Context, close: bool) -> Vec<ViewportCommand> {
    let mut info = ViewportInfo::default();
    if close {
        info.events.push(ViewportEvent::Close);
    }
    let mut raw = RawInput::default();
    raw.viewports.insert(ViewportId::ROOT, info);
    let output = ctx.run_logic(&raw, |ctx| desktop.tick(ctx));
    output.viewport_commands.get(&ViewportId::ROOT).cloned().unwrap_or_default()
}

#[test]
fn closing_the_window_closes_it_and_keeps_inboxcraft_running_without_one() {
    let (mut desktop, _send) = desktop();
    let ctx = egui::Context::default();
    assert!(tick(&mut desktop, &ctx, false).is_empty(), "nothing happens without a close");
    assert!(desktop.has_window());
    // The close goes ahead (a Wayland window cannot be hidden), but InboxCraft does not end with it.
    assert!(tick(&mut desktop, &ctx, true).is_empty());
    assert!(!desktop.has_window());
    assert!(!desktop.is_quitting());
}

#[test]
fn a_window_that_ends_when_closed_ends_inboxcraft() {
    let (send, heard) = mpsc::channel::<Request>();
    let mut desktop = Desktop::new(sample_app(), heard).closing_ends();
    let ctx = egui::Context::default();
    assert!(tick(&mut desktop, &ctx, true).is_empty());
    assert!(desktop.is_quitting());
    drop(send);
}

#[test]
fn a_second_start_opens_the_window_again() {
    let (mut desktop, send) = desktop();
    let ctx = egui::Context::default();
    tick(&mut desktop, &ctx, true);
    desktop.wait(SHORT);
    assert!(!desktop.has_window(), "nothing asked for it yet");
    send.send(Request::Show).unwrap();
    desktop.wait(SHORT);
    assert!(desktop.has_window());
    assert!(!desktop.is_quitting());
}

#[test]
fn a_second_start_brings_a_shown_window_forward() {
    let (mut desktop, send) = desktop();
    let ctx = egui::Context::default();
    send.send(Request::Show).unwrap();
    assert_eq!(tick(&mut desktop, &ctx, false), [ViewportCommand::Minimized(false), ViewportCommand::Focus]);
}

#[test]
fn a_background_start_has_no_window_until_asked() {
    let (send, heard) = mpsc::channel();
    let mut desktop = Desktop::new(sample_app(), heard).without_window();
    assert!(!desktop.has_window());
    send.send(Request::Show).unwrap();
    desktop.wait(SHORT);
    assert!(desktop.has_window());
}

#[test]
fn quit_ends_inboxcraft_even_though_closing_does_not() {
    let (mut desktop, send) = desktop();
    let ctx = egui::Context::default();
    send.send(Request::Quit).unwrap();
    assert_eq!(tick(&mut desktop, &ctx, false), [ViewportCommand::Close]);
    assert!(desktop.is_quitting());
    // The close it asked for comes back as a close request, which ends InboxCraft.
    assert!(tick(&mut desktop, &ctx, true).is_empty());
    assert!(desktop.is_quitting());
}

#[test]
fn a_quit_without_a_window_ends_it() {
    let (mut desktop, send) = desktop();
    let ctx = egui::Context::default();
    tick(&mut desktop, &ctx, true);
    send.send(Request::Quit).unwrap();
    desktop.wait(SHORT);
    assert!(desktop.is_quitting());
}

#[test]
fn with_nobody_left_to_ask_for_the_window_inboxcraft_ends() {
    let (mut desktop, send) = desktop();
    let ctx = egui::Context::default();
    tick(&mut desktop, &ctx, true);
    drop(send);
    desktop.wait(SHORT);
    assert!(desktop.is_quitting(), "no second start and no tray could bring a window back");
}

/// Workers that write down the jobs they are handed.
struct Recording(Arc<Mutex<Vec<JobKind>>>);

impl Workers for Recording {
    fn send(&mut self, _account: &Account, job: Job) {
        self.0.lock().unwrap().push(job.kind);
    }

    fn keep(&mut self, _accounts: &[AccountId]) {}

    fn events(&mut self) -> Vec<Event> {
        Vec::new()
    }
}

fn account() -> Account {
    let imap = Server::new(Host::new("imap.example.org").unwrap(), Security::Tls.imap_port(), Security::Tls);
    Account::new(AccountId::new(1), "Home", EmailAddress::new("anna@example.org").unwrap(), "anna", imap).unwrap()
}

/// A desktop over a store with one account whose folder list is queued, and the jobs its workers were handed.
fn syncing() -> (Desktop, Sender<Request>, Arc<Mutex<Vec<JobKind>>>) {
    let mut store = Memory::default();
    store.apply(&Batch::new().with(Change::PutAccount(account()))).unwrap();
    let mut session = Session::new(Box::new(store), clock()).unwrap();
    session.start();
    let sent = Arc::new(Mutex::new(Vec::new()));
    let workers = Box::new(Recording(Arc::clone(&sent)));
    let app = InboxApp::new(Mailbox::new(session, clock())).with_accounts(Arc::new(NoKeychain), Box::new(NoDetect::default()), workers);
    let (send, heard) = mpsc::channel();
    (Desktop::new(app, heard), send, sent)
}

#[test]
fn without_a_window_the_jobs_still_go_out() {
    let (desktop, _send, sent) = syncing();
    let mut desktop = desktop.without_window();
    desktop.wait(SHORT);
    assert_eq!(*sent.lock().unwrap(), [JobKind::SyncFolderList], "the folder list went to the worker");
}

#[test]
fn every_tick_hands_the_jobs_on_also_when_no_frame_is_drawn() {
    let (mut desktop, _send, sent) = syncing();
    let ctx = egui::Context::default();
    tick(&mut desktop, &ctx, false);
    assert_eq!(*sent.lock().unwrap(), [JobKind::SyncFolderList]);
}

/// A badge that writes down every count it was shown.
#[derive(Clone, Default)]
struct Shown(Arc<Mutex<Vec<u64>>>);

impl Badge for Shown {
    fn show(&mut self, unread: u64) {
        self.0.lock().unwrap().push(unread);
    }
}

#[test]
fn the_badge_shows_the_unread_count_when_it_changes() {
    let shown = Shown::default();
    let (send, heard) = mpsc::channel();
    let mut desktop = Desktop::new(sample_app(), heard).with_badge(Box::new(shown.clone()));
    let count = unread(desktop.app().mailbox());
    assert!(count > 0, "the sample has unread mail");
    let ctx = egui::Context::default();
    tick(&mut desktop, &ctx, false);
    tick(&mut desktop, &ctx, false);
    assert_eq!(*shown.0.lock().unwrap(), [count], "shown once, not every frame");
    let first = desktop.app().mailbox().messages().iter().find(|row| !row.seen).unwrap().id;
    desktop.app_mut().mailbox_mut().act(first, inboxcraft_ui_egui::mailbox::Act::Read);
    tick(&mut desktop, &ctx, true);
    assert_eq!(*shown.0.lock().unwrap(), [count, count - 1]);
    desktop.app_mut().mailbox_mut().act(first, inboxcraft_ui_egui::mailbox::Act::Unread);
    desktop.wait(SHORT);
    assert_eq!(*shown.0.lock().unwrap(), [count, count - 1, count], "also without a window");
    drop(send);
}

#[test]
fn the_unread_count_is_the_inboxes_of_every_account() {
    let app = sample_app();
    let mailbox = app.mailbox();
    let inboxes: u64 = mailbox
        .accounts()
        .iter()
        .flat_map(|account| mailbox.folders_of(account.id))
        .filter(|folder| folder.role == Some(inboxcraft_model::Role::Inbox))
        .map(|folder| folder.unread)
        .sum();
    assert_eq!(unread(mailbox), inboxes);
    assert!(mailbox.accounts().len() > 1, "more than one account counts");
}

#[test]
fn a_desktop_shows_its_state_and_not_the_mail_in_debug_output() {
    let (desktop, _send) = desktop();
    let shown = format!("{desktop:?}");
    assert!(shown.starts_with("Desktop {") && shown.contains("window: true"), "{shown}");
}

/// Where the notifications went, or an error for each.
#[derive(Clone, Default)]
struct Told {
    shown: Arc<Mutex<Vec<crate::notify::Shown>>>,
    fails: bool,
}

impl crate::notify::Deliver for Told {
    fn deliver(&mut self, shown: &crate::notify::Shown) -> Result<(), String> {
        if self.fails {
            return Err("the notification was not shown: no server".to_owned());
        }
        self.shown.lock().unwrap().push(shown.clone());
        Ok(())
    }
}

/// Workers that hand back `events` once.
struct Feeding(Vec<Event>);

impl Workers for Feeding {
    fn send(&mut self, _account: &Account, _job: Job) {}

    fn keep(&mut self, _accounts: &[AccountId]) {}

    fn events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.0)
    }
}

/// A desktop over one account whose synced inbox gets a new message from Bob, telling `told`.
fn arriving(told: &Told) -> (Desktop, Sender<Request>, Sender<crate::notify::Click>) {
    use inboxcraft_model::{Address, Envelope, EventKind, Folder, FolderId, FolderListing, FolderPath, FolderState, Summary, Uid, UidValidity};
    let validity = UidValidity::new(7).unwrap();
    let inbox = Folder::new(FolderId::new(1), AccountId::new(1), FolderListing::new(FolderPath::new("INBOX", Some('/')).unwrap(), vec![], true));
    let mut store = Memory::default();
    let batch = Batch::new()
        .with(Change::PutAccount(account()))
        .with(Change::PutFolder(inbox))
        .with(Change::SetFolderState(FolderId::new(1), FolderState::new(validity)));
    store.apply(&batch).unwrap();
    let session = Session::new(Box::new(store), clock()).unwrap();
    let envelope = Envelope::new("Lunch?").with_from(vec![Address::new(Some("Bob"), EmailAddress::new("bob@example.net").unwrap())]);
    let added =
        EventKind::MessagesAdded { folder: FolderId::new(1), uid_validity: validity, messages: vec![Summary::new(Uid::new(3).unwrap(), envelope)] };
    let workers = Box::new(Feeding(vec![Event::new(AccountId::new(1), None, added)]));
    let app = InboxApp::new(Mailbox::new(session, clock())).with_accounts(Arc::new(NoKeychain), Box::new(NoDetect::default()), workers);
    let (send, heard) = mpsc::channel();
    let (click, clicks) = mpsc::channel();
    (Desktop::new(app, heard).with_notices(Box::new(told.clone()), clicks), send, click)
}

#[test]
fn new_mail_shows_as_a_notification_also_without_a_window() {
    let told = Told::default();
    let (desktop, _send, _click) = arriving(&told);
    let mut desktop = desktop.without_window();
    desktop.wait(SHORT);
    desktop.wait(SHORT);
    let shown = told.shown.lock().unwrap();
    assert_eq!(shown.len(), 1, "told once");
    assert_eq!((shown[0].title.as_str(), shown[0].body.as_str()), ("Bob", "Lunch?"));
    assert!(shown[0].key.starts_with("mail-"), "{:?}", shown[0]);
}

#[test]
fn a_notification_that_fails_leaves_inboxcraft_running() {
    let told = Told { fails: true, ..Told::default() };
    let (mut desktop, _send, _click) = arriving(&told);
    let ctx = egui::Context::default();
    tick(&mut desktop, &ctx, false);
    tick(&mut desktop, &ctx, false);
    assert!(desktop.has_window() && !desktop.is_quitting());
}

/// The sample desktop answering clicks, with the sender of its clicks.
fn clicking() -> (Desktop, Sender<Request>, Sender<crate::notify::Click>) {
    let (send, heard) = mpsc::channel();
    let (click, clicks) = mpsc::channel();
    (Desktop::new(sample_app(), heard).with_notices(Box::new(Told::default()), clicks), send, click)
}

fn unread_message(desktop: &Desktop) -> u64 {
    desktop.app().mailbox().messages().iter().find(|row| !row.seen).unwrap().id
}

#[test]
fn a_click_on_a_notification_opens_the_message_full_width_and_brings_the_window_forward() {
    use crate::notify::Click;
    let (mut desktop, _send, click) = clicking();
    let ctx = egui::Context::default();
    let id = unread_message(&desktop);
    click.send(Click::Open(id)).unwrap();
    assert_eq!(tick(&mut desktop, &ctx, false), [ViewportCommand::Minimized(false), ViewportCommand::Focus]);
    assert_eq!(desktop.app().reader().open_id(), Some(id));
    assert!(desktop.app().reader().full_width());
    click.send(Click::Show).unwrap();
    assert_eq!(tick(&mut desktop, &ctx, false), [ViewportCommand::Minimized(false), ViewportCommand::Focus]);
}

#[test]
fn without_a_window_a_click_opens_one() {
    use crate::notify::Click;
    let (desktop, _send, click) = clicking();
    let mut desktop = desktop.without_window();
    let id = unread_message(&desktop);
    click.send(Click::Open(id)).unwrap();
    desktop.wait(SHORT);
    assert!(desktop.has_window());
    assert_eq!(desktop.app().reader().open_id(), Some(id));
}

#[test]
fn the_buttons_mark_the_message_read_or_done() {
    use crate::notify::Click;
    let (mut desktop, _send, click) = clicking();
    let ctx = egui::Context::default();
    let id = unread_message(&desktop);
    click.send(Click::Read(id)).unwrap();
    assert!(tick(&mut desktop, &ctx, false).is_empty(), "the window stays where it is");
    assert!(desktop.app().mailbox().messages().iter().any(|row| row.id == id && row.seen));
    click.send(Click::Done(id)).unwrap();
    tick(&mut desktop, &ctx, false);
    assert!(desktop.app().mailbox().messages().iter().all(|row| row.id != id), "done leaves the inbox");
}
