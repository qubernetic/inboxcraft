// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The settings pages as the design lays them out: the sections in its order, the pages as section labels over
//! settings rows, coming soon where nothing is built, and a sheet that keeps the keyboard.

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
    let app = InboxApp::new(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    let mut harness =
        Harness::builder().with_size(vec2(1200.0, 900.0)).with_theme(egui::Theme::Dark).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

fn opened() -> Harness<'static, InboxApp> {
    let mut harness = harness();
    harness.get_by_label("Appearance settings").click();
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
        let sheet = harness.get_by_role_and_label(Role::Dialog, "Settings").rect();
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
fn inbox_view_is_set_on_appearance_and_not_on_triage() {
    let mut harness = opened();
    harness.get_by_label("[ B ] Layout");
    let view = harness
        .get_by_role_and_label(Role::Group, "Inbox view. Grouped opens every group, Smart shows the first rows of each, List is one list by time.");
    let size = harness.get_by_role_and_label(Role::Group, "Text size. 90 % to 130 %. Rows and bars follow the type.");
    assert!(view.rect().top() > size.rect().bottom() - 1.0, "Layout follows Look");
    let x = |label: &str| harness.get_by_role_and_label(Role::RadioButton, label).rect().center().x;
    assert!(x("Grouped") < x("Smart") && x("Smart") < x("List"));
    for (label, view) in [("Grouped", InboxView::List), ("List", InboxView::Simple), ("Smart", InboxView::Cards)] {
        harness.get_by_role_and_label(Role::RadioButton, label).click();
        harness.run();
        assert_eq!(harness.state().mailbox().inbox_view(), view, "{label}");
    }
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

#[test]
fn the_sheet_keeps_tab_inside_and_esc_returns_the_focus() {
    let mut harness = harness();
    let rail = "Appearance settings";
    harness.get_by_label(rail).focus();
    harness.run();
    assert_eq!(focused(&harness).map(|(label, _)| label).as_deref(), Some(rail));
    harness.key_press(Key::Enter);
    harness.run();
    assert!(harness.state().settings().is_open());
    let sheet = harness.get_by_role_and_label(Role::Dialog, "Settings").rect();
    let mut stops = 0;
    for _ in 0..60 {
        harness.key_press(Key::Tab);
        harness.run();
        if let Some((label, rect)) = focused(&harness) {
            stops += 1;
            assert!(sheet.contains_rect(rect), "{label} lies outside the sheet");
        }
    }
    assert!(stops > 30, "Tab goes round the sheet's controls");
    harness.key_press(Key::Escape);
    harness.run();
    harness.run();
    assert!(!harness.state().settings().is_open());
    assert_eq!(focused(&harness).map(|(label, _)| label).as_deref(), Some(rail), "the keyboard is back where it was");
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
            let app = InboxApp::new(Mailbox::new(sample::session(clock()).unwrap(), clock()));
            let mut harness =
                Harness::builder().with_size(vec2(1200.0, 2400.0)).with_theme(theme).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
            harness.state_mut().settings_mut().open_at(section);
            harness.run();
            assert!(crate::component::tests::rests(&mut harness, 4), "{name} rests");
            let sheet = harness.get_by_role_and_label(Role::Dialog, "Settings").rect();
            let pressable = pressable_on_sheet(&harness, sheet);
            assert!(pressable.len() > Section::ALL.len(), "{name}: the sections and Close at least");
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
    let mut app = InboxApp::new(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    // The engine cleans a name (controls out, at most its length): what is left is still long, wide and without spaces.
    let label = format!("Allotment\u{202e}\r\n\u{0}\u{7}{}", "Ω".repeat(400));
    let answer = app.mailbox_mut().run("account.update", &serde_json::json!({ "account": 1, "label": label }));
    assert!(answer.is_ok(), "{answer:?}");
    app.mailbox_mut().refresh();
    let kept = app.mailbox().accounts().iter().map(|account| account.label.chars().count()).max().unwrap_or(0);
    assert!(kept >= 60, "a long name: {kept} characters");
    for scale in [crate::theme::TextScale::default(), crate::theme::TextScale::from_percent(130).unwrap()] {
        let mut harness = Harness::builder()
            .with_size(vec2(900.0, 640.0))
            .with_context_setup(crate::theme::install_fonts)
            .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
        harness.state_mut().set_text_scale(scale);
        for section in Section::ALL {
            harness.state_mut().settings_mut().open_at(section);
            harness.run();
            let sheet = harness.get_by_role_and_label(Role::Dialog, "Settings").rect();
            // On the page (the window under the sheet has its own rows of the account).
            let page = harness.get_by_role_and_label(Role::Heading, section.name()).rect();
            for node in harness.query_all_by_label_contains("ΩΩΩΩ").filter(|node| node.rect().left() >= page.left() - 16.0) {
                let rect = node.rect();
                assert!(rect.right() <= sheet.right() + 0.5, "{section:?}: {rect:?} past the sheet {sheet:?}");
            }
        }
        app = harness.into_state();
    }
}
