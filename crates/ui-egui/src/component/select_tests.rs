// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::accesskit::{Role, Toggled};
use egui::{Color32, Key, Rect, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::Select;
use crate::component::Shown;
use crate::component::press_tests::{Part, keys_press_as_the_pointer};
use crate::component::tests::{Look, Own, bring, builder, own, painted, staged, theme};
use crate::iconset::IconId;
use crate::motion;
use crate::theme::{Mode, TextScale, Theme};

const OPTIONS: [(u8, &str); 3] = [(0, "Never"), (1, "Daily"), (2, "Weekly")];

/// The field's width in the test window, and what the state holds: open, and the choice.
fn harness(width: f32, options: &'static [(u8, &'static str)]) -> Harness<'static, (bool, u8)> {
    let theme = theme();
    let mut harness = builder().with_size(vec2(500.0, 400.0)).build_ui_state(
        move |ui, (open, current): &mut (bool, u8)| {
            motion::set_reduced(ui.ctx(), true);
            let shown = options.iter().find(|(value, _)| value == current).map_or("Custom", |(_, name)| name);
            let select = Select { name: "Repeat", shown, options, current: Some(*current), icon: IconId::Changed, width };
            let (_, chosen) = select.show(ui, &theme, open);
            if let Some(chosen) = chosen {
                *current = chosen;
            }
        },
        (false, 0),
    );
    harness.run();
    harness
}

#[test]
fn the_closed_field_is_a_combo_box_named_for_what_it_is_and_saying_its_choice() {
    let harness = harness(240.0, &OPTIONS);
    let field = harness.get_by_role_and_label(Role::ComboBox, "Repeat");
    assert_eq!(field.accesskit_node().value().as_deref(), Some("Never"));
    assert!(harness.query_by_role_and_label(Role::MenuItemRadio, "Daily").is_none(), "closed, the choices are not there");
    assert!(field.rect().height() > 20.0);
}

#[test]
fn a_press_opens_the_choices_with_the_current_one_marked_and_taking_one_closes_them() {
    let mut harness = harness(240.0, &OPTIONS);
    harness.get_by_role_and_label(Role::ComboBox, "Repeat").click();
    harness.run();
    assert!(harness.state().0);
    assert_eq!(harness.get_by_role_and_label(Role::MenuItemRadio, "Never").accesskit_node().toggled(), Some(Toggled::True));
    assert_eq!(harness.get_by_role_and_label(Role::MenuItemRadio, "Weekly").accesskit_node().toggled(), Some(Toggled::False));
    assert!(harness.get_by_role_and_label(Role::MenuItemRadio, "Never").is_focused(), "the keyboard starts on the current choice");
    harness.get_by_role_and_label(Role::MenuItemRadio, "Weekly").click();
    harness.run();
    assert_eq!(*harness.state(), (false, 2));
    assert_eq!(harness.get_by_role_and_label(Role::ComboBox, "Repeat").accesskit_node().value().as_deref(), Some("Weekly"));
}

#[test]
fn escape_closes_the_choices_and_gives_the_focus_back() {
    let mut harness = harness(240.0, &OPTIONS);
    harness.get_by_role_and_label(Role::ComboBox, "Repeat").click();
    harness.run();
    harness.key_press(Key::Escape);
    harness.run();
    assert_eq!(*harness.state(), (false, 0), "nothing was picked");
    assert!(harness.get_by_role_and_label(Role::ComboBox, "Repeat").is_focused());
}

#[test]
fn a_press_beside_the_popup_closes_it_and_a_press_on_the_field_does_too() {
    let mut harness = harness(240.0, &OPTIONS);
    harness.get_by_role_and_label(Role::ComboBox, "Repeat").click();
    harness.run();
    harness.hover_at(pos2(480.0, 380.0));
    harness.run();
    harness.drag_at(pos2(480.0, 380.0));
    harness.run();
    assert!(!harness.state().0, "a press beside it");

    harness.drop_at(pos2(480.0, 380.0));
    harness.run();
    harness.get_by_role_and_label(Role::ComboBox, "Repeat").click();
    harness.run();
    assert!(harness.state().0);
    // A press of the pointer on the field, which lies outside the open choices: they close, and the release on the
    // field does not open them again.
    let field = harness.get_by_role_and_label(Role::ComboBox, "Repeat").rect().center();
    harness.hover_at(field);
    harness.run();
    harness.drag_at(field);
    harness.run();
    harness.drop_at(field);
    harness.run();
    assert!(!harness.state().0, "the field closes what it opened");
    assert!(harness.get_by_role_and_label(Role::ComboBox, "Repeat").is_focused(), "and has the keyboard back");
}

#[test]
fn many_or_long_choices_stay_inside_the_window() {
    const MANY: [(u8, &str); 40] = {
        let mut rows = [(0_u8, "A rather long name for a calendar, cut where it does not fit in the popup"); 40];
        let mut index = 0;
        while index < 40 {
            rows[index].0 = index as u8;
            index += 1;
        }
        rows
    };
    let mut harness = harness(120.0, &MANY);
    harness.get_by_role_and_label(Role::ComboBox, "Repeat").click();
    harness.run();
    let first = harness.get_all_by_role(Role::MenuItemRadio).next().map(|row| row.rect());
    assert!(first.is_some_and(|row| row.width() <= 130.0), "{first:?}");
}

#[test]
fn an_odd_width_is_no_trouble() {
    for width in [0.0, -5.0, f32::NAN, f32::INFINITY, 1.0] {
        let mut harness = harness(width, &OPTIONS);
        harness.get_by_role_and_label(Role::ComboBox, "Repeat").click();
        harness.run();
        assert!(harness.state().0);
    }
}

/// The states of the select's sheet, in its order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Default,
    Hover,
    Focus,
    Open,
    Disabled,
    Unavailable,
}

const STATES: [State; 6] = [State::Default, State::Hover, State::Focus, State::Open, State::Disabled, State::Unavailable];

const WEEKS: [(u8, &str); 2] = [(0, "Every week"), (1, "Every month")];

/// The select "Repeat" showing "Every week", 200 wide, in `state`: its rect, its look and its caret's rect and colour.
fn select_in(mode: Mode, scale: TextScale, state: State, shown: Shown) -> (Rect, Own, Option<(Rect, Color32)>, Harness<'static>) {
    let theme = Theme::new(mode, scale);
    let mut harness = staged(
        theme,
        |p| p.surface,
        vec2(400.0, 300.0),
        move |ui, theme| {
            if state == State::Disabled {
                ui.disable();
            }
            let note = (state == State::Unavailable).then_some("Not yet: add a calendar account first");
            let mut open = state == State::Open;
            let select = Select { name: "Repeat", shown: "Every week", options: &WEEKS, current: Some(0), icon: IconId::Changed, width: 200.0 };
            select.show_noted(ui, theme, &mut open, note);
        },
    );
    bring(&mut harness, "Repeat", shown);
    let rect = harness.get_by_role_and_label(Role::ComboBox, "Repeat").rect();
    let paint = painted(&harness, "Repeat");
    let caret = paint.rects().find_map(|shape| match shape.look {
        Look::Rect { fill, stroke_kind: egui::StrokeKind::Middle, .. } if fill.a() > 0 && shape.rect.width() < 10.0 => Some((shape.rect, fill)),
        _ => None,
    });
    (rect, own(&paint, rect), caret, harness)
}

#[test]
fn a_select_looks_as_its_sheet_says_in_every_state() {
    for mode in [Mode::Dark, Mode::Light] {
        let p = Theme::new(mode, TextScale::default()).palette;
        for state in STATES {
            let shown = match state {
                State::Hover => Shown::Hover,
                State::Focus => Shown::Focus,
                _ => Shown::Rest,
            };
            let (rect, look, caret, _) = select_in(mode, TextScale::default(), state, shown);
            let at = format!("{mode:?} {state:?}: {look:#?} {caret:?}");
            assert!((rect.width() - 200.0).abs() < 0.01 && (rect.height() - 36.0).abs() < 0.01, "{at} {rect:?}");
            let (fill, edge, value, mark) = match state {
                State::Default | State::Focus => (Some(p.term), (p.line, false), p.ink, p.muted),
                State::Hover => (Some(p.term), (p.muted, false), p.ink, p.ink),
                State::Open => (Some(p.term), (p.cyan_edge, false), p.ink, p.cyan_text),
                State::Disabled | State::Unavailable => (None, (p.line, true), p.muted, p.dim),
            };
            assert_eq!(look.fill, fill, "{at}");
            assert_eq!(look.edge, Some((1.0, edge.0, edge.1)), "{at}");
            let texts: Vec<_> = look.texts.iter().filter(|(text, ..)| text == "Every week").cloned().collect();
            assert_eq!(texts, vec![("Every week".to_owned(), value, "Monospace".to_owned(), 13.0)], "{at}");
            let place = Rect::from_min_size(rect.min + vec2(180.0, 15.5), vec2(8.0, 5.0));
            assert_eq!(caret, Some((place, mark)), "{at}");
            let ring = (state == State::Focus).then_some((rect.expand(3.0), 1.0, p.cyan_edge));
            assert_eq!(look.ring, ring, "{at}");
            assert!(look.radius <= 0.0 && !look.overflows, "{at}");
        }
    }
}

#[test]
fn a_select_at_130_percent_is_the_field_height() {
    let theme = Theme::new(Mode::Light, TextScale::ALL[4]);
    let (rect, ..) = select_in(Mode::Light, TextScale::ALL[4], State::Default, Shown::Rest);
    assert!((rect.height() - theme.boxes().field).abs() < 0.01, "{rect:?}");
}

#[test]
fn an_unavailable_select_takes_no_press_and_its_tooltip_says_why() {
    let (_, _, _, mut harness) = select_in(Mode::Light, TextScale::default(), State::Unavailable, Shown::Hover);
    assert!(harness.get_by_role_and_label(Role::ComboBox, "Repeat").accesskit_node().is_disabled());
    for _ in 0..90 {
        harness.step();
    }
    assert!(harness.query_by_label_contains("add a calendar account first").is_some(), "the reason shows under the pointer");
}

#[test]
fn the_choices_open_under_the_field() {
    let mut harness = harness(240.0, &OPTIONS);
    harness.get_by_role_and_label(Role::ComboBox, "Repeat").click();
    harness.run();
    let field = harness.get_by_role_and_label(Role::ComboBox, "Repeat").rect();
    let first = harness.get_all_by_role(Role::MenuItemRadio).next().map(|row| row.rect());
    assert!(first.is_some_and(|row| row.top() >= field.bottom()), "{first:?} under {field:?}");
}

#[test]
fn a_pressed_select_is_the_stage_fill_with_an_ink_edge_and_caret() {
    for mode in [Mode::Dark, Mode::Light] {
        let p = Theme::new(mode, TextScale::default()).palette;
        let (_, look, caret, _) = select_in(mode, TextScale::default(), State::Default, Shown::Pressed);
        assert_eq!((look.fill, look.edge), (Some(p.stage), Some((1.0, p.ink, false))), "{mode:?} {look:#?}");
        assert_eq!(caret.map(|(_, colour)| colour), Some(p.ink));
    }
}

#[test]
fn enter_and_space_press_a_focused_select() {
    let whole: Part = |rect, _| rect;
    let ring: Part = |rect, _| rect.expand(3.0);
    keys_press_as_the_pointer("Repeat", vec2(400.0, 200.0), whole, ring, |ui, theme| {
        let mut open = false;
        Select { name: "Repeat", shown: "Every week", options: &WEEKS, current: Some(0), icon: IconId::Changed, width: 200.0 }
            .show(ui, theme, &mut open);
    });
}

#[test]
fn the_gallery_forces_a_hovered_select() {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let harness = staged(
        theme,
        |p| p.surface,
        vec2(400.0, 200.0),
        |ui, theme| {
            let select = Select { name: "Repeat", shown: "Every week", options: &WEEKS, current: Some(0), icon: IconId::Changed, width: 200.0 };
            select.show_with(ui, theme, &mut false, None, Some(Shown::Hover));
        },
    );
    let rect = harness.get_by_role_and_label(Role::ComboBox, "Repeat").rect();
    assert_eq!(own(&painted(&harness, "Repeat"), rect).edge, Some((1.0, theme.palette.muted, false)));
}

#[test]
fn enter_opens_the_choices_when_it_comes_up() {
    let mut harness = harness(240.0, &OPTIONS);
    harness.get_by_role_and_label(Role::ComboBox, "Repeat").focus();
    harness.run();
    harness.key_down(Key::Enter);
    harness.run();
    assert!(!harness.state().0, "held down, it is pressed");
    harness.key_up(Key::Enter);
    harness.run();
    assert!(harness.state().0, "let go, it opens");
    assert!(harness.get_by_role_and_label(Role::MenuItemRadio, "Never").is_focused());
}

#[test]
fn a_choice_taken_by_enter_or_space_leaves_the_choices_closed_when_the_key_comes_up() {
    for key in [Key::Enter, Key::Space] {
        let mut harness = harness(240.0, &OPTIONS);
        harness.get_by_role_and_label(Role::ComboBox, "Repeat").click();
        harness.run();
        harness.get_by_role_and_label(Role::MenuItemRadio, "Weekly").focus();
        harness.run();
        harness.key_down(key);
        harness.run();
        assert_eq!(*harness.state(), (false, 2), "{key:?} down takes the row and closes the list");
        harness.key_up(key);
        harness.run();
        for _ in 0..4 {
            harness.step();
        }
        assert_eq!(*harness.state(), (false, 2), "{key:?} up opens nothing");
        assert!(harness.get_by_role_and_label(Role::ComboBox, "Repeat").is_focused(), "the focus is back on the field");
    }
}

#[test]
fn a_key_held_on_the_closed_field_shows_it_pressed_and_opens_it_once_when_it_comes_up() {
    for key in [Key::Enter, Key::Space] {
        let mut harness = harness(240.0, &OPTIONS);
        harness.get_by_role_and_label(Role::ComboBox, "Repeat").focus();
        harness.run();
        harness.key_down(key);
        harness.run();
        assert!(!harness.state().0, "{key:?} held: pressed, not open");
        harness.key_up(key);
        harness.run();
        for _ in 0..4 {
            harness.step();
        }
        assert!(harness.state().0, "{key:?} up: open, and it stays open");
    }
}

#[test]
fn a_select_of_any_width_returns_and_is_at_least_as_wide_as_high() {
    let theme = theme();
    let high = super::height(&theme);
    for width in [f32::NAN, f32::INFINITY, -f32::INFINITY, -40.0, 0.0, 1.0, 1e9] {
        let ctx = egui::Context::default();
        let mut wide = 0.0;
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let mut open = false;
            let (response, _) = Select { name: "Repeat", shown: "Every week", options: &WEEKS, current: Some(0), icon: IconId::Changed, width }
                .show_with(ui, &theme, &mut open, None, None);
            wide = response.rect.width();
        });
        output.textures_delta.clear();
        assert!((high..=2048.0).contains(&wide), "width {width}: {wide}");
    }
}

/// A select of 40 choices 240 wide in a window too low for them, open, in `mode`, egui's scroll style the theme's.
fn overflowing(mode: Mode) -> Harness<'static, (bool, u8)> {
    const MANY: [(u8, &str); 40] = {
        let mut rows = [(0_u8, "Every week"); 40];
        let mut index = 0;
        while index < 40 {
            rows[index].0 = index as u8;
            index += 1;
        }
        rows
    };
    let theme = Theme::new(mode, TextScale::default());
    let mut harness = builder().with_size(vec2(500.0, 400.0)).build_ui_state(
        move |ui, (open, current): &mut (bool, u8)| {
            motion::set_reduced(ui.ctx(), true);
            ui.ctx().all_styles_mut(|style| style.spacing.scroll = crate::component::scroll::style(&theme));
            let select = Select { name: "Repeat", shown: "Every week", options: &MANY, current: Some(*current), icon: IconId::Changed, width: 240.0 };
            let (_, chosen) = select.show(ui, &theme, open);
            if let Some(chosen) = chosen {
                *current = chosen;
            }
        },
        (false, 0),
    );
    harness.run();
    harness.get_by_role_and_label(Role::ComboBox, "Repeat").click();
    harness.run();
    harness
}

/// The filled rects of `fill` painted in the last frame, wherever they stand.
fn filled(harness: &Harness<'static, (bool, u8)>, fill: Color32) -> Vec<Rect> {
    let mut found = Vec::new();
    for clipped in &harness.output().shapes {
        let mut stack = vec![&clipped.shape];
        while let Some(shape) = stack.pop() {
            match shape {
                egui::Shape::Vec(shapes) => stack.extend(shapes.iter()),
                egui::Shape::Rect(rect) if rect.fill == fill => found.push(rect.rect),
                _ => {}
            }
        }
    }
    found
}

#[test]
fn an_open_select_too_long_for_the_window_shows_the_sheets_scroll_thumb_beside_its_rows() {
    for mode in [Mode::Dark, Mode::Light] {
        let p = Theme::new(mode, TextScale::default()).palette;
        let harness = overflowing(mode);
        assert!(harness.state().0, "{mode:?}: open");
        let rows: Vec<Rect> = harness.get_all_by_role(Role::MenuItemRadio).map(|row| row.rect()).collect();
        let (Some(first), Some(focused)) =
            (rows.first().copied(), harness.query_all_by_role(Role::MenuItemRadio).find(|row| row.is_focused()).map(|row| row.rect()))
        else {
            panic!("{mode:?}: rows and a focused one, {rows:?}");
        };
        let strip = Rect::from_x_y_ranges(first.right()..=first.right() + 12.0, first.top()..=first.top() + 400.0);
        let thumbs: Vec<Rect> = filled(&harness, p.muted).into_iter().filter(|rect| strip.expand(0.01).contains_rect(*rect)).collect();
        let [thumb] = thumbs[..] else { panic!("{mode:?}: one muted thumb beside the rows, {thumbs:?}") };
        assert!(
            (thumb.left() - (strip.left() + 4.0)).abs() < 0.01 && (thumb.width() - 4.0).abs() < 0.01,
            "{mode:?}: 4 wide in the middle of the 12, {thumb:?} {strip:?}"
        );
        assert!((thumb.top() - first.top()).abs() < 0.01 && thumb.height() >= 32.0, "{mode:?}: at the top, at least 32 high, {thumb:?}");
        assert!(rows.iter().all(|row| !thumb.intersects(row.shrink(0.01))), "{mode:?}: the thumb over a row, {thumb:?}");
        // A focused row in the list is lit to its edge: the thumb stands clear of that edge.
        assert!(thumb.left() >= focused.right() - 0.01, "{mode:?}: the thumb over the focused row's edge, {thumb:?} {focused:?}");
    }
}

#[test]
fn a_held_key_repeating_on_a_row_never_opens_the_choices_again() {
    for key in [Key::Enter, Key::Space] {
        let mut harness = harness(240.0, &OPTIONS);
        harness.get_by_role_and_label(Role::ComboBox, "Repeat").click();
        harness.run();
        harness.get_by_role_and_label(Role::MenuItemRadio, "Weekly").focus();
        harness.run();
        harness.key_down(key);
        harness.run();
        assert_eq!(*harness.state(), (false, 2), "{key:?} takes the row");
        // The key held: the system repeats its press.
        for _ in 0..3 {
            harness.event(egui::Event::Key { key, physical_key: None, pressed: true, repeat: true, modifiers: egui::Modifiers::NONE });
            harness.run();
        }
        harness.key_up(key);
        harness.run();
        harness.run_steps(4);
        assert_eq!(*harness.state(), (false, 2), "{key:?}: the repeats and the release open nothing");
    }
}

#[test]
fn a_select_whose_choice_the_options_no_longer_hold_opens_on_its_first_row() {
    let theme = theme();
    let mut harness = builder().with_size(vec2(500.0, 400.0)).build_ui_state(
        move |ui, open: &mut bool| {
            motion::set_reduced(ui.ctx(), true);
            let select = Select { name: "Account", shown: "Gone", options: &OPTIONS, current: Some(99), icon: IconId::Changed, width: 240.0 };
            select.show(ui, &theme, open);
        },
        false,
    );
    harness.run();
    harness.get_by_role_and_label(Role::ComboBox, "Account").click();
    harness.run();
    assert!(harness.get_by_role_and_label(Role::MenuItemRadio, "Never").is_focused(), "the first row has the keyboard");
}

#[test]
fn a_screen_reader_s_click_on_the_field_closes_the_open_choices() {
    let mut harness = harness(240.0, &OPTIONS);
    harness.get_by_role_and_label(Role::ComboBox, "Repeat").click_accesskit();
    harness.run();
    assert!(harness.state().0, "it opens");
    harness.get_by_role_and_label(Role::ComboBox, "Repeat").click_accesskit();
    harness.run();
    assert_eq!(*harness.state(), (false, 0), "the field closes what it opened, nothing taken");
    assert!(harness.get_by_role_and_label(Role::ComboBox, "Repeat").is_focused());
}
