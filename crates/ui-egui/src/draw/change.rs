// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A change of place (`docs/design.md`, "Motion", M12): the index's place changed (Home, Mail, Calendar, Later,
//! Settings), not a mailbox, a view or a range. The command runs in the frame of its key, and that frame draws the new
//! place: the frame that drew the old one is discarded once its body was kept ([`keep`]).
//!
//! At full the old body's text scrambles out from the right over [`OUT`] (the old body stands over the new one), then
//! the new body's text resolves from the left over [`IN`]. At reduced the old body fades out over the new one over
//! [`OUT`]. At off the new place is there at once. A key or a press during a change ends it; nothing of the old body
//! takes input, it is paint only.

use egui::epaint::ClippedShape;
use egui::{Color32, Context, LayerId, Rect, Shape};

use crate::motion::Level;
use crate::scramble::{self, Side};

/// The `fast` token, in seconds: the old body scrambling (full) or fading (reduced) out.
pub(crate) const OUT: f64 = 0.125;

/// The `medium` token, in seconds: the new body's text resolving at full.
pub(crate) const IN: f64 = 0.3;

/// The shapes past which a body is not kept: what one frame paints in a body is far fewer; a body that painted more
/// changes at once.
pub(crate) const MOST_SHAPES: usize = 20_000;

/// A change of place under way.
pub(crate) struct Change {
    /// The time of the first frame it painted, the frame of its key that showed the new place.
    started: Option<f64>,
    /// The old body as it was painted, clipped to the body.
    old: Vec<ClippedShape>,
    /// The body's rect.
    body: Rect,
    /// The level it runs at: full or reduced.
    level: Level,
}

/// The layer a body paints in: the window's own, under every float (a floating composer, a menu), which a change
/// neither keeps, nor paints over, nor scrambles.
fn body_layers() -> [LayerId; 1] {
    [LayerId::background()]
}

/// Keeps what this frame painted in `body` (the old place, drawn before its change's command ran) for a change at
/// `level`; none at off, or when the body painted more than [`MOST_SHAPES`].
pub(crate) fn keep(ctx: &Context, body: Rect, level: Level) -> Option<Change> {
    if level == Level::Off {
        return None;
    }
    let old = super::picture::shapes(ctx, body, &body_layers())?;
    Some(Change { started: None, old, body, level })
}

impl Change {
    /// How long it runs in all.
    fn length(&self) -> f64 {
        if self.level == Level::Full { OUT + IN } else { OUT }
    }

    /// Paints this frame of the change over the body drawn (the new place), on the ground `ground`; false when it is
    /// over (nothing painted). A repaint is asked for while it runs.
    pub(crate) fn paint(&mut self, ctx: &Context, ground: Color32) -> bool {
        let now = ctx.input(|input| input.time);
        let age = now - *self.started.get_or_insert(now);
        if !age.is_finite() || age >= self.length() || age < 0.0 {
            return false;
        }
        ctx.request_repaint();
        // Shares of a fraction of a second: the casts lose nothing that shows.
        let out = ((age / OUT) as f32).clamp(0.0, 1.0);
        // Last in the window's own layer: over the body, under every float.
        let painter = ctx.layer_painter(LayerId::background());
        match (self.level, age < OUT) {
            (Level::Full, true) => {
                // The old body stands over the new one, its text going to glyphs from the right.
                painter.with_clip_rect(self.body).rect_filled(self.body, 0.0, ground);
                for clipped in &self.old {
                    let mut shape = clipped.shape.clone();
                    scramble::shape(&painter, &mut shape, out, Side::Right);
                    painter.with_clip_rect(clipped.clip_rect).add(shape);
                }
            }
            (Level::Full, false) => {
                let share = (((age - OUT) / IN) as f32).clamp(0.0, 1.0);
                self.resolve(ctx, &painter, share);
            }
            _ => {
                // Reduced: the old body over the new one, fading out.
                let mut faded = painter.with_clip_rect(self.body);
                faded.set_opacity(1.0 - out);
                faded.rect_filled(self.body, 0.0, ground);
                for clipped in &self.old {
                    faded.with_clip_rect(clipped.clip_rect).add(clipped.shape.clone());
                }
            }
        }
        true
    }

    /// The new body's text, resolved from the left to `share`.
    fn resolve(&self, ctx: &Context, painter: &egui::Painter, share: f32) {
        let layers = body_layers();
        // Read first, laid out outside the paint's lock, then written back.
        let texts: Vec<(LayerId, usize, Shape)> = ctx.graphics(|graphics| {
            let mut texts = Vec::new();
            for layer in &layers {
                let Some(list) = graphics.get(*layer) else { continue };
                for (index, clipped) in list.all_entries().enumerate() {
                    if texts.len() >= MOST_SHAPES {
                        return texts;
                    }
                    if matches!(clipped.shape, Shape::Text(_) | Shape::Vec(_)) && super::picture::inside(clipped, self.body) {
                        texts.push((*layer, index, clipped.shape.clone()));
                    }
                }
            }
            texts
        });
        let mut changed = Vec::with_capacity(texts.len());
        for (layer, index, mut shape) in texts {
            scramble::shape(painter, &mut shape, share, Side::Left);
            changed.push((layer, index, shape));
        }
        ctx.graphics_mut(|graphics| {
            for (layer, index, shape) in changed {
                if let Some(list) = graphics.get_mut(layer) {
                    list.mutate_shape(egui::layers::ShapeIdx(index), |clipped| clipped.shape = shape);
                }
            }
        });
    }
}

#[cfg(test)]
#[path = "change_tests.rs"]
mod tests;
