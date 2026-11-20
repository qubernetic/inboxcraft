// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The settings pages as the design lays them out: the sections in its order, the pages as section labels over
//! settings rows, coming soon where nothing is built, and a place the keyboard walks.

use egui::accesskit::Role;
use egui::{Key, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_time::{FakeClock, Timestamp};

use super::super::InboxApp;
use crate::mailbox::{InboxView, Mailbox};
use crate::sample;
use crate::settings::Section;

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

fn harness() -> Harness<'static, InboxApp> {
    let app = InboxApp::at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    let mut harness =
        Harness::builder().with_size(vec2(1440.0, 900.0)).with_theme(egui::Theme::Dark).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

fn opened() -> Harness<'static, InboxApp> {
    let mut harness = harness();
    harness.get_by_role_and_label(egui::accesskit::Role::Button, "Settings").click();
    harness.run();
    assert!(harness.state().settings().is_open());
    harness
}

const NAMES: [&str; 11] =
    ["Appearance", "Accounts", "Notifications", "Triage", "Sending", "Calendar", "Shortcuts", "Privacy", "About", "AI", "Plugins"];

/// The label of the node with the keyboard focus, if one has it.
fn focused(harness: &Harness<'static, InboxApp>) -> Option<(String, egui::Rect)> {
    harness.root().children_recursive().find(|node| node.accesskit_node().is_focused()).map(|node| {
        let label = node.accesskit_node().label().unwrap_or_default();
        (label, node.rect())
    })
}

#[test]
fn the_nine_sections_stand_in_the_design_s_order() {
    let mut harness = opened();
    // The design counts its nine; AI and Plugins come after them.
    harness.get_by_label("11 sections");
    let tops: Vec<f32> = NAMES.iter().map(|name| harness.get_by_label(&format!("{name}, settings page")).rect().top()).collect();
    assert!(tops.windows(2).all(|pair| pair[0] < pair[1]), "the design's order, then AI and Plugins: {tops:?}");
    let first = harness.get_by_label("Appearance, settings page").rect();
    assert!((first.width() - 232.0).abs() <= 1.0, "the sections' column is 232 wide: {}", first.width());
    for (name, section) in NAMES.iter().zip(Section::ALL) {
        let label = format!("{name}, settings page");
        let row = harness.get_by_label(&label);
        assert!(!row.accesskit_node().is_disabled(), "{name} opens");
        row.click();
        harness.run();
        assert_eq!(harness.state().settings().section(), section);
        harness.get_by_label(&format!("[ 04 ] Settings · {name}"));
        let title = harness.get_by_role_and_label(Role::Heading, name).rect();
        // The page's column: 720 wide, its rows as wide.
        let sheet = harness.get_by_role_and_label(Role::Pane, "Settings").rect();
        assert!(title.left() > first.right() + 40.0 && title.right() < sheet.right(), "{name}");
        let planned = matches!(section, Section::About | Section::Ai | Section::Plugins);
        let tags = harness.query_all_by_label("coming soon").filter(|tag| (tag.rect().center().y - title.center().y).abs() < 4.0).count();
        assert_eq!(tags, usize::from(planned), "{name}: the coming-soon mark on the head of a planned section only");
    }
    harness.get_by_label("Appearance, settings page").click();
    harness.run();
    let row = harness.get_by_role_and_label(Role::Group, "Mode. Auto follows the computer's light or dark choice.").rect();
    assert!((row.width() - 720.0).abs() <= 1.0, "a row fills the 720 column: {}", row.width());
}

#[test]
fn the_sections_column_stands_as_the_design_draws_it() {
    let harness = opened();
    let sheet = harness.get_by_role_and_label(Role::Pane, "Settings").rect();
    // From the body's top left: the head 48, its hairline, 8, then the rows 30 high one under the other.
    let mut top = sheet.top() + 48.0 + 1.0 + 8.0;
    for name in NAMES {
        let row = harness.get_by_label(&format!("{name}, settings page")).rect();
        assert_eq!((row.top(), row.height()), (top, 30.0), "{name}");
        assert_eq!((row.left(), row.width()), (sheet.left(), 232.0), "{name}");
        top = row.bottom();
    }
    // A place has no Close: the design's head has none.
    assert!(harness.query_by_role_and_label(Role::Button, "Close").is_none(), "no Close");
}

#[test]
fn inbox_view_is_set_on_appearance_and_not_on_triage() {
    let mut harness = opened();
    harness.get_by_label("[ B ] Layout");
    let view = harness.get_by_role_and_label(
        Role::Group,
        "Inbox view. Grouped opens every group, Smart shows the first three rows of each, List is one list by time.",
    );
    let size = harness.get_by_role_and_label(Role::Group, "Text size. 90 % to 130 %. Rows and bars follow the type.");
    assert!(view.rect().top() > size.rect().bottom() - 1.0, "Layout follows Look");
    let x = |label: &str| harness.get_by_role_and_label(Role::RadioButton, label).rect().center().x;
    assert!(x("Grouped") < x("Smart") && x("Smart") < x("List"));
    for (label, view) in [("Grouped", InboxView::Grouped), ("List", InboxView::List), ("Smart", InboxView::Smart)] {
        harness.get_by_role_and_label(Role::RadioButton, label).click();
        harness.run();
        assert_eq!(harness.state().mailbox().inbox_view(), view, "{label}");
    }
    // One stored value: the list's switch (out of the settings, where the list is) changes it, and the page shows
    // the new view.
    harness.state_mut().settings_mut().leave();
    harness.run();
    let ctx = harness.ctx.clone();
    harness.state_mut().run_action(&ctx, crate::keymap::VIEW_LIST);
    harness.run();
    assert_eq!(harness.state().mailbox().inbox_view(), InboxView::List);
    harness.state_mut().settings_mut().open_at(Section::Appearance);
    harness.run();
    assert!(harness.get_by_role_and_label(Role::RadioButton, "List").accesskit_node().toggled() == Some(egui::accesskit::Toggled::True));
    assert!(harness.get_by_role_and_label(Role::RadioButton, "Smart").accesskit_node().toggled() != Some(egui::accesskit::Toggled::True));
    harness.get_by_label("Triage, settings page").click();
    harness.run();
    assert_eq!(harness.state().settings().section(), Section::Triage);
    for gone in ["Grouped", "Smart", "List", "Cards", "Grouped list", "Simple list"] {
        assert!(harness.query_by_role_and_label(Role::RadioButton, gone).is_none(), "{gone} is not on Triage");
    }
    assert!(harness.query_all_by_label_contains("Inbox view").next().is_none());
}

#[test]
fn a_coming_soon_row_takes_no_focus_and_no_press() {
    let mut harness = opened();
    let soon: Vec<_> =
        harness.query_all_by_label_contains("for Open a message").map(|node| (node.accesskit_node().is_disabled(), node.rect())).collect();
    assert_eq!(soon.len(), 2, "Pane and Window");
    assert!(soon.iter().all(|(disabled, _)| *disabled));
    let before = harness.state().mailbox().last_run().map(str::to_owned);
    let (_, window) = soon[1];
    harness.hover_at(window.center());
    harness.run();
    for pressed in [true, false] {
        harness.event(egui::Event::PointerButton {
            pos: window.center(),
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run();
    }
    assert_eq!(harness.state().mailbox().last_run().map(str::to_owned), before, "a press runs nothing");
    // Tab goes round the page and never lands on a coming-soon control.
    let mut seen = Vec::new();
    for _ in 0..40 {
        harness.key_press(Key::Tab);
        harness.run();
        if let Some((label, _)) = focused(&harness) {
            seen.push(label);
        }
    }
    assert!(!seen.is_empty());
    assert!(seen.iter().all(|label| !label.contains("Open a message") && !label.contains("Write a message")), "{seen:?}");
    // A planned section: every control on it is unavailable.
    harness.get_by_label("AI, settings page").click();
    harness.run();
    for label in ["None for Where the model runs", "Elsewhere for Where the model runs", "Send mail to a model elsewhere"] {
        assert!(harness.get_by_label(label).accesskit_node().is_disabled(), "{label}");
    }
}

/// Where the control with the keyboard stands: the index (and the command line) left of the settings, the sections'
/// column, or the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Region {
    Index,
    Sections,
    Page,
}

fn region(harness: &Harness<'static, InboxApp>) -> Option<(String, Region)> {
    let place = harness.get_by_role_and_label(Role::Pane, "Settings").rect();
    focused(harness).map(|(label, rect)| {
        let x = rect.center().x;
        let region = if x < place.left() {
            Region::Index
        } else if x < place.left() + 232.0 {
            Region::Sections
        } else {
            Region::Page
        };
        (label, region)
    })
}

#[test]
fn the_keyboard_walks_the_index_the_sections_and_the_page_and_back() {
    let mut harness = harness();
    let index = "Settings";
    harness.get_by_role_and_label(Role::Button, index).focus();
    harness.run();
    harness.key_press(Key::Enter);
    harness.run();
    assert!(harness.state().settings().is_open());
    assert_eq!(region(&harness), Some((index.to_owned(), Region::Index)), "the index's row keeps the keyboard");
    // Tab: on from the index to the sections' rows, then on to the page; never back to a section before the page ends.
    let mut walk = Vec::new();
    for _ in 0..30 {
        harness.key_press(Key::Tab);
        harness.run();
        if let Some(stop) = region(&harness) {
            walk.push(stop);
        }
    }
    let regions: Vec<Region> = walk.iter().map(|(_, region)| *region).collect();
    let first_section = regions.iter().position(|region| *region == Region::Sections).unwrap();
    let first_page = regions.iter().position(|region| *region == Region::Page).unwrap();
    assert!(regions[..first_section].iter().all(|region| *region == Region::Index), "the index first: {walk:?}");
    assert_eq!(first_page - first_section, NAMES.len(), "every section's row, then the page: {walk:?}");
    // Then the page, until Tab comes round to the window's first control.
    let page = regions[first_page..].iter().take_while(|region| **region != Region::Index);
    assert!(page.clone().count() > 1 && page.clone().all(|region| *region == Region::Page), "then the page: {walk:?}");
    // Shift+Tab: back from the page's first stop to the last section, and from the first section to the index.
    let first_stop = walk[first_page].0.clone();
    harness.get_by_label(&first_stop).focus();
    harness.run();
    assert_eq!(region(&harness), Some((first_stop, Region::Page)), "on the page's first stop");
    harness.key_press_modifiers(egui::Modifiers::SHIFT, Key::Tab);
    harness.run();
    assert_eq!(region(&harness), Some(("Plugins, settings page".to_owned(), Region::Sections)), "back to the last section");
    for _ in 0..NAMES.len() {
        harness.key_press_modifiers(egui::Modifiers::SHIFT, Key::Tab);
        harness.run();
    }
    assert_eq!(region(&harness).map(|(_, region)| region), Some(Region::Index), "and back to the index");
    // The index's SETTINGS pressed again shows the mail; the keyboard stays on the row.
    harness.get_by_role_and_label(Role::Button, index).focus();
    harness.run();
    harness.key_press(Key::Enter);
    harness.run();
    assert!(!harness.state().settings().is_open());
    assert_eq!(focused(&harness).map(|(label, _)| label).as_deref(), Some(index), "the keyboard is where it was");
}

/// The controls on the sheet that can be pressed (not disabled, not text or a scroll bar), by node, with their names; a
/// radio button reached through its group is left out.
fn pressable_on_sheet(harness: &Harness<'static, InboxApp>, sheet: egui::Rect) -> Vec<(egui::accesskit::NodeId, String)> {
    harness
        .root()
        .children_recursive()
        .filter(|node| {
            let data = node.accesskit_node();
            let actions = data.data();
            (actions.supports_action(egui::accesskit::Action::Click) || actions.supports_action(egui::accesskit::Action::Focus))
                && !data.is_disabled()
                && !data.is_hidden()
                // Text, which egui makes focusable for selecting, is no Tab stop.
                && data.role() != Role::Label
                && data.role() != Role::Heading
                && data.role() != Role::ScrollBar
                && !(data.role() == Role::RadioButton && !actions.supports_action(egui::accesskit::Action::Focus))
                // On the sheet: not the dim under it, nor what the window holds beneath.
                && sheet.contains_rect(node.rect())
        })
        .map(|node| (node.accesskit_node().locate().0, node.accesskit_node().label().unwrap_or_default()))
        .collect()
}

#[test]
fn tab_reaches_every_control_of_every_page_and_each_page_rests() {
    for theme in [egui::Theme::Dark, egui::Theme::Light] {
        for (name, section) in NAMES.iter().zip(Section::ALL) {
            let app = InboxApp::at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock()));
            let mut harness =
                Harness::builder().with_size(vec2(1200.0, 2400.0)).with_theme(theme).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
            harness.state_mut().settings_mut().open_at(section);
            harness.run();
            assert!(crate::component::tests::rests(&mut harness, 4), "{name} rests");
            let sheet = harness.get_by_role_and_label(Role::Pane, "Settings").rect();
            let pressable = pressable_on_sheet(&harness, sheet);
            assert!(pressable.len() >= Section::ALL.len(), "{name}: the sections at least");
            let mut reached = Vec::new();
            for _ in 0..400 {
                harness.key_press(Key::Tab);
                harness.step();
                harness.step();
                let now =
                    harness.root().children_recursive().find(|node| node.accesskit_node().is_focused()).map(|node| node.accesskit_node().locate().0);
                match now {
                    Some(id) if reached.contains(&id) => break,
                    Some(id) => reached.push(id),
                    None => {}
                }
            }
            let missed: Vec<&String> = pressable.iter().filter(|(id, _)| !reached.contains(id)).map(|(_, label)| label).collect();
            assert!(missed.is_empty(), "{name} in {theme:?}: Tab never reaches {missed:?}");
        }
    }
}

#[test]
fn hostile_account_names_never_break_a_page() {
    let mut app = InboxApp::at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    // The engine cleans a name (controls out, at most its length): what is left is still long, wide and without spaces.
    let ids: Vec<u64> = app.mailbox().accounts().iter().map(|account| account.id).collect();
    assert!(ids.len() >= 2, "more than one account");
    for (n, id) in ids.iter().enumerate() {
        let label = format!("Allotment{n}\u{202e}\r\n\u{0}\u{7}{}", "Ω".repeat(400));
        let answer = app.mailbox_mut().run("account.update", &serde_json::json!({ "account": id, "label": label }));
        assert!(answer.is_ok(), "{answer:?}");
    }
    app.mailbox_mut().refresh();
    let kept = app.mailbox().accounts().iter().map(|account| account.label.chars().count()).min().unwrap_or(0);
    assert!(kept >= 60, "long names: {kept} characters");
    for size in [vec2(1200.0, 820.0), vec2(900.0, 640.0), vec2(820.0, 600.0)] {
        for scale in [crate::theme::TextScale::default(), crate::theme::TextScale::from_percent(130).unwrap()] {
            let mut harness = Harness::builder()
                .with_size(size)
                .with_context_setup(crate::theme::install_fonts)
                .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
            harness.state_mut().set_text_scale(scale);
            for section in Section::ALL {
                harness.state_mut().settings_mut().open_at(section);
                harness.run();
                // Every text painted on the settings lies inside the row or button it belongs to, inside its pane, and
                // over no other text.
                let place = harness.get_by_role_and_label(Role::Pane, "Settings").rect();
                let found = super::super::fit_tests::misfits_within(&harness, place);
                assert!(found.is_empty(), "{section:?} at {size:?}, {scale:?}:\n{}", found.join("\n"));
            }
            app = harness.into_state();
        }
    }
}

/// Where a page's words end in the settings `place`: 16 in from the page column's right end.
fn words_end(place: egui::Rect) -> f32 {
    super::Columns::of(place, Section::Appearance).page.right() - 16.0
}

#[test]
fn the_accounts_and_shortcuts_pages_take_the_design_s_values() {
    let app = InboxApp::at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    let mut harness = Harness::builder()
        .with_size(vec2(1440.0, 900.0))
        .with_context_setup(crate::component::tests::record_layers)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    // Shortcuts: the field 320 wide with its own label, Reset all keys 2 over its foot at the column's right end, the
    // list's first hairline 12 under them and the first chip 8 into its 36 row.
    harness.state_mut().settings_mut().open_at(Section::Shortcuts);
    harness.run();
    let sheet = harness.get_by_role_and_label(Role::Pane, "Settings").rect();
    let find = harness.get_by_role_and_label(Role::TextInput, "Find a command").rect();
    assert_eq!(find.width(), 320.0, "{find:?}");
    let title = harness.get_by_role_and_label(Role::Heading, "Shortcuts").rect();
    // The head's foot (its intro, a line of 22 under 6), 16, the label 16 and 8, then the box.
    assert_eq!(find.top(), title.top() - 1.0 + 32.0 + 6.0 + 22.0 + 16.0 + 16.0 + 8.0, "{find:?} under {title:?}");
    let reset = harness.get_by_role_and_label(Role::Button, "Reset all keys").rect();
    assert_eq!((reset.bottom(), reset.right()), (find.bottom() - 2.0, words_end(sheet)), "{reset:?}");
    let chip = harness.get_all_by_role(Role::Button).find(|node| node.accesskit_node().label().unwrap_or_default().starts_with("Key of ")).unwrap();
    assert_eq!(chip.rect().top(), find.bottom() + 12.0 + 1.0 + 8.0, "{:?}", chip.rect());
    // Accounts: the bar 3 × 12 at 16 and 18 in its 48 row, the address at 191, "open" a small link 16 from the end.
    harness.state_mut().settings_mut().open_at(Section::Accounts);
    harness.run();
    let account = harness.state().mailbox().accounts()[0].clone();
    let label = format!("{}, {}", account.label, account.address);
    let row = harness.get_by_label(&label).rect();
    assert_eq!(row.height(), 48.0);
    let paint = crate::component::tests::painted(&harness, &label);
    let bars: Vec<egui::Rect> = paint.rects().map(|shape| shape.rect).filter(|rect| rect.width() == 3.0).collect();
    assert_eq!(bars, [egui::Rect::from_min_size(row.min + vec2(16.0, 18.0), vec2(3.0, 12.0))], "the bar");
    let address = paint
        .shapes
        .iter()
        .find(|shape| matches!(&shape.look, crate::component::tests::Look::Text { text, .. } if *text == account.address))
        .unwrap()
        .rect;
    assert_eq!(address.left(), row.left() + 191.0, "the address");
    let open = harness.get_by_label(&format!("Open the settings of {}", account.label)).rect();
    assert!((open.right() - (row.right() - 16.0)).abs() <= 1.0 && (open.center().y - row.center().y).abs() <= 0.5, "{open:?} in {row:?}");
    // Triage names the design's groups.
    harness.state_mut().settings_mut().open_at(Section::Triage);
    harness.run();
    assert!(harness.query_all_by_label_contains("in People, Notifications and Newsletters,").next().is_some());
}

#[test]
fn calendar_rows_stand_56_apart_with_read_again_at_their_end_and_about_names_the_icons() {
    let mut harness = harness();
    harness.state_mut().settings_mut().open_at(Section::Calendar);
    harness.run();
    let sheet = harness.get_by_role_and_label(Role::Pane, "Settings").rect();
    let links: Vec<egui::Rect> = harness.query_all_by_label("read again").map(|node| node.rect()).collect();
    assert_eq!(links.len(), harness.state().mailbox().accounts().len(), "one row per account");
    for link in &links {
        assert!((link.right() - words_end(sheet)).abs() <= 1.0, "{link:?}");
    }
    for pair in links.windows(2) {
        assert_eq!(pair[1].center().y - pair[0].center().y, 56.0, "{pair:?}");
    }
    harness.state_mut().settings_mut().open_at(Section::About);
    harness.run();
    assert!(harness.query_all_by_label_contains("· Phosphor. The typefaces under the SIL Open Font Licence, the icons under MIT.").next().is_some());
}

#[test]
fn a_window_too_small_for_the_sheet_draws_it_without_a_fault() {
    for size in [vec2(10.0, 10.0), vec2(30.0, 400.0), vec2(400.0, 20.0)] {
        let app = InboxApp::at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock()));
        let mut harness = Harness::builder().with_size(size).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
        for section in Section::ALL {
            harness.state_mut().settings_mut().open_at(section);
            harness.run();
            let sheet = harness.get_by_role_and_label(Role::Pane, "Settings").rect();
            assert!(sheet.width() >= 0.0 && sheet.height() >= 0.0, "{size:?} {section:?}: {sheet:?}");
        }
    }
}

#[test]
fn a_page_with_many_accounts_lays_out_the_rows_in_sight_and_tab_still_reaches_every_one() {
    let mut app = InboxApp::at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    for n in 0..40 {
        let account = serde_json::json!({
            "address": format!("many{n}@example.org"),
            "imap": { "host": "imap.example.org", "security": "tls" },
        });
        app.mailbox_mut().run("account.add", &account).unwrap();
    }
    app.mailbox_mut().refresh();
    let accounts = app.mailbox().accounts().len();
    let mut harness = Harness::builder().with_size(vec2(1440.0, 900.0)).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.state_mut().settings_mut().open_at(Section::Sending);
    harness.run();
    let built = harness.query_all_by_role(Role::MultilineTextInput).count();
    assert!(built < accounts / 2, "{built} of {accounts} signature fields laid out");
    // Tab goes from field to field, the page scrolling along, and reaches the last account's.
    let last = format!("Signature of {}", harness.state().mailbox().accounts().last().unwrap().label);
    let mut reached = false;
    for _ in 0..(accounts * 3) {
        harness.key_press(Key::Tab);
        harness.run();
        if focused(&harness).is_some_and(|(label, _)| label == last) {
            reached = true;
            break;
        }
    }
    assert!(reached, "Tab reaches {last}");
}

#[test]
fn tab_to_a_control_below_the_page_s_view_scrolls_the_page_to_it() {
    let app = InboxApp::at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    // Too low for the whole Appearance page: its last row, the window frame, starts out of sight.
    let mut harness = Harness::builder().with_size(vec2(1100.0, 600.0)).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.state_mut().settings_mut().open_at(Section::Appearance);
    harness.run();
    let sheet = harness.get_by_role_and_label(Role::Pane, "Settings").rect();
    let mut seen = Vec::new();
    for _ in 0..60 {
        harness.key_press(Key::Tab);
        harness.run();
        // The index's and the command line's stops lie beside the settings.
        if let Some((label, rect)) = focused(&harness).filter(|(label, rect)| sheet.x_range().contains(rect.center().x) && label != "Command line") {
            assert!(sheet.contains_rect(rect), "{label} {rect:?} has the keyboard out of sight of the settings {sheet:?}");
            seen.push(label);
        }
    }
    assert!(seen.iter().any(|label| label == "Own / System"), "Tab reaches the window frame: {seen:?}");
}

/// The Appearance page of a fresh window, 1440 by 900 in dark.
fn appearance_page() -> Harness<'static, InboxApp> {
    let mut harness = harness();
    harness.state_mut().settings_mut().open_at(Section::Appearance);
    harness.run();
    harness
}

/// Whether the option `label` (`<option> for <setting>`) is the one chosen.
fn chosen(harness: &Harness<'static, InboxApp>, label: &str) -> bool {
    harness.get_by_label(label).accesskit_node().toggled() == Some(egui::accesskit::Toggled::True)
}

/// Where the top of the settings row `title` stands (its group is named `<title>. <help>`), or of the painted text
/// `title` (a group's label); infinite when neither is drawn.
fn row_top(harness: &Harness<'static, InboxApp>, title: &str) -> f32 {
    let row = format!("{title}. ");
    let rows = harness.query_all_by_label_contains(&row).filter(|node| node.accesskit_node().label().is_some_and(|label| label.starts_with(&row)));
    let painted = painted_texts(harness).into_iter().filter(|(text, _)| text == title).map(|(_, rect)| rect.top()).collect::<Vec<_>>();
    rows.map(|node| node.rect().top()).chain(painted).fold(f32::INFINITY, f32::min)
}

/// The motion level's help, as the design words it for each level.
const MOTION_HELP: [(&str, &str); 3] = [
    (
        "Full for Motion",
        "Full: the character field reacts wherever it shows, labels scramble and rows decode in. The showpiece; nothing turns it on but you.",
    ),
    (
        "Reduced for Motion",
        "Reduced keeps what explains a change and drops the text effects. Until you choose, the computer's reduce-motion setting gives Off.",
    ),
    (
        "Off for Motion",
        "Off: every change happens at once and the sync bar becomes words. It is what the computer's reduce-motion setting gives until you choose.",
    ),
];

#[test]
fn appearance_is_look_then_layout_as_the_default_frame_draws_it() {
    let harness = appearance_page();
    // `settings-appearance/default`: two groups, Look over Layout, the rows in the design's order.
    let order = ["LOOK", "Mode", "Motion", "Text size", "LAYOUT", "Inbox view", "Open a message", "Write a message", "Window frame"];
    let tops: Vec<f32> = order.iter().map(|title| row_top(&harness, title)).collect();
    assert!(tops.iter().all(|y| y.is_finite()), "every row is drawn: {order:?} {tops:?}");
    assert!(tops.windows(2).all(|pair| pair[0] < pair[1]), "in the design's order: {order:?} {tops:?}");
    // As the app starts: Auto, Reduced, 100 %, the inbox view kept (the sample's), pane, pane, own frame.
    let view = super::inbox_views().into_iter().find(|(view, _)| *view == harness.state().mailbox().inbox_view()).map(|(_, name)| name).unwrap();
    for label in ["Auto", "Reduced for Motion", view, "Pane for Open a message", "Pane for Write a message", "Own"] {
        assert!(harness.query_all_by_label(label).any(|node| node.accesskit_node().toggled() == Some(egui::accesskit::Toggled::True)), "{label}");
    }
    assert_eq!(harness.get_by_role_and_label(Role::Slider, "Text size").accesskit_node().numeric_value(), Some(100.0));
    harness.get_by_label_contains(MOTION_HELP[1].1);
    harness.get_by_label_contains("InboxCraft's own frame, where the command line is the title bar, or the desktop's title bar above it.");
    // Full is no longer coming: every level can be chosen; the two window choices still come later.
    for (level, _) in MOTION_HELP {
        assert!(!harness.get_by_label(level).accesskit_node().is_disabled(), "{level}");
    }
    for later in ["Window for Open a message", "Window for Write a message"] {
        assert!(harness.get_by_label(later).accesskit_node().is_disabled(), "{later}");
    }
    let motion = harness.get_by_label("Full for Motion").rect();
    assert!(
        harness.query_all_by_label("coming soon").all(|tag| (tag.rect().center().y - motion.center().y).abs() > 1.0),
        "no coming-soon mark on the motion row"
    );
}

#[test]
fn each_motion_level_says_what_it_does_and_nothing_under_it_moves() {
    // `settings-appearance/motion-full` and `motion-off`, and back to the default's Reduced.
    let mut harness = appearance_page();
    let text_size = harness.get_by_role_and_label(Role::Slider, "Text size").rect().top();
    for (level, help) in [MOTION_HELP[0], MOTION_HELP[2], MOTION_HELP[1]] {
        harness.get_by_label(level).click();
        harness.run();
        assert_eq!(harness.state().mailbox().last_run(), Some("ui.motion.set"));
        assert!(chosen(&harness, level), "{level}");
        harness.get_by_label_contains(help);
        for (other, other_help) in MOTION_HELP.iter().filter(|(other, _)| *other != level) {
            assert!(!chosen(&harness, other), "{level}: {other} is not chosen");
            assert!(harness.query_by_label_contains(other_help).is_none(), "{level}: one help");
        }
        let now = harness.get_by_role_and_label(Role::Slider, "Text size").rect().top();
        assert!((now - text_size).abs() < 0.5, "{level}: the rows below stay ({text_size} then {now})");
    }
    assert_eq!(harness.state().appearance().motion_level(), inboxcraft_engine::appearance::MotionLevel::Reduced);
    assert!(harness.state().appearance().motion_chosen());
}

#[test]
fn full_runs_today_s_motion_and_reduced_and_off_land_it_at_once() {
    use crate::motion::{Level, level, reduced};
    let mut harness = appearance_page();
    for (label, shown, still) in
        [("Full for Motion", Level::Full, false), ("Off for Motion", Level::Off, true), ("Reduced for Motion", Level::Reduced, true)]
    {
        harness.get_by_label(label).click();
        harness.run();
        assert_eq!(level(&harness.ctx), shown, "{label}");
        assert_eq!(reduced(&harness.ctx), still, "{label}");
        // The page rests at every level: nothing asks for frames once the press is over.
        assert!(crate::component::tests::rests(&mut harness, 30), "{label}: the page rests");
    }
}

#[test]
fn the_computer_s_wish_for_less_motion_shows_off_until_the_owner_chooses() {
    let mut harness = harness();
    crate::motion::set_system_wish(&harness.ctx, Some(true));
    harness.state_mut().settings_mut().open_at(Section::Appearance);
    harness.run();
    assert!(!harness.state().appearance().motion_chosen());
    assert_eq!(crate::motion::level(&harness.ctx), crate::motion::Level::Off, "the window moves at Off");
    assert!(chosen(&harness, "Off for Motion"), "and the page shows Off");
    harness.get_by_label_contains(MOTION_HELP[2].1);
    // Chosen, the owner's level holds whatever the computer wishes.
    harness.get_by_label("Reduced for Motion").click();
    harness.run();
    assert!(chosen(&harness, "Reduced for Motion"));
    assert_eq!(crate::motion::level(&harness.ctx), crate::motion::Level::Reduced);
    // The computer that does not ask, or cannot be read, leaves the owner's default.
    let mut fresh = self::harness();
    crate::motion::set_system_wish(&fresh.ctx, Some(false));
    fresh.run();
    assert_eq!(crate::motion::level(&fresh.ctx), crate::motion::Level::Reduced);
}

#[test]
fn a_press_on_the_level_shown_records_it_as_the_owner_s_choice_once() {
    use crate::motion::{Level, level, set_system_wish};
    // The computer asks for less motion: the page shows Off, not chosen; a press on Off chooses it, so the window
    // stays at Off when the wish clears.
    let mut harness = appearance_page();
    set_system_wish(&harness.ctx, Some(true));
    harness.run();
    assert!(chosen(&harness, "Off for Motion") && !harness.state().appearance().motion_chosen());
    harness.get_by_label("Off for Motion").click();
    harness.run();
    assert_eq!(harness.state().mailbox().last_run(), Some("ui.motion.set"));
    assert!(harness.state().appearance().motion_chosen(), "the level shown is now the owner's");
    set_system_wish(&harness.ctx, Some(false));
    harness.run();
    assert_eq!(level(&harness.ctx), Level::Off, "the owner's Off holds without the wish");
    // No wish: the default Reduced, pressed, is chosen; a later wish no longer gives Off.
    let mut fresh = appearance_page();
    fresh.get_by_label("Reduced for Motion").click();
    fresh.run();
    assert!(fresh.state().appearance().motion_chosen());
    set_system_wish(&fresh.ctx, Some(true));
    fresh.run();
    assert_eq!(level(&fresh.ctx), Level::Reduced);
    // Chosen already, a press on the level shown asks for nothing.
    fresh.get_by_role_and_label(Role::RadioButton, "Light").click();
    fresh.run();
    let before = fresh.state().mailbox().last_run();
    assert_ne!(before, Some("ui.motion.set"));
    fresh.get_by_label("Reduced for Motion").click();
    fresh.run();
    assert_eq!(fresh.state().mailbox().last_run(), before, "a press on the chosen level runs nothing");
}

#[test]
fn text_130_and_the_system_frame_stand_as_their_frames() {
    // `settings-appearance/text-130`: the handle on the last tick, the rows still in order.
    let mut harness = appearance_page();
    harness.state_mut().set_text_scale(crate::theme::TextScale::from_percent(130).unwrap());
    harness.run();
    assert_eq!(harness.get_by_role_and_label(Role::Slider, "Text size").accesskit_node().numeric_value(), Some(130.0));
    assert!(chosen(&harness, "Reduced for Motion"));
    let tops = ["Mode", "Motion", "Text size", "Inbox view", "Window frame"].map(|title| row_top(&harness, title));
    assert!(tops.iter().all(|y| y.is_finite()) && tops.windows(2).all(|pair| pair[0] < pair[1]), "{tops:?}");
    // `settings-appearance/frame-system`: System chosen runs `ui.frame.set`.
    // Below the fold at 130 %: pressed the way a screen reader presses it.
    harness.get_by_label("System").click_accesskit();
    harness.run();
    assert_eq!(harness.state().mailbox().last_run(), Some("ui.frame.set"));
    assert_eq!(harness.state().appearance().frame, inboxcraft_engine::appearance::WindowFrame::System);
    assert!(harness.query_all_by_label("System").any(|node| node.accesskit_node().toggled() == Some(egui::accesskit::Toggled::True)));
    // `settings-appearance/layout-window`: both window choices come later; pane stays.
    for later in ["Window for Open a message", "Window for Write a message"] {
        harness.get_by_label(later).click();
        harness.run();
    }
    assert!(chosen(&harness, "Pane for Open a message") && chosen(&harness, "Pane for Write a message"));
}

#[test]
fn the_keyboard_walks_the_appearance_page_in_the_drawn_order() {
    let mut harness = appearance_page();
    harness.get_by_label("Auto / Dark / Light").focus();
    harness.run();
    let mut walk: Vec<(String, egui::Rect)> = Vec::new();
    for _ in 0..30 {
        let Some(stop) = focused(&harness) else { break };
        if walk.iter().any(|(label, _)| *label == stop.0) {
            break;
        }
        walk.push(stop);
        harness.key_press(Key::Tab);
        harness.run();
    }
    let labels: Vec<&str> = walk.iter().map(|(label, _)| label.as_str()).collect();
    let at = |part: &str| labels.iter().position(|label| label.contains(part)).unwrap_or_else(|| panic!("{part} not reached: {labels:?}"));
    // Look, then Layout, as drawn; the coming-soon rows take no stop.
    let order = [at("Auto / Dark / Light"), at("Motion"), at("Text size"), at("Grouped / Smart / List"), at("Own / System")];
    assert!(order.windows(2).all(|pair| pair[0] < pair[1]), "{labels:?}");
    assert!(labels.iter().all(|label| !label.contains("Open a message") && !label.contains("Write a message")), "{labels:?}");
    // Shift Tab walks it back.
    let last = walk[order[4]].0.clone();
    harness.get_by_label(&last).focus();
    harness.run();
    let mut back = Vec::new();
    for _ in 0..order[4] {
        harness.key_press_modifiers(egui::Modifiers::SHIFT, Key::Tab);
        harness.run();
        back.push(focused(&harness).map(|(label, _)| label).unwrap_or_default());
    }
    let mut forth: Vec<String> = labels[..order[4]].iter().map(|label| (*label).to_owned()).collect();
    forth.reverse();
    assert_eq!(back, forth);
}

/// Every text painted on the sheet in the last frame, as it shows, with where it stands.
fn painted_texts(harness: &Harness<'static, InboxApp>) -> Vec<(String, egui::Rect)> {
    fn open(shape: &egui::Shape, out: &mut Vec<(String, egui::Rect)>) {
        match shape {
            egui::Shape::Vec(inner) => inner.iter().for_each(|shape| open(shape, out)),
            // The sheet's scrim over the whole window covers every text drawn before it.
            egui::Shape::Rect(rect) if rect.rect.min == egui::Pos2::ZERO && rect.rect.width() > 500.0 && (1..255).contains(&rect.fill.a()) => {
                out.clear();
            }
            // The letters shown, not the text laid out: a cut text shows fewer, and an ellipsis.
            egui::Shape::Text(text) if !text.galley.is_empty() => {
                let shown = text.galley.rows.iter().flat_map(|row| row.glyphs.iter().map(|glyph| glyph.chr)).collect();
                out.push((shown, shape.visual_bounding_rect()));
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    harness.output().shapes.iter().for_each(|clipped| open(&clipped.shape, &mut out));
    out
}

#[test]
fn at_every_text_size_the_section_count_stands_whole() {
    for percent in [90, 100, 110, 120, 130] {
        for section in [Section::Appearance, Section::Accounts, Section::Shortcuts] {
            let app = InboxApp::at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock()));
            let mut harness = Harness::builder()
                .with_size(vec2(1200.0, 820.0))
                .with_context_setup(crate::theme::install_fonts)
                .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
            harness.state_mut().set_text_scale(crate::theme::TextScale::from_percent(percent).unwrap());
            harness.state_mut().settings_mut().open_at(section);
            harness.run();
            let texts = painted_texts(&harness);
            assert!(texts.iter().any(|(text, _)| text == "11 sections"), "{percent} %, {section:?}: the count is cut");
        }
    }
}

#[test]
fn shift_tab_round_a_page_with_many_accounts_reaches_the_last_account() {
    let mut app = InboxApp::at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    for n in 0..40 {
        let account = serde_json::json!({ "address": format!("many{n}@example.org"), "imap": { "host": "imap.example.org", "security": "tls" } });
        app.mailbox_mut().run("account.add", &account).unwrap();
    }
    app.mailbox_mut().refresh();
    let last = format!("Signature of {}", app.mailbox().accounts().last().unwrap().label);
    let mut harness = Harness::builder().with_size(vec2(1440.0, 900.0)).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.state_mut().settings_mut().open_at(Section::Sending);
    harness.run();
    harness.get_by_label("Command line").focus();
    harness.run();
    // Back from the window's first control comes round to its last, the page's: the Scheduled link, the Templates link
    // under the signatures (#317), then the last account's signature, scrolled to.
    harness.key_press_modifiers(egui::Modifiers::SHIFT, Key::Tab);
    harness.run();
    assert_eq!(focused(&harness).map(|(label, _)| label).as_deref(), Some("open the list →"), "Shift+Tab from the first control");
    harness.key_press_modifiers(egui::Modifiers::SHIFT, Key::Tab);
    harness.run();
    assert_eq!(focused(&harness).map(|(label, _)| label).as_deref(), Some("manage templates →"), "Shift+Tab from the Scheduled link");
    harness.key_press_modifiers(egui::Modifiers::SHIFT, Key::Tab);
    harness.run();
    let sheet = harness.get_by_role_and_label(Role::Pane, "Settings").rect();
    let (label, rect) = focused(&harness).unwrap_or((String::new(), egui::Rect::NOTHING));
    assert_eq!(label, last, "Shift+Tab from the Templates link");
    assert!(sheet.contains_rect(rect), "{label} {rect:?} in sight on the sheet {sheet:?}");
    // And on back, the account before it.
    harness.key_press_modifiers(egui::Modifiers::SHIFT, Key::Tab);
    harness.run();
    let before = format!("Signature of {}", harness.state().mailbox().accounts().iter().rev().nth(1).unwrap().label);
    assert_eq!(focused(&harness).map(|(label, _)| label), Some(before));
}

/// The sample with 40 more accounts, on the Sending page at `percent` in a window `size`.
fn many_on_sending(size: egui::Vec2, percent: u16) -> Harness<'static, InboxApp> {
    let mut app = InboxApp::at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    for n in 0..40 {
        let account = serde_json::json!({ "address": format!("many{n}@example.org"), "imap": { "host": "imap.example.org", "security": "tls" } });
        app.mailbox_mut().run("account.add", &account).unwrap();
    }
    app.mailbox_mut().refresh();
    app.set_text_scale(crate::theme::TextScale::from_percent(percent).unwrap());
    let mut harness = Harness::builder().with_size(size).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.state_mut().settings_mut().open_at(Section::Sending);
    harness.run();
    harness
}

/// Where the last account's signature stands: below every other row, laid out or not.
fn last_signature_top(harness: &Harness<'static, InboxApp>) -> f32 {
    let last = format!("Signature of {}", harness.state().mailbox().accounts().last().unwrap().label);
    harness.get_by_role_and_label(Role::MultilineTextInput, &last).rect().top()
}

#[test]
fn rows_out_of_sight_take_their_new_height_after_a_change_of_text_size_or_width() {
    let wide = vec2(1200.0, 900.0);
    let narrow = vec2(700.0, 900.0);
    for (size, percent) in [(wide, 130), (narrow, 100)] {
        let mut changed = many_on_sending(wide, 100);
        changed.state_mut().set_text_scale(crate::theme::TextScale::from_percent(percent).unwrap());
        changed.set_size(size);
        changed.run();
        changed.run();
        let fresh = many_on_sending(size, percent);
        assert_eq!(last_signature_top(&changed), last_signature_top(&fresh), "{size:?} at {percent} %: the rows above keep stale heights");
    }
}

#[test]
fn the_privacy_page_says_its_words_as_the_design_has_them() {
    let mut harness = opened();
    harness.get_by_label("Privacy, settings page").click();
    harness.run();
    harness.get_by_label(
        "The senders whose remote pictures load without asking. “ask again” takes one off the list: its pictures are held again, in every message.",
    );
}

#[test]
fn the_motion_row_says_what_the_computer_gives_until_the_owner_chooses() {
    let harness = opened();
    // `settings-appearance/default`: the help in two sentences, the row 84 high.
    let help = "Reduced keeps what explains a change and drops the text effects. Until you choose, the computer's reduce-motion setting gives Off.";
    harness.get_by_label_contains(help);
}

#[test]
fn nothing_changed_stands_12_right_of_a_signature_s_save() {
    let mut harness = harness();
    harness.state_mut().settings_mut().open_at(Section::Sending);
    harness.run();
    // `settings-sending/default`: Save 56 wide, disabled, then "nothing changed" 12 to its right.
    let save = harness
        .get_all_by_role(Role::Button)
        .find(|node| node.accesskit_node().label().unwrap_or_default().starts_with("Save the signature of"))
        .unwrap()
        .rect();
    let words = harness.get_all_by_label("nothing changed").find(|node| (node.rect().center().y - save.center().y).abs() < 8.0).unwrap().rect();
    assert_eq!(words.left() - save.right(), 12.0, "{save:?} {words:?}");
}

#[test]
fn the_calendar_s_week_row_shows_the_rule_s_first_day() {
    let mut harness = harness();
    harness.state_mut().settings_mut().open_at(Section::Calendar);
    harness.run();
    assert!(harness.query_all_by_label_contains("Fixed").next().is_none(), "the week is no longer fixed");
    let shown = |harness: &Harness<'static, InboxApp>| harness.get_by_role_and_label(Role::ComboBox, "Week starts on").accesskit_node().value();
    assert_eq!(shown(&harness).as_deref(), Some("Monday"));
    harness.state_mut().mailbox_mut().run("week.set", &serde_json::json!({ "first_day": "sunday" })).unwrap();
    harness.run();
    assert_eq!(shown(&harness).as_deref(), Some("Sunday"));
}

#[test]
fn a_signature_s_field_stands_on_the_page_s_inset_side_by_side_and_stacked() {
    // At 1440 the signatures stand two by two; at 800 and 130 % the column is under 240 and they stack. Either way the
    // first field stands on the inset the group's note stands on (16 in), the same in both.
    let mut offsets = Vec::new();
    for (width, percent) in [(1440.0, 100), (800.0, 130)] {
        let app = InboxApp::at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock()));
        let mut harness = Harness::builder()
            .with_size(vec2(width, 900.0))
            .with_context_setup(crate::theme::install_fonts)
            .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
        harness.state_mut().set_text_scale(crate::theme::TextScale::from_percent(percent).unwrap());
        harness.state_mut().settings_mut().open_at(Section::Sending);
        harness.run();
        let note = text_at(&harness, "A signature goes under").expect("the group's note");
        let first = format!("Signature of {}", harness.state().mailbox().accounts().first().unwrap().label);
        let field = harness.get_by_role_and_label(Role::MultilineTextInput, &first).rect();
        offsets.push(field.left() - note.x);
    }
    assert!(offsets.windows(2).all(|pair| (pair[0] - pair[1]).abs() < 0.5), "stacked as side by side: {offsets:?}");
}

/// Where the text painted in the last frame that starts with `wanted` is laid out (its galley's corner).
fn text_at(harness: &Harness<'static, InboxApp>, wanted: &str) -> Option<egui::Pos2> {
    fn find(shape: &egui::Shape, wanted: &str) -> Option<egui::Pos2> {
        match shape {
            egui::Shape::Vec(inner) => inner.iter().find_map(|shape| find(shape, wanted)),
            egui::Shape::Text(text) if text.galley.text().starts_with(wanted) => Some(text.pos),
            _ => None,
        }
    }
    harness.output().shapes.iter().find_map(|clipped| find(&clipped.shape, wanted))
}

#[test]
fn at_the_frame_s_1440_every_appearance_help_stands_on_one_line_in_the_720_column() {
    // The frame's rows: Mode, Open a message, Write a message and Window frame 66 high, a help of one line each. At 1200 the
    // column is narrower than 720 and the Window frame's help wraps (a gaps line), as the frames draw no 1200.
    let app = InboxApp::at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    let mut harness = Harness::builder()
        .with_size(vec2(1440.0, 900.0))
        .with_context_setup(crate::theme::install_fonts)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.state_mut().settings_mut().open_at(Section::Appearance);
    harness.run();
    let texts = painted_texts(&harness);
    for help in ["Auto follows", "In the pane beside", "Docked in the reader", "InboxCraft's own frame"] {
        let (_, rect) = texts.iter().find(|(text, _)| text.starts_with(help)).unwrap_or_else(|| panic!("{help} is drawn"));
        assert!(rect.height() < 22.0, "{help} on one line: {rect:?}");
    }
}
