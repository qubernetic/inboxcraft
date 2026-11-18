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
    let mut app = InboxApp::new(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    app.set_text_scale(crate::theme::TextScale::from_percent(percent).unwrap());
    let mut harness = Harness::builder().with_size(vec2(width, 800.0)).with_max_steps(12).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

/// [`window`] with New message open in the reader's place.
fn composing(width: f32, percent: u16) -> Harness<'static, InboxApp> {
    let mut harness = window(width, percent);
    harness.get_by_role_and_label(Role::Button, "New message").click();
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

/// The list's search field, while the list is drawn.
fn search_box(harness: &Harness<'static, InboxApp>) -> Option<Rect> {
    harness.query_all_by_role(Role::TextInput).map(|node| node.rect()).find(|rect| rect.top() < 100.0)
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
            // Right of the folder pane: the list's head and its cards' heads, when the list is drawn.
            let folders = harness.get_by_label("Accounts").rect().right() + 1.0;
            if let Some(search) = search_box(&harness) {
                assert_eq!(overdrawn(&harness, band(folders, 0.0, search.bottom() + 24.0)), Vec::<String>::new(), "{at}: the list's head");
                for done in harness.query_all_by_label("Done all").map(|node| node.rect()) {
                    assert_eq!(overdrawn(&harness, band(folders, done.top(), done.bottom())), Vec::<String>::new(), "{at}: a card's head");
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
        harness.get_by_role_and_label(Role::Button, "New message").click();
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
            assert_ne!(harness.ctx.memory(|memory| memory.focused()), Some(super::list::search_id()), "{percent}: Tab reached the search field");
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
    let mut harness = window(720.0, 100);
    harness.query_all_by_label_contains("Draft logo for the bakery").next().unwrap().click();
    harness.run();
    let row = |harness: &Harness<'static, InboxApp>| harness.query_all_by_label_contains("Your bike is due").next().map(|node| node.rect());
    let list = search_box(&harness).unwrap().center() + vec2(-100.0, 300.0);
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
    harness.get_by_role_and_label(Role::Button, "New message").click();
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
    harness.ctx.memory_mut(|memory| memory.request_focus(super::list::search_id()));
    harness.run();
    assert_eq!(harness.ctx.memory(|memory| memory.focused()), Some(super::list::search_id()));
    harness.set_size(vec2(720.0, 800.0));
    harness.run();
    assert!(search_box(&harness).is_none());
    assert!(super::compose::has_keys(&harness.ctx), "the keys went with the list");
}

/// Drags the folder pane's edge, 12 right of its Accounts box (its card's pad), 800 to the right.
fn widen_folders(harness: &mut Harness<'static, InboxApp>) {
    let edge = harness.get_by_label("Accounts").rect().right() + 12.0;
    let mut at = egui::pos2(edge - 1.0, 400.0);
    harness.event(egui::Event::PointerMoved(at));
    harness.step();
    harness.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    harness.step();
    for _ in 0..16 {
        at.x += 50.0;
        harness.event(egui::Event::PointerMoved(at));
        harness.step();
    }
    harness.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    harness.run();
}

#[test]
fn a_folder_pane_dragged_wide_leaves_the_composer_its_send_whole() {
    for percent in [100, 130] {
        for width in [720.0, 800.0, 1100.0] {
            let mut harness = composing(width, percent);
            let accounts = harness.get_by_label("Accounts").rect().right();
            widen_folders(&mut harness);
            let at = format!("{percent} % {width}");
            if width >= 1100.0 {
                assert!(harness.get_by_label("Accounts").rect().right() > accounts + 40.0, "{at}: the pane was dragged wider");
            }
            let composer = super::compose::drawn_rect(&harness.ctx).unwrap();
            assert!(composer.right() <= width, "{at}: {composer:?} in the window");
            for label in ["Send", "Send later", "Discard the draft"] {
                let rect = harness.get_by_label(label).rect();
                assert!(composer.contains_rect(rect) && rect.right() <= width, "{at}: {label} {rect:?} in {composer:?}");
            }
            for text in texts(&harness) {
                assert!(text.0.right() <= width + 0.5, "{at}: {text:?} past the window");
            }
        }
    }
}
