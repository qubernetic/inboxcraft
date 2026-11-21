// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The sync control at the foot of the calendar's side panel in the window: its four states as the frames draw them,
//! in light and dark, the command run on the owner's press and on nothing else, one sync at a time, the keyboard on
//! one control through every state, the rest between frames, and the event editor's typing untouched by a sync.
//!
//! The window runs no worker here: [`Feed`] stands in for the `net` workers, writes down every job handed to it and
//! hands back the events a test queues, as a worker would answer.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use egui::accesskit::Role;
use egui::{Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _, by};
use inboxcraft_model::{Account, AccountId, CalendarJob, CalendarListing, CalendarReport, ErrorKind, Event, EventKind, Job, JobId, JobKind};
use inboxcraft_time::{FakeClock, Timestamp};
use serde_json::json;

use super::sync_id;
use crate::calendar::sync::SyncState;
use crate::mailbox::Mailbox;
use crate::workers::Workers;
use crate::{InboxApp, sample};

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

/// What the stand-in workers were handed and what they answer next.
#[derive(Debug, Default)]
struct Fed {
    sent: Vec<(JobId, AccountId, JobKind)>,
    answers: Vec<Event>,
}

/// Workers that run nothing: they keep the jobs handed to them and give back the events queued in [`Fed`].
#[derive(Debug, Clone, Default)]
struct Feed(Arc<Mutex<Fed>>);

impl Workers for Feed {
    fn send(&mut self, _account: &Account, job: Job) {
        self.0.lock().unwrap().sent.push((job.id, job.account, job.kind));
    }

    fn keep(&mut self, _accounts: &[AccountId]) {}

    fn events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.0.lock().unwrap().answers)
    }
}

impl Feed {
    /// The calendar discoveries and syncs handed out so far.
    fn calendar_jobs(&self) -> Vec<(JobId, AccountId, CalendarJob)> {
        let fed = self.0.lock().unwrap();
        fed.sent
            .iter()
            .filter_map(|(id, account, kind)| match kind {
                JobKind::Calendar(job @ (CalendarJob::Discover { .. } | CalendarJob::Sync { .. })) => Some((*id, *account, job.clone())),
                _ => None,
            })
            .collect()
    }

    fn discoveries(&self) -> usize {
        self.calendar_jobs().iter().filter(|(_, _, job)| matches!(job, CalendarJob::Discover { .. })).count()
    }

    /// Queues `report` as the answer to job `job` of `account`.
    fn answer(&self, account: AccountId, job: JobId, report: CalendarReport) {
        self.0.lock().unwrap().answers.push(Event::new(account, Some(job), EventKind::Calendar(report)));
    }

    /// Answers every discovery handed out with the calendars account 1 holds, and every sync with nothing changed.
    fn answer_all(&self, done: &mut Vec<JobId>) {
        for (id, account, job) in self.calendar_jobs() {
            if done.contains(&id) {
                continue;
            }
            done.push(id);
            let report = match job {
                CalendarJob::Discover { .. } => CalendarReport::Found {
                    home: "https://dav.example.org/calendars/mira/".to_owned(),
                    calendars: ["personal", "choir"].map(listing).to_vec(),
                },
                CalendarJob::Sync { calendar, .. } => {
                    CalendarReport::Synced { calendar, changed: Vec::new(), removed: Vec::new(), token: None, full: true }
                }
                _ => continue,
            };
            self.answer(account, id, report);
        }
    }
}

fn listing(name: &str) -> CalendarListing {
    CalendarListing {
        url: format!("https://dav.example.org/calendars/mira/{name}/"),
        name: None,
        colour: None,
        ctag: None,
        sync_token: None,
        events: true,
    }
}

/// The sample window, its first account with a CalDAV address, on the calendar screen, its jobs going to `feed`.
fn on_calendar(theme: egui::Theme, feed: &Feed) -> Harness<'static, InboxApp> {
    on_calendar_named(theme, feed, None)
}

/// [`on_calendar`], the first account named `label` by the owner when it is given.
fn on_calendar_named(theme: egui::Theme, feed: &Feed, label: Option<&str>) -> Harness<'static, InboxApp> {
    let mut session = sample::session(clock()).unwrap();
    session.run("calendar.server", &json!({ "account": 1, "url": "https://dav.example.org/" })).unwrap();
    if let Some(label) = label {
        session.run("account.update", &json!({ "account": 1, "label": label })).unwrap();
    }
    let keychain = Arc::new(crate::onboard::NoKeychain);
    let mut app =
        InboxApp::at_mail(Mailbox::new(session, clock())).with_accounts(keychain, Box::<crate::onboard::NoDetect>::default(), Box::new(feed.clone()));
    // The side panel stays beside the small editor only.
    app.set_event_editor_host(inboxcraft_engine::appearance::EventEditorHost::Small);
    let mut harness = Harness::builder().with_size(vec2(1200.0, 820.0)).with_theme(theme).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Calendar").click();
    harness.run();
    // The address's own discovery, when it was set, is not the owner's sync: it is left unanswered.
    feed.0.lock().unwrap().sent.clear();
    harness
}

/// The label of the sync control.
fn control(harness: &Harness<'static, InboxApp>) -> String {
    let node = harness.query_all(by().role(Role::Button)).find(|node| node.accesskit_node().label().is_some_and(|label| is_sync(label.as_str())));
    node.and_then(|node| node.accesskit_node().label()).unwrap_or_default()
}

fn is_sync(label: &str) -> bool {
    ["sync now", "syncing calendars", "try again"].iter().any(|words| label.contains(words))
}

fn state(harness: &Harness<'static, InboxApp>) -> SyncState {
    harness.state().calendar().sync_view().state
}

fn started(harness: &Harness<'static, InboxApp>) -> u64 {
    crate::calendar::sync::with(&harness.state().mailbox().calendar_sync(), |tracker| tracker.started())
}

fn has_focus(harness: &Harness<'static, InboxApp>) -> bool {
    harness.ctx.memory(|memory| memory.focused()) == Some(sync_id())
}

/// Puts the keyboard on the control and presses `key` there.
fn press_key(harness: &mut Harness<'static, InboxApp>, key: Key) {
    harness.ctx.memory_mut(|memory| memory.request_focus(sync_id()));
    harness.run_steps(2);
    harness.key_press(key);
    harness.run_steps(3);
}

/// Runs a few frames: the window hands out the jobs and takes the events queued.
fn frames(harness: &mut Harness<'static, InboxApp>) {
    harness.run_steps(4);
}

#[test]
fn the_foot_draws_idle_syncing_synced_and_failed_as_the_frames_do() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let feed = Feed::default();
        let mut harness = on_calendar(theme, &feed);
        assert_eq!(state(&harness), SyncState::Idle);
        assert_eq!(control(&harness), "calendars not read yet, sync now", "before any read the idle words have no time");
        harness.get_by_label_contains("sync now").click();
        frames(&mut harness);
        assert_eq!(feed.discoveries(), 1, "the press asked account 1's server for its calendars");
        assert_eq!(control(&harness), "syncing calendars: Personal, 0 of 0");
        let mut done = Vec::new();
        feed.answer_all(&mut done);
        frames(&mut harness);
        assert!(matches!(state(&harness), SyncState::Syncing { done: 0, of: 2, .. }), "the calendars found are synced: {:?}", state(&harness));
        feed.answer_all(&mut done);
        frames(&mut harness);
        let SyncState::Synced { at: Some(at) } = state(&harness) else { panic!("{:?}", state(&harness)) };
        assert_eq!(control(&harness), format!("calendars synced {}, sync now", at.clock()));
        assert_eq!(harness.state().calendar().sync_view().read, Some(at), "the read's time is the sync's");
        // The side panel's account head (a label); the index's account row of the same name is a button.
        assert!(
            harness.query_all(by().role(Role::Label).label_contains("Personal")).next().is_some(),
            "the calendars kept by the discovery still show"
        );
        // A second sync fails at sign-in.
        harness.get_by_label_contains("sync now").click();
        frames(&mut harness);
        let (job, account, _) = feed.calendar_jobs().into_iter().rfind(|(_, _, job)| matches!(job, CalendarJob::Discover { .. })).unwrap();
        feed.answer(account, job, CalendarReport::Failed { calendar: None, kind: ErrorKind::LoginRefused });
        frames(&mut harness);
        assert_eq!(control(&harness), "Personal calendars: sign-in refused, try again");
        harness.get_by_label("open log");
        assert_eq!(started(&harness), 2);
        harness.get_by_label_contains(", try again").click();
        frames(&mut harness);
        assert_eq!(started(&harness), 3, "a failure offers the press again");
        assert!(matches!(state(&harness), SyncState::Syncing { .. }));
    }
}

#[test]
fn a_failure_names_the_account_on_one_clean_capped_line() {
    let feed = Feed::default();
    // The owner's name for the account, long and with a bidi override in it.
    let label = format!("Stu\u{202E}dio {}", "x".repeat(300));
    let mut harness = on_calendar_named(egui::Theme::Dark, &feed, Some(&label));
    // The engine keeps the name without the override; its length is the window's to cap.
    let kept = harness.state().mailbox().accounts().iter().find(|account| account.id == 1).map(|account| account.label.chars().count());
    assert!(kept.unwrap() > crate::calendar::sync::MOST_NAME, "{kept:?}");
    harness.get_by_label_contains("sync now").click();
    frames(&mut harness);
    let (job, account, _) = feed.calendar_jobs().into_iter().next().unwrap();
    feed.answer(account, job, CalendarReport::Failed { calendar: None, kind: ErrorKind::CertificateExpired });
    frames(&mut harness);
    let SyncState::Failed { account, why } = state(&harness) else { panic!("{:?}", state(&harness)) };
    let account = account.unwrap_or_default();
    assert!(account.chars().count() <= crate::calendar::sync::MOST_NAME, "{}", account.chars().count());
    assert!(!account.chars().any(|c| c.is_control() || c == '\u{202E}'), "{account:?}");
    assert_eq!(why.words(), "not secure", "the window's words, never a server's");
    let link = harness.get_by_label_contains(", try again").rect();
    assert!(link.right() <= 1200.0 - 15.0 + 0.5 && link.bottom() <= 820.0, "{link:?}");
}

#[test]
fn the_command_runs_on_the_owner_s_press_and_on_nothing_else() {
    let feed = Feed::default();
    let mut harness = on_calendar(egui::Theme::Light, &feed);
    let untouched = |harness: &Harness<'static, InboxApp>, what: &str| {
        assert_eq!(started(harness), 0, "{what} ran calendar.sync");
        assert_eq!(feed.discoveries(), 0, "{what} handed out a discovery");
        assert_ne!(harness.state().mailbox().last_run(), Some("calendar.sync"), "{what}");
    };
    untouched(&harness, "opening the calendar");
    // Time passing: a minute and more of frames.
    for _ in 0..3 {
        harness.ctx.request_repaint_after(Duration::from_secs(61));
        harness.run_steps(20);
    }
    untouched(&harness, "a timer");
    let at = harness.get_by_label_contains("sync now").rect().center();
    harness.hover_at(at);
    harness.run_steps(4);
    untouched(&harness, "a hover");
    harness.ctx.memory_mut(|memory| memory.request_focus(sync_id()));
    harness.run_steps(4);
    assert!(has_focus(&harness));
    untouched(&harness, "a focus change");
    // A view picked, a step, the calendar opened again: the calendars are read, never synced.
    harness.get_by_role_and_label(Role::RadioButton, "Month").click();
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Mail").click();
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Calendar").click();
    harness.run();
    untouched(&harness, "reading the calendars again");
    // An event picked: its details are a preview of it, and run nothing either.
    harness.query_all(by().role(Role::Button).label_contains("November")).next().unwrap().click();
    harness.run();
    untouched(&harness, "a preview of an event");
    press_key(&mut harness, Key::Space);
    frames(&mut harness);
    assert_eq!(started(&harness), 1, "Space on the control is the owner's press");
    assert_eq!(feed.discoveries(), 1);
}

#[test]
fn a_second_press_while_syncing_hands_out_no_second_job() {
    let feed = Feed::default();
    let mut harness = on_calendar(egui::Theme::Light, &feed);
    press_key(&mut harness, Key::Enter);
    frames(&mut harness);
    assert!(matches!(state(&harness), SyncState::Syncing { .. }));
    for key in [Key::Enter, Key::Space] {
        press_key(&mut harness, key);
        frames(&mut harness);
    }
    let at = harness.get_by_label_contains("syncing calendars").rect().center();
    click_at(&mut harness, at);
    frames(&mut harness);
    assert_eq!(started(&harness), 1, "one sync of the owner's at a time");
    assert_eq!(feed.discoveries(), 1, "no second job");
}

#[test]
fn the_keyboard_stays_on_the_one_control_through_every_state() {
    let feed = Feed::default();
    let mut harness = on_calendar(egui::Theme::Dark, &feed);
    // Tab from the last calendar's box (account 2's Studio) reaches the control next, once.
    harness.get_by_role_and_label(Role::CheckBox, "Studio").focus();
    harness.run();
    let mut stops = Vec::new();
    for _ in 0..3 {
        harness.key_press(Key::Tab);
        harness.run_steps(2);
        stops.push(harness.ctx.memory(|memory| memory.focused()));
    }
    assert_eq!(stops.first().copied().flatten(), Some(sync_id()), "the control is the side panel's next Tab stop");
    assert_eq!(stops.iter().filter(|stop| **stop == Some(sync_id())).count(), 1, "one Tab stop");
    harness.key_press_modifiers(Modifiers::SHIFT, Key::Tab);
    harness.run_steps(2);
    press_key(&mut harness, Key::Enter);
    let mut done = Vec::new();
    for want in ["syncing calendars", "syncing calendars", "calendars synced"] {
        assert!(has_focus(&harness), "the keyboard stays on the control at {want}");
        assert!(control(&harness).contains(want), "{} for {want}", control(&harness));
        feed.answer_all(&mut done);
        frames(&mut harness);
    }
    assert!(has_focus(&harness));
    harness.key_press(Key::Enter);
    frames(&mut harness);
    let (job, account, _) = feed.calendar_jobs().into_iter().rfind(|(_, _, job)| matches!(job, CalendarJob::Discover { .. })).unwrap();
    feed.answer(account, job, CalendarReport::Failed { calendar: None, kind: ErrorKind::HostNotFound });
    frames(&mut harness);
    assert!(has_focus(&harness), "the keyboard stays on the control when it fails");
    assert_eq!(control(&harness), "Personal calendars: server not found, try again");
}

#[test]
fn the_foot_rests_and_only_a_running_sync_asks_for_its_ring_s_frames() {
    let feed = Feed::default();
    let mut harness = on_calendar(egui::Theme::Light, &feed);
    assert!(crate::component::tests::rests(&mut harness, 4), "idle rests");
    harness.get_by_label_contains("sync now").click();
    frames(&mut harness);
    // The progress line's ring turns a dot every 125 ms while the sync runs: a timed frame, never one at once.
    harness.run_steps(3);
    let delay = harness.output().viewport_output.values().map(|viewport| viewport.repaint_delay).min().unwrap_or(Duration::MAX);
    assert_eq!(delay, Duration::from_millis(125), "syncing asks for the ring's next step only");
    let mut done = Vec::new();
    feed.answer_all(&mut done);
    frames(&mut harness);
    feed.answer_all(&mut done);
    frames(&mut harness);
    assert!(matches!(state(&harness), SyncState::Synced { .. }));
    assert!(crate::component::tests::rests(&mut harness, 4), "synced rests");
    harness.get_by_label_contains("sync now").click();
    frames(&mut harness);
    let (job, account, _) = feed.calendar_jobs().into_iter().rfind(|(_, _, job)| matches!(job, CalendarJob::Discover { .. })).unwrap();
    feed.answer(account, job, CalendarReport::Failed { calendar: None, kind: ErrorKind::TimedOut });
    frames(&mut harness);
    assert!(matches!(state(&harness), SyncState::Failed { .. }));
    assert!(crate::component::tests::rests(&mut harness, 4), "failed rests");
}

#[test]
fn under_the_event_editor_the_control_rests_and_a_sync_leaves_the_typing_whole() {
    let feed = Feed::default();
    let mut harness = on_calendar(egui::Theme::Light, &feed);
    harness.get_by_label_contains("sync now").click();
    frames(&mut harness);
    harness.get_by_role_and_label(Role::Button, "New event").click();
    harness.run();
    harness.get_by_role_and_label(Role::TextInput, "Title").focus();
    harness.run();
    harness.get_by_role_and_label(Role::TextInput, "Title").type_text("Kiln firing");
    harness.run();
    // The caret two letters back from the end.
    for _ in 0..2 {
        harness.key_press(Key::ArrowLeft);
        harness.run();
    }
    // Under the editor the control takes neither the pointer nor the keyboard.
    let node =
        harness.query_all(by().role(Role::Button)).find(|node| node.accesskit_node().label().is_some_and(|label| is_sync(label.as_str()))).unwrap();
    assert!(node.accesskit_node().is_disabled(), "the control rests under the editor");
    // The sync's events come and go while the editor is open.
    let mut done = Vec::new();
    for _ in 0..2 {
        feed.answer_all(&mut done);
        frames(&mut harness);
    }
    assert!(matches!(state(&harness), SyncState::Synced { .. }), "{:?}", state(&harness));
    assert!(harness.state().calendar().editor().is_some(), "the editor is still open");
    harness.get_by_role_and_label(Role::TextInput, "Title").type_text("X");
    harness.run();
    assert_eq!(harness.get_by_role_and_label(Role::TextInput, "Title").value().as_deref(), Some("Kiln firiXng"), "the text and its caret are whole");
    // Tab inside the editor never reaches the control under it.
    for _ in 0..40 {
        harness.key_press(Key::Tab);
        harness.run_steps(2);
        assert!(!has_focus(&harness));
    }
    assert_eq!(started(&harness), 1);
}

/// A click of the pointer's first button at `at`.
fn click_at<State>(harness: &mut Harness<'_, State>, at: egui::Pos2) {
    harness.hover_at(at);
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
    }
}

#[test]
fn an_edit_of_an_event_stays_whole_while_a_sync_changes_that_event_and_fails() {
    let feed = Feed::default();
    let mut harness = on_calendar(egui::Theme::Dark, &feed);
    harness.get_by_label_contains("sync now").click();
    frames(&mut harness);
    let mut done = Vec::new();
    feed.answer_all(&mut done);
    frames(&mut harness);
    // The existing event in the editor, its title edited, the caret three letters from the end.
    harness.query_all(by().role(Role::Button).label_contains("Dentist")).next().unwrap().click();
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Edit").click();
    harness.run();
    assert!(
        harness.state().calendar().editor().is_some(),
        "Edit opened the editor; picked {:?}",
        harness.state().calendar().picked().map(|event| &event.summary)
    );
    harness.get_by_role_and_label(Role::TextInput, "Title").focus();
    harness.run();
    harness.key_press(Key::End);
    harness.run();
    harness.get_by_role_and_label(Role::TextInput, "Title").type_text(" day");
    harness.run();
    for _ in 0..3 {
        harness.key_press(Key::ArrowLeft);
        harness.run();
    }
    // The server changes that very event in Personal, and Choir's sync fails.
    for (id, account, job) in feed.calendar_jobs() {
        if done.contains(&id) {
            continue;
        }
        done.push(id);
        let CalendarJob::Sync { calendar, url, .. } = job else { continue };
        let report = if url.ends_with("/personal/") {
            let ical = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:dentist@example.org\r\nDTSTAMP:20261105T000000Z\r\n\
                        DTSTART:20261105T150000Z\r\nDTEND:20261105T160000Z\r\nSUMMARY:Dentist moved by the server\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n";
            let fetched = inboxcraft_model::FetchedEvent { url: format!("{url}dentist.ics"), etag: Some("\"9\"".to_owned()), ical: ical.to_owned() };
            CalendarReport::Synced { calendar, changed: vec![fetched], removed: Vec::new(), token: None, full: false }
        } else {
            CalendarReport::Failed { calendar: Some(calendar), kind: ErrorKind::LoginRefused }
        };
        feed.answer(account, id, report);
    }
    frames(&mut harness);
    assert!(matches!(state(&harness), SyncState::Failed { .. }), "{:?}", state(&harness));
    assert!(harness.query_by_label_contains("Dentist moved by the server").is_some(), "the server's change came in under the editor");
    assert!(harness.state().calendar().editor().is_some(), "the editor is still open");
    let title = harness.get_by_role_and_label(Role::TextInput, "Title");
    assert!(title.accesskit_node().is_focused(), "the keyboard is still in the title");
    title.type_text("X");
    harness.run();
    assert_eq!(harness.get_by_role_and_label(Role::TextInput, "Title").value().as_deref(), Some("Dentist Xday"), "the text and its caret are whole");
}

#[test]
fn calendars_found_on_another_server_end_as_a_failure_in_the_window_s_words() {
    let feed = Feed::default();
    let mut harness = on_calendar(egui::Theme::Light, &feed);
    harness.get_by_label_contains("sync now").click();
    frames(&mut harness);
    let (job, account, _) = feed.calendar_jobs().into_iter().next().unwrap();
    let elsewhere = CalendarListing { url: "https://calendars.example.net/mira/personal/".to_owned(), ..listing("personal") };
    feed.answer(account, job, CalendarReport::Found { home: "https://calendars.example.net/mira/".to_owned(), calendars: vec![elsewhere] });
    frames(&mut harness);
    assert_eq!(control(&harness), "Personal calendars: on another server, try again");
    assert!(harness.query_all_by_label_contains("Choir").next().is_some(), "the calendars kept stay");
}
