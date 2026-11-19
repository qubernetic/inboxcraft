// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `ui.*.set`: the appearance choices, one command each, all `safe`: they change how the window looks and nothing
//! that leaves the computer. Each answers the whole [`Appearance`] after the change; the front end draws with it and
//! keeps it on disk.
//!
//! `ui.settings.open` (`safe`) asks the window to show the settings, on the page its optional `page` names (one of
//! [`crate::shown::SETTINGS_PAGES`]); a `page` that names none is no page, never an error. It changes nothing; its
//! answer's `window` says whether a window takes the ask (`false` headless, where nothing is shown).
//!
//! `ui.motion.set` takes `level` (`full`, `reduced` or `off`); the older `reduce` (`on`, `off` or `system`) still
//! answers for one release, and `level` wins when both are given.

use serde::Deserialize;
use serde_json::{Value, json};

use crate::appearance::{Appearance, MotionLevel, TEXT_SCALES, ThemeMode, WindowFrame};
use crate::params::decode;
use crate::shown::{self, SettingsAsked};
use crate::{AgentClass, Caller, CommandSpec, Context, EngineError, Outcome};

/// The commands of this module, in the registry.
pub(crate) const COMMANDS: &[CommandSpec] = &[FRAME, MOTION, SETTINGS, TEXT_SCALE, THEME];

/// The parameter names these commands read, for `hostile_params_never_panic`.
#[cfg(test)]
pub(crate) const FIELDS: &[&str] = &["mode", "percent", "level", "reduce", "frame", "page"];

/// `ui.theme.set`: light, dark or the system's.
pub(crate) const THEME: CommandSpec =
    CommandSpec { id: "ui.theme.set", class: Some(AgentClass::Safe), summary: "Show light, dark or what the system shows", run: theme };

/// `ui.text_scale.set`: the text scale.
pub(crate) const TEXT_SCALE: CommandSpec =
    CommandSpec { id: "ui.text_scale.set", class: Some(AgentClass::Safe), summary: "Draw every text at 90 to 130 per cent", run: text_scale };

/// `ui.motion.set`: how much the window moves.
pub(crate) const MOTION: CommandSpec =
    CommandSpec { id: "ui.motion.set", class: Some(AgentClass::Safe), summary: "Let the window move fully, calmly or not at all", run: motion };

/// `ui.frame.set`: who draws the window's frame.
pub(crate) const FRAME: CommandSpec = CommandSpec {
    id: "ui.frame.set",
    class: Some(AgentClass::Safe),
    summary: "Draw the window frame: the system's, controls in the top row, or a slim strip",
    run: frame,
};

/// `ui.settings.open`: the settings, on a page when one is named.
pub(crate) const SETTINGS: CommandSpec = CommandSpec {
    id: "ui.settings.open",
    class: Some(AgentClass::Safe),
    summary: "Show the settings, on the page named; without one as their key does",
    run: settings_open,
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
    /// `full`, `reduced` or `off`.
    level: Option<String>,
    /// Deprecated, for one release: `on` (off), `off` (full) or `system` (the default level, not chosen).
    reduce: Option<String>,
}

/// The level and whether the owner chose it, from `ui.motion.set`'s parameters: `level` when given, else the old
/// `reduce`. Either one that is not one of its words is refused, even beside a good `level`.
fn motion_level(params: &MotionParams) -> Result<(MotionLevel, bool), EngineError> {
    let refused = |what| EngineError::bad_params(MOTION.id, what);
    let level = params.level.as_deref().map(|level| MotionLevel::named(level).ok_or_else(|| refused("level is full, reduced or off"))).transpose()?;
    let old = params
        .reduce
        .as_deref()
        .map(|reduce| match reduce {
            "on" => Ok((MotionLevel::Off, true)),
            "off" => Ok((MotionLevel::Full, true)),
            "system" => Ok((MotionLevel::Reduced, false)),
            _ => Err(refused("reduce is on, off or system")),
        })
        .transpose()?;
    level.map(|level| (level, true)).or(old).ok_or_else(|| refused("level is full, reduced or off"))
}

fn motion(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: MotionParams = decode(MOTION.id, params)?;
    let (level, chosen) = motion_level(&params)?;
    Ok(changed(context.state().appearance.with_motion(level, chosen)))
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

/// Reads `page` alone, by hand: a `page` that is no page's id is no page, and a refusal repeats nothing it was sent. The
/// answer is the page asked for and whether a window takes the ask; it says nothing of what the window shows, so a
/// window that passes over an agent's ask (the owner typing in the account form) answers the same.
fn settings_open(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let given = match params {
        Value::Null => None,
        Value::Object(fields) => {
            if fields.keys().any(|field| field != "page") {
                return Err(EngineError::bad_params(SETTINGS.id, "the only parameter is page"));
            }
            fields.get("page").filter(|page| !page.is_null())
        }
        _ => return Err(EngineError::bad_params(SETTINGS.id, "the parameters are not a JSON object")),
    };
    let page = shown::page(given);
    let asked = SettingsAsked { page, bare: given.is_none(), by_agent: context.caller() == Caller::Agent };
    Ok(Outcome::new(json!({ "page": page, "window": context.state().window })).with_settings(asked))
}
