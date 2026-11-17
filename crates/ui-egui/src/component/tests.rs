// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What every component shares: the reduced-motion flag stops all their animations.

use std::time::Duration;

use egui::accesskit::{self, NodeId};
use egui::epaint::{ClippedShape, TextShape};
use egui::epaint::{ColorMode, PathStroke};
use egui::{Color32, Context, Id, Key, LayerId, RawInput, Rect, Shape, Stroke, StrokeKind, Vec2, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT as _, Queryable as _};

use super::card::card;
use super::hover::{self, Action};
use super::toast::{self, Toast};
use super::{button::IconButton, pill, popup};
use crate::draw::paint::{DashedEdges, dashed_edges_id};
use crate::iconset::IconId;
use crate::keymap::Keymap;
use crate::motion::{self, tests::FRAME};
use crate::theme::{Mode, TextScale, Theme, install_fonts};

/// The context setup of [`builder`]: the bundled fonts, and the paint recorded for [`painted`].
fn setup(ctx: &Context) {
    install_fonts(ctx);
    record_layers(ctx);
}

/// A harness whose context has the bundled fonts from its first frame and records its paint, stepping a frame at a
/// time.
pub(super) fn builder<State>() -> egui_kittest::HarnessBuilder<State> {
    egui_kittest::Harness::<State>::builder().with_context_setup(setup).with_step_dt(FRAME as f32).with_max_steps(60)
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

/// What one shape painted for a widget looks like.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Look {
    /// A rect, or a circle, ellipse, path or line read as one: its fill, its stroke `(width, colour)` and where the
    /// stroke lies, its corner radius (the largest corner; a circle's radius; 0 for paths and lines), and whether it
    /// is an edge [`paint::dashed`](crate::draw::paint::dashed) drew.
    Rect { fill: Color32, stroke: Option<(f32, Color32)>, stroke_kind: StrokeKind, radius: f32, dashed: bool },
    /// Text: its colour, its font family's name (`Proportional`, `Monospace` or the name given), its size and the
    /// height of its first row, which is the style's line height.
    Text { text: String, colour: Color32, family: String, size: f32, row_height: f32 },
    /// A shadow (a rect with a blur): where it lies against the widget's own rect (`offset` of the centres, `spread`
    /// of the half widths), its blur and colour.
    Shadow { offset: Vec2, blur: f32, spread: f32, colour: Color32 },
}

/// One shape painted for a widget: its rect, whether it lies inside the widget's own rect (`false`: in the 4 points
/// around it, where a focus ring stands), whether its clip rect cuts it, whether it is text that reaches past the
/// 4 points around the widget (`overflows`; always `false` for other shapes), and its look.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Painted {
    pub rect: Rect,
    pub inside: bool,
    pub clipped: bool,
    pub overflows: bool,
    pub look: Look,
}

/// The margin [`stage`] keeps around a widget, in points: more than the 4 points [`painted`] looks around it.
const STAGE_MARGIN: i8 = 16;

/// Places a widget for [`painted`]: paints `ground` over the room `ui` has and adds the widget with `add` inside a
/// 16 point margin, so the ground always strictly holds the widget and the 4 points around it, and the harness's
/// own background is never read as the widget's fill.
pub(crate) fn stage<R>(ui: &mut egui::Ui, ground: Color32, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.painter().rect_filled(ui.available_rect_before_wrap(), 0.0, ground);
    egui::Frame::NONE.inner_margin(egui::Margin::same(STAGE_MARGIN)).show(ui, add).inner
}

/// What [`painted`] found for a widget: the fill it stands on (`ground`) and its own shapes in paint order.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Paint {
    pub ground: Option<Color32>,
    pub shapes: Vec<Painted>,
}

impl Paint {
    /// The rect looks, `(rect, look)`, in paint order.
    pub(crate) fn rects(&self) -> impl Iterator<Item = &Painted> {
        self.shapes.iter().filter(|shape| matches!(shape.look, Look::Rect { .. }))
    }
}

/// How far around a widget its shapes may lie: a focus ring stands 3 points out (2 clear and its 1 px line).
const AROUND: f32 = 4.0;

/// One frame's paint as [`record_layers`] keeps it: each layer's shapes in paint order, each widget's layer by its
/// AccessKit id, and the edges [`paint::dashed`](crate::draw::paint::dashed) drew.
#[derive(Clone, Default)]
struct Recorded {
    layers: Vec<(LayerId, Vec<ClippedShape>)>,
    widgets: Vec<(NodeId, LayerId)>,
    dashed: Vec<(LayerId, Rect, Color32, f32)>,
}

fn recorded_id() -> Id {
    Id::unique("component-tests-recorded-paint")
}

/// Keeps, at the end of every pass of `ctx`, what each layer painted, for [`painted`]. [`builder`] does it; another
/// harness calls it in its context setup.
pub(crate) fn record_layers(ctx: &Context) {
    ctx.on_end_pass(
        "component-tests-record",
        std::sync::Arc::new(|ui: &mut egui::Ui| {
            let ctx = ui.ctx().clone();
            let widgets: Vec<(NodeId, LayerId)> = ctx.viewport(|viewport| {
                viewport.this_pass.widgets.layers().flat_map(|(layer, rects)| rects.iter().map(|rect| (rect.id.accesskit_id(), *layer))).collect()
            });
            let mut layer_ids: Vec<LayerId> = ctx.memory(|memory| memory.areas().visible_layer_ids().into_iter().collect());
            for layer in widgets.iter().map(|(_, layer)| *layer).chain([LayerId::background()]) {
                if !layer_ids.contains(&layer) {
                    layer_ids.push(layer);
                }
            }
            let layers = ctx.graphics(|graphics| {
                layer_ids.iter().filter_map(|layer| graphics.get(*layer).map(|list| (*layer, list.all_entries().cloned().collect()))).collect()
            });
            let pass = ctx.cumulative_pass_nr();
            let dashed = ctx
                .data(|data| data.get_temp::<DashedEdges>(dashed_edges_id()))
                .filter(|edges| edges.0 == pass)
                .map(|edges| edges.1)
                .unwrap_or_default();
            ctx.data_mut(|data| data.insert_temp(recorded_id(), Recorded { layers, widgets, dashed }));
        }),
    );
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

/// The colour text is drawn in: the override, else the first section's colour, else the painter's fallback.
fn text_colour(text: &TextShape) -> Color32 {
    let section = text.galley.job.sections.first().map(|section| section.format.color).filter(|colour| *colour != Color32::PLACEHOLDER);
    text.override_text_color.or(section).unwrap_or(text.fallback_color)
}

/// A shape's rect and look, against the widget's own rect `own`; `None` for meshes other than a float's shadow,
/// callbacks and what shows nothing.
fn read(shape: &Shape, own: Rect) -> Option<(Rect, Look)> {
    let flat = |fill: Color32, stroke: Option<(f32, Color32)>, stroke_kind, radius| {
        (fill.a() > 0 || stroke.is_some()).then_some(Look::Rect { fill, stroke, stroke_kind, radius, dashed: false })
    };
    match shape {
        Shape::Rect(rect) if rect.blur_width > 0.0 => {
            let shadow = rect.rect;
            let offset = shadow.center() - own.center();
            let spread = (shadow.width() - own.width()) / 2.0;
            Some((shadow, Look::Shadow { offset, blur: rect.blur_width, spread, colour: rect.fill }))
        }
        Shape::Mesh(mesh) => crate::draw::paint::shadow_box(mesh).map(|shadow| {
            // The float's own shadow mesh: read as the shadow token it draws.
            let token = crate::theme::FLOAT_SHADOW;
            let look = Look::Shadow {
                offset: shadow.center() - own.center(),
                blur: f32::from(token.blur),
                spread: (shadow.width() - own.width()) / 2.0,
                colour: token.color,
            };
            (shadow, look)
        }),
        Shape::Rect(rect) => {
            let corner = rect.corner_radius;
            let largest = f32::from(corner.nw.max(corner.ne).max(corner.sw).max(corner.se));
            flat(rect.fill, shown_stroke(rect.stroke), rect.stroke_kind, largest).map(|look| (rect.rect, look))
        }
        Shape::Circle(circle) => {
            let rect = Rect::from_center_size(circle.center, vec2(circle.radius * 2.0, circle.radius * 2.0));
            flat(circle.fill, shown_stroke(circle.stroke), StrokeKind::Middle, circle.radius).map(|look| (rect, look))
        }
        Shape::Ellipse(ellipse) => {
            let rect = Rect::from_center_size(ellipse.center, ellipse.radius * 2.0);
            flat(ellipse.fill, shown_stroke(ellipse.stroke), StrokeKind::Middle, ellipse.radius.min_elem()).map(|look| (rect, look))
        }
        Shape::Path(path) => {
            flat(path.fill, shown_path_stroke(&path.stroke), StrokeKind::Middle, 0.0).map(|look| (Rect::from_points(&path.points), look))
        }
        Shape::LineSegment { points, stroke } => {
            flat(Color32::TRANSPARENT, shown_stroke(*stroke), StrokeKind::Middle, 0.0).map(|look| (Rect::from_two_pos(points[0], points[1]), look))
        }
        Shape::Text(text) => {
            let font = text.galley.job.sections.first().map(|section| section.format.font_id.clone()).unwrap_or_default();
            let row_height = text.galley.rows.first().map_or(0.0, |row| row.rect().height());
            let look = Look::Text {
                text: text.galley.job.text.clone(),
                colour: text_colour(text),
                family: font.family.to_string(),
                size: font.size,
                row_height,
            };
            // The run itself: a fraction of a pixel laid out before it (`paint::run_at`) is not text.
            let lead = text.galley.job.sections.first().map_or(0.0, |section| section.leading_space.max(0.0));
            let mut rect = text.galley.rect.translate(text.pos.to_vec2());
            rect.min.x = (rect.min.x + lead).min(rect.max.x);
            Some((rect, look))
        }
        _ => None,
    }
}

/// What the last frame painted for the widget labelled `label`, read from its own layer only (a popup, tooltip or
/// toast over it is not its paint):
/// - `shapes`: the shapes inside its AccessKit rect grown by 4 points, in paint order, each marked `inside` its own
///   rect or not (a focus ring is not), and `clipped` when its clip rect cuts it; shapes clipped away wholly are left
///   out. A shadow is kept when its centre lies in the widget's rect, however far its blur reaches.
/// - `ground`: the fill of the last shape before the widget's first one whose rect strictly holds the grown rect
///   (the parent it stands on, or the [`stage`]); such shapes are not in `shapes`. Always `Some`: without one
///   `painted` panics and asks for a [`stage`].
/// - Text is kept whenever its galley touches the widget's own rect, marked `overflows` when it reaches past the
///   grown rect; it is never dropped.
///
/// The line segments of an edge [`paint::dashed`](crate::draw::paint::dashed) drew are one shape with
/// `dashed: true`; nothing else is ever dashed.
///
/// # Panics
///
/// When no widget or more than one has `label`, when the harness does not [`record_layers`] (build it with
/// [`builder`]), or when no ground holds the widget (place it with [`stage`]).
pub(crate) fn painted<State>(harness: &Harness<'_, State>, label: &str) -> Paint {
    let node = harness.get_by_label(label);
    let own = node.rect();
    let grown = own.expand(AROUND);
    let id = node.accesskit_node().locate().0;
    let recorded = harness
        .ctx
        .data(|data| data.get_temp::<Recorded>(recorded_id()))
        .unwrap_or_else(|| panic!("the harness records no layers: build it with component::tests::builder"));
    let layer =
        recorded.widgets.iter().find(|(widget, _)| *widget == id).map(|(_, layer)| *layer).unwrap_or_else(|| panic!("{label}: no widget rect"));
    let list: Vec<&ClippedShape> = recorded.layers.iter().filter(|(of, _)| *of == layer).flat_map(|(_, shapes)| shapes.iter()).collect();
    let dashes: Vec<(Rect, Color32, f32)> = recorded
        .dashed
        .iter()
        .filter(|(of, rect, ..)| *of == layer && grown.contains_rect(*rect))
        .map(|(_, rect, colour, radius)| (*rect, *colour, *radius))
        .collect();
    // A dashed edge is one mesh of pixel squares, which `read` skips; `dashes` stands for it.
    let (mut grounds, mut shapes) = (Vec::new(), Vec::new());
    for clipped in list {
        let mut flat = Vec::new();
        leaves(&clipped.shape, &mut flat);
        for shape in flat {
            let Some((rect, look)) = read(shape, own) else { continue };
            let visible = clipped.clip_rect.intersect(rect);
            let thin = rect.width() == 0.0 || rect.height() == 0.0;
            if !(visible.is_positive() || (thin && clipped.clip_rect.expand(0.5).contains_rect(rect))) {
                continue;
            }
            let clipped = !clipped.clip_rect.expand(0.01).contains_rect(rect);
            if let Look::Text { .. } = look {
                // Text is never dropped while it touches the widget: text that leaves the grown rect is marked.
                if rect.intersects(own) {
                    shapes.push(Painted { rect, inside: own.expand(0.01).contains_rect(rect), clipped, overflows: !grown.contains_rect(rect), look });
                }
                continue;
            }
            let shadow = matches!(look, Look::Shadow { .. }) && own.contains(rect.center());
            if !shadow && rect.contains_rect(grown) && rect != grown {
                if let Look::Rect { fill, .. } = look
                    && shapes.is_empty()
                    && fill.a() > 0
                {
                    grounds.push(fill);
                }
                continue;
            }
            if shadow || grown.contains_rect(rect) {
                shapes.push(Painted { rect, inside: own.expand(0.01).contains_rect(rect), clipped, overflows: false, look });
            }
        }
    }
    for (rect, colour, radius) in dashes {
        let look = Look::Rect { fill: Color32::TRANSPARENT, stroke: Some((1.0, colour)), stroke_kind: StrokeKind::Inside, radius, dashed: true };
        shapes.push(Painted { rect, inside: own.expand(0.01).contains_rect(rect), clipped: false, overflows: false, look });
    }
    let Some(ground) = grounds.last().copied() else {
        panic!("{label}: no ground under the widget; place it with component::tests::stage so its own fill is never confused with the page's")
    };
    Paint { ground: Some(ground), shapes }
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
    // A radio button without a Tab stop of its own is reached through its group (a segmented control's option: the
    // group takes the focus and the arrow keys move the choice).
    let through_group: Vec<NodeId> = harness
        .root()
        .children_recursive()
        .filter(|node| {
            let data = node.accesskit_node();
            data.role() == accesskit::Role::RadioButton
                && !data.data().supports_action(accesskit::Action::Focus)
                && std::iter::successors(node.parent(), |up| up.parent())
                    .find(|up| up.accesskit_node().role() == accesskit::Role::RadioGroup)
                    .is_some_and(|group| reached.contains(&group.accesskit_node().locate().0))
        })
        .map(|node| node.accesskit_node().locate().0)
        .collect();
    pressable.into_iter().filter(|(id, _)| !reached.contains(id) && !through_group.contains(id)).map(|(_, label)| label).collect()
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

/// Draws, at the end of every pass of `ctx`, an icon button for `action` inside [`soon`](super::soon::soon), and with
/// `normal` a usable one for it too: a window test's way to put a coming-soon control into the app's frame.
pub(crate) fn offer_soon(ctx: &Context, action: &'static str, normal: bool) {
    ctx.on_end_pass(
        "component-tests-offer-soon",
        std::sync::Arc::new(move |ui: &mut egui::Ui| {
            let theme = theme();
            let keymap = Keymap::default();
            let button = IconButton::new(IconId::Keyboard, "Later control").action(action);
            super::soon::soon(ui, &theme, |ui| button.show(ui, &theme, &keymap));
            if normal {
                IconButton::new(IconId::Keyboard, "Usable control").action(action).show(ui, &theme, &keymap);
            }
        }),
    );
}

/// A widget's own look in one state, read from a [`Paint`] for the state tests of the sheets.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Own {
    /// Its fill: a shape inside it whose fill covers its whole box.
    pub fill: Option<Color32>,
    /// Its edge over its whole box, drawn inside: `(width, colour, dashed)`.
    pub edge: Option<(f32, Color32, bool)>,
    /// A stroke outside its box (the focus ring): the outer bound of the line, its width and colour.
    pub ring: Option<(Rect, f32, Color32)>,
    /// Its texts, glyphs included: `(text, colour, family, size)`.
    pub texts: Vec<(String, Color32, String, f32)>,
    /// The colours of the level hairlines and thin level fills inside it (a link's underline).
    pub lines: Vec<Color32>,
    /// The largest corner radius it paints.
    pub radius: f32,
    /// Whether a text of it reaches past the 4 points around it.
    pub overflows: bool,
}

/// [`Own`] of the widget whose own rect is `rect`, from `paint`.
pub(crate) fn own(paint: &Paint, rect: Rect) -> Own {
    let near = |a: Rect, b: Rect| (a.min - b.min).length() < 0.51 && (a.max - b.max).length() < 0.51;
    let mut found = Own { fill: None, edge: None, ring: None, texts: Vec::new(), lines: Vec::new(), radius: 0.0, overflows: false };
    for shape in &paint.shapes {
        found.overflows |= shape.overflows;
        match &shape.look {
            Look::Rect { fill, stroke, stroke_kind, radius, dashed } => {
                found.radius = found.radius.max(*radius);
                if let (Some((_, colour)), StrokeKind::Middle, false) = (stroke, stroke_kind, dashed)
                    && shape.inside
                    && shape.rect.height() == 0.0
                {
                    found.lines.push(*colour);
                    continue;
                }
                // A thin level fill inside it is a line too (a link's underline, the font's thickness).
                if fill.a() > 0 && stroke.is_none() && shape.inside && shape.rect.height() < 1.0 && shape.rect.width() > 4.0 {
                    found.lines.push(*fill);
                    continue;
                }
                if fill.a() > 0 && near(shape.rect, rect) {
                    found.fill = Some(*fill);
                }
                if let Some((width, colour)) = stroke {
                    let outer = match stroke_kind {
                        StrokeKind::Inside => shape.rect,
                        StrokeKind::Middle => shape.rect.expand(width / 2.0),
                        StrokeKind::Outside => shape.rect.expand(*width),
                    };
                    if near(outer, rect) {
                        found.edge = Some((*width, *colour, *dashed));
                    } else if !shape.inside && outer.contains_rect(rect) {
                        found.ring = Some((outer, *width, *colour));
                    }
                }
            }
            Look::Text { text, colour, family, size, .. } => found.texts.push((text.clone(), *colour, family.clone(), *size)),
            Look::Shadow { .. } => {}
        }
    }
    found
}

/// Brings the widget labelled `label` in `harness` into `shown` the way a person would: the pointer over it, the
/// keyboard focus on it, or the pointer held down on it; then runs the frames.
pub(crate) fn bring<State>(harness: &mut Harness<'_, State>, label: &str, shown: super::Shown) {
    let node = harness.get_by_label(label);
    let centre = node.rect().center();
    match shown {
        super::Shown::Rest => {}
        super::Shown::Hover => node.hover(),
        super::Shown::Focus => node.focus(),
        super::Shown::Pressed => {
            harness.hover_at(centre);
            harness.drag_at(centre);
        }
    }
    harness.run();
}

/// A harness that draws `add` on a [`stage`] of `ground` in `theme`, motion reduced, as large as `size`.
pub(crate) fn staged(
    theme: Theme,
    ground: fn(&crate::theme::Palette) -> Color32,
    size: Vec2,
    add: impl Fn(&mut egui::Ui, &Theme) + 'static,
) -> Harness<'static> {
    let mut harness = builder().with_size(size).build_ui(move |ui| {
        motion::set_reduced(ui.ctx(), true);
        stage(ui, ground(theme.palette), |ui| add(ui, &theme));
    });
    harness.run();
    harness
}

/// [`staged`] with motion `reduced` or full: frames step one [`FRAME`] at a time.
pub(crate) fn staged_motion(
    theme: Theme,
    reduced: bool,
    ground: fn(&crate::theme::Palette) -> Color32,
    size: Vec2,
    add: impl Fn(&mut egui::Ui, &Theme) + 'static,
) -> Harness<'static> {
    let mut harness = builder().with_size(size).build_ui(move |ui| {
        motion::set_reduced(ui.ctx(), reduced);
        stage(ui, ground(theme.palette), |ui| add(ui, &theme));
    });
    harness.run();
    harness
}

/// The looks of the widget `label` frame by frame as the pointer comes onto it, `frames` frames, then leaves it,
/// `frames` frames more: `(on, off)`, the first of each the frame the pointer moved in.
pub(crate) fn hover_frames(harness: &mut Harness<'static>, label: &str, frames: usize) -> (Vec<Own>, Vec<Own>) {
    let look = |harness: &mut Harness<'static>| {
        harness.step();
        let rect = harness.get_by_label(label).rect();
        own(&painted(harness, label), rect)
    };
    harness.get_by_label(label).hover();
    let on = (0..frames).map(|_| look(harness)).collect();
    harness.hover_at(pos2(1.0, 1.0));
    let off = (0..frames).map(|_| look(harness)).collect();
    (on, off)
}

/// Whether `mid` lies strictly between `a` and `b`: every channel within them, and neither end itself.
pub(crate) fn strictly_between(mid: Color32, a: Color32, b: Color32) -> bool {
    let within = (0..4).all(|i| {
        let (m, x, y) = (mid.to_array()[i], a.to_array()[i], b.to_array()[i]);
        x.min(y) <= m && m <= x.max(y)
    });
    within && mid != a && mid != b
}

/// The family name of the icons.
pub(crate) fn icons() -> String {
    crate::theme::icon_font(1.0).family.to_string()
}

/// The centre of the ink in `picture` inside the whole pixels `x` and `y` (start inclusive, end exclusive): every
/// pixel weighted by how far it stands from `ground` towards `ink`, a pixel's centre at its index + 0.5.
pub(crate) fn ink_centre(
    picture: &crate::shot::Picture,
    x: std::ops::Range<usize>,
    y: std::ops::Range<usize>,
    ground: Color32,
    ink: Color32,
) -> Option<(f32, f32)> {
    let channels = |colour: Color32| [f32::from(colour.r()), f32::from(colour.g()), f32::from(colour.b())];
    let (g, i) = (channels(ground), channels(ink));
    let towards = [i[0] - g[0], i[1] - g[1], i[2] - g[2]];
    let length = towards.iter().map(|d| d * d).sum::<f32>();
    if length <= 0.0 {
        return None;
    }
    let (mut mass, mut sx, mut sy) = (0.0_f32, 0.0_f32, 0.0_f32);
    for py in y {
        for px in x.clone() {
            let p = channels(picture.pixel(px, py)?);
            let cover = ((p[0] - g[0]) * towards[0] + (p[1] - g[1]) * towards[1] + (p[2] - g[2]) * towards[2]) / length;
            let cover = cover.clamp(0.0, 1.0);
            mass += cover;
            sx += cover * (px as f32 + 0.5);
            sy += cover * (py as f32 + 0.5);
        }
    }
    (mass > 0.0).then(|| (sx / mass, sy / mass))
}

#[test]
fn a_cut_line_ends_its_last_word_with_the_ellipsis_and_no_space_before_it() {
    let theme = theme();
    let mut harness = builder().build_ui_state(
        move |ui, shown: &mut Vec<String>| {
            let font = theme.fonts.button;
            let show = |text: &str, width: f32| -> String {
                let galley = super::cut(ui, text, font, theme.palette.ink, width);
                galley.rows.iter().flat_map(|row| row.glyphs.iter().map(|glyph| glyph.chr)).collect()
            };
            *shown = vec![show("Zusammenfassung des Posteingangs", 136.0), show("Keep a copy", 400.0), show("ab cd", 1.0)];
        },
        Vec::new(),
    );
    harness.run();
    let shown = harness.state();
    assert_eq!(shown[0], "Zusammenfassung…", "the design drops the space the cut leaves before the ellipsis");
    assert_eq!(shown[1], "Keep a copy", "a name that fits is not touched");
    assert!(shown[2].chars().count() <= 2, "{:?}", shown[2]);
}
