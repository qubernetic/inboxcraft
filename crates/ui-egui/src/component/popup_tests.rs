// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::accesskit::Role;
use egui::{Id, Key, KeyboardShortcut, Modifiers, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::{item, show};
use crate::component::tests::{builder, drew_icon, theme};
use crate::iconset::IconId;
use crate::keymap::Keymap;
use crate::motion;

/// A popup at (40, 30) with two rows; the state keeps the rows chosen and the key of Snooze.
fn harness() -> Harness<'static, (Vec<&'static str>, Keymap)> {
    let theme = theme();
    let mut keymap = Keymap::default();
    keymap.bind("mail.snooze", KeyboardShortcut::new(Modifiers::NONE, Key::S));
    let mut harness = builder().build_ui_state(
        move |ui, (chosen, keymap): &mut (Vec<&'static str>, Keymap)| {
            motion::set_reduced(ui.ctx(), true);
            let picked = show(ui.ctx(), &theme, Id::unique("menu"), pos2(40.0, 30.0), 220.0, |ui| {
                let mut picked = None;
                for (icon, name, action) in [(IconId::Snooze, "Snooze", Some("mail.snooze")), (IconId::Flag, "Flag", None)] {
                    if item(ui, &theme, keymap, icon, name, action).clicked() {
                        picked = Some(name);
                    }
                }
                picked
            });
            chosen.extend(picked);
        },
        (Vec::new(), keymap),
    );
    harness.run();
    harness
}

#[test]
fn a_popup_shows_its_rows_from_its_anchor_with_their_icons() {
    let harness = harness();
    let snooze = harness.get_by_label("Snooze");
    assert_eq!(snooze.accesskit_node().role(), Role::MenuItem);
    let rect = snooze.rect();
    assert!(rect.left() >= 40.0 && rect.top() >= 30.0 && rect.left() < 60.0, "{rect:?}");
    assert!(rect.width() > 180.0, "a row spans the popup: {rect:?}");
    assert!(drew_icon(harness.output(), IconId::Snooze));
    assert!(drew_icon(harness.output(), IconId::Flag));
}

#[test]
fn choosing_a_row_names_it() {
    let mut harness = harness();
    harness.get_by_label("Flag").click();
    harness.run();
    assert_eq!(harness.state().0, vec!["Flag"]);
}

#[test]
fn a_row_shows_the_key_of_its_action_as_it_is_now() {
    let mut harness = harness();
    let texts = |harness: &Harness<'_, (Vec<&'static str>, Keymap)>| {
        let mut found = Vec::new();
        for clipped in &harness.output().shapes {
            collect(&clipped.shape, &mut found);
        }
        found
    };
    assert!(texts(&harness).contains(&"S".to_owned()));
    harness.state_mut().1.bind("mail.snooze", KeyboardShortcut::new(Modifiers::NONE, Key::Z));
    harness.step();
    let now = texts(&harness);
    assert!(now.contains(&"Z".to_owned()) && !now.contains(&"S".to_owned()), "{now:?}");
}

fn collect(shape: &egui::Shape, found: &mut Vec<String>) {
    match shape {
        egui::Shape::Text(text) => found.push(text.galley.job.text.clone()),
        egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| collect(shape, found)),
        _ => {}
    }
}

#[test]
fn the_rows_stand_one_under_the_other_even_inside_a_row() {
    let theme = theme();
    let harness = builder().build_ui(move |ui| {
        ui.horizontal(|ui| {
            super::panel(ui, &theme, 200.0, |ui| {
                item(ui, &theme, &Keymap::default(), IconId::Snooze, "Snooze", None);
                item(ui, &theme, &Keymap::default(), IconId::Flag, "Flag", None);
            });
        });
    });
    let (snooze, flag) = (harness.get_by_label("Snooze").rect(), harness.get_by_label("Flag").rect());
    assert!(flag.top() >= snooze.bottom() - 0.5 && (flag.left() - snooze.left()).abs() < 0.5, "{snooze:?} {flag:?}");
    // The rows stand 1 px in from the float's edge, edge to edge.
    assert!((snooze.width() - 198.0).abs() < 0.01, "{snooze:?}");
}

#[test]
fn choice_rows_mark_the_chosen_one_and_entries_line_up_with_items() {
    let theme = theme();
    let harness = builder().build_ui(move |ui| {
        super::panel(ui, &theme, 220.0, |ui| {
            item(ui, &theme, &Keymap::default(), IconId::Snooze, "Snooze", None);
            super::entry(ui, &theme, &Keymap::default(), "Full width", None);
            super::choice(ui, &theme, "Show the next message", true);
            super::choice(ui, &theme, "Go back to the list", false);
        });
    });
    let chosen = harness.get_by_label("Show the next message");
    assert_eq!(chosen.accesskit_node().role(), Role::MenuItemRadio);
    assert_eq!(chosen.accesskit_node().toggled(), Some(egui::accesskit::Toggled::True));
    assert_eq!(harness.get_by_label("Go back to the list").accesskit_node().toggled(), Some(egui::accesskit::Toggled::False));
    assert_eq!(harness.get_by_label("Full width").accesskit_node().role(), Role::MenuItem);
    assert!(
        crate::component::field::tests::drawn(&harness).iter().any(|text| text.text == super::CHECK.to_string()),
        "the chosen one is ticked"
    );
}

#[test]
fn a_hovered_row_of_a_float_is_the_stage_and_shows_in_dark_mode() {
    let theme = crate::theme::Theme::new(crate::theme::Mode::Dark, crate::theme::TextScale::default());
    let p = theme.palette;
    let keymap = Keymap::default();
    let mut harness = builder().with_theme(egui::Theme::Dark).build_ui(move |ui| {
        motion::set_reduced(ui.ctx(), true);
        show(ui.ctx(), &theme, Id::unique("menu"), pos2(40.0, 30.0), 220.0, |ui| {
            item(ui, &theme, &keymap, IconId::Snooze, "Snooze", None);
            item(ui, &theme, &keymap, IconId::Flag, "Flag", None);
        });
    });
    harness.run();
    let row = harness.get_by_label("Snooze").rect().center();
    harness.hover_at(row);
    harness.run();
    let fills: Vec<egui::Color32> = harness
        .output()
        .shapes
        .iter()
        .flat_map(|clipped| match &clipped.shape {
            egui::Shape::Vec(inner) => inner.clone(),
            other => vec![other.clone()],
        })
        .filter_map(|shape| match shape {
            egui::Shape::Rect(rect) if rect.rect.contains(row) && rect.blur_width == 0.0 => Some(rect.fill),
            _ => None,
        })
        .collect();
    assert!(fills.contains(&p.surface), "the float's ground: {fills:?}");
    assert!(fills.contains(&p.stage), "the hovered row is the stage: {fills:?}");
    assert!(!fills.contains(&p.grid), "{fills:?}");
    assert!(crate::theme::contrast(p.stage, p.surface) >= 1.15);
}

/// Every rect the last frame painted: `(rect, fill, stroke)`, and whether a mesh (the float's shadow) was painted.
fn rects<State>(harness: &Harness<'_, State>) -> (Vec<(egui::Rect, egui::Color32, Option<(f32, egui::Color32)>)>, bool) {
    fn walk(shape: &egui::Shape, found: &mut Vec<(egui::Rect, egui::Color32, Option<(f32, egui::Color32)>)>, mesh: &mut bool) {
        match shape {
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| walk(shape, found, mesh)),
            egui::Shape::Rect(rect) if rect.blur_width == 0.0 => {
                found.push((rect.rect, rect.fill, (rect.stroke.width > 0.0).then_some((rect.stroke.width, rect.stroke.color))));
            }
            egui::Shape::Mesh(_) => *mesh = true,
            _ => {}
        }
    }
    let (mut found, mut mesh) = (Vec::new(), false);
    harness.output().shapes.iter().for_each(|clipped| walk(&clipped.shape, &mut found, &mut mesh));
    (found, mesh)
}

/// The states of the popup panel's sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Panel {
    Default,
    Scrolling,
    Empty,
}

const MAILBOXES: [&str; 6] = ["Archive", "Clients", "Clients / Bakery", "Invoices 2026", "Newsletters", "Receipts"];

/// A float of the sheet in `state`, 240 wide, opened from a control at (20, 20) 100 × 32.
fn float_in(mode: crate::theme::Mode, state: Panel) -> Harness<'static> {
    let theme = crate::theme::Theme::new(mode, crate::theme::TextScale::default());
    let mut keymap = Keymap::default();
    keymap.bind("mail.archive", KeyboardShortcut::new(Modifiers::NONE, Key::A));
    let mut harness = builder().with_size(egui::vec2(600.0, 500.0)).build_ui(move |ui| {
        motion::set_reduced(ui.ctx(), true);
        ui.painter().rect_filled(ui.max_rect(), 0.0, theme.palette.bg);
        let anchor = egui::Rect::from_min_size(pos2(20.0, 20.0), egui::vec2(100.0, 32.0));
        let float = super::Float { id: Id::unique("float"), anchor, width: 240.0, returns_to: Id::unique("opener") };
        super::float(ui.ctx(), &theme, &float, |ui| match state {
            Panel::Default => {
                let row = |icon, name, action, state| super::Item { icon: Some(icon), name, caption: None, action, state };
                super::menu_item(ui, &theme, &keymap, &row(IconId::Mail, "Mark as read", None, super::ItemState::Default));
                super::menu_item(ui, &theme, &keymap, &row(IconId::Archive, "Archive", Some("mail.archive"), super::ItemState::Default));
                super::menu_item(ui, &theme, &keymap, &row(IconId::Snooze, "Snooze…", None, super::ItemState::Default));
                super::menu_item(ui, &theme, &keymap, &row(IconId::Move, "Move to…", None, super::ItemState::Default));
                super::hairline(ui, &theme);
                super::menu_item(ui, &theme, &keymap, &row(IconId::Delete, "Delete", None, super::ItemState::Danger));
            }
            Panel::Scrolling => {
                super::title(ui, &theme, "Move to", Some("40"));
                super::scrolled(ui, &theme, Id::unique("rows"), 147.0, |ui| {
                    for name in MAILBOXES.iter().chain(MAILBOXES.iter()) {
                        ui.push_id(ui.next_auto_id(), |ui| {
                            super::menu_item(ui, &theme, &keymap, &super::Item { icon: None, name, caption: None, action: None, state: super::ItemState::Default })
                        });
                    }
                });
            }
            Panel::Empty => super::empty(ui, &theme, "No other mailbox to move this to."),
        });
    });
    harness.run();
    harness
}

#[test]
fn a_float_looks_as_its_sheet_says_in_every_state() {
    for mode in [crate::theme::Mode::Dark, crate::theme::Mode::Light] {
        let p = crate::theme::Theme::new(mode, crate::theme::TextScale::default()).palette;
        for state in [Panel::Default, Panel::Scrolling, Panel::Empty] {
            let harness = float_in(mode, state);
            let at = format!("{mode:?} {state:?}");
            let (found, shadow) = rects(&harness);
            let ground = found.iter().find(|(_, fill, _)| *fill == p.surface).map(|(rect, ..)| *rect);
            let height = match state {
                Panel::Default => 163.0,
                Panel::Scrolling => 178.0,
                Panel::Empty => 48.0,
            };
            // It hangs 4 under the control, its left edge on the control's, 240 wide, its edge `cyan_edge` inside.
            let float = egui::Rect::from_min_size(pos2(20.0, 56.0), egui::vec2(240.0, height));
            assert_eq!(ground, Some(float), "{at} {found:?}");
            assert!(found.iter().any(|(rect, _, stroke)| *rect == float && *stroke == Some((1.0, p.cyan_edge))), "{at} {found:?}");
            assert!(shadow, "{at}: the shadow");
            match state {
                Panel::Default => {
                    for (index, name) in ["Mark as read", "Archive", "Snooze…", "Move to…"].into_iter().enumerate() {
                        let row = harness.get_by_label(name).rect();
                        let top = 57.0 + 32.0 * index as f32;
                        assert_eq!(row, egui::Rect::from_min_size(pos2(21.0, top), egui::vec2(238.0, 32.0)), "{at} {name}");
                    }
                    let line = egui::Rect::from_min_size(pos2(21.0, 185.0), egui::vec2(238.0, 1.0));
                    assert!(found.iter().any(|(rect, fill, _)| *rect == line && *fill == p.line), "{at}: the hairline");
                    assert_eq!(harness.get_by_label("Delete").rect().top(), 186.0, "{at}");
                }
                Panel::Scrolling => {
                    let title = egui::Rect::from_min_size(pos2(21.0, 57.0), egui::vec2(238.0, 28.0));
                    assert!(found.iter().any(|(rect, fill, _)| *rect == title && *fill == p.panel), "{at}: the title row");
                    let label = crate::component::field::tests::one(&harness, "MOVE TO");
                    assert_eq!((label.rect.min, label.colour), (pos2(33.0, 63.0), p.muted), "{at}");
                    assert!(found.iter().any(|(rect, fill, _)| rect.left() == 255.0 && rect.width() == 2.0 && rect.height() == 40.0 && *fill == p.muted), "{at}: the thumb");
                    assert_eq!(harness.get_all_by_label("Archive").next().map(|row| row.rect().top()), Some(86.0), "{at}");
                }
                Panel::Empty => {
                    let text = crate::component::field::tests::one(&harness, "No other mailbox to move this to.");
                    assert_eq!((text.rect.min, text.colour), (pos2(33.0, 69.0), p.muted), "{at}");
                }
            }
        }
    }
}

/// A button "Open" that opens a float of three rows under it; the state is whether it is open.
fn opener() -> Harness<'static, bool> {
    let theme = theme();
    let keymap = Keymap::default();
    let mut harness = builder().with_size(egui::vec2(600.0, 400.0)).build_ui_state(
        move |ui, open: &mut bool| {
            motion::set_reduced(ui.ctx(), true);
            let button = crate::component::button::TextButton::new("Open").show(ui, &theme, &keymap);
            if button.clicked() {
                *open = true;
            }
            if *open {
                let float = super::Float { id: Id::unique("menu"), anchor: button.rect, width: 240.0, returns_to: button.id };
                let shown = super::float(ui.ctx(), &theme, &float, |ui| {
                    for name in ["First", "Second", "Third"] {
                        super::menu_item(ui, &theme, &keymap, &super::Item { icon: None, name, caption: None, action: None, state: super::ItemState::Default });
                    }
                });
                *open = shown.is_some();
            }
        },
        false,
    );
    harness.run();
    harness
}

#[test]
fn tab_stays_inside_a_float_and_esc_gives_the_focus_back() {
    let mut harness = opener();
    harness.get_by_label("Open").focus();
    harness.run();
    harness.key_press(Key::Enter);
    harness.run();
    assert!(*harness.state(), "Enter opens it");
    for _ in 0..4 {
        harness.key_press(Key::Tab);
        harness.run();
    }
    assert!(harness.get_by_label("First").is_focused(), "four Tabs go round to the first row");
    harness.key_press_modifiers(Modifiers::SHIFT, Key::Tab);
    harness.run();
    assert!(harness.get_by_label("Third").is_focused(), "Shift Tab from the first goes to the last");
    harness.key_press(Key::Escape);
    harness.run();
    assert!(!*harness.state(), "Esc closes it");
    assert!(harness.query_by_label("First").is_none());
    assert!(harness.get_by_label("Open").is_focused(), "the button has the keyboard back");
}

#[test]
fn a_press_outside_a_float_closes_it_and_gives_the_focus_back() {
    let mut harness = opener();
    harness.get_by_label("Open").click();
    harness.run();
    assert!(*harness.state());
    harness.hover_at(pos2(580.0, 380.0));
    harness.run();
    harness.drag_at(pos2(580.0, 380.0));
    harness.run();
    assert!(!*harness.state(), "a press beside it closes it");
    assert!(harness.get_by_label("Open").is_focused());
}

#[test]
fn a_float_at_the_window_s_edge_keeps_three_pixels() {
    let theme = crate::theme::Theme::new(crate::theme::Mode::Dark, crate::theme::TextScale::default());
    let (width, height) = (600.0, 400.0);
    for anchor in [pos2(width - 10.0, 20.0), pos2(20.0, height - 10.0), pos2(-50.0, -50.0)] {
        let keymap = Keymap::default();
        let mut harness = builder().with_size(egui::vec2(width, height)).build_ui(move |ui| {
            motion::set_reduced(ui.ctx(), true);
            let float = super::Float { id: Id::unique("edge"), anchor: egui::Rect::from_min_size(anchor, egui::vec2(8.0, 8.0)), width: 240.0, returns_to: Id::unique("x") };
            super::float(ui.ctx(), &theme, &float, |ui| {
                for name in ["First", "Second", "Third"] {
                    super::menu_item(ui, &theme, &keymap, &super::Item { icon: None, name, caption: None, action: None, state: super::ItemState::Default });
                }
            });
        });
        harness.run();
        let (found, _) = rects(&harness);
        let float = found.iter().find(|(_, fill, _)| *fill == theme.palette.surface).map(|(rect, ..)| *rect).unwrap_or(egui::Rect::NOTHING);
        assert!(float.left() >= 3.0 && float.top() >= 3.0 && float.right() <= width - 3.0 && float.bottom() <= height - 3.0, "{anchor:?}: {float:?}");
        assert_eq!(float.size(), egui::vec2(240.0, 98.0), "{anchor:?}: it moves, it never shrinks");
        assert!(found.iter().any(|(rect, _, stroke)| *rect == float && *stroke == Some((1.0, theme.palette.cyan_edge))), "{anchor:?}: all four edges");
    }
}

#[test]
fn a_float_s_rows_come_in_one_after_the_other_at_full_motion_and_at_once_reduced() {
    for reduced in [false, true] {
        let theme = crate::theme::Theme::new(crate::theme::Mode::Dark, crate::theme::TextScale::default());
        let keymap = Keymap::default();
        let mut harness = builder().with_size(egui::vec2(600.0, 400.0)).build_ui_state(move |ui, open: &mut bool| {
            motion::set_reduced(ui.ctx(), reduced);
            if !*open {
                return;
            }
            let float = super::Float { id: Id::unique("in"), anchor: egui::Rect::from_min_size(pos2(20.0, 20.0), egui::vec2(8.0, 8.0)), width: 240.0, returns_to: Id::unique("x") };
            super::float(ui.ctx(), &theme, &float, |ui| {
                for name in ["First", "Second", "Third"] {
                    super::menu_item(ui, &theme, &keymap, &super::Item { icon: None, name, caption: None, action: None, state: super::ItemState::Default });
                }
            });
        }, false);
        harness.run();
        *harness.state_mut() = true;
        harness.step();
        harness.step();
        let colour = |harness: &Harness<'static, bool>, name: &str| crate::component::field::tests::one(harness, name).colour;
        let ink = theme.palette.ink;
        if reduced {
            assert!(["First", "Second", "Third"].iter().all(|name| colour(&harness, name) == ink), "all at once");
        } else {
            assert_eq!(colour(&harness, "Third"), theme.palette.surface, "the third row has not started");
            for _ in 0..4 {
                harness.step();
            }
            let first = colour(&harness, "First");
            assert!(crate::component::tests::strictly_between(first, theme.palette.surface, ink), "the first row on its way, a mix of colours: {first:?}");
        }
        assert!(crate::component::tests::rests(&mut harness, 30), "reduced {reduced}: it lands and rests");
        assert!(["First", "Second", "Third"].iter().all(|name| colour(&harness, name) == ink));
    }
}

#[test]
fn home_and_end_go_to_the_first_and_the_last_row_of_a_float() {
    let mut harness = opener();
    harness.get_by_label("Open").click();
    harness.run();
    harness.key_press(Key::Tab);
    harness.run();
    assert!(harness.get_by_label("First").is_focused());
    harness.key_press(Key::End);
    harness.run();
    assert!(harness.get_by_label("Third").is_focused(), "End");
    harness.key_press(Key::Home);
    harness.run();
    assert!(harness.get_by_label("First").is_focused(), "Home");
}
