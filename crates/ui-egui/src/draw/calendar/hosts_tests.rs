// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The event editor's three hosts (#370), headless: no real window ever opens. New event and Edit open the editor
//! where Settings › Calendar › "New event opens" says: the small float, the full editor in the calendar's place with
//! the event's day beside it, or a window of its own, which a renderer on the test's thread runs inside the main pass
//! with input of its own, as eframe does. Where no window can open the editor stands full.

use std::cell::{Cell, RefCell};

use egui::accesskit::Role;
use egui::{Context, Event, Key, Modifiers, Pos2, RawInput, Rect, Vec2, ViewportEvent, ViewportInfo, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_engine::appearance::EventEditorHost;
use inboxcraft_time::{FakeClock, Timestamp};
use serde_json::json;

use super::Asked;
use crate::calendar::{EventRow, View};
use crate::draw::InboxApp;
use crate::mailbox::Mailbox;
use crate::sample;

/// 2026-11-05T09:00:00Z.
const NOW: i64 = 1_793_869_200;

/// What the editor's window showed in its last pass.
#[derive(Debug, Default, Clone)]
struct Seen {
    passes: usize,
    title: Option<String>,
    size: Option<Vec2>,
    /// Each control's role and name, as a screen reader hears them.
    named: Vec<(Role, String)>,
    /// Whether the keyboard was on a control at the end of the last pass.
    focused: bool,
}

thread_local! {
    static EVENTS: RefCell<Vec<Event>> = const { RefCell::new(Vec::new()) };
    static CLOSE: Cell<bool> = const { Cell::new(false) };
    static SEEN: RefCell<Seen> = RefCell::new(Seen::default());
}

fn seen() -> Seen {
    SEEN.with(|seen| seen.borrow().clone())
}

/// Whether the window's last pass had a control of `role` named `name`.
fn in_window(role: Role, name: &str) -> bool {
    seen().named.iter().any(|(r, n)| *r == role && n == name)
}

/// Lets `ctx` open windows of their own, simulated on this thread.
fn windowing(ctx: &Context) {
    EVENTS.with(|events| events.borrow_mut().clear());
    CLOSE.set(false);
    SEEN.with(|seen| *seen.borrow_mut() = Seen::default());
    Context::set_immediate_viewport_renderer(|ctx, mut viewport| {
        let this = viewport.ids.this;
        let mut info = ViewportInfo::default();
        if CLOSE.replace(false) {
            info.events.push(ViewportEvent::Close);
        }
        let mut raw = RawInput { events: EVENTS.with(|events| std::mem::take(&mut *events.borrow_mut())), ..RawInput::default() };
        raw.viewport_id = this;
        raw.time = Some(ctx.input_for(egui::ViewportId::ROOT, |input| input.time));
        raw.viewports.insert(this, info);
        raw.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, vec2(704.0, 800.0)));
        let mut focused = false;
        let mut output = ctx.run_ui(raw, |ui| {
            (viewport.viewport_ui_cb)(ui);
            focused = ui.ctx().memory(|memory| memory.focused()).is_some();
        });
        output.textures_delta.clear();
        SEEN.with(|seen| {
            let mut seen = seen.borrow_mut();
            seen.passes += 1;
            seen.title.clone_from(&viewport.builder.title);
            seen.size = viewport.builder.inner_size;
            seen.focused = focused;
            seen.named = output
                .platform_output
                .accesskit_update
                .iter()
                .flat_map(|update| update.nodes.iter())
                .filter_map(|(_, node)| Some((node.role(), node.label()?.to_owned())))
                .collect();
        });
    });
    ctx.set_embed_viewports(false);
}

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(NOW, 0).unwrap()))
}

/// The sample window on the calendar's week, New event opening as `host` says; windows open when `windows`.
fn app(host: EventEditorHost, windows: bool, theme: egui::Theme) -> Harness<'static, InboxApp> {
    let mut app = InboxApp::at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    app.set_event_editor_host(host);
    app.show_calendar();
    let (calendar, mailbox) = app.calendar_mut();
    calendar.set_view(mailbox, View::Week);
    let mut harness = Harness::builder()
        .with_size(vec2(1440.0, 900.0))
        .with_theme(theme)
        .with_max_steps(12)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    if windows {
        windowing(&harness.ctx);
    }
    harness.run();
    harness
}

/// New event, as its press asks.
fn new_event(harness: &mut Harness<'static, InboxApp>) {
    harness.state_mut().apply_calendar(vec![Asked::New]);
    harness.run();
}

/// Picks the event `summary` (the first of the owner's own with `None`) and opens it in the editor, as Edit does.
fn edit(harness: &mut Harness<'static, InboxApp>, summary: Option<&str>) {
    let (calendar, _) = harness.state_mut().calendar_mut();
    let key = calendar.events().find(|event| summary.map_or(event.own, |summary| event.summary == summary)).map(EventRow::key);
    assert!(key.is_some(), "the sample has {summary:?}");
    calendar.pick(key);
    harness.state_mut().apply_calendar(vec![Asked::Edit]);
    harness.run();
}

/// Types into the open editor's title, as the keyboard would.
fn typed(harness: &mut Harness<'static, InboxApp>, text: &str) {
    harness.state_mut().calendar_mut().0.editor_mut().unwrap().form_mut().summary.push_str(text);
    harness.run();
}

fn open(harness: &Harness<'static, InboxApp>) -> bool {
    harness.state().calendar().editor().is_some()
}

/// Where the main window shows the editor: `None` while it draws no title field of it.
fn main_title_width(harness: &Harness<'static, InboxApp>) -> Option<f32> {
    harness.query_by_role_and_label(Role::TextInput, "Title").map(|title| title.rect().width())
}

/// Runs the frames with `events` in the window's next pass.
fn window_keys(harness: &mut Harness<'static, InboxApp>, events: Vec<Event>) {
    EVENTS.with(|queued| *queued.borrow_mut() = events);
    harness.run();
}

fn esc() -> Event {
    Event::Key { key: Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }
}

/// The host the editor stands in now, as the screen shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shown {
    Small,
    Full,
    Window,
}

fn shown(harness: &Harness<'static, InboxApp>) -> Option<Shown> {
    let before = seen().passes;
    match main_title_width(harness) {
        // The float is 432 wide, its fields 400; the full editor's are the calendar's place less 80.
        Some(width) if width <= 400.0 => Some(Shown::Small),
        Some(_) => Some(Shown::Full),
        None if before > 0 && in_window(Role::TextInput, "Title") => Some(Shown::Window),
        None => None,
    }
}

#[test]
fn new_event_and_edit_open_the_editor_where_the_setting_says() {
    for (host, expected) in [(EventEditorHost::Small, Shown::Small), (EventEditorHost::Full, Shown::Full), (EventEditorHost::Window, Shown::Window)] {
        let mut harness = app(host, true, egui::Theme::Light);
        new_event(&mut harness);
        assert_eq!(shown(&harness), Some(expected), "{host:?}: New event");
        if expected == Shown::Window {
            assert_eq!(seen().title.as_deref(), Some("New event"));
            assert_eq!(seen().size, Some(vec2(704.0, 800.0)), "the window is 704 × 800");
            assert!(harness.query_by_role_and_label(Role::Button, "New event").is_some(), "the main window's calendar stays");
        }
        if expected == Shown::Full {
            assert!(harness.query_by_role_and_label(Role::Button, "New event").is_none(), "the editor takes the view's place");
        }
        harness.state_mut().apply_calendar(vec![Asked::Cancel]);
        harness.run();
        assert!(!open(&harness));
        edit(&mut harness, None);
        assert!(open(&harness), "{host:?}: Edit");
        assert_eq!(shown(&harness), Some(expected), "{host:?}: Edit follows the same setting");
    }
}

#[test]
fn the_full_editor_stands_in_the_calendar_s_place_with_the_event_s_day_beside_it_and_takes_the_keyboard() {
    let mut harness = app(EventEditorHost::Full, false, egui::Theme::Dark);
    new_event(&mut harness);
    let title = harness.get_by_role_and_label(Role::TextInput, "Title");
    // The calendar's card: the editor 40 in from its left, the day 320 wide at its right after a hairline.
    let card = harness.ctx.data(|data| data.get_temp::<Rect>(super::card_id())).unwrap();
    assert!((title.rect().left() - (card.left() + 40.0)).abs() <= 1.0, "40 in: {:?} in {card:?}", title.rect());
    assert!((title.rect().right() - (card.right() - 320.0 - 1.0 - 40.0)).abs() <= 1.0, "the day beside: {:?} in {card:?}", title.rect());
    assert!(title.is_focused(), "the keyboard goes to the title");
    harness.state_mut().apply_calendar(vec![Asked::Cancel]);
    harness.run();
    harness.run();
    assert!(harness.get_by_role_and_label(Role::Button, "New event").is_focused(), "and back to New event when it closes");
}

#[test]
fn where_no_window_can_open_the_editor_stands_full_and_the_status_line_says_why_once() {
    // A context that embeds its viewports: the web app's, a headless one.
    let mut harness = app(EventEditorHost::Window, false, egui::Theme::Light);
    new_event(&mut harness);
    assert_eq!(shown(&harness), Some(Shown::Full));
    harness.get_by_label_contains("No window of its own can open here");
    harness.state_mut().apply_calendar(vec![Asked::Cancel]);
    harness.state_mut().status.forget_notices();
    harness.run();
    new_event(&mut harness);
    assert_eq!(shown(&harness), Some(Shown::Full));
    assert!(harness.query_by_label_contains("No window of its own can open here").is_none(), "said once");
}

#[test]
fn esc_in_the_small_and_the_full_editor_warns_with_typing_and_a_second_throws_it_away() {
    for host in [EventEditorHost::Small, EventEditorHost::Full] {
        let mut harness = app(host, false, egui::Theme::Light);
        new_event(&mut harness);
        harness.key_press(Key::Escape);
        harness.run();
        assert!(!open(&harness), "{host:?}: nothing typed, one Esc closes it");
        new_event(&mut harness);
        typed(&mut harness, "Kiln");
        harness.key_press(Key::Escape);
        harness.run();
        let editor = harness.state().calendar().editor().cloned();
        assert!(editor.as_ref().is_some_and(crate::calendar::Editor::warned), "{host:?}: the typing stays and the editor says so");
        assert_eq!(editor.map(|editor| editor.form().summary.clone()).as_deref(), Some("Kiln"));
        harness.key_press(Key::Escape);
        harness.run();
        assert!(!open(&harness), "{host:?}: Esc again throws it away");
    }
}

#[test]
fn the_window_s_close_never_loses_typing_and_its_esc_is_the_editor_s() {
    let mut harness = app(EventEditorHost::Window, true, egui::Theme::Light);
    new_event(&mut harness);
    CLOSE.set(true);
    harness.run();
    assert!(!open(&harness), "nothing typed: the desktop's close closes it");
    new_event(&mut harness);
    typed(&mut harness, "Kiln");
    for way in 0..3 {
        CLOSE.set(true);
        harness.step();
        // As eframe: the desktop's close is taken back in the pass that heard it, or the window goes.
        let window = egui::ViewportId::from_hash_of("inboxcraft-event-editor-window");
        let kept = harness.output().viewport_output.get(&window).is_some_and(|asked| asked.commands.contains(&egui::ViewportCommand::CancelClose));
        assert!(kept, "close {way}: the window's close is taken back");
        harness.run();
        let editor = harness.state().calendar().editor().cloned();
        assert!(editor.as_ref().is_some_and(crate::calendar::Editor::warned), "close {way}: the editor stays and says so");
        assert_eq!(editor.map(|editor| editor.form().summary.clone()).as_deref(), Some("Kiln"), "close {way}: nothing lost");
        let before = seen().passes;
        harness.run();
        assert!(seen().passes > before, "close {way}: the window draws on");
    }
    window_keys(&mut harness, vec![esc()]);
    assert!(open(&harness), "the first Esc after a close warns as Esc does");
    window_keys(&mut harness, vec![esc()]);
    assert!(!open(&harness), "the second throws the typing away");
    let before = seen().passes;
    harness.run();
    assert_eq!(seen().passes, before, "the window is gone");
}

#[test]
fn alert_is_drawn_in_the_full_editor_and_the_window_and_not_in_the_float() {
    for host in [EventEditorHost::Small, EventEditorHost::Full, EventEditorHost::Window] {
        let mut harness = app(host, true, egui::Theme::Light);
        new_event(&mut harness);
        let alert = harness.query_by_role_and_label(Role::ComboBox, "Alert").is_some() || in_window(Role::ComboBox, "Alert");
        assert_eq!(alert, host != EventEditorHost::Small, "{host:?}");
        let repeat = harness.query_by_role_and_label(Role::ComboBox, "Repeat").is_some() || in_window(Role::ComboBox, "Repeat");
        assert!(repeat, "{host:?}: the rest of the form is there");
    }
    // What the field shows is the event's alert: a new event's default lead.
    let mut harness = app(EventEditorHost::Full, false, egui::Theme::Light);
    new_event(&mut harness);
    let value = |harness: &Harness<'static, InboxApp>| harness.get_by_role_and_label(Role::ComboBox, "Alert").accesskit_node().value();
    assert_eq!(value(&harness).as_deref(), Some("10 min before"));
    harness.state_mut().calendar_mut().0.editor_mut().unwrap().form_mut().alert = crate::calendar::Alert::Before(60);
    harness.run();
    assert_eq!(value(&harness).as_deref(), Some("1 hour before"));
}

#[test]
fn someone_else_s_event_opens_read_only_in_every_host() {
    for host in [EventEditorHost::Small, EventEditorHost::Full, EventEditorHost::Window] {
        let mut harness = app(host, true, egui::Theme::Light);
        edit(&mut harness, Some("Allotment work party"));
        let named = |role, name: &str| harness.query_by_role_and_label(role, name).is_some() || in_window(role, name);
        assert!(named(Role::Button, "Close"), "{host:?}: Close alone");
        assert!(!named(Role::Button, "Save") && !named(Role::Button, "Cancel"), "{host:?}: no Save, no Cancel");
        assert!(harness.state().calendar().editor().is_some_and(crate::calendar::Editor::read_only));
    }
}

#[test]
fn an_agent_s_undo_is_refused_while_any_host_holds_the_editor() {
    for host in [EventEditorHost::Small, EventEditorHost::Full, EventEditorHost::Window] {
        let mut harness = app(host, true, egui::Theme::Light);
        let flagged = harness.state_mut().mailbox_mut().run_for_agent("mail.flag", &json!({ "id": 1, "flagged": true }));
        assert_eq!(flagged.unwrap()["changed"], json!(true));
        new_event(&mut harness);
        let refused = harness.state_mut().mailbox_mut().run_for_agent("history.undo", &json!({})).unwrap_err();
        assert_eq!(refused.code(), "editing", "{host:?}");
        // The owner's way to the mail does not end the refusal while the editor holds the event.
        harness.state_mut().apply_calendar(vec![Asked::Cancel]);
        harness.run();
        assert!(harness.state_mut().mailbox_mut().run_for_agent("history.undo", &json!({})).is_ok(), "{host:?}: runs again once it closes");
    }
}

#[test]
fn the_window_gives_its_keyboard_to_the_title_and_no_main_key_acts_from_it() {
    let mut harness = app(EventEditorHost::Window, true, egui::Theme::Light);
    new_event(&mut harness);
    harness.run();
    assert!(seen().focused, "the window's keyboard is on its title");
    assert!(
        harness.ctx.memory(|memory| memory.focused()).is_none_or(|id| harness.ctx.read_response(id).is_some()),
        "the main window's is on a drawn control"
    );
    // The main window is no modal surface's while the editor is in its window: its New event is live.
    assert!(!harness.state().editing_event(), "the main window's keys are its own");
}

#[test]
fn the_new_event_opens_row_offers_three_hosts_full_chosen_and_runs_the_owner_s_layout_set() {
    let mut harness = app(EventEditorHost::Full, false, egui::Theme::Light);
    harness.state_mut().open_settings_on("calendar");
    harness.run();
    let chosen = |harness: &Harness<'static, InboxApp>, name: &str| {
        harness.get_by_label(name).accesskit_node().toggled() == Some(egui::accesskit::Toggled::True)
    };
    assert!(chosen(&harness, "Full for New event opens"), "full by default");
    harness.get_by_label_contains("Full: in the calendar's place");
    harness.get_by_label("Window for New event opens").click();
    harness.run();
    assert_eq!(harness.state().mailbox().last_run(), Some("ui.layout.set"));
    assert_eq!(harness.state().mailbox().appearance().layout.event_editor, EventEditorHost::Window);
    assert!(chosen(&harness, "Window for New event opens"));
    harness.get_by_label_contains("Window: in a window of its own");
    harness.get_by_label("Small for New event opens").click();
    harness.run();
    assert_eq!(harness.state().mailbox().appearance().layout.event_editor, EventEditorHost::Small);
}
