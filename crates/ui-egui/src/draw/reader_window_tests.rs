// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A message in a window of its own, headless: no real window ever opens. A renderer on the test's thread runs each
//! window's pass inside the main one, as eframe does, with input of the window's own, and notes what it painted and
//! named; a context that embeds viewports (egui's default, the web app) is the fallback.

use std::cell::{Cell, RefCell};

use egui::{Context, Event, Key, Modifiers, Pos2, RawInput, Rect, ViewportId, ViewportInfo, vec2};
use inboxcraft_model::{AccountId, Event as MailEvent, EventKind, FolderId, FolderState, Uid, UidSet, UidValidity};
use inboxcraft_time::{FakeClock, Timestamp};
use serde_json::json;

use super::super::InboxApp;
use super::{COLUMN, MOST};
use crate::keymap;
use crate::mailbox::{Mailbox, Place};
use crate::reader::tests::Recorder;
use crate::sample;
use crate::save::NoSave;
use crate::theme::install_fonts;

/// What each window painted and named in a frame: its texts and its accessible labels, with where they stand.
type Seen = Vec<(ViewportId, Vec<(String, Rect)>)>;

thread_local! {
    /// The events the next pass of each window gets.
    static INPUT: RefCell<Vec<(ViewportId, Event)>> = const { RefCell::new(Vec::new()) };
    /// Whether the windows have the desktop's focus.
    static FOCUSED: Cell<bool> = const { Cell::new(true) };
    /// What each window painted and named in the last main frame: its texts and its accessible labels, with where they
    /// stand.
    static SEEN: RefCell<Seen> = const { RefCell::new(Vec::new()) };
}

const LIBRARY: &str = "Your library loan";
const COAST: &str = "Photos from the coast walk";
const BAKERY: &str = "Draft logo for the bakery";

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

/// The sample at the mail, messages opening in windows of their own.
fn app(fetch: &Recorder) -> InboxApp {
    let mailbox = Mailbox::new(sample::session(clock()).unwrap(), clock());
    let mut app = InboxApp::with_outside_at_mail(mailbox, Box::new(fetch.clone()), Box::new(NoSave));
    app.set_appearance("ui.layout.set", &json!({ "reader": "window" })).unwrap();
    app
}

fn texts(shape: &egui::Shape, out: &mut Vec<(String, Rect)>) {
    match shape {
        egui::Shape::Vec(inner) => inner.iter().for_each(|shape| texts(shape, out)),
        egui::Shape::Text(text) => out.push((text.galley.text().to_owned(), text.visual_bounding_rect())),
        _ => {}
    }
}

/// A context that opens windows of their own: each window's pass runs at once, inside the main one.
fn windowing() -> Context {
    Context::set_immediate_viewport_renderer(|ctx, mut viewport| {
        let id = viewport.ids.this;
        let events = INPUT.with(|input| {
            let (mine, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut *input.borrow_mut()).into_iter().partition(|(at, _)| *at == id);
            *input.borrow_mut() = rest;
            mine.into_iter().map(|(_, event)| event).collect()
        });
        let mut raw =
            RawInput { events, viewport_id: id, screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(960.0, 900.0))), ..RawInput::default() };
        raw.viewports.insert(id, ViewportInfo { focused: Some(FOCUSED.get()), ..ViewportInfo::default() });
        let mut output = ctx.run_ui(raw, |ui| (viewport.viewport_ui_cb)(ui));
        output.textures_delta.clear();
        let mut seen = Vec::new();
        output.shapes.iter().for_each(|clipped| texts(&clipped.shape, &mut seen));
        if let Some(update) = &output.platform_output.accesskit_update {
            for (_, node) in &update.nodes {
                if let (Some(label), Some(at)) = (node.label(), node.bounds()) {
                    seen.push((label.to_owned(), Rect::from_min_max(Pos2::new(at.x0 as f32, at.y0 as f32), Pos2::new(at.x1 as f32, at.y1 as f32))));
                }
            }
        }
        SEEN.with(|all| all.borrow_mut().push((id, seen)));
    });
    let ctx = Context::default();
    install_fonts(&ctx);
    ctx.enable_accesskit();
    ctx.set_embed_viewports(false);
    ctx
}

/// One frame of the main window with `main` as its events.
fn frame(ctx: &Context, app: &mut InboxApp, main: Vec<Event>) {
    SEEN.with(|seen| seen.borrow_mut().clear());
    let raw = RawInput { events: main, screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1280.0, 800.0))), ..RawInput::default() };
    let mut output = ctx.run_ui(raw, |ui| app.ui(ui));
    output.textures_delta.clear();
}

fn settle(ctx: &Context, app: &mut InboxApp) {
    for _ in 0..4 {
        frame(ctx, app, Vec::new());
    }
}

fn key(key: Key) -> Event {
    Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }
}

/// `key` pressed in window `at`, then the frames it takes.
fn press_in(ctx: &Context, app: &mut InboxApp, at: ViewportId, pressed: Key) {
    INPUT.with(|input| input.borrow_mut().push((at, key(pressed))));
    settle(ctx, app);
}

/// A press of the pointer on what window `at` names `label`, then the frames it takes.
fn click_in(ctx: &Context, app: &mut InboxApp, at: ViewportId, label: &str) {
    let rect = seen_in(at).into_iter().rfind(|(named, _)| named == label).unwrap().1;
    INPUT.with(|input| input.borrow_mut().push((at, Event::PointerMoved(rect.center()))));
    frame(ctx, app, Vec::new());
    for pressed in [true, false] {
        let button = Event::PointerButton { pos: rect.center(), button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
        INPUT.with(|input| input.borrow_mut().extend([(at, Event::PointerMoved(rect.center())), (at, button)]));
        frame(ctx, app, Vec::new());
    }
    settle(ctx, app);
}

fn id_of(app: &InboxApp, subject: &str) -> u64 {
    app.mailbox().messages().iter().find(|row| row.subject.starts_with(subject)).unwrap().id
}

/// The owner opens the message of `subject` from the list: selected, then `Enter`.
fn open(ctx: &Context, app: &mut InboxApp, subject: &str) -> u64 {
    let id = id_of(app, subject);
    app.mailbox_mut().select(Some(id));
    frame(ctx, app, Vec::new());
    app.run_action(ctx, keymap::OPEN);
    settle(ctx, app);
    id
}

/// The viewport of the window showing message `id`.
fn window_of(app: &InboxApp, id: u64) -> ViewportId {
    app.windows.shown.iter().find(|shown| shown.message == id).unwrap().viewport
}

/// What window `at` painted and named in the last frame.
fn seen_in(at: ViewportId) -> Vec<(String, Rect)> {
    SEEN.with(|seen| seen.borrow().iter().filter(|(id, _)| *id == at).flat_map(|(_, seen)| seen.clone()).collect())
}

fn shows(at: ViewportId, words: &str) -> bool {
    seen_in(at).iter().any(|(text, _)| text.contains(words))
}

/// `screen/reader-window/default`: the message the owner opened shows in a window of its own: its tools, its subject,
/// the why line, the answers at the foot, in a column 760 wide in the middle of the 960 window; the pane lets go of it.
#[test]
fn a_message_opened_from_the_list_shows_in_a_window_of_its_own_as_the_default_frame_draws_it() {
    let ctx = windowing();
    let mut app = app(&Recorder::default());
    settle(&ctx, &mut app);
    let id = open(&ctx, &mut app, BAKERY);
    assert_eq!(app.windows.messages(), vec![Some(id)], "one window, on the message");
    assert_eq!(app.mailbox().selected(), None, "the pane lets go of it");
    assert_eq!(app.reader().open_id(), None);
    let window = window_of(&app, id);
    for words in [BAKERY, "Done", "Snooze", "Pin", "Move to a folder", "Delete", "More", "Reply", "Forward"] {
        assert!(shows(window, words), "{words} in the window: {:?}", seen_in(window));
    }
    let why = app.windows.shown.first().unwrap().reader.why().first().cloned().unwrap();
    assert!(shows(window, &why), "the why line as in the pane: {why}");
    // The column: Reply 40 in from its left edge, the column 100 in from the window's.
    // The button's own rect, as a screen reader is told (the last thing named Reply: the shapes come first).
    let reply = seen_in(window).into_iter().rfind(|(label, _)| label == "Reply").unwrap().1;
    let column_left = (960.0 - COLUMN) / 2.0;
    assert!((reply.left() - (column_left + 40.0)).abs() <= 1.5, "Reply at {reply:?}");
    // Nothing of the window drawn in the main window: the main pass painted no subject of it in a reader.
    assert!(app.windows.shown.first().unwrap().reader.opened(id).is_some(), "the window's reader holds the message");
}

/// `screen/reader-window/unfocused`: with another window in front the window keeps its message and its controls.
#[test]
fn an_unfocused_window_keeps_its_message_and_its_controls() {
    let ctx = windowing();
    let mut app = app(&Recorder::default());
    settle(&ctx, &mut app);
    let id = open(&ctx, &mut app, BAKERY);
    let window = window_of(&app, id);
    FOCUSED.set(false);
    settle(&ctx, &mut app);
    FOCUSED.set(true);
    for words in [BAKERY, "Done", "Reply", "Close", "Minimise"] {
        assert!(shows(window, words), "{words}: {:?}", seen_in(window));
    }
    assert_eq!(app.windows.messages(), vec![Some(id)]);
}

/// Two readers at once: each window keeps its own message, an Esc in one closes only that one, and the list moving on
/// changes neither.
#[test]
fn two_readers_open_at_once_each_keep_their_message() {
    let ctx = windowing();
    let mut app = app(&Recorder::default());
    settle(&ctx, &mut app);
    let coast = open(&ctx, &mut app, COAST);
    let bakery = open(&ctx, &mut app, BAKERY);
    assert_eq!(app.windows.messages(), vec![Some(coast), Some(bakery)]);
    let (first, second) = (window_of(&app, coast), window_of(&app, bakery));
    assert_ne!(first, second);
    assert!(shows(first, COAST) && !shows(first, BAKERY), "{:?}", seen_in(first));
    assert!(shows(second, BAKERY) && !shows(second, COAST), "{:?}", seen_in(second));
    // The list moves on: another place, another row selected.
    app.mailbox_mut().open(Place::Folder(3));
    let other = app.mailbox().messages().first().unwrap().id;
    app.mailbox_mut().select(Some(other));
    settle(&ctx, &mut app);
    assert_eq!(app.windows.messages(), vec![Some(coast), Some(bakery)], "the windows keep their messages");
    assert!(shows(first, COAST) && shows(second, BAKERY));
    assert!(shows(first, "Pin"), "the head keeps its tools once the list moved on");
    // A menu opened in one window is that window's alone.
    assert!(!shows(first, "Archive") && !shows(second, "Archive"));
    click_in(&ctx, &mut app, first, "More");
    assert!(shows(first, "Archive"), "the More menu opens: {:?}", seen_in(first));
    assert!(!shows(second, "Archive"), "and only in its window: {:?}", seen_in(second));
    // Under the window's menu no key acts on its message.
    press_in(&ctx, &mut app, first, Key::D);
    assert_eq!(app.windows.messages(), vec![Some(coast), Some(bakery)], "D under the menu took nothing away");
    press_in(&ctx, &mut app, first, Key::Escape);
    assert_eq!(app.windows.messages(), vec![Some(coast), Some(bakery)], "the Esc closed the menu, not the window");
    // Opening the same message again brings its window to the front, no second one.
    app.mailbox_mut().open(Place::View(inboxcraft_model::Role::Inbox));
    settle(&ctx, &mut app);
    open(&ctx, &mut app, COAST);
    assert_eq!(app.windows.messages(), vec![Some(coast), Some(bakery)]);
    press_in(&ctx, &mut app, first, Key::Escape);
    assert_eq!(app.windows.messages(), vec![Some(bakery)], "only the window the Esc was pressed in closes");
}

/// An act by key in the window acts on its message, never on the row the list rings in the main window; an act that
/// takes the message away closes the window.
#[test]
fn a_key_in_the_window_acts_on_its_message_never_on_the_list_s_row() {
    let ctx = windowing();
    let mut app = app(&Recorder::default());
    settle(&ctx, &mut app);
    let coast = open(&ctx, &mut app, COAST);
    let window = window_of(&app, coast);
    // The list moves on and rings another row.
    let ringed = id_of(&app, LIBRARY);
    app.mailbox_mut().select(Some(ringed));
    settle(&ctx, &mut app);
    let (before, ringed_before) = (app.mailbox_mut().message_row(coast).unwrap(), app.mailbox_mut().message_row(ringed).unwrap());
    press_in(&ctx, &mut app, window, Key::D);
    let after = app.mailbox_mut().message_row(coast).unwrap();
    assert!(after.done || after.folder != before.folder, "the window's message is done: {after:?}");
    assert!(app.mailbox().messages().iter().all(|row| row.id != coast), "it left the inbox");
    assert_eq!(app.mailbox_mut().message_row(ringed).unwrap(), ringed_before, "the list's row is as it was");
    assert_eq!(app.mailbox().selected(), Some(ringed), "the list stays where it was");
    assert!(app.windows.messages().is_empty(), "done, the window's work ends");
    // A press on a button in the window acts on its message too: Pin, which keeps the window.
    let bakery = open(&ctx, &mut app, BAKERY);
    let window = window_of(&app, bakery);
    click_in(&ctx, &mut app, window, "Pin");
    assert!(app.mailbox().messages().iter().any(|row| row.id == bakery && row.pinned), "the window's message is pinned");
    assert!(shows(window, "Take the pin off"), "and its window says so");
    // The list moves on: the window keeps what it knows of the pin.
    app.mailbox_mut().open(Place::Folder(3));
    settle(&ctx, &mut app);
    assert!(app.mailbox().messages().iter().all(|row| row.id != bakery), "the list shows it no more");
    assert!(shows(window, "Take the pin off"), "{:?}", seen_in(window));
    assert_eq!(app.windows.messages(), vec![Some(bakery)], "a pin keeps the window");
}

/// The server takes the window's message away: the window says so, keeps nothing of it and acts on nothing.
#[test]
fn a_message_the_server_takes_away_leaves_its_window_saying_so() {
    let ctx = windowing();
    let mut app = app(&Recorder::default());
    settle(&ctx, &mut app);
    let library = open(&ctx, &mut app, LIBRARY);
    let window = window_of(&app, library);
    assert!(shows(window, LIBRARY));
    let validity = UidValidity::new(1).unwrap();
    let synced = EventKind::FolderSynced { folder: FolderId::new(1), state: FolderState::new(validity) };
    app.mailbox_mut().tell(&MailEvent::new(AccountId::new(1), None, synced)).unwrap();
    let gone = EventKind::MessagesExpunged { folder: FolderId::new(1), uid_validity: validity, uids: UidSet::from_iter([Uid::new(2).unwrap()]) };
    app.mailbox_mut().tell(&MailEvent::new(AccountId::new(1), None, gone)).unwrap();
    app.mailbox_mut().refresh();
    settle(&ctx, &mut app);
    assert!(app.mailbox_mut().message_row(library).is_none(), "the store keeps it no more");
    assert!(shows(window, "This message is not here any"), "{:?}", seen_in(window));
    assert!(!shows(window, LIBRARY), "nothing of it stays: {:?}", seen_in(window));
    let shown = app.windows.shown.first().unwrap();
    assert!(shown.reader.open_id().is_none() && shown.reader.thread().is_empty() && shown.textures.is_empty());
    let runs = app.mailbox().runs();
    for pressed in [Key::D, Key::A, Key::Delete, Key::R] {
        press_in(&ctx, &mut app, window, pressed);
    }
    assert_eq!(app.mailbox().runs(), runs, "no act on a message that is gone");
    press_in(&ctx, &mut app, window, Key::Escape);
    assert!(app.windows.messages().is_empty());
}

/// Where no window of its own can open (egui embeds viewports: the web app, a headless run) the message opens in the
/// pane, and the status line says why once.
#[test]
fn where_no_window_can_open_the_message_opens_in_the_pane_and_the_status_line_says_why_once() {
    let ctx = Context::default();
    install_fonts(&ctx);
    let mut app = app(&Recorder::default());
    settle(&ctx, &mut app);
    let coast = open(&ctx, &mut app, COAST);
    assert!(app.windows.messages().is_empty(), "no window");
    assert_eq!(app.reader().open_id(), Some(coast), "the pane opened it");
    assert_eq!(app.notice(), Some("Messages open in the pane: no window of their own can open here."));
    let waiting = app.status.waiting();
    let bakery = id_of(&app, BAKERY);
    assert!(!app.open_in_window(bakery), "the pane opens it");
    assert_eq!(app.status.waiting(), waiting, "said once");
}

/// With the pane chosen (the default) a message opens in the pane, as before.
#[test]
fn with_the_pane_chosen_a_message_opens_in_the_pane() {
    let ctx = windowing();
    let mut app = app(&Recorder::default());
    app.set_appearance("ui.layout.set", &json!({ "reader": "docked" })).unwrap();
    settle(&ctx, &mut app);
    let coast = open(&ctx, &mut app, COAST);
    assert!(app.windows.messages().is_empty());
    assert_eq!(app.reader().open_id(), Some(coast));
}

/// At most [`MOST`] windows: one more closes the oldest.
#[test]
fn one_window_too_many_closes_the_oldest() {
    let ctx = windowing();
    let mut app = app(&Recorder::default());
    settle(&ctx, &mut app);
    let rows: Vec<String> = app.mailbox().messages().iter().map(|row| row.subject.clone()).collect();
    assert!(rows.len() > MOST, "the sample has rows enough");
    let opened: Vec<u64> = rows.iter().take(MOST + 1).map(|subject| open(&ctx, &mut app, subject)).collect();
    let open_now: Vec<u64> = app.windows.messages().into_iter().flatten().collect();
    assert_eq!(open_now, opened[1..], "the oldest closed");
}

/// #165: a picture shown in one window is drawn from a texture of the next window's context, not from the old one's.
#[test]
fn a_picture_is_uploaded_again_in_a_new_context() {
    let fetch = Recorder::default();
    let mut app = app(&fetch);
    app.set_appearance("ui.layout.set", &json!({ "reader": "docked" })).unwrap();
    app.mailbox_mut().open(Place::Folder(5));
    let id = id_of(&app, "You have won a lighthouse");
    app.mailbox_mut().select(Some(id));
    let first = Context::default();
    install_fonts(&first);
    settle(&first, &mut app);
    let (reader, mailbox) = app.reader_mut();
    reader.allow_always(mailbox, id);
    settle(&first, &mut app);
    let url = "https://track.example.net/open.png?id=4417";
    assert!(app.textures.contains_key(url), "the picture is drawn");
    let next = Context::default();
    install_fonts(&next);
    settle(&next, &mut app);
    let texture = app.textures.get(url).unwrap().id();
    let named = next.tex_manager().read().meta(texture).map(|meta| meta.name.clone());
    assert_eq!(named.as_deref(), Some(url), "the texture is the new context's own");
}
