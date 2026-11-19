// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The drafts place: the kept drafts of `compose.list` as draft rows, newest first, or its empty state, in light and
//! dark; J, K and the arrows walk the rows and Enter opens the keyed draft to go on writing, as a press does; discard
//! hides a row behind its Undo and nothing is gone before the toast ends; hostile draft text is one clean row; nothing
//! of the place acts under the command line's results, and a frame at rest runs no command.

use egui::accesskit::Role;
use egui::{Key, Modifiers, Rect, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_model::Role as Folder;
use inboxcraft_time::{FakeClock, Timestamp};
use serde_json::{Value, json};

use super::InboxApp;
use crate::mailbox::{Mailbox, Place};
use crate::sample;

/// 2026-11-05T09:00:00Z.
const START: i64 = 1_793_869_200;

/// The sample window, one clock for the session and the window.
fn sample_app() -> InboxApp {
    let clock = FakeClock::new(Timestamp::from_unix(START, 0).unwrap());
    InboxApp::new(Mailbox::new(sample::session(Box::new(clock.clone())).unwrap(), Box::new(clock)))
}

/// A kept draft of the first account to `to` (none when empty) with `subject` and `text`; its id.
fn draft(app: &mut InboxApp, to: &str, subject: &str, text: &str) -> u64 {
    let mailbox = app.mailbox_mut();
    let account = mailbox.accounts().first().map(|account| account.id).unwrap();
    let id = mailbox.run("compose.new", &json!({ "account": account })).unwrap().get("id").and_then(Value::as_u64).unwrap();
    let to: Vec<&str> = if to.is_empty() { Vec::new() } else { vec![to] };
    mailbox.run("compose.update", &json!({ "id": id, "to": to, "subject": subject, "text": text })).unwrap();
    id
}

/// `app` on the drafts place in a window of the frames' size, in `theme`, run.
fn drafts_place(mut app: InboxApp, theme: egui::Theme) -> Harness<'static, InboxApp> {
    app.show_mail();
    app.mailbox_mut().open(Place::View(Folder::Drafts));
    app.set_reduce_motion(true);
    let mut harness = Harness::builder()
        .with_size(vec2(1440.0, 900.0))
        .with_theme(theme)
        .allow_missing_glyphs()
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

/// The rows in the list's pane, top to bottom: their names and rects.
fn rows(harness: &Harness<'static, InboxApp>) -> Vec<(String, Rect)> {
    let Some(view) = super::list::view_rect(&harness.ctx) else { return Vec::new() };
    let mut rows: Vec<(String, Rect)> = harness
        .query_all_by_role(Role::ListItem)
        .filter(|node| view.contains(node.rect().center()))
        .filter_map(|node| Some((node.accesskit_node().label()?, node.rect())))
        .collect();
    rows.sort_by(|a, b| a.1.top().total_cmp(&b.1.top()));
    rows
}

fn names(harness: &Harness<'static, InboxApp>) -> Vec<String> {
    rows(harness).into_iter().map(|(name, _)| name).collect()
}

/// The row whose name starts with `subject`.
fn row(harness: &Harness<'static, InboxApp>, subject: &str) -> Rect {
    let found = rows(harness).into_iter().find(|(name, _)| name.starts_with(subject));
    let Some((_, rect)) = found else { panic!("no draft row {subject:?} in {:?}", names(harness)) };
    rect
}

/// The draft whose row has the keyboard.
fn keyed(harness: &Harness<'static, InboxApp>) -> Option<u64> {
    super::list::focused_draft(&harness.ctx, harness.state().mailbox())
}

fn key(harness: &mut Harness<'static, InboxApp>, key: Key) {
    harness.key_press(key);
    harness.run();
}

fn opened(harness: &Harness<'static, InboxApp>) -> Option<u64> {
    harness.state().composer().map(crate::compose::Composer::draft)
}

/// The sample's Drafts folder on the server of its first account, which holds one message.
fn server_folder(harness: &Harness<'static, InboxApp>) -> u64 {
    let mailbox = harness.state().mailbox();
    let account = mailbox.accounts().first().map(|account| account.id).unwrap();
    mailbox.folders_of(account).iter().find(|folder| folder.role == Some(Folder::Drafts)).map(|folder| folder.id).unwrap()
}

/// What the row of [`server_folder`] says: its count, and whose it is over the sample's two accounts.
const SERVER_ROW: &str = "1 in the server's Drafts folder · Personal";

/// Whether the row of [`server_folder`] has the keyboard.
fn on_server_row(harness: &Harness<'static, InboxApp>) -> bool {
    let id = super::list::server_row_id(server_folder(harness));
    harness.ctx.memory(|memory| memory.has_focus(id))
}

fn total(app: &mut InboxApp) -> u64 {
    app.mailbox_mut().run("compose.list", &json!({})).unwrap().get("total").and_then(Value::as_u64).unwrap()
}

fn exists(app: &mut InboxApp, id: u64) -> bool {
    app.mailbox_mut().run("compose.show", &json!({ "id": id })).is_ok()
}

#[test]
fn the_filled_place_lists_the_drafts_of_compose_list_newest_first_in_light_and_dark() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let mut app = sample_app();
        draft(&mut app, "Sunniva Aalto <sunniva@example.org>", "Price list for the winter workshops", "The prices for December.");
        draft(&mut app, "", "", "Notes for the open day");
        let harness = drafts_place(app, theme);
        let rows = rows(&harness);
        let names: Vec<&str> = rows.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(rows.len(), 2, "{theme:?}: {names:?}");
        // Newest first: the one without a subject or a recipient, then the price list.
        assert!(names[0].starts_with("(no subject), no recipient yet, edited "), "{theme:?}: {names:?}");
        assert!(names[0].ends_with(", Notes for the open day"), "{theme:?}: the start of its text is told: {names:?}");
        assert!(names[1].starts_with("Price list for the winter workshops, to Sunniva Aalto, edited "), "{theme:?}: {names:?}");
        for (name, rect) in &rows {
            assert_eq!(rect.height().round(), 44.0, "{theme:?}: {name} is a draft row");
        }
        assert!(rows[0].1.bottom() <= rows[1].1.top() + 0.5, "{theme:?}: one under another");
        harness.get_by_label("2 drafts");
        assert!(harness.query_by_label("coming soon").is_none(), "{theme:?}: the place works, nothing of it is coming soon");
        assert!(harness.query_by_label("No drafts.").is_none(), "{theme:?}");
    }
}

#[test]
fn the_empty_place_says_there_are_no_drafts_in_light_and_dark() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let harness = drafts_place(sample_app(), theme);
        assert!(names(&harness).is_empty(), "{theme:?}");
        harness.get_by_label("No drafts.");
        harness.get_by_label("no drafts");
        assert!(harness.query_by_label("coming soon").is_none(), "{theme:?}");
    }
}

#[test]
fn j_k_and_the_arrows_walk_the_drafts_and_enter_opens_the_keyed_one() {
    let mut app = sample_app();
    let oldest = draft(&mut app, "", "Kiln rota", "");
    let middle = draft(&mut app, "", "Glaze order", "");
    let newest = draft(&mut app, "", "Workshop dates", "");
    let mut harness = drafts_place(app, egui::Theme::Dark);
    assert_eq!(keyed(&harness), None);
    key(&mut harness, Key::J);
    assert_eq!(keyed(&harness), Some(newest), "J from nothing keys the first row");
    key(&mut harness, Key::J);
    assert_eq!(keyed(&harness), Some(middle));
    key(&mut harness, Key::ArrowDown);
    assert_eq!(keyed(&harness), Some(oldest));
    key(&mut harness, Key::J);
    assert_eq!(keyed(&harness), None, "past the last draft, the server's Drafts folder's row");
    assert!(on_server_row(&harness));
    key(&mut harness, Key::J);
    assert!(on_server_row(&harness), "the last row stays keyed at the end");
    key(&mut harness, Key::K);
    assert_eq!(keyed(&harness), Some(oldest));
    key(&mut harness, Key::K);
    assert_eq!(keyed(&harness), Some(middle));
    key(&mut harness, Key::ArrowUp);
    assert_eq!(keyed(&harness), Some(newest));
    key(&mut harness, Key::ArrowDown);
    assert_eq!(opened(&harness), None, "walking opens nothing");
    assert!(harness.state().mailbox().selected().is_none(), "nor selects a message");
    key(&mut harness, Key::Enter);
    harness.run();
    assert_eq!(opened(&harness), Some(middle), "Enter opens the keyed draft to go on writing");
    assert!(harness.ctx.memory(|memory| memory.has_focus(super::compose::field_id(crate::compose::Line::To))), "the keys in its To line");
    // Esc leaves the composer's line: the keys are no longer typing into the draft.
    key(&mut harness, Key::Escape);
    harness.run();
    assert!(!harness.ctx.memory(|memory| memory.has_focus(super::compose::field_id(crate::compose::Line::To))));
}

#[test]
fn a_press_opens_a_draft_and_a_hover_or_a_focus_changes_nothing() {
    let mut app = sample_app();
    let older = draft(&mut app, "", "Kiln rota", "");
    draft(&mut app, "", "Glaze order", "");
    let mut harness = drafts_place(app, egui::Theme::Light);
    let before = total(harness.state_mut());
    let at = row(&harness, "Kiln rota").center();
    harness.hover_at(at);
    harness.run();
    harness.run();
    key(&mut harness, Key::Tab);
    key(&mut harness, Key::Tab);
    assert_eq!(opened(&harness), None, "a hover and the keyboard passing open nothing");
    assert_eq!(total(harness.state_mut()), before, "and discard nothing");
    harness.get_by_label("discard");
    harness.query_all_by_role(Role::ListItem).find(|node| node.rect().contains(at)).unwrap().click();
    harness.run();
    harness.run();
    assert_eq!(opened(&harness), Some(older), "the row pressed opens its draft");
}

#[test]
fn discard_hides_the_row_behind_its_undo_and_nothing_is_gone_before_the_toast_ends() {
    let mut app = sample_app();
    let older = draft(&mut app, "", "Kiln rota", "");
    draft(&mut app, "", "Glaze order", "");
    let mut harness = drafts_place(app, egui::Theme::Dark);
    harness.hover_at(row(&harness, "Kiln rota").center());
    harness.run();
    harness.run();
    harness.get_by_label("discard").click();
    harness.run();
    harness.run();
    let left = names(&harness);
    assert_eq!(left.len(), 1, "the row goes at once: {left:?}");
    assert!(left[0].starts_with("Glaze order"));
    harness.get_by_label("1 draft");
    harness.get_by_label("Draft discarded");
    assert!(exists(harness.state_mut(), older), "the discard waits for its toast");
    assert_eq!(harness.state().discarding(), Some(older));
    harness.get_by_role_and_label(Role::Button, "undo").click();
    harness.run();
    harness.run();
    assert_eq!(harness.state().discarding(), None);
    assert_eq!(names(&harness).len(), 2, "Undo brings the row back");
    assert!(exists(harness.state_mut(), older));
}

#[test]
fn a_discard_carried_out_leaves_the_list_and_a_saved_draft_comes_in() {
    let mut app = sample_app();
    let older = draft(&mut app, "", "Kiln rota", "");
    draft(&mut app, "", "Glaze order", "");
    let mut harness = drafts_place(app, egui::Theme::Dark);
    harness.state_mut().discard_draft(older);
    harness.run();
    let later = harness.ctx.input(|input| input.time) + crate::status::UNDO_SECONDS + 1.0;
    while harness.ctx.input(|input| input.time) < later {
        harness.step();
    }
    harness.run();
    assert!(!exists(harness.state_mut(), older), "carried out when the toast ends");
    assert_eq!(names(&harness).len(), 1);
    // A draft saved elsewhere (here by the engine) comes into the list on the next frame.
    draft(harness.state_mut(), "", "Open day", "");
    harness.run();
    assert!(names(&harness)[0].starts_with("Open day"), "{:?}", names(&harness));
}

/// A text from outside at its worst: 10 000 characters with CR, LF, NUL, bidi controls, Hebrew and Arabic.
fn hostile(word: &str) -> String {
    let piece = format!("{word}\r\n\0\u{202e}\u{2067}שלום مرحبا\u{2069}\u{202c} ");
    piece.repeat(10_000 / piece.chars().count() + 1).chars().take(10_000).collect()
}

#[test]
fn a_hostile_draft_is_one_clean_row_inside_the_list() {
    let mut app = sample_app();
    // The longest recipient the engine keeps: a name of hostile words before its address.
    let to = format!("{} <x@example.org>", hostile("name").chars().take(200).collect::<String>());
    // The longest subject the engine keeps, and a text of 10 000 characters.
    let subject: String = hostile("subject")
        .chars()
        .scan(0, |bytes, c| {
            *bytes += c.len_utf8();
            (*bytes <= 998).then_some(c)
        })
        .collect();
    draft(&mut app, &to, &subject, &hostile("text"));
    let harness = drafts_place(app, egui::Theme::Dark);
    let rows = rows(&harness);
    assert_eq!(rows.len(), 1);
    let (name, rect) = &rows[0];
    assert_eq!(rect.height().round(), 44.0, "one row");
    let view = super::list::view_rect(&harness.ctx).unwrap();
    assert!(view.expand(0.5).contains_rect(*rect), "{rect:?} inside {view:?}");
    assert!(!name.contains(['\r', '\n', '\0', '\u{202e}', '\u{2067}', '\u{2069}', '\u{202c}']), "plain words only: {name:?}");
    assert!(name.chars().count() < 2 * crate::component::MOST + 200, "capped where the list is built: {}", name.chars().count());
    let draft = harness.state().mailbox().drafts().next().unwrap();
    for text in [draft.subject.as_deref().unwrap(), &draft.to, &draft.start] {
        assert!(text.chars().count() <= crate::component::MOST + 4, "capped in the list's model: {}", text.chars().count());
    }
}

#[test]
fn nothing_of_the_place_acts_under_the_command_line_s_results() {
    let mut app = sample_app();
    draft(&mut app, "", "Kiln rota", "");
    let mut harness = drafts_place(app, egui::Theme::Dark);
    let at = row(&harness, "Kiln rota").center();
    harness.key_press_modifiers(Modifiers::COMMAND, Key::K);
    harness.run();
    assert!(harness.state().command().results());
    // A screen reader's activate, which egui's layers do not stop; then the pointer, which the results take.
    harness.query_all_by_role(Role::ListItem).find(|node| node.rect().contains(at)).unwrap().click_accesskit();
    harness.run();
    harness.run();
    assert_eq!(opened(&harness), None, "an activate under the results opens nothing");
    assert!(harness.state().command().results(), "and leaves the results open");
    harness.query_all_by_role(Role::ListItem).find(|node| node.rect().contains(at)).unwrap().click();
    harness.run();
    harness.run();
    assert_eq!(opened(&harness), None, "a press under the results opens nothing");
}

#[test]
fn a_frame_at_rest_runs_no_command() {
    let mut app = sample_app();
    draft(&mut app, "", "Kiln rota", "");
    let mut harness = drafts_place(app, egui::Theme::Dark);
    harness.run();
    let runs = harness.state().mailbox().runs();
    for _ in 0..5 {
        harness.step();
    }
    assert_eq!(harness.state().mailbox().runs(), runs, "the drafts are read again only after a write");
}

#[test]
fn the_head_counts_every_draft_past_the_list_s_limit_and_the_newest_comes_first() {
    let mut app = sample_app();
    let account = app.mailbox().accounts().first().map(|account| account.id).unwrap();
    for _ in 0..inboxcraft_engine::MAX_LIST {
        app.mailbox_mut().run("compose.new", &json!({ "account": account })).unwrap();
    }
    draft(&mut app, "", "Newest", "");
    let harness = drafts_place(app, egui::Theme::Light);
    harness.get_by_label(&format!("{} drafts", inboxcraft_engine::MAX_LIST + 1));
    assert!(names(&harness)[0].starts_with("Newest"), "{:?}", names(&harness).first());
}

#[test]
fn a_write_of_anything_else_reads_no_drafts_and_a_save_from_the_composer_updates_its_row() {
    let mut app = sample_app();
    let kiln = draft(&mut app, "", "Kiln rota", "");
    let mut harness = drafts_place(app, egui::Theme::Dark);
    harness.state_mut().mailbox_mut().run("ui.theme.set", &json!({ "mode": "dark" })).unwrap();
    let runs = harness.state().mailbox().runs();
    harness.run();
    assert_eq!(harness.state().mailbox().runs(), runs, "a write that is no draft's reads the drafts not again");
    harness.query_all_by_role(Role::ListItem).find(|node| node.rect().contains(row(&harness, "Kiln rota").center())).unwrap().click();
    harness.run();
    harness.run();
    assert_eq!(opened(&harness), Some(kiln));
    let (Some(composer), _) = harness.state_mut().composer_mut() else { panic!("the draft opens in the composer") };
    "Kiln rota, week 47".clone_into(composer.subject_mut());
    composer.edited();
    harness.state_mut().keep_draft_now();
    harness.run();
    row(&harness, "Kiln rota, week 47");
}

/// Presses the row of the draft whose name starts with `subject`.
fn press(harness: &mut Harness<'static, InboxApp>, subject: &str) {
    let at = row(harness, subject).center();
    harness.query_all_by_role(Role::ListItem).find(|node| node.rect().contains(at)).unwrap().click();
    harness.run();
    harness.run();
}

#[test]
fn a_draft_opened_here_brings_its_text_recipients_and_account_into_the_composer() {
    let mut app = sample_app();
    let studio = app.mailbox().accounts().get(1).map(|account| account.id).unwrap();
    let mailbox = app.mailbox_mut();
    let id = mailbox.run("compose.new", &json!({ "account": studio })).unwrap().get("id").and_then(Value::as_u64).unwrap();
    let update = json!({ "id": id, "to": ["Odile Brannock <odile@example.org>"], "subject": "Glaze order", "text": "Two kilos of celadon." });
    mailbox.run("compose.update", &update).unwrap();
    let mut harness = drafts_place(app, egui::Theme::Light);
    press(&mut harness, "Glaze order");
    let composer = harness.state().composer().unwrap();
    assert_eq!(composer.draft(), id);
    assert_eq!(composer.account(), studio, "written from its own account");
    assert_eq!(composer.subject(), "Glaze order");
    assert_eq!(composer.text(), "Two kilos of celadon.");
    let to: Vec<&str> = composer.chips(crate::compose::Line::To).iter().map(|chip| chip.text.as_str()).collect();
    assert_eq!(to, ["Odile Brannock <odile@example.org>"]);
}

#[test]
fn opening_a_draft_while_another_holds_unsaved_text_keeps_that_one() {
    let mut app = sample_app();
    let kiln = draft(&mut app, "", "Kiln rota", "");
    let glaze = draft(&mut app, "", "Glaze order", "");
    let mut harness = drafts_place(app, egui::Theme::Light);
    press(&mut harness, "Kiln rota");
    assert_eq!(opened(&harness), Some(kiln));
    let (Some(composer), _) = harness.state_mut().composer_mut() else { panic!("the draft opens in the composer") };
    "Firing on Thursday.".clone_into(composer.text_mut());
    composer.edited();
    // The way a press and Enter in this place open a draft, before the composer's own save has run: only the switch
    // keeps the text.
    let ctx = harness.ctx.clone();
    harness.state_mut().open_draft(&ctx, glaze);
    harness.run();
    assert_eq!(opened(&harness), Some(glaze));
    let shown = harness.state_mut().mailbox_mut().run("compose.show", &json!({ "id": kiln })).unwrap();
    assert_eq!(shown.get("text").and_then(Value::as_str), Some("Firing on Thursday."), "{shown}");
}

#[test]
fn under_the_drafts_a_row_opens_the_server_s_drafts_folder_with_its_count() {
    let mut app = sample_app();
    draft(&mut app, "", "Kiln rota", "");
    let mut harness = drafts_place(app, egui::Theme::Light);
    let folder = server_folder(&harness);
    let server = harness.get_by_label(SERVER_ROW).rect();
    assert!(row(&harness, "Kiln rota").bottom() <= server.top() + 0.5, "under the kept drafts");
    harness.get_by_label(SERVER_ROW).click();
    harness.run();
    harness.run();
    assert_eq!(harness.state().mailbox().place(), Place::Folder(folder), "the press opens the folder");
    assert!(harness.state().mailbox().messages().iter().any(|message| message.folder == folder), "and lists its mail");
}

#[test]
fn with_no_kept_drafts_the_empty_state_shows_the_server_s_row_too() {
    let mut harness = drafts_place(sample_app(), egui::Theme::Dark);
    let folder = server_folder(&harness);
    harness.get_by_label("No drafts.");
    harness.get_by_label(SERVER_ROW);
    key(&mut harness, Key::J);
    assert!(on_server_row(&harness), "J keys the row");
    key(&mut harness, Key::Enter);
    harness.run();
    assert_eq!(harness.state().mailbox().place(), Place::Folder(folder), "Enter opens the folder");
}

#[test]
fn enter_on_the_keyed_server_s_row_opens_its_folder() {
    let mut app = sample_app();
    draft(&mut app, "", "Kiln rota", "");
    let mut harness = drafts_place(app, egui::Theme::Dark);
    let folder = server_folder(&harness);
    key(&mut harness, Key::J);
    key(&mut harness, Key::J);
    assert!(on_server_row(&harness));
    key(&mut harness, Key::Enter);
    harness.run();
    assert_eq!(harness.state().mailbox().place(), Place::Folder(folder));
    assert_eq!(opened(&harness), None, "no draft opens");
}

#[test]
fn tab_and_shift_tab_reach_the_server_s_row_after_the_last_draft() {
    let mut app = sample_app();
    draft(&mut app, "", "Kiln rota", "");
    let mut harness = drafts_place(app, egui::Theme::Light);
    let kiln = super::list::draft_row_id(keyed_draft(&harness));
    harness.ctx.memory_mut(|memory| memory.request_focus(kiln));
    harness.run();
    let mut steps = 0;
    while !on_server_row(&harness) && steps < 8 {
        key(&mut harness, Key::Tab);
        steps += 1;
    }
    assert!(on_server_row(&harness), "Tab from the draft's row reaches the server's row");
    harness.key_press_modifiers(Modifiers::SHIFT, Key::Tab);
    harness.run();
    assert!(!on_server_row(&harness), "Shift Tab goes back");
    let back = harness.ctx.memory(|memory| memory.focused());
    let row = harness.ctx.read_response(kiln).map(|response| response.rect).unwrap();
    let on = back.and_then(|id| harness.ctx.read_response(id)).map(|response| response.rect).unwrap();
    assert!(row.expand(0.5).contains_rect(on), "to the draft's row or one of its links");
}

/// The only kept draft of the drafts place.
fn keyed_draft(harness: &Harness<'static, InboxApp>) -> u64 {
    harness.state().mailbox().drafts().map(|draft| draft.id).next().unwrap()
}
