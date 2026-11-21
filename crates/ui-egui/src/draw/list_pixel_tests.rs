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
            assert!((title.left() - (pane.left() + 16.0)).abs() <= 1.0, "{at}: the place 16 in: {title:?} in {pane:?}");
            assert!((holds.left() - title.right() - 10.0).abs() <= 1.0, "{at}: 10 between: {title:?} {holds:?}");
            assert!((switch.right() - (pane.right() - 16.0)).abs() <= 1.0, "{at}: the switch 16 in from the right: {switch:?} in {pane:?}");
            // The filters' line under the head and its hairline, "Show done" at its start.
            let filters_middle = pane.top() + head + 1.0 + filters / 2.0;
            assert!((done.center().y - filters_middle).abs() <= 1.0, "{at}: Show done {done:?} on the filters' line at {filters_middle}");
            assert!((done.left() - (pane.left() + 16.0)).abs() <= 1.0, "{at}: Show done 16 in: {done:?}");
        }
    }
}

/// `inbox-grouped/filled`, `shell/reader-open`: the list stands on the frame's pixels, from the index's line (233 in the
/// 1440 frame) to the reader's (674): 440 wide from the body's left (234), the place and Show done 16 in, the rows
/// across it, the reader's pane (its line first) from 674 and its subject 40 right of that line.
#[test]
fn the_list_stands_440_wide_between_the_index_s_line_and_the_reader_s() {
    for open in [false, true] {
        let mut harness = grouped(1440.0, TextScale::default());
        if open {
            let first = harness.state().mailbox().messages().first().map(|row| row.id);
            harness.state_mut().mailbox_mut().select(first);
            harness.run();
            harness.run();
        }
        let pane = list(&harness);
        let body = super::super::shell::laid(&harness.ctx).unwrap().body;
        assert_eq!((pane.left(), pane.width()), (body.left(), 440.0), "open {open}: the list from the body's left, 440 wide: {pane:?}");
        let left = pane.left();
        assert_eq!(harness.get_by_label("Inbox").rect().left(), left + 16.0, "open {open}: the place 16 in");
        assert_eq!(harness.get_by_role_and_label(Role::Button, "Show done").rect().left(), left + 16.0, "open {open}: Show done 16 in");
        let row = harness.get_all_by_role(Role::ListBoxOption).next().unwrap().rect();
        assert_eq!((row.left(), row.right()), (left, left + 440.0), "open {open}: a row across the list");
        if open {
            let reader = super::super::reader_rect(&harness.ctx).unwrap();
            // The reader's pane starts at the list's right with its line in its first column (674 in the frame).
            assert_eq!(reader.left(), left + 440.0, "the reader's line 440 from the list's left");
            // `shell/reader-open`: the subject 40 right of the reader's line (715 in the frame).
            let subject = harness.state().mailbox().messages().first().unwrap().subject.clone();
            let title = harness.query_all_by_label(&subject).map(|node| node.rect()).find(|rect| rect.left() > reader.left()).unwrap();
            assert_eq!(title.left(), reader.left() + 1.0 + 40.0, "the subject 40 from the reader's line: {title:?}");
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

/// `inbox-grouped/screener-card`: Review (32) centred on the 64 card, 16 from its top, on a whole pixel; the card's
/// line under its 64 (on the next even height, 66) and the first group's head under that line; at 130 % the card is 84
/// and Review 42.
#[test]
fn the_screener_card_centres_review_on_its_64() {
    for (scale, card, review) in [(TextScale::default(), 64.0, 32.0), (TextScale::from_percent(130).unwrap(), 84.0, 42.0)] {
        let harness = grouped(1440.0, scale);
        let pressed = harness.get_by_role_and_label(Role::Button, "Review").rect();
        let head = harness.get_by_label_contains("People, ").rect();
        // `text-130/inbox-reader`: a group's head 32 (42), a row 56 (72).
        let (head_high, row_high) = if card > 64.0 { (42.0, 72.0) } else { (32.0, 56.0) };
        assert_eq!(head.height(), head_high, "{scale:?}: the group's head");
        let row = harness.get_all_by_role(Role::ListBoxOption).next().unwrap().rect();
        assert_eq!(row.height(), row_high, "{scale:?}: a row {row:?}");
        let top = pressed.top() - (card - review) / 2.0;
        assert_eq!(top, top.round(), "{scale:?}: Review {pressed:?} on a whole pixel");
        // The card's 64 (84) and its line under it, on the next even height: 66 (86).
        assert_eq!(head.top() - top, card + 2.0, "{scale:?}: the head {head:?} under the card's {card} and its line from {top}");
    }
}

/// The texts painted in the list's head line, left to right: what each shows (an elided run ends in `…`) and where.
fn head_texts(harness: &Harness<'static, InboxApp>, head: f32) -> Vec<(String, Rect)> {
    fn walk(shape: &egui::Shape, found: &mut Vec<(String, Rect)>) {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, found)),
            egui::Shape::Text(text) => {
                let shown: String = text.galley.rows.iter().flat_map(|row| row.glyphs.iter().map(|glyph| glyph.chr)).collect();
                found.push((shown, text.galley.rect.translate(text.pos.to_vec2())));
            }
            _ => {}
        }
    }
    let pane = list(harness);
    let line = Rect::from_min_size(pane.min, vec2(pane.width(), head));
    let mut found = Vec::new();
    for clipped in &harness.output().shapes {
        walk(&clipped.shape, &mut found);
    }
    found.retain(|(text, rect)| !text.trim().is_empty() && line.contains(rect.center()));
    found.sort_by(|a, b| a.1.left().total_cmp(&b.1.left()));
    found
}

/// The one-line head beside the 440 list never cuts what the list holds: whose mail gives way first, with one `…`, and
/// the count stays whole, in the List view and narrowed to the long account, at 100 and 130 % (#325, I-3).
#[test]
fn the_list_head_cuts_whose_mail_before_the_count() {
    for (scale, head) in [(TextScale::default(), 48.0), (TextScale::from_percent(130).unwrap(), 62.0)] {
        for narrowed in [false, true] {
            let mut harness = grouped(1440.0, scale);
            let mailbox = harness.state_mut().mailbox_mut();
            if narrowed {
                let long = mailbox.accounts().iter().find(|account| account.address.starts_with("holloway")).map(|account| account.id);
                mailbox.narrow(long);
            } else {
                mailbox.set_inbox_view(crate::mailbox::InboxView::List);
            }
            harness.run();
            harness.run();
            let count = harness.state().mailbox().list_count();
            let whose = harness.state().mailbox().subtitle();
            let at = format!("{scale:?} narrowed {narrowed}");
            assert!((list(&harness).width() - 440.0).abs() <= 1.0, "{at}: the list 440 wide");
            let texts = head_texts(&harness, head);
            let shown: String = texts.iter().map(|(text, _)| text.as_str()).collect::<Vec<_>>().join(" ");
            // Nothing in the words runs under the view switch where it stands in the head (with too little room it
            // stands on the filters' line).
            if let Some(switch) =
                harness.query_by_role_and_label(Role::RadioGroup, "the inbox").filter(|switch| switch.rect().top() < list(&harness).top() + head)
            {
                let right = texts
                    .iter()
                    .filter(|(text, _)| !["Grouped", "Smart", "List"].iter().any(|word| text.contains(word)))
                    .map(|(_, rect)| rect.right());
                for edge in right {
                    assert!(edge <= switch.rect().left() + 0.5, "{at}: the words end {edge} before the switch {:?}", switch.rect());
                }
            }
            if !narrowed {
                // Every account's inbox says what is left to read, in the List view too (#328): no whose mail to cut.
                let unread = harness.state().mailbox().messages().iter().filter(|row| !row.seen).count();
                assert!(shown.contains(&format!("{unread} to read")), "{at}: {texts:?}");
                continue;
            }
            assert!(shown.contains(&count), "{at}: the count {count:?} whole in {texts:?}");
            assert!(shown.matches('…').count() <= 1, "{at}: one ellipsis at most: {texts:?}");
            let whose_shown = texts.iter().find(|(text, _)| text.contains('…') || text.contains(&whose)).map(|(text, _)| text.clone());
            let whose_shown = whose_shown.unwrap_or_default();
            let kept = whose_shown.trim_end_matches('…');
            assert!(!kept.is_empty() && whose.starts_with(kept), "{at}: whose mail {whose:?} cut from its end: {texts:?}");
        }
    }
}
