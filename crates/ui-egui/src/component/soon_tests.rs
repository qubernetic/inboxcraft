// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::epaint::{ClippedShape, ColorMode};
use egui::{Color32, Key, Rect, Sense, Shape};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::{blocked, is_soon, soon, soon_tag};
use crate::component::button::{self, IconButton, TextButton};
use crate::component::checkbox::{checkbox, radio};
use crate::component::segmented::segmented;
use crate::component::select::Select;
use crate::component::switch::switch;
use crate::component::tests::{Look, builder, keyboard_walk, painted, stage, theme};
use crate::component::{card, pill, popup};
use crate::iconset::IconId;
use crate::keymap::{self, Keymap};
use crate::motion;
use crate::theme::{Mode, TextScale, Theme};

/// What a test reads back from the frame: the soon text button's sense, the rect `soon` covered, and whether any
/// control inside it reported a click.
#[derive(Debug, Default)]
struct Seen {
    sense: Option<Sense>,
    area: Option<Rect>,
    clicked: bool,
    inside: Option<bool>,
    outside: Option<bool>,
}

/// A text button "Export" inside `soon`, and an ordinary "Keep" outside it so that Tab has somewhere to go.
fn one_button() -> Harness<'static, Seen> {
    let theme = theme();
    let keymap = Keymap::empty();
    let mut harness = builder().build_ui_state(
        move |ui, seen: &mut Seen| {
            motion::set_reduced(ui.ctx(), true);
            stage(ui, theme.palette.surface, |ui| {
                ui.spacing_mut().item_spacing.y = 16.0;
                TextButton::new("Keep").show(ui, &theme, &keymap);
                seen.outside = Some(is_soon(ui));
                let response = soon(ui, &theme, |ui| {
                    seen.inside = Some(is_soon(ui));
                    TextButton::new("Export").secondary().show(ui, &theme, &keymap)
                });
                seen.sense = Some(response.sense);
                seen.clicked |= response.clicked();
            });
        },
        Seen::default(),
    );
    harness.run();
    harness
}

#[test]
fn soon_holds_only_inside_it() {
    let harness = one_button();
    assert_eq!(harness.state().inside, Some(true));
    assert_eq!(harness.state().outside, Some(false));
}

#[test]
fn inside_soon_a_text_button_senses_hover_only_takes_no_focus_and_is_disabled() {
    let mut harness = one_button();
    assert_eq!(harness.state().sense, Some(Sense::hover()));
    assert!(harness.get_by_label("Export").accesskit_node().is_disabled());
    let missed = keyboard_walk(&mut harness);
    assert!(missed.is_empty(), "a soon control is no gap of the walk: {missed:?}");
    for _ in 0..4 {
        harness.key_press(Key::Tab);
        harness.step();
        assert!(!harness.get_by_label("Export").accesskit_node().is_focused(), "Tab never stops on a soon control");
    }
    harness.get_by_label("Export").click();
    harness.run();
    assert!(!harness.state().clicked, "a soon control cannot be pressed");
}

#[test]
fn inside_soon_a_text_buttons_label_is_muted_and_its_edge_dashed_as_a_disabled_buttons() {
    let harness = one_button();
    let muted = theme().palette.muted;
    let line = theme().palette.line;
    let paint = painted(&harness, "Export");
    assert!(
        paint.shapes.iter().any(|shape| matches!(&shape.look, Look::Text { text, colour, .. } if text == "Export" && *colour == muted)),
        "{paint:#?}"
    );
    // The text button's sheet: disabled is a dashed `line` edge and `muted` text.
    let dashed =
        Look::Rect { fill: Color32::TRANSPARENT, stroke: Some((1.0, line)), stroke_kind: egui::StrokeKind::Inside, radius: 0.0, dashed: true };
    assert!(paint.rects().any(|shape| shape.inside && shape.look == dashed), "{paint:#?}");
    assert!(
        !paint.rects().any(|shape| matches!(shape.look, Look::Rect { fill, dashed: false, .. } if shape.inside && fill.a() > 0)),
        "no fill: {paint:#?}"
    );
}

fn tag_alone() -> Harness<'static> {
    let theme = theme();
    let mut harness = builder().build_ui(move |ui| {
        stage(ui, theme.palette.surface, |ui| soon_tag(ui, &theme));
    });
    harness.run();
    harness
}

#[test]
fn the_tag_is_20_high_mono_s_muted_in_a_dashed_muted_edge_and_empty() {
    let harness = tag_alone();
    let theme = theme();
    let muted = theme.palette.muted;
    let node = harness.get_by_label("coming soon");
    assert!((node.rect().height() - theme.size(20.0)).abs() < 0.01, "{:?}", node.rect());
    let paint = painted(&harness, "coming soon");
    let font = theme.fonts.mono_s;
    let text = paint.shapes.iter().find_map(|shape| match &shape.look {
        Look::Text { text, colour, family, size, .. } if text == "coming soon" => Some((*colour, family.clone(), *size, shape.inside)),
        _ => None,
    });
    assert_eq!(text, Some((muted, font.id().family.to_string(), font.size, true)), "{paint:#?}");
    let dashed =
        Look::Rect { fill: Color32::TRANSPARENT, stroke: Some((1.0, muted)), stroke_kind: egui::StrokeKind::Inside, radius: 0.0, dashed: true };
    assert!(paint.rects().any(|shape| shape.inside && shape.look == dashed && shape.rect == node.rect()), "{paint:#?}");
    assert!(!paint.rects().any(|shape| matches!(shape.look, Look::Rect { fill, .. } if fill.a() > 0)), "empty: {paint:#?}");
}

#[test]
fn the_tag_grows_with_the_text() {
    let larger = || Theme::new(Mode::Dark, TextScale::from_percent(130).unwrap_or_default());
    let theme = larger();
    let mut harness = builder().build_ui(move |ui| {
        stage(ui, theme.palette.surface, |ui| soon_tag(ui, &theme));
    });
    harness.run();
    let high = harness.get_by_label("coming soon").rect().height();
    assert!((high - larger().size(20.0)).abs() < 0.01 && high > 20.0, "{high}");
}

/// Every interactive component inside one `soon`, with the tag on it.
fn board() -> Harness<'static, Seen> {
    let theme = theme();
    let keymap = Keymap::empty();
    let mut harness = builder().with_size(egui::vec2(600.0, 1200.0)).build_ui_state(
        move |ui, seen: &mut Seen| {
            motion::set_reduced(ui.ctx(), true);
            stage(ui, theme.palette.surface, |ui| {
                ui.spacing_mut().item_spacing.y = 16.0;
                let (area, clicked) = soon(ui, &theme, |ui| {
                    let mut clicked = false;
                    soon_tag(ui, &theme);
                    clicked |= TextButton::new("Send").primary().show(ui, &theme, &keymap).clicked();
                    clicked |= TextButton::new("Reply all").secondary().show(ui, &theme, &keymap).clicked();
                    clicked |= TextButton::new("Block").danger().show(ui, &theme, &keymap).clicked();
                    clicked |= TextButton::new("Discard").show(ui, &theme, &keymap).clicked();
                    clicked |= IconButton::new(IconId::Archive, "Archive").show(ui, &theme, &keymap).clicked();
                    clicked |= button::link(ui, &theme, &keymap, "Details").clicked();
                    clicked |= switch(ui, &theme, "Sync contacts", true).clicked();
                    clicked |= checkbox(ui, &theme, "Personal", true).clicked();
                    clicked |= radio(ui, &theme, "Daily", true).clicked();
                    clicked |= pill::pill(ui, &theme, "Unread", true).clicked();
                    clicked |= pill::pill(ui, &theme, "Flagged", false).clicked();
                    let font = theme.fonts.button;
                    clicked |= segmented(ui, &theme, &[(0, "Day", font), (1, "Week", font)], 1).is_some();
                    let names = [(0, "Never"), (1, "Weekly")];
                    let mut open = false;
                    let select = Select { name: "Repeat", shown: "Never", options: &names, current: Some(0), icon: IconId::Changed, width: 200.0 };
                    clicked |= select.show(ui, &theme, &mut open).0.clicked() || open;
                    clicked |= popup::entry(ui, &theme, &keymap, "Templates", None).clicked();
                    clicked |= card::card(ui, &theme, egui::Id::unique("soon-card"), "Scheduled", false, |_| ()).header.clicked();
                    (ui.min_rect(), clicked)
                });
                seen.area = Some(area);
                seen.clicked |= clicked;
            });
        },
        Seen::default(),
    );
    harness.run();
    harness
}

/// The controls of [`board`], by label.
const CONTROLS: &[&str] = &[
    "Send",
    "Reply all",
    "Block",
    "Discard",
    "Archive",
    "Details",
    "Sync contacts",
    "Personal",
    "Daily",
    "Unread",
    "Flagged",
    "Day",
    "Week",
    "Repeat",
    "Templates",
    "Scheduled",
];

#[test]
fn every_component_inside_soon_is_disabled_and_cannot_be_pressed() {
    let mut harness = board();
    for label in CONTROLS {
        assert!(harness.get_by_label(label).accesskit_node().is_disabled(), "{label} is disabled");
    }
    assert!(keyboard_walk(&mut harness).is_empty());
    for label in CONTROLS {
        harness.get_by_label(label).click();
        harness.run();
        assert!(!harness.state().clicked, "{label} was pressed");
    }
    harness.get_by_label("Templates").hover();
    harness.run();
    assert!(!harness.state().clicked);
}

#[test]
fn every_text_inside_soon_is_muted() {
    let harness = board();
    let muted = theme().palette.muted;
    // A glyph is no text: a control without a box draws its glyph `dim`.
    let icons = crate::theme::icon_font(1.0).family.to_string();
    for label in CONTROLS {
        let paint = painted(&harness, label);
        let texts: Vec<_> = paint
            .shapes
            .iter()
            .filter_map(|shape| match &shape.look {
                Look::Text { text, colour, family, .. } if shape.inside && *family != icons => Some((text.clone(), *colour)),
                _ => None,
            })
            .collect();
        assert!(texts.iter().all(|(_, colour)| *colour == muted), "{label}: {texts:?}");
    }
}

#[test]
fn every_box_inside_soon_has_a_dashed_edge() {
    let harness = board();
    let p = theme().palette;
    // A button's disabled edge is `line` (its sheet); the other boxes keep the `muted` edge of coming soon.
    let buttons = ["Send", "Reply all", "Block"];
    for label in buttons.into_iter().chain(["Sync contacts", "Personal", "Daily", "Unread", "Flagged", "Repeat", "Scheduled", "coming soon"]) {
        let paint = painted(&harness, label);
        let want = if buttons.contains(&label) { p.line } else { p.muted };
        let edge = paint.rects().find(|shape| matches!(shape.look, Look::Rect { stroke: Some((_, colour)), dashed: true, .. } if colour == want));
        let edge = edge.unwrap_or_else(|| panic!("{label}: no dashed edge in its colour: {paint:#?}"));
        assert!(
            !paint.rects().any(
                |shape| matches!(shape.look, Look::Rect { stroke: Some(_), dashed: false, .. } if shape.rect.contains_rect(edge.rect.shrink(1.0)))
            ),
            "{label}: the dashed edge is its only edge: {paint:#?}"
        );
    }
}

/// Every colour `shape` paints with.
fn colours(shape: &Shape, found: &mut Vec<Color32>) {
    match shape {
        Shape::Vec(shapes) => shapes.iter().for_each(|shape| colours(shape, found)),
        Shape::Rect(rect) => found.extend([rect.fill, rect.stroke.color]),
        Shape::Circle(circle) => found.extend([circle.fill, circle.stroke.color]),
        Shape::Ellipse(ellipse) => found.extend([ellipse.fill, ellipse.stroke.color]),
        Shape::LineSegment { stroke, .. } => found.push(stroke.color),
        Shape::Path(path) => {
            found.push(path.fill);
            if let ColorMode::Solid(colour) = path.stroke.color {
                found.push(colour);
            }
        }
        Shape::Text(text) => {
            found.push(text.fallback_color);
            found.extend(text.override_text_color);
            for row in &text.galley.rows {
                // The placeholder is never drawn: the painter's colour takes its place.
                found.extend(row.row.visuals.mesh.vertices.iter().map(|vertex| vertex.color).filter(|colour| *colour != Color32::PLACEHOLDER));
            }
            if text.opacity_factor < 1.0 {
                found.push(Color32::from_black_alpha(1));
            }
        }
        Shape::Mesh(mesh) => found.extend(mesh.vertices.iter().map(|vertex| vertex.color)),
        _ => {}
    }
}

fn bounds(shape: &ClippedShape) -> Rect {
    shape.shape.visual_bounding_rect()
}

#[test]
fn nothing_inside_soon_is_painted_faded() {
    let harness = board();
    let area = harness.state().area.unwrap_or(Rect::NOTHING);
    assert!(area.is_positive());
    let mut found = Vec::new();
    for clipped in harness.output().shapes.iter().filter(|clipped| area.expand(4.0).contains_rect(bounds(clipped))) {
        colours(&clipped.shape, &mut found);
    }
    assert!(!found.is_empty());
    let faded: Vec<_> = found.into_iter().filter(|colour| colour.a() != 0 && colour.a() != 255).collect();
    assert!(faded.is_empty(), "painted at less than full strength: {faded:?}");
}

/// What the keyed dispatcher of [`keyed`] saw: how often the archive key ran its action.
type Ran = u32;

/// A soon text button that carries the archive action, and with `normal` the same action on a usable button too.
/// Each frame runs the archive action when its key is pressed and it is not [`blocked`], as the window does.
fn keyed(normal: bool) -> Harness<'static, Ran> {
    let theme = theme();
    let keymap = Keymap::default();
    let mut harness = builder().build_ui_state(
        move |ui, ran: &mut Ran| {
            motion::set_reduced(ui.ctx(), true);
            if keymap.pressed(ui.ctx(), keymap::ARCHIVE) && !blocked(ui.ctx(), keymap::ARCHIVE) {
                *ran += 1;
            }
            stage(ui, theme.palette.surface, |ui| {
                ui.spacing_mut().item_spacing.y = 16.0;
                soon(ui, &theme, |ui| TextButton::new("Archive later").action(keymap::ARCHIVE).show(ui, &theme, &keymap));
                if normal {
                    TextButton::new("Archive now").action(keymap::ARCHIVE).show(ui, &theme, &keymap);
                }
            });
        },
        0,
    );
    harness.run();
    harness
}

#[test]
fn the_key_of_an_action_only_a_soon_control_offers_runs_nothing() {
    let mut harness = keyed(false);
    assert!(blocked(&harness.ctx, keymap::ARCHIVE));
    assert!(!blocked(&harness.ctx, keymap::DELETE), "an action nothing offered is not blocked");
    harness.key_press(Key::A);
    harness.run();
    assert_eq!(*harness.state(), 0);
}

#[test]
fn an_action_a_usable_control_offers_too_still_runs_by_its_key() {
    let mut harness = keyed(true);
    assert!(!blocked(&harness.ctx, keymap::ARCHIVE));
    harness.key_press(Key::A);
    harness.run();
    assert_eq!(*harness.state(), 1);
}

#[test]
fn a_block_lasts_only_while_a_soon_control_offers_the_action() {
    let theme = theme();
    let keymap = Keymap::default();
    let mut harness = builder().build_ui_state(
        move |ui, shown: &mut bool| {
            if *shown {
                soon(ui, &theme, |ui| IconButton::new(IconId::Archive, "Archive later").action(keymap::ARCHIVE).show(ui, &theme, &keymap));
            }
        },
        true,
    );
    harness.run();
    assert!(blocked(&harness.ctx, keymap::ARCHIVE));
    *harness.state_mut() = false;
    harness.run();
    assert!(!blocked(&harness.ctx, keymap::ARCHIVE), "the soon control is gone");
}

#[test]
fn a_soon_controls_tooltip_names_it_and_shows_no_key() {
    let theme = theme();
    let keymap = Keymap::default();
    let chip = keymap.chip(&egui::Context::default(), keymap::ARCHIVE).unwrap_or_default();
    assert!(!chip.is_empty());
    for soon_now in [false, true] {
        let keymap = keymap.clone();
        let mut harness = builder()
            .with_context_setup(|ctx| {
                crate::theme::install_fonts(ctx);
                crate::component::tests::record_layers(ctx);
                crate::component::tests::quick_tooltips(ctx);
            })
            .build_ui(move |ui| {
                motion::set_reduced(ui.ctx(), true);
                stage(ui, theme.palette.surface, |ui| {
                    let button = IconButton::new(IconId::Archive, "Archive later").action(keymap::ARCHIVE);
                    if soon_now { soon(ui, &theme, |ui| button.show(ui, &theme, &keymap)) } else { button.show(ui, &theme, &keymap) }
                });
            });
        harness.run();
        harness.get_by_label("Archive later").hover();
        harness.run();
        assert_eq!(harness.query_all_by_label("Archive later").count(), 2, "the tooltip names it, soon: {soon_now}");
        assert_eq!(harness.query_by_label(&chip).is_some(), !soon_now, "the key chip {chip} shows only when it runs");
    }
}
