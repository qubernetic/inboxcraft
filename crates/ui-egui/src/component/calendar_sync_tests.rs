// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The sync line on its own: the four frames' words, marks and heights in light and dark, a press returned only when
//! it may be taken, and a long, hostile account name kept on one line inside the 222 points.

use egui::accesskit::Role;
use egui::{Color32, Id, Rect, Shape, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::{Drawn, HIGH, HIGH_FAILED, calendar_sync};
use crate::calendar::sync::{SyncState, SyncView, Why};
use crate::calendar::{Day, Moment};
use crate::component::gallery::keymap;
use crate::component::tests::builder;
use crate::theme::{Mode, Palette, TextScale, Theme};

/// The line's place on the test's page: the side panel's foot, 222 wide.
fn place(view: &SyncView) -> Rect {
    Rect::from_min_size(pos2(20.0, 20.0), vec2(222.0, super::height(view)))
}

fn read() -> Option<Moment> {
    Day::new(2026, 11, 16).map(|day| Moment { day, minute: 14 * 60 + 2 })
}

/// A page with the line in `view`, `resting` or not; what the last frame's press returned is the harness's state.
fn harness(mode: Mode, view: SyncView, resting: bool) -> Harness<'static, bool> {
    let theme = Theme::new(mode, TextScale::default());
    let keymap = keymap();
    let mut harness = builder().with_size(vec2(300.0, 120.0)).build_ui_state(
        move |ui, pressed: &mut bool| {
            ui.painter().rect_filled(ui.max_rect(), 0.0, theme.palette.panel);
            let drawn = Drawn { id: Id::unique("sync-test"), resting, still: true };
            let rect = place(&view);
            *pressed |= ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| calendar_sync(ui, &theme, &keymap, &view, drawn)).inner;
        },
        false,
    );
    harness.run();
    harness
}

/// The control: the one button that is not coming soon.
fn control<'h>(harness: &'h Harness<'static, bool>) -> egui_kittest::Node<'h> {
    let mut controls = harness.query_all_by_role(Role::Button).filter(|node| node.accesskit_node().label().is_some_and(|label| label != "open log"));
    let control = controls.next().unwrap();
    assert!(controls.next().is_none(), "one control");
    control
}

fn label(harness: &Harness<'static, bool>) -> String {
    control(harness).accesskit_node().label().unwrap_or_default()
}

/// The fills of the discs painted (the status items' and the progress line's marks).
fn discs(harness: &Harness<'static, bool>) -> Vec<Color32> {
    fn open(shape: &Shape, out: &mut Vec<Color32>) {
        match shape {
            Shape::Vec(inner) => inner.iter().for_each(|shape| open(shape, out)),
            Shape::Circle(circle) => out.push(circle.fill),
            _ => {}
        }
    }
    let mut out = Vec::new();
    harness.output().shapes.iter().for_each(|clipped| open(&clipped.shape, &mut out));
    out
}

/// The rects of every text painted.
fn texts(harness: &Harness<'static, bool>) -> Vec<(Rect, String)> {
    fn open(shape: &Shape, out: &mut Vec<(Rect, String)>) {
        match shape {
            Shape::Vec(inner) => inner.iter().for_each(|shape| open(shape, out)),
            Shape::Text(text) => out.push((text.visual_bounding_rect(), text.galley.text().to_owned())),
            _ => {}
        }
    }
    let mut out = Vec::new();
    harness.output().shapes.iter().for_each(|clipped| open(&clipped.shape, &mut out));
    out
}

#[test]
fn the_four_frames_say_what_their_sheets_say_in_light_and_dark() {
    for mode in [Mode::Light, Mode::Dark] {
        let p = Palette::of(mode);
        let cases = [
            (SyncState::Idle, "calendars read 14:02, sync now", Some(p.plus)),
            (SyncState::Syncing { done: 5, of: 10, account: "Studio".to_owned() }, "syncing calendars: Studio, 5 of 10", None),
            (SyncState::Synced { at: read() }, "calendars synced 14:02, sync now", Some(p.plus)),
            (SyncState::Failed { account: Some("Studio".to_owned()), why: Why::Refused }, "Studio calendars: refused, try again", Some(p.minus)),
        ];
        for (state, spoken, mark) in cases {
            let view = SyncView { state: state.clone(), read: read() };
            let harness = harness(mode, view.clone(), false);
            assert_eq!(label(&harness), spoken, "{mode:?}");
            let drawn: Vec<String> = texts(&harness).into_iter().map(|(_, text)| text).collect();
            let words: &[&str] = match state {
                SyncState::Idle => &["calendars read 14:02", "sync now"],
                SyncState::Syncing { .. } => &["calendars", "[=====>    ]", "Studio"],
                SyncState::Synced { .. } => &["calendars", "[==========]", "14:02"],
                SyncState::Failed { .. } => &["Studio calendars: refused", "try again", "open log"],
            };
            for word in words {
                assert!(drawn.iter().any(|text| text == word), "{mode:?} {spoken}: {word} not in {drawn:?}");
            }
            if let Some(mark) = mark {
                assert!(discs(&harness).contains(&mark), "{mode:?} {spoken}: its mark");
            }
            // Idle and failed are links in `cyan_text`; the failure's own line is `minus`.
            let link = control(&harness).rect();
            let line = place(&view);
            assert!(line.expand(0.5).contains_rect(link), "{spoken}: the control {link:?} stands in the line {line:?}");
        }
        assert_eq!(place(&SyncView { state: SyncState::Idle, read: None }).height(), HIGH);
        assert_eq!(place(&SyncView { state: SyncState::Failed { account: None, why: Why::Failed }, read: None }).height(), HIGH_FAILED);
    }
}

#[test]
fn before_any_read_the_idle_line_has_no_time_and_no_mark() {
    let harness = harness(Mode::Dark, SyncView::default(), false);
    assert_eq!(label(&harness), "calendars not read yet, sync now");
    assert!(discs(&harness).is_empty());
}

#[test]
fn a_press_is_returned_only_when_the_line_may_take_it() {
    let mut harness = harness(Mode::Light, SyncView::default(), false);
    control(&harness).click();
    harness.run();
    assert!(*harness.state(), "a click on the link is a press");
    let mut resting = self::harness(Mode::Light, SyncView::default(), true);
    assert!(control(&resting).accesskit_node().is_disabled(), "resting it reads as unavailable");
    let at = control(&resting).rect().center();
    click_at(&mut resting, at);
    resting.run();
    resting.key_press(egui::Key::Tab);
    resting.run();
    resting.key_press(egui::Key::Enter);
    resting.run();
    assert!(!*resting.state(), "resting it takes no press");
    assert!(resting.ctx.memory(|memory| memory.focused()).is_none(), "nor the keyboard");
}

#[test]
fn a_long_hostile_account_name_stays_one_line_inside_the_line() {
    let name = format!("Stu\u{202E}dio\r\n\0{}", "x".repeat(400));
    for state in
        [SyncState::Syncing { done: 1, of: 3, account: name.clone() }, SyncState::Failed { account: Some(name.clone()), why: Why::Certificate }]
    {
        let view = SyncView { state, read: read() };
        let harness = harness(Mode::Dark, view.clone(), false);
        let line = place(&view);
        for (rect, text) in texts(&harness) {
            assert!(rect.left() >= line.left() - 0.5 && rect.right() <= line.right() + 0.5, "{text:?} at {rect:?} leaves {line:?}");
            assert!(!text.contains(['\r', '\n', '\0', '\u{202E}']), "{text:?}");
        }
        if matches!(view.state, SyncState::Syncing { .. }) {
            // The progress line lays its note out as given: it is cut before, with one ellipsis.
            assert!(texts(&harness).iter().any(|(_, text)| text.starts_with("Stu") && text.ends_with('…') && text.matches('…').count() == 1));
        }
    }
}

/// A click of the pointer's first button at `at`.
fn click_at<State>(harness: &mut Harness<'_, State>, at: egui::Pos2) {
    harness.hover_at(at);
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
    }
}
