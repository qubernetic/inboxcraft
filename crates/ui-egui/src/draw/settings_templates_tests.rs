// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The Templates page under Sending: its list, empty and form states, the delete confirmed in a dialog, the keyboard walk,
//! the page at rest, hostile names and texts, and that `template.delete` runs only on the owner's press of the
//! confirmation.

use egui::accesskit::Role;
use egui::{Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_time::{FakeClock, Timestamp};
use serde_json::{Value, json};

use crate::mailbox::Mailbox;
use crate::settings::{Section, Under};
use crate::{InboxApp, sample};

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

/// The sample window with `templates` (account, name, subject, text) kept, on the Templates page.
fn on_templates(templates: &[(u64, &str, &str, &str)]) -> Harness<'static, InboxApp> {
    let mut app = InboxApp::at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    app.set_reduce_motion(true);
    for (account, name, subject, text) in templates {
        app.mailbox_mut().run("template.save", &json!({ "account": account, "name": name, "subject": subject, "text": text })).unwrap();
    }
    app.open_templates();
    let mut harness = Harness::builder()
        .with_size(vec2(1440.0, 900.0))
        .with_max_steps(40)
        .with_theme(egui::Theme::Dark)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

const FIVE: [(u64, &str, &str, &str); 5] = [
    (2, "Workshop invitation", "Thursday workshop", "Hello, the next workshop is on Thursday in Room 3."),
    (2, "Quote for a shop front", "Your quote for {?what}", "Hello {first_name}, thank you for asking."),
    (2, "Order received, thank you", "Your order is with us", "Hello {first_name}, thank you."),
    (2, "Away from the studio", "Away until {?date}", "Hello {first_name}, the studio is closed until {?date}."),
    (1, "Out of town", "Away until {?date}", "Hello {first_name}, I am away until {?date}."),
];

/// The names of the templates the engine keeps, as `template.list` answers them.
fn kept(harness: &mut Harness<'static, InboxApp>) -> Vec<String> {
    let answer = harness.state_mut().mailbox_mut().run("template.list", &json!({})).unwrap();
    answer["templates"].as_array().unwrap().iter().map(|row| row["name"].as_str().unwrap().to_owned()).collect()
}

/// The label of the node with the keyboard focus, if one has it.
fn focused(harness: &Harness<'static, InboxApp>) -> Option<String> {
    harness.root().children_recursive().find(|node| node.accesskit_node().is_focused()).map(|node| node.accesskit_node().label().unwrap_or_default())
}

fn press(harness: &mut Harness<'static, InboxApp>, label: &str) {
    harness.get_by_label(label).click();
    harness.run();
}

fn key(harness: &mut Harness<'static, InboxApp>, key: Key, modifiers: Modifiers) {
    harness.key_press_modifiers(modifiers, key);
    harness.run();
}

#[test]
fn sending_s_row_opens_the_templates_page_and_its_link_comes_back() {
    let mut harness = on_templates(&[]);
    harness.state_mut().settings_mut().show(Section::Sending);
    harness.run();
    press(&mut harness, "manage templates →");
    assert_eq!(harness.state().settings().under(), Some(Under::Templates));
    assert_eq!(harness.state().settings().section(), Section::Sending, "the page stands under Sending");
    press(&mut harness, "← Sending");
    assert_eq!(harness.state().settings().under(), None);
    assert_eq!(harness.state().settings().section(), Section::Sending);
}

#[test]
fn the_list_groups_the_templates_by_account_with_a_row_each() {
    let harness = on_templates(&FIVE);
    harness.get_by_role_and_label(Role::Heading, "Templates");
    harness.get_by_label_contains("] Studio · 4");
    harness.get_by_label_contains("] Personal · 1");
    for (_, name, _, _) in FIVE {
        let edit = harness.get_by_label(&format!("Edit the template {name}")).rect();
        let delete = harness.get_by_label(&format!("Delete the template {name}")).rect();
        assert!((edit.center().y - delete.center().y).abs() < 1.0 && edit.right() < delete.left(), "edit, then delete, on one row: {name}");
    }
    let first = harness.get_by_label("Delete the template Workshop invitation").rect().center().y;
    let second = harness.get_by_label("Delete the template Quote for a shop front").rect().center().y;
    assert!((second - first - 49.0).abs() <= 1.0, "a row is 48 high with its hairline: {}", second - first);
}

#[test]
fn with_none_the_page_says_so_and_offers_new_template() {
    let harness = on_templates(&[]);
    harness.get_by_label("No templates yet.");
    harness.get_by_label("New template");
}

#[test]
fn a_delete_asks_in_a_dialog_over_the_dimmed_window_and_runs_only_on_the_owner_s_press() {
    let mut harness = on_templates(&FIVE);
    press(&mut harness, "Delete the template Away from the studio");
    harness.get_by_role_and_label(Role::Heading, "Delete the template “Away from the studio”?");
    harness.get_by_label("It goes from Studio.");
    harness.get_by_label("Drafts and sent messages that were written with it keep their text.");
    assert!(harness.state().surfaces(&harness.ctx).modal(), "the dialog is a modal surface");
    assert_eq!(focused(&harness).as_deref(), Some("Cancel"), "the confirmation takes the keyboard on Cancel");
    assert_eq!(kept(&mut harness).len(), 5, "asking deletes nothing");
    // A hover and the keyboard on "Delete template" run nothing.
    harness.get_by_label("Delete template").hover();
    harness.run();
    key(&mut harness, Key::Tab, Modifiers::NONE);
    assert_eq!(focused(&harness).as_deref(), Some("Delete template"));
    assert_eq!(kept(&mut harness).len(), 5, "a focus or a hover deletes nothing");
    // Esc is the confirmation's: it closes, the template stays, the keyboard goes back to its delete.
    key(&mut harness, Key::Escape, Modifiers::NONE);
    assert!(harness.query_by_label("It goes from Studio.").is_none());
    assert!(!harness.state().surfaces(&harness.ctx).modal());
    assert_eq!(focused(&harness).as_deref(), Some("Delete the template Away from the studio"));
    assert!(harness.state().settings().is_open(), "Esc closes the confirmation only");
    assert_eq!(kept(&mut harness).len(), 5);
    // The owner's press of the confirmation deletes it.
    press(&mut harness, "Delete the template Away from the studio");
    press(&mut harness, "Delete template");
    let names = kept(&mut harness);
    assert!(!names.iter().any(|name| name == "Away from the studio"));
    assert_eq!(names.len(), 4, "one press, one delete: {names:?}");
    assert!(harness.query_by_label("Delete the template Away from the studio").is_none());
    harness.get_by_label_contains("] Studio · 3");
    harness.run();
    assert_eq!(focused(&harness).as_deref(), Some("New template"), "the keyboard is not left on nothing");
}

#[test]
fn cancel_keeps_the_template() {
    let mut harness = on_templates(&FIVE);
    press(&mut harness, "Delete the template Out of town");
    press(&mut harness, "Cancel");
    assert_eq!(kept(&mut harness).len(), 5);
    assert!(harness.query_by_label("It goes from Personal.").is_none());
}

#[test]
fn new_template_saves_what_was_typed_on_the_owner_s_press() {
    let mut harness = on_templates(&FIVE);
    press(&mut harness, "New template");
    harness.get_by_role_and_label(Role::Heading, "New template");
    assert_eq!(focused(&harness).as_deref(), Some("Name"), "the form opens on the name");
    harness.get_by_label("Name").type_text("Holiday hours");
    harness.run();
    assert_eq!(kept(&mut harness).len(), 5, "typing saves nothing");
    press(&mut harness, "Save");
    assert!(kept(&mut harness).iter().any(|name| name == "Holiday hours"));
    harness.get_by_label("Edit the template Holiday hours");
}

#[test]
fn an_edit_that_renames_leaves_no_old_template_and_a_taken_name_says_save_replaces_it() {
    let mut harness = on_templates(&FIVE);
    press(&mut harness, "Edit the template Workshop invitation");
    harness.get_by_role_and_label(Role::Heading, "Edit a template");
    let form = harness.state_mut().settings_mut().templates_mut().form_mut().unwrap();
    form.name = "Quote for a shop front".to_owned();
    harness.run();
    harness.get_by_label("Studio has a template named “Quote for a shop front”: Save replaces it.");
    let form = harness.state_mut().settings_mut().templates_mut().form_mut().unwrap();
    form.name = "Workshop, autumn".to_owned();
    harness.run();
    press(&mut harness, "Save");
    let names = kept(&mut harness);
    assert!(names.iter().any(|name| name == "Workshop, autumn"));
    assert!(!names.iter().any(|name| name == "Workshop invitation"), "the old name went: {names:?}");
    assert_eq!(names.len(), 5);
}

#[test]
fn hostile_names_and_texts_are_one_clean_row() {
    let long = "x".repeat(10_000);
    let nasty = "Bad\r\nname\u{0}\u{202E}gnol";
    let text = format!("line\r\none\u{0}\u{2066}{long}");
    let mut harness = on_templates(&[(2, nasty, "Subject\r\nBcc: someone@example.org", &text), (2, "Plain", "", "")]);
    let row = harness.get_by_label_contains("Delete the template Bad").rect();
    let plain = harness.get_by_label("Delete the template Plain").rect();
    assert!((plain.center().y - row.center().y - 49.0).abs() <= 1.0, "the hostile row is one row high");
    assert!(row.right() <= 1440.0);
    let template = harness.state().settings().templates().rows().iter().find(|row| row.name != "Plain").unwrap();
    assert!(template.line.chars().count() <= 240, "the line is capped where the row is built");
    assert!(!template.line.chars().any(char::is_control));
    // Its delete still deletes it by its kept name.
    harness.get_by_label_contains("Delete the template Bad").click();
    harness.run();
    // Its dialog's question is one clean line of plain words.
    let question = harness.get_by_label_contains("Delete the template “Bad").accesskit_node().label().unwrap_or_default();
    assert!(question.starts_with("Delete the template “Bad namegnol”"), "{question:?}");
    assert!(!question.chars().any(|c| c.is_control() || c == '\u{202E}'), "{question:?}");
    let (drawn, _) = painted(&harness, "Delete the template “Bad").expect("the question is drawn");
    assert!(drawn.height() <= 30.0 && drawn.right() <= 480.0 + 455.0, "one line inside the card: {drawn:?}");
    press(&mut harness, "Delete template");
    assert_eq!(kept(&mut harness), vec!["Plain".to_owned()]);
}

#[test]
fn the_keyboard_walks_the_page_in_its_drawn_order() {
    let mut harness = on_templates(&FIVE);
    press(&mut harness, "New template");
    press(&mut harness, "Cancel");
    assert_eq!(focused(&harness).as_deref(), Some("New template"), "the closed form hands the keyboard back");
    let mut walked = Vec::new();
    for _ in 0..10 {
        key(&mut harness, Key::Tab, Modifiers::NONE);
        walked.push(focused(&harness).unwrap_or_default());
    }
    let mut wanted = Vec::new();
    // Personal (account 1) comes before Studio, as `template.list` answers.
    for (_, name, _, _) in FIVE.iter().skip(4).chain(FIVE.iter().take(4)) {
        wanted.push(format!("Edit the template {name}"));
        wanted.push(format!("Delete the template {name}"));
    }
    assert_eq!(walked, wanted);
    for _ in 0..9 {
        key(&mut harness, Key::Tab, Modifiers::SHIFT);
    }
    assert_eq!(focused(&harness).as_deref(), Some("Edit the template Out of town"));
}

#[test]
fn a_row_read_from_a_hostile_store_is_plain_and_capped() {
    let text = format!("a\r\nb\u{0}\u{202E}{}", "\u{301}".repeat(10_000));
    let row = crate::templates::TemplateRow::read(
        &json!({ "account": 1, "name": format!("N\u{202E}\r\n{}", "y".repeat(10_000)), "subject": "S\u{0}", "text": text }),
    )
    .unwrap();
    assert!(row.shown.chars().count() <= 100 && !row.shown.chars().any(|c| c.is_control() || c == '\u{202E}'));
    assert!(row.line.chars().count() <= 240 && !row.line.chars().any(|c| c.is_control() || c == '\u{202E}'));
    assert_eq!(crate::templates::TemplateRow::read(&json!({ "account": 1, "name": " \r\n " })), None, "a blank name is no row");
}

#[test]
fn the_page_rests() {
    let mut harness = on_templates(&FIVE);
    assert!(crate::component::tests::rests(&mut harness, 4), "the list rests");
    press(&mut harness, "Delete the template Out of town");
    assert!(crate::component::tests::rests(&mut harness, 4), "the confirmation rests");
    press(&mut harness, "Cancel");
    press(&mut harness, "New template");
    harness.state_mut().settings_mut().leave();
    harness.run();
    assert!(crate::component::tests::rests(&mut harness, 4), "the form left rests");
}

#[test]
fn a_kept_list_of_junk_reads_as_no_rows_not_a_panic() {
    let mut templates = crate::templates::Templates::default();
    for junk in [Value::Null, json!({ "templates": 3 }), json!({ "templates": [1, "a", { "name": 5 }, { "account": 1 }] })] {
        templates.read(&junk);
        assert!(templates.rows().is_empty(), "{junk}");
    }
}

#[test]
fn shift_tab_round_a_long_list_reaches_the_last_template_and_keeps_it() {
    let names: Vec<String> = (0..60).map(|n| format!("T{n:02}")).collect();
    let many: Vec<(u64, &str, &str, &str)> = names.iter().map(|name| (2, name.as_str(), "s", "t")).collect();
    let mut harness = on_templates(&many);
    let last = format!("Delete the template {}", harness.state().settings().templates().rows().last().unwrap().shown);
    harness.get_by_label("Command line").focus();
    harness.run();
    key(&mut harness, Key::Tab, Modifiers::SHIFT);
    assert_eq!(focused(&harness).as_deref(), Some(last.as_str()), "the last row, out of sight, is reached and kept as the page scrolls");
    key(&mut harness, Key::Tab, Modifiers::SHIFT);
    assert!(focused(&harness).is_some_and(|label| label.starts_with("Edit the template")), "and the edit before it");
}

#[test]
fn a_held_key_deletes_at_most_once_and_never_by_its_repeats() {
    use crate::component::tests::{held_on, hold_key};
    for key in [Key::Enter, Key::Space] {
        let mut harness = on_templates(&FIVE);
        // Held on a row's delete: the press asks, the keyboard lands on Cancel, and the repeats delete nothing.
        harness.get_by_label("Delete the template Out of town").focus();
        harness.run();
        hold_key(&mut harness, key, 6);
        assert_eq!(kept(&mut harness).len(), 5, "a held {key:?} on a row's delete deletes nothing");
        // A key already down when the danger button takes the keyboard: its repeats are no press.
        if harness.query_by_label("Delete template").is_none() {
            press(&mut harness, "Delete the template Out of town");
        }
        held_on(&mut harness, key, 6, |harness| harness.get_by_label("Delete template").focus());
        assert_eq!(kept(&mut harness).len(), 5, "repeats of a held {key:?} on Delete template delete nothing");
        // A fresh press, held: one delete, whatever the keyboard lands on next.
        harness.get_by_label("Delete template").focus();
        harness.run();
        hold_key(&mut harness, key, 6);
        assert_eq!(kept(&mut harness).len(), 4, "a held {key:?} on Delete template deletes once");
    }
}

#[test]
fn a_rename_in_case_only_keeps_the_template_renamed() {
    let mut harness = on_templates(&FIVE);
    press(&mut harness, "Edit the template Out of town");
    let form = harness.state_mut().settings_mut().templates_mut().form_mut().unwrap();
    form.name = "out of Town".to_owned();
    harness.run();
    assert!(harness.query_by_label_contains("Save replaces it").is_none(), "its own name in another case replaces nothing");
    press(&mut harness, "Save");
    let names = kept(&mut harness);
    assert!(names.iter().any(|name| name == "out of Town"), "the template is kept, renamed: {names:?}");
    assert!(!names.iter().any(|name| name == "Out of town"));
    assert_eq!(names.len(), 5);
}

#[test]
fn a_name_taken_in_another_case_says_save_replaces_it() {
    let mut harness = on_templates(&FIVE);
    press(&mut harness, "Edit the template Workshop invitation");
    let form = harness.state_mut().settings_mut().templates_mut().form_mut().unwrap();
    form.name = "quote for a SHOP front".to_owned();
    harness.run();
    harness.get_by_label("Studio has a template named “quote for a SHOP front”: Save replaces it.");
}

/// Every text painted in the last frame that starts with `wanted`: where it is drawn and its colour.
fn painted(harness: &Harness<'static, InboxApp>, wanted: &str) -> Option<(egui::Rect, egui::Color32)> {
    fn find(shape: &egui::Shape, wanted: &str) -> Option<(egui::Rect, egui::Color32)> {
        match shape {
            egui::Shape::Vec(inner) => inner.iter().find_map(|shape| find(shape, wanted)),
            egui::Shape::Text(text) if text.galley.text().starts_with(wanted) => {
                let colour = text.galley.job.sections.first().map_or(text.fallback_color, |section| section.format.color);
                Some((egui::Rect::from_min_size(text.pos, text.galley.size()), colour))
            }
            _ => None,
        }
    }
    harness.output().shapes.iter().find_map(|clipped| find(&clipped.shape, wanted))
}

#[test]
fn a_row_s_parts_stand_twelve_apart_and_its_line_is_370_wide_as_the_frame() {
    // The frame's row: the bar at 16, the name at 31 (200), the line at 243 (370), edit at 625, delete at 664 to 704.
    let harness = on_templates(&FIVE);
    let (name, _) = painted(&harness, "Workshop invitation").expect("the name is drawn");
    let (line, _) = painted(&harness, "Thursday workshop").expect("the line is drawn");
    let edit = harness.get_by_label("Edit the template Workshop invitation").rect();
    let delete = harness.get_by_label("Delete the template Workshop invitation").rect();
    let column = name.left() - 31.0;
    assert!((line.left() - column - 243.0).abs() <= 0.5, "the line 12 after the name's 200: {}", line.left() - column);
    assert!((edit.left() - column - 625.0).abs() <= 1.0, "edit 12 after the line's 370: {}", edit.left() - column);
    assert!((delete.left() - edit.right() - 12.0).abs() <= 1.0, "12 between the links: {}", delete.left() - edit.right());
    assert!((delete.right() - column - 704.0).abs() <= 1.0, "16 in from the column's end: {}", delete.right() - column);
}

#[test]
fn the_intro_says_what_the_frame_says() {
    let harness = on_templates(&FIVE);
    let intro = "A template is a subject and a text kept on an account; the composer's Templates menu puts one into a draft. \
                 {name}, {first_name} and {my_name} are filled in; {?prompt} leaves a blank the draft asks for.";
    assert!(painted(&harness, intro).is_some(), "the frame's intro");
}

#[test]
fn the_form_s_account_stands_200_wide_beside_the_name_16_on_as_the_frame() {
    let mut harness = on_templates(&FIVE);
    press(&mut harness, "Edit the template Workshop invitation");
    let account = harness.get_by_role_and_label(Role::ComboBox, "Account").rect();
    let name = harness.get_by_role_and_label(Role::TextInput, "Name").rect();
    assert!((account.width() - 200.0).abs() <= 1.0 && (account.height() - 36.0).abs() <= 1.0, "the select 200 × 36: {account:?}");
    assert!((name.left() - account.left() - 216.0).abs() < 0.5, "the name 16 after the select: {}", name.left() - account.left());
    assert!((name.top() - account.top()).abs() <= 1.0 && (name.height() - 36.0).abs() <= 1.0, "one row of 36: {account:?} {name:?}");
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

#[test]
fn a_press_on_the_dimmed_window_keeps_the_template_and_presses_nothing_under_it() {
    let mut harness = on_templates(&FIVE);
    let new = harness.get_by_label("New template").rect().center();
    press(&mut harness, "Delete the template Away from the studio");
    harness.get_by_label("It goes from Studio.");
    press_at(&mut harness, new);
    assert!(harness.query_by_label("It goes from Studio.").is_none(), "the press on the scrim closes the dialog");
    assert!(harness.state().settings().templates().form().is_none(), "New template under the scrim is not pressed");
    assert_eq!(kept(&mut harness).len(), 5, "the template stays");
}

#[test]
fn the_dialog_stands_where_the_frame_draws_it() {
    // The frame `settings-templates/delete-confirm`: the card 480 wide in the middle of the window, its question 25 in.
    let mut harness = on_templates(&FIVE);
    press(&mut harness, "Delete the template Away from the studio");
    let (question, colour) = painted(&harness, "Delete the template “Away").expect("the question is drawn");
    assert_eq!(colour, crate::theme::Theme::new(crate::theme::Mode::Dark, crate::theme::TextScale::default()).palette.ink);
    assert!((question.left() - (480.0 + 25.0)).abs() <= 0.5, "{question:?}");
    let cancel = harness.get_by_label("Cancel").rect();
    let delete = harness.get_by_label("Delete template").rect();
    assert!((cancel.left() - (480.0 + 28.0)).abs() <= 0.5 && (delete.left() - cancel.right() - 12.0).abs() <= 0.5, "{cancel:?} {delete:?}");
}

#[test]
fn a_screen_reader_s_activate_under_the_dialog_runs_nothing_of_the_page() {
    // egui's layers stop the pointer, not a screen reader's activate: the window's gate does.
    let mut harness = on_templates(&FIVE);
    press(&mut harness, "Delete the template Away from the studio");
    harness.get_by_label("New template").click_accesskit();
    harness.run();
    harness.get_by_label("Edit the template Workshop invitation").click_accesskit();
    harness.run();
    assert!(harness.state().settings().templates().form().is_none(), "New template under the dialog opens no form");
    assert_eq!(harness.state().settings().templates().confirming(), Some((2, "Away from the studio")), "the question stays as it was asked");
    harness.get_by_label("It goes from Studio.");
    assert_eq!(kept(&mut harness).len(), 5);
}
