// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The row's states as the mail row's sheet draws them: read (rest), hover, selected (current) and focus; the panel's
//! hover from the components' rules; the last row without its line.

use egui::accesskit::Role;
use egui::{Color32, Id, Rect, StrokeKind, WidgetInfo, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

use super::{RowGround, row};
use crate::component::Shown;
use crate::component::tests::{Look, Paint, bring, own, painted, staged, staged_motion, strictly_between};
use crate::theme::{Mode, Palette, TextScale, Theme};

/// The sheet's row: 440 × 56.
const WIDTH: f32 = 440.0;

/// A harness with one row "Row" of 56 on `ground`, `current` and `last` as given, motion reduced or not.
fn harness(theme: Theme, reduced: bool, ground: RowGround, current: bool, last: bool) -> Harness<'static> {
    let fill: fn(&Palette) -> Color32 = match ground {
        RowGround::Surface => |p| p.surface,
        RowGround::Panel => |p| p.panel,
    };
    let size = vec2(WIDTH + 48.0, theme.size(56.0) + 64.0);
    let add = move |ui: &mut egui::Ui, theme: &Theme| {
        ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
        let row = row(ui, theme, Id::unique("row"), 56.0, ground, current, last);
        row.response.widget_info(|| WidgetInfo::labeled(Role::Button, true, "Row"));
    };
    if reduced { staged(theme, fill, size, add) } else { staged_motion(theme, false, fill, size, add) }
}

/// The row's paint in the state `shown` brings.
fn look(theme: Theme, ground: RowGround, current: bool, last: bool, shown: Shown) -> (Rect, Paint) {
    let mut harness = harness(theme, true, ground, current, last);
    bring(&mut harness, "Row", shown);
    let rect = harness.get_by_label("Row").rect();
    (rect, painted(&harness, "Row"))
}

/// The 1 px `cyan_edge` strokes drawn inside their rect.
fn edges(paint: &Paint, colour: Color32) -> Vec<Rect> {
    paint
        .rects()
        .filter_map(|shape| match shape.look {
            Look::Rect { stroke: Some((width, c)), stroke_kind: StrokeKind::Inside, dashed: false, .. } if width == 1.0 && c == colour => {
                Some(shape.rect)
            }
            _ => None,
        })
        .collect()
}

/// The filled rects with `colour`, no stroke.
fn fills(paint: &Paint, colour: Color32) -> Vec<Rect> {
    paint
        .rects()
        .filter_map(|shape| match shape.look {
            Look::Rect { fill, stroke: None, .. } if fill == colour => Some(shape.rect),
            _ => None,
        })
        .collect()
}

#[test]
fn a_row_looks_as_the_mail_rows_sheet_says_at_rest_hovered_current_and_focused() {
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::default());
        let p = theme.palette;
        for (state, current, shown) in
            [("read", false, Shown::Rest), ("hover", false, Shown::Hover), ("selected", true, Shown::Rest), ("focus", false, Shown::Focus)]
        {
            let (rect, paint) = look(theme, RowGround::Surface, current, false, shown);
            let at = format!("{mode:?} {state}: {paint:#?}");
            assert!((rect.width() - WIDTH).abs() < 0.01 && (rect.height() - 56.0).abs() < 0.01, "{at} {rect:?}");
            assert_eq!(paint.ground, Some(p.surface), "{at}");
            let found = own(&paint, rect);
            let fill = match state {
                "hover" | "selected" => Some(p.stage),
                _ => None,
            };
            assert_eq!(found.fill, fill, "{at}");
            assert_eq!(found.edge, None, "{at}");
            assert_eq!(found.ring, None, "{at}");
            assert_eq!(found.lines, vec![p.panel_line], "one line under it: {at}");
            assert!(found.radius <= 0.0, "{at}");
            // The line lies on the row's last pixel, across its whole width.
            let line = paint.rects().find(|shape| matches!(shape.look, Look::Rect { stroke: Some((_, c)), .. } if c == p.panel_line));
            let line = line.map(|shape| shape.rect).unwrap_or(Rect::NOTHING);
            assert_eq!(line.top(), rect.bottom() - 0.5, "{at}");
            assert!((line.left() - rect.left()).abs() <= 0.5 && (line.right() - rect.right()).abs() <= 0.5 + 0.01, "{at}");
            let bar = fills(&paint, p.cyan_edge);
            if current {
                assert_eq!(bar, vec![Rect::from_min_size(rect.min, vec2(2.0, 56.0))], "the 2 px bar: {at}");
            } else {
                assert!(bar.is_empty(), "{at}");
            }
            let ring = edges(&paint, p.cyan_edge);
            if shown == Shown::Focus {
                let inside = Rect::from_min_max(rect.min + vec2(2.0, 2.0), pos2(rect.right() - 2.0, rect.bottom() - 3.0));
                assert_eq!(ring, vec![inside], "the focus edge 2,2 436 × 51: {at}");
                assert_eq!(inside.size(), vec2(436.0, 51.0), "{at}");
            } else {
                assert!(ring.is_empty(), "{at}");
            }
        }
    }
}

#[test]
fn a_row_on_the_panel_takes_grid_under_the_pointer() {
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::default());
        let (rect, paint) = look(theme, RowGround::Panel, false, false, Shown::Hover);
        assert_eq!(paint.ground, Some(theme.palette.panel));
        assert_eq!(own(&paint, rect).fill, Some(theme.palette.grid), "{mode:?}: {paint:#?}");
        let (rect, paint) = look(theme, RowGround::Panel, true, false, Shown::Hover);
        assert_eq!(own(&paint, rect).fill, Some(theme.palette.stage), "the current row keeps stage: {mode:?}");
    }
}

#[test]
fn a_row_is_the_scaled_height_at_130_percent() {
    let theme = Theme::new(Mode::Light, TextScale::ALL[4]);
    let (rect, _) = look(theme, RowGround::Surface, false, false, Shown::Rest);
    assert!((rect.height() - theme.size(56.0)).abs() < 0.01, "{rect:?}");
}

#[test]
fn at_full_motion_the_hover_fill_cross_fades_over_125_ms_and_rests() {
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::default());
        let p = theme.palette;
        let mut harness = harness(theme, false, RowGround::Surface, false, false);
        let rect = harness.get_by_label("Row").rect();
        harness.get_by_label("Row").hover();
        let mut frames = Vec::new();
        for _ in 0..10 {
            harness.step();
            frames.push(own(&painted(&harness, "Row"), rect).fill);
        }
        let middle = frames.get(3).copied().flatten().unwrap_or(Color32::TRANSPARENT);
        assert!(strictly_between(middle, p.surface, p.stage), "{mode:?} mid-fade {middle:?}: {frames:?}");
        assert_eq!(frames.last().copied().flatten(), Some(p.stage), "{mode:?}");
        assert!(crate::component::tests::rests(&mut harness, 4), "{mode:?}: no repaint once the fill has landed");
    }
}

#[test]
fn at_reduced_motion_the_hover_fill_comes_in_the_first_frame() {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let mut harness = harness(theme, true, RowGround::Surface, false, false);
    let rect = harness.get_by_label("Row").rect();
    harness.get_by_label("Row").hover();
    harness.step();
    assert_eq!(own(&painted(&harness, "Row"), rect).fill, Some(theme.palette.stage));
}

#[test]
fn a_row_out_of_sight_paints_nothing() {
    let theme = Theme::new(Mode::Dark, TextScale::default());
    let count = |with_row: bool| {
        let mut harness = crate::component::tests::builder().with_size(vec2(400.0, 100.0)).build_ui(move |ui| {
            ui.add_space(300.0);
            if with_row {
                let row = row(ui, &theme, Id::unique("row"), 56.0, RowGround::Surface, true, false);
                row.response.widget_info(|| WidgetInfo::labeled(Role::Button, true, "Row"));
            }
        });
        harness.run();
        harness.output().shapes.len()
    };
    assert_eq!(count(true), count(false), "nothing painted for a row below the window");
}

#[test]
fn the_focus_edge_is_two_pixels_inside_and_over_the_scroll_thumb() {
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::default());
        let p = theme.palette;
        let mut harness = crate::component::tests::builder().with_size(vec2(520.0, 240.0)).build_ui(move |ui| {
            crate::motion::set_reduced(ui.ctx(), true);
            ui.spacing_mut().scroll = crate::component::scroll::style(&theme);
            crate::component::tests::stage(ui, theme.palette.surface, |ui| {
                let area = egui::ScrollArea::vertical().id_salt("rows").max_height(150.0).auto_shrink([false, true]);
                crate::component::scroll::show(ui, &theme, area, |ui| {
                    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                    for index in 0..6 {
                        let row = row(ui, &theme, Id::unique("scrolled-row").with(index), 56.0, RowGround::Surface, false, index == 5);
                        let name = format!("Row {index}");
                        row.response.widget_info(|| WidgetInfo::labeled(Role::Button, true, &name));
                    }
                });
            });
        });
        harness.run();
        harness.get_by_label("Row 1").focus();
        harness.run();
        let rect = harness.get_by_label("Row 1").rect();
        let paint = painted(&harness, "Row 1");
        let inside = Rect::from_min_max(rect.min + vec2(2.0, 2.0), pos2(rect.right() - 2.0, rect.bottom() - 3.0));
        assert_eq!(edges(&paint, p.cyan_edge), vec![inside], "{mode:?}: the edge 2 px inside the row");
        // The thumb stands beside the rows: no `muted` fill of the bar crosses the row, so it never covers the edge.
        let thumbs: Vec<Rect> = harness
            .output()
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                egui::Shape::Rect(shape) if shape.fill == p.muted => Some(shape.rect),
                _ => None,
            })
            .collect();
        assert!(!thumbs.is_empty(), "{mode:?}: the rows overflow, so the bar shows its thumb");
        assert!(thumbs.iter().all(|thumb| !thumb.intersects(rect)), "{mode:?}: {thumbs:?} over {rect:?}");
    }
}

#[test]
fn the_last_row_draws_no_line() {
    for ppp in [1.0, 1.5] {
        let theme = Theme::new(Mode::Light, TextScale::default());
        let p = theme.palette;
        let mut harness = crate::component::tests::builder().with_pixels_per_point(ppp).with_size(vec2(520.0, 260.0)).build_ui(move |ui| {
            crate::component::tests::stage(ui, theme.palette.surface, |ui| {
                ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                for index in 0..3 {
                    let row = row(ui, &theme, Id::unique("stacked-row").with(index), 56.0, RowGround::Surface, false, index == 2);
                    let name = format!("Row {index}");
                    row.response.widget_info(|| WidgetInfo::labeled(Role::Button, true, &name));
                }
            });
        });
        harness.run();
        for index in 0..3 {
            let name = format!("Row {index}");
            let rect = harness.get_by_label(&name).rect();
            let paint = painted(&harness, &name);
            let lines: Vec<(Rect, f32)> = paint
                .rects()
                .filter_map(|shape| match shape.look {
                    Look::Rect { stroke: Some((width, colour)), .. } if colour == p.panel_line => Some((shape.rect, width)),
                    _ => None,
                })
                .collect();
            // Its own line lies inside it (the line just above is the row before's).
            let own: Vec<_> = lines.iter().filter(|(line, _)| rect.contains(line.center())).collect();
            if index == 2 {
                assert!(own.is_empty(), "ppp {ppp}: the last row draws no line, so the pane's edge is the one line: {lines:?}");
                continue;
            }
            // On whole physical pixels, inside the row: never doubled with the next row or a pane's edge.
            assert_eq!(own.len(), 1, "ppp {ppp} {name}: {lines:?}");
            let (line, width) = own[0];
            let (top, bottom) = ((line.center().y - width / 2.0) * ppp, (line.center().y + width / 2.0) * ppp);
            assert!((top - top.round()).abs() < 1e-3 && (bottom - bottom.round()).abs() < 1e-3, "ppp {ppp} {name}: {top}..{bottom}");
            assert!(bottom <= (rect.bottom() * ppp).round() + 1e-3 && top >= (rect.bottom() * ppp).round() - ppp.round() - 1e-3, "ppp {ppp} {name}");
        }
    }
}

#[test]
fn enter_and_space_each_press_a_focused_row() {
    for key in [egui::Key::Enter, egui::Key::Space] {
        let theme = Theme::new(Mode::Light, TextScale::default());
        let mut harness = crate::component::tests::builder().with_size(vec2(520.0, 120.0)).build_ui_state(
            move |ui, pressed: &mut usize| {
                let row = row(ui, &theme, Id::unique("pressed-row"), 56.0, RowGround::Surface, false, false);
                row.response.widget_info(|| WidgetInfo::labeled(Role::Button, true, "Row"));
                *pressed += usize::from(row.response.clicked());
            },
            0,
        );
        harness.run();
        harness.get_by_label("Row").focus();
        harness.run();
        harness.key_press(key);
        harness.run();
        assert_eq!(*harness.state(), 1, "{key:?}");
    }
}
