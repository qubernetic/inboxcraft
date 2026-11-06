// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The reader driven through its AccessKit tree. The harness refuses any control without an accessible name after
//! every run.

use egui::accesskit::Role;
use egui::{Key, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_time::{FakeClock, Timestamp};
use serde_json::json;

use super::InboxApp;
use crate::iconset::{IconId, Style};
use crate::mailbox::{After, Mailbox};
use crate::reader::tests::Recorder;
use crate::sample;
use crate::save::NoSave;

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

fn app_with(fetch: &Recorder) -> InboxApp {
    InboxApp::with_outside(Mailbox::new(sample::session(clock()).unwrap(), clock()), Box::new(fetch.clone()), Box::new(NoSave))
}

fn harness_with(app: InboxApp, theme: egui::Theme) -> Harness<'static, InboxApp> {
    let mut harness = Harness::builder()
        .with_size(vec2(1280.0, 800.0))
        .with_theme(theme)
        .with_max_steps(12)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

/// The app with the message of `subject` selected in the unified inbox (or `open`ed in another folder first).
fn opened(fetch: &Recorder, folder: Option<u64>, subject: &str) -> Harness<'static, InboxApp> {
    let mut app = app_with(fetch);
    if let Some(folder) = folder {
        app.mailbox_mut().open(crate::mailbox::Place::Folder(folder));
    }
    let id = app.mailbox().messages().iter().find(|row| row.subject == subject).unwrap().id;
    app.mailbox_mut().select(Some(id));
    harness_with(app, egui::Theme::Light)
}

#[test]
fn the_reader_shows_the_sender_s_avatar_in_the_avatar_palette() {
    let fetch = Recorder::default();
    let harness = opened(&fetch, None, "Photos from the coast walk");
    assert!(harness.query_all_by_role(Role::Image).any(|node| node.accesskit_node().label().as_deref() == Some("Teodor Lisk")));
    let theme = crate::theme::Theme::new(crate::theme::Mode::Light, &crate::theme::Preset::default());
    let colour = crate::component::avatar::colour(&theme, "teodor@example.org");
    assert!(crate::theme::AVATARS.contains(&colour));
    let filled = |shape: &egui::Shape| matches!(shape, egui::Shape::Circle(circle) if circle.fill == colour);
    fn any(shape: &egui::Shape, found: &dyn Fn(&egui::Shape) -> bool) -> bool {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().any(|shape| any(shape, found)),
            other => found(other),
        }
    }
    assert!(harness.output().shapes.iter().any(|clipped| any(&clipped.shape, &filled)), "the avatar is painted in its colour");
}

#[test]
fn selecting_a_row_opens_the_reader_beside_the_list() {
    let fetch = Recorder::default();
    let mut harness = harness_with(app_with(&fetch), egui::Theme::Light);
    harness.get_by_label("Teodor Lisk, Photos from the coast walk, 04:00, flagged").click();
    harness.run();
    assert!(harness.state().reader().open_id().is_some());
    harness.get_by_label("Photos from the coast walk");
    harness.get_by_label("teodor@example.org");
    harness.get_by_label("to Mira Holloway");
    harness.get_by_label("Save cliff-path.png");
    harness.get_by_label("Save lighthouse.png");
    harness.get_by_label("Accounts");
    harness.check_accessibility();
    harness.get_by_label("More").click();
    harness.run();
    harness.get_by_label("Close").click();
    harness.run();
    assert_eq!(harness.state().mailbox().selected(), None);
    assert!(harness.query_by_label("More").is_none(), "the reader is gone");
}

#[test]
fn every_reader_control_has_an_accessible_name_in_both_modes() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let fetch = Recorder::default();
        let mut app = app_with(&fetch);
        let id = app.mailbox().messages().iter().find(|row| row.subject == "Your bike is due for a service").unwrap().id;
        app.mailbox_mut().select(Some(id));
        let mut harness = harness_with(app, theme);
        harness.check_accessibility();
        harness.get_by_label("Load once").click();
        harness.run();
        harness.check_accessibility();
        harness.get_by_label("Details").click();
        harness.run();
        harness.check_accessibility();
    }
}

/// Picks `name` (Full width or Leave full width) from the reader's More menu.
fn full_width(harness: &mut Harness<'static, InboxApp>, name: &str) {
    harness.get_by_label("More").click();
    harness.run();
    harness.get_by_role_and_label(Role::MenuItem, name).click();
    harness.run();
}

#[test]
fn escape_leaves_the_full_width_reader() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, None, "Photos from the coast walk");
    full_width(&mut harness, "Full width");
    assert!(harness.state().reader().full_width());
    assert!(harness.query_by_label("Accounts").is_none(), "the folder pane makes way");
    assert!(harness.query_by_role(Role::TextInput).is_none(), "and the list with its search");
    harness.key_press(Key::Escape);
    harness.run();
    assert!(!harness.state().reader().full_width());
    harness.get_by_label("Accounts");
    full_width(&mut harness, "Full width");
    full_width(&mut harness, "Leave full width");
    assert!(!harness.state().reader().full_width());
}

#[test]
fn no_remote_request_before_consent() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, None, "Your bike is due for a service");
    for _ in 0..3 {
        harness.run();
    }
    assert!(fetch.asked.lock().unwrap().is_empty(), "nothing asked while the banner stands");
    harness.get_by_label("1 remote picture is not shown. Loading it tells the sender that you opened this message.");
    harness.get_by_label_contains("Remote picture not loaded");
    harness.get_by_label("Load once").click();
    harness.run();
    assert_eq!(*fetch.asked.lock().unwrap(), ["https://pictures.example.org/cycles/workshop.png"]);
    harness.run();
    harness.get_by_role_and_label(Role::Image, "The workshop bench");
    assert!(harness.query_by_label("Load once").is_none(), "the banner is gone");
}

#[test]
fn always_for_this_sender_remembers_the_sender() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, Some(5), "You have won a lighthouse");
    assert!(fetch.asked.lock().unwrap().is_empty());
    harness.get_by_label("Always for prize-desk@example.org").click();
    harness.run();
    assert_eq!(fetch.asked.lock().unwrap().len(), 1);
    let (_, mailbox) = harness.state_mut().reader_mut();
    assert_eq!(mailbox.run("remote.list", &json!({})).unwrap()["senders"], json!(["prize-desk@example.org"]));
}

#[test]
fn a_texture_goes_with_its_picture() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, Some(5), "You have won a lighthouse");
    harness.get_by_label("Always for prize-desk@example.org").click();
    harness.run();
    let url = "https://track.example.net/open.png?id=4417";
    assert!(harness.state().textures.contains_key(url), "the fetched picture is drawn");
    let gone = harness.ctx.load_texture("gone", egui::ColorImage::filled([1, 1], egui::Color32::WHITE), egui::TextureOptions::LINEAR);
    harness.state_mut().textures.insert("https://pictures.example.org/gone.png".to_owned(), gone);
    harness.run();
    assert!(!harness.state().textures.contains_key("https://pictures.example.org/gone.png"), "no picture, no texture");
    assert!(harness.state().textures.contains_key(url));
}

#[test]
fn the_link_under_the_pointer_shows_where_it_goes_and_a_click_copies_it() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, Some(5), "You have won a lighthouse");
    assert!(harness.query_by_label_contains("but the link goes to").is_none(), "no status line before a link is hovered");
    let link = harness.get_by_label("https://bank.example.org/login").rect();
    harness.hover_at(link.left_center() + vec2(8.0, 0.0));
    harness.run();
    harness.get_by_label_contains("but the link goes to prize-desk.example.net");
    harness.hover_at(egui::pos2(5.0, 5.0));
    harness.run();
    assert!(harness.query_by_label_contains("but the link goes to").is_none(), "the status line goes with the pointer");
    harness.hover_at(link.left_center() + vec2(8.0, 0.0));
    harness.run();
    harness.get_by_label("https://bank.example.org/login").click();
    harness.run();
    harness.hover_at(egui::pos2(5.0, 5.0));
    harness.run();
    harness.get_by_label("Link copied: https://prize-desk.example.net/claim");
    harness.get_by_label("Dismiss").click();
    harness.run();
    assert_eq!(harness.state().reader().notice(), None);
}

#[test]
fn quotes_fold_and_the_older_message_unfolds_on_a_click() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, Some(3), "Re: Allotment rota for November");
    harness.get_by_label("Show quoted text (8 lines)").click();
    harness.run();
    harness.get_by_label("Odile Brannock wrote:");
    harness.get_by_label("Hide quoted text").click();
    harness.run();
    assert!(harness.query_by_label("Odile Brannock wrote:").is_none());
    harness.get_by_label_contains("Odile Brannock, 08:35, folded").click();
    harness.run();
    harness.get_by_label_contains("Could you take the second week?");
    harness.get_by_label("Fold").click();
    harness.run();
    harness.get_by_label_contains("Odile Brannock, 08:35, folded");
    harness.get_by_label("Details").click();
    harness.run();
    harness.get_by_label("Mira Holloway <mira@example.org>");
    harness.get_by_label("Thursday 5 Nov 2026, 08:40");
    harness.get_by_label("Hide details").click();
    harness.run();
}

#[test]
fn saving_an_attachment_says_where_it_went_or_why_not() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, None, "Draft logo for the bakery");
    assert!(super::tests::drew_icon(harness.output(), IconId::File, Style::Regular), "the tile shows the file icon");
    harness.get_by_label("Save sketch-2.png").click();
    harness.run();
    harness.get_by_label("Saving attachments is not available in this window.");
}

#[test]
fn the_next_message_rule_is_picked_in_the_more_menu_and_arrows_step_through_the_list() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, None, "Allotment rota for November");
    harness.get_by_label("More").click();
    harness.run();
    harness.get_by_label("When a message leaves the list");
    let chosen = |harness: &Harness<'static, InboxApp>, name: &str| harness.get_by_label(name).accesskit_node().toggled();
    assert_eq!(chosen(&harness, "Show the next message"), Some(egui::accesskit::Toggled::True));
    assert_eq!(chosen(&harness, "Go back to the list"), Some(egui::accesskit::Toggled::False));
    harness.get_by_label("Go back to the list").click();
    harness.run();
    assert_eq!(harness.state().mailbox().after(), After::List);
    harness.key_press(Key::ArrowDown);
    harness.run();
    let selected = harness.state().mailbox().selected();
    let row = harness.state().mailbox().messages().iter().find(|row| Some(row.id) == selected).unwrap();
    assert_eq!(row.subject, "Draft logo for the bakery");
    assert_eq!(harness.state().reader().open_id(), selected, "the reader follows");
    harness.key_press(Key::ArrowUp);
    harness.run();
    assert_eq!(harness.state().reader().opened(harness.state().reader().open_id().unwrap()).unwrap().header.subject, "Allotment rota for November");
}

#[test]
fn the_find_shortcut_leaves_full_width_for_the_search() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, None, "Allotment rota for November");
    full_width(&mut harness, "Full width");
    harness.key_press_modifiers(egui::Modifiers::COMMAND, Key::F);
    harness.run();
    harness.run();
    assert!(!harness.state().reader().full_width());
}

#[test]
fn the_toolbar_holds_the_mail_actions_and_those_without_a_command_are_disabled() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, None, "Photos from the coast walk");
    full_width(&mut harness, "Full width");
    let disabled = |name: &str| harness.get_by_role_and_label(Role::Button, name).accesskit_node().is_disabled();
    assert!(!disabled("Done"), "the sample mailbox can mark it done");
    assert!(!disabled("Delete"), "and delete it");
    for name in ["Archive", "Snooze", "Pin", "Move to a folder"] {
        assert!(disabled(name), "{name} has no command yet");
    }
    assert!(!disabled("More"));
    for icon in [IconId::Done, IconId::Archive, IconId::Delete, IconId::Snooze, IconId::Pin, IconId::Move, IconId::More] {
        assert!(super::tests::drew_icon(harness.output(), icon, Style::Regular), "{icon:?}");
    }
}

#[test]
fn a_disabled_action_says_why_in_its_tooltip() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, None, "Photos from the coast walk");
    assert!(harness.query_by_label("Not available yet").is_none());
    harness.get_by_role_and_label(Role::Button, "Snooze").hover();
    for _ in 0..4 {
        harness.run();
    }
    harness.get_by_label("Not available yet");
    let before = harness.state().mailbox().last_run();
    harness.get_by_role_and_label(Role::Button, "Snooze").click();
    harness.run();
    assert_eq!(harness.state().mailbox().last_run(), before, "a disabled action runs nothing");
}

#[test]
fn outside_the_sample_done_and_delete_are_disabled_too() {
    let fetch = Recorder::default();
    let store = sample::mailbox(Timestamp::from_unix(1_793_869_200, 0).unwrap()).unwrap();
    let session = inboxcraft_engine::Session::new(Box::new(store), clock()).unwrap();
    let mut app = InboxApp::with_outside(Mailbox::new(session, clock()), Box::new(fetch.clone()), Box::new(NoSave));
    assert!(!app.mailbox().can_act());
    let id = app.mailbox().messages().first().unwrap().id;
    app.mailbox_mut().select(Some(id));
    let harness = harness_with(app, egui::Theme::Light);
    for name in ["Done", "Delete"] {
        assert!(harness.get_by_role_and_label(Role::Button, name).accesskit_node().is_disabled(), "{name}");
    }
}

#[test]
fn done_in_the_toolbar_marks_the_open_message_done_and_the_toast_offers_undo() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, None, "Photos from the coast walk");
    let id = harness.state().reader().open_id().unwrap();
    harness.get_by_role_and_label(Role::Button, "Done").click();
    for _ in 0..4 {
        harness.run();
    }
    assert!(harness.state().mailbox().messages().iter().all(|row| row.id != id), "it left the inbox");
    harness.get_by_label("Marked as done");
    assert_ne!(harness.state().reader().open_id(), Some(id), "the next-message rule picks what shows");
}

#[test]
fn delete_in_the_toolbar_moves_the_open_message_to_trash() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, None, "Photos from the coast walk");
    let id = harness.state().reader().open_id().unwrap();
    harness.get_by_role_and_label(Role::Button, "Delete").click();
    for _ in 0..4 {
        harness.run();
    }
    assert!(harness.state().mailbox().messages().iter().all(|row| row.id != id));
    harness.get_by_label("Moved to Trash");
}

#[test]
fn the_open_message_offers_reply_reply_all_and_forward_disabled_for_now() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, Some(3), "Re: Allotment rota for November");
    for name in ["Reply", "Reply all", "Forward"] {
        assert!(harness.get_by_role_and_label(Role::Button, name).accesskit_node().is_disabled(), "{name} has no composer yet");
    }
    harness.get_by_label_contains("Odile Brannock, 08:35, folded").click();
    harness.run();
    assert_eq!(harness.query_all_by_label("Reply").count(), 1, "only the open message has them");
}

#[test]
fn the_subject_heads_the_thread_with_who_wrote_and_how_many_messages() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, Some(3), "Re: Allotment rota for November");
    full_width(&mut harness, "Full width");
    harness.get_by_label("Odile Brannock, Mira Holloway · 2 messages");
    let size = |text: &str| super::tests::font_of(&harness, text).size;
    let subject = size("Re: Allotment rota for November");
    for other in ["Mira Holloway", "The second week works for me."] {
        assert!(subject > size(other), "the subject is larger than {other}");
    }
}

#[test]
fn a_long_subject_wraps_and_is_never_cut_by_the_toolbar() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, None, "Photos from the coast walk");
    full_width(&mut harness, "Full width");
    harness.set_size(vec2(300.0, 800.0));
    harness.run();
    let subject = harness.get_by_label("Photos from the coast walk").rect();
    let more = harness.get_by_role_and_label(Role::Button, "More").rect();
    assert!(subject.top() >= more.bottom(), "the subject sits under the toolbar, not beside it");
    let galley = super::tests::galley_of(&harness, "Photos from the coast walk");
    assert!(!galley.elided, "the subject is never cut");
    assert!(galley.rows.len() > 1, "it wraps instead");
}
