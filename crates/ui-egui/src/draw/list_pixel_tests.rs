// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The list's parts measured against the design's frames (`inbox-*/filled`, `drafts-list/*`, `later/snoozed`,
//! `new-senders/before-inbox`, `shell/reader-open`, `text-130/inbox-reader`): the numbers are the frames'.

use egui::accesskit::Role;
use egui::{Rect, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;
use inboxcraft_time::{FakeClock, Timestamp};

use crate::mailbox::Mailbox;
use crate::theme::TextScale;
use crate::{InboxApp, sample};

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

/// The sample window with the invented groups (the unified inbox in its Grouped view), `width` wide.
fn grouped(width: f32, scale: TextScale) -> Harness<'static, InboxApp> {
    let mut mailbox = Mailbox::new(sample::session(clock()).unwrap(), clock());
    let invented = sample::showcase(&mailbox);
    mailbox.set_showcase(invented);
    let mut app = InboxApp::at_mail(mailbox);
    app.set_text_scale(scale);
    let mut harness = crate::component::tests::builder().with_size(vec2(width, 900.0)).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

fn list(harness: &Harness<'static, InboxApp>) -> Rect {
    harness.state().list_rect.unwrap()
}

/// The design's head: one 48 line (62 at 130 %) with the place, what it holds and the view switch at its right 16 in,
/// a hairline, then the 36 filters' line (46) with "Show done" 16 in.
#[test]
fn the_list_head_is_one_line_with_the_view_switch_at_its_right_and_the_pills_under_it() {
    for (scale, head, filters) in [(TextScale::default(), 48.0, 36.0), (TextScale::from_percent(130).unwrap(), 62.0, 46.0)] {
        for open in [false, true] {
            let mut harness = grouped(1440.0, scale);
            if open {
                let first = harness.state().mailbox().messages().first().map(|row| row.id);
                harness.state_mut().mailbox_mut().select(first);
                harness.run();
                harness.run();
            }
            let pane = list(&harness);
            let title = harness.get_by_label("Inbox").rect();
            let holds = harness.get_by_label_contains(" to read").rect();
            let switch = harness.get_by_role_and_label(Role::RadioGroup, "the inbox").rect();
            let done = harness.get_by_role_and_label(Role::Button, "Show done").rect();
            let at = format!("{scale:?} open {open}");
            // The head's line is `head` high from the pane's top, its parts centred on it.
            let middle = pane.top() + head / 2.0;
            for (name, rect) in [("place", title), ("what it holds", holds), ("switch", switch)] {
                assert!((rect.center().y - middle).abs() <= 1.0, "{at}: {name} {rect:?} on the head's line at {middle}");
            }
            assert!((title.left() - (pane.left() + 1.0 + 16.0)).abs() <= 1.0, "{at}: the place 16 in: {title:?} in {pane:?}");
            assert!((holds.left() - title.right() - 10.0).abs() <= 1.0, "{at}: 10 between: {title:?} {holds:?}");
            assert!((switch.right() - (pane.right() - 16.0)).abs() <= 1.0, "{at}: the switch 16 in from the right: {switch:?} in {pane:?}");
            // The filters' line under the head and its hairline, "Show done" at its start.
            let filters_middle = pane.top() + head + 1.0 + filters / 2.0;
            assert!((done.center().y - filters_middle).abs() <= 1.0, "{at}: Show done {done:?} on the filters' line at {filters_middle}");
            assert!((done.left() - (pane.left() + 1.0 + 16.0)).abs() <= 1.0, "{at}: Show done 16 in: {done:?}");
        }
    }
}

/// The window on New senders, `width` wide.
fn queue(width: f32) -> Harness<'static, InboxApp> {
    let mailbox = Mailbox::new(sample::session(clock()).unwrap(), clock());
    let app = InboxApp::at_mail(mailbox);
    let mut harness = crate::component::tests::builder().with_size(vec2(width, 900.0)).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Review").click();
    harness.run();
    harness.run();
    harness
}

/// `new-senders/before-inbox`: SCREENER (16 high) stands centred on the switch's 32 line, 8 from its top.
#[test]
fn the_screener_label_stands_centred_on_its_switch() {
    let harness = queue(1440.0);
    let label = harness.get_by_label("Screener").rect();
    let switch = harness.get_by_role_and_label(Role::RadioGroup, "the screener").rect();
    assert!((label.center().y - switch.center().y).abs() <= 1.0, "{label:?} centred on {switch:?}");
    assert!((label.left() + label.width() + 12.0 - switch.left()).abs() <= 1.0, "12 before the switch: {label:?} {switch:?}");
}

/// `new-senders/before-inbox`: the card is 900 wide; Let in and Keep out, as wide as they draw themselves and 8
/// apart, end 20 inside its 1 px edge.
#[test]
fn the_answers_of_a_sender_card_end_on_its_padding() {
    let harness = queue(1440.0);
    let left = harness.get_by_label("Who gets in?").rect().left();
    let let_in = harness.get_all_by_role_and_label(Role::Button, "Let in").next().unwrap().rect();
    let keep_out = harness.get_all_by_role_and_label(Role::Button, "Keep out").next().unwrap().rect();
    assert!((keep_out.left() - let_in.right() - 8.0).abs() <= 0.5, "8 apart: {let_in:?} {keep_out:?}");
    assert!((keep_out.right() - (left + 900.0 - 21.0)).abs() <= 0.5, "on the card's padding: {keep_out:?} from {left}");
}
