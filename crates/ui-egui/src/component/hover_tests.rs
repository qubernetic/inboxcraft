// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::accesskit::Role;
use egui::{Color32, Id, Key, Rect, StrokeKind, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::{Action, Shown, actions, actions_with};
use crate::component::tests::{Look, Paint, builder, icons, painted, rests, staged, theme};
use crate::component::{Shown as State, tests::bring};
use crate::iconset::IconId;
use crate::keymap::{self, Keymap};
use crate::theme::{Mode, TextScale, Theme};

const ROW: Rect = Rect { min: pos2(0.0, 40.0), max: pos2(500.0, 80.0) };

const ACTIONS: [Action<'static>; 3] = [
    Action { icon: IconId::Done, name: "Done", action: Some("mail.done"), unavailable: None },
    Action { icon: IconId::MarkRead, name: "Mark as read", action: None, unavailable: None },
    Action { icon: IconId::Delete, name: "Delete", action: None, unavailable: None },
];

/// The sheet's four actions: Done D, Snooze H, Read R, Delete Del; Snooze unavailable when `snooze` says why.
fn sheet(snooze: Option<&'static str>) -> [Action<'static>; 4] {
    [
        Action { icon: IconId::Done, name: "Done", action: Some(keymap::DONE), unavailable: None },
        Action { icon: IconId::Snooze, name: "Snooze", action: Some(keymap::SNOOZE), unavailable: snooze },
        Action { icon: IconId::MarkRead, name: "Mark as read", action: Some(keymap::READ), unavailable: None },
        Action { icon: IconId::Delete, name: "Delete", action: Some(keymap::DELETE), unavailable: None },
    ]
}

/// The group over a row, shown when the state's flag says so; the state keeps what the group drew.
fn harness(shown: bool) -> Harness<'static, (bool, Option<Shown>)> {
    let theme = theme();
    let mut harness = builder().build_ui_state(
        move |ui, (shown, drawn): &mut (bool, Option<Shown>)| {
            *drawn = Some(actions(ui, &theme, &Keymap::default(), Id::unique("row"), ROW, *shown, &ACTIONS));
        },
        (shown, None),
    );
    harness.run();
    harness
}

#[test]
fn a_hidden_group_draws_nothing_and_leaves_the_row_its_width() {
    let harness = harness(false);
    assert!(harness.query_by_label("Done").is_none());
    assert_eq!(harness.state().1, Some(Shown { clicked: None, left: ROW.right(), openness: 0.0 }));
}

#[test]
fn a_shown_group_offers_its_actions_at_the_right_end_of_the_line() {
    let harness = harness(true);
    let shown = harness.state().1.unwrap();
    assert_eq!(shown.openness, 1.0);
    assert!(shown.left > ROW.center().x && shown.left < ROW.right(), "{shown:?}");
    for name in ["Done", "Mark as read", "Delete"] {
        let rect = harness.get_by_label(name).rect();
        assert!(rect.left() >= shown.left && rect.right() <= ROW.right() && ROW.contains(rect.center()), "{name} {rect:?}");
    }
}

#[test]
fn the_group_shows_and_hides_at_once_as_the_presentation_has_it() {
    let mut harness = harness(false);
    harness.state_mut().0 = true;
    harness.step();
    assert_eq!(harness.state().1.unwrap().openness, 1.0, "in the first frame");
    harness.get_by_label("Done");
    harness.state_mut().0 = false;
    harness.step();
    assert!(harness.query_by_label("Done").is_none(), "gone in the first frame");
    assert!(rests(&mut harness, 4));
}

#[test]
fn a_press_on_an_action_names_it() {
    let theme = theme();
    let mut harness = builder().build_ui_state(
        move |ui, clicked: &mut Vec<usize>| {
            let shown = actions(ui, &theme, &Keymap::default(), Id::unique("row"), ROW, true, &ACTIONS);
            clicked.extend(shown.clicked);
        },
        Vec::new(),
    );
    harness.run();
    harness.get_by_label("Mark as read").click();
    harness.run();
    assert_eq!(harness.state(), &[1]);
}

/// The sheet's group on a `stage` row, as wide as its line, with Snooze unavailable or not.
fn group(theme: Theme, snooze: Option<&'static str>, forced: Option<(usize, State)>) -> Harness<'static> {
    staged(
        theme,
        |p| p.stage,
        vec2(320.0, 120.0),
        move |ui, theme| {
            let (_, line) = ui.allocate_space(vec2(197.0, theme.boxes().strip));
            actions_with(ui, theme, &Keymap::default(), Id::unique("sheet"), line, true, &sheet(snooze), forced);
        },
    )
}

/// One action's look: `(rect, fill, edge (colour, dashed), glyph colour, chip (text, edge colour, dashed))`.
#[derive(Debug, PartialEq)]
struct ActionLook {
    rect: Rect,
    fill: Option<Color32>,
    edge: Option<Color32>,
    glyph: Option<Color32>,
    chip: Option<(String, Color32, bool, Rect)>,
}

fn action_look(paint: &Paint, rect: Rect) -> ActionLook {
    let mut found = ActionLook { rect, fill: None, edge: None, glyph: None, chip: None };
    let mut chip_edge = None;
    let mut chip_text = None;
    for shape in &paint.shapes {
        match &shape.look {
            Look::Rect { fill, stroke, stroke_kind, radius, dashed } => {
                if shape.rect == rect && fill.a() > 0 {
                    found.fill = Some(*fill);
                }
                if let Some((width, colour)) = stroke {
                    assert_eq!(*width, 1.0, "{shape:?}");
                    if shape.rect == rect && *stroke_kind == StrokeKind::Inside {
                        found.edge = Some(*colour);
                    } else if *radius == 2.0 {
                        chip_edge = Some((*colour, *dashed, shape.rect));
                    }
                }
            }
            Look::Text { text, colour, family, .. } => {
                if *family == icons() {
                    found.glyph = Some(*colour);
                } else {
                    chip_text = Some(text.clone());
                }
            }
            Look::Shadow { .. } => {}
        }
    }
    if let (Some(text), Some((edge, dashed, at))) = (chip_text, chip_edge) {
        found.chip = Some((text, edge, dashed, at));
    }
    found
}

/// The sheet's actions: name, key, width.
const SHEET: [(&str, &str, f32); 4] = [("Done", "D", 43.0), ("Snooze", "H", 43.0), ("Mark as read", "R", 43.0), ("Delete", "Del", 56.0)];

#[test]
fn hover_actions_look_as_their_sheet_says_in_every_state() {
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::default());
        let p = theme.palette;
        for (state, snooze, forced) in [
            ("default", None, None),
            ("action-hover", None, Some((0, State::Hover))),
            ("focus", None, Some((0, State::Focus))),
            ("unavailable", Some("Not yet: the time picker comes with the pickers"), None),
        ] {
            let harness = group(theme, snooze, forced);
            let mut left = None;
            let mut right = 0.0;
            for (index, (name, key, width)) in SHEET.iter().enumerate() {
                let node = harness.get_by_label(name);
                let rect = node.rect();
                let at = format!("{mode:?} {state} {name}");
                assert_eq!(node.accesskit_node().role(), Role::Button, "{at}");
                assert_eq!((rect.width(), rect.height()), (*width, 24.0), "{at} {rect:?}");
                if let Some(left) = left {
                    assert_eq!(rect.left() - right, 4.0, "the group's gap: {at}");
                    let _: f32 = left;
                }
                left.get_or_insert(rect.left());
                right = rect.right();
                let look = action_look(&painted(&harness, name), rect);
                let lit = index == 0 && forced.is_some();
                let unavailable = index == 1 && snooze.is_some();
                let edge = match (state, lit) {
                    ("action-hover", true) => Some(p.muted),
                    ("focus", true) => Some(p.cyan_edge),
                    _ => None,
                };
                let glyph = if lit {
                    p.ink
                } else if unavailable {
                    p.dim
                } else {
                    p.muted
                };
                assert_eq!(look.fill, None, "{at}: {look:?}");
                assert_eq!(look.edge, edge, "{at}: {look:?}");
                assert_eq!(look.glyph, Some(glyph), "{at}: {look:?}");
                let chip = look.chip.clone().unwrap_or_else(|| panic!("{at}: no chip {look:?}"));
                assert_eq!((chip.0.as_str(), chip.1, chip.2), (*key, p.line, unavailable), "{at}: {look:?}");
                assert_eq!(chip.3.min - rect.min, vec2(22.0, 2.0), "the chip at 22,2: {at}");
                assert_eq!(chip.3.height(), 20.0, "{at}");
            }
            assert_eq!(right - left.unwrap_or(0.0), 197.0, "{mode:?} {state}: the group is 197 wide");
        }
    }
}

#[test]
fn an_unavailable_action_cannot_be_pressed_and_says_why() {
    let theme = Theme::new(Mode::Light, TextScale::default());
    let harness = group(theme, Some("Not yet: the time picker comes with the pickers"), None);
    let node = harness.get_by_label("Snooze");
    assert!(node.accesskit_node().is_disabled());
}

#[test]
fn enter_and_space_each_press_a_focused_action_and_show_the_pressed_look() {
    for key in [Key::Enter, Key::Space] {
        for mode in [Mode::Dark, Mode::Light] {
            let theme = Theme::new(mode, TextScale::default());
            let p = theme.palette;
            let mut harness = group(theme, None, None);
            bring(&mut harness, "Done", State::Focus);
            harness.key_down(key);
            harness.step();
            let rect = harness.get_by_label("Done").rect();
            let look = action_look(&painted(&harness, "Done"), rect);
            assert_eq!((look.fill, look.edge, look.glyph), (Some(p.stage), Some(p.ink), Some(p.ink)), "{mode:?} {key:?}: {look:?}");
            harness.key_up(key);
            harness.step();
        }
    }
    let theme = Theme::new(Mode::Light, TextScale::default());
    for key in [Key::Enter, Key::Space] {
        let mut harness = builder().build_ui_state(
            move |ui, clicked: &mut Vec<usize>| {
                let (_, line) = ui.allocate_space(vec2(197.0, 24.0));
                clicked.extend(actions(ui, &theme, &Keymap::default(), Id::unique("sheet"), line, true, &sheet(None)).clicked);
            },
            Vec::new(),
        );
        harness.run();
        harness.get_by_label("Mark as read").focus();
        harness.run();
        harness.key_press(key);
        harness.run();
        assert_eq!(harness.state(), &[2], "{key:?}");
    }
}

#[test]
fn the_group_is_the_scaled_strip_at_130_percent() {
    let theme = Theme::new(Mode::Light, TextScale::ALL[4]);
    let harness = group(theme, None, None);
    assert_eq!(harness.get_by_label("Done").rect().height(), theme.boxes().strip);
}

/// The group of the sheet's four actions on a line `width` wide; the state keeps what it drew and where it stood.
fn on_a_line(width: f32) -> Harness<'static, (Option<Shown>, Rect, Rect)> {
    let theme = Theme::new(Mode::Light, TextScale::default());
    let mut harness = builder().with_size(vec2(400.0, 100.0)).build_ui_state(
        move |ui, (drawn, line, placed): &mut (Option<Shown>, Rect, Rect)| {
            let (_, at) = ui.allocate_space(vec2(width, 24.0));
            *line = at;
            *placed = super::place(ui, &theme, &Keymap::default(), at, &sheet(None));
            *drawn = Some(actions(ui, &theme, &Keymap::default(), Id::unique("narrow"), at, true, &sheet(None)));
        },
        (None, Rect::NOTHING, Rect::NOTHING),
    );
    harness.run();
    harness
}

#[test]
fn actions_that_do_not_fit_their_line_give_way_from_the_end() {
    for (width, shown) in [(197.0, 4), (196.0, 3), (137.0, 3), (136.0, 2), (43.0, 1), (42.0, 0), (0.0, 0)] {
        let harness = on_a_line(width);
        let (drawn, line, placed) = *harness.state();
        let names: Vec<&str> = SHEET.iter().map(|(name, ..)| *name).filter(|name| harness.query_by_label(name).is_some()).collect();
        let wanted: Vec<&str> = SHEET.iter().take(shown).map(|(name, ..)| *name).collect();
        assert_eq!(names, wanted, "a line {width} wide");
        for name in &names {
            let rect = harness.get_by_label(name).rect();
            assert!(line.contains_rect(rect), "{width}: {name} {rect:?} leaves the line {line:?}");
            assert!(placed.contains_rect(rect), "{width}: {name} {rect:?} outside the group's box {placed:?}");
        }
        let left = drawn.map_or(f32::NAN, |drawn| drawn.left);
        assert!(left >= line.left() - 0.01, "{width}: the group starts at {left}, left of its line");
        if shown == 0 {
            assert_eq!(placed.width(), 0.0, "{width}: nothing to cover");
        }
    }
}

#[test]
fn the_actions_follow_their_row_in_the_tab_order() {
    use egui::{WidgetInfo, accesskit::Role as Kind};
    let theme = Theme::new(Mode::Light, TextScale::default());
    let mut harness = builder().with_size(vec2(520.0, 200.0)).build_ui(move |ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        for index in 0..2 {
            let row = crate::component::row::row(
                ui,
                &theme,
                Id::unique("tab-row").with(index),
                56.0,
                crate::component::row::RowGround::Surface,
                false,
                false,
            );
            let name = format!("Row {index}");
            row.response.widget_info(|| WidgetInfo::labeled(Kind::Button, true, &name));
            let line =
                Rect::from_min_max(egui::pos2(row.rect.center().x, row.rect.top() + 7.0), egui::pos2(row.rect.right() - 16.0, row.rect.top() + 29.0));
            // The first row is the keyed one (the list's selected row while the keyboard works it): its group shows.
            actions(ui, &theme, &Keymap::default(), Id::unique("tab-acts").with(index), line, index == 0, &sheet(Some("Not yet")));
            row.finish(ui, &theme);
        }
    });
    harness.run();
    harness.get_by_label("Row 0").focus();
    harness.run();
    assert!(harness.query_by_label("Done").is_some(), "the focused row shows its group");
    let mut order = Vec::new();
    for _ in 0..5 {
        harness.key_press(Key::Tab);
        harness.run();
        let focused = harness.query_all_by_role(Kind::Button).find(|node| node.accesskit_node().is_focused());
        order.push(focused.and_then(|node| node.accesskit_node().label()).unwrap_or_default());
    }
    // Snooze cannot run, so Tab passes it.
    assert_eq!(order, ["Done", "Mark as read", "Delete", "Row 1", "Row 0"], "{order:?}");
}

#[test]
fn at_130_percent_the_actions_scale_and_stand_on_whole_pixels() {
    let theme = Theme::new(Mode::Light, TextScale::ALL[4]);
    let p = theme.palette;
    // A line with room for the scaled group, its left at a half point so the group's own rounding shows.
    let harness = staged(
        theme,
        |p| p.stage,
        vec2(420.0, 120.0),
        |ui, theme| {
            ui.add_space(0.5);
            let (_, line) = ui.allocate_space(vec2(330.5, theme.boxes().strip));
            actions_with(ui, theme, &Keymap::default(), Id::unique("scaled"), line, true, &sheet(None), Some((0, State::Hover)));
        },
    );
    let mut right: Option<f32> = None;
    for (name, key, _) in SHEET {
        let rect = harness.get_by_label(name).rect();
        let at = format!("{name} {rect:?}");
        assert_eq!(rect.height(), theme.boxes().strip, "{at}");
        for edge in [rect.left(), rect.right(), rect.top(), rect.bottom()] {
            assert_eq!(edge, edge.round(), "every edge on a whole pixel: {at}");
        }
        if let Some(right) = right {
            assert_eq!(rect.left() - right, 4.0, "{at}");
        }
        right = Some(rect.right());
        let look = action_look(&painted(&harness, name), rect);
        let chip = look.chip.clone().unwrap_or_else(|| panic!("{at}: no chip"));
        assert_eq!(chip.0, key, "{at}");
        assert_eq!(chip.3.height(), theme.boxes().chip, "the scaled chip: {at}");
        assert_eq!(chip.3.min.x, rect.left() + 4.0 + theme.mark_side() + 4.0, "{at}");
        assert_eq!((chip.3.right(), chip.3.min.y), (rect.right() - 4.0, chip.3.min.y.round()), "{at}");
        if name == "Done" {
            assert_eq!(look.edge, Some(p.muted), "{at}");
        }
    }
}
