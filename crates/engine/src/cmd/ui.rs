// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `ui.*.set`: the appearance choices, one command each, all `safe`: they change how the window looks and nothing
//! that leaves the computer. Each answers the whole [`Appearance`] after the change; the front end draws with it and
//! keeps it on disk.

use serde::Deserialize;
use serde_json::{Value, json};

use crate::appearance::{AA, Accent, Appearance, CustomAccent, DARK_SURFACE, Icons, Motion, TEXT_SCALES, ThemeMode, Typeface, contrast};
use crate::params::decode;
use crate::{AgentClass, CommandSpec, Context, EngineError, Outcome};

/// `ui.theme.set`: light, dark or the system's.
pub(crate) const THEME: CommandSpec =
    CommandSpec { id: "ui.theme.set", class: Some(AgentClass::Safe), summary: "Show light, dark or what the system shows", run: theme };

/// `ui.accent.set`: a shipped preset or four colours of the owner's own.
pub(crate) const ACCENT: CommandSpec = CommandSpec {
    id: "ui.accent.set",
    class: Some(AgentClass::Safe),
    summary: "Take the colours from a preset or from four colours of your own",
    run: accent,
};

/// `ui.font.set`: the typeface.
pub(crate) const FONT: CommandSpec =
    CommandSpec { id: "ui.font.set", class: Some(AgentClass::Safe), summary: "Draw every text in Figtree or Inter", run: font };

/// `ui.icons.set`: the icon set.
pub(crate) const ICONS: CommandSpec =
    CommandSpec { id: "ui.icons.set", class: Some(AgentClass::Safe), summary: "Draw the icons from Phosphor or Lucide", run: icons };

/// `ui.text_scale.set`: the text scale.
pub(crate) const TEXT_SCALE: CommandSpec =
    CommandSpec { id: "ui.text_scale.set", class: Some(AgentClass::Safe), summary: "Draw every text at 90 to 130 per cent", run: text_scale };

/// `ui.motion.set`: reduced motion.
pub(crate) const MOTION: CommandSpec =
    CommandSpec { id: "ui.motion.set", class: Some(AgentClass::Safe), summary: "Reduce motion, let it run, or follow the system", run: motion };

/// The outcome of a change to the appearance: the whole appearance as the answer, `more` added to it.
fn changed(appearance: Appearance, more: Value) -> Outcome {
    let mut value = json!({ "appearance": appearance.to_value() });
    if let (Some(value), Value::Object(more)) = (value.as_object_mut(), more) {
        value.extend(more);
    }
    Outcome::new(value).with_appearance(appearance)
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
    Ok(changed(Appearance { theme, ..context.state().appearance }, json!({})))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AccentParams {
    /// A shipped preset's id.
    preset: Option<String>,
    /// Four `#rrggbb` colours: the gradient's three stops and the accent.
    custom: Option<Vec<String>>,
}

fn accent(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: AccentParams = decode(ACCENT.id, params)?;
    let current = context.state().appearance;
    match (params.preset, params.custom) {
        (Some(id), None) => {
            let accent = Accent::preset(&id).ok_or_else(|| EngineError::bad_params(ACCENT.id, "preset is electric, aurora or citrus"))?;
            Ok(changed(Appearance { accent, ..current }, json!({ "nudged": false })))
        }
        (None, Some(colours)) => {
            let colours: Vec<&str> = colours.iter().map(String::as_str).collect();
            let asked = CustomAccent::from_hex(&colours)
                .ok_or_else(|| EngineError::bad_params(ACCENT.id, "custom is four #rrggbb colours: three gradient stops and the accent"))?;
            let custom = asked.readable();
            let ratio = f64::from((contrast(custom.accent, DARK_SURFACE) * 100.0).floor() / 100.0);
            let more = if custom == asked {
                json!({ "nudged": false, "contrast": ratio })
            } else {
                json!({
                    "nudged": true,
                    "requested": asked.accent.hex(),
                    "contrast": ratio,
                    "reason": format!("the accent missed WCAG AA ({AA}:1) as text on dark surfaces and was made lighter"),
                })
            };
            Ok(changed(Appearance { accent: Accent::Custom(custom), ..current }, more))
        }
        _ => Err(EngineError::bad_params(ACCENT.id, "give either preset or custom")),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FontParams {
    /// `figtree` or `inter`.
    font: String,
}

fn font(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: FontParams = decode(FONT.id, params)?;
    let font = Typeface::named(&params.font).ok_or_else(|| EngineError::bad_params(FONT.id, "font is figtree or inter"))?;
    Ok(changed(Appearance { font, ..context.state().appearance }, json!({})))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IconsParams {
    /// `phosphor` or `lucide`.
    icons: String,
}

fn icons(context: &Context<'_>, params: &Value) -> Result<Outcome, EngineError> {
    let params: IconsParams = decode(ICONS.id, params)?;
    let icons = Icons::named(&params.icons).ok_or_else(|| EngineError::bad_params(ICONS.id, "icons is phosphor or lucide"))?;
    Ok(changed(Appearance { icons, ..context.state().appearance }, json!({})))
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
    Ok(changed(Appearance { text_scale: params.percent, ..context.state().appearance }, json!({})))
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
    Ok(changed(Appearance { motion, ..context.state().appearance }, json!({})))
}
