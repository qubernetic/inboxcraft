// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Pictures of a message (inline parts and, once the owner allowed them, remote pictures) decoded into RGBA.
//!
//! The bytes are the sender's and hostile: only PNG (`png`) and baseline or progressive JPEG (`zune-jpeg`) are read,
//! picked by their first bytes whatever the server or the part calls them; the size in the header is checked against
//! [`MAX_SIDE`] and [`MAX_PIXELS`] before anything is allocated; and a decoder that fails inside, despite being safe
//! Rust, becomes an error instead of taking the reader down.

use std::io::Cursor;
use std::panic::{AssertUnwindSafe, catch_unwind};

use zune_jpeg::JpegDecoder;
use zune_jpeg::zune_core::colorspace::ColorSpace;
use zune_jpeg::zune_core::options::DecoderOptions;

use crate::UiError;

/// The longest side of a picture shown, in pixels.
pub const MAX_SIDE: usize = 8192;

/// The most pixels of a picture shown (32 MiB as RGBA).
pub const MAX_PIXELS: usize = 8 * 1024 * 1024;

/// A decoded picture: `width` by `height` pixels, four bytes each (red, green, blue, alpha), row by row.
#[derive(Clone, PartialEq, Eq)]
pub struct Decoded {
    /// Its width in pixels.
    pub width: usize,
    /// Its height in pixels.
    pub height: usize,
    /// Its pixels.
    pub rgba: Vec<u8>,
}

impl std::fmt::Debug for Decoded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Decoded").field("width", &self.width).field("height", &self.height).finish_non_exhaustive()
    }
}

fn refused(detail: impl Into<String>) -> UiError {
    UiError::Picture(detail.into())
}

/// Whether a picture of `width` by `height` may be decoded.
fn fits(width: usize, height: usize) -> Result<(), UiError> {
    let pixels = width.saturating_mul(height);
    if width == 0 || height == 0 {
        Err(refused("the picture is empty"))
    } else if width > MAX_SIDE || height > MAX_SIDE || pixels > MAX_PIXELS {
        Err(refused(format!("the picture is too large to show ({width} by {height} pixels)")))
    } else {
        Ok(())
    }
}

/// `bytes` decoded, when they are a PNG or a JPEG of a size that may be shown.
///
/// # Errors
///
/// [`UiError::Picture`] for any other format, a picture too large, or bytes the decoder cannot read.
pub fn decode(bytes: &[u8]) -> Result<Decoded, UiError> {
    let decoder: fn(&[u8]) -> Result<Decoded, UiError> = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        decode_png
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        decode_jpeg
    } else {
        return Err(refused("only PNG and JPEG pictures are shown"));
    };
    catch_unwind(AssertUnwindSafe(|| decoder(bytes))).unwrap_or_else(|_| Err(refused("the picture could not be read")))
}

fn decode_png(bytes: &[u8]) -> Result<Decoded, UiError> {
    let unreadable = |error: png::DecodingError| refused(format!("the PNG could not be read: {error}"));
    let mut decoder = png::Decoder::new_with_limits(Cursor::new(bytes), png::Limits { bytes: MAX_PIXELS.saturating_mul(8) });
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let header = decoder.read_header_info().map_err(unreadable)?;
    let (width, height) = (usize::try_from(header.width).unwrap_or(usize::MAX), usize::try_from(header.height).unwrap_or(usize::MAX));
    fits(width, height)?;
    let mut reader = decoder.read_info().map_err(unreadable)?;
    let size = reader.output_buffer_size().ok_or_else(|| refused("the PNG is too large to show"))?;
    let mut buffer = vec![0; size];
    let frame = reader.next_frame(&mut buffer).map_err(unreadable)?;
    let pixels = buffer.get(..frame.buffer_size()).unwrap_or_default();
    let rgba: Vec<u8> = match frame.color_type {
        png::ColorType::Rgba => pixels.to_vec(),
        png::ColorType::Rgb => pixels.as_chunks::<3>().0.iter().flat_map(|&[r, g, b]| [r, g, b, 255]).collect(),
        png::ColorType::GrayscaleAlpha => pixels.as_chunks::<2>().0.iter().flat_map(|&[v, a]| [v, v, v, a]).collect(),
        png::ColorType::Grayscale => pixels.iter().flat_map(|&v| [v, v, v, 255]).collect(),
        png::ColorType::Indexed => return Err(refused("the PNG palette was not expanded")),
    };
    rgba_of(width, height, rgba)
}

fn decode_jpeg(bytes: &[u8]) -> Result<Decoded, UiError> {
    let unreadable = |error: zune_jpeg::errors::DecodeErrors| refused(format!("the JPEG could not be read: {error}"));
    let options = DecoderOptions::default().set_max_width(MAX_SIDE).set_max_height(MAX_SIDE).jpeg_set_out_colorspace(ColorSpace::RGBA);
    let mut decoder = JpegDecoder::new_with_options(Cursor::new(bytes), options);
    decoder.decode_headers().map_err(unreadable)?;
    let (width, height) = decoder.dimensions().ok_or_else(|| refused("the JPEG has no size"))?;
    fits(width, height)?;
    let size = decoder.output_buffer_size().ok_or_else(|| refused("the JPEG is too large to show"))?;
    let mut rgba = vec![0; size];
    decoder.decode_into(&mut rgba).map_err(unreadable)?;
    rgba_of(width, height, rgba)
}

fn rgba_of(width: usize, height: usize, rgba: Vec<u8>) -> Result<Decoded, UiError> {
    if rgba.len() != width.saturating_mul(height).saturating_mul(4) {
        return Err(refused("the picture has fewer pixels than its size"));
    }
    Ok(Decoded { width, height, rgba })
}

#[cfg(test)]
#[path = "picture_tests.rs"]
mod tests;
