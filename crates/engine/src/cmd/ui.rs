// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `ui.*.set`: the appearance choices, one command each, all `safe`: they change how the window looks and nothing
//! that leaves the computer. Each answers the whole [`Appearance`] after the change; the front end draws with it and
//! keeps it on disk.

use serde::Deserialize;
use serde_json::{Value, json};

use crate::appearance::{Appearance, Motion, TEXT_SCALES, ThemeMode, WindowFrame};
use crate::params::decode;
use crate::{AgentClass, CommandSpec, Context, EngineError, Outcome};

/// The commands of this module, in the registry.
pub(crate) const COMMANDS: &[CommandSpec] = &[FRAME, MOTION, TEXT_SCALE, THEME];

/// The parameter names these commands read, for `hostile_params_never_panic`.
#[cfg(test)]
pub(crate) const FIELDS: &[&str] = &["mode", "percent", "reduce", "frame"];

/// `ui.theme.set`: light, dark or the system's.
pub(crate) const THEME: CommandSpec =
    CommandSpec { id: "ui.theme.set", class: Some(AgentClass::Safe), summary: "Show light, dark or what the system shows", run: theme };

/// `ui.text_scale.set`: the text scale.
pub(crate) const TEXT_SCALE: CommandSpec =
    CommandSpec { id: "ui.text_scale.set", class: Some(AgentClass::Safe), summary: "Draw every text at 90 to 130 per cent", run: text_scale };

/// `ui.motion.set`: reduced motion.
pub(crate) const MOTION: CommandSpec =
    CommandSpec { id: "ui.motion.set", class: Some(AgentClass::Safe), summary: "Reduce motion, let it run, or follow the system", run: motion };

/// `ui.frame.set`: who draws the window's frame.
pub(crate) const FRAME: CommandSpec = CommandSpec {
    id: "ui.frame.set",
    class: Some(AgentClass::Safe),
    summary: "Draw the window frame: the system's, controls in the top row, or a slim strip",
    run: frame,
};

/// The outcome of a change to the appearance: the whole appearance as the answer.
fn changed(appearance: Appearance) -> Outcome {
    Outcome::new(json!({ "appearance": appearance.to_value() })).with_appearance(appearance)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeParams {
    /// `system`, `light` or `dark`.
    mode: String,
}

fn theme(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: ThemeParams = decode(THEME.id, params)?;
    let theme = ThemeMode::named(&params.mode).ok_or_else(|| EngineError::bad_params(THEME.id, "mode is system, light or dark"))?;
    Ok(changed(Appearance { theme, ..context.state().appearance }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TextScaleParams {
    /// 90, 100, 110, 120 or 130.
    percent: u16,
}

fn text_scale(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: TextScaleParams = decode(TEXT_SCALE.id, params)?;
    if !TEXT_SCALES.contains(&params.percent) {
        return Err(EngineError::bad_params(TEXT_SCALE.id, "percent is 90, 100, 110, 120 or 130"));
    }
    Ok(changed(Appearance { text_scale: params.percent, ..context.state().appearance }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MotionParams {
    /// `on`, `off` or `system`.
    reduce: String,
}

fn motion(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: MotionParams = decode(MOTION.id, params)?;
    let motion = match params.reduce.as_str() {
        "on" => Motion::Reduce,
        "off" => Motion::Full,
        "system" => Motion::System,
        _ => return Err(EngineError::bad_params(MOTION.id, "reduce is on, off or system")),
    };
    Ok(changed(Appearance { motion, ..context.state().appearance }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FrameParams {
    /// `system`, `top_row` or `strip`.
    frame: String,
}

fn frame(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: FrameParams = decode(FRAME.id, params)?;
    let frame = WindowFrame::named(&params.frame).ok_or_else(|| EngineError::bad_params(FRAME.id, "frame is system, top_row or strip"))?;
    let now = context.state().appearance;
    Ok(changed(Appearance { frame, own_frame: Appearance::own_of(frame, Some(now.own_frame)), ..now }))
}
