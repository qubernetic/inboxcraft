// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The window's two breakpoints. Under 1280 the index is 56 and the command field takes its room, the status line
//! keeps three facts and three keys, the reader's why line leaves out its last part and the calendar's toolbar its
//! note; the list keeps its 440 beside a second pane. Under 980 one pane: the list and the reader, the list and the
//! composer, the week and the calendar's side panel swap; the status line keeps two and two. 800 is the least width.
//! And what is open while the window is dragged across them: nothing typed is lost, the keyboard is never on nothing,
//! no float stays over a pane that left.

use egui::accesskit::Role;
use egui::{Key, Modifiers, Rect, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_time::{FakeClock, Timestamp};

use super::InboxApp;
use crate::keymap;
use crate::mailbox::Mailbox;
use crate::sample;

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

/// The sample window `width` wide at `percent` text, nothing open yet.
fn window(width: f32, percent: u16) -> Harness<'static, InboxApp> {
    let mut app = InboxApp::at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    app.set_text_scale(crate::theme::TextScale::from_percent(percent).unwrap());
    let mut harness = Harness::builder().with_size(vec2(width, 800.0)).with_max_steps(12).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

/// [`window`] with New message open in the reader's place.
fn composing(width: f32, percent: u16) -> Harness<'static, InboxApp> {
    let mut harness = window(width, percent);
    crate::draw::tests::new_message(&mut harness);
    harness.run();
    harness
}

/// Every text painted in the last frame, as the box it covers.
fn texts(harness: &Harness<'static, InboxApp>) -> Vec<(Rect, String)> {
    fn open(shape: &egui::Shape, out: &mut Vec<(Rect, String)>) {
        match shape {
            egui::Shape::Vec(inner) => inner.iter().for_each(|shape| open(shape, out)),
            egui::Shape::Text(text) if !text.galley.text().trim().is_empty() => {
                out.push((text.visual_bounding_rect(), text.galley.text().to_owned()));
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    harness.output().shapes.iter().for_each(|clipped| open(&clipped.shape, &mut out));
    out
}

/// What stands over something else in `band`: texts over texts, texts over a control they are not in, controls over
/// controls; an empty list when nothing does.
fn overdrawn(harness: &Harness<'static, InboxApp>, band: Rect) -> Vec<String> {
    let inside = |rect: &Rect| band.contains(rect.center());
    let words: Vec<(Rect, String)> = texts(harness).into_iter().filter(|(rect, _)| inside(rect)).collect();
    let controls: Vec<(Rect, String)> = harness
        .root()
        .children_recursive()
        .filter(|node| matches!(node.accesskit_node().role(), Role::Button | Role::TextInput | Role::ComboBox))
        .map(|node| (node.rect(), node.accesskit_node().label().unwrap_or_default()))
        .filter(|(rect, _)| inside(rect))
        .collect();
    let over = |a: &Rect, b: &Rect| a.shrink(0.5).intersects(b.shrink(0.5));
    let mut found = Vec::new();
    for (at, (rect, text)) in words.iter().enumerate() {
        for (other, said) in words.iter().skip(at + 1) {
            if over(rect, other) {
                found.push(format!("{text:?} {rect:?} over {said:?} {other:?}"));
            }
        }
        for (control, name) in &controls {
            if !control.contains(rect.center()) && over(rect, control) {
                found.push(format!("{text:?} {rect:?} over {name:?} {control:?}"));
            }
        }
    }
    for (at, (rect, name)) in controls.iter().enumerate() {
        for (other, said) in controls.iter().skip(at + 1) {
            if over(rect, other) {
                found.push(format!("{name:?} {rect:?} over {said:?} {other:?}"));
            }
        }
    }
    found
}

/// The list's head (its inbox views), while the list is drawn.
fn search_box(harness: &Harness<'static, InboxApp>) -> Option<Rect> {
    // The search is the command line's now: the list's head is found by the inbox's views in it.
    harness.query_by_label("the inbox").map(|node| node.rect())
}

/// A band across the window from `left` on, `top` to `bottom`.
fn band(left: f32, top: f32, bottom: f32) -> Rect {
    Rect::from_min_max(egui::pos2(left, top), egui::pos2(f32::INFINITY, bottom))
}

#[test]
fn at_every_width_nothing_in_the_list_s_or_the_composer_s_heads_stands_over_anything() {
    for percent in [100, 130] {
        let named = [720.0, 800.0, 830.0, 1024.0, 1100.0];
        // And every 6 points between, where the list and the composer each stand at their least.
        for width in named.into_iter().chain((0..64).map(|step| 722.0 + 6.0 * step as f32)) {
            let mut harness = composing(width, percent);
            harness.get_by_role_and_label(Role::TextInput, "To").type_text("odile@example.org,");
            harness.run();
            let at = format!("{percent} % {width}");
            // Right of the index: the list's head and its cards' heads, when the list is drawn.
            let folders = super::shell::laid(&harness.ctx).unwrap().body.left() + 1.0;
            if let Some(search) = search_box(&harness) {
                assert_eq!(overdrawn(&harness, band(folders, 0.0, search.bottom() + 24.0)), Vec::<String>::new(), "{at}: the list's head");
                // Each group's head whole: nothing over anything, no word of it cut.
                let heads: Vec<Rect> =
                    harness.query_all_by_role(Role::Heading).map(|node| node.rect()).filter(|head| head.left() < search.right()).collect();
                assert!(!heads.is_empty() || width < 980.0, "{at}: the groups' heads");
                for head in heads {
                    let inside = Rect::from_min_max(egui::pos2(head.left(), head.top()), egui::pos2(head.right() - 1.0, head.bottom()));
                    assert_eq!(overdrawn(&harness, inside), Vec::<String>::new(), "{at}: a group's head");
                    let cut: Vec<String> = texts(&harness)
                        .into_iter()
                        .filter(|(rect, text)| inside.contains(rect.center()) && text.contains('…'))
                        .map(|(_, text)| text)
                        .collect();
                    assert_eq!(cut, Vec::<String>::new(), "{at}: a group's head cut");
                }
            }
            let composer = super::compose::drawn_rect(&harness.ctx).unwrap();
            let close = harness.get_by_label("Close; the draft is kept").rect();
            assert_eq!(overdrawn(&harness, band(composer.left(), close.top(), close.bottom())), Vec::<String>::new(), "{at}: its head");
            let from = harness.query_all(egui_kittest::kittest::by().role(Role::ComboBox).label_contains("From ")).next().unwrap().rect();
            assert_eq!(overdrawn(&harness, band(composer.left(), from.top(), from.bottom())), Vec::<String>::new(), "{at}: From");
            // The To line: its chips, the field and "Cc  Bcc", each row of chips.
            let cc = harness.get_by_label("Cc  Bcc").rect();
            assert_eq!(overdrawn(&harness, band(composer.left(), cc.top() - 8.0, cc.bottom() + 8.0)), Vec::<String>::new(), "{at}: To");
        }
    }
}

#[test]
fn the_hidden_list_takes_no_tab_stop_and_no_key() {
    for percent in [100, 130] {
        // A row picked and flagged first, so its acts and Undo could run.
        let mut harness = window(720.0, percent);
        harness.query_all_by_label_contains("Draft logo for the bakery").next().unwrap().click();
        harness.run();
        let ctx = harness.ctx.clone();
        harness.state_mut().run_action(&ctx, keymap::FLAG);
        harness.run();
        assert!(harness.state().can_run(keymap::DELETE) && harness.state().can_run(keymap::UNDO), "{percent}: before");
        crate::draw::tests::new_message(&mut harness);
        harness.run();
        assert!(search_box(&harness).is_none(), "{percent}: the composer has the list's place");
        // The palette lists what `can_run` lets through and runs it through `run_action`: nothing of the hidden rows.
        for id in [keymap::DONE, keymap::ARCHIVE, keymap::READ, keymap::FLAG, keymap::DELETE, keymap::SNOOZE, keymap::UNDO, keymap::FIND] {
            assert!(!harness.state().can_run(id), "{percent}: {id} with the list hidden");
        }
        let (rows, picked) = (harness.state().mailbox().messages().len(), harness.state().mailbox().selected());
        for id in [keymap::DELETE, keymap::ARCHIVE, keymap::UNDO] {
            harness.state_mut().run_action(&ctx, id);
            harness.run();
        }
        assert_eq!((harness.state().mailbox().messages().len(), harness.state().mailbox().selected()), (rows, picked), "{percent}");
        harness.ctx.memory_mut(|memory| memory.request_focus(super::compose::text_id()));
        harness.run();
        let mut walked = Vec::new();
        for _ in 0..120 {
            harness.key_press(Key::Tab);
            harness.run();
            if let Some(label) =
                harness.root().children_recursive().find(|node| node.accesskit_node().is_focused()).and_then(|node| node.accesskit_node().label())
            {
                walked.push(label);
            }
        }
        for list in ["Done all", "Review", "Show done", "Grouped", "Cards"] {
            assert!(!walked.iter().any(|label| label == list), "{percent}: {list} in {walked:?}");
        }
        // Ctrl F would go to the list's search field: hidden, it runs nothing, and the keys stay in the composer.
        harness.ctx.memory_mut(|memory| memory.request_focus(super::compose::text_id()));
        harness.run();
        let selected = harness.state().mailbox().selected();
        harness.key_press_modifiers(Modifiers::COMMAND, Key::F);
        harness.run();
        assert!(super::compose::has_keys(&harness.ctx), "{percent}: Ctrl F left the composer");
        assert_eq!(harness.state().mailbox().selected(), selected);
    }
}

#[test]
fn closing_the_composer_brings_the_list_back_with_its_place_and_its_pick() {
    // 800, the least the window is laid out at (a narrower one is clipped).
    let mut harness = window(800.0, 100);
    harness.query_all_by_label_contains("Draft logo for the bakery").next().unwrap().click();
    harness.run();
    // One pane: the message opened in the list's place; Esc brings the list back in front, the row picked.
    harness.key_press(Key::Escape);
    harness.run();
    let row = |harness: &Harness<'static, InboxApp>| harness.query_all_by_label_contains("Your bike is due").next().map(|node| node.rect());
    let list = search_box(&harness).unwrap().center() + vec2(0.0, 300.0);
    harness.event(egui::Event::PointerMoved(list));
    harness.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: vec2(0.0, -60.0),
        modifiers: Modifiers::NONE,
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Wheel,
    });
    harness.run();
    let (picked, before) = (harness.state().mailbox().selected(), row(&harness));
    assert!(picked.is_some() && before.is_some());
    crate::draw::tests::new_message(&mut harness);
    harness.run();
    assert!(search_box(&harness).is_none(), "hidden while the composer is docked");
    harness.get_by_label("Close; the draft is kept").click();
    harness.run();
    assert_eq!(harness.state().mailbox().selected(), picked, "the pick kept");
    assert_eq!(row(&harness), before, "the list's scroll place kept");
}

#[test]
fn the_keyboard_in_the_list_when_it_goes_moves_to_the_composer() {
    let mut harness = composing(1100.0, 100);
    // The keyboard on a row of the list (the search is the command line's now, which stays).
    harness.query_all_by_role(Role::ListBoxOption).next().unwrap().focus();
    harness.run();
    assert!(super::list::focused_row(&harness.ctx).is_some(), "a row of the list has the keyboard");
    harness.set_size(vec2(720.0, 800.0));
    harness.run();
    assert!(search_box(&harness).is_none());
    assert!(super::compose::has_keys(&harness.ctx), "the keys went with the list");
}

#[test]
fn a_list_float_open_when_the_list_goes_closes_and_does_not_come_back() {
    let mut harness = composing(1100.0, 100);
    let id = harness.state().mailbox().messages().iter().find(|row| row.subject == "Allotment rota for November").unwrap().id;
    let menu = super::list::group_menu_id(id);
    let row = harness.query_all_by_label_contains("Allotment rota for November").next().unwrap().rect().center();
    harness.event(egui::Event::PointerMoved(row));
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton { pos: row, button: egui::PointerButton::Secondary, pressed, modifiers: Modifiers::NONE });
        harness.step();
    }
    harness.run();
    assert!(crate::component::popup::is_open(&harness.ctx, menu), "the group menu is open over the row");
    // Into the menu with the keyboard.
    harness.key_press(Key::ArrowDown);
    harness.run();
    harness.set_size(vec2(720.0, 800.0));
    harness.run();
    assert!(search_box(&harness).is_none());
    assert!(!crate::component::popup::is_open(&harness.ctx, menu), "closed with the list");
    assert!(super::compose::has_keys(&harness.ctx), "the keyboard went to the composer");
    harness.set_size(vec2(1100.0, 800.0));
    harness.run();
    assert!(search_box(&harness).is_some());
    assert!(!crate::component::popup::is_open(&harness.ctx, menu) && !crate::component::popup::any_open(&harness.ctx), "not back with the list");
}

#[test]
fn where_the_list_stands_at_its_least_a_group_s_head_hovered_stands_whole() {
    // At 130 % the head is wider than the list's 440: the list is never hidden for that (the cap).
    for percent in [100, 110] {
        // One group whose head is wider than the list's own head: it sets the list's least width.
        let grouped = |width: f32| {
            let mut harness = window(width, percent);
            let ids = harness.state().mailbox().messages().iter().map(|row| row.id).collect();
            let group = crate::mailbox::Group { icon: crate::iconset::IconId::Inbox, name: "Bakery orders".to_owned(), messages: ids };
            harness.state_mut().mailbox_mut().set_showcase(crate::mailbox::Showcase { screener: Vec::new(), groups: vec![group] });
            crate::draw::tests::new_message(&mut harness);
            harness.run();
            harness
        };
        // The first width, 2 points apart, at which the list stands beside the composer: there it is at its least.
        let mut harness = (980..1600).step_by(2).map(|width| grouped(width as f32)).find(|harness| search_box(harness).is_some()).unwrap();
        // Its head at its widest: every row shown ("11 of 11 · show fewer").
        harness.get_by_label("show all").click();
        harness.run();
        let heads: Vec<Rect> = harness.query_all_by_role(Role::Heading).map(|node| node.rect()).collect();
        assert!(!heads.is_empty(), "{percent}: the group's head");
        for head in heads {
            harness.event(egui::Event::PointerMoved(head.center()));
            harness.run();
            let inside = Rect::from_min_max(head.min, egui::pos2(head.right() - 1.0, head.bottom()));
            let words: Vec<String> = texts(&harness).into_iter().filter(|(rect, _)| inside.contains(rect.center())).map(|(_, text)| text).collect();
            assert!(words.iter().any(|text| text == "done all") && words.iter().any(|text| text == "show fewer"), "{percent}: {words:?}");
            assert!(!words.iter().any(|text| text.contains('…')), "{percent}: cut, {words:?}");
            assert_eq!(overdrawn(&harness, inside), Vec::<String>::new(), "{percent}: a group's head");
        }
    }
}

/// The texts painted in the last frame inside `rect`.
fn texts_in(harness: &Harness<'static, InboxApp>, rect: Rect) -> Vec<String> {
    texts(harness).into_iter().filter(|(at, _)| rect.contains(at.center())).map(|(_, text)| text).collect()
}

/// Opens the sample's "Draft logo for the bakery" with a press.
fn open_message(harness: &mut Harness<'static, InboxApp>) {
    harness.query_all_by_label_contains("Draft logo for the bakery").next().unwrap().click();
    harness.run();
}

/// Where the command field starts.
fn harness_field_left(harness: &Harness<'static, InboxApp>) -> f32 {
    harness.ctx.read_response(crate::command::field_id()).unwrap().rect.left()
}

/// Whether some widget has the keyboard.
fn keyboard_somewhere(harness: &Harness<'static, InboxApp>) -> bool {
    harness.ctx.memory(|memory| memory.focused()).is_some()
}

/// The widths the window is dragged through: across 1280, across 980 and back.
const DRAG: [f32; 4] = [1300.0, 1100.0, 900.0, 1300.0];

#[test]
fn under_1280_the_index_is_56_and_the_command_field_takes_its_room() {
    for (width, wide) in [(1440.0, true), (1280.0, true), (1279.0, false), (1100.0, false), (980.0, false), (800.0, false)] {
        let harness = window(width, 100);
        let regions = super::shell::laid(&harness.ctx).unwrap();
        assert_eq!(regions.index.width(), if wide { 232.0 } else { 56.0 }, "{width}: the index");
        let field = harness.ctx.read_response(crate::command::field_id()).unwrap().rect;
        let wide_field = harness_field_left(&harness) > 200.0;
        assert_eq!(wide_field, wide, "{width}: the field {field:?} beside the index {:?}", regions.index);
        // Narrow, the index names nothing: its names come as tooltips.
        let named = texts_in(&harness, regions.index).into_iter().any(|text| text == "Inbox" || text == "Calendar");
        assert_eq!(named, wide, "{width}: the index's names");
    }
}

#[test]
fn the_status_line_keeps_three_facts_and_keys_under_1280_and_two_under_980() {
    // Facts and keys counted at 1440 first: the rule keeps the first facts and the last keys.
    let count = |width: f32| {
        let mut harness = window(width, 100);
        // The design's four facts: synced, two accounts, one in the outbox, the next event.
        let now = harness.ctx.input(|input| input.time);
        let app = harness.state_mut();
        let stamp = (app.mailbox.revision(), app.mailbox.writes(), now);
        let next = Some(("15:00".to_owned(), "Design review".to_owned(), 120));
        let facts =
            super::status::Facts { sync: super::status::Sync::Synced(Some("14:02".to_owned())), accounts: 2, outbox: 1, next, ..Default::default() };
        app.facts = (facts, Some(stamp));
        harness.run();
        let line = super::shell::laid(&harness.ctx).unwrap().status;
        let said = texts_in(&harness, line);
        let facts = said.iter().filter(|text| *text == "·").count() + 1;
        let keys: Vec<String> = ["next", "previous", "done", "snooze", "write", "commands", "list", "open"]
            .iter()
            .filter(|word| harness.query_all_by_label_contains(word).any(|node| line.contains_rect(node.rect())))
            .map(|word| (*word).to_owned())
            .collect();
        (facts, keys)
    };
    let (facts, keys) = count(1440.0);
    assert!(facts == 4 && keys.len() >= 3, "the line at 1440: {facts} facts, {keys:?}");
    let (narrow_facts, narrow_keys) = count(1100.0);
    assert_eq!(narrow_facts, facts.min(3), "1100: the facts");
    assert_eq!(narrow_keys.len(), 3, "1100: the keys {narrow_keys:?}");
    let (single_facts, single_keys) = count(900.0);
    assert_eq!(single_facts, facts.min(2), "900: the facts");
    assert_eq!(single_keys.len(), 2, "900: the keys {single_keys:?}");
    assert!(single_keys.contains(&"commands".to_owned()), "the last key stays: {single_keys:?}");
}

#[test]
fn under_1280_the_why_line_leaves_out_its_last_part_whole() {
    let line = |width: f32| {
        let mut harness = window(width, 100);
        open_message(&mut harness);
        let why = harness
            .query_all_by_role(Role::Group)
            .find(|node| node.accesskit_node().label().is_some_and(|label| label.starts_with("Why here")))
            .unwrap();
        why.accesskit_node().label().unwrap()
    };
    let (wide, narrow) = (line(1440.0), line(1100.0));
    let mut harness = window(1440.0, 100);
    open_message(&mut harness);
    let why = harness.state().reader().why().to_vec();
    assert!(why.len() > 1, "the sample's message has more than one reason: {why:?}");
    let (last, rest) = why.split_last().unwrap();
    assert!(wide.contains(last.as_str()) && !narrow.contains(last.as_str()), "1100: {narrow} leaves out {last}");
    assert!(rest.iter().all(|reason| narrow.contains(reason.as_str())), "1100: {narrow} keeps {rest:?}");
}

#[test]
fn the_reader_has_482_at_980() {
    for width in [980.0, 1024.0, 1279.0] {
        let mut harness = window(width, 100);
        open_message(&mut harness);
        let list = harness.state().list_rect.unwrap();
        let reader = super::reader_rect(&harness.ctx).unwrap();
        let body = super::shell::laid(&harness.ctx).unwrap().body;
        assert_eq!(list.width(), super::LIST_WIDTH + 1.0, "{width}: the list keeps its 440 and its line");
        // The test's window stands in a margin; the app's fills the window: its reader's pane is the design's
        // 980 − 56 − 441 − 1 = 482, less the window's outline, a hairline at each side.
        assert_eq!(body.right() - list.right(), body.width() - 441.0, "{width}: the reader's pane");
        let whole = super::shell::regions(
            vec2(width, 900.0),
            &crate::theme::Theme::new(crate::theme::Mode::Light, crate::theme::TextScale::from_percent(100).unwrap()),
            1.0,
        )
        .body;
        assert_eq!(whole.width() - 441.0, width - 56.0 - 441.0 - 1.0 - 2.0, "{width}: in the app's window");
        assert!(reader.left() >= list.right() && reader.right() <= body.right(), "{width}: the reader {reader:?} in its pane");
    }
    let mut harness = window(979.0, 100);
    open_message(&mut harness);
    assert!(search_box(&harness).is_none(), "979: one pane, the message in the list's place");
}

#[test]
fn under_980_the_composer_takes_the_list_s_place_and_beside_it_the_list_keeps_440() {
    for percent in [100, 130] {
        let harness = composing(980.0, percent);
        assert_eq!(harness.state().list_rect.map(|list| list.width()), Some(super::LIST_WIDTH + 1.0), "{percent}: 980");
        let composer = super::compose::drawn_rect(&harness.ctx).unwrap();
        assert!(composer.width() >= 400.0, "{percent}: the composer beside it {composer:?}");
        let harness = composing(979.0, percent);
        assert!(search_box(&harness).is_none(), "{percent}: 979, the composer in the list's place");
        assert!(harness.state().list_hidden, "{percent}: 979, nothing of the list acts");
    }
}

#[test]
fn under_1280_the_calendar_s_toolbar_drops_its_note_and_under_980_its_side_panel_swaps_with_the_week() {
    let calendar = |width: f32| {
        let mut harness = window(width, 100);
        harness.state_mut().show_calendar();
        harness.run();
        harness
    };
    assert!(calendar(1440.0).query_by_label_contains("week ").is_some(), "1440: the week's note");
    // Just under 1280 the toolbar has room for the note; the rule drops it there too.
    let harness = calendar(1270.0);
    assert!(harness.query_by_label_contains("week ").is_none(), "1270: no note");
    assert!(harness.query_by_label("Calendars").is_some() || harness.query_all_by_label_contains("calendar").count() > 0);
    let mut harness = calendar(900.0);
    assert!(harness.query_by_label("Close the details").is_none(), "nothing picked: the week");
    let first = harness.state().calendar().events().next().unwrap().key();
    harness.state_mut().calendar_mut().0.pick(Some(first));
    harness.run();
    let close = harness.get_by_label("Close the details");
    assert!(close.accesskit_node().is_focused(), "the details take the pane and the keyboard");
    // Widened out of one pane the details stand beside the week again and keep the keyboard.
    harness.set_size(vec2(1300.0, 800.0));
    harness.run();
    assert!(harness.get_by_label("Close the details").accesskit_node().is_focused(), "1300: the details keep the keyboard");
    harness.set_size(vec2(900.0, 800.0));
    harness.run();
    assert!(harness.get_by_label("Close the details").accesskit_node().is_focused(), "900 again: the details have the keyboard");
    harness.key_press(Key::Escape);
    harness.run();
    assert!(harness.query_by_label("Close the details").is_none(), "Esc gives the pane back to the week");
    assert!(harness.state().calendar().picked().is_none());
}

#[test]
fn a_short_window_keeps_the_floating_panel_inside_the_body() {
    for height in [400.0, 520.0, 640.0] {
        let mut harness = window(1100.0, 100);
        harness.set_size(vec2(1100.0, height));
        open_message(&mut harness);
        crate::draw::tests::new_message(&mut harness);
        harness.run();
        harness.state_mut().composer_mut().0.unwrap().set_floating(true);
        harness.run();
        let body = super::shell::laid(&harness.ctx).unwrap().body;
        let panel = super::compose::drawn_rect(&harness.ctx).unwrap();
        assert!(body.contains_rect(panel), "{height}: the panel {panel:?} in the body {body:?}");
        assert!(panel.height() >= 40.0, "{height}: at least its folded height, {panel:?}");
    }
}

#[test]
fn a_window_under_800_lays_out_as_800() {
    let least = window(800.0, 100);
    let at_least = super::shell::laid(&least.ctx).unwrap();
    for width in [799.0, 640.0, 320.0, 1.0] {
        let mut harness = window(width, 100);
        open_message(&mut harness);
        let regions = super::shell::laid(&harness.ctx).unwrap();
        assert_eq!(regions, at_least, "{width}: laid out as 800");
        for rect in [regions.command, regions.index, regions.body, regions.status] {
            assert!(rect.width() >= 0.0 && rect.height() >= 0.0, "{width}: {rect:?}");
        }
    }
}

#[test]
fn in_one_pane_enter_opens_in_the_list_s_place_and_the_back_arrow_and_esc_go_back() {
    let mut harness = window(900.0, 100);
    harness.query_all_by_role(Role::ListBoxOption).next().unwrap().focus();
    harness.run();
    harness.key_press(Key::Enter);
    harness.run();
    assert!(search_box(&harness).is_none() && super::reader_rect(&harness.ctx).is_some(), "Enter: the reader in the list's place");
    assert!(keyboard_somewhere(&harness), "Enter: the keyboard in the reader");
    let back = harness.get_by_role_and_label(Role::Button, "Back to the list").rect();
    let reader = super::reader_rect(&harness.ctx).unwrap();
    let tools: Vec<Rect> =
        harness.query_all_by_role(Role::Button).map(|node| node.rect()).filter(|rect| (rect.center().y - back.center().y).abs() < 2.0).collect();
    assert!(tools.iter().all(|tool| tool.left() >= back.left()), "the arrow at the left of the tool row {back:?} {reader:?} {tools:?}");
    harness.key_press(Key::Escape);
    harness.run();
    assert!(search_box(&harness).is_some(), "Esc: the list back");
    assert!(super::list::focused_row(&harness.ctx).is_some(), "on its row");
    harness.key_press(Key::Enter);
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Back to the list").click();
    harness.run();
    assert!(search_box(&harness).is_some(), "the arrow: the list back");
    // Every control of the one pane is reached by Tab and the keyboard is never on nothing.
    for _ in 0..40 {
        harness.key_press(Key::Tab);
        harness.run();
        assert!(keyboard_somewhere(&harness), "Tab left the keyboard on nothing");
    }
}

#[test]
fn dragging_across_both_breakpoints_keeps_a_docked_draft_and_its_keyboard() {
    let mut harness = composing(DRAG[0], 100);
    harness.get_by_role_and_label(Role::TextInput, "To").type_text("odile@example.org,");
    harness.run();
    harness.ctx.memory_mut(|memory| memory.request_focus(super::compose::text_id()));
    harness.run();
    harness.event(egui::Event::Text("Two directions.".to_owned()));
    harness.run();
    for width in DRAG {
        harness.set_size(vec2(width, 800.0));
        harness.run();
        let composer = harness.state_mut().composer_mut().0.unwrap();
        assert!(composer.text().contains("Two directions."), "{width}: the text kept");
        assert_eq!(harness.ctx.memory(|memory| memory.focused()), Some(super::compose::text_id()), "{width}: the keyboard stays in the text");
        assert!(super::compose::drawn_rect(&harness.ctx).is_some(), "{width}: the composer shows");
        assert_eq!(search_box(&harness).is_some(), width >= 980.0, "{width}: the list beside it or behind it");
    }
}

#[test]
fn dragging_across_both_breakpoints_keeps_a_half_filled_account_form() {
    let mut harness = window(DRAG[0], 100);
    harness.state_mut().onboarding_mut().0.open();
    harness.state_mut().onboarding_mut().0.form_mut().address = "odile@example.org".to_owned();
    harness.state_mut().open_accounts();
    harness.run();
    harness.query_all_by_role(Role::TextInput).find(|node| node.value().is_some_and(|value| value == "odile@example.org")).unwrap().focus();
    harness.run();
    for width in DRAG {
        harness.set_size(vec2(width, 800.0));
        harness.run();
        assert!(harness.state().settings().is_open(), "{width}: the settings stay");
        assert_eq!(harness.state().onboarding().form().address, "odile@example.org", "{width}: the form kept");
        assert!(keyboard_somewhere(&harness), "{width}: the keyboard on something");
        assert!(
            harness.query_all_by_role(Role::TextInput).any(|node| node.value().is_some_and(|value| value == "odile@example.org")),
            "{width}: shown"
        );
    }
    // A message was open before the settings came: crossing 980 does not give its row the keyboard behind them.
    let mut harness = window(DRAG[0], 100);
    open_message(&mut harness);
    harness.state_mut().open_accounts();
    harness.run();
    harness.get_by_label("Accounts, settings page").focus();
    harness.run();
    let on = harness.ctx.memory(|memory| memory.focused());
    assert!(on.is_some(), "the settings' nav has the keyboard");
    for width in DRAG {
        harness.set_size(vec2(width, 800.0));
        harness.run();
        assert_eq!(harness.ctx.memory(|memory| memory.focused()), on, "{width}: the settings keep the keyboard");
    }
}

#[test]
fn dragging_across_both_breakpoints_closes_a_float_over_the_list_when_the_list_leaves() {
    let mut harness = window(DRAG[0], 100);
    open_message(&mut harness);
    // The snooze picker of a row of the list beside the reader, its Snooze pressed, and Tab into its rows.
    let row = harness.query_all_by_label_contains("Allotment rota for November").next().unwrap().rect();
    harness.hover_at(row.center());
    harness.run();
    let snooze = harness.query_all_by_role_and_label(Role::Button, "Snooze").map(|node| node.rect()).find(|rect| row.contains_rect(*rect)).unwrap();
    harness.hover_at(snooze.center());
    harness.run();
    harness.query_all_by_role_and_label(Role::Button, "Snooze").find(|node| node.rect() == snooze).unwrap().click();
    harness.run();
    harness.key_press(Key::Tab);
    harness.run();
    assert!(harness.query_by_label("Snooze until").is_some(), "the picker is open over the list");
    assert!(keyboard_somewhere(&harness), "the picker has the keyboard");
    for width in DRAG.into_iter().skip(1) {
        harness.set_size(vec2(width, 800.0));
        harness.run();
        if search_box(&harness).is_none() {
            assert!(harness.query_by_label("Snooze until").is_none(), "{width}: the list left, its picker with it");
        }
        assert!(keyboard_somewhere(&harness), "{width}: the keyboard on something");
    }
    assert!(harness.query_by_label("Snooze until").is_none(), "back at 1300 the picker does not come back");
}

#[test]
fn dragging_across_both_breakpoints_keeps_the_reader_at_full_width() {
    let mut harness = window(DRAG[0], 100);
    open_message(&mut harness);
    let ctx = harness.ctx.clone();
    harness.state_mut().run_action(&ctx, keymap::FULL_WIDTH);
    harness.run();
    let open = harness.state().reader().open_id();
    assert!(open.is_some() && search_box(&harness).is_none(), "full width: the list away");
    let reader = super::reader_rect(&harness.ctx).unwrap();
    harness.query_all_by_role(Role::Button).find(|node| reader.contains_rect(node.rect())).unwrap().focus();
    harness.run();
    assert!(keyboard_somewhere(&harness), "a control of the reader has the keyboard");
    for width in DRAG {
        harness.set_size(vec2(width, 800.0));
        harness.run();
        assert_eq!(harness.state().reader().open_id(), open, "{width}: the message stays open");
        assert!(search_box(&harness).is_none(), "{width}: still over the whole width");
        let body = super::shell::laid(&harness.ctx).unwrap().body;
        let reader = super::reader_rect(&harness.ctx).unwrap();
        assert!(reader.width() >= body.width() - 2.0, "{width}: the reader {reader:?} over the body {body:?}");
        assert!(keyboard_somewhere(&harness), "{width}: the keyboard on something");
    }
    harness.key_press(Key::Escape);
    harness.run();
    assert!(search_box(&harness).is_some(), "Esc leaves full width: the list beside the reader again");
}
