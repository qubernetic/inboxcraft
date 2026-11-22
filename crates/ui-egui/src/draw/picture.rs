// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! A picture kept from a frame that is drawn again (`docs/design.md`, "Motion"): a command of the frame changed what
//! the reader's place or the whole window shows, so the frame is discarded and drawn again with the change (the owner
//! never waits), and what the discarded pass painted plays the movement over the new one.
//!
//! - M7, a message opens: at full the old content stands under the wipe's edge as it comes down ([`Form::Under`]); at
//!   reduced it fades out over the message in 125 ms ([`Form::Fade`]).
//! - M10, send: at full the composer folds onto the status line, 300 ms then 125 ms ([`Form::Fold`]); at reduced it
//!   fades out over the reader in 125 ms.
//! - M14, dark and light: the old colours fade out over the new ones, 300 ms at full and 125 ms at reduced.
//!
//! At off nothing is kept. A picture is paint only: nothing in it takes input, and a key or a press ends it.

use egui::epaint::ClippedShape;
use egui::{Color32, Context, Id, LayerId, Order, Rect, pos2, vec2};

/// The shapes past which a picture is not kept: a frame paints far fewer; one that painted more changes at once.
pub(crate) const MOST_SHAPES: usize = 20_000;

/// The `fast` token, in seconds.
pub(crate) const FAST: f64 = 0.125;

/// The `medium` token, in seconds.
pub(crate) const MEDIUM: f64 = 0.3;

/// How a kept picture moves.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Form {
    /// It fades out over the new picture, at an even pace.
    Fade,
    /// It stands under an edge that comes down from the top on the design's curve (the reader's wipe).
    Under,
    /// Its top falls on the curve until one line of `line` high is left at its foot ([`MEDIUM`]), then that line goes
    /// down into `onto` ([`FAST`]), the status line.
    Fold { onto: Rect, line: f32 },
}

/// A kept picture playing its movement.
pub(crate) struct Picture {
    /// The time of the first frame it painted in.
    started: Option<f64>,
    /// What the discarded pass painted, clipped to `rect`.
    old: Vec<ClippedShape>,
    /// Where it was painted.
    rect: Rect,
    /// The ground under it.
    ground: Color32,
    /// How it moves.
    form: Form,
    /// How long it moves, in seconds.
    seconds: f64,
    /// The layer it paints in.
    layer: LayerId,
}

/// Whether `clipped` is painted inside `rect`.
pub(crate) fn inside(clipped: &ClippedShape, rect: Rect) -> bool {
    let seen = clipped.shape.visual_bounding_rect().intersect(clipped.clip_rect);
    seen.is_positive() && rect.contains(seen.center())
}

/// What this pass painted inside `rect` in `layers`, in their order, each clipped to `rect`; none past
/// [`MOST_SHAPES`].
pub(crate) fn shapes(ctx: &Context, rect: Rect, layers: &[LayerId]) -> Option<Vec<ClippedShape>> {
    ctx.graphics(|graphics| {
        let mut old = Vec::new();
        for layer in layers {
            let Some(list) = graphics.get(*layer) else { continue };
            for clipped in list.all_entries().filter(|clipped| inside(clipped, rect)) {
                if old.len() >= MOST_SHAPES {
                    return None;
                }
                let mut kept = clipped.clone();
                kept.clip_rect = kept.clip_rect.intersect(rect);
                old.push(kept);
            }
        }
        Some(old)
    })
}

/// The layers painted through a painter of their own, not as areas (the memory does not list them), as egui paints
/// them: after the areas of their order.
const PAINTED_ONLY: [(Order, &str); 2] = [(Order::Foreground, "reader-scan"), (Order::Tooltip, "window-outline")];

/// Every layer of the window in the order they are painted: the window's own first, the tooltips last.
pub(crate) fn every_layer(ctx: &Context) -> Vec<LayerId> {
    let known: Vec<LayerId> = ctx.memory(|memory| memory.layer_ids().filter(|layer| *layer != LayerId::background()).collect());
    let mut layers = Vec::with_capacity(known.len().saturating_add(1 + PAINTED_ONLY.len()));
    for order in [Order::Background, Order::Middle, Order::Foreground, Order::Tooltip, Order::Debug] {
        if order == Order::Background {
            layers.push(LayerId::background());
        }
        layers.extend(known.iter().filter(|layer| layer.order == order));
        let painted = PAINTED_ONLY.iter().filter(|(at, _)| *at == order).map(|(_, name)| LayerId::new(order, Id::unique(name)));
        layers.extend(painted.filter(|layer| !known.contains(layer)));
    }
    layers
}

impl Picture {
    /// Keeps what this pass painted inside `rect` in `layers` to move as `form` for `seconds` on the ground `ground`,
    /// painted in the window's own layer (over the body, under every float) or, `over_all`, over everything.
    pub(crate) fn keep(ctx: &Context, rect: Rect, layers: &[LayerId], ground: Color32, form: Form, seconds: f64, over_all: bool) -> Option<Self> {
        let old = shapes(ctx, rect, layers)?;
        let layer = if over_all { LayerId::new(Order::Tooltip, Id::unique("inboxcraft-kept-picture")) } else { LayerId::background() };
        Some(Self { started: None, old, rect, ground, form, seconds, layer })
    }

    /// The rect it was kept from.
    pub(crate) fn rect(&self) -> Rect {
        self.rect
    }

    /// Paints this frame of the movement, cut to `clip` (where the new content stands; the picture's own rect where
    /// none is given); false when it is over (nothing painted). A repaint is asked for while it runs.
    pub(crate) fn paint(&mut self, ctx: &Context, clip: Option<Rect>) -> bool {
        let now = ctx.input(|input| input.time);
        let age = now - *self.started.get_or_insert(now);
        if !age.is_finite() || age < 0.0 || !self.seconds.is_finite() || age >= self.seconds {
            return false;
        }
        ctx.request_repaint();
        let painter = ctx.layer_painter(self.layer);
        let clip = clip.unwrap_or(self.rect);
        match self.form {
            Form::Fade => {
                let mut faded = painter.with_clip_rect(clip);
                faded.set_opacity(1.0 - crate::motion::part(age, 0.0, self.seconds));
                self.draw(&faded, clip, vec2(0.0, 0.0));
            }
            Form::Under => {
                let came = crate::motion::ease(crate::motion::part(age, 0.0, self.seconds));
                let edge = (clip.top() + clip.height() * came).round();
                let under = Rect::from_min_max(pos2(clip.left(), edge), clip.max);
                if under.is_positive() {
                    self.draw(&painter.with_clip_rect(under), under, vec2(0.0, 0.0));
                }
            }
            Form::Fold { onto, line } => {
                let rect = self.rect;
                let line = line.clamp(1.0, rect.height().max(1.0));
                let rest = rect.bottom() - line;
                let top = if age < MEDIUM {
                    rect.top() + (rest - rect.top()) * crate::motion::ease(crate::motion::part(age, 0.0, MEDIUM))
                } else {
                    rest + (onto.top() - rest).max(0.0) * crate::motion::ease(crate::motion::part(age, MEDIUM, FAST))
                };
                let top = top.round();
                let bottom = if age < MEDIUM { rect.bottom() } else { (top + line).min(onto.top()) };
                let cut = Rect::from_min_max(pos2(rect.left(), top), pos2(rect.right(), bottom));
                if cut.is_positive() {
                    self.draw(&painter.with_clip_rect(cut), cut, vec2(0.0, top - rect.top()));
                }
            }
        }
        true
    }

    /// The ground over `ground_rect` and the kept shapes moved by `by`, through `painter`.
    fn draw(&self, painter: &egui::Painter, ground_rect: Rect, by: egui::Vec2) {
        painter.rect_filled(ground_rect, 0.0, self.ground);
        for clipped in &self.old {
            let mut shape = clipped.shape.clone();
            if by != vec2(0.0, 0.0) {
                shape.translate(by);
            }
            let clip = clipped.clip_rect.translate(by).intersect(painter.clip_rect());
            painter.with_clip_rect(clip).add(shape);
        }
    }
}

#[cfg(test)]
#[path = "reader_motion_tests.rs"]
mod reader_motion_tests;

#[cfg(test)]
#[path = "send_motion_tests.rs"]
mod send_motion_tests;

#[cfg(test)]
#[path = "mode_motion_tests.rs"]
mod mode_motion_tests;
