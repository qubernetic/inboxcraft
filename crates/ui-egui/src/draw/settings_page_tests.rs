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
fn the_sections_column_stands_as_the_design_draws_it() {
    let harness = opened();
    let sheet = harness.get_by_role_and_label(Role::Dialog, "Settings").rect();
    // Inside the sheet's 1 px edge: the head 48, its hairline, 8, then the rows 30 high one under the other.
    let mut top = sheet.top() + 1.0 + 48.0 + 1.0 + 8.0;
    for name in NAMES {
        let row = harness.get_by_label(&format!("{name}, settings page")).rect();
        assert_eq!((row.top(), row.height()), (top, 30.0), "{name}");
        assert_eq!((row.left(), row.width()), (sheet.left() + 1.0, 232.0), "{name}");
        top = row.bottom();
    }
    // Close, which the design does not have, stands at the right end of the column's head, out of every page's way.
    let close = harness.get_by_role_and_label(Role::Button, "Close").rect();
    let head = egui::Rect::from_min_size(sheet.min + vec2(1.0, 1.0), vec2(232.0, 48.0));
    assert!(head.contains_rect(close), "Close {close:?} in the head {head:?}");
    assert!((close.right() - (head.right() - 16.0)).abs() <= 0.5, "16 from the column's edge: {close:?}");
    assert!((close.center().y - head.center().y).abs() <= 0.5, "in the head's middle: {close:?}");
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
    let rail = "Settings";
    harness.get_by_role_and_label(Role::Button, rail).focus();
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
                // Every text painted on the sheet lies inside the row or button it belongs to, inside its pane, and
                // over no other text.
                let found = super::super::fit_tests::misfits(&harness);
                assert!(found.is_empty(), "{section:?} at {size:?}, {scale:?}:\n{}", found.join("\n"));
            }
            app = harness.into_state();
        }
    }
}

#[test]
fn the_accounts_and_shortcuts_pages_take_the_design_s_values() {
    let app = InboxApp::new(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    let mut harness = Harness::builder()
        .with_size(vec2(1200.0, 900.0))
        .with_context_setup(crate::component::tests::record_layers)
        .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    // Shortcuts: the field 320 wide with its own label, Reset all keys 2 over its foot at the column's right end, the
    // list's first hairline 12 under them and the first chip 8 into its 36 row.
    harness.state_mut().settings_mut().open_at(Section::Shortcuts);
    harness.run();
    let sheet = harness.get_by_role_and_label(Role::Dialog, "Settings").rect();
    let find = harness.get_by_role_and_label(Role::TextInput, "Find a command").rect();
    assert_eq!(find.width(), 320.0, "{find:?}");
    let title = harness.get_by_role_and_label(Role::Heading, "Shortcuts").rect();
    // The head's foot (its intro, a line of 22 under 6), 16, the label 16 and 8, then the box.
    assert_eq!(find.top(), title.top() - 1.0 + 32.0 + 6.0 + 22.0 + 16.0 + 16.0 + 8.0, "{find:?} under {title:?}");
    let reset = harness.get_by_role_and_label(Role::Button, "Reset all keys").rect();
    assert_eq!((reset.bottom(), reset.right()), (find.bottom() - 2.0, sheet.right() - 1.0 - 40.0 - 16.0), "{reset:?}");
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
    let sheet = harness.get_by_role_and_label(Role::Dialog, "Settings").rect();
    let links: Vec<egui::Rect> = harness.query_all_by_label("read again").map(|node| node.rect()).collect();
    assert_eq!(links.len(), harness.state().mailbox().accounts().len(), "one row per account");
    for link in &links {
        assert!((link.right() - (sheet.right() - 1.0 - 40.0 - 16.0)).abs() <= 1.0, "{link:?}");
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
        let app = InboxApp::new(Mailbox::new(sample::session(clock()).unwrap(), clock()));
        let mut harness = Harness::builder().with_size(size).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
        for section in Section::ALL {
            harness.state_mut().settings_mut().open_at(section);
            harness.run();
            let sheet = harness.get_by_role_and_label(Role::Dialog, "Settings").rect();
            assert!(sheet.width() >= 0.0 && sheet.height() >= 0.0, "{size:?} {section:?}: {sheet:?}");
        }
    }
}

#[test]
fn a_page_with_many_accounts_lays_out_the_rows_in_sight_and_tab_still_reaches_every_one() {
    let mut app = InboxApp::new(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    for n in 0..40 {
        let account = serde_json::json!({
            "address": format!("many{n}@example.org"),
            "imap": { "host": "imap.example.org", "security": "tls" },
        });
        app.mailbox_mut().run("account.add", &account).unwrap();
    }
    app.mailbox_mut().refresh();
    let accounts = app.mailbox().accounts().len();
    let mut harness = Harness::builder().with_size(vec2(1200.0, 900.0)).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
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
    let app = InboxApp::new(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    // Too low for the whole Appearance page: its last row, the window frame, starts out of sight.
    let mut harness = Harness::builder().with_size(vec2(1100.0, 600.0)).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.state_mut().settings_mut().open_at(Section::Appearance);
    harness.run();
    let sheet = harness.get_by_role_and_label(Role::Dialog, "Settings").rect();
    let mut seen = Vec::new();
    for _ in 0..60 {
        harness.key_press(Key::Tab);
        harness.run();
        if let Some((label, rect)) = focused(&harness) {
            assert!(sheet.contains_rect(rect), "{label} {rect:?} has the keyboard out of sight of the sheet {sheet:?}");
            seen.push(label);
        }
    }
    assert!(seen.iter().any(|label| label == "Own / System"), "Tab reaches the window frame: {seen:?}");
}

#[test]
fn motion_s_full_carries_the_coming_soon_mark() {
    let harness = opened();
    let full = harness.get_by_label("Full for Motion");
    assert!(full.accesskit_node().is_disabled(), "Full cannot be chosen yet");
    let full = full.rect();
    let marks: Vec<egui::Rect> = harness.query_all_by_label("coming soon").map(|tag| tag.rect()).collect();
    let mark = marks.iter().find(|mark| (mark.center().y - full.center().y).abs() <= 1.0);
    let mark = mark.unwrap_or_else(|| panic!("no coming-soon mark beside Full {full:?}: {marks:?}"));
    assert!(mark.right() <= full.left() && full.left() - mark.right() <= 12.0, "the mark {mark:?} stands just before Full {full:?}");
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
fn at_every_text_size_the_section_count_stands_whole_and_close_gives_way() {
    for percent in [90, 100, 110, 120, 130] {
        for section in [Section::Appearance, Section::Accounts, Section::Shortcuts] {
            let app = InboxApp::new(Mailbox::new(sample::session(clock()).unwrap(), clock()));
            let mut harness = Harness::builder()
                .with_size(vec2(1200.0, 820.0))
                .with_context_setup(crate::theme::install_fonts)
                .build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
            harness.state_mut().set_text_scale(crate::theme::TextScale::from_percent(percent).unwrap());
            harness.state_mut().settings_mut().open_at(section);
            harness.run();
            let texts = painted_texts(&harness);
            assert!(texts.iter().any(|(text, _)| text == "11 sections"), "{percent} %, {section:?}: the count is cut");
            let sheet = harness.get_by_role_and_label(Role::Dialog, "Settings").rect();
            let close = harness.get_by_role_and_label(Role::Button, "Close").rect();
            assert!(sheet.contains_rect(close), "{percent} %, {section:?}: Close {close:?} on the sheet {sheet:?}");
            let under: Vec<&String> = texts
                .iter()
                .filter(|(_, rect)| rect.intersects(close) && sheet.contains_rect(*rect) && !close.contains(rect.center()))
                .map(|(text, _)| text)
                .collect();
            assert!(under.is_empty(), "{percent} %, {section:?}: Close {close:?} lies over {under:?}");
            // The controls on the sheet: those after it in the tree (the window's own lie under the dim).
            let nodes: Vec<_> = harness.root().children_recursive().collect();
            let first = nodes.iter().position(|node| node.accesskit_node().role() == Role::Dialog).unwrap();
            for other in nodes.iter().skip(first + 1).filter(|node| node.accesskit_node().label().is_none_or(|label| label != "Close")) {
                let data = other.accesskit_node();
                let pressable = data.data().supports_action(egui::accesskit::Action::Click) && data.role() != Role::Label;
                assert!(!pressable || !other.rect().intersects(close), "{percent} %, {section:?}: Close lies over {:?}", data.label());
            }
        }
    }
}

#[test]
fn shift_tab_round_a_page_with_many_accounts_reaches_the_last_account() {
    let mut app = InboxApp::new(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    for n in 0..40 {
        let account = serde_json::json!({ "address": format!("many{n}@example.org"), "imap": { "host": "imap.example.org", "security": "tls" } });
        app.mailbox_mut().run("account.add", &account).unwrap();
    }
    app.mailbox_mut().refresh();
    let last = format!("Signature of {}", app.mailbox().accounts().last().unwrap().label);
    let mut harness = Harness::builder().with_size(vec2(1200.0, 900.0)).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.state_mut().settings_mut().open_at(Section::Sending);
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Close").focus();
    harness.run();
    // Back from the sheet's first control comes round to its last: the last account's signature, scrolled to.
    harness.key_press_modifiers(egui::Modifiers::SHIFT, Key::Tab);
    harness.run();
    let sheet = harness.get_by_role_and_label(Role::Dialog, "Settings").rect();
    let (label, rect) = focused(&harness).unwrap_or((String::new(), egui::Rect::NOTHING));
    assert_eq!(label, last, "Shift+Tab from the first control");
    assert!(sheet.contains_rect(rect), "{label} {rect:?} in sight on the sheet {sheet:?}");
    // And on back, the account before it.
    harness.key_press_modifiers(egui::Modifiers::SHIFT, Key::Tab);
    harness.run();
    let before = format!("Signature of {}", harness.state().mailbox().accounts().iter().rev().nth(1).unwrap().label);
    assert_eq!(focused(&harness).map(|(label, _)| label), Some(before));
}

/// The sample with 40 more accounts, on the Sending page at `percent` in a window `size`.
fn many_on_sending(size: egui::Vec2, percent: u16) -> Harness<'static, InboxApp> {
    let mut app = InboxApp::new(Mailbox::new(sample::session(clock()).unwrap(), clock()));
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
