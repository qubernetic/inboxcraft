// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::{Color32, Rect, ScrollArea, Shape, pos2, vec2};
use egui_kittest::Harness;

use super::{show, style};
use crate::component::tests::{builder, rests, stage};
use crate::motion;
use crate::theme::{Mode, TextScale, Theme};

/// The height of the area the tests scroll: the sheet's 96, with three times as much content, so the thumb is the
/// sheet's 32.
const AREA: f32 = 96.0;

/// A harness with a scroll area 96 high over 288 of content, motion `reduced` or full; its state is the area's inner
/// rect.
fn area(mode: Mode, reduced: bool) -> Harness<'static, Rect> {
    let theme = Theme::new(mode, TextScale::default());
    let mut harness = builder().with_size(vec2(220.0, 160.0)).build_ui_state(
        move |ui, inner: &mut Rect| {
            motion::set_reduced(ui.ctx(), reduced);
            ui.spacing_mut().scroll = style(&theme);
            stage(ui, theme.palette.surface, |ui| {
                let area = ScrollArea::vertical().id_salt("scroll-test").max_height(AREA).auto_shrink([false, true]);
                let output = show(ui, &theme, area, |ui| {
                    ui.allocate_exact_size(vec2(ui.available_width(), AREA * 3.0), egui::Sense::hover());
                });
                *inner = output.inner_rect;
            });
        },
        Rect::NOTHING,
    );
    harness.run();
    harness
}

/// The 12 points beside the area's inner rect where its bar stands.
fn strip(inner: Rect) -> Rect {
    Rect::from_x_y_ranges(inner.right()..=inner.right() + 12.0, inner.y_range())
}

/// The filled rects painted inside `strip` in the last frame: `(rect, fill)`, in paint order.
fn fills<S>(harness: &Harness<'static, S>, strip: Rect) -> Vec<(Rect, Color32)> {
    fn leaves<'a>(shape: &'a Shape, found: &mut Vec<&'a Shape>) {
        match shape {
            Shape::Vec(shapes) => shapes.iter().for_each(|shape| leaves(shape, found)),
            shape => found.push(shape),
        }
    }
    let mut flat = Vec::new();
    harness.output().shapes.iter().for_each(|clipped| leaves(&clipped.shape, &mut flat));
    flat.into_iter()
        .filter_map(|shape| match shape {
            Shape::Rect(rect) if rect.fill.a() > 0 && strip.expand(0.01).contains_rect(rect.rect) => Some((rect.rect, rect.fill)),
            _ => None,
        })
        .collect()
}

#[test]
fn the_style_is_12_wide_beside_the_content_with_a_square_thumb_and_no_fade() {
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::default());
        let scroll = style(&theme);
        assert_eq!(scroll.bar_width, 12.0);
        assert_eq!(scroll.floating_width, 12.0, "as wide at rest as under the pointer: the thumb moves inside it");
        assert_eq!(scroll.allocated_width(), 12.0, "beside the content, never over it");
        assert_eq!(scroll.fade.strength, 0.0, "no fade over the text");
        let quiet = [
            scroll.dormant_background_opacity,
            scroll.active_background_opacity,
            scroll.interact_background_opacity,
            scroll.dormant_handle_opacity,
            scroll.active_handle_opacity,
            scroll.interact_handle_opacity,
        ];
        assert!(quiet.iter().all(|opacity| *opacity == 0.0), "egui paints none of its own: {quiet:?}");
        assert_eq!(theme.style().spacing.scroll, scroll, "the theme's style holds it");
    }
}

#[test]
fn a_scroll_bar_looks_as_its_sheet_says_at_rest_hovered_and_dragged() {
    for mode in [Mode::Dark, Mode::Light] {
        let p = Theme::new(mode, TextScale::default()).palette;
        let mut harness = area(mode, true);
        let inner = *harness.state();
        let strip = strip(inner);
        assert!((inner.height() - AREA).abs() < 0.01, "{mode:?} {inner:?}");

        let rest = fills(&harness, strip);
        let thumb = Rect::from_x_y_ranges(strip.left() + 4.0..=strip.left() + 8.0, strip.top()..=strip.top() + 32.0);
        assert_eq!(rest, vec![(thumb, p.muted)], "{mode:?} rest: a 4 wide muted thumb in the middle, no track");

        harness.hover_at(strip.center());
        harness.run();
        let wide = Rect::from_x_y_ranges(strip.left() + 2.0..=strip.left() + 10.0, thumb.y_range());
        assert_eq!(fills(&harness, strip), vec![(strip, p.stage), (wide, p.muted)], "{mode:?} hover: the stage track, an 8 wide thumb");

        let grip = wide.center();
        harness.hover_at(grip);
        harness.drag_at(grip);
        harness.run();
        harness.hover_at(grip + vec2(0.0, 24.0));
        harness.run();
        let dragged = fills(&harness, strip);
        let moved = wide.translate(vec2(0.0, 24.0));
        assert_eq!(dragged, vec![(strip, p.stage), (moved, p.ink)], "{mode:?} dragging: the thumb in ink follows the pointer");
    }
}

#[test]
fn a_scroll_thumb_never_covers_the_content_nor_its_focus_edge() {
    let harness = area(Mode::Dark, true);
    let inner = *harness.state();
    let strip = strip(inner);
    assert!(strip.left() >= inner.right() - 0.01, "{inner:?} {strip:?}");
    assert!(fills(&harness, strip).iter().all(|(rect, _)| !rect.intersects(inner.shrink(0.01))));
}

#[test]
fn a_scroll_bar_rests_after_the_pointer_leaves_at_every_level_of_motion() {
    for reduced in [true, false] {
        let mut harness = area(Mode::Light, reduced);
        let strip = strip(*harness.state());
        harness.hover_at(strip.center());
        harness.run();
        harness.hover_at(pos2(1.0, 1.0));
        assert!(rests(&mut harness, 16), "reduced {reduced}: no repaint at rest");
    }
}

/// A harness whose area is 96 high over content as high as its state's first value; the second is the area's inner
/// rect.
fn growing(mode: Mode) -> Harness<'static, (f32, Rect)> {
    let theme = Theme::new(mode, TextScale::default());
    let mut harness = builder().with_size(vec2(220.0, 160.0)).build_ui_state(
        move |ui, (high, inner): &mut (f32, Rect)| {
            motion::set_reduced(ui.ctx(), true);
            ui.spacing_mut().scroll = style(&theme);
            stage(ui, theme.palette.surface, |ui| {
                let area = ScrollArea::vertical().id_salt("scroll-growing").max_height(AREA).auto_shrink([false, true]);
                let output = show(ui, &theme, area, |ui| {
                    ui.allocate_exact_size(vec2(ui.available_width(), *high), egui::Sense::hover());
                });
                *inner = output.inner_rect;
            });
        },
        (AREA / 2.0, Rect::NOTHING),
    );
    harness.run();
    harness
}

/// The bar's fills in the last frame: those of `colours` inside the 12 points right of `room`'s content edge or over
/// the content, `(rect, fill)`.
fn bar_fills(harness: &Harness<'static, (f32, Rect)>, room: Rect, colours: [Color32; 3]) -> Vec<(Rect, Color32)> {
    let wide = Rect::from_x_y_ranges(room.left()..=room.right() + 12.0, room.y_range());
    let mut found = Vec::new();
    for clipped in &harness.output().shapes {
        let mut stack = vec![&clipped.shape];
        while let Some(shape) = stack.pop() {
            match shape {
                Shape::Vec(shapes) => stack.extend(shapes.iter()),
                Shape::Rect(rect) if colours.contains(&rect.fill) && wide.expand(0.01).contains_rect(rect.rect) => found.push((rect.rect, rect.fill)),
                _ => {}
            }
        }
    }
    found
}

#[test]
fn no_frame_paints_the_bar_over_the_content_when_it_starts_or_stops_overflowing() {
    for mode in [Mode::Dark, Mode::Light] {
        let p = Theme::new(mode, TextScale::default()).palette;
        let colours = [p.muted, p.stage, p.ink];
        let mut harness = growing(mode);
        let fits = harness.state().1;
        assert!(bar_fills(&harness, fits, colours).is_empty(), "{mode:?}: no bar while it fits");

        harness.state_mut().0 = AREA * 3.0;
        for frame in 0..4 {
            harness.step();
            let inner = harness.state().1;
            let over: Vec<_> = bar_fills(&harness, fits, colours).into_iter().filter(|(rect, _)| rect.intersects(inner.shrink(0.01))).collect();
            assert!(over.is_empty(), "{mode:?} frame {frame} of overflow: the bar over the content {inner:?}: {over:?}");
        }
        harness.run();
        let inner = harness.state().1;
        assert_eq!(bar_fills(&harness, fits, colours).len(), 1, "{mode:?}: the thumb beside the content {inner:?}");

        harness.state_mut().0 = AREA / 2.0;
        for frame in 0..4 {
            harness.step();
            let inner = harness.state().1;
            let over: Vec<_> = bar_fills(&harness, fits, colours).into_iter().filter(|(rect, _)| rect.intersects(inner.shrink(0.01))).collect();
            assert!(over.is_empty(), "{mode:?} frame {frame} after overflow: {over:?}");
        }
        harness.run();
        assert!(bar_fills(&harness, fits, colours).is_empty(), "{mode:?}: no bar once it fits again");
    }
}

#[test]
fn a_viewport_area_hands_over_the_part_in_sight_and_paints_its_bar_beside_it() {
    let theme = Theme::new(Mode::Light, TextScale::default());
    let mut harness = builder().with_size(vec2(220.0, 160.0)).build_ui_state(
        move |ui, seen: &mut (Rect, Rect)| {
            ui.spacing_mut().scroll = style(&theme);
            stage(ui, theme.palette.surface, |ui| {
                let area = ScrollArea::vertical().id_salt("scroll-viewport").max_height(AREA).auto_shrink([false, true]);
                let output = super::show_viewport(ui, &theme, area, |ui, viewport| {
                    ui.set_min_height(AREA * 3.0);
                    viewport
                });
                *seen = (output.inner_rect, output.inner);
            });
        },
        (Rect::NOTHING, Rect::NOTHING),
    );
    harness.run();
    harness.run();
    let (inner, viewport) = *harness.state();
    assert_eq!(viewport.top(), 0.0, "the content's top in sight");
    assert!((viewport.height() - AREA).abs() < 0.01, "{viewport:?}");
    let strip = strip(inner);
    let fills = fills(&harness, strip);
    assert_eq!(fills.len(), 1, "a thumb at rest: {fills:?}");
    assert!(strip.left() >= inner.right() - 0.01 && fills.iter().all(|(rect, _)| !rect.intersects(inner.shrink(0.01))), "beside the content");
}
