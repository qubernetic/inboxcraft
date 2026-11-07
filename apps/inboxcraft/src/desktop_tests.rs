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
