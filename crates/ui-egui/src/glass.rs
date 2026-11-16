// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Glass without blur. egui cannot blur what lies behind a shape, so glass is faked: large soft colour fields are
//! painted once behind the window ([`backdrop`], a gradient mesh of the preset's three stops), and the panes are
//! translucent layers over them with a light top edge and a soft shadow ([`Layers`]).
//!
//! Readability beats glass. The layers go under everything else the window draws, so translucency is never laid
//! over text. Everything that pops up over content ([`float`]) is on one solid surface with an edge and a shadow, so
//! text under it never shows through. Nothing here asks for a repaint: the fields and shadows are pictures kept by
//! [`crate::gradient`] and only drawn again when the window is.

use egui::layers::ShapeIdx;
use egui::{Color32, Frame, Margin, Painter, Rect, Shape, Stroke, StrokeKind, Ui, pos2, vec2};

use crate::gradient::{self, Field, Gradient, Stop};
use crate::theme::Theme;

/// The colour fields of `theme`: the first stop glowing at the top towards the middle, the middle stop at the top
/// right, the last from the bottom right; the left, where the panes start, stays calm.
#[must_use]
pub fn fields(theme: &Theme) -> Gradient {
    let [first, middle, last] = theme.palette.colour_fields;
    Gradient::Fields {
        base: theme.palette.background,
        fields: vec![
            Field { centre: pos2(0.5, 0.12), radius: 0.45, colour: first },
            Field { centre: pos2(0.86, 0.2), radius: 0.4, colour: middle },
            Field { centre: pos2(0.94, 1.0), radius: 0.48, colour: last },
        ],
    }
}

/// Paints the colour fields of `theme` over `rect`, the window behind everything.
pub fn backdrop(painter: &Painter, theme: &Theme, rect: Rect) {
    backdrop_rounded(painter, theme, rect, 0.0);
}

/// [`backdrop`] with its corners rounded by `corner`, for a window whose corners are round.
pub fn backdrop_rounded(painter: &Painter, theme: &Theme, rect: Rect, corner: impl Into<egui::CornerRadius>) {
    gradient::fill(painter, rect, corner, &fields(theme));
}

/// The soft shadow around a layer of `rect`.
fn shadow(painter: &Painter, theme: &Theme, rect: Rect) -> Shape {
    let blur = theme.spacing.shadow;
    let drop = blur / 3.0;
    let around = Rect::from_min_max(rect.min - vec2(blur, blur), rect.max + vec2(blur, blur + drop));
    let gradient = Gradient::Shadow { colour: theme.palette.shadow, blur, drop, corner: theme.spacing.card };
    gradient::filled(painter, around, 0.0, &gradient)
}

/// The light top edge of a layer of `rect`, fading in and out at its ends.
fn highlight(painter: &Painter, theme: &Theme, rect: Rect) -> Shape {
    let corner = theme.spacing.card;
    let y = rect.top() + 0.5;
    let light = theme.palette.glass_highlight;
    let gradient = Gradient::Linear {
        from: pos2(0.0, 0.5),
        to: pos2(1.0, 0.5),
        stops: vec![Stop::new(0.0, Color32::TRANSPARENT), Stop::new(0.25, light), Stop::new(0.75, light), Stop::new(1.0, Color32::TRANSPARENT)],
    };
    gradient::stroked(painter, vec![pos2(rect.left() + corner / 2.0, y), pos2(rect.right() - corner / 2.0, y)], 1.0, &gradient)
}

/// The fill and the edge of a layer of `rect` in `colour`.
fn pane(theme: &Theme, rect: Rect, colour: Color32) -> [Shape; 2] {
    let corner = theme.spacing.card;
    [Shape::rect_filled(rect, corner, colour), Shape::rect_stroke(rect, corner, Stroke::new(1.0, theme.palette.glass_edge), StrokeKind::Inside)]
}

/// The glass layers of one frame. [`Layers::begin`] keeps a place right after the colour fields; each
/// [`Layers::card`] draws its content and notes its rectangle; [`Layers::finish`] puts every layer's shadow, then
/// every layer's glass, into the kept place, under all the content.
#[derive(Debug)]
pub struct Layers {
    slot: ShapeIdx,
    rects: Vec<Rect>,
}

impl Layers {
    /// Keeps the place for the layers in `painter`'s list, after what it holds now.
    #[must_use]
    pub fn begin(painter: &Painter) -> Self {
        Self { slot: painter.add(Shape::Noop), rects: Vec::new() }
    }

    /// A glass layer over the whole of `ui`, with `add` drawing its content `inner` from its edges.
    pub fn card<R>(&mut self, ui: &mut Ui, inner: Margin, add: impl FnOnce(&mut Ui) -> R) -> R {
        self.rects.push(ui.max_rect());
        Frame::new().inner_margin(inner).show(ui, add).inner
    }

    /// The layers noted so far.
    #[must_use]
    pub fn rects(&self) -> &[Rect] {
        &self.rects
    }

    /// Puts the layers into the kept place of `painter`'s list.
    pub fn finish(self, painter: &Painter, theme: &Theme) {
        let mut shapes: Vec<Shape> = self.rects.iter().map(|rect| shadow(painter, theme, *rect)).collect();
        for rect in &self.rects {
            shapes.extend(pane(theme, *rect, theme.palette.glass));
            shapes.push(highlight(painter, theme, *rect));
        }
        painter.set(self.slot, Shape::Vec(shapes));
    }
}

/// Paints a float over `rect`: its shadow, the solid surface, its edge and its light top edge.
pub fn float(painter: &Painter, theme: &Theme, rect: Rect) {
    painter.extend(float_shapes(painter, theme, rect));
}

/// The shapes of [`float`], for a place kept before the float's content was drawn.
#[must_use]
pub fn float_shapes(painter: &Painter, theme: &Theme, rect: Rect) -> Vec<Shape> {
    let [fill, edge] = pane(theme, rect, theme.palette.float);
    vec![shadow(painter, theme, rect), fill, edge, highlight(painter, theme, rect)]
}

#[cfg(test)]
#[path = "glass_tests.rs"]
mod tests;
