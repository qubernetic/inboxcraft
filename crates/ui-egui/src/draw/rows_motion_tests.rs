// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A row leaving the list (M5 Done, M6 Snooze) and a row arriving (M4) at the three levels, frame by frame, frames 10 ms
//! apart: the act in the frame of its key, the key frames of the design's table, a key during a movement, and rest.

use egui::accesskit::Role;
use egui::{Key, Modifiers, Rect, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use inboxcraft_time::{FakeClock, Timestamp};

use crate::mailbox::{Act, InboxView, Mailbox};
use crate::motion::Level;
use crate::scramble::DOT;
use crate::{InboxApp, sample};

const ODILE: &str = "Odile Brannock, Allotment rota for November, 08:35, unread";

/// The window at the mail's simple list at `level`, at rest, frames 10 ms apart.
pub(super) fn window(level: Level) -> Harness<'static, InboxApp> {
    let clock = || Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()));
    let mut mailbox = Mailbox::new(sample::session(clock()).unwrap(), clock());
    mailbox.set_inbox_view(InboxView::List);
    let app = InboxApp::at_mail(mailbox);
    let mut harness = crate::component::tests::builder()
        .with_size(vec2(1300.0, 820.0))
        .with_step_dt(0.01)
        .with_max_steps(100)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.state_mut().set_motion_level(level);
    harness.run();
    harness
}

/// A key pressed as one event: the frame of the key is the last drawn.
pub(super) fn press(harness: &mut Harness<'static, InboxApp>, key: Key) {
    harness.event(egui::Event::Key { key, pressed: true, modifiers: Modifiers::NONE, repeat: false, physical_key: None });
    harness.step();
}

/// The texts painted with their middle inside `rect`, icons left out (they never change).
pub(super) fn texts_in(harness: &Harness<'static, InboxApp>, rect: Rect) -> Vec<String> {
    let icons = crate::theme::icon_font(1.0).family;
    let mut out = Vec::new();
    for clipped in &harness.output().shapes {
        let mut add = |shape: &egui::Shape| {
            if let egui::Shape::Text(text) = shape
                && !text.galley.job.sections.iter().any(|section| section.format.font_id.family == icons)
                && rect.contains(text.galley.rect.translate(text.pos.to_vec2()).center())
            {
                out.push(text.galley.job.text.clone());
            }
        };
        match &clipped.shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(&mut add),
            shape => add(shape),
        }
    }
    out
}

/// Odile's row selected, at rest: its rect, and the id and top of the row under it.
fn odile_selected(harness: &mut Harness<'static, InboxApp>) -> (Rect, u64, f32) {
    harness.get_by_label(ODILE);
    let mailbox = harness.state().mailbox();
    let at = mailbox.messages().iter().position(|row| row.subject == "Allotment rota for November").unwrap();
    let next = mailbox.messages()[at + 1].id;
    let id = mailbox.messages()[at].id;
    harness.state_mut().mailbox_mut().select(Some(id));
    harness.run();
    // Beside the reader now, and read.
    let mailbox = harness.state().mailbox();
    let row = mailbox.messages().iter().find(|row| row.id == id).unwrap();
    let rect = harness.get_by_label(&mailbox.spoken(row)).rect();
    let top = top(harness, next);
    (rect, next, top)
}

fn gone(harness: &Harness<'static, InboxApp>) -> bool {
    harness.state().mailbox().messages().iter().all(|row| row.subject != "Allotment rota for November")
}

/// The top of message `id`'s row, as it is named now (selected, it reads as read).
pub(super) fn top(harness: &Harness<'static, InboxApp>, id: u64) -> f32 {
    let mailbox = harness.state().mailbox();
    let row = mailbox.messages().iter().find(|row| row.id == id).unwrap();
    harness.get_by_label(&mailbox.spoken(row)).rect().top()
}

/// Whether `text` is all dots (and white space).
fn dots(text: &str) -> bool {
    text.chars().all(|ch| ch == DOT || ch.is_whitespace())
}

/// The undo line's bar: an 80 wide, 2 high track painted in the status line.
fn bar(harness: &Harness<'static, InboxApp>) -> bool {
    let Some(regions) = crate::draw::shell::laid(&harness.ctx) else { return false };
    let mut found = false;
    for clipped in &harness.output().shapes {
        let mut look = |shape: &egui::Shape| {
            if let egui::Shape::Rect(rect) = shape
                && (rect.rect.width() - 80.0).abs() < 0.5
                && (rect.rect.height() - 2.0).abs() < 0.5
                && regions.status.contains(rect.rect.center())
            {
                found = true;
            }
        };
        match &clipped.shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(&mut look),
            shape => look(shape),
        }
    }
    found
}

/// Lets the undo go and steps on: the scene rests.
fn rests(harness: &mut Harness<'static, InboxApp>) -> bool {
    harness.state_mut().mailbox_mut().forget_undo();
    harness.run_steps(40);
    crate::component::tests::rests(harness, 4)
}

#[test]
fn at_off_done_is_in_the_frame_of_its_key_and_the_undo_line_has_no_bar() {
    let mut harness = window(Level::Off);
    let (rect, next, below) = odile_selected(&mut harness);
    press(&mut harness, Key::D);
    assert!(gone(&harness), "the owner never waits: done in the frame of the key");
    assert!(harness.state().mailbox().can_undo(), "and its undo with it");
    assert_eq!(top(&harness, next), rect.top(), "the row under it is in its place in that frame: {below} -> {}", rect.top());
    assert!(texts_in(&harness, rect).iter().all(|text| !text.contains("Odile")), "the row is gone");
    assert!(!bar(&harness), "at off the undo line has no bar");
    assert!(rests(&mut harness));
}

#[test]
fn at_full_done_dissolves_the_words_in_125_ms_then_the_row_closes_in_180() {
    let mut harness = window(Level::Full);
    let (rect, next, below) = odile_selected(&mut harness);
    press(&mut harness, Key::D);
    assert!(gone(&harness), "0 ms: done at once");
    assert!(bar(&harness), "the undo line with its bar");
    assert!(texts_in(&harness, rect).iter().any(|text| text == "Odile Brannock"), "0 ms: the row whole: {:?}", texts_in(&harness, rect));
    harness.run_steps(6);
    let seen = texts_in(&harness, rect);
    assert!(
        seen.iter().any(|text| text.starts_with("Odi") && text.contains(DOT) && !dots(text)),
        "60 ms: the words go to dots from the right: {seen:?}"
    );
    assert_eq!(top(&harness, next), below, "60 ms: the row keeps its height");
    harness.run_steps(7);
    let seen = texts_in(&harness, rect);
    assert!(!seen.is_empty() && seen.iter().all(|text| dots(text)), "130 ms: every character a dot: {seen:?}");
    harness.run_steps(2);
    let closing = top(&harness, next);
    assert!(closing < below - 14.0 && closing > below - 56.0, "150 ms: the row closes on the curve: {below} -> {closing}");
    harness.run_steps(16);
    assert_eq!(top(&harness, next), below - 56.0, "310 ms: closed");
    assert!(rests(&mut harness));
}

#[test]
fn at_reduced_done_closes_the_row_in_180_ms_its_words_intact() {
    let mut harness = window(Level::Reduced);
    let (rect, next, below) = odile_selected(&mut harness);
    press(&mut harness, Key::D);
    assert!(gone(&harness), "0 ms: done at once");
    assert!(bar(&harness), "the undo line with its bar");
    harness.run_steps(3);
    let closing = top(&harness, next);
    assert!(closing < below - 14.0 && closing > below - 56.0, "30 ms: the row closes on the curve: {below} -> {closing}");
    assert!(texts_in(&harness, rect).iter().all(|text| !text.contains(DOT)), "its words intact");
    harness.run_steps(16);
    assert_eq!(top(&harness, next), below - 56.0, "190 ms: closed");
    assert!(rests(&mut harness));
}

#[test]
fn a_key_while_a_row_leaves_acts_in_its_frame_and_the_row_is_gone() {
    for level in [Level::Full, Level::Reduced] {
        let mut harness = window(level);
        let (_, next, below) = odile_selected(&mut harness);
        press(&mut harness, Key::D);
        harness.run_steps(3);
        let selected = harness.state().mailbox().selected();
        press(&mut harness, Key::J);
        assert_ne!(harness.state().mailbox().selected(), selected, "{level:?}: J moved on in the frame of its key");
        let (left, at) = (harness.state().mailbox().left_row().is_some(), top(&harness, next));
        assert!(!left || at == below - 56.0, "{level:?}: the row is gone: {left} {at} {below}");
        harness.step();
        assert!(harness.state().mailbox().left_row().is_none(), "{level:?}: let go");
        assert_eq!(top(&harness, next), below - 56.0, "{level:?}: its gap closed at once");
    }
}

/// Odile's row snoozed by the owner's act, in the frame of the act.
fn snooze_odile(harness: &mut Harness<'static, InboxApp>) {
    let id = harness.state().mailbox().selected().unwrap();
    let until = Timestamp::from_unix(1_793_869_200 + 86_400, 0).unwrap();
    harness.state_mut().mailbox_mut().start(id, Act::Snooze(until));
    harness.step();
}

/// The left edge of the painted texts that start with `start` inside `rect`, and the alpha of the first.
fn left_of(harness: &Harness<'static, InboxApp>, rect: Rect, start: &str) -> Option<(f32, u8)> {
    for clipped in &harness.output().shapes {
        let mut found = None;
        let mut look = |shape: &egui::Shape| {
            if let egui::Shape::Text(text) = shape
                && text.galley.job.text.starts_with(start)
                && rect.contains(text.galley.rect.translate(text.pos.to_vec2()).center())
            {
                // An alpha of 0 to 255: the cast keeps it.
                found = Some((text.pos.x, (text.opacity_factor * f32::from(text.fallback_color.a())) as u8));
            }
        };
        match &clipped.shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(&mut look),
            shape => look(shape),
        }
        if found.is_some() {
            return found;
        }
    }
    None
}

#[test]
fn at_full_a_snoozed_row_slides_24_right_and_fades_in_300_ms_then_closes_in_180() {
    let mut harness = window(Level::Full);
    let (rect, next, below) = odile_selected(&mut harness);
    let (x, alpha) = left_of(&harness, rect, "Odile").unwrap();
    snooze_odile(&mut harness);
    harness.run_steps(4);
    let (moved, faded) = left_of(&harness, rect.expand2(vec2(40.0, 0.0)), "Odile").unwrap();
    assert!(moved - x > 8.0 && moved - x < 14.0, "40 ms: about 11 px to the right: {}", moved - x);
    assert!(faded < alpha && f32::from(faded) > f32::from(alpha) * 0.8, "40 ms: about 87 % there: {faded} of {alpha}");
    assert_eq!(top(&harness, next), below, "the place stays open");
    harness.run_steps(8);
    let (moved, _) = left_of(&harness, rect.expand2(vec2(40.0, 0.0)), "Odile").unwrap();
    assert!(moved - x > 18.0 && moved - x <= 24.0, "120 ms: about 21 px: {}", moved - x);
    harness.run_steps(17);
    assert_eq!(top(&harness, next), below, "290 ms: the row all but gone, its place still open");
    harness.run_steps(19);
    assert_eq!(top(&harness, next), below - 56.0, "480 ms: the rows under it moved up");
    assert!(rests(&mut harness));
}

#[test]
fn at_reduced_a_snoozed_row_fades_in_125_ms_then_closes_in_180() {
    let mut harness = window(Level::Reduced);
    let (rect, next, below) = odile_selected(&mut harness);
    let (x, alpha) = left_of(&harness, rect, "Odile").unwrap();
    snooze_odile(&mut harness);
    harness.run_steps(6);
    let (moved, faded) = left_of(&harness, rect, "Odile").unwrap();
    assert_eq!(moved, x, "it does not slide");
    assert!(f32::from(faded) < f32::from(alpha) * 0.6 && f32::from(faded) > f32::from(alpha) * 0.4, "60 ms: half way out: {faded} of {alpha}");
    harness.run_steps(6);
    assert_eq!(top(&harness, next), below, "120 ms: all but gone, its place still open");
    harness.run_steps(19);
    assert_eq!(top(&harness, next), below - 56.0, "310 ms: the rows under it moved up");
    assert!(rests(&mut harness));
}

#[test]
fn at_off_a_snoozed_row_is_gone_at_once() {
    let mut harness = window(Level::Off);
    let (rect, next, _) = odile_selected(&mut harness);
    snooze_odile(&mut harness);
    harness.step();
    assert_eq!(top(&harness, next), rect.top(), "gone, the rows under it up");
    assert!(harness.query_by_role_and_label(Role::Button, "undo").is_some(), "the undo line shows");
    assert!(rests(&mut harness));
}

const ARRIVED: &str = "Teodor Vasquez";

/// A new unread message from Teodor Vasquez comes in at the top of the list, as a sync brings it: its id.
fn arrive(harness: &mut Harness<'static, InboxApp>) -> u64 {
    let mut rows = harness.state().mailbox().messages().to_vec();
    let mut new = rows[0].clone();
    new.id = 9_999;
    new.sender = ARRIVED.to_owned();
    new.subject = "Draft logo for the bakery".to_owned();
    new.seen = false;
    rows.insert(0, new);
    harness.state_mut().mailbox_mut().set_rows(rows);
    harness.step();
    9_999
}

/// The rect of message `id`'s row.
fn row_of(harness: &Harness<'static, InboxApp>, id: u64) -> Rect {
    let mailbox = harness.state().mailbox();
    let row = mailbox.messages().iter().find(|row| row.id == id).unwrap();
    harness.get_by_label(&mailbox.spoken(row)).rect()
}

/// The text of the row's line one that is as long as the sender's name, and its alpha.
fn sender_in(harness: &Harness<'static, InboxApp>, rect: Rect) -> Option<(String, u8)> {
    let count = ARRIVED.chars().count();
    for clipped in &harness.output().shapes {
        let mut found = None;
        let mut look = |shape: &egui::Shape| {
            if let egui::Shape::Text(text) = shape
                && text.galley.job.text.chars().count() == count
                && rect.contains(text.galley.rect.translate(text.pos.to_vec2()).center())
            {
                // An alpha of 0 to 255: the cast keeps it.
                found = Some((text.galley.job.text.clone(), (text.opacity_factor * f32::from(text.fallback_color.a())) as u8));
            }
        };
        match &clipped.shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(&mut look),
            shape => look(shape),
        }
        if found.is_some() {
            return found;
        }
    }
    None
}

/// Whether the unread square (6 × 6) is painted in `rect`, and its alpha.
fn square_in(harness: &Harness<'static, InboxApp>, rect: Rect) -> Option<u8> {
    let mut found = None;
    for clipped in &harness.output().shapes {
        let mut look = |shape: &egui::Shape| {
            if let egui::Shape::Rect(square) = shape
                && (square.rect.width() - 6.0).abs() < 0.1
                && (square.rect.height() - 6.0).abs() < 0.1
                && rect.contains(square.rect.center())
                && square.fill.a() > 0
            {
                found = Some(square.fill.a());
            }
        };
        match &clipped.shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(&mut look),
            shape => look(shape),
        }
    }
    found
}

/// How many of the sender's characters read, from the left.
fn reads(seen: &str) -> usize {
    seen.chars().zip(ARRIVED.chars()).take_while(|(a, b)| a == b).count()
}

#[test]
fn at_off_a_new_row_is_there_in_the_frame_it_comes_in() {
    let mut harness = window(Level::Off);
    let count = harness.state().mailbox().list_count();
    let id = arrive(&mut harness);
    let rect = row_of(&harness, id);
    assert_eq!(sender_in(&harness, rect).map(|(text, _)| text), Some(ARRIVED.to_owned()), "the row reads at once");
    assert!(square_in(&harness, rect).is_some(), "unread, its square on");
    assert_ne!(harness.state().mailbox().list_count(), count, "the count changed with it");
    assert!(crate::component::tests::rests(&mut harness, 2));
}

#[test]
fn at_full_a_new_row_decodes_in_over_300_ms_and_its_square_blinks_once() {
    let mut harness = window(Level::Full);
    let id = arrive(&mut harness);
    let rect = row_of(&harness, id);
    let (first, _) = sender_in(&harness, rect).unwrap();
    assert_eq!(reads(&first), 0, "0 ms: in its place, every character a glyph: {first}");
    harness.run_steps(12);
    let (half, _) = sender_in(&harness, rect).unwrap();
    assert!((3..=7).contains(&reads(&half)), "120 ms: resolving from the left: {half}");
    harness.run_steps(12);
    let (most, _) = sender_in(&harness, rect).unwrap();
    assert!((10..14).contains(&reads(&most)), "240 ms: four fifths read: {most}");
    assert!(square_in(&harness, rect).is_some(), "the square on while it decodes");
    harness.run_steps(7);
    assert_eq!(sender_in(&harness, rect).map(|(text, _)| text), Some(ARRIVED.to_owned()), "310 ms: it reads");
    assert!(square_in(&harness, rect).is_none(), "300 to 425 ms: the square off");
    harness.run_steps(12);
    assert!(square_in(&harness, rect).is_some(), "430 ms: on again, for good");
    assert!(crate::component::tests::rests(&mut harness, 4), "one blink, then rest");
}

#[test]
fn at_reduced_a_new_row_fades_in_over_125_ms_and_its_square_blinks_once() {
    let mut harness = window(Level::Reduced);
    let id = arrive(&mut harness);
    let rect = row_of(&harness, id);
    let (_, rest) = {
        let mut still = window(Level::Off);
        let id = arrive(&mut still);
        sender_in(&still, row_of(&still, id)).unwrap()
    };
    harness.run_steps(6);
    let (words, alpha) = sender_in(&harness, rect).unwrap();
    assert_eq!(words, ARRIVED, "60 ms: its words its own from the first frame");
    assert!(f32::from(alpha) > f32::from(rest) * 0.35 && f32::from(alpha) < f32::from(rest) * 0.65, "60 ms: half way in: {alpha} of {rest}");
    harness.run_steps(7);
    assert_eq!(sender_in(&harness, rect).map(|(_, alpha)| alpha), Some(rest), "130 ms: there");
    assert!(square_in(&harness, rect).is_none(), "125 to 250 ms: the square off");
    harness.run_steps(13);
    assert!(square_in(&harness, rect).is_some(), "260 ms: on again, for good");
    assert!(crate::component::tests::rests(&mut harness, 4));
}

#[test]
fn a_key_while_a_new_row_comes_in_acts_in_its_frame_and_the_row_is_there() {
    for level in [Level::Full, Level::Reduced] {
        let mut harness = window(level);
        let id = arrive(&mut harness);
        let rect = row_of(&harness, id);
        harness.run_steps(4);
        let selected = harness.state().mailbox().selected();
        press(&mut harness, Key::J);
        assert_ne!(harness.state().mailbox().selected(), selected, "{level:?}: J acted in the frame of its key");
        let (words, _) = sender_in(&harness, rect).unwrap();
        assert_eq!(words, ARRIVED, "{level:?}: the row reads in that frame");
        assert!(square_in(&harness, rect).is_some(), "{level:?}: its square on");
        harness.run_steps(50);
        assert!(square_in(&harness, rect).is_some(), "{level:?}: and it stays on");
        assert!(crate::component::tests::rests(&mut harness, 4), "{level:?}: and nothing moves after");
    }
}
