// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::cell::RefCell;
use std::rc::Rc;

use egui::accesskit::Role;
use egui::{Color32, Id, Key, Rect, Vec2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::{MOST, Scope, Typed, command_field, height, spoken};
use crate::component::Shown;
use crate::component::tests::{Look, Paint, builder, keyboard_walk, painted_in, rests, stage};
use crate::theme::{Mode, Palette, TextScale, Theme};

/// A state of the sheet: its name, the scope, the text, what is forced, and what the sheet draws: the edge, the texts
/// `(x, text, colour)` beside the prompt, the caret's x, the mark `(x, width, text)`, the scope's name and the key.
struct State {
    name: &'static str,
    scope: Scope<'static>,
    text: &'static str,
    forced: Option<Shown>,
    edge: fn(&Palette) -> Color32,
    texts: &'static [Drawn],
    caret: Option<f32>,
    mark: Option<(f32, f32, &'static str)>,
    scope_name: Option<&'static str>,
    key: &'static str,
}

/// A text a state draws: its x, its words and their colour.
type Drawn = (f32, &'static str, fn(&Palette) -> Color32);

const LONG: &str = "from:teodor bakery sign first sketch with the door on the left and the wheat kept in";
const HINT: &str = "search mail or run a command";

fn ink(p: &Palette) -> Color32 {
    p.ink
}
fn muted(p: &Palette) -> Color32 {
    p.muted
}
fn cyan_edge(p: &Palette) -> Color32 {
    p.cyan_edge
}
fn line(p: &Palette) -> Color32 {
    p.line
}

const STATES: [State; 12] = [
    State {
        name: "rest",
        scope: Scope::Open,
        text: "",
        forced: None,
        edge: line,
        texts: &[(29.0, HINT, muted)],
        caret: None,
        mark: None,
        scope_name: None,
        key: "Ctrl K",
    },
    State {
        name: "hover",
        scope: Scope::Open,
        text: "",
        forced: Some(Shown::Hover),
        edge: muted,
        texts: &[(29.0, HINT, muted)],
        caret: None,
        mark: None,
        scope_name: None,
        key: "Ctrl K",
    },
    State {
        name: "focus",
        scope: Scope::Open,
        text: "",
        forced: Some(Shown::Focus),
        edge: cyan_edge,
        texts: &[(44.0, HINT, muted)],
        caret: Some(29.0),
        mark: None,
        scope_name: None,
        key: "Esc",
    },
    State {
        name: "typing",
        scope: Scope::Open,
        text: "bak",
        forced: Some(Shown::Focus),
        edge: cyan_edge,
        texts: &[(29.0, "bak", ink)],
        caret: Some(55.0),
        mark: None,
        scope_name: None,
        key: "Esc",
    },
    State {
        name: "prefix-command",
        scope: Scope::Commands,
        text: "snooze",
        forced: Some(Shown::Focus),
        edge: cyan_edge,
        texts: &[(53.0, "snooze", ink)],
        caret: Some(102.0),
        mark: Some((29.0, 16.0, ">")),
        scope_name: Some("COMMANDS"),
        key: "Esc",
    },
    State {
        name: "prefix-person",
        scope: Scope::People,
        text: "teo",
        forced: Some(Shown::Focus),
        edge: cyan_edge,
        texts: &[(53.0, "teo", ink)],
        caret: Some(79.0),
        mark: Some((29.0, 16.0, "@")),
        scope_name: Some("PEOPLE"),
        key: "Esc",
    },
    State {
        name: "prefix-mailbox",
        scope: Scope::Mailboxes,
        text: "inv",
        forced: Some(Shown::Focus),
        edge: cyan_edge,
        texts: &[(53.0, "inv", ink)],
        caret: Some(79.0),
        mark: Some((29.0, 16.0, "#")),
        scope_name: Some("MAILBOXES"),
        key: "Esc",
    },
    State {
        name: "search-mail",
        scope: Scope::Mailbox("Inbox"),
        text: "",
        forced: Some(Shown::Focus),
        edge: cyan_edge,
        texts: &[(93.0, "search this mailbox", muted)],
        caret: Some(78.0),
        mark: Some((29.0, 41.0, "Inbox")),
        scope_name: Some("MAIL"),
        key: "Esc",
    },
    State {
        name: "search-mail-typing",
        scope: Scope::Mailbox("Inbox"),
        text: "bakery",
        forced: Some(Shown::Focus),
        edge: cyan_edge,
        texts: &[(78.0, "bakery", ink)],
        caret: Some(127.0),
        mark: Some((29.0, 41.0, "Inbox")),
        scope_name: Some("MAIL"),
        key: "Esc",
    },
    State {
        name: "search-mail-set",
        scope: Scope::Mailbox("Inbox"),
        text: "bakery",
        forced: None,
        edge: line,
        texts: &[(78.0, "bakery", ink)],
        caret: None,
        mark: Some((29.0, 41.0, "Inbox")),
        scope_name: Some("MAIL"),
        key: "Esc",
    },
    State {
        name: "search-all-typing",
        scope: Scope::AllMail,
        text: "bakery",
        forced: Some(Shown::Focus),
        edge: cyan_edge,
        texts: &[(29.0, "bakery", ink)],
        caret: Some(78.0),
        mark: None,
        scope_name: Some("ALL MAIL"),
        key: "Esc",
    },
    State {
        name: "long",
        scope: Scope::Open,
        text: LONG,
        forced: Some(Shown::Focus),
        edge: cyan_edge,
        texts: &[],
        caret: Some(502.0),
        mark: None,
        scope_name: None,
        key: "Esc",
    },
];

/// A harness drawing the field 560 wide with `text` in `scope`, `forced`; the last frame's [`Typed`] is kept.
fn board(theme: Theme, scope: Scope<'static>, text: &'static str, forced: Option<Shown>) -> (Harness<'static>, Rc<RefCell<Option<Typed>>>) {
    let kept = Rc::new(RefCell::new(None));
    let seen = Rc::clone(&kept);
    let mut harness = builder().allow_missing_glyphs().with_size(vec2(640.0, 120.0)).build_ui(move |ui| {
        crate::motion::set_reduced(ui.ctx(), true);
        stage(ui, theme.palette.panel, |ui| {
            ui.set_width(560.0);
            let mut value = text.to_owned();
            *seen.borrow_mut() = Some(command_field(ui, &theme, Id::unique("command"), &mut value, scope, "Ctrl K", forced));
        });
    });
    harness.run();
    (harness, kept)
}

/// The box the field took in the last frame.
fn rect_of(kept: &Rc<RefCell<Option<Typed>>>) -> Rect {
    kept.borrow().as_ref().map_or(Rect::NOTHING, |typed| typed.rect)
}

/// The visible texts painted, `(rect, text, colour)`.
fn texts(paint: &Paint) -> Vec<(Rect, String, Color32)> {
    paint
        .shapes
        .iter()
        .filter_map(|shape| match &shape.look {
            Look::Text { text, colour, .. } if colour.a() > 0 && !text.is_empty() => Some((shape.rect, text.clone(), *colour)),
            _ => None,
        })
        .collect()
}

/// Whether `shape` can be seen: text laid out for the edit alone, while a cut copy stands for it, is transparent.
fn visible(shape: &crate::component::tests::Painted) -> bool {
    !matches!(&shape.look, Look::Text { colour, .. } if colour.a() == 0)
}

/// The filled rects painted in `colour`.
fn filled(paint: &Paint, colour: Color32) -> Vec<Rect> {
    paint.rects().filter(|shape| matches!(shape.look, Look::Rect { fill, .. } if fill == colour)).map(|shape| shape.rect).collect()
}

#[test]
fn the_command_field_looks_as_its_sheet_says_in_every_state() {
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::default());
        let p = theme.palette;
        for state in &STATES {
            let (harness, kept) = board(theme, state.scope, state.text, state.forced);
            let rect = rect_of(&kept);
            let paint = painted_in(&harness, spoken(), rect);
            let at = format!("{mode:?} {}: {paint:#?}", state.name);
            assert_eq!(rect.size(), vec2(560.0, 28.0), "{at}");
            assert_eq!(rect.min, rect.min.round(), "{at}");
            let boxed = paint.rects().find(|shape| shape.rect == rect).map(|shape| shape.look.clone());
            let want =
                Look::Rect { fill: p.term, stroke: Some((1.0, (state.edge)(p))), stroke_kind: egui::StrokeKind::Inside, radius: 0.0, dashed: false };
            assert_eq!(boxed, Some(want), "{at}");
            let shown = texts(&paint);
            let find = |text: &str| shown.iter().find(|shown| shown.1 == text).map(|shown| (shown.0.min - rect.min, shown.2));
            assert_eq!(find(">"), Some((vec2(13.0, 4.0), p.cyan_text)), "the prompt: {at}");
            for (x, text, colour) in state.texts {
                assert_eq!(find(text), Some((vec2(*x, 4.0), colour(p))), "{text}: {at}");
            }
            let carets: Vec<Rect> = filled(&paint, p.cyan_edge).into_iter().filter(|caret| caret.size() == vec2(7.0, 16.0)).collect();
            assert_eq!(
                carets.iter().map(|caret| caret.min - rect.min).collect::<Vec<Vec2>>(),
                state.caret.map(|x| vec2(x, 6.0)).into_iter().collect::<Vec<_>>(),
                "the caret: {at}"
            );
            let marks: Vec<Rect> = filled(&paint, p.cyan).into_iter().filter(|mark| mark.width() >= 16.0).collect();
            match state.mark {
                Some((x, wide, text)) => {
                    assert_eq!(marks, vec![Rect::from_min_size(rect.min + vec2(x, 6.0), vec2(wide, 16.0))], "the mark: {at}");
                    let on = shown.iter().find(|shown| shown.1 == text && marks.iter().any(|mark| mark.contains_rect(shown.0.shrink(0.5))));
                    assert_eq!(on.map(|on| on.2), Some(p.on_cyan), "the mark's text: {at}");
                }
                None => assert!(marks.is_empty(), "no mark: {at}"),
            }
            let chip_left = rect.right() - 13.0 - if state.key == "Esc" { 30.0 } else { 50.0 };
            let key = shown.iter().find(|shown| shown.1 == state.key);
            assert_eq!(key.map(|key| (key.0.left() - chip_left, key.2)), Some((5.0, p.muted)), "the key {}: {at}", state.key);
            let name = state.scope_name.and_then(|name| shown.iter().find(|shown| shown.1 == name));
            match state.scope_name {
                Some(_) => {
                    let name = name.map(|name| (name.0.right().ceil(), name.2));
                    assert!(
                        name.is_some_and(|(right, colour)| (right - (chip_left - 8.0)).abs() <= 1.0 && colour == p.muted),
                        "the scope's name: {name:?} {at}"
                    );
                }
                None => {
                    assert!(!shown.iter().any(|shown| ["ALL MAIL", "COMMANDS", "PEOPLE", "MAILBOXES", "MAIL"].contains(&shown.1.as_str())), "{at}")
                }
            }
            if state.name == "long" {
                let cut = shown.iter().find(|shown| shown.1.starts_with("from:teodor"));
                let Some(cut) = cut else { panic!("no text: {at}") };
                assert!(
                    cut.1.ends_with('…') && cut.0.left() - rect.left() == 29.0 && cut.0.right() <= rect.left() + 494.0 + 0.5,
                    "cut with one ellipsis: {at}"
                );
                assert_eq!(cut.2, p.ink);
            }
            assert!(!paint.shapes.iter().any(|shape| shape.overflows && visible(shape)), "{at}");
        }
    }
}

#[test]
fn at_130_percent_the_heights_scale_and_the_widths_stay() {
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::ALL[4]);
        let p = theme.palette;
        assert_eq!(height(&theme), 36.0);
        for state in &STATES {
            let (harness, kept) = board(theme, state.scope, state.text, state.forced);
            let rect = rect_of(&kept);
            let paint = painted_in(&harness, spoken(), rect);
            let at = format!("{mode:?} {}: {paint:#?}", state.name);
            assert_eq!(rect.size(), vec2(560.0, 36.0), "{at}");
            for (text_rect, text, _) in texts(&paint) {
                assert!(rect.shrink(1.0).contains_rect(text_rect), "{text} leaves the box: {at}");
                assert!((text_rect.center().y - rect.center().y).abs() <= 1.5, "{text} in the middle: {at}");
            }
            let carets: Vec<Rect> = filled(&paint, p.cyan_edge).into_iter().filter(|caret| caret.width() < 16.0).collect();
            assert_eq!(carets.len(), usize::from(state.caret.is_some()), "{at}");
            for caret in carets {
                assert_eq!((caret.size(), caret.min, caret.center().y), (vec2(9.0, 20.0), caret.min.round(), rect.center().y), "{at}");
            }
            for mark in filled(&paint, p.cyan).into_iter().filter(|mark| mark.width() >= 16.0) {
                assert_eq!((mark.height(), mark.min), (20.0, mark.min.round()), "{at}");
            }
            // Widths, insets and gaps stay: the prompt 13 in, the chip 13 from the right end, 8 between the parts.
            let shown = texts(&paint);
            let prompt = shown.iter().find(|shown| shown.1 == ">").map_or(Rect::NOTHING, |shown| shown.0);
            assert_eq!(prompt.left() - rect.left(), 13.0, "the prompt: {at}");
            let chip = paint
                .rects()
                .find(|shape| matches!(shape.look, Look::Rect { radius, .. } if radius == 2.0))
                .map_or(Rect::NOTHING, |shape| shape.rect);
            assert_eq!(rect.right() - chip.right(), 13.0, "the chip: {at}");
            if let Some(name) = state.scope_name.and_then(|name| shown.iter().find(|shown| shown.1 == name)) {
                assert!((chip.left() - 8.0 - name.0.right().ceil()).abs() <= 1.0, "the scope's name: {at}");
            }
            let next = prompt.left() + crate::draw::paint::run_box(prompt.width()) + 8.0;
            let first = filled(&paint, p.cyan).into_iter().find(|mark| mark.width() >= 16.0).map(|mark| mark.left());
            let first =
                first.or_else(|| state.texts.first().and_then(|want| shown.iter().find(|shown| shown.1 == want.1)).map(|shown| shown.0.left()));
            if let Some(first) = first.filter(|_| state.name != "focus") {
                assert_eq!(first, next, "8 after the prompt: {at}");
            }
            assert!(!paint.shapes.iter().any(|shape| shape.overflows && visible(shape)), "{at}");
        }
    }
}

/// A live field over a text kept in the harness's state; the last frame's [`Typed`] kept beside it.
fn live(text: &str, scope: Scope<'static>) -> (Harness<'static, String>, Rc<RefCell<Vec<Typed>>>) {
    let theme = Theme::new(Mode::Light, TextScale::default());
    let kept = Rc::new(RefCell::new(Vec::new()));
    let seen = Rc::clone(&kept);
    let mut harness = builder::<String>().allow_missing_glyphs().with_size(vec2(640.0, 120.0)).build_ui_state(
        move |ui, text| {
            crate::motion::set_reduced(ui.ctx(), true);
            stage(ui, theme.palette.panel, |ui| {
                ui.set_width(560.0);
                let typed = command_field(ui, &theme, Id::unique("command"), text, scope, "Ctrl K", None);
                seen.borrow_mut().push(typed);
            });
        },
        text.to_owned(),
    );
    harness.run();
    (harness, kept)
}

#[test]
fn a_long_paste_is_capped() {
    let hostile: String = "a\r\n\u{0}\u{202e}b\u{2066}".repeat(2_000);
    assert!(hostile.chars().count() >= 10_000);
    let (mut harness, _) = live("", Scope::Open);
    harness.get_by_role_and_label(Role::TextInput, spoken()).focus();
    harness.run();
    harness.event(egui::Event::Paste(hostile.clone()));
    // In the very frame of the paste: what went in was cut where it went in.
    harness.step();
    assert_eq!(harness.state().chars().count(), MOST, "a paste of 10 000 is cut at the cap");
    // Its control and bidi characters are dropped as it goes in, then the cap holds what is kept.
    let kept: String = hostile.chars().filter(|c| !c.is_control() && !matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')).collect();
    assert!(kept.starts_with(harness.state().as_str()), "its beginning is kept, cleaned");
    harness.run();
    // Typed past the cap, nothing more goes in.
    harness.event(egui::Event::Text("zzz".to_owned()));
    harness.run();
    assert_eq!(harness.state().chars().count(), MOST);
    // A text the caller gives that is longer is cut on the first frame, between graphemes.
    let given = "e\u{301}".repeat(5_000);
    let (harness, kept) = live(&given, Scope::Mailbox(Box::leak(hostile.into_boxed_str())));
    assert!(harness.state().chars().count() <= MOST && given.starts_with(harness.state().as_str()), "cut to the cap");
    assert!(harness.state().ends_with('\u{301}'), "never in the middle of a letter");
    let rect = kept.borrow().last().map_or(Rect::NOTHING, |typed| typed.rect);
    let paint = painted_in(&harness, spoken(), rect);
    for (text_rect, text, _) in texts(&paint) {
        assert!(rect.contains_rect(text_rect), "{text:?} leaves the box");
    }
    let mark = filled(&paint, Theme::new(Mode::Light, TextScale::default()).palette.cyan);
    assert!(mark.iter().all(|mark| mark.width() <= 560.0 / 3.0), "a long mailbox name keeps to a third of the field: {mark:?}");
}

#[test]
fn the_field_is_one_tab_stop_and_says_what_the_keys_asked() {
    let (mut harness, kept) = live("", Scope::Commands);
    assert!(keyboard_walk(&mut harness).is_empty());
    let (mut harness, kept2) = live("", Scope::Commands);
    let last = |kept: &Rc<RefCell<Vec<Typed>>>, f: fn(&Typed) -> bool| kept.borrow().iter().any(f);
    harness.key_press(Key::Tab);
    harness.run();
    assert!(harness.get_by_role_and_label(Role::TextInput, spoken()).accesskit_node().is_focused());
    kept2.borrow_mut().clear();
    harness.key_press(Key::Backspace);
    harness.run();
    assert!(last(&kept2, |typed| typed.back), "Backspace in an empty field");
    harness.get_by_role_and_label(Role::TextInput, spoken()).type_text("snooze");
    harness.run();
    assert_eq!(harness.state(), "snooze");
    kept2.borrow_mut().clear();
    harness.key_press(Key::Backspace);
    harness.run();
    assert!(!last(&kept2, |typed| typed.back), "Backspace with text takes a letter");
    assert_eq!(harness.state(), "snooz");
    kept2.borrow_mut().clear();
    harness.key_press(Key::Enter);
    harness.run();
    assert!(last(&kept2, |typed| typed.enter));
    assert!(harness.get_by_role_and_label(Role::TextInput, spoken()).accesskit_node().is_focused(), "Enter keeps the focus");
    kept2.borrow_mut().clear();
    harness.key_press(Key::Escape);
    harness.run();
    assert!(last(&kept2, |typed| typed.esc));
    assert!(kept.borrow().iter().all(|typed| !typed.enter && !typed.esc), "the walk asked nothing");
}

#[test]
fn the_key_is_the_opening_key_at_rest_and_esc_with_the_focus() {
    let (mut harness, kept) = live("", Scope::Open);
    let rect = kept.borrow().last().map_or(Rect::NOTHING, |typed| typed.rect);
    let keys = |harness: &Harness<'static, String>| {
        texts(&painted_in(harness, spoken(), rect))
            .into_iter()
            .map(|text| text.1)
            .filter(|text| text == "Ctrl K" || text == "Esc")
            .collect::<Vec<_>>()
    };
    assert_eq!(keys(&harness), ["Ctrl K"]);
    harness.get_by_role_and_label(Role::TextInput, spoken()).focus();
    harness.run();
    assert_eq!(keys(&harness), ["Esc"]);
}

#[test]
fn a_click_anywhere_in_the_box_gives_it_the_focus() {
    let (mut harness, kept) = live("", Scope::Open);
    let rect = kept.borrow().last().map_or(Rect::NOTHING, |typed| typed.rect);
    // On the prompt, outside the text's own place.
    harness.hover_at(rect.left_center() + vec2(15.0, 0.0));
    harness.run();
    harness.event(egui::Event::PointerButton {
        pos: rect.left_center() + vec2(15.0, 0.0),
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    harness.event(egui::Event::PointerButton {
        pos: rect.left_center() + vec2(15.0, 0.0),
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    harness.run();
    assert!(harness.get_by_role_and_label(Role::TextInput, spoken()).accesskit_node().is_focused());
}

#[test]
fn at_rest_it_rests_and_with_the_focus_only_the_caret_blinks() {
    let (mut harness, _) = live("bak", Scope::Open);
    assert!(rests(&mut harness, 8));
    harness.get_by_role_and_label(Role::TextInput, spoken()).focus();
    harness.run();
    harness.step();
    let delay = harness.output().viewport_output.values().map(|viewport| viewport.repaint_delay).min().unwrap_or_default();
    assert!(delay > std::time::Duration::from_millis(100) && delay <= std::time::Duration::from_millis(500), "the caret's timed repaint: {delay:?}");
}

#[test]
fn ten_thousand_hostile_characters_cost_no_more_than_the_cap() {
    // The work of a frame is what is handed to layout: never more than the cap, from the very first frame.
    super::LAID_OUT.with(|most| most.set(0));
    let hostile: String = "\u{202e}x\u{0}\r".repeat(2_500);
    let (mut harness, _) = live(&hostile, Scope::Open);
    harness.get_by_role_and_label(Role::TextInput, spoken()).focus();
    harness.event(egui::Event::Paste("y".repeat(10_000)));
    harness.run_steps(4);
    assert!(harness.state().chars().count() <= MOST);
    let laid = super::LAID_OUT.with(std::cell::Cell::get);
    assert!(laid > 0 && laid <= MOST, "{laid} characters laid out at once");
}

#[test]
fn what_enters_the_field_is_one_line_of_plain_words() {
    // A paste holding LF, NUL and U+202E: dropped, and the caret after what was kept.
    let (mut harness, _) = live("", Scope::Open);
    harness.get_by_role_and_label(Role::TextInput, spoken()).focus();
    harness.run();
    harness.event(egui::Event::Paste("ab\ncd\u{0}e\u{202e}f\u{2067}g".to_owned()));
    harness.run();
    assert_eq!(harness.state(), "abcdefg");
    harness.event(egui::Event::Text("h".to_owned()));
    harness.run();
    assert_eq!(harness.state(), "abcdefgh", "the caret stands after what was kept");
    // A typed text event holding a newline and a tab.
    harness.event(egui::Event::Text("i\nj\tk".to_owned()));
    harness.run();
    assert_eq!(harness.state(), "abcdefghijk");
    // The cap holds after cleaning: 2 000 kept letters of a 4 000-character paste.
    harness.event(egui::Event::Paste("\u{0}z".repeat(2_000)));
    harness.run();
    assert_eq!(harness.state().chars().count(), MOST);
    assert!(harness.state().chars().all(|c| !c.is_control()));
    // The caller's text is cleaned before it is laid out, and stays so.
    super::LAID_OUT.with(|most| most.set(0));
    let (harness, _) = live("p\u{0}q\r\n\u{2066}r\u{85}s", Scope::Open);
    assert_eq!(harness.state(), "pqrs");
}

#[test]
fn a_screen_readers_value_is_cleaned_too() {
    let (mut harness, _) = live("", Scope::Open);
    let target = Id::unique("command").accesskit_id();
    harness.event(egui::Event::AccessKitActionRequest(egui::accesskit::ActionRequest {
        action: egui::accesskit::Action::SetValue,
        target_tree: egui::accesskit::TreeId::ROOT,
        target_node: target,
        data: Some(egui::accesskit::ActionData::Value("one\ntwo\u{202e}".into())),
    }));
    harness.run();
    assert_eq!(harness.state(), "onetwo");
}
