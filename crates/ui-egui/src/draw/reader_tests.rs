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
    InboxApp::with_outside_at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock()), Box::new(fetch.clone()), Box::new(NoSave))
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
    harness.get_by_label("Show mail of: mira@example.org");
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
        harness.get_by_label("load once").click();
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
    // The index is the frame's and stays; the reader fills the body.
    let body = super::shell::laid(&harness.ctx).unwrap().body;
    assert!(harness.query_by_label("Show mail of: mira@example.org").is_some(), "the index stays");
    let reader = super::reader_rect(&harness.ctx).unwrap();
    assert!(reader.left() <= body.left() + 1.0 && reader.right() >= body.right() - 1.0, "the reader fills the body: {reader:?} in {body:?}");
    assert!(harness.query_by_label("the inbox").is_none(), "and the list with its head");
    harness.key_press(Key::Escape);
    harness.run();
    assert!(!harness.state().reader().full_width());
    harness.get_by_role(Role::TextInput);
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
    harness.get_by_label("load once").click();
    harness.run();
    assert_eq!(*fetch.asked.lock().unwrap(), ["https://pictures.example.org/cycles/workshop.png"]);
    harness.run();
    harness.get_by_role_and_label(Role::Image, "The workshop bench");
    assert!(harness.query_by_label("load once").is_none(), "the banner is gone");
}

#[test]
fn always_for_this_sender_remembers_the_sender() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, Some(5), "You have won a lighthouse");
    assert!(fetch.asked.lock().unwrap().is_empty());
    harness.get_by_label("always for prize-desk@example.org").click();
    harness.run();
    assert_eq!(fetch.asked.lock().unwrap().len(), 1);
    let (_, mailbox) = harness.state_mut().reader_mut();
    assert_eq!(mailbox.run("remote.list", &json!({})).unwrap()["senders"], json!(["prize-desk@example.org"]));
}

#[test]
fn a_texture_goes_with_its_picture() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, Some(5), "You have won a lighthouse");
    harness.get_by_label("always for prize-desk@example.org").click();
    harness.run();
    let url = "https://track.example.net/open.png?id=4417";
    assert!(harness.state().textures.contains_key(url), "the fetched picture is drawn");
    let gone = harness.ctx.load_texture("gone", egui::ColorImage::filled([1, 1], egui::Color32::WHITE), egui::TextureOptions::LINEAR);
    harness.state_mut().textures.insert("https://pictures.example.org/gone.png".to_owned(), gone);
    harness.run();
    assert!(!harness.state().textures.contains_key("https://pictures.example.org/gone.png"), "no picture, no texture");
    assert!(harness.state().textures.contains_key(url));
}

/// A picture that is here but cannot be drawn is a box that says its data could not be drawn, with no `try again`:
/// trying again would get the same bytes.
#[test]
fn a_picture_that_cannot_be_drawn_offers_no_try_again() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, Some(5), "You have won a lighthouse");
    harness.get_by_label("always for prize-desk@example.org").click();
    harness.run();
    let url = "https://track.example.net/open.png?id=4417";
    let broken = crate::picture::Decoded { width: 2, height: 2, rgba: vec![0; 3] };
    harness.state_mut().reader_mut().0.put_picture(url, crate::reader::PictureState::Ready(std::sync::Arc::new(broken)));
    settle(&mut harness);
    harness.get_by_label_contains("Picture not shown: its data could not be drawn");
    assert!(harness.query_by_label_contains("does not draw this kind").is_none(), "its own reason, not another kind's");
    assert!(harness.query_by_label("try again").is_none(), "a try again that can do nothing");
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
    harness.get_by_label("dismiss").click();
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
    // The facts are heard as one line: who wrote, to whom, and the date, exactly.
    let facts = harness.get_by_label_contains("From Mira Holloway  <mira@example.org>, to ").accesskit_node().label().unwrap_or_default();
    assert!(facts.starts_with("From Mira Holloway  <mira@example.org>, to ") && facts.ends_with(", Thursday 5 Nov 2026, 08:40"), "{facts}");
    harness.get_by_label("hide details ▴").click();
    harness.run();
}

/// A save that keeps nothing and says it went to the downloads.
struct Kept;

impl crate::save::Save for Kept {
    fn save(&mut self, name: &str, _bytes: &[u8]) -> Result<String, String> {
        Ok(format!("Downloads/{name}"))
    }
}

#[test]
fn saving_an_attachment_says_where_it_went_or_why_not_on_its_box() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, None, "Draft logo for the bakery");
    assert!(super::tests::drew_icon(harness.output(), IconId::File), "the tile shows the file icon");
    harness.get_by_label("Save sketch-2.png").click();
    harness.run();
    // The box says why not, in its own words, and offers to try again; the reader's notice stays quiet.
    let failed = harness.get_by_label_contains("sketch-2.png, ").accesskit_node().label().unwrap_or_default();
    assert!(failed.ends_with("not saved: Saving attachments is not available in this window."), "{failed}");
    assert_eq!(harness.state().reader().notice(), None);
    harness.get_by_label("Save sketch-2.png again").click();
    harness.run();
    harness.get_by_label("Save sketch-2.png again");
    // A save that works says where the file went.
    let mut app = InboxApp::with_outside_at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock()), Box::new(fetch.clone()), Box::new(Kept));
    let id = app.mailbox().messages().iter().find(|row| row.subject == "Draft logo for the bakery").unwrap().id;
    app.mailbox_mut().select(Some(id));
    let mut harness = harness_with(app, egui::Theme::Light);
    settle(&mut harness);
    harness.get_by_label("Save sketch-2.png").click();
    harness.run();
    let saved = harness.get_by_label_contains("sketch-2.png, ").accesskit_node().label().unwrap_or_default();
    assert!(saved.ends_with("saved to Downloads/sketch-2.png"), "{saved}");
    assert!(harness.query_by_label("Save sketch-2.png").is_none(), "saved: no save link");
    // The other box is untouched.
    assert!(harness.query_by_label("Save sketch-1.png").is_some());
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
    // The More menu was the reader's: the list's keys rest until a row is pressed or keyed. A press on the open row
    // gives the list the keyboard.
    let before = harness.state().mailbox().selected();
    harness.key_press(Key::ArrowDown);
    harness.run();
    assert_eq!(harness.state().mailbox().selected(), before, "the arrows rest while the reader has the keyboard");
    harness.query_all_by_label_contains("Allotment rota for November").next().unwrap().click();
    harness.run();
    // The next row as the list shows it.
    let order = super::list::order(&harness.ctx);
    let before = harness.state().mailbox().selected().unwrap();
    let next = order.iter().position(|id| *id == before).and_then(|at| order.get(at + 1)).copied();
    harness.key_press(Key::ArrowDown);
    harness.run();
    let selected = harness.state().mailbox().selected();
    assert_eq!(selected, next);
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
fn home_pressed_over_an_open_message_shows_home_and_mail_shows_the_message_again() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, None, "Photos from the coast walk");
    let open = harness.state().reader().open_id();
    assert!(open.is_some());
    harness.get_by_role_and_label(Role::Button, "Home").click();
    harness.run();
    assert_eq!(harness.state().screen(), crate::Screen::Home, "HOME is a place");
    assert!(harness.query_by_label("Photos from the coast walk").is_none(), "Home stands in the mail's place");
    harness.get_by_role_and_label(Role::Button, "Mail").click();
    harness.run();
    assert_eq!((harness.state().screen(), harness.state().reader().open_id()), (crate::Screen::Mail, open), "the message as it was left");
}

#[test]
fn done_in_the_toolbar_marks_the_open_message_done_offers_undo_and_walks_on_to_the_next() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, None, "Photos from the coast walk");
    let id = harness.state().reader().open_id().unwrap();
    let list = subjects(&harness);
    let behind = list[list.iter().position(|subject| subject == "Photos from the coast walk").unwrap() + 1].clone();
    harness.get_by_role_and_label(Role::Button, "Done").click();
    for _ in 0..4 {
        harness.run();
    }
    assert!(harness.state().mailbox().messages().iter().all(|row| row.id != id), "it left the inbox");
    harness.get_by_label("Marked as done");
    // The owner's own act walks on, as the lists do: the message the next-message rule names opens.
    assert_eq!(open_subject(&harness).as_deref(), Some(behind.as_str()), "the next message opens");
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
    harness.get_by_label("Moved to Bin");
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
    // The window is laid out 800 wide at least: the full-width reader is the body's 566, where this subject fits on
    // one line, so whether it wraps is no longer seen here.
}

#[test]
fn the_subject_is_set_large_under_the_message_s_group_tag() {
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
    harness.get_by_role_and_label(Role::Button, "undo").click();
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
    let mut app = InboxApp::with_outside_at_mail(
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
    assert!(harness.query_by_role_and_label(Role::Button, "undo").is_none(), "nothing to undo it with");
}

#[test]
fn a_thread_s_header_is_the_name_and_whom_it_went_to_and_details_stays_reachable() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, Some(3), "Re: Allotment rota for November");
    assert!(harness.query_by_label_contains("From Mira Holloway").is_none(), "no facts under the resting line");
    harness.get_by_label_contains("Mira Holloway, 08:40, open: mira@example.org  ·  to ");
    // One line, with its avatar, for each message: Mira wrote one (the avatar is the line's, heard in its name).
    assert_eq!(harness.query_all_by_label_contains("Mira Holloway, 08:40").count(), 1);
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
    // A window this narrow leaves the reader a sliver: nothing of the banner paints past the window.
    for (rect, text) in painted_texts(&harness) {
        assert!(rect.left() < 420.0 || !text.contains("remote picture"), "{text:?} at {rect:?} paints past the window");
    }
    // A reader narrower than the banner's links on one line: each link stands whole on a line of its own, inside it.
    harness.set_size(vec2(800.0, 420.0));
    settle(&mut harness);
    let banner = harness.get_by_label_contains("Pictures: ").rect();
    let once = harness.get_by_role_and_label(Role::Button, "load once").rect();
    let always = harness.get_by_role_and_label(Role::Button, "always for prize-desk@example.org").rect();
    for link in [once, always] {
        assert!(banner.contains_rect(link), "{link:?} stands inside the banner {banner:?}");
    }
    // Where the reader is narrower than the two links on one line they wrap; the window is laid out 800 wide at least,
    // and the one-pane reader (the body's 566) holds both on one line.
    assert!(once.bottom() <= always.top() || (once.top() - always.top()).abs() < 0.5, "the links on one line or wrapped: {once:?} {always:?}");
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
    harness.get_by_role_and_label(Role::Button, "dismiss").focus();
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
    assert_eq!(folds, vec!["Show 1 quoted line".to_owned()], "the fold is the one thing to press, named by the app");
    assert!(harness.query_by_label_contains("mail.delete").is_none(), "folded quotes are not drawn");
    assert!(harness.state().is_empty());
    harness.get_by_role_and_label(Role::Button, "Show 1 quoted line").click();
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
    let app = InboxApp::with_outside_at_mail(Mailbox::new(sample::session_over(store, clock()).unwrap(), clock()), Box::new(fetch), Box::new(NoSave));
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
fn after_the_open_message_is_taken_away_the_reader_shows_nothing_open() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, None, "Photos from the coast walk");
    let done = harness.state().reader().open_id().unwrap();
    assert_eq!(harness.state().mailbox().after(), After::Next, "the rule that selects the next row");
    // The reader's pane: its parts stand 40 in from its sides.
    let pane = (harness.get_by_label("Photos from the coast walk").rect().left() - 40.0, harness.get_by_label("More").rect().right() + 40.0);
    taken_away(&mut harness);
    assert!(row_of(&harness, done).is_none(), "it left the list");
    // The list selects no row (the nothing-open frame), and no message opens by itself.
    assert_eq!(harness.state().mailbox().selected(), None);
    assert_eq!(harness.state().reader().open_id(), None, "no message opens by itself");
    let next = harness.state().mailbox().messages().first().map(|row| row.id).unwrap();
    assert!(harness.query_by_role_and_label(Role::Button, "Reply").is_none());
    // The bare empty state, its lines centred in the reader's pane.
    let label = harness.get_by_label("Pick a message.\nIt opens here.").rect();
    // The help says how a message opens from here: the lists' J, K and Enter, as the frames write it.
    fn texts(shape: &egui::Shape, found: &mut String) {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| texts(shape, found)),
            egui::Shape::Text(text) => found.push_str(&text.galley.job.text),
            _ => {}
        }
    }
    let mut painted = String::new();
    harness.output().shapes.iter().for_each(|clipped| texts(&clipped.shape, &mut painted));
    // The frames' words, the keys from the keymap (#328).
    assert!(painted.contains("J and K move in the list, Enter opens the message. Ctrl K runs any"), "{painted}");
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

/// The open message taken away by something other than the owner's act in this window (another device, a sync, the
/// control channel): its row leaves the list, and nothing opens by itself.
fn taken_away(harness: &mut Harness<'static, InboxApp>) {
    let id = harness.state().reader().open_id().unwrap();
    let (_, mailbox) = harness.state_mut().reader_mut();
    mailbox.run("mail.done", &json!({ "id": id, "done": true })).unwrap();
    mailbox.refresh();
    settle(harness);
    assert!(row_of(harness, id).is_none(), "it left the list");
    assert_eq!(harness.state().reader().open_id(), None, "nothing opens by itself");
}

#[test]
fn with_nothing_open_no_row_is_selected_and_the_row_keys_act_on_nothing() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, None, "Photos from the coast walk");
    taken_away(&mut harness);
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
fn after_a_second_removal_no_message_opens_by_itself() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, None, "Photos from the coast walk");
    taken_away(&mut harness);
    // The down arrow opens a row, Done on it, and still nothing opens.
    harness.key_press(Key::ArrowDown);
    settle(&mut harness);
    assert!(harness.state().reader().open_id().is_some(), "the down arrow opens a row");
    taken_away(&mut harness);
    assert!(!any_selected(&harness));
    // The message the rule names opens from the palette's command, and again Done leaves nothing open.
    let ctx = harness.ctx.clone();
    harness.state_mut().run_action(&ctx, crate::keymap::OPEN_HELD);
    settle(&mut harness);
    assert!(harness.state().reader().open_id().is_some(), "the rule's message opens");
    taken_away(&mut harness);
    harness.key_press(Key::ArrowDown);
    settle(&mut harness);
    assert!(harness.state().reader().open_id().is_some(), "the down arrow opens a row again");
    taken_away(&mut harness);
    harness.key_press(Key::ArrowUp);
    settle(&mut harness);
    assert!(harness.state().reader().open_id().is_some(), "the up arrow opens a row");
}

#[test]
fn the_next_message_rule_names_what_a_key_opens_after_a_message_left() {
    for after in [After::Next, After::Previous] {
        // `None`: the command that opens the message the rule names (the lists' Enter, once they land).
        for key in [None, Some(Key::ArrowDown), Some(Key::ArrowUp)] {
            let fetch = Recorder::default();
            let mut harness = opened(&fetch, None, "Photos from the coast walk");
            let list = subjects(&harness);
            let at = list.iter().position(|subject| subject == "Photos from the coast walk").unwrap();
            // The rows each side of the one that leaves.
            let (before, behind) = (list[at - 1].clone(), list[at + 1].clone());
            harness.state_mut().mailbox_mut().set_after(after);
            taken_away(&mut harness);
            let wanted = match (key, after) {
                // The command opens the message the rule names: the next one, or the one before; the arrows open the
                // row each side of the gap.
                (None, After::Next) | (Some(Key::ArrowDown), _) => &behind,
                _ => &before,
            };
            match key {
                Some(key) => harness.key_press(key),
                None => {
                    let ctx = harness.ctx.clone();
                    harness.state_mut().run_action(&ctx, crate::keymap::OPEN_HELD);
                }
            }
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
fn the_scan_line_stays_whole_until_nine_tenths_of_the_time_then_fades() {
    let scan = super::reader::scan;
    // As the presentation's keyframes: whole until 90 % of the time, then out over the last tenth.
    for t in [0.0, 0.3, 0.6, 0.89] {
        assert_eq!(scan(t), 1.0, "whole at {t}");
    }
    let fading = [0.92, 0.95, 0.98].map(scan);
    assert!(fading.iter().all(|alpha| *alpha > 0.0 && *alpha < 1.0) && fading.windows(2).all(|pair| pair[1] < pair[0]), "{fading:?}");
    assert_eq!((scan(1.0), scan(f32::NAN)), (0.0, 0.0));
}

#[test]
fn a_key_or_a_press_during_the_wipe_takes_it_to_its_end() {
    let key = egui::Event::Key { key: Key::F20, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE };
    let press = egui::Event::PointerButton {
        pos: egui::pos2(10.0, 10.0),
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    };
    for (what, event) in [("nothing", None), ("a key", Some(key)), ("a press", Some(press))] {
        let ctx = egui::Context::default();
        let frame = |time: f64, events: Vec<egui::Event>| {
            let mut got = (0.0, 0.0);
            let mut output =
                ctx.run_ui(egui::RawInput { time: Some(time), events, ..Default::default() }, |ui| got = super::reader::wiped(ui.ctx(), 7));
            output.textures_delta.clear();
            got
        };
        assert_eq!(frame(0.0, Vec::new()), (0.0, 0.0), "{what}: the wipe starts");
        let (time, shown) = frame(0.05, event.into_iter().collect());
        if what == "nothing" {
            assert!(time < 1.0 && shown < 1.0, "it runs on: {time} {shown}");
        } else {
            assert_eq!((time, shown), (1.0, 1.0), "{what} took it to its end");
        }
    }
}

#[test]
fn the_wipe_runs_on_the_design_s_curve() {
    let ease = crate::motion::ease;
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

/// Puts the keyboard on the reader's first Tab stop, so a walk that means the open message starts there: Tab onto a
/// row of the list opens that row's message (the lists' preview), so a walk from the window's first stop reaches the
/// reader with another message open.
pub(super) fn into_the_reader(harness: &mut Harness<'static, InboxApp>) {
    let Some(reader) = super::reader_rect(&harness.ctx) else { return };
    let first = harness
        .root()
        .children_recursive()
        .filter(|node| {
            let data = node.accesskit_node();
            data.data().supports_action(egui::accesskit::Action::Focus)
                && !data.is_hidden()
                && data.role() != Role::Label
                && data.role() != Role::ScrollBar
                && reader.contains_rect(node.rect())
        })
        .min_by(|a, b| a.rect().top().total_cmp(&b.rect().top()).then(a.rect().left().total_cmp(&b.rect().left())))
        .map(|node| node.accesskit_node().locate().0);
    if let Some(first) = first {
        if let Some(node) = harness.root().children_recursive().find(|node| node.accesskit_node().locate().0 == first) {
            node.focus();
        }
        harness.run();
    }
}

/// Presses Tab until the node `label` has the keyboard; whether it got it.
fn tab_to(harness: &mut Harness<'static, InboxApp>, label: &str) -> bool {
    into_the_reader(harness);
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
    into_the_reader(harness);
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

#[test]
fn more_and_move_hang_with_their_right_edge_on_their_button_s_right_edge() {
    // The frames: More 288 wide from 1111 to 1399 under More (1367 to 1399), Move 256 wide from 1063 to 1319 under Move
    // (1287 to 1319), 4 under the buttons. A row runs 1 in from the menu's edge.
    for (button, row, width) in [("More", "Archive", 288.0), ("Move to a folder", "Receipts", 256.0)] {
        let fetch = Recorder::default();
        let mut harness = opened(&fetch, None, "Photos from the coast walk");
        let opener = harness.get_by_role_and_label(Role::Button, button).rect();
        harness.get_by_role_and_label(Role::Button, button).click();
        harness.run();
        let item = harness.get_by_role_and_label(Role::MenuItem, row).rect();
        assert!((item.right() + 1.0 - opener.right()).abs() <= 0.5, "{button}: the menu's right edge on the button's: {item:?} {opener:?}");
        assert!((item.left() - 1.0 - (opener.right() - width)).abs() <= 0.5, "{button}: {width} wide: {item:?} {opener:?}");
    }
}

#[test]
fn more_and_move_keep_the_design_s_width_at_every_text_size() {
    // The owner's answer: every menu keeps its fixed width, as the design draws it; at 130 % text a row's words wrap
    // or are cut, the menu does not grow.
    for scale in crate::theme::TextScale::ALL {
        for (button, row, width, id) in
            [("More", "Archive", 288.0, super::reader::more_menu()), ("Move to a folder", "Receipts", 256.0, super::reader::move_menu())]
        {
            let fetch = Recorder::default();
            let mut harness = opened_at(&fetch, "Photos from the coast walk", 1440.0, scale);
            harness.get_by_role_and_label(Role::Button, button).click();
            harness.run();
            let menu = harness.ctx.memory(|memory| memory.area_rect(id)).unwrap();
            assert!((menu.width() - width).abs() <= 0.5, "{scale:?} {button}: the menu {width} wide: {menu:?}");
            let item = harness.get_by_role_and_label(Role::MenuItem, row).rect();
            let rows: Vec<_> = harness.query_all_by_role(Role::MenuItem).map(|node| node.rect()).collect();
            for rect in &rows {
                assert!(rect.width() <= width - 2.0 + 0.5, "{scale:?} {button}: a row wider than the menu: {rect:?} of {rows:?}");
            }
            assert!((item.width() - (width - 2.0)).abs() <= 0.5, "{scale:?} {button}: {width} wide: {item:?}");
        }
    }
}

/// The app with `subject` open in a window `width` wide, its text at `scale`.
fn opened_at(fetch: &Recorder, subject: &str, width: f32, scale: crate::theme::TextScale) -> Harness<'static, InboxApp> {
    let mut app = app_with(fetch);
    let id = app.mailbox().messages().iter().find(|row| row.subject == subject).unwrap().id;
    app.mailbox_mut().select(Some(id));
    app.set_text_scale(scale);
    let mut harness = Harness::builder()
        .with_size(vec2(width, 800.0))
        .with_theme(egui::Theme::Dark)
        .with_max_steps(12)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    settle(&mut harness);
    harness
}

#[test]
fn the_attachments_coming_soon_tag_never_takes_a_line_of_its_own() {
    for scale in [crate::theme::TextScale::default(), crate::theme::TextScale::ALL[4]] {
        for width in [1024.0, 1100.0, 1180.0, 1280.0, 1440.0] {
            let fetch = Recorder::default();
            let harness = opened_at(&fetch, "Draft logo for the bakery", width, scale);
            let boxes: Vec<egui::Rect> = harness
                .query_all_by_role(Role::Button)
                .filter(|node| node.accesskit_node().label().is_some_and(|label| label.starts_with("sketch-")))
                .map(|node| node.rect())
                .collect();
            let Some(last) = boxes.iter().max_by(|a, b| a.top().total_cmp(&b.top()).then(a.left().total_cmp(&b.left()))) else { continue };
            let first = boxes.iter().map(|rect| rect.top()).fold(f32::INFINITY, f32::min);
            let tags: Vec<egui::Rect> =
                harness.query_all_by_label("coming soon").map(|node| node.rect()).filter(|rect| rect.top() >= first - 1.0).collect();
            if width == 1440.0 && scale == crate::theme::TextScale::default() {
                assert_eq!(tags.len(), 1, "a wide reader shows the tag once beside the boxes");
            }
            for tag in tags {
                assert!(
                    (tag.center().y - last.center().y).abs() <= 1.0 && tag.left() > last.right(),
                    "{scale:?} {width}: the tag stands beside the last box: {tag:?} {last:?}"
                );
            }
        }
    }
}

#[test]
fn delete_and_more_stay_in_a_narrow_reader_at_130_percent() {
    const SUBJECT: &str = "Photos from the coast walk";
    for scale in [crate::theme::TextScale::default(), crate::theme::TextScale::ALL[4]] {
        for width in [1440.0, 1100.0, 1024.0, 980.0] {
            let fetch = Recorder::default();
            let mut harness = opened_at(&fetch, SUBJECT, width, scale);
            // The subject runs to the pane's inner right edge.
            let subject = harness.get_by_label(SUBJECT).rect();
            let at = format!("{scale:?} {width}");
            for name in ["Done", "Snooze", "Pin", "Move to a folder", "Delete", "More"] {
                let rect = harness
                    .query_all_by_role_and_label(Role::Button, name)
                    .map(|node| node.rect())
                    .max_by(|a, b| a.left().total_cmp(&b.left()))
                    .unwrap();
                assert!(rect.right() <= width - 1.0 && rect.left() >= subject.left() - 0.5, "{name} stands in the pane: {rect:?} {subject:?} {at}");
                assert!(rect.bottom() <= subject.top(), "{name} stands over the subject: {rect:?} {subject:?} {at}");
            }
            harness.query_all_by_role_and_label(Role::Button, "More").max_by(|a, b| a.rect().left().total_cmp(&b.rect().left())).unwrap().click();
            harness.run();
            assert!(harness.query_by_role_and_label(Role::MenuItem, "Archive").is_some(), "More opens: {at}");
        }
    }
}

#[test]
fn the_why_line_says_the_engines_reasons_asked_once_for_many_frames() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, None, "Photos from the coast walk");
    let reasons = harness.state().reader.why().to_vec();
    assert!(!reasons.is_empty(), "the engine says why");
    let asked = harness.state().reader.asked_why;
    let runs = harness.state().mailbox.runs();
    for _ in 0..20 {
        harness.step();
    }
    assert_eq!(harness.state().reader.asked_why, asked, "asked when the message opened, never per frame");
    assert_eq!(harness.state().mailbox.runs(), runs, "no command per frame");
    // The line says them, heard as one sentence, under the facts and over the body.
    let name = crate::component::why_line::spoken(&reasons);
    let line = harness.get_by_label(&name).rect();
    let subject = harness.get_by_label("Photos from the coast walk").rect();
    assert!(line.top() >= subject.bottom(), "{line:?} {subject:?}");
    // Nothing of the message's own text is in it.
    for made_up in ["Photos", "coast walk"] {
        assert!(!name.contains(made_up), "{made_up}: {name}");
    }
}

#[test]
fn with_no_reason_the_head_keeps_no_line_for_it() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, None, "Photos from the coast walk");
    let name = crate::component::why_line::spoken(harness.state().reader.why());
    let line = harness.get_by_label(&name).rect();
    let tools_before: Vec<egui::Rect> =
        harness.query_all_by_role(Role::Button).map(|node| node.rect()).filter(|rect| rect.bottom() <= line.top()).collect();
    // The first thing the reader shows under the line.
    let first = harness
        .root()
        .children_recursive()
        .filter(|node| node.accesskit_node().bounding_box().is_some())
        .filter_map(|node| node.accesskit_node().label().map(|label| (label, node.rect())))
        .filter(|(_, rect)| rect.top() >= line.bottom() && rect.left() >= line.left() - 0.5 && rect.right() <= line.right() + 0.5)
        .min_by(|a, b| a.1.top().total_cmp(&b.1.top()));
    let (first, at) = first.expect("something under the line");
    // The engine has nothing to say (the message is gone): no line, the head above it unmoved, what was below moves up.
    harness.state_mut().reader.set_why(Vec::new());
    harness.run();
    assert!(harness.query_by_label(&name).is_none(), "no line");
    assert!(harness.query_all_by_label_contains("Why here").next().is_none(), "no line at all");
    let tools_after: Vec<egui::Rect> =
        harness.query_all_by_role(Role::Button).map(|node| node.rect()).filter(|rect| rect.bottom() <= line.top()).collect();
    assert_eq!(tools_before, tools_after, "the head above stays where it was");
    let now = harness.get_by_label(&first).rect();
    assert!(now.top() <= at.top() - line.height(), "{first:?} moves up by the line at least: {now:?} {at:?}");
}

/// Tab reaches every control of the scene, and the scene rests (asks for no repaint) once it is still.
fn walks_and_rests(harness: &mut Harness<'static, InboxApp>, scene: &str) {
    into_the_reader(harness);
    let mut missed = crate::component::tests::keyboard_walk(harness);
    // The toast's Undo (after Done) is its key's, Ctrl Z, while the toast shows: the toast is no Tab stop anywhere.
    missed.retain(|label| label != "undo");
    assert!(missed.is_empty(), "{scene}: Tab never reaches {missed:?}");
    assert!(crate::component::tests::rests(harness, 8), "{scene}: it repaints at rest");
}

#[test]
fn every_reader_scene_is_walked_by_the_keyboard_and_rests() {
    let fetch = Recorder::default();
    for (scene, folder, subject) in [
        ("a plain message", None, "Photos from the coast walk"),
        ("a thread", Some(3), "Re: Allotment rota for November"),
        ("attachments", None, "Draft logo for the bakery"),
    ] {
        let mut harness = opened(&fetch, folder, subject);
        walks_and_rests(&mut harness, scene);
    }
    // An open quote.
    let mut harness = opened(&fetch, Some(3), "Re: Allotment rota for November");
    harness.get_by_label("Show 8 quoted lines").click();
    harness.run();
    harness.get_by_label("Odile Brannock wrote:");
    walks_and_rests(&mut harness, "an open quote");
    // Nothing open.
    let mut harness = opened(&fetch, None, "Photos from the coast walk");
    taken_away(&mut harness);
    walks_and_rests(&mut harness, "nothing open");
    // Full width.
    let mut harness = opened(&fetch, None, "Photos from the coast walk");
    full_width(&mut harness, "Full width");
    assert!(harness.state().reader().full_width());
    walks_and_rests(&mut harness, "full width");
    // The screener's preview of held mail.
    let mut harness = harness_with(app_with(&fetch), egui::Theme::Light);
    harness.get_by_role_and_label(Role::Button, "Review").click();
    harness.run();
    harness.get_by_label("Harbour Swim Club, Winter swim at the harbour steps, 06:00, unread, new sender").click();
    harness.run();
    harness.get_by_label("Let Harbour Swim Club in?");
    walks_and_rests(&mut harness, "a held message");
    // A sent message that bounced, under its banner.
    let mut store = sample::mailbox(Timestamp::from_unix(1_793_869_200, 0).unwrap()).unwrap();
    sample::bounce(&mut store).unwrap();
    let app = InboxApp::with_outside_at_mail(
        Mailbox::new(sample::session_over(store, clock()).unwrap(), clock()),
        Box::new(fetch.clone()),
        Box::new(NoSave),
    );
    let mut harness = harness_with(app, egui::Theme::Light);
    harness.state_mut().mailbox_mut().open(crate::mailbox::Place::View(inboxcraft_model::Role::Sent));
    harness.run();
    harness.get_by_label_contains("not delivered").click();
    harness.run();
    harness.get_by_label_contains("Not delivered: This message did not arrive: petra@example.org: 550 5.1.1");
    walks_and_rests(&mut harness, "a bounced message");
    // The reader's notice, at the top of the message.
    let mut harness = opened(&fetch, None, "Draft logo for the bakery");
    harness.state_mut().reader_mut().0.set_notice("Subject copied".to_owned());
    harness.run();
    walks_and_rests(&mut harness, "a notice");
    // Remote pictures held back: the banner and the boxes in their place.
    let mut harness = opened(&fetch, Some(5), "You have won a lighthouse");
    harness.get_by_label_contains("Pictures: ");
    walks_and_rests(&mut harness, "remote pictures");
    // The states without text, and a cut text.
    for (scene, content, cut) in [
        ("not kept yet", crate::reader::Content::NotKept, false),
        ("too large", crate::reader::Content::TooLarge, false),
        ("unreadable", crate::reader::Content::Unreadable, false),
        ("cut", crate::reader::Content::Text(vec![crate::quote::Segment::Text("The first lines.".to_owned())]), true),
    ] {
        let mut harness = opened_as(&fetch, "Photos from the coast walk", content, cut);
        walks_and_rests(&mut harness, scene);
    }
    // Loading: walked; its progress mark turns every 125 ms by design (M9), so it is no scene at rest.
    let mut harness = opened_as(&fetch, "Photos from the coast walk", crate::reader::Content::Loading, false);
    into_the_reader(&mut harness);
    let missed = crate::component::tests::keyboard_walk(&mut harness);
    assert!(missed.is_empty(), "loading: Tab never reaches {missed:?}");
    // An attachment saved, and one that failed.
    let mut harness = opened(&fetch, None, "Draft logo for the bakery");
    harness.get_by_label("Save sketch-2.png").click();
    harness.run();
    walks_and_rests(&mut harness, "an attachment that failed");
    let mut app = InboxApp::with_outside_at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock()), Box::new(fetch.clone()), Box::new(Kept));
    let id = app.mailbox().messages().iter().find(|row| row.subject == "Draft logo for the bakery").unwrap().id;
    app.mailbox_mut().select(Some(id));
    let mut harness = harness_with(app, egui::Theme::Light);
    settle(&mut harness);
    harness.get_by_label("Save sketch-2.png").click();
    harness.run();
    walks_and_rests(&mut harness, "an attachment saved");
    // The invitations: the update that asks, the version a later one replaced.
    use inboxcraft_store::Storage as _;
    let now = Timestamp::from_unix(1_793_869_200, 0).unwrap();
    for subject in ["Updated invitation: Lunch with Odile", "Invitation: Lunch with Odile"] {
        let mut store = sample::mailbox(now).unwrap();
        let mut batch = inboxcraft_store::Batch::new();
        for change in sample::invitations(now).unwrap() {
            batch.push(change);
        }
        store.apply(&batch).unwrap();
        let mut app = InboxApp::with_outside_at_mail(
            Mailbox::new(sample::session_over(store, clock()).unwrap(), clock()),
            Box::new(fetch.clone()),
            Box::new(NoSave),
        );
        let found = app.mailbox_mut().run("mail.search", &json!({ "subject": subject })).unwrap();
        let id = found["messages"].as_array().unwrap().iter().find(|row| row["subject"] == json!(subject)).unwrap()["id"].as_u64().unwrap();
        app.mailbox_mut().select(Some(id));
        let mut harness = harness_with(app, egui::Theme::Light);
        settle(&mut harness);
        assert!(harness.query_all_by_label_contains("Invitation: Lunch with Odile, ").next().is_some(), "{subject}: the card");
        walks_and_rests(&mut harness, subject);
    }
    // An older message unfolded beside the open one, with its line's menu open.
    let mut harness = opened(&fetch, Some(3), "Re: Allotment rota for November");
    harness.get_by_label_contains("Odile Brannock, 08:35, folded").click();
    settle(&mut harness);
    walks_and_rests(&mut harness, "an older message unfolded");
    // The walk went on round the window, through the list's rows, which open theirs: the scene again.
    let mut harness = opened(&fetch, Some(3), "Re: Allotment rota for November");
    harness.get_by_label_contains("Odile Brannock, 08:35, folded").click();
    settle(&mut harness);
    harness.get_by_label_contains("Odile Brannock, 08:35, open: ").click_secondary();
    harness.run();
    harness.get_by_role_and_label(Role::MenuItem, "Show the full header");
    assert!(crate::component::tests::rests(&mut harness, 8), "the open line's menu repaints at rest");
}

#[test]
fn the_reader_s_menus_are_walked_by_the_keyboard_and_rest() {
    for (button, row) in [("More", "Archive"), ("Move to a folder", "Receipts")] {
        let fetch = Recorder::default();
        let mut harness = opened(&fetch, None, "Photos from the coast walk");
        harness.get_by_role_and_label(Role::Button, button).click();
        harness.run();
        harness.get_by_role_and_label(Role::MenuItem, row);
        // Only the menu takes the keyboard while it is open: Tab goes round its rows, each in turn.
        let rows: Vec<String> = harness.query_all_by_role(Role::MenuItem).filter_map(|node| node.accesskit_node().label()).collect();
        let order = tab_order(&mut harness);
        for row in &rows {
            assert!(order.contains(row), "{button}: Tab never reaches {row}: {order:?}");
        }
        for outside in ["Reply", "Snooze", "Pin", "Delete", "Mail"] {
            assert!(!order.iter().any(|label| label == outside), "{button}: the keyboard stays in the menu: {order:?}");
        }
        assert!(harness.query_by_role_and_label(Role::MenuItem, row).is_some(), "{button}: still open");
        assert!(crate::component::tests::rests(&mut harness, 8), "{button}: it repaints at rest");
    }
}

/// The app with `subject` (in `folder`, or the unified inbox) open in a window `width` wide at `ppp` pixels per point,
/// its text at `percent`.
fn opened_in(fetch: &Recorder, folder: Option<u64>, subject: &str, width: f32, percent: u16, ppp: f32) -> Harness<'static, InboxApp> {
    let mut app = app_with(fetch);
    if let Some(folder) = folder {
        app.mailbox_mut().open(crate::mailbox::Place::Folder(folder));
    }
    let id = app.mailbox().messages().iter().find(|row| row.subject == subject).unwrap().id;
    app.mailbox_mut().select(Some(id));
    app.set_text_scale(crate::theme::TextScale::from_percent(percent).unwrap());
    let mut harness = Harness::builder()
        .with_size(vec2(width, 800.0))
        .with_pixels_per_point(ppp)
        .with_theme(egui::Theme::Dark)
        .with_max_steps(12)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    settle(&mut harness);
    harness
}

/// Every text painted in the last frame that shows (not wholly clipped away): its whole galley's rect and its words.
fn painted_texts(harness: &Harness<'static, InboxApp>) -> Vec<(egui::Rect, String)> {
    fn walk(shape: &egui::Shape, clip: egui::Rect, found: &mut Vec<(egui::Rect, String)>) {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, clip, found)),
            egui::Shape::Text(text) => {
                let at = egui::Rect::from_min_size(text.pos, text.galley.size());
                if at.intersect(clip).is_positive() && !text.galley.job.text.trim().is_empty() {
                    found.push((at, text.galley.job.text.clone()));
                }
            }
            _ => {}
        }
    }
    let mut found = Vec::new();
    harness.output().shapes.iter().for_each(|clipped| walk(&clipped.shape, clipped.clip_rect, &mut found));
    found
}

/// The sizes the reader must keep to: a 1024 and a 1440 window, text at 100 and 130 %, at 1x and 1.5x.
const NARROW_SIZES: [(f32, u16, f32); 8] = [
    (1024.0, 100, 1.0),
    (1024.0, 130, 1.0),
    (1024.0, 100, 1.5),
    (1024.0, 130, 1.5),
    (1440.0, 100, 1.0),
    (1440.0, 130, 1.0),
    (1440.0, 100, 1.5),
    (1440.0, 130, 1.5),
];

#[test]
fn a_thread_s_rows_keep_their_words_inside_them_in_a_narrow_reader() {
    for (folder, subject) in [(Some(3), "Re: Allotment rota for November"), (None, "Draft logo for the bakery")] {
        for (width, percent, ppp) in NARROW_SIZES {
            let fetch = Recorder::default();
            let harness = opened_in(&fetch, folder, subject, width, percent, ppp);
            let at = format!("{subject} {width} {percent}% {ppp}x");
            // The rows in sight: above the answers' foot, which the message scrolls under.
            let foot = harness.query_all_by_role_and_label(Role::Button, "Reply").map(|node| node.rect().top()).fold(800.0_f32, f32::min);
            let rows: Vec<egui::Rect> = harness
                .query_all_by_role(Role::Button)
                .filter(|node| node.accesskit_node().label().is_some_and(|label| label.contains(", folded") || label.contains(", open: ")))
                .map(|node| node.rect())
                .filter(|rect| rect.is_positive() && rect.bottom() <= foot)
                .collect();
            if folder.is_some() {
                assert!(rows.len() > 1, "a thread shows its rows: {at}");
            }
            let texts = painted_texts(&harness);
            for row in &rows {
                assert!(row.right() <= width + 0.5, "the row stands in the window: {row:?} {at}");
                // The texts on the row's line: their middle in its height, their run touching it.
                let inside: Vec<&(egui::Rect, String)> =
                    texts.iter().filter(|(rect, _)| row.y_range().contains(rect.center().y) && rect.intersect(*row).area() > 1.0).collect();
                for (rect, text) in &inside {
                    assert!(row.expand(0.5).contains_rect(*rect), "{text:?} at {rect:?} paints past its row {row:?}: {at}");
                }
                for (index, (one, text)) in inside.iter().enumerate() {
                    for (other, word) in inside.iter().skip(index + 1) {
                        assert!(one.intersect(*other).area() <= 1.0, "{text:?} at {one:?} runs over {word:?} at {other:?}: {at}");
                    }
                }
            }
            if let Some(link) = harness.query_by_label("details ▾") {
                let rect = link.rect();
                assert!(rows.iter().any(|row| row.expand(0.5).contains_rect(rect)), "details at {rect:?} stands in its row: {at}");
            }
        }
    }
}

#[test]
fn the_answers_stand_whole_in_the_pane_in_a_narrow_reader() {
    for (width, percent, ppp) in NARROW_SIZES {
        let fetch = Recorder::default();
        let harness = opened_in(&fetch, None, "Photos from the coast walk", width, percent, ppp);
        let at = format!("{width} {percent}% {ppp}x");
        let subject = harness.get_by_label("Photos from the coast walk").rect();
        let texts = painted_texts(&harness);
        let mut boxes = Vec::new();
        for name in ["Reply", "Reply all", "Forward"] {
            let rect = harness.get_by_role_and_label(Role::Button, name).rect();
            // In the pane: between the subject's left edge and the window's right edge, under the message.
            assert!(rect.left() >= subject.left() - 0.5 && rect.right() <= width - 0.5, "{name} at {rect:?} stands in the pane: {at}");
            assert!(rect.bottom() <= 800.0 + 0.5, "{name} at {rect:?} stands in the window: {at}");
            // Its words whole inside its box, never clipped at the pane's edge.
            let words = texts.iter().find(|(text_rect, text)| text == name && rect.contains_rect(*text_rect));
            assert!(words.is_some(), "{name} reads whole inside {rect:?}: {at}");
            for other in &boxes {
                assert!(!rect.intersects(*other), "{name} at {rect:?} runs over another answer at {other:?}: {at}");
            }
            boxes.push(rect);
        }
    }
}

/// The app with the message at `end` of the unified inbox open (`true`: the last row) under the rule `after`; Done on
/// it, then `key`. The subject opened, if any, and the list as it was before Done.
fn done_at_the_end_then(end_last: bool, after: After, key: Key) -> (Option<String>, Vec<String>) {
    let fetch = Recorder::default();
    let app = app_with(&fetch);
    let rows = app.mailbox().messages();
    let subject = if end_last { rows.last() } else { rows.first() }.unwrap().subject.clone();
    drop(app);
    let mut harness = opened(&fetch, None, &subject);
    harness.state_mut().mailbox_mut().set_after(after);
    let list = subjects(&harness);
    taken_away(&mut harness);
    harness.key_press(key);
    settle(&mut harness);
    if harness.state().reader().open_id().is_none() {
        assert_eq!(harness.state().mailbox().selected(), None, "{after:?} {key:?}: nothing selected where nothing opens");
        assert!(!any_selected(&harness), "{after:?} {key:?}: nothing drawn selected");
    }
    (open_subject(&harness), list)
}

#[test]
fn the_last_row_taken_away_then_up_opens_the_row_before_the_gap() {
    for after in [After::Next, After::Previous] {
        let (got, list) = done_at_the_end_then(true, after, Key::ArrowUp);
        let before = &list[list.len() - 2];
        assert_eq!(got.as_deref(), Some(before.as_str()), "{after:?}: up opens the row before the gap");
        // No row followed the one that left: the down arrow opens nothing.
        let (got, _) = done_at_the_end_then(true, after, Key::ArrowDown);
        assert_eq!(got, None, "{after:?}: down at the end opens nothing");
    }
}

#[test]
fn the_first_row_taken_away_then_down_opens_the_row_after_the_gap() {
    for after in [After::Previous, After::Next] {
        let (got, list) = done_at_the_end_then(false, after, Key::ArrowDown);
        assert_eq!(got.as_deref(), Some(list[1].as_str()), "{after:?}: down opens the row after the gap");
        // No row stood before the one that left: the up arrow opens nothing.
        let (got, _) = done_at_the_end_then(false, after, Key::ArrowUp);
        assert_eq!(got, None, "{after:?}: up at the start opens nothing");
    }
}

/// Workers that bring back, at the next pump, the events a test queued: the server's word from outside the window.
#[derive(Clone, Default)]
struct Outside(std::sync::Arc<std::sync::Mutex<Vec<inboxcraft_model::Event>>>);

impl crate::workers::Workers for Outside {
    fn send(&mut self, _account: &inboxcraft_model::Account, _job: inboxcraft_model::Job) {}

    fn keep(&mut self, _accounts: &[inboxcraft_model::AccountId]) {}

    fn events(&mut self) -> Vec<inboxcraft_model::Event> {
        std::mem::take(&mut *self.0.lock().unwrap())
    }
}

#[test]
fn a_key_in_the_frame_the_server_removes_the_open_message_acts_on_no_row() {
    use inboxcraft_model::{AccountId, Event, EventKind, FolderId, FolderState, Uid, UidSet, UidValidity};
    const LEFT: &str = "Your library loan is due on Friday";
    for key in [Key::D, Key::Delete] {
        let fetch = Recorder::default();
        let outside = Outside::default();
        let mut app = app_with(&fetch).with_accounts(
            std::sync::Arc::new(crate::onboard::NoKeychain),
            Box::<crate::onboard::NoDetect>::default(),
            Box::new(outside.clone()),
        );
        let id = app.mailbox().messages().iter().find(|row| row.subject == LEFT).unwrap().id;
        app.mailbox_mut().select(Some(id));
        let mut harness = harness_with(app, egui::Theme::Light);
        settle(&mut harness);
        assert_eq!(harness.state().reader().open_id(), Some(id));
        let before = subjects(&harness);
        let unseen: Vec<(u64, bool)> = harness.state().mailbox().messages().iter().map(|row| (row.id, row.seen)).collect();
        // The server says the open message is gone (as a sync or another device would), and a key lands in that frame.
        let validity = UidValidity::new(1).unwrap();
        let synced = EventKind::FolderSynced { folder: FolderId::new(1), state: FolderState::new(validity) };
        let gone = EventKind::MessagesExpunged { folder: FolderId::new(1), uid_validity: validity, uids: UidSet::from_iter([Uid::new(2).unwrap()]) };
        outside.0.lock().unwrap().extend([Event::new(AccountId::new(1), None, synced), Event::new(AccountId::new(1), None, gone)]);
        harness.key_press(key);
        harness.step();
        assert!(row_of(&harness, id).is_none(), "{key:?}: the message left");
        assert!(!any_selected(&harness), "{key:?}: no row is drawn selected, not even in the frame it left");
        settle(&mut harness);
        let left: Vec<String> = before.iter().filter(|subject| *subject != LEFT).cloned().collect();
        assert_eq!(subjects(&harness), left, "{key:?} acted on a row the person never saw selected");
        assert_eq!(harness.state().reader().open_id(), None, "{key:?}: nothing opens by itself");
        assert_eq!(harness.state().mailbox().selected(), None, "{key:?}: no row selected");
        for (row, seen) in unseen.iter().filter(|(row, _)| *row != id) {
            assert_eq!(row_of(&harness, *row).map(|row| row.seen), Some(*seen), "{key:?}: {row} read unseen");
        }
    }
}

#[test]
fn the_message_the_rule_names_opens_from_the_palette_and_a_bound_key_and_only_after_one_left() {
    const LEAVES: &str = "Photos from the coast walk";
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, None, LEAVES);
    let list = subjects(&harness);
    let behind = list[list.iter().position(|subject| subject == LEAVES).unwrap() + 1].clone();
    assert!(!harness.state().can_run(crate::keymap::OPEN_HELD), "with a message open there is nothing to open");
    // Through the palette: Ctrl K, its name, Enter.
    let through_palette = |harness: &mut Harness<'static, InboxApp>| {
        harness.key_press_modifiers(egui::Modifiers::COMMAND, Key::K);
        harness.run();
        harness.get_by_role_and_label(Role::TextInput, "Command line").type_text(">the rule names");
        harness.run();
        harness.key_press(Key::Enter);
        settle(harness);
    };
    taken_away(&mut harness);
    assert!(harness.state().can_run(crate::keymap::OPEN_HELD), "after a message left it can run");
    through_palette(&mut harness);
    assert!(!harness.state().command().results());
    assert_eq!(open_subject(&harness).as_deref(), Some(behind.as_str()), "the palette opens the message the rule names");
    assert!(!harness.state().can_run(crate::keymap::OPEN_HELD), "once it opened, nothing is held");
    // With nothing held the palette's Enter opens nothing: close the open one by Close, then run it.
    harness.get_by_label("More").click();
    harness.run();
    harness.get_by_label("Close").click();
    settle(&mut harness);
    assert_eq!(harness.state().reader().open_id(), None);
    assert!(!harness.state().can_run(crate::keymap::OPEN_HELD), "a closed reader holds nothing");
    through_palette(&mut harness);
    assert_eq!(harness.state().reader().open_id(), None, "nothing held: the palette opens nothing");
    // Through a key the owner gave it (the lists' Enter, once they land): Done, then the key.
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, None, LEAVES);
    harness.state_mut().keymap_mut().bind(crate::keymap::OPEN_HELD, egui::KeyboardShortcut::new(egui::Modifiers::NONE, Key::Enter));
    taken_away(&mut harness);
    harness.key_press(Key::Enter);
    settle(&mut harness);
    assert_eq!(open_subject(&harness).as_deref(), Some(behind.as_str()), "its key opens the message the rule names");
}

#[test]
fn an_older_unfolded_message_keeps_its_details_at_every_width_the_design_draws() {
    for percent in [100, 130] {
        for ppp in [1.0, 1.5] {
            let fetch = Recorder::default();
            let mut harness = opened_in(&fetch, Some(3), "Re: Allotment rota for November", 1100.0, percent, ppp);
            harness.get_by_label_contains("Odile Brannock, 08:35, folded").click();
            settle(&mut harness);
            let at = format!("1100 {percent}% {ppp}x");
            let open: Vec<egui::Rect> = harness
                .query_all_by_role(Role::Button)
                .filter(|node| node.accesskit_node().label().is_some_and(|label| label.contains(", open: ")))
                .map(|node| node.rect())
                .collect();
            assert_eq!(open.len(), 2, "the older message unfolded beside the open one: {at}");
            for row in &open {
                let details = harness.query_all_by_label("details ▾").filter(|link| row.contains_rect(link.rect())).count();
                if details == 1 {
                    continue;
                }
                // The lists' 441 leave a line at 130 % no room for `details ▾`: its menu is its way to the full header.
                assert_eq!((details, percent), (0, 130), "an open line at {row:?} keeps its way to the full header: {at}");
                let line = harness
                    .query_all_by_role(Role::Button)
                    .find(|node| node.rect() == *row && node.accesskit_node().label().is_some_and(|label| label.contains(", open: ")))
                    .map(|node| node.accesskit_node().locate().0);
                if let Some(node) = harness.root().children_recursive().find(|node| Some(node.accesskit_node().locate().0) == line) {
                    node.focus();
                }
                harness.run();
                harness.key_press_modifiers(egui::Modifiers::SHIFT, Key::F10);
                harness.run();
                assert!(harness.query_by_label("Show the full header").is_some(), "an open line at {row:?} keeps its menu: {at}");
                harness.key_press(Key::Escape);
                harness.run();
            }
        }
    }
}

/// Presses Tab until a node whose label holds `label` has the keyboard; whether it got it.
fn tab_to_containing(harness: &mut Harness<'static, InboxApp>, label: &str) -> bool {
    into_the_reader(harness);
    for _ in 0..120 {
        harness.key_press(Key::Tab);
        harness.run();
        if harness
            .root()
            .children_recursive()
            .any(|node| node.accesskit_node().is_focused() && node.accesskit_node().label().is_some_and(|said| said.contains(label)))
        {
            return true;
        }
    }
    false
}

#[test]
fn an_older_unfolded_message_s_full_header_is_one_key_away_at_every_width() {
    const OLDER: &str = "Odile Brannock, 08:35, open: ";
    // The windows the design draws at 130 %; beside the index the list stands 441 wide in both.
    for (width, wider) in [(1024.0, 0.0), (1100.0, 0.0)] {
        for (key, modifiers) in [(Key::Menu, egui::Modifiers::NONE), (Key::F10, egui::Modifiers::SHIFT)] {
            let fetch = Recorder::default();
            let mut harness = opened_in(&fetch, Some(3), "Re: Allotment rota for November", width, 130, 1.0);
            harness.get_by_label_contains("Odile Brannock, 08:35, folded").click();
            settle(&mut harness);
            let at = format!("{width} +{wider} {key:?}");
            let older = harness.state().reader().thread().iter().find(|row| row.sender == "Odile Brannock").map(|row| row.id).unwrap();
            assert!(!harness.state().reader().shows_details(older), "{at}: closed to start with");
            assert!(tab_to_containing(&mut harness, OLDER), "{at}: Tab reaches the older open line");
            harness.key_press_modifiers(modifiers, key);
            harness.run();
            harness.get_by_role_and_label(Role::MenuItem, "Show the full header");
            // The first Tab lands on the menu's first row.
            harness.key_press(Key::Tab);
            harness.run();
            harness.key_press(Key::Enter);
            settle(&mut harness);
            assert!(harness.state().reader().shows_details(older), "{at}: the menu opened its full header");
            assert!(harness.query_by_role_and_label(Role::MenuItem, "Show the full header").is_none(), "{at}: the menu closed");
            // Again, and it hides it.
            assert!(
                harness
                    .root()
                    .children_recursive()
                    .any(|node| node.accesskit_node().is_focused() && node.accesskit_node().label().is_some_and(|said| said.contains(OLDER))),
                "{at}: the keyboard went back to the line"
            );
            harness.key_press_modifiers(modifiers, key);
            harness.run();
            harness.get_by_role_and_label(Role::MenuItem, "Hide the full header");
            harness.key_press(Key::Tab);
            harness.run();
            harness.key_press(Key::Enter);
            settle(&mut harness);
            assert!(!harness.state().reader().shows_details(older), "{at}: and hid it");
        }
    }
    // The line's menu opens by the lists' menu key as bound: rebound to F2, F2 opens it and Shift F10 no longer does.
    let fetch = Recorder::default();
    let mut harness = opened_in(&fetch, Some(3), "Re: Allotment rota for November", 1440.0, 100, 1.0);
    harness.state_mut().keymap_mut().bind(crate::keymap::MENU, egui::KeyboardShortcut::new(egui::Modifiers::NONE, Key::F2));
    harness.get_by_label_contains("Odile Brannock, 08:35, folded").click();
    settle(&mut harness);
    assert!(tab_to_containing(&mut harness, OLDER), "rebound: Tab reaches the older open line");
    harness.key_press_modifiers(egui::Modifiers::SHIFT, Key::F10);
    harness.run();
    assert!(harness.query_by_role_and_label(Role::MenuItem, "Show the full header").is_none(), "rebound: Shift F10 opens nothing");
    harness.key_press(Key::F2);
    harness.run();
    harness.get_by_role_and_label(Role::MenuItem, "Show the full header");
}

#[test]
fn a_secondary_press_on_an_open_line_opens_its_menu() {
    let fetch = Recorder::default();
    let mut harness = opened_in(&fetch, Some(3), "Re: Allotment rota for November", 1100.0, 130, 1.0);
    // A folded line has no such menu: it folds and unfolds only.
    harness.get_by_label_contains("Odile Brannock, 08:35, folded").click_secondary();
    harness.run();
    assert!(harness.query_by_role(Role::MenuItem).is_none(), "a folded line opens no menu");
    harness.get_by_label_contains("Odile Brannock, 08:35, folded").click();
    settle(&mut harness);
    let older = harness.state().reader().thread().iter().find(|row| row.sender == "Odile Brannock").map(|row| row.id).unwrap();
    harness.get_by_label_contains("Odile Brannock, 08:35, open: ").click_secondary();
    harness.run();
    harness.get_by_role_and_label(Role::MenuItem, "Show the full header").click();
    settle(&mut harness);
    assert!(harness.state().reader().shows_details(older));
}

#[test]
fn the_reader_notice_is_a_banner_after_the_head_in_tab_order() {
    const SAID: &str = "Subject copied";
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, None, "Draft logo for the bakery");
    harness.state_mut().reader_mut().0.set_notice(SAID.to_owned());
    harness.run();
    // A note: the banner's label, then its words, at the top of the message.
    let banner = harness.get_by_label(&format!("Note: {SAID}")).rect();
    let more = harness.get_by_role_and_label(Role::Button, "More").rect();
    assert!(banner.top() > more.bottom(), "the notice stands under the head: {banner:?}");
    let order = tab_order(&mut harness);
    let at = |label: &str| order.iter().position(|reached| reached.starts_with(label)).unwrap_or_else(|| panic!("{label} not reached: {order:?}"));
    assert!(at("More") < at("dismiss") && at("dismiss") < at("sketch-"), "head, the notice, then the message: {order:?}");
    harness.state_mut().reader_mut().0.dismiss_notice();
    harness.run();
    assert!(harness.query_by_label(&format!("Note: {SAID}")).is_none(), "dismissed");
}

/// The notice stands between the head and the message, not in the message's scroll: scrolled to its end, the message
/// leaves the notice where it was, and its `dismiss` still presses.
#[test]
fn the_notice_stays_in_sight_while_the_message_scrolls() {
    const SAID: &str = "Subject copied";
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, None, "Draft logo for the bakery");
    // As high a body as a 420 window's was before the command and status lines took their 74.
    harness.set_size(vec2(1280.0, 494.0));
    harness.state_mut().reader_mut().0.set_notice(SAID.to_owned());
    settle(&mut harness);
    let label = format!("Note: {SAID}");
    let before = harness.get_by_label(&label).rect();
    let file = |harness: &Harness<'static, InboxApp>| {
        harness
            .root()
            .children_recursive()
            .find(|node| node.accesskit_node().label().is_some_and(|said| said.starts_with("sketch-")))
            .map(|node| node.rect())
    };
    let file_before = file(&harness).unwrap();
    let over = egui::pos2(before.center().x, before.bottom() + 60.0);
    harness.hover_at(over);
    harness.run();
    harness.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: vec2(0.0, -10_000.0),
        modifiers: egui::Modifiers::NONE,
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Wheel,
    });
    settle(&mut harness);
    let file_after = file(&harness).unwrap();
    assert!(file_after.top() < file_before.top(), "the message scrolled: {file_before:?} to {file_after:?}");
    assert_eq!(harness.get_by_label(&label).rect(), before, "the notice stays where it was");
    harness.get_by_role_and_label(Role::Button, "dismiss").click();
    settle(&mut harness);
    assert_eq!(harness.state().reader().notice(), None, "dismiss still presses");
}

#[test]
fn a_warning_shows_whole_while_the_message_wipes_in() {
    const SAID: &str = "The message could not be moved: the server refused.";
    let fetch = Recorder::default();
    let mut app = app_with(&fetch);
    app.set_reduce_motion(false);
    let id = app.mailbox().messages().iter().find(|row| row.subject == "Photos from the coast walk").unwrap().id;
    // Frames a sixtieth of a second apart: the wipe is a fifth of the way in.
    let mut harness = Harness::builder()
        .with_size(vec2(1280.0, 800.0))
        .with_theme(egui::Theme::Dark)
        .with_step_dt(1.0 / 60.0)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.step();
    harness.state_mut().mailbox_mut().select(Some(id));
    harness.step();
    harness.state_mut().reader_mut().0.set_notice(SAID.to_owned());
    harness.step();
    let look = crate::theme::Theme::current(&harness.ctx, &crate::theme::Look::of(harness.state().mailbox().appearance()));
    assert_eq!(scan_lines(&harness, look.palette.cyan).len(), 1, "still wiping in");
    // The banner's words are painted and not clipped away by the wipe.
    fn find(shape: &egui::Shape, clip: egui::Rect, found: &mut Vec<(egui::Rect, egui::Rect)>) {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| find(shape, clip, found)),
            egui::Shape::Text(text) if text.galley.job.text.starts_with("The message could not") => {
                found.push((egui::Rect::from_min_size(text.pos, text.galley.size()), clip));
            }
            _ => {}
        }
    }
    let mut found = Vec::new();
    harness.output().shapes.iter().for_each(|clipped| find(&clipped.shape, clipped.clip_rect, &mut found));
    assert_eq!(found.len(), 1, "the notice's words: {found:?}");
    let (words, clip) = found[0];
    assert!(clip.contains_rect(words), "the wipe clips the warning: {words:?} in {clip:?}");
}

/// The app with `subject` open, its text drawn as `content` (and `cut`).
fn opened_as(fetch: &Recorder, subject: &str, content: crate::reader::Content, cut: bool) -> Harness<'static, InboxApp> {
    let mut harness = opened(fetch, None, subject);
    let id = harness.state().reader().open_id().unwrap();
    harness.state_mut().reader_mut().0.show_as(id, content, cut);
    settle(&mut harness);
    harness
}

#[test]
fn a_message_without_its_text_says_why_in_one_sentence_in_the_text_s_place() {
    use crate::reader::Content;
    for (content, said) in [
        (Content::NotKept, "The text of this message is not here yet. It comes with the next connection."),
        (Content::TooLarge, "This message is too large to keep, so it cannot be shown."),
        (Content::Unreadable, "This message cannot be read."),
    ] {
        let fetch = Recorder::default();
        let harness = opened_as(&fetch, "Photos from the coast walk", content.clone(), false);
        // A note, as the frames draw it, 16 under the head, where the text would stand.
        let banner = harness.get_by_label(&format!("Note: {said}")).rect();
        let more = harness.get_by_role_and_label(Role::Button, "More").rect();
        assert!(banner.top() > more.bottom() && banner.top() < more.bottom() + 300.0, "{content:?}: in the text's place: {banner:?}");
        assert!(harness.query_by_label("Photos from the coast walk").is_some());
    }
    // Loading: the progress line, "loading the message".
    let fetch = Recorder::default();
    let harness = opened_as(&fetch, "Photos from the coast walk", Content::Loading, false);
    let line = harness.get_by_role_and_label(Role::ProgressIndicator, "loading the message").rect();
    assert!((line.height() - 24.0).abs() < 0.5, "the progress line is 24 high: {line:?}");
}

#[test]
fn a_cut_text_ends_with_a_note_that_says_so() {
    const SAID: &str = "The text is cut here: the rest is longer than the reader shows.";
    let fetch = Recorder::default();
    let harness = opened(&fetch, None, "Photos from the coast walk");
    let id = harness.state().reader().open_id().unwrap();
    let content = harness.state().reader().opened(id).unwrap().content.clone();
    drop(harness);
    let harness = opened_as(&fetch, "Photos from the coast walk", content, true);
    let note = harness.get_by_label(&format!("Note: {SAID}")).rect();
    // Under the text: below every line of the message's own words.
    let texts = painted_texts(&harness);
    let words = texts.iter().filter(|(rect, text)| rect.top() > 200.0 && text.contains("coast")).map(|(rect, _)| rect.bottom()).fold(0.0, f32::max);
    assert!(note.top() >= words, "the note comes after the text: {note:?} {words}");
}

#[test]
fn a_blocked_remote_picture_is_a_box_of_its_real_size_until_asked_for() {
    let html = r#"<p>The kiln is in.</p><img src="https://pictures.example.org/kiln.png" alt="The new kiln, lit for the first time" width=600 height=240><p>Open day on Sunday.</p>"#;
    let harness = html_message(html, false);
    let label = "Remote picture not loaded: The new kiln, lit for the first time, 600 × 240";
    assert_eq!(harness.get_by_label(label).rect().size(), vec2(600.0, 240.0), "the box holds the picture's place");
    // The text after it stands under the box.
    let after = harness.get_by_label("Open day on Sunday.").rect();
    assert!(after.top() >= harness.get_by_label(label).rect().bottom());
    assert!(harness.state().is_empty(), "the box asks for nothing by itself");
}

#[test]
fn a_careful_link_says_so_on_a_bar_over_the_answers() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, Some(5), "You have won a lighthouse");
    let link = harness.get_by_label("https://bank.example.org/login").rect();
    harness.hover_at(link.left_center() + vec2(8.0, 0.0));
    harness.run();
    // CAREFUL, what the text says and where the link goes, the address whole on its own line, and what a click does.
    let bar = harness.get_by_label_contains("Careful: The text says “").rect();
    harness.get_by_label("https://prize-desk.example.net/claim");
    harness.get_by_label("a click copies it");
    let reply = harness.get_by_role_and_label(Role::Button, "Reply").rect();
    // Over the answers' hairline, 10 above and below and 40 in from the pane's sides.
    assert!(bar.bottom() <= reply.top() - 12.0 - 10.0 + 0.5 && bar.bottom() >= reply.top() - 12.0 - 10.0 - 1.5, "{bar:?} {reply:?}");
    assert!((bar.left() - reply.left()).abs() <= 0.5, "{bar:?} {reply:?}");
}

#[test]
fn more_opens_4_under_its_button_with_archive_and_its_key_first() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, None, "Photos from the coast walk");
    let opener = harness.get_by_role_and_label(Role::Button, "More").rect();
    harness.get_by_role_and_label(Role::Button, "More").click();
    harness.run();
    let rows: Vec<(String, egui::Rect)> =
        harness.query_all_by_role(Role::MenuItem).map(|node| (node.accesskit_node().label().unwrap_or_default(), node.rect())).collect();
    let (first, rect) = rows.iter().min_by(|a, b| a.1.top().total_cmp(&b.1.top())).cloned().unwrap();
    assert!(first.starts_with("Archive"), "Archive is the first row: {rows:?}");
    // The row runs 1 in from the menu's edge: the menu's top 4 under the button.
    assert!((rect.top() - 1.0 - opener.bottom() - 4.0).abs() <= 0.5, "4 under its button: {rect:?} {opener:?}");
    // Its key, A, stands in the row.
    let keys = painted_texts(&harness);
    assert!(keys.iter().any(|(at, text)| text == "A" && rect.contains_rect(*at)), "the row shows its key: {keys:?}");
}

/// Every text the last frame painted with the rect it was clipped to.
fn clipped_texts(harness: &Harness<'static, InboxApp>) -> Vec<(egui::Rect, String, egui::Rect)> {
    fn walk(shape: &egui::Shape, clip: egui::Rect, found: &mut Vec<(egui::Rect, String, egui::Rect)>) {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, clip, found)),
            egui::Shape::Text(text) if !text.galley.job.text.trim().is_empty() => {
                found.push((egui::Rect::from_min_size(text.pos, text.galley.size()), text.galley.job.text.clone(), clip));
            }
            _ => {}
        }
    }
    let mut found = Vec::new();
    harness.output().shapes.iter().for_each(|clipped| walk(&clipped.shape, clipped.clip_rect, &mut found));
    found
}

/// Checks that every button of the reader's pane (right of `left`) stands whole in the window and its words in it.
fn keeps_inside(harness: &Harness<'static, InboxApp>, left: f32, width: f32, at: &str) {
    let texts = clipped_texts(harness);
    for node in harness.query_all_by_role(Role::Button) {
        let rect = node.rect();
        if rect.left() < left || !rect.is_positive() {
            continue;
        }
        let name = node.accesskit_node().label().unwrap_or_default();
        assert!(rect.right() <= width + 0.5, "{name:?} at {rect:?} runs past the window: {at}");
        for (text_rect, text, clip) in texts.iter().filter(|(text_rect, _, _)| rect.contains_rect(*text_rect)) {
            assert!(
                clip.left() <= text_rect.left() + 0.5 && text_rect.right() <= clip.right() + 0.5,
                "{text:?} in {name:?} is clipped at the side: {at}"
            );
        }
    }
}

/// The left and right of the let-in card round `question`: the `cyan_edge` stroke painted round it.
fn card_edges(harness: &Harness<'static, InboxApp>, question: egui::Rect) -> (f32, f32) {
    let edge = *crate::theme::Theme::current(&harness.ctx, &crate::theme::Look::of(harness.state().mailbox().appearance())).palette;
    let mut found = None;
    fn walk(shape: &egui::Shape, colour: egui::Color32, question: egui::Rect, found: &mut Option<egui::Rect>) {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, colour, question, found)),
            egui::Shape::Rect(rect) if rect.stroke.color == colour && rect.rect.contains_rect(question) => *found = Some(rect.rect),
            _ => {}
        }
    }
    harness.output().shapes.iter().for_each(|clipped| walk(&clipped.shape, edge.cyan_edge, question, &mut found));
    let card = found.unwrap_or_else(|| panic!("no card round {question:?}"));
    (card.left(), card.right())
}

#[test]
fn the_reader_s_banners_and_cards_keep_inside_a_narrow_window() {
    for (width, percent) in [(1024.0, 100), (1024.0, 130), (1100.0, 100), (1100.0, 130)] {
        let scale = crate::theme::TextScale::from_percent(percent).unwrap();
        let at = |scene: &str| format!("{scene} {width} {percent}%");
        // Remote pictures and a careful link.
        let fetch = Recorder::default();
        let mut harness = opened_in(&fetch, Some(5), "You have won a lighthouse", width, percent, 1.0);
        let left = harness.get_by_label("You have won a lighthouse").rect().left() - 1.0;
        keeps_inside(&harness, left, width, &at("pictures"));
        let banner = harness.get_by_label_contains("Pictures: ").rect();
        assert!(banner.right() <= width + 0.5 && banner.left() >= left, "{banner:?}: {}", at("pictures"));
        // The held sender's card.
        let mut app = app_with(&fetch);
        app.set_text_scale(scale);
        let mut held = Harness::builder()
            .with_size(vec2(width, 900.0))
            .with_theme(egui::Theme::Dark)
            .with_max_steps(12)
            .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
        settle(&mut held);
        held.get_by_role_and_label(Role::Button, "Review").click();
        held.run();
        held.get_by_label_contains("Harbour Swim Club, Winter swim at the harbour steps").click();
        settle(&mut held);
        let question = held.get_by_label("Let Harbour Swim Club in?").rect();
        keeps_inside(&held, question.left() - 20.0, width, &at("held"));
        // The card's inner room: its left pad is where the question starts, and its right one mirrors it.
        let card = card_edges(&held, question);
        let inner = (card.0 + 16.0, card.1 - 16.0);
        assert!((question.left() - inner.0).abs() <= 0.5, "the question stands at the card's pad: {question:?} {card:?}");
        for name in ["Accept", "Accept and reply", "Block"] {
            let rect = held.get_by_role_and_label(Role::Button, name).rect();
            assert!(
                rect.left() >= inner.0 - 0.5 && rect.right() <= inner.1 + 0.5,
                "{name} at {rect:?} leaves the card's inner room {inner:?}: {}",
                at("held")
            );
        }
        // A note in the text's place.
        harness.state_mut().reader_mut().0.set_notice("Subject copied".to_owned());
        harness.run();
        let note = harness.get_by_label("Note: Subject copied").rect();
        assert!(note.right() <= width + 0.5, "{note:?}: {}", at("notice"));
        keeps_inside(&harness, left, width, &at("notice"));
    }
}

// The reader beside the lists: one meaning per key, one surface per Esc.

#[test]
fn the_lists_menu_key_on_a_line_of_the_reader_opens_that_line_s_menu_and_no_row_s() {
    const OLDER: &str = "Odile Brannock, 08:35, open: ";
    for (key, modifiers) in [(Key::F10, egui::Modifiers::SHIFT), (Key::Menu, egui::Modifiers::NONE)] {
        let fetch = Recorder::default();
        let mut harness = opened(&fetch, Some(3), "Re: Allotment rota for November");
        let open = harness.state().reader().open_id().unwrap();
        harness.get_by_label_contains("Odile Brannock, 08:35, folded").click();
        settle(&mut harness);
        assert!(tab_to_containing(&mut harness, OLDER), "{key:?}: Tab reaches the older open line");
        harness.key_press_modifiers(modifiers, key);
        harness.run();
        harness.get_by_role_and_label(Role::MenuItem, "Show the full header");
        assert!(!crate::component::popup::is_open(&harness.ctx, super::list::group_menu_id(open)), "{key:?}: the open row's menu stays shut");
        let menus = harness.query_all_by_role(Role::MenuItem).count() + harness.query_all_by_role(Role::MenuItemRadio).count();
        assert_eq!(menus, 1, "{key:?}: one menu, the line's");
    }
}

#[test]
fn the_arrows_step_the_list_once_where_it_has_the_keyboard_and_never_from_the_reader() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, None, "Allotment rota for November");
    // A press on the open row: the list has the keyboard.
    harness.query_all_by_label_contains("Odile Brannock, Allotment rota for November").next().unwrap().click();
    settle(&mut harness);
    let order = super::list::order(&harness.ctx);
    let first = harness.state().mailbox().selected().unwrap();
    let at = order.iter().position(|id| *id == first).unwrap();
    let top = |harness: &Harness<'static, InboxApp>| {
        let open = harness.state().reader().open_id().unwrap();
        let subject = harness.state().reader().opened(open).unwrap().header.subject.clone();
        harness.query_all_by_label(&subject).map(|node| node.rect().top()).fold(f32::MAX, f32::min)
    };
    let shown_at = top(&harness);
    harness.key_press(Key::ArrowDown);
    settle(&mut harness);
    assert_eq!(harness.state().mailbox().selected(), order.get(at + 1).copied(), "one row down, not two");
    assert_eq!(harness.state().reader().open_id(), order.get(at + 1).copied(), "the reader shows it");
    assert_eq!(top(&harness), shown_at, "from its top: the reader did not scroll");
    harness.key_press(Key::ArrowUp);
    settle(&mut harness);
    assert_eq!(harness.state().mailbox().selected(), Some(first), "one row up");
    // A press in the reader: the keyboard is the reader's, and the arrows leave the list where it is.
    let subject = harness.get_by_label("Allotment rota for November").rect().center();
    harness.hover_at(subject);
    harness.run();
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton { pos: subject, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
        harness.run();
    }
    settle(&mut harness);
    let before = top(&harness);
    for key in [Key::ArrowDown, Key::ArrowUp, Key::ArrowDown] {
        harness.key_press(key);
        settle(&mut harness);
        assert_eq!(harness.state().mailbox().selected(), Some(first), "{key:?} in the reader moves no row");
        assert_eq!(top(&harness), before, "{key:?} in the reader scrolls nothing");
    }
}

#[test]
fn one_esc_closes_the_reader_s_menu_and_nothing_under_it_after_the_list_s_menu() {
    const SUBJECT: &str = "Allotment rota for November";
    let fetch = Recorder::default();
    let app = app_with(&fetch);
    // One pane: the message stands in the list's place, so an Esc too many would take it away.
    let mut harness = Harness::builder()
        .with_size(vec2(900.0, 800.0))
        .with_theme(egui::Theme::Light)
        .with_max_steps(12)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    settle(&mut harness);
    // The list's menu first: J keys a row, Shift F10 opens its menu, Esc closes it and gives the row the keyboard.
    harness.key_press(Key::J);
    settle(&mut harness);
    let keyed = harness.state().mailbox().selected().unwrap();
    harness.key_press_modifiers(egui::Modifiers::SHIFT, Key::F10);
    settle(&mut harness);
    assert!(crate::component::popup::is_open(&harness.ctx, super::list::group_menu_id(keyed)), "the row's menu opened");
    harness.key_press(Key::Escape);
    settle(&mut harness);
    assert!(!crate::component::popup::is_open(&harness.ctx, super::list::group_menu_id(keyed)), "and closed");
    // Enter opens the keyed message into the one pane; the subject's menu opens over it by the menu key.
    let subject = harness.state().mailbox().messages().iter().find(|row| row.id == keyed).map(|row| row.subject.clone()).unwrap();
    assert_eq!(subject, SUBJECT);
    harness.key_press(Key::Enter);
    settle(&mut harness);
    assert!(super::reader_rect(&harness.ctx).is_some(), "the message in front");
    assert!(tab_to(&mut harness, SUBJECT), "Tab reaches the subject");
    harness.key_press_modifiers(egui::Modifiers::SHIFT, Key::F10);
    harness.run();
    harness.get_by_label("Copy the subject");
    // One Esc: the menu closes, and nothing under it moves.
    harness.key_press(Key::Escape);
    settle(&mut harness);
    assert!(harness.query_by_label("Copy the subject").is_none(), "Esc closed the menu");
    assert!(super::reader_rect(&harness.ctx).is_some(), "the message stays in front");
    assert_eq!(harness.state().reader().open_id(), Some(keyed), "the same message open");
    assert!(harness.query_by_label(SUBJECT).is_some_and(|node| node.accesskit_node().is_focused()), "the keyboard back on the subject");
    // The next Esc is the reader's: back to the list.
    harness.key_press(Key::Escape);
    settle(&mut harness);
    assert!(super::reader_rect(&harness.ctx).is_none(), "the second Esc goes back to the list");
}

// The owner's own act walks on, as the epic's lists do; the server's removal opens nothing.

/// The coast walk open beside the list, with the rows each side of it.
fn coast_walk_open(fetch: &Recorder) -> (Harness<'static, InboxApp>, u64, u64) {
    let harness = opened(fetch, None, "Photos from the coast walk");
    let id = harness.state().reader().open_id().unwrap();
    let rows = harness.state().mailbox().messages();
    let at = rows.iter().position(|row| row.id == id).unwrap();
    let behind = rows[at + 1].id;
    (harness, id, behind)
}

/// A press on the open message's words: the reader has the pointer, the list not the keyboard.
fn press_in_the_reader(harness: &mut Harness<'static, InboxApp>) {
    harness.get_by_label_contains("Here are the ones from the cliff path").click();
    settle(harness);
}

fn read(harness: &Harness<'static, InboxApp>, id: u64) -> bool {
    row_of(harness, id).is_some_and(|row| row.seen)
}

/// The reader's Done (right of the list's own hover acts).
fn reader_done<'h>(harness: &'h Harness<'static, InboxApp>) -> egui_kittest::Node<'h> {
    harness.query_all_by_role_and_label(Role::Button, "Done").max_by(|a, b| a.rect().left().total_cmp(&b.rect().left())).unwrap()
}

#[test]
fn done_by_the_reader_s_button_walks_on_to_the_next_message_and_reads_it() {
    let fetch = Recorder::default();
    let (mut harness, id, behind) = coast_walk_open(&fetch);
    press_in_the_reader(&mut harness);
    reader_done(&harness).click();
    settle(&mut harness);
    assert!(row_of(&harness, id).is_none(), "it left the list");
    assert_eq!(harness.state().reader().open_id(), Some(behind), "the next message opens");
    assert!(read(&harness, behind), "and is read, as the lists read it");
}

#[test]
fn d_in_the_reader_walks_on_and_the_keyboard_stays_in_the_reader() {
    let fetch = Recorder::default();
    let (mut harness, _, behind) = coast_walk_open(&fetch);
    press_in_the_reader(&mut harness);
    harness.key_press(Key::D);
    settle(&mut harness);
    assert_eq!(harness.state().reader().open_id(), Some(behind), "the next message opens");
    assert!(read(&harness, behind));
    assert!(reader_done(&harness).accesskit_node().is_focused(), "the keyboard is on the reader's first tool, not on nothing");
    assert_eq!(super::list::focused_row(&harness.ctx), None, "and on no row of the list");
}

#[test]
fn d_on_the_list_s_row_walks_on_with_the_keyboard_on_the_list() {
    let fetch = Recorder::default();
    let (mut harness, _, behind) = coast_walk_open(&fetch);
    row_node(&harness, "Photos from the coast walk").click();
    settle(&mut harness);
    harness.key_press(Key::D);
    settle(&mut harness);
    assert_eq!(harness.state().reader().open_id(), Some(behind), "the next message opens");
    assert_eq!(super::list::focused_row(&harness.ctx), Some(behind), "the keyboard stays on the list, on the next row");
}

#[test]
fn done_on_the_list_row_s_own_act_walks_on() {
    let fetch = Recorder::default();
    let (mut harness, id, behind) = coast_walk_open(&fetch);
    row_node(&harness, "Photos from the coast walk").hover();
    harness.run();
    harness.query_all_by_role_and_label(Role::Button, "Done").min_by(|a, b| a.rect().left().total_cmp(&b.rect().left())).unwrap().click();
    settle(&mut harness);
    assert!(row_of(&harness, id).is_none(), "it left the list");
    assert_eq!(harness.state().reader().open_id(), Some(behind), "the next message opens");
}

#[test]
fn a_message_the_server_takes_away_opens_nothing_and_the_keyboard_goes_to_the_row_in_its_place() {
    let fetch = Recorder::default();
    let (mut harness, _, behind) = coast_walk_open(&fetch);
    // The simple list, where the row under it is the next in the list: the grouped inbox keeps the keyboard in the
    // card (`after_a_server_removal_the_keyboard_stays_in_the_card`).
    harness.state_mut().mailbox_mut().set_inbox_view(crate::mailbox::InboxView::List);
    settle(&mut harness);
    press_in_the_reader(&mut harness);
    let unread = !read(&harness, behind);
    taken_away(&mut harness);
    assert_eq!(harness.state().mailbox().selected(), None, "no row is selected");
    assert_eq!(read(&harness, behind), !unread, "nothing is read");
    assert_eq!(super::list::focused_row(&harness.ctx), Some(behind), "the keyboard is on the row that took its place");
    assert_eq!(harness.state().reader().open_id(), None, "and that row's message is not opened by it");
    harness.key_press(Key::Enter);
    settle(&mut harness);
    assert_eq!(harness.state().reader().open_id(), Some(behind), "Enter opens the row the keyboard is on");
}

/// The last row taken away by the server: the keyboard goes to the row that is now last.
#[test]
fn the_last_row_the_server_takes_away_leaves_the_keyboard_on_the_new_last_row() {
    let fetch = Recorder::default();
    let app = app_with(&fetch);
    let last = app.mailbox().messages().last().unwrap().subject.clone();
    drop(app);
    let mut harness = opened(&fetch, None, &last);
    taken_away(&mut harness);
    let now_last = harness.state().mailbox().messages().last().unwrap().id;
    assert_eq!(super::list::focused_row(&harness.ctx), Some(now_last), "the keyboard is on the last row");
}

/// A Flag from the list on the open message, then the server takes it away: only an act that takes the row out walks on,
/// so nothing opens.
#[test]
fn a_flag_from_the_list_does_not_turn_a_later_removal_into_a_walk_on() {
    let fetch = Recorder::default();
    let (mut harness, id, _) = coast_walk_open(&fetch);
    row_node(&harness, "Photos from the coast walk").click();
    settle(&mut harness);
    harness.key_press(Key::S);
    settle(&mut harness);
    assert!(row_of(&harness, id).is_some(), "a flag leaves the row in the list");
    taken_away(&mut harness);
    assert_eq!(harness.state().mailbox().selected(), None, "nothing is selected");
}

/// The owner flags the open message and the server takes it away before the window draws again: a Flag leaves its row
/// in the list, so it notes nothing the removal could be taken for, and nothing walks on.
#[test]
fn a_flag_then_a_server_removal_does_not_walk_on() {
    let fetch = Recorder::default();
    let (mut harness, id, behind) = coast_walk_open(&fetch);
    let unread = !read(&harness, behind);
    // No frame between the two: the act's note is not read before the removal.
    let mailbox = harness.state_mut().mailbox_mut();
    mailbox.act(id, crate::mailbox::Act::Flag);
    mailbox.run("mail.done", &json!({ "id": id, "done": true })).unwrap();
    mailbox.refresh();
    settle(&mut harness);
    assert!(row_of(&harness, id).is_none(), "it left the list");
    assert_eq!(harness.state().reader().open_id(), None, "nothing opens by itself");
    assert_eq!(harness.state().mailbox().selected(), None, "nothing is selected");
    assert_eq!(read(&harness, behind), !unread, "the next message is not read");
}

/// The owner's Done walks on, Undo brings the message back open, the owner reads it (R), then the server takes it
/// away: the Done was noted once, when it took the row out, and its note does not outlive that frame.
#[test]
fn a_read_then_a_server_removal_does_not_walk_on() {
    let fetch = Recorder::default();
    let (mut harness, id, behind) = coast_walk_open(&fetch);
    press_in_the_reader(&mut harness);
    reader_done(&harness).click();
    settle(&mut harness);
    assert_eq!(harness.state().reader().open_id(), Some(behind), "the Done walked on");
    harness.key_press_modifiers(egui::Modifiers::COMMAND, Key::Z);
    settle(&mut harness);
    assert_eq!(harness.state().reader().open_id(), Some(id), "Undo brings it back open");
    let was = read(&harness, id);
    press_in_the_reader(&mut harness);
    harness.key_press(Key::R);
    settle(&mut harness);
    assert_ne!(read(&harness, id), was, "R marked it");
    assert!(row_of(&harness, id).is_some(), "a read leaves the row in the list");
    taken_away(&mut harness);
    assert_eq!(harness.state().mailbox().selected(), None, "nothing is selected");
}

/// The app at `width` in the unified inbox's `view`, a new message docked in the reader's place if `compose`.
fn inbox_at(width: f32, view: crate::mailbox::InboxView, compose: bool) -> Harness<'static, InboxApp> {
    let fetch = Recorder::default();
    let mut app = app_with(&fetch);
    app.mailbox_mut().set_inbox_view(view);
    let mut harness = Harness::builder()
        .with_size(vec2(width, 820.0))
        .with_theme(egui::Theme::Light)
        .with_max_steps(12)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    if compose {
        crate::draw::tests::new_message(&mut harness);
        settle(&mut harness);
    }
    harness
}

/// The card of the unified inbox that holds message `id`: a group's name, or `None` for the rows no group holds.
fn card_of(harness: &Harness<'static, InboxApp>, id: u64) -> Option<String> {
    harness.state().mailbox().groups().into_iter().find(|group| group.messages.contains(&id)).map(|group| group.name)
}

/// The rows of `card` ([`card_of`]) the list draws now, in its order.
fn card_rows(harness: &Harness<'static, InboxApp>, card: &Option<String>) -> Vec<u64> {
    let groups = harness.state().mailbox().groups();
    let held = |id: &u64| groups.iter().any(|group| group.messages.contains(id));
    let order = super::list::order(&harness.ctx);
    order
        .iter()
        .copied()
        .filter(|id| match card {
            Some(name) => groups.iter().any(|group| group.name == *name && group.messages.contains(id)),
            None => !held(id),
        })
        .collect()
}

/// The server takes the message of the row `id` away.
fn server_removes(harness: &mut Harness<'static, InboxApp>, id: u64) {
    let (_, mailbox) = harness.state_mut().reader_mut();
    mailbox.run("mail.done", &json!({ "id": id, "done": true })).unwrap();
    mailbox.refresh();
    settle(harness);
    assert!(row_of(harness, id).is_none(), "it left the list");
}

/// Grouped, the keyboard Tabbed onto the last row of a card with another card under it, then the server takes that
/// message away: the keyboard goes to the row now last in the same card, not to the next card's first row.
#[test]
fn after_a_server_removal_the_keyboard_stays_in_the_card() {
    let mut harness = inbox_at(1280.0, crate::mailbox::InboxView::Grouped, false);
    let groups = harness.state().mailbox().groups();
    let card = groups
        .iter()
        .take(groups.len().saturating_sub(1))
        .map(|group| Some(group.name.clone()))
        .find(|card| card_rows(&harness, card).len() >= 2)
        .unwrap();
    let last = *card_rows(&harness, &card).last().unwrap();
    for _ in 0..200 {
        if super::list::focused_row(&harness.ctx) == Some(last) {
            break;
        }
        harness.key_press(Key::Tab);
        harness.run();
    }
    settle(&mut harness);
    assert_eq!(super::list::focused_row(&harness.ctx), Some(last), "the card's last row has the keyboard");
    server_removes(&mut harness, last);
    let now_last = card_rows(&harness, &card).last().copied();
    assert!(now_last.is_some(), "the card still has rows");
    assert_eq!(super::list::focused_row(&harness.ctx), now_last, "the keyboard is on the row now last in the card {card:?}");
}

/// The keyboard on the list's last row in sight, its message shown as a preview (behind the docked composer: Shift Tab
/// out of From; without one: End), then the server takes it away: nothing opens, nothing is selected, and the
/// keyboard is on the row now last in its card.
#[test]
fn a_server_removal_of_the_focused_row_opens_nothing() {
    for (width, height, compose) in [(1300.0, 820.0, true), (1300.0, 820.0, false), (720.0, 820.0, false), (1300.0, 420.0, false)] {
        let mut harness = inbox_at(width, crate::mailbox::InboxView::Smart, compose);
        harness.set_size(vec2(width, height));
        settle(&mut harness);
        if compose {
            type_into_text(&mut harness, "Hello Odile");
            harness.ctx.memory_mut(|memory| memory.request_focus(egui::Id::unique("compose-from")));
            harness.run();
            harness.key_press_modifiers(egui::Modifiers::SHIFT, Key::Tab);
            harness.run();
        } else {
            for _ in 0..60 {
                if super::list::focused_row(&harness.ctx).is_some() {
                    break;
                }
                harness.key_press(Key::Tab);
                harness.run();
            }
            harness.key_press(Key::End);
            settle(&mut harness);
        }
        let keyed = super::list::focused_row(&harness.ctx).unwrap();
        assert_eq!(harness.state().reader().open_id(), Some(keyed), "{width}, composer {compose}: the keyed row is the preview");
        let card = card_of(&harness, keyed);
        let seen: Vec<(u64, bool)> =
            harness.state().mailbox().messages().iter().map(|row| (row.id, row.seen)).filter(|(id, _)| *id != keyed).collect();
        server_removes(&mut harness, keyed);
        assert_eq!(harness.state().reader().open_id(), None, "{width}, composer {compose}: nothing is opened or previewed");
        assert_eq!(harness.state().mailbox().selected(), None, "{width}, composer {compose}: nothing is selected");
        let now: Vec<(u64, bool)> = harness.state().mailbox().messages().iter().map(|row| (row.id, row.seen)).collect();
        assert_eq!(now, seen, "{width}, composer {compose}: nothing is read");
        let focused = super::list::focused_row(&harness.ctx);
        assert!(focused.is_some(), "{width}, composer {compose}: the keyboard is on a row");
        assert_eq!(focused, card_rows(&harness, &card).last().copied(), "{width}, composer {compose}: the row now last in the card");
        if compose {
            let composer = harness.state().composer().unwrap();
            assert!(!composer.floating() && composer.text() == "Hello Odile", "{width}: the composer stays docked, the draft whole");
        }
    }
}

/// Types `text` into the composer's message text.
fn type_into_text(harness: &mut Harness<'static, InboxApp>, text: &str) {
    harness.ctx.memory_mut(|memory| memory.request_focus(super::compose::text_id()));
    harness.run();
    harness.event(egui::Event::Text(text.to_owned()));
    harness.run();
}

/// Turns the wheel over the list by `points` (down when positive) and lets it settle.
fn wheel_list(harness: &mut Harness<'static, InboxApp>, points: f32) {
    let view = super::list::view_rect(&harness.ctx).unwrap();
    harness.hover_at(view.center());
    harness.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: vec2(0.0, -points),
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Wheel,
        modifiers: egui::Modifiers::NONE,
    });
    settle(harness);
}

/// Whether the row of `id` is drawn wholly in the list's viewport.
fn in_sight(harness: &Harness<'static, InboxApp>, id: u64) -> bool {
    let view = super::list::view_rect(&harness.ctx).unwrap();
    let mailbox = harness.state().mailbox();
    let Some(row) = mailbox.messages().iter().find(|row| row.id == id) else { return false };
    harness
        .query_all_by_role(Role::ListBoxOption)
        .any(|node| node.accesskit_node().label().is_some_and(|label| label == mailbox.spoken(row)) && view.contains_rect(node.rect()))
}

/// How the open row was reached before it went out of sight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reached {
    /// The owner pressed it: opened and read.
    Opened,
    /// Tab walked to it: a preview.
    Keyed,
    /// Shift Tab from the docked composer's From walked to it: a preview beside the composer.
    BesideComposer,
}

/// A window 1300 × 494 in `view` with the first row of the list reached as `how`, then the list wheeled `away` so far
/// that the row is out of sight.
fn reached_and_wheeled(view: crate::mailbox::InboxView, how: Reached, away: f32) -> (Harness<'static, InboxApp>, u64) {
    let (mut harness, open) = reached(view, how);
    wheel_list(&mut harness, away);
    if in_sight(&harness, open) {
        // A row at the list's foot (Shift Tab from the composer lands there): away is up.
        wheel_list(&mut harness, -2.0 * away);
    }
    assert!(!in_sight(&harness, open), "{view:?} {how:?} {away}: the open row is out of sight");
    (harness, open)
}

/// A window 1300 × 494 in `view` with the first row of the list reached as `how`; the open message.
fn reached(view: crate::mailbox::InboxView, how: Reached) -> (Harness<'static, InboxApp>, u64) {
    let mut harness = inbox_at(1300.0, view, how == Reached::BesideComposer);
    // The body as high as a 420 window's was before the command and status lines took their 74.
    harness.set_size(vec2(1300.0, 494.0));
    settle(&mut harness);
    match how {
        Reached::Opened => {
            let first = super::list::order(&harness.ctx).first().copied().unwrap();
            let row = harness.state().mailbox().messages().iter().find(|row| row.id == first).unwrap().clone();
            row_node(&harness, &row.subject).click();
            settle(&mut harness);
        }
        Reached::Keyed => {
            for _ in 0..120 {
                if super::list::focused_row(&harness.ctx).is_some() {
                    break;
                }
                harness.key_press(Key::Tab);
                harness.run();
            }
            settle(&mut harness);
        }
        Reached::BesideComposer => {
            type_into_text(&mut harness, "Hello Odile");
            harness.ctx.memory_mut(|memory| memory.request_focus(egui::Id::unique("compose-from")));
            harness.run();
            harness.key_press_modifiers(egui::Modifiers::SHIFT, Key::Tab);
            settle(&mut harness);
        }
    }
    let open = harness.state().reader().open_id().unwrap();
    (harness, open)
}

/// The server takes away the open message whose row the owner scrolled out of sight: nothing opens, nothing is read,
/// a docked composer stays docked, and the keyboard is on a row the list draws.
#[test]
fn a_server_removal_of_a_row_out_of_sight_opens_nothing() {
    use crate::mailbox::InboxView;
    for view in [InboxView::Grouped, InboxView::Smart, InboxView::List] {
        for how in [Reached::Opened, Reached::Keyed, Reached::BesideComposer] {
            for away in [400.0, 3000.0] {
                let (mut harness, open) = reached_and_wheeled(view, how, away);
                let seen: Vec<(u64, bool)> =
                    harness.state().mailbox().messages().iter().map(|row| (row.id, row.seen)).filter(|(id, _)| *id != open).collect();
                server_removes(&mut harness, open);
                let what = format!("{view:?} {how:?} {away}");
                assert_eq!(harness.state().reader().open_id(), None, "{what}: nothing is opened or previewed");
                assert_eq!(harness.state().mailbox().selected(), None, "{what}: nothing is selected");
                let now: Vec<(u64, bool)> = harness.state().mailbox().messages().iter().map(|row| (row.id, row.seen)).collect();
                assert_eq!(now, seen, "{what}: nothing is read");
                let focused = super::list::focused_row(&harness.ctx);
                assert!(focused.is_some_and(|id| super::list::order(&harness.ctx).contains(&id)), "{what}: the keyboard is on a drawn row");
                if how == Reached::BesideComposer {
                    let composer = harness.state().composer().unwrap();
                    assert!(!composer.floating() && composer.text() == "Hello Odile", "{what}: the composer stays docked, the draft whole");
                }
                // A step from there opens nothing by itself either.
                settle(&mut harness);
                assert_eq!(harness.state().reader().open_id(), None, "{what}: still nothing opened");
            }
        }
    }
}

/// The owner's own act on a message it opened, its row then scrolled out of sight.
#[derive(Debug, Clone, Copy)]
enum OwnersAct {
    Done,
    Delete,
    Snooze,
    Move,
}

/// Does `act` on the open message as the owner does from the list: its key, or the act the picker starts.
fn owners_act(harness: &mut Harness<'static, InboxApp>, act: OwnersAct) {
    let open = harness.state().reader().open_id().unwrap();
    match act {
        OwnersAct::Done => harness.key_press(Key::D),
        OwnersAct::Delete => harness.key_press(Key::Delete),
        OwnersAct::Snooze => {
            let until = Timestamp::from_unix(1_793_869_200 + 86_400, 0).unwrap();
            harness.state_mut().mailbox_mut().start(open, crate::mailbox::Act::Snooze(until));
        }
        OwnersAct::Move => {
            let mailbox = harness.state().mailbox();
            let row = mailbox.messages().iter().find(|row| row.id == open).unwrap();
            let to = mailbox.folders_of(row.account).into_iter().find(|folder| folder.id != row.folder).unwrap().id;
            harness.state_mut().mailbox_mut().start(open, crate::mailbox::Act::Move(to));
        }
    }
    settle(harness);
    settle(harness);
}

/// The owner's act walks on to the rule's next row and opens it, whether the acted row is in sight or scrolled away.
#[test]
fn the_owner_s_act_on_a_row_out_of_sight_walks_on_to_the_same_row_as_in_sight() {
    use crate::mailbox::InboxView;
    for view in [InboxView::Grouped, InboxView::Smart, InboxView::List] {
        for act in [OwnersAct::Done, OwnersAct::Delete, OwnersAct::Snooze, OwnersAct::Move] {
            // In sight first: where the walk goes today.
            let (mut harness, open) = reached(view, Reached::Opened);
            owners_act(&mut harness, act);
            let walked = harness.state().reader().open_id();
            assert!(walked.is_some_and(|id| id != open), "{view:?} {act:?}: in sight the act walks on");
            // Out of sight: the same row opens, and is read.
            let (mut harness, gone) = reached_and_wheeled(view, Reached::Opened, 3000.0);
            assert_eq!(gone, open);
            owners_act(&mut harness, act);
            assert_eq!(harness.state().reader().open_id(), walked, "{view:?} {act:?}: out of sight it walks on to the same row");
            assert!(read(&harness, walked.unwrap()), "{view:?} {act:?}: and reads it");
            assert_eq!(super::list::focused_row(&harness.ctx), walked, "{view:?} {act:?}: the keyboard on it");
        }
    }
}

/// Grouped, the owner opened the last message of a card with another card under it and read it (then pressed in
/// the reader), and the server takes it away: nothing opens, the keyboard goes to the row now last in that card, not to
/// the next card's first row, and nothing is read.
#[test]
fn in_the_grouped_inbox_a_message_the_owner_read_and_the_server_took_away_opens_nothing() {
    let mut harness = inbox_at(1280.0, crate::mailbox::InboxView::Grouped, false);
    let groups = harness.state().mailbox().groups();
    let card = groups
        .iter()
        .take(groups.len().saturating_sub(1))
        .map(|group| Some(group.name.clone()))
        .find(|card| card_rows(&harness, card).len() >= 2)
        .unwrap();
    let last = *card_rows(&harness, &card).last().unwrap();
    let subject = row_of(&harness, last).unwrap().subject;
    row_node(&harness, &subject).click();
    settle(&mut harness);
    assert_eq!(harness.state().reader().open_id(), Some(last), "the owner opened it");
    assert!(read(&harness, last), "and read it");
    // A press in the reader: the reader has the pointer, the list not the keyboard.
    let reader = super::reader_rect(&harness.ctx).unwrap();
    harness.event(egui::Event::PointerButton {
        pos: reader.center(),
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    harness.event(egui::Event::PointerButton {
        pos: reader.center(),
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    settle(&mut harness);
    let seen: Vec<(u64, bool)> = harness.state().mailbox().messages().iter().map(|row| (row.id, row.seen)).filter(|(row, _)| *row != last).collect();
    taken_away(&mut harness);
    assert_eq!(harness.state().mailbox().selected(), None, "no row is selected");
    let now: Vec<(u64, bool)> = harness.state().mailbox().messages().iter().map(|row| (row.id, row.seen)).collect();
    assert_eq!(now, seen, "nothing is read");
    let now_last = card_rows(&harness, &card).last().copied();
    assert!(now_last.is_some(), "the card still has rows");
    assert_eq!(super::list::focused_row(&harness.ctx), now_last, "the keyboard is on the row now last in the card {card:?}");
    assert_eq!(harness.state().reader().open_id(), None, "and that row's message is not opened by it");
}

/// The why line's reasons as the open reader holds them, and how many times it asked.
fn why_now(harness: &Harness<'static, InboxApp>) -> (Vec<String>, u64) {
    (harness.state().reader.why().to_vec(), harness.state().reader.asked_why)
}

/// A few frames, as the window draws them after an act.
fn frames(harness: &mut Harness<'static, InboxApp>) {
    for _ in 0..3 {
        harness.run();
    }
}

/// The open message's reasons are what the engine says now, and the line says them.
fn fresh(harness: &mut Harness<'static, InboxApp>) -> Vec<String> {
    let open = harness.state().reader.open_id().unwrap();
    let held = harness.state().reader.why().to_vec();
    let app = harness.state_mut();
    let now = crate::why::ask(&mut app.mailbox, open);
    assert_eq!(held, now, "the reasons are the engine's of now");
    harness.get_by_label(&crate::component::why_line::spoken(&held));
    held
}

#[test]
fn a_group_chosen_from_the_lists_menu_changes_the_open_messages_reasons() {
    let fetch = Recorder::default();
    let mut harness = opened(&fetch, None, "Allotment rota for November");
    let id = harness.state().reader.open_id().unwrap();
    let (before, asked) = why_now(&harness);
    assert!(before.iter().all(|reason| !reason.contains("Notifications")), "{before:?}");
    harness.state_mut().mailbox_mut().select_keyed(id);
    harness.ctx.memory_mut(|memory| memory.request_focus(super::list::row_id(id)));
    harness.run();
    harness.key_press_modifiers(egui::Modifiers::SHIFT, Key::F10);
    harness.run();
    harness.get_by_role_and_label(Role::MenuItemRadio, "Notifications").click();
    frames(&mut harness);
    assert_eq!(harness.state().reader.open_id(), Some(id), "still open");
    let (after, now) = why_now(&harness);
    assert!(after.iter().any(|reason| reason.ends_with("in Notifications")), "{before:?} -> {after:?}");
    assert_eq!(now, asked + 1, "asked once more");
    fresh(&mut harness);
    // Idle frames after: no more asking.
    for _ in 0..10 {
        harness.step();
    }
    assert_eq!(why_now(&harness).1, now);
}

#[test]
fn not_done_with_show_done_on_keeps_the_message_open_and_says_where_it_is() {
    let fetch = Recorder::default();
    let mut app = app_with(&fetch);
    app.mailbox_mut().set_show_done(true);
    let id = app.mailbox().messages().iter().find(|row| row.subject == "Allotment rota for November").unwrap().id;
    app.mailbox_mut().select(Some(id));
    app.mailbox_mut().act(id, crate::mailbox::Act::Done);
    let mut harness = harness_with(app, egui::Theme::Light);
    frames(&mut harness);
    let (before, asked) = why_now(&harness);
    // Done, it is out of the grouped inbox: no group is a reason.
    assert!(before.iter().all(|reason| !reason.starts_with("grouped as")), "{before:?}");
    harness.get_by_role_and_label(Role::Button, "Not done").click();
    frames(&mut harness);
    assert_eq!(harness.state().reader.open_id(), Some(id), "still open");
    let (after, now) = why_now(&harness);
    assert_eq!(after.first().map(String::as_str), Some("grouped as People"), "back in the inbox: {before:?} -> {after:?}");
    assert_eq!(now, asked + 1);
    fresh(&mut harness);
}

#[test]
fn brought_back_from_a_snooze_the_reader_says_the_engines_reasons_of_now() {
    let fetch = Recorder::default();
    let mut app = app_with(&fetch);
    let id = app.mailbox().messages().iter().find(|row| row.subject == "Allotment rota for November").unwrap().id;
    let later = app.mailbox().now().unix_seconds().saturating_add(2 * 86_400);
    app.mailbox_mut().act(id, crate::mailbox::Act::Snooze(Timestamp::from_unix(later, 0).unwrap()));
    app.mailbox_mut().open(crate::mailbox::Place::Later(crate::mailbox::Pile::Snoozed));
    app.mailbox_mut().select(Some(id));
    app.mailbox_mut().forget_undo();
    let mut harness = harness_with(app, egui::Theme::Light);
    let (before, _) = why_now(&harness);
    assert!(before.iter().any(|reason| reason.starts_with("snoozed until")), "{before:?}");
    harness.get_by_role_and_label(Role::Button, "More").click();
    harness.run();
    harness.get_by_label("Bring back now").click();
    frames(&mut harness);
    // The pile is empty: whatever is open says the engine's reasons of now, none of the snooze that ended.
    if harness.state().reader.open_id().is_some() {
        let after = fresh(&mut harness);
        assert!(after.iter().all(|reason| !reason.starts_with("snoozed until")), "{after:?}");
    } else {
        assert!(harness.query_all_by_label_contains("snoozed until").next().is_none());
    }
}

#[test]
fn a_sender_let_in_from_the_lists_card_leaves_no_reason_stale() {
    let fetch = Recorder::default();
    for (open, card) in [("Winter swim at the harbour steps", 0), ("Bare-root fruit trees are in", 0)] {
        let mut app = app_with(&fetch);
        app.mailbox_mut().open(crate::mailbox::Place::Senders);
        let id = app.mailbox().messages().iter().find(|row| row.subject == open).unwrap().id;
        app.mailbox_mut().select(Some(id));
        let mut harness = harness_with(app, egui::Theme::Light);
        let (before, asked) = why_now(&harness);
        assert_eq!(before, ["held back by the screener"], "{open}");
        harness.query_all_by_role_and_label(Role::Button, "Let in").nth(card).unwrap().click();
        frames(&mut harness);
        let still = harness.state().reader.open_id() == Some(id);
        let (_, now) = why_now(&harness);
        if still {
            // Another sender let in: this message did not move, nothing asked.
            assert_eq!(now, asked, "{open}");
        }
        fresh(&mut harness);
    }
}

#[test]
fn in_one_pane_back_to_the_list_leads_the_head_8_before_the_tags() {
    // The frame `size-800/inbox-reader`: the way back at the tool line's left (40 in, as the subject), the group's tag
    // 8 after it, the acts at the right from Done to More.
    let fetch = Recorder::default();
    let harness = opened_at(&fetch, "Photos from the coast walk", 800.0, crate::theme::TextScale::default());
    let back = harness.get_by_role_and_label(Role::Button, "Back to the list").rect();
    let subject = harness.get_by_label("Photos from the coast walk").rect();
    assert!((back.left() - subject.left()).abs() < 0.5, "the arrow at the head's left: {back:?} {subject:?}");
    let tag = harness
        .query_all_by_role(Role::Label)
        .map(|node| node.rect())
        .filter(|rect| (rect.center().y - back.center().y).abs() < 1.0 && rect.left() > back.right())
        .min_by(|a, b| a.left().total_cmp(&b.left()))
        .unwrap();
    assert_eq!(tag.left() - back.right(), 8.0, "the first tag 8 after the arrow: {back:?} {tag:?}");
    let done = harness.get_by_role_and_label(Role::Button, "Done").rect();
    let more =
        harness.query_all_by_role_and_label(Role::Button, "More").map(|node| node.rect()).max_by(|a, b| a.left().total_cmp(&b.left())).unwrap();
    assert!(done.left() > tag.right() && more.right() <= subject.right() + 0.5, "the acts at the right: {done:?} {more:?} {subject:?}");
}

#[test]
fn at_1024_and_130_percent_the_acts_keep_one_line_and_a_long_subject_wraps_under_them() {
    // The reader beside the list at 1024 and 130 % text is about 524 wide: the six acts and More keep the tags' line, a
    // subject longer than the head takes its second line under them.
    const SUBJECT: &str = "Your bike is due for a service";
    let fetch = Recorder::default();
    let harness = opened_at(&fetch, SUBJECT, 1024.0, crate::theme::TextScale::ALL[4]);
    let line = |name: &str| {
        harness.query_all_by_role_and_label(Role::Button, name).map(|node| node.rect()).max_by(|a, b| a.left().total_cmp(&b.left())).unwrap()
    };
    let done = line("Done");
    for name in ["Snooze", "Pin", "Move to a folder", "Delete", "More"] {
        let rect = line(name);
        assert!((rect.center().y - done.center().y).abs() < 0.5 && rect.left() > done.left(), "{name} on Done's line: {rect:?} {done:?}");
    }
    let subject = harness.get_by_label(SUBJECT).rect();
    let theme = crate::theme::Theme::current(&harness.ctx, &crate::theme::Look::of(harness.state().mailbox().appearance()));
    assert!(subject.top() >= done.bottom(), "the subject under the acts: {subject:?} {done:?}");
    assert!((line("More").right() - subject.right()).abs() < 0.5, "More ends at the head's right edge: {subject:?}");
    assert!((subject.height() - 2.0 * theme.fonts.title.line).abs() < 0.5, "the subject on two lines: {subject:?}");
}

/// A link target with right-to-left host labels, a path and a query: drawn, it reads in the order a click copies it.
const RIGHT_TO_LEFT: &str = "https://שלום.קום.example.org/rota?week=שלום";

/// The HTML message `html` open in the reader of a window 2400 wide (no line of the link status is cut), the
/// pointer on what is named `on` (the first box, 8 in): the harness.
fn hovered_in(html: &str, on: &str) -> Harness<'static, InboxApp> {
    let mut harness = shown_in(html);
    let at = harness.get_by_label(on).rect();
    harness.hover_at(at.left_center() + vec2(8.0, 0.0));
    settle(&mut harness);
    harness
}

/// The HTML message `html` open in the reader of a window 2400 wide.
fn shown_in(html: &str) -> Harness<'static, InboxApp> {
    let mut harness = opened_at(&Recorder::default(), "Photos from the coast walk", 2400.0, crate::theme::TextScale::default());
    let id = harness.state().reader().open_id().unwrap();
    let document = inboxcraft_html::display(html, &inboxcraft_html::Options::default());
    harness.state_mut().reader_mut().0.show_as(id, crate::reader::Content::Html(document), false);
    settle(&mut harness);
    harness
}

/// Whether the last frame painted `text` as a line of its own, its parts in the order it is written (a line's job holds
/// its pieces in the order they are drawn, left to right; a right-to-left word in the order it is written).
fn painted_line(harness: &Harness<'static, InboxApp>, text: &str) -> bool {
    painted_texts(harness).iter().any(|(_, painted)| painted == text)
}

#[test]
fn a_link_s_target_on_the_status_float_reads_in_the_order_a_click_copies() {
    let harness = hovered_in(&format!(r#"<p>See <a href="{RIGHT_TO_LEFT}">the rota</a> for the week.</p>"#), "the rota");
    assert!(painted_line(&harness, RIGHT_TO_LEFT), "the labels stay where they are written: {:?}", painted_texts(&harness));
}

#[test]
fn a_button_link_s_target_on_the_status_float_reads_in_the_order_a_click_copies() {
    let html = format!(r#"<p><a role="button" href="{RIGHT_TO_LEFT}">Book a slot</a></p>"#);
    let harness = hovered_in(&html, "Book a slot");
    assert!(painted_line(&harness, RIGHT_TO_LEFT), "the labels stay where they are written: {:?}", painted_texts(&harness));
}

#[test]
fn a_picture_link_s_target_on_the_status_float_reads_in_the_order_a_click_copies() {
    let html = format!(r#"<p><a href="{RIGHT_TO_LEFT}"><img src="cid:none" alt="The rota board" width=200 height=80></a></p>"#);
    let mut harness = shown_in(&html);
    let at = harness.get_by_role(Role::Link).rect();
    harness.hover_at(at.center());
    settle(&mut harness);
    assert!(painted_line(&harness, RIGHT_TO_LEFT), "the labels stay where they are written: {:?}", painted_texts(&harness));
}

#[test]
fn a_careful_link_s_host_and_target_read_in_the_order_a_click_copies() {
    let html = format!(r#"<p><a href="{RIGHT_TO_LEFT}">https://bank.example.org/login</a></p>"#);
    let harness = hovered_in(&html, "https://bank.example.org/login");
    // The address whole on its own line, and the host in the words above it.
    assert!(painted_line(&harness, RIGHT_TO_LEFT), "the target: {:?}", painted_texts(&harness));
    let said = "The text says “https://bank.example.org/login”, but the link goes to שלום.קום.example.org:";
    assert!(painted_line(&harness, said), "the host: {:?}", painted_texts(&harness));
}

#[test]
fn a_mail_link_s_address_on_the_status_float_reads_in_the_order_a_click_copies() {
    let harness = hovered_in(r#"<p>Or <a href="mailto:rota@שלום.קום.example.org">write to the rota</a>.</p>"#, "write to the rota");
    assert!(painted_line(&harness, "Write to rota@שלום.קום.example.org"), "the labels stay: {:?}", painted_texts(&harness));
}

#[test]
fn a_copied_link_s_notice_reads_in_the_order_it_was_copied() {
    let mut harness = hovered_in(&format!(r#"<p>See <a href="{RIGHT_TO_LEFT}">the rota</a> for the week.</p>"#), "the rota");
    harness.get_by_label("the rota").click();
    settle(&mut harness);
    let notice = format!("Link copied: {RIGHT_TO_LEFT}");
    assert!(painted_line(&harness, &notice), "the notice: {:?}", painted_texts(&harness));
}
