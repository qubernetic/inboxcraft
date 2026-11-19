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

use super::place_tests::{OF_THE_COMPOSER, OF_THE_MAIL, THE_WINDOW_S};
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
    // Away and back with nothing changed: once on return, as on every return.
    command(&mut harness, Key::Num1);
    command(&mut harness, Key::Num0);
    harness.run();
    assert_eq!(harness.state().home.builds(), 5, "once on a return with nothing changed");
    // Nothing while another place shows, whatever changes there.
    command(&mut harness, Key::Num1);
    draft(harness.state_mut(), "Clay invoice");
    clock.advance(std::time::Duration::from_secs(3_600));
    for _ in 0..4 {
        harness.run();
    }
    assert_eq!(harness.state().home.builds(), 5, "nothing is built while the mail shows");
    command(&mut harness, Key::Num0);
    harness.run();
    assert_eq!(harness.state().home.builds(), 6, "once on return");
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

/// A press on the discard of the draft row whose name starts with `subject`.
fn press_discard(harness: &mut Harness<'static, InboxApp>, subject: &str) {
    let row = draft_row(harness, subject);
    harness.hover_at(row.center());
    harness.run();
    harness.run();
    harness.get_by_label("discard").click();
    harness.run();
    harness.run();
}

/// The draft rows Home shows, top to bottom, by the start of their names.
fn rows(harness: &Harness<'static, InboxApp>) -> Vec<String> {
    let mut rows: Vec<(f32, String)> = harness
        .query_all_by_role(Role::ListItem)
        .filter_map(|node| {
            let label = node.accesskit_node().label()?;
            let subject = ["Workshop dates", "Glaze order"].into_iter().find(|subject| label.starts_with(subject))?;
            Some((node.rect().top(), subject.to_owned()))
        })
        .collect();
    rows.sort_by(|a, b| a.0.total_cmp(&b.0));
    rows.into_iter().map(|(_, subject)| subject).collect()
}

/// Frames until the toast's time has gone.
fn wait_out_the_toast(harness: &mut Harness<'static, InboxApp>) {
    let later = harness.ctx.input(|input| input.time) + super::TOAST_SECONDS + 1.0;
    while harness.ctx.input(|input| input.time) < later {
        harness.step();
    }
    harness.run();
}

fn exists(app: &mut InboxApp, id: u64) -> bool {
    app.mailbox_mut().run("compose.show", &json!({ "id": id })).is_ok()
}

#[test]
fn a_draft_row_opens_its_draft() {
    let mut app = sample_app(&clock());
    let newer = draft(&mut app, "Glaze order");
    let mut harness = window(app);
    press_draft(&mut harness, "Glaze order");
    assert_eq!(harness.state().screen(), Screen::Mail);
    assert_eq!(harness.state().composer().map(crate::compose::Composer::draft), Some(newer), "the row's draft in the composer");
}

#[test]
fn a_draft_row_s_discard_hides_it_at_once_and_discards_it_when_its_toast_ends() {
    let mut app = sample_app(&clock());
    let older = draft(&mut app, "Workshop dates");
    let newer = draft(&mut app, "Glaze order");
    let mut harness = window(app);
    let before = drafts_total(harness.state_mut());
    let all = format!("all drafts ({before}) →");
    harness.get_by_label(&all);
    press_discard(&mut harness, "Workshop dates");
    assert_eq!(rows(&harness), ["Glaze order"], "the row goes at once");
    harness.get_by_label(&format!("all drafts ({}) →", before - 1));
    harness.get_by_label("Draft discarded");
    harness.get_by_role_and_label(Role::Button, "Undo");
    assert_eq!(harness.state().screen(), Screen::Home, "discard leaves Home where it is");
    // Nothing is gone yet: a crash now leaves the draft where it was.
    assert!(exists(harness.state_mut(), older), "the discard waits for its toast");
    assert_eq!(harness.state().discarding(), Some(older));
    wait_out_the_toast(&mut harness);
    assert!(!exists(harness.state_mut(), older), "carried out when the toast ends");
    assert!(exists(harness.state_mut(), newer), "that draft, not another");
    assert_eq!(drafts_total(harness.state_mut()), before - 1);
    assert_eq!(harness.state().discarding(), None);
    assert!(harness.query_by_label("Draft discarded").is_none());
    assert_eq!(rows(&harness), ["Glaze order"]);
}

#[test]
fn undo_brings_the_discarded_draft_back_where_it_was_by_button_and_by_key() {
    for by_key in [false, true] {
        let mut app = sample_app(&clock());
        let older = draft(&mut app, "Workshop dates");
        draft(&mut app, "Glaze order");
        let mut harness = window(app);
        let before = rows(&harness);
        assert_eq!(before, ["Glaze order", "Workshop dates"]);
        let total = drafts_total(harness.state_mut());
        press_discard(&mut harness, "Workshop dates");
        if by_key {
            harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
            harness.run();
            harness.run();
        } else {
            press(&mut harness, "Undo");
        }
        assert_eq!(harness.state().discarding(), None, "by key {by_key}");
        assert_eq!(rows(&harness), before, "by key {by_key}: the row is back where it was");
        harness.get_by_label(&format!("all drafts ({total}) →"));
        wait_out_the_toast(&mut harness);
        assert!(exists(harness.state_mut(), older), "by key {by_key}: undone, nothing is discarded");
        assert_eq!(drafts_total(harness.state_mut()), total);
    }
}

#[test]
fn a_second_discard_while_one_waits_carries_the_first_out() {
    let mut app = sample_app(&clock());
    let older = draft(&mut app, "Workshop dates");
    let newer = draft(&mut app, "Glaze order");
    let mut harness = window(app);
    press_discard(&mut harness, "Workshop dates");
    press_discard(&mut harness, "Glaze order");
    assert!(!exists(harness.state_mut(), older), "the first is carried out");
    assert!(exists(harness.state_mut(), newer), "the second waits");
    assert_eq!(harness.state().discarding(), Some(newer));
    press(&mut harness, "Undo");
    wait_out_the_toast(&mut harness);
    assert!(exists(harness.state_mut(), newer), "its Undo takes back the second only");
    assert!(!exists(harness.state_mut(), older));
}

#[test]
fn leaving_home_does_not_cancel_the_wait_and_a_closing_window_carries_it_out() {
    let mut app = sample_app(&clock());
    let older = draft(&mut app, "Workshop dates");
    let mut harness = window(app);
    press_discard(&mut harness, "Workshop dates");
    harness.state_mut().show_mail();
    harness.run();
    assert!(exists(harness.state_mut(), older), "still waiting on the mail");
    harness.get_by_label("Draft discarded");
    wait_out_the_toast(&mut harness);
    assert!(!exists(harness.state_mut(), older), "carried out off Home too");

    let mut app = sample_app(&clock());
    let older = draft(&mut app, "Workshop dates");
    let mut harness = window(app);
    press_discard(&mut harness, "Workshop dates");
    harness.input_mut().viewports.entry(egui::ViewportId::ROOT).or_default().events.push(egui::ViewportEvent::Close);
    harness.step();
    assert!(!exists(harness.state_mut(), older), "the window closes: carried out, not lost with the toast");
    assert_eq!(harness.state().discarding(), None);
}

#[test]
fn discarding_the_draft_being_written_keeps_and_closes_it_first() {
    let mut app = sample_app(&clock());
    let older = draft(&mut app, "Workshop dates");
    let mut harness = window(app);
    press_draft(&mut harness, "Workshop dates");
    harness.get_by_role_and_label(Role::TextInput, "Subject").focus();
    harness.get_by_role_and_label(Role::TextInput, "Subject").type_text(" and kilns");
    harness.run();
    command(&mut harness, Key::Num0);
    assert_eq!(harness.state().screen(), Screen::Home);
    press_discard(&mut harness, "Workshop dates");
    assert!(harness.state().composer().is_none(), "the composer closes with its draft");
    assert_eq!(harness.state().discarding(), Some(older));
    press(&mut harness, "Undo");
    assert_eq!(subject_of(harness.state_mut(), older), "Workshop dates and kilns", "what was typed was kept first");
}

#[test]
fn the_discard_s_toast_covers_no_control_of_home() {
    for size in [vec2(1440.0, 900.0), vec2(1300.0, 900.0), vec2(1000.0, 900.0), vec2(800.0, 900.0)] {
        let mut app = sample_app(&clock());
        draft(&mut app, "Workshop dates");
        draft(&mut app, "Glaze order");
        let mut harness = window_of(app, size);
        press_discard(&mut harness, "Workshop dates");
        // The toast's float, which takes the presses over it: the area its Undo stands on.
        let undo = harness.get_by_role_and_label(Role::Button, "Undo").rect();
        let layer = harness.ctx.layer_id_at(undo.center()).unwrap();
        let float = harness.ctx.memory(|memory| memory.area_rect(layer.id)).unwrap();
        assert!(float.contains_rect(harness.get_by_label("Draft discarded").rect()), "{float:?}");
        let home = harness.state().screen();
        assert_eq!(home, Screen::Home);
        for node in
            harness.query_all_by_role(Role::Button).chain(harness.query_all_by_role(Role::ListItem)).chain(harness.query_all_by_role(Role::Link))
        {
            let label = node.accesskit_node().label().unwrap_or_default().to_owned();
            if label == "Undo" {
                continue;
            }
            assert!(!node.rect().intersects(float), "{size:?}: the toast covers {label:?} at {:?} ({float:?})", node.rect());
        }
    }
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
    // The composer's own: by the facts, not the place. With no composer they rest; with the floating one drawn over
    // Home they run.
    let with = crate::surface::Surfaces::new(crate::surface::Facts { home: true, composer_drawn: true, ..crate::surface::Facts::default() });
    for id in OF_THE_COMPOSER {
        assert!(!home.allows_press(id) && !home.allows(id, &keymap::Keymap::default()), "{id}: no composer on Home");
        assert!(with.allows_press(id) && with.allows(id, &keymap::Keymap::default()), "{id}: the floating composer over Home");
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

fn type_into(harness: &mut Harness<'static, InboxApp>, field: &str, text: &str) {
    harness.get_by_role_and_label(Role::TextInput, field).focus();
    harness.run();
    harness.get_by_role_and_label(Role::TextInput, field).type_text(text);
    harness.run();
}

fn folded(harness: &Harness<'static, InboxApp>) -> bool {
    harness.state().composer().is_some_and(crate::compose::Composer::folded)
}

/// The composer's own (Send, fold) run where a composer is drawn: Home draws the floating one, so there they act, by
/// their keys and by their buttons. Before #303's fix both rested on Home as on the settings.
#[test]
fn on_home_the_floating_composer_s_send_and_fold_act_by_key_and_by_button() {
    let mut harness = window(sample_app(&clock()));
    harness.state_mut().keymap_mut().bind(keymap::FOLD, egui::KeyboardShortcut::new(Modifiers::COMMAND, Key::F9));
    let ctx = harness.ctx.clone();
    harness.state_mut().run_action(&ctx, keymap::COMPOSE);
    harness.run();
    if harness.state().screen() != Screen::Home {
        command(&mut harness, Key::Num0);
    }
    assert_eq!(harness.state().screen(), Screen::Home);
    assert!(harness.state().composer().is_some() && !harness.state().docked(), "the composer floats over Home");
    type_into(&mut harness, "Subject", "Kiln report");
    for round in 0..2 {
        command(&mut harness, Key::F9);
        harness.run();
        assert_eq!(folded(&harness), round == 0, "fold by its key, round {round}");
    }
    for (round, words) in ["Fold the draft", "Open the draft"].into_iter().enumerate() {
        harness.query_all_by_label_contains(words).next().unwrap_or_else(|| panic!("no {words:?}")).click();
        harness.run();
        harness.run();
        assert_eq!(folded(&harness), round == 0, "fold by its button: {words}");
    }
    assert_eq!(harness.state().screen(), Screen::Home, "folding leaves Home where it is");
    type_into(&mut harness, "To", "odile@example.org");
    harness.key_press(Key::Enter);
    harness.run();
    harness.get_by_role_and_label(Role::TextInput, "Subject").focus();
    harness.run();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Enter);
    harness.run();
    // The sample account has no submission server: the engine refuses the send, which shows it was asked for.
    let refused = harness.state().mailbox().error().is_some_and(|error| error.contains("mail.send"));
    assert!(refused, "Send by its key sends from Home");
}

#[test]
fn on_the_settings_the_composer_kept_open_neither_sends_nor_folds() {
    let mut harness = window(sample_app(&clock()));
    let ctx = harness.ctx.clone();
    harness.state_mut().run_action(&ctx, keymap::COMPOSE);
    harness.run();
    if harness.state().screen() != Screen::Home {
        command(&mut harness, Key::Num0);
    }
    type_into(&mut harness, "To", "odile@example.org");
    harness.key_press(Key::Enter);
    harness.run();
    type_into(&mut harness, "Subject", "Kiln report");
    assert!(harness.state().can_run(keymap::FOLD) && harness.state().can_run(keymap::SEND), "both run over Home");
    super::tests::at_settings(harness.state_mut(), crate::settings::Section::Appearance);
    harness.run();
    for id in super::place_tests::OF_THE_COMPOSER {
        assert!(!harness.state().can_run(id), "{id}: the settings draw no composer");
    }
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Enter);
    harness.run();
    assert!(harness.state().composer().is_some_and(|composer| !composer.folded()), "nothing sent, nothing folded");
    let asked = harness.state().mailbox().error().is_some_and(|error| error.contains("mail.send"));
    assert!(!asked, "Send rests on the settings");
}

/// The owner's ruling: Ctrl+0 is Home, and egui's own keyboard zoom is off. The interface's size comes from the
/// text-size setting alone: Ctrl+Plus and Ctrl+Minus change nothing, Ctrl+0 shows Home, and a zoom factor a context
/// came with (an earlier zoom) is set back to 1 at the start.
#[test]
fn the_zoom_keys_change_nothing_and_ctrl_0_shows_home_at_a_zoom_of_one() {
    let mut app = InboxApp::at_mail(Mailbox::new(sample::session(Box::new(clock())).unwrap(), Box::new(clock())));
    app.set_own_frame(true);
    let mut harness = Harness::builder()
        .with_size(vec2(1440.0, 900.0))
        .with_context_setup(|ctx| ctx.set_zoom_factor(1.5))
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness.run();
    assert_eq!(harness.ctx.zoom_factor(), 1.0, "a kept zoom is set back to 1 at the start");
    assert!(!harness.ctx.options(|options| options.zoom_with_keyboard), "egui's keyboard zoom is off");
    for key in [Key::Plus, Key::Equals, Key::Minus, Key::Plus] {
        command(&mut harness, key);
        harness.run();
        assert_eq!(harness.ctx.zoom_factor(), 1.0, "{key:?} zooms nothing");
        assert_eq!(harness.state().screen(), Screen::Mail, "{key:?} moves nothing");
    }
    command(&mut harness, Key::Num0);
    assert_eq!(harness.state().screen(), Screen::Home, "Ctrl+0 is Home");
    assert_eq!(harness.ctx.zoom_factor(), 1.0);
}

/// A draft row pressed while the draft being written cannot be kept (the store refuses `compose.update`; here because
/// the draft was taken away underneath, as an agent's `compose.discard` would): the switch is refused, what was typed
/// stays on screen, the owner is told, and the keyboard is not sent to the To line of a composer that did not change.
#[test]
fn a_switch_to_another_draft_is_refused_while_the_one_being_written_cannot_be_kept() {
    let mut app = sample_app(&clock());
    let other = draft(&mut app, "Workshop dates");
    let mut harness = window(app);
    let ctx = harness.ctx.clone();
    harness.state_mut().run_action(&ctx, keymap::COMPOSE);
    harness.run();
    if harness.state().screen() != Screen::Home {
        command(&mut harness, Key::Num0);
    }
    harness.get_by_role_and_label(Role::TextInput, "Subject").focus();
    harness.run();
    let typing = harness.state().composer().map(crate::compose::Composer::draft).unwrap();
    let subject = harness.ctx.memory(|memory| memory.focused());
    // No frame between: what was typed is not kept yet, and keeping it is refused.
    if let (Some(composer), _) = harness.state_mut().composer_mut() {
        composer.subject_mut().push_str("Typed, not kept");
    }
    harness.state_mut().mailbox_mut().run("compose.discard", &json!({ "id": typing })).unwrap();
    harness.state_mut().apply_home(&ctx, super::home::Asked::OpenDraft(other));
    let composer = harness.state().composer().unwrap();
    assert_eq!(composer.draft(), typing, "the switch is refused");
    assert_eq!(composer.subject(), "Typed, not kept", "what was typed stays on screen");
    assert!(harness.state().mailbox().error().is_some(), "the owner is told");
    assert_eq!(harness.ctx.memory(|memory| memory.focused()), subject, "the keys stay where they were");
}

/// Off Home the waiting discard's toast still takes Ctrl+Z: on the mail and on the calendar, with an older act in the
/// history whose own toast has ended, the key brings the draft back and leaves the older act alone. Before the fix
/// the window's Undo took the key first: the older act came back and the draft was discarded all the same.
#[test]
fn off_home_ctrl_z_brings_the_discarded_draft_back_and_not_an_older_act() {
    for (name, place) in [("mail", InboxApp::show_mail as fn(&mut InboxApp)), ("calendar", InboxApp::show_calendar)] {
        let mut app = sample_app(&clock());
        let older = draft(&mut app, "Workshop dates");
        let archived = app.mailbox().messages().first().map(|row| row.id).unwrap();
        app.mailbox_mut().act(archived, crate::mailbox::Act::Archive);
        let mut harness = window(app);
        wait_out_the_toast(&mut harness);
        assert!(harness.state().mailbox().can_undo() && harness.state().mailbox().undoable().is_none(), "{name}");
        press_discard(&mut harness, "Workshop dates");
        place(harness.state_mut());
        harness.run();
        harness.get_by_label("Draft discarded");
        command(&mut harness, Key::Z);
        harness.run();
        assert_eq!(harness.state().discarding(), None, "{name}: Ctrl+Z took back the discard");
        assert!(harness.state().mailbox().can_undo(), "{name}: the older act is still in the history");
        wait_out_the_toast(&mut harness);
        assert!(exists(harness.state_mut(), older), "{name}: the draft is kept");
        harness.state_mut().show_mail();
        harness.run();
        assert!(harness.state().mailbox().messages().iter().all(|row| row.id != archived), "{name}: still archived");
    }
}

/// What the owner sees of the window, in short: the place, the mailbox or pile shown, the calendar's view, the draft in
/// the composer, the settings and their page, a waiting discard.
fn seen(app: &InboxApp) -> String {
    format!(
        "{:?} {:?} {:?} {:?} {} {:?} {:?}",
        app.screen(),
        app.mailbox().place(),
        app.calendar().view(),
        app.composer().map(crate::compose::Composer::draft),
        app.settings().is_open(),
        app.settings().section(),
        app.discarding(),
    )
}

/// Every ask a press on Home can make, run as the window runs it ([`InboxApp::apply_home`], then the action it gives
/// back through `run_action`), changes what the owner sees, but the sample mail's, which is drawn coming soon and asks
/// nothing (`draw::home::tests::no_pressable_control_on_home_is_a_no_op`). An arm emptied fails it.
#[test]
fn every_ask_of_home_changes_what_the_owner_sees() {
    use super::home::Asked;
    use crate::home::Counted;
    let asks = |draft: u64| {
        [
            Asked::Count(Counted::ToRead),
            Asked::Count(Counted::ToScreen),
            Asked::Count(Counted::BackToday),
            Asked::Count(Counted::Outbox),
            Asked::Count(Counted::NotSent),
            Asked::OpenOutbox,
            Asked::AllDrafts,
            Asked::Week,
            Asked::Today,
            Asked::OpenDraft(draft),
            Asked::DiscardDraft(draft),
            Asked::ReadMail,
            Asked::Write,
            Asked::Calendar,
            Asked::AddAccount,
            Asked::OpenAccount,
        ]
    };
    for at in 0..asks(0).len() {
        let mut app = sample_app(&clock());
        let kept = draft(&mut app, "Workshop dates");
        let Some(&ask) = asks(kept).get(at) else { continue };
        let mut harness = window(app);
        let ctx = harness.ctx.clone();
        assert_eq!(harness.state().screen(), Screen::Home);
        let before = seen(harness.state());
        if let Some(action) = harness.state_mut().apply_home(&ctx, ask) {
            harness.state_mut().run_action(&ctx, action);
        }
        harness.run();
        assert_ne!(seen(harness.state()), before, "{ask:?} changes nothing the owner sees");
    }
}
