// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The composer in the reader's place in a narrow window: the list beside it while its heads fit, else the composer in
//! its place; nothing drawn over anything else in the list's head, a group's head or the composer's head, From and To
//! lines.

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
