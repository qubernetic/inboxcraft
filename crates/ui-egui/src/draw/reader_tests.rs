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
use crate::iconset::IconId;
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
fn the_reader_shows_the_sender_s_avatar_on_the_stage() {
    let fetch = Recorder::default();
    // A thread's lines carry the writers' avatars.
    let harness = opened(&fetch, Some(3), "Re: Allotment rota for November");
    harness.get_by_label_contains("Odile Brannock, 08:35, folded");
    let stages = [crate::theme::Mode::Light, crate::theme::Mode::Dark].map(|mode| crate::theme::Palette::of(mode).stage);
    let square = egui::CornerRadius::same(2);
    let filled = |shape: &egui::Shape| matches!(shape, egui::Shape::Rect(rect) if stages.contains(&rect.fill) && rect.corner_radius == square);
    fn any(shape: &egui::Shape, found: &dyn Fn(&egui::Shape) -> bool) -> bool {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().any(|shape| any(shape, found)),
            other => found(other),
        }
    }
    assert!(harness.output().shapes.iter().any(|clipped| any(&clipped.shape, &filled)), "the avatar is a stage square with corners of 2");
}

#[test]
fn selecting_a_row_opens_the_reader_beside_the_list() {
    let fetch = Recorder::default();
    let mut harness = harness_with(app_with(&fetch), egui::Theme::Light);
    harness.get_by_label("Teodor Lisk, Photos from the coast walk, 04:00, flagged").click();
    harness.run();
    assert!(harness.state().reader().open_id().is_some());
    harness.get_by_label("Photos from the coast walk");
    harness.get_by_label_contains("From Teodor Lisk  <teodor@example.org>");
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
        harness.get_by_label("details ▾").click();
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
    harness.get_by_label("Show 8 quoted lines").click();
    harness.run();
    harness.get_by_label("Odile Brannock wrote:");
    harness.get_by_label("Hide quoted text").click();
    harness.run();
    assert!(harness.query_by_label("Odile Brannock wrote:").is_none());
    harness.get_by_label_contains("Odile Brannock, 08:35, folded").click();
    harness.run();
    harness.get_by_label_contains("Could you take the second week?");
    // A click on its open line folds it again.
    harness.get_by_label_contains("Odile Brannock, 08:35, open").click();
    harness.run();
    harness.get_by_label_contains("Odile Brannock, 08:35, folded");
    harness.get_by_label("details ▾").click();
    harness.run();
    harness.get_by_label_contains("From Mira Holloway  <mira@example.org>");
    harness.get_by_label_contains("Thursday 5 Nov 2026, 08:40");
    harness.get_by_label("hide details ▴").click();
    harness.run();
}

#[test]
fn saving_an_attachment_says_where_it_went_or_why_not() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, None, "Draft logo for the bakery");
    assert!(super::tests::drew_icon(harness.output(), IconId::File), "the tile shows the file icon");
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
fn the_toolbar_holds_the_mail_actions() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, None, "Photos from the coast walk");
    full_width(&mut harness, "Full width");
    let disabled = |name: &str| harness.get_by_role_and_label(Role::Button, name).accesskit_node().is_disabled();
    for name in ["Done", "Delete", "Snooze", "Pin", "Move to a folder"] {
        assert!(!disabled(name), "{name} acts on the open message");
    }
    assert!(!disabled("More"));
    for icon in [IconId::Done, IconId::Delete, IconId::Snooze, IconId::Pin, IconId::Move, IconId::More] {
        assert!(super::tests::drew_icon(harness.output(), icon), "{icon:?}");
    }
}

#[test]
fn a_disabled_action_says_why_in_its_tooltip() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, None, "Photos from the coast walk");
    assert!(harness.query_by_label("Not available yet").is_none());
    harness.get_by_role_and_label(Role::Button, "Start").hover();
    for _ in 0..4 {
        harness.run();
    }
    harness.get_by_label("Not available yet");
    let before = harness.state().mailbox().last_run();
    harness.get_by_role_and_label(Role::Button, "Start").click();
    harness.run();
    assert_eq!(harness.state().mailbox().last_run(), before, "a disabled action runs nothing");
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
    assert_eq!(harness.state().reader().open_id(), None, "no message opens by itself: the reader shows nothing open");
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
fn the_open_message_offers_reply_reply_all_and_forward() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, Some(3), "Re: Allotment rota for November");
    for name in ["Reply", "Reply all", "Forward"] {
        assert!(!harness.get_by_role_and_label(Role::Button, name).accesskit_node().is_disabled(), "{name} opens the composer");
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
    harness.get_by_label("Odile Brannock, Mira Holloway  ·  2 messages");
    let size = |text: &str| super::tests::font_of(&harness, text).size;
    let subject = size("Re: Allotment rota for November");
    for other in ["Mira Holloway", "The second week works for me."] {
        assert!(subject > size(other), "the subject is larger than {other}");
    }
}

#[test]
fn a_long_subject_wraps_under_the_toolbar() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, None, "Photos from the coast walk");
    full_width(&mut harness, "Full width");
    harness.set_size(vec2(320.0, 800.0));
    harness.run();
    let subject = harness.get_by_label("Photos from the coast walk").rect();
    let more = harness.get_by_role_and_label(Role::Button, "More").rect();
    assert!(subject.top() >= more.bottom(), "the subject sits under the toolbar, not beside it");
    assert!(subject.height() >= 60.0, "it wraps onto a second line: {subject:?}");
}

#[test]
fn the_subject_is_set_large_with_the_message_s_group_as_a_chip_beside_who_wrote() {
    let fetch = Recorder::default();
    let mut app = app_with(&fetch);
    let showcase = sample::showcase(app.mailbox());
    app.mailbox_mut().set_showcase(showcase);
    let id = app.mailbox().messages().iter().find(|row| row.subject == "Allotment rota for November").unwrap().id;
    app.mailbox_mut().select(Some(id));
    let harness = harness_with(app, egui::Theme::Light);
    let theme = crate::theme::Theme::current(&harness.ctx, &crate::theme::Look::of(harness.state().mailbox().appearance()));
    let size = |wanted: &str| {
        fn find(shape: &egui::Shape, wanted: &str) -> Option<f32> {
            match shape {
                egui::Shape::Vec(inner) => inner.iter().find_map(|shape| find(shape, wanted)),
                egui::Shape::Text(text) if text.galley.text() == wanted => {
                    text.galley.job.sections.first().map(|section| section.format.font_id.size)
                }
                _ => None,
            }
        }
        harness.output().shapes.iter().find_map(|clipped| find(&clipped.shape, wanted))
    };
    assert_eq!(size("Allotment rota for November"), Some(theme.fonts.title.size), "the subject is in the title style");
    // The list's group is called so too: the reader's tag is the one at the right.
    let tag = harness.query_all_by_label("People").map(|node| node.rect()).max_by(|a, b| a.left().total_cmp(&b.left())).unwrap();
    let subject_line = harness.get_by_label("Allotment rota for November");
    assert!(tag.bottom() <= subject_line.rect().top(), "the group's tag heads the reader, over the subject");
}

/// Runs a few frames, so a press, the act, the list read again and the toast all show.
fn settle(harness: &mut Harness<'static, InboxApp>) {
    for _ in 0..4 {
        harness.run();
    }
}

fn row_of(harness: &Harness<'static, InboxApp>, id: u64) -> Option<crate::mailbox::MessageRow> {
    harness.state().mailbox().messages().iter().find(|row| row.id == id).cloned()
}

#[test]
fn opening_a_message_marks_it_read_and_ctrl_z_marks_it_unread_again() {
    let fetch = Recorder::default();
    let mut app = app_with(&fetch);
    let id = app.mailbox().messages().iter().find(|row| row.subject == "Allotment rota for November").unwrap().id;
    assert_eq!(app.mailbox().messages().iter().find(|row| row.id == id).map(|row| row.seen), Some(false));
    app.mailbox_mut().select(Some(id));
    let mut harness = harness_with(app, egui::Theme::Light);
    assert_eq!(row_of(&harness, id).map(|row| row.seen), Some(true), "opened, it is read");
    harness.get_by_label("Odile Brannock, Allotment rota for November, 08:35");
    harness.key_press_modifiers(egui::Modifiers::COMMAND, Key::Z);
    settle(&mut harness);
    assert_eq!(row_of(&harness, id).map(|row| row.seen), Some(false), "Ctrl+Z takes the read back");
    assert_eq!(harness.state().reader().open_id(), Some(id), "and the message stays open");
}

#[test]
fn archive_in_more_moves_the_open_message_to_the_archive_and_undo_brings_it_back() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, None, "Photos from the coast walk");
    let id = harness.state().reader().open_id().unwrap();
    // The design puts Archive first in More, with its key.
    harness.get_by_label("More").click();
    harness.run();
    harness.get_by_role_and_label(Role::MenuItem, "Archive").click();
    settle(&mut harness);
    assert!(row_of(&harness, id).is_none(), "it left the inbox");
    harness.get_by_label("Moved to Archive");
    harness.get_by_role_and_label(Role::Button, "Undo").click();
    settle(&mut harness);
    assert!(row_of(&harness, id).is_some());
    assert_eq!(harness.state().reader().open_id(), Some(id), "open again, as it was");
}

#[test]
fn move_to_a_folder_lists_the_folders_of_the_message_s_account_and_moves_it() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, None, "Photos from the coast walk");
    let id = harness.state().reader().open_id().unwrap();
    harness.get_by_role_and_label(Role::Button, "Move to a folder").click();
    harness.run();
    harness.get_by_label("Move to");
    for name in ["Archive", "Receipts", "Travel/2026"] {
        harness.get_by_role_and_label(Role::MenuItem, name);
    }
    assert!(harness.query_by_role_and_label(Role::MenuItem, "INBOX").is_none(), "not the folder it is in");
    assert!(harness.query_by_role_and_label(Role::MenuItem, "Clients").is_none(), "not another account's");
    harness.get_by_role_and_label(Role::MenuItem, "Receipts").click();
    settle(&mut harness);
    assert!(row_of(&harness, id).is_none());
    harness.get_by_label("Moved to Receipts");
    harness.key_press_modifiers(egui::Modifiers::COMMAND, Key::Z);
    settle(&mut harness);
    assert!(row_of(&harness, id).is_some(), "Ctrl+Z brings it back");
}

#[test]
fn move_to_a_folder_never_offers_a_folder_that_holds_no_mail() {
    use inboxcraft_model::{AccountId, Folder, FolderId, FolderListing, FolderPath};
    use inboxcraft_store::{Batch, Change, Storage as _};
    // A real account whose only other folder is a `\Noselect` parent.
    let mut store = sample::mailbox(Timestamp::from_unix(1_793_869_200, 0).unwrap()).unwrap();
    let mut batch = Batch::new();
    for id in 11..=15 {
        batch.push(Change::RemoveFolder(FolderId::new(id)));
    }
    let listing = FolderListing::new(FolderPath::new("Projects", Some('/')).unwrap(), Vec::new(), false);
    batch.push(Change::PutFolder(Folder::new(FolderId::new(40), AccountId::new(2), listing)));
    store.apply(&batch).unwrap();
    let mut app = InboxApp::with_outside(
        Mailbox::new(inboxcraft_engine::Session::new(Box::new(store), clock()).unwrap(), clock()),
        Box::new(Recorder::default()),
        Box::new(NoSave),
    );
    let id = app.mailbox().messages().iter().find(|row| row.subject == "Draft logo for the bakery").unwrap().id;
    assert!(app.mailbox().folders_of(2).iter().any(|folder| folder.name == "Projects" && !folder.selectable));
    assert!(app.mailbox().move_targets(id).is_empty(), "Projects holds no mail");
    app.mailbox_mut().select(Some(id));
    let harness = harness_with(app, egui::Theme::Light);
    assert!(harness.get_by_role_and_label(Role::Button, "Move to a folder").accesskit_node().is_disabled());
}

#[test]
fn more_flags_the_open_message_and_takes_the_flag_off() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, None, "Allotment rota for November");
    let id = harness.state().reader().open_id().unwrap();
    for (name, flagged, told) in [("Flag", true, "Flagged"), ("Take the flag off", false, "Flag taken off")] {
        harness.get_by_role_and_label(Role::Button, "More").click();
        harness.run();
        harness.get_by_role_and_label(Role::MenuItem, name).click();
        settle(&mut harness);
        assert_eq!(row_of(&harness, id).map(|row| row.flagged), Some(flagged), "{name}");
        harness.get_by_label(told);
    }
}

#[test]
fn more_marks_the_open_message_unread_and_read_again() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, None, "Allotment rota for November");
    let id = harness.state().reader().open_id().unwrap();
    for (name, seen, told) in [("Mark as unread", false, "Marked as unread"), ("Mark as read", true, "Marked as read")] {
        harness.get_by_role_and_label(Role::Button, "More").click();
        harness.run();
        harness.get_by_role_and_label(Role::MenuItem, name).click();
        settle(&mut harness);
        assert_eq!(row_of(&harness, id).map(|row| row.seen), Some(seen), "{name}");
        harness.get_by_label(told);
    }
}

#[test]
fn delete_in_the_trash_deletes_for_good_with_a_toast_without_undo() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, Some(13), "Studio Collective news");
    let id = harness.state().reader().open_id().unwrap();
    harness.get_by_role_and_label(Role::Button, "Delete").click();
    settle(&mut harness);
    assert!(row_of(&harness, id).is_none());
    harness.get_by_label("Deleted for good");
    assert!(harness.query_by_role_and_label(Role::Button, "Undo").is_none(), "nothing to undo it with");
}

#[test]
fn a_thread_s_header_is_the_name_and_whom_it_went_to_and_details_stays_reachable() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, Some(3), "Re: Allotment rota for November");
    assert!(harness.query_by_label_contains("From Mira Holloway").is_none(), "no facts under the resting line");
    harness.get_by_label_contains("Mira Holloway, 08:40, open: mira@example.org  ·  to ");
    harness.get_by_role_and_label(Role::Button, "details ▾").click();
    harness.run();
    harness.get_by_label_contains("From Mira Holloway  <mira@example.org>");
    harness.get_by_role_and_label(Role::Button, "hide details ▴");
}

#[test]
fn a_message_alone_in_its_thread_has_no_header_and_more_shows_the_full_one() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, None, "Photos from the coast walk");
    assert!(
        harness
            .query_all_by_role(Role::Image)
            .all(|node| node.accesskit_node().label().as_deref() != Some("Teodor Lisk") || node.rect().left() < 500.0)
    );
    assert!(harness.query_by_label("to Mira Holloway").is_none());
    harness.get_by_label("More").click();
    harness.run();
    harness.get_by_label("Show the full header").click();
    harness.run();
    harness.get_by_role_and_label(Role::Button, "hide details ▴").click();
    harness.run();
    harness.get_by_label_contains("From Teodor Lisk  <teodor@example.org>");
}

#[test]
fn the_reader_keeps_to_a_narrow_window() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, Some(3), "Re: Allotment rota for November");
    harness.set_size(vec2(420.0, 420.0));
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Reply");
    let mut harness = opened(&fetch, Some(5), "You have won a lighthouse");
    harness.set_size(vec2(420.0, 420.0));
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Load once");
    harness.get_by_role_and_label(Role::Button, "Always for prize-desk@example.org");
}

/// How wide and how high the text `label` comes out in a message made of `html`, in a reader 800 points wide.
fn drawn_text(html: &str, label: &str) -> (f32, f32) {
    let options = inboxcraft_html::Options::default();
    let document = inboxcraft_html::display(html, &options);
    let theme = crate::theme::Theme::new(crate::theme::Mode::Light, crate::theme::TextScale::default());
    let reader = crate::reader::Reader::new(Box::<crate::fetch::Offline>::default(), Box::new(NoSave));
    let mut harness = Harness::builder().with_size(vec2(800.0, 400.0)).with_context_setup(crate::theme::install_fonts).build_ui(move |ui| {
        let mut textures = super::reader::Textures::default();
        let mut drawn = super::reader::Drawn::default();
        super::reader::blocks(ui, &theme, &reader, 1, &document, &document.blocks, 0, &mut textures, &mut drawn);
    });
    harness.run();
    let rect = harness.get_by_label(label).rect();
    (rect.width() as f32, rect.height() as f32)
}

#[test]
fn a_tables_text_keeps_its_line_instead_of_breaking_at_the_smallest_column() {
    let text = "The words of the first column stay on one line";
    let (width, height) = drawn_text(&format!("<table><tr><td>{text}</td><td>Second</td></tr></table>"), text);
    assert!(width > 200.0, "the column came out {width} points wide");
    assert!(height < 30.0, "the text came out {height} points high");
}

#[test]
fn columns_that_fit_get_what_they_ask_for() {
    assert_eq!(super::reader::column_widths(&[100.0, 50.0], 800.0, 10.0), [100.0, 50.0]);
}

#[test]
fn a_long_column_gives_way_and_the_narrow_one_keeps_its_width() {
    assert_eq!(super::reader::column_widths(&[2000.0, 80.0], 500.0, 20.0), [400.0, 80.0]);
}

#[test]
fn columns_never_pass_the_room_however_little_there_is() {
    for room in [0.0, 1.0, 37.5, 300.0] {
        let widths = super::reader::column_widths(&[900.0, 900.0, 5.0, 0.0], room, 12.0);
        assert!(widths.iter().all(|width| *width >= 0.0), "{widths:?}");
        assert!(widths.iter().sum::<f32>() <= room + 0.001, "{widths:?} in {room}");
    }
    assert!(super::reader::column_widths(&[], 300.0, 12.0).is_empty());
}

/// Whether `font` is a face of IBM Plex Sans (the families `theme` gives it) or of IBM Plex Mono.
fn plex(font: &egui::FontId) -> &'static str {
    use crate::theme::Face;
    let named = |face: Face| egui::FontFamily::Name(face.name().into());
    match &font.family {
        egui::FontFamily::Proportional => "sans",
        egui::FontFamily::Monospace => "mono",
        family if *family == named(Face::SansSemiBold) => "sans",
        family if *family == named(Face::MonoSemiBold) => "mono",
        _ => "other",
    }
}

#[test]
fn people_s_words_are_in_plex_sans_and_the_app_s_in_plex_mono() {
    let fetch = Recorder::default();
    let harness = opened(&fetch, None, "Photos from the coast walk");
    let font = |start: &str| super::tests::font_of(&harness, start);
    for people in ["Teodor Lisk", "Here are the ones from the cliff path"] {
        assert_eq!(plex(&font(people)), "sans", "{people}");
    }
    for app in ["Reply", "Forward", "Show done", "04:00", "4"] {
        assert_eq!(plex(&font(app)), "mono", "{app}");
    }
}

#[test]
fn the_quote_fold_is_a_link_that_enter_and_space_press() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, Some(3), "Re: Allotment rota for November");
    harness.get_by_role_and_label(Role::Button, "Show 8 quoted lines").focus();
    harness.run();
    harness.key_press(Key::Enter);
    harness.run();
    harness.get_by_label("Odile Brannock wrote:");
    harness.get_by_role_and_label(Role::Button, "Hide quoted text").focus();
    harness.run();
    harness.key_press(Key::Space);
    harness.run();
    assert!(harness.query_by_label("Odile Brannock wrote:").is_none());
}

#[test]
fn the_notice_s_dismiss_is_a_link_that_space_presses() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, Some(5), "You have won a lighthouse");
    let link = harness.get_by_label("https://bank.example.org/login").rect();
    harness.hover_at(link.left_center() + vec2(8.0, 0.0));
    harness.run();
    harness.get_by_label("https://bank.example.org/login").click();
    harness.run();
    harness.get_by_label("Link copied: https://prize-desk.example.net/claim");
    harness.get_by_role_and_label(Role::Button, "Dismiss").focus();
    harness.run();
    harness.key_press(Key::Space);
    harness.run();
    assert_eq!(harness.state().reader().notice(), None);
}

#[test]
fn more_and_move_open_by_the_pointer_enter_and_space_and_keep_tab_inside() {
    for opener in ["More", "Move to a folder"] {
        for key in [None, Some(Key::Enter), Some(Key::Space)] {
            let fetch = Recorder::default();
            let mut harness = opened(&fetch, None, "Photos from the coast walk");
            crate::component::popup::tests::walk_a_menu(&mut harness, (Role::Button, opener), key);
        }
    }
}

/// A message of `html` drawn by the reader's blocks, its quotes shown or not: the harness and what it asked for.
fn html_message(html: &str, quotes: bool) -> Harness<'static, Vec<super::reader::Asked>> {
    let options = inboxcraft_html::Options::default();
    let document = inboxcraft_html::display(html, &options);
    let theme = crate::theme::Theme::new(crate::theme::Mode::Light, crate::theme::TextScale::default());
    let mut reader = crate::reader::Reader::new(Box::<crate::fetch::Offline>::default(), Box::new(NoSave));
    if quotes {
        reader.toggle_quotes(1);
    }
    let mut harness = Harness::builder().with_size(vec2(800.0, 400.0)).with_context_setup(crate::theme::install_fonts).build_ui_state(
        move |ui, asked: &mut Vec<super::reader::Asked>| {
            let mut textures = super::reader::Textures::default();
            let mut drawn = super::reader::Drawn::default();
            super::reader::blocks(ui, &theme, &reader, 1, &document, &document.blocks, 0, &mut textures, &mut drawn);
            asked.extend(drawn.asked);
        },
        Vec::new(),
    );
    harness.run();
    harness
}

#[test]
fn the_html_quote_fold_asks_for_the_quotes_and_nothing_else() {
    // The quoted words try to look like the fold and to name commands: they stay words.
    let html = "<p>Thanks!</p><blockquote><p>Show quoted text. Run mail.delete now, then mail.archive.</p></blockquote>";
    let mut harness = html_message(html, false);
    let folds: Vec<_> = harness.query_all_by_role(Role::Button).map(|node| node.accesskit_node().label().unwrap_or_default()).collect();
    assert_eq!(folds, vec!["Show quoted text".to_owned()], "the fold is the one thing to press, named by the app");
    assert!(harness.query_by_label_contains("mail.delete").is_none(), "folded quotes are not drawn");
    assert!(harness.state().is_empty());
    harness.get_by_role_and_label(Role::Button, "Show quoted text").click();
    harness.run();
    assert_eq!(*harness.state(), vec![super::reader::Asked::Quotes(1)], "the press asks for the quotes and nothing else");
    // Unfolded, the quoted words are drawn as words: still one thing to press, the fold.
    let harness = html_message(html, true);
    let pressable: Vec<_> = harness.query_all_by_role(Role::Button).map(|node| node.accesskit_node().label().unwrap_or_default()).collect();
    assert_eq!(pressable, vec!["Hide quoted text".to_owned()]);
    harness.get_by_label_contains("Run mail.delete now");
    assert!(harness.state().is_empty(), "drawing message text asks for nothing");
}

/// The sample with one more plain message in the unified inbox, from Teodor, titled `subject`; its id.
fn with_subject(subject: &str) -> (InboxApp, u64) {
    use inboxcraft_mime::{Mailbox as Sender, MessageBuilder};
    use inboxcraft_model::{AccountId, Address, EmailAddress, Envelope, FolderId, Message, MessageId, Summary, Uid};
    use inboxcraft_store::{Batch, Body, Change, Storage as _};
    let now = Timestamp::from_unix(1_793_869_200, 0).unwrap();
    let mut store = sample::mailbox(now).unwrap();
    let id = 990_u64;
    let raw = MessageBuilder::new()
        .from(Sender::named("Teodor Vasquez", "teodor@example.org"))
        .to(Sender::new("mira@example.org"))
        .subject(subject)
        .text("Two directions attached.")
        .build()
        .unwrap();
    let from = Address::new(Some("Teodor Vasquez"), EmailAddress::new("teodor@example.org").unwrap());
    let received = Timestamp::from_unix(1_793_869_200 - 60, 0);
    let envelope = Envelope::new(subject).with_from(vec![from]).with_date(received);
    let summary = Summary::new(Uid::new(990).unwrap(), envelope).with_received(received);
    let message = Message::new(MessageId::new(id), AccountId::new(1), FolderId::new(1), summary);
    let batch =
        Batch::new().with(Change::PutMessage(message)).with(Change::PutBody(MessageId::new(id), Body::new(raw, "Two directions attached.").unwrap()));
    store.apply(&batch).unwrap();
    let fetch = Recorder::default();
    let app = InboxApp::with_outside(Mailbox::new(sample::session_over(store, clock()).unwrap(), clock()), Box::new(fetch), Box::new(NoSave));
    (app, id)
}

/// The texts painted inside `rect` in the last frame: `(rect, rows, elided, size)`.
fn texts_in(harness: &Harness<'static, InboxApp>, rect: egui::Rect) -> Vec<(egui::Rect, usize, bool, f32)> {
    fn walk(shape: &egui::Shape, rect: egui::Rect, found: &mut Vec<(egui::Rect, usize, bool, f32)>) {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, rect, found)),
            egui::Shape::Text(text) => {
                let at = egui::Rect::from_min_size(text.pos, text.galley.size());
                if rect.expand(1.0).contains_rect(at) {
                    let size = text.galley.job.sections.first().map_or(0.0, |section| section.format.font_id.size);
                    found.push((at, text.galley.rows.len(), text.galley.elided, size));
                }
            }
            _ => {}
        }
    }
    let mut found = Vec::new();
    harness.output().shapes.iter().for_each(|clipped| walk(&clipped.shape, rect, &mut found));
    found
}

#[test]
fn a_subject_takes_two_lines_then_an_ellipsis() {
    let long = "Draft logo for the bakery on the corner of the market square, the second round of sketches with the colours we \
                talked about on Thursday and the shop front";
    let spaceless: String = "Bakerylogo".repeat(40);
    for (subject, percent) in [(long.to_owned(), 100), (spaceless.clone(), 130), (long.to_owned(), 130)] {
        let (mut app, id) = with_subject(&subject);
        app.set_text_scale(crate::theme::TextScale::from_percent(percent).unwrap());
        app.mailbox_mut().select(Some(id));
        let harness = harness_with(app, egui::Theme::Dark);
        let theme = crate::theme::Theme::current(&harness.ctx, &crate::theme::Look::of(harness.state().mailbox().appearance()));
        let title = theme.fonts.title;
        // A screen reader hears the subject whole, as the reader keeps it.
        let kept = harness.state().reader().opened(id).unwrap().header.subject.clone();
        assert!(subject.starts_with(&kept), "{kept}");
        let node = harness.get_by_label(&kept);
        let rect = node.rect();
        let runs: Vec<(egui::Rect, usize, bool, f32)> = texts_in(&harness, rect).into_iter().filter(|text| text.3 == title.size).collect();
        let at = format!("{percent} %: {runs:#?}");
        assert_eq!(runs.len(), 1, "one run: {at}");
        let (drawn, rows, elided, _) = runs[0];
        assert_eq!((rows, elided), (2, true), "two lines, the second cut with an ellipsis: {at}");
        assert!((rect.height() - 2.0 * title.line).abs() < 0.5, "as high as two lines: {rect:?} {at}");
        assert!(drawn.right() <= rect.right() + 0.5, "nothing passes the head's width: {at}");
    }
}

#[test]
fn after_done_the_reader_shows_nothing_open() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, None, "Photos from the coast walk");
    let done = harness.state().reader().open_id().unwrap();
    assert_eq!(harness.state().mailbox().after(), After::Next, "the rule that selects the next row");
    // The reader's pane: its parts stand 40 in from its sides.
    let pane = (harness.get_by_label("Photos from the coast walk").rect().left() - 40.0, harness.get_by_label("More").rect().right() + 40.0);
    harness.get_by_role_and_label(Role::Button, "Done").click();
    settle(&mut harness);
    assert!(row_of(&harness, done).is_none(), "it left the list");
    // The list selects no row (the nothing-open frame), and no message opens by itself.
    assert_eq!(harness.state().mailbox().selected(), None);
    assert_eq!(harness.state().reader().open_id(), None, "no message opens by itself");
    let next = harness.state().mailbox().messages().first().map(|row| row.id).unwrap();
    assert!(harness.query_by_role_and_label(Role::Button, "Reply").is_none());
    // The bare empty state, its lines centred in the reader's pane.
    let label = harness.get_by_label("Pick a message.\nIt opens here.").rect();
    // The help in the frame's words, with the keys the owner has.
    fn texts(shape: &egui::Shape, found: &mut String) {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| texts(shape, found)),
            egui::Shape::Text(text) => found.push_str(&text.galley.job.text),
            _ => {}
        }
    }
    let mut painted = String::new();
    harness.output().shapes.iter().for_each(|clipped| texts(&clipped.shape, &mut painted));
    assert!(painted.contains("J and K move in the list, Enter opens"), "{painted}");
    assert!(painted.contains("Ctrl K runs any"), "{painted}");
    let pane_middle = (pane.0 + pane.1) / 2.0;
    assert!((label.center().x - pane_middle).abs() <= 1.0, "in the middle of the reader's pane: {label:?} {pane:?}");
    // A row picked by a click opens again.
    let subject = row_of(&harness, next).unwrap().subject;
    row_node(&harness, &subject).click();
    settle(&mut harness);
    assert_eq!(harness.state().reader().open_id(), Some(next));
}

/// The row of the list whose spoken words hold `subject`.
fn row_node<'h>(harness: &'h Harness<'static, InboxApp>, subject: &str) -> egui_kittest::Node<'h> {
    harness
        .query_all_by_role(Role::ListBoxOption)
        .find(|node| node.accesskit_node().label().is_some_and(|label| label.contains(subject)))
        .unwrap_or_else(|| panic!("no row of {subject}"))
}

/// Whether a row of the list is selected.
fn any_selected(harness: &Harness<'static, InboxApp>) -> bool {
    harness.query_all_by_role(Role::ListBoxOption).any(|node| node.accesskit_node().is_selected() == Some(true))
}

/// The subjects of the list, in order.
fn subjects(harness: &Harness<'static, InboxApp>) -> Vec<String> {
    harness.state().mailbox().messages().iter().map(|row| row.subject.clone()).collect()
}

/// The subject of the open message, if one is.
fn open_subject(harness: &Harness<'static, InboxApp>) -> Option<String> {
    let id = harness.state().reader().open_id()?;
    row_of(harness, id).map(|row| row.subject)
}

/// Done on the open message by its button, and the frames after it.
fn done(harness: &mut Harness<'static, InboxApp>) {
    // The reader's Done: a row picked by a key shows its own hover actions in the list, left of it.
    let reader = harness.query_all_by_role_and_label(Role::Button, "Done").max_by(|a, b| a.rect().left().total_cmp(&b.rect().left()));
    reader.unwrap().click();
    settle(harness);
    assert_eq!(harness.state().reader().open_id(), None, "nothing opens by itself");
}

#[test]
fn with_nothing_open_no_row_is_selected_and_the_row_keys_act_on_nothing() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, None, "Photos from the coast walk");
    done(&mut harness);
    assert_eq!(harness.state().mailbox().selected(), None, "the list selects no row, as the nothing-open frame draws it");
    assert!(!any_selected(&harness), "no row is drawn selected");
    let before = subjects(&harness);
    let seen: Vec<bool> = harness.state().mailbox().messages().iter().map(|row| row.seen || row.flagged).collect();
    // Done, Archive, read or unread, flag, Delete: none lands on a message that is not shown.
    for key in [Key::D, Key::A, Key::R, Key::S, Key::Delete] {
        harness.key_press(key);
        settle(&mut harness);
        assert_eq!(subjects(&harness), before, "{key:?} acted on a row that is not shown");
        assert_eq!(harness.state().reader().open_id(), None, "{key:?}: still nothing opens by itself");
    }
    let after: Vec<bool> = harness.state().mailbox().messages().iter().map(|row| row.seen || row.flagged).collect();
    assert_eq!(after, seen, "no row was read or flagged");
    assert!(!any_selected(&harness));
}

#[test]
fn after_a_second_done_no_message_opens_by_itself() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, None, "Photos from the coast walk");
    done(&mut harness);
    // J opens the message the rule names, Done on it, and still nothing opens.
    harness.key_press(Key::J);
    settle(&mut harness);
    assert!(harness.state().reader().open_id().is_some(), "J opens a row");
    done(&mut harness);
    assert!(!any_selected(&harness));
    // The arrows open a row too.
    harness.key_press(Key::ArrowDown);
    settle(&mut harness);
    assert!(harness.state().reader().open_id().is_some(), "the down arrow opens a row");
    done(&mut harness);
    harness.key_press(Key::K);
    settle(&mut harness);
    assert!(harness.state().reader().open_id().is_some(), "K opens a row");
    done(&mut harness);
    harness.key_press(Key::ArrowUp);
    settle(&mut harness);
    assert!(harness.state().reader().open_id().is_some(), "the up arrow opens a row");
}

#[test]
fn the_next_message_rule_names_what_a_key_opens_after_a_message_left() {
    for after in [After::Next, After::Previous] {
        for key in [Key::Enter, Key::J, Key::ArrowDown, Key::K, Key::ArrowUp] {
            let fetch = Recorder::default();
            let mut harness = opened(&fetch, None, "Photos from the coast walk");
            let list = subjects(&harness);
            let at = list.iter().position(|subject| subject == "Photos from the coast walk").unwrap();
            // The rows each side of the one that leaves.
            let (before, behind) = (list[at - 1].clone(), list[at + 1].clone());
            harness.state_mut().mailbox_mut().set_after(after);
            done(&mut harness);
            let wanted = match (key, after) {
                // Enter opens the message the rule names: the next one, or the one before.
                (Key::Enter, After::Next) | (Key::J | Key::ArrowDown, _) => &behind,
                _ => &before,
            };
            harness.key_press(key);
            settle(&mut harness);
            assert_eq!(open_subject(&harness).as_deref(), Some(wanted.as_str()), "{after:?} {key:?}");
        }
    }
}

#[test]
fn reply_is_the_one_primary_button() {
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let fetch = Recorder::default();
        let mut app = app_with(&fetch);
        let id = app.mailbox().messages().iter().find(|row| row.subject == "Photos from the coast walk").unwrap().id;
        app.mailbox_mut().select(Some(id));
        let harness = harness_with(app, theme);
        let look = crate::theme::Theme::current(&harness.ctx, &crate::theme::Look::of(harness.state().mailbox().appearance()));
        let cyan = look.palette.cyan;
        fn filled(shape: &egui::Shape, cyan: egui::Color32, found: &mut Vec<egui::Rect>) {
            match shape {
                egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| filled(shape, cyan, found)),
                egui::Shape::Rect(rect) if rect.fill == cyan && rect.rect.height() >= 20.0 => found.push(rect.rect),
                _ => {}
            }
        }
        let mut primary = Vec::new();
        harness.output().shapes.iter().for_each(|clipped| filled(&clipped.shape, cyan, &mut primary));
        let reply = harness.get_by_role_and_label(Role::Button, "Reply").rect();
        // The reader's pane: 40 left of its parts.
        primary.retain(|rect| rect.left() >= reply.left() - 40.0);
        assert_eq!(primary, vec![reply], "{theme:?}: the reader's one cyan button is Reply");
        for other in ["Reply all", "Forward"] {
            assert!(!primary.contains(&harness.get_by_role_and_label(Role::Button, other).rect()), "{other}");
        }
        // Reply carries its arrow: wider than its word and the room beside it.
        assert!(reply.width() > 70.0, "{reply:?}");
        // The answers stand at the foot, under the message.
        let more = harness.get_by_role_and_label(Role::Button, "More").rect();
        assert!(reply.top() > more.bottom() + 200.0, "{reply:?}");
    }
}

/// The scan lines in `cyan` the last frame painted across the reader (right of the list): their heights on the screen.
fn scan_lines(harness: &Harness<'static, InboxApp>, cyan: egui::Color32) -> Vec<f32> {
    fn walk(shape: &egui::Shape, cyan: egui::Color32, found: &mut Vec<f32>) {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, cyan, found)),
            egui::Shape::LineSegment { points, stroke }
                if stroke.color == cyan && points[0].y == points[1].y && points[1].x - points[0].x > 200.0 && points[0].x > 600.0 =>
            {
                found.push(points[0].y);
            }
            _ => {}
        }
    }
    let mut found = Vec::new();
    harness.output().shapes.iter().for_each(|clipped| walk(&clipped.shape, cyan, &mut found));
    found
}

#[test]
fn a_message_wipes_in_behind_a_scan_line_and_is_there_at_once_with_reduced_motion() {
    for reduced in [false, true] {
        let fetch = Recorder::default();
        let mut app = app_with(&fetch);
        app.set_reduce_motion(reduced);
        let id = app.mailbox().messages().iter().find(|row| row.subject == "Photos from the coast walk").unwrap().id;
        let mut harness = harness_with(app, egui::Theme::Dark);
        let look = crate::theme::Theme::current(&harness.ctx, &crate::theme::Look::of(harness.state().mailbox().appearance()));
        harness.state_mut().mailbox_mut().select(Some(id));
        harness.step();
        let lines = scan_lines(&harness, look.palette.cyan);
        if reduced {
            assert!(lines.is_empty(), "at once with reduced motion: {lines:?}");
        } else {
            assert_eq!(lines.len(), 1, "the scan line while it wipes in: {lines:?}");
        }
        harness.run();
        assert!(scan_lines(&harness, look.palette.cyan).is_empty(), "{reduced}: gone once it is in");
        assert!(crate::component::tests::rests(&mut harness, 4), "{reduced}: no frame asked for at rest");
        harness.get_by_role_and_label(Role::Button, "Reply");
    }
}

#[test]
fn the_wipe_runs_on_the_design_s_curve() {
    let ease = super::reader::ease;
    assert_eq!((ease(0.0), ease(1.0)), (0.0, 1.0));
    assert!(ease(0.5) > 0.8, "a quick start: {}", ease(0.5));
    let mut last = 0.0;
    for step in 1..=100 {
        let now = ease(step as f32 / 100.0);
        assert!(now >= last, "it never goes back");
        last = now;
    }
    assert_eq!(ease(f32::NAN), 1.0, "a value that is no number lands at the end");
}

/// The rects of `colour`'s strokes the frame painted.
fn stroked(harness: &Harness<'static, InboxApp>, colour: egui::Color32) -> Vec<egui::Rect> {
    fn walk(shape: &egui::Shape, colour: egui::Color32, found: &mut Vec<egui::Rect>) {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, colour, found)),
            egui::Shape::Rect(rect) if rect.stroke.color == colour && rect.stroke.width > 0.0 => found.push(rect.rect),
            _ => {}
        }
    }
    let mut found = Vec::new();
    harness.output().shapes.iter().for_each(|clipped| walk(&clipped.shape, colour, &mut found));
    found
}

/// Presses Tab until the node `label` has the keyboard; whether it got it.
fn tab_to(harness: &mut Harness<'static, InboxApp>, label: &str) -> bool {
    for _ in 0..80 {
        harness.key_press(Key::Tab);
        harness.run();
        if harness.query_by_label(label).is_some_and(|node| node.accesskit_node().is_focused()) {
            return true;
        }
    }
    false
}

#[test]
fn the_subject_is_a_tab_stop_with_a_ring_and_enter_or_the_menu_key_copies_it() {
    const SUBJECT: &str = "Photos from the coast walk";
    for key in [Key::Enter, Key::Menu] {
        let fetch = Recorder::default();
        let mut harness = opened(&fetch, None, SUBJECT);
        assert!(tab_to(&mut harness, SUBJECT), "Tab reaches the subject");
        let look = crate::theme::Theme::current(&harness.ctx, &crate::theme::Look::of(harness.state().mailbox().appearance()));
        let subject = harness.get_by_label(SUBJECT).rect();
        let ring = stroked(&harness, look.palette.cyan_edge);
        assert!(
            ring.iter().any(|ring| ring.contains_rect(egui::Rect::from_min_size(subject.min, vec2(40.0, subject.height())))
                && (ring.left() - subject.left()).abs() <= 3.0),
            "{key:?}: the subject shows its focus ring: {ring:?} {subject:?}"
        );
        harness.key_press(key);
        harness.run();
        harness.get_by_label("Copy the subject");
        // The first Tab lands on the menu's first row.
        harness.key_press(Key::Tab);
        harness.run();
        harness.key_press(Key::Enter);
        settle(&mut harness);
        // The copy says so in the reader's notice (the harness keeps the clipboard's text to itself).
        harness.get_by_label("Subject copied");
        assert!(harness.query_by_label("Copy the subject").is_none(), "{key:?}: the menu closed");
    }
}

/// The labels Tab gives the keyboard to, in order, from the first press until it comes back round.
fn tab_order(harness: &mut Harness<'static, InboxApp>) -> Vec<String> {
    let mut order: Vec<String> = Vec::new();
    let mut seen = Vec::new();
    for _ in 0..300 {
        harness.key_press(Key::Tab);
        harness.run();
        let Some(node) = harness.root().children_recursive().find(|node| node.accesskit_node().is_focused()) else { continue };
        let id = node.accesskit_node().locate().0;
        if seen.contains(&id) {
            break;
        }
        seen.push(id);
        order.push(node.accesskit_node().label().unwrap_or_default());
    }
    order
}

#[test]
fn tab_goes_from_the_head_through_the_message_to_the_answers() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, None, "Draft logo for the bakery");
    let order = tab_order(&mut harness);
    let at = |label: &str| order.iter().position(|reached| reached.contains(label)).unwrap_or_else(|| panic!("{label} not reached: {order:?}"));
    let (head, message, reply) = (at("More"), at("sketch-2.png"), at("Reply"));
    assert!(head < message && message < reply, "head, then the message, then the answers: {order:?}");
    assert!(at("Reply all") > message && at("Forward") > message, "{order:?}");
}
