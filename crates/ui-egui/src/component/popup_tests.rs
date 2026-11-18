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
    assert!(crate::component::field::tests::drawn(&harness).iter().any(|text| text.text == super::CHECK.to_string()), "the chosen one is ticked");
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
/// A painted rect: where, its fill and its stroke.
type Drawn = (egui::Rect, egui::Color32, Option<(f32, egui::Color32)>);

fn rects<State>(harness: &Harness<'_, State>) -> (Vec<Drawn>, bool) {
    fn walk(shape: &egui::Shape, found: &mut Vec<Drawn>, mesh: &mut bool) {
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
                            super::menu_item(
                                ui,
                                &theme,
                                &keymap,
                                &super::Item { icon: None, name, caption: None, action: None, state: super::ItemState::Default },
                            )
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
                    assert!(
                        found
                            .iter()
                            .any(|(rect, fill, _)| *rect == egui::Rect::from_min_size(pos2(255.0, 92.0), egui::vec2(2.0, 40.0)) && *fill == p.muted),
                        "{at}: the thumb, 6 in from the list's top as the board has it"
                    );
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
                        super::menu_item(
                            ui,
                            &theme,
                            &keymap,
                            &super::Item { icon: None, name, caption: None, action: None, state: super::ItemState::Default },
                        );
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
            let float = super::Float {
                id: Id::unique("edge"),
                anchor: egui::Rect::from_min_size(anchor, egui::vec2(8.0, 8.0)),
                width: 240.0,
                returns_to: Id::unique("x"),
            };
            super::float(ui.ctx(), &theme, &float, |ui| {
                for name in ["First", "Second", "Third"] {
                    super::menu_item(
                        ui,
                        &theme,
                        &keymap,
                        &super::Item { icon: None, name, caption: None, action: None, state: super::ItemState::Default },
                    );
                }
            });
        });
        harness.run();
        let (found, _) = rects(&harness);
        let float = found.iter().find(|(_, fill, _)| *fill == theme.palette.surface).map(|(rect, ..)| *rect).unwrap_or(egui::Rect::NOTHING);
        assert!(float.left() >= 3.0 && float.top() >= 3.0 && float.right() <= width - 3.0 && float.bottom() <= height - 3.0, "{anchor:?}: {float:?}");
        assert_eq!(float.size(), egui::vec2(240.0, 98.0), "{anchor:?}: it moves, it never shrinks");
        assert!(
            found.iter().any(|(rect, _, stroke)| *rect == float && *stroke == Some((1.0, theme.palette.cyan_edge))),
            "{anchor:?}: all four edges"
        );
    }
}

#[test]
fn a_float_s_rows_come_in_one_after_the_other_at_full_motion_and_at_once_reduced() {
    for reduced in [false, true] {
        let theme = crate::theme::Theme::new(crate::theme::Mode::Dark, crate::theme::TextScale::default());
        let keymap = Keymap::default();
        let mut harness = builder().with_size(egui::vec2(600.0, 400.0)).build_ui_state(
            move |ui, open: &mut bool| {
                motion::set_reduced(ui.ctx(), reduced);
                if !*open {
                    return;
                }
                let float = super::Float {
                    id: Id::unique("in"),
                    anchor: egui::Rect::from_min_size(pos2(20.0, 20.0), egui::vec2(8.0, 8.0)),
                    width: 240.0,
                    returns_to: Id::unique("x"),
                };
                super::float(ui.ctx(), &theme, &float, |ui| {
                    for name in ["First", "Second", "Third"] {
                        super::menu_item(
                            ui,
                            &theme,
                            &keymap,
                            &super::Item { icon: None, name, caption: None, action: None, state: super::ItemState::Default },
                        );
                    }
                });
            },
            false,
        );
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
            assert!(
                crate::component::tests::strictly_between(first, theme.palette.surface, ink),
                "the first row on its way, a mix of colours: {first:?}"
            );
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

/// The states of the menu item's sheet, in its order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Menu {
    Default,
    Hover,
    Focus,
    Chosen,
    Unavailable,
    WithKey,
    WithCaption,
    Danger,
    Long,
}

const MENU: [Menu; 9] =
    [Menu::Default, Menu::Hover, Menu::Focus, Menu::Chosen, Menu::Unavailable, Menu::WithKey, Menu::WithCaption, Menu::Danger, Menu::Long];

/// The row of the sheet in `state`: its icon, name, caption, action and state.
fn menu_row(state: Menu) -> (IconId, &'static str, Option<&'static str>, Option<&'static str>, super::ItemState) {
    use super::ItemState as S;
    match state {
        Menu::Unavailable => (IconId::Move, "Move to…", Some("no other mailbox"), None, S::Unavailable),
        Menu::WithKey => (IconId::Snooze, "Snooze…", None, Some("mail.snooze"), S::Default),
        Menu::WithCaption => (IconId::Snooze, "Tomorrow morning", Some("Tue 08:00"), None, S::Default),
        Menu::Danger => (IconId::Delete, "Delete", None, Some("mail.delete"), S::Danger),
        Menu::Long => (IconId::Move, "Move to Studio / Clients / Bakery on the corner / 2026", None, Some("mail.move"), S::Default),
        Menu::Chosen => (IconId::Archive, "Archive", None, None, S::Chosen),
        _ => (IconId::Archive, "Archive", None, None, S::Default),
    }
}

/// The keys the sheet's rows show: H for snooze and move, Del for delete.
fn menu_keys() -> Keymap {
    let mut keymap = Keymap::default();
    keymap.bind("mail.snooze", KeyboardShortcut::new(Modifiers::NONE, Key::H));
    keymap.bind("mail.move", KeyboardShortcut::new(Modifiers::NONE, Key::V));
    keymap.bind("mail.delete", KeyboardShortcut::new(Modifiers::NONE, Key::Delete));
    keymap
}

/// The menu item of the sheet, 240 wide on a `surface` stage, in `state` brought as a person brings it.
fn menu_in(mode: crate::theme::Mode, state: Menu, shown: crate::component::Shown) -> (egui::Rect, crate::component::tests::Own, Harness<'static>) {
    let theme = crate::theme::Theme::new(mode, crate::theme::TextScale::default());
    let (icon, name, caption, action, row) = menu_row(state);
    let mut harness = crate::component::tests::staged(
        theme,
        |p| p.surface,
        egui::vec2(400.0, 120.0),
        move |ui, theme| {
            let keymap = menu_keys();
            ui.vertical(|ui| {
                ui.set_width(240.0);
                super::menu_item(ui, theme, &keymap, &super::Item { icon: Some(icon), name, caption, action, state: row });
            });
        },
    );
    crate::component::tests::bring(&mut harness, name, shown);
    let rect = harness.get_by_label(name).rect();
    let look = crate::component::tests::own(&crate::component::tests::painted(&harness, name), rect);
    (rect, look, harness)
}

#[test]
fn a_menu_item_looks_as_its_sheet_says_in_every_state() {
    use crate::component::Shown;
    use crate::component::field::tests::{drawn, one};
    for mode in [crate::theme::Mode::Dark, crate::theme::Mode::Light] {
        let p = crate::theme::Theme::new(mode, crate::theme::TextScale::default()).palette;
        for state in MENU {
            let shown = match state {
                Menu::Hover => Shown::Hover,
                Menu::Focus => Shown::Focus,
                _ => Shown::Rest,
            };
            let (rect, look, harness) = menu_in(mode, state, shown);
            let at = format!("{mode:?} {state:?}: {look:#?}");
            assert_eq!(rect.size(), egui::vec2(240.0, 32.0), "{at}");
            let fill = matches!(state, Menu::Hover | Menu::Chosen).then_some(p.stage);
            assert_eq!(look.fill, fill, "{at}");
            assert_eq!(look.edge, (state == Menu::Focus).then_some((1.0, p.cyan_edge, false)), "{at}");
            assert!(look.ring.is_none() && look.radius <= 2.0, "{at}");
            let (icon, name, caption, _, _) = menu_row(state);
            let shift = if state == Menu::Focus { 1.0 } else { 0.0 };
            let (icon_ink, ink) = match state {
                Menu::Hover | Menu::Chosen => (p.ink, p.ink),
                Menu::Unavailable => (p.dim, p.muted),
                Menu::Danger => (p.minus, p.minus),
                _ => (p.muted, p.ink),
            };
            let glyph = one(&harness, &crate::iconset::glyph(icon).to_string());
            assert!((glyph.rect.center() - (rect.min + egui::vec2(20.0 + shift, 16.0))).length() < 1.0 && glyph.colour == icon_ink, "{at} {glyph:?}");
            let shown_name = drawn(&harness).into_iter().find(|text| text.family == "Proportional").unwrap_or_else(|| panic!("{at}: no name"));
            assert_eq!(shown_name.rect.min, rect.min + egui::vec2(36.0 + shift, 5.0), "{at} {shown_name:?}");
            assert_eq!((shown_name.colour, shown_name.size, shown_name.row), (ink, 14.0, 22.0), "{at}");
            if state == Menu::Long {
                // Cut with an ellipsis 8 before the chip: 167 wide at most.
                assert!(shown_name.rect.right() <= rect.left() + 203.0 + 0.5, "{at} {shown_name:?}");
            }
            assert_eq!(shown_name.text, name, "{at}");
            // The key chip at the right end, 6 down, 20 high: H, Del, V, or Enter on the chosen row.
            let key = match state {
                Menu::WithKey => Some(("H", 17.0)),
                Menu::Danger => Some(("Del", 30.0)),
                Menu::Long => Some(("V", 17.0)),
                Menu::Chosen => Some(("Enter", 43.0)),
                _ => None,
            };
            let (found, _) = rects(&harness);
            let chip = found.iter().find(|(r, _, stroke)| *stroke == Some((1.0, p.line)) && (r.height() - 20.0).abs() < 0.01).map(|(r, ..)| *r);
            match key {
                Some((text, width)) => {
                    assert_eq!(chip, Some(egui::Rect::from_min_size(rect.min + egui::vec2(228.0 - width, 6.0), egui::vec2(width, 20.0))), "{at}");
                    assert_eq!(one(&harness, text).colour, p.muted, "{at}");
                }
                None => assert_eq!(chip, None, "{at}"),
            }
            // The caption in `mono_s`, `muted`, 8 down, in a box at 228's left as wide as the run rounded up, the run at
            // the box's left: `Tue 08:00` is 9 × 6.6 = 59.4 wide, so its box 60 and its right end at 227.4.
            if let Some(caption) = caption {
                let text = one(&harness, caption);
                let wide = caption.chars().count() as f32 * 6.6;
                let end = 228.0 - wide.ceil() + wide;
                assert!((text.rect.right() - (rect.left() + end)).abs() < 0.05 && text.rect.top() == rect.top() + 8.0, "{at} {text:?}");
                assert_eq!((text.colour, text.family.as_str(), text.size), (p.muted, "Monospace", 11.0), "{at}");
            }
            let bar = found.iter().find(|(r, fill, _)| *fill == p.cyan_edge && r.width() == 2.0).map(|(r, ..)| *r);
            assert_eq!(bar, (state == Menu::Chosen).then_some(egui::Rect::from_min_size(rect.min, egui::vec2(2.0, 32.0))), "{at}");
        }
    }
}

/// `label` drawn by `draw` on a `surface` stage held down by the pointer, and by `key` while it has the focus: the
/// looks of both and of the focused row at rest.
fn held(mode: crate::theme::Mode, label: &'static str, draw: fn(&mut egui::Ui, &crate::theme::Theme)) {
    use crate::component::press_tests::held_by;
    let whole: crate::component::press_tests::Part = |rect, _| rect;
    let size = egui::vec2(400.0, 120.0);
    let (_, pointer) = held_by(mode, label, None, size, whole, draw);
    let (_, rest) = held_by(mode, label, Some(Key::A), size, whole, draw);
    for key in [Key::Enter, Key::Space] {
        let (_, look) = held_by(mode, label, Some(key), size, whole, draw);
        let what = format!("{mode:?} {label} {key:?}: {look:#?} pointer {pointer:#?} rest {rest:#?}");
        assert_eq!((look.fill, look.edge, &look.texts), (pointer.fill, pointer.edge, &pointer.texts), "{what}");
        assert_ne!((look.fill, look.edge), (rest.fill, rest.edge), "pressed differs from rest: {what}");
    }
}

#[test]
fn enter_and_space_press_a_menu_item_as_the_pointer_does() {
    for mode in [crate::theme::Mode::Dark, crate::theme::Mode::Light] {
        let p = crate::theme::Theme::new(mode, crate::theme::TextScale::default()).palette;
        held(mode, "Archive", |ui, theme| {
            ui.set_width(240.0);
            super::menu_item(
                ui,
                theme,
                &Keymap::default(),
                &super::Item { icon: Some(IconId::Archive), name: "Archive", caption: None, action: None, state: super::ItemState::Default },
            );
        });
        // Pressed, an unchosen row is the secondary press; the chosen row the accent wash.
        let (_, look, _) = menu_in(mode, Menu::Default, crate::component::Shown::Pressed);
        assert_eq!((look.fill, look.edge), (Some(p.stage), Some((1.0, p.ink, false))), "{mode:?}");
        let (_, look, _) = menu_in(mode, Menu::Chosen, crate::component::Shown::Pressed);
        assert_eq!((look.fill, look.edge), (Some(p.cyan_muted), Some((1.0, p.cyan_edge, false))), "{mode:?}");
    }
}

#[test]
fn an_unavailable_menu_item_takes_no_press() {
    let (_, _, mut harness) = menu_in(crate::theme::Mode::Light, Menu::Unavailable, crate::component::Shown::Rest);
    assert!(harness.get_by_label("Move to…").accesskit_node().is_disabled());
    harness.get_by_label("Move to…").hover();
    harness.run();
    assert_eq!(
        crate::component::tests::own(&crate::component::tests::painted(&harness, "Move to…"), harness.get_by_label("Move to…").rect()).fill,
        None
    );
}

#[test]
fn a_menu_choice_looks_as_its_sheet_says_in_every_state() {
    use crate::component::Shown;
    use crate::component::field::tests::{drawn, one};
    for mode in [crate::theme::Mode::Dark, crate::theme::Mode::Light] {
        let p = crate::theme::Theme::new(mode, crate::theme::TextScale::default()).palette;
        for state in ["off", "on", "hover", "unavailable"] {
            let theme = crate::theme::Theme::new(mode, crate::theme::TextScale::default());
            let name = if state == "unavailable" { "Every month" } else { "Every week" };
            let mut harness = crate::component::tests::staged(
                theme,
                |p| p.surface,
                egui::vec2(400.0, 120.0),
                move |ui, theme| {
                    ui.vertical(|ui| {
                        ui.set_width(240.0);
                        let reason = (state == "unavailable").then_some("not synced");
                        super::menu_choice_with(ui, theme, name, state == "on", reason, None);
                    });
                },
            );
            if state == "hover" {
                crate::component::tests::bring(&mut harness, name, Shown::Hover);
            }
            let node = harness.get_by_label(name);
            let rect = node.rect();
            assert_eq!(node.accesskit_node().role(), Role::MenuItemRadio);
            assert_eq!(node.accesskit_node().is_disabled(), state == "unavailable");
            let look = crate::component::tests::own(&crate::component::tests::painted(&harness, name), rect);
            let at = format!("{mode:?} {state}: {look:#?}");
            assert_eq!(rect.size(), egui::vec2(240.0, 32.0), "{at}");
            assert_eq!(look.fill, (state == "hover").then_some(p.stage), "{at}");
            assert_eq!(look.edge, None, "{at}");
            let tick: Vec<_> = drawn(&harness).into_iter().filter(|text| text.text == super::CHECK.to_string()).collect();
            if state == "on" {
                assert!(
                    tick.len() == 1 && (tick[0].rect.center() - (rect.min + egui::vec2(20.0, 16.0))).length() < 1.0 && tick[0].colour == p.cyan_text,
                    "{at} {tick:?}"
                );
            } else {
                assert!(tick.is_empty(), "{at}");
            }
            let label = one(&harness, name);
            let ink = if state == "unavailable" { p.muted } else { p.ink };
            assert_eq!((label.rect.min, label.colour, label.size, label.row), (rect.min + egui::vec2(36.0, 5.0), ink, 14.0, 22.0), "{at}");
            if state == "unavailable" {
                let reason = one(&harness, "not synced");
                assert!((reason.rect.right() - (rect.left() + 228.0)).abs() < 0.5 && reason.rect.top() == rect.top() + 8.0, "{at} {reason:?}");
                assert_eq!((reason.colour, reason.family.as_str(), reason.size), (p.muted, "Monospace", 11.0), "{at}");
            }
        }
    }
}

#[test]
fn enter_and_space_press_a_menu_choice_as_the_pointer_does() {
    for mode in [crate::theme::Mode::Dark, crate::theme::Mode::Light] {
        held(mode, "Every week", |ui, theme| {
            ui.set_width(240.0);
            super::menu_choice(ui, theme, "Every week", false, true);
        });
        held(mode, "Every month", |ui, theme| {
            ui.set_width(240.0);
            super::menu_choice(ui, theme, "Every month", true, true);
        });
    }
}

/// The states of the suggestion row's sheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Suggest {
    Default,
    Hover,
    Focus,
    Selected,
    Long,
}

#[test]
fn a_suggestion_row_looks_as_its_sheet_says_in_every_state() {
    use crate::component::Shown;
    use crate::component::field::tests::{drawn, one};
    for mode in [crate::theme::Mode::Dark, crate::theme::Mode::Light] {
        let p = crate::theme::Theme::new(mode, crate::theme::TextScale::default()).palette;
        for state in [Suggest::Default, Suggest::Hover, Suggest::Focus, Suggest::Selected, Suggest::Long] {
            let theme = crate::theme::Theme::new(mode, crate::theme::TextScale::default());
            let (name, address) = if state == Suggest::Long {
                ("Bakery on the corner, orders and deliveries", "orders-and-deliveries@bakery-on-the-corner.example.org")
            } else {
                ("Teodor Vasquez", "teodor@example.org")
            };
            let mut harness = crate::component::tests::staged(
                theme,
                |p| p.surface,
                egui::vec2(420.0, 120.0),
                move |ui, theme| {
                    ui.vertical(|ui| {
                        ui.set_width(320.0);
                        super::suggestion_row(ui, theme, name, address, state == Suggest::Selected);
                    });
                },
            );
            let spoken = format!("{name}, {address}");
            match state {
                Suggest::Hover => crate::component::tests::bring(&mut harness, &spoken, Shown::Hover),
                Suggest::Focus => crate::component::tests::bring(&mut harness, &spoken, Shown::Focus),
                _ => {}
            }
            let rect = harness.get_by_label(&spoken).rect();
            let paint = crate::component::tests::painted(&harness, &spoken);
            let look = crate::component::tests::own(&paint, rect);
            let at = format!("{mode:?} {state:?}: {look:#?}");
            assert_eq!(rect.size(), egui::vec2(320.0, 40.0), "{at}");
            let lit = matches!(state, Suggest::Hover | Suggest::Selected);
            assert_eq!(look.fill, lit.then_some(p.stage), "{at}");
            assert_eq!(look.edge, (state == Suggest::Focus).then_some((1.0, p.cyan_edge, false)), "{at}");
            let shift = if state == Suggest::Focus { 1.0 } else { 0.0 };
            // The avatar: 24, corners of 2, on `stage`, on `surface` when the row is lit.
            let face = egui::Rect::from_min_size(rect.min + egui::vec2(12.0 + shift, 8.0), egui::vec2(24.0, 24.0));
            let ground = if lit { p.surface } else { p.stage };
            assert!(
                paint.rects().any(|shape| shape.rect == face
                    && matches!(shape.look, crate::component::tests::Look::Rect { fill, radius, .. } if fill == ground && radius == 2.0)),
                "{at}: the avatar"
            );
            let shown = one(&harness, name);
            assert_eq!((shown.rect.min, shown.colour, shown.size), (rect.min + egui::vec2(44.0 + shift, 9.0), p.ink, 14.0), "{at}");
            let mail = drawn(&harness).into_iter().find(|text| text.text == address).unwrap_or_else(|| panic!("{at}: no address"));
            // 8 after the name's box: the name's width rounded up, or all of its 144 when it is cut (the board's
            // address starts at 196).
            let name_box = if state == Suggest::Long { 144.0 } else { shown.rect.width().ceil() };
            assert!(
                (mail.rect.left() - (shown.rect.left() + name_box + 8.0)).abs() < 0.01 && mail.rect.top() == rect.top() + 12.0,
                "{at} {mail:?} after {shown:?}"
            );
            assert_eq!((mail.colour, mail.family.as_str(), mail.size), (p.muted, "Monospace", 11.0), "{at}");
            let right = if state == Suggest::Selected { 257.0 } else { 308.0 - shift };
            assert!(mail.rect.right() <= rect.left() + right + 0.5, "{at} {mail:?}");
            if state == Suggest::Long {
                assert!(shown.rect.width() <= 144.0 + 0.01, "{at} {shown:?}");
            }
            let (found, _) = rects(&harness);
            let bar = found.iter().find(|(r, fill, _)| *fill == p.cyan_edge && r.width() == 2.0).map(|(r, ..)| *r);
            assert_eq!(bar, (state == Suggest::Selected).then_some(egui::Rect::from_min_size(rect.min, egui::vec2(2.0, 40.0))), "{at}");
            let chip = found.iter().find(|(r, _, stroke)| *stroke == Some((1.0, p.line)) && r.height() == 20.0).map(|(r, ..)| *r);
            assert_eq!(
                chip,
                (state == Suggest::Selected).then_some(egui::Rect::from_min_size(rect.min + egui::vec2(265.0, 10.0), egui::vec2(43.0, 20.0))),
                "{at}"
            );
        }
    }
}

#[test]
fn enter_and_space_press_a_suggestion_as_the_pointer_does() {
    for mode in [crate::theme::Mode::Dark, crate::theme::Mode::Light] {
        held(mode, "Teodor Vasquez, teodor@example.org", |ui, theme| {
            ui.set_width(320.0);
            super::suggestion_row(ui, theme, "Teodor Vasquez", "teodor@example.org", false);
        });
    }
}

/// The node with the keyboard, its role and name.
fn focused_node<State>(harness: &Harness<'_, State>) -> Option<(Role, Option<String>, egui::accesskit::NodeId)> {
    harness.root().children_recursive().find(|node| node.accesskit_node().is_focused()).map(|node| {
        let data = node.accesskit_node();
        (data.role(), data.label(), data.locate().0)
    })
}

/// A menu that the control `opener` (by role and name) opens, walked by the keyboard: open by `key` on the focused
/// opener (down and up in frames of their own), Tab round its rows (each a menu row, back on the first, never out of
/// the menu), then Esc: closed, the keyboard back on the opener.
pub(crate) fn walk_a_menu<State>(harness: &mut Harness<'_, State>, opener: (Role, &str), key: Option<Key>) {
    let (role, label) = opener;
    harness.get_by_role_and_label(role, label).focus();
    harness.run_ok();
    match key {
        Some(key) => {
            harness.key_down(key);
            harness.run_ok();
            harness.key_up(key);
            harness.run_ok();
        }
        None => {
            harness.get_by_role_and_label(role, label).click();
            harness.run_ok();
        }
    }
    walk_an_open_menu(harness, opener);
}

/// A menu just opened from `opener`, walked as [`walk_a_menu`] walks it after opening it.
pub(crate) fn walk_an_open_menu<State>(harness: &mut Harness<'_, State>, opener: (Role, &str)) {
    let (role, label) = opener;
    let mut seen = Vec::new();
    for _ in 0..60 {
        harness.key_press(Key::Tab);
        harness.run_ok();
        let Some((role, name, id)) = focused_node(harness) else { panic!("{label}: Tab left nothing focused") };
        assert!(matches!(role, Role::MenuItem | Role::MenuItemRadio), "{label}: Tab left the menu for {role:?} {name:?}");
        if seen.first() == Some(&id) {
            break;
        }
        seen.push(id);
    }
    assert!(seen.len() >= 2 && seen.len() < 60, "{label}: Tab goes round {} rows", seen.len());
    harness.key_press_modifiers(Modifiers::SHIFT, Key::Tab);
    harness.run_ok();
    assert_eq!(focused_node(harness).map(|(.., id)| id), seen.last().copied(), "{label}: Shift Tab from the first is the last");
    harness.key_press(Key::Escape);
    harness.run_ok();
    assert!(
        harness
            .query_all_by_role(Role::MenuItem)
            .chain(harness.query_all_by_role(Role::MenuItemRadio))
            .all(|row| !seen.contains(&row.accesskit_node().locate().0)),
        "{label}: Esc closed it"
    );
    assert!(harness.get_by_role_and_label(role, label).is_focused(), "{label}: the keyboard is back on the opener");
}

#[test]
fn every_row_pressed_by_the_pointer_enter_or_space_takes_the_decided_colours() {
    use crate::component::press_tests::{Draw, Part, held_by};
    let whole: Part = |rect, _| rect;
    let rows: [(&str, Draw, bool); 6] = [
        (
            "Archive",
            |ui, theme| {
                ui.set_width(240.0);
                super::menu_item(
                    ui,
                    theme,
                    &Keymap::default(),
                    &super::Item { icon: Some(IconId::Archive), name: "Archive", caption: None, action: None, state: super::ItemState::Default },
                );
            },
            false,
        ),
        (
            "Chosen",
            |ui, theme| {
                ui.set_width(240.0);
                super::menu_item(
                    ui,
                    theme,
                    &Keymap::default(),
                    &super::Item { icon: Some(IconId::Archive), name: "Chosen", caption: None, action: None, state: super::ItemState::Chosen },
                );
            },
            true,
        ),
        (
            "Every week",
            |ui, theme| {
                ui.set_width(240.0);
                super::menu_choice(ui, theme, "Every week", false, true);
            },
            false,
        ),
        (
            "Every month",
            |ui, theme| {
                ui.set_width(240.0);
                super::menu_choice(ui, theme, "Every month", true, true);
            },
            true,
        ),
        (
            "Teodor Vasquez, teodor@example.org",
            |ui, theme| {
                ui.set_width(320.0);
                super::suggestion_row(ui, theme, "Teodor Vasquez", "teodor@example.org", false);
            },
            false,
        ),
        (
            "Mirela Anton, mirela@example.org",
            |ui, theme| {
                ui.set_width(320.0);
                super::suggestion_row(ui, theme, "Mirela Anton", "mirela@example.org", true);
            },
            true,
        ),
    ];
    for mode in [crate::theme::Mode::Dark, crate::theme::Mode::Light] {
        let p = crate::theme::Theme::new(mode, crate::theme::TextScale::default()).palette;
        for (label, draw, chosen) in rows {
            let want = if chosen { (Some(p.cyan_muted), Some((1.0, p.cyan_edge, false))) } else { (Some(p.stage), Some((1.0, p.ink, false))) };
            for key in [None, Some(Key::Enter), Some(Key::Space)] {
                let (_, look) = held_by(mode, label, key, egui::vec2(420.0, 120.0), whole, draw);
                assert_eq!((look.fill, look.edge), want, "{mode:?} {label} {key:?}");
            }
        }
    }
}

#[test]
fn a_float_larger_than_the_window_stays_inside_and_scrolls() {
    let theme = crate::theme::Theme::new(crate::theme::Mode::Dark, crate::theme::TextScale::default());
    let (width, height) = (600.0, 300.0);
    let mut harness = builder().with_size(egui::vec2(width, height)).build_ui_state(
        move |ui, chosen: &mut Option<usize>| {
            motion::set_reduced(ui.ctx(), true);
            let keymap = Keymap::default();
            let float = super::Float {
                id: Id::unique("tall"),
                anchor: egui::Rect::from_min_size(pos2(40.0, 20.0), egui::vec2(8.0, 8.0)),
                width: 1000.0,
                returns_to: Id::unique("x"),
            };
            super::float(ui.ctx(), &theme, &float, |ui| {
                for n in 0..40 {
                    let name = format!("Row {n}");
                    if super::menu_item(
                        ui,
                        &theme,
                        &keymap,
                        &super::Item { icon: None, name: &name, caption: None, action: None, state: super::ItemState::Default },
                    )
                    .clicked()
                    {
                        *chosen = Some(n);
                    }
                }
            });
        },
        None,
    );
    harness.run();
    let (found, _) = rects(&harness);
    let float = found.iter().find(|(_, fill, _)| *fill == theme.palette.surface).map(|(rect, ..)| *rect).unwrap_or(egui::Rect::NOTHING);
    assert!(float.left() >= 3.0 && float.top() >= 3.0 && float.right() <= width - 3.0 && float.bottom() <= height - 3.0, "{float:?}");
    assert!(
        found.iter().any(|(rect, fill, _)| *fill == theme.palette.muted && rect.width() == 2.0 && float.contains_rect(*rect)),
        "its rows scroll: the thumb"
    );
    // The last row is reached by scrolling.
    harness.get_by_label("Row 39").scroll_to_me();
    harness.run();
    assert!(float.contains_rect(harness.get_by_label("Row 39").rect()), "the last row scrolls into the float");
    harness.get_by_label("Row 39").click();
    harness.run();
    assert_eq!(*harness.state(), Some(39), "a row in the scrolled float takes its press");
}

#[test]
fn esc_in_a_float_opened_from_a_float_closes_only_the_inner_one() {
    let theme = theme();
    let mut harness = builder().with_size(egui::vec2(600.0, 400.0)).build_ui_state(
        move |ui, (outer, inner): &mut (bool, bool)| {
            motion::set_reduced(ui.ctx(), true);
            let keymap = Keymap::default();
            let open = crate::component::button::TextButton::new("Open").show(ui, &theme, &keymap);
            if open.clicked() {
                *outer = true;
            }
            if !*outer {
                return;
            }
            let float = super::Float { id: Id::unique("outer"), anchor: open.rect, width: 240.0, returns_to: open.id };
            let shown = super::float(ui.ctx(), &theme, &float, |ui| {
                let more = super::menu_item(
                    ui,
                    &theme,
                    &keymap,
                    &super::Item { icon: None, name: "More", caption: None, action: None, state: super::ItemState::Default },
                );
                if more.clicked() {
                    *inner = true;
                }
                if *inner {
                    let float = super::Float { id: Id::unique("inner"), anchor: more.rect, width: 200.0, returns_to: more.id };
                    let shown = super::float(ui.ctx(), &theme, &float, |ui| {
                        for name in ["Inner first", "Inner second"] {
                            super::menu_item(
                                ui,
                                &theme,
                                &keymap,
                                &super::Item { icon: None, name, caption: None, action: None, state: super::ItemState::Default },
                            );
                        }
                    });
                    *inner = shown.is_some();
                }
            });
            *outer = shown.is_some();
        },
        (false, false),
    );
    harness.run();
    harness.get_by_label("Open").click();
    harness.run();
    harness.key_press(Key::Tab);
    harness.run();
    harness.key_press(Key::Enter);
    harness.run();
    assert_eq!(*harness.state(), (true, true), "both open");
    harness.key_press(Key::Tab);
    harness.run();
    assert!(harness.get_by_label("Inner first").is_focused(), "Tab is the inner float's");
    harness.key_press(Key::Escape);
    harness.run();
    assert_eq!(*harness.state(), (true, false), "only the inner one closed");
    assert!(harness.get_by_label("More").is_focused(), "the keyboard is back in the outer one");
    harness.key_press(Key::Escape);
    harness.run();
    assert_eq!(*harness.state(), (false, false));
    assert!(harness.get_by_label("Open").is_focused());
}

/// A button "Actions" that opens a [`super::menu`] of "Archive" and "Delete"; the state is the row last chosen.
fn menu_opener() -> Harness<'static, Option<&'static str>> {
    let theme = theme();
    let keymap = Keymap::default();
    let mut harness = builder().with_size(egui::vec2(600.0, 400.0)).build_ui_state(
        move |ui, chosen: &mut Option<&'static str>| {
            motion::set_reduced(ui.ctx(), true);
            let button = crate::component::button::TextButton::new("Actions").show(ui, &theme, &keymap);
            super::menu(ui.ctx(), &theme, &button, Id::unique("actions"), 240.0, |ui| {
                let mut picked = false;
                for name in ["Archive", "Delete"] {
                    if super::entry(ui, &theme, &keymap, name, None).clicked() {
                        *chosen = Some(name);
                        picked = true;
                    }
                }
                ((), picked)
            });
        },
        None,
    );
    harness.run_ok();
    harness
}

/// How a menu is opened or a row chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum By {
    Key,
    Pointer,
}

#[test]
fn a_menu_opened_by_the_keyboard_gives_it_back_after_any_choice_and_one_opened_by_the_pointer_does_not() {
    for opened in [By::Key, By::Pointer] {
        for chose in [By::Key, By::Pointer] {
            let mut harness = menu_opener();
            match opened {
                By::Key => {
                    harness.get_by_label("Actions").focus();
                    harness.run_ok();
                    harness.key_press(Key::Enter);
                }
                By::Pointer => harness.get_by_label("Actions").click(),
            }
            harness.run_ok();
            assert!(harness.query_by_label("Delete").is_some(), "{opened:?}: open");
            match chose {
                By::Key => {
                    harness.key_press(Key::Tab);
                    harness.run_ok();
                    harness.key_press(Key::Tab);
                    harness.run_ok();
                    assert!(harness.get_by_label("Delete").is_focused(), "{opened:?}/{chose:?}");
                    harness.key_press(Key::Enter);
                }
                By::Pointer => harness.get_by_label("Delete").click(),
            }
            harness.run_ok();
            assert_eq!(*harness.state(), Some("Delete"), "{opened:?}/{chose:?}: chosen");
            assert!(harness.query_by_label("Archive").is_none(), "{opened:?}/{chose:?}: closed");
            let focused = focused_node(&harness).and_then(|(_, name, _)| name);
            match opened {
                By::Key => assert_eq!(focused.as_deref(), Some("Actions"), "{opened:?}/{chose:?}: the keyboard is back on the opener"),
                By::Pointer => assert_eq!(focused, None, "{opened:?}/{chose:?}: the keyboard stays where it was, with nothing"),
            }
        }
    }
}

#[test]
fn esc_gives_a_menu_s_opener_the_keyboard_however_it_opened() {
    for opened in [By::Key, By::Pointer] {
        let mut harness = menu_opener();
        match opened {
            By::Key => {
                harness.get_by_label("Actions").focus();
                harness.run_ok();
                harness.key_press(Key::Space);
            }
            By::Pointer => harness.get_by_label("Actions").click(),
        }
        harness.run_ok();
        assert!(harness.query_by_label("Delete").is_some(), "{opened:?}: open");
        harness.key_press(Key::Escape);
        harness.run_ok();
        assert!(harness.query_by_label("Delete").is_none(), "{opened:?}: closed");
        assert!(harness.get_by_label("Actions").is_focused(), "{opened:?}: Esc gives the opener the keyboard");
    }
}

#[test]
fn a_press_outside_a_menu_gives_the_keyboard_back_only_to_one_opened_by_the_keyboard() {
    for opened in [By::Key, By::Pointer] {
        let mut harness = menu_opener();
        match opened {
            By::Key => {
                harness.get_by_label("Actions").focus();
                harness.run_ok();
                harness.key_press(Key::Enter);
            }
            By::Pointer => harness.get_by_label("Actions").click(),
        }
        harness.run_ok();
        harness.hover_at(pos2(580.0, 380.0));
        harness.run_ok();
        harness.drag_at(pos2(580.0, 380.0));
        harness.run_ok();
        assert!(harness.query_by_label("Delete").is_none(), "{opened:?}: closed");
        let focused = focused_node(&harness).and_then(|(_, name, _)| name);
        let expected = (opened == By::Key).then(|| "Actions".to_owned());
        assert_eq!(focused, expected, "{opened:?}");
    }
}

#[test]
fn a_screen_reader_s_press_opens_a_menu_from_the_keyboard() {
    let mut harness = menu_opener();
    assert!(!harness.get_by_label("Actions").is_focused(), "the opener does not have the keyboard first");
    harness.get_by_label("Actions").click_accesskit();
    harness.run_ok();
    assert!(harness.query_by_label("Delete").is_some(), "open");
    harness.get_by_label("Delete").click();
    harness.run_ok();
    assert_eq!(*harness.state(), Some("Delete"));
    let focused = focused_node(&harness).and_then(|(_, name, _)| name);
    assert_eq!(focused.as_deref(), Some("Actions"), "the keyboard is back on the opener, as after Enter");
}

#[test]
fn a_menu_hung_by_its_right_edge_stays_in_the_window() {
    // A button near the window's left edge: hung by its right edge the menu would start left of the window, so it
    // stands 3 in; a button far right hangs it with the two right edges on one line.
    for (left, wanted) in [(8.0, None), (400.0, Some(()))] {
        let theme = theme();
        let keymap = Keymap::default();
        let mut harness = builder().with_size(egui::vec2(600.0, 400.0)).build_ui_state(
            move |ui, _: &mut ()| {
                motion::set_reduced(ui.ctx(), true);
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(pos2(left, 8.0), egui::vec2(32.0, 32.0))));
                let button = crate::component::button::TextButton::new("Actions").show(&mut child, &theme, &keymap);
                super::menu_right(ui.ctx(), &theme, &button, Id::unique("actions"), 240.0, |ui| {
                    super::entry(ui, &theme, &keymap, "Archive", None);
                    ((), false)
                });
            },
            (),
        );
        harness.run_ok();
        let button = harness.get_by_label("Actions").rect();
        harness.get_by_label("Actions").click();
        harness.run_ok();
        let row = harness.get_by_label("Archive").rect();
        // A row runs at most 1 in from the menu's edge.
        match wanted {
            Some(_) => assert!((row.right() - button.right()).abs() <= 1.5, "{row:?} {button:?}"),
            None => assert!(row.left() >= 3.0 && row.left() <= 4.5, "3 in from the window's left edge: {row:?}"),
        }
        assert!(row.left() >= 3.0 && row.right() <= 597.0, "{row:?}");
    }
}
