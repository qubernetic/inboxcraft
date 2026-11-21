// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The settings in a narrow window, as the design's `settings-800` and `settings-1000-130` frames draw them: under
//! Mail's breakpoint one pane, the list of sections or a page whose head leads back to it; at 1000 and 130 % the list
//! beside a narrower page; a control wider than half its row under its words; the signatures one under the other; list
//! rows whose text columns grow with the type; a long page scrolling under a scroll bar at the body's right edge.

use egui::accesskit::Role;
use egui::{Key, Rect, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_time::{FakeClock, Timestamp};
use serde_json::json;

use super::Columns;
use crate::draw::InboxApp;
use crate::mailbox::Mailbox;
use crate::sample;
use crate::settings::Section;

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

/// The sample window that can send, `width` × 900 at `percent` text with the design's fonts, on the settings' `section`.
fn window(width: f32, percent: u16, section: Section) -> Harness<'static, InboxApp> {
    let mut app = InboxApp::at_mail(Mailbox::new(sample::sending(clock()).unwrap(), clock()));
    app.set_reduce_motion(true);
    app.set_text_scale(crate::theme::TextScale::from_percent(percent).unwrap());
    app.settings_mut().open_at(section);
    let mut harness = Harness::builder()
        .with_size(vec2(width, 900.0))
        .with_max_steps(40)
        .with_context_setup(crate::theme::install_fonts)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

/// Where the text painted in the last frame that starts with `wanted` stands.
fn painted(harness: &Harness<'static, InboxApp>, wanted: &str) -> Option<Rect> {
    painted_in(harness, wanted, Rect::EVERYTHING)
}

/// [`painted`], the first that starts inside `room`.
fn painted_in(harness: &Harness<'static, InboxApp>, wanted: &str, room: Rect) -> Option<Rect> {
    fn find(shape: &egui::Shape, wanted: &str, room: Rect) -> Option<Rect> {
        match shape {
            egui::Shape::Vec(inner) => inner.iter().find_map(|shape| find(shape, wanted, room)),
            egui::Shape::Text(text) if text.galley.text().starts_with(wanted) && room.contains(text.pos) => {
                Some(Rect::from_min_size(text.pos, text.galley.size()))
            }
            _ => None,
        }
    }
    harness.output().shapes.iter().find_map(|clipped| find(&clipped.shape, wanted, room))
}

/// The window's body, as the shell lays it out.
fn body(harness: &Harness<'static, InboxApp>) -> Rect {
    super::super::shell::laid(&harness.ctx).map(|laid| laid.body).unwrap()
}

fn is_focused(harness: &Harness<'static, InboxApp>, label: &str) -> bool {
    harness.get_by_label(label).accesskit_node().is_focused()
}

fn rows(harness: &Harness<'static, InboxApp>) -> usize {
    harness.query_all_by_label_contains(", settings page").count()
}

#[test]
fn the_columns_stand_where_the_narrow_frames_draw_them() {
    // `settings-800`: the body 741 wide at 58, the list across it, the page 40 in and 663 wide (38 from the edge).
    // `settings-1000-130`: the list 232, its hairline, the page 40 in from it and 630 wide.
    let theme = crate::theme::Theme::new(crate::theme::Mode::Dark, crate::theme::TextScale::default());
    let narrow = super::super::shell::regions(vec2(800.0, 900.0), &theme, 1.0).body;
    assert_eq!((narrow.left(), narrow.width()), (58.0, 741.0));
    let alone = Columns::of(narrow, Section::Appearance, true);
    assert_eq!(alone.nav, narrow, "the list takes the whole body");
    assert_eq!(alone.page, Rect::from_min_size(pos2(98.0, narrow.top() + 24.0), vec2(663.0, narrow.height() - 24.0 - 16.0)));
    let wide = super::super::shell::regions(vec2(1000.0, 900.0), &theme, 1.0).body;
    let beside = Columns::of(wide, Section::Appearance, false);
    assert_eq!(beside.nav.width(), 232.0);
    assert_eq!(beside.divider, wide.left() + 232.0);
    assert_eq!((beside.page.left(), beside.page.width()), (wide.left() + 233.0 + 40.0, 630.0));
    // However small the body, nothing inverts.
    for single in [false, true] {
        let tiny = Columns::of(Rect::from_min_size(pos2(10.0, 10.0), vec2(30.0, 20.0)), Section::Shortcuts, single);
        assert!(tiny.page.width() >= 1.0 && tiny.page.height() >= 1.0 && tiny.nav.width() >= 0.0, "{tiny:?}");
    }
}

#[test]
fn under_mail_s_breakpoint_the_body_holds_one_pane_and_from_it_two() {
    for (width, single) in [(super::super::ONE_PANE - 1.0, true), (super::super::ONE_PANE, false)] {
        let harness = window(width, 100, Section::Triage);
        assert_eq!(harness.query_by_label("← Settings").is_some(), single, "{width}: the page's way back");
        assert_eq!(rows(&harness), if single { 0 } else { 11 }, "{width}: the list beside the page");
        harness.get_by_role_and_label(Role::Heading, "Triage");
    }
}

#[test]
fn back_and_esc_go_to_the_list_and_a_row_opens_its_page() {
    let mut harness = window(800.0, 100, Section::Appearance);
    let title = harness.get_by_role_and_label(Role::Heading, "Appearance").rect();
    let page = Columns::of(body(&harness), Section::Appearance, true).page;
    assert!((title.left() - (page.left() + 16.0)).abs() <= 1.0, "the title 16 into the page: {title:?} {page:?}");
    assert!(painted(&harness, "[ 04 ]").is_none(), "where it is gives way to the way back");
    harness.get_by_label("← Settings").click();
    harness.run();
    assert!(harness.state().settings().listed());
    assert!(harness.query_by_role_and_label(Role::Heading, "Appearance").is_none(), "the page left");
    assert_eq!(rows(&harness), 11);
    let row = harness.get_by_label("Appearance, settings page");
    assert_eq!(row.rect().width(), body(&harness).width(), "the rows take the whole body");
    let picked = |node: &egui_kittest::Node<'_>| node.accesskit_node().toggled() == Some(egui::accesskit::Toggled::True);
    assert!(!harness.query_all_by_label_contains(", settings page").any(|node| picked(&node)), "no row is picked");
    // The row of the page shown opens it too.
    row.click();
    harness.run();
    harness.get_by_role_and_label(Role::Heading, "Appearance");
    assert_eq!(rows(&harness), 0, "the list left");
    harness.key_press(Key::Escape);
    harness.run();
    assert!(harness.state().settings().listed(), "Esc goes back to the list");
    assert!(harness.state().settings().is_open(), "and leaves the settings open");
    harness.get_by_label("Triage, settings page").click();
    harness.run();
    harness.get_by_role_and_label(Role::Heading, "Triage");
    assert_eq!(harness.state().settings().section(), Section::Triage);
}

#[test]
fn the_keyboard_goes_from_the_row_to_the_way_back_and_home_again() {
    let mut harness = window(800.0, 100, Section::Appearance);
    harness.get_by_label("← Settings").focus();
    harness.run();
    harness.key_press(Key::Enter);
    harness.run();
    harness.run();
    assert!(is_focused(&harness, "Appearance, settings page"), "back on the row of the page left");
    harness.get_by_label("Sending, settings page").focus();
    harness.run();
    harness.key_press(Key::Enter);
    harness.run();
    harness.run();
    assert!(is_focused(&harness, "← Settings"), "the page's way back has the keyboard");
    // Esc from a control further down the page, which leaves with it.
    harness.key_press(Key::Tab);
    harness.run();
    assert!(!is_focused(&harness, "← Settings"), "Tab went on into the page");
    harness.key_press(Key::Escape);
    harness.run();
    harness.run();
    assert!(is_focused(&harness, "Sending, settings page"), "Esc, and the keyboard is on the row again");
}

#[test]
fn esc_in_a_field_is_the_field_s() {
    let mut harness = window(800.0, 100, Section::Shortcuts);
    harness.get_by_role_and_label(Role::TextInput, "Find a command").focus();
    harness.run();
    harness.key_press(Key::Escape);
    harness.run();
    assert!(!harness.state().settings().listed(), "the field gives up the keyboard; the page stays");
}

#[test]
fn at_1000_and_130_percent_the_list_stays_beside_the_narrower_page() {
    let harness = window(1000.0, 130, Section::Appearance);
    let row = harness.get_by_label("Appearance, settings page").rect();
    assert_eq!(row.width(), 232.0);
    let title = harness.get_by_role_and_label(Role::Heading, "Appearance").rect();
    assert!((title.left() - (body(&harness).left() + 233.0 + 40.0 + 16.0)).abs() <= 1.0, "{title:?}");
    harness.get_by_label_contains("[ 04 ]");
}

/// The rect of the segment `name`, and of the title `title` painted on its row.
fn control_and_title(harness: &Harness<'static, InboxApp>, name: &str, title: &str) -> (Rect, Rect) {
    (harness.get_by_label(name).rect(), painted(harness, title).unwrap())
}

#[test]
fn a_control_wider_than_half_its_row_stands_under_its_words_at_the_left() {
    // `settings-800`: Triage's screener modes stack, Sending's Undo send and Appearance's mode stay at the right.
    // `settings-1000-130`: the screener's modes stack, Appearance's inbox view stays. (The frame's Undo send stacks
    // there too: it has an Off segment the app does not, and without it the control keeps under half its row.)
    let check = |harness: &Harness<'static, InboxApp>, name: &str, title: &str, stacked: bool| {
        let page = Columns::of(body(harness), harness.state().settings().section(), harness.ctx.content_rect().width() < 980.0).page;
        let (control, words) = control_and_title(harness, name, title);
        if stacked {
            assert!(control.top() > words.bottom(), "{title}: under the words: {control:?} {words:?}");
            assert!((control.left() - (page.left() + 16.0)).abs() <= 1.0, "{title}: at the left: {control:?}");
        } else {
            assert!(control.left() > page.center().x, "{title}: beside the words, at the right: {control:?}");
        }
    };
    let harness = window(800.0, 100, Section::Triage);
    check(&harness, "Before the inbox", "New senders", true);
    let harness = window(800.0, 100, Section::Sending);
    check(&harness, "5 s", "Undo send", false);
    let harness = window(800.0, 100, Section::Appearance);
    check(&harness, "Auto", "Mode", false);
    let harness = window(1000.0, 130, Section::Triage);
    check(&harness, "Before the inbox", "New senders", true);
    let harness = window(1000.0, 130, Section::Appearance);
    check(&harness, "Grouped", "Inbox view", false);
    let harness = window(1440.0, 100, Section::Triage);
    check(&harness, "Before the inbox", "New senders", false);
}

#[test]
fn in_a_narrow_window_the_signatures_stand_one_under_the_other_16_apart() {
    for (width, percent, under) in [(800.0, 100, true), (1000.0, 130, true), (1440.0, 100, false)] {
        let harness = window(width, percent, Section::Sending);
        let accounts = harness.state().mailbox().accounts();
        let field = |index: usize| {
            let label = format!("Signature of {}", accounts.get(index).unwrap().label);
            harness.get_by_role_and_label(Role::MultilineTextInput, &label).rect()
        };
        let (first, second) = (field(0), field(1));
        if under {
            assert!((second.left() - first.left()).abs() < 0.5 && second.top() > first.bottom(), "{width}: one under the other");
            let save = harness.query_all_by_label_contains("Save the signature of").next().unwrap().rect();
            let label = painted(&harness, &format!("SIGNATURE OF {}", accounts.get(1).unwrap().label.to_uppercase())).unwrap();
            assert!((label.top() - save.bottom() - 16.0).abs() <= 1.5, "{width}: 16 between them: {}", label.top() - save.bottom());
        } else {
            assert!((second.top() - first.top()).abs() < 0.5 && second.left() > first.right(), "{width}: side by side");
        }
    }
}

#[test]
fn at_130_percent_a_list_row_s_text_columns_grow_with_the_type() {
    // The time 96 → 124 and the kind 72 → 94 of a scheduled entry; at 1000 the second text (whom it names, how long
    // before) stands under the subject, at 1440 beside it.
    let mut harness = window(1440.0, 130, Section::Sending);
    sample::scheduled(harness.state_mut().mailbox_mut()).unwrap();
    harness.state_mut().open_scheduled();
    harness.run();
    let time = painted(&harness, "Today 14:50").unwrap();
    let tag = painted(&harness, "alert").unwrap();
    let subject = painted(&harness, "Design review").unwrap();
    let person = painted(&harness, "10 min before").unwrap();
    assert!(tag.left() - time.left() >= 124.0 + 12.0 - 0.5, "the time's column 124: {}", tag.left() - time.left());
    assert!((subject.center().y - person.center().y).abs() < 1.0, "1440: the person beside the subject");
    harness.set_size(vec2(1000.0, 900.0));
    harness.run();
    let subject = painted(&harness, "Design review").unwrap();
    let person = painted(&harness, "10 min before").unwrap();
    assert!((person.left() - subject.left()).abs() < 0.5 && person.top() >= subject.bottom() - 4.0, "1000: under it: {subject:?} {person:?}");
    let row = harness.get_by_label_contains("Cancel the snooze Today 16:00").rect();
    let next = harness.get_by_label_contains("Cancel the send Fri 11:00").rect();
    assert!((next.center().y - row.center().y - 53.0).abs() <= 1.0, "rows 52 and a hairline: {}", next.center().y - row.center().y);
    // A template's name: 200 → 260.
    let mut harness = window(1440.0, 130, Section::Sending);
    let saved = json!({ "account": 2, "name": "Workshop invitation", "subject": "Thursday workshop", "text": "Hello." });
    harness.state_mut().mailbox_mut().run("template.save", &saved).unwrap();
    harness.state_mut().open_templates();
    harness.run();
    let name = painted(&harness, "Workshop invitation").unwrap();
    let line = painted(&harness, "Thursday workshop").unwrap();
    assert!(line.left() - name.left() >= 260.0 + 12.0 - 0.5, "the name's column 260: {}", line.left() - name.left());
    // An account's name: 148 → 192, its address 12 after it.
    let harness = window(1440.0, 130, Section::Accounts);
    let account = harness.state().mailbox().accounts().first().unwrap();
    let page = Columns::of(body(&harness), Section::Accounts, false).page;
    let name = painted_in(&harness, &account.label, page).unwrap();
    let address = painted_in(&harness, &account.address, page).unwrap();
    assert!((address.left() - name.left() - 192.0 - 12.0).abs() <= 0.5, "the name's column 192: {}", address.left() - name.left());
}

#[test]
fn a_page_taller_than_the_body_scrolls_under_a_bar_at_the_body_s_right_edge() {
    let mut harness = window(800.0, 100, Section::Sending);
    // The harness's window stands 8 in from the screen's corner, so its right edge lies past the screen's: the pointer
    // goes 3 in from the screen's, right of the column and of the bar's old place beside it.
    let column = Columns::of(body(&harness), Section::Sending, true).page;
    let at = 797.0;
    assert!(at > column.right() + 2.0 * 12.0 && at < body(&harness).right(), "{column:?}");
    let before = painted(&harness, "Undo send").unwrap();
    harness.event(egui::Event::PointerMoved(pos2(at, 600.0)));
    harness.run();
    harness.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: vec2(0.0, -200.0),
        modifiers: egui::Modifiers::NONE,
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Wheel,
    });
    harness.run();
    let after = painted(&harness, "Undo send").map_or(f32::NEG_INFINITY, |rect| rect.top());
    assert!(after < before.top() - 50.0, "the wheel at the body's edge scrolls the page: {} → {after}", before.top());
}

/// Whether the text painted in the last frame that starts with `wanted` ends with an ellipsis (egui keeps the whole
/// text in a cut galley and marks it `elided`).
fn cut_short(harness: &Harness<'static, InboxApp>, wanted: &str) -> Option<bool> {
    fn find(shape: &egui::Shape, wanted: &str) -> Option<bool> {
        match shape {
            egui::Shape::Vec(inner) => inner.iter().find_map(|shape| find(shape, wanted)),
            egui::Shape::Text(text) if text.galley.text().starts_with(wanted) => Some(text.galley.elided),
            _ => None,
        }
    }
    harness.output().shapes.iter().find_map(|clipped| find(&clipped.shape, wanted))
}

#[test]
fn the_all_day_alert_choice_reads_whole_at_1000_and_130_percent() {
    // `settings-1000-130-calendar` read "09:00 the day be…": the field kept its 220 at any text size.
    let harness = window(1000.0, 130, Section::Calendar);
    assert_eq!(cut_short(&harness, "09:00 the day before"), Some(false), "the chosen hour reads whole");
    let whole = painted(&harness, "09:00 the day before").unwrap();
    let field = harness.get_by_role_and_label(Role::ComboBox, "All-day alerts ring at").rect();
    assert!(field.contains_rect(whole), "{whole:?} inside the field {field:?}");
    assert!(field.right() <= body(&harness).right(), "the field stays in the body");
}
