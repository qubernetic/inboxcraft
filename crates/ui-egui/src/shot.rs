// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Headless pictures of the window for `ui_shot`: egui's shapes tessellated into triangles and filled on the CPU,
//! written as PNG. No window, no GPU and no network; the sample mailbox at a fixed time, so a picture only changes
//! when the look or the content does.
//!
//! A script is JSON lines, one picture per line:
//!
//! ```json
//! {"out": "docs/images/list-light.png", "mode": "light", "size": [1200, 760]}
//! {"out": "docs/images/list-dark.png", "mode": "dark", "narrow": 2, "open": {"role": "inbox"}, "search": "logo"}
//! ```
//!
//! `mode` is `light` or `dark`, `size` the window in points (1200 by 760 when left out), `scale` the pixels per
//! point (1 to 4, 1 when left out), `narrow` an account id, `open` a `role` or a `folder` id, `search` the text in
//! the search field and `select` the subject of the row to select. `onboard` shows the add-account page over the
//! sample: `form` (typed by hand), `detected` (after Detect), `checking` (half the steps in), `passed` or `failed`
//! (the certificate refused); the check is scripted and nothing connects.

use std::collections::HashMap;
use std::io::Write;
use std::sync::Arc;

use egui::epaint::{ClippedPrimitive, ImageData, Mesh, Primitive, TextureId};
use egui::{Color32, Pos2, RawInput, Rect, Vec2, pos2, vec2};
use inboxcraft_model::{CheckStep, ErrorKind, Role, StepStatus};
use inboxcraft_time::{FakeClock, Timestamp};
use serde::Deserialize;

use crate::mailbox::{Mailbox, Place};
use crate::onboard::{Keychain, Proposal, Proposed, Secret, Security, Source, Suggestion};
use crate::save::NoSave;
use crate::theme::Mode;
use crate::workers::Scripted;
use crate::{InboxApp, UiError, sample};

/// The largest picture side in pixels.
pub const MAX_SIDE: usize = 4096;

/// The time the sample mailbox is shown at: 2026-11-05T09:00:00Z.
pub const SHOT_TIME: i64 = 1_793_869_200;

/// What a script line asks to open.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "lowercase")]
pub enum Open {
    /// A unified view by role name (`inbox`, `sent` ...).
    Role(String),
    /// A folder by id.
    Folder(u64),
}

/// One picture of a script.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Shot {
    /// Where the PNG goes.
    pub out: String,
    /// `light` or `dark`.
    pub mode: String,
    /// The window in points.
    #[serde(default)]
    pub size: Option<[f32; 2]>,
    /// Pixels per point.
    #[serde(default)]
    pub scale: Option<f32>,
    /// The account to narrow to.
    #[serde(default)]
    pub narrow: Option<u64>,
    /// The view or folder to open.
    #[serde(default)]
    pub open: Option<Open>,
    /// The search text.
    #[serde(default)]
    pub search: Option<String>,
    /// The subject of the row to select, which opens it in the reader.
    #[serde(default)]
    pub select: Option<String>,
    /// The remote pictures of the selected message: `once`, or `always` for its sender (drawn by the sample, never
    /// fetched).
    #[serde(default)]
    pub pictures: Option<String>,
    /// Whether the reader fills the window.
    #[serde(default)]
    pub full_width: Option<bool>,
    /// Whether the selected message shows its full header.
    #[serde(default)]
    pub details: Option<bool>,
    /// Whether the selected message shows its quotes.
    #[serde(default)]
    pub quotes: Option<bool>,
    /// The add-account page: `form`, `detected`, `checking`, `passed` or `failed`.
    #[serde(default)]
    pub onboard: Option<String>,
}

/// The credential store of a picture: it takes the password and keeps nothing.
struct Pictured;

impl Keychain for Pictured {
    fn store(&self, _account: u64, _password: &str) -> Result<(), String> {
        Ok(())
    }

    fn forget(&self, _account: u64) -> Result<(), String> {
        Ok(())
    }
}

/// What Detect proposes in a picture: the invented provider's autoconfig file.
fn pictured_proposal() -> Proposal {
    let suggestion = |host: &str, port, security| Suggestion {
        host: host.to_owned(),
        port,
        security,
        username: "robin@example.org".to_owned(),
        source: Source::Autoconfig,
    };
    Proposal {
        imap: vec![suggestion("imap.example.org", 993, Security::Tls), suggestion("imap.example.org", 143, Security::StartTls)],
        asked: vec!["dns MX example.org".to_owned(), "https://autoconfig.example.org/mail/config-v1.1.xml".to_owned()],
        failed: Vec::new(),
    }
}

/// Puts the sample window on the add-account page in `state`.
fn onboard(app: InboxApp, out: &str, state: &str) -> Result<InboxApp, UiError> {
    let passed = |step| (step, StepStatus::Passed);
    let workers = match state {
        "form" | "detected" | "passed" => Scripted::passing(),
        "checking" => Scripted::answering(&[passed(CheckStep::Dns), passed(CheckStep::Tcp), passed(CheckStep::Tls)]).unfinished(),
        "failed" => Scripted::answering(&[
            passed(CheckStep::Dns),
            passed(CheckStep::Tcp),
            passed(CheckStep::Tls),
            (CheckStep::Certificate, StepStatus::Failed(ErrorKind::CertificateExpired)),
        ]),
        other => return Err(bad(format!("{out}: onboard {other:?} is not form, detected, checking, passed or failed"))),
    };
    let mut app = app.with_accounts(Arc::new(Pictured), Box::new(Proposed::new(pictured_proposal())), Box::new(workers));
    let (onboarding, mailbox) = app.onboarding_mut();
    onboarding.open();
    let form = onboarding.form_mut();
    form.address = "robin@example.org".to_owned();
    form.display_name = "Robin Example".to_owned();
    form.password = Secret::new("invented-pass");
    if state == "form" {
        form.host = "mail.example.org".to_owned();
        form.security = Some(Security::StartTls);
        return Ok(app);
    }
    onboarding.detect();
    onboarding.poll();
    if state != "detected" {
        onboarding.add(mailbox);
        app.pump();
    }
    Ok(app)
}

fn bad(detail: impl Into<String>) -> UiError {
    UiError::Shot(detail.into())
}

/// The pictures a script asks for. Blank lines and lines starting with `#` are skipped.
///
/// # Errors
///
/// [`UiError::Shot`] naming the first line that is not a picture.
pub fn parse_script(text: &str) -> Result<Vec<Shot>, UiError> {
    let mut shots = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let shot: Shot = serde_json::from_str(line).map_err(|error| bad(format!("line {}: {error}", index.saturating_add(1))))?;
        shots.push(shot);
    }
    Ok(shots)
}

/// An RGBA picture, eight bits a channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    width: usize,
    height: usize,
    rgba: Vec<u8>,
}

impl Picture {
    /// Its width in pixels.
    #[must_use]
    pub fn width(&self) -> usize {
        self.width
    }

    /// Its height in pixels.
    #[must_use]
    pub fn height(&self) -> usize {
        self.height
    }

    /// The colour at `x`, `y`, or `None` outside.
    #[must_use]
    pub fn pixel(&self, x: usize, y: usize) -> Option<Color32> {
        if x >= self.width {
            return None;
        }
        let at = y.checked_mul(self.width)?.checked_add(x)?.checked_mul(4)?;
        let px = self.rgba.get(at..at.checked_add(4)?)?;
        Some(Color32::from_rgba_premultiplied(*px.first()?, *px.get(1)?, *px.get(2)?, *px.get(3)?))
    }

    /// Writes it as PNG.
    ///
    /// # Errors
    ///
    /// [`UiError::Shot`] when the writer fails.
    pub fn write_png(&self, out: &mut dyn Write) -> Result<(), UiError> {
        let width = u32::try_from(self.width).map_err(|_| bad("too wide"))?;
        let height = u32::try_from(self.height).map_err(|_| bad("too high"))?;
        let mut encoder = png::Encoder::new(out, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|error| bad(error.to_string()))?;
        writer.write_image_data(&self.rgba).map_err(|error| bad(error.to_string()))?;
        writer.finish().map_err(|error| bad(error.to_string()))
    }
}

/// A texture egui asked for: premultiplied colours.
struct Texture {
    width: usize,
    height: usize,
    pixels: Vec<Color32>,
}

impl Texture {
    fn texel(&self, x: usize, y: usize) -> [f32; 4] {
        let x = x.min(self.width.saturating_sub(1));
        let y = y.min(self.height.saturating_sub(1));
        let colour = y.checked_mul(self.width).and_then(|row| row.checked_add(x)).and_then(|at| self.pixels.get(at)).copied().unwrap_or_default();
        unit(colour)
    }

    /// The colour at `uv` (0 to 1 across), filtered between the four nearest texels.
    fn sample(&self, uv: Pos2) -> [f32; 4] {
        let x = uv.x * self.width as f32 - 0.5;
        let y = uv.y * self.height as f32 - 0.5;
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let index = |value: f32| if value <= 0.0 { 0 } else { value as usize };
        let (ix, iy) = (index(x0), index(y0));
        let (jx, jy) = (if x0 < 0.0 { 0 } else { ix.saturating_add(1) }, if y0 < 0.0 { 0 } else { iy.saturating_add(1) });
        let (a, b, c, d) = (self.texel(ix, iy), self.texel(jx, iy), self.texel(ix, jy), self.texel(jx, jy));
        std::array::from_fn(|i| {
            let top = a[i] + (b[i] - a[i]) * fx;
            let bottom = c[i] + (d[i] - c[i]) * fx;
            top + (bottom - top) * fy
        })
    }
}

fn unit(colour: Color32) -> [f32; 4] {
    colour.to_array().map(|channel| f32::from(channel) / 255.0)
}

/// The canvas the triangles are filled into: premultiplied colours, 0 to 1.
struct Canvas {
    width: usize,
    height: usize,
    pixels: Vec<[f32; 4]>,
}

/// Twice the signed area of the triangle `a`, `b`, `p`: positive when `p` is on the inner side of `a` to `b` in a
/// clockwise triangle on a screen whose y grows downwards.
fn edge(a: Pos2, b: Pos2, p: Pos2) -> f32 {
    (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x)
}

/// Whether the edge `a` to `b` of a clockwise triangle is a top or a left edge, which owns the pixels exactly on it,
/// so two triangles that share an edge never both fill a pixel.
fn top_left(a: Pos2, b: Pos2) -> bool {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    dy < 0.0 || (dy == 0.0 && dx > 0.0)
}

fn covers(weight: f32, a: Pos2, b: Pos2) -> bool {
    weight > 0.0 || (weight == 0.0 && top_left(a, b))
}

impl Canvas {
    fn new(width: usize, height: usize) -> Self {
        Self { width, height, pixels: vec![[0.0; 4]; width.saturating_mul(height)] }
    }

    fn blend(&mut self, x: usize, y: usize, src: [f32; 4]) {
        if let Some(dst) = y.checked_mul(self.width).and_then(|row| row.checked_add(x)).and_then(|at| self.pixels.get_mut(at)) {
            let keep = 1.0 - src[3];
            *dst = std::array::from_fn(|i| src[i] + dst[i] * keep);
        }
    }

    /// Fills `mesh` (positions in points) inside `clip` (in pixels), scaled by `scale`.
    fn mesh(&mut self, mesh: &Mesh, texture: &Texture, clip: [usize; 4], scale: f32) {
        let (triangles, _) = mesh.indices.as_chunks::<3>();
        for [a, b, c] in triangles {
            let vertex = |at: &u32| usize::try_from(*at).ok().and_then(|i| mesh.vertices.get(i));
            if let (Some(a), Some(b), Some(c)) = (vertex(a), vertex(b), vertex(c)) {
                self.triangle([a, b, c], texture, clip, scale);
            }
        }
    }

    fn triangle(&mut self, vertices: [&egui::epaint::Vertex; 3], texture: &Texture, clip: [usize; 4], scale: f32) {
        let [a, mut b, mut c] = vertices;
        let mut area = edge(a.pos * scale, b.pos * scale, c.pos * scale);
        if area < 0.0 {
            std::mem::swap(&mut b, &mut c);
            area = -area;
        }
        if area <= 0.0 || !area.is_finite() {
            return;
        }
        let (pa, pb, pc) = (a.pos * scale, b.pos * scale, c.pos * scale);
        let [left, top, right, bottom] = clip;
        let first = |value: f32, low: usize| if value.is_finite() && value > low as f32 { value.floor() as usize } else { low };
        let last = |value: f32, high: usize| if value.is_finite() && value < high as f32 { value.ceil() as usize } else { high };
        let (x0, x1) = (first(pa.x.min(pb.x).min(pc.x), left), last(pa.x.max(pb.x).max(pc.x), right));
        let (y0, y1) = (first(pa.y.min(pb.y).min(pc.y), top), last(pa.y.max(pb.y).max(pc.y), bottom));
        let (ca, cb, cc) = (unit(a.color), unit(b.color), unit(c.color));
        for y in y0..y1 {
            for x in x0..x1 {
                let p = pos2(x as f32 + 0.5, y as f32 + 0.5);
                let (wa, wb, wc) = (edge(pb, pc, p), edge(pc, pa, p), edge(pa, pb, p));
                if !(covers(wa, pb, pc) && covers(wb, pc, pa) && covers(wc, pa, pb)) {
                    continue;
                }
                let (wa, wb, wc) = (wa / area, wb / area, wc / area);
                let uv = pos2(a.uv.x * wa + b.uv.x * wb + c.uv.x * wc, a.uv.y * wa + b.uv.y * wb + c.uv.y * wc);
                let texel = texture.sample(uv);
                let src = std::array::from_fn(|i| (ca[i] * wa + cb[i] * wb + cc[i] * wc) * texel[i]);
                self.blend(x, y, src);
            }
        }
    }

    fn picture(&self) -> Picture {
        let rgba = self.pixels.iter().flat_map(|pixel| pixel.map(|channel| (channel.clamp(0.0, 1.0) * 255.0).round() as u8)).collect();
        Picture { width: self.width, height: self.height, rgba }
    }
}

/// Fills `primitives` into a picture of `width` by `height` pixels with the `textures` egui set.
fn fill(primitives: &[ClippedPrimitive], textures: &HashMap<TextureId, Texture>, width: usize, height: usize, scale: f32) -> Picture {
    let mut canvas = Canvas::new(width, height);
    for primitive in primitives {
        let Primitive::Mesh(mesh) = &primitive.primitive else { continue };
        let Some(texture) = textures.get(&mesh.texture_id) else { continue };
        let rect = primitive.clip_rect;
        // Infinite edges (an unclipped primitive) stop at the picture's edge; the casts see 0 to `high` only.
        let px = |value: f32, high: usize| if value > 0.0 { (value * scale).round().min(high as f32) as usize } else { 0 };
        let clip = [px(rect.min.x, width), px(rect.min.y, height), px(rect.max.x, width), px(rect.max.y, height)];
        canvas.mesh(mesh, texture, clip, scale);
    }
    canvas.picture()
}

/// Applies and empties `delta` (egui insists every delta is handled).
fn apply_textures(textures: &mut HashMap<TextureId, Texture>, delta: &mut egui::TexturesDelta) {
    for (id, patches) in delta.set.drain() {
        for patch in patches {
            let ImageData::Color(image) = &patch.image;
            let [width, height] = image.size;
            match patch.pos {
                None => {
                    textures.insert(id, Texture { width, height, pixels: image.pixels.clone() });
                }
                Some([x, y]) => {
                    let Some(texture) = textures.get_mut(&id) else { continue };
                    for row in 0..height {
                        for column in 0..width {
                            let source = row.checked_mul(width).and_then(|r| r.checked_add(column)).and_then(|at| image.pixels.get(at));
                            let target = y
                                .checked_add(row)
                                .filter(|ty| *ty < texture.height)
                                .zip(x.checked_add(column).filter(|tx| *tx < texture.width))
                                .and_then(|(ty, tx)| ty.checked_mul(texture.width).and_then(|r| r.checked_add(tx)));
                            if let (Some(source), Some(target)) = (source, target.and_then(|at| texture.pixels.get_mut(at))) {
                                *target = *source;
                            }
                        }
                    }
                }
            }
        }
    }
    for id in delta.free.drain() {
        textures.remove(&id);
    }
}

/// Draws `app` for a few frames in a window of `size` points at `scale` pixels per point in `mode`, and fills the
/// last frame into a picture.
///
/// # Errors
///
/// [`UiError::Shot`] for a size of nothing, a scale outside 1 to 4, or a picture side over [`MAX_SIDE`].
pub fn render(app: &mut InboxApp, size: Vec2, scale: f32, mode: Mode) -> Result<Picture, UiError> {
    paint(size, scale, mode, |ui| app.ui(ui))
}

/// Runs `draw` for a few frames on a transparent window of `size` points at `scale` pixels per point in `mode`, and
/// fills the last frame into a picture.
///
/// # Errors
///
/// [`UiError::Shot`] for a size of nothing, a scale outside 1 to 4, or a picture side over [`MAX_SIDE`].
pub(crate) fn paint(size: Vec2, scale: f32, mode: Mode, mut draw: impl FnMut(&mut egui::Ui)) -> Result<Picture, UiError> {
    if !(1.0..=4.0).contains(&scale) {
        return Err(bad(format!("scale {scale} is not between 1 and 4")));
    }
    let side = |points: f32| {
        let pixels = (points * scale).round();
        if pixels.is_finite() && pixels >= 1.0 && pixels <= MAX_SIDE as f32 { Some(pixels as usize) } else { None }
    };
    let (Some(width), Some(height)) = (side(size.x), side(size.y)) else {
        return Err(bad(format!("a window of {} by {} points at scale {scale} is not 1 to {MAX_SIDE} pixels a side", size.x, size.y)));
    };
    let ctx = egui::Context::default();
    ctx.set_pixels_per_point(scale);
    ctx.set_theme(match mode {
        Mode::Light => egui::Theme::Light,
        Mode::Dark => egui::Theme::Dark,
    });
    let mut textures = HashMap::new();
    let mut last = None;
    for _ in 0..4 {
        let input = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)), ..RawInput::default() };
        let mut output = ctx.run_ui(input, &mut draw);
        apply_textures(&mut textures, &mut output.textures_delta);
        last = Some(output);
    }
    let Some(output) = last else { return Err(bad("no frame")) };
    let primitives = ctx.tessellate(output.shapes, output.pixels_per_point);
    Ok(fill(&primitives, &textures, width, height, output.pixels_per_point))
}

/// The sample window as `shot` asks for it.
///
/// # Errors
///
/// [`UiError::Shot`] for a mode, role, row or size the shot names wrongly; the errors of [`render`].
pub fn shoot(shot: &Shot) -> Result<Picture, UiError> {
    let mode = match shot.mode.as_str() {
        "light" => Mode::Light,
        "dark" => Mode::Dark,
        other => return Err(bad(format!("{}: mode {other:?} is not light or dark", shot.out))),
    };
    let time = Timestamp::from_unix(SHOT_TIME, 0).ok_or_else(|| bad("no time"))?;
    let session = sample::session(Box::new(FakeClock::new(time)))?;
    let mailbox = Mailbox::new(session, Box::new(FakeClock::new(time)));
    let mut app = InboxApp::with_outside(mailbox, Box::new(sample::Pictures::default()), Box::new(NoSave));
    if let Some(account) = shot.narrow {
        app.mailbox_mut().narrow(Some(account));
    }
    match &shot.open {
        Some(Open::Role(name)) => {
            let role = [Role::Inbox, Role::Drafts, Role::Sent, Role::Archive, Role::Junk, Role::Trash]
                .into_iter()
                .find(|role| role.label().eq_ignore_ascii_case(name))
                .ok_or_else(|| bad(format!("{}: no role {name:?}", shot.out)))?;
            app.mailbox_mut().open(Place::View(role));
        }
        Some(Open::Folder(id)) => app.mailbox_mut().open(Place::Folder(*id)),
        None => {}
    }
    if let Some(text) = &shot.search {
        app.search(text);
    }
    if let Some(subject) = &shot.select {
        let id = app.mailbox().messages().iter().find(|row| row.subject == *subject).map(|row| row.id);
        let Some(id) = id else { return Err(bad(format!("{}: no row with the subject {subject:?}", shot.out))) };
        app.mailbox_mut().select(Some(id));
        let (reader, mailbox) = app.reader_mut();
        match shot.pictures.as_deref() {
            Some("once") => reader.allow_once(mailbox, id),
            Some("always") => reader.allow_always(mailbox, id),
            Some(other) => return Err(bad(format!("{}: pictures {other:?} is not once or always", shot.out))),
            None => {}
        }
        reader.set_full_width(shot.full_width.unwrap_or(false));
        if shot.details == Some(true) {
            reader.toggle_details(id);
        }
        if shot.quotes == Some(true) {
            reader.toggle_quotes(id);
        }
    } else if shot.pictures.is_some() || shot.full_width.is_some() || shot.details.is_some() || shot.quotes.is_some() {
        return Err(bad(format!("{}: the reader settings need a selected row", shot.out)));
    }
    if let Some(state) = &shot.onboard {
        app = onboard(app, &shot.out, state)?;
    }
    let [width, height] = shot.size.unwrap_or([1200.0, 760.0]);
    render(&mut app, vec2(width, height), shot.scale.unwrap_or(1.0), mode)
}

#[cfg(test)]
#[path = "shot_tests.rs"]
mod tests;
