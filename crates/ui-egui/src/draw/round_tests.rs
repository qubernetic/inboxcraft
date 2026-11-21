// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The frame's regions measured against the frames in the whole-app picture round (#325): the list 440 wide beside
//! the reader's empty state, the docked composer flush with the body, the status line's keys by what the window
//! shows, its separators, the three frame hairlines and the command line's head.

use egui::{Color32, Rect, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use inboxcraft_model::Role as Folder;
use inboxcraft_time::{FakeClock, Timestamp};

use super::{InboxApp, shell};
use crate::mailbox::{Mailbox, Place};
use crate::sample;
use crate::theme::{Mode, TextScale, Theme};

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

fn sample_app() -> InboxApp {
    let mut app = InboxApp::at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    app.set_reduce_motion(true);
    app
}

/// `app` in a window `size` big, run.
fn window(app: InboxApp, size: egui::Vec2) -> Harness<'static, InboxApp> {
    let mut harness = Harness::builder()
        .with_size(size)
        .with_theme(egui::Theme::Dark)
        .with_max_steps(12)
        .allow_missing_glyphs()
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

fn frames_window(app: InboxApp) -> Harness<'static, InboxApp> {
    window(app, vec2(1440.0, 900.0))
}

fn regions(harness: &Harness<'static, InboxApp>) -> shell::Regions {
    shell::laid(&harness.ctx).unwrap()
}

fn shows(harness: &Harness<'static, InboxApp>, text: &str) -> bool {
    harness.query_by_label_contains(text).is_some()
}

/// The texts the last frame painted, and where.
fn painted(harness: &Harness<'static, InboxApp>) -> Vec<(String, egui::Pos2)> {
    fn walk(shape: &egui::Shape, found: &mut Vec<(String, egui::Pos2)>) {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, found)),
            egui::Shape::Text(text) => found.push((text.galley.text().to_owned(), text.pos)),
            _ => {}
        }
    }
    let mut found = Vec::new();
    for clipped in &harness.output().shapes {
        walk(&clipped.shape, &mut found);
    }
    found
}

/// The words of the status line's key hints, left to right.
pub(super) fn key_words(harness: &Harness<'static, InboxApp>) -> Vec<String> {
    let status = regions(harness).status;
    let hints = ["next", "previous", "done", "snooze", "open", "write", "send", "commands"];
    let mut found: Vec<(f32, String)> = painted(harness)
        .into_iter()
        .filter(|(text, at)| status.contains(*at) && hints.contains(&text.as_str()))
        .map(|(text, at)| (at.x, text))
        .collect();
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    found.into_iter().map(|(_, word)| word).collect()
}

fn drafts(mut app: InboxApp, kept: usize) -> Harness<'static, InboxApp> {
    for at in 0..kept {
        let mailbox = app.mailbox_mut();
        let account = mailbox.accounts().first().map(|account| account.id).unwrap();
        let id = mailbox.run("compose.new", &serde_json::json!({ "account": account })).unwrap();
        let id = id.get("id").and_then(serde_json::Value::as_u64).unwrap();
        mailbox.run("compose.update", &serde_json::json!({ "id": id, "subject": format!("Draft {at}") })).unwrap();
    }
    app.mailbox_mut().open(Place::View(Folder::Drafts));
    frames_window(app)
}

#[test]
fn with_nothing_open_the_list_stands_440_wide_beside_the_reader_s_empty_state() {
    // `shell/default` and `drafts-list/*` at 1440 × 900: the list 440 wide at the body's left, a hairline, the reader
    // 764 wide with its empty state in its middle.
    let harness = frames_window(sample_app());
    assert!(harness.state().reader.open_id().is_none());
    let body = regions(&harness).body;
    let list = harness.state().list_rect.unwrap();
    assert_eq!((list.left(), list.width()), (body.left(), 440.0), "the list's 440, not the whole body {body:?}");
    assert!(shows(&harness, "Pick a message."), "the reader says nothing is open");

    let filled = drafts(sample_app(), 2);
    assert_eq!(filled.state().list_rect.map(|list| list.width()), Some(440.0), "drafts: the list 440 wide");
    assert!(shows(&filled, "Pick a draft."), "drafts: pick one, it opens in the composer");
    assert!(painted(&filled).iter().any(|(text, _)| text.contains("opens the draft where you left it")));

    let empty = drafts(sample_app(), 0);
    assert_eq!(empty.state().list_rect.map(|list| list.width()), Some(440.0), "no drafts: the list 440 wide");
    assert!(shows(&empty, "Nothing to open."), "no drafts: nothing to open");
    assert!(painted(&empty).iter().any(|(text, _)| text == "A message opens here when the list holds one."));

    // Under 980 the list holds the one pane alone.
    let narrow = window(sample_app(), vec2(900.0, 700.0));
    let (body, list) = (regions(&narrow).body, narrow.state().list_rect.unwrap());
    assert!(list.width() > 441.0 && (list.right() - body.right()).abs() < 1.0, "one pane: the list {list:?} over the body {body:?}");
    assert!(!shows(&narrow, "Pick a message."));
}

#[test]
fn the_docked_composer_is_flush_with_the_body_like_the_list() {
    // `size-1100/composer`: the composer at 441, 0 of the body, 600 × 826: no band of the ground above, beside or under it.
    let mut app = sample_app();
    app.compose();
    let harness = window(app, vec2(1100.0, 900.0));
    let body = regions(&harness).body;
    let composer = harness.state().reader_place.unwrap();
    assert_eq!(composer.top(), body.top(), "flush with the body's top");
    assert_eq!(composer.right(), body.right(), "flush with the body's right");
    assert_eq!(composer.bottom(), body.bottom(), "flush with the body's foot");
    assert_eq!(composer.left(), body.left() + 440.0, "beside the list's 440, its own line first");
}

#[test]
fn the_status_line_s_keys_follow_what_the_window_shows() {
    // `shell/default`, `drafts-list/filled`: next, previous, open, commands; `shell/reader-open`: next, previous, done,
    // snooze, commands; `drafts-list/empty`: write, commands; `size-1100/composer`: send, commands.
    let words = |list: &[&str]| list.iter().map(|word| (*word).to_owned()).collect::<Vec<_>>();
    let nothing_open = frames_window(sample_app());
    assert_eq!(key_words(&nothing_open), words(&["next", "previous", "open", "commands"]));
    assert_eq!(key_words(&drafts(sample_app(), 2)), words(&["next", "previous", "open", "commands"]));
    assert_eq!(key_words(&drafts(sample_app(), 0)), words(&["write", "commands"]));

    let mut app = sample_app();
    let first = app.mailbox().messages().first().map(|row| row.id).unwrap();
    app.mailbox_mut().select(Some(first));
    let reading = frames_window(app);
    assert!(reading.state().reader.open_id().is_some());
    assert_eq!(key_words(&reading), words(&["next", "previous", "done", "snooze", "commands"]));

    let mut app = sample_app();
    app.compose();
    assert_eq!(key_words(&window(app, vec2(1100.0, 900.0))), words(&["send", "commands"]));
}

#[test]
fn the_status_line_s_separators_stand_4_from_the_facts_beside_them() {
    // `status-line/synced`: an item, 4, the separator 7 × 16 at 5 from the line's top, 4, the next item.
    let harness = frames_window(sample_app());
    let status = regions(&harness).status;
    let dots: Vec<Rect> = harness.query_all_by_label("·").map(|node| node.rect()).filter(|rect| status.contains_rect(*rect)).collect();
    let facts: Vec<Rect> =
        ["offline", "showing what is kept on this computer"].iter().filter_map(|text| harness.query_by_label(text).map(|node| node.rect())).collect();
    assert_eq!((dots.len(), facts.len()), (1, 2), "two facts and a separator between them");
    let (dot, before, after) = (dots[0], facts[0], facts[1]);
    assert!((dot.left() - before.right() - 4.0).abs() < 0.5, "4 after the fact: {before:?} {dot:?}");
    assert!((after.left() - dot.right() - 4.0).abs() < 0.5, "4 before the next: {dot:?} {after:?}");
    assert!((dot.width() - 7.0).abs() < 0.5, "the separator 7 wide: {dot:?}");
}

/// The colours of the line segments that run along `y` (across) or `x` (down) through `at`.
fn hairline_colours(harness: &Harness<'static, InboxApp>, at: egui::Pos2, across: bool) -> Vec<Color32> {
    fn walk(shape: &egui::Shape, at: egui::Pos2, across: bool, found: &mut Vec<Color32>) {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, at, across, found)),
            egui::Shape::LineSegment { points: [a, b], stroke } => {
                let hit = if across {
                    (a.y - at.y).abs() < 1.0 && (b.y - at.y).abs() < 1.0 && a.x.min(b.x) <= at.x && a.x.max(b.x) >= at.x
                } else {
                    (a.x - at.x).abs() < 1.0 && (b.x - at.x).abs() < 1.0 && a.y.min(b.y) <= at.y && a.y.max(b.y) >= at.y
                };
                if hit {
                    found.push(stroke.color);
                }
            }
            _ => {}
        }
    }
    let mut found = Vec::new();
    for clipped in &harness.output().shapes {
        walk(&clipped.shape, at, across, &mut found);
    }
    found
}

#[test]
fn the_three_frame_hairlines_are_painted_in_line() {
    // `shell/default`: the hairlines under the command line (y 45), between the index and the body (x 232) and over
    // the status line (y 872) are `line`, the index's own one over the owner `panel_line`.
    let harness = frames_window(sample_app());
    let line = Theme::new(Mode::Dark, TextScale::default()).palette.line;
    let laid = regions(&harness);
    let under_command = pos2(700.0, laid.command.bottom() + 0.5);
    let beside_index = pos2(laid.index.right() + 0.5, 400.0);
    let over_status = pos2(700.0, laid.index.bottom() + 0.5);
    for (at, across, name) in
        [(under_command, true, "under the command line"), (beside_index, false, "beside the index"), (over_status, true, "over the status line")]
    {
        let colours = hairline_colours(&harness, at, across);
        assert!(colours.contains(&line), "{name}: {colours:?} holds `line` {line:?}");
    }
}

#[test]
fn the_command_line_s_head_says_how_many_of_all_and_what_enter_does() {
    // `command-line/commands`: "sn" typed, the head "COMMANDS" and "2 of 82 · Enter runs the marked one".
    let mut harness = frames_window(sample_app());
    harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::K);
    harness.run();
    harness.get_by_role_and_label(egui::accesskit::Role::TextInput, "Command line").type_text(">");
    harness.run();
    harness.get_by_role_and_label(egui::accesskit::Role::TextInput, "Command line").type_text("sn");
    harness.run();
    let shown = harness.state().command().results();
    assert!(shown);
    let head: Vec<(usize, usize)> = painted(&harness)
        .into_iter()
        .filter_map(|(text, _)| {
            let (shown, rest) = text.strip_suffix(" · Enter runs the marked one")?.split_once(" of ")?;
            Some((shown.parse().ok()?, rest.parse().ok()?))
        })
        .collect();
    let rows = harness.query_all_by_role(egui::accesskit::Role::MenuItem).count();
    assert_eq!(head.len(), 1, "one head: {head:?}");
    let (shown, all) = head[0];
    assert!(shown > 0 && shown < all && shown <= rows, "the head says how many of all show: {shown} of {all}, {rows} rows");
}
