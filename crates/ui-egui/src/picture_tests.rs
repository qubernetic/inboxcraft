// SPDX-FileCopyrightText: Qubernetic and the InboxCraft contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use super::{MAX_PIXELS, MAX_SIDE, decode};

fn encoded(width: u32, height: u32, colour: png::ColorType, data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, width, height);
    encoder.set_color(colour);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header().unwrap().write_image_data(data).unwrap();
    out
}

/// A baseline grayscale JPEG of one 8 by 8 block of mid grey: every Huffman table holds one code, `0`.
pub(crate) fn tiny_jpeg() -> Vec<u8> {
    let mut out = vec![0xFF, 0xD8, 0xFF, 0xDB, 0x00, 0x43, 0x00];
    out.extend([1_u8; 64]);
    out.extend([0xFF, 0xC0, 0x00, 0x0B, 0x08, 0x00, 0x08, 0x00, 0x08, 0x01, 0x01, 0x11, 0x00]);
    for class in [0x00, 0x10] {
        out.extend([0xFF, 0xC4, 0x00, 0x14, class, 0x01]);
        out.extend([0_u8; 15]);
        out.push(0x00);
    }
    out.extend([0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3F, 0x00, 0x3F, 0xFF, 0xD9]);
    out
}

#[test]
fn a_png_becomes_rgba_whatever_its_colour_type() {
    let rgba = encoded(2, 1, png::ColorType::Rgba, &[255, 0, 0, 255, 0, 0, 255, 128]);
    let picture = decode(&rgba).unwrap();
    assert_eq!((picture.width, picture.height), (2, 1));
    assert_eq!(picture.rgba, [255, 0, 0, 255, 0, 0, 255, 128]);
    let rgb = decode(&encoded(1, 1, png::ColorType::Rgb, &[1, 2, 3])).unwrap();
    assert_eq!(rgb.rgba, [1, 2, 3, 255]);
    let grey = decode(&encoded(1, 1, png::ColorType::Grayscale, &[9])).unwrap();
    assert_eq!(grey.rgba, [9, 9, 9, 255]);
    let grey_alpha = decode(&encoded(1, 1, png::ColorType::GrayscaleAlpha, &[9, 7])).unwrap();
    assert_eq!(grey_alpha.rgba, [9, 9, 9, 7]);
}

#[test]
fn a_jpeg_becomes_rgba() {
    let picture = decode(&tiny_jpeg()).unwrap();
    assert_eq!((picture.width, picture.height), (8, 8));
    assert_eq!(picture.rgba.len(), 8 * 8 * 4);
    assert_eq!(picture.rgba.get(..4), Some([128, 128, 128, 255].as_slice()));
}

#[test]
fn what_is_no_png_or_jpeg_is_refused() {
    for bytes in [b"GIF89a....".as_slice(), b"<svg onload=alert(1)>", b"", b"\x89PNG\r\n\x1a\n", b"\xff\xd8\xff"] {
        assert!(decode(bytes).is_err(), "{bytes:?}");
    }
    let mut cut = tiny_jpeg();
    cut.truncate(cut.len() - 4);
    let _ = decode(&cut);
    let mut png = encoded(1, 1, png::ColorType::Rgb, &[1, 2, 3]);
    png.truncate(png.len() - 20);
    assert!(decode(&png).is_err());
}

#[test]
fn a_picture_too_large_to_show_is_refused_before_it_is_decoded() {
    // A header that claims a huge picture over a few bytes of data.
    let side = u32::try_from(MAX_SIDE).unwrap() + 1;
    let mut header = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut header, side, 1);
        encoder.set_color(png::ColorType::Grayscale);
        let _ = encoder.write_header().unwrap();
    }
    let error = decode(&header).unwrap_err();
    assert!(error.to_string().contains("too large"), "{error}");
    let square = u32::try_from(MAX_PIXELS.isqrt()).unwrap() + 1;
    let mut header = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut header, square, square);
        encoder.set_color(png::ColorType::Grayscale);
        let _ = encoder.write_header().unwrap();
    }
    assert!(decode(&header).unwrap_err().to_string().contains("too large"));
    let mut jpeg = tiny_jpeg();
    // The frame header's height and width (SOF0 after the quantisation table).
    let sof = jpeg.windows(2).position(|pair| pair == [0xFF, 0xC0]).unwrap();
    jpeg[sof + 5..sof + 9].copy_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]);
    assert!(decode(&jpeg).is_err());
}
