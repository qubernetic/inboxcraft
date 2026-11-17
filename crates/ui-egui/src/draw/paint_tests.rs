// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use egui::epaint::RectShape;
use egui::{CornerRadius, Shape, pos2, vec2};

use super::{FloatEdge, float_shapes};
use crate::theme::{Mode, TextScale, Theme};

fn rects(shapes: &[Shape]) -> Vec<&RectShape> {
    shapes.iter().filter_map(|shape| if let Shape::Rect(rect) = shape { Some(rect) } else { None }).collect()
}

#[test]
fn a_keyboard_float_has_the_cyan_edge_and_the_one_shadow() {
    let rect = egui::Rect::from_min_size(pos2(40.0, 30.0), vec2(200.0, 120.0));
    for mode in [Mode::Dark, Mode::Light] {
        let theme = Theme::new(mode, TextScale::default());
        let p = theme.palette;
        for (edge, colour) in [(FloatEdge::Keyboard, p.cyan_edge), (FloatEdge::Quiet, p.line)] {
            let shapes = float_shapes(&theme, rect, edge);
            let rects = rects(&shapes);
            let shadows: Vec<_> = rects.iter().filter(|shape| shape.blur_width > 0.0).collect();
            assert_eq!(shadows.len(), 1, "{mode:?} {edge:?}: one shadow");
            let shadow = shadows[0];
            assert_eq!(shadow.blur_width, 32.0);
            assert_eq!(shadow.rect.center() - rect.center(), vec2(0.0, 12.0), "12 px down");
            assert!(
                rects.iter().any(|shape| shape.rect == rect && shape.fill == p.surface && shape.corner_radius == CornerRadius::ZERO),
                "{mode:?} {edge:?}: a square surface"
            );
            let edges: Vec<_> = rects.iter().filter(|shape| shape.stroke.width > 0.0).collect();
            assert_eq!(edges.len(), 1, "{mode:?} {edge:?}: one edge");
            assert_eq!((edges[0].stroke.width, edges[0].stroke.color), (1.0, colour), "{mode:?} {edge:?}");
            assert_eq!(edges[0].corner_radius, CornerRadius::ZERO);
            assert!(rects.iter().all(|shape| shape.corner_radius == CornerRadius::ZERO), "square");
        }
    }
}
