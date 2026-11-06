// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `cargo xtask side-by-side`: every `ui_shot` picture next to the mood sketch of its preset and mode.
//!
//! For each line of the shot script (`crates/ui-egui/shots.jsonl` unless `--script` names another) the picture at
//! `out` is read, the mood sketch `<preset>-<mode>.png` (`electric` when the line names no preset) is scaled to the
//! picture's height, and the two are written next to each other, with a grey gap, to `<out dir>/<picture name>`.
//! The sketches come from the owner and are never committed: they sit in `plan/mood/` (gitignored) unless `--mood`
//! names another folder, and the pairs go to `plan/side-by-side/` unless `--out` names another. A missing picture or
//! sketch is reported and fails the command; the other pairs are still written.

use std::fs;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};

use crate::{Error, count};

/// The shot script when `--script` names none.
const SCRIPT: &str = "crates/ui-egui/shots.jsonl";
/// The mood sketches when `--mood` names no folder.
const MOOD: &str = "plan/mood";
/// The pairs when `--out` names no folder.
const OUT: &str = "plan/side-by-side";
/// The width of the gap between the picture and the sketch, in pixels.
pub(crate) const GAP: usize = 16;
/// The colour of the gap: a mid grey that stands apart from light and dark pictures alike.
pub(crate) const GAP_COLOUR: [u8; 4] = [128, 128, 128, 255];
/// The longest side of a picture this reads or writes.
pub(crate) const MAX_SIDE: usize = 16_384;

/// The folders `side-by-side` works with; `None` is the default. Relative paths start at the workspace root.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Options {
    pub(crate) script: Option<PathBuf>,
    pub(crate) mood: Option<PathBuf>,
    pub(crate) out: Option<PathBuf>,
}

/// A picture as 8-bit RGBA rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Image {
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) rgba: Vec<u8>,
}

/// Writes one pair for every shot of the script and reports them to `out`. `Ok(false)` when a picture or a sketch
/// was missing.
pub(crate) fn gate(root: &Path, options: &Options, out: &mut dyn Write) -> Result<bool, Error> {
    let at = |path: &Option<PathBuf>, default: &str| root.join(path.as_deref().unwrap_or_else(|| Path::new(default)));
    let script = at(&options.script, SCRIPT);
    let mood = at(&options.mood, MOOD);
    let pairs = at(&options.out, OUT);
    let text = fs::read_to_string(&script).map_err(|source| Error::Io { path: script.clone(), source })?;
    let mut written = 0_usize;
    let mut passed = true;
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (shot, sketch) = sketch_for(line)
            .map_err(|message| Error::Config { path: script.clone(), message: format!("line {}: {message}", index.saturating_add(1)) })?;
        let shot_path = root.join(&shot);
        let sketch_path = mood.join(format!("{sketch}.png"));
        if !shot_path.is_file() {
            writeln!(out, "side-by-side: {shot}: no such picture, run `just shot` first")?;
            passed = false;
            continue;
        }
        if !sketch_path.is_file() {
            writeln!(out, "side-by-side: {}: no such mood sketch (the sketches are attachments on the planning issue)", sketch_path.display())?;
            passed = false;
            continue;
        }
        let Some(name) = shot_path.file_name() else { continue };
        let target = pairs.join(name);
        let pair = beside(&read(&shot_path)?, &read(&sketch_path)?)?;
        fs::create_dir_all(&pairs).map_err(|source| Error::Io { path: pairs.clone(), source })?;
        let bytes = encode(&pair, &target)?;
        fs::write(&target, bytes).map_err(|source| Error::Io { path: target.clone(), source })?;
        let shown = target.strip_prefix(root).unwrap_or(&target);
        writeln!(out, "side-by-side: {} ({shot} beside {sketch})", shown.display())?;
        written = written.saturating_add(1);
    }
    writeln!(out, "side-by-side: {} in {}", count(written, "picture"), pairs.strip_prefix(root).unwrap_or(&pairs).display())?;
    Ok(passed)
}

/// The picture a shot line writes and the name of its mood sketch, `<preset>-<mode>`.
pub(crate) fn sketch_for(line: &str) -> Result<(String, String), String> {
    let value: serde_json::Value = serde_json::from_str(line).map_err(|error| format!("not a shot: {error}"))?;
    let field = |name: &str| value.get(name).and_then(serde_json::Value::as_str);
    let shot = field("out").ok_or("the shot names no `out`")?;
    let mode = field("mode").ok_or("the shot names no `mode`")?;
    let preset = field("preset").unwrap_or("electric");
    let plain = |word: &str| !word.is_empty() && word.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if !plain(mode) || !plain(preset) {
        return Err(format!("the mode {mode:?} or the preset {preset:?} is not a plain word"));
    }
    Ok((shot.to_owned(), format!("{preset}-{mode}")))
}

/// `shot` with `sketch` scaled to its height on its right, a grey gap between them.
pub(crate) fn beside(shot: &Image, sketch: &Image) -> Result<Image, Error> {
    let too_large = || Error::Image { path: PathBuf::new(), message: "the pair is too large".to_owned() };
    let height = shot.height;
    let scaled_width = if sketch.height == 0 { 0 } else { sketch.width.saturating_mul(height).checked_div(sketch.height).unwrap_or(0).max(1) };
    let sketch = resize(sketch, scaled_width, height);
    let width = shot.width.checked_add(GAP).and_then(|w| w.checked_add(sketch.width)).filter(|w| *w <= MAX_SIDE).ok_or_else(too_large)?;
    let mut rgba = Vec::with_capacity(width.saturating_mul(height).saturating_mul(4));
    for y in 0..height {
        rgba.extend_from_slice(row(shot, y));
        for _ in 0..GAP {
            rgba.extend_from_slice(&GAP_COLOUR);
        }
        rgba.extend_from_slice(row(&sketch, y));
    }
    Ok(Image { width, height, rgba })
}

fn row(image: &Image, y: usize) -> &[u8] {
    let stride = image.width.saturating_mul(4);
    let start = y.saturating_mul(stride);
    image.rgba.get(start..start.saturating_add(stride)).unwrap_or_default()
}

/// `image` scaled to `width` by `height` pixels: each new pixel is the average of the area of the old picture it
/// covers, so a shrunk sketch keeps its colours without aliasing.
pub(crate) fn resize(image: &Image, width: usize, height: usize) -> Image {
    let wide = scale_axis(image, width, true);
    scale_axis(&wide, height, false)
}

/// Scales one axis of `image` to `size` pixels: the rows when `horizontal`, else the columns.
#[expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "sizes are capped at MAX_SIDE, channels at 0..=255"
)]
fn scale_axis(image: &Image, size: usize, horizontal: bool) -> Image {
    let (from, across) = if horizontal { (image.width, image.height) } else { (image.height, image.width) };
    let (width, height) = if horizontal { (size, across) } else { (across, size) };
    let mut rgba = vec![0_u8; width.saturating_mul(height).saturating_mul(4)];
    if from == 0 || size == 0 {
        return Image { width, height, rgba };
    }
    let step = from as f64 / size as f64;
    for to in 0..size {
        let (start, end) = (to as f64 * step, (to as f64 + 1.0) * step);
        for line in 0..across {
            let mut sum = [0.0_f64; 4];
            let mut weight = 0.0_f64;
            let mut source = start.floor() as usize;
            while (source as f64) < end && source < from {
                let cover = end.min(source as f64 + 1.0) - start.max(source as f64);
                let (x, y) = if horizontal { (source, line) } else { (line, source) };
                let at = y.saturating_mul(image.width).saturating_add(x).saturating_mul(4);
                for (total, value) in sum.iter_mut().zip(image.rgba.get(at..at.saturating_add(4)).unwrap_or_default()) {
                    *total += f64::from(*value) * cover;
                }
                weight += cover;
                source = source.saturating_add(1);
            }
            let (x, y) = if horizontal { (to, line) } else { (line, to) };
            let at = y.saturating_mul(width).saturating_add(x).saturating_mul(4);
            if let Some(pixel) = rgba.get_mut(at..at.saturating_add(4)) {
                for (channel, total) in pixel.iter_mut().zip(sum) {
                    *channel = if weight > 0.0 { (total / weight).round().clamp(0.0, 255.0) as u8 } else { 0 };
                }
            }
        }
    }
    Image { width, height, rgba }
}

fn read(path: &Path) -> Result<Image, Error> {
    let bytes = fs::read(path).map_err(|source| Error::Io { path: path.to_path_buf(), source })?;
    decode(&bytes, path)
}

/// The PNG `bytes` (read from `path`) as RGBA; refuses a picture with a side over [`MAX_SIDE`].
pub(crate) fn decode(bytes: &[u8], path: &Path) -> Result<Image, Error> {
    let bad = |message: String| Error::Image { path: path.to_path_buf(), message };
    let unreadable = |error: png::DecodingError| bad(format!("the PNG could not be read: {error}"));
    let mut decoder = png::Decoder::new_with_limits(Cursor::new(bytes), png::Limits { bytes: MAX_SIDE.saturating_mul(MAX_SIDE).saturating_mul(4) });
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let header = decoder.read_header_info().map_err(unreadable)?;
    let (width, height) = (usize::try_from(header.width).unwrap_or(usize::MAX), usize::try_from(header.height).unwrap_or(usize::MAX));
    if width > MAX_SIDE || height > MAX_SIDE {
        return Err(bad(format!("{width} by {height} pixels is larger than {MAX_SIDE} on a side")));
    }
    let mut reader = decoder.read_info().map_err(unreadable)?;
    let size = reader.output_buffer_size().ok_or_else(|| bad("the PNG is too large".to_owned()))?;
    let mut buffer = vec![0; size];
    let frame = reader.next_frame(&mut buffer).map_err(unreadable)?;
    let pixels = buffer.get(..frame.buffer_size()).unwrap_or_default();
    let rgba: Vec<u8> = match frame.color_type {
        png::ColorType::Rgba => pixels.to_vec(),
        png::ColorType::Rgb => pixels.as_chunks::<3>().0.iter().flat_map(|&[r, g, b]| [r, g, b, 255]).collect(),
        png::ColorType::GrayscaleAlpha => pixels.as_chunks::<2>().0.iter().flat_map(|&[v, a]| [v, v, v, a]).collect(),
        png::ColorType::Grayscale => pixels.iter().flat_map(|&v| [v, v, v, 255]).collect(),
        png::ColorType::Indexed => return Err(bad("the PNG palette was not expanded".to_owned())),
    };
    Ok(Image { width, height, rgba })
}

/// `image` as a PNG for `path`.
pub(crate) fn encode(image: &Image, path: &Path) -> Result<Vec<u8>, Error> {
    let bad = |error: png::EncodingError| Error::Image { path: path.to_path_buf(), message: format!("the PNG could not be written: {error}") };
    let side = |n: usize| u32::try_from(n).map_err(|_| Error::Image { path: path.to_path_buf(), message: "the picture is too large".to_owned() });
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, side(image.width)?, side(image.height)?);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(bad)?;
    writer.write_image_data(&image.rgba).map_err(bad)?;
    writer.finish().map_err(bad)?;
    Ok(bytes)
}

#[cfg(test)]
#[path = "side_by_side_tests.rs"]
mod tests;
