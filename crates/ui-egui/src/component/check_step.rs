// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! One step of an account's check, a line as a terminal prints it: a mark in brackets, the step, what it checks and a
//! note at the right end, in mono, 10 apart, 24 high; the step's column is 64 wide, each part cut with one ellipsis.
//! A failed step prints its message under the line in `minus`, under what it checks.
//!
//! The mark says how the step stands: `[    ]` waiting (`dim`), `[ .. ]` running (`warn`), `[ ok ]` passed (`plus`),
//! `[fail]` failed (`minus`), `[skip]`, `[ -- ]` not run, `[none]` no server and `[ n/a]` not built (all `muted`).
//! The running mark is characters and stands still: the lines print one by one as their steps run (motion M13), which
//! is the caller's, and nothing of a step repaints by itself.

use egui::{Role, Sense, Ui, WidgetInfo, pos2, vec2};

use crate::theme::Theme;
use crate::tr;

/// How a step of the check stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepState {
    /// Not started: its words `muted`.
    Waiting,
    /// Under way.
    Running,
    /// Passed.
    Passed,
    /// Failed: its message under it.
    Failed,
    /// Left out because a step before it failed.
    Skipped,
    /// Not run in this check.
    NotRun,
    /// The account has no server for it.
    NoServer,
    /// The app cannot do it yet.
    NotBuilt,
}

/// The room between the line's parts, in points.
const GAP: f32 = 10.0;

/// The width of the step's column, in points.
const STEP: f32 = 64.0;

/// The room between the line and the message under it, as the sheet's message line is 20 high with 4 under its text.
const MESSAGE_LINE: f32 = 20.0;

/// The mark of `state` and the colours of the mark, the step and what it checks.
fn look(theme: &Theme, state: StepState) -> (&'static str, [egui::Color32; 3]) {
    let p = &theme.palette;
    match state {
        StepState::Waiting => ("[    ]", [p.dim, p.muted, p.muted]),
        StepState::Running => ("[ .. ]", [p.warn, p.ink, p.ink]),
        StepState::Passed => ("[ ok ]", [p.plus, p.ink, p.ink]),
        StepState::Failed => ("[fail]", [p.minus, p.ink, p.ink]),
        StepState::Skipped => ("[skip]", [p.muted, p.muted, p.muted]),
        StepState::NotRun => ("[ -- ]", [p.muted, p.muted, p.muted]),
        StepState::NoServer => ("[none]", [p.muted, p.muted, p.muted]),
        StepState::NotBuilt => ("[ n/a]", [p.muted, p.muted, p.muted]),
    }
}

/// What a screen reader hears for a step: its name and how it stands.
pub(crate) fn spoken(step: &str, state: StepState) -> String {
    let stands = match state {
        StepState::Waiting => tr!("waiting"),
        StepState::Running => tr!("running"),
        StepState::Passed => tr!("passed"),
        StepState::Failed => tr!("failed"),
        StepState::Skipped => tr!("skipped"),
        StepState::NotRun => tr!("not run"),
        StepState::NoServer => tr!("no server"),
        StepState::NotBuilt => tr!("not built"),
    };
    format!("{step}: {stands}")
}

/// A step of the check across `ui`'s width: `step`, what it checks (`detail`), `note` at the right end, in `state`;
/// `message` under it when it failed.
pub fn check_step(ui: &mut Ui, theme: &Theme, step: &str, detail: &str, note: &str, state: StepState, message: Option<&str>) {
    let f = &theme.fonts;
    let p = &theme.palette;
    let (mark, [mark_ink, step_ink, detail_ink]) = look(theme, state);
    let width = ui.available_width().max(1.0);
    let line = theme.boxes().strip;
    let message = message.filter(|_| state == StepState::Failed);
    let high = if message.is_some() { line + MESSAGE_LINE } else { line };
    let (_, rect) = ui.allocate_space(vec2(width, high));
    let response = ui.interact(rect, ui.scope_id().with("check-step").with(step).with(detail), Sense::hover());
    response.widget_info(|| WidgetInfo::labeled(Role::Label, true, spoken(step, state)));
    if !ui.is_rect_visible(rect) {
        return;
    }
    let mark = super::galley(ui, mark, f.mono, mark_ink);
    let note = super::galley(ui, super::capped(&super::plain(note), super::MOST).0, f.mono_s, p.muted);
    let note_width = note.size().x.ceil();
    let step_left = rect.left() + mark.size().x.ceil() + GAP;
    let detail_left = step_left + STEP + GAP;
    let detail_width = (rect.right() - note_width - GAP - detail_left).max(1.0);
    let step = super::fitted(ui, step, f.mono, step_ink, STEP);
    let detail = super::fitted(ui, detail, f.mono, detail_ink, detail_width);
    let mono_top = (rect.top() + (line - f.mono.line) / 2.0).round();
    let small_top = (rect.top() + (line - f.mono_s.line) / 2.0).round();
    let painter = ui.painter();
    painter.galley(pos2(rect.left(), mono_top), mark, mark_ink);
    painter.galley(pos2(step_left, mono_top), step, step_ink);
    painter.galley(pos2(detail_left, mono_top), detail, detail_ink);
    painter.galley(pos2(rect.right() - note_width, small_top), note, p.muted);
    if let Some(message) = message {
        let room = (rect.right() - detail_left).max(1.0);
        let message = super::fitted(ui, message, f.mono_s, p.minus, room);
        ui.painter().galley(pos2(detail_left, rect.top() + line), message, p.minus);
    }
}

#[cfg(test)]
#[path = "check_step_tests.rs"]
mod tests;
