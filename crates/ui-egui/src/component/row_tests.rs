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
        row.finish(ui, theme);
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
                row.finish(ui, &theme);
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
                        row.finish(ui, &theme);
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
                    row.finish(ui, &theme);
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
                row.finish(ui, &theme);
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

/// A focused row with real content: the caller's whole-row selection fill, an avatar, a subject running past both
/// ends, and the hover actions shown on its first line; then the row is finished. Its rect, and the picture at `scale`.
fn focused_with_content(mode: Mode, scale: f32) -> (Rect, crate::shot::Picture) {
    let theme = Theme::new(mode, TextScale::default());
    let mut placed = Rect::NOTHING;
    let mut fonts = true;
    let picture = crate::shot::paint(vec2(520.0, 120.0), scale, mode, |ui| {
        if std::mem::take(&mut fonts) {
            crate::theme::install_fonts(ui.ctx());
        }
        let p = theme.palette;
        ui.painter().rect_filled(ui.max_rect(), 0.0, p.surface);
        ui.add_space(24.0);
        ui.horizontal(|ui| {
            ui.add_space(24.0);
            ui.vertical(|ui| {
                ui.set_width(440.0);
                let row = super::row_with(ui, &theme, Id::unique("content-row"), 56.0, RowGround::Surface, false, false, Some(Shown::Focus));
                let rect = row.rect;
                // Content that reaches every edge of the row: a whole-row fill, an avatar, a subject past both ends.
                ui.painter().rect_filled(rect, 0.0, p.stage);
                let face = Rect::from_min_size(rect.min + vec2(1.0, 1.0), vec2(40.0, 40.0));
                crate::component::avatar::paint(ui.painter(), &theme, face, "Teodor Vasquez", "teodor@example.org");
                let subject =
                    crate::component::galley(ui, &"Draft logo for the bakery — two directions attached ".repeat(4), theme.fonts.strong, p.ink);
                ui.painter().galley(rect.left_top() + vec2(-40.0, 0.0), subject, p.ink);
                let line = Rect::from_min_max(egui::pos2(rect.center().x, rect.top() + 7.0), egui::pos2(rect.right() - 16.0, rect.top() + 29.0));
                let actions = [crate::component::hover::Action { icon: crate::iconset::IconId::Done, name: "Done", action: None, unavailable: None }];
                crate::component::hover::actions(ui, &theme, &crate::keymap::Keymap::default(), Id::unique("content-actions"), line, true, &actions);
                row.finish(ui, &theme);
                placed = rect;
            });
        });
    });
    (placed, picture.unwrap_or_else(|error| panic!("{error}")))
}

#[test]
fn the_focus_edge_is_drawn_after_the_rows_content_every_pixel_of_it() {
    for scale in [1.0_f32, 1.5] {
        for mode in [Mode::Dark, Mode::Light] {
            let edge = Theme::new(mode, TextScale::default()).palette.cyan_edge;
            let (rect, picture) = focused_with_content(mode, scale);
            assert!(rect.is_positive(), "{mode:?} {scale}: no row");
            // The edge's band in pixels: a 1 point line 2 points inside the row above its line.
            let ring = Rect::from_min_max(rect.min + vec2(2.0, 2.0), egui::pos2(rect.right() - 2.0, rect.bottom() - 3.0));
            let (outer, inner) = (ring, ring.shrink(1.0));
            let mut checked = 0;
            let mut wrong = Vec::new();
            for y in (outer.top() * scale).floor() as usize..(outer.bottom() * scale).ceil() as usize {
                for x in (outer.left() * scale).floor() as usize..(outer.right() * scale).ceil() as usize {
                    let pixel = Rect::from_min_size(egui::pos2(x as f32, y as f32) / scale, vec2(1.0, 1.0) / scale);
                    // Only the pixels the line covers whole: in the band, not touching what it rings.
                    if !outer.expand(0.001).contains_rect(pixel) || pixel.intersect(inner).is_positive() {
                        continue;
                    }
                    checked += 1;
                    if picture.pixel(x, y) != Some(edge) {
                        wrong.push((x, y, picture.pixel(x, y)));
                    }
                }
            }
            assert!(checked > 900, "{mode:?} {scale}: {checked} pixels");
            assert!(
                wrong.is_empty(),
                "{mode:?} {scale}: {} of {checked} pixels of the edge are not the edge: {:?}",
                wrong.len(),
                &wrong[..wrong.len().min(8)]
            );
        }
    }
}

/// The rows `text` lays down (`let <name> = row(…)` or `row_with(…)`, a path before it or not) whose `<name>.finish(`
/// never follows. A row whose result is dropped is refused by `#[must_use]` already.
fn unfinished(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().map(|line| line.split("//").next().unwrap_or_default()).collect();
    let mut found = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let Some(rest) = line.trim_start().strip_prefix("let ") else { continue };
        let Some((name, call)) = rest.split_once(" = ") else { continue };
        let callee = call.split('(').next().unwrap_or_default();
        if !(callee == "row" || callee == "row_with" || callee.ends_with("::row") || callee.ends_with("::row_with")) {
            continue;
        }
        let name = name.trim_start_matches("mut ").trim();
        let finish = format!("{name}.finish(");
        if !lines.iter().skip(index + 1).any(|later| later.contains(&finish)) {
            found.push(format!("line {}: {name}", index + 1));
        }
    }
    found
}

#[test]
fn every_row_a_view_lays_down_is_finished() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    let mut dirs = vec![root];
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().and_then(|name| name.to_str()).unwrap_or_default().to_owned();
            if path.is_dir() {
                dirs.push(path);
            } else if name.ends_with(".rs") && !name.ends_with("tests.rs") && !path.ends_with("component/row.rs") {
                files.push(path);
            }
        }
    }
    let mut unfinished = Vec::new();
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        if !text.contains("RowGround") {
            continue;
        }
        for row in self::unfinished(&text) {
            unfinished.push(format!("{}: {row}", file.display()));
        }
    }
    assert!(unfinished.is_empty(), "a row's focus edge comes with Row::finish after its content:\n{}", unfinished.join("\n"));
    let sample = "let drawn = super::row::row_with(ui, theme, id, 56.0, ground, false, false, None);\nlet text = 1;";
    assert_eq!(self::unfinished(sample), vec!["line 1: drawn".to_owned()], "the check finds a row left unfinished");
    assert!(self::unfinished(&format!("{sample}\ndrawn.finish(ui, theme);")).is_empty());
}
