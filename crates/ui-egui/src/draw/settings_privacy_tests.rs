// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The Privacy page: the senders whose remote pictures always load, the empty state, "ask again" on the owner's press
//! only, the keyboard walk and where the keyboard goes after, the page at rest, and hostile addresses.

use egui::accesskit::Role;
use egui::{Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};
use inboxcraft_time::{FakeClock, Timestamp};
use serde_json::{Value, json};

use crate::mailbox::Mailbox;
use crate::privacy::{Privacy, SenderRow};
use crate::settings::Section;
use crate::{InboxApp, sample};

fn clock() -> Box<FakeClock> {
    Box::new(FakeClock::new(Timestamp::from_unix(1_793_869_200, 0).unwrap()))
}

const THREE: [&str; 3] = ["finds@quietweb.example.org", "hello@typeweekly.example.org", "orders@kettle.example.org"];

/// The sample window with `allowed` senders' pictures loading always, on the Privacy page.
fn on_privacy(allowed: &[&str]) -> Harness<'static, InboxApp> {
    on_privacy_at(allowed, vec2(1440.0, 900.0))
}

/// [`on_privacy`] in a window `size` big.
fn on_privacy_at(allowed: &[&str], size: egui::Vec2) -> Harness<'static, InboxApp> {
    let mut app = InboxApp::at_mail(Mailbox::new(sample::session(clock()).unwrap(), clock()));
    app.set_reduce_motion(true);
    for address in allowed {
        app.mailbox_mut().run("remote.allow", &json!({ "address": address })).unwrap();
    }
    app.settings_mut().open_at(Section::Privacy);
    let mut harness =
        Harness::builder().with_size(size).with_max_steps(40).with_theme(egui::Theme::Dark).build_ui_state(|ui, app: &mut InboxApp| app.ui(ui), app);
    harness.run();
    harness
}

fn again(address: &str) -> String {
    format!("Ask again before the pictures of {address} load")
}

/// The senders the engine still lets load, as `remote.list` answers them.
fn kept(harness: &mut Harness<'static, InboxApp>) -> Vec<String> {
    let answer = harness.state_mut().mailbox_mut().run("remote.list", &json!({})).unwrap();
    answer["senders"].as_array().unwrap().iter().map(|sender| sender.as_str().unwrap().to_owned()).collect()
}

fn focused(harness: &Harness<'static, InboxApp>) -> Option<String> {
    harness.root().children_recursive().find(|node| node.accesskit_node().is_focused()).map(|node| node.accesskit_node().label().unwrap_or_default())
}

fn key(harness: &mut Harness<'static, InboxApp>, key: Key, modifiers: Modifiers) {
    harness.key_press_modifiers(modifiers, key);
    harness.run();
}

#[test]
fn the_page_lists_the_senders_whose_pictures_always_load_a_row_each() {
    let harness = on_privacy(&THREE);
    harness.get_by_role_and_label(Role::Heading, "Privacy");
    harness.get_by_label_contains("Pictures always load · 3");
    let tops: Vec<f32> = THREE.iter().map(|address| harness.get_by_label(&again(address)).rect().center().y).collect();
    for pair in tops.windows(2) {
        assert!((pair[1] - pair[0] - 45.0).abs() <= 1.0, "a row is 44 high with its hairline: {tops:?}");
    }
}

#[test]
fn with_nobody_the_page_says_so() {
    let harness = on_privacy(&[]);
    harness.get_by_label("No sender loads pictures without asking.");
    assert!(harness.query_by_label_contains("ask again before").is_none());
}

#[test]
fn ask_again_runs_only_on_the_owner_s_press_and_the_status_line_says_so() {
    let mut harness = on_privacy(&THREE);
    let kettle = again("orders@kettle.example.org");
    // A hover and the keyboard on it run nothing.
    harness.get_by_label(&kettle).hover();
    harness.run();
    harness.get_by_label(&kettle).focus();
    harness.run();
    assert_eq!(kept(&mut harness).len(), 3, "a focus or a hover asks nothing");
    harness.get_by_label(&kettle).click();
    harness.run();
    assert_eq!(kept(&mut harness), THREE[..2]);
    assert!(harness.query_by_label(&kettle).is_none(), "the row went");
    harness.get_by_label_contains("Pictures always load · 2");
    assert_eq!(harness.state().notice(), Some("orders@kettle.example.org · pictures ask again"));
    harness.run();
    assert_eq!(focused(&harness), Some(again(THREE[1])), "the keyboard goes to the row before, not to nothing");
}

#[test]
fn asking_again_for_the_last_sender_hands_the_keyboard_to_the_page_s_section() {
    let mut harness = on_privacy(&THREE[..1]);
    harness.get_by_label(&again(THREE[0])).click();
    harness.run();
    harness.run();
    assert!(kept(&mut harness).is_empty());
    harness.get_by_label("No sender loads pictures without asking.");
    assert_eq!(focused(&harness).as_deref(), Some("Privacy, settings page"));
}

#[test]
fn the_list_is_read_again_each_time_the_page_shows() {
    let mut harness = on_privacy(&THREE[..1]);
    harness.state_mut().settings_mut().show(Section::Sending);
    harness.run();
    harness.state_mut().mailbox_mut().run("remote.allow", &json!({ "address": THREE[1] })).unwrap();
    harness.state_mut().settings_mut().show(Section::Privacy);
    harness.run();
    harness.get_by_label(&again(THREE[1]));
}

#[test]
fn the_keyboard_walks_the_rows_in_their_drawn_order() {
    let mut harness = on_privacy(&THREE);
    // The settings' own column comes first; its last row, Plugins, hands on to the page.
    harness.get_by_label("Plugins, settings page").focus();
    harness.run();
    let mut walked = Vec::new();
    for _ in 0..3 {
        key(&mut harness, Key::Tab, Modifiers::NONE);
        walked.push(focused(&harness).unwrap_or_default());
    }
    assert_eq!(walked, THREE.map(again));
    for _ in 0..2 {
        key(&mut harness, Key::Tab, Modifiers::SHIFT);
    }
    assert_eq!(focused(&harness), Some(again(THREE[0])));
}

#[test]
fn shift_tab_round_a_long_list_reaches_the_last_sender_and_keeps_it() {
    let many: Vec<String> = (0..60).map(|n| format!("s{n:02}@example.org")).collect();
    let many: Vec<&str> = many.iter().map(String::as_str).collect();
    let mut harness = on_privacy(&many);
    harness.get_by_label("Command line").focus();
    harness.run();
    key(&mut harness, Key::Tab, Modifiers::SHIFT);
    assert_eq!(focused(&harness), Some(again("s59@example.org")), "the last row, out of sight, is reached and kept");
    key(&mut harness, Key::Tab, Modifiers::SHIFT);
    assert_eq!(focused(&harness), Some(again("s58@example.org")));
}

#[test]
fn the_page_rests() {
    let mut harness = on_privacy(&THREE);
    assert!(crate::component::tests::rests(&mut harness, 4), "the list rests");
    let mut empty = on_privacy(&[]);
    assert!(crate::component::tests::rests(&mut empty, 4), "the empty page rests");
}

#[test]
fn a_hostile_address_is_one_plain_capped_row_and_junk_is_none() {
    let row = SenderRow::read(&json!(format!("a\r\nb\u{0}\u{202E}{}@example.org", "y".repeat(10_000)))).unwrap();
    assert!(row.shown.chars().count() <= 254 && !row.shown.chars().any(|c| c.is_control() || c == '\u{202E}'));
    assert_eq!(SenderRow::read(&json!(" \r\n ")), None);
    let mut privacy = Privacy::default();
    for junk in [Value::Null, json!({ "senders": 3 }), json!({ "senders": [1, null, { "a": 1 }] })] {
        privacy.read(&junk);
        assert_eq!(privacy.rows(), Some(&[][..]), "{junk}");
    }
}

#[test]
fn a_held_key_on_ask_again_forgets_one_sender_only() {
    for key in [Key::Enter, Key::Space] {
        let mut harness = on_privacy(&THREE);
        harness.get_by_label(&again(THREE[0])).focus();
        harness.run();
        // The press forgets the first sender; its repeats land on the row that took its place and forget nothing.
        crate::component::tests::hold_key(&mut harness, key, 6);
        assert_eq!(kept(&mut harness), THREE[1..], "a held {key:?} on ask again forgets one sender");
    }
}

#[test]
fn a_stale_note_clears_when_the_list_is_read_again() {
    let mut harness = on_privacy(&THREE);
    harness.state_mut().settings_mut().privacy_mut().set_note(Some("The list could not be read.".to_owned()));
    harness.run();
    assert!(harness.query_by_label("The list could not be read.").is_some());
    harness.get_by_label(&again(THREE[0])).click();
    harness.run();
    assert_eq!(kept(&mut harness), THREE[1..]);
    assert!(harness.query_by_label("The list could not be read.").is_none(), "the old note goes when the list reads well");
}

/// The text painted in the last frame that starts with `wanted`: where its galley stands and its colour.
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

/// What the galley laid out from `wanted` shows (an elided run ends in `…`), and where it stands.
fn shown(harness: &Harness<'static, InboxApp>, wanted: &str) -> Option<(egui::Rect, String)> {
    fn find(shape: &egui::Shape, wanted: &str) -> Option<(egui::Rect, String)> {
        match shape {
            egui::Shape::Vec(inner) => inner.iter().find_map(|shape| find(shape, wanted)),
            egui::Shape::Text(text) if text.galley.text().starts_with(wanted) => {
                let glyphs: String = text.galley.rows.iter().flat_map(|row| row.glyphs.iter().map(|glyph| glyph.chr)).collect();
                Some((egui::Rect::from_min_size(text.pos, text.galley.size()), glyphs))
            }
            _ => None,
        }
    }
    harness.output().shapes.iter().find_map(|clipped| find(&clipped.shape, wanted))
}

/// The left of the rows (the group's note stands 16 in from it).
fn rows_left(harness: &Harness<'static, InboxApp>) -> f32 {
    painted(harness, "The senders whose remote pictures").expect("the group's note").0.left() - 16.0
}

/// The sample on the Privacy page, its list shown as `rows`.
fn showing(rows: Vec<SenderRow>, size: egui::Vec2) -> Harness<'static, InboxApp> {
    let mut harness = on_privacy_at(&THREE, size);
    harness.state_mut().settings_mut().privacy_mut().show(rows);
    harness.run();
    harness
}

fn row(address: &str) -> SenderRow {
    SenderRow::read(&json!(address)).unwrap()
}

#[test]
fn without_a_name_the_address_stands_at_16_in_muted_mono_and_takes_the_name_column() {
    // No name is kept: the address starts at the row's inner left, 16, and has the name's column and gap besides the
    // frame's 424 (616 in a 720 row), "ask again" at 644 to 704 as the frame draws it.
    let harness = on_privacy(&THREE);
    let column = rows_left(&harness);
    let (address, colour) = painted(&harness, "hello@typeweekly.example.org").expect("the address is drawn");
    let theme = crate::theme::Theme::current(&harness.ctx, &crate::theme::Look::of(harness.state().mailbox().appearance()));
    assert!((address.left() - column - 16.0).abs() <= 0.5, "the address at 16: {}", address.left() - column);
    assert_eq!(colour, theme.palette.muted, "the address in muted");
    let link = harness.get_by_label(&again("hello@typeweekly.example.org")).rect();
    assert!((link.right() - column - 704.0).abs() <= 1.0, "ask again ends 16 in: {}", link.right() - column);
    // An address wider than the frame's 424 but within 616 shows whole.
    let long = "newsletters-and-weekly-digest@longer-typeweekly.example.org";
    let harness = showing(vec![row(long)], vec2(1440.0, 900.0));
    let column = rows_left(&harness);
    let (drawn, glyphs) = shown(&harness, long).expect("the long address is drawn");
    assert!(drawn.width() > 424.0, "the test's address is wider than the frame's column: {}", drawn.width());
    assert_eq!(glyphs, long, "the address takes the name's column and shows whole");
    assert!((drawn.left() - column - 16.0).abs() <= 0.5, "the long address at 16: {}", drawn.left() - column);
}

#[test]
fn with_a_name_the_frame_s_columns_stand() {
    // The frame's row: the name at 16 in 180 (strong, ink), the address at 208 in 424, "ask again" at 644 to 704.
    let address = "hello@typeweekly.example.org";
    let harness = showing(vec![row(address).named("Type Weekly")], vec2(1440.0, 900.0));
    let column = rows_left(&harness);
    let theme = crate::theme::Theme::current(&harness.ctx, &crate::theme::Look::of(harness.state().mailbox().appearance()));
    let (name, colour) = painted(&harness, "Type Weekly").expect("the name is drawn");
    assert!((name.left() - column - 16.0).abs() <= 0.5, "the name at 16: {}", name.left() - column);
    assert!(name.width() <= 180.0, "the name in 180: {}", name.width());
    assert_eq!(colour, theme.palette.ink, "the name in ink");
    let (drawn, _) = painted(&harness, address).expect("the address is drawn");
    assert!((drawn.left() - column - 208.0).abs() <= 0.5, "the address at 208: {}", drawn.left() - column);
    let link = harness.get_by_label(&again(address)).rect();
    assert!((link.right() - column - 704.0).abs() <= 1.0, "ask again ends 16 in: {}", link.right() - column);
    // A long name is cut in its column and the address still starts at 208.
    let harness = showing(vec![row(address).named(&"Weekly ".repeat(40))], vec2(1440.0, 900.0));
    let column = rows_left(&harness);
    let (name, glyphs) = shown(&harness, "Weekly").expect("the long name is drawn");
    assert!(name.width() <= 180.0 && glyphs.ends_with('…'), "the long name cut in 180: {} {glyphs}", name.width());
    let (drawn, _) = painted(&harness, address).expect("the address is drawn");
    assert!((drawn.left() - column - 208.0).abs() <= 0.5, "the address at 208 after a long name: {}", drawn.left() - column);
    assert_eq!(SenderRow::read(&json!(address)).unwrap().named(" \r\n ").name, None, "a blank name keeps none");
}

#[test]
fn a_long_address_is_cut_on_one_line_before_ask_again_its_start_kept() {
    // The frame's address is one line that truncates at its end (an ellipsis after the start it keeps); the cut keeps
    // 12 before "ask again", on a wide page and on a narrow one.
    let long = format!("{}@example.org", "a-very-long-local-part".repeat(8));
    for width in [1440.0, 800.0] {
        let harness = showing(vec![row(&long)], vec2(width, 700.0));
        let column = rows_left(&harness);
        let (drawn, glyphs) = shown(&harness, "a-very-long").unwrap_or_else(|| panic!("{width}: the address is drawn"));
        assert!(glyphs.ends_with('…') && long.starts_with(glyphs.trim_end_matches('…')), "{width}: cut at its end: {glyphs}");
        assert!((drawn.left() - column - 16.0).abs() <= 0.5, "{width}: at 16: {}", drawn.left() - column);
        let link = harness.get_by_label(&again(&long)).rect();
        assert!(drawn.right() <= link.left() - 11.5, "{width}: the address {drawn:?} ends 12 before ask again {link:?}");
        if width >= 1440.0 {
            assert!((link.right() - column - 704.0).abs() <= 1.0, "ask again ends 16 in: {}", link.right() - column);
        }
        assert!(drawn.width() >= link.left() - 12.0 - drawn.left() - 16.0, "{width}: the cut uses the room: {drawn:?} {link:?}");
    }
}

#[test]
fn a_hostile_address_and_name_are_one_line_inside_their_row() {
    let hostile = format!("a\r\nb\u{0}\u{202E}c\u{2066}{}{}@example.org", "\u{0301}".repeat(200), "y".repeat(400));
    let rows = vec![row(&hostile), row("hello@typeweekly.example.org").named(&format!("Ty\npe\u{202E}{}", "W".repeat(5_000)))];
    let harness = showing(rows, vec2(1440.0, 900.0));
    let column = rows_left(&harness);
    for (wanted, right) in [("a b", 704.0 - 60.0 - 12.0), ("Ty pe", 16.0 + 180.0)] {
        let (rect, glyphs) = shown(&harness, wanted).unwrap_or_else(|| panic!("{wanted}: drawn"));
        assert!(!glyphs.chars().any(|c| c.is_control() || c == '\u{202E}' || c == '\u{2066}'), "{wanted}: plain: {glyphs:?}");
        assert!(rect.height() <= 22.5, "{wanted}: one line: {rect:?}");
        assert!(rect.left() - column >= 15.5 && rect.right() - column <= right + 0.5, "{wanted}: inside its column: {rect:?}");
    }
}

#[test]
fn on_a_narrow_page_the_address_without_a_name_stays_at_16_whole() {
    // At 1200 and at 800 the address stands at 16 and shows whole, "ask again" still beside it (#325, #337).
    let address = "hello@typeweekly.example.org";
    for width in [1200.0, 800.0] {
        let harness = on_privacy_at(&THREE, vec2(width, 700.0));
        let column = rows_left(&harness);
        let (drawn, glyphs) = shown(&harness, address).unwrap_or_else(|| panic!("{width}: the address is drawn"));
        assert_eq!(glyphs, address, "{width}: the address whole");
        let link = harness.get_by_label(&again(address)).rect();
        assert!(drawn.right() <= link.left() - 11.5, "{width}: the address {drawn:?} ends 12 before ask again {link:?}");
        assert!((drawn.left() - column - 16.0).abs() <= 0.5, "{width}: the address at 16: {}", drawn.left() - column);
    }
}
