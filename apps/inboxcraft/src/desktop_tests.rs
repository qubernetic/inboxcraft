// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use eframe::egui::{self, RawInput, ViewportCommand, ViewportEvent, ViewportId, ViewportInfo};
use inboxcraft_control_key::Failure;
use inboxcraft_engine::Session;
use inboxcraft_model::{Account, AccountId, EmailAddress, Event, Host, Job, JobKind, Security, Server};
use inboxcraft_store::{Batch, Change, Memory, Storage};
use inboxcraft_time::{Clock, FakeClock, Timestamp};
use inboxcraft_ui_egui::mailbox::Mailbox;
use inboxcraft_ui_egui::onboard::{NoDetect, NoKeychain};
use inboxcraft_ui_egui::sample;
use inboxcraft_ui_egui::workers::Workers;
use inboxcraft_ui_egui::{InboxApp, Screen};
use serde_json::json;

use super::{Badge, Desktop, unread};
use crate::control::Call;
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
fn the_owner_s_choice_to_run_in_the_background_decides_what_closing_the_window_does() {
    use std::sync::atomic::Ordering;

    let (mut desktop, _send) = desktop();
    let stays = desktop.stays();
    assert!(stays.load(Ordering::SeqCst));
    stays.store(false, Ordering::SeqCst);
    let ctx = egui::Context::default();
    tick(&mut desktop, &ctx, true);
    assert!(desktop.is_quitting(), "turned off on the Notifications page, closing ends InboxCraft");
    let (send, heard) = mpsc::channel::<Request>();
    let desktop = Desktop::new(sample_app(), heard).closing_ends();
    assert!(!desktop.stays().load(Ordering::SeqCst));
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

#[test]
fn a_tick_of_a_hidden_window_asks_to_tick_again_when_a_snooze_ends() {
    let (mut desktop, _send) = desktop();
    let ctx = egui::Context::default();
    // A new context asks for frames of its own at first.
    for _ in 0..4 {
        tick(&mut desktop, &ctx, false);
    }
    assert!(!ctx.has_requested_repaint(), "nothing waits in the sample");
    let first = desktop.app().mailbox().messages().first().unwrap().id;
    let until = Timestamp::from_unix(1_793_869_200 + 3600, 0).unwrap();
    desktop.app_mut().mailbox_mut().act(first, inboxcraft_ui_egui::mailbox::Act::Snooze(until));
    // eframe runs only the logic of a hidden window, so the tick asks for the next one itself.
    tick(&mut desktop, &ctx, false);
    assert!(ctx.has_requested_repaint(), "the window wakes when the snooze ends");
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
    assert_eq!(desktop.app().screen(), Screen::Mail, "the reader shows only on the mail");
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
    assert_eq!(desktop.app().screen(), Screen::Mail);
    assert_eq!(desktop.app().reader().open_id(), Some(id));
}

/// Every way in from outside the window (a notification's click or button, the tray's or a second start's "show", an
/// agent's call) leaves the window on a place where what it opened is in sight, and moves nothing it did not open.
#[test]
fn what_comes_from_outside_opens_where_it_is_seen_from_every_place() {
    use crate::notify::Click;
    let ctx = egui::Context::default();
    let places: [fn(&mut InboxApp); 4] = [InboxApp::show_home, InboxApp::show_mail, InboxApp::show_calendar, |app| {
        app.show_home();
        app.open_settings();
    }];
    for (at, place) in places.iter().enumerate() {
        let (mut desktop, send, click) = clicking();
        place(desktop.app_mut());
        let before = desktop.app().screen();
        let id = unread_message(&desktop);
        click.send(Click::Open(id)).unwrap();
        tick(&mut desktop, &ctx, false);
        assert_eq!(desktop.app().screen(), Screen::Mail, "place {at}: a message opens on the mail");
        assert!(!desktop.app().settings().is_open(), "place {at}: nor under the settings");
        assert_eq!(desktop.app().reader().open_id(), Some(id), "place {at}");

        let (mut desktop, _send2, click) = clicking();
        place(desktop.app_mut());
        let event = {
            let (calendar, mailbox) = desktop.app_mut().calendar_mut();
            calendar.refresh(mailbox);
            calendar.events().find(|event| event.summary == "Studio stand-up").unwrap().id
        };
        click.send(Click::Event(event, 1_793_869_200 + 4 * 86_400)).unwrap();
        tick(&mut desktop, &ctx, false);
        assert_eq!(desktop.app().screen(), Screen::Calendar, "place {at}: an event opens on the calendar");
        assert!(!desktop.app().settings().is_open(), "place {at}: nor under the settings");
        assert_eq!(desktop.app().calendar().picked().map(|event| event.id), Some(event), "place {at}");

        // What opens nothing leaves the window where it was: the tray's and a second start's "show", a
        // notification's buttons, and an agent's calls (the control channel's and the CLI's `safe` commands only
        // read or change the mail; none of them opens a message, a thread or the composer in the window).
        let (desktop, send_show, click) = clicking();
        let (calls_in, calls) = mpsc::channel();
        let mut desktop = desktop.with_control(calls);
        place(desktop.app_mut());
        let settings = desktop.app().settings().is_open();
        let id = unread_message(&desktop);
        click.send(Click::Show).unwrap();
        click.send(Click::Read(id)).unwrap();
        send_show.send(Request::Show).unwrap();
        let (listing, listed) = call("mail.list", json!({}));
        calls_in.send(listing).unwrap();
        let (thread, threaded) = call("mail.thread", json!({ "id": id }));
        calls_in.send(thread).unwrap();
        click.send(Click::Done(id)).unwrap();
        tick(&mut desktop, &ctx, false);
        assert!(listed.try_recv().unwrap().is_ok() && threaded.try_recv().unwrap().is_ok(), "place {at}");
        assert_eq!(desktop.app().screen(), before, "place {at}: show, read, done and a call move nothing");
        assert_eq!(desktop.app().settings().is_open(), settings, "place {at}");
        assert_eq!(desktop.app().reader().open_id(), None, "place {at}: nor open a message");
        drop(send);
    }
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

/// A call of `command` with `params` over the control channel, with where its answer arrives.
fn call(command: &str, params: serde_json::Value) -> (Call, mpsc::Receiver<Result<serde_json::Value, Failure>>) {
    let (send, answer) = mpsc::channel();
    (Call::new(command.to_owned(), params, send), answer)
}

#[test]
fn a_control_call_runs_for_an_agent_with_or_without_a_window() {
    let (send, heard) = mpsc::channel();
    let (calls_in, calls) = mpsc::channel();
    let mut desktop = Desktop::new(sample_app(), heard).with_control(calls);
    let (listing, listed) = call("account.list", json!({}));
    calls_in.send(listing).unwrap();
    tick(&mut desktop, &egui::Context::default(), false);
    assert!(listed.try_recv().unwrap().unwrap()["accounts"].is_array());
    let (archive, archived) = call("mail.archive", json!({ "id": 1 }));
    calls_in.send(archive).unwrap();
    let ctx = egui::Context::default();
    tick(&mut desktop, &ctx, true);
    assert!(!desktop.has_window());
    assert_eq!(archived.try_recv().unwrap().unwrap_err().code, "not_allowed", "only safe commands run for an agent");
    let (listing, listed) = call("account.list", json!({}));
    calls_in.send(listing).unwrap();
    desktop.wait(SHORT);
    assert!(listed.try_recv().unwrap().is_ok(), "answered without a window");
    drop(send);
}

#[test]
fn an_agent_s_undo_is_refused_while_an_event_is_edited_in_the_window_and_runs_once_it_closes() {
    let (send, heard) = mpsc::channel();
    let (calls_in, calls) = mpsc::channel();
    let mut desktop = Desktop::new(sample_app(), heard).with_control(calls);
    let ctx = egui::Context::default();
    let (flag, flagged) = call("mail.flag", json!({ "id": 1, "flagged": true }));
    calls_in.send(flag).unwrap();
    tick(&mut desktop, &ctx, false);
    assert!(flagged.try_recv().unwrap().is_ok());
    desktop.app_mut().show_calendar();
    desktop.app_mut().calendar_mut().0.new_event();
    assert!(desktop.app().calendar().editor().is_some(), "the editor is open");
    let (undo, undone) = call("history.undo", json!({}));
    calls_in.send(undo).unwrap();
    tick(&mut desktop, &ctx, false);
    let refused = undone.try_recv().unwrap().unwrap_err();
    assert_eq!((refused.code.as_str(), refused.message.as_str()), ("editing", "an event is being edited"));
    // The window closes with the editor in it: nobody edits, the agent's undo runs, also without a window.
    let (undo, undone) = call("history.undo", json!({}));
    calls_in.send(undo).unwrap();
    tick(&mut desktop, &ctx, true);
    assert!(!desktop.has_window());
    assert_eq!(undone.try_recv().unwrap().unwrap()["command"], json!("mail.flag"));
    drop(send);
}

/// One frame of the window, 1200 × 820, with `events` from the keyboard.
fn frame(desktop: &mut Desktop, ctx: &egui::Context, events: Vec<egui::Event>) {
    let raw = RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 820.0))), events, ..RawInput::default() };
    let mut drawn = ctx.run_ui(raw, |ui| desktop.frame(ui));
    // No painter takes the textures in a test.
    drawn.textures_delta.clear();
}

/// The settings' key, Ctrl+, (the owner's own).
fn settings_key() -> egui::Event {
    egui::Event::Key { key: egui::Key::Comma, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::COMMAND }
}

/// A window over the sample and the control channel's sender, one frame drawn.
fn controlled() -> (Desktop, mpsc::Sender<Call>, egui::Context) {
    let (_send, heard) = mpsc::channel();
    let (calls, received) = mpsc::channel();
    let mut desktop = Desktop::new(sample_app(), heard).with_control(received);
    let ctx = egui::Context::default();
    frame(&mut desktop, &ctx, Vec::new());
    (desktop, calls, ctx)
}

/// Runs `ui.settings.open` with `params` over the control channel, ticks and draws a frame; its answer.
fn open_settings(desktop: &mut Desktop, calls: &mpsc::Sender<Call>, ctx: &egui::Context, params: serde_json::Value) -> serde_json::Value {
    let (open, answer) = call("ui.settings.open", params);
    calls.send(open).unwrap();
    tick(desktop, ctx, false);
    frame(desktop, ctx, Vec::new());
    answer.try_recv().unwrap().unwrap()
}

#[test]
fn ui_settings_open_over_the_control_channel_shows_the_page_named() {
    use inboxcraft_ui_egui::settings::Section;
    let (mut desktop, calls, ctx) = controlled();
    assert_eq!(open_settings(&mut desktop, &calls, &ctx, json!({ "page": "accounts" })), json!({ "page": "accounts", "window": true }));
    let settings = desktop.app().settings();
    assert!(settings.is_open());
    assert_eq!(settings.section(), Section::Accounts);
    // A page while the settings show goes to that page: the settings stay.
    open_settings(&mut desktop, &calls, &ctx, json!({ "page": "about" }));
    assert!(desktop.app().settings().is_open(), "a page never toggles back");
    assert_eq!(desktop.app().settings().section(), Section::About);
    // Junk is no page: the page last shown, the settings still shown.
    let junk = open_settings(&mut desktop, &calls, &ctx, json!({ "page": "\u{202e}accounts\u{0}" }));
    assert_eq!(junk, json!({ "page": null, "window": true }));
    assert!(desktop.app().settings().is_open(), "junk is no page, and no toggle");
    assert_eq!(desktop.app().settings().section(), Section::About, "the page last shown");
    // An agent's bare call never toggles: shown, the settings stay; not shown, they show on the page last shown.
    open_settings(&mut desktop, &calls, &ctx, json!({}));
    assert!(desktop.app().settings().is_open(), "an agent never takes the owner off the settings");
    frame(&mut desktop, &ctx, vec![settings_key()]);
    assert!(!desktop.app().settings().is_open(), "the owner's key still toggles");
    open_settings(&mut desktop, &calls, &ctx, json!(null));
    assert!(desktop.app().settings().is_open());
    assert_eq!(desktop.app().settings().section(), Section::About, "the page last shown");
}

/// The window on the Accounts page with the add form open, `typed` into it.
fn on_the_form(typed: impl FnOnce(&mut inboxcraft_ui_egui::onboard::Form)) -> (Desktop, mpsc::Sender<Call>, egui::Context) {
    let (mut desktop, calls, ctx) = controlled();
    desktop.app_mut().open_settings_page(Some("accounts"));
    desktop.app_mut().onboarding_mut().0.open();
    typed(desktop.app_mut().onboarding_mut().0.form_mut());
    frame(&mut desktop, &ctx, Vec::new());
    (desktop, calls, ctx)
}

#[test]
fn an_agent_s_bare_call_leaves_a_typed_password_where_it_is() {
    use inboxcraft_ui_egui::settings::Section;
    let (mut desktop, calls, ctx) = on_the_form(|form| form.password = inboxcraft_ui_egui::onboard::Secret::new("hunter2"));
    open_settings(&mut desktop, &calls, &ctx, json!({}));
    assert!(desktop.app().settings().is_open());
    assert_eq!(desktop.app().settings().section(), Section::Accounts);
    assert!(!desktop.app().onboarding().form().password.is_empty(), "the password is still there");
}

#[test]
fn an_agent_s_page_waits_while_the_form_holds_typing_and_goes_when_it_is_empty() {
    use inboxcraft_ui_egui::settings::Section;
    let (mut desktop, calls, ctx) = on_the_form(|form| form.host = "imap.example.org".to_owned());
    let skipped = open_settings(&mut desktop, &calls, &ctx, json!({ "page": "appearance" }));
    assert_eq!(desktop.app().settings().section(), Section::Accounts, "the owner's page stays");
    assert_eq!(desktop.app().onboarding().form().host, "imap.example.org", "the field is kept");
    let (mut desktop, calls, ctx) = on_the_form(|_| {});
    let acted = open_settings(&mut desktop, &calls, &ctx, json!({ "page": "appearance" }));
    assert_eq!(desktop.app().settings().section(), Section::Appearance, "an empty form holds nothing to lose");
    assert_eq!(skipped.to_string(), acted.to_string(), "the answer tells nothing of what the owner does");
}

#[test]
fn an_agent_s_call_lets_a_key_capture_go_on() {
    use inboxcraft_ui_egui::settings::Section;
    let (mut desktop, calls, ctx) = controlled();
    desktop.app_mut().open_settings_page(Some("shortcuts"));
    desktop.app_mut().settings_mut().capture(inboxcraft_ui_egui::keymap::FIND);
    frame(&mut desktop, &ctx, Vec::new());
    open_settings(&mut desktop, &calls, &ctx, json!({ "page": "about" }));
    assert_eq!(desktop.app().settings().capturing(), Some(inboxcraft_ui_egui::keymap::FIND), "the capture goes on");
    assert_eq!(desktop.app().settings().section(), Section::Shortcuts);
}

#[test]
fn the_owner_s_key_in_the_same_frame_wins_over_an_agent_s_ask() {
    use inboxcraft_ui_egui::settings::Section;
    let (mut desktop, calls, ctx) = controlled();
    assert!(!desktop.app().settings().is_open());
    let (open, _answer) = call("ui.settings.open", json!({ "page": "about" }));
    calls.send(open).unwrap();
    tick(&mut desktop, &ctx, false);
    frame(&mut desktop, &ctx, vec![settings_key()]);
    assert!(desktop.app().settings().is_open(), "the owner's key opened the settings");
    assert_eq!(desktop.app().settings().section(), Section::Appearance, "on the owner's page, not the agent's");
    frame(&mut desktop, &ctx, Vec::new());
    assert_eq!(desktop.app().settings().section(), Section::Appearance, "the agent's ask went");
}

#[test]
fn a_thousand_calls_in_a_tick_act_once_and_leave_the_window_usable() {
    use inboxcraft_ui_egui::settings::Section;
    let (mut desktop, calls, ctx) = controlled();
    let mut answers = Vec::new();
    for index in 0..1_000 {
        let params = if index % 2 == 0 { json!({ "page": "about" }) } else { json!({}) };
        let (open, answer) = call("ui.settings.open", params);
        calls.send(open).unwrap();
        answers.push(answer);
    }
    tick(&mut desktop, &ctx, false);
    assert!(answers.iter().all(|answer| answer.try_recv().is_ok_and(|answer| answer.is_ok())), "every call answered");
    assert!(desktop.app_mut().mailbox_mut().take_settings_asked().is_empty(), "nothing left in the queue");
    frame(&mut desktop, &ctx, Vec::new());
    // The newest, a bare call, acted once: the settings on the page last shown.
    assert!(desktop.app().settings().is_open());
    assert_eq!(desktop.app().settings().section(), Section::Appearance);
    assert!(!desktop.app().holds_agent_ask());
    frame(&mut desktop, &ctx, vec![settings_key()]);
    assert!(!desktop.app().settings().is_open(), "the owner's key works the next frame");
}

#[test]
fn a_click_on_an_alert_opens_its_event_on_the_calendar_and_brings_the_window_forward() {
    use crate::notify::Click;
    let (mut desktop, _send, click) = clicking();
    let ctx = egui::Context::default();
    let id = {
        let (calendar, mailbox) = desktop.app_mut().calendar_mut();
        calendar.refresh(mailbox);
        calendar.events().find(|event| event.summary == "Studio stand-up").unwrap().id
    };
    // Next Monday's stand-up, 09:00 UTC.
    click.send(Click::Event(id, 1_793_869_200 + 4 * 86_400)).unwrap();
    assert_eq!(tick(&mut desktop, &ctx, false), [ViewportCommand::Minimized(false), ViewportCommand::Focus]);
    assert_eq!(desktop.app().screen(), inboxcraft_ui_egui::Screen::Calendar);
    assert_eq!(desktop.app().calendar().picked().map(|event| event.id), Some(id));
    click.send(Click::Event(id, i64::MAX)).unwrap();
    tick(&mut desktop, &ctx, false);
    assert_eq!(desktop.app().calendar().picked().map(|event| event.id), Some(id), "a time out of range moves nothing");
}

/// A desktop over one account on a SQLite store in `dir` that writes drafts behind, telling `told`; its draft 1 holds
/// "Plot 14", which the disk refused: the keeper's hooks still refuse.
fn refusing(dir: &std::path::Path, told: &Told) -> (Desktop, Sender<Request>, inboxcraft_store::KeeperHooks) {
    let mut store = inboxcraft_store::Sqlite::open(dir, inboxcraft_store::StoreConfig::default()).unwrap();
    store.apply(&Batch::new().with(Change::PutAccount(account()))).unwrap();
    store.keep_drafts_behind().unwrap();
    let hooks = store.keeper_hooks().unwrap();
    let mut mailbox = Mailbox::new(Session::new(Box::new(store), clock()).unwrap(), clock());
    let id = mailbox.run_for_agent("compose.new", &json!({})).unwrap()["id"].as_u64().unwrap();
    hooks.refuse(true);
    mailbox.run_for_agent("compose.update", &json!({ "id": id, "text": "Plot 14" })).unwrap();
    let (send, heard) = mpsc::channel();
    let (_click, clicks) = mpsc::channel();
    (Desktop::new(InboxApp::new(mailbox), heard).with_notices(Box::new(told.clone()), clicks), send, hooks)
}

/// A quit without a window whose draft the disk refused says so in a notification: there is no window to say it in
/// (#338).
#[test]
fn a_quit_without_a_window_says_a_draft_not_kept_in_a_notification() {
    let dir = tempfile::tempdir().unwrap();
    let told = Told::default();
    let (desktop, send, _hooks) = refusing(dir.path(), &told);
    let mut desktop = desktop.without_window();
    send.send(Request::Quit).unwrap();
    desktop.wait(SHORT);
    assert!(desktop.is_quitting());
    let shown = told.shown.lock().unwrap();
    assert_eq!(
        shown.iter().filter(|shown| shown.key == "draft-not-kept").count(),
        1,
        "{:?}",
        shown.iter().map(|shown| &shown.key).collect::<Vec<_>>()
    );
}

/// A close of a window that ends InboxCraft, with a draft the disk refused, is taken back: the window stays and
/// InboxCraft does not end; a second close goes through (#338).
#[test]
fn a_close_with_a_draft_not_kept_keeps_the_window_and_inboxcraft() {
    let dir = tempfile::tempdir().unwrap();
    let told = Told::default();
    let (desktop, _send, _hooks) = refusing(dir.path(), &told);
    let mut desktop = desktop.closing_ends();
    let ctx = egui::Context::default();
    let frame = |desktop: &mut Desktop| {
        let mut info = ViewportInfo::default();
        info.events.push(ViewportEvent::Close);
        let mut raw = RawInput::default();
        raw.viewports.insert(ViewportId::ROOT, info);
        let mut output = ctx.run_ui(raw, |ui| {
            desktop.tick(ui.ctx());
            desktop.frame(ui);
        });
        // Nothing paints these: the frame's textures are dropped unused.
        output.textures_delta.clear();
        output.viewport_output.get(&ViewportId::ROOT).map(|output| output.commands.clone()).unwrap_or_default()
    };
    assert!(frame(&mut desktop).contains(&ViewportCommand::CancelClose));
    assert!(desktop.has_window() && !desktop.is_quitting(), "{desktop:?}");
    assert!(!frame(&mut desktop).contains(&ViewportCommand::CancelClose), "a second close goes through");
    assert!(!desktop.has_window() && desktop.is_quitting());
}
