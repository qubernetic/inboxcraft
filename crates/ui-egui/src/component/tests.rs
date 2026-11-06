// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! What every component shares: the reduced-motion flag stops all their animations.

use std::time::Duration;

use egui::{Context, Id, RawInput, pos2, vec2};

use super::card::card;
use super::hover::{self, Action};
use super::toast::{self, Toast};
use super::{button::IconButton, pill, popup};
use crate::glass::Under;
use crate::iconset::{IconId, IconSet, Style};
use crate::keymap::Keymap;
use crate::motion::{self, tests::FRAME};
use crate::theme::{Mode, Preset, Theme, install_fonts};

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
    Theme::new(Mode::Light, &Preset::default())
}

/// Whether the last frame of `output` drew `id` in `style` from the default icon set: its front glyph in its font.
pub(super) fn drew_icon(output: &egui::FullOutput, id: IconId, style: Style) -> bool {
    fn texts<'a>(shape: &'a egui::Shape, found: &mut Vec<&'a egui::epaint::TextShape>) {
        match shape {
            egui::Shape::Text(text) => found.push(text),
            egui::Shape::Vec(shapes) => shapes.iter().for_each(|shape| texts(shape, found)),
            _ => {}
        }
    }
    let glyph = IconSet::default().glyph(id, style);
    let Some((front, _)) = glyph.layers.last() else { return false };
    let family = glyph.face.id(1.0).family;
    let mut found = Vec::new();
    output.shapes.iter().for_each(|clipped| texts(&clipped.shape, &mut found));
    found.iter().any(|text| {
        text.galley.job.text == front.to_string() && text.galley.job.sections.iter().all(|section| section.format.font_id.family == family)
    })
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
        popup::show(&ctx, theme, Id::unique("popup"), pos2(10.0, 10.0), Under::Sharp, 200.0, |ui| ui.label("Item"));
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
