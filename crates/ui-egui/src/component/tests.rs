// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What every component shares: the reduced-motion flag stops all their animations.

use std::time::Duration;

use egui::accesskit::{self, NodeId};
use egui::epaint::{ColorMode, PathStroke};
use egui::{Color32, Context, Id, Key, RawInput, Rect, Shape, Stroke, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::card::card;
use super::hover::{self, Action};
use super::toast::{self, Toast};
use super::{button::IconButton, pill, popup};
use crate::iconset::IconId;
use crate::keymap::Keymap;
use crate::motion::{self, tests::FRAME};
use crate::theme::{Mode, TextScale, Theme, install_fonts};

/// A harness whose context has the bundled fonts from its first frame, stepping a frame at a time.
pub(super) fn builder<State>() -> egui_kittest::HarnessBuilder<State> {
    egui_kittest::Harness::<State>::builder().with_context_setup(install_fonts).with_step_dt(FRAME as f32).with_max_steps(60)
}

/// Tooltips at once, wherever the pointer is.
pub(super) fn quick_tooltips(ctx: &Context) {
    ctx.all_styles_mut(|style| {
        style.interaction.tooltip_delay = 0.0;
        style.interaction.show_tooltips_only_when_still = false;
    });
}

pub(super) fn theme() -> Theme {
    Theme::new(Mode::Light, TextScale::default())
}

/// Whether the last frame of `output` drew `id`: its glyph in the icon family.
pub(super) fn drew_icon(output: &egui::FullOutput, id: IconId) -> bool {
    fn texts<'a>(shape: &'a egui::Shape, found: &mut Vec<&'a egui::epaint::TextShape>) {
        match shape {
            egui::Shape::Text(text) => found.push(text),
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| texts(shape, found)),
            _ => {}
        }
    }
    let front = crate::iconset::glyph(id);
    let family = crate::theme::icon_font(1.0).family;
    let mut found = Vec::new();
    output.shapes.iter().for_each(|clipped| texts(&clipped.shape, &mut found));
    found.iter().any(|text| {
        text.galley.job.text == front.to_string() && text.galley.job.sections.iter().all(|section| section.format.font_id.family == family)
    })
}

/// One shape painted inside a widget: where, its fill, its stroke `(width, colour, dashed)` and its corner radius
/// (the largest corner; a circle's radius).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Painted {
    pub rect: Rect,
    pub fill: Color32,
    pub stroke: Option<(f32, Color32, bool)>,
    pub radius: f32,
}

/// The shapes of `shape` flattened, nested `Shape::Vec`s opened.
fn leaves<'a>(shape: &'a Shape, out: &mut Vec<&'a Shape>) {
    match shape {
        Shape::Vec(shapes) => shapes.iter().for_each(|shape| leaves(shape, out)),
        shape => out.push(shape),
    }
}

/// A stroke that shows, as `(width, colour)`.
fn shown_stroke(stroke: Stroke) -> Option<(f32, Color32)> {
    (stroke.width > 0.0 && stroke.color.a() > 0).then_some((stroke.width, stroke.color))
}

/// A path stroke of one colour that shows.
fn shown_path_stroke(stroke: &PathStroke) -> Option<(f32, Color32)> {
    match stroke.color {
        ColorMode::Solid(colour) if stroke.width > 0.0 && colour.a() > 0 => Some((stroke.width, colour)),
        _ => None,
    }
}

/// A shape as [`Painted`], when it is one this helper reads (rects, circles, ellipses, paths, lines); text, meshes
/// and callbacks are left out.
fn as_painted(shape: &Shape) -> Option<Painted> {
    let painted = |rect, fill, stroke: Option<(f32, Color32)>, radius| Painted { rect, fill, stroke: stroke.map(|(w, c)| (w, c, false)), radius };
    match shape {
        Shape::Rect(rect) => {
            let radius = rect.corner_radius;
            let largest = f32::from(radius.nw.max(radius.ne).max(radius.sw).max(radius.se));
            Some(painted(rect.rect, rect.fill, shown_stroke(rect.stroke), largest))
        }
        Shape::Circle(circle) => {
            let rect = Rect::from_center_size(circle.center, vec2(circle.radius * 2.0, circle.radius * 2.0));
            Some(painted(rect, circle.fill, shown_stroke(circle.stroke), circle.radius))
        }
        Shape::Ellipse(ellipse) => {
            let rect = Rect::from_center_size(ellipse.center, ellipse.radius * 2.0);
            Some(painted(rect, ellipse.fill, shown_stroke(ellipse.stroke), ellipse.radius.min_elem()))
        }
        Shape::Path(path) => {
            let rect = Rect::from_points(&path.points);
            Some(painted(rect, path.fill, shown_path_stroke(&path.stroke), 0.0))
        }
        Shape::LineSegment { points, stroke } => {
            Some(painted(Rect::from_two_pos(points[0], points[1]), Color32::TRANSPARENT, shown_stroke(*stroke), 0.0))
        }
        _ => None,
    }
}

/// The line segments of `shapes` that run on with the same stroke, a gap between each and the next (as
/// `Shape::dashed_line` paints them), merged into one dashed [`Painted`]; every other shape as it is.
fn merge_dashes(shapes: Vec<(&Shape, Painted)>) -> Vec<Painted> {
    let mut out: Vec<Painted> = Vec::new();
    // Where the last segment kept ended, and its stroke.
    let mut last_end: Option<(egui::Pos2, Stroke)> = None;
    for (shape, painted) in shapes {
        if painted.fill.a() == 0 && painted.stroke.is_none() {
            last_end = None;
            continue;
        }
        let Shape::LineSegment { points, stroke } = shape else {
            last_end = None;
            out.push(painted);
            continue;
        };
        let runs_on = last_end.is_some_and(|(end, before)| before == *stroke && end.distance(points[0]) > 0.01);
        match out.last_mut() {
            Some(previous) if runs_on => {
                previous.rect = previous.rect.union(painted.rect);
                if let Some(stroke) = previous.stroke.as_mut() {
                    stroke.2 = true;
                }
            }
            _ => out.push(painted),
        }
        last_end = Some((points[1], *stroke));
    }
    out
}

/// The shapes the last frame painted inside the widget labelled `label` (its AccessKit rect, a point of slack for
/// strokes): fills and strokes that show, in paint order. Text is not among them; a run of dashes is one shape.
///
/// # Panics
///
/// When no widget or more than one has `label`.
pub(crate) fn painted<State>(harness: &Harness<'_, State>, label: &str) -> Vec<Painted> {
    let rect = harness.get_by_label(label).rect().expand(1.0);
    let mut flat = Vec::new();
    harness.output().shapes.iter().for_each(|clipped| leaves(&clipped.shape, &mut flat));
    let inside = flat
        .into_iter()
        .filter_map(|shape| as_painted(shape).map(|painted| (shape, painted)))
        .filter(|(_, painted)| rect.contains_rect(painted.rect));
    merge_dashes(inside.collect())
}

/// The node focused in the harness's AccessKit tree.
fn focused<State>(harness: &Harness<'_, State>) -> Option<NodeId> {
    harness.root().children_recursive().find(|node| node.accesskit_node().is_focused()).map(|node| node.accesskit_node().locate().0)
}

/// The labels of the pressable controls (a click or focus action, not disabled, not hidden; static text, which egui
/// makes focusable for selecting, left out) Tab never focuses: Tab is pressed
/// until the focus comes back to a control it has been on (at most 2 000 presses), two frames after each press. A
/// control without a label is named by its role.
pub(crate) fn keyboard_walk<State>(harness: &mut Harness<'_, State>) -> Vec<String> {
    let pressable: Vec<(NodeId, String)> = harness
        .root()
        .children_recursive()
        .filter(|node| {
            let node = node.accesskit_node();
            let data = node.data();
            (data.supports_action(accesskit::Action::Click) || data.supports_action(accesskit::Action::Focus))
                && !node.is_disabled()
                && !node.is_hidden()
                && node.role() != accesskit::Role::Label
        })
        .map(|node| {
            let node = node.accesskit_node();
            (node.locate().0, node.label().unwrap_or_else(|| format!("{:?} without a name", node.role())))
        })
        .collect();
    let mut reached: Vec<NodeId> = Vec::new();
    for _ in 0..2000 {
        harness.key_press(Key::Tab);
        harness.step();
        harness.step();
        match focused(harness) {
            Some(id) if reached.contains(&id) => break,
            Some(id) => reached.push(id),
            None => {}
        }
    }
    pressable.into_iter().filter(|(id, _)| !reached.contains(id)).map(|(_, label)| label).collect()
}

/// Whether the page rests: after `frames` frames without input, the last asks for no repaint sooner than a second
/// (a clock that ticks each minute rests; an animation or a repaint every frame does not).
pub(crate) fn rests<State>(harness: &mut Harness<'_, State>, frames: usize) -> bool {
    harness.run_steps(frames);
    let delay = harness.output().viewport_output.values().map(|viewport| viewport.repaint_delay).min().unwrap_or(Duration::MAX);
    delay >= Duration::from_secs(1)
}

/// Every component in one frame, `on` deciding what is shown, open, hovered or selected.
fn everything(ui: &mut egui::Ui, theme: &Theme, keymap: &Keymap, on: bool) {
    let ctx = ui.ctx().clone();
    card(ui, theme, Id::unique("card"), "Card", on, |ui| ui.label("Body"));
    IconButton::new(IconId::Done, "Done").show(ui, theme, keymap);
    pill::pill(ui, theme, "Pill", on);
    let row = egui::Rect::from_min_size(pos2(0.0, 300.0), vec2(400.0, 40.0));
    hover::actions(ui, theme, keymap, Id::unique("hover"), row, on, &[Action { icon: IconId::Done, name: "Done", action: None, unavailable: None }]);
    if on {
        toast::show(&ctx, theme, keymap, Id::unique("toast"), &Toast { text: "Toast", undo: None }, pos2(200.0, 400.0));
        popup::show(&ctx, theme, Id::unique("popup"), pos2(10.0, 10.0), 200.0, |ui| ui.label("Item"));
    }
}

/// How soon a repaint is wanted two frames after everything was switched on, with motion reduced or not.
fn repaint_after_switching(reduce: bool) -> Duration {
    let ctx = Context::default();
    install_fonts(&ctx);
    let theme = theme();
    let keymap = Keymap::default();
    let mut delay = Duration::MAX;
    for n in 0..8 {
        let input = RawInput {
            time: Some(f64::from(n) * FRAME),
            predicted_dt: FRAME as f32,
            screen_rect: Some(egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(600.0, 500.0))),
            ..RawInput::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            motion::set_reduced(ui.ctx(), reduce);
            everything(ui, &theme, &keymap, n >= 4);
        });
        output.textures_delta.clear();
        if n == 6 {
            delay = output.viewport_output.values().map(|viewport| viewport.repaint_delay).min().unwrap_or(Duration::MAX);
        }
    }
    delay
}

#[test]
fn the_reduced_motion_flag_stops_every_animation_of_every_component() {
    assert!(repaint_after_switching(false) < Duration::from_millis(100), "with motion, things are still moving");
    assert!(repaint_after_switching(true) > Duration::from_secs(1), "with reduced motion, everything has landed");
}
