// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Home as a place of the window: the start place with an account, its row and key, the model built only while it
//! shows and only when what it shows changed, each press going to its place or act, the mail's keys resting on it,
//! and the page inside the body at every width.

use egui::accesskit::{Role, Toggled};
use egui::{Key, Modifiers, Rect, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_model::Role as Folder;
use inboxcraft_time::{FakeClock, Timestamp};
use serde_json::{Value, json};

use super::place_tests::{OF_THE_MAIL, THE_WINDOW_S};
use super::{InboxApp, Screen};
use crate::keymap;
use crate::mailbox::{Mailbox, Pile, Place};
use crate::sample;
use crate::theme::TextScale;

/// 2026-11-05T09:00:00Z.
const START: i64 = 1_793_869_200;

fn clock() -> FakeClock {
    FakeClock::new(Timestamp::from_unix(START, 0).unwrap())
}

/// The sample window as it starts, one clock for the session and the window.
fn sample_app(clock: &FakeClock) -> InboxApp {
    InboxApp::new(Mailbox::new(sample::session(Box::new(clock.clone())).unwrap(), Box::new(clock.clone())))
}

/// `app` in a window of `size`, in the app's own frame, run once.
fn window_of(mut app: InboxApp, size: egui::Vec2) -> Harness<'static, InboxApp> {
    app.set_own_frame(true);
    app.set_frame(inboxcraft_engine::appearance::WindowFrame::Strip);
    let mut harness = Harness::builder().with_size(size).with_theme(egui::Theme::Dark).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

fn window(app: InboxApp) -> Harness<'static, InboxApp> {
    window_of(app, vec2(1440.0, 900.0))
}

/// A press on the button whose name ends with `end` (a count tile: "7 To read").
fn press_ending(harness: &mut Harness<'static, InboxApp>, end: &str) {
    let node = harness.query_all_by_role(Role::Button).find(|node| node.accesskit_node().label().is_some_and(|label| label.ends_with(end)));
    let Some(node) = node else { panic!("no button ending {end:?}") };
    node.click();
    harness.run();
    harness.run();
}

fn press(harness: &mut Harness<'static, InboxApp>, label: &str) {
    harness.get_by_label(label).click();
    harness.run();
    harness.run();
}

/// A kept draft of the first account with `subject`; its id.
fn draft(app: &mut InboxApp, subject: &str) -> u64 {
    let mailbox = app.mailbox_mut();
    let account = mailbox.accounts().first().map(|account| account.id).unwrap();
    let id = mailbox.run("compose.new", &json!({ "account": account })).unwrap().get("id").and_then(Value::as_u64).unwrap();
    mailbox.run("compose.update", &json!({ "id": id, "subject": subject })).unwrap();
    id
}

/// The draft row whose name starts with `subject`.
fn draft_row(harness: &Harness<'static, InboxApp>, subject: &str) -> Rect {
    let row = harness.query_all_by_role(Role::ListItem).find(|node| node.accesskit_node().label().is_some_and(|label| label.starts_with(subject)));
    let Some(row) = row else { panic!("no draft row {subject:?}") };
    row.rect()
}

/// A press on the draft row whose name starts with `subject`.
fn press_draft(harness: &mut Harness<'static, InboxApp>, subject: &str) {
    let row = harness.query_all_by_role(Role::ListItem).find(|node| node.accesskit_node().label().is_some_and(|label| label.starts_with(subject)));
    let Some(row) = row else { panic!("no draft row {subject:?}") };
    row.click();
    harness.run();
    harness.run();
}

fn drafts_total(app: &mut InboxApp) -> u64 {
    app.mailbox_mut().run("compose.list", &json!({})).unwrap().get("total").and_then(Value::as_u64).unwrap()
}

fn subject_of(app: &mut InboxApp, id: u64) -> String {
    let shown = app.mailbox_mut().run("compose.show", &json!({ "id": id })).unwrap();
    shown.get("subject").and_then(Value::as_str).unwrap_or_default().to_owned()
}

/// Ctrl and `key`.
fn command(harness: &mut Harness<'static, InboxApp>, key: Key) {
    harness.key_press_modifiers(Modifiers::COMMAND, key);
    harness.run();
}

#[test]
fn the_window_starts_on_home_with_an_account_and_on_the_mail_without() {
    let harness = window(sample_app(&clock()));
    assert_eq!(harness.state().screen(), Screen::Home, "Home is the start place once there is an account");
    assert_eq!(harness.get_by_role_and_label(Role::Button, "Home").accesskit_node().toggled(), Some(Toggled::True), "HOME is the place shown");
    assert_eq!(harness.get_by_role_and_label(Role::Button, "Mail").accesskit_node().toggled(), Some(Toggled::False));
    assert!(harness.query_by_label("Read mail").is_some() || harness.query_by_label("Write").is_some(), "the brief's buttons");
    let empty = window(InboxApp::new(crate::onboard::tests::mailbox()));
    assert_eq!(empty.state().screen(), Screen::Mail, "with no account the window offers to add one, as before");
}

#[test]
fn home_s_key_its_row_and_the_palette_show_it_and_mail_s_key_leaves_it() {
    let mut harness = window(InboxApp::at_mail(Mailbox::new(sample::session(Box::new(clock())).unwrap(), Box::new(clock()))));
    assert_eq!(harness.state().keymap().shortcut(keymap::HOME), Some(egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::Num0)));
    command(&mut harness, Key::Num0);
    assert_eq!(harness.state().screen(), Screen::Home, "Command 0 shows Home");
    command(&mut harness, Key::Num1);
    assert_eq!(harness.state().screen(), Screen::Mail, "Command 1 shows the mail");
    harness.get_by_role_and_label(Role::Button, "Home").click();
    harness.run();
    assert_eq!(harness.state().screen(), Screen::Home, "the index's HOME shows it");
    command(&mut harness, Key::Num2);
    assert_eq!(harness.state().screen(), Screen::Calendar);
    let ctx = harness.ctx.clone();
    harness.state_mut().run_action(&ctx, keymap::PALETTE);
    harness.run();
    assert!(harness.state().palette().is_open());
    harness.state_mut().palette_mut().set_query("Show Home");
    harness.run();
    harness.key_press(Key::Enter);
    harness.run();
    harness.run();
    assert_eq!(harness.state().screen(), Screen::Home, "the palette lists and runs it");
    // From the settings too: Home leaves them.
    super::tests::at_settings(harness.state_mut(), crate::settings::Section::Appearance);
    harness.run();
    command(&mut harness, Key::Num0);
    assert!(!harness.state().settings().is_open());
    assert_eq!(harness.state().screen(), Screen::Home);
}

#[test]
fn home_is_built_once_at_rest_and_again_only_while_it_shows_when_what_it_shows_changed() {
    let clock = clock();
    let mut harness = window(sample_app(&clock));
    for _ in 0..12 {
        harness.run();
    }
    assert_eq!(harness.state().home.builds(), 1, "frames at rest build nothing more");
    // A draft kept while Home shows: once more.
    draft(harness.state_mut(), "Glaze order");
    harness.run();
    harness.run();
    assert_eq!(harness.state().home.builds(), 2, "a draft kept");
    assert!(
        harness.query_all_by_role(Role::ListItem).any(|node| node.accesskit_node().label().is_some_and(|label| label.starts_with("Glaze order")))
    );
    // An event saved while Home shows: once more.
    let calendar = harness.state_mut().mailbox_mut().run("calendar.list", &json!({})).unwrap()["calendars"][0]["id"].as_u64().unwrap();
    let start = harness.state().mailbox().now().checked_add(std::time::Duration::from_secs(1_800)).unwrap();
    let event = json!({ "calendar": calendar, "summary": "Kiln check", "start": crate::mailbox::engine_date(start) });
    harness.state_mut().mailbox_mut().run("event.create", &event).unwrap();
    harness.run();
    assert_eq!(harness.state().home.builds(), 3, "an event saved");
    // The hour turns: once more.
    clock.advance(std::time::Duration::from_secs(3_600));
    harness.run();
    assert_eq!(harness.state().home.builds(), 4, "the hour turned");
    // Nothing while another place shows, whatever changes there.
    command(&mut harness, Key::Num1);
    draft(harness.state_mut(), "Clay invoice");
    clock.advance(std::time::Duration::from_secs(3_600));
    for _ in 0..4 {
        harness.run();
    }
    assert_eq!(harness.state().home.builds(), 4, "nothing is built while the mail shows");
    command(&mut harness, Key::Num0);
    harness.run();
    assert_eq!(harness.state().home.builds(), 5, "once on return");
}

#[test]
fn home_is_built_again_when_the_first_event_left_today_ends() {
    let clock = clock();
    let mut app = sample_app(&clock);
    let calendar = app.mailbox_mut().run("calendar.list", &json!({})).unwrap()["calendars"][0]["id"].as_u64().unwrap();
    let now = app.mailbox().now();
    let start = crate::mailbox::engine_date(now.checked_add(std::time::Duration::from_secs(60)).unwrap());
    let end = crate::mailbox::engine_date(now.checked_add(std::time::Duration::from_secs(600)).unwrap());
    app.mailbox_mut().run("event.create", &json!({ "calendar": calendar, "summary": "Short call", "start": start, "end": end })).unwrap();
    let mut harness = window(app);
    let built = harness.state().home.builds();
    assert!(harness.query_all_by_label_contains("Short call").count() > 0, "the event is the rest of today");
    clock.advance(std::time::Duration::from_secs(300));
    harness.run();
    assert_eq!(harness.state().home.builds(), built, "not before it ends");
    clock.advance(std::time::Duration::from_secs(301));
    harness.run();
    harness.run();
    assert_eq!(harness.state().home.builds(), built + 1, "once it ended");
    assert_eq!(harness.query_all_by_label_contains("Short call").count(), 0, "an event that ended is not the rest of today");
}

#[test]
fn each_count_tile_and_link_goes_to_its_place() {
    let cases: [(&str, Screen, Option<Place>); 4] = [
        (" To read", Screen::Mail, Some(Place::View(Folder::Inbox))),
        (" To screen", Screen::Mail, Some(Place::Senders)),
        (" Back today", Screen::Mail, Some(Place::Later(Pile::Snoozed))),
        (" In the outbox", Screen::Mail, Some(Place::Outbox)),
    ];
    for (end, screen, place) in cases {
        // The outbox case on the sample whose first account has sends waiting (an empty outbox opens the inbox).
        let app = if end == " In the outbox" {
            let clock = clock();
            let mut app = InboxApp::new(Mailbox::new(sample::sending(Box::new(clock.clone())).unwrap(), Box::new(clock)));
            let id = draft(&mut app, "Plot 14 handover");
            let mailbox = app.mailbox_mut();
            mailbox.run("compose.update", &json!({ "id": id, "to": ["Hollis Fenwick <hollis@example.org>"] })).unwrap();
            mailbox.run("mail.send", &json!({ "id": id })).unwrap();
            mailbox.refresh();
            app
        } else {
            sample_app(&clock())
        };
        let mut harness = window(app);
        assert!(end != " In the outbox" || !harness.state().mailbox().outbox().is_empty(), "sends wait in the outbox");
        // From another mailbox, so that each press is seen to open its own.
        harness.state_mut().mailbox_mut().open(Place::View(Folder::Archive));
        harness.run();
        // A count of 0 says "Nothing …" on its tile, which presses the same.
        let nothing = match end {
            " To screen" => "Nobody to screen",
            " Back today" => "Nothing back today",
            " In the outbox" => "Nothing to send",
            _ => "Nothing to read",
        };
        let some = harness.query_all_by_role(Role::Button).any(|node| node.accesskit_node().label().is_some_and(|label| label.ends_with(end)));
        press_ending(&mut harness, if some { end } else { nothing });
        assert_eq!(harness.state().screen(), screen, "{end}");
        assert_eq!(Some(harness.state().mailbox().place()), place, "{end}");
    }
    // The agenda's link: the calendar, this week, on today.
    let mut harness = window(sample_app(&clock()));
    let week = harness.query_by_label("open the week →").is_some();
    match week {
        true => harness.get_by_label("open the week →").click(),
        false => harness.query_all_by_label_contains("more today →").next().unwrap().click(),
    }
    harness.run();
    harness.run();
    assert_eq!(harness.state().screen(), Screen::Calendar);
    let view = if week { crate::calendar::View::Week } else { crate::calendar::View::Day };
    assert_eq!(harness.state().calendar().view(), view);
}

#[test]
fn the_buttons_do_what_they_say() {
    let mut harness = window(sample_app(&clock()));
    let read = harness.query_by_label("Read mail").is_some();
    if read {
        harness.state_mut().mailbox_mut().open(Place::View(Folder::Sent));
        harness.run();
        press(&mut harness, "Read mail");
        assert_eq!((harness.state().screen(), harness.state().mailbox().place()), (Screen::Mail, Place::View(Folder::Inbox)), "Read mail");
    } else {
        press(&mut harness, "Open the calendar");
        assert_eq!(harness.state().screen(), Screen::Calendar, "Open the calendar");
    }
    command(&mut harness, Key::Num0);
    press(&mut harness, "Write");
    assert_eq!(harness.state().screen(), Screen::Mail, "Write");
    assert!(harness.state().composer().is_some_and(|composer| !composer.floating()), "a new message in the reader's place");
}

#[test]
fn a_draft_row_opens_its_draft_and_its_discard_discards_it() {
    let mut app = sample_app(&clock());
    let older = draft(&mut app, "Workshop dates");
    let newer = draft(&mut app, "Glaze order");
    let mut harness = window(app);
    let row = draft_row(&harness, "Workshop dates");
    harness.hover_at(row.center());
    harness.run();
    harness.run();
    let before = drafts_total(harness.state_mut());
    harness.get_by_label("discard").click();
    harness.run();
    harness.run();
    assert_eq!(drafts_total(harness.state_mut()), before - 1, "the draft is discarded");
    assert!(harness.state_mut().mailbox_mut().run("compose.show", &json!({ "id": older })).is_err(), "that draft, not another");
    assert_eq!(harness.state().notice(), Some("Draft discarded"), "the composer's Discard's toast");
    assert_eq!(harness.state().screen(), Screen::Home, "discard leaves Home where it is");
    press_draft(&mut harness, "Glaze order");
    assert_eq!(harness.state().screen(), Screen::Mail);
    assert_eq!(harness.state().composer().map(crate::compose::Composer::draft), Some(newer), "the row's draft in the composer");
}

#[test]
fn a_draft_row_pressed_over_a_draft_being_written_keeps_what_was_typed() {
    let mut app = InboxApp::at_mail(Mailbox::new(sample::session(Box::new(clock())).unwrap(), Box::new(clock())));
    let older = draft(&mut app, "Workshop dates");
    let mut harness = window(app);
    let ctx = harness.ctx.clone();
    harness.state_mut().run_action(&ctx, keymap::COMPOSE);
    harness.run();
    harness.get_by_role_and_label(Role::TextInput, "Subject").focus();
    harness.get_by_role_and_label(Role::TextInput, "Subject").type_text("Typed before Home");
    harness.run();
    let typing = harness.state().composer().map(crate::compose::Composer::draft).unwrap();
    command(&mut harness, Key::Num0);
    assert_eq!(harness.state().screen(), Screen::Home);
    assert!(harness.state().composer().is_some(), "the composer floats over Home");
    press_draft(&mut harness, "Workshop dates");
    assert_eq!(harness.state().composer().map(crate::compose::Composer::draft), Some(older), "the row's draft opens");
    assert_eq!(subject_of(harness.state_mut(), typing), "Typed before Home", "the draft being written is kept whole");
}

#[test]
fn on_home_no_key_and_no_press_runs_an_action_of_the_mail_and_the_window_s_own_run() {
    let home = crate::surface::Surfaces::new(crate::surface::Facts { home: true, ..crate::surface::Facts::default() });
    for id in OF_THE_MAIL {
        let mut with_ctrl = keymap::Keymap::default();
        with_ctrl.bind(id, egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::F9));
        for keys in [keymap::Keymap::default(), with_ctrl] {
            assert!(!home.allows(id, &keys), "{id}: its key runs nothing on Home");
        }
        assert!(!home.allows_press(id), "{id}: a press runs nothing on Home");
    }
    for id in THE_WINDOW_S {
        assert!(home.allows_press(id), "{id}: the window's own runs by a press on Home");
        assert!(home.allows(id, &keymap::Keymap::default()), "{id}: and by its key, Home has no field the letters are for");
    }
}

#[test]
fn nothing_of_the_mail_runs_on_home_and_the_window_s_own_runs_as_off_it() {
    let mut harness = window(InboxApp::at_mail(Mailbox::new(sample::session(Box::new(clock())).unwrap(), Box::new(clock()))));
    let last = harness.state().mailbox().messages().last().map(|row| row.id).unwrap();
    harness.state_mut().mailbox_mut().act(last, crate::mailbox::Act::Archive);
    let first = harness.state().mailbox().messages().first().map(|row| row.id);
    harness.state_mut().mailbox_mut().select(first);
    harness.run();
    assert!(harness.state().can_run(keymap::UNDO), "Undo could run off Home");
    let before: Vec<(&str, bool)> = keymap::actions().into_iter().map(|action| (action.id, harness.state().can_run(action.id))).collect();
    command(&mut harness, Key::Num0);
    assert_eq!(harness.state().screen(), Screen::Home);
    for (id, could) in before {
        let can = harness.state().can_run(id);
        if OF_THE_MAIL.contains(&id) {
            assert!(!can, "{id}: acts on the mail or the calendar, which Home does not show");
        } else {
            assert_eq!(can, could, "{id}: the window's own runs on Home as off it");
        }
    }
    // A row act's bare key does nothing to the selected message.
    let row = harness.state().mailbox().selected().unwrap();
    harness.key_press(Key::D);
    harness.run();
    harness.run();
    assert!(harness.state().mailbox().messages().iter().any(|listed| listed.id == row && !listed.done), "D rests on Home");
    // The window's bare key runs: C begins a message.
    harness.key_press(Key::C);
    harness.run();
    assert!(harness.state().composer().is_some(), "C, the window's own, runs on Home");
}

#[test]
fn on_home_esc_closes_a_float_and_otherwise_nothing() {
    let mut harness = window(sample_app(&clock()));
    harness.key_press(Key::Escape);
    harness.run();
    assert_eq!(harness.state().screen(), Screen::Home, "Esc does not leave Home");
    // The account picker, a float of the index, over Home: one Esc closes it, Home stays.
    let owner =
        harness.query_all_by_role(Role::Button).find(|node| node.accesskit_node().label().is_some_and(|label| label.starts_with("Show mail of")));
    let Some(owner) = owner else { panic!("no owner row") };
    owner.click();
    harness.run();
    assert!(crate::component::popup::any_open(&harness.ctx), "the picker is open");
    harness.key_press(Key::Escape);
    harness.run();
    harness.run();
    assert!(!crate::component::popup::any_open(&harness.ctx), "one Esc closes it");
    assert_eq!(harness.state().screen(), Screen::Home, "and Home stays");
}

#[test]
fn home_stands_in_the_body_at_every_width_and_text_size() {
    for width in [1440.0, 1300.0, 1000.0, 800.0] {
        for percent in [100, 130] {
            let mut app = sample_app(&clock());
            app.set_text_scale(TextScale::from_percent(percent).unwrap());
            let mut harness = window_of(app, vec2(width, 900.0));
            harness.run();
            // The body the window laid out this frame: the window less the index (232, wide at every width until the
            // widths slice turns its breakpoint on), its hairline and the window's two edges.
            let regions = super::shell::laid(&harness.ctx).unwrap();
            let body = regions.body;
            assert_eq!((regions.index.width(), body.left()), (232.0, regions.index.right() + 1.0), "{width}/{percent}: the body beside the index");
            assert_eq!(body.right(), regions.command.right(), "{width}/{percent}: the body to the window's edge");
            println!("{width}/{percent}: the body {body:?}");
            let roles = [Role::Button, Role::Link, Role::ListItem];
            // Home's controls: those right of the index, in the body's height (the brief scrolls below it).
            let parts: Vec<Rect> = harness
                .query_all_by_role(Role::Button)
                .chain(harness.query_all_by_role(Role::Link))
                .chain(harness.query_all_by_role(Role::ListItem))
                .filter(|node| roles.contains(&node.accesskit_node().role()))
                .map(|node| node.rect())
                .filter(|rect| rect.left() >= body.left() && rect.top() >= body.top() && rect.top() < body.bottom())
                .collect();
            assert!(parts.len() >= 4, "{width}/{percent}: Home's tiles at least: {parts:?}");
            for part in &parts {
                assert!(part.right() <= body.right() + 0.5, "{width}/{percent}: {part:?} leaves the body {body:?}");
            }
            for (at, one) in parts.iter().enumerate() {
                for other in parts.iter().skip(at + 1) {
                    let inside = one.contains_rect(*other) || other.contains_rect(*one);
                    let meet = one.intersect(*other);
                    assert!(inside || meet.width() <= 0.5 || meet.height() <= 0.5, "{width}/{percent}: {one:?} and {other:?} overlap");
                }
            }
        }
    }
}
