// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Removing an account from its form: "Remove account…" at the form's right end asks in a dialog over the dimmed
//! window, the question says first
//! that it cannot be undone, and `account.remove` runs only on the owner's press of the question's "Remove account" —
//! never from a hover, a focus, Enter on "Remove account…" alone, or Esc. The removal takes the account's password out
//! of the system's credential store, says so in the status line and goes back to the list.

use std::sync::Arc;

use egui::accesskit::Role;
use egui::{Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_time::{FakeClock, Timestamp};

use crate::mailbox::Mailbox;
use crate::onboard::tests::Kept;
use crate::onboard::{NoDetect, Stage};
use crate::workers::NoWorkers;
use crate::{InboxApp, sample};

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

/// The sample window, its passwords in `kept`, on the form of Studio (account 2).
fn on_studio(kept: &Kept) -> Harness<'static, InboxApp> {
    on_studio_writing(kept, None)
}

/// [`on_studio`], with a new message being written from the account `writing`, its text typed and kept.
fn on_studio_writing(kept: &Kept, writing: Option<u64>) -> Harness<'static, InboxApp> {
    let mut app = InboxApp::at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock())).with_accounts(
        Arc::new(kept.clone()),
        Box::new(NoDetect::default()),
        Box::new(NoWorkers),
    );
    app.set_reduce_motion(true);
    if let Some(account) = writing {
        app.compose();
        let (composer, mailbox) = app.composer_mut();
        let composer = composer.unwrap();
        composer.set_account(mailbox, account).unwrap();
        composer.text_mut().push_str(WRITTEN);
        composer.edited();
        composer.save(mailbox).unwrap();
        assert_eq!(app.composer().unwrap().account(), account);
    }
    app.open_accounts();
    let (onboarding, mailbox) = app.onboarding_mut();
    onboarding.edit(mailbox, 2);
    let mut harness = Harness::builder()
        .with_size(vec2(1440.0, 900.0))
        .with_max_steps(40)
        .with_theme(egui::Theme::Dark)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

fn accounts(harness: &Harness<'static, InboxApp>) -> Vec<u64> {
    harness.state().mailbox().accounts().iter().map(|account| account.id).collect()
}

fn focused(harness: &Harness<'static, InboxApp>) -> Option<String> {
    harness.root().children_recursive().find(|node| node.accesskit_node().is_focused()).map(|node| node.accesskit_node().label().unwrap_or_default())
}

fn press(harness: &mut Harness<'static, InboxApp>, label: &str) {
    harness.get_by_label(label).scroll_to_me();
    harness.run();
    harness.get_by_label(label).click();
    harness.run();
}

fn key(harness: &mut Harness<'static, InboxApp>, key: Key) {
    harness.key_press_modifiers(Modifiers::NONE, key);
    harness.run();
}

const QUESTION: &str = "Remove Studio from this computer?";

#[test]
fn removing_asks_in_a_dialog_says_first_it_cannot_be_undone_and_runs_only_on_the_owner_s_press() {
    let kept = Kept::default();
    let mut harness = on_studio(&kept);
    assert_eq!(harness.state().onboarding().stage(), Stage::Edit(2));
    // The keyboard on "Remove account…" and Enter there only ask.
    harness.get_by_label("Remove account…").focus();
    harness.run();
    assert_eq!(focused(&harness).as_deref(), Some("Remove account…"));
    assert_eq!(accounts(&harness), [1, 2], "a focus removes nothing");
    key(&mut harness, Key::Enter);
    let heading = harness.get_by_role_and_label(Role::Heading, QUESTION).rect();
    assert_eq!(accounts(&harness), [1, 2], "Enter on Remove account… only asks");
    // The question says first that it cannot be undone, then what goes and what stays.
    let first = harness.get_by_label("This cannot be undone.").rect();
    let goes = harness.get_by_label_contains("From this computer go:").rect();
    let stays = harness.get_by_label_contains("Everything on the server stays:").rect();
    assert!(heading.bottom() <= first.top() && first.bottom() <= goes.top() && goes.bottom() <= stays.top(), "{first:?} {goes:?} {stays:?}");
    assert!(harness.state().surfaces(&harness.ctx).modal(), "the dialog is a modal surface");
    assert_eq!(focused(&harness).as_deref(), Some("Cancel"), "the question takes the keyboard on Cancel");
    // A hover and the keyboard on the question's "Remove account" run nothing.
    harness.get_by_label("Remove account").hover();
    harness.run();
    key(&mut harness, Key::Tab);
    assert_eq!(focused(&harness).as_deref(), Some("Remove account"));
    assert_eq!(accounts(&harness), [1, 2], "a focus or a hover removes nothing");
    assert!(kept.forgotten.lock().unwrap().is_empty());
    // Esc is the question's: it closes, the form stays, the keyboard goes back to Remove account….
    key(&mut harness, Key::Escape);
    assert!(harness.query_by_label("This cannot be undone.").is_none());
    assert_eq!(harness.state().onboarding().stage(), Stage::Edit(2), "Esc closes the question only");
    assert!(harness.state().settings().is_open());
    assert_eq!(focused(&harness).as_deref(), Some("Remove account…"));
    assert_eq!(accounts(&harness), [1, 2]);
    // The owner's press of the question removes the account and its password.
    press(&mut harness, "Remove account…");
    press(&mut harness, "Remove account");
    assert_eq!(accounts(&harness), [1], "removed from this computer");
    assert_eq!(kept.forgotten.lock().unwrap().as_slice(), [2], "its password leaves the credential store");
    assert_eq!(harness.state().onboarding().stage(), Stage::Closed, "back to the list");
    assert_eq!(harness.state().notice(), Some("Studio removed from this computer"));
    harness.get_by_label_contains("Your accounts · 1");
    harness.run();
    assert!(focused(&harness).is_some(), "the keyboard is not left on nothing");
}

#[test]
fn cancel_keeps_the_account_and_hands_the_keyboard_back() {
    let kept = Kept::default();
    let mut harness = on_studio(&kept);
    press(&mut harness, "Remove account…");
    // Two Cancels: the form's, under the dimmed window, and the dialog's, which has the keyboard.
    assert_eq!(focused(&harness).as_deref(), Some("Cancel"));
    let dialogs = harness.root().children_recursive().find(|node| node.accesskit_node().is_focused()).unwrap().rect().center();
    press_at(&mut harness, dialogs);
    assert_eq!(accounts(&harness), [1, 2]);
    assert!(kept.forgotten.lock().unwrap().is_empty());
    assert!(harness.query_by_label("This cannot be undone.").is_none());
    assert_eq!(harness.state().onboarding().stage(), Stage::Edit(2), "the form stays open");
    assert_eq!(focused(&harness).as_deref(), Some("Remove account…"));
}

#[test]
fn the_form_under_the_question_saves_nothing_and_the_question_does_not_outlast_the_page() {
    let kept = Kept::default();
    let mut harness = on_studio(&kept);
    let save = harness.get_by_label("Save").rect().center();
    press(&mut harness, "Remove account…");
    harness.get_by_role_and_label(Role::Heading, QUESTION);
    // Save lies under the dimmed window: a press there closes the question and saves nothing.
    press_at(&mut harness, save);
    assert!(harness.query_by_label("This cannot be undone.").is_none(), "the press on the scrim closes the question");
    assert_eq!(harness.state().onboarding().stage(), Stage::Edit(2), "Save under the scrim is not pressed");
    assert_eq!(accounts(&harness), [1, 2]);
    press(&mut harness, "Remove account…");
    harness.get_by_role_and_label(Role::Heading, QUESTION);
    // Another page of the settings and back: the form asks nothing.
    harness.state_mut().leave_settings();
    harness.run();
    harness.state_mut().open_accounts();
    harness.run();
    assert_eq!(harness.state().onboarding().stage(), Stage::Edit(2));
    assert!(harness.query_by_label("This cannot be undone.").is_none(), "the question went with the page");
    let (onboarding, mailbox) = harness.state_mut().onboarding_mut();
    assert!(onboarding.remove(mailbox).is_none(), "not asked, not removed");
    assert_eq!(accounts(&harness), [1, 2], "nothing removes an account the owner was not asked about");
}

#[test]
fn the_list_has_no_coming_soon_row_for_removing_any_more() {
    let kept = Kept::default();
    let mut harness = on_studio(&kept);
    harness.state_mut().onboarding_mut().0.close();
    harness.run();
    assert!(harness.query_by_label("Remove account").is_none(), "removing lives on each account's form");
}

/// `key` held down: one press, then `repeats` repeats, a frame each, then let go. With `fresh` off the press goes down
/// while no control has the keyboard, which the control in `then` takes before the repeats: the key was already down
/// when it got the keyboard.
fn hold(harness: &mut Harness<'static, InboxApp>, key: Key, repeats: usize, fresh: bool, then: &str) {
    let event = |pressed, repeat| egui::Event::Key { key, physical_key: None, pressed, repeat, modifiers: Modifiers::NONE };
    if !fresh && let Some(id) = harness.ctx.memory(|memory| memory.focused()) {
        harness.ctx.memory_mut(|memory| memory.surrender_focus(id));
    }
    harness.event(event(true, false));
    harness.run();
    if !fresh {
        harness.get_by_label(then).focus();
        harness.run();
    }
    for _ in 0..repeats {
        harness.event(event(true, true));
        harness.run();
    }
    harness.event(event(false, false));
    harness.run();
}

#[test]
fn a_held_key_never_removes_the_account() {
    let kept = Kept::default();
    let mut harness = on_studio(&kept);
    // Enter held on "Remove account…": the press asks, its repeats never answer the question.
    harness.get_by_label("Remove account…").focus();
    harness.run();
    for key in [Key::Enter, Key::Space] {
        hold(&mut harness, key, 12, true, "");
        assert_eq!(accounts(&harness), [1, 2], "a held {key:?} on Remove account… removes nothing");
    }
    // A key already down when the question's "Remove account" takes the keyboard: its repeats are no press.
    if harness.query_by_label("This cannot be undone.").is_none() {
        press(&mut harness, "Remove account…");
    }
    for key in [Key::Enter, Key::Space] {
        hold(&mut harness, key, 12, false, "Remove account");
        assert_eq!(accounts(&harness), [1, 2], "repeats of a held {key:?} on the question's Remove account remove nothing");
    }
    assert!(kept.forgotten.lock().unwrap().is_empty());
    // A fresh press there is the owner's.
    harness.get_by_label("Remove account").focus();
    harness.run();
    hold(&mut harness, Key::Enter, 0, true, "");
    assert_eq!(accounts(&harness), [1]);
}

const WRITTEN: &str = "Half a thought about the studio's rota";
const WRITING: &str = "The message you are writing from Studio is discarded with it.";

#[test]
fn removing_the_account_a_message_is_written_from_says_so_and_closes_it() {
    let kept = Kept::default();
    let mut harness = on_studio_writing(&kept, Some(2));
    press(&mut harness, "Remove account…");
    let heading = harness.get_by_role_and_label(Role::Heading, QUESTION).rect();
    let writing = harness.get_by_label(WRITING).rect();
    let stays = harness.get_by_label_contains("Everything on the server stays:").rect();
    assert!(heading.bottom() <= writing.top() && writing.bottom() <= stays.top(), "said with what goes: {writing:?}");
    press(&mut harness, "Remove account");
    assert_eq!(accounts(&harness), [1]);
    assert!(harness.state().composer().is_none(), "the message written from it closes with the account");
    for _ in 0..4 {
        harness.run();
    }
    assert_eq!(harness.state().mailbox().error(), None, "no keep into a draft that is gone");
    assert_eq!(harness.state().notice(), Some("Studio removed from this computer"));
}

#[test]
fn a_message_written_from_another_account_stays_with_its_text() {
    let kept = Kept::default();
    let mut harness = on_studio_writing(&kept, Some(1));
    press(&mut harness, "Remove account…");
    harness.get_by_role_and_label(Role::Heading, QUESTION);
    assert!(harness.query_by_label(WRITING).is_none(), "nothing is written from Studio");
    press(&mut harness, "Remove account");
    assert_eq!(accounts(&harness), [1]);
    let composer = harness.state().composer().expect("the message from the other account stays");
    assert_eq!(composer.account(), 1);
    assert!(composer.text().contains(WRITTEN), "with its text");
    for _ in 0..4 {
        harness.run();
    }
    assert_eq!(harness.state().mailbox().error(), None);
}

/// A press of the pointer at `at`, down in one frame and up in the next.
fn press_at(harness: &mut Harness<'static, InboxApp>, at: egui::Pos2) {
    harness.hover_at(at);
    harness.run();
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
        harness.run();
    }
}

/// Where the text starting with `wanted` was painted in the last frame.
fn painted(harness: &Harness<'static, InboxApp>, wanted: &str) -> Option<egui::Rect> {
    fn find(shape: &egui::Shape, wanted: &str) -> Option<egui::Rect> {
        match shape {
            egui::Shape::Vec(inner) => inner.iter().find_map(|shape| find(shape, wanted)),
            egui::Shape::Text(text) if text.galley.text().starts_with(wanted) => Some(egui::Rect::from_min_size(text.pos, text.galley.size())),
            _ => None,
        }
    }
    harness.output().shapes.iter().find_map(|clipped| find(&clipped.shape, wanted))
}

#[test]
fn the_dialog_stands_where_the_frame_draws_it() {
    // The frame `account-remove/confirm`: "Remove account…" has no room under it, so the card (328 high) stands 12
    // over it, centred across the window. The frame's button is at 602, so its card at 480, 262, as the picture has
    // them; this window's fonts are egui's own, so here the button stands elsewhere, and the card keeps the rule.
    let mut harness = on_studio(&Kept::default());
    press(&mut harness, "Remove account…");
    let label = painted(&harness, "CONFIRM").expect("the card's label is drawn");
    let card = label.min - vec2(25.0, 25.0);
    let asked = harness.get_all_by_label("Remove account…").next().unwrap().rect();
    assert_eq!(card, egui::pos2(480.0, asked.top() - 12.0 - 328.0), "the card's corner, over {asked:?}");
    assert!(harness.query_by_label("Esc cancel").is_some(), "the status line says Esc cancels");
}

#[test]
fn in_the_least_window_at_130_the_dialog_s_buttons_stay_in_reach() {
    // The window's least size and 130 % text, with the message being written: the card stays in the dimmed part and
    // the owner's press of its danger button removes.
    let kept = Kept::default();
    let mut app = InboxApp::at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock())).with_accounts(
        Arc::new(kept.clone()),
        Box::new(NoDetect::default()),
        Box::new(NoWorkers),
    );
    app.set_reduce_motion(true);
    app.set_text_scale(crate::theme::TextScale::ALL[4]);
    app.compose();
    let (composer, mailbox) = app.composer_mut();
    let composer = composer.unwrap();
    composer.set_account(mailbox, 2).unwrap();
    composer.text_mut().push_str(WRITTEN);
    composer.edited();
    composer.save(mailbox).unwrap();
    app.open_accounts();
    let (onboarding, mailbox) = app.onboarding_mut();
    onboarding.edit(mailbox, 2);
    let mut harness = Harness::builder()
        .with_size(vec2(800.0, 480.0))
        .with_max_steps(40)
        .with_theme(egui::Theme::Dark)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    press(&mut harness, "Remove account…");
    harness.get_by_label(WRITING);
    let cover = crate::component::dialog::cover(&harness.ctx, &crate::theme::Theme::new(crate::theme::Mode::Dark, crate::theme::TextScale::ALL[4]));
    let dialog = crate::component::dialog::layer(egui::Id::unique("account-remove-confirm"));
    for label in ["Cancel", "Remove account"] {
        let button = harness.get_all_by_label(label).map(|node| node.rect()).find(|rect| harness.ctx.layer_id_at(rect.center()) == Some(dialog));
        let button = button.unwrap_or_else(|| panic!("the dialog's {label}"));
        assert!(cover.contains_rect(button), "{label}: {button:?} in {cover:?}");
    }
    let remove = harness.get_all_by_label("Remove account").map(|node| node.rect()).find(|rect| cover.contains_rect(*rect)).unwrap();
    harness.hover_at(remove.center());
    harness.run();
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton { pos: remove.center(), button: egui::PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
        harness.run();
    }
    assert_eq!(accounts(&harness), [1], "the owner's press of Remove account removes");
}

/// The status line's 2-high top lines the last frame painted: their rects and colours.
fn top_lines(harness: &Harness<'static, InboxApp>) -> Vec<(egui::Rect, egui::Color32)> {
    fn walk(shape: &egui::Shape, found: &mut Vec<(egui::Rect, egui::Color32)>) {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, found)),
            egui::Shape::Rect(rect) if rect.rect.height() == 2.0 => found.push((rect.rect, rect.fill)),
            _ => {}
        }
    }
    let line = super::shell::laid(&harness.ctx).unwrap().status;
    let mut found = Vec::new();
    harness.output().shapes.iter().for_each(|clipped| walk(&clipped.shape, &mut found));
    found.retain(|(rect, _)| rect.top() == line.top() || rect.top() == line.top() + 1.0);
    found
}

#[test]
fn a_removal_that_went_through_is_a_done_message_and_one_the_store_refused_is_not() {
    let p = crate::theme::Theme::of(crate::theme::Mode::Dark, &crate::theme::Look::default()).palette;
    let check = crate::iconset::glyph(crate::iconset::IconId::Check).to_string();
    // `account-remove/removed`: the line's message 560 wide, a 2-high `plus` edge on top, the check in `plus`.
    let mut harness = on_studio(&Kept::default());
    assert!(harness.state_mut().remove_account_as_owner(2));
    harness.run();
    harness.get_by_label_contains("Studio removed from this computer");
    let lines = top_lines(&harness);
    assert!(lines.iter().any(|(rect, fill)| *fill == p.plus && rect.width() == 560.0), "{lines:?}");
    let drawn = crate::component::field::tests::drawn(&harness);
    assert!(drawn.iter().any(|text| text.text == check && text.colour == p.plus), "the check in plus");
    // The store kept the password: the removal is not done, the line keeps the notice's look.
    let kept = Kept::default();
    kept.refuse_forget.store(true, std::sync::atomic::Ordering::SeqCst);
    let mut harness = on_studio(&kept);
    assert!(harness.state_mut().remove_account_as_owner(2));
    harness.run();
    harness.get_by_label_contains("did not let its password go");
    let lines = top_lines(&harness);
    assert!(!lines.is_empty() && lines.iter().all(|(_, fill)| *fill != p.plus), "{lines:?}");
    assert!(lines.iter().any(|(_, fill)| *fill == p.muted), "the notice's line: {lines:?}");
    assert!(!crate::component::field::tests::drawn(&harness).iter().any(|text| text.text == check), "no check");
    let status = &harness.state().status;
    assert!(status.notice().is_some_and(|text| text.contains("did not let its password go")), "a notice");
    assert!(!status.has_error(), "not an error");
}
