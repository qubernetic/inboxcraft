// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Coming soon: everything that is designed is shown, and what is not built yet is drawn faint inside [`soon`].
//! There it has `muted` text and dashed `muted` edges, never a fade; it cannot be pressed, takes no keyboard focus
//! and reads as disabled to a screen reader. Each component asks [`is_soon`] before it senses clicks or focus. The
//! surface carries the mark once, the [`soon_tag`], on its head, its row or its control.

use egui::{Color32, Response, Role, Sense, Ui, UiBuilder, UiStackInfo, WidgetInfo, pos2, vec2};

use crate::draw::paint;
use crate::theme::Theme;
use crate::tr;

/// The tag on the [`Ui`] that [`soon`] opens; every `Ui` inside it has it in its stack.
const SOON: &str = "inboxcraft-coming-soon";

/// The widest the tag's words get before they end with an ellipsis, in points.
const TAG_WIDEST: f32 = 240.0;

/// Draws `add` as not built yet: inside it every component senses hover only, reports itself disabled, paints its
/// text in `muted` and its edge dashed in `muted`, all at full strength. What `add` gave.
pub fn soon<R>(ui: &mut Ui, theme: &Theme, add: impl FnOnce(&mut Ui) -> R) -> R {
    let builder = UiBuilder::new().ui_stack_info(UiStackInfo::default().with_tag(SOON));
    ui.scope_builder(builder, |ui| {
        let visuals = ui.visuals_mut();
        // Disabled is drawn faint by colour, never by fading what is painted.
        visuals.disabled_alpha = 1.0;
        visuals.override_text_color = Some(theme.palette.muted);
        ui.disable();
        add(ui)
    })
    .inner
}

/// Whether `ui` lies inside [`soon`]: a component then senses hover only and draws itself unavailable.
#[must_use]
pub fn is_soon(ui: &Ui) -> bool {
    ui.stack().iter().any(|frame| frame.tags().contains(SOON))
}

/// The sense a component takes in `ui`: `wanted`, or hover only inside [`soon`].
pub(crate) fn sense(ui: &Ui, wanted: Sense) -> Sense {
    if is_soon(ui) { Sense::hover() } else { wanted }
}

/// The colour a component paints its text in inside `ui`: `colour`, or `muted` inside [`soon`].
pub(crate) fn ink(ui: &Ui, theme: &Theme, colour: Color32) -> Color32 {
    if is_soon(ui) { theme.palette.muted } else { colour }
}

/// The coming-soon mark: "coming soon" in `mono_s` and `muted` in a dashed `muted` edge with nothing inside, as
/// high as a tag. It is never a button.
pub fn soon_tag(ui: &mut Ui, theme: &Theme) -> Response {
    let p = &theme.palette;
    let text = tr!("coming soon");
    let pad = theme.spacing.s3 / 2.0;
    let galley = super::cut(ui, text, theme.fonts.mono_s, p.muted, TAG_WIDEST);
    let (rect, response) = ui.allocate_exact_size(vec2(galley.size().x + pad * 2.0, theme.boxes().chip), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Label, ui.is_enabled(), text));
    if ui.is_rect_visible(rect) {
        paint::dashed(ui.painter(), rect, p.muted);
        ui.painter().galley(pos2(rect.left() + pad, rect.center().y - galley.size().y / 2.0), galley, p.muted);
    }
    response
}

#[cfg(test)]
#[path = "soon_tests.rs"]
mod tests;
