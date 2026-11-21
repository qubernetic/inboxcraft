// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The status line in the window: each place's own key hints (Home, the calendar, the settings, the command line's
//! results, one pane), the error's `dismiss` by `Esc` where it reaches nothing else, the mailbox's error a full queue
//! dropped, and a message heard by a screen reader as a live region.

use egui::accesskit::{Live, Role};
use egui::{Key, Rect, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_time::{FakeClock, Timestamp};

use crate::InboxApp;
use crate::draw::shell;
use crate::mailbox::Mailbox;
use crate::sample;
use crate::status::{Message, QUEUE};

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

/// The sample window `width` wide at the mail, nothing open; `prepare` runs on it before the first frame.
fn window(width: f32, prepare: impl FnOnce(&mut InboxApp)) -> Harness<'static, InboxApp> {
    let mut app = InboxApp::at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    prepare(&mut app);
    let mut harness = Harness::builder().with_size(vec2(width, 900.0)).with_max_steps(12).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

fn line(harness: &Harness<'static, InboxApp>) -> Rect {
    shell::laid(&harness.ctx).unwrap().status
}

/// The words of the key hints in the status line, in order: what the hints say after their key.
fn hints(harness: &Harness<'static, InboxApp>) -> Vec<String> {
    const WORDS: [&str; 15] =
        ["next", "previous", "done", "snooze", "open", "write", "send", "commands", "mail", "calendar", "back", "list", "run", "close", "dismiss"];
    let status = line(harness);
    let mut found: Vec<(f32, String)> = harness
        .query_all_by_role(Role::Label)
        .filter(|node| status.contains_rect(node.rect()))
        .filter_map(|node| {
            let label = node.accesskit_node().label().or_else(|| node.accesskit_node().value())?.clone();
            let (key, word) = label.rsplit_once(' ')?;
            (!key.is_empty() && WORDS.contains(&word)).then(|| (node.rect().left(), word.to_owned()))
        })
        .collect();
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    found.into_iter().map(|(_, word)| word).collect()
}

/// Opens the list's first row by Enter: in one pane the message takes the list's place.
fn open_in_one_pane(harness: &mut Harness<'static, InboxApp>) {
    harness.query_all_by_role(Role::ListBoxOption).next().unwrap().focus();
    harness.run();
    harness.key_press(Key::Enter);
    harness.run();
    assert!(super::super::reader_rect(&harness.ctx).is_some(), "the message open");
}

/// The sample window with the mailbox's error `text` told, as a refused command tells it.
fn failing(width: f32, text: &str) -> Harness<'static, InboxApp> {
    let mut harness = window(width, |_| {});
    harness.state_mut().mailbox_mut().fail(crate::UiError::Shot(text.to_owned()));
    harness.run();
    harness
}

#[test]
fn each_place_says_its_own_keys() {
    // The frames `home/today`, `calendar-day/default`, `settings-about/default` (whose `back` the window leaves out
    // while its `Esc` keeps the settings shown), `command-line/commands`.
    let mut home = window(1440.0, InboxApp::show_home);
    assert_eq!(hints(&home), ["mail", "calendar", "write", "commands"], "Home");
    home.state_mut().show_calendar();
    home.run();
    assert_eq!(hints(&home), ["mail", "write", "commands"], "the calendar");
    home.state_mut().open_settings();
    home.run();
    assert_eq!(hints(&home), ["mail", "commands"], "the settings");
    let mut mail = window(1440.0, |_| {});
    let ctx = mail.ctx.clone();
    mail.state_mut().run_action(&ctx, crate::keymap::PALETTE);
    mail.run();
    assert_eq!(hints(&mail), ["next", "previous", "run", "close"], "the command line's results");
}

#[test]
fn in_one_pane_with_a_message_in_the_list_s_place_esc_is_the_list_s_key() {
    // `size-800/inbox-reader`: `Esc list`, `Ctrl K commands`.
    let mut harness = window(800.0, |_| {});
    open_in_one_pane(&mut harness);
    assert_eq!(hints(&harness), ["list", "commands"]);
    assert!(harness.query_by_label("Esc list").is_some(), "its key is Esc");
}

#[test]
fn esc_dismisses_the_error_shown_where_it_reaches_nothing_else() {
    // `status-line/error`: `Esc dismiss` and `Ctrl K commands` at the right.
    let mut harness = failing(1440.0, "Not moved: the message is still in Inbox.");
    assert!(harness.query_by_label_contains("Not moved").is_some(), "the error shows");
    assert_eq!(hints(&harness), ["dismiss", "commands"]);
    harness.key_press(Key::Escape);
    harness.run();
    assert!(harness.query_by_label_contains("Not moved").is_none(), "Esc dismissed it");
    assert_eq!(harness.state().mailbox().error(), None, "and the mailbox's error with it");
}

#[test]
fn the_dismiss_hint_dismisses_the_error() {
    let mut harness = failing(1440.0, "Not moved: the message is still in Inbox.");
    harness.get_by_label("Esc dismiss").click();
    harness.run();
    assert!(harness.query_by_label_contains("Not moved").is_none());
}

#[test]
fn one_esc_in_one_pane_goes_back_to_the_list_and_leaves_the_error() {
    let mut harness = window(800.0, |_| {});
    open_in_one_pane(&mut harness);
    harness.state_mut().mailbox_mut().fail(crate::UiError::Shot("Not moved: the message is still in Inbox.".to_owned()));
    // No field or control of the reader holds the keyboard: the Esc is still the list's, not the error's.
    harness.ctx.memory_mut(|memory| {
        if let Some(focused) = memory.focused() {
            memory.surrender_focus(focused);
        }
    });
    harness.run();
    assert_eq!(hints(&harness), ["list", "commands"], "Esc is the list's, not the error's");
    harness.key_press(Key::Escape);
    harness.run();
    assert!(harness.query_by_label_contains("Not moved").is_some(), "the error stays: the Esc went back to the list");
}

#[test]
fn the_mailbox_s_error_a_full_queue_dropped_goes_with_it() {
    let mut harness = failing(1440.0, "the first failure");
    for n in 0..QUEUE {
        harness.state_mut().status.push(Message::Error { text: format!("later failure {n}") });
    }
    harness.run();
    assert_eq!(harness.state().mailbox().error(), None, "it shows nowhere, so it does not stay the mailbox's");
    // The next failure is queued and shown in its turn.
    harness.state_mut().mailbox_mut().fail(crate::UiError::Shot("the first failure".to_owned()));
    harness.run();
    assert!(harness.state().status.holds_error("ui_shot: the first failure"), "queued again");
}

#[test]
fn a_message_is_a_live_region_an_error_at_once() {
    let harness = failing(1440.0, "Not moved: the message is still in Inbox.");
    let error = harness.get_by_label_contains("Not moved");
    assert_eq!(error.accesskit_node().live(), Live::Assertive);
    let mut harness = window(1440.0, |app| app.say("Link copied: https://example.org/open-day"));
    harness.run();
    let notice = harness.get_by_label_contains("Link copied");
    assert_eq!(notice.accesskit_node().live(), Live::Polite);
}

#[test]
fn in_two_panes_the_first_esc_gives_the_list_the_keyboard_the_second_dismisses() {
    let mut harness = window(1440.0, |_| {});
    harness.query_all_by_role(Role::ListBoxOption).next().unwrap().focus();
    harness.run();
    harness.key_press(Key::Enter);
    harness.run();
    harness.state_mut().mailbox_mut().fail(crate::UiError::Shot("Not moved: the message is still in Inbox.".to_owned()));
    harness.run();
    assert!(!hints(&harness).contains(&"dismiss".to_owned()), "the reader has the keyboard: Esc is the list's");
    harness.key_press(Key::Escape);
    harness.run();
    assert!(harness.query_by_label_contains("Not moved").is_some(), "the first Esc gave the list the keyboard");
    assert!(super::super::list::focused_row(&harness.ctx).is_some(), "on its row");
    harness.key_press(Key::Escape);
    harness.run();
    assert!(harness.query_by_label_contains("Not moved").is_none(), "the second dismissed the error");
}

const NOT_MOVED: &str = "Not moved: the message is still in Inbox.";

/// Tells the mailbox's error [`NOT_MOVED`] and draws it.
fn fail(harness: &mut Harness<'static, InboxApp>) {
    harness.state_mut().mailbox_mut().fail(crate::UiError::Shot(NOT_MOVED.to_owned()));
    harness.run();
    assert!(harness.query_by_label_contains("Not moved").is_some(), "the error shows");
}

fn shown(harness: &Harness<'static, InboxApp>) -> bool {
    harness.query_by_label_contains("Not moved").is_some()
}

fn escape(harness: &mut Harness<'static, InboxApp>) {
    harness.key_press(Key::Escape);
    harness.run();
}

/// Focuses the field `label` of `role`, an error showing: the first `Esc` leaves the field and leaves the error, the
/// next dismisses it.
fn esc_leaves_the_field_first(harness: &mut Harness<'static, InboxApp>, role: Role, label: &str) {
    fail(harness);
    harness.get_by_role_and_label(role, label).focus();
    harness.run();
    assert!(harness.ctx.memory(|memory| memory.focused().is_some()), "the field has the keyboard");
    escape(harness);
    assert!(shown(harness), "the Esc that left the field left the error");
    assert!(harness.ctx.memory(|memory| memory.focused().is_none()), "and left the field");
    assert!(hints(harness).contains(&"dismiss".to_owned()), "now Esc is the error's: {:?}", hints(harness));
    escape(harness);
    assert!(!shown(harness), "the next Esc dismissed it");
}

#[test]
fn esc_in_a_settings_field_over_home_leaves_the_field_not_the_error() {
    let mut harness = window(1440.0, |app| {
        app.show_home();
        app.open_settings_page(Some("sending"));
    });
    esc_leaves_the_field_first(&mut harness, Role::MultilineTextInput, "Signature of Personal");
}

#[test]
fn esc_in_a_template_field_leaves_the_field_not_the_error() {
    let mut harness = window(1440.0, |app| {
        let template = serde_json::json!({ "account": 2, "name": "Workshop invitation", "subject": "Thursday workshop", "text": "Hello." });
        app.mailbox_mut().run("template.save", &template).unwrap();
        app.open_templates();
    });
    harness.get_by_label("Edit the template Workshop invitation").click();
    harness.run();
    esc_leaves_the_field_first(&mut harness, Role::TextInput, "Subject");
}

#[test]
fn esc_in_the_floating_composer_s_suggestions_puts_them_away_not_the_error() {
    let mut harness = window(1440.0, |_| {});
    crate::draw::tests::new_message(&mut harness);
    harness.run();
    harness.state_mut().composer_mut().0.unwrap().set_floating(true);
    harness.run();
    harness.get_by_role_and_label(Role::TextInput, "To").focus();
    harness.run();
    harness.get_by_role_and_label(Role::TextInput, "To").type_text("teo");
    harness.run();
    assert!(!harness.state().composer().unwrap().suggestions().1.is_empty(), "suggested");
    fail(&mut harness);
    escape(&mut harness);
    assert!(harness.state().composer().unwrap().suggestions().1.is_empty(), "the suggestions went");
    assert!(shown(&harness), "the Esc that put them away left the error");
    // The line keeps the keys: its next Esc leaves it, and only then is Esc the error's.
    while harness.ctx.memory(|memory| memory.focused().is_some()) {
        escape(&mut harness);
        assert!(shown(&harness), "an Esc that left a field left the error");
    }
    escape(&mut harness);
    assert!(!shown(&harness), "the Esc with nothing holding the keyboard dismissed it");
}

#[test]
fn the_same_failure_told_again_after_a_dismissal_shows_again() {
    let mut harness = failing(1440.0, NOT_MOVED);
    // The line's dismiss applied, as a frame applies it, and no frame drawn after it.
    let dismissed = super::Pressed { dismiss: true, ..super::Pressed::default() };
    harness.state_mut().apply_line(&dismissed, None);
    assert_eq!(harness.state().mailbox().error(), None, "dismissed");
    // Told again before the next frame's status line reads it.
    harness.state_mut().mailbox_mut().fail(crate::UiError::Shot(NOT_MOVED.to_owned()));
    harness.run();
    assert!(shown(&harness), "the same failure told again is shown again");
}

#[test]
fn on_the_settings_esc_dismisses_whatever_is_under_them() {
    // Over the mail with the reader keyed, where Esc would give the list the keyboard were the mail shown.
    let mut harness = window(1440.0, |_| {});
    harness.query_all_by_role(Role::ListBoxOption).next().unwrap().focus();
    harness.run();
    harness.key_press(Key::Enter);
    harness.run();
    harness.state_mut().open_settings();
    harness.ctx.memory_mut(|memory| {
        if let Some(focused) = memory.focused() {
            memory.surrender_focus(focused);
        }
    });
    harness.run();
    fail(&mut harness);
    assert_eq!(hints(&harness), ["dismiss", "commands"]);
    escape(&mut harness);
    assert!(!shown(&harness), "dismissed, as over Home");
}

#[test]
fn with_the_list_over_the_docked_composer_an_error_keeps_the_composer_s_send() {
    // In one pane the list comes over the docked composer by Esc; an error then says its dismiss and keeps send.
    let mut harness = window(800.0, |_| {});
    crate::draw::tests::new_message(&mut harness);
    harness.run();
    harness.ctx.memory_mut(|memory| {
        if let Some(focused) = memory.focused() {
            memory.surrender_focus(focused);
        }
    });
    harness.run();
    escape(&mut harness);
    harness.ctx.memory_mut(|memory| {
        if let Some(focused) = memory.focused() {
            memory.surrender_focus(focused);
        }
    });
    harness.run();
    fail(&mut harness);
    // All three kept, the error's first; the narrow line keeps the last two, so the composer's send stays in sight.
    assert_eq!(super::hints(super::Keys::ErrorComposer).iter().map(|(_, word)| *word).collect::<Vec<_>>(), ["dismiss", "send", "commands"]);
    assert_eq!(hints(&harness), ["send", "commands"]);
    escape(&mut harness);
    assert!(!shown(&harness), "Esc still dismisses it, its hint left out for room");
    assert!(harness.state().composer().is_some(), "and leaves the draft");
}

#[test]
fn in_one_pane_the_docked_composer_s_keys_come_before_the_list_s() {
    // `size-800/composer`: send and commands, not `Esc list`, with a message opened before.
    let mut harness = window(800.0, |_| {});
    open_in_one_pane(&mut harness);
    crate::draw::tests::new_message(&mut harness);
    harness.run();
    assert_eq!(hints(&harness), ["send", "commands"]);
}

#[test]
fn the_results_say_no_next_previous_or_run_with_nothing_listed() {
    let mut harness = window(1440.0, |_| {});
    let ctx = harness.ctx.clone();
    harness.state_mut().run_action(&ctx, crate::keymap::PALETTE);
    harness.run();
    assert_eq!(hints(&harness), ["next", "previous", "run", "close"]);
    harness.get_by_role_and_label(Role::TextInput, "Command line").type_text(">qqqzzzxxx");
    harness.run();
    assert_eq!(hints(&harness), ["close"], "nothing listed: only close works");
}
